use super::*;

/// A frame's view and lights.
#[derive(Clone)]
pub struct View {
    /// The camera the scene was drawn with (jitter included).
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    /// The native size bound's scale (`render::cull_scale`).
    pub pixels_per_metre: f32,
    /// Street lamps (position, outer radius, colour × strength;
    /// `volumetric::nearest_lamps`).
    pub lamps: Vec<(Vec3, f32, Vec3)>,
    /// The cars' matrices (x forward, z up) and headlight colours.
    pub cars: Vec<(Mat4, [u8; 3])>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TickUniform {
    pub(super) dt: [f32; 4],
    pub(super) counts: [u32; 4],
    pub(super) tier: [f32; 4],
    pub(super) wind: [f32; 4],
    pub(super) ncars: [u32; 4],
    pub(super) cars: [[f32; 4]; 16],
}

pub(super) const TICK_STRIDE: u64 = 512;
pub(super) const MAX_LAMPS: usize = 16;
pub(super) const MAX_SPOTS: usize = 4;
pub(super) const MAX_POINTS: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FrameUniform {
    pub(super) view_proj: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) right: [f32; 4],
    pub(super) up: [f32; 4],
    pub(super) forward: [f32; 4],
    pub(super) counts: [u32; 4],
    pub(super) params: [f32; 4],
    pub(super) lamps: [[f32; 4]; 2 * MAX_LAMPS],
    pub(super) spots: [[f32; 4]; 3 * MAX_SPOTS],
    pub(super) points: [[f32; 4]; 2 * MAX_POINTS],
    /// The depth texel under a fragment = clip.xy · xy + zw.
    pub(super) depth_map: [f32; 4],
}

/// A light wind (m/s; inferred, the game has none): older smoke drifts
/// with it.
pub const WIND: Vec3 = Vec3::new(0.55, 0.35, 0.0);
/// Turbulence (m/s² at full age).
pub const TURBULENCE: f32 = 1.1;
/// Tail lights: colour, radius (m), and their place on the car (inferred:
/// the cars have no tail light markers; 0.6 m either side, 0.85 m up, at
/// the rear of the body, 2.1 m behind the centre).
pub const TAIL_COLOUR: Vec3 = Vec3::new(0.35, 0.025, 0.012);
pub const TAIL_RADIUS: f32 = 2.5;
pub(super) const TAIL_AT: [Vec3; 2] = [Vec3::new(-2.1, 0.6, 0.85), Vec3::new(-2.1, -0.6, 0.85)];
/// Headlight cone: half angles (cos) and reach (m) (inferred, by eye).
pub(super) const HEAD_COS: [f32; 2] = [0.82, 0.94];
pub(super) const HEAD_RANGE: f32 = 28.0;
