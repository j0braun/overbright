//! The NIS scenes' characters: the starter ("flag girl") and the crowd of
//! the race start scenes (`Scene_IntroNis<nn>`), skinned and animated.
//!
//! **Verified** unless marked (SPEED2.EXE):
//!
//! * The scene bundle holds, per character kind, a texture pack
//!   (0xB3300000, `FLAG_GIRL_7`, the crowd's, and the blob `CHARACTERSHADOW`
//!   that 0x4398A0 looks up), a skeleton (chunk 0xE34009, an EAGL ELF
//!   object; 0x434E10 loads it into "Anim_CAnimSkeleton_SlotPool", 0x4365B0
//!   unloads it), a model (0x80134000 solid list, mesh flags 0x4081: bit 0
//!   the skinned vertex format), the character animation bank (0xE34010,
//!   the bank without `ZPM_` channels) and, in the scene header chunk
//!   0x80037020, one 0x37040 record a character (0x4377F0 hands each one,
//!   16-byte aligned, to 0x435E10, which lists it for the scene).
//! * A 0x37040 record ([`Instance`]; read by the character's set-up
//!   0x433880): +0x00 a word (1), +0x04 its name hash (render info +0xD0),
//!   +0x10 a 4×4 row-vector matrix (copied to render info +0x50, its place
//!   in the scene), +0x50 three solid hashes (the model, given to 0x61D060),
//!   +0x60..+0x68 channel hashes for unskinned objects (0x434A80 slots
//!   0–2), +0x6C its animation's name hash (BinHash of the bank entry's
//!   name, e.g. "two_flag_girl_idle_starts_01_07_s"; bound only with a
//!   skeleton), +0x70 its skeleton's name hash (BinHash("Bip11"), found by
//!   0x434DC0), +0x74 the rate, +0x78 a delay, +0x80 / +0x84 a loop range
//!   (see [`Instance::frame`]).
//! * The skeleton ([`Skeleton`]): `__Skeleton:::<name>` → +0x00 hash, +0x08
//!   bone count, +0x10 bones of 0x70 bytes: scale xyz, parent (−1 for the
//!   root), rotation quaternion xyzw, translation xyz, the bone's index, and
//!   a 4×4 row-vector matrix (bone +0x30; 0x431400 walks the skeleton
//!   object's +0x24 from +0x40, 0x70 a bone): the inverse of the bind pose.
//! * The pose: the animation's channels write 12 floats a bone (scale
//!   0..2, quaternion 4..7, translation 8..10; the 0x0B quaternion
//!   channels' slots run 4, 16, 28, …), a 0x0F compound of a 0x0B
//!   (rotations), a 0x14 (vec3s: translations and scales, see
//!   [`eagl::decode_keyblock_vec3`]) and a 0x15 (scalars). 0x431400 evaluates
//!   it at the controller's frame (+0x68, advanced 30 frames a second by
//!   0x434B00), 0x6E3B10 makes each bone's model-space matrix (+0x2C, 0x40
//!   bytes a bone) and, for a skinned bank (+0x34 bit 0), multiplies each
//!   by its bone's inverse bind matrix (0x43C990). The character update 0x436050 (vtable
//!   0x785FB8) runs that every frame and points the render info's +0xE8 at
//!   the matrices, the skinned draw's palette.
//! * The skinned vertex (60 bytes): position, normal, colour (D3DCOLOR), uv,
//!   three weights (summing to 1) and three bone indices as floats into the
//!   draw range's palette; the 60-byte material keeps the solid layout
//!   (+0x0C index count, +0x1C texture slot, +0x34 first index) with the
//!   palette's bone numbers in bytes +0x21..+0x34.
//! * The shadow (0x433220): a CHARACTERSHADOW quad between two bones of the
//!   character kind's table (0x7EA1E0, 0x10 bytes a kind), 0.25 along and
//!   0.35 across, each corner on the ground + 0.05; **not ported**.
//! * The ground step (0x436050 every update, 0x437BE0 once at the set-up;
//!   **verified**, and **observed** under Wine at STAGE_1_CIRCUIT_1's
//!   IntroNis13: each character's world z = its ground + 0.05, the ground
//!   differing per character and following its walk): a skinned character
//!   with +0x18 set (the constructor 0x431220 sets it; nothing in the NIS
//!   code clears it) asks 0x5E72F0 for the ground under its bone 1
//!   (0x431280(1): the bone's model matrix through the world matrix), keeps
//!   the first one found (+0x20, +0x19 set) and blends later ones half and
//!   half, and sets its world matrix's z to that ground + 0.05. So a scene
//!   made on flat ground stands on any start's slope or kerb; the scene's
//!   one origin height alone left people up to 1.8 m under the road or 2 m
//!   over it. See [`SceneCharacters::ground_probe`], [`filter_ground`].
//!
// prov: inferred character.module
//! **Inferred**: the 0x43C990 product order (the standard skin matrix, the
//! bone's model matrix after its inverse bind, matches the bind pose to the
//! mesh); the controller's frame from the scene's start (the scene frame);
//! the shading (the clone's own: the texture under a fixed light, the
//! character effect is not traced).

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3, Vec3A};

