//! One writer per mod: the writer holds the operating system's exclusive lock on the mod's `.editor/studio.lock`
//! (a process lock through `fcntl` on Linux, `LockFileEx` on Windows) for as long as it has the mod open. The
//! system lets the lock go when the process ends in any way, a crash or `kill -9` included, so a lock left by a
//! dead process is never seen as held and needs no clean-up; a live writer's lock is never taken away. Who holds
//! it is written beside it (`studio-owner.toml`), for the read-only state to name. The note is only a name: it
//! is removed when the writer lets the lock go, and a note left by a writer that died ([`holder`]: the lock is
//! free) or by this process is never taken as another program having the mod.
//!
//! The lock never travels to a program the writer starts (the test game, a test run). On Linux a starting
//! program is a copy of the writer, open files included, until it runs the new program; a `flock` lock belongs
//! to the open file and would stay held by that copy for a moment after the writer let it go, so the next
//! opener was refused now and then. A `fcntl` process lock belongs to the process and is never copied. Such a
//! lock does not keep two openers in the same process apart, and closing any handle of the file lets it go, so
//! this process's held locks are also kept in a list, checked before the file is opened. On Windows a file
//! handle is not passed to a started program (Rust opens files not inheritable) and a `LockFileEx` lock stays
//! with the handle that took it.
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// The lock file, in the mod's editor folder.
pub const LOCK: &str = "studio.lock";
/// Who holds the lock.
pub const OWNER: &str = "studio-owner.toml";

/// The program that has a mod open for writing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    /// "Overbright Studio", "overbright-studio (command line)".
    pub program: String,
    pub pid: u32,
    /// When it took the lock (Unix time, ms).
    pub since: u64,
}

impl Owner {
    fn write(&self) -> String {
        format!(
            "# Who has this mod open for writing (only while it holds studio.lock).\nprogram = {:?}\npid = {}\nsince = {}\n",
            self.program, self.pid, self.since
        )
    }

    fn read(text: &str) -> Option<Owner> {
        let t: toml::Table = toml::from_str(text).ok()?;
        Some(Owner {
            program: t.get("program")?.as_str()?.to_string(),
            pid: u32::try_from(t.get("pid")?.as_integer()?).ok()?,
            since: u64::try_from(t.get("since")?.as_integer()?).ok()?,
        })
    }
}

/// The held lock; dropping it lets the lock go.
#[derive(Debug)]
pub struct WriteLock {
    file: Option<File>,
    /// The lock file, as kept in [`HELD`].
    key: PathBuf,
    dir: PathBuf,
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        // Closed while the list is locked: another opener of this process cannot open the file in between.
        let mut held = held();
        // The note goes while the lock is still held: the next writer's note is never removed.
        let _ = std::fs::remove_file(self.dir.join(OWNER));
        drop(self.file.take());
        held.remove(&self.key);
    }
}

/// The lock files this process holds.
static HELD: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

struct Held<'a>(MutexGuard<'a, Option<HashSet<PathBuf>>>);

impl Held<'_> {
    fn contains(&self, k: &Path) -> bool {
        self.0.as_ref().is_some_and(|h| h.contains(k))
    }
    fn insert(&mut self, k: PathBuf) {
        self.0.get_or_insert_with(HashSet::new).insert(k);
    }
    fn remove(&mut self, k: &Path) {
        if let Some(h) = self.0.as_mut() {
            h.remove(k);
        }
    }
}

fn held() -> Held<'static> {
    Held(HELD.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Try the exclusive lock on `file`: Ok(true) taken, Ok(false) another process holds it.
#[cfg(unix)]
fn try_lock(file: &File) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    // SAFETY: an all-zero `flock` is a valid value; the fields used are set below.
    let mut l: libc::flock = unsafe { std::mem::zeroed() };
    l.l_type = libc::F_WRLCK as _;
    l.l_whence = libc::SEEK_SET as _;
    // l_start = 0, l_len = 0: the whole file.
    // SAFETY: `file` is open for the call and `l` is a valid `flock`.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &l) } == 0 {
        return Ok(true);
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EACCES | libc::EAGAIN) => Ok(false),
        _ => Err(e),
    }
}

