//! The Simulate tab, its toolbar button, the frame hook that reads the
//! running game, and the cars drawn over the 3D view and the map.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Color32, Pos2, Rect, Stroke};
use glam::{Mat4, Vec2, Vec3};

use super::cams::{self, CamMode, ChaseCars};
use super::format::{CarKind, CarSample, EventKind, SimMode};
use super::record::{Phase, Playback, Recording};
use super::run::{SimChild, What, args};
use super::summary::{self, Group, RunFindings};
use crate::editor::app::Overlay as ViewOverlay;
use crate::editor::modes::{self, SIMULATE, SIMULATE_WORLD};
use crate::editor::overlay::{Overlay, colour, label2d, line2d};
use crate::editor::state::{EditorState, Issue, Level, Panel};
use crate::editor::theme;

/// The bottom tab's key (`--tab simulate`).
pub const TAB: &str = "simulate";
/// Validation entries from a simulation start with this.
pub const ISSUE_PREFIX: &str = "Simulation: ";
/// The bottom panel's height at least while this tab shows (the toolbar
/// rows, the legend, the slot note, the results of a full grid and the
/// first events under them; the playback is on the timeline under the view).
pub const MIN_HEIGHT: f32 = 370.0;
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
    /// How many runs Simulate race makes (seeds `seed`, `seed + 1`, …).
    pub runs: u32,
    /// The seeds still to run in this batch.
    pending: Vec<u64>,
    /// The runs of this batch so far: what went wrong in each, and their
    /// recordings other than the one shown (by seed).
    pub batch: Vec<RunFindings>,
    stash: Vec<Recording>,
    /// The summary over `batch`.
    pub groups: Vec<Group>,
    /// The list under the results: what went wrong, or the events.
    pub list: List,
    /// The summary row last shown (index over groups and their rows).
    pub pick: Option<usize>,
    /// Scroll the picked row into view next frame (moved by the keyboard).
    scroll_pick: bool,
    /// The summary row with the keyboard focus last frame.
    focus_row: Option<usize>,
    /// The summary rows' widget ids as last drawn.
    pub row_ids: Vec<egui::Id>,
    /// How the 3D camera rides with the car followed.
    pub cam: CamMode,
    /// The cars the chase camera has loaded.
    pub chase_cars: ChaseCars,
    /// The timeline's track as last drawn.
    pub track_rect: Rect,
    /// A second run drawn as ghosts over the first, on the same race clock (Runs, compare).
    pub ghost: Option<Recording>,
    /// The car whose telemetry the Telemetry tab shows (none: the car followed, else the first racer).
    pub tele_car: Option<u32>,
    /// The name of the ghost's run (its file's name).
    pub ghost_name: String,
    /// When the ghost's run parts from the one shown (a state hash or a car's place differs), if it does.
    pub parts_at: Option<f32>,
    /// The lines the game streamed for the run going (kept to save it as a run file when it ends).
    raw: String,
    /// The name of the run shown, as the Runs list names it (empty: a run not saved).
    pub shown: String,
}

/// The list under the results table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum List {
    #[default]
    Results,
    Summary,
    Events,
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
            runs: 1,
            pending: Vec::new(),
            batch: Vec::new(),
            stash: Vec::new(),
            groups: Vec::new(),
            list: List::Summary,
            pick: None,
            scroll_pick: false,
            row_ids: Vec::new(),
            focus_row: None,
            cam: CamMode::default(),
            chase_cars: ChaseCars::default(),
            track_rect: Rect::NOTHING,
            ghost: None,
            tele_car: None,
            ghost_name: String::new(),
            parts_at: None,
            raw: String::new(),
            shown: String::new(),
        }
    }
}

pub type Shared = Rc<RefCell<Sim>>;

/// The validation warnings of a recording for race `race` (index into
/// `map.races`): one per problem ("Simulation: AI got stuck before
/// checkpoint 3"), with the spot to show. The race checks may call this to
/// list them with their own; the frame hook keeps them in `s.session.issues`.
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

/// The top view's transform this frame (centre, points per metre, rect; None: the 3D view shows), so overlays
/// can draw on the plan.
pub fn publish_map(ctx: &egui::Context, map: Option<(Vec2, f32, Rect)>) {
    ctx.data_mut(|d| d.insert_temp(ctx_id("map"), map));
}

/// The top view's transform as last published.
pub fn map_transform(ctx: &egui::Context) -> Option<(Vec2, f32, Rect)> {
    ctx.data(|d| d.get_temp::<Option<(Vec2, f32, Rect)>>(ctx_id("map")).flatten())
}

/// A recording to show without running the game (`--sim <file>[@seconds[:car[:chase]]]`
/// for the headless shot).
pub fn load_recording(ctx: &egui::Context, spec: &str) {
    ctx.data_mut(|d| d.insert_temp(ctx_id("load"), spec.to_string()));
}

impl Sim {
    pub fn running(&self) -> bool {
        self.child.is_some()
    }