use crate::formats::bundle::{Chunk, chunks, read_f32, read_u16, read_u32};
use crate::formats::eagl::{self, Channel};
use crate::formats::hash::bin_hash;
use crate::formats::texture::{Texture, parse_texture_pack};

/// One bone of a skeleton (0x70 bytes).
#[derive(Clone, Debug)]
pub struct Bone {
    pub parent: i32,
    pub scale: Vec3,
    pub rotation: Quat,
    pub translation: Vec3,
    /// The inverse bind matrix (column-major, from the file's row-vector
    /// matrix).
    pub inverse_bind: Mat4,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    /// The skeleton record's +0x00 (its low half is the animations'
    /// compound +0x02).
    pub hash: u32,
    /// The name of `__Skeleton:::<name>` hashed (BinHash("Bip11")), what
    /// an instance names.
    pub name_hash: u32,
    pub bones: Vec<Bone>,
}

/// An ELF object's `.data` and its symbols (name, value).
fn elf_data(elf: &[u8]) -> Result<(&[u8], Vec<(String, usize)>)> {
    let start = elf.windows(4).position(|w| w == b"\x7fELF").context("not an ELF object")?;
    let elf = &elf[start..];
    let u32_ = |o: usize| -> Result<u32> { Ok(read_u32(elf.get(o..o + 4).context("ELF bounds")?, 0)) };
    let shoff = u32_(0x20)? as usize;
    // Fuzzing: a cut-off header is an error, not a panic.
    anyhow::ensure!(elf.len() >= 0x34, "ELF header");
    let shnum = read_u16(elf, 0x30) as usize;
    let sec = |i: usize, f: usize| u32_(shoff + 40 * i + 4 * f).map(|v| v as usize);
    let (data_off, data_len) = (sec(1, 4)?, sec(1, 5)?);
    let data = elf.get(data_off..data_off + data_len).context(".data bounds")?;
    let sym = (0..shnum).find(|&i| sec(i, 1).is_ok_and(|t| t == 2)).context("no .symtab")?;
    let strs = sec(sec(sym, 6)?, 4)?;
    let mut out = Vec::new();
    for i in 0..sec(sym, 5)? / 16 {
        let so = sec(sym, 4)? + 16 * i;
        let name_at = strs + u32_(so)? as usize;
        let name = elf.get(name_at..).map(|s| String::from_utf8_lossy(s.split(|&c| c == 0).next().unwrap_or_default()).into_owned()).unwrap_or_default();
        out.push((name, u32_(so + 4)? as usize));
    }
    Ok((data, out))
}

fn row_matrix(b: &[u8], at: usize) -> Mat4 {
    let f: Vec<f32> = (0..16).map(|k| read_f32(b, at + 4 * k)).collect();
    // Row-vector rows are the column-major columns.
    Mat4::from_cols_array(&f.try_into().unwrap())
}

impl Skeleton {
    pub fn parse(elf: &[u8]) -> Result<Skeleton> {
        let (data, syms) = elf_data(elf)?;
        let (name, at) = syms.iter().find(|(n, _)| n.starts_with("__Skeleton:::")).context("no skeleton symbol")?;
        let name_hash = bin_hash(&name["__Skeleton:::".len()..]);
        let count = read_u32(data.get(*at..at.saturating_add(0x10)).context("skeleton bounds")?, 8) as usize;
        anyhow::ensure!(count.checked_mul(0x70).and_then(|n| n.checked_add(at + 0x10)).is_some_and(|end| data.len() >= end), "skeleton bounds");
        let bones = (0..count)
            .map(|i| {
                let b = at + 0x10 + i * 0x70;
                let v3 = |o: usize| Vec3::new(read_f32(data, b + o), read_f32(data, b + o + 4), read_f32(data, b + o + 8));
                Bone {
                    scale: v3(0),
                    parent: read_u32(data, b + 0xC) as i32,
                    rotation: Quat::from_xyzw(read_f32(data, b + 0x10), read_f32(data, b + 0x14), read_f32(data, b + 0x18), read_f32(data, b + 0x1C)),
                    translation: v3(0x20),
                    inverse_bind: row_matrix(data, b + 0x30),
                }
            })
            .collect();
        Ok(Skeleton { hash: read_u32(data, *at), name_hash, bones })
    }

    /// The bind pose as the 12-float bone blocks the channels write.
    pub fn bind_pose(&self) -> Vec<f32> {
        let mut p = vec![0.0; self.bones.len() * 12];
        for (i, b) in self.bones.iter().enumerate() {
            p[i * 12..i * 12 + 3].copy_from_slice(&b.scale.to_array());
            p[i * 12 + 4..i * 12 + 8].copy_from_slice(&b.rotation.to_array());
            p[i * 12 + 8..i * 12 + 11].copy_from_slice(&b.translation.to_array());
        }
        p
    }

