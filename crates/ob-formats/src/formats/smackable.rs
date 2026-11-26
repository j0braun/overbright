//! Knock-over props ("smackables"): the track bundle's world-object infos
//! (`0x34026`) and each stream section's placements of them (`0x34027`),
//! both read by 0x5B0E40.
use glam::{Mat3, Mat4, Vec3};

use super::bundle::Chunk;
use super::reader::{FormatResult, Reader};

pub const OBJECT_INFOS: u32 = 0x0003_4026;
pub const OBJECT_INSTANCES: u32 = 0x0003_4027;
/// The track bundle's scenery-instance table (read by 0x5DF8B0 into
/// 0x883DF0; 0x5D6440 indexes it): see [`SceneryLink`].
pub const SCENERY_LINKS: u32 = 0x0003_4107;

/// The track bundle's scenery groups (registered by 0x5DB2F0 into the
/// list 0x88F760; 0x5DB350 finds one by hash): see [`SceneryGroup`].
pub const SCENERY_GROUPS: u32 = 0x0003_4108;

/// One scenery group: list links at +0 (written by the game), the name at
/// +0x08 (0x20 bytes), its hash at +0x28, +0x2C on (set by 0x5710B0 when a
/// race or free roam switches the like-named barrier group on, 0x571170),
/// +0x30 the count and from +0x34 that many u16 indices into the
/// `0x34107` table; entries are (count·2 + 0x37) & !3 bytes.
#[derive(Clone, Debug)]
pub struct SceneryGroup {
    pub name: String,
    pub hash: u32,
    pub on: bool,
    pub members: Vec<u16>,
}

pub fn parse_scenery_groups(chunk: Chunk<'_>) -> Vec<SceneryGroup> {
    let b = Reader::new(chunk.body);
    let mut out = Vec::new();
    let mut at = 0;
    // One group at `at`; `None` (the table ends) when it is cut off.
    let group = |at: usize| -> FormatResult<(SceneryGroup, usize)> {
        let count = b.u32_at(at + 0x30)? as usize;
        let size = (count * 2 + 0x37) & !3;
        let mut members = b.sub_at(at + 0x34, count * 2)?;
        Ok((
            SceneryGroup {
                name: b.cstr_at(at + 8, 0x20)?,
                hash: b.u32_at(at + 0x28)?,
                on: b.u32_at(at + 0x2C)? != 0,
                members: members.u16s("scenery group members", count, usize::MAX)?,
            },
            size,
        ))
    };
    while at + 0x34 <= b.len() {
        let Ok((g, size)) = group(at) else { break };
        out.push(g);
        at += size;
    }
    out
}

/// The material names, by index (0x7FC808; 0x5B0E40 stores the index of
/// the one whose hash matches info +0xDC at +0xD8, else 0).
pub const MATERIALS: [&str; 19] = [
    "NONE", "GARBAGE_BAG", "SHOPPING_CART", "METAL_FENCE", "TRAFFIC_CONE", "WATER_DRUM", "OIL_DRUM",
    "PARKING_METER", "ROAD_SIGN_METAL", "ROAD_SIGN_WOOD", "MARKET_STALL", "SANDWICH_BOARD", "SAW_HORSE",
    "METAL_SMALL", "METAL_BIG", "WOODBOX", "TRASHCAN", "GLASS", "CHAIR",
];

/// One kind of prop, 0x100 bytes (list links at +0, written by the game).
#[derive(Clone, Debug)]
pub struct ObjectInfo {
    /// +0x08, e.g. `ZCS_PylonA` (the `ZCS_` volumes in the bundle's
    /// `0x80034020` pack share the names).
    pub name: String,
    /// +0x80: what instances refer to (0x5AD900 looks the info up by it).
    pub key: u32,
    /// +0x20, tonnes: +0x20 is the rigid-body description 0x5B1550 hands
    /// to 0x5A7F40; 0x59B950 keeps its first float as the mass (body
    /// +0x164) and 1/m (body +0x168, what vtable +0x5C returns).
    pub mass: f32,
    /// The 3×3 of the 4×4 at +0x30 (description +0x10): 0x59B950 inverts
    /// it (0x43C7C0) into body +0x170, the inverse inertia vtable +0x60
    /// returns.
    pub inertia: Mat3,
    /// +0xB0, prop space: 0x5B1550 copies it to the body's centre-of-mass
    /// offset (+0x1F0), which 0x5ADB30 adds to the pose.
    pub centre_of_mass: Vec3,
    /// +0xC4: the impulse it takes to knock the prop over (0x5B1550 copies
    /// it to +0x150; until then the prop is fixed, 0x5B2080).
    pub break_impulse: f32,
    /// +0xCC, held to at most 0.005 by 0x5B1550; vtable +0x70 (0x5B1CC0).
    pub restitution: f32,
    /// +0xE0, held to at least 0.3 by 0x5B1550; vtable +0x74 (0x5B1CD0).
    pub friction: f32,
    /// Index into [`MATERIALS`] (from +0xDC).
    pub material: usize,
    /// +0x88: the description group of the effects started when the prop
    /// first moves (0x5B4230 gathers its emitters by 0x48B8B0; 0x5ADFA0
    /// starts them): `ZCS_Barrel_A`'s is `XES_Barrel_WaterA_SplashA`'s.
    /// 0 none.
    pub effects: u32,
}

