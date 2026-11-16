//! Eviction: obsolete cache files and the size limit.

use super::*;

/// The surface maps' folder in the cache folder
/// (`render::enh::assets::surface::cache_dir`): its files count against
/// the size limit and are evicted with the others ([`prune`]).
pub const SURFACE_MAPS: &str = "surface-maps";

/// Cache file names that nothing reads (removed when found).
pub(super) const OBSOLETE: [&str; 2] = ["ui2d-vector.nfc", "ui2d-hd.nfc"];

/// [`sweep_tmp`] once per process, at the first cache file opened.
pub(super) fn sweep_once(dir: &Path) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        sweep_tmp(dir);
        // The surface maps' folder (`render::enh::assets::surface`).
        sweep_tmp(&dir.join(SURFACE_MAPS));
        // Files no current code makes: a 2D bake of Vector Fonts / HD UI
        // Art (~165 MB) that sharp bilinear sampling replaced.
        for name in OBSOLETE {
            if std::fs::remove_file(dir.join(name)).is_ok() {
                log::info!("cache: removed obsolete {name}");
            }
        }
        // Other builds' entries of the kinds this build wrote in an
        // earlier run (a write prunes its own kind: [`prune`]): on a
        // writer thread (`flush` waits for it), off the loading thread.
        let dir = dir.to_path_buf();
        spawn_writer(move || {
            prune_superseded(&dir, build_id(), None, SUPERSEDED_AFTER);
        });
    });
}

/// Keep the folder under `NFSU2_CACHE_MAX_MB` (default 2048), temporary
/// files and the surface maps' folder ([`SURFACE_MAPS`]) counted: other
/// builds' files (and the files of builds before the build id was in the
/// name) go first, then this build's and the surface maps, each the one
/// used longest ago first (an open touches a file, `CacheFile::open_in`,
/// [`touch_file`]).
/// Never removed: `keep` (just written) and every file younger than
/// [`PRUNE_GRACE`] (another process's just written or just opened file).
/// A file in use stays readable: on Unix its mapping keeps the old inode,
/// and Windows does not remove a mapped file.
pub(super) fn prune(dir: &Path, keep: &str) {
    sweep_tmp(dir);
    // The entry just written supersedes the other builds' entries of its
    // kind ([`prune_superseded`]), whatever the folder's size.
    if let Some((kind, build)) = split_name(keep) {
        prune_superseded(dir, build, Some(kind), SUPERSEDED_AFTER);
    }
    let limit = std::env::var("NFSU2_CACHE_MAX_MB").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(2048) << 20;
    prune_to(dir, keep, limit);
}

/// [`prune`] to `limit` bytes.
pub(super) fn prune_to(dir: &Path, keep: &str, limit: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let now = std::time::SystemTime::now();
    let own = build_id();
    let young = |m: &std::fs::Metadata| {
        m.modified().map_or(true, |t| now.duration_since(t).map_or(true, |age| age < PRUNE_GRACE))
    };
    let mut total = 0;
    // (another build's, time, size, path)
    let mut files: Vec<(bool, std::time::SystemTime, u64, std::path::PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        // Temporary files the sweep left (live writers, or not yet stale)
        // still take room: counted (never removed here).
        if is_tmp_name(name) {
            total += m.len();
            continue;
        }
        if !name.ends_with(NFC) {
            continue;
        }
        total += m.len();
        let Ok(t) = m.modified() else { continue };
        if name != keep && !young(&m) {
            files.push((split_name(name).is_none_or(|(_, b)| b != own), t, m.len(), e.path()));
        }
    }
    // The surface maps (`<key>.rg8`, an empty `<key>.none` for a texture
    // without a map, `car-<key>.*`): every file counted; the ones with
    // bytes are evicted as this build's (the generator's version is in
    // the key, not in a header the prune could read). An empty one frees
    // nothing and would only be judged again: kept.
    if let Ok(rd) = std::fs::read_dir(dir.join(SURFACE_MAPS)) {
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if !m.is_file() {
                continue;
            }
            total += m.len();
            let tmp = e.file_name().to_str().is_none_or(is_tmp_name);
            if !tmp
                && m.len() > 0
                && !young(&m)
                && let Ok(t) = m.modified()
            {
                files.push((false, t, m.len(), e.path()));
            }
        }
    }
    // Other builds' first, then oldest first.
    files.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, _, len, path) in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// Files younger than this are never pruned (see [`prune`]).
