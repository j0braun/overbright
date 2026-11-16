//! Wet Car (clone deviation, opt-in; docs/weather.md): the cars get wet
//! with the rain. Nothing here is native: car.efx has no wetness or rain
//! input (docs/car-raindrops.md), so the original draws a dry and a wet car
//! the same. The weather it follows is native: the rain's amount (weather
//! +0x1F0), and the water film follows +0x509C's rule (0x61F2BD: up by
//! 0.05 of the gap a step, down by 0.0015) from 0 at load, aimed at the
//! rain's amount rather than 1 (drizzle leaves a thinner film; **clone
//! choice**), and none while the player's car is under a cover zone
//! (+0x1E4, 0x61E710).
//!
//! The CPU half: the scene's weather as the in-game frame publishes it
//! ([`publish`]), each car's state ([`WetState`]: its velocity from its
//! drawn poses, its spray film from driving on a wet road) and the per-draw
//! values (`Draw.wet` in car.wgsl, read by car_wet.wgsl). A car whose film
//! and spray are both at or below [`WET_MIN`] is drawn with the native /
//! Enhanced Paint pipelines: a dry car is the same image bit for bit.

use glam::{Mat4, Vec3};

/// The scene's weather for the cars this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene {
    /// Weather +0x1F0.
    pub rain: f32,
    /// The road's wetness from the rain (`Weather::road_wet`).
    pub road_wet: f32,
    /// The cars' water film (`Weather::car_wet`).
    pub car_wet: f32,
    /// The player's car under a cover zone (+0x1E4).
    pub covered: bool,
    /// The world clock 0x7FB718 (s).
    pub clock: f32,
}

static SCENE: std::sync::Mutex<Option<Scene>> = std::sync::Mutex::new(None);

/// The in-game frame's weather (None: no game view; the cars stay dry).
pub fn publish(scene: Option<Scene>) {
    *SCENE.lock().unwrap() = scene;
}

/// The in-game frame's weather from the native weather object, when a
/// weather row is on (else None: nothing of this module runs; `on`: any
/// weather row on, as the drawn state was stepped with).
pub fn publish_weather(w: &crate::weather::Weather, clock: f32, on: bool) {
    publish(on.then_some(Scene {
        rain: w.amount,
        road_wet: w.road_wet,
        car_wet: w.car_wet,
        covered: w.under_cover,
        clock,
    }));
}

pub fn scene() -> Option<Scene> {
    *SCENE.lock().unwrap()
}

impl Scene {
    /// The one wetness (`materials::world::wetness`): what every wet effect reads.
    pub fn wetness(&self) -> crate::render::enh::materials::world::wetness::Wetness {
        crate::render::enh::materials::world::wetness::Wetness::of(self.rain, self.road_wet)
    }
}

/// Below this the film counts as dry (the native pipelines draw).
pub const WET_MIN: f32 = 0.002;

/// The film's step (one 1/60 s step): +0x509C's rates toward `target`.
pub fn film_step(film: f32, target: f32) -> f32 {
    let rate = if target > film { 0.05 } else { 0.0015 };
    let f = film + (target - film) * rate;
    if target == 0.0 && f < 1e-4 { 0.0 } else { f }
}

/// The spray film a car picks up driving on a wet road: aimed at the road's
/// wetness, scaled by speed (none below 4 m/s, full at 20 m/s), rising more
/// slowly than the rain wets it, drying at the film's rate. **Inferred**
/// (no native counterpart), by eye.
pub fn spray_step(spray: f32, road_wet: f32, speed: f32) -> f32 {
    let t = ((speed - 4.0) / 16.0).clamp(0.0, 1.0);
    let target = road_wet * t * t * (3.0 - 2.0 * t);
    let rate = if target > spray { 0.02 } else { 0.0015 };
    let s = spray + (target - spray) * rate;
    if target == 0.0 && s < 1e-4 { 0.0 } else { s }
}

/// What a car's draw is, for the wet layer (by its light material).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WetClass {
    /// Lights, decals, the interior, the radiator, brake discs: unchanged.
    None = 0,
    /// Body paint (and carbon): darker, a water film, beads, rivulets.
    Paint = 1,
    /// Windows and light covers: film, beads and rivulets.
    Glass = 2,
    /// Tyres: darker rubber with a wet sheen.
    Tyre = 3,
    /// Plastics, mouldings, grilles, chrome and rims: darker, a thinner film.
    Trim = 4,
}