    /// Start simulating `what`: a race `runs` times (seeds `seed`,
    /// `seed + 1`, … one after the other), the world once.
    pub fn start(&mut self, s: &mut EditorState, exe: &std::path::Path, what: What) {
        self.stop();
        let n = if matches!(what, What::Race(_)) { self.runs.clamp(1, MAX_RUNS) } else { 1 };
        self.pending = (1..n as u64).rev().map(|k| self.seed.wrapping_add(k)).collect();
        self.batch.clear();
        self.stash.clear();
        self.groups.clear();
        self.pick = None;
        self.launch(s, exe, what, self.seed);
    }

    /// Runs in this batch: done (the one running included) and in all.
    pub fn batch_progress(&self) -> (usize, usize) {
        let done = self.batch.len() + usize::from(self.running());
        (done, done + self.pending.len())
    }

    /// Write the mod's run copy (unsaved changes included; `map.obmap`
    /// is not written) and start the game simulating `what` with `seed`.
    fn launch(&mut self, s: &mut EditorState, exe: &std::path::Path, what: What, seed: u64) {
        let file = match super::super::testplay::write_run_copy(s) {
            Ok(f) => f,
            Err(e) => {
                s.status = e;
                return;
            }
        };
        self.race = match &what {
            What::Race(id) => s.session.map.races.iter().position(|r| &r.id == id),
            What::World(..) => None,
        };
        let unsaved = if s.session.dirty { " (unsaved changes)" } else { "" };
        let link = crate::tools::map::link::sim_dir(&s.maps, &s.session.folder);
        match SimChild::launch(exe, &args(&file, &what, seed), &s.root, &link, &s.maps) {
            Ok(c) => {
                self.child = Some(c);
                self.rec = Recording::default();
                self.raw.clear();
                self.shown.clear();
                self.play.start();
                s.status = match &what {
                    What::Race(_) => format!("Simulating the race…{unsaved}"),
                    What::World(..) => format!("Simulating the world around the camera…{unsaved}"),
                };
                self.what = Some(what);
            }
            Err(e) => s.status = e,
        }
    }

    /// Keep the run that just ended as a run file of the open mod (its Runs list), named from its race and seed.
    fn save_run(&mut self, s: &mut EditorState) {
        let dir = super::library::dir(&s.maps, &s.session.folder);
        let h = self.rec.header.clone().unwrap_or_default();
        let what = if h.race.is_empty() { "world".to_string() } else { h.race.clone() };
        let base = format!("sim-{what}-seed{}", h.seed);
        match super::library::save(&dir, &base, &std::mem::take(&mut self.raw)) {
            Ok(name) => self.shown = name,
            Err(e) => s.status = format!("The run is not saved to Runs: {e}"),
        }
    }

