//! Replacing Overbright's own files (saves, options, caches) by rename,
//! with the Windows rules (docs/playing/platform-dirs.md).
//!
//! Unix: [`rename_replace`] is `std::fs::rename`, nothing else.
//!
//! Windows: a rename over a file fails while another process holds that
//! file open without delete sharing (antivirus scanners, cloud sync, the
//! original game reading its profile) — `ERROR_SHARING_VIOLATION` (32),
//! `ERROR_LOCK_VIOLATION` (33) or `ERROR_ACCESS_DENIED` (5); those holds are
//! short, so the rename is retried for up to [`RETRY_FOR`]. A file with a
//! live mapped view cannot be replaced at all (also `ERROR_ACCESS_DENIED`)
//! but can be renamed: [`rename_replace_aside`] moves it to a name of the
//! caller's choosing first, and the mapping keeps reading the old data.
//! Windows has no directory fsync: `sync_all` (FlushFileBuffers) on the
//! file is all a durable write gets there.

use std::path::Path;

/// How long a Windows rename is retried for (in steps of 1, 2, 4 … ms).
pub const RETRY_FOR: std::time::Duration = std::time::Duration::from_millis(500);

/// Whether `e` is a Windows "someone else holds the file" error.
#[cfg(windows)]
pub fn is_held(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Rename `from` over `to`. Windows: retried while the target is held
/// ([`is_held`]), for up to [`RETRY_FOR`]; the last error otherwise.
pub fn rename_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(not(windows))]
    {
        std::fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        retry(|| std::fs::rename(from, to))
    }
}

/// `op` retried while it fails with [`is_held`], for up to [`RETRY_FOR`].
#[cfg(windows)]
pub fn retry(mut op: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    let mut wait = std::time::Duration::from_millis(1);
    loop {
        match op() {
            Err(e) if is_held(&e) && start.elapsed() + wait <= RETRY_FOR => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(std::time::Duration::from_millis(64));
            }
            r => return r,
        }
    }
}

/// [`rename_replace`], and on Windows, when `to` still cannot be replaced
/// (a mapped view of it is open, in this process or another), `to` moved
/// to `aside` first and `from` renamed into its place. Unix: the plain
/// rename (a mapping there keeps the replaced file's inode).
pub fn rename_replace_aside(from: &Path, to: &Path, aside: impl FnOnce() -> std::path::PathBuf) -> std::io::Result<()> {
    #[cfg(not(windows))]
    {
        let _ = aside;
        std::fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        match rename_replace(from, to) {
            Err(e) if is_held(&e) => {
                let aside = aside();
                retry(|| std::fs::rename(to, &aside)).map_err(|_| e)?;
                rename_replace(from, to).inspect_err(|_| {
                    // Put the old file back rather than leave none.
                    let _ = std::fs::rename(&aside, to);
                })
            }
            r => r,
        }
    }
}

#[cfg(all(test, windows))]
mod tests;

/// Write `bytes` to `path` through a temporary file next to it and a
/// rename, so a crash leaves the old file or the new one, never half of
/// either (the original does the same: CreateFileA on a temporary file,
/// then MoveFileA, 0x6F9BFC). With `durable`, the temporary file is
/// flushed to the disk before the rename and the folder after it.
pub fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        if durable {
            f.sync_all()?;
        }
    }
    if let Err(e) = rename_replace(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // The rename itself reaches the disk with the folder's entry (Windows
    // has no folder fsync: the file's FlushFileBuffers above is all).
    #[cfg(unix)]
    if durable && let Some(dir) = path.parent() {
        std::fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// Free bytes on the disk holding `dir` (its nearest existing parent);
/// None when the system does not say.
pub fn free_space(dir: &Path) -> Option<u64> {
    let mut d = dir;
    while !d.exists() {
        d = d.parent()?;
    }
    free_space_of(d)
}

#[cfg(unix)]
fn free_space_of(d: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(d.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path, `s` a writable statvfs.
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(windows)]
fn free_space_of(d: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let w: Vec<u16> = d.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut avail = 0u64;
    // SAFETY: `w` is NUL-terminated; the out pointers may be null except `avail`.
    (unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) } != 0)
        .then_some(avail)
}

#[cfg(not(any(unix, windows)))]
fn free_space_of(_d: &Path) -> Option<u64> {
    None
}
