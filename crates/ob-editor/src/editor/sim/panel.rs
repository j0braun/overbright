//! The Simulate tab, its toolbar button, the frame hook that reads the
//! running game, and the cars drawn over the 3D view and the map.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Color32, Pos2, Rect, Stroke};
use glam::{Mat4, Vec2, Vec3};

use super::format::{CarKind, CarSample, EventKind, SimMode};
use super::record::{Phase, Playback, Recording, SPEEDS};
use super::run::{SimChild, What, args};
use crate::editor::app::Overlay as ViewOverlay;
use crate::editor::overlay::{Overlay, colour, label2d, line2d};
use crate::editor::state::{EditorState, Issue, Level, Panel, View};
use crate::editor::theme;
use crate::tools::map::store;

/// The bottom tab's key (`--tab simulate`).
pub const TAB: &str = "simulate";
/// Validation entries from a simulation start with this.
pub const ISSUE_PREFIX: &str = "Simulation: ";
/// Lines read from the game per frame at most (the rest next frame).
const LINES_PER_FRAME: usize = 4000;
/// A car's box around its position (the body's centre): half length, half
/// width, and its bottom and top (m).
const CAR_HALF: Vec3 = Vec3::new(2.2, 0.9, 0.0);
const CAR_BOTTOM: f32 = -0.5;
const CAR_TOP: f32 = 0.8;

/// One simulation: the run, its recording and the playback. Shared by the
/// tab, the toolbar button, the frame hook and the overlay.
pub struct Sim {
    pub rec: Recording,
    pub play: Playback,
    child: Option<SimChild>,
    pub what: Option<What>,
    /// The race simulated (index into `map.races`), for its warnings.
    pub race: Option<usize>,
    pub seed: u64,
    pub traces: bool,
    /// The warnings last put into the Validation list.
    issued: Vec<Issue>,
}

impl Default for Sim {
    fn default() -> Sim {
        Sim {
            rec: Recording::default(),
            play: Playback::default(),
            child: None,
            what: None,
            race: None,
            seed: 1,
            traces: true,
            issued: Vec::new(),
        }
    }
}

pub type Shared = Rc<RefCell<Sim>>;

/// The validation warnings of a recording for race `race` (index into
/// `map.races`): one per problem ("Simulation: AI got stuck before
/// checkpoint 3"), with the spot to show. The race checks may call this to
/// list them with their own; the frame hook keeps them in `s.issues`.
pub fn issues(rec: &Recording, race: Option<usize>) -> Vec<Issue> {
    rec.problems().into_iter().map(|p| Issue { level: Level::Warning, text: p.text(), race, at: p.at }).collect()
}

fn ctx_id(name: &str) -> egui::Id {
    egui::Id::unique(("editor.sim", name))
}

/// Ask the shell to show bottom tab `key` (the shell takes it next frame).
pub fn request_tab(ctx: &egui::Context, key: &'static str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("editor.tab.request"), key));
}

/// The tab the shell was asked to show (taken once).
pub fn take_tab_request(ctx: &egui::Context) -> Option<&'static str> {
    ctx.data_mut(|d| d.remove_temp::<&'static str>(egui::Id::unique("editor.tab.request")))
}

/// The map view's transform this frame (centre, points per metre, rect),
/// so overlays can draw on the map.
pub fn publish_map(ctx: &egui::Context, centre: Vec2, scale: f32, rect: Rect) {
    ctx.data_mut(|d| d.insert_temp(ctx_id("map"), (centre, scale, rect)));
}

fn map_transform(ctx: &egui::Context) -> Option<(Vec2, f32, Rect)> {
    ctx.data(|d| d.get_temp(ctx_id("map")))
}

/// A recording to show without running the game (`--sim <file>[@seconds[:car]]`
/// for the headless shot).
pub fn load_recording(ctx: &egui::Context, spec: &str) {
    ctx.data_mut(|d| d.insert_temp(ctx_id("load"), spec.to_string()));
}

impl Sim {
    pub fn running(&self) -> bool {
        self.child.is_some()
    }

    /// Save the mod and start the game simulating `what`.
    pub fn start(&mut self, s: &mut EditorState, exe: &std::path::Path, what: What) {
        self.stop();
        let mut m = s.map.clone();
        if m.name.trim().is_empty() {
            m.name = s.folder.clone();
        }
        if let Err(e) = store::save(&s.maps, &s.folder, &m) {
            s.status = format!("Not saved: {e:#}");
            return;
        }
        s.dirty = false;
        self.race = match &what {
            What::Race(id) => s.map.races.iter().position(|r| &r.id == id),
            What::World(..) => None,
        };
        match SimChild::start(exe, &args(&s.folder, &what, self.seed), &s.root) {
            Ok(c) => {
                self.child = Some(c);
                self.rec = Recording::default();
                self.play.start();
                s.status = match &what {
                    What::Race(_) => "Simulating the race…".into(),
                    What::World(..) => "Simulating the world around the camera…".into(),
                };
                self.what = Some(what);
            }
            Err(e) => s.status = e,
        }
    }

