//! The link between Studio and the test game it started (`game
//! --studio-link <dir>`, docs/modding/map-editor.md, "The test window stays
//! open"): two small files in a folder of the mod's own `.editor/` (so in
//! the player's data folder, which only the player can read and write;
//! nothing listens on a network, and the same files work on Windows and
//! Linux). Studio writes `command.txt` (what to do, with a running number);
//! the game, between two frames, reads it and writes `game.txt` (what it did
//! and that it is alive). Both are written through a temporary file, so a
//! reader never sees half a line. Without the flag the game never looks at
//! the folder.

use std::path::{Path, PathBuf};

/// The link folder of a mod (`<maps>/<folder>/.editor/link`).
pub const DIR: &str = "link";
/// Studio's file: the latest command.
pub const COMMAND: &str = "command.txt";
/// The game's file: its state.
pub const STATE: &str = "game.txt";
/// How long the game's last sign of life counts as connected (it writes one
/// about twice a second).
pub const ALIVE_MS: u64 = 3000;
/// The most game runs (test windows and simulations, over every mod and
/// every Studio window) that run at once ([`Slot::take`]).
pub const MAX_RUNS: usize = 4;
/// A run's slot counts while it was touched this recently (Studio touches
/// it every poll); a Studio that died leaves a slot that expires.
pub const SLOT_MS: u64 = 15_000;

/// `<maps>/<folder>/.editor/link`.
pub fn dir(maps: &Path, folder: &str) -> PathBuf {
    super::store::editor_dir(maps, folder).join(DIR)
}

/// `<maps>/<folder>/.editor/link-sim`: the link of a headless simulation
/// (apart from the test window's, so both can run).
pub fn sim_dir(maps: &Path, folder: &str) -> PathBuf {
    super::store::editor_dir(maps, folder).join("link-sim")
}

/// What Studio asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Read the mod file again and start the run over with it (props, race
    /// points and all).
    Apply,
    /// Start the run over with the mod as the game has it.
    Restart,
    /// Health check: the game answers at once with its state, so Studio
    /// knows the command file is read (not only that the game beats).
    Probe,
    /// End the run (a headless simulation has no window to close).
    Stop,
}

impl Op {
    fn word(&self) -> &'static str {
        match self {
            Op::Apply => "apply",
            Op::Restart => "restart",
            Op::Probe => "probe",
            Op::Stop => "stop",
        }
    }
}

/// What the test starts with besides the place: the car and the world
/// clock (seconds since the world loaded; it sets the time of day and the
/// weather as the game's own cycle has them then). Empty: the run's own
/// (the save's car or the first Quick Race car, the clock of a new load).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Focus {
    pub car: Option<String>,
    pub clock: Option<f32>,
}

/// The longest clock a focus may name (this only keeps the game's catch-up
/// of the sky cycle bounded).
pub const MAX_CLOCK: f32 = 3600.0;

/// A car name that can sit in a command line (`240SX`, `RX-8`).
pub fn valid_car(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl Focus {
    pub fn is_empty(&self) -> bool {
        self.car.is_none() && self.clock.is_none()
    }

    /// Whether every part is one the game takes.
    pub fn valid(&self) -> bool {
        self.car.as_deref().is_none_or(valid_car) && self.clock.is_none_or(|c| c.is_finite() && (1.0..=MAX_CLOCK).contains(&c))
    }

    /// ` car=240SX clock=600` (nothing when empty).
    pub fn words(&self) -> String {
        let mut w = String::new();
        if let Some(c) = &self.car {
            w += &format!(" car={c}");
        }
        if let Some(c) = self.clock {
            w += &format!(" clock={c}");
        }
        w
    }

    /// The game's flags for it (`--map-car`, `--map-clock`).
    pub fn args(&self) -> Vec<String> {
        let mut a = Vec::new();
        if let Some(c) = &self.car {
            a.extend(["--map-car".to_string(), c.clone()]);
        }
        if let Some(c) = self.clock {
            a.extend(["--map-clock".to_string(), c.to_string()]);
        }
        a
    }
}

/// What the run is about, when a command changes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Aim {
    Race(String),
    /// Free roam: x, y, z, heading in degrees.
    At(f32, f32, f32, f32),
}

/// One command; `seq` grows with each, the game does a number once.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub seq: u64,
    pub op: Op,
    pub aim: Option<Aim>,
    /// The car and clock the run starts over with (None: as it was).
    pub focus: Option<Focus>,
}

impl Command {
    /// `seq=3 op=apply race=dock` (or `at=1,2,3,90`).
    pub fn line(&self) -> String {
        let op = self.op.word();
        let aim = match &self.aim {
            None => String::new(),
            Some(Aim::Race(id)) => format!(" race={id}"),
            Some(Aim::At(x, y, z, h)) => format!(" at={x},{y},{z},{h}"),
        };
        let focus = self.focus.as_ref().map_or(String::new(), |f| if f.is_empty() { " focus=none".into() } else { f.words() });
        format!("seq={} op={op}{aim}{focus}\n", self.seq)
    }

