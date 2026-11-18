//! The launcher's own log from its first line (docs/launcher.md "When the
//! launcher does not open"): `launcher.log` in Overbright's folder, next
//! to options.txt, so a start that ends before or soon after the window
//! opens still leaves a file a player can attach to a bug report.
//!
//! * [`start`]: the previous run's file is kept as `launcher.old.log`, the
//!   new one begins with the version, the system, the folders and the
//!   arguments; the logger (env_logger for the terminal, `RUST_LOG`) also
//!   writes every Info line and above here, each line written at once, so
//!   even a hard stop (a stack overflow, the process ended by the system)
//!   leaves the steps up to it. Home folders are shown as `~`.
//! * [`install_panic_hook`]: a panic, on any thread, is written to the file
//!   (message, place, thread, call stack) before the program unwinds or
//!   ends; [`panic_message`] hands its text to `main`, which says it in a
//!   message box on Windows (`console::fatal`).
//! * [`finish`]: the last line of a run that ended by itself. A file
//!   without it was left by a run that stopped hard; the next start says
//!   so in its own log.
//!
//! Nothing here may stop the launcher: a folder that cannot be written
//! leaves the launcher without a file (and says so on stderr).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The file's name in Overbright's folder.
pub const NAME: &str = "launcher.log";
/// The previous run's file.
pub const OLD_NAME: &str = "launcher.old.log";
/// The last line of a run that ended by itself ([`finish`]).
pub const END: &str = "end: the launcher closed normally";

static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
static PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
/// The arguments [`start`] was given (for [`follow`]'s header).
static ARGS: OnceLock<Vec<String>> = OnceLock::new();
static PANIC: Mutex<Option<String>> = Mutex::new(None);

/// Whether these arguments (the program name left out) are a window run
/// (the launcher window or the skip rule's game start): the runs that
/// write `launcher.log`. The tool flags print their output and leave the
/// player's log alone.
pub fn window_run(args: &[String]) -> bool {
    const TOOLS: [&str; 8] =
        ["--render", "--version", "--list-screens", "--list-folder", "--add-packages", "--add-hud-mod", "-h", "--help"];
    !args.iter().take_while(|a| *a != "--").any(|a| TOOLS.contains(&a.as_str()))
}

/// The log's path for this run (where [`start`] put it), or where it would
/// be in Overbright's folder.
pub fn path() -> PathBuf {
    PATH.lock().ok().and_then(|p| p.clone()).unwrap_or_else(|| super::dirs::data_dir().join(NAME))
}

/// Whether a run's log ended by itself ([`END`] is its last line).
pub fn ended_normally(text: &str) -> bool {
    text.lines().rev().find(|l| !l.trim().is_empty()).is_some_and(|l| l.contains(END))
}

/// The system line: the OS and, on Windows, its version and build number
/// (Windows 11 is version 10.0, build 22000 and later).
fn system_line() -> String {
    #[cfg(windows)]
    {
        if let Some((major, minor, build)) = crate::console::windows_version() {
            return format!("system: windows {major}.{minor} build {build}, {}", std::env::consts::ARCH);
        }
    }
    format!("system: {} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Opens `dir/launcher.log` (the previous one renamed to
/// `launcher.old.log`) and writes the header. Err: the file could not be
/// made; the launcher goes on without it.
pub fn open_in(dir: &Path, args: &[String]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(NAME);
    let old = dir.join(OLD_NAME);
    let previous = std::fs::read_to_string(&path).ok();
    if previous.is_some() {
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&path, &old);
    }
    let mut f = std::fs::File::create(&path)?;
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|e| format!("({e})"));
    let mut head = vec![
        crate::build_info::version_line(),
        format!("started (UTC): {}", crate::qol::saves::timestamp(std::time::SystemTime::now())),
        system_line(),
        format!("program: {exe}"),
        format!("arguments: {args:?}"),
        format!("Overbright's folder: {}", dir.display()),
    ];
    if let Some(p) = previous.filter(|p| !p.trim().is_empty() && !ended_normally(p)) {
        let last = p.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
        head.push(format!(
            "note: the previous start ended without closing normally ({OLD_NAME}; its last line: {last})"
        ));
    }
    for l in head {
        writeln!(f, "{}", crate::qol::crash::scrub(&l))?;
    }
    Ok(path)
}

