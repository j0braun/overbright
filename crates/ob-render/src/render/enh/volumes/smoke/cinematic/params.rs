use super::*;

/// The Particles row's value for this tier (`qol::particles`).
pub const TIER: u8 = 3;

/// Whether the row's value selects the Cinematic path.
pub fn active(tier: u8) -> bool {
    tier >= TIER
}

/// The Cinematic tier's constants (inferred, by eye against the clone's
/// Original and Ultra frames; no native reference exists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Billows per native smoke birth (plus one haze puff).
    pub billows: u32,
    /// A billow's size, life and opacity against the native particle's.
    pub size: f32,
    pub linger: f32,
    pub alpha: f32,
    /// Size growth with √age (s^-½).
    pub growth: f32,
    /// The haze puff: size, life, opacity (against the native particle's)
    /// and growth.
    pub haze_size: f32,
    pub haze_life: f32,
    pub haze_alpha: f32,
    pub haze_growth: f32,
    /// The GPU pool (puffs alive at once; the oldest reused).
    pub pool: u32,
    /// Street lamps that light the smoke.
    pub lamps: usize,
    /// Turbulence (m/s² of the noise flow at full age).
    pub turbulence: f32,
}

pub const CINEMATIC: Params = Params {
    billows: 3,
    size: 0.8,
    linger: 1.9,
    alpha: 1.5,
    growth: 0.22,
    haze_size: 1.3,
    haze_life: 3.2,
    haze_alpha: 0.18,
    haze_growth: 0.45,
    pool: 32768,
    lamps: 16,
    turbulence: 2.2,
};

/// What the shared [`Feeder`] needs (its `linger` bounds how long the
/// last puff lives: the haze's life).
pub fn feed_params() -> TierParams {
    let p = CINEMATIC;
    TierParams {
        children: p.billows + 1,
        size: p.size,
        linger: p.linger.max(p.haze_life),
        alpha: p.alpha,
        pool: p.pool,
        lamps: p.lamps,
        detail: 1.0,
    }
}

/// The reduced resolution's divisor for a scene `height` px high: 2 up to
/// 1440p, 3 at 4K (a buffer about 720 px high), 4 beyond.
/// `NFSU2_SMOKE2_DIV` overrides (1..4; tests and the cost table).
pub fn lowres_div(height: u32) -> u32 {
    static DIV: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    if let Some(d) = *DIV.get_or_init(|| crate::render::debug_env::nfsu2_smoke2_div()) {
        return d.clamp(1, 4);
    }
    ((height as f32 / 720.0).round() as u32).clamp(2, 4)
}

/// The reduced buffer's size for a `size` scene at divisor `div`.
pub fn lowres_size(size: (u32, u32), div: u32) -> (u32, u32) {
    (size.0.div_ceil(div).max(1), size.1.div_ceil(div).max(1))
}

/// The composite's depth weight of a low-res tap at view depth `zl` for a
/// pixel at `zf` (smoke_cinematic.wgsl `bilateral`).
pub fn bilateral(zl: f32, zf: f32) -> f32 {
    let r = (zl - zf).abs() / (0.02 * zf + 0.05);
    1.0 / (1.0 + r * r) + 1e-4
}

/// The air a car moving at `cv` drags along at `r` (puff − car): the
/// weight and the air's velocity (smoke_cinematic.wgsl `wake`; inferred).
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

/// The street lamps' falloff at squared distance `d2` from a lamp of
/// outer radius `r`: the light pack's (1 − d²/r²)², or with Modern
/// Lighting its physical pool (`modern_light::pool_soft` without the
/// cosine, capped). Mirrors smoke_cinematic.wgsl `lamp_falloff`.
pub fn lamp_falloff(d2: f32, r: f32, modern: bool) -> f32 {
    if modern {
        let y = d2 / (r * r);
        let window = (1.0 - y * y).max(0.0);
        let k = crate::render::modern_light::POOL_REF * crate::render::modern_light::POOL_REF
            + crate::render::modern_light::POOL_SOFT;
        return (k / (d2 + crate::render::modern_light::POOL_SOFT)).min(1.6) * window * window;
    }
    let x = (1.0 - d2 / (r * r)).clamp(0.0, 1.0);
    x * x
}
