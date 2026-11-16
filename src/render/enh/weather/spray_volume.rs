//! Wet Road Interaction's volumetric spray (clone deviation, opt-in;
//! docs/wet-road-car.md "Volumetric spray"). Nothing here exists in
//! SPEED2.EXE. The spray pool (`weather_fx::Spray`, fed by
//! `wet_road::emit`) decides where the water goes; its volume puffs —
//! rooster tails off the tyres, the mist cloud behind a fast car, splash
//! sheets through puddles, wheel-spin and drift clouds — and its mist are
//! drawn through the shared volumetric renderer ([`crate::render::volume_fx`]):
//! splatted into a world-aligned grid around the eye, lit by the street
//! lamps (Modern Lighting's falloff when it is on), the cars' headlight
//! cones, tail and brake lights and the player's underglow with forward
//! scattering and self-shadowing, marched soft against the scene's depth
//! and composited before the post chain (exposure, grade, HDR output), so
//! it sits in the same layer as the smoke.
//!
//! Every constant is **inferred**, by eye (no native reference exists).

use glam::{Mat4, Vec3};

use super::spray::CarSample;
use super::wet_road::CarExtra;
use crate::render::Gpu;
use crate::render::volume_fx::{self, Budget, Lamp, Look, Particle, SceneLights, Spot, VolumeFx};

/// The still air's drift (m/s; `volume_fx::Fields::default`'s wind).
pub const WIND: Vec3 = Vec3::new(0.55, 0.35, 0.0);
/// A volume puff's drag toward the air (1/s: fine spray is carried by the
/// air within a fraction of a second) and its fall (m/s², fading with age
/// as the big drops leave it).
pub const CLOUD_DRAG: f32 = 2.2;
pub const CLOUD_FALL: f32 = 3.0;

/// The grids: On 128 × 128 × 16 voxels of 0.3 m (38 × 38 × 4.8 m), High
/// 160 × 160 × 24 of 0.25 m (40 × 40 × 6 m): from 3 m under the eye
/// (about 1 m under the road) to ~4 m over the road, as high as spray
/// is thrown.
pub const BUDGETS: [Budget; 2] = [
    Budget { dims: [128, 128, 16], voxel: 0.3, particles: 16384, steps: 160 },
    Budget { dims: [160, 160, 24], voxel: 0.25, particles: 24576, steps: 170 },
];
/// The grid's anchor: its centre this far ahead of the eye (the chase
/// camera's car is ~6 m ahead, the cars it follows 10–30 m), from this far
/// under the eye (the road ~1.5–2.5 m under it).
pub const ANCHOR: [f32; 2] = [14.0, 3.0];

/// Water spray's shading: droplets scatter strongly forward (g 0.6: a
/// headlight or a tail light seen through the spray blooms), white
/// (albedo ~0.9, thin), soft billows, a dim city ambient (the lights do
/// the rest).
pub const LOOK: Look = Look {
    scene: 1.1,
    near: 0.3,
    shadow: 0.0,
    ambient: Vec3::splat(0.08),
    g: 0.6,
    erosion: 0.75,
    detail: 0.5,
    thin_white: 0.92,
    shade: 0.35,
    sharp: 1.5,
    lamp: 1.0,
    spot: 0.7,
    point: 1.0,
    fine: 0.0,
    directional: 0.0,
    temporal: 0.0,
    step: 1.0,
};

/// [`LOOK`], with `NFSU2_SPRAY_LOOK=k=v,…` overriding fields by name
/// (tuning aid, as `NFSU2_SMOKE3_LOOK`).
pub fn look() -> Look {
    static L: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    *L.get_or_init(|| {
        let mut l = LOOK;
        for kv in crate::render::debug_env::nfsu2_spray_look().split(',') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            match k.trim() {
                "scene" => l.scene = v,
                "near" => l.near = v,
                "shadow" => l.shadow = v,
                "ambient" => l.ambient = Vec3::splat(v),
                "g" => l.g = v,
                "erosion" => l.erosion = v,
                "detail" => l.detail = v,
                "thin_white" => l.thin_white = v,
                "shade" => l.shade = v,
                "sharp" => l.sharp = v,
                "lamp" => l.lamp = v,
                "spot" => l.spot = v,
                "point" => l.point = v,
                _ => {}
            }
        }
        l
    })
}

