use std::path::{Path, PathBuf};

// Moved to the bottom layer (ob-core's `platform::fs`) so the map data crate writes its files the same way.
pub use crate::platform::fs::write_atomic;

/// Where a profile's backups go: `<data home>/NFS Underground 2
/// Backups/<NAME>/`, beside the profiles' folder and not in it — the
/// original lists every entry of its profile folder as a profile
/// (FindFirstFileA "\*", 0x6FC01E), so nothing of the clone's goes
/// there.
pub fn backup_root() -> PathBuf {
    crate::platform::dirs::data_home().join(format!("{} Backups", crate::frontend::profiles::FOLDER))
}

/// Copy the profile file `path` (if there is one) into `dir` as
/// `<file name>.<YYYYMMDD-HHMMSS>[-n]` and keep the newest `keep` there.
/// Returns the copy's path. The copy is the file's bytes unchanged, so
/// it loads in the original once renamed back to `<NAME>`.
pub fn backup(path: &Path, dir: &Path, keep: usize) -> std::io::Result<Option<PathBuf>> {
    if keep == 0 || !path.is_file() {
        return Ok(None);
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("profile").to_string();
    std::fs::create_dir_all(dir)?;
    let stamp = timestamp(std::time::SystemTime::now());
    // A second backup within the same second gets the next number.
    let same: Vec<u32> =
        list(dir, &name).iter().filter_map(|p| key(p, &name)).filter(|k| k.0 == stamp).map(|k| k.1 + 1).collect();
    let to = match same.iter().max() {
        None => dir.join(format!("{name}.{stamp}")),
        Some(&n) => dir.join(format!("{name}.{stamp}-{n}")),
    };
    let bytes = std::fs::read(path)?;
    write_atomic(&to, &bytes, false)?;
    prune(dir, &name, keep)?;
    Ok(Some(to))
}

/// The backups of `name` in `dir`, oldest first (the stamps sort by
/// time).
pub fn list(dir: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(&prefix) && !n.ends_with(".tmp"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|p| key(p, name).unwrap_or_default());
    v
}

/// A backup's (stamp, number): "<stamp>" is 0, "<stamp>-n" n, so they
/// sort by time ("-10" after "-9").
fn key(path: &Path, name: &str) -> Option<(String, u32)> {
    let n = path.file_name()?.to_str()?;
    let tail = n.strip_prefix(name)?.strip_prefix('.')?;
    Some(match tail.split_once('-').and_then(|(d, rest)| rest.split_once('-').map(|(t, k)| (d, t, k))) {
        Some((d, t, k)) => (format!("{d}-{t}"), k.parse().unwrap_or(0)),
        None => (tail.to_string(), 0),
    })
}

fn prune(dir: &Path, name: &str, keep: usize) -> std::io::Result<()> {
    let all = list(dir, name);
    for old in &all[..all.len().saturating_sub(keep)] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// `YYYYMMDD-HHMMSS` in UTC (with the log file, `log_file::timestamp`).
pub use crate::log_file::timestamp;
