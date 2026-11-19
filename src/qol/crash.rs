use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static ADAPTER: OnceLock<String> = OnceLock::new();
static AUDIO_OUTPUT: Mutex<Option<String>> = Mutex::new(None);
static DEVIATIONS: Mutex<String> = Mutex::new(String::new());
static LOG: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static KEEP_LOG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const LOG_LINES: usize = 200;

/// Where the reports go: `platform::dirs::crashes()` (`<data home>/overbright/crashes/`).
pub fn dir() -> PathBuf {
    crate::platform::dirs::crashes()
}

/// The GPU adapter, once known (for the report).
pub fn set_adapter(info: &wgpu::AdapterInfo) {
    let _ = ADAPTER.set(format!(
        "{} (vendor 0x{:04X}, device 0x{:04X}, {:?}, driver {} {})",
        info.name, info.vendor, info.device, info.backend, info.driver, info.driver_info
    ));
}

/// The adapter line [`set_adapter`] kept (the support file's "in use").
pub fn adapter() -> Option<String> {
    ADAPTER.get().cloned()
}

/// The audio output last opened (device, rate, channels; the support
/// file's "audio output in use"). A reopened stream replaces it.
pub fn set_audio_output(line: String) {
    if let Ok(mut a) = AUDIO_OUTPUT.lock() {
        *a = Some(line);
    }
}

/// The line [`set_audio_output`] kept.
pub fn audio_output() -> Option<String> {
    AUDIO_OUTPUT.lock().ok().and_then(|a| a.clone())
}

/// The deviations line the report shows.
pub fn set_deviations(text: String) {
    if let Ok(mut d) = DEVIATIONS.lock() {
        *d = text;
    }
}

/// From now on the log keeps warnings and errors for the report (also
/// when `RUST_LOG` hides them), raising the log's level to Warn if it
/// was lower.
pub fn keep_log() {
    KEEP_LOG.store(true, std::sync::atomic::Ordering::Relaxed);
    if log::max_level() < log::LevelFilter::Warn {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

/// env_logger with the report's ring of the last log lines on top.
struct Logger {
    inner: env_logger::Logger,
}

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        self.inner.enabled(m)
            || ((KEEP_LOG.load(std::sync::atomic::Ordering::Relaxed) || super::log_file::active())
                && m.level() <= log::Level::Warn)
    }

    fn log(&self, r: &log::Record) {
        if self.inner.enabled(r.metadata()) {
            self.inner.log(r);
        }
        // The report's ring and the log file (`log_file`) keep the same lines.
        let keep = KEEP_LOG.load(std::sync::atomic::Ordering::Relaxed);
        let file = super::log_file::active();
        if (keep || file) && (r.level() <= log::Level::Warn || self.inner.enabled(r.metadata())) {
            let line = format!("[{} {}] {}", r.level(), r.target(), r.args());
            if !keep {
                super::log_file::send(line);
                return;
            }
            if file {
                super::log_file::send(line.clone());
            }
            if let Ok(mut l) = LOG.lock() {
                if l.len() >= LOG_LINES {
                    l.pop_front();
                }
                l.push_back(line);
            }
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Install the logger (env_logger as before: `RUST_LOG`) and the panic
/// hook. The hook writes a report only while crash reports are on; the
/// default hook's message is printed either way. A plain game run (the
/// process's arguments, `log_file::wanted`) also starts the log file in
/// `platform::dirs::logs()`.
pub fn install() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let inner = env_logger::Builder::from_default_env().build();
    let level = inner.filter();
    if log::set_boxed_logger(Box::new(Logger { inner })).is_ok() {
        log::set_max_level(level);
        if super::log_file::wanted(&args, std::env::var("NFSU2_LOG_FILE").ok().as_deref()) {
            super::log_file::start(crate::platform::dirs::logs());
            if log::max_level() < log::LevelFilter::Warn {
                log::set_max_level(log::LevelFilter::Warn);
            }
        }
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A monitor call's panic the caller catches and answers with a
        // fallback (`platform::monitor::guarded`): no message, no report.
        if crate::platform::monitor::in_guard() {
            return;
        }
        default(info);
        if super::log_file::active() {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(no message)".into());
            let at = info.location().map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
            let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
            // The call stack too: a panic inside a library (a wgpu
            // validation error) names only the library's line otherwise,
            // and the report (Crash Reports, off by default) may not exist.
            let trace = symbol_names(&std::backtrace::Backtrace::force_capture().to_string());
            super::log_file::send(format!("[PANIC {thread}]{at}: {message}\nbacktrace:\n{trace}"));
            // What the window, the display and the GPU did just before.
            super::log_file::send(format!(
                "[WARN events] the last events (a panic):\n{}",
                scrub(&crate::diag::events::text())
            ));
            super::log_file::flush(std::time::Duration::from_millis(500));
        }
        if super::crash_reports() {
            match write_report(info) {
                Ok(p) => eprintln!("crash report written to {}", p.display()),
                Err(e) => eprintln!("crash report not written: {e}"),
            }
        }
    }));
}

