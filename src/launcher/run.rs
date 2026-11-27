//! Starting and watching the game (docs/launcher.md): `game` beside the
//! launcher with the arguments given after `--`, its output read into the
//! launcher's own log file in the logs folder, and the end judged — a
//! non-zero exit or a panic line is a failure, and one within
//! [`START_SECS`] a failed start. Also: one launcher at a time (a lock
//! file), the skip rule's "Shift held" check, and opening the logs folder.
//!
//! The game writes its own log file for a plain run (`qol::log_file`: no
//! arguments but `--no-boot`, `NFSU2_LOG_FILE` not 0). The launcher's file
//! (`launcher-<UTC time>.log`) then holds only its own lines and, after a
//! failure, the game's last output lines; for the other runs, which leave
//! no game log, it holds all of the game's output.

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A non-zero exit within this many seconds of the start is a failed
/// start (the design's "within N seconds").
pub const START_SECS: f32 = 10.0;
pub use crate::launch::error::{ErrorKind, KIND_PREFIX, NO_WINDOW};

/// The output lines kept for the failure screen and the log.
pub const TAIL: usize = 200;
/// The launcher's log files kept, the new one included.
pub const KEEP: usize = 5;

#[derive(Default)]
struct Shared {
    tail: VecDeque<String>,
    panicked: bool,
    /// The game said its window could not open (it then ends with exit
    /// code 0: a failure all the same).
    no_window: bool,
    /// The game's `error.kind=` line, if it printed one.
    kind: Option<ErrorKind>,
    /// Every line goes to the file (the game writes no log of its own).
    file_all: bool,
    file: Option<std::io::BufWriter<std::fs::File>>,
}

impl Shared {
    fn line(&mut self, from: &str, line: String) {
        if line.trim().is_empty() {
            return;
        }
        if line.contains("panicked at") || line.starts_with("[PANIC") {
            self.panicked = true;
        }
        if line.starts_with(NO_WINDOW) {
            self.no_window = true;
        }
        // The machine line is for the launcher: kept in the file, not
        // in the lines the failure screen shows.
        let kind = ErrorKind::parse(&line);
        if kind.is_some() {
            self.kind = kind;
        }
        if self.file_all
            && let Some(f) = self.file.as_mut()
        {
            let _ = writeln!(f, "[{from}] {}", crate::qol::crash::scrub(&line));
        }
        if kind.is_some() {
            return;
        }
        if self.tail.len() == TAIL {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
    }
}

/// The game, running.
pub struct Running {
    child: Child,
    started: Instant,
    started_at: std::time::SystemTime,
    shared: Arc<Mutex<Shared>>,
    readers: Vec<std::thread::JoinHandle<()>>,
    /// The start marker this start wrote (removed at the end).
    marker: PathBuf,
    pub log: Option<PathBuf>,
    /// The resident memory (bytes) at which the launcher stops this run, and when it looked last (opt-in,
    /// [`MEM_CAP_ENV`]; None: no cap, nothing is measured).
    cap: Option<(u64, Instant)>,
}

/// Opt-in memory cap for the game runs the launcher starts: the resident memory in MiB (256 to 1048576) at which
/// the launcher stops the game it started, so one runaway run cannot take the machine down. Unset: no cap, and the
/// launcher measures nothing.
pub const MEM_CAP_ENV: &str = "OVERBRIGHT_RUN_MEM_CAP_MB";

/// The cap a value of [`MEM_CAP_ENV`] names, in bytes (None: unset or not a size in range).
pub fn mem_cap_bytes(v: Option<&str>) -> Option<u64> {
    let mb: u64 = v?.trim().parse().ok().filter(|m| (256..=1_048_576).contains(m))?;
    Some(mb << 20)
}

/// The resident memory of a process the launcher started, in bytes (None: not measurable).
#[cfg(unix)]
fn resident_bytes(child: &Child) -> Option<u64> {
    let t = std::fs::read_to_string(format!("/proc/{}/status", child.id())).ok()?;
    let kb = t.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.split_whitespace().next()?.parse::<u64>().ok()?;
    Some(kb * 1024)
}

#[cfg(windows)]
fn resident_bytes(child: &Child) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    let mut c = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
    // SAFETY: the handle is the live child's own; `c` is a properly sized counters block.
    unsafe { GetProcessMemoryInfo(HANDLE(child.as_raw_handle()), &mut c, c.cb) }.ok()?;
    Some(c.WorkingSetSize as u64)
}

