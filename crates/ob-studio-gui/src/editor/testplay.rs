//! Test race and Drive here: the editor writes the mod as it is (saved or
//! not) to its run copy (`.editor/run/map.obmap`; `map.obmap` is only
//! written by Save), starts the game beside itself with only that copy
//! (`game --map-file <run copy> --map-race <id>` or `--map-at
//! x,y,z,heading`, `--map-result <file>`, and `--looks-live
//! <mod>/textures`: the mod's painted textures, shown and reloaded live in
//! that run, docs/modding/studio-looks.md), keeps editing while it runs,
//! and reads the run's result line when the game has ended. A race that
//! finished is marked tested by its content (`validate::Tested`,
//! `.editor/tested.txt`, never exported), so the mark holds after the same
//! race is saved and goes when it changes. One test at a time.
//!
//! The game stays open and is linked (`--studio-link <mod>/.editor/link`,
//! `tools::map::link`: two small files in the player's own data folder, no
//! network): edits to the mod (props, race points) go to the run copy and
//! the game starts the run over with them between two frames; Test race and
//! Drive here while it runs send the new aim; Restart race starts over. The
//! status bar says whether the game is connected. Moved or added props reach
//! the game by a reload of the region's world (the loading screen, a few
//! seconds: the placements are set up when the world is built, and a prop
//! moved in place is not yet taken by every part of the game), and the
//! status bar says so while it reloads. The game is probed every few
//! seconds (a command it must answer), at most `link::MAX_RUNS` runs are
//! open at once over every window, and the run starts with the focus
//! (`EditorState::run_focus`: car and world clock).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use glam::Vec3;

use super::state::{EditorState, Level};
use crate::tools::map::link::{self, Aim, Op};
use crate::tools::map::{CustomRace, MapMod, store};

/// The result file's name in the mod's `.editor/` folder.
pub const RESULT_FILE: &str = crate::tools::map::store::TEST_RESULT;

/// What the game is asked to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Race the custom race with this id, from its grid.
    Race(String),
    /// Free roam from this spot (heading in degrees around +z from +x).
    At(Vec3, f32),
}

/// The game's executable beside the editor's (`game`, `game.exe`);
/// `OB_GAME_EXE` names another (tests).
pub fn game_exe() -> PathBuf {
    crate::platform::dirs::game_exe()
}

/// The game's command line for a test run of the mod file `file`, with the
/// mod's texture folder `looks` (`<mod>/textures`; watched in that run, so a
/// texture painted while it runs shows too).
pub fn args(file: &Path, target: &Target, result: &Path, looks: &Path, link: &Path, focus: &link::Focus) -> Vec<String> {
    run_args(file, target, result, looks, link, focus, None)
}

/// [`args`] for a run that is kept (Drive to record): the game also writes its run file to `record`, with the
/// drive channels and the state hash of every tick (`--record`, `--run-channels`, docs/modding/studio.md, "Runs").
pub fn run_args(
    file: &Path,
    target: &Target,
    result: &Path,
    looks: &Path,
    link: &Path,
    focus: &link::Focus,
    record: Option<&Path>,
) -> Vec<String> {
    let mut a = vec!["--map-file".to_string(), file.display().to_string()];
    match target {
        Target::Race(id) => a.extend(["--map-race".to_string(), id.clone()]),
        Target::At(p, h) => a.extend(["--map-at".to_string(), format!("{},{},{},{}", p.x, p.y, p.z, h)]),
    }
    a.extend(["--map-result".to_string(), result.display().to_string()]);
    a.extend(["--looks-live".to_string(), looks.display().to_string()]);
    a.extend(["--studio-link".to_string(), link.display().to_string()]);
    a.extend(focus.args());
    if let Some(r) = record {
        a.extend(["--record".to_string(), r.display().to_string(), "--run-channels".to_string(), "drive,hash".to_string()]);
    }
    a
}

/// A test run's result as the game wrote it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The race was finished (its results shown).
    pub finished: bool,
    pub place: Option<usize>,
    pub of: Option<usize>,
    /// Seconds.
    pub time: Option<f32>,
    pub laps: Option<u32>,
    /// A free-roam run ended normally.
    pub drove: bool,
}

impl Outcome {
    /// `finished=yes place=1 of=4 time=83.412 laps=2` (any order; unknown
    /// keys ignored, so a newer game's line still reads).
    pub fn parse(line: &str) -> Outcome {
        let mut o = Outcome::default();
        for (k, v) in line.split_whitespace().filter_map(|w| w.split_once('=')) {
            match k {
                "finished" => o.finished = v == "yes",
                "drove" => o.drove = v == "yes",
                "place" => o.place = v.parse().ok(),
                "of" => o.of = v.parse().ok(),
                "time" => o.time = v.parse().ok(),
                "laps" => o.laps = v.parse().ok(),
                _ => {}
            }
        }
        o
    }

    /// The status bar's words.
    pub fn words(&self, label: &str) -> String {
        match (self.finished, self.place, self.of, self.time) {
            (true, Some(p), Some(n), Some(t)) => {
                format!("Tested {label}: finished {p} of {n} in {}:{:06.3}", (t / 60.0) as u32, t % 60.0)
            }
            (true, ..) => format!("Tested {label}: finished"),
            _ if self.drove => format!("Back from driving ({label})"),
            _ => format!("Test of {label} left before the finish"),
        }
    }
}

