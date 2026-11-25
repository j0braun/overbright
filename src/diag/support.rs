//! The support file (docs/troubleshooting.md): one plain text file a player
//! attaches to a bug report, made in one step — the launcher's "Save support
//! file", `overbright --support-file`, `game --support-file`, or the game's
//! Ctrl+F8. It holds:
//!
//! * what happened (the caller's words: a failure screen, a crash) and who
//!   wrote the file;
//! * the version, the system (OS and version, session, memory, processors)
//!   and Overbright's folders, portable mode and the environment variables
//!   that change Overbright, the graphics driver or Vulkan;
//! * the game folder's check: the files found, their sizes and SHA-256,
//!   the edition verdict (`game --check-game`'s lines);
//! * the graphics adapters and the one the game picks, asked from the game
//!   program in a process of its own with a time limit (`game
//!   --list-adapters`, `game --probe`), so a driver that crashes or hangs
//!   while listing shows as that, not as a lost support file;
//! * on Windows, the Application event log's recent crash entries for
//!   Overbright's programs (the faulting module and exception code of a
//!   start that ended at once, without a log line);
//! * the settings files, and the logs of the last starts: the launcher's
//!   (`launcher.log`, `launcher.old.log`, `logs/launcher-*.log`), the
//!   game's (`logs/game-*.log`), crash reports, and in the game the recent
//!   events ([`super::events`]).
//!
//! Home folders are shown as `~` and user names in paths as `<user>`. The
//! file is written to the logs folder (or the folder asked for; when that
//! cannot be written, the program's folder, then the temporary folder);
//! the newest [`KEEP`] support files are kept. Nothing is sent anywhere.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Support files kept in a folder, the new one included.
pub const KEEP: usize = 5;
/// The files' name before the UTC time.
pub const PREFIX: &str = "overbright-support-";
/// The command-line flag of both programs.
pub const FLAG: &str = "--support-file";
/// One log's most bytes in the file (the start and the end are kept).
const LOG_MAX: u64 = 192 * 1024;
/// How long a question to the game program may take.
const CHILD_LIMIT: Duration = Duration::from_secs(20);

/// Who writes the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Origin {
    /// The launcher's window (its error and crash screens, About).
    #[default]
    Launcher,
    /// The game while it runs (Ctrl+F8).
    Game,
    /// `overbright --support-file` or `game --support-file`.
    Command,
}

/// What to put in the file and where.
#[derive(Clone, Debug, Default)]
pub struct SupportRequest {
    pub origin: Origin,
    /// What happened, in the program's words (a failure screen's text, a
    /// panic's message); None for a file made on request.
    pub reason: Option<String>,
    /// The folder to write to; None: the logs folder.
    pub out_dir: Option<PathBuf>,
    /// The game folder to check; None: the one Overbright finds
    /// (`qol::setup::find`).
    pub game_root: Option<PathBuf>,
    /// Ask the game program for the adapters and its hardware check (a
    /// child process each, at most 20 s). Off in the running game, which
    /// writes the adapter it uses instead.
    pub ask_game: bool,
    /// The game program to ask; None: `game` beside the launcher for
    /// [`Origin::Launcher`], else this program.
    pub game_exe: Option<PathBuf>,
    /// More sections from the caller: (title, text), e.g. the launcher's
    /// failure details and the game's last output lines.
    pub extra: Vec<(String, String)>,
}

impl SupportRequest {
    /// The launcher's request: the reason (a failure screen's text, or
    /// None), the game asked.
    pub fn launcher(reason: Option<String>) -> SupportRequest {
        SupportRequest { origin: Origin::Launcher, reason, ask_game: true, ..Default::default() }
    }
}