    /// Stop the run (and the batch's runs still to come).
    pub fn stop(&mut self) {
        self.pending.clear();
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
                self.raw.push_str(l);
                self.raw.push('\n');
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

    /// Show a saved recording (`path[@seconds[:car[:chase]]]`), paused there.
    pub fn show_file(&mut self, spec: &str) -> Result<(), String> {
        // `path[@seconds[:car[:chase]]]`: the time shown, the car followed
        // and the game's chase camera on it.
        let mut chase = false;
        let (path, at, follow) = match spec.rsplit_once('@') {
            Some((p, t)) => {
                let (t, car) = t.split_once(':').map_or((t, None), |(t, c)| {
                    let (c, cam) = c.split_once(':').unwrap_or((c, ""));
                    chase = cam == "chase";
                    (t, c.parse::<u32>().ok())
                });
                match t.parse::<f32>() {
                    Ok(t) => (p, Some(t), car),
                    Err(_) => (spec, None, None),
                }
            }
            None => (spec, None, None),
        };
        // Several runs: `a.run,b.run,…` (the last one shown).
        let mut files: Vec<&str> = path.split(',').collect();
        let path = files.pop().unwrap_or(path);
        self.stash.clear();
        for f in files {
            let text = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
            self.stash.push(Recording::read(&text));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        self.rec = Recording::read(&text);
        self.shown = std::path::Path::new(path).file_stem().map_or_else(String::new, |n| n.to_string_lossy().into());
        self.play.start();
        self.play.observe(&self.rec, false);
        if let Some(t) = at {
            self.play.seek(&self.rec, t);
        }
        self.play.follow = follow;
        if chase {
            self.cam = CamMode::Chase;
        }
        self.what = self.rec.header.as_ref().map(|h| match h.mode {
            SimMode::Race => What::Race(h.race.clone()),
            SimMode::World => What::World(Vec3::ZERO, 0.0),
        });
        Ok(())
    }

    /// Nothing shown any more (the run shown was deleted): the recording, the playback and what was read from them.
    pub fn unload(&mut self) {
        self.stop();
        self.rec = Recording::default();
        self.play = Playback::default();
        self.batch.clear();
        self.stash.clear();
        self.groups.clear();
        self.pick = None;
        self.shown.clear();
        self.parts_at = None;
        self.what = None;
    }

    /// Draw the run in `path` as ghosts over the one shown (the same race clock).
    pub fn set_ghost(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.ghost = Some(Recording::read(&text));
        self.ghost_name = path.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into());
        self.refresh_parting();
        Ok(())
    }

    pub fn clear_ghost(&mut self) {
        self.ghost = None;
        self.ghost_name.clear();
        self.parts_at = None;
    }

    /// Work out where the ghost's run parts from the one shown (its racers a metre apart at the same time, or the
    /// first state hash that differs).
    pub fn refresh_parting(&mut self) {
        self.parts_at = self.ghost.as_ref().and_then(|g| {
            let mut at: Vec<f32> = self
                .rec
                .racers()
                .into_iter()
                .filter_map(|id| {
                    let p = super::telemetry::parting(&self.rec, g, id);
                    p.apart.into_iter().chain(p.time).reduce(f32::min)
                })
                .collect();
            at.sort_by(f32::total_cmp);
            at.first().copied()
        });
    }

    /// The racers of the ghost run at the time shown.
    pub fn ghost_cars(&self) -> Vec<CarSample> {
        let Some(g) = self.ghost.as_ref() else { return Vec::new() };
        if self.play.time == f32::NEG_INFINITY {
            return Vec::new();
        }
        g.cars_at(self.play.time).into_iter().filter(|c| c.kind.races()).collect()
    }

    /// Show what went wrong in `f`: its run (the recording swapped in when
    /// another run is shown), paused there, following its car, the 3D
    /// camera and the Map on its spot.
    pub fn show_finding(&mut self, s: &mut EditorState, ctx: &egui::Context, f: &summary::Finding) {
        let seed = |r: &Recording| r.header.as_ref().map(|h| h.seed);
        if seed(&self.rec) != Some(f.seed)
            && let Some(k) = self.stash.iter().position(|r| seed(r) == Some(f.seed))
        {
            std::mem::swap(&mut self.rec, &mut self.stash[k]);
            let follow = self.play.follow;
            self.play.start();
            self.play.observe(&self.rec, false);
            self.play.follow = follow;
        }
        self.play.seek(&self.rec, f.time);
        self.play.follow = Some(f.car);
        if let Some(at) = f.at {
            let c = crate::editor::validate::Check {
                level: f.kind.level(),
                ok: false,
                text: String::new(),
                race: self.race,
                point: None,
                at: Some(at.truncate()),
                fix: None,
                wall: None,
            };
            crate::editor::validate::show(s, &c);
            request_map_centre(ctx, at.truncate());
        }
        s.status = format!("Showing: {} (seed {})", f.text, f.seed);
    }

    /// The summary again from `batch`.
    fn regroup(&mut self) {
        self.groups = summary::summarize(&self.batch);
        self.pick = None;
        // A batch that ended with problems opens on them.
        if self.pending.is_empty() && !self.running() && self.groups.iter().any(|g| g.level() == Level::Warning) {
            self.list = List::Summary;
        }
    }
}

/// Runs in one batch at most.
pub const MAX_RUNS: u32 = 10;

/// What the summary needs of the mod: its added and moved props (named),
/// and the distance to the base track's roads (read when not loaded yet).
fn findings_of(rec: &Recording, s: &mut EditorState) -> RunFindings {
    use ob_map::tools::map::format::PropEdit;
    let props: Vec<(String, Vec3)> = s
        .session
        .map
        .props
        .iter()
        .filter_map(|p| match p {
            PropEdit::Add { id, solid, position, .. } => Some((format!("{solid} ({id})"), *position)),
            PropEdit::Move { target, position, .. } => Some((format!("{} (moved)", target.solid), *position)),
            PropEdit::Delete { .. } => None,
        })
        .collect();
    let track = rec.header.as_ref().map(|h| h.track).filter(|&t| t != 0);
    if let Some(t) = track
        && !s.nets.contains_key(&t)
    {
        let net = s.assets.road_net(t);
        s.nets.insert(t, net);
    }
    let net = track.and_then(|t| s.nets.get(&t)).and_then(|n| n.as_ref());
    let road = |p: Vec2| net.and_then(|n| ob_map::tools::map::race::snap(n, p)).map_or(0.0, |r| r.distance);
    let cx = summary::Context { props: &props, road: net.is_some().then_some(&road as &dyn Fn(Vec2) -> f32) };
    summary::run_findings(rec, &cx)
}

/// Ask the Map to centre on `at` (it takes it next frame).
pub fn request_map_centre(ctx: &egui::Context, at: Vec2) {
    ctx.data_mut(|d| d.insert_temp(ctx_id("map.centre"), at));
}

/// The spot the Map was asked to centre on (taken once).
pub fn take_map_centre(ctx: &egui::Context) -> Option<Vec2> {
    ctx.data_mut(|d| d.remove_temp::<Vec2>(ctx_id("map.centre")))
}

/// Put the simulation's warnings into the Validation list (replacing the
/// last ones), so they sit beside the race checks.
fn sync_issues(sim: &mut Sim, s: &mut EditorState) {
    let now = if sim.what.as_ref().is_some_and(|w| w.mode() == SimMode::Race) {
        issues(&sim.rec, sim.race)
    } else {
        Vec::new()
    };
    let present = |i: &Issue| s.session.issues.iter().any(|x| x == i);
    if now == sim.issued && now.iter().all(present) {
        return;
    }
    s.session.issues.retain(|i| !i.text.starts_with(ISSUE_PREFIX));
    s.session.issues.extend(now.iter().cloned());
    sim.issued = now;
}

/// The frame hook: read the game, play, follow, warnings.
pub struct SimHook(pub Shared);

impl Panel for SimHook {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut sim = self.0.borrow_mut();
        if let Some(spec) = ctx.data_mut(|d| d.remove_temp::<String>(ctx_id("load"))) {
            match sim.show_file(&spec) {
                Err(e) => s.status = format!("Simulation not read: {e}"),
                Ok(()) => {
                    let sim = &mut *sim;
                    sim.batch = sim.stash.iter().chain([&sim.rec]).map(|r| findings_of(r, s)).collect();
                    sim.regroup();
                    sim.refresh_parting();
                }
            }
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        let was_running = sim.running();
        if sim.pump(dt) {
            ctx.request_repaint();
        }
        if was_running && !sim.running() {
            let race = sim.what.as_ref().is_some_and(|w| w.mode() == SimMode::Race);
            let ok = !matches!(sim.play.phase, Phase::Failed(_));
            if ok && sim.rec.end.is_some() && !sim.rec.frames.is_empty() {
                sim.save_run(s);
            }
            if race && ok {
                let f = findings_of(&sim.rec, s);
                sim.batch.push(f);
                sim.regroup();
            }
            if ok && let Some(seed) = sim.pending.pop() {
                let sim = &mut *sim;
                sim.stash.push(std::mem::take(&mut sim.rec));
                let what = sim.what.clone().expect("a race ran");
                sim.launch(s, &crate::editor::testplay::game_exe(), what, seed);
                let (k, n) = sim.batch_progress();
                s.status = format!("Simulating run {k} of {n} (seed {seed})…");
            } else {
                sim.pending.clear();
                s.status = match &sim.play.phase {
                    Phase::Failed(why) => format!("Simulation failed: {why}"),
                    _ => format!("Simulation: {}. {}", end_words(sim.rec.end.as_deref()), problems_words(&sim)),
                };
            }
        }
        if sim.play.playing || sim.running() || s.replay.loading {
            ctx.request_repaint();
        }
        // In the top view the cars are arrows on the plan, and the camera stays where the user put it.
        let plan = super::super::viewport::top_view(&ctx).is_some();
        // The cars of the time shown, for the viewport to draw as models.
        let shown = if !plan && s.replay.models {
            super::cars::replay_cars(&sim.rec, sim.play.time, &sim.play.cars(&sim.rec))
        } else {
            Vec::new()
        };
        s.replay.set(shown);
        // Ride with the car followed in the 3D view (Follow or the game's
        // chase camera).
        if !plan && let Some(id) = sim.play.follow {
            let sim = &mut *sim;
            if let Some(look) = cams::view(&sim.rec, sim.cam, &mut sim.chase_cars, &s.root, id, sim.play.time) {
                (s.camera.position, s.camera.yaw, s.camera.pitch) = (look.position, look.yaw, look.pitch);
            }
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
        let tip = if sim.running() { "Stop the simulation (what ran so far stays)".to_string() } else { tool_tip(s) };
        let r = ui
            .add(egui::Button::new(egui::RichText::new(text).font(theme::font(theme::LABEL, 15.0))))
            .on_hover_text(tip);
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

/// The toolbar button's tooltip: what it will run, then who drives.
fn tool_tip(s: &EditorState) -> String {
    match what_now(s) {
        What::Race(id) => {
            let name = s.session.map.races.iter().find(|r| r.id == id).map_or(id.clone(), |r| r.name.clone());
            format!("Simulate {name}\n{}", SIMULATE.tooltip())
        }
        What::World(..) => format!(
            "Simulate the traffic around the camera\n{}\n{SIMULATE_WORLD}\nSelect a race in the Races tree to simulate it.",
            SIMULATE.lead()
        ),
    }
}

/// How the run ended, in plain words (the stream's END reason).
fn end_words(end: Option<&str>) -> String {
    match end {
        None | Some("finish") => "finished, every racer is through".to_string(),
        Some("limit") => "stopped at the time limit".to_string(),
        Some("quit") => "stopped".to_string(),
        Some(e) => format!("ended ({e})"),
    }
}

/// How many problems the run added to Validation, in words.
fn problems_words(sim: &Sim) -> String {
    let race = sim.what.as_ref().is_some_and(|w| w.mode() == SimMode::Race);
    match (race, sim.issued.len()) {
        (false, _) => String::new(),
        (true, 0) => "No problems found.".to_string(),
        (true, 1) => "1 problem added to Validation.".to_string(),
        (true, n) => format!("{n} problems added to Validation."),
    }
}

/// The colour key for the cars drawn over the view.
fn legend(ui: &mut egui::Ui, s: &mut EditorState, traces: &mut bool) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(traces, "Lines").on_hover_text("Each racer's driven line");
        // The 3D view's cars: the game's car models (the default) or boxes
        // in the colours below.
        let mut models = s.replay.models;
        if ui.checkbox(&mut models, "Car models").on_hover_text("The cars as the game draws them; off: boxes").changed()
        {
            s.replay.set_models(models);
        }
        ui.add_space(6.0);
        ui.weak("Cars:");
        let keys = [
            (colour::SELECT, "Your slot (AI) and the car you follow"),
            (colour::INK, "Opponents"),
            (colour::DIM, "Traffic"),
            (colour::WARN, "Street racers"),
        ];
        for (c, text) in keys {
            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 2.0, c);
            ui.label(text);
            ui.add_space(6.0);
        }
    });
}

