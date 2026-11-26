//! The launcher's crash report window (docs/troubleshooting.md): what a
//! bad end of the game is told in plain words, the system's recent errors
//! for that process, the log files written during the run, and the text
//! for a bug report. The text is made with the support file's redaction
//! ([`super::support::scrub`]: home folders, user names, hardware serials,
//! device ids) and is only ever copied to the clipboard or saved by the
//! player: nothing is sent anywhere.

use crate::diag::support::{self, scrub};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How long one question to a system tool may take.
const ASK_LIMIT: Duration = Duration::from_secs(5);
/// One log's most bytes in the report (its start and end are kept).
const LOG_MAX: usize = 24 * 1024;
/// Lines of system errors kept.
const ERRORS_MAX: usize = 12;

/// How a program ended, in plain words (a clause after "it": no capital, no
/// full stop).
pub fn reason(code: Option<i32>, signal: Option<i32>, panicked: bool) -> String {
    if let Some(sig) = signal {
        let what = match sig {
            11 => "it tried to use memory it must not (a segmentation fault, signal 11)".to_string(),
            6 => "it stopped itself after a fatal error (an abort, signal 6)".into(),
            4 => "it ran an instruction this processor does not have (signal 4)".into(),
            7 => "it read memory that was not there (a bus error, signal 7)".into(),
            8 => "it did an impossible calculation (signal 8)".into(),
            9 => "it was killed from outside, often because the computer ran out of memory (signal 9)".into(),
            15 => "it was asked to stop by the system (signal 15)".into(),
            2 => "it was interrupted (signal 2)".into(),
            1 => "its terminal closed (signal 1)".into(),
            n => format!("it was ended by the system (signal {n})"),
        };
        return what;
    }
    match code {
        Some(c) => {
            let u = c as u32;
            if let Some(n) = crate::diag::native_crash::status_name(u) {
                format!("it crashed with {n} (Windows code 0x{u:08X})")
            } else if u >= 0xC000_0000 {
                format!("it crashed with the Windows code 0x{u:08X}")
            } else if panicked {
                format!("it hit an internal error (a panic) and stopped, exit code {c}")
            } else if c == 0 {
                "it ended with exit code 0 but did not do its job".into()
            } else {
                format!("it stopped with exit code {c}")
            }
        }
        None if panicked => "it hit an internal error (a panic) and stopped".into(),
        None => "it ended and the system gave no reason".into(),
    }
}

/// One line of a command's output when it answered in time; nothing when
/// the tool is missing, was refused or hung (no admin rights needed or
/// asked for).
fn answer(mut cmd: std::process::Command, what: &str) -> Vec<String> {
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
    support::no_window(&mut cmd);
    let t = support::run_limited(cmd, ASK_LIMIT, what);
    let mut lines = t.lines();
    match lines.next() {
        Some(h) if h.contains("ended normally") => {}
        _ => return Vec::new(),
    }
    lines
        .take_while(|l| !l.starts_with("(error output)"))
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// The system's own record of a program's bad end, when it can be read
/// without admin rights: on Linux the coredump list and the journal lines
/// of that process; on Windows the Application event log's crash entries
/// for the program in the last few minutes. Empty when there is nothing or
/// it is not readable.
pub fn system_errors(pid: u32, exe_name: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    #[cfg(unix)]
    {
        let pid = pid.to_string();
        let mut c = std::process::Command::new("coredumpctl");
        c.args(["list", "--no-pager", "--no-legend", "-q", &pid]);
        v.extend(answer(c, "coredumpctl"));
        let mut c = std::process::Command::new("journalctl");
        c.args(["--no-pager", "-q", "-n", "10", &format!("_PID={pid}")]);
        v.extend(answer(c, "journalctl"));
    }
    #[cfg(windows)]
    {
        let _ = pid;
        let mut c = std::process::Command::new("wevtutil");
        c.args([
            "qe",
            "Application",
            "/q:*[System[(EventID=1000 or EventID=1001 or EventID=1002) and TimeCreated[timediff(@SystemTime) <= 600000]]]",
            "/c:30",
            "/rd:true",
            "/f:text",
        ]);
        let all = answer(c, "wevtutil").join("\n");
        let want = exe_name.to_ascii_lowercase();
        for e in support::ours_in_events(&all).into_iter().filter(|e| e.to_ascii_lowercase().contains(&want)) {
            v.extend(e.lines().filter(|l| !l.trim().is_empty()).take(8).map(str::to_string));
        }
    }
    #[cfg(not(any(unix, windows)))]
    let _ = (pid, exe_name);
    v.truncate(ERRORS_MAX);
    v.into_iter().map(|l| scrub(&l)).collect()
}

/// Overbright's own log files written since `since` (a little earlier,
/// for clocks that round): the launcher's log, the per-start and game
/// logs, crash reports. Newest last.
pub fn run_logs(since: SystemTime) -> Vec<PathBuf> {
    use crate::platform::dirs;
    let from = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);
    let mut found: Vec<(SystemTime, PathBuf)> = Vec::new();
    let mut add = |p: PathBuf| {
        if let Ok(t) = std::fs::metadata(&p)
            .and_then(|m| if m.is_file() { m.modified() } else { Err(std::io::Error::other("dir")) })
            && t >= from
        {
            found.push((t, p));
        }
    };
    add(dirs::config().join("launcher.log"));
    for d in [dirs::logs(), dirs::crashes()] {
        if let Ok(r) = std::fs::read_dir(&d) {
            for e in r.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if (n.ends_with(".log") || n.ends_with(".txt")) && !n.starts_with(support::PREFIX) {
                    add(e.path());
                }
            }
        }
    }
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// What the report is made of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// "the game".
    pub program: String,
    /// Facts about the program to print under the first line (its version, the mod that was open).
    pub extra: Vec<String>,
    pub reason: String,
    pub secs: f32,
    pub during_start: bool,
    /// The program's last output lines.
    pub tail: Vec<String>,
    pub errors: Vec<String>,
    pub logs: Vec<PathBuf>,
}