/// Writes the support file; its path. Errors only when no folder at all
/// could be written.
pub fn write_support_file(req: &SupportRequest) -> io::Result<PathBuf> {
    let text = build(req);
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.extend(req.out_dir.clone());
    if req.out_dir.is_none() {
        dirs.push(crate::platform::dirs::logs());
    }
    dirs.extend(crate::platform::dirs::program_dir());
    dirs.push(std::env::temp_dir());
    let stamp = crate::qol::saves::timestamp(std::time::SystemTime::now());
    let mut last = io::Error::other("no folder to write to");
    for (n, dir) in dirs.iter().enumerate() {
        let place =
            if n == 0 { "the folder asked for" } else { "a fallback: the folders before it could not be written" };
        let text = format!("{text}\n===== written to =====\n{} ({place})\n", scrub(&dir.display().to_string()));
        match write_in(dir, &stamp, &text) {
            Ok(p) => return Ok(p),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn write_in(dir: &Path, stamp: &str, text: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let names: Vec<String> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect();
    for old in to_prune(&names) {
        let _ = std::fs::remove_file(dir.join(old));
    }
    for n in 0..100 {
        let name = if n == 0 { format!("{PREFIX}{stamp}.txt") } else { format!("{PREFIX}{stamp}-{n}.txt") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                io::Write::write_all(&mut f, text.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("no free file name"))
}

/// The older support files to delete so that, with the new one, [`KEEP`]
/// remain.
pub fn to_prune(names: &[String]) -> Vec<String> {
    let mut v: Vec<&String> = names.iter().filter(|n| n.starts_with(PREFIX) && n.ends_with(".txt")).collect();
    v.sort();
    let excess = v.len().saturating_sub(KEEP - 1);
    v.into_iter().take(excess).cloned().collect()
}

/// The file's text, scrubbed.
pub fn build(req: &SupportRequest) -> String {
    let mut s = String::new();
    let mut section = |title: &str, body: &str| {
        s += &format!("\n===== {title} =====\n");
        s += body;
        if !body.ends_with('\n') {
            s.push('\n');
        }
    };
    let head = format!(
        "Overbright support file\n{}\nwritten (UTC): {}\nby: {}\nwhat happened: {}\n\nAttach this file to a bug report. It holds no saves and no game files; home folders are shown as ~.\n",
        crate::build_info::version_line(),
        crate::qol::saves::timestamp(std::time::SystemTime::now()),
        match req.origin {
            Origin::Launcher => "the launcher",
            Origin::Game => "the game (Ctrl+F8)",
            Origin::Command => "the --support-file command",
        },
        req.reason.as_deref().unwrap_or("(nothing given: made on request)"),
    );
    section("summary", &head);
    section("system", &system());
    section("Overbright's folders", &folders());
    section("environment", &environment());
    section("game folder", &game_folder(req.game_root.as_deref()));
    if req.ask_game {
        let exe = req.game_exe.clone().unwrap_or_else(|| default_game_exe(req.origin));
        section("graphics adapters (game --list-adapters)", &ask(&exe, &["--list-adapters"]));
        section("hardware check (game --probe)", &ask(&exe, &["--probe"]));
    } else {
        section("graphics adapter in use", &crate::qol::crash::adapter().unwrap_or_else(|| "(not opened yet)".into()));
        section("audio output in use", &crate::qol::crash::audio_output().unwrap_or_else(|| "(none opened)".into()));
    }
    if cfg!(windows) {
        section("Windows crash events for Overbright", &windows_events());
    }
    let events = super::events::text();
    if !events.starts_with("0 events") {
        section("recent events", &events);
    }
    for (title, body) in &req.extra {
        section(title, body);
    }
    section("settings files", &settings());
    section("logs", &logs());
    scrub(&s)
}

/// Home folders as `~` (`qol::crash::scrub`), then any user name left in a
/// path as `<user>`.
pub fn scrub(s: &str) -> String {
    scrub_ids(&scrub_users(&crate::qol::crash::scrub(s)))
}

/// Hardware identifiers made unreadable: the value of any `serial` line,
/// MAC addresses and UUIDs / GUIDs (device and pipeline-cache ids).
pub fn scrub_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (n, line) in s.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let line = if line.to_ascii_lowercase().contains("serial")
            && let Some(i) = line.find([':', '='])
        {
            format!("{} <serial>", &line[..=i])
        } else {
            line.to_string()
        };
        out.push_str(&scrub_hex_ids(&line));
    }
    out
}

/// `8-4-4-4-12` hex groups as `<id>` and six hex pairs joined by `:` or
/// `-` as `<mac>`.
fn scrub_hex_ids(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |i: usize, n: usize, sep: Option<u8>| -> bool {
        // n hex digits at i, or (sep) pairs separated by sep.
        match sep {
            None => i + n <= b.len() && b[i..i + n].iter().all(u8::is_ascii_hexdigit),
            Some(c) => (0..6).all(|k| {
                let j = i + k * 3;
                j + 2 <= b.len()
                    && b[j].is_ascii_hexdigit()
                    && b[j + 1].is_ascii_hexdigit()
                    && (k == 5 || b.get(j + 2) == Some(&c))
            }),
        }
    };
    let word = |i: usize| i > 0 && (b[i - 1].is_ascii_alphanumeric());
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if !word(i) && b[i].is_ascii_hexdigit() {
            let uuid = [8usize, 4, 4, 4, 12];
            let mut j = i;
            let mut ok = true;
            for (k, n) in uuid.iter().enumerate() {
                if k > 0 {
                    if b.get(j) == Some(&b'-') {
                        j += 1
                    } else {
                        ok = false;
                        break;
                    }
                }
                if hex(j, *n, None) {
                    j += n
                } else {
                    ok = false;
                    break;
                }
            }
            if ok && !b.get(j).is_some_and(u8::is_ascii_alphanumeric) {
                out.push_str("<id>");
                i = j;
                continue;
            }
            if let Some(c) = [b':', b'-']
                .into_iter()
                .find(|&c| hex(i, 17, Some(c)) && !b.get(i + 17).is_some_and(u8::is_ascii_alphanumeric))
            {
                let _ = c;
                out.push_str("<mac>");
                i += 17;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

/// The folder after `/home/`, `/Users/` or `\Users\` (any case; Wine's
/// `drive_c/users/` and `Z:\home\` too) replaced by `<user>`.
pub fn scrub_users(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        // A separator, `home` or `users`, the same separator: matched in
        // parts so the program holds no home path string of its own (the
        // release checks look for one).
        let rest = &lower.as_bytes()[i..];
        let hit = match rest.first() {
            Some(&sep @ (b'/' | b'\\')) => ["home", "users"].iter().find_map(|w| {
                let n = w.len() + 2;
                (rest.len() >= n && &rest[1..n - 1] == w.as_bytes() && rest[n - 1] == sep).then_some(n)
            }),
            _ => None,
        };
        if let Some(n) = hit {
            out.push_str(&s[i..i + n]);
            i += n;
            let end = s[i..].find(['/', '\\', '\n', '\r', '"', '\'', ';', ')', ']']).map_or(s.len(), |n| i + n);
            let name = &s[i..end];
            let keep = name.is_empty() || name == "~" || name == "<user>" || name.eq_ignore_ascii_case("public");
            if keep {
                out.push_str(name);
            } else {
                out.push_str("<user>");
            }
            i = end;
        } else {
            let ch = s[i..].chars().next().unwrap_or('\0');
            out.push(ch);
            i += ch.len_utf8().max(1);
        }
    }
    out
}

fn default_game_exe(origin: Origin) -> PathBuf {
    match origin {
        Origin::Launcher => crate::platform::dirs::game_exe(),
        _ => std::env::current_exe().unwrap_or_else(|_| crate::platform::dirs::game_exe()),
    }
}

pub(crate) fn system() -> String {
    let mut v = Vec::new();
    #[cfg(windows)]
    match crate::console::windows_version() {
        Some((a, b, build)) => v.push(format!(
            "os: Windows {a}.{b} build {build}{}",
            if a == 10 && build >= 22000 { " (Windows 11)" } else { "" }
        )),
        None => v.push("os: Windows (version not read)".into()),
    }
    #[cfg(not(windows))]
    {
        let pretty = std::fs::read_to_string("/etc/os-release").ok().and_then(|t| {
            t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
        });
        v.push(format!("os: {} ({})", pretty.unwrap_or_else(|| std::env::consts::OS.into()), std::env::consts::OS));
        if let Ok(k) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            v.push(format!("kernel: {}", k.trim()));
        }
    }
    v.push(format!("architecture: {}", std::env::consts::ARCH));
    v.push(format!("processors: {}", std::thread::available_parallelism().map_or(0, |n| n.get())));
    if let Ok(m) = std::fs::read_to_string("/proc/meminfo")
        && let Some(l) = m.lines().find(|l| l.starts_with("MemTotal:"))
    {
        v.push(format!("memory: {}", l.trim_start_matches("MemTotal:").trim()));
    }
    if let Ok(c) = std::fs::read_to_string("/proc/cpuinfo")
        && let Some(l) = c.lines().find(|l| l.starts_with("model name"))
    {
        v.push(format!("cpu: {}", l.split_once(':').map_or("", |x| x.1).trim()));
    }
    if let Ok(c) = std::env::var("PROCESSOR_IDENTIFIER") {
        v.push(format!("cpu: {c}"));
    }
    let set = |n: &str| std::env::var_os(n).is_some_and(|v| !v.is_empty());
    if !cfg!(windows) {
        v.push(format!(
            "session: {} (Wayland display {}, X display {}), desktop {}",
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "?".into()),
            if set("WAYLAND_DISPLAY") { "set" } else { "not set" },
            if set("DISPLAY") { "set" } else { "not set" },
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "?".into()),
        ));
    }
    if set("GAMESCOPE_WAYLAND_DISPLAY") {
        v.push("gamescope: yes".into());
    }
    if std::env::var("SteamDeck").as_deref() == Ok("1") {
        v.push("Steam Deck: yes".into());
    }
    for (var, what) in [("FLATPAK_ID", "Flatpak"), ("APPIMAGE", "AppImage"), ("SteamAppId", "started by Steam")] {
        if set(var) {
            v.push(format!("{what}: yes"));
        }
    }
    v.join("\n")
}

fn folders() -> String {
    use crate::platform::dirs;
    let mut v = Vec::new();
    v.push(format!(
        "program: {}",
        std::env::current_exe().map_or_else(|e| format!("({e})"), |p| p.display().to_string())
    ));
    v.push(match dirs::portable_base() {
        Some(p) => format!("portable: yes (marker in {})", p.display()),
        None => "portable: no".into(),
    });
    if let Some(d) = std::env::var_os(dirs::DATA_DIR_VAR).filter(|d| !d.is_empty()) {
        v.push(format!("{}: {}", dirs::DATA_DIR_VAR, PathBuf::from(d).display()));
    }
    let app = dirs::app();
    v.push(format!("Overbright's folder: {} ({})", app.display(), writable(&app)));
    v.push(format!("logs: {}", dirs::logs().display()));
    v.push(format!("profiles: {}", crate::frontend::profiles::root().display()));
    v.push(format!(
        "cache: {}",
        crate::cache::dir().map_or_else(|| "(none)".into(), |d| format!("{} ({})", d.display(), writable(&d)))
    ));
    if let Ok(e) = std::env::current_exe()
        && let Some(d) = e.parent()
    {
        let mut names: Vec<String> = std::fs::read_dir(d)
            .map(|r| r.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect())
            .unwrap_or_default();
        names.sort();
        names.truncate(60);
        v.push(format!("program folder holds: {}", names.join(", ")));
    }
    v.join("\n")
}

/// Whether a folder exists (and is marked read-only).
fn writable(dir: &Path) -> &'static str {
    // Nothing is written to find out: the support file's own folder (its
    // last section) shows whether the logs folder took it.
    match std::fs::metadata(dir) {
        Ok(m) if !m.is_dir() => "NOT a folder",
        Ok(m) if m.permissions().readonly() => "read-only",
        Ok(_) => "exists",
        Err(_) => "does not exist yet",
    }
}

/// The variables that change Overbright, wgpu, Vulkan, Mesa or DXVK.
fn environment() -> String {
    const PREFIXES: [&str; 9] = ["NFSU2_", "OB_", "WGPU_", "VK_", "MESA_", "DXVK_", "RADV_", "__GL", "SDL_"];
    // Wine's own (the host's paths and names left out).
    const NAMES: [&str; 9] = [
        "RUST_LOG",
        "RUST_BACKTRACE",
        "LD_PRELOAD",
        "ENABLE_VKBASALT",
        "WINEPREFIX",
        "WINEDLLOVERRIDES",
        "WINEDEBUG",
        "WINEESYNC",
        "WINEFSYNC",
    ];
    let mut v: Vec<String> = std::env::vars_os()
        .filter_map(|(k, val)| {
            let k = k.into_string().ok()?;
            (PREFIXES.iter().any(|p| k.starts_with(p)) || NAMES.contains(&k.as_str()))
                .then(|| format!("{k}={}", val.to_string_lossy()))
        })
        .collect();
    v.sort();
    if v.is_empty() { "(none set)".into() } else { v.join("\n") }
}

fn game_folder(given: Option<&Path>) -> String {
    use crate::qol::setup;
    let mut v = Vec::new();
    let root = given.map(Path::to_path_buf).or_else(setup::find);
    let Some(root) = root else {
        v.push("no game folder found".into());
        v.push(format!("kept folder file: {}", setup::saved_root_file().display()));
        for c in setup::candidates() {
            v.push(format!("  looked at {} ({})", c.display(), if c.is_dir() { "exists" } else { "absent" }));
        }
        return v.join("\n");
    };
    v.push(format!("game folder: {}{}", root.display(), if root.is_dir() { "" } else { " (does not exist)" }));
    v.push(format!(
        "kept in {}: {}",
        setup::saved_root_file().display(),
        setup::saved_root().map_or_else(|| "(none)".into(), |p| p.display().to_string())
    ));
    let c = setup::check(&root, true);
    for file in c.key_files() {
        let file = file.as_str();
        let path = setup::key_file_path(&root, file);
        let state = if c.missing.iter().any(|m| m == file) {
            "MISSING".to_string()
        } else {
            let len = std::fs::metadata(&path).map_or(0, |m| m.len());
            format!(
                "{len} bytes{}",
                if c.different.iter().any(|d| d == file) {
                    " (differs from the known release)"
                } else if c.unverified.iter().any(|d| d == file) {
                    " (not verified)"
                } else {
                    ""
                }
            )
        };
        v.push(format!("  {file}: {state}"));
    }
    let fp = crate::fingerprint::compute(&root);
    v.push(fp.report().trim_end().to_string());
    v.push("summary:".into());
    for (mark, line) in setup::check_lines(&c, Some(&fp)) {
        let tag = match mark {
            setup::Mark::Ok => "ok",
            setup::Mark::Warn => "warning",
            setup::Mark::Info => "note",
            setup::Mark::Error => "ERROR",
        };
        v.push(format!("  {tag}: {line}"));
    }
    v.push("game files the game looks for:".into());
    for line in setup::check_details(&c) {
        v.push(format!("  {line}"));
    }
    v.join("\n")
}

/// A program's exit in words: the code, a Windows status's name, a signal.
pub fn exit_words(status: std::process::ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return format!("ended by signal {sig}");
        }
    }
    match status.code() {
        Some(c) => {
            let u = c as u32;
            match super::native_crash::status_name(u) {
                Some(n) => format!("exit code 0x{u:08x} ({n})"),
                None => format!("exit code {c}"),
            }
        }
        None => "ended without an exit code".into(),
    }
}

