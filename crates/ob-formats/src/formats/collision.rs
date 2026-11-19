//! Track collision (STREAM<track>.BUN, one `0x80034130` pack per section;
//! read by 0x5D8150, faces turned into polygons by 0x5D7F20 / 0x5D2CD0).
//!
//! * `0x34131` header (16-byte aligned): section number (u16), two bytes,
//!   grid width and height (u16 each, 32 m cells), the grid origin (x, y),
//!   then the bounds min and max (x, y, z, pad).
//! * `0x34134` faces, 32 bytes each:
//!   * byte 0: barrier group — 0 always present, otherwise only while the
//!     group is switched on (see [`parse_barrier_groups`]; checked at
//!     0x5E1037); byte 1: which corners give the face's x/y bounds
//!     (2 bits each);
//!   * byte 2: surface; byte 3: flags — 0x02 wall, 0x04 the wall reaches
//!     ±2048 m, 0x08 a second floor class, 0x10 four corners (else three);
//!   * i16 at 4: base height in metres;
//!   * i16 at 6: the game's cache slot (0xFFFF on disk);
//!   * x[4] and y[4] in i16 eighths of a metre, z[4] in i16 256ths of a
//!     metre above the base height.
//! * `0x34135` u16s, `0x34136` face references and `0x34137` grid cells
//!   (u16 start, u16 count): the game's lookup, rebuilt by the clone.

use glam::Vec3;

use super::bundle::Chunk;
use super::reader::Reader;

pub const COLLISION_PACK: u32 = 0x8003_4130;
const HEADER: u32 = 0x0003_4131;
const FACES: u32 = 0x0003_4134;

pub const FLAG_WALL: u8 = 0x02;
pub const FLAG_TALL: u8 = 0x04;
pub const FLAG_QUAD: u8 = 0x10;

#[derive(Clone, Copy, Debug)]
pub struct CollisionFace {
    /// Corners, world space; only the first `corners` are used.
    pub vertices: [Vec3; 4],
    pub corners: usize,
    pub surface: u8,
    pub flags: u8,
    /// Barrier group; 0 = always present.
    pub group: u8,
    /// Byte 1: which corners give the x-min, y-min, x-max and y-max.
    pub bounds: u8,
    /// The corners as stored, eighths of a metre.
    pub x: [i16; 4],
    pub y: [i16; 4],
}

impl CollisionFace {
    pub fn is_wall(&self) -> bool {
        self.flags & FLAG_WALL != 0
    }
}

/// The section number and faces of one collision pack.
pub fn parse_collision_pack(pack: Chunk) -> (Option<u16>, Vec<CollisionFace>) {
    let section = pack.child(HEADER).and_then(|h| Reader::new(h.aligned(16)).u16_at(0).ok());
    let mut faces = Vec::new();
    if let Some(chunk) = pack.child(FACES) {
        for f in Reader::new(chunk.body).whole_records(32) {
            // A whole 32-byte record: every field below is inside it.
            let f: [u8; 32] = f.array_at(0).unwrap_or([0; 32]);
            let i16_at = |at: usize| i16::from_le_bytes([f[at], f[at + 1]]) as f32;
            let base = i16_at(4);
            let flags = f[3];
            let mut vertices: [Vec3; 4] =
                std::array::from_fn(|i| Vec3::new(i16_at(8 + i * 2) / 8.0, i16_at(16 + i * 2) / 8.0, base + i16_at(24 + i * 2) / 256.0));
            if flags & FLAG_TALL != 0 {
                // 0x5D7F20: corners 0 and 3 drop to −2048, 1 and 2 rise to 2048.
                for (v, z) in vertices.iter_mut().zip([-2048.0, 2048.0, 2048.0, -2048.0]) {
                    v.z = z;
                }
            }
            let corners = if flags & FLAG_QUAD != 0 { 4 } else { 3 };
            let raw = |at: usize| -> [i16; 4] { std::array::from_fn(|i| i16::from_le_bytes([f[at + i * 2], f[at + i * 2 + 1]])) };
            faces.push(CollisionFace { vertices, corners, surface: f[2], flags, group: f[0], bounds: f[1], x: raw(8), y: raw(16) });
        }
    }
    (section, faces)
}

/// The track's barrier groups (track bundle `0x3412F`, registered by
/// 0x5E78B0 at 0x88F768 +0x828 count / +0x82C entries): 32-byte entries
/// {u32 name hash, u32, u8 on, name[23]}, indexed by a face's group byte.
/// Entry 0 is `NONE`. The others are `BARRIERS_<race>` and
/// `PLAYER_BARRIERS_<race>`, switched on for their race (0x578070), and
/// `BARRIERS_CAREER<n>`, switched on in free roam for career stage n. The
/// `on` byte is 1 in every shipped entry; the game resets it once the
/// bundle is in (0x5D8360), see `nfsu2::collision::BarrierSwitches`.
pub const BARRIER_GROUPS: u32 = 0x0003_412F;

/// One barrier group: its name and the name's hash (+0), by which
/// 0x5D3C70 finds it (among entries 1.., so never `NONE`).
#[derive(Clone, Debug)]
pub struct BarrierGroup {
    pub hash: u32,
    pub name: String,
}

pub fn parse_barrier_groups(chunk: Chunk) -> Vec<BarrierGroup> {
    chunk
        .body
        .chunks_exact(32)
        .map(|e| {
            let e: &[u8; 32] = e.try_into().unwrap_or(&[0; 32]);
            let name = &e[9..];
            BarrierGroup {
                hash: u32::from_le_bytes([e[0], e[1], e[2], e[3]]),
                name: String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())]).into_owned(),
            }
        })
        .collect()
}

