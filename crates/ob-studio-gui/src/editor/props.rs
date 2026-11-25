//! Props mode: the move / rotate handles in the viewport, the actions on
//! the selected placements (delete, duplicate, drop to ground, reset to
//! original, upright) and the properties panel of the selection.
//!
//! Every gesture is one [`Op`] run through [`run`] (one undo step): a
//! handle drag moves the world live and runs its op on release, a typed
//! value on Enter or when the field loses focus, a button at once. The mod
//! itself is only ever changed by ops; props another map mod added are
//! left out of a selection's op.

use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui};
use glam::{Mat3, Mat4, Vec3, Vec4};

use super::state::{EditorState, Panel, project};
use super::undo::run;
use crate::draw::camera::Camera;
use crate::launch::theme::{Rgb, THEME_A};
use crate::tools::map::edit::{self, PropState, Refused};
use crate::tools::map::ops::{Change, Op, PropKey};
use crate::world::World;

/// Default snap steps.
pub const SNAP_METRES: f32 = 0.25;
pub const SNAP_DEGREES: f32 = 15.0;
/// How near (screen points) the pointer must be to grab a handle.
const GRAB: f32 = 7.0;
/// The handles' length on screen, roughly (points).
const HANDLE_POINTS: f32 = 80.0;

/// A theme colour for egui.
pub fn colour(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// The axis colours: x red, y green, z blue (the usual editor convention).
const AXIS: [Color32; 3] =
    [Color32::from_rgb(0xff, 0x6b, 0x5e), Color32::from_rgb(0x8f, 0xe0, 0x6a), Color32::from_rgb(0x5e, 0x9b, 0xff)];

/// The tool the left button works with in the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select only.
    Select,
    /// Axis handles move the selection.
    #[default]
    Move,
    /// A ring turns the selection about z.
    Rotate,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Select, Tool::Move, Tool::Rotate];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "Q",
            Tool::Move => "W",
            Tool::Rotate => "E",
        }
    }
}

/// The part of the handles under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// A move axis: 0 x, 1 y, 2 z.
    Axis(usize),
    /// The rotate ring.
    Ring,
}

/// A handle drag in progress: the world moves live, one op on release.
#[derive(Clone, Debug)]
struct Drag {
    handle: Handle,
    from: Pos2,
    /// The placements as they were when the drag began.
    live: Live,
    pivot: Vec3,
    /// The ring's start angle (degrees) under the pointer.
    angle: f32,
    verb: &'static str,
    /// A Shift + move drag: the props copied (their keys), the copies
    /// being the dragged placements in the same order. The copy made at
    /// the start is taken back on release and made again where the drag
    /// ended, so the gesture is one undo step.
    copied: Option<Vec<PropKey>>,
}

/// Props mode: the tool, the snap settings and the properties panel.
#[derive(Clone, Debug)]
pub struct Props {
    pub tool: Tool,
    pub snap: bool,
    pub snap_metres: f32,
    pub snap_degrees: f32,
    drag: Option<Drag>,
    /// A typed or dragged value of the properties panel in progress.
    live: Option<Live>,
    /// The handle under the pointer this frame.
    pub hover: Option<Handle>,
}

impl Default for Props {
    fn default() -> Props {
        Props {
            tool: Tool::Move,
            snap: true,
            snap_metres: SNAP_METRES,
            snap_degrees: SNAP_DEGREES,
            drag: None,
            live: None,
            hover: None,
        }
    }
}

/// The solid name of placement `i`.
pub fn name(world: &World, i: usize) -> String {
    world.placements.get(i).and_then(|p| world.solids.get(p.solid)).map_or_else(|| format!("#{i}"), |s| s.name.clone())
}

/// Whether placement `i` is a knock-over prop (it carries its own
/// collision and is knocked about by cars).
pub fn knock_over(world: &World, i: usize) -> bool {
    world.smackables.iter().any(|p| p.placement == i)
}

/// What an action on the selection is called in the undo list: the one
/// prop's name, else the count.
#[cfg(test)]
pub(crate) fn label(s: &EditorState, verb: &str) -> String {
    label_of(&s.session.world, verb, &s.selection)
}

/// The pivot of the selection: the mean of the placements' origins.
pub fn pivot(s: &EditorState) -> Option<Vec3> {
    let pts: Vec<Vec3> = s.selection.iter().filter_map(|&i| edit::placement_parts(&s.session.world, i).map(|p| p.0)).collect();
    (!pts.is_empty()).then(|| pts.iter().sum::<Vec3>() / pts.len() as f32)
}

/// The mouse ray through screen point `at` of the viewport `rect`: the eye
/// and a unit direction.
pub fn screen_ray(s: &EditorState, rect: Rect, at: Pos2) -> (Vec3, Vec3) {
    let x = (at.x - rect.min.x) / rect.width().max(1.0) * 2.0 - 1.0;
    let y = 1.0 - (at.y - rect.min.y) / rect.height().max(1.0) * 2.0;
    let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
    let inv = s.camera.view_projection(aspect).inverse();
    let (far, near) = (inv * Vec4::new(x, y, 1.0, 1.0), inv * Vec4::new(x, y, 0.0, 1.0));
    let dir = (far.truncate() / far.w - near.truncate() / near.w).normalize_or(s.camera.forward());
    (s.camera.position, dir)
}

/// Where the ray through `at` meets the ground (1.5 km at most).
pub fn ground_under(s: &EditorState, rect: Rect, at: Pos2) -> Option<Vec3> {
    let (eye, dir) = screen_ray(s, rect, at);
    edit::ray_ground(&s.session.world, eye, dir, 1500.0)
}