    /// Model-space bone matrices from a pose (0x6E3B10: scale, then
    /// rotation, then translation, under the parent's).
    pub fn model_matrices(&self, pose: &[f32]) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let at = |k: usize| pose.get(i * 12 + k).copied().unwrap_or(0.0);
            let scale = Vec3::new(at(0), at(1), at(2));
            let q = Quat::from_xyzw(at(4), at(5), at(6), at(7));
            let q = if q.length_squared() > 1e-12 { q.normalize() } else { Quat::IDENTITY };
            let local = Mat4::from_scale_rotation_translation(scale, q, Vec3::new(at(8), at(9), at(10)));
            let parent = usize::try_from(b.parent).ok().filter(|&p| p < i).map(|p| out[p]);
            out.push(parent.map_or(local, |p| p * local));
        }
        out
    }

    /// The skin palette (0x431400, skinned bank): each bone's model matrix
    /// after its inverse bind matrix.
    pub fn skin_matrices(&self, pose: &[f32]) -> Vec<Mat4> {
        self.model_matrices(pose).iter().zip(&self.bones).map(|(m, b)| *m * b.inverse_bind).collect()
    }
}

/// A skinned draw range.
#[derive(Clone, Debug)]
pub struct SkinRange {
    pub texture_slot: u8,
    pub first_index: u32,
    pub index_count: u32,
    /// The palette's bone numbers (bytes +0x21..+0x34).
    pub palette: [u8; 19],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SkinVertex {
    pub position: Vec3,
    pub normal: Vec3,
    /// D3DCOLOR bytes (B, G, R, A).
    pub color: [u8; 4],
    pub uv: [f32; 2],
    pub weights: [f32; 3],
    pub bones: [u8; 3],
}

#[derive(Clone, Debug)]
pub struct SkinnedModel {
    pub hash: u32,
    pub name: String,
    pub textures: Vec<u32>,
    pub vertices: Vec<SkinVertex>,
    pub indices: Vec<u16>,
    pub ranges: Vec<SkinRange>,
}

/// Conservative model-space skin envelope, built once from immutable mesh data.
/// Bone indices are resolved through each material range's palette, exactly as
/// `skin_ranges` does. Positive weights need not add to one.
#[derive(Clone, Debug)]
pub struct SkinBounds {
    bind: PosedBounds,
    weight_min: f32,
    weight_max: f32,
    bones: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct PosedBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl PosedBounds {
    fn finite(self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min.cmple(self.max).all()
    }

    fn transformed(self, matrix: Mat4) -> Option<Self> {
        if !self.finite() || !matrix.is_finite() { return None; }
        let centre = (self.min + self.max) * 0.5;
        let extent = (self.max - self.min) * 0.5;
        let x = matrix.x_axis.truncate().abs();
        let y = matrix.y_axis.truncate().abs();
        let z = matrix.z_axis.truncate().abs();
        let c = matrix.transform_point3(centre);
        let e = x * extent.x + y * extent.y + z * extent.z;
        // Bound roundoff from the original float32 multiply/add sequence,
        // including cancellation at large world coordinates. Widen, never
        // normalize or change the positions sent to the renderer.
        let magnitude = self.min.abs().max(self.max.abs());
        let pad = (x * magnitude.x + y * magnitude.y + z * magnitude.z
            + matrix.w_axis.truncate().abs()) * 0.00001 + Vec3::splat(0.001);
        let b = Self { min: c - e - pad, max: c + e + pad };
        b.finite().then_some(b)
    }

    #[cfg(test)]
    fn contains(self, p: Vec3) -> bool { p.cmpge(self.min).all() && p.cmple(self.max).all() }

    /// Only reject a box wholly outside a side plane. Depth conventions and
    /// near/far clipping are left to the existing shader/rasterizer.
    pub fn visible(self, view_proj: Mat4) -> bool {
        if !self.finite() || !view_proj.is_finite() { return true; }
        let (x,y,w) = (view_proj.row(0),view_proj.row(1),view_proj.row(3));
        for plane in [w+x,w-x,w+y,w-y] {
            if !plane.is_finite() { return true; }
            let p = plane.to_array();
            let corner = Vec3::new(if p[0]>=0.0 {self.max.x} else {self.min.x},
                if p[1]>=0.0 {self.max.y} else {self.min.y}, if p[2]>=0.0 {self.max.z} else {self.min.z});
            let terms = [p[0] as f64 * corner.x as f64, p[1] as f64 * corner.y as f64,
                p[2] as f64 * corner.z as f64, p[3] as f64];
            let margin = terms.iter().map(|v| v.abs()).sum::<f64>() * 0.000001 + 0.0001;
            if terms.iter().sum::<f64>() < -margin { return false; }
        }
        true
    }
}

impl SkinBounds {
    pub fn build(model: &SkinnedModel) -> Option<Self> {
        let mut bind = PosedBounds { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) };
        let (mut weight_min, mut weight_max) = (f32::INFINITY,0.0f32);
        let mut bones = Vec::new();
        for range in &model.ranges {
            let first = range.first_index as usize;
            let end = first.checked_add(range.index_count as usize)?;
            for &index in model.indices.get(first..end)? {
                let v = model.vertices.get(index as usize)?;
                if !v.position.is_finite() || !v.weights.iter().all(|w| w.is_finite()) { return None; }
                bind.min = bind.min.min(v.position); bind.max = bind.max.max(v.position);
                let mut sum = 0.0;
                for k in 0..3 {
                    if v.weights[k] <= 0.0 { continue; }
                    sum += v.weights[k];
                    bones.push(range.palette.get(v.bones[k] as usize).copied().unwrap_or(0) as usize);
                }
                weight_min = weight_min.min(sum); weight_max = weight_max.max(sum);
            }
        }
        if !bind.finite() || !weight_min.is_finite() || !weight_max.is_finite() { return None; }
        bones.sort_unstable(); bones.dedup();
        Some(Self { bind, weight_min, weight_max, bones })
    }

