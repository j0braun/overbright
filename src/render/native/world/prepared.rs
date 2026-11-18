//! The world renderer's upload-ready data: the vertex and index buffers'
//! bytes and the texture arrays' levels, rows padded to wgpu's copy
//! alignment (so each level goes up in one copy), built from the world or
//! taken from the track's cache (`crate::cache`), where a first load puts
//! them. Construction only: what is drawn is the same either way.

use std::collections::HashMap;

use crate::cache::{Bytes, CacheFile, Persist};
use crate::world::World;

/// One level of an array: every layer's rows, `bytes_per_row` apart
/// (a multiple of 256), `rows` rows a layer.
#[derive(Clone)]
pub(crate) struct Level {
    pub bytes_per_row: u32,
    pub rows: u32,
    /// The copy's size in texels (whole blocks).
    pub extent: (u32, u32),
    pub data: Bytes,
}

#[derive(Clone)]
pub(crate) struct Array {
    /// Format, width, height, mip count.
    pub key: (wgpu::TextureFormat, u32, u32, u32),
    pub layers: u32,
    /// The levels to write (from the top; a level no layer has data for
    /// ends them, the rest stay zero as the texture starts).
    pub levels: Vec<Level>,
}

#[derive(Clone)]
pub(crate) struct Arrays {
    pub arrays: Vec<Array>,
    /// Per slot: its array and layer.
    pub slots: Vec<(u32, u32)>,
}

impl Arrays {
    /// The levels' bytes (rows padded), about what the GPU holds.
    pub(crate) fn bytes(&self) -> usize {
        self.arrays.iter().flat_map(|a| &a.levels).map(|l| l.data.len()).sum()
    }
}

/// The arrays for `sources` (a slot each; None the white stand-in): the
/// slots grouped by shape in slot order, at most `max_layers` an array.
pub(crate) fn arrays(sources: &[Option<&crate::formats::texture::Texture>], bc: bool, max_layers: usize) -> Arrays {
    arrays_impl(sources, bc, max_layers, None)
}

pub(crate) fn arrays_with_pack(
    sources: &[Option<&crate::formats::texture::Texture>],
    bc: bool,
    max_layers: usize,
    max_dimension: u32,
) -> Arrays {
    arrays_impl(sources, bc, max_layers, Some(max_dimension))
}

fn arrays_impl(
    sources: &[Option<&crate::formats::texture::Texture>],
    bc: bool,
    max_layers: usize,
    max_dimension: Option<u32>,
) -> Arrays {
    use rayon::prelude::*;
    let t_layers = crate::prof::span("prepared::arrays: layers");
    let layers: Vec<_> = sources
        .par_iter()
        .map(|&t| match max_dimension {
            Some(max) => crate::render::texture_layer_with_pack(crate::render::texture_pack::Domain::World, t, bc, max),
            None => crate::render::texture_layer(t, bc),
        })
        .collect();
    drop(t_layers);
    let _t_levels = crate::prof::span("prepared::arrays: levels");
    let mut shapes: Vec<((wgpu::TextureFormat, u32, u32, u32), Vec<usize>)> = Vec::new();
    let mut shape_of: HashMap<(wgpu::TextureFormat, u32, u32, u32), usize> = HashMap::new();
    for (slot, (key, _)) in layers.iter().enumerate() {
        let i = *shape_of.entry(*key).or_insert_with(|| {
            shapes.push((*key, Vec::new()));
            shapes.len() - 1
        });
        shapes[i].1.push(slot);
    }
    let chunks: Vec<((wgpu::TextureFormat, u32, u32, u32), &[usize])> =
        shapes.iter().flat_map(|(key, slots)| slots.chunks(max_layers.max(1)).map(move |c| (*key, c))).collect();
    let mut slots = vec![(0, 0); sources.len()];
    for (array, (_, chunk)) in chunks.iter().enumerate() {
        for (layer, &slot) in chunk.iter().enumerate() {
            slots[slot] = (array as u32, layer as u32);
        }
    }
    let arrays = chunks
        .par_iter()
        .map(|&(key, chunk)| {
            let data: Vec<&[u8]> = chunk.iter().map(|&slot| &layers[slot].1[..]).collect();
            Array { key, layers: chunk.len() as u32, levels: levels(key, &data) }
        })
        .collect();
    Arrays { arrays, slots }
}

