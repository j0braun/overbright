use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::current::*;
use super::sanitize;
use crate::formats::native_save::NativeSave;

/// The folder name (0x7F66A0 → 0x78E8F4).
pub const FOLDER: &str = "NFS Underground 2";

/// The clone's %LOCALAPPDATA%: `platform::dirs::data_home` (Linux: the
/// XDG data directory; a test thread's sandbox when it is in one).
pub fn data_home() -> PathBuf {
    crate::platform::dirs::data_home()
}

pub fn root() -> PathBuf {
    data_home().join(FOLDER)
}

pub fn file(name: &str) -> PathBuf {
    root().join(name).join(name)
}

/// The folder a profile name opens, as Windows' case-insensitive file
/// system finds it. The original names the file by the name it saves under
/// (0x4D9AB0 → the library's write 0x6F9BFC: CreateFileA on a temporary
/// file, then MoveFileA onto `<root>\<NAME>\<NAME>`) and the library's
/// own name lists (the file cache's sorted list 0x6F9988, its lookup
/// 0x6F9AE6) compare with _stricmp; Windows' file system ignores case. So
/// "Ray" and "RAY" are one profile. Under Wine (2026-10-30), creating
/// "name" with the profile "NAME" on disk asked "Do you want to overwrite
/// previously saved game?" and Yes wrote into NAME/NAME (the payload's
/// name then "name"; MC_List still showed the folder's "NAME").
///
/// Folders that differ only in case cannot both exist on Windows; on Linux
/// (older clone builds made them) the clone takes the one whose profile
/// file is the newest (then the exact spelling, then name order) for
/// every operation, and never merges or deletes the others (**clone
/// choice**). With no profile file in any of them, the
/// first such folder (exact spelling first), else the name itself (a new
/// folder).
pub fn resolve(name: &str) -> String {
    resolve_in(&root(), name)
}

/// The profile file for `name` found case-insensitively (folder and file;
/// see [`resolve`]); `file(name)` when there is none.
pub fn find(name: &str) -> PathBuf {
    find_in(&root(), name)
}

/// Whether a profile of this name exists, as the original's save sees it
/// (case-insensitively).
pub fn exists(name: &str) -> bool {
    find(name).is_file()
}

/// The profiles on disk, one per name in any case (see [`resolve`]): each
/// the folder holding a file of its own name, in name order (the order
/// the original's list showed them in). MC_List shows the folder's name,
/// as the original did; the current profile's own name (help bar, save
/// question) is its payload's.
pub fn list() -> Vec<String> {
    list_in(&root())
}

pub(super) fn dirs_in(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn modified(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok().filter(|m| m.is_file()).and_then(|m| m.modified().ok())
}

/// The profile file inside folder `dir` (its own name, exact spelling
/// first, else the newest file of that name in any case).
pub(super) fn file_in_dir(root: &Path, dir: &str) -> Option<PathBuf> {
    let exact = root.join(dir).join(dir);
    if exact.is_file() {
        return Some(exact);
    }
    std::fs::read_dir(root.join(dir))
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|f| f.eq_ignore_ascii_case(dir)))
        .filter_map(|e| modified(&e.path()).map(|t| (t, e.path())))
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
        .map(|(_, p)| p)
}

pub fn resolve_in(root: &Path, name: &str) -> String {
    let mut dirs: Vec<String> = dirs_in(root).into_iter().filter(|d| d.eq_ignore_ascii_case(name)).collect();
    // Exact spelling first, then name order: the tie-break.
    dirs.sort_by_key(|d| (d != name, d.to_ascii_uppercase(), d.clone()));
    let newest = dirs
        .iter()
        .enumerate()
        .filter_map(|(i, d)| file_in_dir(root, d).and_then(|f| modified(&f)).map(|t| (t, std::cmp::Reverse(i), d)))
        .max()
        .map(|(_, _, d)| d.clone());
    newest.or_else(|| dirs.first().cloned()).unwrap_or_else(|| name.to_string())
}

pub fn find_in(root: &Path, name: &str) -> PathBuf {
    let dir = resolve_in(root, name);
    file_in_dir(root, &dir).unwrap_or_else(|| root.join(&dir).join(&dir))
}