#[cfg(not(any(unix, windows)))]
fn resident_bytes(_: &Child) -> Option<u64> {
    None
}

/// How the game ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    /// The exit code (None: ended by a signal).
    pub code: Option<i32>,
    /// The signal that ended it (Linux and macOS), if one did.
    pub signal: Option<i32>,
    /// Its process id and when it started: the system's records and the
    /// logs of this run are looked up by them.
    pub pid: u32,
    pub started_at: std::time::SystemTime,
    pub secs: f32,
    pub panicked: bool,
    /// The game's window could not open ([`NO_WINDOW`]).
    pub no_window: bool,
    /// The game's `error.kind=` line, if it printed one.
    pub named: Option<ErrorKind>,
    /// The last lines it printed (stdout and stderr, in order of reading).
    pub tail: Vec<String>,
    /// The launcher's log file, if one could be written.
    pub log: Option<PathBuf>,
}

impl Ended {
    pub fn failed(&self) -> bool {
        self.panicked || self.no_window || self.code != Some(0)
    }

    /// What went wrong: the game's own `error.kind=` line, else a panic
    /// line (a crash) or the window line; [`ErrorKind::Other`] otherwise.
    pub fn kind(&self) -> ErrorKind {
        match self.named {
            Some(k) => k,
            None if self.panicked => ErrorKind::Crash,
            None if self.no_window => ErrorKind::NoWindow,
            None => ErrorKind::Other,
        }
    }

    /// A failure within [`START_SECS`].
    pub fn during_start(&self) -> bool {
        self.failed() && self.secs < START_SECS
    }

    /// The end in plain words (`launcher::crash_report::reason`).
    pub fn reason(&self) -> String {
        crate::launcher::crash_report::reason(self.code, self.signal, self.panicked)
    }

    /// "exit code 101", "a crash (panic), exit code 101", "ended by the
    /// system".
    pub fn how(&self) -> String {
        let code = match (self.code, self.signal) {
            (Some(c), _) => format!("exit code {c}"),
            (None, Some(s)) => format!("signal {s}"),
            (None, None) => "ended by the system".into(),
        };
        if self.panicked {
            format!("a crash (panic), {code}")
        } else if self.no_window {
            format!("no game window, {code}")
        } else {
            code
        }
    }
}

/// The launcher's log files to delete from a folder holding `names` so
/// that, with the new one, [`KEEP`] remain (the game's `game-*.log` are
/// its own business).
pub fn to_prune(names: &[String]) -> Vec<String> {
    let mut logs: Vec<&String> = names.iter().filter(|n| n.starts_with("launcher-") && n.ends_with(".log")).collect();
    logs.sort();
    let excess = logs.len().saturating_sub(KEEP - 1);
    logs.into_iter().take(excess).cloned().collect()
}