/// A one-layer array of `bytes`.
pub(crate) fn single(key: (wgpu::TextureFormat, u32, u32, u32), bytes: &[u8]) -> Array {
    Array { key, layers: 1, levels: levels(key, &[bytes]) }
}

/// Per level: every layer's bytes of it gathered, rows padded (zeros where
/// a layer's data runs short, as the texture starts), until a level no
/// layer has.
fn levels((format, width, height, mips): (wgpu::TextureFormat, u32, u32, u32), layers: &[&[u8]]) -> Vec<Level> {
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None).unwrap_or(4);
    let mut out = Vec::new();
    let mut offset = 0usize;
    for mip in 0..mips {
        let (w, h) = ((width >> mip).max(1), (height >> mip).max(1));
        let (cols, rows) = (w.div_ceil(bw), h.div_ceil(bh));
        let row = (cols * block) as usize;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        let size = row * rows as usize;
        let image = padded * rows as usize;
        let mut level = vec![0u8; image * layers.len()];
        let mut any = false;
        for (layer, bytes) in layers.iter().enumerate() {
            if let Some(b) = bytes.get(offset..offset + size) {
                for (r, src) in b.chunks_exact(row).enumerate() {
                    let at = layer * image + r * padded;
                    level[at..at + row].copy_from_slice(src);
                }
                any = true;
            }
        }
        offset += size;
        if !any {
            break;
        }
        out.push(Level { bytes_per_row: padded as u32, rows, extent: (cols * bw, rows * bh), data: level.into() });
    }
    out
}

/// All solids' vertices and indices back to back (the indices padded to
/// a 4-byte multiple, as buffer sizes must be).
pub(crate) fn geometry(world: &World) -> (Bytes, Bytes) {
    let mut vertices: Vec<u8> = Vec::with_capacity(
        world.solids.iter().map(|s| s.vertices.len() * std::mem::size_of::<crate::formats::solid::Vertex>()).sum(),
    );
    let mut indices: Vec<u16> = Vec::with_capacity(world.solids.iter().map(|s| s.indices.len()).sum::<usize>() + 1);
    for solid in &world.solids {
        vertices.extend_from_slice(bytemuck::cast_slice(&solid.vertices));
        indices.extend_from_slice(&solid.indices);
    }
    if indices.len() % 2 == 1 {
        indices.push(0);
    }
    (vertices.into(), bytemuck::cast_slice::<u16, u8>(&indices).into())
}

/// The vertex and index buffers' bytes in a track's cache: the world's
/// ("w.vertices" / "w.indices", which [`geometry`] makes from the world),
/// unless the renderer stored its own because they differed.
pub(crate) fn file_geometry(file: &CacheFile) -> Option<(Bytes, Bytes)> {
    let get = |gpu: &str, world: &str| file.bytes(gpu).or_else(|| file.bytes(world));
    Some((get("gpu.vertices", "w.vertices")?, get("gpu.indices", "w.indices")?))
}

/// Per texture of `sources` whose bytes the arrays hold verbatim (blocks
/// uploaded as they are, every level whole): where they lie in the cache's
/// "gpu.a*" sections, so the file need not keep them twice. Each is
/// checked by gathering it back.
pub(crate) fn texture_map(
    arrays: &Arrays,
    sources: &[Option<&crate::formats::texture::Texture>],
) -> Vec<(u32, Vec<crate::cache::TexturePiece>)> {
    use rayon::prelude::*;
    sources
        .par_iter()
        .enumerate()
        .filter_map(|(slot, t)| {
            let t = (*t)?;
            let &(a, layer) = arrays.slots.get(slot)?;
            let array = arrays.arrays.get(a as usize)?;
            let (bw, _) = array.key.0.block_dimensions();
            let block = array.key.0.block_copy_size(None)?;
            let mut pieces = Vec::new();
            let mut bytes = Vec::with_capacity(t.data.len());
            for (m, l) in array.levels.iter().enumerate() {
                let image = l.bytes_per_row as usize * l.rows as usize;
                let row = (l.extent.0 / bw * block) as usize;
                let p = crate::cache::GatherPiece {
                    start: layer as usize * image,
                    row,
                    stride: l.bytes_per_row as usize,
                    rows: l.rows as usize,
                };
                if p.start + image > l.data.len() {
                    return None;
                }
                bytes.extend(crate::cache::gather(&l.data, &[p]));
                pieces.push(crate::cache::TexturePiece {
                    section: format!("gpu.a{a}.m{m}"),
                    offset: p.start as u64,
                    row: row as u64,
                    stride: p.stride as u64,
                    rows: p.rows as u64,
                });
            }
            (bytes[..] == t.data[..]).then_some((t.hash, pieces))
        })
        .collect()
}