/// One placed prop, 0x70 bytes after a 0x10-byte header {u32, u32, i16
/// section, i16 first (index into the "already made" bit set 0x86AD78),
/// i16 count}.
#[derive(Clone, Debug)]
pub struct ObjectInstance {
    pub info_key: u32,
    /// +4 (u16): the scenery instance it stands in for, an index into the
    /// [`SceneryLink`] table (0x5D6440).
    pub scenery: u16,
    /// +0x10 / +0x20: world bounds (0x5AD900 gives the unhit body this box
    /// and half its diagonal as the radius).
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    /// +0x30: local → world, row vectors (0x5B0E40 re-orthonormalises the
    /// rotation on first load, 0x5BB0E0).
    pub transform: Mat4,
}

/// One entry of the `0x34107` table, 8 bytes: i16 stream section, i16
/// the instance's index in that section's scenery, u8 flags (bit 0x10 set
/// by 0x5B0C40 while a prop stands in for the instance, cleared by 0x5AD900;
/// 0x5DFA40 redraws the instance), u8, u8 kind (+6: 1 makes the placed prop
/// an unhit body, 0x5B0C40), u8. Sorted by the section's low byte (0x5D63F0
/// indexes the first entry of each in 0x883AC0).
#[derive(Clone, Copy, Debug)]
pub struct SceneryLink {
    pub section: i16,
    pub instance: i16,
    pub flags: u8,
    pub kind: u8,
    /// The u16 at +4 (the flags above and the byte after them): what
    /// 0x5DB0F0 writes into the instance's flags (+0x1A) each time the
    /// entry changes, e.g. when a scenery group lists it (0x5710B0 clears
    /// 0x10, 0x5710F0 sets it). Every L4RA entry equals its instance's
    /// flags on disk.
    pub instance_flags: u16,
}

pub fn parse_scenery_links(chunk: Chunk<'_>) -> Vec<SceneryLink> {
    // Read straight from the body (0x5DF8B0 takes the chunk +8, unaligned).
    let link = |r: Reader<'_>| -> FormatResult<SceneryLink> {
        Ok(SceneryLink { section: r.i16_at(0)?, instance: r.i16_at(2)?, flags: r.u8_at(4)?, kind: r.u8_at(6)?, instance_flags: r.u16_at(4)? })
    };
    // Whole 8-byte records: no read fails.
    Reader::new(chunk.body).whole_records(8).filter_map(|r| link(r).ok()).collect()
}

fn hash(name: &str) -> u32 {
    super::hash::bin_hash(name)
}

pub fn parse_infos(chunk: Chunk<'_>) -> Vec<ObjectInfo> {
    let info = |r: Reader<'_>| -> FormatResult<ObjectInfo> {
        let material_hash = r.u32_at(0xDC)?;
        let mut inertia = [0f32; 9];
        for (i, v) in inertia.iter_mut().enumerate() {
            *v = r.f32_at(0x30 + 0x10 * (i / 3) + 4 * (i % 3))?;
        }
        Ok(ObjectInfo {
            name: r.cstr_at(0x08, 0x18)?,
            key: r.u32_at(0x80)?,
            mass: r.f32_at(0x20)?,
            inertia: Mat3::from_cols_array(&inertia),
            centre_of_mass: Vec3::from(r.vec3_at(0xB0)?),
            break_impulse: r.f32_at(0xC4)?,
            restitution: r.f32_at(0xCC)?.min(0.005),
            friction: r.f32_at(0xE0)?.max(0.3),
            material: MATERIALS.iter().position(|m| hash(m) == material_hash).unwrap_or(0),
            effects: r.u32_at(0x88)?,
        })
    };
    // Whole 0x100-byte records: no read fails.
    Reader::new(chunk.aligned(16)).whole_records(0x100).filter_map(|r| info(r).ok()).collect()
}

pub fn parse_instances(chunk: Chunk<'_>) -> Vec<ObjectInstance> {
    let mut b = Reader::new(chunk.aligned(16));
    let Ok(count) = b.u16_at(0x0C) else { return Vec::new() };
    if b.skip(0x10).is_err() {
        return Vec::new();
    }
    let instance = |r: Reader<'_>| -> FormatResult<ObjectInstance> {
        let mut m = [0f32; 16];
        for (i, v) in m.iter_mut().enumerate() {
            *v = r.f32_at(0x30 + 4 * i)?;
        }
        Ok(ObjectInstance {
            info_key: r.u32_at(0)?,
            scenery: r.u16_at(4)?,
            bbox_min: Vec3::from(r.vec3_at(0x10)?),
            bbox_max: Vec3::from(r.vec3_at(0x20)?),
            // Rows are the local axes and the position, so glam's
            // column-major load is local → world for column vectors.
            transform: Mat4::from_cols_array(&m),
        })
    };
    // Whole 0x70-byte records: no read fails.
    b.whole_records(0x70).take(count as usize).filter_map(|r| instance(r).ok()).collect()
}

crate::persist!(SceneryGroup { name, hash, on, members });
crate::persist!(SceneryLink { section, instance, flags, kind, instance_flags });
crate::persist!(ObjectInfo { name, key, mass, inertia, centre_of_mass, break_impulse, restitution, friction, material, effects });