/// The camera's heading (degrees, the game's convention).
pub fn camera_heading(s: &EditorState) -> f32 {
    let f = s.camera.forward();
    f.y.atan2(f.x).to_degrees()
}

/// The world length at `pivot` that is [`HANDLE_POINTS`] long on screen
/// in the viewport `rect` seen by `cam`.
fn handle_length(cam: &Camera, rect: Rect, pivot: Vec3) -> f32 {
    let d = cam.position.distance(pivot).max(0.5);
    // World metres per screen point at distance d; fov_y clamped only
    // against a degenerate camera, not to a size (the Top overview's is
    // about 2 degrees).
    let fov = cam.fov_y.clamp(1e-4, 3.0);
    d * 2.0 * (fov * 0.5).tan() / rect.height().max(1.0) * HANDLE_POINTS
}

/// Distance from `p` to the segment `a`–`b` (screen points).
fn to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (ab, ap) = (b - a, p - a);
    let t = (ap.dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// A turn of `degrees` about z through `pivot` applied to a position.
fn turn_about(p: Vec3, pivot: Vec3, degrees: f32) -> Vec3 {
    pivot + Mat3::from_rotation_z(degrees.to_radians()) * (p - pivot)
}

/// Wrap an angle difference into −180…180 degrees.
fn wrap(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

// ---- actions on the selection -------------------------------------------

/// The selected placements this mod can edit, with their keys. Another
/// mod's props are left out: an op group is made whole or not at all, so
/// one of them would refuse the rest.
fn editable(s: &EditorState) -> Vec<(usize, PropKey)> {
    s.selection.iter().filter_map(|&i| s.session.key_of(i).map(|k| (i, k))).collect()
}

/// What an action on placements `of` is called in the undo list: the one
/// prop's name, else the count.
fn label_of(world: &World, verb: &str, of: &[usize]) -> String {
    match of {
        [i] => format!("{verb} {}", name(world, *i)),
        sel => format!("{verb} {} props", sel.len()),
    }
}

/// Run `ops` on placements `of` as one undo step named for `verb`. None
/// (and the reason in the status bar) when refused or with nothing to do.
fn run_on(s: &mut EditorState, verb: &str, of: &[usize], ops: Vec<Op>) -> Option<Change> {
    if ops.is_empty() {
        if !s.selection.is_empty() {
            s.status = Refused::OtherMod.to_string();
        }
        return None;
    }
    let label = label_of(&s.session.world, verb, of);
    run(s, Op::Group { label, ops }).ok()
}

/// The placements the props an op added are in the world.
fn placements_of(s: &EditorState, added: &[PropKey]) -> Vec<usize> {
    added.iter().filter_map(|k| s.session.placement_of(k)).collect()
}

/// Delete the selection (Del).
pub fn delete_selection(s: &mut EditorState) {
    if s.selection.is_empty() {
        return;
    }
    let (of, ops): (Vec<usize>, Vec<Op>) = editable(s).into_iter().map(|(i, key)| (i, Op::Delete { key })).unzip();
    if run_on(s, "Delete", &of, ops).is_some() {
        s.selection.clear();
    }
}

/// Copy the selection (Ctrl+D): around `at` when given (the ground under
/// the pointer), else one box width to the camera's right. The copies
/// become the selection.
pub fn duplicate_selection(s: &mut EditorState, at: Option<Vec3>) {
    let Some(pivot) = pivot(s) else { return };
    let shift = match at {
        Some(at) => at - pivot,
        None => {
            let r = s
                .selection
                .iter()
                .filter_map(|&i| edit::placement_sphere(&s.session.world, i))
                .map(|c| c.1)
                .fold(1.0, f32::max);
            (s.camera.right() * r * 1.2).truncate().extend(0.0)
        }
    };
    let (of, ops): (Vec<usize>, Vec<Op>) = editable(s)
        .into_iter()
        .filter_map(|(i, key)| {
            let (p, _) = edit::placement_parts(&s.session.world, i)?;
            Some((i, Op::Duplicate { key, position: p + shift }))
        })
        .unzip();
    if let Some(c) = run_on(s, "Duplicate", &of, ops) {
        s.selection = placements_of(s, &c.added);
    }
}

/// Stand each selected placement on the ground under it (End).
pub fn drop_selection(s: &mut EditorState) {
    let (of, ops): (Vec<usize>, Vec<Op>) =
        editable(s).into_iter().map(|(i, key)| (i, Op::DropToGround { key })).unzip();
    if run_on(s, "Drop to ground", &of, ops).is_some_and(|c| !c.changed) {
        s.status = "No ground under the selection".into();
    }
}

/// Put the selection back as the original has it (the props this mod
/// moved or deleted; the others have nothing to go back to).
pub fn reset_selection(s: &mut EditorState) {
    let resettable = |s: &EditorState, i: usize| {
        let m = &s.session;
        matches!(edit::prop_state(&m.map, &m.folder, &m.world, &m.region, i), PropState::Moved | PropState::Deleted)
    };
    let (of, ops): (Vec<usize>, Vec<Op>) =
        editable(s).into_iter().filter(|(i, _)| resettable(s, *i)).map(|(i, key)| (i, Op::Reset { key })).unzip();
    if ops.is_empty() {
        if !s.selection.is_empty() {
            s.status = Refused::NotEdited.to_string();
        }
        return;
    }
    run_on(s, "Reset", &of, ops);
}

/// Stand the selection upright (heading kept).
pub fn upright_selection(s: &mut EditorState) {
    let (of, ops): (Vec<usize>, Vec<Op>) = editable(s)
        .into_iter()
        .filter_map(|(i, key)| {
            let (position, r) = edit::placement_parts(&s.session.world, i)?;
            Some((i, Op::Place { key, position, rotation: edit::upright(edit::heading_of(r)) }))
        })
        .unzip();
    run_on(s, "Upright", &of, ops);
}

/// Place a new prop of `solid` standing on the ground at `at`, facing
/// `heading`; it becomes the selection.
pub fn place_new(s: &mut EditorState, solid: &str, at: Vec3, heading: f32) -> Option<usize> {
    let op = Op::Add { solid: solid.into(), position: at, rotation: edit::upright(heading), id: None, ground: true };
    let c = run(s, op).ok()?;
    let i = placements_of(s, &c.added).first().copied();
    s.selection = i.into_iter().collect();
    i
}

/// A live edit of the selection: the placements are moved in the world
/// only (the viewer shows them at once, the mod is untouched) until
/// [`Live::finish`] puts them back and runs the result as one op.
#[derive(Clone, Debug, Default)]
struct Live {
    /// (placement, key, its transform before the edit), for the props
    /// this mod can edit, keyed before anything moved.
    starts: Vec<(usize, PropKey, Mat4)>,
}

impl Live {
    fn begin(s: &EditorState) -> Live {
        let w = &s.session.world;
        Live { starts: editable(s).into_iter().map(|(i, k)| (i, k, w.placements[i].transform)).collect() }
    }

    /// Each placement to `f(position, rotation)` of its start.
    fn set(&self, s: &mut EditorState, f: impl Fn(Vec3, Mat3) -> (Vec3, Mat3)) {
        for (i, _, t) in &self.starts {
            let (p, r) = crate::world::parts_of(*t);
            let (p, r) = f(p, r);
            s.session.world.set_placement_transform(*i, crate::world::transform_of(p, r));
        }
        s.session.world_changed = true;
    }

    /// Each placement by `f(position, rotation)` from where it is now.
    fn nudge(&self, s: &mut EditorState, f: impl Fn(Vec3, Mat3) -> (Vec3, Mat3)) {
        for (i, ..) in &self.starts {
            let Some((p, r)) = edit::placement_parts(&s.session.world, *i) else { continue };
            let (p, r) = f(p, r);
            s.session.world.set_placement_transform(*i, crate::world::transform_of(p, r));
        }
        s.session.world_changed = true;
    }

    /// Where each placement is now (its key, position, rotation); the
    /// placements back where they started.
    fn take_back(self, s: &mut EditorState) -> Vec<(usize, PropKey, Vec3, Mat3)> {
        let mut out = Vec::new();
        for (i, key, t) in self.starts {
            let Some(now) = s.session.world.placements.get(i).map(|p| p.transform) else { continue };
            if now != t {
                let (p, r) = crate::world::parts_of(now);
                out.push((i, key, p, r));
                s.session.world.set_placement_transform(i, t);
                s.session.world_changed = true;
            }
        }
        out
    }

    /// End the edit: the moved placements as one op named for `verb`.
    fn finish(self, s: &mut EditorState, verb: &str) -> Option<Change> {
        let moved = self.take_back(s);
        let of: Vec<usize> = moved.iter().map(|m| m.0).collect();
        let ops = moved.into_iter().map(|(_, key, position, rotation)| Op::Place { key, position, rotation }).collect();
        if of.is_empty() { None } else { run_on(s, verb, &of, ops) }
    }
}

impl Props {
    /// Whether a handle drag is in progress (the viewport leaves the left
    /// button alone meanwhile).
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn snapping(&self, ctx: &egui::Context) -> bool {
        self.snap && !ctx.input(|i| i.modifiers.alt)
    }

    /// The handles' world length at `pivot`: [`HANDLE_POINTS`] on screen,
    /// in any view (the Top overview's field of view is a couple of
    /// degrees, its camera kilometres away).
    fn handle_length(s: &EditorState, rect: Rect, pivot: Vec3) -> f32 {
        handle_length(&s.camera, rect, pivot)
    }

    /// The rotate ring's points (screen) around `pivot`.
    fn ring(s: &EditorState, rect: Rect, pivot: Vec3, radius: f32) -> Vec<Option<Pos2>> {
        (0..=48)
            .map(|k| {
                let a = k as f32 / 48.0 * std::f32::consts::TAU;
                project(s, rect, pivot + Vec3::new(a.cos(), a.sin(), 0.0) * radius)
            })
            .collect()
    }

    /// The handle under `p`.
    fn handle_at(&self, s: &EditorState, rect: Rect, pivot: Vec3, p: Pos2) -> Option<Handle> {
        let len = Self::handle_length(s, rect, pivot);
        let c = project(s, rect, pivot)?;
        match self.tool {
            Tool::Select => None,
            Tool::Move => (0..3)
                .filter_map(|k| {
                    let end = project(s, rect, pivot + Vec3::AXES[k] * len)?;
                    let d = to_segment(p, c, end);
                    (d < GRAB && end.distance(c) > 4.0).then_some((k, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| Handle::Axis(k)),
            Tool::Rotate => {
                let pts = Self::ring(s, rect, pivot, len);
                let near = pts.windows(2).filter_map(|w| Some(to_segment(p, w[0]?, w[1]?))).fold(f32::MAX, f32::min);
                (near < GRAB).then_some(Handle::Ring)
            }
        }
    }

    /// The angle (degrees) about `pivot` at which the ray through `p` meets
    /// the level plane through it.
    fn ring_angle(s: &EditorState, rect: Rect, pivot: Vec3, p: Pos2) -> Option<f32> {
        let (eye, dir) = screen_ray(s, rect, p);
        if dir.z.abs() < 1e-4 {
            return None;
        }
        let t = (pivot.z - eye.z) / dir.z;
        if t <= 0.0 {
            return None;
        }
        let h = eye + dir * t - pivot;
        Some(h.y.atan2(h.x).to_degrees())
    }

    /// The handles of the selection in the viewport `rect`: drawn, and
    /// dragged with the left button (Shift at the start of a move drag
    /// copies the selection first; Alt bypasses snapping). True while the
    /// pointer is on a handle or a drag runs: the viewport must not pick
    /// with that click.
    pub fn handles(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) -> bool {
        self.hover = None;
        let Some(pivot) = (match &self.drag {
            Some(d) if !s.selection.is_empty() => Some(match d.handle {
                Handle::Ring => d.pivot,
                Handle::Axis(_) => pivot(s).unwrap_or(d.pivot),
            }),
            _ => pivot(s),
        }) else {
            if let Some(d) = self.drag.take() {
                d.live.take_back(s);
            }
            return false;
        };
        let (pointer, pressed, down, released) = ctx.input(|i| {
            (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released())
        });
        // Continue or end a drag.
        if self.drag.is_some() {
            if let Some(p) = pointer {
                self.drag_to(s, ctx, rect, p);
            }
            if released || !down {
                let d = self.drag.take().unwrap();
                d.release(s);
            }
            self.draw(s, painter, rect, pivot);
            return true;
        }
        let over = pointer.filter(|p| rect.contains(*p)).and_then(|p| self.handle_at(s, rect, pivot, p));
        self.hover = over;
        if pressed && let (Some(h), Some(p)) = (over, pointer) {
            self.begin(s, ctx, rect, h, p, pivot);
        }
        self.draw(s, painter, rect, pivot);
        over.is_some() || self.drag.is_some()
    }

    fn begin(&mut self, s: &mut EditorState, ctx: &egui::Context, rect: Rect, handle: Handle, from: Pos2, pivot: Vec3) {
        let copy = handle != Handle::Ring && ctx.input(|i| i.modifiers.shift);
        let copied = if copy { copy_in_place(s) } else { None };
        let verb = match (copied.is_some(), handle) {
            (true, _) => "Copy",
            (_, Handle::Ring) => "Rotate",
            _ => "Move",
        };
        let angle = Self::ring_angle(s, rect, pivot, from).unwrap_or(0.0);
        self.drag = Some(Drag { handle, from, live: Live::begin(s), pivot, angle, verb, copied });
    }

    fn drag_to(&mut self, s: &mut EditorState, ctx: &egui::Context, rect: Rect, p: Pos2) {
        let snapping = self.snapping(ctx);
        let (step_m, step_deg) = (self.snap_metres, self.snap_degrees);
        let Some(d) = self.drag.as_ref() else { return };
        let pivot = d.pivot;
        match d.handle {
            Handle::Axis(k) => {
                let len = Self::handle_length(s, rect, pivot);
                let (Some(c), Some(e)) = (project(s, rect, pivot), project(s, rect, pivot + Vec3::AXES[k] * len))
                else {
                    return;
                };
                let axis = e - c;
                let screen_len = axis.length();
                if screen_len < 4.0 {
                    return;
                }
                let mut by = (p - d.from).dot(axis / screen_len) / screen_len * len;
                if snapping {
                    // The pivot lands on the grid along the axis.
                    by = edit::snap(pivot[k] + by, step_m) - pivot[k];
                }
                let delta = Vec3::AXES[k] * by;
                d.live.set(s, |p, r| (p + delta, r));
            }
            Handle::Ring => {
                let Some(a) = Self::ring_angle(s, rect, pivot, p) else { return };
                let mut turn = wrap(a - d.angle);
                if snapping {
                    turn = edit::snap(turn, step_deg);
                }
                d.live.set(s, |p, r| (turn_about(p, pivot, turn), edit::turned(r, turn)));
            }
        }
    }

    fn draw(&self, s: &EditorState, painter: &egui::Painter, rect: Rect, pivot: Vec3) {
        let Some(c) = project(s, rect, pivot) else { return };
        let len = Self::handle_length(s, rect, pivot);
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        match self.tool {
            Tool::Select => {}
            Tool::Move => {
                for k in 0..3 {
                    let Some(e) = project(s, rect, pivot + Vec3::AXES[k] * len) else { continue };
                    let hot = active == Some(Handle::Axis(k));
                    let col = if hot { colour(THEME_A.play) } else { AXIS[k] };
                    painter.line_segment([c, e], Stroke::new(if hot { 4.0 } else { 2.5 }, col));
                    // The arrow head.
                    let dir = (e - c).normalized();
                    let side = egui::vec2(-dir.y, dir.x) * 5.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![e + dir * 9.0, e + side, e - side],
                        col,
                        Stroke::NONE,
                    ));
                }
            }
            Tool::Rotate => {
                let hot = active == Some(Handle::Ring);
                let col = if hot { colour(THEME_A.play) } else { AXIS[2] };
                let pts = Self::ring(s, rect, pivot, len);
                for w in pts.windows(2) {
                    if let (Some(a), Some(b)) = (w[0], w[1]) {
                        painter.line_segment([a, b], Stroke::new(if hot { 3.5 } else { 2.0 }, col));
                    }
                }
                if let Some(d) = &self.drag
                    && let Some(&(i, ..)) = d.live.starts.first()
                    && let Some((_, r)) = edit::placement_parts(&s.session.world, i)
                {
                    painter.text(
                        c + egui::vec2(12.0, -12.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.1}°", edit::heading_of(r)),
                        egui::FontId::proportional(13.0),
                        colour(THEME_A.ink),
                    );
                }
            }
        }
        painter.circle_filled(c, 3.5, colour(THEME_A.play));
    }

    /// The Props keys (not while a text field has the keyboard or the
    /// camera flies): Q / W / E tools, Del delete, End drop to ground,
    /// Ctrl+D duplicate at `cursor` (the ground under the pointer), Esc
    /// clears the selection.
    pub fn keys(&mut self, s: &mut EditorState, ctx: &egui::Context, cursor: Option<Vec3>) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::{Key, Modifiers};
        let hosted = super::menu::hosted(ctx);
        let (q, w, e, del, end, dup, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Q),
                i.consume_key(Modifiers::NONE, Key::W),
                i.consume_key(Modifiers::NONE, Key::E),
                i.consume_key(Modifiers::NONE, Key::Delete),
                i.consume_key(Modifiers::NONE, Key::End),
                !hosted && i.consume_key(Modifiers::COMMAND, Key::D),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        for (hit, tool) in [(q, Tool::Select), (w, Tool::Move), (e, Tool::Rotate)] {
            if hit {
                self.tool = tool;
            }
        }
        if del {
            delete_selection(s);
        }
        if end {
            drop_selection(s);
        }
        if dup {
            duplicate_selection(s, cursor);
        }
        if esc && self.drag.is_none() {
            s.selection.clear();
        }
    }
}

impl Drag {
    /// The end of the drag: its result as one op.
    fn release(self, s: &mut EditorState) {
        let Some(originals) = self.copied else {
            self.live.finish(s, self.verb);
            return;
        };
        // Where each copy ended, then the copy taken back and made there.
        let copies: Vec<usize> = self.live.starts.iter().map(|st| st.0).collect();
        let ends: Vec<Option<Vec3>> =
            copies.iter().map(|&i| edit::placement_parts(&s.session.world, i).map(|p| p.0)).collect();
        self.live.take_back(s);
        s.session.undo();
        let ops: Vec<Op> = originals
            .into_iter()
            .zip(ends)
            .filter_map(|(key, at)| Some(Op::Duplicate { key, position: at? }))
            .collect();
        let of: Vec<usize> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Duplicate { key, .. } => s.session.placement_of(key),
                _ => None,
            })
            .collect();
        if let Some(c) = run_on(s, self.verb, &of, ops) {
            s.selection = placements_of(s, &c.added);
        }
    }
}

