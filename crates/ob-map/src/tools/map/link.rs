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

/// `<maps>/<folder>/.editor/link`.
pub fn dir(maps: &Path, folder: &str) -> PathBuf {
    super::store::editor_dir(maps, folder).join(DIR)
}

/// What Studio asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Read the mod file again and start the run over with it (props, race
    /// points and all).
    Apply,
    /// Start the run over with the mod as the game has it.
    Restart,
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
}

impl Command {
    /// `seq=3 op=apply race=dock` (or `at=1,2,3,90`).
    pub fn line(&self) -> String {
        let op = if self.op == Op::Apply { "apply" } else { "restart" };
        let aim = match &self.aim {
            None => String::new(),
            Some(Aim::Race(id)) => format!(" race={id}"),
            Some(Aim::At(x, y, z, h)) => format!(" at={x},{y},{z},{h}"),
        };
        format!("seq={} op={op}{aim}\n", self.seq)
    }

    pub fn parse(text: &str) -> Option<Command> {
        let (mut seq, mut op, mut aim) = (None, None, None);
        for (k, v) in text.split_whitespace().filter_map(|w| w.split_once('=')) {
            match k {
                "seq" => seq = v.parse().ok(),
                "op" => {
                    op = match v {
                        "apply" => Some(Op::Apply),
                        "restart" => Some(Op::Restart),
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
                _ => {}
            }
        }
        Some(Command { seq: seq?, op: op?, aim })
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
}

impl GameState {
    pub fn line(&self) -> String {
        let note = self.note.replace(['\n', '\r'], " ");
        format!("pid={} seq={} beat={} ok={}{}{}\n", self.pid, self.seq, self.beat_ms, if self.ok { "yes" } else { "no" }, if note.is_empty() { "" } else { " note=" }, note)
    }

    pub fn parse(text: &str) -> Option<GameState> {
        let text = text.lines().next()?;
        let (head, note) = text.split_once(" note=").unwrap_or((text, ""));
        let (mut pid, mut seq, mut beat, mut ok) = (None, None, None, None);
        for (k, v) in head.split_whitespace().filter_map(|w| w.split_once('=')) {
            match k {
                "pid" => pid = v.parse().ok(),
                "seq" => seq = v.parse().ok(),
                "beat" => beat = v.parse().ok(),
                "ok" => ok = Some(v == "yes"),
                _ => {}
            }
        }
        Some(GameState { pid: pid?, seq: seq?, beat_ms: beat?, ok: ok?, note: note.to_string() })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_states_round_trip() {
        for c in [
            Command { seq: 1, op: Op::Apply, aim: None },
            Command { seq: 7, op: Op::Restart, aim: Some(Aim::Race("dock_sprint".into())) },
            Command { seq: 9, op: Op::Apply, aim: Some(Aim::At(1.5, -2.0, 3.25, 90.0)) },
        ] {
            assert_eq!(Command::parse(&c.line()), Some(c));
        }
        assert_eq!(Command::parse("seq=x op=apply"), None);
        assert_eq!(Command::parse("seq=1 op=dance"), None);
        let s = GameState { pid: 4, seq: 3, beat_ms: 99, ok: false, note: "no race x in this mod".into() };
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
        write_command(&d, &Command { seq: 2, op: Op::Restart, aim: None }).unwrap();
        assert_eq!(read_command(&d).unwrap().seq, 2);
        write_state(&d, &GameState { pid: 1, seq: 2, beat_ms: now_ms(), ok: true, note: String::new() }).unwrap();
        assert!(read_state(&d).unwrap().alive(now_ms()));
        clear(&d);
        assert_eq!((read_command(&d), read_state(&d)), (None, None));
        let _ = std::fs::remove_dir_all(&d);
    }
}