/// Runs `exe args` without a window, at most [`CHILD_LIMIT`]; its output
/// and how it ended.
fn ask(exe: &Path, args: &[&str]) -> String {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    no_window(&mut cmd);
    run_limited(cmd, CHILD_LIMIT, &format!("{} {}", exe.display(), args.join(" ")))
}

pub(crate) fn no_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

pub(crate) fn run_limited(mut cmd: std::process::Command, limit: Duration, what: &str) -> String {
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return format!("{what}: could not start: {e}"),
    };
    let read = |p: Option<Box<dyn io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = p {
                let _ = p.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn io::Read + Send>));
    let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn io::Read + Send>));
    let t0 = Instant::now();
    let ended = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if t0.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let how = match ended {
        Some(s) if s.success() => format!("ended normally after {:.1} s", t0.elapsed().as_secs_f32()),
        Some(s) => format!("FAILED: {} after {:.1} s", exit_words(s), t0.elapsed().as_secs_f32()),
        None => format!("FAILED: no answer within {} s; stopped", limit.as_secs()),
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    let mut s = format!("{what}: {how}\n");
    if !out.trim().is_empty() {
        s += out.trim_end();
        s.push('\n');
    }
    if !err.trim().is_empty() {
        s += "(error output)\n";
        s += &tail_text(&err, 16 * 1024);
    }
    s
}