/// Try the exclusive lock on `file`: Ok(true) taken, Ok(false) another handle holds it.
#[cfg(not(unix))]
fn try_lock(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

impl WriteLock {
    /// The folder the lock is in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Take the write lock of the mod whose editor folder is `dir`: Ok(Ok) the lock, Ok(Err) another live program
/// holds it (who, when it says so).
pub fn try_take(dir: &Path, program: &str) -> io::Result<Result<WriteLock, Option<Owner>>> {
    std::fs::create_dir_all(dir)?;
    let key = std::fs::canonicalize(dir)?.join(LOCK);
    let mut held = held();
    // Held here: opening (and closing) the file again would let this process's lock go.
    if held.contains(&key) {
        return Ok(Err(owner(dir)));
    }
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&key)?;
    if !try_lock(&file)? {
        return Ok(Err(owner(dir)));
    }
    held.insert(key.clone());
    drop(held);
    let me = Owner { program: program.to_string(), pid: std::process::id(), since: super::now_ms() };
    // Only a note for others: a failed write keeps the lock.
    let _ = ob_core::platform::fs::write_atomic(&dir.join(OWNER), me.write().as_bytes(), false);
    Ok(Ok(WriteLock { file: Some(file), key, dir: dir.to_path_buf() }))
}

/// Who has a mod's lock, as [`holder`] finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Holder {
    /// Nobody: the mod is free to open for writing (a note left by a writer that died is ignored).
    Free,
    /// This process (its own writer, such as the editor Studio hosts).
    ThisProcess,
    /// Another live program (who, when its note says so).
    Other(Option<Owner>),
}

/// Who holds the lock of the mod whose editor folder is `dir`, without taking it (what a look at the mod asks).
pub fn holder(dir: &Path) -> Holder {
    let Ok(key) = std::fs::canonicalize(dir).map(|d| d.join(LOCK)) else { return Holder::Free };
    // Kept for the whole look: a file of this process's lock must not be opened (closing it lets the lock go),
    // and no opener of this process takes the lock meanwhile.
    let held = held();
    if held.contains(&key) {
        return Holder::ThisProcess;
    }
    let Ok(file) = OpenOptions::new().read(true).write(true).open(&key) else { return Holder::Free };
    match probe(&file) {
        Ok(false) => Holder::Free,
        // An error: say held, as the writer itself would find it.
        Ok(true) | Err(_) => match owner(dir) {
            Some(o) if o.pid == std::process::id() => Holder::Other(None),
            o => Holder::Other(o),
        },
    }
}

/// Whether another process holds the lock on `file` (the lock is not taken).
#[cfg(unix)]
fn probe(file: &File) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    // SAFETY: an all-zero `flock` is a valid value; the fields used are set below.
    let mut l: libc::flock = unsafe { std::mem::zeroed() };
    l.l_type = libc::F_WRLCK as _;
    l.l_whence = libc::SEEK_SET as _;
    // SAFETY: `file` is open for the call and `l` is a valid `flock`.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut l) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(i32::from(l.l_type) != libc::F_UNLCK)
}

/// Whether another handle holds the lock on `file`: taken and let go again at once when free.
#[cfg(not(unix))]
fn probe(file: &File) -> io::Result<bool> {
    if try_lock(file)? {
        file.unlock()?;
        return Ok(false);
    }
    Ok(true)
}

