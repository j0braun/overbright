/// The froxel grid (screen tiles x, y; depth slices).
pub const DIMS: [u32; 3] = [160, 90, 64];
/// The slices' range (m, view depth; exponential).
pub const NEAR: f32 = 0.5;
pub const FAR: f32 = 320.0;
/// Lights, spots and shadow layers the pass takes at most.
pub const MAX_LIGHTS: usize = 96 + CAR_LAMPS;
/// The cars' tail / brake lamps the medium takes at most (the nearest, after the street lamps and signs).
pub const CAR_LAMPS: usize = 16;
pub const MAX_SPOTS: usize = 8;
/// The veil histogram's bins (atmosphere.wgsl `VEIL_BINS`).
pub const VEIL_BINS: usize = 256;
/// The local floor's screen tiles (atmosphere.wgsl `VEIL_TILES`).
pub const VEIL_TILES: [usize; 2] = [16, 9];
pub const MAX_LAYERS: usize = 32;
/// Street lamps / emitters taken (the nearest within the reach, not behind the eye).
pub const LAMPS: usize = 56;
pub const EMITTERS: usize = 40;
pub const LAMP_REACH: f32 = 260.0;
pub const EMITTER_REACH: f32 = 140.0;

/// A Volumetric Fog tier of the one medium (docs/notes/graphics/atmosphere.md "The tiers: one medium at every fog tier"):
/// every tier is the same froxel medium with the same lights, energy rule and veil floors; the cheaper
/// tiers take a coarser grid, a shorter range and fewer lights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tier {
    /// The froxel grid (screen tiles x, y; depth slices).
    pub dims: [u32; 3],
    /// The slices' far end (m, view depth).
    pub far: f32,
    /// The street lamps, signs, car tail / brake lamps and headlight cones taken at most (the nearest).
    pub lamps: usize,
    pub emitters: usize,
    pub car_lamps: usize,
    pub spots: usize,
    /// Light shafts: the lamps' Lamp Shadows layers sampled per cell.
    pub shafts: bool,
    /// The temporal blend with the last frame's cells and the jittered slice sample. Off, a frame's cells
    /// depend on its own view alone: the picture at a tick is the same at every display rate (the rate
    /// gate's Extreme frames, docs/developing/checks.md), at the price of the history's smoothing.
    pub history: bool,
}

/// Next-Gen (manual): the full grid, every light, the history.
pub const TIER_NEXT_GEN: Tier = Tier {
    dims: DIMS,
    far: FAR,
    lamps: LAMPS,
    emitters: EMITTERS,
    car_lamps: CAR_LAMPS,
    spots: MAX_SPOTS,
    shafts: true,
    history: true,
};
/// High (Ultra, Extreme).
pub const TIER_HIGH: Tier = Tier {
    dims: [96, 54, 48],
    far: 240.0,
    lamps: 32,
    emitters: 24,
    car_lamps: 8,
    spots: 4,
    shafts: true,
    history: false,
};
/// Low.
pub const TIER_LOW: Tier = Tier {
    dims: [64, 36, 32],
    far: 160.0,
    lamps: 12,
    emitters: 8,
    car_lamps: 4,
    spots: 2,
    shafts: false,
    history: false,
};

/// The tier for a Volumetric Fog row value (1 Low, 2 High, 3 Next-Gen; None off).
pub fn tier(fog: u8) -> Option<Tier> {
    match fog {
        0 => None,
        1 => Some(TIER_LOW),
        2 => Some(TIER_HIGH),
        _ => Some(TIER_NEXT_GEN),
    }
}