/// The Application event log's recent crash entries (Application Error,
/// Windows Error Reporting) that name Overbright's programs.
fn windows_events() -> String {
    let mut cmd = std::process::Command::new("wevtutil");
    cmd.args([
        "qe",
        "Application",
        "/q:*[System[(EventID=1000 or EventID=1001 or EventID=1002)]]",
        "/c:60",
        "/rd:true",
        "/f:text",
    ]);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
    no_window(&mut cmd);
    let all = run_limited(cmd, Duration::from_secs(15), "wevtutil");
    let found = ours_in_events(&all);
    if found.is_empty() {
        let first = all.lines().next().unwrap_or("");
        format!("no crash entries for overbright.exe or game.exe in the last 60 ({first})")
    } else {
        found.join("\n")
    }
}

/// The entries of `wevtutil /f:text` output (each from an `Event[n]:`
/// line) that name overbright.exe or game.exe, at most 8.
pub fn ours_in_events(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, v: &mut Vec<String>| {
        let l = cur.to_ascii_lowercase();
        if (l.contains("overbright.exe") || l.contains("game.exe")) && v.len() < 8 {
            v.push(cur.trim_end().to_string());
        }
        cur.clear();
    };
    for line in text.lines() {
        if line.starts_with("Event[") {
            flush(&mut cur, &mut v);
        }
        cur += line;
        cur.push('\n');
    }
    flush(&mut cur, &mut v);
    v
}