    pub fn parse(text: &str) -> Option<Command> {
        let (mut seq, mut op, mut aim) = (None, None, None);
        let (mut focus, mut car, mut clock) = (None, None, None);
        for (k, v) in text.split_whitespace().filter_map(|w| w.split_once('=')) {
            match k {
                "seq" => seq = v.parse().ok(),
                "op" => {
                    op = match v {
                        "apply" => Some(Op::Apply),
                        "restart" => Some(Op::Restart),
                        "probe" => Some(Op::Probe),
                        "stop" => Some(Op::Stop),
                        _ => None,
                    }
                }
                "race" => aim = Some(Aim::Race(v.to_string())),
                "at" => {
                    let n: Vec<f32> = v.split(',').filter_map(|p| p.parse().ok()).collect();
                    if let [x, y, z, h] = n[..] {
                        aim = Some(Aim::At(x, y, z, h));
                    }
                }
                "car" => car = Some(v.to_string()),
                "clock" => clock = v.parse().ok(),
                "focus" if v == "none" => focus = Some(Focus::default()),
                _ => {}
            }
        }
        if car.is_some() || clock.is_some() {
            focus = Some(Focus { car, clock });
        }
        Some(Command { seq: seq?, op: op?, aim, focus })
    }
}

/// Write Studio's command.
pub fn write_command(dir: &Path, c: &Command) -> std::io::Result<()> {
    crate::platform::fs::write_atomic(&dir.join(COMMAND), c.line().as_bytes(), false)
}

/// Studio's latest command (None: none yet, or not readable).
pub fn read_command(dir: &Path) -> Option<Command> {
    Command::parse(&std::fs::read_to_string(dir.join(COMMAND)).ok()?)
}

/// What the game says of itself.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub pid: u32,
    /// The last command number the game dealt with.
    pub seq: u64,
    /// Unix milliseconds of the game's last sign of life.
    pub beat_ms: u64,
    /// The last command worked.
    pub ok: bool,
    /// Why it did not (one line).
    pub note: String,
    /// How many times the game's loop turned (a game that beats but does
    /// not advance is stuck).
    pub turns: u64,
}

impl GameState {
    pub fn line(&self) -> String {
        let note = self.note.replace(['\n', '\r'], " ");
        format!(
            "pid={} seq={} beat={} ok={} turns={}{}{}\n",
            self.pid,
            self.seq,
            self.beat_ms,
            if self.ok { "yes" } else { "no" },
            self.turns,
            if note.is_empty() { "" } else { " note=" },
            note
        )
    }

    pub fn parse(text: &str) -> Option<GameState> {
        let text = text.lines().next()?;
        let (head, note) = text.split_once(" note=").unwrap_or((text, ""));
        let (mut pid, mut seq, mut beat, mut ok, mut turns) = (None, None, None, None, 0);
        for (k, v) in head.split_whitespace().filter_map(|w| w.split_once('=')) {
            match k {
                "pid" => pid = v.parse().ok(),
                "seq" => seq = v.parse().ok(),
                "beat" => beat = v.parse().ok(),
                "ok" => ok = Some(v == "yes"),
                "turns" => turns = v.parse().unwrap_or(0),
                _ => {}
            }
        }
        Some(GameState { pid: pid?, seq: seq?, beat_ms: beat?, ok: ok?, note: note.to_string(), turns })
    }

    /// Whether the game showed a sign of life lately, at `now_ms`.
    pub fn alive(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.beat_ms) < ALIVE_MS
    }
}

/// Unix milliseconds now.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub fn write_state(dir: &Path, s: &GameState) -> std::io::Result<()> {
    crate::platform::fs::write_atomic(&dir.join(STATE), s.line().as_bytes(), false)
}

pub fn read_state(dir: &Path) -> Option<GameState> {
    GameState::parse(&std::fs::read_to_string(dir.join(STATE)).ok()?)
}

/// Clear the folder's files before a run starts (a leftover state of an
/// earlier game must not read as a connected one).
pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(COMMAND));
    let _ = std::fs::remove_file(dir.join(STATE));
}

/// One of the few places for a running game (a test window or a
/// simulation): a small file in `<data>/studio-runs/`, touched while the
/// run lives and removed when the slot is dropped. Every Studio window and
/// mod of a data folder shares them, so a loop that starts runs cannot fill the machine; a
/// slot whose Studio died expires after [`SLOT_MS`].
#[derive(Debug)]
pub struct Slot {
    file: PathBuf,
}

impl Slot {
    /// `<data>/studio-runs`, beside the map mods' folder `maps`.
    pub fn dir(maps: &Path) -> PathBuf {
        maps.parent().unwrap_or(maps).join("studio-runs")
    }

    /// A place for one more run of the data folder of `maps`, at most
    /// `MAX_RUNS` at once.
    pub fn take(maps: &Path) -> Result<Slot, String> {
        Slot::take_in(&Slot::dir(maps), MAX_RUNS)
    }