    pub fn stop(&mut self) {
        if let Some(c) = self.child.as_mut() {
            c.stop();
        }
    }

    /// Read what arrived, move the playback on by `dt` seconds.
    pub fn pump(&mut self, dt: f32) -> bool {
        let mut changed = false;
        if let Some(c) = self.child.as_mut() {
            let (lines, running) = c.take(LINES_PER_FRAME);
            changed |= !lines.is_empty();
            for l in &lines {
                self.rec.push_line(l);
            }
            self.play.observe(&self.rec, running);
            if !running {
                self.child = None;
                changed = true;
            }
        }
        let before = self.play.time;
        self.play.tick(&self.rec, dt);
        changed || self.play.time != before
    }

    /// Show a saved recording (`path[@seconds]`), paused there.
    pub fn show_file(&mut self, spec: &str) -> Result<(), String> {
        // `path[@seconds[:car]]`: the time shown and the car followed.
        let (path, at, follow) = match spec.rsplit_once('@') {
            Some((p, t)) => {
                let (t, car) = t.split_once(':').map_or((t, None), |(t, c)| (t, c.parse::<u32>().ok()));
                match t.parse::<f32>() {
                    Ok(t) => (p, Some(t), car),
                    Err(_) => (spec, None, None),
                }
            }
            None => (spec, None, None),
        };
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        self.rec = Recording::read(&text);
        self.play.start();
        self.play.observe(&self.rec, false);
        if let Some(t) = at {
            self.play.seek(&self.rec, t);
        }
        self.play.follow = follow;
        self.what = self.rec.header.as_ref().map(|h| match h.mode {
            SimMode::Race => What::Race(h.race.clone()),
            SimMode::World => What::World(Vec3::ZERO, 0.0),
        });
        Ok(())
    }
}

/// Put the simulation's warnings into the Validation list (replacing the
/// last ones), so they sit beside the race checks.
fn sync_issues(sim: &mut Sim, s: &mut EditorState) {
    let now = if sim.what.as_ref().is_some_and(|w| w.mode() == SimMode::Race) {
        issues(&sim.rec, sim.race)
    } else {
        Vec::new()
    };
    let present = |i: &Issue| s.issues.iter().any(|x| x == i);
    if now == sim.issued && now.iter().all(present) {
        return;
    }
    s.issues.retain(|i| !i.text.starts_with(ISSUE_PREFIX));
    s.issues.extend(now.iter().cloned());
    sim.issued = now;
}

/// The frame hook: read the game, play, follow, warnings.
pub struct SimHook(pub Shared);

impl Panel for SimHook {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut sim = self.0.borrow_mut();
        if let Some(spec) = ctx.data_mut(|d| d.remove_temp::<String>(ctx_id("load")))
            && let Err(e) = sim.show_file(&spec)
        {
            s.status = format!("Simulation not read: {e}");
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let was_running = sim.running();
        if sim.pump(dt) {
            ctx.request_repaint();
        }
        if was_running && !sim.running() {
            s.status = match &sim.play.phase {
                Phase::Failed(why) => format!("Simulation failed: {why}"),
                _ => format!("Simulation done ({})", sim.rec.end.as_deref().unwrap_or("game ended")),
            };
        }
        if sim.play.playing || sim.running() {
            ctx.request_repaint();
        }
        // Follow a car in the 3D view: behind and above it.
        if s.view == View::ThreeD
            && let Some(id) = sim.play.follow
            && let Some(c) = sim.play.cars(&sim.rec).iter().find(|c| c.id == id)
        {
            let h = c.heading();
            let back = Vec3::new(h.cos(), h.sin(), 0.0);
            let eye = c.position - back * 14.0 + Vec3::Z * 6.0;
            let look = crate::draw::camera::Camera::looking_at(eye, c.position + Vec3::Z * 1.0);
            (s.camera.position, s.camera.yaw, s.camera.pitch) = (look.position, look.yaw, look.pitch);
        }
        sync_issues(&mut sim, s);
    }
}

/// The toolbar's Simulate button: the current race in Races mode with a
/// race chosen, else the world around the camera.
pub struct SimTool(pub Shared);

