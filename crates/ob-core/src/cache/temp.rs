//! Temporary files for atomic cache writes, and the stale-file sweep.

use super::*;

/// A cache file being written. Killed mid-write, a writer must leave
/// nothing behind: on Linux the data goes to an anonymous file in the
/// cache folder (`O_TMPFILE`), given a name only once complete and synced
/// (a temporary one, renamed over the old file at once); elsewhere, or on
/// a file system without it, to `<name>.<pid>-<hex>-<n>.<seq>.tmp`
/// (`crate::unique_tag`; a bare pid in that field is read too), locked while it
/// is written, which [`prune`] removes once stale and unlocked.
pub(super) enum TempFile {
    #[cfg(target_os = "linux")]
    Anonymous(std::path::PathBuf),
    Named(std::path::PathBuf),
}

impl TempFile {
    pub(super) fn create(dir: &Path, name: &str) -> std::io::Result<(std::fs::File, TempFile)> {
        let tmp = tmp_path(dir, name);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if let Ok(f) =
                std::fs::OpenOptions::new().read(true).write(true).mode(0o644).custom_flags(libc::O_TMPFILE).open(dir)
            {
                // Locked as a named one is: if it is given its temporary
                // name and the rename is slow (a busy disk), the sweep of
                // another process sees a live writer.
                let _ = f.try_lock();
                return Ok((f, TempFile::Anonymous(tmp)));
            }
        }
        let f = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&tmp)?;
        if let Err(e) = f.try_lock() {
            drop(f);
            let _ = std::fs::remove_file(&tmp);
            return Err(std::io::Error::other(format!("temporary file not lockable: {e}")));
        }
        Ok((f, TempFile::Named(tmp)))
    }

    /// The written, synced `file` in place as `to`. On Windows a `to` that
    /// cannot be replaced (mapped by a [`CacheFile`] still in use) is moved
    /// to a temporary name first, which [`sweep_tmp`] removes once nothing
    /// maps it (`platform::fs::rename_replace_aside`).
    pub(super) fn finish(self, file: &std::fs::File, to: &Path) -> std::io::Result<()> {
        #[cfg(not(target_os = "linux"))]
        let _ = file;
        let tmp = match &self {
            #[cfg(target_os = "linux")]
            TempFile::Anonymous(tmp) => {
                use std::os::unix::ffi::OsStrExt;
                use std::os::unix::io::AsRawFd;
                // A path of ASCII digits and slashes: no NUL inside.
                let proc = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap();
                let link = |at: &Path| {
                    let at = std::ffi::CString::new(at.as_os_str().as_bytes()).map_err(std::io::Error::other)?;
                    // SAFETY: two NUL-terminated paths; links the open file.
                    if unsafe {
                        libc::linkat(
                            libc::AT_FDCWD,
                            proc.as_ptr(),
                            libc::AT_FDCWD,
                            at.as_ptr(),
                            libc::AT_SYMLINK_FOLLOW,
                        )
                    } != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                };
                // No file there yet: linked in place, nothing in between.
                match link(to) {
                    Ok(()) => return Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e),
                }
                // Replacing one: under a temporary name, renamed over it at
                // once (killed in between, the name is left for `sweep_tmp`).
                link(tmp)?;
                tmp
            }
            TempFile::Named(tmp) => tmp,
        };
        let aside = || {
            let dir = to.parent().unwrap_or(Path::new("."));
            tmp_path(dir, &to.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()))
        };
        let r = crate::platform::fs::rename_replace_aside(tmp, to, aside);
        if r.is_err() {
            let _ = std::fs::remove_file(tmp);
        }
        r
    }
}

impl Drop for TempFile {
    /// Not finished (an error): the named temporary file goes (after a
    /// rename there is nothing left to remove).
    #[cfg_attr(not(target_os = "linux"), allow(irrefutable_let_patterns))]
    fn drop(&mut self) {
        if let TempFile::Named(tmp) = self {
            let _ = std::fs::remove_file(tmp);
        }
    }
}

pub(super) const TMP_SUFFIX: &str = ".tmp";

/// A fresh temporary name for the file `name` in `dir`,
/// `<name>.<unique tag>.<seq>.tmp`: unique per write, not only per
/// process (two writers of one kind in a process — the world, then its
/// renderer's sections — would otherwise share and interleave one file).
fn tmp_path(dir: &Path, name: &str) -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    dir.join(format!("{name}.{}.{seq}{TMP_SUFFIX}", crate::unique_tag()))
}

/// A temporary file older than this, and not locked by its writer, was
/// left by a writer that died (see [`TempFile`]).
pub(super) const STALE_TMP: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// A temporary file not locked, this old and whose writer's process is
/// gone ([`writer_gone`]) is removed before [`STALE_TMP`]. The age covers
/// a writer between creating its file and locking it.
pub(super) const DEAD_TMP: std::time::Duration = std::time::Duration::from_secs(30);

/// A [`TempFile`] name (`<name>.<pid>.<seq>.tmp`), the pipeline cache's
/// older `<name>.tmp<pid>` or a surface map's `<key>.tmp<pid>-<n>`.
pub(super) fn is_tmp_name(n: &str) -> bool {
    tmp_pid(n).is_some()
}