/// The race chosen, else the world at the camera's spot.
fn what_now(s: &EditorState) -> What {
    match s.race.and_then(|k| s.session.map.races.get(k)) {
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

pub(super) fn clock(t: f32) -> String {
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
        let world_run = sim.what.as_ref().is_some_and(|w| w.mode() == SimMode::World);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(SIMULATE.lead()).font(theme::font(theme::LABEL, 15.0)).color(theme::lime()));
            ui.label(if world_run { SIMULATE_WORLD } else { SIMULATE.what });
        });
        ui.horizontal_wrapped(|ui| {
            let race = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| (r.id.clone(), r.name.clone()));
            if sim.running() {
                if ui.button("Stop").clicked() {
                    sim.stop();
                }
            } else {
                let label = race.as_ref().map_or("Simulate race".to_string(), |r| format!("Simulate {}", r.1));
                let r = ui.add_enabled(race.is_some(), theme::main_action(&label));
                if r.on_hover_text(SIMULATE.what).on_disabled_hover_text("Choose a race (Races)").clicked()
                    && let Some((id, _)) = &race
                {
                    sim.start(s, &crate::editor::testplay::game_exe(), What::Race(id.clone()));
                }
                if ui.button("Simulate world").on_hover_text(SIMULATE_WORLD).clicked() {
                    let w = world_spot(s);
                    sim.start(s, &crate::editor::testplay::game_exe(), w);
                }
                ui.label("Seed").on_hover_text(modes::SEED_TIP);
                ui.add(egui::DragValue::new(&mut sim.seed).range(0..=u32::MAX as u64)).on_hover_text(modes::SEED_TIP);
                ui.label("Runs").on_hover_text(RUNS_TIP);
                ui.add(egui::DragValue::new(&mut sim.runs).range(1..=MAX_RUNS)).on_hover_text(RUNS_TIP);
            }
            ui.separator();
            let phase = match &sim.play.phase {
                Phase::Idle => "No simulation yet".to_string(),
                Phase::Starting => "Starting the game without a window…".into(),
                Phase::Running if sim.batch_progress().1 > 1 => {
                    let (k, n) = sim.batch_progress();
                    format!("Run {k} of {n}: running in the game, shown here as it arrives")
                }
                Phase::Running => "Running in the game, shown here as it arrives".into(),
                Phase::Done => {
                    let mut w = end_words(sim.rec.end.as_deref());
                    w[..1].make_ascii_uppercase();
                    w
                }
                Phase::Failed(why) => format!("Failed: {why}"),
            };
            let colour = if matches!(sim.play.phase, Phase::Failed(_)) { theme::error() } else { theme::dim() };
            ui.colored_label(colour, phase);
            if !sim.running() && sim.rec.span().is_some() {
                let words = problems_words(sim);
                if !sim.issued.is_empty() {
                    ui.colored_label(theme::warn(), words);
                    if ui.small_button("Show Validation").clicked() {
                        request_tab(ui.ctx(), "validation");
                    }
                } else if !words.is_empty() {
                    ui.weak(words);
                }
            }
        });
        let Some(_) = sim.rec.span() else {
            if !sim.running() {
                intro(ui);
            }
            return;
        };
        // The playback, the time and the camera are on the timeline under
        // the view.
        legend(ui, s, &mut sim.traces);
        ui.separator();
        let shown: Vec<_> =
            sim.rec.events.iter().filter(|e| e.kind != EventKind::Checkpoint || e.car == 0).cloned().collect();
        let race_run = sim.rec.header.as_ref().is_some_and(|h| h.mode == SimMode::Race);
        if race_run {
            // The results, what went wrong and the events share the space
            // under the legend (the browser's filter row).
            ui.horizontal_wrapped(|ui| {
                let problems = sim.groups.iter().filter(|g| g.level() == Level::Warning).count();
                let what = match sim.batch.len() {
                    0 | 1 => format!("What went wrong ({problems})"),
                    n => format!("What went wrong, {n} runs ({problems})"),
                };
                for (list, label) in [
                    (List::Results, "Results".to_string()),
                    (List::Summary, what),
                    (List::Events, format!("Events ({})", shown.len())),
                ] {
                    if ui.selectable_label(sim.list == list, label).clicked() {
                        sim.list = list;
                    }
                }
            });
        }
        match (race_run, sim.list) {
            (true, List::Summary) => {
                summary_list(ui, s, sim);
            }
            (true, List::Events) => events_list(ui, sim, &shown),
            (true, List::Results) => standings(ui, sim),
            (false, _) => {
                standings(ui, sim);
                ui.separator();
                events_list(ui, sim, &shown);
            }
        }
    }
}

