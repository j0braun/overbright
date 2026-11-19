//! The game's log file (docs/platform-dirs.md; `qol::log_file`): a plain game run
//! also writes its log to `platform::dirs::logs()/game-<UTC time>.log`,
//! keeping the newest [`KEEP`]. The lines are the ones the crash report's
//! ring keeps (`qol::crash`'s logger: warnings and errors always, plus what
//! `RUST_LOG` shows), home folders scrubbed to `~`. A thread of its own
//! writes the file: the game's threads only hand a line to a channel.
//!
//! Written only for the plain game run (no arguments but `--no-boot`) and
//! not with `NFSU2_LOG_FILE=0`: the developer harness's runs, `--check-game`,
//! `--set-root`, `--verify-cache` and the tests leave no file behind.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

/// Log files kept, the new one included.
pub const KEEP: usize = 5;

enum Msg {
    Line(Instant, String),
    Flush(Sender<()>),
}

static SINK: OnceLock<Sender<Msg>> = OnceLock::new();

/// Whether a run with these arguments (the program name left out) writes
/// a log file: the plain game run only (no arguments but `--no-boot`), and
/// `NFSU2_LOG_FILE` is not `0`.
pub fn wanted(args: &[String], env_switch: Option<&str>) -> bool {
    env_switch != Some("0") && args.iter().all(|a| a == "--no-boot")
}

/// Whether the file is being written.
pub fn active() -> bool {
    SINK.get().is_some()
}

/// Start writing to a new file in `dir` (once per process; later calls do
/// nothing). The folder is made, old files pruned and the file opened on
/// the writer thread.
pub fn start(dir: PathBuf) {
    let (tx, rx) = std::sync::mpsc::channel();
    if SINK.set(tx).is_err() {
        return;
    }
    let started = Instant::now();
    let spawned = std::thread::Builder::new().name("log-file".into()).spawn(move || writer(&dir, started, rx));
    if let Err(e) = spawned {
        eprintln!("log file not started: {e}");
    }
}

/// Hand a line to the writer (never blocks on the file).
pub fn send(line: String) {
    if let Some(tx) = SINK.get() {
        let _ = tx.send(Msg::Line(Instant::now(), line));
    }
}

/// A start-up fact a maintainer needs from a player's log (the adapter,
/// the swap chain's format and present mode): logged at Info, and written
/// to the file even while it keeps only warnings.
pub fn fact(line: &str) {
    if log::log_enabled!(target: "game", log::Level::Info) {
        log::info!(target: "game", "{line}");
    } else {
        send(format!("[INFO game] {line}"));
    }
}

/// Wait until the lines sent so far are in the file, at most `timeout`
/// (the panic hook, before the process may end).
pub fn flush(timeout: Duration) {
    if let Some(tx) = SINK.get() {
        let (ack, done) = std::sync::mpsc::channel();
        if tx.send(Msg::Flush(ack)).is_ok() {
            let _ = done.recv_timeout(timeout);
        }
    }
}

/// The old files to delete from a folder holding `names` so that, with the
/// new one, [`KEEP`] remain: the oldest `game-*.log` (the UTC time in the
/// name sorts them).
pub fn to_prune(names: &[String]) -> Vec<String> {
    let mut logs: Vec<&String> = names.iter().filter(|n| n.starts_with("game-") && n.ends_with(".log")).collect();
    logs.sort();
    let excess = logs.len().saturating_sub(KEEP - 1);
    logs.into_iter().take(excess).cloned().collect()
}

