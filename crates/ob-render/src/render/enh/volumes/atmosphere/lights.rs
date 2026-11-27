use super::*;

/// A light the medium scatters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: Vec3,
    pub radius: f32,
    /// rgb × strength (the lamps' `gpu_lamp` colour).
    pub colour: Vec3,
    /// The lobe's axis (zero: all around).
    pub axis: Vec3,
    pub kind: LightKind,
    /// Its Lamp Shadows layer (-1 none).
    pub layer: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    /// A street lamp's head: lights downward.
    Lamp,
    /// A sign / neon: the half-space in front of `axis`.
    Emitter,
    /// A car's tail / brake lamp, `axis` its facing (the car lights' lamps in the one medium).
    Point,
}

/// A projector cone (the headlight's view 9 parameters, `headlight_uniform`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub eye: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub dir: Vec3,
    /// x / z and y / z scales of the cone.
    pub across: f32,
    pub down: f32,
    pub reach: f32,
    pub colour: Vec3,
    pub layer: i32,
}

/// A volume_fx density volume (r: extinction 1/m), world-aligned.
#[derive(Clone)]
pub struct Smoke {
    pub density: wgpu::TextureView,
    pub origin: Vec3,
    pub voxel: f32,
    pub dims: [u32; 3],
}

/// The medium's inputs beside `volumetric::Frame`'s (every Volumetric Fog tier).
#[derive(Clone)]
pub struct NextGen {
    /// The Volumetric Fog tier: the grid, range and light counts ([`tier`]).
    pub tier: Tier,
    pub fog_color: Vec3,
    pub rain: f32,
    pub time: f32,
    pub lights: Vec<Light>,
    pub spots: Vec<Spot>,
    /// The spots' gain × this ([`NX6_SPOT_SCALE`] for the Next-Gen car lights' cones).
    pub spot_scale: f32,
    pub smoke: Option<Smoke>,
    /// The Lamp Shadows maps and their layers' views.
    pub shadows: Option<(wgpu::TextureView, Vec<crate::render::lamps::GpuShadowView>)>,
}

/// The lights for an eye: the nearest lamps (with their shadow layers) and emitters ahead, as many as
/// `tier` takes within its range.
pub fn pick_lights(
    lamps: &[crate::render::lamps::Lamp],
    emitters: &[crate::render::lamps::Lamp],
    layers: Option<&crate::render::lamps::ShadowLayers>,
    eye: Vec3,
    forward: Vec3,
    tier: &Tier,
) -> Vec<Light> {
    let mut layer_of = std::collections::HashMap::new();
    if let Some(s) = layers {
        for (layer, h) in s.holders.iter().enumerate() {
            if let Some(i) = h {
                layer_of.insert(*i, layer as i32);
            }
        }
    }
    let nearest = |list: &[crate::render::lamps::Lamp], reach: f32, n: usize| {
        let mut v: Vec<(f32, usize)> = list
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let d = l.light.position - eye;
                let dist = d.length();
                (dist < reach + l.light.outer && d.dot(forward) > -l.light.outer).then_some((dist, i))
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.truncate(n);
        v
    };
    let mut out = Vec::new();
    for (_, i) in nearest(lamps, LAMP_REACH.min(tier.far), tier.lamps.min(LAMPS)) {
        let g = crate::render::lamps::gpu_lamp(&lamps[i].light, -1);
        out.push(Light {
            position: lamps[i].light.position,
            radius: g.position[3],
            colour: Vec3::from_slice(&g.colour[..3]),
            axis: Vec3::NEG_Z,
            kind: LightKind::Lamp,
            layer: layer_of.get(&i).copied().unwrap_or(-1),
        });
    }
    for (_, i) in nearest(emitters, EMITTER_REACH.min(tier.far), tier.emitters.min(EMITTERS)) {
        let l = &emitters[i].light;
        let g = crate::render::lamps::gpu_lamp(l, -1);
        // A sign lights the half-space in front of it (`emitter_light` keeps the facing when most of its
        // area faces one way); the emitters are kind 2 lights, so the facing is read whatever the kind
        // (a kind 3 test before left every sign glowing all round, its glow over the dark facade).
        let axis = if l.direction.length_squared() > 0.25 { l.direction.normalize() } else { Vec3::ZERO };
        out.push(Light {
            position: l.position,
            radius: g.position[3] * look().emitter_reach,
            colour: Vec3::from_slice(&g.colour[..3]),
            axis,
            kind: LightKind::Emitter,
            layer: -1,
        });
    }
    out.truncate(MAX_LIGHTS);
    out
}

