//! Source file bytes (mapped or read) and the shared, refcounted `Bytes`.

use super::*;

/// A source file's bytes: mapped read-only when the system allows it (a
/// mapping's pages come straight from the page cache, where a read copies
/// them into fresh memory, which on a 118 MB stream file costs ~100 ms of
/// page faults), else read.
///
/// The game's data is not written while the game runs; a mapping is held
/// only while a load parses the file.
pub enum SourceBytes {
    Mapped(memmap2::Mmap),
    Read(Vec<u8>),
}

impl SourceBytes {
    pub fn open(path: &Path) -> std::io::Result<SourceBytes> {
        let file = std::fs::File::open(path)?;
        // SAFETY: a read-only private mapping of a file the game only reads
        // (see above); on any failure the file is read instead.
        match unsafe { memmap2::MmapOptions::new().map(&file) } {
            Ok(m) => Ok(SourceBytes::Mapped(m)),
            Err(_) => std::fs::read(path).map(SourceBytes::Read),
        }
    }
}

impl std::ops::Deref for SourceBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            SourceBytes::Mapped(m) => m,
            SourceBytes::Read(v) => v,
        }
    }
}

/// Immutable bytes shared by every holder: a clone is a reference count,
/// not a copy. Texture pixels use it, so the shared car and rim texture
/// packs, GLOBALB's and the cache's mapped data are held once however many
/// cars use them.
#[derive(Clone)]
pub struct Bytes(BytesInner);

#[derive(Clone)]
pub(super) enum BytesInner {
    Owned(std::sync::Arc<[u8]>),
    /// A range of a mapped cache file.
    Mapped(std::sync::Arc<memmap2::Mmap>, std::ops::Range<usize>),
    /// Pieces of a mapped cache file, gathered into one buffer on first
    /// use (a texture whose pixels the file keeps only in the renderer's
    /// padded arrays, see [`TrackCache::store_gpu`]).
    Gathered(std::sync::Arc<Gather>),
}

/// Strided pieces of a mapping: per piece its start, `rows` rows of
/// `row` bytes, `stride` apart.
pub(crate) struct Gather {
    pub(super) map: std::sync::Arc<memmap2::Mmap>,
    pub(super) pieces: Vec<GatherPiece>,
    pub(super) len: usize,
    pub(super) done: std::sync::OnceLock<Box<[u8]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GatherPiece {
    pub start: usize,
    pub row: usize,
    pub stride: usize,
    pub rows: usize,
}

impl GatherPiece {
    pub(super) fn end(&self) -> Option<usize> {
        if self.rows == 0 {
            return Some(self.start);
        }
        self.stride.checked_mul(self.rows - 1)?.checked_add(self.row)?.checked_add(self.start)
    }
}

/// The bytes the pieces of `data` make, back to back.
pub fn gather(data: &[u8], pieces: &[GatherPiece]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pieces.iter().map(|p| p.row * p.rows).sum());
    for p in pieces {
        for r in 0..p.rows {
            let at = p.start + r * p.stride;
            out.extend_from_slice(&data[at..at + p.row]);
        }
    }
    out
}

impl Bytes {
    pub fn new() -> Bytes {
        Bytes(BytesInner::Owned(std::sync::Arc::from(&[][..])))
    }

    pub(crate) fn mapped(map: std::sync::Arc<memmap2::Mmap>, range: std::ops::Range<usize>) -> Bytes {
        assert!(range.end <= map.len());
        Bytes(BytesInner::Mapped(map, range))
    }

    /// The pieces of `map`, gathered when first read; None if one lies
    /// outside it.
    pub(crate) fn gathered(map: std::sync::Arc<memmap2::Mmap>, pieces: Vec<GatherPiece>) -> Option<Bytes> {
        if pieces.iter().any(|p| (p.rows > 1 && p.row > p.stride) || p.end().is_none_or(|e| e > map.len())) {
            return None;
        }
        let len = pieces.iter().map(|p| p.row * p.rows).sum();
        Some(Bytes(BytesInner::Gathered(std::sync::Arc::new(Gather { map, pieces, len, done: Default::default() }))))
    }

    /// The length, without gathering lazily gathered bytes.
    pub fn len(&self) -> usize {
        match &self.0 {
            BytesInner::Owned(a) => a.len(),
            BytesInner::Mapped(_, r) => r.len(),
            BytesInner::Gathered(g) => g.len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Bytes {
    fn default() -> Bytes {
        Bytes::new()
    }
}

impl std::ops::Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match &self.0 {
            BytesInner::Owned(a) => a,
            BytesInner::Mapped(m, r) => &m[r.clone()],
            BytesInner::Gathered(g) => g.done.get_or_init(|| gather(&g.map, &g.pieces).into_boxed_slice()),
        }
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Bytes {
        Bytes(BytesInner::Owned(v.into()))
    }
}

impl From<&[u8]> for Bytes {
    fn from(v: &[u8]) -> Bytes {
        Bytes(BytesInner::Owned(v.into()))
    }
}

impl PartialEq for Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        self[..] == other[..]
    }
}

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bytes({} bytes)", self.len())
    }
}

impl<'a> IntoIterator for &'a Bytes {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;
    fn into_iter(self) -> Self::IntoIter {
        self[..].iter()
    }
}
