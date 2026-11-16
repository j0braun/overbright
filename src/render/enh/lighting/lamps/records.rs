use super::*;
pub const TILES_X: u32 = 16;
pub const TILES_Y: u32 = 9;
pub const SLICES: u32 = 24;
pub const CLUSTERS: usize = (TILES_X * TILES_Y * SLICES) as usize;
/// The clusters' depth range (view-space metres).
pub const NEAR: f32 = 1.0;
pub const FAR: f32 = 600.0;
/// Lights uploaded per frame at most (the nearest kept).
pub const MAX_LIGHTS: usize = 1024;
/// Light indices over all clusters at most.
/// (65536 cut the far clusters' lists to nothing in the neon
/// districts with Modern Lighting's emitters: 116 612 wanted at the casino
/// canopy, 1583 of the 3456 clusters lost lights, and the cut moved with
/// the view and the traffic: dark tiles that flickered.)
pub const MAX_INDICES: usize = 262144;
/// Car occluders (contact shadows, the cars' lamp shadows) at most.
pub const MAX_OCCLUDERS: usize = 32;
/// Lamp lights farther than this from the eye are not uploaded (the fog
/// has them long before).
pub(in crate::render) const REACH: f32 = 450.0;
/// A light this wide is a sky / "moon" light (A21's 9999.99 m), which the
/// world's vertex colours already hold: not a lamp.
pub(in crate::render) const MAX_LAMP_RADIUS: f32 = 200.0;

/// The strength of the added light against the vertex colour (the light
/// packs' colours are dim, (110, 110, 125) for most of Bayview; the
/// native car harmonic takes them × 0.75).
pub const STRENGTH: f32 = 0.45;
/// So that dynamic lamps do not make the lamp spots too bright: the
/// baked light level (vertex-colour units, 0.5 = the texture at 1×) from
/// which Dynamic Lamps add nothing to the world, and the most the summed
/// lamp term may be (world.wgsl `LAMP_PEAK`, `LAMP_MAX`).
/// docs/look-direction.md, energy rule "the bake is the anchor": the bake already holds every
/// lamp's light, so the per-pixel term only shapes it. 0.35 let a dark gap
/// gain up to x1.7 of its bake (+2.6 mean luma over the look spots, the
/// darkness between the lamps lifted); 0.12 caps the gain at x1.24.
pub const LAMP_PEAK: f32 = 0.5;
pub const LAMP_MAX: f32 = 0.12;

/// world.wgsl's light term with Dynamic Lamps for a baked level `base`
/// (luminance, vertex-colour units) and the summed lamp term `lamp`:
/// `base · (1 + 2 · min(lamp, LAMP_MAX) · headroom)`, headroom = 1 −
/// base / LAMP_PEAK (≥ 0). The baked pools (base ≥ LAMP_PEAK) are left as
/// they are; nothing ends brighter than max(base, LAMP_PEAK).
pub fn lamp_term(base: f32, lamp: f32) -> f32 {
    let headroom = (1.0 - base / LAMP_PEAK).clamp(0.0, 1.0);
    base * (1.0 + 2.0 * lamp.min(LAMP_MAX) * headroom)
}

/// How much of a lamp's added light a shadow takes away at most.
pub const SHADOW_DEPTH: f32 = 0.7;
/// How much of the headlight projector's light a shadow takes away at most.
pub const HEADLIGHT_SHADOW_DEPTH: f32 = 0.7;
/// How much light a car takes away at most from a lamp or the headlight
/// behind it (world.wgsl `SHADOW_DEPTH_CARS`).
pub const CAR_SHADOW_DEPTH: f32 = 0.6;
/// How dark the contact shadow gets right under a car (fraction removed).
pub const CONTACT_DEPTH: f32 = 0.5;

/// Lamp Shadows: the map size and the number of lamp layers (the last
/// layer is the headlight's).
pub fn shadow_layout(quality: u8) -> Option<(u32, u32)> {
    match quality {
        0 => None,
        1 => Some((512, 4)),
        2 => Some((1024, 12)),
        // Modern Lighting (`modern_light_bold::layers`).
        _ => Some((1024, 24)),
    }
}

/// One light as world.wgsl's `Lamp` reads it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLamp {
    /// xyz position, w outer radius.
    pub position: [f32; 4],
    /// rgb colour × strength (0..1 scale), w inner radius.
    pub colour: [f32; 4],
    /// xyz spot direction, w the cone's cosine threshold (−2: a point light).
    pub direction: [f32; 4],
    /// x: shadow layer (−1 none); yzw unused.
    pub shadow: [f32; 4],
}

/// A shadow layer's view (world.wgsl `ShadowView`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuShadowView {
    pub view_proj: [[f32; 4]; 4],
    /// xyz the eye, w the depth bias (metres along the normal).
    pub eye: [f32; 4],
}

/// A car as an oriented box (world.wgsl `Occluder`): the axes carry the
/// half extents in w.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuOccluder {
    pub centre: [f32; 4],
    pub x: [f32; 4],
    pub y: [f32; 4],
    pub z: [f32; 4],
}

