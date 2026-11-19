//! Cache file framing: magic, header, section table and the integrity checks.

// ---------------------------------------------------------------------------
// Cache files.
//
// One file per cached thing (a track, a car's file) and build,
// `<kind>.<build id>.nfc`, in `$NFSU2_CACHE_DIR`, else the default folder
// (`platform::dirs::cache`: the player's game `$XDG_CACHE_HOME/overbright`,
// else `~/.cache/overbright`, `<Local AppData>\Overbright\cache` on
// Windows; every other program the developer cache `overbright-dev`).
// `NFSU2_NO_CACHE=1` turns it off. docs/asset-cache.md. Layout:
//
// ```text
// "NFSU2CCH" | format u32 | header length u64 | header hash u64 | header
// sections, each at a 256-byte aligned offset
// ```
//
// The header holds the build id (a hash of the library's sources, from
// build.rs), the kind, and per source file its path (under the game's
// folder), size, modification time, inode and content hash (XXH3-128), and
// per section its name, offset, length and an XXH3-64 per 4 MB.
//
// The build id is the only version: every library source and the enabled
// features are hashed into it (build.rs), so no change of what a kind
// stores can be read by a build without it. It is in the name, so two
// builds never replace each other's file, and in the header.
//
// A file is used only when the format, build id and kind match, every
// source file's size, time and inode match (or, when only the time or the
// inode changed, its content hash: the file is then rewritten with the new
// times), and every section's hashes match; anything else (missing, stale,
// corrupt, truncated) is a miss and the data is parsed from the game's
// files as without a cache. Files are written on a background thread, at
// most one per file and process, each contents once (`file::submit`), to a
// temporary file renamed (or linked) into place, so a reader never sees a
// half-written file, a mapping in use keeps the old one, and a writer
// killed mid-write leaves at most a temporary file the next start sweeps
// (see `TempFile`, `sweep_tmp`).

pub(super) const MAGIC: &[u8; 8] = b"NFSU2CCH";
/// 2: no duplicated texture pixels or vertices (`w.vertices`,
/// `w.indices`, `w.tdmap`), stale temporary files swept.
pub(super) const FORMAT: u32 = 2;
pub(super) const CHUNK: usize = 4 << 20;
/// Sections start at multiples of this: rows of texture levels padded to
/// wgpu's copy alignment stay aligned in the mapping.
pub const SECTION_ALIGN: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct SourceFile {
    pub rel: String,
    pub(super) size: u64,
    pub(super) mtime_ns: u64,
    pub(super) ino: u64,
    pub(super) hash: [u64; 2],
}
crate::persist!(SourceFile { rel, size, mtime_ns, ino, hash });

#[derive(Clone, Debug)]
pub(super) struct SectionEntry {
    pub(super) name: String,
    pub(super) offset: u64,
    pub(super) len: u64,
    pub(super) hashes: Vec<u64>,
}
crate::persist!(SectionEntry { name, offset, len, hashes });

#[derive(Clone, Debug)]
pub struct Header {
    pub(super) build: u64,
    pub kind: String,
    pub sources: Vec<SourceFile>,
    pub(super) sections: Vec<SectionEntry>,
}
crate::persist!(Header { build, kind, sources, sections });

/// Whether every section lies in the file and every 4 MB of it has its
/// hash. The chunks are read with positioned reads from several threads at
/// once, each hashed as it arrives: a single sequential reader (a mapping
/// faulted in, or `MAP_POPULATE`) gets only the readahead window's worth of
/// queue depth, and a cold track file then reads at a fraction of the
/// drive's speed. The reads leave the file in the page cache, where the
/// mapping's pages come from; the mapping itself is not touched.
pub(super) fn sections_intact(file: &std::fs::File, map: &[u8], sections: &[SectionEntry]) -> bool {
    let mut jobs: Vec<(usize, usize, u64)> = Vec::new();
    for e in sections {
        let (o, l) = (e.offset as usize, e.len as usize);
        if o.checked_add(l).is_none_or(|end| end > map.len()) || e.hashes.len() != l.div_ceil(CHUNK) {
            return false;
        }
        jobs.extend(e.hashes.iter().enumerate().map(|(i, &h)| (o + i * CHUNK, (l - i * CHUNK).min(CHUNK), h)));
    }
    let total: usize = jobs.iter().map(|j| j.1).sum();
    let threads = if total < 2 * CHUNK {
        1
    } else {
        std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, READERS).min(jobs.len())
    };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let bad = std::sync::atomic::AtomicBool::new(false);
    let work = || {
        let mut buf = Vec::new();
        loop {
            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some(&(at, len, hash)) = jobs.get(i) else { break };
            if bad.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let ok = match read_at(file, &mut buf, at, len) {
                Some(b) => xxhash_rust::xxh3::xxh3_64(b) == hash,
                // No positioned reads here: from the mapping.
                None => xxhash_rust::xxh3::xxh3_64(&map[at..at + len]) == hash,
            };
            if !ok {
                bad.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    };
    if threads <= 1 {
        work();
    } else {
        std::thread::scope(|s| {
            for _ in 1..threads {
                s.spawn(work);
            }
            work();
        });
    }
    !bad.load(std::sync::atomic::Ordering::Relaxed)
}

/// At most this many threads read a cache file at once (8 already keep an
/// NVMe drive's queue busy; 16 gained ~10% more in the research runs).
pub(super) const READERS: usize = 8;

/// `len` bytes of `file` at `at`, read into `buf`; None where the platform
/// has no positioned reads or the read fails (the caller then uses the
/// mapping, which reports a short file the same way).
pub(super) fn read_at<'b>(file: &std::fs::File, buf: &'b mut Vec<u8>, at: usize, len: usize) -> Option<&'b [u8]> {
    buf.resize(len, 0);
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(file, &mut buf[..], at as u64).ok()?;
        Some(&buf[..])
    }
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < len {
            match std::os::windows::fs::FileExt::seek_read(file, &mut buf[done..], (at + done) as u64) {
                Ok(0) | Err(_) => return None,
                Ok(n) => done += n,
            }
        }
        Some(&buf[..])
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, at);
        None
    }
}