fn open(dir: &Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    std::fs::create_dir_all(dir)?;
    let names: Vec<String> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect();
    for old in to_prune(&names) {
        let _ = std::fs::remove_file(dir.join(old));
    }
    let stamp = timestamp(std::time::SystemTime::now());
    for n in 0..100 {
        let path = dir.join(if n == 0 { format!("game-{stamp}.log") } else { format!("game-{stamp}-{n}.log") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free log file name"))
}

fn writer(dir: &Path, started: Instant, rx: Receiver<Msg>) {
    let homes = crate::platform::dirs::home_tokens(&|name| std::env::var(name).ok());
    let (path, file) = match open(dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("log file not written in {}: {e}", scrub_homes(&dir.display().to_string(), &homes));
            // Drain, so senders never pile up.
            for m in rx {
                if let Msg::Flush(ack) = m {
                    let _ = ack.send(());
                }
            }
            return;
        }
    };
    let mut out = std::io::BufWriter::new(file);
    let _ = writeln!(
        out,
        "Overbright {} log, started {} UTC, {} {}, file {}",
        env!("CARGO_PKG_VERSION"),
        timestamp(std::time::SystemTime::now()),
        std::env::consts::OS,
        std::env::consts::ARCH,
        scrub_homes(&path.display().to_string(), &homes),
    );
    let _ = out.flush();
    while let Ok(first) = rx.recv() {
        let mut next = Some(first);
        while let Some(m) = next {
            match m {
                Msg::Line(at, line) => {
                    let t = at.saturating_duration_since(started).as_secs_f64();
                    let _ = writeln!(out, "{t:9.3} {}", scrub_homes(&line, &homes));
                }
                Msg::Flush(ack) => {
                    let _ = out.flush();
                    let _ = ack.send(());
                }
            }
            next = rx.try_recv().ok();
        }
        let _ = out.flush();
    }
}

/// The home folder replaced by `~`: `$HOME` and, on Windows,
/// `%USERPROFILE%` (`platform::dirs::home_tokens`).
pub fn scrub(s: &str) -> String {
    scrub_homes(s, &crate::platform::dirs::home_tokens(&|name| std::env::var(name).ok()))
}

/// [`scrub`] with the home folders given.
pub fn scrub_homes(s: &str, homes: &[(String, &str)]) -> String {
    let mut out = s.to_string();
    for (home, _) in homes {
        out = out.replace(home.as_str(), "~");
    }
    out
}

/// `YYYYMMDD-HHMMSS` in UTC.
pub fn timestamp(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil from days (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_plain_game_run_writes_one() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(wanted(&a(&[]), None));
        assert!(wanted(&a(&["--no-boot"]), None));
        assert!(wanted(&a(&[]), Some("1")));
        assert!(!wanted(&a(&[]), Some("0")));
        for flag in [
            "--shot",
            "--selftest",
            "--bench",
            "--flow",
            "--check-game",
            "--set-root",
            "--verify-cache",
            "--crash-test",
        ] {
            assert!(!wanted(&a(&[flag]), None), "{flag}");
        }
        assert!(!wanted(&a(&["--no-boot", "--shot"]), None));
    }

    #[test]
    fn pruning_keeps_the_newest_four_for_the_new_one() {
        let names: Vec<String> = [
            "game-20261101-100000.log",
            "game-20261103-100000.log",
            "game-20261102-100000.log",
            "game-20261105-100000.log",
            "game-20261104-100000.log",
            "game-20261104-100000-1.log",
            "other.txt",
            "game-notes.txt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(to_prune(&names), vec!["game-20261101-100000.log", "game-20261102-100000.log"]);
        assert!(to_prune(&names[..3]).is_empty());
    }

    #[test]
    fn the_writer_writes_scrubbed_lines_and_prunes() {
        let dir = std::env::temp_dir().join(format!("nfsu2-log-file-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for d in 1..=6 {
            std::fs::write(dir.join(format!("game-2026110{d}-000000.log")), "old").unwrap();
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let d = dir.clone();
        let t = std::thread::spawn(move || writer(&d, Instant::now(), rx));
        tx.send(Msg::Line(Instant::now(), "[WARN x] first".into())).unwrap();
        let (ack, done) = std::sync::mpsc::channel();
        tx.send(Msg::Flush(ack)).unwrap();
        done.recv_timeout(Duration::from_secs(10)).unwrap();
        drop(tx);
        t.join().unwrap();
        let mut names: Vec<String> =
            std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names.len(), KEEP, "{names:?}");
        assert_eq!(names[0], "game-20261103-000000.log");
        let new = std::fs::read_to_string(dir.join(names.last().unwrap())).unwrap();
        assert!(new.lines().nth(1).unwrap().ends_with(" [WARN x] first"), "{new}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