/// Copies of the selection at the same places, one undo step; the copies
/// become the selection (the start of a Shift + move drag). The keys of
/// the props copied, in the copies' order (None: nothing copied).
fn copy_in_place(s: &mut EditorState) -> Option<Vec<PropKey>> {
    let (keys, ops): (Vec<PropKey>, Vec<Op>) = editable(s)
        .into_iter()
        .filter_map(|(i, key)| {
            let (p, _) = edit::placement_parts(&s.session.world, i)?;
            Some((key.clone(), Op::Duplicate { key, position: p }))
        })
        .unzip();
    let of = s.selection.clone();
    let c = run_on(s, "Copy", &of, ops)?;
    s.selection = placements_of(s, &c.added);
    (s.selection.len() == keys.len()).then_some(keys)
}

/// A section heading in the panel.
fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong().italics().color(colour(THEME_A.lime)));
}

/// A small outlined tag.
pub fn tag(ui: &mut Ui, text: &str, col: Color32) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, col))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(RichText::new(text).small().color(col)));
}

/// Did a value field finish its edit this frame (drag released, Enter,
/// focus lost, or a single click step)?
fn finished(r: &egui::Response) -> bool {
    r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus())
}

impl Panel for Props {
    /// The properties of the selection.
    fn ui(&mut self, s: &mut EditorState, ui: &mut Ui) {
        if s.selection.is_empty()
            && let Some(live) = self.live.take()
        {
            live.take_back(s);
        }
        let Some(&first) = s.selection.first() else {
            ui.label(RichText::new("Nothing selected").color(colour(THEME_A.dim)));
            ui.label(
                RichText::new("Click a prop in the viewport or the tree, or drag one in from the prop browser.")
                    .small()
                    .color(colour(THEME_A.dim)),
            );
            return;
        };
        if first >= s.session.world.placements.len() {
            s.selection.clear();
            return;
        }
        let state = edit::prop_state(&s.session.map, &s.session.folder, &s.session.world, &s.session.region, first);
        let knock = knock_over(&s.session.world, first);
        ui.horizontal(|ui| {
            ui.label(RichText::new(name(&s.session.world, first)).strong().color(colour(THEME_A.ink)));
            tag(ui, if knock { "knock-over" } else { "static" }, colour(THEME_A.dim));
            if state != PropState::Original {
                let col = match state {
                    PropState::Deleted => colour(THEME_A.err),
                    PropState::OtherMod => colour(THEME_A.warn),
                    _ => colour(THEME_A.lime),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| tag(ui, state.name(), col));
            }
        });
        let origin = match state {
            PropState::Added(_) => "Added by this map mod".to_string(),
            PropState::OtherMod => "Added by another map mod".to_string(),
            _ => match s.session.world.placement_ref(&s.session.region, first) {
                Some(r) => format!(
                    "Original placement · section {} · at {:.2}, {:.2}, {:.2}",
                    r.section, r.at.x, r.at.y, r.at.z
                ),
                None => String::new(),
            },
        };
        ui.label(RichText::new(origin).small().color(colour(THEME_A.dim)));
        let editable = !matches!(state, PropState::OtherMod | PropState::Deleted);
        ui.separator();

        // Position: of the one prop, or the selection's pivot (all move).
        heading(ui, "Position (m)");
        let pivot = pivot(s).unwrap_or_default();
        let mut p = pivot;
        let mut done = false;
        ui.add_enabled_ui(editable, |ui| {
            ui.horizontal(|ui| {
                for (k, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.label(RichText::new(axis).color(AXIS[k]));
                    let r = ui.add(egui::DragValue::new(&mut p[k]).speed(0.05).fixed_decimals(2));
                    done |= finished(&r);
                }
            });
        });
        if p != pivot {
            let by = p - pivot;
            self.live.get_or_insert_with(|| Live::begin(s)).nudge(s, |q, r| (q + by, r));
        }
        if done && let Some(live) = self.live.take() {
            live.finish(s, "Move");
        }

        // Rotation.
        heading(ui, "Rotation");
        let (_, rot) = edit::placement_parts(&s.session.world, first).unwrap_or((Vec3::ZERO, Mat3::IDENTITY));
        let h0 = edit::heading_of(rot);
        let mut h = h0;
        let mut done = false;
        ui.add_enabled_ui(editable, |ui| {
            egui::Grid::new("prop-rotation").num_columns(2).show(ui, |ui| {
                ui.label("Heading");
                let r =
                    ui.add(egui::DragValue::new(&mut h).speed(0.5).range(-180.0..=180.0).suffix("°").fixed_decimals(1));
                done |= finished(&r);
                ui.end_row();
                ui.label("Tilt");
                let t = edit::tilt_of(rot);
                ui.horizontal(|ui| {
                    ui.label(format!("{:.1}°, {:.1}°", t.x, t.y));
                    if ui.button("Upright").on_hover_text("Stand it straight, heading kept").clicked() {
                        upright_selection(s);
                    }
                });
                ui.end_row();
                ui.label("Ground");
                match edit::ground_gap(&s.session.world, first) {
                    Some(g) if g.abs() < 0.05 => tag(ui, "on the ground", colour(THEME_A.ok)),
                    Some(g) if g > 0.0 => tag(ui, &format!("{g:.2} m above the ground"), colour(THEME_A.warn)),
                    Some(g) => tag(ui, &format!("{:.2} m in the ground", -g), colour(THEME_A.warn)),
                    None => tag(ui, "no floor under it", colour(THEME_A.warn)),
                }
                ui.end_row();
            });
        });
        if h != h0 {
            let turn = h - h0;
            self.live.get_or_insert_with(|| Live::begin(s)).nudge(s, |q, r| (q, edit::turned(r, turn)));
        }
        if done && let Some(live) = self.live.take() {
            live.finish(s, "Rotate");
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let resettable = matches!(state, PropState::Moved | PropState::Deleted);
            if ui.add_enabled(resettable, egui::Button::new("Reset to original")).clicked() {
                reset_selection(s);
            }
            if ui.add_enabled(editable, egui::Button::new("Duplicate")).on_hover_text(super::menu::duplicate_key(ui.ctx())).clicked() {
                duplicate_selection(s, None);
            }
            if ui.add_enabled(editable, egui::Button::new("Delete")).on_hover_text("Del").clicked() {
                delete_selection(s);
            }
        });
        ui.separator();
        heading(ui, "Collision");
        ui.label(RichText::new(if knock {
            "Knock-over props carry their own collision: it moves with the prop."
        } else {
            "Static scenery keeps its collision where the original has it: a moved or deleted piece still blocks there."
        })
        .small()
        .color(colour(THEME_A.dim)));
        if s.selection.len() > 1 {
            heading(ui, &format!("Selection ({})", s.selection.len()));
            ui.label(
                RichText::new("Ctrl+click adds. Moves and turns apply to all, around their centre.")
                    .small()
                    .color(colour(THEME_A.dim)),
            );
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::draw::camera::Camera;
    use crate::tools::map::{MapMod, PropEdit};

    /// A flat floor at z = 0 with two cones standing on it and a barrier
    /// 2 m above it (the library's `tools::map::edit` tests use the same).
    fn world() -> crate::world::World {
        use glam::Mat4;
        let solid = |name: &str| crate::formats::solid::Solid {
            name: name.into(),
            hash: crate::formats::hash::bin_hash(name),
            bbox_min: [-0.5, -0.5, 0.0],
            bbox_max: [0.5, 0.5, 1.0],
            textures: Vec::new(),
            vertices: Vec::new(),
            normals: Vec::new(),
            light_materials: Vec::new(),
            indices: Vec::new(),
            materials: Vec::new(),
            mesh_flags: 0,
            markers: Vec::new(),
            light_markers: Vec::new(),
        };
        let mut w = crate::world::World::flat(200.0);
        w.solids = vec![solid("XC_CONE_01"), solid("XC_BARRIER_03")];
        let place = |solid, at: Vec3| crate::world::Placement {
            solid,
            low_solid: None,
            mid_solid: None,
            transform: Mat4::from_translation(at),
            section: 618,
            flags: 0,
            cull_radius: 5.0,
        };
        w.placements = vec![
            place(0, Vec3::new(10.0, 0.0, 0.0)),
            place(0, Vec3::new(20.0, 0.0, 0.0)),
            place(1, Vec3::new(30.0, 0.0, 2.0)),
        ];
        w
    }

    pub(crate) fn state() -> EditorState {
        let world = world();
        let camera = Camera::looking_at(Vec3::new(0.0, -20.0, 10.0), Vec3::new(10.0, 0.0, 0.0));
        EditorState::new("root".into(), "L4RA", "maps".into(), "my-map", MapMod::default(), world, camera)
    }

    #[test]
    fn selection_actions_are_one_undo_step_each() {
        let mut s = state();
        let n = s.session.world.placements.len();
        // The floating barrier dropped to the ground.
        s.selection = vec![2];
        drop_selection(&mut s);
        assert!(s.session.world.placements[2].transform.w_axis.z.abs() < 1e-4);
        assert_eq!(s.session.undo.undo_label(), Some("Drop to ground XC_BARRIER_03"));
        // Two cones copied 5 m along y: the copies are selected.
        s.selection = vec![0, 1];
        duplicate_selection(&mut s, Some(Vec3::new(15.0, 5.0, 0.0)));
        assert_eq!(s.session.world.placements.len(), n + 2);
        assert_eq!(s.selection.len(), 2);
        assert!(s.selection.iter().all(|&i| i >= n));
        let at: Vec<Vec3> = s.selection.iter().map(|&i| s.session.world.placements[i].transform.w_axis.truncate()).collect();
        assert!(at.contains(&Vec3::new(10.0, 5.0, 0.0)) && at.contains(&Vec3::new(20.0, 5.0, 0.0)), "{at:?}");
        assert!(s.session.adds_changed);
        // Deleting a copy removes its add; deleting an original hides it.
        s.selection = vec![s.selection[0], 0];
        delete_selection(&mut s);
        assert_eq!(s.session.world.placements.len(), n + 1);
        assert!(s.session.map.props.iter().any(|e| matches!(e, PropEdit::Delete { .. })));
        assert!(s.selection.is_empty());
        // Undo takes back the delete only.
        assert!(crate::editor::undo::undo(&mut s));
        assert_eq!(s.session.world.placements.len(), n + 2);
        assert!(s.session.world.placements[0].flags & crate::world::HIDDEN_FLAG == 0);
        // Reset the dropped barrier: back 2 m up, its edit gone.
        s.selection = vec![2];
        reset_selection(&mut s);
        assert_eq!(s.session.world.placements[2].transform.w_axis.z, 2.0);
        assert!(!s.session.map.props.iter().any(|e| matches!(e, PropEdit::Move { .. })));
    }

    #[test]
    fn a_new_prop_stands_on_the_ground_facing_the_heading() {
        let mut s = state();
        let i = place_new(&mut s, "XC_CONE_01", Vec3::new(5.0, 5.0, 0.3), 90.0).unwrap();
        assert_eq!(s.selection, vec![i]);
        let (p, r) = edit::placement_parts(&s.session.world, i).unwrap();
        assert!(p.z.abs() < 1e-4 && (edit::heading_of(r) - 90.0).abs() < 1e-3, "{p}");
        assert_eq!(s.session.undo.undo_label(), Some("Add XC_CONE_01"));
        assert!(place_new(&mut s, "NO_SUCH", Vec3::ZERO, 0.0).is_none());
    }

    /// One frame of the handles in a 1600 x 960 viewport with `events`.
    fn frame(p: &mut Props, s: &mut EditorState, ctx: &egui::Context, shift: bool, events: Vec<egui::Event>) -> bool {
        let rect = viewport();
        let mods = egui::Event::ModifiersChanged(egui::Modifiers { shift, ..Default::default() });
        let events = std::iter::once(mods).chain(events).collect();
        let input = egui::RawInput { screen_rect: Some(rect), events, ..Default::default() };
        let mut taken = false;
        let _ = crate::editor::run_headless(ctx, input, |ui| taken = p.handles(s, ui.ctx(), ui.painter(), rect));
        taken
    }

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0))
    }

    fn button(at: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// Press at `from`, move to `to`, release: a handle drag.
    fn drag(p: &mut Props, s: &mut EditorState, shift: bool, from: Pos2, to: Pos2) {
        let ctx = egui::Context::default();
        frame(p, s, &ctx, shift, vec![egui::Event::PointerMoved(from)]);
        assert!(frame(p, s, &ctx, shift, vec![button(from, true)]), "no handle at {from:?}");
        assert!(p.dragging());
        frame(p, s, &ctx, shift, vec![egui::Event::PointerMoved(to)]);
        frame(p, s, &ctx, shift, vec![button(to, false)]);
        assert!(!p.dragging());
    }

    /// The Top overview over the test props.
    fn top(s: &mut EditorState) {
        let t = crate::editor::camera::TopView { centre: glam::Vec2::new(10.0, 0.0), half_height: 50.0 };
        s.camera = t.camera();
    }

    #[test]
    fn handles_are_their_screen_size_in_every_view() {
        let mut s = state();
        let rect = viewport();
        let pivot = Vec3::new(10.0, 0.0, 0.0);
        for look in ["perspective", "top"] {
            if look == "top" {
                top(&mut s);
            }
            let len = Props::handle_length(&s, rect, pivot);
            let (c, e) = (project(&s, rect, pivot).unwrap(), project(&s, rect, pivot + Vec3::X * len).unwrap());
            // The x axis seen square on in Top, at a slant in the 3D view.
            let on_screen = c.distance(e);
            assert!(on_screen <= HANDLE_POINTS + 1.0 && on_screen > HANDLE_POINTS * 0.5, "{look}: {on_screen}");
            if look == "top" {
                assert!((on_screen - HANDLE_POINTS).abs() < 1.0, "{on_screen}");
            }
        }
    }

    #[test]
    fn a_top_view_drag_moves_a_prop_as_one_op() {
        let mut s = state();
        top(&mut s);
        s.selection = vec![0];
        let mut p = Props::default();
        let rect = viewport();
        let pivot = Vec3::new(10.0, 0.0, 0.0);
        let len = Props::handle_length(&s, rect, pivot);
        let c = project(&s, rect, pivot).unwrap();
        let mid = project(&s, rect, pivot + Vec3::X * len * 0.6).unwrap();
        // 96 points to the right: 10 m at 50 m from the middle to the top.
        drag(&mut p, &mut s, false, mid, mid + egui::vec2(96.0, 0.0));
        let (at, _) = edit::placement_parts(&s.session.world, 0).unwrap();
        assert!((at.x - 20.0).abs() < 0.01 && at.y == 0.0 && at.z == 0.0, "{at} from {c:?}");
        assert_eq!(edit::snap(at.x, SNAP_METRES), at.x, "snapped");
        assert_eq!(s.session.undo.undo_label(), Some("Move XC_CONE_01"));
        assert!(matches!(s.session.map.props.as_slice(), [PropEdit::Move { position, .. }] if *position == at));
        // One step: undo puts it back where it stood.
        assert!(crate::editor::undo::undo(&mut s));
        assert_eq!(edit::placement_parts(&s.session.world, 0).unwrap().0, pivot);
        assert!(s.session.map.props.is_empty() && s.session.undo.undo_label().is_none());
    }

    #[test]
    fn a_ring_drag_turns_and_a_shift_drag_copies_in_one_step() {
        let mut s = state();
        top(&mut s);
        s.selection = vec![0];
        let rect = viewport();
        let pivot = Vec3::new(10.0, 0.0, 0.0);
        let len = Props::handle_length(&s, rect, pivot);
        let mut p = Props { tool: Tool::Rotate, ..Props::default() };
        let east = project(&s, rect, pivot + Vec3::X * len).unwrap();
        let north = project(&s, rect, pivot + Vec3::Y * len).unwrap();
        drag(&mut p, &mut s, false, east, north);
        let (at, r) = edit::placement_parts(&s.session.world, 0).unwrap();
        assert!(
            (edit::heading_of(r) - 90.0).abs() < 1e-3 && at.abs_diff_eq(pivot, 1e-4),
            "{at} {}",
            edit::heading_of(r)
        );
        assert_eq!(s.session.undo.undo_label(), Some("Rotate XC_CONE_01"));

        // Shift + move: a copy where the drag ends, the original kept.
        let n = s.session.world.placements.len();
        let steps = |s: &EditorState| {
            let mut u = s.session.undo.clone();
            std::iter::from_fn(|| u.undo()).count()
        };
        let before = steps(&s);
        p.tool = Tool::Move;
        let mid = project(&s, rect, pivot + Vec3::X * len * 0.6).unwrap();
        drag(&mut p, &mut s, true, mid, mid + egui::vec2(96.0, 0.0));
        assert_eq!(s.session.world.placements.len(), n + 1);
        assert_eq!(steps(&s), before + 1, "one step");
        assert_eq!(s.session.undo.undo_label(), Some("Copy XC_CONE_01"));
        assert_eq!(s.selection, vec![n]);
        let (copy, cr) = edit::placement_parts(&s.session.world, n).unwrap();
        assert!((copy.x - 20.0).abs() < 0.01 && (edit::heading_of(cr) - 90.0).abs() < 1e-3, "{copy}");
        assert!(edit::placement_parts(&s.session.world, 0).unwrap().0.abs_diff_eq(pivot, 1e-4));
        assert!(matches!(s.session.map.props.last(), Some(PropEdit::Add { position, .. }) if *position == copy));
    }

    #[test]
    fn the_panel_moves_by_ops_and_another_mods_props_are_left_out() {
        let mut s = state();
        // Placement 3: added by another mod (not this one's to edit).
        let i = s.session.world.add_placement("XC_CONE_01", glam::Mat4::from_translation(Vec3::Y * 5.0)).unwrap();
        s.session.world.map_overlay.added.insert(i, ("other".into(), "a1".into()));
        s.selection = vec![i, 1];
        assert_eq!(s.session.key_of(i), None);
        delete_selection(&mut s);
        assert_eq!(s.session.undo.undo_label(), Some("Delete XC_CONE_01"), "{}", s.status);
        assert!(matches!(s.session.map.props.as_slice(), [PropEdit::Delete { .. }]));
        assert!(s.selection.is_empty());
        assert!(crate::editor::undo::undo(&mut s));

        // Live: the world moves, the mod waits for the end of the edit.
        s.selection = vec![0, 1];
        let live = Live::begin(&s);
        live.nudge(&mut s, |p, r| (p + Vec3::Z, r));
        assert_eq!(s.session.world.placements[0].transform.w_axis.z, 1.0);
        assert!(s.session.map.props.is_empty());
        live.finish(&mut s, "Move");
        assert_eq!(s.session.undo.undo_label(), Some("Move 2 props"));
        assert_eq!(s.session.map.props.len(), 2);
        assert_eq!(s.session.world.placements[1].transform.w_axis.z, 1.0);
        // Upright and reset are ops too; reset leaves the unedited alone.
        s.selection = vec![0, 2];
        reset_selection(&mut s);
        assert_eq!(s.session.undo.undo_label(), Some("Reset XC_CONE_01"));
        assert_eq!(s.session.world.placements[0].transform.w_axis.z, 0.0);
        s.selection = vec![2];
        reset_selection(&mut s);
        assert_eq!(s.status, Refused::NotEdited.to_string());
    }

    #[test]
    fn angles_wrap_and_turn_about_a_pivot() {
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert_eq!(wrap(45.0), 45.0);
        let p = turn_about(Vec3::new(2.0, 1.0, 5.0), Vec3::new(1.0, 1.0, 0.0), 90.0);
        assert!(p.abs_diff_eq(Vec3::new(1.0, 2.0, 5.0), 1e-5), "{p}");
        // A position and its rotation turn the same way.
        let r = edit::turned(Mat3::IDENTITY, 90.0);
        assert!(r.row(0).abs_diff_eq(turn_about(Vec3::X, Vec3::ZERO, 90.0), 1e-5));
    }

    #[test]
    fn a_point_near_a_segment() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0));
        assert_eq!(to_segment(Pos2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(to_segment(Pos2::new(-4.0, 3.0), a, b), 5.0);
    }
}
