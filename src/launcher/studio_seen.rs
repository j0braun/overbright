//! What the launcher sees of Studio without starting anything: which map mods a live program has open for writing
//! (the write lock Studio and its command-line tool hold on the mod's `.editor/studio.lock`, and who holds it from
//! `studio-owner.toml` beside it) and the last mod opened (the recent list Studio's World workspace and the map
//! editor keep, `editor-recent.txt`). Everything here only reads: no lock is taken (Linux asks with `F_GETLK`; on
//! Windows the lock is tried and let go at once, as Studio's own look at a mod does), no file is written.
//!
//! The file names and the note's form are Studio's (`ob_studio::history::lock`, the editor's recent list); a test
//! in that crate holds a lock from another process and checks this module sees it.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// The lock file, in the mod's editor folder (`ob_studio::history::lock::LOCK`).
pub const LOCK: &str = "studio.lock";
/// Who holds the lock (`ob_studio::history::lock::OWNER`).
pub const OWNER: &str = "studio-owner.toml";
/// The recently opened mods' list, newest first, in Overbright's own folder (the editor's `RECENT_FILE`).
pub const RECENT_FILE: &str = "editor-recent.txt";
/// The name Studio and the map editor write in the note (`ob_studio::history::STUDIO`).
pub const STUDIO: &str = "Overbright Studio";

/// A map mod a live program has open for writing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    /// The mod's folder.
    pub folder: String,
    /// The program's name, its process and when it took the lock (Unix time, ms), as its note says (None: no
    /// readable note).
    pub program: Option<String>,
    pub pid: Option<u32>,
    pub since: Option<u64>,
}

impl Held {
    /// Held by Studio (or the map editor, which writes the same name), not by the command-line tool.
    pub fn by_studio(&self) -> bool {
        self.program.as_deref() == Some(STUDIO)
    }
}

/// What the launcher sees of Studio.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StudioSeen {
    /// The mods whose lock a live program holds, in the mods' order.
    pub held: Vec<Held>,
    /// The last mod opened in Studio or the map editor (its folder).
    pub last: Option<String>,
}

impl StudioSeen {
    /// The lock on `folder`, when a live program holds it.
    pub fn held_on(&self, folder: &str) -> Option<&Held> {
        self.held.iter().find(|h| h.folder == folder)
    }
}

/// Reads what is seen of Studio for the map mods `folders` in `maps`, and the recent list `recent`.
pub fn read(maps: &Path, folders: &[String], recent: &Path) -> StudioSeen {
    let held = folders
        .iter()
        .filter_map(|f| {
            let edir = maps.join(f).join(crate::tools::map::store::EDITOR_DIR);
            if !held_by_another(&edir) {
                return None;
            }
            let note = std::fs::read_to_string(edir.join(OWNER)).ok().and_then(|t| read_note(&t));
            let (program, pid, since) = match note {
                Some((p, pid, s)) => (Some(p), Some(pid), Some(s)),
                None => (None, None, None),
            };
            Some(Held { folder: f.clone(), program, pid, since })
        })
        .collect();
    let last = std::fs::read_to_string(recent).ok().and_then(|t| first_recent(&t));
    StudioSeen { held, last }
}

/// The owner note's program, process and time (`program = "…"`, `pid = …`, `since = …`).
pub fn read_note(text: &str) -> Option<(String, u32, u64)> {
    let t: toml::Table = toml::from_str(text).ok()?;
    Some((
        t.get("program")?.as_str()?.to_string(),
        u32::try_from(t.get("pid")?.as_integer()?).ok()?,
        u64::try_from(t.get("since")?.as_integer()?).ok()?,
    ))
}

/// The recent list's first mod folder (lines that are not a mod folder's name are skipped, as the editor does).
pub fn first_recent(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| crate::tools::map::format::valid_id(l)).map(str::to_string)
}

/// Whether another live process holds the lock in the editor folder `edir` (a lock left by a process that ended
/// is free: the system lets it go). No lock file: free.
fn held_by_another(edir: &Path) -> bool {
    let Ok(file) = OpenOptions::new().read(true).open(edir.join(LOCK)) else { return false };
    // An error: held, as Studio itself would find it.
    probe(&file).unwrap_or(true)
}

/// Whether another process holds the lock on `file`; nothing is taken.
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

/// Whether another handle holds the lock on `file`: tried and let go again at once when free (as Studio's own
/// look at a mod does).
#[cfg(not(unix))]
fn probe(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => {
            file.unlock()?;
            Ok(false)
        }
        Err(std::fs::TryLockError::WouldBlock) => Ok(true),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_and_the_recent_list_are_read_as_studio_writes_them() {
        let note = "# Who has this mod open for writing (only while it holds studio.lock).\nprogram = \"Overbright Studio\"\npid = 4242\nsince = 1700000000000\n";
        assert_eq!(read_note(note), Some((STUDIO.to_string(), 4242, 1_700_000_000_000)));
        assert_eq!(read_note("program = 3"), None);
        assert_eq!(first_recent("\n  \nNot a folder!\ndrift-park\nopen-docks\n").as_deref(), Some("drift-park"));
        assert_eq!(first_recent(""), None);
    }

    #[test]
    fn a_note_without_a_held_lock_is_not_a_holder() {
        let dir = std::env::temp_dir().join(format!("ob-studio-seen-{}", std::process::id()));
        let edir = dir.join("drift-park").join(crate::tools::map::store::EDITOR_DIR);
        std::fs::create_dir_all(&edir).unwrap();
        // A writer that died: its lock file and note stay, the lock is free.
        std::fs::write(edir.join(LOCK), b"").unwrap();
        std::fs::write(edir.join(OWNER), "program = \"Overbright Studio\"\npid = 999999\nsince = 1\n").unwrap();
        let recent = dir.join(RECENT_FILE);
        std::fs::write(&recent, "drift-park\n").unwrap();
        let folders = ["drift-park".to_string(), "none-here".to_string()];
        let seen = read(&dir, &folders, &recent);
        assert_eq!(seen, StudioSeen { held: Vec::new(), last: Some("drift-park".into()) });
        let lock = std::fs::read(edir.join(LOCK)).unwrap();
        assert!(lock.is_empty(), "nothing written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