/// A test run in progress.
#[derive(Debug)]
pub struct TestPlay {
    child: Child,
    target: Target,
    /// What the status bar calls it ("Casino sprint").
    pub label: String,
    result: PathBuf,
    /// The race as it was for this test (to mark it tested).
    race: Option<CustomRace>,
    /// The mod's `.editor/` folder (the tested marks).
    dir: PathBuf,
    /// The link folder (`--studio-link`).
    link: PathBuf,
    /// The run this test keeps (Drive to record): its name in the Runs list.
    recording: Option<String>,
    /// The last command number sent.
    seq: u64,
    /// The mod as the game was last given it.
    sent: MapMod,
    /// What the game said last (None: nothing yet).
    game: Option<link::GameState>,
    /// This run's place among the few ([`link::Slot`]).
    slot: link::Slot,
    /// The last command number that starts the run over (an apply or a
    /// restart); a probe's is not counted.
    work: u64,
    /// The last such command reloads the world (props changed).
    reloading: bool,
    /// The focus as the game was last given it.
    focus_sent: link::Focus,
    /// The probe sent and when (None: none waiting for its answer).
    probe: Option<(u64, std::time::Instant)>,
    /// When the next probe is due, and how often they go.
    probe_next: std::time::Instant,
    probe_every: std::time::Duration,
}

/// How long a probe may wait for its answer before the game counts as not
/// answering, and how often one is sent.
const PROBE_WAIT: std::time::Duration = std::time::Duration::from_secs(3);
const PROBE_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// The status bar's words for the link: whether the game is connected.
pub const GAME_CONNECTED: &str = "Game connected";
pub const GAME_NOT_CONNECTED: &str = "Game not connected";
pub const GAME_APPLYING: &str = "Game applying changes";
pub const GAME_RELOADING: &str = "Game reloading the world";
pub const GAME_NO_ANSWER: &str = "Game not answering";

/// How a test run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Ended {
    /// The game wrote its result.
    Done(Outcome),
    /// The game ended without one (it failed to start the race, or
    /// crashed): its exit status.
    Failed(String),
}

/// Why a test cannot start (the status bar shows it).
pub fn blocked(s: &EditorState, target: &Target) -> Option<String> {
    if let Target::Race(id) = target {
        let Some(k) = s.session.map.races.iter().position(|r| &r.id == id) else {
            return Some(format!("No race {id} in this mod"));
        };
        if s.session.map.races[k].checkpoints.len() < 2 {
            return Some("A race needs 2 checkpoints or more".into());
        }
        if let Some(e) = s.session.issues.iter().find(|i| i.level == Level::Error && i.race.is_none_or(|r| r == k)) {
            return Some(format!("Cannot test: {}", e.text));
        }
    }
    None
}

impl TestPlay {
    /// Write the run copy (the game reads it at its first world load) and
    /// start the game with `exe`. The editor keeps its state, unsaved
    /// changes included; only the status changes.
    pub fn start(s: &mut EditorState, exe: &Path, target: Target) -> Result<TestPlay, String> {
        TestPlay::start_run(s, exe, target, false)
    }

    /// [`TestPlay::start`], with `record` the drive is kept as a run in the mod's runs folder
    /// (`.editor/runs/drive-<race>.obrun`, never over another run).
    pub fn start_run(s: &mut EditorState, exe: &Path, target: Target, record: bool) -> Result<TestPlay, String> {
        if let Some(why) = blocked(s, &target) {
            return Err(why);
        }
        if !s.run_focus.valid() {
            return Err("The test car or clock is not one the game takes".into());
        }
        let slot = link::Slot::take(&s.maps)?;
        let file = write_run_copy(s)?;
        let dir = store::editor_dir(&s.maps, &s.session.folder);
        let result = dir.join(RESULT_FILE);
        let _ = std::fs::remove_file(&result);
        let link = link::dir(&s.maps, &s.session.folder);
        link::clear(&link);
        let race = match &target {
            Target::Race(id) => s.session.map.races.iter().find(|r| &r.id == id).cloned(),
            Target::At(..) => None,
        };
        let label = race.as_ref().map_or_else(|| "free roam".to_string(), |r| r.name.clone());
        let recording = if record {
            let base = match &target {
                Target::Race(id) => format!("drive-{id}"),
                Target::At(..) => "drive-free-roam".to_string(),
            };
            let dir = super::sim::library::dir(&s.maps, &s.session.folder);
            let name = super::sim::library::free_name(&dir, &base)?;
            Some((super::sim::library::path(&dir, &name), name))
        } else {
            None
        };
        let child = Command::new(exe)
            .args(run_args(
                &file,
                &target,
                &result,
                &s.maps.join(&s.session.folder).join(store::TEXTURES),
                &link,
                &s.run_focus,
                recording.as_ref().map(|r| r.0.as_path()),
            ))
            .env("NFSU2_ROOT", &s.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("The game could not start ({}): {e}", exe.display()))?;
        let unsaved = if s.session.dirty { " (testing unsaved changes)" } else { "" };
        s.status = match (&target, &recording) {
            (Target::Race(_), Some((_, n))) => format!("Recording {label} as {n}…{unsaved}"),
            (Target::At(..), Some((_, n))) => format!("Recording a drive from the camera's spot as {n}…{unsaved}"),
            (Target::Race(_), None) => format!("Testing {label}…{unsaved}"),
            (Target::At(..), None) => format!("Driving from the camera's spot…{unsaved}"),
        };
        let sent = s.session.map.clone();
        Ok(TestPlay {
            child,
            target,
            label,
            result,
            race,
            dir,
            link,
            seq: 0,
            sent,
            game: None,
            slot,
            recording: recording.map(|r| r.1),
            work: 0,
            reloading: false,
            focus_sent: s.run_focus.clone(),
            probe: None,
            probe_next: std::time::Instant::now() + PROBE_EVERY,
            probe_every: PROBE_EVERY,
        })
    }