/// Starts the log for this run: the file ([`open_in`] in Overbright's
/// folder) and the logger (stderr as `RUST_LOG` says, the file from Info
/// up). Call once, first thing in `main`.
pub fn start(args: &[String]) {
    let inner = env_logger::Builder::from_default_env().build();
    let level = inner.filter().max(log::LevelFilter::Info);
    if log::set_boxed_logger(Box::new(Logger { inner })).is_ok() {
        log::set_max_level(level);
    }
    let _ = ARGS.set(args.to_vec());
    open_here(&super::dirs::data_dir(), args);
}

/// Opens the log in `dir` ([`open_in`]) as this run's file.
fn open_here(dir: &Path, args: &[String]) -> bool {
    match open_in(dir, args) {
        Ok(p) => {
            if let Ok(mut g) = FILE.lock() {
                *g = std::fs::OpenOptions::new().append(true).open(&p).ok();
            }
            if let Ok(mut path) = PATH.lock() {
                *path = Some(p);
            }
            true
        }
        Err(e) => {
            eprintln!("overbright: no launcher log in {}: {e}", crate::qol::crash::scrub(&dir.display().to_string()));
            false
        }
    }
}

/// Overbright's folder moved while the launcher runs (portable mode turned
/// on): the log goes on in the new folder's `launcher.log`, the old file
/// ends with a line naming it. Nothing when [`start`] wrote no log or the
/// folder is the same.
pub fn follow(dir: &Path) {
    let Some(args) = ARGS.get() else { return };
    let old = path();
    if old.parent() == Some(dir) {
        return;
    }
    line(&format!("the log goes on in {}", dir.join(NAME).display()));
    finish();
    if open_here(dir, args) {
        line(&format!("the log goes on from {}", old.display()));
    }
}

/// One line into the file (nothing when there is no file).
pub fn line(text: &str) {
    if let Ok(mut g) = FILE.lock()
        && let Some(f) = g.as_mut()
    {
        let _ = writeln!(f, "{}", crate::qol::crash::scrub(text));
    }
}

/// The run's last line ([`END`]).
pub fn finish() {
    line(END);
}

/// The first panic's text ("message (at file:line)"), once one happened.
pub fn panic_message() -> Option<String> {
    PANIC.lock().ok().and_then(|p| p.clone())
}

/// The panic hook: the default message on stderr, then the panic in the
/// file before the thread unwinds (or the process ends). A panic that
/// `platform::monitor::guarded` catches stays quiet, as in the game.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if crate::platform::monitor::in_guard() {
            return;
        }
        default(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let at = info.location().map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        if let Ok(mut p) = PANIC.lock()
            && p.is_none()
        {
            *p = Some(format!("{message}{at}"));
        }
        let trace = readable_trace(&std::backtrace::Backtrace::force_capture().to_string());
        line(&format!("[PANIC {thread}]{at}: {message}\nbacktrace:\n{trace}"));
    }));
}

/// A call stack's text for the file: the function names (`.llvm.` suffixes
/// dropped), or one line saying the build has none (a stripped Windows
/// release lists only `<unknown>` frames).
pub fn readable_trace(trace: &str) -> String {
    let named = trace.lines().any(|l| {
        let l = l.trim_start();
        l.split_once(": ").is_some_and(|(n, rest)| n.parse::<u32>().is_ok() && rest.trim() != "<unknown>")
    });
    if named { crate::qol::crash::symbol_names(trace) } else { "   (no function names in this build)".into() }
}

