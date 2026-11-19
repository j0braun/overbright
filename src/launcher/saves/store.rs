//! The Saves page's changes to Overbright's profiles: launcher backups
//! (snapshots), restore, rename, duplicate, delete and undo.
//!
//! The rules (docs/launcher.md, "Saves"):
//! * Nothing is overwritten. Every file is written beside its place as
//!   `<file>.part`, checked (SHA-256 against the bytes meant), and then
//!   linked into a path that did not exist ([`write_new`]); the one
//!   replace is the asked "Replace", after its snapshot.
//! * Copies are byte for byte; only a new name (an import's or restore's
//!   copy, a rename, a duplicate) changes the payload's 32 name bytes and
//!   the container's checksums (`NativeSave::set_name`, `to_bytes`).
//! * Every destructive step takes a snapshot first; a failed snapshot
//!   cancels the step.
//! * A step that fails half way removes what it wrote (the profile folder
//!   never shows half a profile).
//!
//! Snapshots sit beside the profiles, never in their folder: the original
//! lists every entry there as a profile (FindFirstFileA "\*", 0x6FC01E).
//! ```text
//! <data home>/NFS Underground 2 Backups/<NAME>/
//!   <NAME>.20261119-101500             the game's own Save Backups (files)
//!   launcher-20261119-101500[-n]/      a launcher snapshot (a folder)
//!     NAME/NAME, NAME DVD 1/NAME DVD 1  byte copies, the original layout
//!     backup.txt                        why, from which name, when, sha256 size path
//! ```
//! The game's pruning (`qol::saves::backup`, keep N) lists files only, so
//! it never touches a snapshot; snapshots are kept until the player
//! deletes them.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use super::summary::{covers_of, dirs_in};
use crate::formats::native_save::NativeSave;

/// The profiles folder and the backups folder beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub profiles: PathBuf,
    pub backups: PathBuf,
}

impl Paths {
    /// The player's: `profiles::root()` and `qol::saves::backup_root()`
    /// (both follow `platform::dirs`: portable mode, locations.txt).
    pub fn player() -> Paths {
        Paths { profiles: crate::frontend::profiles::root(), backups: crate::qol::saves::backup_root() }
    }

    /// For tests: `<base>/NFS Underground 2` and its Backups beside it.
    pub fn under(base: &Path) -> Paths {
        let f = crate::frontend::profiles::FOLDER;
        Paths { profiles: base.join(f), backups: base.join(format!("{f} Backups")) }
    }
}

/// Delete takes the profile's saved covers with it (they belong to it;
/// the snapshot taken first holds them and Undo brings them back).
pub const DELETE_COVERS: bool = true;

/// Profile names: letters and digits, 1 to 7 of them (the name editor's
/// rule, 0x551280: kind 6 takes a–z, A–Z and digits, up to +0x20C = 7).
pub const NAME_MAX: usize = 7;

pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= NAME_MAX && s.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn sha256(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
thread_local! {
    /// Tests: the n-th [`write_new`] from now fails (0 = the next one).
    pub static FAIL_AT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

fn part_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// Writes `bytes` to `path`, which must not exist: to `<path>.part` first,
/// read back and compared by SHA-256, the time set to `mtime` (the
/// source's), then linked into place (a hard link fails when the name is
/// taken, so a file that appeared meanwhile is never replaced; where the
/// file system has no links, a rename after a check). The folder is made
/// when missing.
pub fn write_new(path: &Path, bytes: &[u8], mtime: Option<SystemTime>) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    #[cfg(test)]
    if FAIL_AT.with(|f| match f.get() {
        Some(0) => {
            f.set(None);
            true
        }
        Some(n) => {
            f.set(Some(n - 1));
            false
        }
        None => false,
    }) {
        return Err(Error::other("a test's injected failure"));
    }
    if path.exists() {
        return Err(Error::new(ErrorKind::AlreadyExists, format!("{} is already there", path.display())));
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let part = part_of(path);
    let _ = std::fs::remove_file(&part);
    let r = (|| {
        {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&part)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            if let Some(t) = mtime {
                f.set_modified(t)?;
            }
        }
        if sha256(&std::fs::read(&part)?) != sha256(bytes) {
            return Err(Error::other(format!("{}: the copy differs from what was written", part.display())));
        }
        match std::fs::hard_link(&part, path) {
            Ok(()) => {
                std::fs::remove_file(&part)?;
                Ok(())
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(e),
            Err(_) if path.exists() => {
                Err(Error::new(ErrorKind::AlreadyExists, format!("{} is already there", path.display())))
            }
            Err(_) => std::fs::rename(&part, path),
        }
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    r
}

/// What a step wrote, removed again when the step fails.
#[derive(Default)]
pub struct Undoer {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl Undoer {
    pub fn write(&mut self, path: &Path, bytes: &[u8], mtime: Option<SystemTime>) -> Result<(), String> {
        if let Some(d) = path.parent()
            && !d.exists()
        {
            self.dirs.push(d.to_path_buf());
        }
        write_new(path, bytes, mtime).map_err(|e| format!("{}: {e}", path.display()))?;
        self.files.push(path.to_path_buf());
        Ok(())
    }

    /// Takes back every file and folder written.
    pub fn roll_back(self) {
        for f in self.files.iter().rev() {
            let _ = std::fs::remove_file(f);
        }
        for d in self.dirs.iter().rev() {
            let _ = std::fs::remove_dir(d);
        }
    }
}

/// The time a file was last changed.
pub fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

/// A profile's files: the profile file (folder, file name, bytes, time),
/// then its covers'.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub profile: FileCopy,
    pub covers: Vec<FileCopy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCopy {
    /// The folder's name (= the file's).
    pub name: String,
    pub bytes: Vec<u8>,
    pub mtime: Option<SystemTime>,
}

impl Files {
    /// Profile `name` (a folder under `root`, found as the game finds it)
    /// and, with `covers`, its saved covers.
    pub fn read(root: &Path, name: &str, covers: bool) -> Result<Files, String> {
        let file = crate::frontend::profiles::find_in(root, name);
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let dir = crate::frontend::profiles::resolve_in(root, name);
        let mut out = Files { profile: FileCopy { name: dir.clone(), bytes, mtime: mtime(&file) }, covers: Vec::new() };
        if covers {
            let dirs = dirs_in(root);
            for c in covers_of(&dir, &dirs) {
                let f = crate::frontend::profiles::find_in(root, c);
                if let Ok(b) = std::fs::read(&f) {
                    out.covers.push(FileCopy { name: c.clone(), bytes: b, mtime: mtime(&f) });
                }
            }
        }
        Ok(out)
    }

    /// The same files under a new profile name: the payload's name set
    /// (and the checksums), the covers' folders renamed. A file the game
    /// can't load can't be renamed.
    pub fn renamed(&self, to: &str) -> Result<Files, String> {
        let mut s =
            NativeSave::parse(&self.profile.bytes).map_err(|e| format!("it can't be loaded by the game ({e})"))?;
        s.set_name(to);
        let from = &self.profile.name;
        Ok(Files {
            profile: FileCopy { name: to.to_string(), bytes: s.to_bytes(), mtime: self.profile.mtime },
            covers: self
                .covers
                .iter()
                .map(|c| FileCopy { name: format!("{to}{}", &c.name[from.len()..]), ..c.clone() })
                .collect(),
        })
    }

    /// Writes them all under `root` as new files (none may exist); on a
    /// failure nothing of them is left.
    pub fn write_all(&self, root: &Path) -> Result<(), String> {
        let mut u = Undoer::default();
        // The covers first and the profile last: until the profile file is
        // there, nothing shows as a profile.
        for f in self.covers.iter().chain(std::iter::once(&self.profile)) {
            if let Err(e) = u.write(&root.join(&f.name).join(&f.name), &f.bytes, f.mtime) {
                u.roll_back();
                return Err(e);
            }
        }
        Ok(())
    }

    /// Every folder name this set would take.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.profile.name.as_str()).chain(self.covers.iter().map(|c| c.name.as_str()))
    }
}

/// A name for a copy of `name` that nothing under `taken` has (any case):
/// `NAME2`, `NAME3`, …, trimmed so it stays within 7 letters when `name`
/// was, and its cover folders free too.
pub fn copy_name(name: &str, taken: &[String], covers: &[String]) -> String {
    let clean: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
    let clean = if clean.is_empty() { "NAME".to_string() } else { clean };
    let max = if clean.len() <= NAME_MAX { NAME_MAX } else { usize::MAX };
    let free = |n: &str| {
        !taken.iter().any(|t| t.eq_ignore_ascii_case(n))
            && covers.iter().all(|c| {
                let tail = c.split_once(' ').map_or("", |x| x.1);
                !taken.iter().any(|t| t.eq_ignore_ascii_case(&format!("{n} {tail}")))
            })
    };
    for k in 2u32.. {
        let suf = k.to_string();
        let keep = clean.len().min(max.saturating_sub(suf.len()));
        let cand = format!("{}{suf}", &clean[..keep]);
        if free(&cand) {
            return cand;
        }
    }
    unreachable!()
}

/// Why a snapshot was taken (backup.txt's first line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Manual,
    BeforeDelete,
    BeforeRename,
    BeforeReplace,
    BeforeUndo,
}

impl Why {
    pub fn word(self) -> &'static str {
        match self {
            Why::Manual => "manual",
            Why::BeforeDelete => "before-delete",
            Why::BeforeRename => "before-rename",
            Why::BeforeReplace => "before-replace",
            Why::BeforeUndo => "before-undo",
        }
    }

    pub fn parse(s: &str) -> Option<Why> {
        [Why::Manual, Why::BeforeDelete, Why::BeforeRename, Why::BeforeReplace, Why::BeforeUndo]
            .into_iter()
            .find(|w| w.word() == s)
    }

    /// The backups list's words.
    pub fn shown(self) -> &'static str {
        match self {
            Why::Manual => "Backup",
            Why::BeforeDelete => "Before delete",
            Why::BeforeRename => "Before rename",
            Why::BeforeReplace => "Before replace",
            Why::BeforeUndo => "Before undo",
        }
    }
}

pub fn stamp_now() -> String {
    crate::qol::saves::timestamp(SystemTime::now())
}

/// A snapshot of profile `name` and its covers into
/// `<backups>/<NAME>/launcher-<stamp>[-n]/`: written into a `.part` folder,
/// then renamed into place. Returns the snapshot's folder.
pub fn snapshot(p: &Paths, name: &str, why: Why) -> Result<PathBuf, String> {
    let files = Files::read(&p.profiles, name, true)?;
    snapshot_files(p, &files, why)
}

fn snapshot_files(p: &Paths, files: &Files, why: Why) -> Result<PathBuf, String> {
    let name = &files.profile.name;
    let dir = p.backups.join(name);
    let stamp = stamp_now();
    let mut to = dir.join(format!("launcher-{stamp}"));
    let mut n = 1;
    while to.exists() || part_of(&to).exists() {
        n += 1;
        to = dir.join(format!("launcher-{stamp}-{n}"));
    }
    let part = part_of(&to);
    let r = (|| {
        let mut manifest = format!("{}\nfrom {name}\nutc {stamp}\n", why.word());
        for f in std::iter::once(&files.profile).chain(&files.covers) {
            let rel = format!("{}/{}", f.name, f.name);
            write_new(&part.join(&f.name).join(&f.name), &f.bytes, f.mtime)
                .map_err(|e| format!("the backup could not be written: {e}"))?;
            manifest += &format!("{} {} {rel}\n", sha256(&f.bytes), f.bytes.len());
        }
        write_new(&part.join("backup.txt"), manifest.as_bytes(), None)
            .map_err(|e| format!("the backup could not be written: {e}"))?;
        std::fs::rename(&part, &to).map_err(|e| format!("the backup could not be written: {e}"))
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e);
    }
    Ok(to)
}

/// A backup of a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backup {
    /// The snapshot's folder, or the game's backup file.
    pub path: PathBuf,
    /// The profile's name it was taken of.
    pub name: String,
    /// `YYYYMMDD-HHMMSS` (UTC).
    pub stamp: String,
    /// A launcher snapshot (why), else the game's own Save Backups file.
    pub why: Option<Why>,
    pub covers: usize,
}

