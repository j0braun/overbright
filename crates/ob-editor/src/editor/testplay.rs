//! Test race and Drive here: the editor saves the mod, starts the game
//! beside itself with only that mod (`game --map-mod <folder> --map-race
//! <id>` or `--map-at x,y,z,heading`, and `--map-result <file>`), keeps
//! editing while it runs, and reads the run's result line when the game
//! has ended. A race that finished is marked tested until it changes
//! (`tested.txt` in the mod's folder, never exported). One test at a time.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use glam::Vec3;

use super::state::{EditorState, Level};
use crate::tools::map::{CustomRace, MapMod, store};

/// The result file's name in the mod's folder.
pub const RESULT_FILE: &str = "test-result.txt";
/// The tested races' file in the mod's folder.
pub const TESTED_FILE: &str = "tested.txt";

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

/// The game's command line for a test run.
pub fn args(folder: &str, target: &Target, result: &Path) -> Vec<String> {
    let mut a = vec!["--map-mod".to_string(), folder.to_string()];
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
    /// The race as it was saved for this test (to mark it tested).
    race: Option<CustomRace>,
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
        let Some(k) = s.map.races.iter().position(|r| &r.id == id) else {
            return Some(format!("No race {id} in this mod"));
        };
        if s.map.races[k].checkpoints.len() < 2 {
            return Some("A race needs 2 checkpoints or more".into());
        }
        if let Some(e) = s.issues.iter().find(|i| i.level == Level::Error && i.race.is_none_or(|r| r == k)) {
            return Some(format!("Cannot test: {}", e.text));
        }
    }
    None
}