pub fn list_in(root: &Path) -> Vec<String> {
    let mut keys: Vec<String> = dirs_in(root).iter().map(|d| d.to_ascii_uppercase()).collect();
    keys.sort();
    keys.dedup();
    // A saved cover ("<NAME> DVD 1", [`cover_name`]) is a folder with a
    // file of its own name too; it is not a profile (**inferred**: how the
    // original's list leaves them out is not traced; its files are the
    // picture's size, `PICTURE_FILE_LEN`).
    let picture = |d: &String| {
        file_in_dir(root, d)
            .and_then(|f| std::fs::metadata(f).ok())
            .is_some_and(|m| m.len() == crate::formats::native_save::PICTURE_FILE_LEN as u64)
    };
    let mut v: Vec<String> =
        keys.iter().map(|k| resolve_in(root, k)).filter(|d| file_in_dir(root, d).is_some() && !picture(d)).collect();
    v.sort_by_key(|n| n.to_ascii_uppercase());
    v
}

/// A saved cover's name (**verified**): 0x523F00 hands the memory-card
/// library (0x4A95B0, op 2) the 8-character name 0x510AF0 builds (the
/// profile name padded with '_' to 7, then the milestone's index as a
/// digit or letter), which 0x523CC0 → 0x510B80 shows as "DVD %d"
/// (0x79ACC8) for kinds 2 / 4 or "Magazine %d" (0x79ACBC) for 1 / 3, n
/// the count of that kind's milestones up to and including this one in
/// record order; on the PC the file is `<root>\<NAME> DVD <n>\<NAME> DVD
/// <n>` beside the profile (the original's files, e.g. "<NAME> DVD 1" and
/// "<NAME> Magazine 12"; its "Unable to load NAME DVD 1." under Wine).
pub fn cover_name(profile: &str, dvd: bool, n: usize) -> String {
    format!("{} {} {n}", sanitize(profile), if dvd { "DVD" } else { "Magazine" })
}

/// Write a saved cover's file (`formats::native_save::picture`) under the
/// profiles' root, as the library writes a profile (folder made, a
/// temporary file renamed).
pub fn write_cover(name: &str, file: &[u8]) -> Result<PathBuf> {
    let path = root().join(name).join(name);
    crate::qol::saves::write_atomic(&path, file, crate::qol::durable_saves())?;
    Ok(path)
}

/// A saved cover's picture (`<NAME> Magazine <n>` / `<NAME> DVD <n>` under
/// the profiles' root, found as profiles are, case-insensitively): top-down
/// RGBA, 512 × 512 (`native_save::read_picture`).
pub fn read_cover(name: &str) -> Result<Vec<u8>> {
    let path = find(name);
    let bytes = std::fs::read(&path).with_context(|| format!("{}", path.display()))?;
    crate::formats::native_save::read_picture(&bytes)
}

pub fn read(name: &str) -> Result<NativeSave> {
    let path = find(name);
    let bytes = std::fs::read(&path).with_context(|| format!("{}", path.display()))?;
    NativeSave::parse(&bytes).with_context(|| format!("{}", path.display()))
}

/// Write a profile (the folder is made first, as CreateDirectoryA does).
/// The file is the existing one of any case (see [`resolve`]).
pub fn write(name: &str, save: &NativeSave) -> Result<()> {
    let path = find(name);
    write_to(&path, save)
}

/// The file goes through a temporary file and a rename, as in the
/// original (0x6F9BFC). The Enhancements page adds (clone deviations, off
/// by default): Save Backups, the last N copies of the file before each
/// write in `qol::saves::backup_root()/<NAME>/` (outside the profiles'
/// folder), and Durable Saves, the file and folder flushed to the disk.
/// The bytes written are the native format either way.
pub fn write_to(path: &Path, save: &NativeSave) -> Result<()> {
    let keep = crate::qol::save_backups();
    if keep > 0
        && let Some(name) = path.file_name().and_then(|n| n.to_str())
        && let Err(e) = crate::qol::saves::backup(path, &crate::qol::saves::backup_root().join(name), keep)
    {
        log::warn!("profile backup of {} failed: {e}", path.display());
    }
    crate::qol::saves::write_atomic(path, &save.to_bytes(), crate::qol::durable_saves())?;
    Ok(())
}

/// Delete a profile: its file, then its folder (the library's DeleteFileA
/// and RemoveDirectoryA, 0x760B0C / 0x760AE2), found case-insensitively
/// (see [`resolve`]); other folders differing only in case stay.
pub fn delete(name: &str) -> Result<()> {
    let dir = resolve(name);
    let path = find(&dir);
    std::fs::remove_file(&path)?;
    let _ = std::fs::remove_dir(root().join(&dir));
    if current_name().is_some_and(|c| c.eq_ignore_ascii_case(&dir)) {
        clear_current();
    }
    Ok(())
}
