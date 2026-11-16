//! The world draws, their keys, uniforms and planes (moved from render/mod.rs).

use crate::render::*;

/// Per-pass values the shader reads (`Frame` in world.wgsl).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FrameUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) camera: [f32; 4],
    pub(crate) forward: [f32; 4],
    pub(crate) fog_value: [f32; 4],
    pub(crate) fog_color: [f32; 4],
    /// x: 1 for enhanced reflection shading, otherwise unused (-inf);
    /// y: time in seconds (texture scrolling); z:
    /// SKYMORPHALPHA; w: 1 in the road reflection.
    pub(crate) clip: [f32; 4],
    /// x: the view's cull scale ([`cull_scale`]), y: the size it needs.
    pub(crate) cull: [f32; 4],
    /// View 9 ([`headlight_uniform`]); eye.w = 0 turns the term off.
    pub(crate) headlight: [[f32; 4]; 5],
    /// Wet Roads (clone deviation, docs/wet-roads-ssr.md), main view
    /// only: on (1 / 0 native), the rain's amount, the wetness, unused.
    pub(crate) wet: [f32; 4],
    /// Dynamic Lamps / Lamp Shadows / Contact Shadows (clone deviations,
    /// docs/lamps-shadows.md, [`lamps`]), main view only; zeros natively.
    pub(crate) lamps: [f32; 4],
    pub(crate) cluster: [f32; 4],
    pub(crate) cluster_near: [f32; 4],
}

/// Fog used before the track's fog map has been sampled: none.
pub(crate) const NO_FOG: Fog = Fog { value: [1.0e9, 0.0, 1.0], color: [0.0; 3] };

/// The game effect a draw uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Effect {
    /// skymorph.efx (effect 20).
    SkyMorph,
    World,
    NoFog,
    /// Clone-only road capture fragment; native entries have no plane discard.
    PlanarNoFog,
    Reflect,
    /// clone, lamps module only: the cars' shadow texture over a
    /// ground the Contact Shadows already darkened, the two combined as
    /// the darker of them, not their product (world.wgsl `fs_car_shadow`).
    CarShadow,
}

