//! Instance encoding, the scenery cull and the CPU pass cull (moved from render/mod.rs).

use crate::render::*;

/// A placement's per-instance data: its matrix, with the otherwise unused
/// m0.w carrying whether the game gives the draw a matrix at all (only
/// those are flipped in the road reflection; see `vs_common`) and m1.w the
/// radius the scenery cull sizes it with (`Placement::cull_radius`; 0 for
/// draws that do not go through the cull).
/// An instance's matrix, with the has-matrix flag in m0.w and the cull
/// radius in m1.w; the radius negated when the instance's flags have
/// [`HIDDEN_FLAG`], which every view culls (the shader drops it).
pub(crate) fn instance_matrix(transform: Mat4, has_matrix: bool, cull_radius: f32, hidden: bool) -> [[f32; 4]; 4] {
    let mut m = transform.to_cols_array_2d();
    m[0][3] = if has_matrix { 1.0 } else { 0.0 };
    m[1][3] = if hidden { -cull_radius.abs().max(1.0) } else { cull_radius };
    m
}

/// What [`PassCull`] reads of an instance's data: its position and its
/// signed cull radius.
pub(crate) fn instance_cull(m: &[[f32; 4]; 4]) -> [f32; 4] {
    [m[3][0], m[3][1], m[3][2], m[1][3]]
}

/// The size scale a view's cull context carries at +0x80: 0x5E9A40 copies
/// it from the view's +0x0C, which 0x5C7ED0 sets with the projection to
/// `256 / tan(fov / 2)` (0x78D860; 10 when the tangent is 0, 0x784560),
/// from the camera's own field of view (+0xC4, halved by a shift), before
/// the 1.1 widening of the player views.
pub fn cull_scale(fov: f32) -> f32 {
    let t = (fov * 0.5).tan();
    if t == 0.0 { 10.0 } else { 256.0 / t }
}

/// 0x5D64E0 on the CPU (the shader's `cull_size`): how big the scenery
/// cull finds an instance at `p` with radius `r`, truncated: 0 behind
/// (forward distance below −r), the scale itself within 2r, else
/// `scale · r / (|eye − p| − r)`.
pub fn scenery_cull_size(p: Vec3, r: f32, eye: Vec3, forward: Vec3, scale: f32) -> f32 {
    let d = p - eye;
    if d.dot(forward) < -r {
        return 0.0;
    }
    let gap = d.length() - r;
    if gap <= r { scale.trunc() } else { (scale / gap * r).trunc() }
}

/// The sizes 0x5D6580 needs (0x5D64E0's `scale · r / (|d| − r)`,
/// truncated): 17 in the full-screen main view (23 in split screen,
/// 0x5D66A4), 32 in the views whose cull base has 0x1800 (0x5D6730 /
/// 0x5D673E).
pub const MAIN_MIN_SIZE: f32 = 17.0;

/// The main view's size cull in force: [`MAIN_MIN_SIZE`] divided by the
/// Graphics page's Detail Distance (clone deviation; 1 native).
///
/// docs/pop-in.md: at World Detail Ultra / Extreme the size need
/// is set on the output's pixels instead — a radius of
/// [`crate::render::RenderOptions::DETAIL_FULL_PX`] output pixels (the cull's sizes are
/// pixels of radius on a 512-wide view) — when that is smaller, so
/// scenery comes in where it is a speck at any resolution, not at the
/// 640-wide game's 17 px (×7.5 at 4K: a 64-pixel radius at 200 %).
pub fn main_min_size(opts: &crate::render::RenderOptions) -> f32 {
    // Pop-in measuring aid (NFSU2_POP_ORACLE=1, docs/pop-in.md): every
    // instance of the drawn sections drawn, the reference the pop counts
    // are taken against.
    if pop_meter::oracle() {
        return 0.0;
    }
    let native = MAIN_MIN_SIZE / opts.detail_distance;
    if opts.pop_free {
        let w = opts.output_size_or((1280, 960)).0.max(1) as f32;
        native.min(crate::render::RenderOptions::DETAIL_FULL_PX * 512.0 / w)
    } else {
        native
    }
}

/// The Detail Distance's size need alone (the road reflection's enhanced
/// capture and the views that follow the option's percentage, not the
/// pixel rule of [`main_min_size`]).
pub(crate) fn detail_min_size(opts: &crate::render::RenderOptions) -> f32 {
    MAIN_MIN_SIZE / opts.detail_distance
}
pub(crate) const LOW_LOD_MIN_SIZE: f32 = 32.0;

