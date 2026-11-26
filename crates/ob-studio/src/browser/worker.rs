//! Reading a file that came from outside in a process of its own, with a time limit, a memory limit (where the
//! system has one to give) and a cap on what it may hand back. A file that makes the reader run away, crash or
//! write without end ends that process only: Studio and the mod are untouched, and the import says so.
//!
//! The worker is `overbright-studio-cli --worker-read-gltf <file>` ([`run_worker`]): it reads the file with
//! [`super::import::read_gltf`] and writes the result as JSON on its standard output. [`read`] starts it from the
//! program found beside the running one (or named by `OB_STUDIO_WORKER`); with none to be found the file is read in
//! this process, behind a panic guard, and still under the file size limits of the reader.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::import::{Foreign, Refusal, read_gltf};

/// The argument that makes the command-line program a worker.
pub const WORKER_ARG: &str = "--worker-read-gltf";
/// The worker is stopped after this long.
pub const TIME_LIMIT: Duration = Duration::from_secs(20);
/// Address space a worker may use on a system with limits (Linux, macOS), in KiB.
pub const MEMORY_LIMIT_KIB: u64 = 3 << 20;
/// What a worker may hand back before it is stopped.
pub const OUTPUT_LIMIT: usize = 256 << 20;

/// The worker's side: read `path`, print the result (`{"ok": Foreign}` or `{"refused": [reasons]}`).
pub fn run_worker(path: &Path) -> i32 {
    let doc = match read_gltf(path) {
        Ok(f) => serde_json::json!({ "ok": f }),
        Err(r) => serde_json::json!({ "refused": r.0 }),
    };
    println!("{doc}");
    0
}

/// The program that can act as the worker, when there is one.
pub fn worker_program() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OB_STUDIO_WORKER").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let me = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "overbright-studio-cli.exe" } else { "overbright-studio-cli" };
    if me.file_name().is_some_and(|n| n == name) {
        return Some(me);
    }
    let p = me.parent()?.join(name);
    p.is_file().then_some(p)
}

/// Read `path` in a worker: its [`Foreign`] or why not.
pub fn read(path: &Path) -> Result<Foreign, Refusal> {
    match worker_program() {
        Some(p) => read_with(&p, path),
        None => std::panic::catch_unwind(|| read_gltf(path)).unwrap_or_else(|_| {
            Err(Refusal::from("reading the file failed unexpectedly: it was left alone".to_string()))
        }),
    }
}

fn command(program: &Path, file: &Path) -> Command {
    #[cfg(unix)]
    {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(format!("ulimit -v {MEMORY_LIMIT_KIB} 2>/dev/null; exec \"$0\" \"$@\""))
            .arg(program)
            .arg(WORKER_ARG)
            .arg(file);
        c
    }
    #[cfg(not(unix))]
    {
        let mut c = Command::new(program);
        c.arg(WORKER_ARG).arg(file);
        c
    }
}

/// [`read`] with a given worker program and the standard limits.
pub fn read_with(program: &Path, file: &Path) -> Result<Foreign, Refusal> {
    read_limited(program, file, TIME_LIMIT, OUTPUT_LIMIT)
}

/// [`read`] with given limits (the tests use small ones).
pub fn read_limited(program: &Path, file: &Path, time: Duration, cap: usize) -> Result<Foreign, Refusal> {
    let untouched = "Studio and your mod are untouched";
    let mut child = command(program, file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Refusal::from(format!("cannot start the file reader: {e}")))?;
    let mut out = child.stdout.take().ok_or_else(|| Refusal::from("cannot start the file reader".to_string()))?;
    // A thread collects the output so a full pipe never stalls the worker, and stops at the cap.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1 << 16];
        loop {
            match out.read(&mut chunk) {
                Ok(0) | Err(_) => return (buf, false),
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > cap {
                        return (buf, true);
                    }
                }
            }
        }
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > time || reader.is_finished() => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => break None,
        }
    };
    // A stopped worker's own children may hold the pipe open: only a reader that has ended is waited for.
    let (bytes, over) =
        if status.is_some() || reader.is_finished() { reader.join().unwrap_or_default() } else { (Vec::new(), false) };
    if over {
        return Err(format!(
            "reading the file ran away (more than {} MB of output): it was stopped. {untouched}",
            cap >> 20
        )
        .into());
    }
    let Some(status) = status else {
        if bytes.is_empty() || start.elapsed() > time {
            return Err(
                format!("reading the file took longer than {} s: it was stopped. {untouched}", time.as_secs()).into()
            );
        }
        return Err(format!("reading the file failed. {untouched}").into());
    };
    if !status.success() {
        return Err(format!("the file made its reader crash (it was a worker process: {untouched})").into());
    }
    let doc: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| Refusal::from(format!("the file reader gave no answer. {untouched}")))?;
    if let Some(r) = doc.get("refused").and_then(|r| r.as_array()) {
        return Err(Refusal(r.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()));
    }
    serde_json::from_value(doc["ok"].clone())
        .map_err(|_| Refusal::from(format!("the file reader's answer does not read. {untouched}")))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(tag: &str, body: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-worker-{tag}-{}", ob_core::unique_tag()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("worker.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn err(body: &str, time: Duration, cap: usize) -> String {
        read_limited(&script("t", body), Path::new("x.gltf"), time, cap).unwrap_err().to_string()
    }

    #[test]
    fn a_crash_a_hang_and_a_runaway_end_the_worker_only() {
        let long = Duration::from_secs(10);
        assert!(err("kill -SEGV $$", long, 1 << 20).contains("crash"));
        assert!(err("exit 3", long, 1 << 20).contains("crash"));
        assert!(err("echo nonsense", long, 1 << 20).contains("no answer"));
        let t = Instant::now();
        assert!(err("sleep 30", Duration::from_millis(300), 1 << 20).contains("took longer"));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(err("yes", long, 1 << 16).contains("ran away"));
    }

    #[test]
    fn answers_come_back() {
        let ok = script("ok", r#"echo '{"ok":{"sets":[],"prims":[],"model":"A","hash":null,"origin":null}}'"#);
        let f = read_limited(&ok, Path::new("x"), Duration::from_secs(10), 1 << 20).unwrap();
        assert_eq!(f.model.as_deref(), Some("A"));
        let no = script("no", r#"echo '{"refused":["a","b"]}'"#);
        let r = read_limited(&no, Path::new("x"), Duration::from_secs(10), 1 << 20).unwrap_err();
        assert_eq!(r.0, vec!["a", "b"]);
    }
}