/// Who says they hold the lock of `dir` (only meaningful while it is held: see [`holder`]).
pub fn owner(dir: &Path) -> Option<Owner> {
    Owner::read(&std::fs::read_to_string(dir.join(OWNER)).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_holder_at_a_time_and_the_lock_goes_with_it() {
        let s = ob_testkit::Scratch::new("studio-lock");
        let a = try_take(s.path(), "Overbright Studio").unwrap().expect("free");
        // A second opener (here the same process, another handle) is refused and told who.
        let held = try_take(s.path(), "other").unwrap().expect_err("held");
        let o = held.expect("owner note");
        assert_eq!((o.program.as_str(), o.pid), ("Overbright Studio", std::process::id()));
        drop(a);
        let b = try_take(s.path(), "other").unwrap().expect("free again");
        assert_eq!(owner(b.dir()).unwrap().program, "other");
    }

    /// A look at the mod (Home, Changes) believes the lock, not the note: the note goes on a clean close, a note
    /// left by a writer that died is ignored, and this process's own writer is not "another program".
    #[test]
    fn the_owner_note_never_outlives_the_lock() {
        let s = ob_testkit::Scratch::new("studio-lock-note");
        assert_eq!(holder(s.path()), Holder::Free);
        let a = try_take(s.path(), "Overbright Studio").unwrap().expect("free");
        assert_eq!(holder(s.path()), Holder::ThisProcess);
        // Looking did not let the lock go.
        assert!(try_take(s.path(), "other").unwrap().is_err());
        drop(a);
        assert!(!s.path().join(OWNER).exists(), "a clean close removes the note");
        assert_eq!(holder(s.path()), Holder::Free);
        // A note left by a killed writer (another pid, no lock held).
        let dead = Owner { program: "Overbright Studio".into(), pid: 999_999, since: 1 };
        std::fs::write(s.path().join(OWNER), dead.write()).unwrap();
        assert_eq!(holder(s.path()), Holder::Free);
        let b = try_take(s.path(), "other").unwrap().expect("a dead writer's note does not hold the mod");
        assert_eq!(owner(b.dir()).unwrap().program, "other");
    }

    /// A program started by another thread of the writer (the test game, a test run) gets a copy of every open
    /// file of the writer for a moment before it starts the new program. The lock must not travel with it: once
    /// the writer lets the lock go, the next opener gets it at once, whatever is being started meanwhile.
    #[cfg(unix)]
    #[test]
    fn a_program_started_meanwhile_does_not_keep_the_lock() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let s = ob_testkit::Scratch::new("studio-lock-child");
        let stop = Arc::new(AtomicBool::new(false));
        let starters: Vec<_> = (0..4)
            .map(|_| {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let _ = std::process::Command::new("true").status();
                    }
                })
            })
            .collect();
        let mut refused = 0;
        for _ in 0..3000 {
            let a = try_take(s.path(), "Overbright Studio").unwrap().expect("free");
            drop(a);
            match try_take(s.path(), "Overbright Studio").unwrap() {
                Ok(b) => drop(b),
                Err(_) => refused += 1,
            }
        }
        stop.store(true, Ordering::Relaxed);
        for t in starters {
            t.join().unwrap();
        }
        assert_eq!(refused, 0, "a lock let go was still held by a program being started");
    }

    /// The launcher's Dev and Mods pages read the lock without taking it (`nfsu2::studio_api::launcher::studio_seen`, which
    /// cannot name this crate): its file names are these, and it sees a lock another process holds, with the note's
    /// holder, and a free lock once that process lets it go.
    #[test]
    fn the_launcher_sees_a_lock_held_by_another_process() {
        use nfsu2::studio_api::launcher::studio_seen as seen;
        assert_eq!((seen::LOCK, seen::OWNER, seen::STUDIO), (LOCK, OWNER, super::super::STUDIO));
        let s = ob_testkit::Scratch::new("studio-lock-launcher");
        let edir = s.path().join("drift-park").join(".editor");
        let folders = ["drift-park".to_string()];
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "history::lock::tests::launcher_lock_holder", "--nocapture", "--test-threads=1"])
            .env("OB_LAUNCHER_LOCK_HOLD", s.path())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let ready = s.path().join("ready");
        let t0 = std::time::Instant::now();
        while !ready.exists() {
            assert!(t0.elapsed().as_secs() < 30 && child.try_wait().unwrap().is_none(), "the holder did not start");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let got = seen::read(s.path(), &folders, &s.path().join("none"));
        std::fs::write(s.path().join("release"), b"").unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(got.held.len(), 1, "{got:?}");
        let h = &got.held[0];
        assert_eq!((h.folder.as_str(), h.program.as_deref(), h.pid), ("drift-park", Some(super::super::STUDIO), Some(child.id())));
        assert!(h.by_studio());
        assert!(edir.join(LOCK).is_file());
        assert_eq!(seen::read(s.path(), &folders, &s.path().join("none")).held, Vec::new(), "free once the holder ended");
    }

    /// The other process of [`the_launcher_sees_a_lock_held_by_another_process`] (does nothing on its own).
    #[test]
    fn launcher_lock_holder() {
        let Some(dir) = std::env::var_os("OB_LAUNCHER_LOCK_HOLD").map(std::path::PathBuf::from) else { return };
        let lock = try_take(&dir.join("drift-park").join(".editor"), super::super::STUDIO).unwrap().expect("free");
        std::fs::write(dir.join("ready"), b"").unwrap();
        let t0 = std::time::Instant::now();
        while !dir.join("release").exists() && t0.elapsed().as_secs() < 30 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(lock);
    }
}
