//! World animations ("WorldAnimations", manager 0x827EF0): animated
//! scenery in a track bundle such as the shops' roll-up doors, the jumbo
//! planes, cranes and graffiti trains. Chunks, all with a 16-byte aligned
//! record after the header (`(chunk + 0x17) & ~0xF`):
//!
//! * 0x00037080 (handled at 0x435A94 / 0x438C24), 0x78 bytes: an entity of
//!   an animation tree — +0x04 the entity's hash, +0x08 its parent's (0 for
//!   the root), +0x0C the scenery-info LOD hash of its model (the solid's
//!   hash: the door slats' `XO_DoorPieceA…D_1a_RB_00` infos in the stream
//!   sections carry 0x10246183, 0x142EF2A4, 0x183983C5, 0x1C4414E6), +0x20
//!   the tree's hash, +0x24 the bin hash of its animation's name (the
//!   bank channels less `_q` / `_t` / `_s`), +0x30 its rest matrix (row
//!   vectors, tree space).
//! * 0x00037090 (0x43A460 → 0x43A040, instances made by 0x43A4A0), 0xA0
//!   bytes: an instance — +0x04 the tree's hash (0x439EE0 finds the tree),
//!   +0x0C flags, +0x20 / +0x24 the hashes a world message addresses it by
//!   (a shop door: the shop's world trigger, e.g. CC_CAR_LOT_1's
//!   0xFFCC6A4B — 0x438CA0 matches either against 0x827ED4), +0x40 its
//!   matrix (row vectors, world space). An instance with +0x20 set starts in
//!   state 0 (held, under control), one without but with +0x3C set in
//!   state 3; the rest loop (0x43A4A0: entity controller +0x10 = 1).
//! * 0x00E34010: the animation banks (`formats::eagl`).
//!
//! L4RA has 45 instances, 23 of them shop doors (tree "DoorAnim",
//! 0xB8A18038: four slats animated by `ANM_DoorAnim_RB_00_XO_DoorPiece
//! {A,B,C,D}_1a_RB_00`).

use std::collections::HashMap;

use glam::Mat4;

use super::bundle::{Chunk, chunks};
use super::reader::{FormatResult, Reader};
use super::eagl::{self, Channel};

pub const ENTITY: u32 = 0x0003_7080;
pub const INSTANCE: u32 = 0x0003_7090;
pub const BANK: u32 = 0x00E3_4010;

/// The door tree's hash, bin_hash("DoorAnim").
pub const DOOR_TREE: u32 = 0xB8A1_8038;

#[derive(Clone, Debug)]
pub struct Entity {
    pub hash: u32,
    pub parent: u32,
    /// The model's solid hash (0 for none).
    pub solid: u32,
    pub tree: u32,
    /// bin_hash of the animation's name (0 for none).
    pub animation: u32,
    /// Rest matrix, column-major, tree space.
    pub rest: Mat4,
}

#[derive(Clone, Debug)]
pub struct Instance {
    /// +0x00: the instance's number, the key the sound objects find it by
    /// (0x432170 compares record +0x00; 0x439F40).
    pub id: u32,
    pub tree: u32,
    pub flags: u32,
    /// +0x20 / +0x24: the hashes world messages address it by.
    pub keys: [u32; 2],
    /// Local → world, column-major.
    pub transform: Mat4,
    /// +0x10: the play speed (0 → 1, 0x4389AE).
    pub speed: f32,
    /// +0x14 / +0x18: the player's +0x68 / +0x6C (0x4389C2, 0x4389DA).
    pub delay: [f32; 2],
    /// +0x1C / +0x1E: the frame range used with flag 0x40 (0x438B23).
    pub range: [u16; 2],
    /// +0x38..+0x3B: the track-number window (0x43A4E7).
    pub window: [u8; 4],
    /// +0x3C (u16): set → starts in state 3 when +0x20 is 0 (0x43A763).
    pub hold: u16,
}

/// One animation's channels: rotation (`_q`), translation (`_t`) and scale
/// (`_s`), whichever the bank has.
#[derive(Clone, Debug, Default)]
pub struct Animation {
    pub rotation: Option<Channel>,
    pub translation: Option<Channel>,
    pub scale: Option<Channel>,
}

impl Animation {
    /// The frame count (the longest channel's).
    pub fn frames(&self) -> usize {
        [&self.rotation, &self.translation, &self.scale].iter().filter_map(|c| c.as_ref().map(|c| c.frames())).max().unwrap_or(0)
    }

    /// The player's length +0x60 (0x438860 → 0x4350C0): each bound channel
    /// (`_t`, then `_q`, then `_s`) writes its compound header's frame
    /// count (vtable +0x10 = 0x6E2110: header +0x0A, as a float), so the
    /// last one bound wins.
    pub fn length(&self) -> f32 {
        [&self.scale, &self.rotation, &self.translation].iter().find_map(|c| c.as_ref().map(|c| c.frame_count as f32)).unwrap_or(0.0)
    }