/// The results table (in free roam the cars on the road).
fn standings(ui: &mut egui::Ui, sim: &Sim) {
    let world = sim.rec.header.as_ref().is_some_and(|h| h.mode == SimMode::World);
    let st = if world { Vec::new() } else { sim.rec.standings(sim.play.time) };
    if st.is_empty() {
        let cars = sim.play.cars(&sim.rec);
        let traffic = cars.iter().filter(|c| c.kind == CarKind::Traffic).count();
        let racers = cars.iter().filter(|c| c.kind == CarKind::Encounter).count();
        ui.weak(format!("{traffic} traffic cars and {racers} street racers on the road"));
    } else {
        let player = |r: &super::record::Standing| sim.rec.cars.get(&r.car).is_some_and(|c| c.kind == CarKind::Player);
        let st: Vec<_> = st.into_iter().filter(|r| modes::SHOW_AI_SLOT || !player(r)).collect();
        if st.iter().any(player) {
            ui.colored_label(theme::dim(), modes::SLOT_NOTE);
        }
        results_table(ui, sim, &st);
    }
}

/// The Runs field's tooltip.
const RUNS_TIP: &str = "Simulate the race this many times, one seed after the other (Seed, Seed + 1, …); \
What went wrong then counts how often each problem happened";

/// What went wrong: one line per problem and place, its occurrences under
/// it; a click (or Enter, or Up / Down between the rows) shows it. True
/// while a row has the keyboard focus.
fn summary_list(ui: &mut egui::Ui, s: &mut EditorState, sim: &mut Sim) -> bool {
    if sim.groups.is_empty() {
        ui.weak(if sim.running() || !sim.pending.is_empty() {
            "What went wrong shows when the run has ended"
        } else {
            "Nothing went wrong: no car got stuck, crashed, missed a checkpoint or left the roads"
        });
        return false;
    }
    let runs = sim.batch.len();
    // The rows: (group, item: None for the group's own line).
    let mut rows: Vec<(usize, Option<usize>)> = Vec::new();
    for (g, grp) in sim.groups.iter().enumerate() {
        rows.push((g, None));
        rows.extend((0..grp.items.len().min(summary::SHOWN)).map(|i| (g, Some(i))));
    }
    let mut go: Option<usize> = None;
    let mut ids: Vec<egui::Id> = Vec::with_capacity(rows.len());
    let left = (ui.clip_rect().bottom() - ui.cursor().top() - 4.0).max(80.0);
    egui::ScrollArea::vertical().id_salt("sim.summary").max_height(left).auto_shrink([false, true]).show(ui, |ui| {
        for (k, &(g, item)) in rows.iter().enumerate() {
            let grp = &sim.groups[g];
            let picked = sim.pick == Some(k);
            let r = ui
                .horizontal(|ui| match item {
                    None => {
                        let mark = crate::editor::validate::Check {
                            level: grp.level(),
                            ok: false,
                            text: String::new(),
                            race: None,
                            point: None,
                            at: None,
                            fix: None,
                            wall: None,
                        };
                        crate::editor::validate::mark(ui, &mark);
                        let r = ui.selectable_label(picked, &grp.text);
                        if grp.items.len() > summary::SHOWN {
                            ui.weak(format!("and {} more", grp.items.len() - summary::SHOWN));
                        }
                        r
                    }
                    Some(i) => {
                        ui.add_space(22.0);
                        let text = summary::item_text(&grp.items[i], runs);
                        ui.selectable_label(picked, egui::RichText::new(text).color(theme::dim()))
                    }
                })
                .inner;
            if r.clicked() {
                go = Some(k);
            }
            if picked && sim.scroll_pick {
                r.scroll_to_me(None);
            }
            ids.push(r.id);
        }
    });
    sim.scroll_pick = false;
    sim.row_ids = ids.clone();
    // The keyboard (Tab, or Up / Down, which egui moves the focus with)
    // brought the focus to another row: show it.
    let focused = ui.ctx().memory(|m| m.focused());
    let has_focus = ids.iter().position(|&id| focused == Some(id));
    if has_focus.is_some() && has_focus != sim.focus_row && go.is_none() {
        go = has_focus;
        sim.scroll_pick = true;
    }
    sim.focus_row = has_focus;
    if let Some(k) = go {
        let (g, item) = rows[k];
        let f = sim.groups[g].items[item.unwrap_or(0)].clone();
        sim.pick = Some(k);
        sim.show_finding(s, ui.ctx(), &f);
    }
    has_focus.is_some() || go.is_some()
}