fn open_log(dir: &Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    std::fs::create_dir_all(dir)?;
    let names: Vec<String> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect();
    for old in to_prune(&names) {
        let _ = std::fs::remove_file(dir.join(old));
    }
    let stamp = crate::qol::saves::timestamp(std::time::SystemTime::now());
    for n in 0..100 {
        let name = if n == 0 { format!("launcher-{stamp}.log") } else { format!("launcher-{stamp}-{n}.log") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free log file name"))
}

/// Whether the game writes its own log file for these arguments.
pub fn game_logs_itself(args: &[String]) -> bool {
    crate::qol::log_file::wanted(args, std::env::var("NFSU2_LOG_FILE").ok().as_deref())
}

/// Starts `exe` with `args`, its output read on two threads; the log in
/// `log_dir` (None: no file). `root`: the game folder the launcher shows,
/// given to the game as `NFSU2_ROOT` so it plays the folder the player
/// chose even when another one is set in the environment.
pub fn start(exe: &Path, args: &[String], log_dir: Option<&Path>, root: Option<&Path>) -> Result<Running, String> {
    start_env(exe, args, log_dir, root, &[])
}

/// [`start`] with more environment variables for the game (a trial run's).
pub fn start_env(
    exe: &Path,
    args: &[String],
    log_dir: Option<&Path>,
    root: Option<&Path>,
    env: &[(String, String)],
) -> Result<Running, String> {
    start_marked(exe, args, log_dir, root, env, super::dirs::start_marker())
}

/// [`start`] with the start marker at `marker` (the tests keep it in their
/// own folder: the default one is in the player's data folder).
fn start_marked(
    exe: &Path,
    args: &[String],
    log_dir: Option<&Path>,
    root: Option<&Path>,
    env: &[(String, String)],
    marker: PathBuf,
) -> Result<Running, String> {
    let mut shared = Shared { file_all: !game_logs_itself(args), ..Default::default() };
    let mut log = None;
    if let Some(dir) = log_dir {
        match open_log(dir) {
            Ok((path, f)) => {
                let mut w = std::io::BufWriter::new(f);
                let _ = writeln!(
                    w,
                    "Overbright launcher {} log, started {} UTC",
                    crate::build_info::VERSION,
                    crate::qol::saves::timestamp(std::time::SystemTime::now())
                );
                let _ = writeln!(
                    w,
                    "starting: {} {}",
                    crate::qol::crash::scrub(&exe.display().to_string()),
                    args.join(" ")
                );
                if !shared.file_all {
                    let _ = writeln!(
                        w,
                        "the game writes its own log (game-*.log in this folder); its output is added here only after a failure"
                    );
                }
                let _ = w.flush();
                shared.file = Some(w);
                log = Some(path);
            }
            Err(e) => log::warn!("launcher log not written: {e}"),
        }
    }
    let mut cmd = Command::new(exe);
    if let Some(r) = root {
        cmd.env("NFSU2_ROOT", r);
    }
    cmd.envs(env.iter().map(|(k, v)| (k, v)));
    // Windows: no console window for a console-subsystem game (a check
    // build); the shipping game is a window program anyway.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("the game could not be started ({}): {e}", exe.display()))?;
    // The start marker: the game is starting. It goes once the start is
    // past (START_SECS) or the game ends; one left over means the
    // launcher and the game went down together during a start.
    if std::fs::write(&marker, format!("pid {}\n", child.id())).is_ok() {
        let (m, pid) = (marker.clone(), child.id());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f32(START_SECS));
            clear_marker_of(&m, pid);
        });
    }
    let shared = Arc::new(Mutex::new(shared));
    let mut readers = Vec::new();
    let out = child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>);
    let err = child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>);
    for (name, pipe) in [("out", out), ("err", err)] {
        let Some(pipe) = pipe else { continue };
        let sh = shared.clone();
        readers.push(std::thread::spawn(move || {
            let mut r = std::io::BufReader::new(pipe);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf).trim_end_matches(['\n', '\r']).to_string();
                        if let Ok(mut s) = sh.lock() {
                            s.line(name, line);
                        }
                    }
                }
            }
        }));
    }
    let cap = mem_cap_bytes(std::env::var(MEM_CAP_ENV).ok().as_deref()).map(|c| (c, Instant::now()));
    Ok(Running { child, started: Instant::now(), started_at: std::time::SystemTime::now(), shared, readers, marker, log, cap })
}

impl Running {
    /// The end, once the game has ended (None while it runs).
    pub fn poll(&mut self) -> Option<Ended> {
        self.stop_over_cap();
        match self.child.try_wait() {
            Ok(Some(status)) => Some(self.finish(Some(status))),
            Ok(None) => None,
            Err(_) => Some(self.finish(None)),
        }
    }

    /// With a cap set, look at the game's memory about once a second and stop the game when it is over; the
    /// launcher's log and the game's tail say why.
    fn stop_over_cap(&mut self) {
        let Some((cap, at)) = self.cap.as_mut() else { return };
        if at.elapsed() < Duration::from_secs(1) {
            return;
        }
        *at = Instant::now();
        let cap = *cap;
        let Some(used) = resident_bytes(&self.child).filter(|&u| u > cap) else { return };
        let note = format!("the launcher stopped the game: it used {} MiB, over the cap of {} MiB", used >> 20, cap >> 20);
        log::error!("{note}");
        if let Ok(mut s) = self.shared.lock() {
            s.line("err", note);
        }
        let _ = self.child.kill();
        self.cap = None;
    }