    /// The pose a world-animation controller gives its node at `frame`
    /// (0x4316B0): the pose buffer starts as the "ROOT" skeleton's two
    /// bones (0x434F20 → 0x6E3C70; read from the running original: both
    /// scale 1, rotation identity, translation 0, bone 1's parent bone 0),
    /// each channel writes its floats at its slots (12 a bone: scale 0–2,
    /// rotation 4–7, translation 8–10), 0x6E3B10 makes each bone S · R(q)
    /// · T (0x6E2D60, the quaternion as is) times its parent's, and the
    /// node takes bone 1's. A channel that is missing keeps the skeleton's
    /// value, not the entity's rest matrix.
    pub fn bone_pose(&self, frame: f32) -> Mat4 {
        let mut p = [0.0f32; 24];
        for b in 0..2 {
            p[b * 12..b * 12 + 12].copy_from_slice(&[1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        }
        for c in [&self.rotation, &self.scale, &self.translation].into_iter().flatten() {
            c.sample_each(frame, |k, v| {
                if let Some(f) = c.slots.get(k).and_then(|&slot| p.get_mut(slot as usize)) {
                    *f = v;
                }
            });
        }
        let bone = |b: usize| {
            let f = &p[b * 12..];
            Mat4::from_scale_rotation_translation(glam::Vec3::new(f[0], f[1], f[2]), glam::Quat::from_xyzw(f[4], f[5], f[6], f[7]), glam::Vec3::new(f[8], f[9], f[10]))
        };
        bone(0) * bone(1)
    }

    /// The pose at `frame` (clamped), as a column-major matrix
    /// S · R · T in the game's row-vector order, `rest` standing in for a
    /// missing rotation or translation.
    pub fn pose(&self, frame: f32, rest: Mat4) -> Mat4 {
        let (rest_scale, rest_rot, rest_pos) = rest.to_scale_rotation_translation();
        let rot = self.rotation.as_ref().map_or(rest_rot, |c| c.quat(frame));
        let pos = self.translation.as_ref().map_or(rest_pos, |c| c.vec3(frame));
        let scale = self.scale.as_ref().map_or(rest_scale, |c| c.vec3(frame));
        Mat4::from_scale_rotation_translation(scale, rot, pos)
    }
}

fn row_matrix(b: &Reader<'_>, at: usize) -> FormatResult<Mat4> {
    // Row vectors on disk: the same 16 floats read as columns.
    let mut m = [0f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = b.f32_at(at + 4 * i)?;
    }
    Ok(Mat4::from_cols_array(&m))
}

/// The entities, instances and animations of a track bundle.
#[derive(Clone, Debug, Default)]
pub struct WorldAnimations {
    pub entities: Vec<Entity>,
    pub instances: Vec<Instance>,
    /// By bin_hash of the name less its `_q` / `_t` / `_s`.
    pub animations: HashMap<u32, Animation>,
}

impl WorldAnimations {
    pub fn parse(track_bun: &[u8]) -> WorldAnimations {
        let mut out = WorldAnimations::default();
        for c in chunks(track_bun, 0) {
            out.add(c);
        }
        out
    }

    fn add(&mut self, c: Chunk<'_>) {
        let b = Reader::new(c.aligned(16));
        let entity = || -> FormatResult<Entity> {
            Ok(Entity {
                hash: b.u32_at(0x04)?,
                parent: b.u32_at(0x08)?,
                solid: b.u32_at(0x0C)?,
                tree: b.u32_at(0x20)?,
                animation: b.u32_at(0x24)?,
                rest: row_matrix(&b, 0x30)?,
            })
        };
        let instance = || -> FormatResult<Instance> {
            Ok(Instance {
                id: b.u32_at(0x00)?,
                tree: b.u32_at(0x04)?,
                // 0x43A72C: a non-zero +0x30 adds the range flag 0x40 to +0x0C.
                flags: b.u32_at(0x0C)? | if b.u32_at(0x30)? != 0 { 0x40 } else { 0 },
                keys: [b.u32_at(0x20)?, b.u32_at(0x24)?],
                transform: row_matrix(&b, 0x40)?,
                speed: b.f32_at(0x10)?,
                delay: [b.f32_at(0x14)?, b.f32_at(0x18)?],
                range: [b.u16_at(0x1C)?, b.u16_at(0x1E)?],
                window: b.array_at(0x38)?,
                hold: b.u16_at(0x3C)?,
            })
        };
        // The length guards make every read below succeed.
        match c.id {
            ENTITY if b.len() >= 0x70 => self.entities.extend(entity().ok()),
            INSTANCE if b.len() >= 0x80 => self.instances.extend(instance().ok()),
            BANK => {
                let Ok(channels) = eagl::parse_bank(b.data()) else { return };
                for (name, channel) in channels {
                    let (base, kind) = match name.rsplit_once('_') {
                        Some((base, k @ ("q" | "t" | "s"))) => (base, k),
                        _ => continue,
                    };
                    let a = self.animations.entry(super::hash::bin_hash(base)).or_default();
                    match kind {
                        "q" => a.rotation = Some(channel),
                        "t" => a.translation = Some(channel),
                        _ => a.scale = Some(channel),
                    }
                }
            }
            _ => {}
        }
    }

    /// The entities of a tree.
    pub fn tree(&self, tree: u32) -> impl Iterator<Item = &Entity> {
        self.entities.iter().filter(move |e| e.tree == tree)
    }
}

crate::persist!(WorldAnimations { entities, instances, animations });
crate::persist!(Entity { hash, parent, solid, tree, animation, rest });
crate::persist!(Instance { id, tree, flags, keys, transform, speed, delay, range, window, hold });
crate::persist!(Animation { rotation, translation, scale });