impl Backup {
    /// When it was taken, as the player reads times: local, in the
    /// system's form (`launcher::localtime`); the stamp itself is UTC.
    pub fn when(&self) -> String {
        crate::launcher::localtime::from_stamp(&self.stamp)
            .map_or_else(|| self.stamp.clone(), crate::launcher::localtime::date_time)
    }

    /// The backup's files (the game's backup: the profile file alone).
    pub fn files(&self) -> Result<Files, String> {
        if self.why.is_none() {
            let bytes = std::fs::read(&self.path).map_err(|e| format!("{}: {e}", self.path.display()))?;
            return Ok(Files {
                profile: FileCopy { name: self.name.clone(), bytes, mtime: mtime(&self.path) },
                covers: Vec::new(),
            });
        }
        let mut f = Files::read(&self.path, &self.name, true)?;
        f.profile.name = self.name.clone();
        Ok(f)
    }
}

/// The backups of profile `name` (any case), the newest first.
pub fn backups(p: &Paths, name: &str) -> Vec<Backup> {
    let mut v = Vec::new();
    for d in dirs_in(&p.backups).into_iter().filter(|d| d.eq_ignore_ascii_case(name)) {
        let dir = p.backups.join(&d);
        for f in crate::qol::saves::list(&dir, &d) {
            let stamp = f.file_name().and_then(|n| n.to_str()).map_or(String::new(), |n| n[d.len() + 1..].to_string());
            v.push(Backup { path: f, name: d.clone(), stamp, why: None, covers: 0 });
        }
        for s in dirs_in(&dir) {
            let Some(stamp) = s.strip_prefix("launcher-").filter(|x| !x.ends_with(".part")) else { continue };
            let snap = dir.join(&s);
            let manifest = std::fs::read_to_string(snap.join("backup.txt")).unwrap_or_default();
            let mut lines = manifest.lines();
            let why = lines.next().and_then(Why::parse).unwrap_or(Why::Manual);
            let from = lines.next().and_then(|l| l.strip_prefix("from ")).unwrap_or(&d).to_string();
            let inner = dirs_in(&snap);
            let covers = covers_of(&from, &inner).len();
            if crate::frontend::profiles::find_in(&snap, &from).is_file() {
                v.push(Backup { path: snap, name: from, stamp: stamp.to_string(), why: Some(why), covers });
            }
        }
    }
    v.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    v
}