    /// Waits for the end.
    pub fn wait(mut self) -> Ended {
        let status = self.child.wait().ok();
        self.finish(status)
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    fn finish(&mut self, status: Option<std::process::ExitStatus>) -> Ended {
        let code = status.and_then(|s| s.code());
        #[cfg(unix)]
        let signal = status.and_then(|s| std::os::unix::process::ExitStatusExt::signal(&s));
        #[cfg(not(unix))]
        let signal = None;
        clear_marker_of(&self.marker, self.child.id());
        let secs = self.started.elapsed().as_secs_f32();
        // The readers end at the pipes' end (a child the game started could
        // keep them open: wait at most a second).
        let until = Instant::now() + Duration::from_secs(1);
        for r in std::mem::take(&mut self.readers) {
            while !r.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            if r.is_finished() {
                let _ = r.join();
            }
        }
        let mut sh = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let tail: Vec<String> = sh.tail.iter().cloned().collect();
        let e = Ended {
            code,
            signal,
            pid: self.child.id(),
            started_at: self.started_at,
            secs,
            panicked: sh.panicked,
            no_window: sh.no_window,
            named: sh.kind,
            tail,
            log: self.log.clone(),
        };
        let file_all = sh.file_all;
        if let Some(f) = sh.file.as_mut() {
            if e.failed() && !file_all {
                let _ = writeln!(f, "the game's last output lines:");
                for l in &e.tail {
                    let _ = writeln!(f, "  {}", crate::qol::crash::scrub(l));
                }
            }
            let verdict = if e.during_start() {
                "a failed start"
            } else if e.failed() {
                "a failure"
            } else {
                "a normal end"
            };
            let _ = writeln!(f, "the game ended after {secs:.1} s: {} ({verdict})", e.how());
            let _ = f.flush();
        }
        e
    }
}

/// One launcher at a time: the lock file held while the launcher runs
/// (released when the process ends, however it ends). None when another
/// launcher holds it.
pub fn single_instance(path: &Path) -> Result<Option<std::fs::File>, String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(format!("{}: {e}", path.display())),
    }
}

/// Opens `dir` in the system's file manager (Explorer, Finder,
/// `xdg-open`).
pub fn open_folder(dir: &Path) -> Result<(), String> {
    let _ = std::fs::create_dir_all(dir);
    let prog = if cfg!(windows) {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(prog)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{prog}: {e}"))
}

/// Removes the start marker if it is the one of the game `pid`.
fn clear_marker_of(marker: &Path, pid: u32) {
    if std::fs::read_to_string(marker).is_ok_and(|t| t.trim() == format!("pid {pid}")) {
        let _ = std::fs::remove_file(marker);
    }
}

/// Whether the last start did not finish (its marker is still there).
pub fn start_unfinished() -> bool {
    super::dirs::start_marker().is_file()
}

/// Forgets an unfinished start (the player chose what to do about it).
pub fn clear_start_marker() {
    let _ = std::fs::remove_file(super::dirs::start_marker());
}

/// Whether a Shift or Ctrl key is held now (the skip rule's way back to
/// the launcher). Windows: GetAsyncKeyState. X11 (and XWayland): the server's
/// key map (XQueryKeymap, libX11 loaded at run time). Wayland gives a
/// program without a focused window no keyboard state, so there Shift is
/// seen only through XWayland when DISPLAY is set; `--launcher` (or the
/// pad's Back / Select button) always works.
pub fn shift_held() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetAsyncKeyState(key: i32) -> i16;
        }
        const VK_SHIFT: i32 = 0x10;
        const VK_CONTROL: i32 = 0x11;
        // SAFETY: plain documented calls.
        [VK_SHIFT, VK_CONTROL].into_iter().any(|k| (unsafe { GetAsyncKeyState(k) } as u16 & 0x8000) != 0)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        x11_shift_held()
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    false
}

