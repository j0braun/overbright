//! One writer per mod: the writer holds the operating system's exclusive lock on the mod's `.editor/studio.lock`
//! (`flock` on Linux, `LockFileEx` on Windows) for as long as it has the mod open. The system lets the lock go
//! when the process ends in any way, a crash or `kill -9` included, so a lock left by a dead process is never
//! seen as held and needs no clean-up; a live writer's lock is never taken away. Who holds it is written beside
//! it (`studio-owner.toml`), for the read-only state to name.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

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
    _file: File,
    dir: PathBuf,
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
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(LOCK))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(Err(owner(dir))),
        Err(TryLockError::Error(e)) => return Err(e),
    }
    let me = Owner { program: program.to_string(), pid: std::process::id(), since: super::now_ms() };
    // Only a note for others: a failed write keeps the lock.
    let _ = ob_core::platform::fs::write_atomic(&dir.join(OWNER), me.write().as_bytes(), false);
    Ok(Ok(WriteLock { _file: file, dir: dir.to_path_buf() }))
}

/// Who says they hold the lock of `dir` (only meaningful while it is held).
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
}