    pub fn posed(&self, palette: &[Mat4], world: Mat4) -> Option<PosedBounds> {
        let mut union = PosedBounds { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) };
        for &bone in &self.bones {
            let b = self.bind.transformed(*palette.get(bone).unwrap_or(&Mat4::IDENTITY))?;
            union.min = union.min.min(b.min); union.max = union.max.max(b.max);
        }
        if self.bones.is_empty() { union = PosedBounds { min: Vec3::ZERO, max: Vec3::ZERO }; }
        // The weighted sum lies in the bone-envelope scaled by its weight
        // sum. Keep all interval products, including zero and nonunit sums.
        let lo0 = union.min * self.weight_min; let lo1 = union.min * self.weight_max;
        let hi0 = union.max * self.weight_min; let hi1 = union.max * self.weight_max;
        let b = PosedBounds { min: lo0.min(lo1).min(hi0).min(hi1), max: lo0.max(lo1).max(hi0).max(hi1) };
        b.transformed(world)
    }
}

impl SkinnedModel {
    /// The skinned solids of a solid list (mesh flags bit 0).
    pub fn parse_list(list: Chunk<'_>) -> Vec<SkinnedModel> {
        list.children().filter(|c| c.id == 0x8013_4010).filter_map(|s| SkinnedModel::parse(s).ok()).collect()
    }