/// A volume puff's albedo (water) and lumpiness (mist / cloud).
const ALBEDO: [f32; 3] = [0.86, 0.88, 0.9];

/// The tail lights' reach (m) in the spray and their gain against the
/// mirrored colour (`wet_road::push_car`'s, brake ×2.6 brighter).
pub const TAIL_REACH: f32 = 2.6;
/// A headlight cone's reach in the spray (m; once 30: the cone lit
/// the spray at full strength far down the road, a white wall).
pub const SPOT_REACH: f32 = 24.0;
const TAIL_GAIN: f32 = 0.55;
/// Against a glow shell around the car: the underglow lights the spray near the road
/// only (once 3.2 m at ×4 from 0.15 m: it lit the whole plume around the
/// body, a glowing shell in the underglow's colour). From the sills'
/// height down at the road, reaching the low spray beside and behind the
/// car, dim (the road takes the underglow; the spray only catches it).
pub const NEON_REACH: f32 = 1.6;
pub const NEON_GAIN: f32 = 1.2;
pub const NEON_HEIGHT: f32 = 0.05;

/// The march's step length × this (its puffs are 0.1–1.2 m: at the
/// default 0.2–0.8 m steps the start jitter's pattern stayed in them).
pub const STEP_SCALE: f32 = 0.5;
/// The share of the march's start jitter kept (its lattice showed at
/// 4K without TAA; the short steps need little).
pub const JITTER: f32 = 0.25;

/// The largest puff radius splatted (m).
pub const MAX_RADIUS: f32 = 1.2;

/// A spray puff of radius `r` (m), peak opacity `alpha`, at age fraction
/// `t`, as the volume's particle (none when spent). Clouds come in fast
/// (they are thrown off the tyre) and thin out; mist swells in slowly.
pub fn puff(pos: Vec3, r: f32, alpha: f32, t: f32, seed: f32, cloud: bool) -> Option<Particle> {
    let t = t.clamp(0.0, 1.0);
    // The splat's cost grows with (r / voxel)³: a big mist ball is capped
    // (its opacity kept), the grid's noise does the rest.
    let r = r.min(MAX_RADIUS);
    // A cloud thins as it spreads (its water over a larger ball): dense
    // right behind the tyre, a veil once it has grown.
    let fade = if cloud {
        (t / 0.025).min(1.0) * (1.0 - t).powf(1.4) * (0.5 / r.max(0.5))
    } else {
        0.3 * (t / 0.2).min(1.0) * (1.0 - t)
    };
    let a = alpha * fade;
    if a < 0.004 || r <= 0.0 || !pos.is_finite() {
        return None;
    }
    let fr = |x: f32| x - x.floor();
    Some(Particle {
        pos_r: pos.extend(r).to_array(),
        albedo_sigma: [ALBEDO[0], ALBEDO[1], ALBEDO[2], volume_fx::sigma_for_opacity(a, r)],
        emit_lump: [0.0, 0.0, 0.0, if cloud { 0.55 } else { 0.3 }],
        noise: [fr(seed * 7.13), fr(seed * 13.7), fr(seed * 3.31), (seed - 0.5) * 2.0],
        misc: [t, 1.0, 0.0, 0.0],
    })
}