/// Enhanced capture retains main-view mesh detail and distance. The native
/// view flags, bounds and projection still apply to both road and car captures.
pub(crate) fn reflection_capture_quality(enhanced: bool, main_lod: u8, main_min_size: f32) -> (u8, f32) {
    if enhanced { (main_lod, main_min_size) } else { (LOW_LOD, LOW_LOD_MIN_SIZE) }
}

/// The headlight projector, view 9 (0x5C2AF0 → camera 0x870A20; drawn
/// when g_CarHeadlightEnable, 0x870CC4, is on — at the top detail levels,
/// 0x5BE90C). In game (0x8654A4 = 6) it sits on car 0 (0x5ED990(0) →
/// 0x5F0E40): eye at the car's position + 0.75 × its up row (+0xB0,
/// 0x7A27BC; the forward row's 0 × 0x7A27B8 adds nothing), looking along
/// its forward row (+0x90) with 0.1 taken off z (0x7A27C8), normalised
/// (0x43CF10), up (0, 0, 1), through the look-at 0x5BA9B0; near 1
/// (0x7A27B4), far 100 (0x7FEE58), field of view 2.5 rad (0x7A27CC ×
/// 0x78423C = 26075 binary). 0x5C7ED0 projects views 9 and 10 with
/// factor 1 across and 0.6 (0x3F19A956) in place of h/w.
pub(crate) const HEADLIGHT_HEIGHT: f32 = 0.75;
pub(crate) const HEADLIGHT_DROP: f32 = 0.1;
pub(crate) const HEADLIGHT_FOV_BINARY: u16 = 26075;

/// What the world shaders need of view 9: the projector's frame, the
/// projection's two scales, the view's cull scale and the colour.
pub fn headlight_uniform(pool: Option<&HeadlightPool>) -> [[f32; 4]; 5] {
    let Some(pool) = pool else { return [[0.0; 4]; 5] };
    let forward = pool.transform.x_axis.truncate();
    let up = pool.transform.z_axis.truncate();
    let eye = pool.transform.w_axis.truncate() + up * HEADLIGHT_HEIGHT;
    let dir = (forward - Vec3::Z * HEADLIGHT_DROP).normalize_or(Vec3::Y);
    let right = dir.cross(Vec3::Z).normalize_or(Vec3::X);
    let view_up = right.cross(dir);
    // 0x5C7ED0 halves the binary angle with a shift.
    let half = f32::from(HEADLIGHT_FOV_BINARY >> 1) * std::f32::consts::TAU / 65536.0;
    let fov = f32::from(HEADLIGHT_FOV_BINARY) * std::f32::consts::TAU / 65536.0;
    let across = 1.0 / (fov * 0.5).tan();
    let down = 1.0 / (fov * 0.6 * 0.5).tan();
    let scale = 256.0 / half.tan();
    // HeadlightColor (0x5BDAB0): 0x60F520 of the player (render info
    // +0x380, the pool's colour) as bytes / 128 (0x787AF0). It is the
    // ps_1_1 pixel shader's c0, and D3D9 clamps a ps_1_x float constant to
    // [-1, 1]: under Wine (apitrace, a Lancer at the garage exit) c0
    // was set to (1.17, 1.80, 1.99) from 0xFFE696 and the pool came out
    // white (231, 238, 224 before the post passes), not blue.
    let c = pool.color.map(|b| (f32::from(b) / 128.0).min(1.0));
    [
        [eye.x, eye.y, eye.z, 1.0],
        [right.x, right.y, right.z, across],
        [view_up.x, view_up.y, view_up.z, down],
        [dir.x, dir.y, dir.z, scale],
        [c[0], c[1], c[2], 0.0],
    ]
}

/// The world views the clone draws, by the game's view number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldView {
    /// View 1, the player's.
    Main,
    /// View 4, the road reflection.
    RoadReflection,
    /// Opt-in view 4 scenery, clipped to its local receiving plane.
    PlanarRoadReflection,
    /// Views 0x10–0x15, the cars' environment cube.
    Environment,
    /// View 3, the rear-view mirror (camera 0x8718C0, target record 3).
    RearView,
}

impl WorldView {
    pub(crate) fn is_road_reflection(self) -> bool {
        matches!(self, Self::RoadReflection | Self::PlanarRoadReflection)
    }

