//! Running `game --probe` from the launcher (docs/launcher.md): in the
//! background ([`spawn`]), read back with `Probe::parse`. The format itself
//! is the library's (`launch::probe`), shared with the game.

use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

pub use crate::launch::probe::*;

/// How long the launcher waits for the probe (a driver's first start can
/// compile its caches).
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Runs `exe --probe` and reads its result; a failure in plain words
/// (with the last line the game printed to stderr). `root`: the game
/// folder the player chose, given to the probe as `NFSU2_ROOT`; None
/// leaves the environment as it is.
pub fn run(exe: &Path, root: Option<&Path>, timeout: Duration) -> Result<Probe, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(exe);
    if let Some(r) = root {
        cmd.env("NFSU2_ROOT", r);
    }
    // Windows: no console window for the check.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd
        .arg("--probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("the game could not be started ({}): {e}", exe.display()))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("the game's hardware check did not finish within {} s", timeout.as_secs()));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    match Probe::parse(&stdout) {
        Ok(p) if status.success() || p.error.is_some() => Ok(p),
        Ok(_) | Err(_) => {
            let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
            let code = status.code().map_or("a signal".to_string(), |c| format!("code {c}"));
            Err(if last.is_empty() {
                format!("the game's hardware check ended with {code}")
            } else {
                format!("the game's hardware check ended with {code}: {last}")
            })
        }
    }
}

/// [`run`] on a thread of its own; the result arrives on the receiver
/// (the launcher runs it at start, and again whenever the game folder
/// changes).
pub fn spawn(exe: std::path::PathBuf, root: Option<std::path::PathBuf>) -> Receiver<Result<Probe, String>> {
    let (tx, rx) = channel();
    let _ = std::thread::Builder::new().name("probe".into()).spawn(move || {
        let _ = tx.send(run(&exe, root.as_deref(), TIMEOUT));
    });
    rx
}