/// A puff `d` m from the eye (its surface): faded in from 1.5 m to 6 m, so
/// the spray flying past the chase camera does not veil its own car, and a
/// ball a few voxels across never fills the view with its facets.
pub fn near_fade(d: f32) -> f32 {
    let t = ((d - 1.5) / 4.5).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

static NEON: std::sync::Mutex<Option<(Mat4, Vec3)>> = std::sync::Mutex::new(None);

/// The player's underglow this frame (car matrix, colour × alpha), from the
/// game (as the smoke's `neons`); none without one.
pub fn set_neon(neon: Option<(Mat4, Vec3)>) {
    *NEON.lock().unwrap() = neon;
}

/// The underglow [`set_neon`] gave.
pub fn neon_now() -> Option<(Mat4, Vec3)> {
    *NEON.lock().unwrap()
}

/// The volume's camera for the weather pass's view (eye-relative
/// view-projection, as the smoke's).
/// With the main view's native fog (FogValue, FogColor), applied
/// inside the medium as world.efx applies it, so the spray far
/// down the road sinks into the same fog as the road under it.
pub fn view_of(view: &super::rain_fx::View, time: f32, fog: Option<([f32; 3], [f32; 3])>) -> volume_fx::View {
    let cam = &view.camera;
    let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
    let view_proj = cam.projection_depth(view.aspect, view.reversed) * rot;
    volume_fx::View {
        fog,
        eye: cam.position,
        forward: cam.forward(),
        view_proj,
        near: cam.near,
        far: cam.far,
        reversed: view.reversed,
        time,
    }
}

/// The spray's lights: the street lamps; per drawn car within 80 m its
/// headlights as one cone (their mean position and direction, the car's
/// headlight colour) and its tail / brake lights as points (brighter while
/// braking: `wet_road::push_car`'s colours); the underglow.
pub fn scene_lights(
    lamps: &[Lamp],
    cars: &[CarSample],
    extras: &[CarExtra],
    eye: Vec3,
    modern: bool,
    neon: Option<(Mat4, Vec3)>,
) -> SceneLights {
    let mut l = SceneLights { lamps: lamps.to_vec(), modern, ..Default::default() };
    for car in cars {
        let at = car.world.w_axis.truncate();
        if at.distance(eye) > 80.0 {
            continue;
        }
        let Some(e) = super::wet_road::match_extra(car, extras) else { continue };
        let heads: Vec<_> = e.lights.iter().filter(|x| x.kind == 0).collect();
        if !heads.is_empty() {
            let n = heads.len() as f32;
            let p = heads.iter().map(|h| h.position).sum::<Vec3>() / n;
            let d = heads.iter().map(|h| h.direction).sum::<Vec3>() / n;
            let c = heads.iter().map(|h| h.colour).sum::<Vec3>() / n;
            let dir = car.world.transform_vector3(d).normalize_or(car.world.x_axis.truncate().normalize_or(Vec3::X));
            l.spots.push(Spot {
                position: car.world.transform_point3(p),
                reach: SPOT_REACH,
                direction: dir,
                cos_outer: 0.84,
                cos_inner: 0.95,
                colour: c,
            });
        }
        for t in e.lights.iter().filter(|x| x.kind == 1) {
            l.points.push((car.world.transform_point3(t.position), TAIL_REACH, t.colour * TAIL_GAIN));
        }
    }
    if let Some((m, c)) = neon.filter(|(m, _)| m.w_axis.truncate().distance(eye) < 60.0) {
        for x in [-1.1f32, 1.1] {
            l.points.push((m.transform_point3(Vec3::new(x, 0.0, NEON_HEIGHT)), NEON_REACH, c * NEON_GAIN));
        }
    }
    l
}

/// How much spray the eye sits in (0..1): the volume puffs' opacity within
/// 2.5 m of it (the lens catches the spray of the car ahead).
pub fn hit(particles: &[Particle], eye: Vec3) -> f32 {
    let mut s = 0.0;
    for p in particles {
        hit_add(&mut s, p, eye);
    }
    hit_total(s)
}

/// How far past its radius a puff reaches the eye in [`hit`] (m).
pub const HIT_REACH: f32 = 2.5;

/// One puff's share of [`hit`]'s sum (none out of its reach).
pub fn hit_add(s: &mut f32, p: &Particle, eye: Vec3) {
    let c = Vec3::new(p.pos_r[0], p.pos_r[1], p.pos_r[2]);
    let r = p.pos_r[3] + HIT_REACH;
    let d2 = c.distance_squared(eye);
    if d2 < r * r {
        let opacity = 1.0 - (-p.albedo_sigma[3] * 0.9 * p.pos_r[3]).exp();
        *s += opacity * (1.0 - d2.sqrt() / r);
    }
}

/// [`hit`]'s value from its sum.
pub fn hit_total(s: f32) -> f32 {
    (s * 0.5).min(1.0)
}

/// The spray's renderer (made when the row is first on with spray to draw).
/// The grid is built in the frame's encoder right before the march (the
/// draw), as the smoke's: one submit, no idle gap in the GPU timings.
pub struct SprayVolume {
    fx: std::sync::Mutex<VolumeFx>,
    tier: u8,
    count: u32,
}

impl SprayVolume {
    pub fn new(gpu: &Gpu, tier: u8) -> SprayVolume {
        let mut fx = VolumeFx::new(gpu, BUDGETS[usize::from(tier.clamp(1, 2) - 1)], look());
        fx.set_anchor(ANCHOR[0], ANCHOR[1]);
        fx.set_step_scale(crate::render::debug_env::nfsu2_spray_step().unwrap_or(STEP_SCALE));
        fx.set_jitter(crate::render::debug_env::nfsu2_spray_jitter().unwrap_or(JITTER));
        SprayVolume { fx: std::sync::Mutex::new(fx), tier, count: 0 }
    }

    pub fn tier(&self) -> u8 {
        self.tier
    }

    /// This frame's particles, camera and lights (the build runs in
    /// [`SprayVolume::draw`]).
    pub fn prepare(&mut self, gpu: &Gpu, view: volume_fx::View, lights: &SceneLights, particles: &[Particle]) {
        let fx = self.fx.get_mut().unwrap();
        let n = particles.len().min(fx.budget().particles as usize);
        self.count = n as u32;
        if n == 0 {
            return;
        }
        fx.write_particles(gpu, 0, &particles[..n]);
        fx.frame(gpu, view, lights);
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// Build the grid, march and composite over `target` (its format and
    /// samples read off the views), soft against `depth`.
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
    ) {
        if self.count == 0 {
            return;
        }
        let mut fx = self.fx.lock().unwrap();
        fx.build(gpu, encoder, self.count);
        let t = target.texture();
        let (format, samples, depth_samples) = (t.format(), t.sample_count(), depth.texture().sample_count());
        fx.warm(gpu, format, samples, depth_samples);
        fx.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Puffs come in and go out with age; spent or broken puffs are none.
    #[test]
    fn puffs_fade_and_skip() {
        let p = |t: f32, cloud: bool| puff(Vec3::ZERO, 0.5, 0.4, t, 0.3, cloud).map_or(0.0, |p| p.albedo_sigma[3]);
        assert!(p(0.3, true) > p(0.01, true) && p(0.3, true) > p(0.9, true));
        assert!(p(0.25, false) > p(0.02, false));
        assert_eq!(p(1.0, true), 0.0);
        assert!(puff(Vec3::splat(f32::NAN), 0.5, 0.4, 0.3, 0.3, true).is_none());
        assert!(puff(Vec3::ZERO, 0.0, 0.4, 0.3, 0.3, true).is_none());
        let q = puff(Vec3::ZERO, 0.5, 0.4, 0.3, 0.3, true).unwrap();
        assert_eq!(q.misc[1], 1.0, "wet: thin");
    }

    /// The lights: one headlight cone ahead and the tail lights behind per
    /// near car, brighter tails while braking, the underglow under the car;
    /// far cars none.
    #[test]
    fn spray_lights_follow_the_cars() {
        use super::super::wet_road::CarLight;
        let car = |x: f32| {
            let world = Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
            CarSample { world, contacts: [Vec3::ZERO; 4], velocity: Vec3::X * 20.0 }
        };
        let lights = |brake: f32| {
            vec![
                CarLight { position: Vec3::new(2.0, 0.7, 0.6), direction: Vec3::X, colour: Vec3::ONE, kind: 0 },
                CarLight { position: Vec3::new(2.0, -0.7, 0.6), direction: Vec3::X, colour: Vec3::ONE, kind: 0 },
                CarLight {
                    position: Vec3::new(-2.1, 0.6, 0.8),
                    direction: -Vec3::X,
                    colour: Vec3::new(1.0, 0.06, 0.03) * brake,
                    kind: 1,
                },
            ]
        };
        let cars = [car(10.0), car(500.0)];
        let extras = [
            CarExtra { at: Vec3::new(10.0, 0.0, 0.0), lights: lights(1.3), wheels: [(0.0, 0.0, true); 4] },
            CarExtra { at: Vec3::new(500.0, 0.0, 0.0), lights: lights(0.5), wheels: [(0.0, 0.0, true); 4] },
        ];
        let neon = Some((Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)), Vec3::new(1.0, 0.0, 1.0)));
        let l = scene_lights(&[(Vec3::Z * 6.0, 12.0, Vec3::ONE)], &cars, &extras, Vec3::ZERO, true, neon);
        assert_eq!(l.spots.len(), 1);
        assert!(l.spots[0].position.x > 11.5 && l.spots[0].direction.x > 0.99);
        assert_eq!(l.points.len(), 3, "one tail + two underglow");
        assert!(l.points[0].0.x < 10.0 && l.points[0].2.x > 0.5);
        assert!(l.points[1..].iter().all(|p| p.0.z < 0.5));
        assert!(l.modern && l.lamps.len() == 1);
    }

    /// docs/look-direction.md, "the bake is the anchor": the spray's light is anchored to
    /// the scene: its ambient is the scene's around it, not a flat
    /// grey (the earlier 0.32 lifted every dark street to a milky veil), the
    /// fallback for an unreadable target is dim, the headlight cones are
    /// held below the lamps' gain and reach no further than 24 m, and the
    /// march takes short, barely jittered steps (no lattice at 4K).
    #[test]
    fn look_is_anchored_to_the_scene() {
        if !crate::render::debug_env::nfsu2_spray_look_set() {
            assert_eq!(look(), LOOK);
        }
        assert!(LOOK.scene > 0.5 && LOOK.scene <= 1.5);
        assert!(LOOK.ambient.max_element() <= 0.1);
        assert!(LOOK.spot < LOOK.lamp && LOOK.spot <= 1.0);
        assert!(SPOT_REACH <= 24.0 && MAX_RADIUS <= 1.2);
        assert!(STEP_SCALE < 1.0 && JITTER < 0.5);
        let car = CarSample { world: Mat4::IDENTITY, contacts: [Vec3::ZERO; 4], velocity: Vec3::X * 20.0 };
        let extra = CarExtra {
            at: Vec3::ZERO,
            lights: vec![super::super::wet_road::CarLight {
                position: Vec3::new(2.0, 0.7, 0.6),
                direction: Vec3::X,
                colour: Vec3::ONE,
                kind: 0,
            }],
            wheels: [(0.0, 0.0, true); 4],
        };
        let l = scene_lights(&[], &[car], &[extra], Vec3::ZERO, false, None);
        assert_eq!(l.spots[0].reach, SPOT_REACH);
        assert!(l.spots[0].cos_outer >= 0.84);
    }

    /// The underglow lights only the spray near the road: its lights sit
    /// at the road under the sills and reach no higher than the sills' box
    /// margin (no glow up the flanks), dimmer than the tail lights' gain
    /// It was once at (×4 from 0.15 m, 3.2 m: the whole plume glowed).
    #[test]
    fn underglow_lights_only_the_low_spray() {
        let neon = (Mat4::IDENTITY, Vec3::new(1.0, 1.0, 0.0));
        let l = scene_lights(&[], &[], &[], Vec3::ZERO, false, Some(neon));
        assert_eq!(l.points.len(), 2);
        for (p, reach, c) in &l.points {
            assert!(p.z <= 0.06 && p.z + reach <= 1.7, "{p:?} {reach}");
            assert!(c.max_element() <= 1.5);
        }
        // A yellow neon stays yellow (neon_rgb's B, G, R bytes).
        assert_eq!(l.points[0].2.z, 0.0);
    }

    /// The eye inside the spray of the car ahead is hit; far away it is not.
    #[test]
    fn eye_in_the_spray() {
        let ps: Vec<Particle> =
            (0..20).filter_map(|i| puff(Vec3::new(i as f32 * 0.2, 0.0, 1.0), 0.8, 0.4, 0.3, 0.5, true)).collect();
        assert!(hit(&ps, Vec3::new(1.0, 0.5, 1.2)) > 0.3);
        assert_eq!(hit(&ps, Vec3::new(40.0, 0.0, 1.0)), 0.0);
    }
}