#[cfg(all(unix, not(target_os = "macos")))]
fn x11_shift_held() -> bool {
    if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
        return false;
    }
    let Ok(x) = x11_dl::xlib::Xlib::open() else { return false };
    // SAFETY: Xlib calls on a display this function opens and closes.
    unsafe {
        let d = (x.XOpenDisplay)(std::ptr::null());
        if d.is_null() {
            return false;
        }
        let mut keys = [0 as std::os::raw::c_char; 32];
        (x.XQueryKeymap)(d, keys.as_mut_ptr());
        let mut held = false;
        for sym in [
            x11_dl::keysym::XK_Shift_L,
            x11_dl::keysym::XK_Shift_R,
            x11_dl::keysym::XK_Control_L,
            x11_dl::keysym::XK_Control_R,
        ] {
            let kc = (x.XKeysymToKeycode)(d, sym as std::os::raw::c_ulong) as usize;
            if kc != 0 && (keys[kc / 8] as u8) & (1 << (kc % 8)) != 0 {
                held = true;
            }
        }
        (x.XCloseDisplay)(d);
        held
    }
}

/// Whether a pad's Back / Select button is held now (gilrs; the pads
/// report their state after a short poll).
pub fn pad_back_held() -> bool {
    if std::env::var_os("NFSU2_NO_PADS").is_some() {
        return false;
    }
    let Ok(mut g) = gilrs::Gilrs::new() else { return false };
    let until = Instant::now() + Duration::from_millis(60);
    while Instant::now() < until {
        while g.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(10));
    }
    g.gamepads().any(|(_, p)| p.is_pressed(gilrs::Button::Select))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_memory_cap_is_opt_in_and_in_range() {
        assert_eq!(mem_cap_bytes(None), None, "unset: no cap");
        assert_eq!(mem_cap_bytes(Some("8192")), Some(8192 << 20));
        assert_eq!(mem_cap_bytes(Some(" 256 ")), Some(256 << 20));
        for bad in ["", "0", "255", "1048577", "-1", "8 GB", "x"] {
            assert_eq!(mem_cap_bytes(Some(bad)), None, "{bad}");
        }
    }

    /// A game over its cap is stopped by the launcher (a stub that sleeps; the cap is set below what any process
    /// uses); without a cap it is left alone.
    #[cfg(unix)]
    #[test]
    fn a_game_over_the_memory_cap_is_stopped() {
        let dir = std::env::temp_dir().join(format!("ob-run-cap-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("stub-game");
        crate::testkit::write_script(&stub, "#!/bin/sh\nsleep 30\n");
        let past = Instant::now() - Duration::from_secs(2);
        let mut r = start_marked(&stub, &[], None, None, &[], dir.join("start-marker.txt")).unwrap();
        r.cap = None;
        assert!(r.poll().is_none(), "no cap: left alone");
        r.cap = Some((64 << 10, past));
        let until = Instant::now() + Duration::from_secs(10);
        let ended = loop {
            if let Some(e) = r.poll() {
                break e;
            }
            assert!(Instant::now() < until, "not stopped");
            if let Some(c) = r.cap.as_mut() {
                c.1 = past;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(ended.failed(), "{ended:?}");
        assert!(ended.tail.iter().any(|l| l.contains("over the cap")), "{:?}", ended.tail);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stub game (a shell script) that prints a log and a panic line and
    /// exits 101: a failed start, the tail and the log file hold its lines.
    #[test]
    fn a_failed_start_is_seen_with_its_tail() {
        let dir = std::env::temp_dir().join(format!("ob-run-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("stub-game");
        crate::testkit::write_script(
            &stub,
            "#!/bin/sh\necho \"stub: starting with $*\"\necho 'GPU adapter: stub' >&2\n\
             echo \"thread 'main' panicked at src/stub.rs:1:1:\" >&2\necho 'no GPU adapter' >&2\nexit 101\n",
        );
        let logs = dir.join("logs");
        // Never the player's data folder: every start here keeps its marker
        // in the test's own folder.
        let marker = dir.join("start-marker.txt");
        let start = |exe: &Path, args: &[String], logs: Option<&Path>, root: Option<&Path>| {
            start_marked(exe, args, logs, root, &[], marker.clone())
        };
        let r = start(&stub, &["--windowed".into()], Some(&logs), None).unwrap();
        assert!(marker.is_file(), "the start writes its marker where it is told");
        let e = r.wait();
        assert_eq!(e.code, Some(101));
        assert!(!marker.exists(), "the start's end removes its marker");
        assert!(e.panicked && e.failed() && e.during_start(), "{e:?}");
        assert!(e.tail.iter().any(|l| l == "no GPU adapter") && e.tail.iter().any(|l| l.contains("--windowed")));
        let text = std::fs::read_to_string(e.log.as_ref().unwrap()).unwrap();
        assert!(
            text.contains("[err] no GPU adapter") && text.contains("exit code 101") && text.contains("a failed start"),
            "{text}"
        );
        assert!(e.how().contains("panic"));
        // A plain run: the game logs itself; the launcher's file has the
        // tail only after a failure.
        let r = start(&stub, &[], Some(&logs), None).unwrap();
        let e = r.wait();
        let text = std::fs::read_to_string(e.log.as_ref().unwrap()).unwrap();
        assert!(
            !text.contains("[err]") && text.contains("last output lines") && text.contains("  no GPU adapter"),
            "{text}"
        );
        // Exit 0: no failure.
        std::fs::write(&stub, "#!/bin/sh\necho fine\nexit 0\n").unwrap();
        let e = start(&stub, &[], None, None).unwrap().wait();
        assert!(!e.failed() && e.log.is_none());
        // Exit 0 without a window (the game's own line): a failed start.
        std::fs::write(&stub, "#!/bin/sh\necho 'could not open a window: no GPU adapter' >&2\nexit 0\n").unwrap();
        let e = start(&stub, &[], None, None).unwrap().wait();
        assert!(e.failed() && e.during_start() && e.how().starts_with("no game window"), "{e:?}");
        // The chosen game folder reaches the game as NFSU2_ROOT.
        std::fs::write(&stub, "#!/bin/sh\necho \"root=$NFSU2_ROOT\"\nexit 0\n").unwrap();
        let e = start(&stub, &[], None, Some(Path::new("/games/nfsu2"))).unwrap().wait();
        assert!(e.tail.iter().any(|l| l == "root=/games/nfsu2"), "{e:?}");
        // A missing program.
        assert!(start(&dir.join("nope"), &[], None, None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A game killed by a signal: no exit code, the signal and the words for
    /// it, a failure; a clean exit is none.
    #[test]
    fn a_signal_kill_is_told_in_words() {
        let dir = std::env::temp_dir().join(format!("ob-run-sig-{}", crate::unique_tag()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("stub-game");
        crate::testkit::write_script(&stub, "#!/bin/sh\necho 'stub: about to crash' >&2\nkill -SEGV $$\nsleep 5\n");
        let marker = dir.join("start-marker.txt");
        let e = start_marked(&stub, &[], None, None, &[], marker.clone()).unwrap().wait();
        assert_eq!((e.code, e.signal), (None, Some(11)), "{e:?}");
        assert!(e.failed() && e.pid > 0);
        assert!(e.reason().contains("segmentation fault"), "{}", e.reason());
        assert_eq!(e.how(), "signal 11");
        assert!(e.tail.iter().any(|l| l.contains("about to crash")));
        crate::testkit::write_script(&stub, "#!/bin/sh\nexit 0\n");
        let e = start_marked(&stub, &[], None, None, &[], marker).unwrap().wait();
        assert!(!e.failed() && e.signal.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launcher_logs_pruned_and_one_instance() {
        let names: Vec<String> =
            ["launcher-1.log", "launcher-2.log", "game-1.log", "launcher-3.log", "launcher-4.log", "launcher-5.log"]
                .map(String::from)
                .to_vec();
        assert_eq!(to_prune(&names), ["launcher-1.log"], "four old ones and the new one");
        let p = std::env::temp_dir().join(format!("ob-lock-{}/launcher.lock", crate::unique_tag()));
        let a = single_instance(&p).unwrap();
        assert!(a.is_some());
        assert!(single_instance(&p).unwrap().is_none(), "a second launcher is refused");
        drop(a);
        // Free again. Another test's thread may have forked a child between
        // the lock being taken and released: until its exec the child holds
        // a copy of the lock's descriptor, so give that a moment.
        let free = (0..200).any(|_| {
            single_instance(&p).unwrap().is_some() || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                false
            }
        });
        assert!(free, "free again");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