/// What the cache's renderer data was made for: it is used only by a
/// device that would make the same.
#[derive(Clone, PartialEq)]
struct Meta {
    bc: bool,
    max_layers: u32,
    /// Per slot: its texture's hash (None: the white stand-in).
    slot_hashes: Vec<Option<u32>>,
    slots: Vec<(u32, u32)>,
    /// Per array: format code, width, height, mips, layers; per level its
    /// bytes per row, rows and extent.
    arrays: Vec<([u32; 5], Vec<[u32; 4]>)>,
}
crate::persist!(Meta { bc, max_layers, slot_hashes, slots, arrays });

const FORMATS: [wgpu::TextureFormat; 5] = [
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Bc1RgbaUnorm,
    wgpu::TextureFormat::Bc2RgbaUnorm,
    wgpu::TextureFormat::Bc3RgbaUnorm,
    // Overbright packs' compiled PNGs (pack-applied arrays only).
    wgpu::TextureFormat::Bc7RgbaUnorm,
];

/// The cache's sections for this data.
pub(crate) fn to_sections(
    geometry: &(Bytes, Bytes),
    arrays: &Arrays,
    slot_hashes: &[Option<u32>],
    bc: bool,
    max_layers: usize,
) -> Option<Vec<(String, Bytes)>> {
    let mut meta = Meta {
        bc,
        max_layers: max_layers as u32,
        slot_hashes: slot_hashes.to_vec(),
        slots: arrays.slots.clone(),
        arrays: Vec::new(),
    };
    let mut out =
        vec![("gpu.vertices".to_string(), geometry.0.clone()), ("gpu.indices".to_string(), geometry.1.clone())];
    for (i, a) in arrays.arrays.iter().enumerate() {
        let format = FORMATS.iter().position(|&f| f == a.key.0)? as u32;
        meta.arrays.push((
            [format, a.key.1, a.key.2, a.key.3, a.layers],
            a.levels.iter().map(|l| [l.bytes_per_row, l.rows, l.extent.0, l.extent.1]).collect(),
        ));
        for (m, l) in a.levels.iter().enumerate() {
            out.push((format!("gpu.a{i}.m{m}"), l.data.clone()));
        }
    }
    let mut w = crate::cache::Writer::default();
    meta.put(&mut w);
    out.push(("gpu.meta".to_string(), w.buf.into()));
    Some(out)
}

/// The cache's renderer data, if it has some made for this device.
pub(crate) struct Cached {
    meta: Meta,
    pub geometry: (Bytes, Bytes),
    arrays: Vec<Array>,
}

impl Cached {
    pub(crate) fn open(file: &CacheFile, bc: bool, max_layers: usize) -> Option<Cached> {
        let (meta, arrays) = read_arrays(file, bc, max_layers)?;
        Some(Cached { geometry: file_geometry(file)?, meta, arrays })
    }

    /// The arrays with this run's slots (`slot_hashes`, each slot's
    /// texture) found in them by texture; None if one is missing.
    pub(crate) fn arrays(&self, slot_hashes: &[Option<u32>]) -> Option<Arrays> {
        Some(Arrays { arrays: self.arrays.clone(), slots: slots_of(&self.meta, slot_hashes)? })
    }

    /// The same data made from `world` now, slot for slot as cached
    /// (`--verify-cache`): whether every byte matches.
    pub(crate) fn matches(&self, world: &World) -> bool {
        let sources: Option<Vec<Option<&crate::formats::texture::Texture>>> = self
            .meta
            .slot_hashes
            .iter()
            .map(|h| match h {
                None => Some(None),
                Some(h) => world.textures.get(h).map(Some),
            })
            .collect();
        let Some(sources) = sources else { return false };
        let fresh = arrays(&sources, self.meta.bc, self.meta.max_layers as usize);
        let geometry = geometry(world);
        geometry.0[..] == self.geometry.0[..]
            && geometry.1[..] == self.geometry.1[..]
            && fresh.slots == self.meta.slots
            && fresh.arrays.len() == self.arrays.len()
            && fresh.arrays.iter().zip(&self.arrays).all(|(a, b)| {
                a.key == b.key
                    && a.layers == b.layers
                    && a.levels.len() == b.levels.len()
                    && a.levels.iter().zip(&b.levels).all(|(x, y)| {
                        (x.bytes_per_row, x.rows, x.extent) == (y.bytes_per_row, y.rows, y.extent)
                            && x.data[..] == y.data[..]
                    })
            })
    }
}

