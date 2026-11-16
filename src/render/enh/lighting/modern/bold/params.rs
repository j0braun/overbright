use super::*;
/// A synthetic light's marker in `WorldLight::sub` (the packs use 0..=3).
pub const SUB_EMITTER: u8 = 0xE1;
pub const SUB_TAIL: u8 = 0xE2;
pub const SUB_HEAD: u8 = 0xE3;

/// The tier in force (the row's value, the hidden switch of phase 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Off,
    Low,
    Modern,
    Rt,
}

/// The Graphics page's row (0 Original, 1 Low, 2 Modern, 3 Modern + RT;
/// 3 only while the device traces). Phase 1's hidden switch
/// `NFSU2_MODERN_LIGHTING` is Modern at strength 0 when the row is off.
pub fn tier(opts: &crate::render::RenderOptions) -> Tier {
    match opts.modern_lighting {
        1 => Tier::Low,
        2 => Tier::Modern,
        3 => Tier::Rt,
        _ if crate::render::modern_light::switch_district().is_some() => Tier::Modern,
        _ => Tier::Off,
    }
}

/// The strength 0..1 (the row's %, `NFSU2_MODERN_STRENGTH` for the hidden
/// switch: 0 by default, phase 1's look).
pub fn strength(opts: &crate::render::RenderOptions) -> f32 {
    if opts.modern_lighting == 0 {
        static S: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
        return *S.get_or_init(|| {
            crate::render::debug_env::nfsu2_modern_strength().map_or(0.0, |v| (v / 100.0).clamp(0.0, 1.0))
        });
    }
    f32::from(opts.modern_strength.min(100)) / 100.0
}

/// The frame's Modern Lighting values (world.wgsl `MlParams`, the record
/// at `lamp_list[ML_PARAMS]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The strength 0..1.
    pub strength: f32,
    /// The bake's residual (what the lamps do not explain) × this.
    pub ambient: f32,
    /// The lamps' relit pools × this.
    pub boost: f32,
    /// The relit pools' softening (m²): phase 1's fitted 150 → sharper.
    pub soft: f32,
    /// How far the relit pools leave the bake's per-channel cap (0 phase
    /// 1's cap, 1 none).
    pub uncap: f32,
    /// Neon / signs' and the cars' lights' gain.
    pub emit: f32,
    /// The specular's gain.
    pub spec: f32,
    /// The cars' harmonic (the bake's light on them) × this.
    pub car_ambient: f32,
    /// Lights with a shadow test per pixel at most (Low: fewer).
    pub shadowed: u32,
    /// Ray queries per pixel (Modern + RT; 0 otherwise).
    pub rays: u32,
    /// Lights evaluated per pixel at most (Low: the nearest few).
    pub lights: u32,
    /// 1 Low, 2 Modern, 3 Modern + RT.
    pub tier: u32,
}

/// Bold's end values (strength 1).
pub const AMBIENT_BOLD: f32 = 0.5;
pub const BOOST_BOLD: f32 = 1.6;
pub const SOFT_BOLD: f32 = 40.0;
pub const UNCAP_BOLD: f32 = 0.7;
pub const EMIT_BOLD: f32 = 1.0;
pub const SPEC_BOLD: f32 = 1.6;
pub const CAR_AMBIENT_BOLD: f32 = 0.75;

impl Params {
    pub fn new(tier: Tier, s: f32) -> Params {
        let s = s.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * s;
        Params {
            strength: s,
            ambient: lerp(1.0, AMBIENT_BOLD),
            boost: lerp(1.0, BOOST_BOLD),
            soft: lerp(crate::render::modern_light::POOL_SOFT, SOFT_BOLD),
            uncap: lerp(0.0, UNCAP_BOLD),
            emit: lerp(0.0, EMIT_BOLD),
            spec: lerp(1.0, SPEC_BOLD),
            car_ambient: lerp(1.0, CAR_AMBIENT_BOLD),
            shadowed: if tier == Tier::Low { 2 } else { 12 },
            rays: if tier == Tier::Rt { 4 } else { 0 },
            lights: if tier == Tier::Low { 4 } else { 1024 },
            tier: match tier {
                Tier::Off => 0,
                Tier::Low => 1,
                Tier::Modern => 2,
                Tier::Rt => 3,
            },
        }
    }

    /// The record world.wgsl reads (`ml_params`).
    pub fn gpu(&self) -> GpuLamp {
        GpuLamp {
            position: [self.strength, self.ambient, self.boost, self.soft],
            colour: [self.tier as f32, self.emit, self.spec, self.uncap],
            direction: [self.shadowed as f32, self.rays as f32, self.car_ambient, self.lights as f32],
            shadow: [0.0; 4],
        }
    }
}

/// The params record's index in the lamps' buffer: after the lamps and
/// the beams (world.wgsl `ML_PARAMS`).
pub const PARAMS_INDEX: usize = crate::render::lamps::MAX_LIGHTS + crate::render::lamps::MAX_BEAMS;

/// The lamp shadow layers (map size, lamp layers) per tier: Low 4 at 512,
/// Modern / + RT 24 at 1024 (RT traces the strongest; the layers are its
/// fallback for the rest). Lamp Shadows' own rows: [`super::lamps::shadow_layout`].
pub fn layers(tier: Tier) -> Option<(u32, u32)> {
    match tier {
        Tier::Off => None,
        Tier::Low => Some((512, 4)),
        Tier::Modern | Tier::Rt => Some((1024, 24)),
    }
}

/// The reach of the lamps given a layer (m; Lamp Shadows' is 70).
pub const SHADOW_REACH: f32 = 110.0;
