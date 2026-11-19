//! Races mode: the race being edited, its properties, the tools that put
//! its start line, grid, checkpoints and finish on the road, the route
//! preview drawn over the viewport and the 2D map, and the validation
//! behind it ([`super::validate`]).
//!
//! Every tool works on a world point (the viewport's ground point under
//! the cursor, or the map view's x/y) and snaps it to the base track's
//! road network unless snapping is off or Alt is held. Each gesture ends
//! in one undo step.

use std::collections::HashMap;

use glam::{Vec2, Vec3};

use super::state::{EditorState, Mode, Panel, RacePoint, commit, project};
use super::validate::{self, Check, Tested};
use crate::formats::roads::RoadNet;
use crate::frontend::career::Track;
use crate::tools::map::edit::RacePart;
use crate::tools::map::race::{self, RoadPoint};
use crate::tools::map::{CustomRace, MAX_CHECKPOINTS, MAX_GRID, MAX_LAPS, MapMod, RaceKind, Spot};

/// A click this close (metres) to a race point picks it instead of placing.
pub const PICK_M: f32 = 5.0;
/// The start and finish lines are drawn this wide.
const LINE_HALF_M: f32 = 7.0;

/// The race tools (C / G / S / X; Select picks and drags only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    Select,
    #[default]
    Checkpoint,
    Grid,
    Start,
    Finish,
}

impl Tool {
    pub const ALL: [(Tool, &'static str, &'static str); 5] = [
        (Tool::Select, "Select", "Q"),
        (Tool::Checkpoint, "Checkpoint", "C"),
        (Tool::Grid, "Grid", "G"),
        (Tool::Start, "Start", "S"),
        (Tool::Finish, "Finish", "X"),
    ];
}

/// A base track a race can be on.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackInfo {
    pub number: u16,
    pub name: String,
}

/// A race's route as drawn: the whole road when it builds, else each leg's.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    /// The race road on the ground (x, y, height), when the route builds.
    pub road: Option<Vec<Vec3>>,
    /// Each leg's road (None: no road through it).
    pub legs: Vec<Option<Vec<Vec2>>>,
    pub length: f32,
}

/// Races mode's own state; the shell keeps one and calls its parts.
#[derive(Default)]
pub struct Races {
    pub tool: Tool,
    /// Snap placed points to the road network.
    pub snap: bool,
    /// The region's race tracks (read on first use from the career data).
    tracks: Option<Vec<TrackInfo>>,
    records: HashMap<u16, Track>,
    tested: Option<Tested>,
    /// What the checks and previews were made from.
    seen: Option<(MapMod, Vec<u16>)>,
    pub checks: Vec<Check>,
    pub previews: Vec<Preview>,
    /// The point being dragged.
    dragging: Option<RacePoint>,
    /// A text field being typed into (committed on focus loss).
    typing: bool,
}

/// The x/y of a race point (grid slots of a race without its own grid:
/// the default grid's).
pub fn point_xy(r: &CustomRace, p: RacePoint) -> Option<Vec2> {
    match p.part {
        RacePart::Checkpoint => r.checkpoints.get(p.index).copied(),
        RacePart::Grid => race::effective_grid(r).get(p.index).map(|g| g.position.truncate()),
        RacePart::Start => r.start.map(|s| s.position.truncate()),
        RacePart::Finish => r.finish.map(|s| s.position.truncate()),
    }
}

/// Every point of a race, for picking.
pub fn points(r: &CustomRace) -> Vec<(RacePoint, Vec2)> {
    let mut out: Vec<(RacePoint, Vec2)> = r
        .checkpoints
        .iter()
        .enumerate()
        .map(|(i, &c)| (RacePoint { part: RacePart::Checkpoint, index: i }, c))
        .collect();
    out.extend(
        race::effective_grid(r)
            .iter()
            .enumerate()
            .map(|(i, g)| (RacePoint { part: RacePart::Grid, index: i }, g.position.truncate())),
    );
    out.extend(r.start.map(|s| (RacePoint { part: RacePart::Start, index: 0 }, s.position.truncate())));
    out.extend(r.finish.map(|s| (RacePoint { part: RacePart::Finish, index: 0 }, s.position.truncate())));
    out
}

/// The race point within `radius` of `at`, nearest first.
pub fn pick(r: &CustomRace, at: Vec2, radius: f32) -> Option<RacePoint> {
    points(r)
        .into_iter()
        .map(|(p, xy)| (p, xy.distance(at)))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(p, _)| p)
}

/// A heading (degrees) along `dir` or against it, whichever is nearer
/// `want` (zero: `dir` itself).
pub fn heading_along(dir: Vec2, want: Vec2) -> f32 {
    let d = if want != Vec2::ZERO && dir.dot(want) < 0.0 { -dir } else { dir };
    let d = if d == Vec2::ZERO { want } else { d };
    d.y.atan2(d.x).to_degrees()
}