/// Collision volumes of a model (`0x80034020` in a car's GEOMETRY.BIN), in
/// pairs: `0x39200` a header — {u32 3, u32 3, u32, u32, u32 1, u32 vertex
/// count, u32 name hash, name} — and `0x39201` the vertices, 48 bytes each
/// with the position at +16. `<CAR>_CV` is the body's convex hull, the one
/// the car's collision body is built from (looked up by name at 0x5A18D0;
/// constructor 0x5AEC50); `<CAR>_W0n_CV` are the wheels'. Each vertex
/// record holds its link count (i16 +8), its first link (i16 +0xA) and the
/// index it reports (i16 +0xC); `0x39202` after the pair is the link list,
/// i16 vertex indices from the 16-byte aligned payload (0x5A1760: shape
/// +0x50 links, +0x54 records). The hull's support climbs over them
/// (0x5AFBB0, [`HullLinks`]).
pub const COLLISION_VOLUMES: u32 = 0x8003_4020;
const VOLUME_HEADER: u32 = 0x0003_9200;
const VOLUME_VERTICES: u32 = 0x0003_9201;
const VOLUME_LINKS: u32 = 0x0003_9202;

/// A hull's vertex links (0x5A1760, 0x5AFBB0): per vertex the index it
/// reports as a support (record +0xC: its own, or 0 for a vertex with no
/// links — duplicates and points inside), its neighbours `list[first ..
/// first + count]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HullLinks {
    pub index: Vec<u16>,
    pub first: Vec<u16>,
    pub count: Vec<u16>,
    pub list: Vec<u16>,
}

crate::persist!(HullLinks { index, first, count, list });

impl HullLinks {
    /// Vertex `v`'s neighbours (none when the list does not hold them).
    pub fn of(&self, v: usize) -> &[u16] {
        let (f, n) = (self.first[v] as usize, self.count[v] as usize);
        self.list.get(f..f + n).unwrap_or(&[])
    }
}

#[derive(Clone, Debug)]
pub struct CollisionVolume {
    pub name: String,
    /// The header's name hash (+24): what 0x5A18D0 matches a key against.
    pub hash: u32,
    pub vertices: Vec<Vec3>,
    pub links: HullLinks,
}

pub fn parse_collision_volumes(chunk: Chunk) -> Vec<CollisionVolume> {
    let mut volumes = Vec::new();
    let mut header: Option<(String, usize, u32)> = None;
    for c in chunk.children() {
        let b = Reader::new(c.payload());
        match c.id {
            VOLUME_HEADER if b.len() >= 28 => {
                header = (|| Some((b.cstr_at(28, 32).ok()?, b.u32_at(20).ok()? as usize, b.u32_at(24).ok()?)))();
            }
            VOLUME_VERTICES => {
                if let Some((name, count, hash)) = header.take() {
                    // At most the whole 48-byte records there are.
                    let records: Vec<_> = b.whole_records(48).take(count).collect();
                    let vertices = records.iter().filter_map(|v| v.vec3_at(16).ok().map(Vec3::from)).collect();
                    let field = |at: usize| -> Vec<u16> { records.iter().map(|v| v.i16_at(at).unwrap_or(0).max(0) as u16).collect() };
                    let links = HullLinks { count: field(8), first: field(0xA), index: field(0xC), list: Vec::new() };
                    volumes.push(CollisionVolume { name, hash, vertices, links });
                }
            }
            VOLUME_LINKS => {
                if let Some(v) = volumes.last_mut().filter(|v| v.links.list.is_empty()) {
                    v.links.list = c.aligned(16).chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]]).max(0) as u16).collect();
                }
            }
            _ => {}
        }
    }
    volumes
}

/// A pack's lookup grid: `0x34131` header (section u16 at 0, width and
/// height u16 at 4 and 6, origin x/y f32 at 8 and 12; 32 m cells), cells
/// `0x34137` {u16 first reference, u16 count}, references `0x34136`
/// {u16 face, u16 mask of the cell's 4×4 eight-metre sub-cells the face
/// touches}. Every cell lists its floors before its walls.
#[derive(Clone, Debug, Default)]
pub struct CollisionGrid {
    pub section: u16,
    pub width: u16,
    pub height: u16,
    pub origin: [f32; 2],
    pub cells: Vec<(u16, u16)>,
    pub refs: Vec<(u16, u16)>,
}

const GRID_REFS: u32 = 0x0003_4136;
const GRID_CELLS: u32 = 0x0003_4137;

pub fn parse_collision_grid(pack: Chunk) -> Option<CollisionGrid> {
    let h = Reader::new(pack.child(HEADER)?.aligned(16));
    if h.len() < 16 {
        return None;
    }
    let pairs = |id: u32| -> Vec<(u16, u16)> {
        pack.child(id).map_or_else(Vec::new, |c| Reader::new(c.body).whole_records(4).filter_map(|b| Some((b.u16_at(0).ok()?, b.u16_at(2).ok()?))).collect())
    };
    Some(CollisionGrid {
        section: h.u16_at(0).ok()?,
        width: h.u16_at(4).ok()?,
        height: h.u16_at(6).ok()?,
        origin: [h.f32_at(8).ok()?, h.f32_at(12).ok()?],
        cells: pairs(GRID_CELLS),
        refs: pairs(GRID_REFS),
    })
}

crate::persist!(BarrierGroup { hash, name });
crate::persist!(CollisionGrid { section, width, height, origin, cells, refs });
crate::persist!(CollisionFace { vertices, corners, surface, flags, group, bounds, x, y });
