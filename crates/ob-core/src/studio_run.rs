//! A running Studio (or map editor) leaves a note that goes only with a clean end
//! (docs/troubleshooting.md "After a crash"). The launcher reads what is left to show
//! the same report window the game's bad end gets: Studio's log, its version, the mod
//! that was open (its name only) and how it ended.
//!
//! Each program writes `studio-runs/run-<pid>.txt` in Overbright's own folder
//! ([`dir`]) and holds an exclusive lock on it for as long as it lives; the system lets
//! go of the lock when the process ends however it ends. A note whose lock is free is
//! one nobody lives to remove: the program crashed, panicked or was killed. A clean end
//! (a quit, or an error that was told to the player) removes the note. Nothing here
//! reads or changes the game; the log written is `studio-<UTC time>.log` in the logs
//! folder (five kept), home folders scrubbed to `~`.

use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// The notes' folder, in Overbright's own folder.
pub const DIR: &str = "studio-runs";
/// Log files kept, the new one included.
pub const KEEP: usize = 5;
/// The log file name's start.
pub const LOG_PREFIX: &str = "studio-";

/// The notes' folder.
pub fn dir() -> PathBuf {
    crate::platform::dirs::config().join(DIR)
}

/// What a note says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// "Overbright Studio" or "Overbright map editor".
    pub program: String,
    /// The program's file name without its extension (the system's records are looked up by it).
    pub exe: String,
    pub pid: u32,
    /// When it started (Unix time, ms).
    pub since: u64,
    /// The build (`build_info::version_line`).
    pub version: String,
    /// The mod open now: its name only.
    pub opened: Option<String>,
    /// The log file's name, in the logs folder.
    pub log: Option<String>,
    /// A panic's first line, once one happened.
    pub panic: Option<String>,
}

fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

impl Note {
    fn write(&self) -> String {
        let mut s = format!(
            "# A running Overbright program; this note goes with a clean end.\nprogram = {}\nexe = {}\npid = {}\nsince = {}\nversion = {}\n",
            one_line(&self.program),
            one_line(&self.exe),
            self.pid,
            self.since,
            one_line(&self.version)
        );
        for (k, v) in [("mod", &self.opened), ("log", &self.log), ("panic", &self.panic)] {
            if let Some(v) = v {
                s += &format!("{k} = {}\n", one_line(v));
            }
        }
        s
    }

    /// None for text that holds no process id (a note not written yet).
    pub fn read(text: &str) -> Option<Note> {
        let mut n = Note::default();
        let mut pid = false;
        for line in text.lines() {
            let Some((k, v)) = line.split_once(" = ") else { continue };
            let v = v.trim().to_string();
            match k {
                "program" => n.program = v,
                "exe" => n.exe = v,
                "pid" => {
                    n.pid = v.parse().ok()?;
                    pid = true;
                }
                "since" => n.since = v.parse().unwrap_or(0),
                "version" => n.version = v,
                "mod" => n.opened = Some(v),
                "log" => n.log = Some(v),
                "panic" => n.panic = Some(v),
                _ => {}
            }
        }
        pid.then_some(n)
    }
}

/// A note held by a living program: the lock goes with this value (and with the process).
pub struct Held {
    file: File,
    path: PathBuf,
    pub note: Note,
}

impl Held {
    /// Writes the note for this process in `dir` and takes its lock.
    pub fn create(dir: &Path, mut note: Note) -> std::io::Result<Held> {
        std::fs::create_dir_all(dir)?;
        note.pid = std::process::id();
        if note.since == 0 {
            note.since = unix_ms(SystemTime::now());
        }
        let path = dir.join(format!("run-{}.txt", note.pid));
        let file = File::options().read(true).write(true).create(true).truncate(true).open(&path)?;
        file.lock()?;
        let mut h = Held { file, path, note };
        h.save()?;
        Ok(h)
    }

    fn save(&mut self) -> std::io::Result<()> {
        let text = self.note.write();
        self.file.rewind()?;
        self.file.set_len(0)?;
        self.file.write_all(text.as_bytes())?;
        self.file.flush()
    }

    /// The mod open now (None: none).
    pub fn set_open(&mut self, name: Option<&str>) {
        let name = name.map(str::to_string);
        if self.note.opened != name {
            self.note.opened = name;
            let _ = self.save();
        }
    }

    /// A panic happened: its first line.
    pub fn record_panic(&mut self, line: &str) {
        self.note.panic = Some(line.lines().next().unwrap_or("").to_string());
        let _ = self.save();
    }

    /// A clean end: the note goes. (Dropping a `Held` without this is a crash for the reader: the lock
    /// goes, the note stays.)
    pub fn finish(self) {
        let Held { file, path, .. } = self;
        drop(file);
        let _ = std::fs::remove_file(path);
    }
}