    fn parse(solid: Chunk<'_>) -> Result<SkinnedModel> {
        let h = solid.child(0x0013_4011).context("solid header")?.aligned(16);
        anyhow::ensure!(h.len() > 0xA4, "solid header");
        let name: String = h[0xA4..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
        let textures = solid.child(0x0013_4012).map(|t| t.body.chunks_exact(8).map(|e| read_u32(e, 0)).collect()).unwrap_or_default();
        let mesh = solid.child(0x8013_4100).context("mesh")?;
        let mh = mesh.child(0x0013_4900).context("mesh header")?.aligned(16);
        anyhow::ensure!(mh.len() >= 0x38 && read_u32(mh, 0x0C) & 1 != 0, "not skinned");
        let count = read_u32(mh, 0x34) as usize;
        let vb = mesh.child(0x0013_4B01).context("vertices")?.payload();
        anyhow::ensure!(vb.len() >= count * 60, "vertex bounds");
        let vertices = vb[..count * 60]
            .chunks_exact(60)
            .map(|v| {
                let f = |o: usize| read_f32(v, o);
                SkinVertex {
                    position: Vec3::new(f(0), f(4), f(8)),
                    normal: Vec3::new(f(12), f(16), f(20)),
                    color: [v[24], v[25], v[26], v[27]],
                    uv: [f(28), f(32)],
                    weights: [f(36), f(40), f(44)],
                    bones: [f(48) as u8, f(52) as u8, f(56) as u8],
                }
            })
            .collect();
        let materials = read_u32(mh, 0x10) as usize;
        let mb = mesh.child(0x0013_4B02).context("materials")?.payload();
        let ranges: Vec<SkinRange> = mb
            .chunks_exact(0x3C)
            .take(materials)
            .map(|m| SkinRange { texture_slot: m[0x1C], index_count: read_u32(m, 0x0C), first_index: read_u32(m, 0x34), palette: m[0x21..0x34].try_into().unwrap() })
            .collect();
        let ib = mesh.child(0x0013_4B03).context("indices")?.payload();
        let n = ranges.iter().map(|r| (r.first_index + r.index_count) as usize).max().unwrap_or(0);
        anyhow::ensure!(ib.len() >= n * 2, "index bounds");
        let indices: Vec<u16> = (0..n).map(|i| read_u16(ib, i * 2)).collect();
        anyhow::ensure!(indices.iter().all(|&i| (i as usize) < count), "index range");
        Ok(SkinnedModel { hash: read_u32(h, 0x10), name, textures, vertices, indices, ranges })
    }

    /// The model posed by a skin palette, under `world`: each range's
    /// vertices blended over its palette's bones (position and normal), as
    /// triangle-list (position, normal, uv, texture slot) corners.
    pub fn skin(&self, palette: &[Mat4], world: Mat4) -> Vec<(Vec3, Vec3, [f32; 2], u8)> {
        let mut out = Vec::with_capacity(self.indices.len());
        let (mut done, mut posed) = (Vec::new(), Vec::new());
        self.skin_ranges(palette, world, &mut done, &mut posed, |p, n| (p, n), |_| true, |(p, n), uv, slot| out.push((p, n, uv, slot)));
        out
    }

    /// [`SkinnedModel::skin`] without the corner list: `range(slot)` is
    /// asked at each range's start (false leaves its corners out, unposed),
    /// each vertex is posed once per range and `pose(position, normal)`
    /// made of it, and `emit` gets each corner's `pose` value, uv and
    /// texture slot in order. `done` / `posed` are scratch kept between
    /// calls. The same arithmetic on the same inputs as `skin`.
    #[allow(clippy::too_many_arguments)]
    pub fn skin_ranges<T: Copy>(&self, palette: &[Mat4], world: Mat4, done: &mut Vec<u32>, posed: &mut Vec<T>, pose: impl Fn(Vec3, Vec3) -> T, mut range: impl FnMut(u8) -> bool, mut emit: impl FnMut(T, [f32; 2], u8)) {
        // A vertex is shared by several corners of a range: posed once per
        // range and reused (the same arithmetic on the same inputs, so the
        // same values; the corners came out ~4× per vertex).
        done.clear();
        done.resize(self.vertices.len(), u32::MAX);
        posed.clear();
        posed.resize(self.vertices.len(), pose(Vec3::ZERO, Vec3::ZERO));
        for (ri, r) in self.ranges.iter().enumerate() {
            if !range(r.texture_slot) {
                continue;
            }
            let first = r.first_index as usize;
            for &i in &self.indices[first..first + r.index_count as usize] {
                let v = &self.vertices[i as usize];
                if done[i as usize] == ri as u32 {
                    emit(posed[i as usize], v.uv, r.texture_slot);
                    continue;
                }
                // In SIMD lanes (Vec3A): the same multiplies and adds,
                // lane by lane, as Vec3's, so the same values.
                let (position, normal) = (Vec3A::from(v.position), Vec3A::from(v.normal));
                let (mut p, mut n) = (Vec3A::ZERO, Vec3A::ZERO);
                for k in 0..3 {
                    let w = v.weights[k];
                    if w <= 0.0 {
                        continue;
                    }
                    let bone = r.palette.get(v.bones[k] as usize).copied().unwrap_or(0) as usize;
                    let m = palette.get(bone).unwrap_or(&Mat4::IDENTITY);
                    p += w * m.transform_point3a(position);
                    n += w * m.transform_vector3a(normal);
                }
                let t = pose(Vec3::from(world.transform_point3a(p)), Vec3::from(world.transform_vector3a(n)).normalize_or_zero());
                done[i as usize] = ri as u32;
                posed[i as usize] = t;
                emit(t, v.uv, r.texture_slot);
            }
        }
    }
}

/// A 0x37040 record: one character of the scene.
#[derive(Clone, Debug)]
pub struct Instance {
    pub name: u32,
    /// Its place in the scene's space (record +0x10, column-major).
    pub matrix: Mat4,
    pub solids: [u32; 3],
    pub anim: u32,
    pub skeleton: u32,
    /// +0x0C: 0x20 loop, 0x40 loop over [+0x80, +0x84], 0x10 back and
    /// forth (0x437BE0 turns them into the controller's +0x34 bits).
    pub flags: u32,
    /// +0x74: the play rate (0 means 1).
    pub rate: f32,
    /// +0x78: seconds the animation waits (controller +0x6C).
    pub delay: f32,
    /// +0x80 / +0x84: the loop range (flag 0x40), animation frames at the
    /// file's rate.
    pub range: (u32, u32),
}

impl Instance {
    /// The animation frame `t` seconds into the scene (0x434B00 on the
    /// controller 0x437BE0 sets up for a skinned character): the
    /// controller's rate is the record's +0x74 halved, so 15 animation
    /// frames a second; it holds still for the delay (+0x78; 0 in every
    /// race start scene), then runs, wrapping at the end with flag 0x20,
    /// over the range ×rate with 0x40, back and forth with 0x10, else
    /// stopping at the end. `length` is the animation's frame count
    /// (controller +0x64).
    pub fn frame(&self, t: f32, length: f32) -> f32 {
        let rate = if self.rate == 0.0 { 1.0 } else { self.rate } * 0.5;
        let f = ((t - self.delay).max(0.0)) * 30.0 * rate;
        let (start, end) = if self.flags & 0x40 != 0 { (self.range.0 as f32 * rate, self.range.1 as f32 * rate) } else { (0.0, length) };
        let span = end - start;
        if f <= end || span <= 0.0 {
            return f.min(end.max(0.0));
        }
        if self.flags & 0x60 != 0 {
            start + (f - start).rem_euclid(span)
        } else if self.flags & 0x10 != 0 {
            let k = (f - start).rem_euclid(2.0 * span);
            start + if k > span { 2.0 * span - k } else { k }
        } else {
            end
        }
    }
}

impl Instance {
    pub fn parse(c: Chunk<'_>) -> Option<Instance> {
        // 0x4377F0: the record starts at (chunk + 0x17) & !0xF, i.e. the
        // body's first 16-byte aligned byte.
        let r = c.aligned(16);
        (r.len() >= 0x88).then(|| Instance {
            name: read_u32(r, 4),
            matrix: row_matrix(r, 0x10),
            solids: [read_u32(r, 0x50), read_u32(r, 0x54), read_u32(r, 0x58)],
            anim: read_u32(r, 0x6C),
            skeleton: read_u32(r, 0x70),
            flags: read_u32(r, 0x0C),
            rate: read_f32(r, 0x74),
            delay: read_f32(r, 0x78),
            range: (read_u32(r, 0x80), read_u32(r, 0x84)),
        })
    }
}

/// Everything a scene bundle holds for its characters.
#[derive(Clone, Debug, Default)]
pub struct SceneCharacters {
    pub skeletons: Vec<Skeleton>,
    /// The character banks' animations by their name's hash.
    pub anims: Vec<(u32, Channel)>,
    pub models: Vec<SkinnedModel>,
    pub textures: Vec<Texture>,
    pub instances: Vec<Instance>,
}

impl SceneCharacters {
    pub fn parse(bun: &[u8]) -> Result<SceneCharacters> {
        let mut out = SceneCharacters::default();
        for c in chunks(bun, 0) {
            match c.id {
                0x00E3_4009 => out.skeletons.push(Skeleton::parse(c.aligned(16))?),
                0x00E3_4010 => {
                    for (name, ch) in eagl::parse_bank(c.aligned(16)).unwrap_or_default() {
                        if !name.starts_with("ZPM_") && !name.starts_with("XO_") {
                            out.anims.push((bin_hash(&name), ch));
                        }
                    }
                }
                0x8013_4000 => out.models.extend(SkinnedModel::parse_list(c)),
                0xB330_0000 => out.textures.extend(parse_texture_pack(c).unwrap_or_default()),
                0x8003_7020 => out.instances.extend(c.children().filter(|k| k.id == 0x0003_7040).filter_map(Instance::parse)),
                _ => {}
            }
        }
        Ok(out)
    }