    /// The run's end once the game has ended (None while it runs).
    pub fn poll(&mut self) -> Option<Ended> {
        let status = match self.child.try_wait() {
            Ok(None) => return None,
            Ok(Some(st)) => st,
            Err(e) => return Some(Ended::Failed(e.to_string())),
        };
        Some(match std::fs::read_to_string(&self.result) {
            Ok(line) => Ended::Done(Outcome::parse(&line)),
            Err(_) => Ended::Failed(status.to_string()),
        })
    }

    pub fn target(&self) -> &Target {
        &self.target
    }

    /// The game's words now (read from its state file): connected, applying
    /// (a command it has not dealt with yet), or not connected (it has
    /// not said anything lately).
    pub fn link_words(&mut self) -> &'static str {
        self.slot.touch();
        self.game = link::read_state(&self.link).filter(|g| g.pid == self.child.id());
        if let (Some(g), Some((q, _))) = (&self.game, self.probe)
            && g.seq >= q
        {
            self.probe = None;
        }
        match &self.game {
            Some(g) if !g.alive(link::now_ms()) => GAME_NOT_CONNECTED,
            Some(g) if g.seq < self.work => {
                if self.reloading {
                    GAME_RELOADING
                } else {
                    GAME_APPLYING
                }
            }
            Some(_) if self.probe.is_some_and(|(_, at)| at.elapsed() > PROBE_WAIT) => GAME_NO_ANSWER,
            Some(_) => GAME_CONNECTED,
            None => GAME_NOT_CONNECTED,
        }
    }

    /// Send a probe when one is due: the game is there, has dealt with
    /// everything sent, and none waits for its answer. The command file
    /// holds one command, so a probe never goes over one the game has not
    /// read.
    pub fn probe_due(&mut self) -> Result<(), String> {
        let idle = self.game.as_ref().is_some_and(|g| g.seq >= self.seq && g.alive(link::now_ms()));
        if !idle || self.probe.is_some() || std::time::Instant::now() < self.probe_next {
            return Ok(());
        }
        self.seq += 1;
        self.probe = Some((self.seq, std::time::Instant::now()));
        self.probe_next = std::time::Instant::now() + self.probe_every;
        link::write_command(&self.link, &link::Command { seq: self.seq, op: Op::Probe, aim: None, focus: None })
            .map_err(|e| format!("The game was not probed: {e}"))
    }

    /// Whether the game has said it is there (a command may be sent).
    pub fn connected(&mut self) -> bool {
        self.link_words() != GAME_NOT_CONNECTED
    }

    /// Send the game a command: the mod as it is in the editor to the run
    /// copy first (for `Apply`), then the command.
    pub fn send(&mut self, s: &EditorState, op: Op, aim: Option<Aim>) -> Result<(), String> {
        if !s.run_focus.valid() {
            return Err("The test car or clock is not one the game takes".into());
        }
        let mut reloading = false;
        if op == Op::Apply {
            write_run_copy(s)?;
            // A reload still owed stays one (this command takes its place).
            let owed = self.reloading && self.game.as_ref().is_none_or(|g| g.seq < self.work);
            reloading = owed || s.session.map.props != self.sent.props;
            self.sent = s.session.map.clone();
        }
        let focus = (self.focus_sent != s.run_focus).then(|| s.run_focus.clone());
        self.seq += 1;
        link::write_command(&self.link, &link::Command { seq: self.seq, op, aim, focus: focus.clone() })
            .map_err(|e| format!("The game was not told: {e}"))?;
        // Written: what the game now owes (a probe over it is lost).
        (self.work, self.reloading, self.probe) = (self.seq, reloading, None);
        if focus.is_some() {
            self.focus_sent = s.run_focus.clone();
        }
        Ok(())
    }

    /// The mod changed since the game was last given it.
    pub fn changed(&self, s: &EditorState) -> bool {
        s.session.map != self.sent || s.run_focus != self.focus_sent
    }

    /// What the game said about the last command: its note when it could
    /// not do it.
    pub fn problem(&self) -> Option<String> {
        self.game.as_ref().filter(|g| !g.ok && g.seq == self.work).map(|g| g.note.clone())
    }
}

/// The mod as it is in the editor written to its run copy
/// (`store::run_file`) for the game; `map.obmap` and `dirty` stay as they
/// are. A mod without a name is given its folder's.
pub fn write_run_copy(s: &EditorState) -> Result<PathBuf, String> {
    let mut m = s.session.map.clone();
    if m.name.trim().is_empty() {
        m.name = s.session.folder.clone();
    }
    store::write_run_copy(&s.maps, &s.session.folder, &m).map_err(|e| format!("The run copy was not written: {e:#}"))
}

/// Look at a running test: when it ended, the status and (a finished
/// race) the tested mark. Some(marked) when it ended.
pub fn poll(s: &mut EditorState, test: &mut Option<TestPlay>) -> Option<bool> {
    let t = test.as_mut()?;
    let end = t.poll()?;
    let mut marked = false;
    s.status = match &end {
        Ended::Done(o) => {
            if o.finished
                && let Some(r) = &t.race
            {
                match super::validate::mark_tested(&t.dir, r) {
                    Ok(()) => marked = true,
                    Err(e) => log::warn!("tested mark not kept: {e:#}"),
                }
            }
            o.words(&t.label)
        }
        Ended::Failed(how) => format!("The game ended without a result ({how}); see its log"),
    };
    if let Some(name) = &t.recording {
        s.status = format!("{} · kept as the run {name} (Runs)", s.status);
    }
    *test = None;
    Some(marked)
}

