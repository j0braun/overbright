//! What the view shows, apart from the mod: hiding and isolating props (H, Shift+H, Alt+H), the "this mod's
//! changes only" filter, the camera presets (Top, Isometric), **Go to** (a place, a prop by name, a race point)
//! and the saved views (a camera with its layers, kept per mod in `.editor/editor.toml`).
//!
//! All of it is view state. Nothing here is written to the mod's map: hiding a prop changes how the viewport
//! draws and picks it, never the placement, the undo history or the saved file.

use std::collections::{BTreeSet, HashSet};

use glam::{Vec2, Vec3};

use super::collision::Show;
use super::state::{EditorState, RacePoint};
use crate::draw::camera::Camera;
use crate::tools::map::edit::RacePart;

/// Which props the view leaves out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewFilter {
    /// Hidden with H.
    pub hidden: BTreeSet<usize>,
    /// Isolated with Shift+H: only these are drawn (None: no isolation).
    pub isolated: Option<BTreeSet<usize>>,
    /// View ▸ This mod's changes only: only the props the mod moved, deleted or added are drawn.
    pub mod_only: bool,
    /// Counts every change, so the viewport knows to bring the renderer up to date.
    pub generation: u64,
}

impl ViewFilter {
    /// Whether anything is left out.
    pub fn active(&self) -> bool {
        !self.hidden.is_empty() || self.isolated.is_some() || self.mod_only
    }

    /// Whether placement `i` is left out; `changed`: the mod changes it (see [`changed`]).
    pub fn hides(&self, i: usize, changed: bool) -> bool {
        self.hidden.contains(&i)
            || self.isolated.as_ref().is_some_and(|s| !s.contains(&i))
            || (self.mod_only && !changed)
    }

    /// H: hide `selection`.
    pub fn hide(&mut self, selection: &[usize]) {
        self.hidden.extend(selection.iter().copied());
        if let Some(iso) = self.isolated.as_mut() {
            for i in selection {
                iso.remove(i);
            }
        }
        self.generation += 1;
    }

    /// Shift+H: draw `selection` only.
    pub fn isolate(&mut self, selection: &[usize]) {
        for i in selection {
            self.hidden.remove(i);
        }
        self.isolated = Some(selection.iter().copied().collect());
        self.generation += 1;
    }

    /// Alt+H: draw everything again (the "this mod's changes only" filter stays as it is).
    pub fn show_all(&mut self) {
        self.hidden.clear();
        self.isolated = None;
        self.generation += 1;
    }

    pub fn set_mod_only(&mut self, on: bool) {
        self.mod_only = on;
        self.generation += 1;
    }

    /// Forget placements past the world's end (an undo took them away).
    pub fn prune(&mut self, placements: usize) {
        let before = (self.hidden.len(), self.isolated.as_ref().map(BTreeSet::len));
        self.hidden.retain(|&i| i < placements);
        if let Some(s) = self.isolated.as_mut() {
            s.retain(|&i| i < placements);
        }
        if before != (self.hidden.len(), self.isolated.as_ref().map(BTreeSet::len)) {
            self.generation += 1;
        }
    }
}

/// Whether the mod changes placement `i`: moved, deleted (its hidden flag) or added.
pub fn changed(s: &EditorState, i: usize) -> bool {
    let Some(p) = s.session.world.placements.get(i) else { return false };
    match s.session.base.placements.get(i) {
        Some(b) => b.transform != p.transform || b.flags != p.flags,
        None => true,
    }
}

/// Whether the view leaves placement `i` out.
pub fn view_hides(s: &EditorState, i: usize) -> bool {
    s.filter.active() && s.filter.hides(i, changed(s, i))
}

/// Every placement the view leaves out now.
pub fn hidden_set(s: &EditorState) -> HashSet<usize> {
    if !s.filter.active() {
        return HashSet::new();
    }
    (0..s.session.world.placements.len()).filter(|&i| view_hides(s, i)).collect()
}

/// H, Shift+H and Alt+H on the selection; the status line says what happened.
pub fn hide_selected(s: &mut EditorState) {
    if s.selection.is_empty() {
        s.status = "Select a prop first".into();
        return;
    }
    let n = s.selection.len();
    s.filter.hide(&s.selection.clone());
    s.selection.clear();
    s.status = format!("Hid {n} (view only: Alt+H shows all)");
}

pub fn isolate_selected(s: &mut EditorState) {
    if s.selection.is_empty() {
        s.status = "Select a prop first".into();
        return;
    }
    s.filter.isolate(&s.selection.clone());
    s.status = format!("Showing only {} (view only: Alt+H shows all)", s.selection.len());
}

pub fn show_all(s: &mut EditorState) {
    s.filter.show_all();
    s.status = "Showing every prop".into();
}

/// Where the camera goes next (the viewport takes it, as it holds the overview's state).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    /// The top-down overview (T).
    Top,
    /// A 45° view of the selection, or of what the camera looks at.
    Isometric,
    /// The selection, as F frames it.
    Frame,
    /// Near this point (z is the ground's when the viewport finds it).
    At(Vec2),
    /// A saved view's camera.
    Camera(Vec3, f32, f32),
}

/// A camera and its layers, saved under a name.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedView {
    pub name: String,
    pub camera: (Vec3, f32, f32),
    pub collision: Show,
    pub race_barriers: bool,
    pub mod_only: bool,
}

impl SavedView {
    pub fn take(name: &str, s: &EditorState, camera: &Camera) -> SavedView {
        SavedView {
            name: name.trim().to_string(),
            camera: (camera.position, camera.yaw, camera.pitch),
            collision: s.collision,
            race_barriers: s.race_barriers,
            mod_only: s.filter.mod_only,
        }
    }