/// The text for a bug report: the reason, the system, the system's errors,
/// the program's last lines and the run's logs; redacted. No saves, no
/// game files, no environment.
pub fn report(f: &Facts) -> String {
    let mut s = format!(
        "Overbright crash report\n{}\n\nwhat happened: {} ended after {:.1} s{}: {}\n",
        crate::build_info::version_line(),
        f.program,
        f.secs,
        if f.during_start { " (while starting)" } else { "" },
        f.reason,
    );
    if !f.extra.is_empty() {
        s += &format!("{}\n", f.extra.join("\n"));
    }
    s += &format!("\n===== system =====\n{}\n", support::system());
    s += "\n===== system errors =====\n";
    if f.errors.is_empty() {
        s += "(none found, or not readable without admin rights)\n";
    } else {
        s += &f.errors.join("\n");
        s.push('\n');
    }
    s += &format!("\n===== {}'s last lines =====\n", f.program);
    if f.tail.is_empty() {
        s += "(it printed nothing)\n";
    } else {
        s += &f.tail.join("\n");
        s.push('\n');
    }
    for p in &f.logs {
        s += &format!("\n===== log: {} =====\n", p.display());
        match std::fs::read(p) {
            Ok(b) => {
                let t = support::tail_text(&String::from_utf8_lossy(&b), LOG_MAX);
                s += &t;
                if !t.ends_with('\n') {
                    s.push('\n');
                }
            }
            Err(e) => s += &format!("({e})\n"),
        }
    }
    s += "\nThis text holds no saves and no game files; home folders are shown as ~. It was not sent anywhere.\n";
    scrub(&s)
}