/// The writer's process id in a temporary file's name ([`is_tmp_name`]).
pub(super) fn tmp_pid(n: &str) -> Option<u32> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if let Some(rest) = n.strip_suffix(TMP_SUFFIX) {
        let mut it = rest.rsplitn(3, '.');
        // The middle field is `crate::unique_tag()`: `<pid>-<hex>-<seq>`
        // (a pid alone is accepted as well).
        let tag = |s: &str| -> Option<u32> {
            if digits(s) {
                return s.parse().ok();
            }
            let mut p = s.splitn(3, '-');
            match (p.next(), p.next(), p.next()) {
                (Some(pid), Some(hex), Some(seq))
                    if digits(pid) && !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) && digits(seq) =>
                {
                    pid.parse().ok()
                }
                _ => None,
            }
        };
        return match (it.next(), it.next(), it.next()) {
            (Some(seq), Some(pid), Some(name)) if digits(seq) && !name.is_empty() => tag(pid),
            _ => None,
        };
    }
    // `<name>.tmp<pid>`, and the surface maps' `<key>.tmp<pid>-<n>`.
    let (name, rest) = n.rsplit_once(".tmp")?;
    let pid = match rest.split_once('-') {
        Some((pid, seq)) if digits(seq) => pid,
        Some(_) => return None,
        None => rest,
    };
    (!name.is_empty() && digits(pid)).then(|| pid.parse().ok())?
}

/// Whether the process `pid` is certainly gone: Linux, no `/proc/<pid>`
/// (with `/proc` there). A pid in use again by another process reads as
/// alive, and so does every pid where this cannot be told (other systems,
/// another pid namespace's writer whose number is in use here): those
/// files wait for [`STALE_TMP`].
pub(super) fn writer_gone(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let proc = Path::new("/proc");
        proc.join("self").exists() && !proc.join(pid.to_string()).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

/// Write `data` as the file `path` in the cache folder, whole or not at
/// all (as the cache files: see [`TempFile`]).
pub fn replace_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| std::io::Error::other("no file name"))?.to_string_lossy().into_owned();
    std::fs::create_dir_all(dir)?;
    let (mut file, named) = TempFile::create(dir, &name)?;
    file.write_all(data)?;
    file.sync_all()?;
    named.finish(&file, path)
}

/// [`replace_file`] for many files written by one job (the texture-pack
/// generator's `<HASH>.dds`): each file goes to its own temporary file at
/// once and stays invisible; every [`ReplaceBatch::LIMIT`] files, and at
/// [`ReplaceBatch::commit`], the pending ones are synced together and put
/// in place. Whole or not at all, as [`replace_file`], with one sync per
/// batch instead of one per file: on Linux one `syncfs` for the batch,
/// elsewhere a sync of each file. Shared by the job's threads; a batch
/// dropped without `commit` leaves its last files unwritten.
pub struct ReplaceBatch {
    pending: std::sync::Mutex<Vec<(std::fs::File, TempFile, std::path::PathBuf)>>,
}

impl Default for ReplaceBatch {
    fn default() -> Self {
        ReplaceBatch { pending: std::sync::Mutex::new(Vec::new()) }
    }
}

impl ReplaceBatch {
    /// Files held open before a sync (open descriptors, not memory).
    pub const LIMIT: usize = 64;

    /// `data` as the file `path`, visible once its batch is synced.
    pub fn add(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let name =
            path.file_name().ok_or_else(|| std::io::Error::other("no file name"))?.to_string_lossy().into_owned();
        std::fs::create_dir_all(dir)?;
        let (mut file, named) = TempFile::create(dir, &name)?;
        file.write_all(data)?;
        let full = {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            p.push((file, named, path.to_path_buf()));
            if p.len() >= Self::LIMIT { std::mem::take(&mut *p) } else { Vec::new() }
        };
        Self::put_in_place(full)
    }

    /// Sync and put in place the files still pending.
    pub fn commit(&self) -> std::io::Result<()> {
        let rest = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        Self::put_in_place(rest)
    }

    /// The files pending (tests).
    pub fn pending(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    fn put_in_place(files: Vec<(std::fs::File, TempFile, std::path::PathBuf)>) -> std::io::Result<()> {
        let Some((first, ..)) = files.first() else { return Ok(()) };
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            // SAFETY: plain syscall on an open descriptor; syncs the file
            // system holding it (every file of the batch: one folder).
            if unsafe { libc::syncfs(first.as_raw_fd()) } != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = first;
            for (f, ..) in &files {
                f.sync_all()?;
            }
        }
        let mut result = Ok(());
        for (file, named, to) in files {
            if let Err(e) = named.finish(&file, &to) {
                result = result.and(Err(e));
            }
        }
        result
    }
}

/// Remove the stale temporary files in `dir`; the bytes they held.
pub(super) fn sweep_tmp(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let now = std::time::SystemTime::now();
    let mut freed = 0;
    for e in rd.flatten() {
        if !e.file_name().to_str().is_some_and(is_tmp_name) {
            continue;
        }
        let Ok(m) = e.metadata() else { continue };
        let Some(age) = m.modified().ok().and_then(|t| now.duration_since(t).ok()) else { continue };
        let pid = e.file_name().to_str().and_then(tmp_pid);
        let stale = age >= STALE_TMP || (age >= DEAD_TMP && pid.is_some_and(writer_gone));
        if !m.is_file() || !stale {
            continue;
        }
        // A live writer holds its lock (and on Windows, its open file
        // cannot be removed).
        let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(e.path()) else { continue };
        if f.try_lock().is_err() {
            continue;
        }
        if std::fs::remove_file(e.path()).is_ok() {
            freed += m.len();
        }
    }
    if freed > 0 {
        log::info!("cache: removed {} MB of stale temporary files", freed >> 20);
    }
    freed
}