fn unix_ms(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A program that ended without a clean end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadExit {
    pub note: Note,
    /// The program's log, when it wrote one that is still there.
    pub log: Option<PathBuf>,
    /// When it last wrote (the log's, else the note's time): its run's length is from `note.since`.
    pub last_seen: SystemTime,
}

impl BadExit {
    /// Seconds from the start to the last sign of life.
    pub fn secs(&self) -> f32 {
        let since = SystemTime::UNIX_EPOCH + Duration::from_millis(self.note.since);
        self.last_seen.duration_since(since).map_or(0.0, |d| d.as_secs_f32())
    }

    /// The program's start.
    pub fn started_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.note.since)
    }
}

/// The notes in `dir` whose lock nobody holds (their programs ended badly), oldest first, taken: each
/// note is removed, so a bad end is told once. A note that is held (a living program), empty (being
/// written) or unreadable is left alone. `logs` is where the programs' logs are.
pub fn take_bad_exits(dir: &Path, logs: &Path) -> Vec<BadExit> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("run-")) {
            continue;
        }
        let Ok(mut f) = File::options().read(true).write(true).open(&p) else { continue };
        if f.try_lock().is_err() {
            continue;
        }
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut f, &mut text);
        let Some(note) = Note::read(&text) else { continue };
        let modified = f.metadata().and_then(|m| m.modified()).unwrap_or_else(|_| SystemTime::now());
        let log = note.log.as_ref().map(|n| logs.join(n)).filter(|l| l.is_file());
        let last_seen =
            log.as_ref().and_then(|l| std::fs::metadata(l).and_then(|m| m.modified()).ok()).unwrap_or(modified);
        drop(f);
        let _ = std::fs::remove_file(&p);
        out.push(BadExit { note, log, last_seen });
    }
    out.sort_by_key(|b| (b.note.since, b.note.pid));
    out
}

/// The last `n` lines of a log file (empty when it cannot be read).
pub fn tail_lines(log: &Path, n: usize) -> Vec<String> {
    let Ok(b) = std::fs::read(log) else { return Vec::new() };
    let t = String::from_utf8_lossy(&b);
    let lines: Vec<&str> = t.lines().collect();
    lines[lines.len().saturating_sub(n)..].iter().map(|s| s.to_string()).collect()
}

/// The old logs to delete from a folder holding `names` so that, with the new one, [`KEEP`] remain.
pub fn to_prune(names: &[String]) -> Vec<String> {
    let mut logs: Vec<&String> = names.iter().filter(|n| n.starts_with(LOG_PREFIX) && n.ends_with(".log")).collect();
    logs.sort();
    let excess = logs.len().saturating_sub(KEEP - 1);
    logs.into_iter().take(excess).cloned().collect()
}