impl Panel for SimTool {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let mut sim = self.0.borrow_mut();
        let text = if sim.running() { "Stop simulation" } else { "Simulate" };
        let r = ui
            .add(egui::Button::new(egui::RichText::new(text).font(theme::font(theme::LABEL, 15.0))))
            .on_hover_text("Run the race (or the traffic around the camera) with the AI driving every car");
        if r.clicked() {
            if sim.running() {
                sim.stop();
            } else {
                let what = what_now(s);
                sim.start(s, &crate::editor::testplay::game_exe(), what);
                request_tab(ui.ctx(), TAB);
            }
        }
    }
}

/// The race chosen, else the world at the camera's spot.
fn what_now(s: &EditorState) -> What {
    match s.race.and_then(|k| s.map.races.get(k)) {
        Some(r) if s.mode == crate::editor::state::Mode::Races => What::Race(r.id.clone()),
        _ => world_spot(s),
    }
}

fn world_spot(s: &EditorState) -> What {
    match crate::editor::testplay::drive_spot(s) {
        crate::editor::testplay::Target::At(p, h) => What::World(p, h),
        crate::editor::testplay::Target::Race(_) => What::World(s.camera.position, 0.0),
    }
}

fn clock(t: f32) -> String {
    let sign = if t < 0.0 { "-" } else { "" };
    let t = t.abs();
    format!("{sign}{}:{:04.1}", (t / 60.0) as u32, t % 60.0)
}

/// The bottom tab.
pub struct SimTab(pub Shared);