fn settings() -> String {
    let dir = crate::platform::dirs::config();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|r| r.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect())
        .unwrap_or_default();
    names.sort();
    let mut s = format!("{} holds: {}\n", dir.display(), names.join(", "));
    for n in names.iter().filter(|n| n.ends_with(".txt")) {
        let p = dir.join(n);
        if std::fs::metadata(&p).is_ok_and(|m| m.is_file() && m.len() <= 64 * 1024) {
            s += &format!(
                "--- {n} ---\n{}\n",
                std::fs::read_to_string(&p).unwrap_or_else(|e| format!("({e})")).trim_end()
            );
        }
    }
    s
}

/// The newest `n` files in `dir` named `prefix…suffix` (their names sort
/// by time).
fn newest(dir: &Path, prefix: &str, suffix: &str, n: usize) -> Vec<PathBuf> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok())).collect())
        .unwrap_or_default();
    v.retain(|f| f.starts_with(prefix) && f.ends_with(suffix));
    v.sort();
    v.into_iter().rev().take(n).map(|f| dir.join(f)).collect()
}

fn logs() -> String {
    use crate::platform::dirs;
    let mut files: Vec<PathBuf> = Vec::new();
    let app = dirs::config();
    files.push(app.join("launcher.log"));
    files.push(app.join("launcher.old.log"));
    let logs = dirs::logs();
    files.extend(newest(&logs, "launcher-", ".log", 3));
    files.extend(newest(&logs, "game-", ".log", 3));
    files.extend(newest(&dirs::crashes(), "crash-", ".txt", 2));
    let mut s = String::new();
    for f in files {
        match std::fs::metadata(&f) {
            Ok(m) if m.is_file() => {
                let age = m
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map_or(String::new(), |d| format!(", written {} ago", ago(d)));
                s += &format!("\n--- {} ({} bytes{age}) ---\n", f.display(), m.len());
                s += &read_capped(&f, LOG_MAX);
            }
            _ => s += &format!("--- {}: none ---\n", f.display()),
        }
    }
    s
}

