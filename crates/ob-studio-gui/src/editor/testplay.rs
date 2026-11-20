//! Test race and Drive here: the editor writes the mod as it is (saved or
//! not) to its run copy (`.editor/run/map.obmap`; `map.obmap` is only
//! written by Save), starts the game beside itself with only that copy
//! (`game --map-file <run copy> --map-race <id>` or `--map-at
//! x,y,z,heading`, and `--map-result <file>`), keeps editing while it runs,
//! and reads the run's result line when the game has ended. A race that
//! finished is marked tested by its content (`validate::Tested`,
//! `.editor/tested.txt`, never exported), so the mark holds after the same
//! race is saved and goes when it changes. One test at a time.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use glam::Vec3;

use super::state::{EditorState, Level};
use crate::tools::map::{CustomRace, store};

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

/// The game's command line for a test run of the mod file `file`.
pub fn args(file: &Path, target: &Target, result: &Path) -> Vec<String> {
    let mut a = vec!["--map-file".to_string(), file.display().to_string()];
    match target {
        Target::Race(id) => a.extend(["--map-race".to_string(), id.clone()]),
        Target::At(p, h) => a.extend(["--map-at".to_string(), format!("{},{},{},{}", p.x, p.y, p.z, h)]),
    }
    a.extend(["--map-result".to_string(), result.display().to_string()]);
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
}

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
        if let Some(why) = blocked(s, &target) {
            return Err(why);
        }
        let file = write_run_copy(s)?;
        let dir = store::editor_dir(&s.maps, &s.session.folder);
        let result = dir.join(RESULT_FILE);
        let _ = std::fs::remove_file(&result);
        let race = match &target {
            Target::Race(id) => s.session.map.races.iter().find(|r| &r.id == id).cloned(),
            Target::At(..) => None,
        };
        let label = race.as_ref().map_or_else(|| "free roam".to_string(), |r| r.name.clone());
        let child = Command::new(exe)
            .args(args(&file, &target, &result))
            .env("NFSU2_ROOT", &s.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("The game could not start ({}): {e}", exe.display()))?;
        let unsaved = if s.session.dirty { " (testing unsaved changes)" } else { "" };
        s.status = match &target {
            Target::Race(_) => format!("Testing {label}…{unsaved}"),
            Target::At(..) => format!("Driving from the camera's spot…{unsaved}"),
        };
        Ok(TestPlay { child, target, label, result, race, dir })
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
}

/// How often a running test is looked at while the window is idle.
const POLL_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

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
        ui.ctx().data_mut(|d| match words {
            Some(w) => {
                d.insert_temp(strip_id(), w);
            }
            None => {
                d.remove::<String>(strip_id());
            }
        });
    }
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
        if self.test.is_some() {
            if race || drive {
                s.status = "A test is running; close the game first".into();
            }
            ctx.request_repaint_after(POLL_EVERY);
            return;
        }
        let target = if race {
            match s.race.and_then(|k| s.session.map.races.get(k)) {
                Some(r) => Some(Target::Race(r.id.clone())),
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
            match TestPlay::start(s, &game_exe(), t) {
                Ok(t) => self.test = Some(t),
                Err(e) => s.status = e,
            }
            ctx.request_repaint_after(POLL_EVERY);
        }
    }
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
        assert_eq!(
            args(f, &Target::Race("dock".into()), r),
            [
                "--map-file",
                "/m/my-map/.editor/run/map.obmap",
                "--map-race",
                "dock",
                "--map-result",
                "/m/my-map/.editor/test-result.txt"
            ]
        );
        let a = args(f, &Target::At(Vec3::new(-1.5, 2.0, 3.25), 90.0), r);
        assert_eq!(a[2..4], ["--map-at".to_string(), "-1.5,2,3.25,90".to_string()]);
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
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| panel.ui(&mut s, ui));
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
        use std::os::unix::fs::PermissionsExt;
        let exe = dir.join("game");
        std::fs::write(
            &exe,
            format!("#!/bin/sh\nsleep {pause}\nwhile [ $# -gt 0 ]; do [ \"$1\" = --map-result ] && r=$2; echo \"$1\" >> \"$(dirname \"$0\")/args\"; shift; done\necho 'finished=yes place=1 of=4 time=61.5 laps=1' > \"$r\"\n"),
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
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
        // The mark is the race's content: saving the same race keeps it, a change clears it.
        store::save(&maps, "my-map", &map).unwrap();
        assert!(Tested::load(&editor).is_tested(&store::read_file(&maps.join("my-map").join(store::FILE)).unwrap().races[0]));
        race.checkpoints[1].x += 0.5;
        assert!(!Tested::load(&editor).is_tested(&race));
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
}