impl Effect {
    /// The effect's slot in 0x86EB28 (descriptor table 0x7FF850).
    pub(crate) fn index(self) -> u8 {
        match self {
            Effect::World => 0,
            Effect::Reflect => 2,
            Effect::NoFog | Effect::PlanarNoFog | Effect::CarShadow => 7,
            Effect::SkyMorph => 20,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PipelineKey {
    pub(crate) effect: Effect,
    pub(crate) blend: Blend,
    pub(crate) alpha_test: bool,
}

impl PipelineKey {
    pub(crate) fn all() -> Vec<PipelineKey> {
        let effects = [Effect::SkyMorph, Effect::World, Effect::NoFog, Effect::Reflect];
        let blends = [Blend::None, Blend::Alpha, Blend::Additive, Blend::DestAlpha, Blend::DestAlphaAdditive];
        let mut keys = Vec::new();
        for effect in effects {
            for blend in blends {
                for alpha_test in [false, true] {
                    keys.push(PipelineKey { effect, blend, alpha_test });
                }
            }
        }
        keys
    }

    /// The same draw in an off-screen target (the road reflection, the
    /// environment cube): 0x5C5940 turns every non-car effect into
    /// worldnofog whenever the current target record (0x870848, set by
    /// 0x5BA3D0) is not the back buffer (record index 0).
    pub(crate) fn in_reflection(self) -> PipelineKey {
        PipelineKey { effect: Effect::NoFog, ..self }
    }
}

/// A draw's texture: a fixed one, a flip-book chosen by time, or half
/// (0 main, 1 cap) of the sky pair chosen by the time of day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TextureRef {
    Fixed(usize),
    Animated(usize),
    Sky(usize),
}

/// One instanced draw: a solid's material run, for all of its placements.
pub(crate) struct Draw {
    /// Part of the SKYDOME (drawn first, at the camera, main views only).
    pub(crate) sky: bool,
    /// Scenery section the instances belong to (for visibility culling).
    pub(crate) section: u32,
    /// The instances' flags the views cull by ([`WorldView::culls`]; 0x0400
    /// is left out of the road reflection: 0x5D6AE0 turns it into draw
    /// flag 0x2000, which the reflection's pass flag 8 rejects).
    pub(crate) flags: u16,
    /// Which views draw it: [`MAIN_LOD`] (the top LOD, view 1) and/or
    /// [`LOW_LOD`] (the placement's `low_solid`, views 4 and 0x10–0x15).
    pub(crate) lods: u8,
    pub(crate) key: PipelineKey,
    pub(crate) rendering_order: u8,
    /// Texture entry byte 0x54 set (the second bucket set).
    pub(crate) late_buckets: bool,
    pub(crate) texture: TextureRef,
    pub(crate) first_index: u32,
    pub(crate) index_count: u32,
    pub(crate) base_vertex: i32,
    pub(crate) instances: std::ops::Range<u32>,
    /// Its instance group's bounds ([`WorldRenderer::group_bounds`]).
    pub(crate) group: u32,
    /// Animated Water (docs/water.md): its texture is a water
    /// texture ([`water::kind_of`]); the main view leaves it to
    /// `render::water` while that is on.
    pub(crate) water: bool,
}

/// The four side planes of a view's clip volume (x and y within ±w),
/// for skipping draws wholly outside them: they would put no pixel on
/// screen, so the image is the same without them.
pub(crate) fn side_planes(view_proj: Mat4) -> [glam::Vec4; 4] {
    let (r0, r1, r3) = (view_proj.row(0), view_proj.row(1), view_proj.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1].map(|p| p / p.truncate().length().max(f32::MIN_POSITIVE))
}

/// The near and far planes of a clip volume (depth within 0..w): all six
/// planes, for the cube faces, whose far plane (64 m) leaves out most of
/// what their sides take in.
pub(crate) fn frustum_planes(view_proj: Mat4) -> Vec<glam::Vec4> {
    let (r2, r3) = (view_proj.row(2), view_proj.row(3));
    let mut planes = side_planes(view_proj).to_vec();
    planes.extend([r2, r3 - r2].map(|p| p / p.truncate().length().max(f32::MIN_POSITIVE)));
    planes
}

/// Whether a sphere lies wholly outside one of the planes, with a margin
/// against rounding (the draw is kept when in doubt).
pub(crate) fn outside(planes: &[glam::Vec4], centre: Vec3, radius: f32) -> bool {
    let r = radius * 1.01 + 0.25;
    planes.iter().any(|p| p.truncate().dot(centre) + p.w < -r)
}

/// The most a transform's 3×3 part stretches a vector (its largest
/// singular value, the square root of MᵀM's largest eigenvalue), a hair
/// over: what a bounding sphere's radius scales by. A placement's longest
/// column falls short of it for the 3% of Bayview's whose 3×3 is not a
/// scaled rotation (up to 1.24×), and so does every matrix the road
/// reflection mirrors.
pub(crate) fn max_stretch(m: glam::Mat3) -> f32 {
    let m = glam::DMat3::from_cols(m.x_axis.as_dvec3(), m.y_axis.as_dvec3(), m.z_axis.as_dvec3());
    let a = m.transpose() * m;
    let (a00, a01, a02, a11, a12, a22) = (a.x_axis.x, a.y_axis.x, a.z_axis.x, a.y_axis.y, a.z_axis.y, a.z_axis.z);
    // The largest eigenvalue of a symmetric 3×3 (Smith, 1961).
    let p1 = a01 * a01 + a02 * a02 + a12 * a12;
    let largest = if p1 == 0.0 {
        a00.max(a11).max(a22)
    } else {
        let q = (a00 + a11 + a22) / 3.0;
        let p2 = (a00 - q).powi(2) + (a11 - q).powi(2) + (a22 - q).powi(2) + 2.0 * p1;
        let p = (p2 / 6.0).sqrt();
        let b = (a - glam::DMat3::from_diagonal(glam::DVec3::splat(q))) * (1.0 / p);
        let r = (b.determinant() / 2.0).clamp(-1.0, 1.0);
        q + 2.0 * p * (r.acos() / 3.0).cos()
    };
    (largest.max(0.0).sqrt() * (1.0 + 1e-5)) as f32
}

/// Write instance matrices (slot, matrix): one copy per run of adjacent
/// slots.
/// `x.trunc()` without the libm call (baseline x86-64 has no rounding
/// instruction; per instance per frame in `update_fades`): the same value
/// for every finite `x` and NaN (a negative fraction gives +0 where trunc
/// gives −0, which compare equal).
#[inline]
pub(crate) fn trunc_fast(x: f32) -> f32 {
    // Below 2^23 the float has a fraction; from there it is whole.
    if x.abs() < 8_388_608.0 { x as i32 as f32 } else { x }
}

pub(crate) fn write_slots(gpu: &Gpu, buffer: &wgpu::Buffer, mut writes: Vec<(u32, [[f32; 4]; 4])>) {
    writes.sort_by_key(|w| w.0);
    writes.dedup_by(|b, a| {
        // A slot written twice keeps its last matrix (as the writes in
        // turn did).
        if a.0 == b.0 {
            a.1 = b.1;
            true
        } else {
            false
        }
    });
    let mut i = 0;
    while i < writes.len() {
        let mut j = i + 1;
        while j < writes.len() && writes[j].0 == writes[j - 1].0 + 1 {
            j += 1;
        }
        let run: Vec<[[f32; 4]; 4]> = writes[i..j].iter().map(|w| w.1).collect();
        gpu.queue.write_buffer(buffer, writes[i].0 as u64 * 64, bytemuck::cast_slice(&run));
        i = j;
    }
}

/// A flip-book: bind groups for each frame, shown at `fps`.
pub(crate) struct Animation {
    pub(crate) fps: f32,
    pub(crate) frames: Vec<usize>,
}

/// A car's headlight pool (0x6154A0 → 0x60D5E0): a quad on the car's
/// ground plane ahead of it, textured `2PLAYERHEADLIGHT1` (GLOBALB,
/// additive), which the game draws into the road reflection (view 4) only —
/// 0x6154A0's one caller is 0x5CAC17, in view 4's block of 0x5CAAE0 — so
/// the road shows it through worldreflect.
pub struct HeadlightPool {
    /// The car's matrix (0x6154A0: car +0xD0 with the position at +0x50;
    /// not flipped, 0x6154A0 hands it over as it is).
    pub transform: Mat4,
    /// R, G, B ([`crate::car::Car::headlight_pool_color`]).
    pub color: [u8; 3],
}

/// Car-space corners of the pool and their UVs (0x60D5E0): x from
/// 13 − 10 to 13 + 10 (0x8028CC, 0x8028E4/E8/F4/F8), y ±4.9 (0x8028EC/F0/FC,
/// 0x802900) about 0x8A1C4C (BSS, never written: 0), z 0; a fan
/// (D3DPT_TRIANGLEFAN, 0x5C5590) through (0,0), (1,0), (1,1), (0,1).
pub(crate) const POOL_CORNERS: [([f32; 3], [f32; 2]); 4] = [
    ([3.0, -4.9, 0.0], [0.0, 0.0]),
    ([23.0, -4.9, 0.0], [1.0, 0.0]),
    ([23.0, 4.9, 0.0], [1.0, 1.0]),
    ([3.0, 4.9, 0.0], [0.0, 1.0]),
];
/// Pools drawn per frame at most (the clone's bound, not the game's).
pub(crate) const MAX_POOLS: usize = 32;

/// Road reflection flare quads drawn per frame at most (
/// **verified** capacity): 0x491860 writes them into the sprite batch
/// 0x86B7A4, which 0x5BB230 creates for 1000 quads (0x5D1DDE / 0x5D27F0
/// `mov …,0x3e8`: 4000 vertices, 6000 indices); 0x5BB440 drops a quad once
/// the write pointer reaches the end. The flares are one texture
/// (HEADLIGHTGLOW), so one batch: the rest of a frame's are dropped.
pub(crate) const MAX_REFLECTION_GLOWS: usize = 1000;

/// Particle quads drawn per frame at most (the game's pool, 0x488750).
pub(crate) const MAX_PARTICLE_QUADS: usize = crate::particles::MAX_PARTICLES;

#[cfg(test)]
mod sprite_batch_tests {
    /// 0x5BB230's sprite batch holds 1000 quads (0x5D27F0 `mov …,0x3e8`).
    #[test]
    fn reflection_flares_stop_at_the_batch_size() {
        assert_eq!(super::MAX_REFLECTION_GLOWS, 1000);
    }
}

#[cfg(test)]
mod trunc_fast_tests {
    #[test]
    fn trunc_fast_is_trunc() {
        let mut x = 0x1234_5678u32;
        let check = |v: f32| {
            let (a, b) = (crate::render::trunc_fast(v), v.trunc());
            assert!(a == b || (a.is_nan() && b.is_nan()), "{v}: {a} vs {b}");
        };
        for v in [0.0, -0.0, 0.5, -0.5, 1.0, 16.999, 8_388_607.5, 8_388_608.0, -8_388_607.5, 1e30, -1e30, f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, f32::MIN_POSITIVE] {
            check(v);
        }
        for _ in 0..200_000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            check(f32::from_bits(x));
        }
    }
}
