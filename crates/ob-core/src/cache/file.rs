//! `CacheFile`: open, check, read sections; and the (background) writer.

use super::*;

/// An open, checked cache file.
pub struct CacheFile {
    pub(super) map: std::sync::Arc<memmap2::Mmap>,
    pub(super) header: Header,
    /// Section by name.
    pub(super) index: std::collections::HashMap<String, usize>,
    /// Valid, but a source's time or inode changed (same content).
    pub(super) retimed: bool,
}

/// Cache files [`CacheFile::open`] found valid, and asked for but missing
/// or stale (the cache on), since the start.
static OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static MISSED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The cache files opened valid and missed so far (the log's waits,
/// `qol::waits`: a load's difference).
pub fn open_counts() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (OPENED.load(Relaxed), MISSED.load(Relaxed))
}

impl CacheFile {
    /// The cache of `kind` over the game files `sources` (paths under
    /// `root`), if there is a valid one written by the build `build` (the
    /// id of the crate whose code makes it, [`BuildId`]).
    pub fn open(build: BuildId, root: &Path, kind: &str, sources: &[&str]) -> Option<CacheFile> {
        let _t = crate::prof::span("cache: open");
        let opened = Self::open_in(&dir()?, build, root, kind, sources);
        let counter = if opened.is_some() { &OPENED } else { &MISSED };
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let c = opened?;
        if c.retimed {
            // Rewrite it with the files' new times (same sections). Every
            // name comes from the header, which the index is built from, so
            // `bytes` finds each one.
            let sections = c.header.sections.iter().map(|e| (e.name.clone(), c.bytes(&e.name).unwrap())).collect();
            write(build, root, kind, sources, sections);
        }
        Some(c)
    }

    /// [`CacheFile::open`] in the cache folder `dir` (tests).
    #[doc(hidden)]
    pub fn open_in(dir: &Path, build: BuildId, root: &Path, kind: &str, sources: &[&str]) -> Option<CacheFile> {
        Self::open_build(dir, root, kind, sources, build.0)
    }

    /// [`CacheFile::open_in`] as the build `build` would open it (tests:
    /// another build's files).
    pub(super) fn open_build(dir: &Path, root: &Path, kind: &str, sources: &[&str], build: u64) -> Option<CacheFile> {
        let path = dir.join(file_name_of(kind, build));
        sweep_once(dir, BuildId(build));
        let file = std::fs::File::open(&path).ok()?;
        // SAFETY: cache files are only ever replaced by rename, never
        // written in place (see above), so the mapped inode does not change.
        // Not populated: the check below reads the file (in parallel, see
        // `sections_intact`), and a load touches the pages it uses.
        let map = unsafe { memmap2::MmapOptions::new().map(&file) }.ok()?;
        let map = std::sync::Arc::new(map);
        let Some(header) = Self::header(&map) else {
            log::info!("cache {kind}: unreadable header");
            return None;
        };
        if header.build != build || header.kind != kind || header.sources.len() != sources.len() {
            log::info!("cache {kind}: stale (build or kind)");
            return None;
        }
        let mut retimed = false;
        for (s, rel) in header.sources.iter().zip(sources) {
            if s.rel != *rel {
                log::info!("cache {kind}: other source files");
                return None;
            }
            let (size, mtime, ino) = stat(&source_path(root, rel))?;
            if size != s.size {
                log::info!("cache {kind}: {rel} changed");
                return None;
            }
            if (mtime, ino) != (s.mtime_ns, s.ino) {
                // Touched or copied: the same content is still good.
                if content_hash(&source_path(root, rel)) != Some(s.hash) {
                    log::info!("cache {kind}: {rel} changed");
                    return None;
                }
                retimed = true;
            }
        }
        if !sections_intact(&file, &map, &header.sections) {
            log::warn!("cache {kind}: corrupt, ignored");
            return None;
        }
        touch(&path, &file);
        log::debug!("cache {kind}: hit");
        let index = header.sections.iter().enumerate().map(|(i, e)| (e.name.clone(), i)).collect();
        let c = CacheFile { map, header, index, retimed };
        MAPS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(std::sync::Arc::downgrade(&c.map));
        Some(c)
    }

