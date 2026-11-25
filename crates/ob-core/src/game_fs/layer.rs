//! The asset layer: the game's file reads answered from a mounted base set
//! (docs/modding/asset-format/import.md §13 "Reading through the asset
//! layer"), opt-in.
//!
//! Without a mounted layer (the default) every function here is the plain
//! file read it replaces. With one ([`mount`], once at start), a game file —
//! named by a path under the mounted game folder, or by its name relative to
//! it — is served from the layer when the layer holds it (the bytes are a
//! slice of a mapped bundle, never a copy until a caller asks for one), and
//! from the folder otherwise. The layer holds the original's bytes, so the
//! parsers above see the same input either way.
//!
//! Names are matched without ASCII case and with `/` or `\` (as the
//! original's file names, [`super`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// A game file's bytes inside a mapping (a stored bundle entry).
#[derive(Clone)]
pub struct Mapped {
    map: Arc<memmap2::Mmap>,
    range: std::ops::Range<usize>,
}

impl Mapped {
    /// `range` of `map`; None when it lies outside.
    pub fn new(map: Arc<memmap2::Mmap>, range: std::ops::Range<usize>) -> Option<Mapped> {
        (range.start <= range.end && range.end <= map.len()).then_some(Mapped { map, range })
    }
}

impl std::ops::Deref for Mapped {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.map[self.range.clone()]
    }
}

/// What a mounted layer answers.
pub trait Layer: Send + Sync {
    /// The bytes of the game file `key` (its name relative to the game
    /// folder, upper case, `/` separated: `GLOBAL/GLOBALB.LZC`), when the
    /// layer holds it.
    fn get(&self, key: &str) -> Option<Mapped>;
    /// Whether the game file `key`, which [`Layer::get`] does not hold, was
    /// not there when the layer was made (a name in a folder it took whole):
    /// the read is answered "no such file" without asking the folder.
    fn absent(&self, _key: &str) -> bool {
        false
    }
    /// A short name for log lines (the base set's id).
    fn name(&self) -> &str;
}

struct Mount {
    /// The game folder as given and, when it differs, as resolved.
    roots: Vec<PathBuf>,
    layer: Arc<dyn Layer>,
}

static MOUNT: RwLock<Option<Mount>> = RwLock::new(None);
static FROM_LAYER: AtomicU64 = AtomicU64::new(0);
static LAYER_BYTES: AtomicU64 = AtomicU64::new(0);
static FROM_FOLDER: AtomicU64 = AtomicU64::new(0);
static ABSENT: AtomicU64 = AtomicU64::new(0);
static MISSED: std::sync::Mutex<std::collections::BTreeMap<String, u64>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The game files asked for that the layer did not answer, with how often.
pub fn misses() -> Vec<(String, u64)> {
    MISSED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().map(|(k, n)| (k.clone(), *n)).collect()
}

/// Mounts `layer` over the game folder `root`: from now on the reads below
/// take `root`'s files from it where it has them.
pub fn mount(root: &Path, layer: Arc<dyn Layer>) {
    let mut roots = vec![root.to_path_buf()];
    if let Ok(c) = std::fs::canonicalize(root)
        && c != root
    {
        roots.push(c);
    }
    log::info!("asset layer: game files from base set {} over {}", layer.name(), root.display());
    *MOUNT.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Mount { roots, layer });
}

/// Takes the layer down (tests; the game mounts once and keeps it).
pub fn unmount() {
    *MOUNT.write().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// The mounted layer's name, if one is mounted.
pub fn mounted() -> Option<String> {
    MOUNT.read().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|m| m.layer.name().to_string())
}

/// Files and bytes served by the layer, and game files read from the folder
/// while a layer was mounted (the exit log line; the gate's layer mode).
pub fn counts() -> (u64, u64, u64) {
    (FROM_LAYER.load(Ordering::Relaxed), LAYER_BYTES.load(Ordering::Relaxed), FROM_FOLDER.load(Ordering::Relaxed))
}

/// Reads the layer answered "no such file" (names the original asks for
/// that the install does not have).
pub fn absent_count() -> u64 {
    ABSENT.load(Ordering::Relaxed)
}

/// The layer's key of `rel` (refused names have none).
fn key(rel: &str) -> Option<String> {
    let parts = super::components(rel).ok()?;
    (!parts.is_empty()).then(|| parts.join("/").to_ascii_uppercase())
}

/// The key of `path` when it lies under the mounted game folder.
fn key_of_path(m: &Mount, path: &Path) -> Option<String> {
    let rel = m.roots.iter().find_map(|r| path.strip_prefix(r).ok())?;
    key(rel.to_str()?)
}

/// What the layer answers for one game file.
pub enum Lookup {
    /// The layer's bytes.
    Held(Mapped),
    /// The layer knows there is no such file (the original finds none
    /// either): "not found", the folder is not asked.
    Absent,
    /// Not the layer's (no layer, outside the game folder, or a name it
    /// cannot answer): read the folder.
    Folder,
}