/// The events, newest first, each with Show.
fn events_list(ui: &mut egui::Ui, sim: &mut Sim, shown: &[super::format::Event]) {
    if shown.is_empty() {
        ui.weak("No events yet");
    }
    let mut seek: Option<(f32, u32)> = None;
    // The events scroll on their own in the space left under the table
    // (the table stays in view).
    let left = (ui.clip_rect().bottom() - ui.cursor().top() - 4.0).max(80.0);
    egui::ScrollArea::vertical().id_salt("sim.events").max_height(left).auto_shrink([false, true]).show(ui, |ui| {
        for e in shown.iter().rev().take(200) {
            ui.horizontal(|ui| {
                let c = if e.kind.is_problem() { theme::warn() } else { theme::dim() };
                ui.monospace(clock(e.time));
                let who = if e.kind == EventKind::Start { String::new() } else { format!("{}: ", sim.rec.name(e.car)) };
                ui.colored_label(c, format!("{who}{}", event_text(e)));
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
}

/// The results: place, car, time, best lap, laps; the followed car in lime,
/// a racer without a finish when the run ended "did not finish".
fn results_table(ui: &mut egui::Ui, sim: &Sim, st: &[super::record::Standing]) {
    egui::Grid::new("sim.results").striped(true).num_columns(6).min_col_width(56.0).show(ui, |ui| {
        let laps = sim.rec.header.as_ref().map_or(0, |h| h.laps);
        for h in ["Place", "Car", "Time", "Best lap", "Laps done"] {
            ui.strong(h);
        }
        ui.end_row();
        for r in st {
            ui.label(if r.place == 0 { "-".to_string() } else { r.place.to_string() });
            let name = sim.rec.name(r.car);
            let car =
                if sim.play.follow == Some(r.car) { ui.colored_label(theme::lime(), name) } else { ui.label(name) };
            if sim.rec.cars.get(&r.car).is_some_and(|c| c.kind == CarKind::Player) {
                car.on_hover_text(format!(
                    "The player's slot ({}), driven by the AI in a simulation",
                    label_name(&sim.rec, r.car)
                ));
            }
            if r.dnf {
                ui.colored_label(theme::warn(), "did not finish");
            } else if let Some(t) = r.time {
                ui.monospace(clock(t));
            } else {
                ui.weak(if r.place == 0 { "on the grid" } else { "racing" });
            }
            ui.monospace(r.best.map_or("-".into(), clock));
            let done = r.laps.len();
            ui.monospace(if laps > 0 { format!("{done} of {laps}") } else { done.to_string() });
            if sim.rec.cars.get(&r.car).is_some_and(|c| c.kind == CarKind::Player) {
                ui.weak(modes::SLOT_ROW);
            }
            ui.end_row();
        }
    });
}

/// An event in words ("lap 2 in 1:02.3").
fn event_text(e: &super::format::Event) -> String {
    match e.kind {
        EventKind::Start => "Green light".to_string(),
        EventKind::Checkpoint => format!("checkpoint {}", e.checkpoint),
        EventKind::Lap => format!("lap {} in {}", e.checkpoint, clock(e.value)),
        EventKind::Finish => format!("finished in {}", clock(e.value)),
        EventKind::Crash => format!("crash ({:.0} km/h lost)", e.value * 3.6),
        EventKind::Stuck => format!("stuck before checkpoint {}", e.checkpoint),
        EventKind::WrongWay => format!("wrong way before checkpoint {}", e.checkpoint),
    }
}

/// The empty tab: the three ways to try a race and the help page.
fn intro(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.weak("To drive it yourself:");
    for m in modes::ALL.into_iter().filter(|m| m.name != SIMULATE.name) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(m.lead());
            ui.label(m.what);
        });
    }
    ui.add_space(4.0);
    if ui.button(format!("Help: {}", modes::HELP_TITLE)).clicked() {
        modes::request_help(ui.ctx());
    }
}

/// A car's colour: the followed one lime, the player's slot lime, racers
/// ink, traffic grey (a 1.6-point outline, so it shows over the city), street
/// racers amber.
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
        // The live positions first: over them the view does not pick.
        let taken = super::timeline::board(ctx, &mut self.0.borrow_mut(), rect);
        let sim = self.0.borrow();
        let cars = sim.play.cars(&sim.rec);
        if cars.is_empty() {
            return taken;
        }
        let traces: Vec<(u32, Vec<Vec<Vec3>>)> = if sim.traces {
            sim.rec.racers().into_iter().map(|id| (id, sim.rec.trace(id, sim.play.time, 6))).collect()
        } else {
            Vec::new()
        };
        // The places the results table shows (none before the green).
        let race = sim.rec.header.as_ref().is_some_and(|h| h.mode == SimMode::Race);
        let st = if race { sim.rec.standings(sim.play.time) } else { Vec::new() };
        let place = |id: u32| st.iter().find(|r| r.car == id && r.place > 0).map(|r| r.place);
        let trace_colour = |id: u32| {
            let c = if id == 0 || sim.play.follow == Some(id) { colour::SELECT } else { colour::INK };
            c.gamma_multiply(0.55)
        };
        match map_transform(ctx) {
            None => {
                let o = Overlay::new(painter, &s.camera, rect);
                for (id, pieces) in &traces {
                    for p in pieces {
                        let lifted: Vec<Vec3> = p.iter().map(|q| *q + Vec3::Z * 0.3).collect();
                        o.polyline(&lifted, false, trace_colour(*id), 1.5);
                    }
                }
                for c in &cars {
                    let followed = sim.play.follow == Some(c.id);
                    let t = Mat4::from_rotation_translation(c.rotation, c.position);
                    let col = car_colour(c, followed);
                    let w = if c.kind.races() { 2.0 } else { 1.6 };
                    // A car drawn as its model needs no box.
                    if !s.replay.shows(c.id) {
                        o.box_outline(t, -CAR_HALF + Vec3::Z * CAR_BOTTOM, CAR_HALF + Vec3::Z * CAR_TOP, col, w);
                    }
                    if let Some(p) = place(c.id) {
                        o.label(
                            c.position + Vec3::Z * 2.2,
                            &format!("P{p} {}", short(&label_name(&sim.rec, c.id))),
                            col,
                        );
                    }
                }
                // The second run's racers as ghosts: outlines in the info colour.
                let gcol = ghost_colour();
                for c in sim.ghost_cars() {
                    let t = Mat4::from_rotation_translation(c.rotation, c.position);
                    o.box_outline(t, -CAR_HALF + Vec3::Z * CAR_BOTTOM, CAR_HALF + Vec3::Z * CAR_TOP, gcol, 1.5);
                    let g = sim.ghost.as_ref().expect("ghost cars come from a ghost");
                    o.label(c.position + Vec3::Z * 2.2, &format!("B {}", short(&label_name(g, c.id))), gcol);
                }
            }
            Some((centre, scale, mrect)) => {
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
                for c in &cars {
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
                    if let Some(p) = place(c.id) {
                        label2d(painter, at - egui::vec2(0.0, len + 8.0), &format!("P{p}"), col);
                    }
                    if followed {
                        line2d(painter, at - egui::vec2(len, 0.0), at + egui::vec2(len, 0.0), col, 1.0);
                    }
                }
                let gcol = ghost_colour();
                for c in sim.ghost_cars() {
                    let at = to(c.position);
                    if !rect.expand(20.0).contains(at) {
                        continue;
                    }
                    let h = c.heading();
                    let len = (4.4 * scale).max(9.0);
                    let d = egui::vec2(h.cos(), -h.sin());
                    let side = egui::vec2(-d.y, d.x) * len * 0.45;
                    let tip = at + d * len * 0.6;
                    let back = at - d * len * 0.4;
                    painter.add(egui::Shape::closed_line(vec![tip, back + side, back - side], Stroke::new(1.5, gcol)));
                }
            }
        }
        taken
    }
}