    pub fn header(map: &[u8]) -> Option<Header> {
        if map.len() < 28 || &map[..8] != MAGIC || u32::from_le_bytes(map[8..12].try_into().ok()?) != FORMAT {
            return None;
        }
        let len = u64::from_le_bytes(map[12..20].try_into().ok()?) as usize;
        let hash = u64::from_le_bytes(map[20..28].try_into().ok()?);
        let h = map.get(28..28usize.checked_add(len)?)?;
        if xxhash_rust::xxh3::xxh3_64(h) != hash {
            return None;
        }
        let mut r = Reader::new(h);
        let header = Header::get(&mut r).ok()?;
        r.done().then_some(header)
    }

    pub(super) fn entry(&self, name: &str) -> Option<&SectionEntry> {
        self.index.get(name).map(|&i| &self.header.sections[i])
    }

    pub fn has(&self, name: &str) -> bool {
        self.entry(name).is_some()
    }

    /// A section's bytes (pointing into the mapping).
    pub fn bytes(&self, name: &str) -> Option<Bytes> {
        let e = self.entry(name)?;
        Some(Bytes::mapped(self.map.clone(), e.offset as usize..(e.offset + e.len) as usize))
    }

    /// A reader over a section; [`Bytes`] read from it point into the
    /// mapping.
    pub fn reader(&self, name: &str) -> Option<Reader<'_>> {
        let e = self.entry(name)?;
        Some(Reader::mapped(&self.map, e.offset as usize, e.len as usize))
    }

    /// The mapping's pages are no longer needed in memory (they are the
    /// file's, read again if touched).
    pub fn release_pages(&self) {
        #[cfg(unix)]
        // SAFETY: DONTNEED on a private read-only file mapping only drops
        // the pages; later reads fault them back in from the file.
        unsafe {
            let _ = self.map.unchecked_advise(memmap2::UncheckedAdvice::DontNeed);
        }
    }
}

/// A cache source's file: `rel` under the game folder `root` in the
/// folder's own spelling ([`crate::game_fs::path`]); per-file caches pass
/// an empty `root` and the file's whole path as `rel`.
fn source_path(root: &Path, rel: &str) -> std::path::PathBuf {
    if root.as_os_str().is_empty() { std::path::PathBuf::from(rel) } else { crate::game_fs::path(root, rel) }
}

/// Every cache mapping in use (see [`release_mapped_pages`]).
pub(super) static MAPS: std::sync::Mutex<Vec<std::sync::Weak<memmap2::Mmap>>> = std::sync::Mutex::new(Vec::new());

/// Drop the pages of every cache mapping still in use from memory: once a
/// load has handed their data to the GPU they are not read again (and if
/// they are, they come back from the file). A cache file's data then costs
/// no resident memory.
pub fn release_mapped_pages() {
    let mut maps = MAPS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    maps.retain(|m| m.strong_count() > 0);
    // Elsewhere (Windows) the pages stay resident.
    #[cfg(unix)]
    for m in maps.iter().filter_map(std::sync::Weak::upgrade) {
        // SAFETY: as `CacheFile::release_pages`.
        unsafe {
            let _ = m.unchecked_advise(memmap2::UncheckedAdvice::DontNeed);
        }
    }
}

/// A used cache file's modification time set to now when it is older than
/// [`TOUCH_AFTER`]: the size limit removes the files used longest ago
/// first ([`prune`]), not the ones written longest ago. The age is read
/// from the open file; the new time is set on the cache's toucher thread
/// ([`touch_later`]), not by the load that opened the file.
fn touch(path: &Path, file: &std::fs::File) {
    let old = file
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age >= TOUCH_AFTER);
    if old {
        touch_later(path.to_path_buf());
    }
}

/// A file opened again within this long is not touched again ([`touch`]).
pub(super) const TOUCH_AFTER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Writers still running (joined by [`flush`]).
pub(super) static WRITERS: std::sync::Mutex<Vec<std::thread::JoinHandle<()>>> = std::sync::Mutex::new(Vec::new());