/// The window's part: Test race (F5) and Drive here (Shift+F5) from the
/// menu's commands, the running test polled each frame (a hook of
/// `app::Panels`, no visible place of its own: it speaks in the status bar).
#[derive(Default)]
pub struct TestPanel {
    pub test: Option<TestPlay>,
    /// The race tools, told to read the tested marks again after a test
    /// marked one.
    pub races: Option<std::rc::Rc<std::cell::RefCell<super::races::Races>>>,
    /// The last problem the game reported, so the status says it once.
    pub told: Option<String>,
}

/// How often a running test is looked at while the window is idle.
const POLL_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

fn started_id() -> egui::Id {
    egui::Id::unique("editor.testplay.started")
}

/// How many test runs this window started (Studio's tutorial ticks its Drive it step on it).
pub fn started(ctx: &egui::Context) -> u64 {
    ctx.data(|d| d.get_temp::<u64>(started_id()).unwrap_or(0))
}

/// Count one more started test run.
pub fn count_start(ctx: &egui::Context) {
    ctx.data_mut(|d| *d.get_temp_mut_or_default::<u64>(started_id()) += 1);
}

fn strip_id() -> egui::Id {
    egui::Id::unique("editor.testplay.strip")
}

/// The game-window strip's words while a test runs (the shell draws it
/// over the view; `modes::strip`).
pub fn strip(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<String>(strip_id()))
}

impl super::state::Panel for TestPanel {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        self.run(s, ui);
        let words = self.test.as_ref().map(|t| super::modes::strip(t.race.as_ref().map(|r| r.name.as_str())));
        set_strip(ui.ctx(), words);
    }
}

/// Set (Some) or clear the game-window strip's words: a test runs (Studio's status bar and Jobs tab show it too).
pub fn set_strip(ctx: &egui::Context, words: Option<String>) {
    ctx.data_mut(|d| match words {
        Some(w) => {
            d.insert_temp(strip_id(), w);
        }
        None => {
            d.remove::<String>(strip_id());
        }
    });
}

impl TestPanel {
    fn run(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let race = super::menu::take(&ctx, super::menu::Command::TestRace);
        let drive = super::menu::take(&ctx, super::menu::Command::DriveHere);
        if let Some(marked) = poll(s, &mut self.test) {
            if marked && let Some(r) = &self.races {
                r.borrow_mut().reload_tested();
            }
            ctx.request_repaint();
        }
        let restart = super::menu::take(&ctx, super::menu::Command::RestartTest);
        let record = super::menu::take(&ctx, super::menu::Command::RecordDrive);
        if record && self.test.is_some() {
            // The game writes its run from its start: the open test stays as it is.
            s.status = "A test is open: close its game window, then Drive to record starts a new one".into();
            return;
        }
        if let Some(t) = self.test.as_mut() {
            let (words, quiet) = (t.link_words(), t.problem());
            if let Err(e) = t.probe_due() {
                s.status = e;
            }
            set_link(&ctx, Some(words.to_string()));
            let aim = if race {
                s.race.and_then(|k| s.session.map.races.get(k)).map(|r| Aim::Race(r.id.clone()))
            } else if drive {
                Some(match drive_spot(s) {
                    Target::At(p, h) => Aim::At(p.x, p.y, p.z, h),
                    Target::Race(id) => Aim::Race(id),
                })
            } else {
                None
            };
            let ask = race || drive || restart;
            if ask && !t.connected() {
                s.status = "The game is not connected yet; try again in a moment".into();
            } else if ask {
                if race && aim.is_none() {
                    s.status = "Choose a race to test (Races)".into();
                } else if let Some(why) = aim.as_ref().and_then(|a| match a {
                    Aim::Race(id) => blocked(s, &Target::Race(id.clone())),
                    Aim::At(..) => None,
                }) {
                    s.status = why;
                } else {
                    let op = if t.changed(s) || aim.is_some() { Op::Apply } else { Op::Restart };
                    s.status = match t.send(s, op, aim) {
                        Ok(()) => if restart { "Race started over".into() } else { "Test updated in the open game".into() },
                        Err(e) => e,
                    };
                }
            } else if t.changed(s) && t.connected() && blocked_now(s, t).is_none() {
                // An edit (a prop moved, a point added): the game takes the new mod.
                if let Err(e) = t.send(s, Op::Apply, None) {
                    s.status = e;
                }
            }
            if let Some(p) = quiet.filter(|p| self.told.as_ref() != Some(p)) {
                s.status = format!("The game kept the old run: {p}");
                self.told = Some(p);
            }
            ctx.request_repaint_after(POLL_EVERY);
            return;
        }
        set_link(&ctx, None);
        self.told = None;
        if restart {
            s.status = "No test is running (F5 starts one)".into();
        }
        let target = if race || record {
            match s.race.and_then(|k| s.session.map.races.get(k)) {
                Some(r) => Some(Target::Race(r.id.clone())),
                None if record => Some(drive_spot(s)),
                None => {
                    s.status = "Choose a race to test (Races)".into();
                    None
                }
            }
        } else if drive {
            Some(drive_spot(s))
        } else {
            None
        };
        if let Some(t) = target {
            match TestPlay::start_run(s, &game_exe(), t, record) {
                Ok(t) => {
                    self.test = Some(t);
                    count_start(&ctx);
                }
                Err(e) => s.status = e,
            }
            ctx.request_repaint_after(POLL_EVERY);
        }
    }
}

