//! The tracks' cache (`TrackCache`, `TextureMap`).

use super::*;

// ---------------------------------------------------------------------------
// The tracks' cache: the parsed world ("w.*" sections) and the renderer's
// upload-ready data ("gpu.*", see `render::WorldRenderer::new`).

/// A track's cache as a load found or left it.
pub struct TrackCache {
    /// The build id of the crate that loads the world ([`BuildId`]).
    pub(super) build: BuildId,
    pub(super) root: std::path::PathBuf,
    pub(super) kind: String,
    pub(super) sources: Vec<String>,
    /// The file the world came from (a hit).
    pub(super) file: Option<CacheFile>,
    /// A miss: the world's sections, written once the renderer has added
    /// its own (or when the world goes, or at [`flush`]).
    pub(super) pending: std::sync::Mutex<Option<Vec<(String, Bytes)>>>,
}

pub(super) static PENDING: std::sync::Mutex<Vec<std::sync::Weak<TrackCache>>> = std::sync::Mutex::new(Vec::new());

/// Where a texture's pixels lie when the file keeps them only in the
/// renderer's arrays ("gpu.a*"): per texture hash, per mip level a piece
/// of a section (`rows` rows of `row` bytes, `stride` apart, from
/// `offset`). Written only for textures whose bytes the pieces give back
/// exactly (checked when written, see `prepared::texture_map`).
#[derive(Clone, Debug, PartialEq)]
pub struct TexturePiece {
    pub section: String,
    pub offset: u64,
    pub row: u64,
    pub stride: u64,
    pub rows: u64,
}
crate::persist!(TexturePiece { section, offset, row, stride, rows });

pub struct TextureMap(std::collections::HashMap<u32, Vec<TexturePiece>>);

impl TextureMap {
    pub const SECTION: &'static str = "w.tdmap";

    /// The file's map: Some(None) when it has none, None when it does not
    /// decode.
    pub fn read(file: &CacheFile) -> Option<Option<TextureMap>> {
        let Some(mut r) = file.reader(Self::SECTION) else { return Some(None) };
        let m = <std::collections::HashMap<u32, Vec<TexturePiece>> as Persist>::get(&mut r).ok()?;
        r.done().then_some(Some(TextureMap(m)))
    }

    pub fn section(entries: &[(u32, Vec<TexturePiece>)]) -> Bytes {
        let m: std::collections::HashMap<u32, Vec<TexturePiece>> = entries.iter().cloned().collect();
        let mut w = Writer::default();
        m.put(&mut w);
        w.buf.into()
    }

    /// Texture `hash`'s bytes, gathered from the file when first read.
    pub fn bytes(&self, file: &CacheFile, hash: u32) -> Option<Bytes> {
        let pieces = self
            .0
            .get(&hash)?
            .iter()
            .map(|p| {
                let e = file.entry(&p.section)?;
                let (offset, row, stride, rows) = (
                    usize::try_from(p.offset).ok()?,
                    usize::try_from(p.row).ok()?,
                    usize::try_from(p.stride).ok()?,
                    usize::try_from(p.rows).ok()?,
                );
                let piece = GatherPiece { start: (e.offset as usize).checked_add(offset)?, row, stride, rows };
                // Within its section.
                (piece.end()? <= (e.offset + e.len) as usize).then_some(piece)
            })
            .collect::<Option<Vec<_>>>()?;
        Bytes::gathered(file.map.clone(), pieces)
    }
}

impl TrackCache {
    pub fn kind(name: &str) -> String {
        format!("track-{name}")
    }

    /// The game folder and the region (`L4RA`, …) this world was loaded
    /// from (Light Probes, the region's route files).
    pub fn region(&self) -> Option<(&Path, &str)> {
        Some((&self.root, self.kind.strip_prefix("track-")?))
    }

    pub fn hit(build: BuildId, root: &Path, kind: String, sources: Vec<String>, file: CacheFile) -> std::sync::Arc<TrackCache> {
        std::sync::Arc::new(TrackCache {
            build,
            root: root.into(),
            kind,
            sources,
            file: Some(file),
            pending: Default::default(),
        })
    }

    pub fn miss(
        build: BuildId,
        root: &Path,
        kind: String,
        sources: Vec<String>,
        sections: Vec<(String, Bytes)>,
    ) -> std::sync::Arc<TrackCache> {
        let c = std::sync::Arc::new(TrackCache {
            build,
            root: root.into(),
            kind,
            sources,
            file: None,
            pending: std::sync::Mutex::new(Some(sections)),
        });
        let mut p = PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        p.retain(|w| w.strong_count() > 0);
        p.push(std::sync::Arc::downgrade(&c));
        c
    }

    pub fn file(&self) -> Option<&CacheFile> {
        self.file.as_ref()
    }