impl Lookup {
    /// The bytes, `Err(NotFound)` for [`Lookup::Absent`], or None (read the
    /// folder).
    fn bytes(self, what: &dyn std::fmt::Display) -> Option<std::io::Result<Mapped>> {
        match self {
            Lookup::Held(b) => Some(Ok(b)),
            Lookup::Absent => Some(Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{what}: no such game file (not in the base set)"),
            ))),
            Lookup::Folder => None,
        }
    }
}

fn served(m: &Mount, k: &str) -> Lookup {
    if let Some(b) = m.layer.get(k) {
        FROM_LAYER.fetch_add(1, Ordering::Relaxed);
        LAYER_BYTES.fetch_add(b.len() as u64, Ordering::Relaxed);
        return Lookup::Held(b);
    }
    if m.layer.absent(k) {
        ABSENT.fetch_add(1, Ordering::Relaxed);
        return Lookup::Absent;
    }
    FROM_FOLDER.fetch_add(1, Ordering::Relaxed);
    *MISSED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(k.to_string()).or_default() += 1;
    Lookup::Folder
}

/// What the layer answers for the game file at `path` (a path under the
/// game folder, as [`super::path`] gives).
pub fn lookup(path: &Path) -> Lookup {
    let g = MOUNT.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(m) = g.as_ref() else { return Lookup::Folder };
    match key_of_path(m, path) {
        Some(k) => served(m, &k),
        None => Lookup::Folder,
    }
}

/// The layer's bytes of the game file at `path`; None without a layer,
/// outside the game folder, or when the layer does not hold it.
pub fn layered(path: &Path) -> Option<Mapped> {
    match lookup(path) {
        Lookup::Held(b) => Some(b),
        _ => None,
    }
}

/// What the layer answers for the game file `rel` under `root`.
pub(super) fn lookup_rel(root: &Path, rel: &str) -> Lookup {
    let g = MOUNT.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(m) = g.as_ref() else { return Lookup::Folder };
    let k = match key_of_path(m, root) {
        Some(base) => key(&format!("{base}/{rel}")),
        None if m.roots.iter().any(|r| r == root) => key(rel),
        None => None,
    };
    match k {
        Some(k) => served(m, &k),
        None => Lookup::Folder,
    }
}

/// [`lookup_rel`] as bytes ([`Lookup::bytes`]).
pub(super) fn layered_rel(root: &Path, rel: &str) -> Option<std::io::Result<Mapped>> {
    lookup_rel(root, rel).bytes(&rel)
}

/// `std::fs::read` of a game file at `path`: from the layer when mounted and
/// it holds the file, else from disk.
pub fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    match lookup(path).bytes(&path.display()) {
        Some(b) => b.map(|b| b.to_vec()),
        None => std::fs::read(path),
    }
}

/// A game file's bytes without a copy: the layer's slice, else the file
/// mapped (read when it cannot be mapped).
pub enum FileBytes {
    Layer(Mapped),
    Mapped(memmap2::Mmap),
    Read(Vec<u8>),
}

impl std::ops::Deref for FileBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            FileBytes::Layer(m) => m,
            FileBytes::Mapped(m) => m,
            FileBytes::Read(v) => v,
        }
    }
}

/// The game file at `path`, mapped ([`FileBytes`]).
pub fn open_file(path: &Path) -> std::io::Result<FileBytes> {
    if let Some(b) = lookup(path).bytes(&path.display()) {
        return b.map(FileBytes::Layer);
    }
    let file = std::fs::File::open(path)?;
    // SAFETY: a read-only private mapping of a file the game only reads; on
    // any failure the file is read instead.
    match unsafe { memmap2::MmapOptions::new().map(&file) } {
        Ok(m) => Ok(FileBytes::Mapped(m)),
        Err(_) => std::fs::read(path).map(FileBytes::Read),
    }
}

/// A seekable reader over the game file at `path` (streamed audio, movies):
/// the layer's bytes, else the file itself.
pub enum FileReader {
    Layer(std::io::Cursor<Mapped>),
    File(std::fs::File),
}

impl AsRef<[u8]> for Mapped {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl std::io::Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            FileReader::Layer(c) => c.read(buf),
            FileReader::File(f) => f.read(buf),
        }
    }
}

impl std::io::Seek for FileReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            FileReader::Layer(c) => c.seek(pos),
            FileReader::File(f) => f.seek(pos),
        }
    }
}

/// `File::open` of a game file at `path`, through the layer.
pub fn open_reader(path: &Path) -> std::io::Result<FileReader> {
    match lookup(path).bytes(&path.display()) {
        Some(b) => b.map(|b| FileReader::Layer(std::io::Cursor::new(b))),
        None => std::fs::File::open(path).map(FileReader::File),
    }
}