/// Write a cache file of `kind` on a background thread: its sections, and
/// the source files' keys (their content hashes taken there too). See
/// [`submit`] for how often a process writes one file. `build`: the id of
/// the crate whose code made the sections ([`BuildId`]).
pub fn write(build: BuildId, root: &Path, kind: &str, sources: &[&str], sections: Vec<(String, Bytes)>) {
    let Some(dir) = dir() else { return };
    submit(Job {
        dir,
        root: root.to_path_buf(),
        kind: kind.to_string(),
        sources: sources.iter().map(|s| s.to_string()).collect(),
        sections,
        build: build.0,
    });
}

/// A cache file to write.
pub(super) struct Job {
    pub(super) dir: std::path::PathBuf,
    pub(super) root: std::path::PathBuf,
    pub(super) kind: String,
    pub(super) sources: Vec<String>,
    pub(super) sections: Vec<(String, Bytes)>,
    pub(super) build: u64,
}

/// One cache file's writes in this process ([`submit`]).
#[derive(Default)]
struct Slot {
    /// A writer thread is on this file.
    busy: bool,
    /// The newest request that came while it was busy.
    queued: Option<Job>,
    /// The contents ([`content_id`]) this process wrote or found written.
    done: std::collections::HashSet<u64>,
}

static SLOTS: std::sync::Mutex<Option<std::collections::HashMap<std::path::PathBuf, Slot>>> =
    std::sync::Mutex::new(None);

/// What [`submit`] did with each request, per file of this process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WriteCounts {
    /// Written to disk.
    pub written: u32,
    /// Not written: this process wrote the same contents before.
    pub repeated: u32,
    /// Not written: the file on disk already holds them (another process
    /// finished it).
    pub present: u32,
    /// Replaced by a newer request before its writer got to it.
    pub superseded: u32,
    /// Tried and failed (logged).
    pub failed: u32,
}

static COUNTS: std::sync::Mutex<Option<std::collections::HashMap<std::path::PathBuf, WriteCounts>>> =
    std::sync::Mutex::new(None);

fn count(path: &Path, f: impl FnOnce(&mut WriteCounts)) {
    let mut c = COUNTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(c.get_or_insert_with(Default::default).entry(path.to_path_buf()).or_default());
}

/// What this process did with its write requests, per cache file (the
/// tests, and `NFSU2_CACHE_LOG=1`'s summary at [`flush`]).
pub fn write_counts() -> Vec<(std::path::PathBuf, WriteCounts)> {
    let c = COUNTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut v: Vec<_> = c.iter().flatten().map(|(p, n)| (p.clone(), *n)).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// A write request. A file has at most one writer thread in a process: a
/// request while it runs waits as the file's queued one (a newer request
/// replaces it), and the writer takes it when done. A writer writes only
/// contents (sources' keys and sections) that this process has not written
/// to that file before and that the file on disk does not already hold
/// (another process of this build finished it meanwhile). So a burst of
/// loads that all missed (several cars of one model, a load repeated before
/// the first write landed, an entry another process keeps replacing) costs
/// one write per process, never one per load.
pub(super) fn submit(job: Job) {
    let path = job.dir.join(file_name_of(&job.kind, job.build));
    {
        let mut slots = SLOTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = slots.get_or_insert_with(Default::default).entry(path.clone()).or_default();
        if slot.busy {
            if slot.queued.replace(job).is_some() {
                count(&path, |c| c.superseded += 1);
            }
            return;
        }
        slot.busy = true;
    }
    let p = path.clone();
    if !spawn_writer(move || run_slot(&p, job)) {
        let mut slots = SLOTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(slot) = slots.get_or_insert_with(Default::default).get_mut(&path) {
            slot.busy = false;
            slot.queued = None;
        }
    }
}

/// The writer of one file: `job`, then each request queued meanwhile.
fn run_slot(path: &Path, mut job: Job) {
    loop {
        let done = {
            let slots = SLOTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            slots.as_ref().and_then(|s| s.get(path)).map(|s| s.done.clone()).unwrap_or_default()
        };
        let outcome = write_unless_done(&job, &done);
        match &outcome {
            Ok((Outcome::Written, _)) => count(path, |c| c.written += 1),
            Ok((Outcome::Repeated, _)) => count(path, |c| c.repeated += 1),
            Ok((Outcome::Present, _)) => count(path, |c| c.present += 1),
            Ok((Outcome::Failed, _)) | Err(_) => count(path, |c| c.failed += 1),
        }
        let mut slots = SLOTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let slot = slots.get_or_insert_with(Default::default).entry(path.to_path_buf()).or_default();
        // A failed write counts as done too: tried once a process.
        if let Ok((_, id)) = &outcome {
            slot.done.insert(*id);
        }
        match slot.queued.take() {
            Some(next) => job = next,
            None => {
                slot.busy = false;
                return;
            }
        }
    }
}

/// What [`write_unless_done`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Written,
    Repeated,
    Present,
    Failed,
}