    /// A place among at most `cap` in `dir`.
    pub fn take_in(dir: &Path, cap: usize) -> Result<Slot, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("No place made for the run: {e}"))?;
        let busy = || Slot::live(dir);
        if busy() >= cap {
            return Err(format!("{cap} game runs are open already; end one first"));
        }
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let file = dir.join(format!("{}-{}-{n}.run", std::process::id(), now_ms()));
        std::fs::write(&file, b"").map_err(|e| format!("No place made for the run: {e}"))?;
        // Two Studios taking at once: the later one gives way.
        if busy() > cap {
            let _ = std::fs::remove_file(&file);
            return Err(format!("{cap} game runs are open already; end one first"));
        }
        Ok(Slot { file })
    }

    /// The runs in `dir` that were touched lately (the stale files are
    /// removed).
    pub fn live(dir: &Path) -> usize {
        let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
        let mut n = 0;
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "run") {
                continue;
            }
            let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
            if age.is_some_and(|a| a.as_millis() < SLOT_MS as u128) {
                n += 1;
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
        n
    }

    /// Show the run is still there (every poll).
    pub fn touch(&self) {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&self.file) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_states_round_trip() {
        for c in [
            Command { seq: 1, op: Op::Apply, aim: None, focus: None },
            Command { seq: 2, op: Op::Probe, aim: None, focus: None },
            Command { seq: 3, op: Op::Stop, aim: None, focus: None },
            Command { seq: 4, op: Op::Restart, aim: None, focus: Some(Focus { car: Some("240SX".into()), clock: Some(600.5) }) },
            Command { seq: 5, op: Op::Restart, aim: Some(Aim::Race("a".into())), focus: Some(Focus::default()) },
            Command { seq: 7, op: Op::Restart, aim: Some(Aim::Race("dock_sprint".into())), focus: None },
            Command { seq: 9, op: Op::Apply, aim: Some(Aim::At(1.5, -2.0, 3.25, 90.0)), focus: None },
        ] {
            assert_eq!(Command::parse(&c.line()), Some(c));
        }
        assert_eq!(Command::parse("seq=x op=apply"), None);
        assert_eq!(Command::parse("seq=1 op=dance"), None);
        let s = GameState { pid: 4, seq: 3, beat_ms: 99, ok: false, note: "no race x in this mod".into(), turns: 12 };
        assert_eq!(GameState::parse(&s.line()), Some(s.clone()));
        let s = GameState { note: String::new(), ok: true, ..s };
        assert_eq!(GameState::parse(&s.line()), Some(s.clone()));
        assert!(s.alive(99 + ALIVE_MS - 1) && !s.alive(99 + ALIVE_MS));
    }

    #[test]
    fn files_go_through_the_folder() {
        let d = std::env::temp_dir().join(format!("ob-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(read_command(&d), None);
        write_command(&d, &Command { seq: 2, op: Op::Restart, aim: None, focus: None }).unwrap();
        assert_eq!(read_command(&d).unwrap().seq, 2);
        write_state(&d, &GameState { pid: 1, seq: 2, beat_ms: now_ms(), ok: true, note: String::new(), turns: 0 }).unwrap();
        assert!(read_state(&d).unwrap().alive(now_ms()));
        clear(&d);
        assert_eq!((read_command(&d), read_state(&d)), (None, None));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_focus_checks_its_words() {
        assert!(Focus::default().valid() && Focus::default().is_empty());
        assert!(Focus { car: Some("RX-8".into()), clock: Some(900.0) }.valid());
        for bad in [
            Focus { car: Some("../x".into()), clock: None },
            Focus { car: Some(String::new()), clock: None },
            Focus { car: None, clock: Some(0.0) },
            Focus { car: None, clock: Some(f32::NAN) },
            Focus { car: None, clock: Some(MAX_CLOCK + 1.0) },
        ] {
            assert!(!bad.valid(), "{bad:?}");
        }
        let f = Focus { car: Some("350Z".into()), clock: Some(120.0) };
        assert_eq!(f.args(), ["--map-car", "350Z", "--map-clock", "120"]);
        assert_eq!(Command::parse("seq=1 op=restart focus=none").unwrap().focus, Some(Focus::default()));
        assert_eq!(Command::parse("seq=1 op=restart").unwrap().focus, None, "no word: the run's own");
    }

    #[test]
    fn the_runs_are_capped_and_a_dead_studios_slots_expire() {
        let d = std::env::temp_dir().join(format!("ob-slots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = Slot::take_in(&d, 2).unwrap();
        let b = Slot::take_in(&d, 2).unwrap();
        assert_eq!(Slot::live(&d), 2);
        assert!(Slot::take_in(&d, 2).unwrap_err().contains("2 game runs"), "the third waits");
        drop(a);
        let c = Slot::take_in(&d, 2).expect("a place came free");
        // A slot nobody touches for a while stops counting.
        let old = std::time::SystemTime::now() - std::time::Duration::from_millis(SLOT_MS + 1000);
        let f = std::fs::OpenOptions::new().write(true).open(&b.file).unwrap();
        f.set_modified(old).unwrap();
        assert_eq!(Slot::live(&d), 1, "the expired one is not counted");
        b.touch();
        assert!(Slot::take_in(&d, 2).is_ok(), "a run that was gone is not holding a place");
        drop(c);
        let _ = std::fs::remove_dir_all(&d);
    }
}
