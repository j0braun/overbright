//! The game's files, found the way the original finds them: by name,
//! without regard to case (docs/platform-dirs.md).
//!
//! SPEED2.EXE opens its files through Win32, which matches names without
//! case on NTFS, and asks for upper-case names (`LANGUAGES\ENGLISH.BIN`).
//! Installs spell them in many ways (`LANGUAGES/English.bin` on the
//! reference install), and Linux file systems match case exactly, so every
//! game-file path under the game folder is built here:
//!
//! * [`path`] — `root` + a relative name such as `"GLOBAL/GlobalB.lzc"`,
//!   resolved to the file's real spelling. `/` and `\` both separate.
//! * [`find`] — the same, `None` when there is no such file.
//! * [`read`] — the file's bytes.
//!
//! Rules:
//! * The exact spelling is tried first: when it exists the result is
//!   `root.join(rel)` itself (one `stat`, which the open that follows needs
//!   anyway; [`read`] does not even stat — it opens, and resolves only when
//!   that open finds nothing).
//! * Otherwise each component is matched against its directory's names
//!   without ASCII case (a component spelled exactly is still taken as it
//!   is). A directory is listed once per process and the listing is kept
//!   (the game folder is only read, never written). Two names differing
//!   only in case (possible on Linux): the exact one, else the smallest.
//! * No match: the literal path `root.join(rel)` (so messages name the
//!   file as asked for; opening it fails as before).
//! * The name never leaves `root`: an empty, `.` or `..` component, a
//!   leading separator or a drive / prefix (`C:`, `\\?\`) is refused —
//!   [`path`] then gives a name that cannot exist under `root` and logs it.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Why a name was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// `..`, `.` or an empty component, a leading separator, or a component
    /// with a drive or prefix (`:`): it could name a file outside the root.
    Escapes(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Escapes(rel) => write!(f, "game file name {rel:?} leaves the game folder"),
        }
    }
}

/// The components of `rel`, checked ([`Refused`]).
pub fn components(rel: &str) -> Result<Vec<&str>, Refused> {
    let refuse = || Refused::Escapes(rel.to_string());
    if rel.starts_with(['/', '\\']) {
        return Err(refuse());
    }
    if rel.is_empty() {
        return Ok(Vec::new());
    }
    let parts: Vec<&str> = rel.split(['/', '\\']).collect();
    for (i, p) in parts.iter().enumerate() {
        // A trailing separator ("TRACKS/") names the directory.
        if p.is_empty() && i + 1 == parts.len() && i > 0 {
            continue;
        }
        if p.is_empty() || *p == "." || *p == ".." || p.contains(':') || p.contains('\0') {
            return Err(refuse());
        }
    }
    Ok(parts.into_iter().filter(|p| !p.is_empty()).collect())
}

/// `root` joined with `rel` as it is spelled (`\` read as `/` off Windows).
fn literal(root: &Path, rel: &str) -> PathBuf {
    if cfg!(windows) || !rel.contains('\\') { root.join(rel) } else { root.join(rel.replace('\\', "/")) }
}

/// A directory's names by their lower-case form: the exact name when only
/// one, else every spelling (sorted).
type Listing = HashMap<String, Vec<OsString>>;

/// Per directory: its listing, when it was read, and the directory's
/// modification time then.
type Kept = (Arc<Listing>, std::time::Instant, Option<std::time::SystemTime>);

static LISTINGS: Mutex<Option<HashMap<PathBuf, Kept>>> = Mutex::new(None);

/// How many directories have been listed (tests).
static LISTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A kept listing is read again after a miss when the directory's
/// modification time has changed since, or when it is at least this old
/// (file systems with coarse times): a file added to the folder while the
/// program runs (the launcher's "check again" after the player copied a
/// missing file) is found, and a name that really is not there costs one
/// `stat` per miss and one listing per directory per second at most.
const RELIST_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