fn write_report(info: &std::panic::PanicHookInfo) -> std::io::Result<PathBuf> {
    let report = report(info);
    let dir = dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("crash-{}.txt", super::saves::timestamp(std::time::SystemTime::now())));
    std::fs::write(&path, report)?;
    Ok(path)
}

/// The report's text, home paths shown as `~`.
pub fn report(info: &std::panic::PanicHookInfo) -> String {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".into());
    let location = info.location().map_or_else(String::new, |l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
    let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
    let backtrace = symbol_names(&std::backtrace::Backtrace::force_capture().to_string());
    let os = std::fs::read_to_string("/proc/version")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    let log: Vec<String> = LOG.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default();
    let mut s = String::new();
    s += &format!("Overbright {} crash report\n", env!("CARGO_PKG_VERSION"));
    s += &format!("time (UTC): {}\n", super::saves::timestamp(std::time::SystemTime::now()));
    s += &format!("os: {os}\n");
    s += &format!("gpu: {}\n", ADAPTER.get().map_or("(not yet known)", String::as_str));
    s += &format!(
        "game files: {}\n",
        crate::fingerprint::current().map_or_else(|| "(not fingerprinted yet)".to_string(), |f| f.summary())
    );
    s += &format!(
        "deviations on: {}\n",
        DEVIATIONS.lock().map(|d| if d.is_empty() { "none".to_string() } else { d.clone() }).unwrap_or_default()
    );
    s += &format!("\npanic on thread '{thread}' at {location}:\n{message}\n\nbacktrace:\n{backtrace}\n");
    s += &format!("\nlast {} log lines:\n", log.len());
    for l in &log {
        s += l;
        s.push('\n');
    }
    scrub(&s)
}

/// A backtrace's text with the `.llvm.<word>` suffix dropped from its
/// symbol names. A release build made reproducible names some functions
/// `name.llvm.<crate>_<hash>_cgu_<n>` (the module ThinLTO promoted them
/// from); the report shows the plain name. Everything else is kept.
pub fn symbol_names(s: &str) -> String {
    const SUFFIX: &str = ".llvm.";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(SUFFIX) {
        let after = &rest[i + SUFFIX.len()..];
        let n = after.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
        if n == 0 {
            out.push_str(&rest[..i + SUFFIX.len()]);
        } else {
            out.push_str(&rest[..i]);
        }
        rest = &after[n..];
    }
    out.push_str(rest);
    out
}

/// The home folder replaced by `~` (with the log file, `log_file::scrub`).
pub use crate::log_file::{scrub, scrub_homes};

#[cfg(test)]
mod tests {
    use super::symbol_names;

    #[test]
    fn the_llvm_suffix_is_dropped_from_symbol_names() {
        let frame = "  12: nfsu2::render::native::world::draws::submit.llvm.nfsu2_8f3a1c0e5b7d2946_cgu_07\n             at ./src/render/native/world/draws.rs:120:9\n";
        assert_eq!(
            symbol_names(frame),
            "  12: nfsu2::render::native::world::draws::submit\n             at ./src/render/native/world/draws.rs:120:9\n"
        );
        // The older hex form, several in one text, and a line without one.
        assert_eq!(symbol_names("a.llvm.123456789 b::c.llvm.x_1_cgu_0 d"), "a b::c d");
        let plain = "   3: std::panicking::begin_panic_handler\n";
        assert_eq!(symbol_names(plain), plain);
        // `.llvm.` with no word after it is not a suffix.
        assert_eq!(symbol_names("x.llvm. y"), "x.llvm. y");
    }
}