impl WetClass {
    pub fn of(material: &str) -> WetClass {
        let m = material.to_ascii_uppercase();
        if crate::render::car::PaintType::of(&m) != crate::render::car::PaintType::Native {
            // Gloss / metallic / pearl / candy paints, and chrome rims
            // (Chrome is the trim class below).
            return if crate::render::car::PaintType::of(&m) == crate::render::car::PaintType::Chrome {
                WetClass::Trim
            } else {
                WetClass::Paint
            };
        }
        match m.as_str() {
            "CARBONFIBER" | "CARBONFIBRE" => WetClass::Paint,
            "WINDSHIELD" | "WINDOW" | "WINDOWMASK" | "HEADLIGHTGLASS" | "BRAKELIGHTGLASS" | "CLEARPLASTIC" => {
                WetClass::Glass
            }
            "RUBBER" | "TIRE" | "TYRE" => WetClass::Tyre,
            "DULLPLASTIC" | "MATTEPLASTIC" | "MOLDINGS" | "GRILL" | "BOTTOM" | "ALUMINUM" | "DEFAULT" => WetClass::Trim,
            _ if m.starts_with("MAG") || m.starts_with("USER_RIMS") || m.starts_with("RIM") => WetClass::Trim,
            _ => WetClass::None,
        }
    }
}

/// A car's wet values this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetParams {
    pub film: f32,
    pub rain: f32,
    pub spray: f32,
    /// The car's velocity in its own axes (x forward, z up).
    pub velocity: Vec3,
    pub clock: f32,
}

impl WetParams {
    /// car.wgsl's `Draw.wet` for a draw of `class` (zeros: unchanged);
    /// `next_gen`: Rain Detail at Next-Gen.
    pub fn uniform(&self, class: WetClass, hood_x: f32, sill_z: f32, next_gen: bool) -> [[f32; 4]; 3] {
        if class == WetClass::None {
            return [[0.0; 4]; 3];
        }
        [
            [self.film, self.rain, class as u8 as f32, self.clock],
            [self.velocity.x, self.velocity.y, self.velocity.z, self.spray],
            // Rain Detail Next-Gen's rain impacts and glass sheeting
            // (car_wet.wgsl `rain_on_car`; 0 below Next-Gen: unchanged).
            [hood_x, sill_z, if next_gen { 1.0 } else { 0.0 }, 0.0],
        ]
    }
}

/// The drawn frame's place between the last two ticks (α, 1 on
/// the newest), published by the in-game draw before the cars are queued
/// (`publish_alpha`): the cars' wet values are blended by it
/// ([`WetParams::blend`]). The harness's frames and a steady 60 Hz draw
/// α = 1 (the tick's own values).
static FRAME_ALPHA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F80_0000);