    /// The scenery cull flags 0x4888A0 gives the view, as 0x5B9C30 sets
    /// them up in game: a base of 0x6000 for views 1 and 2 (0x8707A4, never
    /// written, is 0), 0x800 for view 4 and 0x1000 for the cube faces; then
    /// 0x10 always, 0x02 for views 1 and 2, 0x08 forward / 0x04 reversed
    /// (0x89E7A4), 0x40 with a base in 0x1800, 0x20 for view 3, 0x01 in
    /// the split-screen layouts (0x8707DC = 3 or 4; never in the clone).
    /// View 3 takes base 0x1000 (0x5B9C30: `iVar3 == 3`), so it has 0x40
    /// and 0x20.
    pub fn cull_flags(self, reversed: bool) -> u32 {
        let (base, main) = match self {
            WorldView::Main => (0x6000, true),
            WorldView::RoadReflection | WorldView::PlanarRoadReflection => (0x800, false),
            WorldView::Environment | WorldView::RearView => (0x1000, false),
        };
        let mut flags = base | 0x10 | if reversed { 0x04 } else { 0x08 };
        if main {
            flags |= 0x02;
        }
        if flags & 0x1800 != 0 {
            flags |= 0x40;
        }
        if self == WorldView::RearView {
            flags |= 0x20;
        }
        flags
    }

    /// Whether the view drops a scenery instance with these flags: the cull
    /// 0x5D6580 skips it when `(flags ^ 0x60) & cull flags' low byte`, and
    /// the road reflection's pass (flag 8 to 0x5D6AE0) also skips 0x0400.
    pub fn culls(self, instance_flags: u16, reversed: bool) -> bool {
        (u32::from(instance_flags) ^ 0x60) & self.cull_flags(reversed) & 0xFF != 0
            || (self.is_road_reflection() && instance_flags & NOT_REFLECTED_FLAG != 0)
    }
}

/// Which half of a sky pair the dome's material is authored with: 0x60F890
/// remaps `SKY_SUNRISE_A` to the pair's main texture and `SKY_SUNRISE_A_CAP`
/// to its cap.
pub(crate) fn sky_part(authored: &Texture) -> Option<usize> {
    match authored.name.as_str() {
        "SKY_SUNRISE_A" => Some(0),
        "SKY_SUNRISE_A_CAP" => Some(1),
        _ => None,
    }
}

/// Section 2600 (`Z0`) holds scenery that is drawn everywhere (the sky).
pub const GLOBAL_SECTION: u32 = 2600;

/// An instance's bounding spheres as the views test them: its solid's
/// sphere (`local`) placed by the matrix (radius × [`max_stretch`]), and
/// as the road reflection draws it (m22 negated when it has a matrix).
/// The same values `WorldRenderer::new` puts round its groups.
pub(crate) fn instance_spheres(local: (Vec3, f32), m: Mat4, has_matrix: bool) -> [(Vec3, f32); 2] {
    let mut mirrored = m;
    if has_matrix {
        mirrored.z_axis.z = -mirrored.z_axis.z;
    }
    let sphere = |m: Mat4| (m.transform_point3(local.0), local.1 * max_stretch(glam::Mat3::from_mat4(m)));
    [sphere(m), sphere(mirrored)]
}

/// How a pass's draws are tested on the CPU before they are recorded. A
/// draw is left out only when it would put no pixel on screen — wholly
/// outside the view's planes, or every one of its instances dropped by
/// the vertex shader's scenery cull — so the image is the same without
/// it, and fewer draws cost less to record and to run.
pub(crate) struct PassCull {
    /// The planes the view clips by.
    pub(crate) planes: Vec<glam::Vec4>,
    /// Test the groups' bounds as the road reflection mirrors them
    /// ([`WorldRenderer::mirrored_bounds`]).
    pub(crate) mirrored: bool,
    /// The scenery cull as the pass's frame uniform sets it up (`camera`,
    /// `forward`, `cull`; see `cull_size` in world.wgsl).
    pub(crate) eye: Vec3,
    pub(crate) forward: Vec3,
    pub(crate) scale: f32,
    pub(crate) min_size: f32,
    /// LOD Fade (main view): each slot's fade; the CPU's state decides
    /// which scenery instances are drawn (`min_size` is then 0).
    pub(crate) fades: Option<std::sync::Arc<Vec<f32>>>,
}