/// A cache file's arrays ("gpu.meta", "gpu.a*"), if made for this device.
fn read_arrays(file: &CacheFile, bc: bool, max_layers: usize) -> Option<(Meta, Vec<Array>)> {
    let mut r = file.reader("gpu.meta")?;
    let meta = Meta::get(&mut r).ok()?;
    if !r.done() || meta.bc != bc || meta.max_layers != max_layers as u32 {
        return None;
    }
    let mut arrays = Vec::new();
    for (i, ([format, w, h, mips, layers], levels)) in meta.arrays.iter().enumerate() {
        let format = *FORMATS.get(*format as usize)?;
        let mut out = Vec::new();
        for (m, &[bytes_per_row, rows, ew, eh]) in levels.iter().enumerate() {
            let data = file.bytes(&format!("gpu.a{i}.m{m}"))?;
            if data.len() != bytes_per_row as usize * rows as usize * *layers as usize {
                return None;
            }
            out.push(Level { bytes_per_row, rows, extent: (ew, eh), data });
        }
        arrays.push(Array { key: (format, *w, *h, *mips), layers: *layers, levels: out });
    }
    if meta.slots.len() != meta.slot_hashes.len()
        || meta.slots.iter().any(|&(a, l)| arrays.get(a as usize).is_none_or(|x| l >= x.layers))
    {
        return None;
    }
    Some((meta, arrays))
}

/// The slots `slot_hashes` found in arrays laid out for `meta`'s slots, by
/// texture; None if one is missing.
fn slots_of(meta: &Meta, slot_hashes: &[Option<u32>]) -> Option<Vec<(u32, u32)>> {
    let index: HashMap<Option<u32>, usize> = meta.slot_hashes.iter().enumerate().map(|(i, &h)| (h, i)).collect();
    slot_hashes.iter().map(|h| index.get(h).map(|&i| meta.slots[i])).collect()
}

// ---------------------------------------------------------------------------
// docs/texture-pack-cache.md: the world arrays with a texture pack
// applied, cached in a file of their own beside the track's
// (`TrackCache::pack_file`), checked against the pack's fingerprint
// (`texture_pack::fingerprint`, section "pack.key"). The track's own cache
// keeps the native arrays: turning the pack off finds them unchanged.

/// The pack-applied arrays in `file` for these slots, if the file was made
/// for this device and for the pack as it is now (`key`).
pub(crate) fn pack_arrays(
    file: &CacheFile,
    bc: bool,
    max_layers: usize,
    key: &str,
    slot_hashes: &[Option<u32>],
) -> Option<Arrays> {
    if file.bytes("pack.key")?[..] != *key.as_bytes() {
        log::info!("pack cache: the pack changed, rebuilt");
        return None;
    }
    let (meta, arrays) = read_arrays(file, bc, max_layers)?;
    Some(Arrays { slots: slots_of(&meta, slot_hashes)?, arrays })
}

/// The sections of a pack-applied arrays' file: the arrays as
/// [`to_sections`] stores them (no geometry: the track's file has it) and
/// the pack's fingerprint.
pub(crate) fn pack_sections(
    arrays: &Arrays,
    slot_hashes: &[Option<u32>],
    bc: bool,
    max_layers: usize,
    key: &str,
) -> Option<Vec<(String, Bytes)>> {
    let mut out = to_sections(&(Bytes::new(), Bytes::new()), arrays, slot_hashes, bc, max_layers)?;
    out.retain(|(name, _)| name != "gpu.vertices" && name != "gpu.indices");
    out.push(("pack.key".to_string(), key.as_bytes().into()));
    Some(out)
}

/// `--verify-cache` for a track's renderer data: None when the cache has
/// none, else whether it equals a fresh build from `world`.
pub fn verify(file: &CacheFile, world: &World) -> Option<bool> {
    let mut r = file.reader("gpu.meta")?;
    let meta = Meta::get(&mut r).ok()?;
    Some(Cached::open(file, meta.bc, meta.max_layers as usize).is_some_and(|c| c.matches(world)))
}