    /// One instance's model, skeleton and pose `t` seconds into the scene
    /// (its animation sampled over the bind pose).
    fn pose_of(&self, inst: &Instance, t: f32) -> Option<(usize, &Skeleton, Vec<f32>)> {
        let model = self.models.iter().position(|m| inst.solids.contains(&m.hash))?;
        let skel = self.skeletons.iter().find(|s| s.name_hash == inst.skeleton).or(self.skeletons.first())?;
        let mut pose = skel.bind_pose();
        if let Some((_, ch)) = self.anims.iter().find(|(h, _)| *h == inst.anim) {
            let frame = inst.frame(t, ch.frames() as f32);
            for (s, v) in ch.slots.iter().zip(ch.sample(frame)) {
                if let Some(p) = pose.get_mut(*s as usize) {
                    *p = v;
                }
            }
        }
        Some((model, skel, pose))
    }

    /// Instance `i`'s world matrix: origin × the record's matrix, its
    /// height set to `lifts[i]` + [`GROUND_LIFT`] when the ground under it
    /// is known (0x436050, see [`SceneCharacters::ground_probe`]).
    pub fn world_of(&self, i: usize, origin: Mat4, lifts: &[Option<f32>]) -> Option<Mat4> {
        let mut world = origin * self.instances.get(i)?.matrix;
        if let Some(g) = lifts.get(i).copied().flatten() {
            world.w_axis.z = g + GROUND_LIFT;
        }
        Some(world)
    }

    /// The posed characters `t` seconds into the scene, under the scene's
    /// origin: per instance its model, its skin palette and its world
    /// matrix ([`SceneCharacters::world_of`] with the grounds `lifts`, one
    /// per instance; empty leaves every character at the scene's height).
    pub fn posed(&self, t: f32, origin: Mat4, lifts: &[Option<f32>]) -> Vec<(usize, Vec<Mat4>, Mat4)> {
        (0..self.instances.len())
            .filter_map(|i| {
                let (model, skel, pose) = self.pose_of(&self.instances[i], t)?;
                Some((model, skel.skin_matrices(&pose), self.world_of(i, origin, lifts)?))
            })
            .collect()
    }