/// The second run's colour: the look's info token.
pub fn ghost_colour() -> Color32 {
    theme::rgb(theme::tokens().info)
}

/// The car's own name for a label (its model, not "Your slot (AI)").
pub(super) fn label_name(rec: &Recording, id: u32) -> String {
    rec.cars.get(&id).map(|c| c.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| rec.name(id))
}

/// A name short enough for a label over a car.
pub(super) fn short(name: &str) -> String {
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
        s.session.issues.push(Issue { level: Level::Error, text: "no road".into(), race: Some(0), at: None });
        let mut sim = Sim { rec, what: Some(What::Race("r".into())), race: Some(0), ..Default::default() };
        sync_issues(&mut sim, &mut s);
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.session.issues.len(), 2, "the race's own issue kept, the warning once");
        // The race checks rebuilt the list: the warning comes back.
        s.session.issues.retain(|i| !i.text.starts_with(ISSUE_PREFIX));
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.session.issues.len(), 2);
        // A new run without problems clears it.
        sim.rec = Recording::default();
        sync_issues(&mut sim, &mut s);
        assert_eq!(s.session.issues.len(), 1);
    }

    /// A run that ended is kept in the mod's Runs list, named from its race and seed, as the game streamed it.
    #[test]
    fn a_run_that_ended_is_kept_as_a_run_file() {
        let maps = crate::testkit::Scratch::new("sim-keeps-run");
        let mut s = EditorState::new(
            std::path::PathBuf::new(),
            "L4RA",
            std::path::PathBuf::new(),
            "m",
            Default::default(),
            crate::world::World::default(),
            crate::draw::camera::Camera::new(Vec3::ZERO, 0.0, 0.0),
        );
        s.maps = maps.path().to_path_buf();
        let text: String =
            super::super::summary::tests::lines(7, true, false).iter().map(|l| l.write() + "\n").collect();
        let mut sim = Sim { rec: Recording::read(&text), raw: text.clone(), ..Default::default() };
        sim.save_run(&mut s);
        assert_eq!(sim.shown, "sim-r-seed7");
        let dir = super::super::library::dir(maps.path(), "m");
        assert_eq!(std::fs::read_to_string(super::super::library::path(&dir, "sim-r-seed7")).unwrap(), text);
        // The same seed again does not replace it.
        sim.raw = text;
        sim.save_run(&mut s);
        assert_eq!(sim.shown, "sim-r-seed7-2");
        assert_eq!(super::super::library::list(&dir).len(), 2);
    }

    #[test]
    fn how_a_run_ended_in_words() {
        assert_eq!(end_words(Some("finish")), "finished, every racer is through");
        assert_eq!(end_words(Some("limit")), "stopped at the time limit");
        assert_eq!(end_words(Some("quit")), "stopped");
        let mut sim = Sim { what: Some(What::Race("r".into())), ..Default::default() };
        assert_eq!(problems_words(&sim), "No problems found.");
        sim.issued = vec![Issue { level: Level::Warning, text: "x".into(), race: None, at: None }; 2];
        assert_eq!(problems_words(&sim), "2 problems added to Validation.");
    }

    #[test]
    fn clock_and_short_names() {
        assert_eq!(clock(83.41), "1:23.4");
        assert_eq!(clock(-2.5), "-0:02.5");
        assert_eq!(short("Mitsubishi Lancer Evolution"), "Mitsubishi Lan.");
    }
}