/// A car for the contact and lamp shadows: its matrix (car → world) and
/// its box in car space (`Car::shadow_box`).
#[derive(Clone, Copy, Debug)]
pub struct Occluder {
    pub transform: Mat4,
    pub min: Vec3,
    pub max: Vec3,
    /// docs/glow-darkening.md: the light the car's underglow throws
    /// on its footprint, 0..1 ([`crate::shadow::neon_light`]); the contact
    /// shadow keeps 1 − glow of its depth, so it never dims the glow.
    pub glow: f32,
}

impl Occluder {
    pub fn gpu(&self) -> GpuOccluder {
        let c = (self.min + self.max) * 0.5;
        let h = ((self.max - self.min) * 0.5).abs().max(Vec3::splat(0.05));
        let centre = self.transform.transform_point3(c);
        let axis = |v: Vec3, half: f32| {
            let a = self.transform.transform_vector3(v);
            let len = a.length().max(1e-6);
            (a / len).extend(half * len).to_array()
        };
        GpuOccluder {
            centre: centre.extend(contact_share(self.glow)).to_array(),
            x: axis(Vec3::X, h.x),
            y: axis(Vec3::Y, h.y),
            z: axis(Vec3::Z, h.z),
        }
    }
}

/// How much of its depth a car's contact shadow keeps under an underglow
/// of light `glow` (centre.w of [`GpuOccluder`]; world.wgsl
/// `contact_visibility`): 1 without one, 0 under a full one.
pub fn contact_share(glow: f32) -> f32 {
    1.0 - glow.clamp(0.0, 1.0)
}

/// A light-pack lamp the renderer keeps: its section and record.
#[derive(Clone, Debug)]
pub struct Lamp {
    pub section: u32,
    pub light: WorldLight,
}

/// The light packs' lamps: on, point or spot, not a sky light.
pub fn lamps_of(packs: &[crate::formats::lights::LightPack]) -> Vec<Lamp> {
    packs
        .iter()
        .flat_map(|p| p.lights.iter().map(move |l| (p.section, l)))
        .filter(|(_, l)| {
            l.on && matches!(l.kind, 2 | 3)
                && l.outer > 0.0
                && l.outer < MAX_LAMP_RADIUS
                && l.colour.iter().any(|&c| c > 0)
        })
        .map(|(section, l)| Lamp { section, light: l.clone() })
        .collect()
}

/// The native weight of a light at distance² `d2` (0x4893A0): 1 inside
/// the inner radius, falling linearly in d² to 0 at the outer one.
pub fn weight(d2: f32, inner: f32, outer: f32) -> f32 {
    let (i2, o2) = (inner * inner, outer * outer);
    if d2 >= o2 {
        0.0
    } else if d2 <= i2 || o2 <= i2 {
        1.0
    } else {
        1.0 - (d2 - i2) / (o2 - i2)
    }
}

/// The spot fade (0x48ED60): over the cone, cos(cone / 2) to 1; −2 for a
/// point light (no fade).
pub fn cone_threshold(l: &WorldLight) -> f32 {
    if l.kind == 3 {
        // sin(cone / 2 + π/2) = cos(cone / 2), the binary-angle sine as
        // the car lighting uses it.
        (l.cone * 0.5).cos()
    } else {
        -2.0
    }
}

pub fn gpu_lamp(l: &WorldLight, shadow: i32) -> GpuLamp {
    let c = Vec3::new(l.colour[0] as f32, l.colour[1] as f32, l.colour[2] as f32) / 255.0
        * (STRENGTH * if l.intensity > 0.0 { l.intensity.min(4.0) } else { 1.0 });
    let d = l.direction.normalize_or(Vec3::NEG_Z);
    GpuLamp {
        position: l.position.extend(l.outer).to_array(),
        colour: c.extend(l.inner.max(0.0)).to_array(),
        direction: d.extend(cone_threshold(l)).to_array(),
        shadow: [shadow as f32, 0.0, 0.0, 0.0],
    }
}

/// The (up to) four lamps that light a car at `at` most, for car.wgsl
/// (`Draw.lamps`): position + outer, colour + inner per lamp.
pub fn car_lamps(lamps: &[&WorldLight], at: Vec3) -> [[f32; 4]; 8] {
    let mut best: Vec<(f32, &WorldLight)> = lamps
        .iter()
        .filter(|l| l.on && matches!(l.kind, 2 | 3) && l.outer < MAX_LAMP_RADIUS)
        .map(|l| (weight((l.position - at).length_squared(), l.inner, l.outer), *l))
        .filter(|(w, _)| *w > 0.0)
        .collect();
    best.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = [[0.0; 4]; 8];
    for (i, (_, l)) in best.iter().take(4).enumerate() {
        let g = gpu_lamp(l, -1);
        out[i * 2] = g.position;
        out[i * 2 + 1] = g.colour;
    }
    out
}
