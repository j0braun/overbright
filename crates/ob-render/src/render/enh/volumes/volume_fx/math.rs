use super::*;

/// The grid's min corner for an eye at `eye` looking along `forward`:
/// centred [`AHEAD`] m in front (horizontally), from [`BELOW`] m under the
/// eye, snapped to whole voxels in world space.
pub fn grid_origin(budget: Budget, eye: Vec3, forward: Vec3) -> Vec3 {
    grid_origin_at(budget, eye, forward, [AHEAD, BELOW])
}

/// [`grid_origin`] with another anchor: `[ahead, below]` (m; the spray's
/// sits nearer the eye than smoke, [`VolumeFx::set_anchor`]).
pub fn grid_origin_at(budget: Budget, eye: Vec3, forward: Vec3, anchor: [f32; 2]) -> Vec3 {
    let h = Vec3::new(forward.x, forward.y, 0.0).normalize_or(Vec3::X);
    let extent = Vec3::new(budget.dims[0] as f32, budget.dims[1] as f32, 0.0) * budget.voxel;
    let centre = eye + h * anchor[0];
    let corner = Vec3::new(centre.x - extent.x * 0.5, centre.y - extent.y * 0.5, eye.z - anchor[1]);
    (corner / budget.voxel).floor() * budget.voxel
}

/// The march's divisor for a scene `height` px high: 2 up to 1440p, 3 at
/// 4K, 4 beyond (`NFSU2_VFX_DIV` 1..4 overrides).
pub fn march_div(height: u32) -> u32 {
    static DIV: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    if let Some(d) = *DIV.get_or_init(|| crate::render::debug_env::nfsu2_vfx_div()) {
        return d.clamp(1, 4);
    }
    ((height as f32 / 720.0).round() as u32).clamp(2, 4)
}

/// The march's step scale and jitter share as the uniform carries
/// them (look6.x, look6.y): zeros for the defaults (1, 1), so every other
/// user of the renderer marches exactly as before.
pub fn march_tuning(step_scale: f32, jitter: f32) -> [f32; 2] {
    [if step_scale < 1.0 { step_scale } else { 0.0 }, if jitter < 1.0 { 1.0 + jitter } else { 0.0 }]
}

/// Henyey–Greenstein phase (1/sr) of asymmetry `g` at cosine `c`.
pub fn hg(g: f32, c: f32) -> f32 {
    let g2 = g * g;
    (1.0 - g2) / (4.0 * std::f32::consts::PI * (1.0 + g2 - 2.0 * g * c).max(1e-4).powf(1.5))
}

/// Light through optical depth `tau`, scattered at cosine `c`: single
/// scattering plus two octaves of the multiple-scattering approximation
/// (Wrenninge 2013): each octave sees 0.35× the extinction, adds 0.55× and
/// scatters half as forward; 1 is an unshadowed isotropic single
/// scattering × 2.
pub fn multi_scatter(tau: f32, c: f32, g: f32) -> f32 {
    let (mut s, mut a, mut b, mut gk) = (0.0, 1.0, 1.0, g);
    for _ in 0..3 {
        s += b * (-a * tau).exp() * hg(gk, c);
        a *= 0.35;
        b *= 0.55;
        gk *= 0.5;
    }
    s * 2.0 * std::f32::consts::PI
}

/// The air a car moving at `cv` drags along at `r` (particle − car): the
/// weight and the air's velocity (volume_fx.wgsl `wake`; inferred).
pub fn wake(r: Vec3, cv: Vec3) -> (f32, Vec3) {
    let speed = cv.length();
    if speed < 0.5 {
        return ((-r.dot(r) / 3.0).exp(), 0.35 * cv);
    }
    let dir = cv / speed;
    let along = r.dot(dir);
    let lat = r - dir * along;
    let length = 2.5 + 0.35 * speed;
    let radius = 1.3 + 0.12 * (-along).max(0.0);
    let s = ((2.4 - along) / 2.4).clamp(0.0, 1.0);
    let behind = s * s * (3.0 - 2.0 * s) * (along.min(0.0) / length).exp();
    let w = behind * (-lat.dot(lat) / (radius * radius)).exp();
    let side = Vec3::new(lat.x, lat.y, 0.0);
    let out = side / side.length().max(0.3);
    let roll = (out * 0.06 + Vec3::new(0.0, 0.0, 0.07)) * speed.min(30.0);
    (w, cv * 0.35 + roll)
}

/// The composite's depth weight of a low-res tap at view depth `zl` for a
/// pixel at `zf` (volume_fx.wgsl `bilateral`).
pub fn bilateral(zl: f32, zf: f32) -> f32 {
    let r = (zl - zf).abs() / (0.02 * zf + 0.05);
    1.0 / (1.0 + r * r) + 1e-4
}