/// Puts `text` on the clipboard through the system's own tool (Windows
/// `clip`, macOS `pbcopy`, Wayland `wl-copy`, X11 `xclip` / `xsel`). Err
/// with the words when none could be used.
pub fn copy_text(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tools: Vec<(&str, &[&str])> = if cfg!(windows) {
        vec![("clip", &[][..])]
    } else if cfg!(target_os = "macos") {
        vec![("pbcopy", &[][..])]
    } else {
        vec![
            ("wl-copy", &[][..]),
            ("xclip", &["-selection", "clipboard"][..]),
            ("xsel", &["--clipboard", "--input"][..]),
        ]
    };
    let bytes: Vec<u8> = if cfg!(windows) {
        // clip.exe reads UTF-16 with a byte order mark as it is.
        std::iter::once(0xFEFFu16).chain(text.encode_utf16()).flat_map(u16::to_le_bytes).collect()
    } else {
        text.as_bytes().to_vec()
    };
    for (tool, args) in tools {
        let mut cmd = Command::new(tool);
        cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        support::no_window(&mut cmd);
        let Ok(mut child) = cmd.spawn() else { continue };
        let wrote = child.stdin.take().is_some_and(|mut i| i.write_all(&bytes).is_ok());
        let t0 = std::time::Instant::now();
        // xclip and wl-copy stay behind to serve the clipboard: a tool that
        // is still running after a moment has taken the text.
        let ok = loop {
            match child.try_wait() {
                Ok(Some(st)) => break wrote && st.success(),
                Ok(None) if t0.elapsed() < Duration::from_millis(1500) => std::thread::sleep(Duration::from_millis(20)),
                Ok(None) => break wrote,
                Err(_) => break false,
            }
        };
        if ok {
            return Ok(());
        }
    }
    Err("no clipboard tool answered".into())
}

/// The report saved as a file in `dir` when the clipboard cannot be used;
/// its path.
pub fn save_text(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join(format!("overbright-crash-report-{}.txt", crate::qol::saves::timestamp(SystemTime::now())));
    std::fs::write(&p, text)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exits_are_told_in_words() {
        assert!(reason(None, Some(11), false).contains("segmentation fault"));
        assert!(reason(None, Some(9), false).contains("killed") && reason(None, Some(9), false).contains("memory"));
        assert!(reason(Some(101), None, true).contains("panic"));
        assert!(reason(Some(0xC000_0005u32 as i32), None, false).contains("access violation"));
        assert!(reason(Some(0xC123_4567u32 as i32), None, false).contains("0xC1234567"));
        assert!(reason(Some(3), None, false).contains("exit code 3"));
        assert!(reason(None, Some(77), false).contains("signal 77"));
    }

    /// The report holds no home folder, user name, serial, MAC address or
    /// device id, in the text, the lines, the errors and the logs.
    #[test]
    fn the_report_is_redacted() {
        let dir = std::env::temp_dir().join(format!("ob-crash-report-{}", crate::unique_tag()));
        std::fs::create_dir_all(&dir).unwrap();
        let home = format!("/{}/{}", "home", "janedoe");
        let log = dir.join("game-1.log");
        std::fs::write(&log, format!("opened {home}/.local/share/overbright/x\nSerial Number: ABC123XYZ\nadapter uuid 123e4567-e89b-12d3-a456-426614174000\n")).unwrap();
        let f = Facts {
            program: "the game".into(),
            reason: reason(None, Some(11), false),
            secs: 12.5,
            tail: vec![format!("reading {home}/Games/NFSU2"), "net card 00:1A:2B:3C:4D:5E".into()],
            errors: vec![format!("coredump {home}/core"), "C:\\Users\\Jane Doe\\AppData\\x".into()],
            logs: vec![log],
            during_start: false,
            extra: Vec::new(),
        };
        let t = report(&f);
        for bad in ["janedoe", "Jane Doe", "ABC123XYZ", "123e4567", "00:1A:2B", "4D:5E"] {
            assert!(!t.contains(bad), "{bad} left in:\n{t}");
        }
        for want in ["segmentation fault", "<user>", "===== system errors =====", "log: ", "not sent anywhere"] {
            assert!(t.contains(want), "{want} missing:\n{t}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_logs_of_the_run_are_listed() {
        let dir = std::env::temp_dir().join(format!("ob-crash-logs-{}", crate::unique_tag()));
        crate::platform::dirs::set_thread_data_home(dir.clone());
        let logs = crate::platform::dirs::logs();
        std::fs::create_dir_all(&logs).unwrap();
        let old = logs.join("game-old.log");
        std::fs::write(&old, "old").unwrap();
        let t = std::fs::File::options().write(true).open(&old).unwrap();
        t.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
        let start = SystemTime::now();
        std::fs::write(logs.join("game-new.log"), "new").unwrap();
        std::fs::write(logs.join(format!("{}x.txt", support::PREFIX)), "support").unwrap();
        let got = run_logs(start);
        let names: Vec<String> = got.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["game-new.log"], "{names:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