/// Whether an edit must wait: a race of the test with a blocking issue.
fn blocked_now(s: &EditorState, t: &TestPlay) -> Option<String> {
    blocked(s, t.target())
}

fn link_id() -> egui::Id {
    egui::Id::unique("editor.testplay.link")
}

/// The status bar's words for the game link while a test runs (None: no test).
pub fn link_words(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<String>(link_id()))
}

fn set_link(ctx: &egui::Context, words: Option<String>) {
    ctx.data_mut(|d| match words {
        Some(w) => {
            d.insert_temp(link_id(), w);
        }
        None => {
            d.remove::<String>(link_id());
        }
    });
}

/// Drive here's spot: the ground the camera looks at (within 300 m), else
/// the ground under the camera, else the camera's own place; facing the
/// camera's way.
pub fn drive_spot(s: &EditorState) -> Target {
    let (eye, f) = (s.camera.position, s.camera.forward());
    let at = crate::tools::map::edit::ray_ground(&s.session.world, eye, f, 300.0)
        .or_else(|| crate::tools::map::edit::ray_ground(&s.session.world, eye, -Vec3::Z, 1500.0))
        .unwrap_or(eye);
    let heading = if f.truncate().length() > 1e-3 { f.y.atan2(f.x).to_degrees() } else { 0.0 };
    Target::At(at, heading)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::Panel;
    use crate::tools::map::{MapMod, RaceKind};
    use super::super::validate::Tested;
    use glam::Vec2;

    #[test]
    fn the_game_is_asked_for_the_race_or_the_spot() {
        let r = Path::new("/m/my-map/.editor/test-result.txt");
        let f = Path::new("/m/my-map/.editor/run/map.obmap");
        let l = Path::new("/m/my-map/textures");
        let k = Path::new("/m/my-map/.editor/link");
        assert_eq!(
            args(f, &Target::Race("dock".into()), r, l, k, &link::Focus::default()),
            [
                "--map-file",
                "/m/my-map/.editor/run/map.obmap",
                "--map-race",
                "dock",
                "--map-result",
                "/m/my-map/.editor/test-result.txt",
                "--looks-live",
                "/m/my-map/textures",
                "--studio-link",
                "/m/my-map/.editor/link"
            ]
        );
        let a = args(f, &Target::At(Vec3::new(-1.5, 2.0, 3.25), 90.0), r, l, k, &link::Focus::default());
        assert_eq!(a[2..4], ["--map-at".to_string(), "-1.5,2,3.25,90".to_string()]);
        let focus = link::Focus { car: Some("240SX".into()), clock: Some(600.0) };
        let a = args(f, &Target::At(Vec3::ZERO, 0.0), r, l, k, &focus);
        assert_eq!(a[a.len() - 4..], ["--map-car", "240SX", "--map-clock", "600"], "the focus is the game's flags");
    }

    #[test]
    fn drive_to_record_asks_the_game_for_its_run_file_with_the_channels() {
        let (r, f) = (Path::new("/m/m/.editor/test-result.txt"), Path::new("/m/m/.editor/run/map.obmap"));
        let (l, k) = (Path::new("/m/m/textures"), Path::new("/m/m/.editor/link"));
        let plain = args(f, &Target::Race("dock".into()), r, l, k, &link::Focus::default());
        assert!(!plain.iter().any(|a| a.starts_with("--record")), "a test that is not recorded has no flag");
        let rec = Path::new("/m/m/.editor/runs/drive-dock.obrun");
        let a = run_args(f, &Target::Race("dock".into()), r, l, k, &link::Focus::default(), Some(rec));
        assert_eq!(a[..plain.len()], plain[..], "the same test");
        assert_eq!(a[plain.len()..], ["--record", "/m/m/.editor/runs/drive-dock.obrun", "--run-channels", "drive,hash"]);
    }

    #[test]
    fn result_lines_read() {
        let o = Outcome::parse("finished=yes place=2 of=4 time=83.412 laps=2\n");
        assert_eq!(
            o,
            Outcome { finished: true, place: Some(2), of: Some(4), time: Some(83.412), laps: Some(2), drove: false }
        );
        assert_eq!(o.words("Dock sprint"), "Tested Dock sprint: finished 2 of 4 in 1:23.412");
        assert!(!Outcome::parse("finished=no laps=1").finished);
        assert!(Outcome::parse("drove=yes future=1").drove);
        assert_eq!(Outcome::parse(""), Outcome::default());
    }

    fn state(dir: &Path, map: MapMod) -> EditorState {
        EditorState::new(
            dir.to_path_buf(),
            "L4RA",
            dir.join("maps"),
            "my-map",
            map,
            crate::world::World::default(),
            crate::draw::camera::Camera::new(Vec3::new(10.0, 20.0, 5.0), 0.0, 0.0),
        )
    }

    #[test]
    fn the_menu_commands_reach_the_test() {
        let dir = std::env::temp_dir().join(format!("ob-testpanel-{}", std::process::id()));
        let mut s = state(&dir, MapMod::default());
        let mut panel = TestPanel::default();
        let ctx = egui::Context::default();
        super::super::menu::post(&ctx, super::super::menu::Command::TestRace);
        let _ = crate::editor::run_headless(&ctx, egui::RawInput::default(), |ui| panel.ui(&mut s, ui));
        assert_eq!(s.status, "Choose a race to test (Races)");
        assert!(panel.test.is_none() && !dir.join("maps").exists(), "nothing saved or started");
        // Drive here with no ground: the camera's own place, facing its way.
        let Target::At(at, h) = drive_spot(&s) else { panic!("a spot") };
        assert_eq!(at, Vec3::new(10.0, 20.0, 5.0));
        let f = s.camera.forward();
        assert!((h - f.y.atan2(f.x).to_degrees()).abs() < 1e-3);
    }

    /// A stand-in game (a shell script) that writes its arguments beside
    /// itself and the result line, after `pause` seconds.
    #[cfg(unix)]
    fn stand_in(dir: &Path, pause: &str) -> PathBuf {
        let exe = dir.join("game");
        crate::testkit::write_script(
            &exe,
            &format!("#!/bin/sh\nsleep {pause}\nwhile [ $# -gt 0 ]; do [ \"$1\" = --map-result ] && r=$2; [ \"$1\" = --record ] && w=$2; echo \"$1\" >> \"$(dirname \"$0\")/args\"; shift; done\n[ -n \"$w\" ] && printf 'OBSIM 1 schema=ob.run/1.1 mode=race seed=1 race=dock hash=1\\nEND quit\\n' > \"$w\"\necho 'finished=yes place=1 of=4 time=61.5 laps=1' > \"$r\"\n"),
        );
        exe
    }

    /// [`TestPlay::start`], tried again while the stand-in is "busy": a
    /// test on another thread may fork while this one still has the
    /// script open for writing (ETXTBSY until that child execs).
    #[cfg(unix)]
    fn start_test(s: &mut EditorState, exe: &Path, t: Target) -> Result<TestPlay, String> {
        let mut tries = 0;
        loop {
            match TestPlay::start(s, exe, t.clone()) {
                Err(e) if e.contains("os error 26") && tries < 100 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                r => return r,
            }
        }
    }

    #[cfg(unix)]
    fn wait(s: &mut EditorState, test: &mut Option<TestPlay>) -> bool {
        let t0 = std::time::Instant::now();
        loop {
            if let Some(marked) = poll(s, test) {
                return marked;
            }
            assert!(t0.elapsed().as_secs() < 20, "the stand-in game ended");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The whole hand-off with a stand-in game: unsaved changes are tested
    /// from the run copy, `map.obmap` is never written.
    #[cfg(unix)]
    #[test]
    fn a_test_plays_the_run_copy_and_reads_its_result() {
        let scratch = crate::testkit::Scratch::new("testplay");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "0");
        let mut race = CustomRace::new("dock", 4001, RaceKind::Sprint);
        race.name = "Dock sprint".into();
        race.checkpoints = vec![Vec2::new(1.0, 2.0), Vec2::new(30.0, 2.0)];
        let maps = dir.join("maps");
        let saved = MapMod { name: "Saved".into(), ..Default::default() };
        store::save(&maps, "my-map", &saved).unwrap();
        let on_disk = std::fs::read(maps.join("my-map").join(store::FILE)).unwrap();
        let map = MapMod { name: "Saved".into(), races: vec![race.clone()], ..Default::default() };
        let mut s = EditorState::new(
            dir.clone(),
            "L4RA",
            maps.clone(),
            "my-map",
            map.clone(),
            crate::world::World::default(),
            crate::draw::camera::Camera::new(Vec3::ZERO, 0.0, 0.0),
        );
        s.session.dirty = true;
        // A blocking issue stops the test before anything is written.
        s.session.issues.push(super::super::state::Issue {
            level: Level::Error,
            text: "no road from 1 to 2".into(),
            race: Some(0),
            at: None,
        });
        assert!(start_test(&mut s, &exe, Target::Race("dock".into())).unwrap_err().contains("no road"));
        assert!(!store::run_file(&maps, "my-map").exists());
        s.session.issues.clear();
        let mut test = Some(start_test(&mut s, &exe, Target::Race("dock".into())).unwrap());
        assert!(s.session.dirty, "still unsaved");
        assert_eq!(store::read_file(&store::run_file(&maps, "my-map")).unwrap(), map, "the run copy is the editor's mod");
        assert_eq!(s.status, "Testing Dock sprint… (testing unsaved changes)");
        assert!(wait(&mut s, &mut test), "marked");
        assert_eq!(s.status, "Tested Dock sprint: finished 1 of 4 in 1:01.500");
        assert_eq!(std::fs::read(maps.join("my-map").join(store::FILE)).unwrap(), on_disk, "map.obmap untouched");
        let editor = store::editor_dir(&maps, "my-map");
        assert!(Tested::load(&editor).is_tested(&race));
        assert!(editor.join(RESULT_FILE).is_file() && !maps.join("my-map").join(RESULT_FILE).exists());
        let argv = std::fs::read_to_string(dir.join("args")).unwrap();
        let run = store::run_file(&maps, "my-map").display().to_string();
        assert!(argv.starts_with(&format!("--map-file\n{run}\n--map-race\ndock\n--map-result\n")), "{argv}");
        let looks = maps.join("my-map").join(store::TEXTURES).display().to_string();
        let link = link::dir(&maps, "my-map").display().to_string();
        assert!(argv.ends_with(&format!("--looks-live\n{looks}\n--studio-link\n{link}\n")), "textures and link: {argv}");
        // The mark is the race's content: saving the same race keeps it, a change clears it.
        store::save(&maps, "my-map", &map).unwrap();
        assert!(Tested::load(&editor).is_tested(&store::read_file(&maps.join("my-map").join(store::FILE)).unwrap().races[0]));
        race.checkpoints[1].x += 0.5;
        assert!(!Tested::load(&editor).is_tested(&race));
    }

    /// Drive to record with a stand-in game: its run file lands in the mod's runs folder under a free name, the
    /// game is asked for the channels, and the status says where the drive was kept; a second drive never
    /// replaces the first.
    #[cfg(unix)]
    #[test]
    fn a_recorded_drive_is_kept_as_a_run_in_the_list() {
        let scratch = crate::testkit::Scratch::new("testplay-record");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "0");
        let mut race = CustomRace::new("dock", 4001, RaceKind::Sprint);
        race.name = "Dock sprint".into();
        race.checkpoints = vec![Vec2::new(1.0, 2.0), Vec2::new(30.0, 2.0)];
        let mut s = state(&dir, MapMod { name: "Rec".into(), races: vec![race], ..Default::default() });
        let runs = super::super::sim::library::dir(&s.maps, &s.session.folder);
        for name in ["drive-dock", "drive-dock-2"] {
            let mut test = None;
            let mut tries = 0;
            loop {
                match TestPlay::start_run(&mut s, &exe, Target::Race("dock".into()), true) {
                    Err(e) if e.contains("os error 26") && tries < 100 => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    r => break test = Some(r.unwrap()),
                }
            }
            assert_eq!(s.status, format!("Recording Dock sprint as {name}…"));
            wait(&mut s, &mut test);
            assert!(s.status.ends_with(&format!("· kept as the run {name} (Runs)")), "{}", s.status);
            assert!(super::super::sim::library::path(&runs, name).is_file(), "{name} is in the list");
        }
        let kept: Vec<String> = super::super::sim::library::list(&runs).into_iter().map(|e| e.name).collect();
        assert_eq!(kept.len(), 2, "{kept:?}");
        let argv = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(argv.contains("--record\n") && argv.contains("--run-channels\ndrive,hash\n"), "{argv}");
        // A test that is open is not turned into a recording: the command says so and starts nothing.
        let mut panel = TestPanel::default();
        let ctx = egui::Context::default();
        let exe = stand_in(&dir, "5");
        panel.test = Some(start_test(&mut s, &exe, Target::Race("dock".into())).unwrap());
        super::super::menu::post(&ctx, super::super::menu::Command::RecordDrive);
        let _ = crate::editor::run_headless(&ctx, egui::RawInput::default(), |ui| panel.ui(&mut s, ui));
        assert!(s.status.starts_with("A test is open"), "{}", s.status);
        assert_eq!(super::super::sim::library::list(&runs).len(), 2);
        drop(panel);
    }

    /// Edits made while the game runs reach neither the running game's copy
    /// nor the mod file; the next test takes them.
    #[cfg(unix)]
    #[test]
    fn editing_during_a_test_leaves_its_copy_and_the_mod_alone() {
        let scratch = crate::testkit::Scratch::new("testplay-busy");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "0.3");
        let mut s = state(&dir, MapMod { name: "Before".into(), ..Default::default() });
        let mut test = Some(start_test(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap());
        assert_eq!(s.status, "Driving from the camera's spot…");
        s.session.map.name = "During".into();
        s.session.dirty = true;
        assert!(!wait(&mut s, &mut test), "free roam marks nothing");
        let run = store::run_file(&s.maps, "my-map");
        assert_eq!(store::read_file(&run).unwrap().name, "Before");
        assert!(!s.maps.join("my-map").join(store::FILE).exists(), "Test never saves");
        let mut test = Some(start_test(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap());
        assert_eq!(store::read_file(&run).unwrap().name, "During");
        assert_eq!(s.status, "Driving from the camera's spot… (testing unsaved changes)");
        wait(&mut s, &mut test);
    }

    /// The link with a stand-in game that stays: its state file reads as
    /// connected, an edit is sent as the run copy and an apply command, the
    /// game's note about a command it could not do reaches the status.
    #[cfg(unix)]
    #[test]
    fn the_game_stays_open_and_takes_edits() {
        let scratch = crate::testkit::Scratch::new("testplay-link");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "5");
        let mut s = state(&dir, MapMod { name: "Before".into(), ..Default::default() });
        let mut panel = TestPanel::default();
        panel.test = Some(start_test(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap());
        let ctx = egui::Context::default();
        let frame = |s: &mut EditorState, panel: &mut TestPanel| {
            let _ = crate::editor::run_headless(&ctx, egui::RawInput::default(), |ui| panel.ui(s, ui));
        };
        let linkdir = link::dir(&s.maps, "my-map");
        let pid = panel.test.as_ref().unwrap().child.id();
        frame(&mut s, &mut panel);
        assert_eq!(link_words(&ctx).as_deref(), Some(GAME_NOT_CONNECTED), "the game has said nothing yet");
        let said = |seq: u64, ok: bool, note: &str| {
            link::write_state(&linkdir, &link::GameState { pid, seq, beat_ms: link::now_ms(), ok, note: note.into(), turns: 1 }).unwrap();
        };
        said(0, true, "");
        frame(&mut s, &mut panel);
        assert_eq!(link_words(&ctx).as_deref(), Some(GAME_CONNECTED));
        // An edit goes out: the run copy and one apply command.
        s.session.map.name = "During".into();
        frame(&mut s, &mut panel);
        assert_eq!(store::read_file(&store::run_file(&s.maps, "my-map")).unwrap().name, "During");
        let c = link::read_command(&linkdir).unwrap();
        assert_eq!((c.seq, c.op), (1, Op::Apply));
        frame(&mut s, &mut panel);
        assert_eq!(link_words(&ctx).as_deref(), Some(GAME_APPLYING), "the game has not dealt with it");
        assert_eq!(link::read_command(&linkdir).unwrap().seq, 1, "nothing changed, nothing sent again");
        said(1, true, "");
        frame(&mut s, &mut panel);
        assert_eq!(link_words(&ctx).as_deref(), Some(GAME_CONNECTED));
        // Restart race: the mod is as the game has it, so a plain restart.
        super::super::menu::post(&ctx, super::super::menu::Command::RestartTest);
        frame(&mut s, &mut panel);
        let c = link::read_command(&linkdir).unwrap();
        assert_eq!((c.seq, c.op), (2, Op::Restart));
        said(2, false, "race x is gone");
        frame(&mut s, &mut panel);
        assert!(s.status.contains("race x is gone"), "{}", s.status);
        // A game that is gone says nothing: not connected.
        link::write_state(&linkdir, &link::GameState { pid, seq: 2, beat_ms: 1, ok: true, note: String::new(), turns: 1 }).unwrap();
        frame(&mut s, &mut panel);
        assert_eq!(link_words(&ctx).as_deref(), Some(GAME_NOT_CONNECTED));
        let _ = panel.test.as_mut().unwrap().child.kill();
    }

    /// The health check, the reload words and the focus over the link: a
    /// probe goes out when the game is idle and never over a command it has
    /// not read; moved props say the world reloads; a game that beats but
    /// does not answer a probe is "not answering"; a changed focus goes
    /// with an apply.
    #[cfg(unix)]
    #[test]
    fn the_link_probes_says_reloads_and_carries_the_focus() {
        use crate::tools::map::PropEdit;
        let scratch = crate::testkit::Scratch::new("testplay-probe");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "5");
        let mut s = state(&dir, MapMod::default());
        let mut test = start_test(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap();
        test.probe_every = std::time::Duration::ZERO;
        test.probe_next = std::time::Instant::now();
        let linkdir = link::dir(&s.maps, "my-map");
        let pid = test.child.id();
        let said = |seq: u64| {
            link::write_state(&linkdir, &link::GameState { pid, seq, beat_ms: link::now_ms(), ok: true, note: String::new(), turns: 9 }).unwrap();
        };
        // Nothing said yet: no probe.
        test.link_words();
        test.probe_due().unwrap();
        assert!(link::read_command(&linkdir).is_none(), "not to a game that has not appeared");
        said(0);
        assert_eq!(test.link_words(), GAME_CONNECTED);
        test.probe_due().unwrap();
        let c = link::read_command(&linkdir).unwrap();
        assert_eq!((c.seq, c.op), (1, Op::Probe));
        // Unanswered for a while: the game beats, but does not answer.
        test.probe_due().unwrap();
        assert_eq!(link::read_command(&linkdir).unwrap().seq, 1, "one probe at a time");
        test.probe = Some((1, std::time::Instant::now() - PROBE_WAIT - std::time::Duration::from_millis(10)));
        assert_eq!(test.link_words(), GAME_NO_ANSWER);
        said(1);
        assert_eq!(test.link_words(), GAME_CONNECTED, "answered");
        // Props moved: the game says it reloads the world; the probe does not wait over it.
        s.session.map.props.push(PropEdit::Delete {
            target: crate::tools::map::PropRef { region: "L4RA".into(), section: 1, solid: "X".into(), at: Vec3::ZERO, nth: 0 },
        });
        test.send(&s, Op::Apply, None).unwrap();
        assert_eq!(test.link_words(), GAME_RELOADING);
        test.probe_due().unwrap();
        assert_eq!(link::read_command(&linkdir).unwrap().op, Op::Apply, "a probe never goes over a command not read");
        said(2);
        assert_eq!(test.link_words(), GAME_CONNECTED);
        // Another change (not props): applying, not reloading.
        s.session.map.name = "Other".into();
        test.send(&s, Op::Apply, None).unwrap();
        assert_eq!(test.link_words(), GAME_APPLYING);
        said(3);
        // The focus goes with the next command, once.
        assert!(!test.changed(&s));
        s.run_focus = link::Focus { car: Some("240SX".into()), clock: Some(600.0) };
        assert!(test.changed(&s), "a new focus starts the run over");
        test.send(&s, Op::Apply, None).unwrap();
        assert_eq!(link::read_command(&linkdir).unwrap().focus, Some(s.run_focus.clone()));
        said(4);
        assert!(!test.changed(&s));
        test.send(&s, Op::Restart, None).unwrap();
        assert_eq!(link::read_command(&linkdir).unwrap().focus, None, "sent once");
        s.run_focus.clock = Some(0.1);
        assert!(test.send(&s, Op::Restart, None).is_err(), "a clock the game cannot take is not sent");
        let _ = test.child.kill();
    }

    /// At most `MAX_RUNS` runs at once: a new test waits for a place, and the
    /// place comes back when the run ends.
    #[cfg(unix)]
    #[test]
    fn a_test_waits_for_a_place_when_the_runs_are_all_open() {
        let scratch = crate::testkit::Scratch::new("testplay-cap");
        let dir = scratch.path().to_path_buf();
        let exe = stand_in(&dir, "5");
        let mut s = state(&dir, MapMod::default());
        let held: Vec<_> = (0..link::MAX_RUNS).map(|_| link::Slot::take(&s.maps).unwrap()).collect();
        let e = TestPlay::start(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap_err();
        assert!(e.contains("game runs are open"), "{e}");
        assert!(!store::run_file(&s.maps, "my-map").exists(), "nothing written for a run that did not start");
        drop(held);
        let mut t = start_test(&mut s, &exe, Target::At(Vec3::ZERO, 0.0)).unwrap();
        assert_eq!(link::Slot::live(&link::Slot::dir(&s.maps)), 1);
        let _ = t.child.kill();
        drop(t);
        assert_eq!(link::Slot::live(&link::Slot::dir(&s.maps)), 0, "the place is back");
    }
}