impl TestPlay {
    /// Save the mod (the game reads it at its first world load) and start
    /// the game with `exe`. The editor keeps its state; only `dirty` and
    /// the status change.
    pub fn start(s: &mut EditorState, exe: &Path, target: Target) -> Result<TestPlay, String> {
        if let Some(why) = blocked(s, &target) {
            return Err(why);
        }
        let mut m = s.map.clone();
        if m.name.trim().is_empty() {
            m.name = s.folder.clone();
        }
        store::save(&s.maps, &s.folder, &m).map_err(|e| format!("Not saved: {e:#}"))?;
        s.dirty = false;
        let result = s.maps.join(&s.folder).join(RESULT_FILE);
        let _ = std::fs::remove_file(&result);
        let race = match &target {
            Target::Race(id) => s.map.races.iter().find(|r| &r.id == id).cloned(),
            Target::At(..) => None,
        };
        let label = race.as_ref().map_or_else(|| "free roam".to_string(), |r| r.name.clone());
        let child = Command::new(exe)
            .args(args(&s.folder, &target, &result))
            .env("NFSU2_ROOT", &s.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("The game could not start ({}): {e}", exe.display()))?;
        s.status = match &target {
            Target::Race(_) => format!("Testing {label}…"),
            Target::At(..) => "Driving from the camera's spot…".into(),
        };
        Ok(TestPlay { child, target, label, result, race })
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

/// Look at a running test: when it ended, the status and (a finished
/// race) the tested mark. True when it ended.
pub fn poll(s: &mut EditorState, test: &mut Option<TestPlay>) -> bool {
    let Some(t) = test.as_mut() else { return false };
    let Some(end) = t.poll() else { return false };
    s.status = match &end {
        Ended::Done(o) => {
            if o.finished
                && let Some(r) = &t.race
                && let Err(e) = mark_tested(&s.maps.join(&s.folder), r)
            {
                log::warn!("tested mark not kept: {e}");
            }
            o.words(&t.label)
        }
        Ended::Failed(how) => format!("The game ended without a result ({how}); see its log"),
    };
    *test = None;
    true
}

/// The window's part: Test race (F5) and Drive here (Shift+F5) from the
/// menu's commands, the running test polled each frame (a hook of
/// `app::Panels`, no visible place of its own: it speaks in the status bar).
#[derive(Default)]
pub struct TestPanel {
    pub test: Option<TestPlay>,
}

/// How often a running test is looked at while the window is idle.
const POLL_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

impl super::state::Panel for TestPanel {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let race = super::menu::take(&ctx, super::menu::Command::TestRace);
        let drive = super::menu::take(&ctx, super::menu::Command::DriveHere);
        if poll(s, &mut self.test) {
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
            match s.race.and_then(|k| s.map.races.get(k)) {
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
    let at = crate::tools::map::edit::ray_ground(&s.world, eye, f, 300.0)
        .or_else(|| crate::tools::map::edit::ray_ground(&s.world, eye, -Vec3::Z, 1500.0))
        .unwrap_or(eye);
    let heading = if f.truncate().length() > 1e-3 { f.y.atan2(f.x).to_degrees() } else { 0.0 };
    Target::At(at, heading)
}

/// A race's fingerprint: its text in the mod file (FNV-1a), so any change
/// to it clears the tested mark.
pub fn fingerprint(race: &CustomRace) -> u64 {
    let text = MapMod { races: vec![race.clone()], ..Default::default() }.write();
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

fn tested_lines(dir: &Path) -> Vec<(String, u64)> {
    let Ok(text) = std::fs::read_to_string(dir.join(TESTED_FILE)) else { return Vec::new() };
    text.lines()
        .filter_map(|l| l.split_once(' '))
        .filter_map(|(id, h)| Some((id.to_string(), u64::from_str_radix(h.trim(), 16).ok()?)))
        .collect()
}

/// Whether `race` finished a test as it is now (`dir`: the mod's folder).
pub fn is_tested(dir: &Path, race: &CustomRace) -> bool {
    let f = fingerprint(race);
    tested_lines(dir).iter().any(|(id, h)| id == &race.id && *h == f)
}

/// Keep `race` as tested in its current form.
pub fn mark_tested(dir: &Path, race: &CustomRace) -> std::io::Result<()> {
    let mut lines = tested_lines(dir);
    lines.retain(|(id, _)| id != &race.id);
    lines.push((race.id.clone(), fingerprint(race)));
    let text: String = lines.iter().map(|(id, h)| format!("{id} {h:016x}\n")).collect();
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{TESTED_FILE}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join(TESTED_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::Panel;
    use crate::tools::map::RaceKind;
    use glam::Vec2;

    #[test]
    fn the_game_is_asked_for_the_race_or_the_spot() {
        let r = Path::new("/m/my-map/test-result.txt");
        assert_eq!(
            args("my-map", &Target::Race("dock".into()), r),
            ["--map-mod", "my-map", "--map-race", "dock", "--map-result", "/m/my-map/test-result.txt"]
        );
        let a = args("my-map", &Target::At(Vec3::new(-1.5, 2.0, 3.25), 90.0), r);
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

    #[test]
    fn a_tested_race_stays_tested_until_it_changes() {
        let dir = std::env::temp_dir().join(format!("ob-tested-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut race = CustomRace::new("dock", 4001, RaceKind::Sprint);
        race.checkpoints = vec![Vec2::new(1.0, 2.0), Vec2::new(30.0, 2.0)];
        let other = CustomRace::new("park", 4001, RaceKind::Circuit);
        assert!(!is_tested(&dir, &race));
        mark_tested(&dir, &race).unwrap();
        mark_tested(&dir, &other).unwrap();
        mark_tested(&dir, &race).unwrap();
        assert!(is_tested(&dir, &race) && is_tested(&dir, &other));
        assert_eq!(tested_lines(&dir).len(), 2, "one line a race");
        race.checkpoints[1].x += 0.5;
        assert!(!is_tested(&dir, &race), "a moved checkpoint clears it");
        let _ = std::fs::remove_dir_all(&dir);
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

    /// The whole hand-off with a stand-in game (a shell script writing
    /// the result line).
    #[cfg(unix)]
    #[test]
    fn a_test_saves_starts_the_game_and_reads_its_result() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ob-testplay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("game");
        // Writes its arguments beside the result, then the result line.
        std::fs::write(
            &exe,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = --map-result ] && r=$2; echo \"$1\" >> \"$(dirname \"$0\")/args\"; shift; done\necho 'finished=yes place=1 of=4 time=61.5 laps=1' > \"$r\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut race = CustomRace::new("dock", 4001, RaceKind::Sprint);
        race.name = "Dock sprint".into();
        race.checkpoints = vec![Vec2::new(1.0, 2.0), Vec2::new(30.0, 2.0)];
        let map = MapMod { races: vec![race.clone()], ..Default::default() };
        let maps = dir.join("maps");
        let mut s = EditorState::new(
            dir.clone(),
            "L4RA",
            maps.clone(),
            "my-map",
            map,
            crate::world::World::default(),
            crate::draw::camera::Camera::new(Vec3::ZERO, 0.0, 0.0),
        );
        s.dirty = true;
        // A blocking issue stops the test before anything is written.
        s.issues.push(super::super::state::Issue {
            level: Level::Error,
            text: "no road from 1 to 2".into(),
            race: Some(0),
            at: None,
        });
        assert!(TestPlay::start(&mut s, &exe, Target::Race("dock".into())).unwrap_err().contains("no road"));
        assert!(!maps.join("my-map").exists());
        s.issues.clear();
        let mut test = Some(TestPlay::start(&mut s, &exe, Target::Race("dock".into())).unwrap());
        assert!(!s.dirty && maps.join("my-map").join(store::FILE).is_file(), "saved first");
        assert_eq!(s.status, "Testing Dock sprint…");
        let t0 = std::time::Instant::now();
        while !poll(&mut s, &mut test) {
            assert!(t0.elapsed().as_secs() < 20, "the stand-in game ended");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(s.status, "Tested Dock sprint: finished 1 of 4 in 1:01.500");
        assert!(is_tested(&maps.join("my-map"), &race));
        let argv = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(argv.starts_with("--map-mod\nmy-map\n--map-race\ndock\n--map-result\n"), "{argv}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
