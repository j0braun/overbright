use super::*;

/// The grid and the work it may do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    /// Voxels along x, y (horizontal) and z (up): multiples of 8 (8³
    /// bricks, a half-resolution light volume).
    pub dims: [u32; 3],
    /// The voxel's edge (m).
    pub voxel: f32,
    /// The particle buffer's capacity.
    pub particles: u32,
    /// The march's step limit a pixel.
    pub steps: u32,
}

/// 160 × 160 × 40 voxels of 0.4 m (64 × 64 × 16 m), 64 Ki particles.
pub const BUDGET_HIGH: Budget = Budget { dims: [160, 160, 40], voxel: 0.4, particles: 65536, steps: 160 };

/// How far ahead of the eye the grid's centre sits (m, along the
/// horizontal view direction), and how far below the eye it starts.
pub const AHEAD: f32 = 22.0;
pub const BELOW: f32 = 6.0;

/// The shading (inferred, by eye; no native reference exists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The sky / city ambient (in the scene's units, × albedo).
    pub ambient: Vec3,
    /// The lamps' Henyey–Greenstein asymmetry (forward scattering).
    pub g: f32,
    /// How hard thin parts are eroded by the detail noise (0..1).
    pub erosion: f32,
    /// The detail noise's frequency (1/m: it tiles every 1/this m).
    pub detail: f32,
    /// Thin (wet) particles' whiteness (their albedo floor).
    pub thin_white: f32,
    /// How dark a billow's underside gets (0..1).
    pub shade: f32,
    /// The eroded edge's contrast (1: soft; 2: a crisp billow outline).
    pub sharp: f32,
    /// The lamps', spot lights' and point lights' gains.
    pub lamp: f32,
    pub spot: f32,
    pub point: f32,
    /// The ambient taken from the scene drawn around the volume
    /// (its block average, `volume_fx_ambient.wgsl`) × this; 0 keeps the
    /// flat [`Look::ambient`] (also the fallback with an unreadable target).
    pub scene: f32,
    /// The medium fades in over this distance from the eye (m; no
    /// wall of smoke on the lens).
    pub near: f32,
    /// How much the medium darkens the surfaces under it (0..1; its
    /// optical depth straight up, the lamps' and the sky's light).
    pub shadow: f32,
    /// A third, finer erosion octave's weight (0 none): billow
    /// detail near the eye, faded out where the march's step is too long
    /// for it (no aliasing).
    pub fine: f32,
    /// How far the billow shading turns from straight up toward the
    /// light-volume voxel's dominant light (0 up only, as before; 1 fully):
    /// lit sides facing a lamp, shaded sides away from it.
    pub directional: f32,
    /// The temporal resolve's history weight (0 none, as before: the
    /// march's fixed pattern; e.g. 0.85: the march's start offset turns
    /// each frame and the reprojected, neighbourhood-clamped history
    /// integrates it — smoother edges, the fine octave without grain).
    pub temporal: f32,
    /// The march's step × this (1 as before; longer steps are only
    /// smooth with [`Look::temporal`] integrating their offsets).
    pub step: f32,
}

pub const LOOK: Look = Look {
    ambient: Vec3::splat(0.9),
    g: 0.45,
    erosion: 0.9,
    detail: 0.15,
    thin_white: 0.8,
    shade: 0.35,
    sharp: 2.0,
    lamp: 1.0,
    spot: 1.2,
    point: 1.0,
    scene: 2.6,
    near: 3.5,
    shadow: 0.3,
    fine: 0.0,
    directional: 0.0,
    temporal: 0.0,
    step: 1.0,
};

/// A particle the renderer splats (volume_fx.wgsl `Particle`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    /// World position, radius (m; 0: none).
    pub pos_r: [f32; 4],
    /// Albedo, peak extinction at the centre (1/m).
    pub albedo_sigma: [f32; 4],
    /// Emitted radiance per unit extinction (fire), lumpiness 0..1.
    pub emit_lump: [f32; 4],
    /// Noise offset (0..1 of the noise volume), spin about the vertical.
    pub noise: [f32; 4],
    /// Age fraction 0..1, thinness 0..1 (wet / steam), 0, 0.
    pub misc: [f32; 4],
}

/// The extinction at a ball's centre for an opacity `alpha` through it
/// (radius `r`): −ln(1 − α) over 0.9 r (the profile's mean chord).
pub fn sigma_for_opacity(alpha: f32, r: f32) -> f32 {
    -(1.0 - alpha.clamp(0.0, 0.985)).ln() / (0.9 * r.max(1e-3))
}

/// An emitted particle for the [`ParticlePool`] (volume_fx.wgsl `Emit`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Emit {
    /// Position, life (s).
    pub pos_life: [f32; 4],
    /// Velocity (m/s), start radius (m).
    pub vel_r: [f32; 4],
    /// Albedo, start extinction (1/m).
    pub albedo_sigma: [f32; 4],
    /// Temperature 0..1 (lift and glow; cools), growth (m/√s), lumpiness
    /// 0..1, thinness 0..1.
    pub heat: [f32; 4],
}

/// A street lamp: world position, outer radius (m), colour × strength.
pub type Lamp = (Vec3, f32, Vec3);

/// A spot light (headlights).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub position: Vec3,
    pub reach: f32,
    pub direction: Vec3,
    /// Cosines of the cone's outer and inner edges.
    pub cos_outer: f32,
    pub cos_inner: f32,
    pub colour: Vec3,
}

/// The lights that light the volume this frame (world space).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneLights {
    pub lamps: Vec<Lamp>,
    pub spots: Vec<Spot>,
    /// Point lights: position, radius (m), colour. A negative radius
    /// is a ground light (an underglow): its light is held under ~1 m above
    /// it (the car's body over it), within |radius|.
    pub points: Vec<Lamp>,
    /// Modern Lighting on (the lamps' physical falloff).
    pub modern: bool,
}

pub const MAX_LAMPS: usize = 64;
pub const MAX_SPOTS: usize = 8;
pub const MAX_POINTS: usize = 32;

/// The wind, turbulence and cars that move a [`ParticlePool`].
#[derive(Clone, Debug, PartialEq)]
pub struct Fields {
    pub wind: Vec3,
    /// The turbulence's strength (m/s² at full age).
    pub turbulence: f32,
    /// Drag (1/s), lift per unit temperature (m/s²), cooling (1/s), glow gain.
    pub drag: f32,
    pub buoyancy: f32,
    pub cooling: f32,
    pub glow: f32,
    /// Cars: position, velocity (their wakes).
    pub cars: Vec<(Vec3, Vec3)>,
}

impl Default for Fields {
    fn default() -> Self {
        Fields {
            wind: Vec3::new(0.55, 0.35, 0.0),
            turbulence: 2.2,
            drag: 0.6,
            buoyancy: 3.0,
            cooling: 1.5,
            glow: 1.0,
            cars: Vec::new(),
        }
    }
}

/// A frame's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: Vec3,
    pub forward: Vec3,
    /// The eye-relative view-projection (rotation and projection only).
    pub view_proj: Mat4,
    pub near: f32,
    pub far: f32,
    pub reversed: bool,
    pub time: f32,
    /// The main view's native fog (FogValue x, y, z; colour), applied
    /// to the medium as world.efx applies it (None: fog off).
    pub fog: Option<([f32; 3], [f32; 3])>,
}