impl Panel for SimTab {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let mut sim = self.0.borrow_mut();
        let sim = &mut *sim;
        ui.horizontal(|ui| {
            let race = s.race.and_then(|k| s.map.races.get(k)).map(|r| (r.id.clone(), r.name.clone()));
            if sim.running() {
                if ui.button("Stop").clicked() {
                    sim.stop();
                }
            } else {
                let label = race.as_ref().map_or("Simulate race".to_string(), |r| format!("Simulate {}", r.1));
                let r = ui.add_enabled(race.is_some(), theme::main_action(&label));
                if r.on_disabled_hover_text("Choose a race (Races)").clicked()
                    && let Some((id, _)) = &race
                {
                    sim.start(s, &crate::editor::testplay::game_exe(), What::Race(id.clone()));
                }
                if ui.button("Simulate world").on_hover_text("Traffic and street racers around the camera").clicked() {
                    let w = world_spot(s);
                    sim.start(s, &crate::editor::testplay::game_exe(), w);
                }
                ui.label("Seed");
                ui.add(egui::DragValue::new(&mut sim.seed).range(0..=u32::MAX as u64));
            }
            ui.separator();
            let phase = match &sim.play.phase {
                Phase::Idle => "No simulation yet".to_string(),
                Phase::Starting => "Starting the game…".into(),
                Phase::Running => "Running".into(),
                Phase::Done => format!("Done ({})", sim.rec.end.as_deref().unwrap_or("ended")),
                Phase::Failed(why) => format!("Failed: {why}"),
            };
            let colour = if matches!(sim.play.phase, Phase::Failed(_)) { theme::error() } else { theme::dim() };
            ui.colored_label(colour, phase);
        });
        let Some((lo, hi)) = sim.rec.span() else { return };
        ui.horizontal(|ui| {
            if ui.button(if sim.play.playing { "Pause" } else { "Play" }).on_hover_text("Space").clicked() {
                sim.play.toggle(&sim.rec);
            }
            egui::ComboBox::from_id_salt("sim.speed")
                .width(64.0)
                .selected_text(format!("{}x", SPEEDS[sim.play.speed]))
                .show_ui(ui, |ui| {
                    for (k, v) in SPEEDS.iter().enumerate() {
                        ui.selectable_value(&mut sim.play.speed, k, format!("{v}x"));
                    }
                });
            let mut t = sim.play.time;
            let w = (ui.available_width() - 330.0).max(120.0);
            ui.spacing_mut().slider_width = w;
            if ui.add(egui::Slider::new(&mut t, lo..=hi).show_value(false)).changed() {
                sim.play.seek(&sim.rec, t);
            }
            ui.monospace(format!("{} / {}", clock(sim.play.time), clock(hi)));
            let follow = sim.play.follow.map_or("Free camera".to_string(), |id| sim.rec.name(id));
            egui::ComboBox::from_id_salt("sim.follow").width(130.0).selected_text(follow).show_ui(ui, |ui| {
                ui.selectable_value(&mut sim.play.follow, None, "Free camera");
                for id in sim.rec.racers() {
                    let n = sim.rec.name(id);
                    ui.selectable_value(&mut sim.play.follow, Some(id), n);
                }
            });
            ui.checkbox(&mut sim.traces, "Lines");
        });
        ui.separator();
        let mut seek: Option<(f32, u32)> = None;
        ui.columns(2, |cols| {
            let world = sim.rec.header.as_ref().is_some_and(|h| h.mode == SimMode::World);
            let st = if world { Vec::new() } else { sim.rec.standings(sim.play.time) };
            if st.is_empty() {
                let cars = sim.play.cars(&sim.rec);
                let traffic = cars.iter().filter(|c| c.kind == CarKind::Traffic).count();
                let racers = cars.iter().filter(|c| c.kind == CarKind::Encounter).count();
                cols[0].weak(format!("{traffic} traffic cars and {racers} street racers on the road"));
            } else {
                egui::Grid::new("sim.results").striped(true).num_columns(5).show(&mut cols[0], |ui| {
                    for h in ["Place", "Car", "Time", "Best lap", "Laps"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for r in &st {
                        ui.label(if r.place == 0 { "-".to_string() } else { r.place.to_string() });
                        let name = sim.rec.name(r.car);
                        if sim.play.follow == Some(r.car) {
                            ui.colored_label(theme::lime(), name);
                        } else {
                            ui.label(name);
                        }
                        ui.monospace(r.time.map_or("-".into(), clock));
                        ui.monospace(r.best.map_or("-".into(), clock));
                        ui.monospace(r.laps.len().to_string());
                        ui.end_row();
                    }
                });
            }
            let shown: Vec<_> =
                sim.rec.events.iter().filter(|e| e.kind != EventKind::Checkpoint || e.car == 0).cloned().collect();
            if shown.is_empty() {
                cols[1].weak("No events yet");
            }
            for e in shown.iter().rev().take(200) {
                cols[1].horizontal(|ui| {
                    let c = if e.kind.is_problem() { theme::warn() } else { theme::dim() };
                    ui.monospace(clock(e.time));
                    let what = match e.kind {
                        EventKind::Start => "Green light".to_string(),
                        EventKind::Checkpoint => format!("checkpoint {}", e.checkpoint),
                        EventKind::Lap => format!("lap {} in {}", e.checkpoint, clock(e.value)),
                        EventKind::Finish => format!("finished in {}", clock(e.value)),
                        EventKind::Crash => format!("crash ({:.0} km/h lost)", e.value * 3.6),
                        EventKind::Stuck => format!("stuck before checkpoint {}", e.checkpoint),
                        EventKind::WrongWay => format!("wrong way before checkpoint {}", e.checkpoint),
                    };
                    let who =
                        if e.kind == EventKind::Start { String::new() } else { format!("{}: ", sim.rec.name(e.car)) };
                    ui.colored_label(c, format!("{who}{what}"));
                    if e.kind != EventKind::Start && ui.small_button("Show").clicked() {
                        seek = Some((e.time, e.car));
                    }
                });
            }
        });
        if let Some((t, car)) = seek {
            sim.play.seek(&sim.rec, t);
            sim.play.follow = Some(car);
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input() {
            sim.play.toggle(&sim.rec);
        }
    }
}

/// A car's colour: the followed one lime, the player's slot lime, racers
/// ink, traffic grey, street racers the info tone.
fn car_colour(c: &CarSample, followed: bool) -> Color32 {
    if followed {
        return colour::SELECT;
    }
    match c.kind {
        CarKind::Player => colour::SELECT,
        CarKind::Racer => colour::INK,
        CarKind::Traffic => colour::DIM,
        CarKind::Encounter => colour::WARN,
    }
}

/// The cars over the viewport (3D boxes or map arrows) and the racing
/// lines.
pub struct SimOverlay(pub Shared);

impl ViewOverlay for SimOverlay {
    fn overlay(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) -> bool {
        let sim = self.0.borrow();
        let cars = sim.play.cars(&sim.rec);
        if cars.is_empty() {
            return false;
        }
        let traces: Vec<(u32, Vec<Vec<Vec3>>)> = if sim.traces {
            sim.rec.racers().into_iter().map(|id| (id, sim.rec.trace(id, sim.play.time, 6))).collect()
        } else {
            Vec::new()
        };
        let trace_colour = |id: u32| {
            let c = if id == 0 || sim.play.follow == Some(id) { colour::SELECT } else { colour::INK };
            c.gamma_multiply(0.55)
        };
        match s.view {
            View::ThreeD => {
                let o = Overlay::new(painter, &s.camera, rect);
                for (id, pieces) in &traces {
                    for p in pieces {
                        let lifted: Vec<Vec3> = p.iter().map(|q| *q + Vec3::Z * 0.3).collect();
                        o.polyline(&lifted, false, trace_colour(*id), 1.5);
                    }
                }
                for c in cars {
                    let followed = sim.play.follow == Some(c.id);
                    let t = Mat4::from_rotation_translation(c.rotation, c.position);
                    let col = car_colour(c, followed);
                    let w = if c.kind.races() { 2.0 } else { 1.2 };
                    o.box_outline(t, -CAR_HALF + Vec3::Z * CAR_BOTTOM, CAR_HALF + Vec3::Z * CAR_TOP, col, w);
                    if c.kind.races() && c.place > 0 {
                        o.label(
                            c.position + Vec3::Z * 2.2,
                            &format!("P{} {}", c.place, short(&label_name(&sim.rec, c.id))),
                            col,
                        );
                    }
                }
            }
            View::Map => {
                let Some((centre, scale, mrect)) = map_transform(ctx) else { return false };
                let to = |p: Vec3| {
                    let c = mrect.center();
                    Pos2::new(c.x + (p.x - centre.x) * scale, c.y - (p.y - centre.y) * scale)
                };
                for (id, pieces) in &traces {
                    for p in pieces {
                        for w in p.windows(2) {
                            painter.line_segment([to(w[0]), to(w[1])], Stroke::new(1.5, trace_colour(*id)));
                        }
                    }
                }
                for c in cars {
                    let followed = sim.play.follow == Some(c.id);
                    let col = car_colour(c, followed);
                    let at = to(c.position);
                    if !rect.expand(20.0).contains(at) {
                        continue;
                    }
                    let h = c.heading();
                    let len = (4.4 * scale).max(if c.kind.races() { 9.0 } else { 6.0 });
                    let d = egui::vec2(h.cos(), -h.sin());
                    let side = egui::vec2(-d.y, d.x) * len * 0.45;
                    let tip = at + d * len * 0.6;
                    let back = at - d * len * 0.4;
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, back + side, back - side],
                        col,
                        Stroke::new(1.0, colour::SHADOW),
                    ));
                    if c.kind.races() && c.place > 0 {
                        label2d(painter, at - egui::vec2(0.0, len + 8.0), &format!("P{}", c.place), col);
                    }
                    if followed {
                        line2d(painter, at - egui::vec2(len, 0.0), at + egui::vec2(len, 0.0), col, 1.0);
                    }
                }
            }
        }
        false
    }
}