fn open_log(dir: &Path) -> std::io::Result<(PathBuf, File)> {
    std::fs::create_dir_all(dir)?;
    let names: Vec<String> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect();
    for old in to_prune(&names) {
        let _ = std::fs::remove_file(dir.join(old));
    }
    let stamp = crate::log_file::timestamp(SystemTime::now());
    for n in 0..100 {
        let name = if n == 0 { format!("{LOG_PREFIX}{stamp}.log") } else { format!("{LOG_PREFIX}{stamp}-{n}.log") };
        let path = dir.join(name);
        match File::options().write(true).create_new(true).open(&path) {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free log file name"))
}

/// This process's note and log.
struct Me {
    held: Option<Held>,
    log: Option<BufWriter<File>>,
    started: Instant,
}

static ME: Mutex<Option<Me>> = Mutex::new(None);

fn me() -> std::sync::MutexGuard<'static, Option<Me>> {
    ME.lock().unwrap_or_else(|e| e.into_inner())
}

/// The log's lines: what `inner` shows, plus warnings and errors always, in the file too.
struct Tee {
    inner: Box<dyn log::Log>,
}

impl log::Log for Tee {
    fn enabled(&self, m: &log::Metadata) -> bool {
        self.inner.enabled(m) || m.level() <= log::Level::Warn
    }

    fn log(&self, r: &log::Record) {
        if self.inner.enabled(r.metadata()) {
            self.inner.log(r);
        }
        if self.inner.enabled(r.metadata()) || r.level() <= log::Level::Warn {
            line(&format!("[{} {}] {}", r.level(), r.target(), r.args()));
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

fn line(text: &str) {
    if let Some(m) = me().as_mut()
        && let Some(w) = m.log.as_mut()
    {
        let t = m.started.elapsed().as_secs_f64();
        let _ = writeln!(w, "{t:9.3} {}", crate::log_file::scrub(text));
        let _ = w.flush();
    }
}

/// Starts this program's run: the log file, the note and the panic hook, and installs the logger (`inner`,
/// at the level `filter`; the file keeps warnings and errors too). Call once, in the window run (not for
/// `--version` or a picture).
pub fn begin(program: &str, inner: Box<dyn log::Log>, filter: log::LevelFilter) {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_default();
    let mut note = Note {
        program: program.to_string(),
        exe,
        version: crate::build_info::version_line(),
        ..Note::default()
    };
    let mut log_writer = None;
    match open_log(&crate::platform::dirs::logs()) {
        Ok((path, mut f)) => {
            let _ = writeln!(
                f,
                "{program} {} log, started {} UTC, {} {}",
                crate::build_info::version_line(),
                crate::log_file::timestamp(SystemTime::now()),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            note.log = path.file_name().map(|n| n.to_string_lossy().to_string());
            log_writer = Some(BufWriter::new(f));
        }
        Err(e) => eprintln!("{program}: log file not written: {e}"),
    }
    let held = match Held::create(&dir(), note) {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("{program}: run note not written: {e}");
            None
        }
    };
    *me() = Some(Me { held, log: log_writer, started: Instant::now() });
    if log::set_logger(Box::leak(Box::new(Tee { inner }))).is_ok() {
        log::set_max_level(filter.max(log::LevelFilter::Warn));
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let at = info.location().map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        line(&format!("[PANIC {thread}]{at}: {message}\nbacktrace:\n{}", std::backtrace::Backtrace::force_capture()));
        if let Some(m) = me().as_mut()
            && let Some(h) = m.held.as_mut()
        {
            h.record_panic(&format!("{message}{at}"));
        }
    }));
}

/// The mod open now, by name (None: none). Cheap when it did not change.
pub fn set_open(name: Option<&str>) {
    if let Some(m) = me().as_mut()
        && let Some(h) = m.held.as_mut()
    {
        h.set_open(name);
    }
}

/// A clean end: the note goes (the log stays). Everything that returns from `main` calls this, a crash or
/// a kill does not.
pub fn finish() {
    let mine = me().as_mut().and_then(|m| m.held.take());
    if let Some(h) = mine {
        h.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-studio-run-{tag}-{}", crate::unique_tag()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn note(name: &str) -> Note {
        Note { program: "Overbright Studio".into(), exe: "overbright-studio".into(), opened: Some(name.into()), ..Note::default() }
    }

    #[test]
    fn a_clean_quit_leaves_nothing_a_crash_does() {
        let d = tmp("clean");
        let (runs, logs) = (d.join("runs"), d.join("logs"));
        let h = Held::create(&runs, note("drift-park")).unwrap();
        // Alive: its lock is held, the note is not a bad end.
        assert!(take_bad_exits(&runs, &logs).is_empty());
        h.finish();
        assert!(take_bad_exits(&runs, &logs).is_empty());
        assert_eq!(std::fs::read_dir(&runs).unwrap().count(), 0, "a clean quit removes the note");
        // Killed: the lock goes with the process, the note stays; told once.
        let h = Held::create(&runs, note("drift-park")).unwrap();
        drop(h);
        let got = take_bad_exits(&runs, &logs);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].note.opened.as_deref(), Some("drift-park"));
        assert!(got[0].note.panic.is_none());
        assert!(take_bad_exits(&runs, &logs).is_empty(), "told once");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_panic_is_in_the_note_and_the_log_is_found() {
        let d = tmp("panic");
        let (runs, logs) = (d.join("runs"), d.join("logs"));
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("studio-1.log"), "1.0 [INFO x] hello\n2.0 [PANIC main]: boom\n").unwrap();
        let mut n = note("m");
        n.log = Some("studio-1.log".into());
        let mut h = Held::create(&runs, n).unwrap();
        h.set_open(None);
        h.record_panic("boom at src/x.rs:1\nsecond line");
        drop(h);
        let got = take_bad_exits(&runs, &logs);
        assert_eq!(got[0].note.panic.as_deref(), Some("boom at src/x.rs:1"));
        assert_eq!(got[0].note.opened, None);
        assert_eq!(got[0].log.as_deref(), Some(logs.join("studio-1.log").as_path()));
        assert_eq!(tail_lines(got[0].log.as_ref().unwrap(), 1), ["2.0 [PANIC main]: boom"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_note_being_written_or_unreadable_is_left_alone() {
        let d = tmp("odd");
        let (runs, logs) = (d.join("runs"), d.join("logs"));
        std::fs::create_dir_all(&runs).unwrap();
        std::fs::write(runs.join("run-1.txt"), "").unwrap();
        std::fs::write(runs.join("run-2.txt"), "nonsense").unwrap();
        assert!(take_bad_exits(&runs, &logs).is_empty());
        assert_eq!(std::fs::read_dir(&runs).unwrap().count(), 2);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn old_logs_are_pruned() {
        let names: Vec<String> = (1..=6).map(|i| format!("studio-{i}.log")).chain(["game-1.log".to_string()]).collect();
        assert_eq!(to_prune(&names), ["studio-1.log", "studio-2.log"]);
    }
}