/// `dir`'s listing: the kept one, or (`fresh`: when the directory changed
/// since or the kept one is older than [`RELIST_AFTER`]) a new one.
fn listing(dir: &Path, fresh: bool) -> Option<Arc<Listing>> {
    let modified = |d: &Path| std::fs::metadata(d).and_then(|m| m.modified()).ok();
    let kept = LISTINGS.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(dir).cloned();
    if let Some((l, at, time)) = kept
        && !(fresh && (at.elapsed() >= RELIST_AFTER || modified(dir) != time))
    {
        return Some(l);
    }
    let time = modified(dir);
    // Listed outside the lock (directories with thousands of entries);
    // two threads listing the same directory keep the later result.
    let mut l = Listing::new();
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name();
        l.entry(name.to_string_lossy().to_ascii_lowercase()).or_default().push(name);
    }
    for v in l.values_mut() {
        v.sort();
    }
    LISTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let l = Arc::new(l);
    let mut cache = LISTINGS.lock().unwrap_or_else(PoisonError::into_inner);
    cache.get_or_insert_with(HashMap::new).insert(dir.to_path_buf(), (l.clone(), std::time::Instant::now(), time));
    Some(l)
}

/// `rel` under `root` in its real spelling, if there is such a file or
/// directory; `Err` when the name is refused.
pub fn resolve(root: &Path, rel: &str) -> Result<Option<PathBuf>, Refused> {
    let parts = components(rel)?;
    let direct = literal(root, rel);
    if direct.exists() {
        return Ok(Some(direct));
    }
    Ok(walk(root, &parts))
}

/// The case-blind walk, component by component.
fn walk(root: &Path, parts: &[&str]) -> Option<PathBuf> {
    let mut p = root.to_path_buf();
    for part in parts {
        let key = part.to_ascii_lowercase();
        let mut names = listing(&p, false)?;
        if !names.contains_key(&key) {
            // Not in the kept listing: the folder may have changed since.
            names = listing(&p, true)?;
        }
        let spellings = names.get(&key)?;
        let name = spellings.iter().find(|n| n.to_str() == Some(part)).unwrap_or(&spellings[0]);
        p.push(name);
    }
    Some(p)
}

/// The name [`path`] gives for a refused one: `<` and `>` make it invalid
/// on Windows, and the read-only game folder has no such file elsewhere.
const REFUSED: &str = "<refused game file name>";

/// `rel` (e.g. `"GLOBAL/GlobalB.lzc"`, `"LANGUAGES\\ENGLISH.BIN"`) under
/// the game folder `root`, in the spelling the folder has; the literal
/// `root.join(rel)` when there is no such file (see the module's rules).
pub fn path(root: &Path, rel: &str) -> PathBuf {
    match resolve(root, rel) {
        Ok(Some(p)) => p,
        Ok(None) => literal(root, rel),
        Err(e) => {
            log::warn!("{e}");
            root.join(REFUSED)
        }
    }
}

/// `rel` under `root` in its real spelling, `None` when there is no such
/// file or directory (or the name is refused).
pub fn find(root: &Path, rel: &str) -> Option<PathBuf> {
    resolve(root, rel).ok().flatten()
}

/// The bytes of the game file `rel` under `root` (as `std::fs::read` of
/// [`path`]): the exact spelling is opened first, so a matching install
/// costs nothing extra; the error is the literal path's.
pub fn read(root: &Path, rel: &str) -> std::io::Result<Vec<u8>> {
    #[cfg(any(test, feature = "dev-harness"))]
    READS.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut() {
            v.push(rel.to_string());
        }
    });
    let refused = |e: Refused| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string());
    let parts = components(rel).map_err(refused)?;
    let direct = literal(root, rel);
    match std::fs::read(&direct) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match walk(root, &parts) {
            Some(p) if p != direct => std::fs::read(p),
            _ => Err(e),
        },
        r => r,
    }
}

#[cfg(any(test, feature = "dev-harness"))]
thread_local! {
    /// The files [`read`] read on this thread, as asked, while recording
    /// ([`record_reads`]; the read-once tests).
    static READS: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Tests (developer harness): from now on, note every file [`read`] reads
/// on this thread ([`recorded_reads`]); the notes so far are dropped.
#[cfg(any(test, feature = "dev-harness"))]
pub fn record_reads() {
    READS.with(|r| *r.borrow_mut() = Some(Vec::new()));
}

/// The files [`read`] read on this thread since [`record_reads`], as asked.
#[cfg(any(test, feature = "dev-harness"))]
pub fn recorded_reads() -> Vec<String> {
    READS.with(|r| r.borrow().clone().unwrap_or_default())
}

/// [`read`] as text (UTF-8).
pub fn read_to_string(root: &Path, rel: &str) -> std::io::Result<String> {
    String::from_utf8(read(root, rel)?).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests;