fn ago(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..120 => format!("{s} s"),
        120..7200 => format!("{} min", s / 60),
        7200..172_800 => format!("{} h", s / 3600),
        _ => format!("{} days", s / 86_400),
    }
}

/// A file's text, its middle left out past `max` bytes.
fn read_capped(path: &Path, max: u64) -> String {
    match std::fs::read(path) {
        Ok(b) => {
            let t = String::from_utf8_lossy(&b);
            let mut s = tail_text(&t, max as usize);
            if !s.ends_with('\n') {
                s.push('\n');
            }
            s
        }
        Err(e) => format!("({e})\n"),
    }
}

/// `t` whole, or its first 8 KB and its last `max - 8 KB` with a line
/// saying how much was left out.
pub fn tail_text(t: &str, max: usize) -> String {
    if t.len() <= max {
        return t.to_string();
    }
    let head_len = (8 * 1024).min(max / 4);
    let cut = |i: usize| (0..=i.min(t.len())).rev().find(|&j| t.is_char_boundary(j)).unwrap_or(0);
    let head = &t[..cut(head_len)];
    let tail_from = cut(t.len() - (max - head_len));
    format!("{head}\n[… {} bytes left out …]\n{}", tail_from - head.len(), &t[tail_from..])
}

/// The adapters this machine's graphics drivers offer, as the game's
/// instance sees them, and the one the game would pick (no window, no
/// device): `game --list-adapters`.
pub fn adapter_lines() -> Vec<String> {
    let instance = crate::render::Gpu::new_instance();
    let all = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let mut v = vec![format!("{} adapter(s)", all.len())];
    for a in &all {
        let i = a.get_info();
        v.push(format!(
            "adapter: {} | {:?} | {:?} | vendor 0x{:04x} device 0x{:04x} | driver {} {}",
            i.name, i.backend, i.device_type, i.vendor, i.device, i.driver, i.driver_info
        ));
    }
    match pollster::block_on(instance.request_adapter(&crate::render::Gpu::adapter_options(None))) {
        Ok(a) => {
            let i = a.get_info();
            v.push(format!("picked without a window: {} ({:?})", i.name, i.backend));
        }
        Err(e) => v.push(format!("picked without a window: none ({e})")),
    }
    v
}