    /// Where 0x436050 asks for the ground under each skinned character
    /// (`None` for one that is not posed): 0x431280(1), bone 1's
    /// model-space position (its skin matrix after its bind matrix, i.e.
    /// the model matrix) through the character's world matrix, the one
    /// `lifts` places.
    pub fn ground_probe(&self, t: f32, origin: Mat4, lifts: &[Option<f32>]) -> Vec<Option<Vec3>> {
        (0..self.instances.len())
            .map(|i| {
                let (_, skel, pose) = self.pose_of(&self.instances[i], t)?;
                let bone = *skel.model_matrices(&pose).get(1)?;
                Some(self.world_of(i, origin, lifts)?.transform_point3(bone.w_axis.truncate()))
            })
            .collect()
    }
}

/// 0x436050 / 0x437BE0: a skinned character stands this far over the
/// ground found under its bone 1 (0.05, the world matrix's z set to
/// ground − its z + its z + 0.05).
pub const GROUND_LIFT: f32 = 0.05;

/// The ground filter of 0x436050: the first ground found is taken as is,
/// every later one blended half and half with the last (`0.5·new +
/// (1 − 0.5)·old`) once a character update. **Inferred**: the clone runs
/// that blend once a 30 fps scene frame (frame-rate independent: a
/// weight of 1 − 0.5^(frames)), the original once a drawn frame.
pub fn filter_ground(old: Option<f32>, new: f32, frames: f32) -> f32 {
    match old {
        None => new,
        Some(o) => {
            let k = 1.0 - 0.5f32.powf(frames.max(0.0));
            o + (new - o) * k
        }
    }
}

/// The flag girl's flare table (0x7EA110, 0x34 bytes an entry, to
/// 0x7EA1E0): a model name, then four points in the model's bind space
/// (row-vector floats): the left flare's tip and base, the right one's.
const FLARE_MODELS: [(&str, [[f32; 3]; 4]); 4] = [
    ("FLAG_GIRL_MODEL_5_FLARES", [[0.6865, -0.0878, 1.2030], [0.5926, 0.0478, 1.2020], [-0.6865, -0.0878, 1.2030], [-0.5926, 0.0478, 1.2020]]),
    ("FLAG_GIRL_MODEL_7_FLARES", [[0.6642, -0.2284, 1.2219], [0.5694, 0.0558, 1.2091], [-0.6642, -0.2284, 1.2219], [-0.5694, 0.0558, 1.2091]]),
    ("FLAG_GIRL_MODEL_8_FLARES", [[0.6735, -0.0397, 1.1411], [0.5896, 0.0816, 1.2023], [-0.6735, -0.0397, 1.1411], [-0.5896, 0.0816, 1.2023]]),
    ("FLAG_GIRL_MODEL_9_FLARES", [[0.6187, -0.0830, 1.0365], [0.5235, 0.0725, 1.0930], [-0.6187, -0.0830, 1.0365], [-0.5235, 0.0725, 1.0930]]),
];
/// 0x7EA1E8 / 0x7EA1EC (kind 0 of the 0x10-byte table at 0x7EA1E0): the
/// bones that carry the left and the right flare.
const FLARE_BONES: [usize; 2] = [18, 28];
/// 0x433090: the glow sits 0.4 (0x3ECCCCCD) of the tip − base vector past
/// the tip.
const FLARE_REACH: f32 = 0.4;

impl SceneCharacters {
    /// The flag girl's flares (0x4335C0, from the character update
    /// 0x436050 when its +0x1C ≥ 0): for each skinned character (skeleton
    /// and animation bound) whose skeleton has 35 bones (0x437BE0:
    /// skeleton +0x10 = 0x23 sets +0x24 = 0) and whose model (record
    /// +0x50) is one of [`FLARE_MODELS`] (+0x1C its index), the two flare
    /// glows 0x433090 pools (pool 1, mask 3, type 14, flags 2): each tip
    /// p1 and base p2 through its bone's skin matrix and the character's
    /// world matrix, the glow at p1 + 0.4·(p1 − p2).
    pub fn flare_glows(&self, t: f32, origin: Mat4, lifts: &[Option<f32>]) -> Vec<Vec3> {
        self.flares(t, origin, lifts).into_iter().map(|(tip, v)| tip + v * FLARE_REACH).collect()
    }