pub(super) const PRUNE_GRACE: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Another build's entry not used for this long is removed once the
/// current build has its own entry of the same kind
/// ([`prune_superseded`]). An entry in use is touched at an open
/// ([`TOUCH_AFTER`]), so a build still played within the hour (two builds
/// side by side, or a player who goes back to the last one) keeps its
/// entries.
pub(super) const SUPERSEDED_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The entries in `dir` that the build `current` no longer needs: every
/// other build's file of a kind that `current` has its own file of (and
/// the old-named `<kind>.nfc` of such a kind), when it was not used for
/// `after`. Only the kinds in `only` when given. Never `current`'s files,
/// temporary files, anything that is not a plain file directly in `dir`
/// (folders such as the surface maps', links) or a kind `current` has no
/// file of. The bytes removed.
pub(super) fn prune_superseded(dir: &Path, current: u64, only: Option<&str>, after: std::time::Duration) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let now = std::time::SystemTime::now();
    let mut own: std::collections::HashSet<String> = std::collections::HashSet::new();
    // (kind, path, size, last used)
    let mut others: Vec<(String, std::path::PathBuf, u64, std::time::SystemTime)> = Vec::new();
    for e in rd.flatten() {
        // The entry itself (a link is not followed: it is no plain file).
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        if is_tmp_name(name) || !name.ends_with(NFC) {
            continue;
        }
        let (kind, build) = match split_name(name) {
            Some((kind, build)) => (kind.to_string(), Some(build)),
            None => (name.strip_suffix(NFC).unwrap_or(name).to_string(), None),
        };
        if only.is_some_and(|k| k != kind) {
            continue;
        }
        if build == Some(current) {
            own.insert(kind);
        } else if let Ok(t) = m.modified() {
            others.push((kind, e.path(), m.len(), t));
        }
    }
    let mut freed = 0;
    for (kind, path, len, used) in others {
        let unused = now.duration_since(used).is_ok_and(|age| age >= after);
        if own.contains(&kind) && unused && std::fs::remove_file(&path).is_ok() {
            log::info!("cache: removed {} (another build's {kind}, superseded)", path.display());
            freed += len;
        }
    }
    freed
}

/// `path` (a cache file just read) marked as used: its modification time
/// set to now when it is older than [`TOUCH_AFTER`], so the size limit
/// removes the files used longest ago first. Read-only folders and
/// Windows' rules on a mapped file: the file keeps its time.
pub fn touch_file(path: &Path) {
    let old = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age >= TOUCH_AFTER);
    if old {
        touch_later(path.to_path_buf());
    }
}

/// `path` marked as used (its modification time set to now) on the cache's
/// toucher thread, in the order asked, within milliseconds: a load never
/// waits for it. Setting a file's time is a metadata change, which on a
/// journalling file system waits for the journal's commit in progress, and
/// a commit waits for the data another program (or this game's cache
/// writers) is writing: 0.1–0.2 s with a 150 MB write and fsync beside it,
/// seconds on a busy disk, where the loading thread itself waited before.
/// The times the pruning reads ("used within the hour", "used longest
/// ago") are the same: a mark lands long before they matter, and
/// [`wait_touches`] (the cache's flush, the game's exit) waits for the
/// queue. Without a thread to be had the mark is made at once, as before.
pub(super) fn touch_later(path: std::path::PathBuf) {
    use std::sync::mpsc::Sender;
    static QUEUE: std::sync::Mutex<Option<Sender<std::path::PathBuf>>> = std::sync::Mutex::new(None);
    *TOUCHES.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    let mut q = QUEUE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if q.is_none() {
        let (tx, rx) = std::sync::mpsc::channel::<std::path::PathBuf>();
        let spawned = std::thread::Builder::new().name("cache touch".into()).spawn(move || {
            for p in rx {
                mark_used(&p);
            }
        });
        if spawned.is_ok() {
            *q = Some(tx);
        }
    }
    let left = match q.as_ref() {
        Some(tx) => tx.send(path).err().map(|e| e.0),
        None => Some(path),
    };
    drop(q);
    if let Some(p) = left {
        mark_used(&p);
    }
}

/// The marks queued and not yet made, and their change's signal.
static TOUCHES: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// One mark (read-only folders and Windows' rules on a mapped file: the
/// file keeps its time, only the eviction order changes).
fn mark_used(path: &Path) {
    let _ = std::fs::OpenOptions::new().write(true).open(path).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    let mut n = TOUCHES.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *n = n.saturating_sub(1);
    TOUCHES.1.notify_all();
}

/// Waits until every queued mark is made, at most `limit`; how many are
/// left.
pub fn wait_touches(limit: Option<std::time::Duration>) -> usize {
    let start = std::time::Instant::now();
    let mut n = TOUCHES.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    while *n > 0 {
        let wait = match limit {
            Some(l) if start.elapsed() >= l => break,
            Some(l) => l - start.elapsed(),
            None => std::time::Duration::from_secs(1),
        };
        n = TOUCHES.1.wait_timeout(n, wait).unwrap_or_else(std::sync::PoisonError::into_inner).0;
    }
    *n
}