/// Every profile's backups (for the page), by the backups folder's names.
pub fn all_backups(p: &Paths) -> Vec<Backup> {
    let mut v: Vec<Backup> = dirs_in(&p.backups).iter().flat_map(|d| backups(p, d)).collect();
    v.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    v.dedup_by(|a, b| a.path == b.path);
    v
}

/// Left-over `.part` files and folders of an interrupted write, removed
/// (in the profiles' and the backups' folders, one level of folders deep).
pub fn clean_parts(p: &Paths) {
    for root in [&p.profiles, &p.backups] {
        for d in dirs_in(root) {
            let dir = root.join(&d);
            if d.ends_with(".part") {
                continue;
            }
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.ends_with(".part") {
                    if e.path().is_dir() {
                        let _ = std::fs::remove_dir_all(e.path());
                    } else {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
    }
}

/// The names in the profiles folder (folders of profiles and covers).
pub fn taken(p: &Paths) -> Vec<String> {
    dirs_in(&p.profiles)
}

/// What Undo does: removes a profile (the copy a step made), then puts a
/// snapshot back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Undo {
    pub remove: Option<String>,
    pub restore: Option<PathBuf>,
    /// The bar's words ("Deleted NIGHTOWL").
    pub said: String,
}

/// Removes profile `name`'s file and folder and, with `covers`, its saved
/// covers (each folder only when it is empty then).
fn remove(p: &Paths, name: &str, covers: bool) -> Result<(), String> {
    let dir = crate::frontend::profiles::resolve_in(&p.profiles, name);
    let file = crate::frontend::profiles::find_in(&p.profiles, &dir);
    let mut gone = vec![(file, p.profiles.join(&dir))];
    if covers {
        let dirs = dirs_in(&p.profiles);
        for c in covers_of(&dir, &dirs) {
            gone.push((crate::frontend::profiles::find_in(&p.profiles, c), p.profiles.join(c)));
        }
    }
    for (f, d) in gone {
        std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let _ = std::fs::remove_dir(&d);
    }
    Ok(())
}

fn exists(p: &Paths, name: &str) -> bool {
    crate::frontend::profiles::find_in(&p.profiles, name).is_file()
}

/// Delete: a snapshot (with the covers), then the files.
pub fn delete(p: &Paths, name: &str, covers: bool) -> Result<Undo, String> {
    let snap = snapshot(p, name, Why::BeforeDelete)?;
    remove(p, name, covers)?;
    Ok(Undo { remove: None, restore: Some(snap), said: format!("Deleted {name}") })
}

/// Rename: a snapshot, the files written under the new name (payload name
/// too), then the old ones removed.
pub fn rename(p: &Paths, from: &str, to: &str) -> Result<Undo, String> {
    if !valid_name(to) {
        return Err("a name is 1 to 7 letters or digits".into());
    }
    if from.eq_ignore_ascii_case(to) {
        return Err("that is the same name".into());
    }
    let have = taken(p);
    let files = Files::read(&p.profiles, from, true)?;
    let new = files.renamed(to)?;
    if let Some(n) = new.names().find(|n| have.iter().any(|h| h.eq_ignore_ascii_case(n))) {
        return Err(format!("{n} is already taken"));
    }
    let snap = snapshot_files(p, &files, Why::BeforeRename)?;
    new.write_all(&p.profiles)?;
    remove(p, from, true)?;
    Ok(Undo { remove: Some(to.to_string()), restore: Some(snap), said: format!("Renamed {from} to {to}") })
}

/// Duplicate: a copy under a free name (`copy_name`), covers included.
pub fn duplicate(p: &Paths, name: &str) -> Result<(String, Undo), String> {
    let files = Files::read(&p.profiles, name, true)?;
    let cover_names: Vec<String> = files.covers.iter().map(|c| c.name.clone()).collect();
    let to = copy_name(&files.profile.name, &taken(p), &cover_names);
    files.renamed(&to)?.write_all(&p.profiles)?;
    Ok((to.clone(), Undo { remove: Some(to.clone()), restore: None, said: format!("Made {to}, a copy of {name}") }))
}

/// How a backup comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// Under its own name (no profile of that name is there).
    Back,
    /// Under a free name (`copy_name`).
    AsCopy,
    /// Over the profile of that name, which is backed up first.
    Replace,
}

/// Restores `b`; returns the name it came back as.
pub fn restore(p: &Paths, b: &Backup, how: How) -> Result<String, String> {
    let files = b.files()?;
    let name = files.profile.name.clone();
    match how {
        How::Back if exists(p, &name) => Err(format!("{name} is already there")),
        How::Back => {
            let have = taken(p);
            // Covers of the same name already there (from a newer play)
            // stay; the backup's are left out then.
            let mut f = files.clone();
            f.covers.retain(|c| !have.iter().any(|h| h.eq_ignore_ascii_case(&c.name)));
            f.write_all(&p.profiles)?;
            Ok(name)
        }
        How::AsCopy => {
            let cover_names: Vec<String> = files.covers.iter().map(|c| c.name.clone()).collect();
            let to = copy_name(&name, &taken(p), &cover_names);
            files.renamed(&to)?.write_all(&p.profiles)?;
            Ok(to)
        }
        How::Replace => {
            if exists(p, &name) {
                let current = Files::read(&p.profiles, &name, b.why.is_some())?;
                snapshot_files(p, &current, Why::BeforeReplace)?;
                // The game's backups hold the profile file alone: the
                // covers stay as they are then.
                remove(p, &name, b.why.is_some())?;
                let dir = current.profile.name.clone();
                let mut f = files.clone();
                f.profile.name = dir;
                if let Err(e) = f.write_all(&p.profiles) {
                    // Put the current one back.
                    let _ = current.write_all(&p.profiles);
                    return Err(e);
                }
                Ok(name)
            } else {
                restore(p, b, How::Back)
            }
        }
    }
}

/// Undo: the copy removed (after a snapshot of it), the snapshot put back
/// (under its name, or as a copy when the name was taken meanwhile).
pub fn undo(p: &Paths, u: &Undo) -> Result<String, String> {
    if let Some(r) = &u.remove
        && exists(p, r)
    {
        snapshot(p, r, Why::BeforeUndo)?;
        remove(p, r, true)?;
    }
    let Some(snap) = &u.restore else { return Ok(String::new()) };
    let b = backups_at(snap).ok_or_else(|| format!("{}: the backup is gone", snap.display()))?;
    let how = if exists(p, &b.name) { How::AsCopy } else { How::Back };
    restore(p, &b, how)
}

/// The snapshot in folder `snap`.
pub fn backups_at(snap: &Path) -> Option<Backup> {
    let manifest = std::fs::read_to_string(snap.join("backup.txt")).ok()?;
    let mut lines = manifest.lines();
    let why = lines.next().and_then(Why::parse)?;
    let name = lines.next()?.strip_prefix("from ")?.to_string();
    let stamp = snap.file_name()?.to_str()?.strip_prefix("launcher-")?.to_string();
    let covers = covers_of(&name, &dirs_in(snap)).len();
    Some(Backup { path: snap.to_path_buf(), name, stamp, why: Some(why), covers })
}

/// Deletes a backup (asked first on the page): a snapshot folder or the
/// game's backup file. Only paths inside the backups folder.
pub fn delete_backup(p: &Paths, b: &Backup) -> Result<(), String> {
    let inside = b.path.starts_with(&p.backups) && b.path != p.backups;
    if !inside {
        return Err("not one of Overbright's backups".into());
    }
    let r = if b.path.is_dir() { std::fs::remove_dir_all(&b.path) } else { std::fs::remove_file(&b.path) };
    r.map_err(|e| format!("{}: {e}", b.path.display()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::formats::native_save as ns;

    fn base(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-saves-{tag}-{}", crate::unique_tag()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    pub(crate) fn profile(name: &str, cash: u32) -> Vec<u8> {
        let mut s = NativeSave { payload: vec![0; ns::PAYLOAD_LEN] };
        s.set_name(name);
        s.set_u32(ns::CASH, cash);
        s.to_bytes()
    }

    fn put(root: &Path, name: &str, b: &[u8]) {
        std::fs::create_dir_all(root.join(name)).unwrap();
        std::fs::write(root.join(name).join(name), b).unwrap();
    }

    /// Every file under `d`: (path, bytes, mtime).
    pub(crate) fn tree(d: &Path) -> Vec<(PathBuf, Vec<u8>, Option<SystemTime>)> {
        let mut v = Vec::new();
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                v.extend(tree(&e.path()));
            } else {
                v.push((e.path(), std::fs::read(e.path()).unwrap(), mtime(&e.path())));
            }
        }
        v.sort();
        v
    }

    #[test]
    fn names() {
        assert!(valid_name("KIMMY2") && valid_name("ABCDEFG") && !valid_name("ABCDEFGH") && !valid_name("A B"));
        assert!(!valid_name(""));
        let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(copy_name("MIKE", &t(&["MIKE"]), &[]), "MIKE2");
        assert_eq!(copy_name("tuner", &t(&["TUNER", "tuner2"]), &[]), "tuner3");
        assert_eq!(copy_name("NIGHTOW", &t(&["NIGHTOW"]), &[]), "NIGHTO2");
        assert_eq!(
            copy_name(
                "ABCDEFG",
                &t(&["ABCDEF2", "ABCDEF3", "ABCDEF4", "ABCDEF5", "ABCDEF6", "ABCDEF7", "ABCDEF8", "ABCDEF9"]),
                &[]
            ),
            "ABCDE10"
        );
        assert_eq!(copy_name("NIGHTOWL", &t(&["NIGHTOWL"]), &[]), "NIGHTOWL2", "a longer name is kept whole");
        // A cover folder of the would-be copy is taken: the next name.
        assert_eq!(copy_name("KIMMY", &t(&["KIMMY", "KIMMY2 DVD 1"]), &t(&["KIMMY DVD 1"])), "KIMMY3");
    }

    #[test]
    fn write_new_never_replaces() {
        let d = base("new");
        let p = d.join("A/A");
        write_new(&p, b"one", None).unwrap();
        let e = write_new(&p, b"two", None).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&p).unwrap(), b"one");
        assert!(!part_of(&p).exists());
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        write_new(&d.join("B/B"), b"x", Some(t)).unwrap();
        assert_eq!(mtime(&d.join("B/B")), Some(t), "the source's time kept");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Snapshot, delete, undo; rename and its undo; duplicate; restore in
    /// each way. Every copy byte for byte, or with only the name changed.
    #[test]
    fn snapshot_delete_rename_restore_undo() {
        let d = base("ops");
        let p = Paths::under(&d);
        let kimmy = profile("KIMMY", 500);
        put(&p.profiles, "KIMMY", &kimmy);
        let cover = ns::picture(&[1, 2, 3, 255], 1, 1);
        put(&p.profiles, "KIMMY DVD 1", &cover);
        let before = tree(&p.profiles);

        let snap = snapshot(&p, "KIMMY", Why::Manual).unwrap();
        assert_eq!(std::fs::read(snap.join("KIMMY/KIMMY")).unwrap(), kimmy);
        assert_eq!(std::fs::read(snap.join("KIMMY DVD 1/KIMMY DVD 1")).unwrap(), cover);
        let manifest = std::fs::read_to_string(snap.join("backup.txt")).unwrap();
        assert!(manifest.starts_with("manual\nfrom KIMMY\n") && manifest.contains(&sha256(&kimmy)), "{manifest}");
        assert_eq!(tree(&p.profiles), before, "a snapshot changes nothing");

        // Delete with its covers, then undo: the tree as it was.
        let u = delete(&p, "KIMMY", true).unwrap();
        assert!(!p.profiles.join("KIMMY").exists() && !p.profiles.join("KIMMY DVD 1").exists());
        assert_eq!(undo(&p, &u).unwrap(), "KIMMY");
        assert_eq!(tree(&p.profiles), before, "undo brings back the same bytes and times");

        // Rename: the payload's name changes and nothing else but the
        // checksums.
        let u = rename(&p, "KIMMY", "RAY").unwrap();
        let ray = std::fs::read(p.profiles.join("RAY/RAY")).unwrap();
        let s = NativeSave::parse(&ray).unwrap();
        assert_eq!(s.name(), "RAY");
        let diff: Vec<usize> = (0..kimmy.len()).filter(|&i| kimmy[i] != ray[i]).collect();
        let name_at = ns::HEADER_LEN + 8 + ns::NAME;
        assert!(diff.iter().all(|&i| i < 0x1C || (name_at..name_at + ns::NAME_LEN).contains(&i)), "{diff:?}");
        assert_eq!(std::fs::read(p.profiles.join("RAY DVD 1/RAY DVD 1")).unwrap(), cover);
        assert!(!p.profiles.join("KIMMY").exists());
        assert!(rename(&p, "RAY", "bad name").is_err());
        undo(&p, &u).unwrap();
        assert_eq!(tree(&p.profiles), before, "rename undone");

        // Duplicate, then delete the copy by undo.
        let (copy, u) = duplicate(&p, "KIMMY").unwrap();
        assert_eq!(copy, "KIMMY2");
        assert_eq!(
            NativeSave::parse(&std::fs::read(p.profiles.join("KIMMY2/KIMMY2")).unwrap()).unwrap().name(),
            "KIMMY2"
        );
        assert!(p.profiles.join("KIMMY2 DVD 1/KIMMY2 DVD 1").is_file());
        undo(&p, &u).unwrap();
        assert_eq!(tree(&p.profiles), before, "duplicate undone");

        // Restore: as a copy, and replacing (the current one backed up).
        let b = backups(&p, "KIMMY").into_iter().find(|b| b.path == snap).unwrap();
        assert_eq!((b.why, b.covers), (Some(Why::Manual), 1));
        assert!(restore(&p, &b, How::Back).is_err(), "never over a profile");
        assert_eq!(restore(&p, &b, How::AsCopy).unwrap(), "KIMMY2");
        std::fs::write(p.profiles.join("KIMMY/KIMMY"), profile("KIMMY", 999)).unwrap();
        assert_eq!(restore(&p, &b, How::Replace).unwrap(), "KIMMY");
        assert_eq!(std::fs::read(p.profiles.join("KIMMY/KIMMY")).unwrap(), kimmy);
        let replaced = backups(&p, "KIMMY").into_iter().find(|b| b.why == Some(Why::BeforeReplace)).unwrap();
        assert_eq!(NativeSave::parse(&replaced.files().unwrap().profile.bytes).unwrap().u32(ns::CASH), 999);

        // The game's own pruning keeps the snapshot folders.
        let game_dir = p.backups.join("KIMMY");
        crate::qol::saves::backup(&p.profiles.join("KIMMY/KIMMY"), &game_dir, 1).unwrap();
        crate::qol::saves::backup(&p.profiles.join("KIMMY/KIMMY"), &game_dir, 1).unwrap();
        assert!(snap.is_dir());
        let all = backups(&p, "KIMMY");
        assert_eq!(all.iter().filter(|b| b.why.is_none()).count(), 1);
        // A game backup restores as a copy, byte for byte but its name.
        let g = all.iter().find(|b| b.why.is_none()).unwrap();
        assert_eq!(restore(&p, g, How::AsCopy).unwrap(), "KIMMY3");
        delete_backup(&p, &b).unwrap();
        assert!(!snap.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A write that fails half way leaves nothing: no profile, no cover,
    /// no .part file.
    #[test]
    fn a_failed_copy_leaves_nothing() {
        let d = base("fail");
        let p = Paths::under(&d);
        put(&p.profiles, "KIMMY", &profile("KIMMY", 1));
        put(&p.profiles, "KIMMY DVD 1", &ns::picture(&[1, 2, 3, 255], 1, 1));
        put(&p.profiles, "KIMMY DVD 2", &ns::picture(&[1, 2, 3, 255], 1, 1));
        let before = tree(&p.profiles);
        // The third write (the profile's, after two covers) fails.
        FAIL_AT.with(|f| f.set(Some(2)));
        assert!(duplicate(&p, "KIMMY").is_err());
        FAIL_AT.with(|f| f.set(None));
        assert_eq!(tree(&p.profiles), before);
        assert_eq!(dirs_in(&p.profiles).len(), 3);
        // A failed snapshot cancels a delete.
        FAIL_AT.with(|f| f.set(Some(0)));
        assert!(delete(&p, "KIMMY", true).is_err());
        FAIL_AT.with(|f| f.set(None));
        assert_eq!(tree(&p.profiles), before, "nothing deleted");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The backups go beside the profiles in the folder `NFSU2_DATA_DIR`
    /// names (a portable or sandboxed set-up), not in the player's.
    #[test]
    fn backups_follow_the_data_folder() {
        let p = Paths::player();
        assert_eq!(p.profiles.parent(), p.backups.parent());
        assert!(p.backups.file_name().unwrap().to_string_lossy().ends_with(" Backups"));
    }

    /// The SPEED2.EXE fixtures renamed: they parse, carry the new name, and
    /// differ from the original only in the name bytes and checksums.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn fixtures_renamed_differ_only_in_the_name() {
        for f in ["career_start.sav", "garage.sav", "career_complete.sav", "options.sav"] {
            let b = crate::testkit::private(&format!("tests/fixtures/native_saves/{f}"));
            let files =
                Files { profile: FileCopy { name: "NAME".into(), bytes: b.clone(), mtime: None }, covers: vec![] };
            let r = files.renamed("MIKE2").unwrap().profile.bytes;
            let s = NativeSave::parse(&r).unwrap();
            assert_eq!(s.name(), "MIKE2");
            let name_at = ns::HEADER_LEN + 8 + ns::NAME;
            let diff: Vec<usize> = (0..b.len()).filter(|&i| b[i] != r[i]).collect();
            assert!(diff.iter().all(|&i| i < 0x1C || (name_at..name_at + ns::NAME_LEN).contains(&i)), "{f}: {diff:?}");
            // Renamed back: the original file, byte for byte.
            let back = Files { profile: FileCopy { name: "MIKE2".into(), bytes: r, mtime: None }, covers: vec![] };
            let orig = NativeSave::parse(&b).unwrap().name();
            assert_eq!(back.renamed(&orig).unwrap().profile.bytes, b, "{f}");
        }
    }
}