    /// The flares 0x4335C0 hands 0x433090: each tip p1 and its vector
    /// p1 − p2 (the glow is pooled at p1 + 0.4·v, the emitters fired at p1).
    pub fn flares(&self, t: f32, origin: Mat4, lifts: &[Option<f32>]) -> Vec<(Vec3, Vec3)> {
        let mut out = Vec::new();
        for (i, inst) in self.instances.iter().enumerate() {
            let Some(points) = FLARE_MODELS.iter().find(|(name, _)| bin_hash(name) == inst.solids[0]).map(|e| e.1) else { continue };
            if inst.skeleton == 0 || inst.anim == 0 {
                continue;
            }
            let Some(skel) = self.skeletons.iter().find(|s| s.name_hash == inst.skeleton) else { continue };
            if skel.bones.len() != 0x23 {
                continue;
            }
            let mut pose = skel.bind_pose();
            if let Some((_, ch)) = self.anims.iter().find(|(h, _)| *h == inst.anim) {
                let frame = inst.frame(t, ch.frames() as f32);
                for (s, v) in ch.slots.iter().zip(ch.sample(frame)) {
                    if let Some(p) = pose.get_mut(*s as usize) {
                        *p = v;
                    }
                }
            }
            let palette = skel.skin_matrices(&pose);
            let Some(world) = self.world_of(i, origin, lifts) else { continue };
            for (side, bone) in FLARE_BONES.iter().enumerate() {
                let Some(m) = palette.get(*bone) else { continue };
                let tip = world.transform_point3(m.transform_point3(Vec3::from(points[2 * side])));
                let base = world.transform_point3(m.transform_point3(Vec3::from(points[2 * side + 1])));
                out.push((tip, tip - base));
            }
        }
        out
    }
}

/// A simple directional shading for the posed vertices (**not native**: the
/// character effect is not traced): ambient plus half-Lambert.
pub fn shade(normal: Vec3, light: Vec3) -> f32 {
    0.55 + 0.45 * normal.dot(light).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_plays_at_half_rate_and_loops() {
        let mut i = Instance { name: 0, matrix: Mat4::IDENTITY, solids: [0; 3], anim: 0, skeleton: 0, flags: 0x20, rate: 1.0, delay: 0.0, range: (0, 0) };
        // 15 animation frames a second.
        assert!((i.frame(2.0, 100.0) - 30.0).abs() < 1e-4);
        assert!((i.frame(8.0, 100.0) - 20.0).abs() < 1e-3);
        // A loop range ×rate: [0, 60] for +0x84 = 120.
        i.flags = 0x60;
        i.range = (0, 120);
        assert!((i.frame(5.0, 61.0) - 15.0).abs() < 1e-3);
        // No loop: held at the end.
        i.flags = 0;
        assert_eq!(i.frame(10.0, 40.0), 40.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_flag_girls_load_and_stand() {
        let root = crate::testkit::game_root();
        let bun = std::fs::read(std::path::Path::new(&root).join("NIS/Scene_IntroNis08_BundleB.bun")).unwrap();
        let set = SceneCharacters::parse(&bun).unwrap();
        assert_eq!((set.skeletons.len(), set.models.len(), set.instances.len()), (2, 2, 2));
        assert_eq!(set.skeletons[0].bones.len(), 35);
        assert_eq!(set.skeletons[0].name_hash, bin_hash("Bip11"));
        assert_eq!(set.instances[0].anim, bin_hash("two_flag_girl_idle_starts_01_07_s"));
        assert_eq!(set.anims.len(), 2);
        // The bind pose reproduces the mesh: every skin matrix is the
        // identity.
        let s = &set.skeletons[0];
        for m in s.skin_matrices(&s.bind_pose()) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-3), "{m:?}");
        }
        // IntroNis08's girls (models 7 and 8, no flares) hold none.
        assert!(set.flare_glows(0.0, Mat4::IDENTITY, &[]).is_empty());
        // Posed at frame 0 the girl stands upright: her vertices span about
        // her height, feet near the ground.
        for (model, palette, world) in set.posed(0.0, Mat4::IDENTITY, &[]) {
            let tris = set.models[model].skin(&palette, world);
            let (lo, hi) = tris.iter().fold((f32::MAX, f32::MIN), |(lo, hi), t| (lo.min(t.0.z), hi.max(t.0.z)));
            assert!(hi - lo > 1.4 && hi - lo < 2.1, "height {}", hi - lo);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_flare_girl_holds_two_glows_at_hand_height() {
        let root = crate::testkit::game_root();
        let bun = std::fs::read(std::path::Path::new(&root).join("NIS/Scene_IntroNis12_BundleB.bun")).unwrap();
        let set = SceneCharacters::parse(&bun).unwrap();
        let world = set.instances.iter().find(|i| i.solids[0] == bin_hash("FLAG_GIRL_MODEL_9_FLARES")).expect("the flare girl").matrix;
        let g = set.flare_glows(0.0, Mat4::IDENTITY, &[]);
        assert_eq!(g.len(), 2, "{g:?}");
        let at = world.w_axis.truncate();
        for p in &g {
            // About her hands: within 1.5 m of her origin, 0.3–2.5 m up.
            assert!((p.truncate() - at.truncate()).length() < 1.5 && p.z - at.z > 0.3 && p.z - at.z < 2.5, "{p:?} from {at:?}");
        }
    }
}

#[cfg(test)]
mod cost {
    use super::*;

    /// The crowd's CPU cost: every IntroNis scene's characters posed and
    /// skinned at 300 frames, and a checksum of the corners' bits (to
    /// check a change keeps the values). `cargo test --release
    /// character::cost -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark: CPU cost of the crowd skinning"]
    fn skin_cost() {
        let root = crate::testkit::game_root();
        let sets: Vec<SceneCharacters> = (1..=18)
            .filter_map(|k| std::fs::read(std::path::Path::new(&root).join(format!("NIS/Scene_IntroNis{k:02}_BundleB.bun"))).ok())
            .filter_map(|b| SceneCharacters::parse(&b).ok())
            .collect();
        let origin = Mat4::from_translation(Vec3::new(1000.0, -2000.0, 30.0));
        let (mut pose_ns, mut skin_ns, mut sum, mut corners) = (0u128, 0u128, 0u64, 0usize);
        let (mut done, mut scratch, mut scratch2, mut out) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for f in 0..300 {
            let t = f as f32 / 60.0;
            for set in &sets {
                let t0 = std::time::Instant::now();
                let posed = set.posed(t, origin, &[]);
                pose_ns += t0.elapsed().as_nanos();
                let t1 = std::time::Instant::now();
                out.clear();
                for (model, palette, at) in &posed {
                    if f % 2 == 0 {
                        set.models[*model].skin_ranges(palette, *at, &mut done, &mut scratch, |p, n| (p, n), |_| true, |(p, n), uv, slot| {
                            for x in [p.x, p.y, p.z, n.x, n.y, n.z, uv[0], uv[1]] {
                                sum = sum.rotate_left(5) ^ x.to_bits() as u64;
                            }
                            sum = sum.wrapping_add(slot as u64);
                            corners += 1;
                        });
                    } else {
                        // Timed alone: the corners into a vertex list.
                        set.models[*model].skin_ranges(palette, *at, &mut done, &mut scratch2, |p, n| (p, n.x), |_| true, |(p, g), uv, _| out.push((p, g, uv)));
                    }
                }
                if f % 2 == 1 {
                    skin_ns += t1.elapsed().as_nanos();
                }
            }
        }
        println!("skin_cost: {} scenes, {:.0} corners a frame; pose {:.3} ms, skin {:.3} ms a frame; checksum {sum:016x} ({} corners last)", sets.len(), corners as f64 / 150.0, pose_ns as f64 / 300.0 / 1e6, skin_ns as f64 / 150.0 / 1e6, out.len());
    }
}

#[cfg(test)]
#[path = "character/visibility_tests.rs"]
mod visibility_tests;