    /// Bring the layers back and ask for the camera.
    pub fn recall(&self, s: &mut EditorState) {
        s.collision = self.collision;
        s.race_barriers = self.race_barriers;
        if s.filter.mod_only != self.mod_only {
            s.filter.set_mod_only(self.mod_only);
        }
        let (p, yaw, pitch) = self.camera;
        s.goal = Some(Goal::Camera(p, yaw, pitch));
        s.status = format!("View {}", self.name);
    }
}

/// Save the current view as `name` (the same name replaces); an empty name is refused.
pub fn save_view(s: &mut EditorState, name: &str, camera: &Camera) -> bool {
    let v = SavedView::take(name, s, camera);
    if v.name.is_empty() {
        return false;
    }
    match s.views.iter_mut().find(|o| o.name.eq_ignore_ascii_case(&v.name)) {
        Some(o) => *o = v,
        None => s.views.push(v),
    }
    true
}

/// A 45° camera over `target`, `distance` metres out.
pub fn isometric(target: Vec3, distance: f32) -> Camera {
    let dir = Vec3::new(-1.0, -1.0, 1.0).normalize();
    Camera::looking_at(target + dir * distance.max(40.0), target)
}

/// What a Go to query names.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Coordinates.
    At(Vec2),
    /// A prop by name: the one nearest the camera of `of` matches.
    Prop { index: usize, of: usize },
    /// A race point of the selected race.
    Race(RacePoint, Vec2),
}

fn numbers(q: &str) -> Option<Vec<f32>> {
    let parts: Vec<&str> =
        q.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    let v: Option<Vec<f32>> = parts.iter().map(|p| p.parse::<f32>().ok().filter(|v| v.is_finite())).collect();
    v.filter(|v| (2..=3).contains(&v.len()))
}

/// Read a Go to query: `x, y` (or `x y z`; the height is not used), `start`, `finish`, `checkpoint 3` /
/// `grid 2` (counted from 1, in the selected race), else part of a prop's name.
pub fn resolve(s: &EditorState, query: &str) -> Result<Target, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("Type coordinates, a prop's name or a race point".into());
    }
    if let Some(n) = numbers(q) {
        return Ok(Target::At(Vec2::new(n[0], n[1])));
    }
    let low = q.to_ascii_lowercase();
    let mut words = low.split_whitespace();
    let (head, num) = (words.next().unwrap_or(""), words.next());
    let part = match head {
        "start" => Some(RacePart::Start),
        "finish" => Some(RacePart::Finish),
        "checkpoint" | "cp" => Some(RacePart::Checkpoint),
        "grid" => Some(RacePart::Grid),
        _ => None,
    };
    if let Some(part) = part {
        let k = s.race.filter(|&k| k < s.session.map.races.len()).ok_or("Select a race first")?;
        let index = match (part, num) {
            (RacePart::Start | RacePart::Finish, _) => 0,
            (_, Some(n)) => n.parse::<usize>().ok().filter(|&n| n >= 1).ok_or("Count from 1: checkpoint 3")? - 1,
            (_, None) => 0,
        };
        let p = RacePoint { part, index };
        let at = super::races::point_xy(&s.session.map.races[k], p).ok_or_else(|| format!("This race has no {low}"))?;
        return Ok(Target::Race(p, at));
    }
    let world = &s.session.world;
    let name = |i: usize| super::props::name(world, i).to_ascii_lowercase();
    let live = |i: &usize| world.placements[*i].flags & crate::world::HIDDEN_FLAG == 0;
    let all: Vec<usize> = (0..world.placements.len()).filter(live).collect();
    let exact: Vec<usize> = all.iter().copied().filter(|&i| name(i) == low).collect();
    let found = if exact.is_empty() { all.into_iter().filter(|&i| name(i).contains(&low)).collect() } else { exact };
    let eye = s.camera.position;
    let dist = |i: &usize| world.placements[*i].transform.w_axis.truncate().distance_squared(eye);
    found
        .iter()
        .copied()
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .map(|index| Target::Prop { index, of: found.len() })
        .ok_or_else(|| format!("Nothing named \"{q}\""))
}

/// Carry out a Go to: select and frame a prop, or point the camera at the place.
pub fn go_to(s: &mut EditorState, query: &str) -> Result<(), String> {
    let t = resolve(s, query)?;
    match t {
        Target::At(at) => {
            s.goal = Some(Goal::At(at));
            s.status = format!("Went to {:.0}, {:.0}", at.x, at.y);
        }
        Target::Prop { index, of } => {
            s.selection = vec![index];
            s.mode = super::state::Mode::Props;
            s.goal = Some(Goal::Frame);
            let n = super::props::name(&s.session.world, index);
            s.status = if of > 1 { format!("Went to {n}, the nearest of {of}") } else { format!("Went to {n}") };
            // A prop the view hides would not show: bring it back.
            if view_hides(s, index) {
                s.filter.hidden.remove(&index);
                if let Some(iso) = s.filter.isolated.as_mut() {
                    iso.insert(index);
                }
                s.filter.generation += 1;
                if view_hides(s, index) {
                    s.filter.set_mod_only(false);
                }
            }
        }
        Target::Race(p, at) => {
            s.point = Some(p);
            s.mode = super::state::Mode::Races;
            s.goal = Some(Goal::At(at));
            s.status = format!("Went to the race point at {:.0}, {:.0}", at.x, at.y);
        }
    }
    Ok(())
}