impl PassCull {
    /// [`PassCull::drops`] for slot `i`: with LOD Fade, a scenery
    /// instance whose fade is 0 or that is behind the eye.
    pub(crate) fn drops_slot(&self, i: usize, c: [f32; 4]) -> bool {
        let Some(f) = &self.fades else { return self.drops(c) };
        let r = c[3];
        if r < 0.0 {
            return true;
        }
        if f.get(i).copied().unwrap_or(1.0) == 0.0 {
            return true;
        }
        if r == 0.0 {
            return false;
        }
        let d = Vec3::new(c[0], c[1], c[2]) - self.eye;
        d.dot(self.forward) < -r - (1e-4 * (d.length() + r) + 1e-3)
    }

    /// Whether the vertex shader surely drops an instance (position, cull
    /// radius as [`instance_matrix`] stores them): the shader's own test
    /// with a margin for the GPU's rounding, so an instance near the
    /// threshold is always left to the shader.
    pub(crate) fn drops(&self, c: [f32; 4]) -> bool {
        let r = c[3];
        if r < 0.0 {
            // HIDDEN_FLAG: every view drops it.
            return true;
        }
        // No size needed (`min_size` 0): the shader drops nothing else.
        if r == 0.0 || self.min_size <= 0.0 {
            return false;
        }
        let d = Vec3::new(c[0], c[1], c[2]) - self.eye;
        let len = d.length();
        let slack = 1e-4 * (len + r) + 1e-3;
        // Behind the eye: size 0.
        if d.dot(self.forward) < -r - slack {
            return true;
        }
        let gap = len - r;
        if gap <= r + slack {
            return false;
        }
        // trunc(scale / gap · r) < min, the sizes being whole numbers.
        self.scale / gap * r < self.min_size * (1.0 - 1e-4) - 1e-3
    }
}

// Refactor: from render/mod.rs
/// Scenery instance flag 0x0400 (see `Draw::flags`).
pub(in crate::render) const NOT_REFLECTED_FLAG: u16 = 0x0400;
/// The instance flags the world views test (the cull's low byte and
/// 0x0400), but for [`HIDDEN_FLAG`], which a scenery group switches at run
/// time and so is tested per instance (see [`instance_matrix`]).
pub(in crate::render) const VIEW_FLAGS: u16 = (0x00FF & !HIDDEN_FLAG) | NOT_REFLECTED_FLAG;
/// Scenery instance flag 0x0010 (`world::HIDDEN_FLAG`): in every view's
/// cull flags (`WorldView::cull_flags`), so no view draws the instance.
pub(in crate::render) const HIDDEN_FLAG: u16 = crate::world::HIDDEN_FLAG;
/// Scenery instance flag 0x0100: no matrix (a null transform, 0x5D6752).
pub(in crate::render) const NO_MATRIX_FLAG: u16 = 0x0100;

#[cfg(test)]
mod lod_fade_tests {
    use crate::render::*;

    fn pass(fades: Option<Vec<f32>>) -> PassCull {
        PassCull { planes: Vec::new(), mirrored: false, eye: Vec3::ZERO, forward: Vec3::Y, scale: cull_scale(71.5f32.to_radians()), min_size: if fades.is_some() { 0.0 } else { 17.0 }, fades: fades.map(std::sync::Arc::new) }
    }

    /// With LOD Fade off the main view's instance test is the native one
    /// (`drops`); with it on, the CPU's fades decide: 0 dropped (geometry
    /// too), any other value (a negative one fading out) kept, behind the
    /// eye dropped.
    #[test]
    fn fades_decide_only_when_on() {
        let off = pass(None);
        for c in [[0.0, 30.0, 0.0, 1.0], [0.0, 400.0, 0.0, 1.0], [0.0, -30.0, 0.0, 1.0], [5.0, 5.0, 0.0, 0.0], [0.0, 10.0, 0.0, -1.0]] {
            assert_eq!(off.drops_slot(0, c), off.drops(c));
        }
        let on = pass(Some(vec![0.0, 0.5, -0.5, 1.0]));
        let far = [0.0, 400.0, 0.0, 1.0];
        assert!(on.drops_slot(0, far));
        assert!(!on.drops_slot(1, far) && !on.drops_slot(2, far) && !on.drops_slot(3, far));
        assert!(on.drops_slot(0, [5.0, 5.0, 0.0, 0.0]));
        assert!(on.drops_slot(3, [0.0, -30.0, 0.0, 1.0]));
        assert!(on.drops_slot(3, [0.0, 10.0, 0.0, -1.0]));
    }
}