/// `job` written unless its contents' id is in `done` or the file on disk
/// already holds them; the outcome and the id.
pub(super) fn write_unless_done(job: &Job, done: &std::collections::HashSet<u64>) -> std::io::Result<(Outcome, u64)> {
    let header = header_for(&job.root, &job.kind, &job.sources, &job.sections, job.build)
        .inspect_err(|e| log::warn!("cache {}: not written: {e}", job.kind))?;
    let id = content_id(&header);
    if done.contains(&id) {
        log::debug!("cache {}: written before by this process", job.kind);
        return Ok((Outcome::Repeated, id));
    }
    let path = job.dir.join(file_name_of(&job.kind, job.build));
    if holds(&path, &header) {
        log::debug!("cache {}: already on disk", job.kind);
        return Ok((Outcome::Present, id));
    }
    match write_header(&job.dir, &job.kind, header, &job.sections) {
        Ok(()) => Ok((Outcome::Written, id)),
        Err(e) => {
            log::warn!("cache {}: not written: {e}", job.kind);
            Ok((Outcome::Failed, id))
        }
    }
}

/// A header's contents: the build, kind, sources' keys and every
/// section's name, length and hashes (not their offsets).
pub(super) fn content_id(h: &Header) -> u64 {
    let mut w = Writer::default();
    h.build.put(&mut w);
    h.kind.put(&mut w);
    h.sources.put(&mut w);
    for e in &h.sections {
        e.name.put(&mut w);
        e.len.put(&mut w);
        e.hashes.put(&mut w);
    }
    xxhash_rust::xxh3::xxh3_64(&w.buf)
}

/// Whether the cache file at `path` already holds `want`'s contents: its
/// header (read alone, not the sections) has the same build, kind and
/// sources' keys and every section of `want` with the same length and
/// hashes, and the file is long enough for them. Its sections' bytes are
/// checked when it is opened, as always.
pub(super) fn holds(path: &Path, want: &Header) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut head = [0u8; 28];
    if f.read_exact(&mut head).is_err() {
        return false;
    }
    let len = u64::from_le_bytes(head[12..20].try_into().unwrap_or_default());
    if len > 64 << 20 {
        return false;
    }
    let mut buf = head.to_vec();
    buf.resize(28 + len as usize, 0);
    if f.read_exact(&mut buf[28..]).is_err() {
        return false;
    }
    let Some(h) = CacheFile::header(&buf) else { return false };
    let size = f.metadata().map_or(0, |m| m.len());
    h.build == want.build
        && h.kind == want.kind
        && h.sources == want.sources
        && want.sections.iter().all(|w| {
            h.sections.iter().any(|e| {
                e.name == w.name && e.len == w.len && e.hashes == w.hashes && e.offset.saturating_add(e.len) <= size
            })
        })
}

/// Run a write into the cache folder on a thread of its own, which
/// [`flush`] waits for; false when no thread could be had.
pub fn spawn_writer(f: impl FnOnce() + Send + 'static) -> bool {
    // No thread to be had (the process at its limit): the write is skipped
    // with a warning, as a failed write is (the next run makes the file).
    let handle = match std::thread::Builder::new().name("cache writer".into()).spawn(f) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("cache: no thread for a write ({e}); not written");
            return false;
        }
    };
    let mut w = WRITERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    w.retain(|h| !h.is_finished());
    w.push(handle);
    true
}

/// A cache file written now, on this thread, in the folder `dir`, by the
/// build `build` (tests).
#[doc(hidden)]
pub fn write_now(
    dir: &Path,
    root: &Path,
    kind: &str,
    sources: &[String],
    sections: &[(String, Bytes)],
    build: BuildId,
) -> std::io::Result<()> {
    let header = header_for(root, kind, sources, sections, build.0)?;
    write_header(dir, kind, header, sections)
}