/// Where a new checkpoint goes: on Shift, between the two checkpoints of
/// the leg whose road passes nearest; else after the selected checkpoint,
/// else last.
pub fn insert_index(
    r: &CustomRace,
    preview: Option<&Preview>,
    selected: Option<RacePoint>,
    at: Vec2,
    between: bool,
) -> usize {
    if between && let Some(pv) = preview {
        let best = pv
            .legs
            .iter()
            .enumerate()
            .filter_map(|(k, l)| l.as_deref().map(|l| (k, race::polyline_distance(l, at).0)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((k, _)) = best {
            return (k + 1).min(r.checkpoints.len());
        }
    }
    match selected {
        Some(RacePoint { part: RacePart::Checkpoint, index }) => (index + 1).min(r.checkpoints.len()),
        _ => r.checkpoints.len(),
    }
}

/// Put `tool`'s point of race `r` at `at` (already snapped: `road` is
/// the road there, when snapped). Returns the point made or moved and the
/// undo step's name, or the reason it could not be.
pub fn place(
    r: &mut CustomRace,
    tool: Tool,
    at: Vec3,
    road: Option<RoadPoint>,
    selected: Option<RacePoint>,
    insert_at: usize,
    camera_dir: Vec2,
) -> Result<(RacePoint, String), String> {
    let xy = at.truncate();
    let dir = road.map_or(Vec2::ZERO, |rp| rp.dir);
    match tool {
        Tool::Select => Err("Select a point to move it".into()),
        Tool::Checkpoint => {
            if r.checkpoints.len() >= MAX_CHECKPOINTS {
                return Err(format!("A race holds at most {MAX_CHECKPOINTS} checkpoints"));
            }
            r.checkpoints.insert(insert_at, xy);
            let n = insert_at + 1;
            let verb = if insert_at + 1 == r.checkpoints.len() { "Add" } else { "Insert" };
            Ok((RacePoint { part: RacePart::Checkpoint, index: insert_at }, format!("{verb} checkpoint {n}")))
        }
        Tool::Start => {
            // Facing the first checkpoint (the second when the start is on the first).
            let want = r.checkpoints.iter().find(|c| c.distance(xy) > 10.0).map_or(camera_dir, |c| *c - xy);
            r.start = Some(Spot { position: at, heading: heading_along(dir, want) });
            Ok((RacePoint { part: RacePart::Start, index: 0 }, "Place start line".into()))
        }
        Tool::Finish => {
            let want = r.checkpoints.last().filter(|c| c.distance(xy) > 1.0).map_or(camera_dir, |c| xy - *c);
            r.finish = Some(Spot { position: at, heading: heading_along(dir, want) });
            Ok((RacePoint { part: RacePart::Finish, index: 0 }, "Place finish line".into()))
        }
        Tool::Grid => {
            if r.grid.is_empty() {
                r.grid = race::effective_grid(r);
            }
            let heading = r.start.map_or_else(|| heading_along(dir, camera_dir), |s| s.heading);
            match selected {
                Some(RacePoint { part: RacePart::Grid, index }) if index < r.grid.len() => {
                    r.grid[index].position = at;
                    Ok((RacePoint { part: RacePart::Grid, index }, format!("Move grid slot {}", index + 1)))
                }
                _ if r.grid.len() < MAX_GRID => {
                    r.grid.push(Spot { position: at, heading });
                    let i = r.grid.len() - 1;
                    Ok((RacePoint { part: RacePart::Grid, index: i }, format!("Add grid slot {}", i + 1)))
                }
                _ => Err(format!("The grid is full ({MAX_GRID} slots): select a slot to move it")),
            }
        }
    }
}

/// Move point `p` of `r` to `at` (a drag); the start and finish keep
/// their heading turned along the road when snapped.
pub fn move_point(r: &mut CustomRace, p: RacePoint, at: Vec3, road: Option<RoadPoint>) -> bool {
    let turn = |s: Spot| {
        let h = s.heading.to_radians();
        let heading = road.map_or(s.heading, |rp| heading_along(rp.dir, Vec2::new(h.cos(), h.sin())));
        Spot { position: at, heading }
    };
    match p.part {
        RacePart::Checkpoint => match r.checkpoints.get_mut(p.index) {
            Some(c) => *c = at.truncate(),
            None => return false,
        },
        RacePart::Grid => {
            if r.grid.is_empty() {
                r.grid = race::effective_grid(r);
            }
            match r.grid.get_mut(p.index) {
                Some(g) => g.position = at,
                None => return false,
            }
        }
        RacePart::Start => match r.start {
            Some(s) => r.start = Some(turn(s)),
            None => return false,
        },
        RacePart::Finish => match r.finish {
            Some(s) => r.finish = Some(turn(s)),
            None => return false,
        },
    }
    true
}

/// Delete point `p` of `r`; the undo step's name.
pub fn delete_point(r: &mut CustomRace, p: RacePoint) -> Option<String> {
    match p.part {
        RacePart::Checkpoint if p.index < r.checkpoints.len() => {
            r.checkpoints.remove(p.index);
            Some(format!("Delete checkpoint {}", p.index + 1))
        }
        RacePart::Grid => {
            if r.grid.is_empty() {
                r.grid = race::effective_grid(r);
            }
            (p.index < r.grid.len()).then(|| {
                r.grid.remove(p.index);
                format!("Delete grid slot {}", p.index + 1)
            })
        }
        RacePart::Start => r.start.take().map(|_| "Delete start line".into()),
        RacePart::Finish => r.finish.take().map(|_| "Delete finish line".into()),
        _ => None,
    }
}

/// The point after `p` of the same part, or before it after a delete.
fn after_delete(r: &CustomRace, p: RacePoint) -> Option<RacePoint> {
    let n = match p.part {
        RacePart::Checkpoint => r.checkpoints.len(),
        RacePart::Grid => race::effective_grid(r).len(),
        _ => 0,
    };
    (n > 0).then(|| RacePoint { part: p.part, index: p.index.min(n - 1) })
}

/// A new race on `track` with a free id, its name "Race N".
pub fn new_race(map: &mut MapMod, track: u16, kind: RaceKind) -> usize {
    let mut n = map.races.len() + 1;
    while map.races.iter().any(|r| r.id == format!("race-{n}")) {
        n += 1;
    }
    let mut r = CustomRace::new(&format!("race-{n}"), track, kind);
    r.name = format!("Race {n}");
    map.races.push(r);
    map.races.len() - 1
}

/// The route preview of `r` on `net`; `ground`: heights for the road.
pub fn preview(net: Option<&RoadNet>, r: &CustomRace, ground: &dyn Fn(Vec2) -> f32) -> Preview {
    let Some(net) = net else { return Preview::default() };
    if r.checkpoints.len() < 2 {
        return Preview::default();
    }
    match race::route(net, r, &|_| None) {
        Ok(route) => Preview {
            road: Some(route.road.points.iter().map(|p| p.extend(ground(*p) + 0.3)).collect()),
            legs: race::legs(net, r),
            length: route.road.length,
        },
        Err(_) => Preview { road: None, legs: race::legs(net, r), length: 0.0 },
    }
}

fn rgb(c: crate::launch::theme::Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(c.0, c.1, c.2)
}

fn theme() -> crate::launch::theme::Theme {
    crate::launch::theme::THEME_A
}

impl Races {
    pub fn new() -> Races {
        Races { snap: true, ..Races::default() }
    }

    /// The region's race tracks: those with a career event (whose set-up
    /// a custom race borrows), by number.
    pub fn tracks(&mut self, s: &EditorState) -> &[TrackInfo] {
        if self.tracks.is_none() {
            let mut list = Vec::new();
            match crate::frontend::career::CareerData::load(&s.root) {
                Ok(career) => {
                    for t in career.tracks.values().filter(|t| t.region == s.region) {
                        if career.events.iter().any(|e| e.races.first().map_or(e.track, |r| r.0) == t.number) {
                            list.push(TrackInfo { number: t.number, name: t.name.clone() });
                            self.records.insert(t.number, t.clone());
                        }
                    }
                }
                Err(e) => log::warn!("map editor: no career data: {e:#}"),
            }
            list.sort_by_key(|t| t.number);
            self.tracks = Some(list);
        }
        self.tracks.as_deref().unwrap_or(&[])
    }

    /// The road network of `track`, read once into `s.nets`.
    pub fn net<'s>(&mut self, s: &'s mut EditorState, track: u16) -> Option<&'s RoadNet> {
        if !s.nets.contains_key(&track) {
            self.tracks(s);
            let net = match self.records.get(&track) {
                Some(t) => race::road_net(&s.root, t).map_err(|e| log::warn!("map editor: {e:#}")).ok(),
                None => None,
            };
            s.nets.insert(track, net);
        }
        s.nets.get(&track).and_then(|n| n.as_ref())
    }

    /// The tested record read again (after a test run marked a race).
    pub fn reload_tested(&mut self) {
        self.tested = None;
        self.seen = None;
    }

    /// Checks and previews made again when the mod changed. Call once a
    /// frame before drawing.
    pub fn update(&mut self, s: &mut EditorState) {
        let tracks: Vec<u16> = self.tracks(s).iter().map(|t| t.number).collect();
        if self.seen.as_ref().is_some_and(|(m, t)| *m == s.map && *t == tracks) {
            return;
        }
        if self.tested.is_none() {
            self.tested = Some(Tested::load(&s.maps.join(&s.folder)));
        }
        let race_tracks: Vec<u16> = s.map.races.iter().map(|r| r.track).collect();
        for t in &race_tracks {
            self.net(s, *t);
        }
        // A broken route is tried on the region's other tracks.
        let broken = s.map.races.iter().any(|r| {
            r.checkpoints.len() >= 2
                && s.nets.get(&r.track).and_then(|n| n.as_ref()).is_some_and(|n| race::check(n, r).is_err())
        });
        if broken {
            for t in &tracks {
                self.net(s, *t);
            }
        }
        let tested = self.tested.clone().unwrap_or_default();
        self.checks = validate::refresh(s, &tracks, &tested);
        let ground = |p: Vec2| validate::ground(s, p);
        self.previews =
            s.map.races.iter().map(|r| preview(s.nets.get(&r.track).and_then(|n| n.as_ref()), r, &ground)).collect();
        self.seen = Some((s.map.clone(), tracks));
    }

    /// Why Test race cannot start for the current race (None: it can).
    pub fn test_blocked(&mut self, s: &mut EditorState) -> Option<String> {
        self.update(s);
        let Some(k) = s.race.filter(|&k| k < s.map.races.len()) else { return Some("No race selected".into()) };
        self.checks.iter().find(|c| c.race == Some(k) && c.level == super::state::Level::Error).map(|c| c.text.clone())
    }

    fn snapped(&mut self, s: &mut EditorState, at: Vec3, bypass: bool) -> (Vec3, Option<RoadPoint>) {
        let Some(track) = s.race.and_then(|k| s.map.races.get(k)).map(|r| r.track) else { return (at, None) };
        if !self.snap || bypass {
            return (at, None);
        }
        let Some(rp) = self.net(s, track).and_then(|n| race::snap(n, at.truncate())) else { return (at, None) };
        (rp.at.extend(validate::ground(s, rp.at)), Some(rp))
    }

    /// A click at world point `at` in the viewport or the map: pick a
    /// point near it, else use the tool there. `shift`: insert a
    /// checkpoint into the route; `alt`: no snapping.
    pub fn click(&mut self, s: &mut EditorState, at: Vec3, shift: bool, alt: bool) {
        self.update(s);
        let Some(k) = s.race.filter(|&k| k < s.map.races.len()) else {
            s.status = "Make a race first: Race ▸ New race".into();
            return;
        };
        if !shift && let Some(p) = pick(&s.map.races[k], at.truncate(), PICK_M) {
            s.point = Some(p);
            self.dragging = Some(p);
            return;
        }
        if self.tool == Tool::Select {
            s.point = None;
            return;
        }
        let (at, road) = self.snapped(s, at, alt);
        let insert_at = insert_index(&s.map.races[k], self.previews.get(k), s.point, at.truncate(), shift);
        let f = s.camera.forward();
        match place(&mut s.map.races[k], self.tool, at, road, s.point, insert_at, Vec2::new(f.x, f.y)) {
            Ok((p, label)) => {
                s.point = Some(p);
                commit(s, &label);
            }
            Err(why) => s.status = why,
        }
    }

    /// The pointer dragged to `at` with a point picked by [`Races::click`].
    pub fn drag(&mut self, s: &mut EditorState, at: Vec3, alt: bool) {
        let (Some(p), Some(k)) = (self.dragging, s.race) else { return };
        let (at, road) = self.snapped(s, at, alt);
        if let Some(r) = s.map.races.get_mut(k) {
            move_point(r, p, at, road);
        }
    }

    /// The drag ended: one undo step.
    pub fn release(&mut self, s: &mut EditorState) {
        if let Some(p) = self.dragging.take() {
            let what = match p.part {
                RacePart::Checkpoint => format!("Move checkpoint {}", p.index + 1),
                RacePart::Grid => format!("Move grid slot {}", p.index + 1),
                RacePart::Start => "Move start line".into(),
                RacePart::Finish => "Move finish line".into(),
            };
            commit(s, &what);
        }
    }

    /// Delete the selected point (Del).
    pub fn delete_selected(&mut self, s: &mut EditorState) {
        let (Some(k), Some(p)) = (s.race, s.point) else { return };
        let Some(r) = s.map.races.get_mut(k) else { return };
        if let Some(label) = delete_point(r, p) {
            s.point = after_delete(r, p);
            commit(s, &label);
        }
    }

    /// A new race on the region's track whose roads pass nearest the
    /// camera's view, the start-line tool chosen.
    pub fn new_race(&mut self, s: &mut EditorState) {
        let f = s.camera.forward();
        let look = s.camera.position + f * if f.z < -0.05 { (s.camera.position.z / -f.z).min(300.0) } else { 50.0 };
        let current = s.race.and_then(|k| s.map.races.get(k)).map(|r| r.track);
        let tracks: Vec<u16> = self.tracks(s).iter().map(|t| t.number).collect();
        let track = current.or_else(|| {
            tracks
                .iter()
                .filter_map(|&t| self.net(s, t).and_then(|n| race::snap(n, look.truncate())).map(|rp| (t, rp.distance)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(t, _)| t)
        });
        let Some(track) = track else {
            s.status = format!("No race track of region {} found", s.region);
            return;
        };
        let k = new_race(&mut s.map, track, RaceKind::Sprint);
        (s.race, s.point, s.mode) = (Some(k), None, Mode::Races);
        self.tool = Tool::Start;
        commit(s, &format!("New race on track {track}"));
        s.status = "New race: click the road for the start line, then for the checkpoints".into();
    }

    /// The selected checkpoint moved one place earlier or later.
    fn reorder(&mut self, s: &mut EditorState, later: bool) {
        let (Some(k), Some(RacePoint { part: RacePart::Checkpoint, index })) = (s.race, s.point) else { return };
        let Some(r) = s.map.races.get_mut(k) else { return };
        let j = if later { index + 1 } else { index.wrapping_sub(1) };
        if j < r.checkpoints.len() {
            r.checkpoints.swap(index, j);
            s.point = Some(RacePoint { part: RacePart::Checkpoint, index: j });
            commit(s, &format!("Move checkpoint {} {}", index + 1, if later { "down" } else { "up" }));
        }
    }

    /// The race keys: C / G / S / X / Q tools, Del, [ / ] previous / next
    /// checkpoint. Only in Races mode and while no text field has focus.
    pub fn keys(&mut self, s: &mut EditorState, ctx: &egui::Context) {
        if s.mode != Mode::Races || ctx.egui_wants_keyboard_input() {
            return;
        }
        use egui::Key;
        let pressed = |k: Key| ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        for (key, tool) in [
            (Key::Q, Tool::Select),
            (Key::C, Tool::Checkpoint),
            (Key::G, Tool::Grid),
            (Key::S, Tool::Start),
            (Key::X, Tool::Finish),
        ] {
            if pressed(key) {
                self.tool = tool;
            }
        }
        if pressed(Key::Delete) {
            self.delete_selected(s);
        }
        let n = s.race.and_then(|k| s.map.races.get(k)).map_or(0, |r| r.checkpoints.len());
        if n > 0 && (pressed(Key::OpenBracket) || pressed(Key::CloseBracket)) {
            let cur = match s.point {
                Some(RacePoint { part: RacePart::Checkpoint, index }) => index,
                _ => 0,
            };
            let next = if pressed(Key::OpenBracket) { (cur + n - 1) % n } else { (cur + 1) % n };
            s.point = Some(RacePoint { part: RacePart::Checkpoint, index: next });
        }
    }

    /// The Races tools on the toolbar.
    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        for (tool, name, key) in Tool::ALL {
            ui.selectable_value(&mut self.tool, tool, format!("{name}  {key}"))
                .on_hover_text(format!("{name} tool ({key})"));
        }
        ui.separator();
        ui.checkbox(&mut self.snap, "Snap to road").on_hover_text("Hold Alt while placing to place freely");
    }

    /// The hint under the viewport for the current tool.
    pub fn hint(&self) -> &'static str {
        match self.tool {
            Tool::Select => "Click a point to select · drag: move · Del: remove · [ ] previous / next",
            Tool::Checkpoint => {
                "Click road: place checkpoint · Shift+click on the route: insert · drag: move · Del: remove · [ ] previous / next"
            }
            Tool::Grid => "Click road: add a grid slot, or move the selected one · Del: remove",
            Tool::Start => "Click road: start line, facing the first checkpoint",
            Tool::Finish => "Click road: finish line (sprint); a circuit finishes at its start",
        }
    }

    /// The route, checkpoints, grid and lines over the 3D view.
    pub fn paint_overlay(&mut self, s: &mut EditorState, painter: &egui::Painter, rect: egui::Rect) {
        self.update(s);
        let to = |p: Vec3| project(s, rect, p);
        let ground = |p: Vec2| p.extend(validate::ground(s, p) + 0.3);
        self.paint(s, painter, &|p: Vec3| to(p), &ground, true);
    }

    /// The same over the 2D map view; `to_screen` maps world x/y.
    pub fn paint_map(&mut self, s: &mut EditorState, painter: &egui::Painter, to_screen: &dyn Fn(Vec2) -> egui::Pos2) {
        self.update(s);
        let to = |p: Vec3| Some(to_screen(p.truncate()));
        self.paint(s, painter, &to, &|p: Vec2| p.extend(0.0), false);
    }

    fn paint(
        &self,
        s: &EditorState,
        painter: &egui::Painter,
        to: &dyn Fn(Vec3) -> Option<egui::Pos2>,
        ground: &dyn Fn(Vec2) -> Vec3,
        labels: bool,
    ) {
        let t = theme();
        let (lime, err, ink, page) = (rgb(t.lime), rgb(t.err), rgb(t.ink), rgb(t.page));
        let font = egui::FontId::proportional(13.0);
        let line = |pts: &[Vec3], stroke: egui::Stroke, dashed: bool| {
            let mut run: Vec<egui::Pos2> = Vec::new();
            let flush = |run: &mut Vec<egui::Pos2>| {
                if run.len() >= 2 {
                    if dashed {
                        painter.extend(egui::Shape::dashed_line(run, stroke, 8.0, 6.0));
                    } else {
                        painter.add(egui::Shape::line(std::mem::take(run), stroke));
                    }
                }
                run.clear();
            };
            for p in pts {
                match to(*p) {
                    Some(q) => run.push(q),
                    None => flush(&mut run),
                }
            }
            flush(&mut run);
        };
        for (k, r) in s.map.races.iter().enumerate() {
            let current = s.race == Some(k);
            if !current && s.mode == Mode::Races && labels {
                continue;
            }
            let alpha = if current { 255 } else { 90 };
            let route = egui::Color32::from_rgba_unmultiplied(lime.r(), lime.g(), lime.b(), alpha);
            let pv = self.previews.get(k);
            match pv.and_then(|p| p.road.as_ref()) {
                Some(road) => line(road, egui::Stroke::new(3.0, route), false),
                None => {
                    // Each leg: its road when there is one, else a red dashed line.
                    let n = r.checkpoints.len();
                    for (leg, l) in pv.map_or(&[][..], |p| &p.legs[..]).iter().enumerate() {
                        match l {
                            Some(l) => line(
                                &l.iter().map(|p| ground(*p)).collect::<Vec<_>>(),
                                egui::Stroke::new(3.0, route),
                                false,
                            ),
                            None if n >= 2 => {
                                let (a, b) = (r.checkpoints[leg % n], r.checkpoints[(leg + 1) % n]);
                                line(&[ground(a), ground(b)], egui::Stroke::new(3.0, err), true);
                            }
                            None => {}
                        }
                    }
                }
            }
            if !current {
                continue;
            }
            let sel = |p: RacePoint| s.point == Some(p);
            // Start and finish lines across the road.
            let across = |spot: Spot, colour: egui::Color32, text: &str| {
                let h = spot.heading.to_radians();
                let side = Vec2::new(-h.sin(), h.cos()) * LINE_HALF_M;
                let c = spot.position.truncate();
                line(&[ground(c - side), ground(c + side)], egui::Stroke::new(4.0, colour), false);
                if labels && let Some(q) = to(ground(c + side)) {
                    painter.text(q, egui::Align2::LEFT_BOTTOM, text, font.clone(), colour);
                }
            };
            if let Some(st) = r.start {
                across(st, if sel(RacePoint { part: RacePart::Start, index: 0 }) { ink } else { lime }, "START");
            }
            if let Some(fi) = r.finish {
                across(fi, if sel(RacePoint { part: RacePart::Finish, index: 0 }) { ink } else { lime }, "FINISH");
            }
            // Grid slots: a car-sized box each, numbered.
            for (i, g) in race::effective_grid(r).iter().enumerate() {
                let h = g.heading.to_radians();
                let f = Vec2::new(h.cos(), h.sin());
                let l = f.perp();
                let c = g.position.truncate();
                let corners = [
                    c + f * 2.2 + l * 0.9,
                    c + f * 2.2 - l * 0.9,
                    c - f * 2.2 - l * 0.9,
                    c - f * 2.2 + l * 0.9,
                    c + f * 2.2 + l * 0.9,
                ];
                let colour = if sel(RacePoint { part: RacePart::Grid, index: i }) { lime } else { ink };
                line(&corners.map(ground), egui::Stroke::new(1.5, colour), r.grid.is_empty());
                if let Some(q) = to(ground(c)) {
                    painter.text(
                        q,
                        egui::Align2::CENTER_CENTER,
                        format!("{}", i + 1),
                        egui::FontId::proportional(11.0),
                        colour,
                    );
                }
            }
            // Checkpoints: numbered discs; the distance to the next one.
            for (i, &c) in r.checkpoints.iter().enumerate() {
                let Some(q) = to(ground(c)) else { continue };
                let selected = sel(RacePoint { part: RacePart::Checkpoint, index: i });
                painter.circle_filled(q, 11.0, if selected { lime } else { page });
                painter.circle_stroke(q, 11.0, egui::Stroke::new(2.0, lime));
                painter.text(
                    q,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    font.clone(),
                    if selected { page } else { lime },
                );
                if labels
                    && selected
                    && let Some(&next) = r.checkpoints.get(i + 1)
                {
                    painter.text(
                        q + egui::vec2(14.0, -14.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("→ {} ({:.0} m)", i + 2, c.distance(next)),
                        font.clone(),
                        lime,
                    );
                }
            }
        }
    }

    /// The race properties (the right panel in Races mode): the race,
    /// then the selected point.
    pub fn properties(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        self.update(s);
        let Some(k) = s.race.filter(|&k| k < s.map.races.len()) else {
            ui.label("No race selected.");
            if ui.button("New race").clicked() {
                self.new_race(s);
            }
            return;
        };
        let tracks: Vec<TrackInfo> = self.tracks(s).to_vec();
        let t = theme();
        let dim = rgb(t.dim);
        ui.heading(egui::RichText::new("Race").italics().color(rgb(t.lime)));
        let mut label = None::<String>;
        let mut typed = false;
        egui::Grid::new("race-props").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            let r = &mut s.map.races[k];
            ui.label("Name");
            let a = ui.text_edit_singleline(&mut r.name);
            ui.end_row();
            ui.label("Id");
            let b = ui.text_edit_singleline(&mut r.id);
            ui.end_row();
            typed = a.has_focus() || b.has_focus();
            if a.lost_focus() {
                label = Some("Rename race".into());
            }
            if b.lost_focus() {
                label = Some("Change race id".into());
            }
            ui.label("Base track");
            let current =
                tracks.iter().find(|t| t.number == r.track).map_or_else(|| format!("{}", r.track), |t| t.name.clone());
            egui::ComboBox::from_id_salt("race-track").selected_text(current).show_ui(ui, |ui| {
                for t in &tracks {
                    if ui.selectable_label(t.number == r.track, &t.name).clicked() && t.number != r.track {
                        r.track = t.number;
                        label = Some(format!("Base track {}", t.number));
                    }
                }
            });
            ui.end_row();
            ui.label("");
            ui.label(
                egui::RichText::new("Gives the road network, barriers, AI lanes, traffic and opponents.")
                    .small()
                    .color(dim),
            );
            ui.end_row();
            ui.label("Mode");
            ui.horizontal(|ui| {
                for kind in RaceKind::ALL {
                    let name = match kind {
                        RaceKind::Circuit => "Circuit",
                        RaceKind::Sprint => "Sprint",
                        RaceKind::StreetX => "StreetX",
                        RaceKind::Drift => "Drift",
                    };
                    if ui.selectable_label(r.kind == kind, name).clicked() && r.kind != kind {
                        r.kind = kind;
                        r.laps = if kind.loops() { r.laps.clamp(2, MAX_LAPS) } else { 1 };
                        label = Some(format!("Mode {name}"));
                    }
                }
            });
            ui.end_row();
            ui.label("Laps");
            let before = r.laps;
            let resp = ui.add_enabled(r.kind.loops(), egui::DragValue::new(&mut r.laps).range(1..=MAX_LAPS).speed(0.1));
            if r.laps != before && !resp.dragged() || resp.drag_stopped() {
                label = Some(format!("Laps {}", r.laps));
            }
            ui.end_row();
            ui.label("Direction");
            ui.horizontal(|ui| {
                for (back, name) in [(false, "Forward"), (true, "Backward")] {
                    if ui.selectable_label(r.backward == back, name).clicked() && r.backward != back {
                        r.backward = back;
                        label = Some(format!("Direction {name}"));
                    }
                }
            });
            ui.end_row();
        });
        self.typing = typed;
        if let Some(l) = label {
            commit(s, &l);
        }
        ui.separator();
        let r = &s.map.races[k];
        let pv = self.previews.get(k).cloned().unwrap_or_default();
        egui::Grid::new("race-status").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Length");
            let km = if pv.road.is_some() { format!("{:.1} km", pv.length / 1000.0) } else { "—".into() };
            ui.label(format!("{km} · {} checkpoints", r.checkpoints.len()));
            ui.end_row();
            ui.label("Status");
            ui.horizontal(|ui| {
                let broken = validate::blocks(&self.checks, k);
                let tested = self.tested.as_ref().is_some_and(|t| t.is_tested(r));
                let badge = |ui: &mut egui::Ui, text: &str, c: egui::Color32| {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, c))
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::symmetric(6, 1))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(text).small().color(c));
                        });
                };
                if broken {
                    badge(ui, "cannot race", rgb(t.err));
                } else {
                    badge(ui, "route ok", rgb(t.ok));
                }
                if tested {
                    badge(ui, "tested", rgb(t.ok));
                } else {
                    badge(ui, "not tested", dim);
                }
            });
            ui.end_row();
        });
        if ui.button("Delete race").clicked() {
            let name = s.map.races[k].name.clone();
            s.map.races.remove(k);
            s.race = s.map.races.len().checked_sub(1).map(|n| k.min(n));
            s.point = None;
            commit(s, &format!("Delete race {name}"));
            return;
        }
        self.point_properties(s, ui, k);
    }

    fn point_properties(&mut self, s: &mut EditorState, ui: &mut egui::Ui, k: usize) {
        let Some(p) = s.point else { return };
        let Some(xy) = point_xy(&s.map.races[k], p) else { return };
        let t = theme();
        ui.separator();
        let title = match p.part {
            RacePart::Checkpoint => format!("Checkpoint {}", p.index + 1),
            RacePart::Grid => format!("Grid slot {}", p.index + 1),
            RacePart::Start => "Start line".into(),
            RacePart::Finish => "Finish line".into(),
        };
        ui.heading(egui::RichText::new(&title).italics().color(rgb(t.lime)));
        let (mut x, mut y) = (xy.x, xy.y);
        let mut moved = false;
        let mut done = false;
        ui.horizontal(|ui| {
            ui.label("X");
            let a = ui.add(egui::DragValue::new(&mut x).speed(0.25).fixed_decimals(1));
            ui.label("Y");
            let b = ui.add(egui::DragValue::new(&mut y).speed(0.25).fixed_decimals(1));
            moved = a.changed() || b.changed();
            done = a.drag_stopped() || b.drag_stopped() || a.lost_focus() || b.lost_focus();
        });
        if moved {
            let at = Vec2::new(x, y);
            let at3 = at.extend(validate::ground(s, at));
            move_point(&mut s.map.races[k], p, at3, None);
        }
        if done || (moved && !ui.ctx().input(|i| i.pointer.any_down())) {
            commit(s, &format!("Move {}", title.to_lowercase()));
        }
        let track = s.map.races[k].track;
        let road = self.net(s, track).and_then(|n| race::snap(n, Vec2::new(x, y)));
        ui.horizontal(|ui| {
            ui.label("Road");
            match road {
                Some(rp) if rp.distance <= validate::OFF_ROAD_M => {
                    ui.label(egui::RichText::new(format!("on road · segment {}", rp.segment)).color(rgb(t.ok)));
                }
                Some(rp) => {
                    ui.label(egui::RichText::new(format!("{:.1} m from the road", rp.distance)).color(rgb(t.warn)));
                }
                None => {
                    ui.label(egui::RichText::new("no road network").color(rgb(t.err)));
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Fly to").clicked() {
                let c = Check {
                    level: super::state::Level::Info,
                    ok: true,
                    text: String::new(),
                    race: Some(k),
                    point: Some(p),
                    at: Some(Vec2::new(x, y)),
                    fix: None,
                };
                validate::show(s, &c);
            }
            if p.part == RacePart::Checkpoint {
                if ui.button("Move up").clicked() {
                    self.reorder(s, false);
                }
                if ui.button("Move down").clicked() {
                    self.reorder(s, true);
                }
            }
            if road.is_some_and(|rp| rp.distance > 0.5) && ui.button("Snap to road").clicked() {
                let rp = road.unwrap();
                let at = rp.at.extend(validate::ground(s, rp.at));
                move_point(&mut s.map.races[k], p, at, Some(rp));
                commit(s, &format!("Snap {} to road", title.to_lowercase()));
            }
            if ui.button("Delete").clicked() {
                self.delete_selected(s);
            }
        });
    }

    /// The Validation tab.
    pub fn validation(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        self.update(s);
        let checks = self.checks.clone();
        if validate::panel(s, &checks, ui) {
            self.update(s);
        }
    }

    /// The counts for the tab's title and the status bar: (errors, warnings).
    pub fn counts(&self) -> (usize, usize) {
        use super::state::Level;
        (
            self.checks.iter().filter(|c| c.level == Level::Error).count(),
            self.checks.iter().filter(|c| c.level == Level::Warning).count(),
        )
    }
}

impl Panel for Races {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        self.properties(s, ui);
    }
}

#[cfg(test)]
#[path = "races_tests.rs"]
mod tests;