/// The logger: env_logger as before (stderr, `RUST_LOG`), and every line
/// from Info up into the file.
struct Logger {
    inner: env_logger::Logger,
}

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        self.inner.enabled(m) || m.level() <= log::Level::Info
    }

    fn log(&self, r: &log::Record) {
        if self.inner.enabled(r.metadata()) {
            self.inner.log(r);
        }
        if r.level() <= log::Level::Info {
            line(&format!("[{} {}] {}", r.level(), r.target(), r.args()));
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// The message box's and stderr's text for a start that failed: what
/// happened, and where the log is.
pub fn failure_text(what: &str, log: &Path) -> String {
    let at = match super::model::source_url() {
        Some(u) => format!(" at {}/issues", u.trim_end_matches('/')),
        None => String::new(),
    };
    format!("{what}\n\nThe launcher's log is {}\nPlease attach that file to a bug report{at}.", log.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn window_runs_log_and_tool_flags_do_not() {
        assert!(window_run(&[]));
        assert!(window_run(&args(&["--launcher", "--screen", "main"])));
        assert!(window_run(&args(&["--", "--version"])), "after -- the arguments are the game's");
        for t in
            ["--render", "--version", "--list-screens", "--list-folder", "--add-packages", "--add-hud-mod", "--help"]
        {
            assert!(!window_run(&args(&[t])), "{t}");
        }
    }

    #[test]
    fn the_log_keeps_the_previous_run_and_notes_a_hard_stop() {
        let dir = std::env::temp_dir().join(format!("ob-start-log-{}", crate::unique_tag()));
        let p = open_in(&dir, &args(&["--launcher"])).unwrap();
        assert_eq!(p, dir.join(NAME));
        let first = std::fs::read_to_string(&p).unwrap();
        assert!(first.starts_with("Overbright "), "{first}");
        assert!(first.contains("system: ") && first.contains("arguments: [\"--launcher\"]"), "{first}");
        assert!(!first.contains("note:"), "a first run has no previous one: {first}");
        // That run stopped hard (no end line): the next one says so.
        std::fs::write(&p, format!("{first}window: open\n")).unwrap();
        open_in(&dir, &[]).unwrap();
        let second = std::fs::read_to_string(&p).unwrap();
        assert!(second.contains("note: the previous start ended without closing normally"), "{second}");
        assert!(second.contains("its last line: window: open"), "{second}");
        assert!(std::fs::read_to_string(dir.join(OLD_NAME)).unwrap().ends_with("window: open\n"));
        // A run that closed normally is not noted.
        std::fs::write(&p, format!("{second}{END}\n")).unwrap();
        open_in(&dir, &[]).unwrap();
        assert!(!std::fs::read_to_string(&p).unwrap().contains("note:"));
        assert!(ended_normally(&std::fs::read_to_string(dir.join(OLD_NAME)).unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Portable mode turned on while the launcher runs: the log goes on in
    /// the new folder, the old file ends normally and names the new one.
    /// The only test that uses this module's open file.
    #[test]
    fn the_log_follows_a_moved_folder() {
        let base = std::env::temp_dir().join(format!("ob-start-log-follow-{}", crate::unique_tag()));
        let (a, b) = (base.join("home"), base.join("prog/data"));
        let _ = ARGS.set(vec!["--launcher".into()]);
        assert!(open_here(&a, &["--launcher".into()]));
        line("before");
        follow(&b);
        line("after");
        let old = std::fs::read_to_string(a.join(NAME)).unwrap();
        assert!(old.contains("before") && old.contains(&format!("the log goes on in {}", b.join(NAME).display())));
        assert!(ended_normally(&old), "{old}");
        let new = std::fs::read_to_string(b.join(NAME)).unwrap();
        assert!(new.contains("the log goes on from") && new.contains("after") && !new.contains("before"), "{new}");
        assert_eq!(path(), b.join(NAME));
        // The same folder again: nothing changes.
        follow(&b);
        assert_eq!(std::fs::read_to_string(b.join(NAME)).unwrap().matches("goes on from").count(), 1);
        if let Ok(mut g) = FILE.lock() {
            *g = None;
        }
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_stripped_call_stack_is_one_line() {
        assert_eq!(readable_trace("   0: <unknown>\n   1: <unknown>\n"), "   (no function names in this build)");
        let named = "   0: std::panicking::begin_panic\n             at /x.rs:1\n   1: <unknown>\n";
        assert_eq!(readable_trace(named), named);
    }

    #[test]
    fn the_failure_text_names_the_log() {
        let t = failure_text("overbright: no display", Path::new("C:/x/launcher.log"));
        assert!(t.starts_with("overbright: no display\n\n"), "{t}");
        assert!(t.contains("C:/x/launcher.log") && t.contains("bug report"), "{t}");
    }
}