/// [`write_now`] as the build `build` would write it (tests).
#[cfg(test)]
pub(super) fn write_build(
    dir: &Path,
    root: &Path,
    kind: &str,
    sources: &[String],
    sections: &[(String, Bytes)],
    build: u64,
) -> std::io::Result<()> {
    write_now(dir, root, kind, sources, sections, BuildId(build))
}

/// The header of a cache file of `kind` over `sources` with `sections`:
/// the sources' keys (sizes, times, content hashes) taken now and the
/// sections' hashes; offsets not yet laid out.
pub(super) fn header_for(
    root: &Path,
    kind: &str,
    sources: &[String],
    sections: &[(String, Bytes)],
    build: u64,
) -> std::io::Result<Header> {
    let _t = crate::prof::span("cache: keys");
    let mut keys = Vec::new();
    for rel in sources {
        let p = source_path(root, rel);
        let before = stat(&p).ok_or_else(|| std::io::Error::other(format!("{rel} unreadable")))?;
        let hash = content_hash(&p).ok_or_else(|| std::io::Error::other(format!("{rel} unreadable")))?;
        // A file changing while it is hashed is not cached.
        if stat(&p) != Some(before) {
            return Err(std::io::Error::other(format!("{rel} changed while caching")));
        }
        keys.push(SourceFile { rel: rel.clone(), size: before.0, mtime_ns: before.1, ino: before.2, hash });
    }
    let entries: Vec<SectionEntry> = {
        use rayon::prelude::*;
        sections
            .par_iter()
            .map(|(name, b)| SectionEntry {
                name: name.clone(),
                offset: 0,
                len: b.len() as u64,
                hashes: chunk_hashes(b),
            })
            .collect()
    };
    Ok(Header { build, kind: kind.into(), sources: keys, sections: entries })
}

/// `header`'s file written with `sections` (in its order) and put in place.
fn write_header(dir: &Path, kind: &str, mut header: Header, sections: &[(String, Bytes)]) -> std::io::Result<()> {
    use std::io::Write;
    let _t = crate::prof::span("cache: write");
    // The offsets depend on the header's length, which does not depend on
    // their values (fixed-width): lay out with zeros, then fill in.
    let header_len = {
        let mut w = Writer::default();
        header.put(&mut w);
        w.buf.len()
    };
    let mut at = (28 + header_len).next_multiple_of(SECTION_ALIGN);
    for e in &mut header.sections {
        e.offset = at as u64;
        at = (at + e.len as usize).next_multiple_of(SECTION_ALIGN);
    }
    let mut hw = Writer::default();
    header.put(&mut hw);
    debug_assert_eq!(hw.buf.len(), header_len);
    std::fs::create_dir_all(dir)?;
    let name = file_name_of(kind, header.build);
    let result = (|| {
        let (file, named) = TempFile::create(dir, &name)?;
        let mut f = std::io::BufWriter::with_capacity(1 << 20, &file);
        f.write_all(MAGIC)?;
        f.write_all(&FORMAT.to_le_bytes())?;
        f.write_all(&(hw.buf.len() as u64).to_le_bytes())?;
        f.write_all(&xxhash_rust::xxh3::xxh3_64(&hw.buf).to_le_bytes())?;
        f.write_all(&hw.buf)?;
        let mut pos = 28 + hw.buf.len();
        for (e, (_, b)) in header.sections.iter().zip(sections) {
            f.write_all(&vec![0u8; e.offset as usize - pos])?;
            f.write_all(b)?;
            pos = (e.offset + e.len) as usize;
        }
        f.flush()?;
        drop(f);
        // Not synced (the slow exit): every section is checked
        // against its chunk hashes when the file is opened
        // (`sections_intact`), so a file the system lost part of after a
        // crash is found corrupt and made again. An fsync here held the
        // game's exit for 20-30 s on a busy disk (a thread in fsync
        // cannot be killed; nor can one that starts the write-back itself,
        // `sync_file_range`, which waits in the block layer's throttle):
        // the system writes it back.
        named.finish(&file, &dir.join(&name))
    })();
    if result.is_ok() {
        prune(dir, &name);
    }
    result
}