/// The player's headlight as a spot (High's beam, kept in Next-Gen until the Next-Gen car lights replace it).
pub fn headlight_spot(h: &[[f32; 4]; 5], layer: Option<usize>) -> Option<Spot> {
    if h[0][3] < 0.5 || h[1][3] <= 0.0 || h[2][3] <= 0.0 {
        return None;
    }
    Some(Spot {
        eye: Vec3::new(h[0][0], h[0][1], h[0][2]),
        right: Vec3::new(h[1][0], h[1][1], h[1][2]),
        up: Vec3::new(h[2][0], h[2][1], h[2][2]),
        dir: Vec3::new(h[3][0], h[3][1], h[3][2]),
        across: h[1][3],
        down: h[2][3],
        reach: 45.0,
        colour: Vec3::new(h[4][0], h[4][1], h[4][2]),
        layer: layer.map_or(-1, |l| l as i32),
    })
}

/// One volume: with the Next-Gen car lights on, their cones are this medium's spots, whole; the car lights'
/// own march does not run while this pass does (`PostFx::pre_passes`), so each beam lights the air once.
/// (Before, the march drew the cones and the medium took them again at 0.3, without the march's near
/// cut-off, aureole or rain rule: the headlights in rain were the medium's light more than the march's.)
pub const NX6_SPOT_SCALE: f32 = 1.0;

/// The Next-Gen car lights' tail and brake lamps (`car_lights::medium_lamps`, world space) as the medium's
/// car lamps; at most `n` (the tier's, never over [`CAR_LAMPS`]), the nearest first, appended after the
/// street lamps and signs.
pub fn car_lamps(lamps: &[(Vec3, Vec3, Vec3)], n: usize) -> Vec<Light> {
    lamps
        .iter()
        .take(n.min(CAR_LAMPS))
        .map(|&(position, colour, facing)| Light {
            position,
            radius: crate::render::car_lights::HALO,
            colour,
            axis: facing,
            kind: LightKind::Point,
            layer: -1,
        })
        .collect()
}

/// The Next-Gen car lights' cones (`car_lights::air_lights`: camera-relative lamp midpoints, view 9's axes and scales, colour ×
/// fade) as spots; the nearest (the list is sorted by distance; the player's first in a chase view) takes the
/// headlight's shadow layer.
pub fn spots_from_car_lights(
    cones: &[crate::render::car_lights::GpuSpot],
    eye: Vec3,
    headlight_layer: Option<usize>,
    n: usize,
) -> Vec<Spot> {
    cones
        .iter()
        .take(n.min(MAX_SPOTS))
        .enumerate()
        .map(|(i, c)| Spot {
            eye: Vec3::new(c.position[0], c.position[1], c.position[2]) + eye,
            right: Vec3::new(c.right[0], c.right[1], c.right[2]),
            up: Vec3::new(c.up[0], c.up[1], c.up[2]),
            dir: Vec3::new(c.dir[0], c.dir[1], c.dir[2]),
            across: c.right[3],
            down: c.up[3],
            reach: 45.0,
            colour: Vec3::new(c.colour[0], c.colour[1], c.colour[2]),
            layer: if i == 0 { headlight_layer.map_or(-1, |l| l as i32) } else { -1 },
        })
        .collect()
}

/// The view depth at slice coordinate `s` (0..1).
pub fn slice_depth(s: f32) -> f32 {
    NEAR * (s * (FAR / NEAR).ln()).exp()
}

/// The slice coordinate (0..1) of a view depth.
pub fn depth_slice(z: f32) -> f32 {
    (z.max(NEAR) / NEAR).ln() / (FAR / NEAR).ln()
}

/// The frame's z jitter (0..1): a golden-ratio sequence (low discrepancy, never repeating a pattern).
pub fn jitter(frame: u32) -> f32 {
    ((frame as f64 * 0.618_033_988_749_895).fract()) as f32
}