/// The car's own name for a label (no "(player slot)").
fn label_name(rec: &Recording, id: u32) -> String {
    rec.cars.get(&id).map(|c| c.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| rec.name(id))
}

/// A name short enough for a label over a car.
fn short(name: &str) -> String {
    let n: String = name.chars().take(14).collect();
    if n.len() < name.len() { format!("{n}.") } else { n }
}

#[cfg(test)]
mod tests {
    use super::super::format::{Event, Header, Line};
    use super::*;

    #[test]
    fn problems_become_validation_warnings_once() {
        let mut rec = Recording::default();
        rec.push(Line::Header(Header { version: 1, race: "r".into(), ..Default::default() }));
        rec.push(Line::Event(Event { tick: 0, time: 9.0, kind: EventKind::Stuck, car: 2, checkpoint: 3, value: 4.0 }));
        let v = issues(&rec, Some(0));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].level, Level::Warning);
        assert_eq!(v[0].text, "Simulation: AI got stuck before checkpoint 3");
        assert_eq!(v[0].race, Some(0));

        let mut s = EditorState::new(
            std::path::PathBuf::new(),
            "L4RA",
            std::path::PathBuf::new(),
            "m",
            Default::default(),
            crate::world::World::default(),
            crate::draw::camera::Camera::new(Vec3::ZERO, 0.0, 0.0),
        );
        s.issues.push(Issue { level: Level::Error, text: "no road".into(), race: Some(0), at: None });
        let mut sim = Sim { rec, what: Some(What::Race("r".into())), race: Some(0), ..Default::default() };
        sync_issues(&mut sim, &mut s);
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.issues.len(), 2, "the race's own issue kept, the warning once");
        // The race checks rebuilt the list: the warning comes back.
        s.issues.retain(|i| !i.text.starts_with(ISSUE_PREFIX));
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.issues.len(), 2);
        // A new run without problems clears it.
        sim.rec = Recording::default();
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.issues.len(), 1);
    }

    #[test]
    fn clock_and_short_names() {
        assert_eq!(clock(83.41), "1:23.4");
        assert_eq!(clock(-2.5), "-0:02.5");
        assert_eq!(short("Mitsubishi Lancer Evolution"), "Mitsubishi Lan.");
    }
}