/// The `--support-file [folder]` command of both programs (`args` without
/// the program name): None when it is not asked for; else the file is
/// written, its path printed (and on Windows, started from Explorer or a
/// shortcut, shown in a message box and selected in Explorer), and the
/// exit code is returned.
pub fn cli(args: &[String], game_exe: Option<PathBuf>) -> Option<u8> {
    let at = args.iter().position(|a| a == FLAG)?;
    let out_dir = args.get(at + 1).filter(|a| !a.starts_with('-')).map(PathBuf::from);
    let req = SupportRequest {
        origin: Origin::Command,
        reason: None,
        out_dir,
        ask_game: true,
        game_exe,
        ..Default::default()
    };
    Some(match write_support_file(&req) {
        Ok(p) => {
            println!("{}", p.display());
            crate::console::info_if_unseen(&format!(
                "A support file was saved:\n\n{}\n\nAttach it to your bug report.",
                p.display()
            ));
            reveal(&p);
            0
        }
        Err(e) => {
            crate::console::fatal(&format!("The support file could not be written: {e}"));
            1
        }
    })
}

/// Shows the file in the system's file manager when no terminal sees the
/// output (Windows: Explorer with the file selected). Nothing elsewhere.
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    if !crate::console::output_seen() {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(not(windows))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names_in_paths_are_scrubbed() {
        // Linux home folders, built so the text holds no literal home path.
        let h = |rest: &str| format!("/{}/{rest}", "home");
        assert_eq!(scrub_users("C:\\Users\\Jane Doe\\AppData"), "C:\\Users\\<user>\\AppData");
        assert_eq!(
            scrub_users(&format!("C:\\Users\\JANEDO~1\\x and {}", h("jane/.cache"))),
            format!("C:\\Users\\<user>\\x and {}", h("<user>/.cache"))
        );
        assert_eq!(scrub_users(&h("u/.wine/drive_c/users/u/AppData")), h("<user>/.wine/drive_c/users/<user>/AppData"));
        assert_eq!(scrub_users("Z:\\home\\jane\\Games"), "Z:\\home\\<user>\\Games");
        let kept = format!("C:\\Users\\Public\\x ~/y {}", h(""));
        assert_eq!(scrub_users(&kept), kept);
        assert_eq!(scrub_users("plain text, ünïcode"), "plain text, ünïcode");
    }

    #[test]
    fn long_texts_keep_their_start_and_end() {
        let t: String = (0..40_000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let s = tail_text(&t, 20_000);
        assert!(s.len() < 20_100 && s.starts_with("abc") && s.ends_with(&t[t.len() - 50..]), "{}", s.len());
        assert!(s.contains("bytes left out"));
        assert_eq!(tail_text("short", 100), "short");
    }

    #[test]
    fn old_support_files_are_pruned() {
        let names: Vec<String> =
            (1..=7).map(|d| format!("{PREFIX}2026111{d}-000000.txt")).chain(["other.txt".into()]).collect();
        assert_eq!(
            to_prune(&names),
            vec![
                format!("{PREFIX}20261111-000000.txt"),
                format!("{PREFIX}20261112-000000.txt"),
                format!("{PREFIX}20261113-000000.txt")
            ]
        );
    }

    #[test]
    fn windows_crash_events_for_our_programs_are_picked() {
        let text = "Event[0]:\n  Log Name: Application\n  Description:\nFaulting application name: overbright.exe, version: 0.0.0.0\nException code: 0xc0000005\nFaulting module name: nvoglv64.dll\nEvent[1]:\n  Description:\nFaulting application name: other.exe\nEvent[2]:\nFaulting application name: GAME.EXE\n";
        let v = ours_in_events(text);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].contains("nvoglv64.dll") && v[1].contains("GAME.EXE"));
    }

    #[cfg(unix)]
    #[test]
    fn a_child_that_fails_or_hangs_is_reported() {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", "echo out; echo err >&2; exit 3"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let s = run_limited(c, Duration::from_secs(10), "t");
        assert!(s.contains("FAILED: exit code 3") && s.contains("out\n") && s.contains("(error output)\nerr"), "{s}");
        let mut c = std::process::Command::new("sleep");
        c.arg("30").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let s = run_limited(c, Duration::from_millis(300), "t");
        assert!(s.contains("no answer within"), "{s}");
        let s = ask(Path::new("/nonexistent/game"), &["--probe"]);
        assert!(s.contains("could not start"), "{s}");
    }

    /// The whole file in a folder of its own: its sections, the game folder
    /// that is not there, the reason, and no home folder.
    #[test]
    fn the_support_file_is_written_with_every_section() {
        let dir = std::env::temp_dir().join(format!("ob-support-{}", crate::unique_tag()));
        crate::platform::dirs::set_thread_data_home(dir.join("data"));
        let req = SupportRequest {
            origin: Origin::Command,
            reason: Some("test reason".into()),
            out_dir: Some(dir.clone()),
            game_root: Some(dir.join("no-game")),
            ask_game: false,
            extra: vec![("caller's part".into(), "line one".into())],
            ..Default::default()
        };
        let p = write_support_file(&req).unwrap();
        assert!(p.starts_with(&dir) && p.file_name().unwrap().to_string_lossy().starts_with(PREFIX));
        let t = std::fs::read_to_string(&p).unwrap();
        for h in [
            "summary",
            "system",
            "Overbright's folders",
            "environment",
            "game folder",
            "graphics adapter in use",
            "audio output in use",
            "caller's part",
            "settings files",
            "logs",
            "written to",
        ] {
            assert!(t.contains(&format!("===== {h} =====")), "{h}: {t}");
        }
        assert!(
            t.contains("what happened: test reason") && t.contains("(does not exist)") && t.contains("MISSING"),
            "{t}"
        );
        if let Ok(home) = std::env::var("HOME")
            && home.len() > 1
        {
            assert!(!t.contains(&home), "home folder left in the file");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