pub fn publish_alpha(alpha: f32) {
    FRAME_ALPHA.store(alpha.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn frame_alpha() -> f32 {
    f32::from_bits(FRAME_ALPHA.load(std::sync::atomic::Ordering::Relaxed))
}

impl WetParams {
    /// The "blend" class: a car's wet values `alpha` of the way from
    /// tick S_{n−1}'s (`prev`) to S_n's (`cur`): exactly `cur` at α ≥ 1,
    /// exactly `prev` at α = 0; a car wet only in S_n is drawn dry until
    /// the frame reaches it, one wet only in S_{n−1} keeps its values (the
    /// snapshot rule for things that appear or go between two ticks).
    pub fn blend(prev: Option<WetParams>, cur: Option<WetParams>, alpha: f32) -> Option<WetParams> {
        if alpha >= 1.0 {
            return cur;
        }
        match (prev, cur) {
            (Some(a), Some(b)) => {
                let l = |x: f32, y: f32| x + (y - x) * alpha;
                Some(WetParams {
                    film: l(a.film, b.film),
                    rain: l(a.rain, b.rain),
                    spray: l(a.spray, b.spray),
                    velocity: a.velocity.lerp(b.velocity, alpha),
                    clock: l(a.clock, b.clock),
                })
            }
            (a, _) => a,
        }
    }
}

/// Wet Road Interaction without Wet Car: only the spray film from
/// the road (the lower body and the tyres), none of the rain's film; None
/// when that is dry.
pub fn spray_only(p: WetParams) -> Option<WetParams> {
    (p.spray > WET_MIN).then_some(WetParams { film: 0.0, rain: 0.0, ..p })
}

/// One car's state (in its renderer).
#[derive(Clone, Debug, Default)]
pub struct WetState {
    /// The clock and position when the clock last moved.
    last: Option<(f32, Vec3)>,
    pub velocity: Vec3,
    pub spray: f32,
}

impl WetState {
    /// Step with the scene's weather and the car's drawn matrix. None: the
    /// car is dry (or the option is off): the native pipelines draw it.
    pub fn step(&mut self, scene: Option<&Scene>, world: Mat4) -> Option<WetParams> {
        let Some(s) = scene else {
            self.last = None;
            self.spray = 0.0;
            return None;
        };
        let at = world.w_axis.truncate();
        match self.last {
            Some((clock, pos)) if s.clock > clock && s.clock - clock < 0.5 => {
                let dt = s.clock - clock;
                let v = (at - pos) / dt;
                // Teleports (a reset, a new race) are not motion.
                if v.length() < 120.0 {
                    self.velocity = self.velocity.lerp(v, 0.35);
                }
                let steps = (dt * 60.0).round().max(1.0) as u32;
                for _ in 0..steps.min(30) {
                    self.spray = spray_step(self.spray, s.wetness().film, self.velocity.length());
                }
                self.last = Some((s.clock, at));
            }
            Some((clock, _)) if s.clock == clock => {}
            _ => {
                self.last = Some((s.clock, at));
                self.velocity = Vec3::ZERO;
            }
        }
        let film = s.car_wet;
        if film <= WET_MIN && self.spray <= WET_MIN {
            return None;
        }
        let rot = Mat4::from_cols(world.x_axis, world.y_axis, world.z_axis, glam::Vec4::W);
        let local = rot.transpose().transform_vector3(self.velocity);
        Some(WetParams {
            film,
            rain: if s.covered { 0.0 } else { s.rain },
            spray: self.spray,
            velocity: local,
            clock: s.clock,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wet Road Interaction without Wet Car: the spray film alone, none of the rain's.
    #[test]
    fn spray_only_keeps_the_road_water() {
        let p = WetParams { film: 0.6, rain: 0.8, spray: 0.3, velocity: Vec3::X, clock: 1.0 };
        let s = spray_only(p).unwrap();
        assert_eq!((s.film, s.rain, s.spray), (0.0, 0.0, 0.3));
        assert!(spray_only(WetParams { spray: 0.0, ..p }).is_none());
    }

    fn scene(rain: f32, road_wet: f32, car_wet: f32, clock: f32) -> Scene {
        Scene { rain, road_wet, car_wet, covered: false, clock }
    }

    /// The blend's ends are the ticks' own values, bit for bit;
    /// a car wet only in S_n is dry before the frame reaches it.
    #[test]
    fn blend_ends_are_the_ticks() {
        let a = WetParams { film: 0.2, rain: 0.5, spray: 0.1, velocity: Vec3::X, clock: 10.0 };
        let b = WetParams { film: 0.3, rain: 0.5, spray: 0.4, velocity: Vec3::Y * 3.0, clock: 10.0 + 1.0 / 60.0 };
        assert_eq!(WetParams::blend(Some(a), Some(b), 1.0), Some(b));
        assert_eq!(WetParams::blend(Some(a), Some(b), 0.0), Some(a));
        let m = WetParams::blend(Some(a), Some(b), 0.5).unwrap();
        assert!(m.film > a.film && m.film < b.film && m.spray > a.spray && m.spray < b.spray);
        assert_eq!(WetParams::blend(None, Some(b), 0.5), None);
        assert_eq!(WetParams::blend(None, Some(b), 1.0), Some(b));
        assert_eq!(WetParams::blend(Some(a), None, 0.5), Some(a));
    }

    /// The R2 lesson: a dry car (no film, no spray) is never wet, whatever
    /// the option: no scene, or a dry scene, gives None (native pipelines).
    #[test]
    fn dry_car_stays_native() {
        let mut w = WetState::default();
        assert_eq!(w.step(None, Mat4::IDENTITY), None);
        for i in 0..600 {
            let m = Mat4::from_translation(Vec3::X * i as f32 * 0.5);
            assert_eq!(w.step(Some(&scene(0.0, 0.0, 0.0, 1.0 + i as f32 / 60.0)), m), None);
        }
        // A dry road at speed adds no spray.
        assert_eq!(w.spray, 0.0);
        assert!(w.velocity.x > 25.0, "{:?}", w.velocity);
    }

    /// The film follows +0x509C's rule toward the rain's amount: wet within
    /// a second, drying with the native 0.0015 a step (half in ~7.7 s).
    #[test]
    fn film_follows_the_native_rates() {
        let mut f = 0.0;
        for _ in 0..60 {
            f = film_step(f, 0.8);
        }
        assert!(f > 0.75 && f <= 0.8, "{f}");
        let wet = f;
        for _ in 0..462 {
            f = film_step(f, 0.0);
        }
        assert!((f / wet - 0.5).abs() < 0.01, "{f}");
        // Drizzle leaves a thinner film than heavy rain.
        let (mut a, mut b) = (0.0, 0.0);
        for _ in 0..300 {
            a = film_step(a, 0.2);
            b = film_step(b, 0.95);
        }
        assert!(a < 0.25 && b > 0.9);
    }

    /// Spray: only on a wet road and at speed; dries afterwards.
    #[test]
    fn spray_needs_a_wet_road_and_speed() {
        assert_eq!(spray_step(0.0, 1.0, 3.0), 0.0);
        assert_eq!(spray_step(0.0, 0.0, 30.0), 0.0);
        let mut s = 0.0;
        for _ in 0..300 {
            s = spray_step(s, 1.0, 25.0);
        }
        assert!(s > 0.95);
        for _ in 0..300 {
            s = spray_step(s, 1.0, 0.0);
        }
        assert!(s < 0.7 && s > 0.5, "{s}");
    }

    /// The car's velocity is taken in its own axes (x forward).
    #[test]
    fn velocity_in_car_axes() {
        let mut w = WetState::default();
        let turn = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let mut p = None;
        for i in 0..120 {
            let m = Mat4::from_translation(Vec3::Y * i as f32 * 0.25) * turn;
            p = w.step(Some(&scene(1.0, 1.0, 0.5, 1.0 + i as f32 / 60.0)), m);
        }
        let p = p.unwrap();
        assert!((p.velocity.x - 15.0).abs() < 0.1 && p.velocity.y.abs() < 0.1, "{:?}", p.velocity);
        assert!(p.spray > 0.0 && p.film == 0.5);
    }

    #[test]
    fn classes_by_light_material() {
        for (m, c) in [
            ("REGPAINTRED", WetClass::Paint),
            ("METPAINTSILVER", WetClass::Paint),
            ("PEARL3", WetClass::Paint),
            ("CUSTOMPAINT_7", WetClass::Paint),
            ("CARBONFIBER", WetClass::Paint),
            ("TRAFFIC", WetClass::Paint),
            ("WINDSHIELD", WetClass::Glass),
            ("HEADLIGHTGLASS", WetClass::Glass),
            ("RUBBER", WetClass::Tyre),
            ("CHROME", WetClass::Trim),
            ("MAGSILVER", WetClass::Paint),
            ("DULLPLASTIC", WetClass::Trim),
            ("DECAL", WetClass::None),
            ("BRAKELIGHT", WetClass::None),
            ("INTERIOR", WetClass::None),
            ("PLAINNOTHING", WetClass::None),
        ] {
            assert_eq!(WetClass::of(m), c, "{m}");
        }
        let p = WetParams { film: 0.5, rain: 1.0, spray: 0.0, velocity: Vec3::ZERO, clock: 3.0 };
        assert_eq!(p.uniform(WetClass::None, 1.0, 0.5, false), [[0.0; 4]; 3]);
        assert_eq!(p.uniform(WetClass::Glass, 1.0, 0.5, false)[0], [0.5, 1.0, 2.0, 3.0]);
    }
}