    /// docs/texture-pack-cache.md: the kind of the file beside this
    /// track's that keeps its world arrays with a texture pack applied
    /// (one per track, the last pack's; `prepared::pack_arrays` checks it).
    fn pack_kind(&self) -> String {
        format!("{}-pack", self.kind)
    }

    /// The pack-applied arrays' file, if there is a valid one over the
    /// track's game files (the pack's own files are in its "pack.key").
    pub fn pack_file(&self) -> Option<CacheFile> {
        let sources: Vec<&str> = self.sources.iter().map(String::as_str).collect();
        CacheFile::open(self.build, &self.root, &self.pack_kind(), &sources)
    }

    /// Write the pack-applied arrays' file (in the background, replacing
    /// the last pack's).
    pub fn store_pack(&self, sections: Vec<(String, Bytes)>) {
        let sources: Vec<&str> = self.sources.iter().map(String::as_str).collect();
        write(self.build, &self.root, &self.pack_kind(), &sources, sections);
    }

    /// The renderer's data for the cache: written with the world's
    /// sections, unless the file already has it. Each copy is stored once:
    /// the textures `textures` maps into the arrays lose their own "w.td.*"
    /// section, and the renderer's vertex and index buffers, when equal to
    /// the world's "w.vertices" / "w.indices", are not stored again.
    pub fn store_gpu(&self, mut gpu: Vec<(String, Bytes)>, textures: Vec<(u32, Vec<TexturePiece>)>) {
        let mut sections =
            match (self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take(), &self.file) {
                (Some(s), _) => s,
                // Names from the file's own header: `bytes` finds each one.
                (None, Some(f)) if !f.has("gpu.meta") => f
                    .header
                    .sections
                    .iter()
                    .filter(|e| e.name.starts_with("w."))
                    .map(|e| (e.name.clone(), f.bytes(&e.name).unwrap()))
                    .collect(),
                _ => return,
            };
        let world = |name: &str| sections.iter().find(|s| s.0 == name).map(|s| s.1.clone());
        let (wv, wi) = (world("w.vertices"), world("w.indices"));
        gpu.retain(|(name, b)| match name.as_str() {
            "gpu.vertices" => wv.as_ref().is_none_or(|w| w[..] != b[..]),
            "gpu.indices" => wi.as_ref().is_none_or(|w| w[..] != b[..]),
            _ => true,
        });
        if !textures.is_empty() && !sections.iter().any(|s| s.0 == TextureMap::SECTION) {
            let covered: std::collections::HashSet<String> =
                textures.iter().map(|(k, _)| format!("w.td.{k:08x}")).collect();
            sections.retain(|s| !covered.contains(&s.0));
            sections.push((TextureMap::SECTION.into(), TextureMap::section(&textures)));
        }
        sections.extend(gpu);
        self.write(sections);
    }

    fn write(&self, sections: Vec<(String, Bytes)>) {
        let sources: Vec<&str> = self.sources.iter().map(String::as_str).collect();
        write(self.build, &self.root, &self.kind, &sources, sections);
    }

    fn write_pending(&self) {
        if let Some(s) = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
            self.write(s);
        }
    }
}

impl Drop for TrackCache {
    fn drop(&mut self) {
        self.write_pending();
    }
}

/// Finish what is waiting to be written, then wait for every write in
/// flight (the game calls it before it exits, so a first run's cache is
/// complete).
pub fn flush() {
    flush_within(None);
}

/// [`flush`], waiting at most `limit` for the writes in flight (the
/// game's exit): what is not done by then is left behind (an
/// unfinished cache file is never given its name, see `TempFile`; the
/// next start makes it). Returns how many writes were left.
pub fn flush_within(limit: Option<std::time::Duration>) -> usize {
    let start = std::time::Instant::now();
    let left = flush_writers(limit);
    // The marks of use queued for the toucher thread (`touch_later`).
    let _ = wait_touches(limit.map(|l| l.saturating_sub(start.elapsed())));
    for (path, c) in write_counts() {
        log::debug!(
            "cache writes {}: {} written, {} repeated, {} already there, {} superseded, {} failed",
            path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()),
            c.written,
            c.repeated,
            c.present,
            c.superseded,
            c.failed
        );
    }
    left
}

fn flush_writers(limit: Option<std::time::Duration>) -> usize {
    let start = std::time::Instant::now();
    let pending: Vec<_> = std::mem::take(&mut *PENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    for c in pending.iter().filter_map(std::sync::Weak::upgrade) {
        c.write_pending();
    }
    let mut handles: Vec<_> = std::mem::take(&mut *WRITERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    if let Some(limit) = limit {
        while !handles.is_empty() && start.elapsed() < limit {
            handles.retain(|h| !h.is_finished());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        handles.retain(|h| !h.is_finished());
        return handles.len();
    }
    for h in handles {
        let _ = h.join();
    }
    0
}