/// The look (inferred, by eye; `NFSU2_ATMO_LOOK=name=value,…` overrides for tuning).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Haze extinction at the ground (1/m), dry.
    pub haze: f32,
    /// Its height falloff (m), dry.
    pub height: f32,
    /// Full rain: extinction × (1 + this), height × (1 + rain_height).
    pub rain: f32,
    pub rain_height: f32,
    /// The ambient's share of the native fog colour.
    pub ambient: f32,
    /// The lit streets' light (× the lamps' mean colour) and its height (m).
    pub glow: f32,
    pub glow_height: f32,
    /// Noise amount 0..1.
    pub noise: f32,
    /// Henyey–Greenstein g.
    pub g: f32,
    /// History blend (the new cell's weight).
    pub blend: f32,
    /// Light gains: lamps, emitters, spots.
    pub lamp: f32,
    pub emitter: f32,
    /// The share of a sign's light radius (Modern Lighting's emitter reach, 8–24 m) its glow in the air
    /// takes.
    pub emitter_reach: f32,
    pub spot: f32,
    /// The cars' tail / brake lamps' gain (their halos in the one medium).
    pub car_lamp: f32,
    /// The summed light's soft knee (scene units).
    pub knee: f32,
    /// docs/notes/graphics/look-direction.md: the share of the haze's extinction applied to the scene behind it.
    /// The native per-vertex fog already fades the scene toward FogColor; veiling it again darkens and
    /// flattens the frame (double fog). The medium's light is still integrated with the full extinction.
    pub veil: f32,
    /// One volume, the energy bound: the quantile of the columns' light taken as the frame's veil
    /// floor and taken off every pixel (`cs_floor`): what the whole frame shares is the native fog's veil.
    pub floor: f32,
    /// The blacks stay black: the share of each screen tile's least column light taken off its pixels
    /// besides the frame's floor (`tile_floor`): light a whole stretch of the view shares is a veil.
    pub tile: f32,
}

/// docs/notes/graphics/look-direction.md, rules "no double fog" / "count the glow twice": the first look (haze 0.005, ambient 0.8, glow 0.18,
/// lamp 130, emitter 110, knee 5, the haze veiling the scene fully) lifted the look spots' mean luma by +14
/// (+63 at the casino's neon, +22 in the sky) and their blacks by +7.7: the native fog colour was put back
/// on top of world.efx's own fog (double fog) and the sign halos on top of the native glow sprites and
/// overbright (double glow). Now the ambient is a trace, the halos are budgeted, the scene keeps the native
/// fog's attenuation and the knee holds overlapping halos down.
/// At max settings the distant light read wrong: it looked like
/// a transparent black texture disappearing when coming close. The earlier veil 0.2 still darkened the scene
/// behind the haze with distance (the rain highway: the far road 0.89 of the Original, the same row at
/// Volumetric Fog off 0.97; the far lamps −10 %), while the medium's light that would pay it back comes
/// only from the lamps marched near the eye: a dark veil over the distance that lifts as one drives in.
/// The native fog alone attenuates the scene now (veil 0, "no double fog" to the letter); the medium only adds
/// its local light (+2.2 mean luma over the row off in that frame, within the brightness rule).
pub const LOOK: Look = Look {
    haze: 0.0025,
    height: 28.0,
    rain: 1.3,
    rain_height: 1.2,
    ambient: 0.05,
    glow: 0.03,
    glow_height: 14.0,
    noise: 0.7,
    g: 0.55,
    blend: 0.12,
    lamp: 40.0,
    emitter: 12.0,
    emitter_reach: 0.5,
    spot: 4.0,
    car_lamp: 40.0,
    knee: 1.5,
    veil: 0.0,
    floor: 0.1,
    tile: 1.0,
};

pub fn look() -> Look {
    static L: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    *L.get_or_init(|| look_from(&crate::render::debug_env::nfsu2_atmo_look()))
}

pub(super) fn look_from(spec: &str) -> Look {
    let mut l = LOOK;
    for kv in spec.split(',') {
        let Some((k, v)) = kv.split_once('=') else { continue };
        let Ok(v) = v.trim().parse::<f32>() else { continue };
        let f = match k.trim() {
            "haze" => &mut l.haze,
            "height" => &mut l.height,
            "rain" => &mut l.rain,
            "rain_height" => &mut l.rain_height,
            "ambient" => &mut l.ambient,
            "glow" => &mut l.glow,
            "glow_height" => &mut l.glow_height,
            "noise" => &mut l.noise,
            "g" => &mut l.g,
            "blend" => &mut l.blend,
            "lamp" => &mut l.lamp,
            "emitter" => &mut l.emitter,
            "emitter_reach" => &mut l.emitter_reach,
            "spot" => &mut l.spot,
            "car_lamp" => &mut l.car_lamp,
            "knee" => &mut l.knee,
            "veil" => &mut l.veil,
            "floor" => &mut l.floor,
            "tile" => &mut l.tile,
            _ => continue,
        };
        *f = v;
    }
    l
}

/// The haze for a rain amount (0..1): extinction at the ground (1/m) and its height falloff (m).
/// The shared medium: the Next-Gen rain haze and the car lights' cones read it.
pub fn medium(rain: f32) -> (f32, f32) {
    let l = look();
    let r = rain.clamp(0.0, 1.0);
    (l.haze * (1.0 + l.rain * r), l.height * (1.0 + l.rain_height * r))
}
