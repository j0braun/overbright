//! Rain: the per-view weather object (`*(view + 0x64)`, constructor
//! 0x61E160, set up at track load by 0x61E100 → 0x613060) that
//! SPEED2.EXE steps once a frame from the frame function (0x5CB47B →
//! 0x61F130). Traced in the executable:
//!
//! * **Whether it rains** (0x61E390, read at 0x61F32F): the "Rain"
//!   parameter map (object 0x8A1F28, name 0x7A9410; data pointer 0x8A1F34,
//!   filled by the map loader) sampled at the camera (0x5F0C60: camera
//!   +0x40 / +0x44) gives `[amount, threshold]`. 0x6134F0 computes
//!   `s = |0.5·cos((clock + x / 50)°) + 0.5|` from the world clock
//!   (0x7FB718 = sim ticks / 4000, 1.0 s at track load) and the camera's
//!   x; the target (+0x5094) is the amount when `threshold ≥ s`, else 0.
//!   In Bayview the thresholds are 0.05–0.25, so a given spot rains for
//!   part of every 6-minute cycle (0x8A1D48 / 0x89E7E6, the dyno run, force
//!   s to 0 / 1). With 0x5B98B0 ≥ 3 (split screen) the target is ×⅛.
//! * **Amount** (+0x1F0): `rain += (target − rain)·dt`, zeroed below 0.01
//!   when the target is 0, capped at 1. 0x6139C0 then splits the 400 drops
//!   (+0x4D00) into `active = 400 − ⌊(1 − rain)·400⌋` falling and the rest
//!   hidden (type 5).
//! * **Wetness** (+0x509C): steps toward 1 while it rains (+0x5098), at
//!   0.05 a frame, back to 0 at 0.0015 a frame; 1.0 at track load
//!   (0x61E100). Only 0x490920 reads it (the reflection flares' size,
//!   `1.2 + 8.8·wet`, 0x7F39A0, `crate::glow::streak_factor`).
//! * **Paused** (0x571260: race state 6, no online): nothing steps; the
//!   tint and the drops are applied with the values as they are.
//! * The step is skipped on a frame of 9 ms or less (0x7A873C); the drops'
//!   motion is per frame. The clone steps the object at a fixed 60 Hz
// prov: inferred weather.module
//!   (**inferred**: the original's frame rate), see [`Weather::update`].
//!
//! The visible parts: the fog blends toward "Night Raining Fog"
//! ([`crate::fog`]), the tint pass (effect slot 2, 0x5C7870), the falling
//! drops (0x61E710 / 0x613A80) and the drops on the lens (0x614100 /
//! 0x5C7AC0, rain_drop.efx). The grip: 0x5ED5E0 (this target at the car's
//! x, y) scales the player's base grip and Melov8r's grip by
//! `1 − 0.025·w` (0x5A4E75, 0x5AB01F; [`wetness_at`]).

use glam::{Vec2, Vec3};

use crate::formats::hash::bin_hash;
use crate::frontend::career::Trigger;
use crate::opponents::GameRng;
use crate::world::World;

/// 0x613060: the drop pool (+0x4D00).
pub const DROPS: usize = 400;
/// 0x614100: the lens drops while it rains (10 in split screen).
pub const SPLATS: usize = 20;
/// The step the per-frame rules assume (**inferred**: 60 Hz).
pub const STEP: f32 = 1.0 / 60.0;

/// The drop box around the car (0x8039AC…0x8039B8; the offsets 0x8A1D3C /
/// 0x8A1D40 are BSS, 0): x (forward) 8 ± 10, y ± 3.5, z 0…6.
const BOX_X: (f32, f32) = (8.0, 20.0);
const BOX_Y: f32 = 7.0;
const BOX_Z: f32 = 6.0;
/// 0x613060: the fall speeds per (type, sub type) (+0x4D4C…, from
/// 0x8039CC…0x8039D8, sub type 1 half; type 4 0, type 5 1).
const FALL: [[f32; 2]; 6] = [[0.3, 0.15], [1.0, 0.5], [0.4, 0.2], [0.3, 0.15], [0.0, 0.0], [1.0, 1.0]];
/// Type 0's streak half sizes (+0x4DB0 width, +0x4DB4 length; 0x8039DC /
/// 0x8039E0).
const STREAK: (f32, f32) = (0.03, 0.7);
/// The drop colours (D3DCOLOR ARGB): template +0x4F90's bytes 0x64646464
/// with alpha 0x1C, and +0x4F10's 0x80 (the drops near the car's heading,
/// +0x27).
const COLOR: [u8; 4] = [100, 100, 100, 28];
const COLOR_AHEAD: [u8; 4] = [128, 128, 128, 128];

/// The fog and tint values while it rains.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RainView {
    /// +0x1F0, 0…1.
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Drop {
    /// +0x24: 0 falling, 1 back to the top, 2 hidden.
    state: u8,
    /// +0x25: 0 rain, 5 unused.
    kind: u8,
    /// +0x26: 0 or 1 (0x43C1C0(2)).
    sub: u8,
    /// +0x27: near the car's heading (0x61F085).
    ahead: bool,
    /// The two positions (+0x200 + 0x10·(3k + 0x20 + i)), `cur` the newer.
    cur: Vec3,
    prev: Vec3,
}

/// The lens drops' fixed values (0x6140A0, the rain object's constructor
/// 0x61E160): per drop its life Rand_Float(1.5) + 0.3, speed
/// Rand_Float(0.1) + 0.02 and size Rand_Float(1.0).
#[derive(Clone, Copy, Debug)]
pub struct LensSetup([(f32, f32, f32); SPLATS]);

impl LensSetup {
    /// 0x6140A0 (0x6140C0 / 0x6140D3 / 0x6140E6) for one view: 20 × three
    /// draws.
    pub fn draw(rng: &mut GameRng) -> LensSetup {
        let mut out = [(0.0, 0.0, 0.0); SPLATS];
        for s in &mut out {
            let life = rng.unit(1.5) + 0.3;
            let speed = rng.unit(0.1) + 0.02;
            let size = rng.unit(1.0);
            *s = (life, speed, size);
        }
        LensSetup(out)
    }

    /// Start-up (0x57ED7C → 0x48CD60 → Views_Init 0x5C2FA0): the rain
    /// objects of views 1 and 2 are built in that order (0x5C3041,
    /// 0x5C30B4), each drawing its lens set-up on the shared generator
    /// (120 draws; **measured** c1/d1 boot, RNG-port-list R3). The clone
    /// keeps view 1's, the object the frame steps.
    pub fn boot(rng: &mut GameRng) -> LensSetup {
        let one = LensSetup::draw(rng);
        LensSetup::draw(rng);
        one
    }
}

impl Default for LensSetup {
    /// A standalone set-up (tests, tools): drawn from 0x12345678.
    fn default() -> LensSetup {
        LensSetup::draw(&mut GameRng::default())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Splat {
    x: f32,
    y: f32,
    timer: f32,
    life: f32,
    size: f32,
    speed: f32,
}

/// One lens drop as 0x5C7AC0 draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensDrop {
    /// Top-left of the quad in 0…1 screen units (y down).
    pub x: f32,
    pub y: f32,
    /// +0x14: 0…1.
    pub size: f32,
    /// raindropalpha: 3 · timer / life.
    pub alpha: f32,
}

/// One falling drop's quad in world space.
#[derive(Clone, Copy, Debug)]
pub struct Streak {
    pub corners: [Vec3; 4],
    pub rgba: [u8; 4],
}

pub struct Weather {
    /// +0x1F0.
    pub amount: f32,
    /// +0x5094.
    pub target: f32,
    /// +0x509C.
    pub wet: f32,
    /// The road's wetness from the rain alone (clone, Wet Roads only;
    /// docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"): +0x509C's rule (0.05 a step up while it
    /// rains, 0.0015 down) but 0 at track load, where the game sets
    /// +0x509C to 1 for the reflection flares' size whatever the weather.
    pub road_wet: f32,
    /// The cars' water film (clone, Wet Car; docs/notes/graphics/weather.md): +0x509C's
    /// rates from 0 at load, aimed at the rain's amount (not 1), none while
    /// the player's car is under a cover zone (+0x1E4).
    pub car_wet: f32,
    splats: [Splat; SPLATS],
    splat_count: usize,
    drops: Vec<Drop>,
    /// +0x4D04: drops per type; +0x4D1C: the quotas.
    counts: [u32; 6],
    /// +0x4EB0: the wind the drops drift with; gust state (0x803E60, 2 at
    /// start), its timers (+0x4ED0 / +0x4ED4, 0x8A44E4) and target (+0x4EC0).
    wind: Vec3,
    gust_state: u8,
    gust_wait: f32,
    gust_time: f32,
    gust_ramp: f32,
    gust_target: Vec3,
    /// 0x8A44F4: the car's heading as a binary angle.
    heading: Option<u16>,
    /// +0x5090: the streaks' stretch with the camera's speed.
    stretch: f32,
    /// The 10 per-frame drop motions (+0x5120…).
    jitter: [Vec3; 10],
    /// Time not yet stepped.
    pending: f32,
    /// The cover zones: the route triggers of kinds 5 and 6 (0x5D9C80
    /// masks 0x20 / 0x40), and whether the car is under one (+0x1E4) and
    /// the point 9.5 m ahead is (+0x1E8).
    covers: [Vec<Trigger>; 2],
    pub under_cover: bool,
    pub cover_ahead: bool,
}

/// The camera and car as one step needs them.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub camera: Vec3,
    /// The camera's look direction (camera +0x50, a unit vector).
    pub camera_forward: Vec3,
    pub camera_velocity: Vec3,
    pub car: Vec3,
    /// The car's forward axis.
    pub forward: Vec3,
    /// 0x7FB718.
    pub clock: f32,
}

/// One step of +0x509C (0x61F2BD): toward 1 (+0x5098 = rain ≠ 0) by 0.05
/// of the gap, back toward 0 by 0.0015 (0x7A67C8 / 0x7A67CC).
pub fn wet_step(wet: f32, raining: bool) -> f32 {
    let target = if raining { 1.0 } else { 0.0 };
    let rate = if target > wet { 0.05 } else { 0.0015 };
    wet + (target - wet) * rate
}

/// 0x6134F0: `|0.5·cos(clock + x / 50 degrees) + 0.5|`, the angle as
/// 0x401020 makes it (`⌊⌊deg·65536⌋ / 360⌋`, a 16-bit binary angle).
pub fn cycle(clock: f32, x: f32) -> f32 {
    let deg = clock / 1.0 + x / 50.0;
    let angle = binary_angle(deg);
    (cos16(angle) * 0.5 + 0.5).abs()
}

fn binary_angle(deg: f32) -> i32 {
    (deg * 65536.0) as i32 / 360
}

fn cos16(a: i32) -> f32 {
    ((a & 0xFFFF) as f32 * (std::f32::consts::TAU / 65536.0)).cos()
}

fn sin16(a: i32) -> f32 {
    ((a & 0xFFFF) as f32 * (std::f32::consts::TAU / 65536.0)).sin()
}

/// The world clock 0x7FB718 (seconds of sim time, 1.0 at track load) as
/// the game last set it ([`set_clock`]); the cars' grip reads the rain
/// through it (0x5ED5E0). The game has one global; the clone keeps it in
/// the loaded world, so two games in one process (the tests run side by
/// side) do not drive each other's rain and grip.
pub struct WorldClock(std::sync::atomic::AtomicU32);

impl Default for WorldClock {
    fn default() -> WorldClock {
        WorldClock(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()))
    }
}

pub fn set_clock(world: &World, clock: f32) {
    world.world_clock.0.store(clock.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn clock(world: &World) -> f32 {
    f32::from_bits(world.world_clock.0.load(std::sync::atomic::Ordering::Relaxed))
}

/// 0x5ED5E0: the rain at a car (0x61E390 at its x, y with the world clock).
pub fn wetness_at(world: &World, p: Vec2) -> f32 {
    target_at(world, p, clock(world))
}

/// `bin_hash("Rain")`, the rain map's key, worked out once at compile time
/// (not hashed by every car each step).
const RAIN_KEY: u32 = {
    let name = b"Rain";
    let mut h = 0xFFFF_FFFF_u32;
    let mut i = 0;
    while i < name.len() {
        h = h.wrapping_mul(33).wrapping_add(name[i] as u32);
        i += 1;
    }
    h
};

/// 0x61E390: the target at ground position `p`.
pub fn target_at(world: &World, p: Vec2, clock: f32) -> f32 {
    let s = cycle(clock, p.x);
    match world.parameter_maps.get(&RAIN_KEY) {
        Some(map) => {
            let row = map.sample(p);
            if row.get(1).copied().unwrap_or(0.0) >= s { row[0] } else { 0.0 }
        }
        None => 0.0,
    }
}

impl Default for Weather {
    fn default() -> Self {
        Weather::new()
    }
}

impl Weather {
    /// Dev harness (rate gate, `state_hash` `fx`): the rain's running
    /// state as text (Debug; exact floats): its RNG, splats, drops, counts,
    /// wind and gusts, stretch and the time not yet stepped.
    #[cfg(feature = "dev-harness")]
    pub fn dev_state(&self) -> String {
        format!(
            "{:?}",
            (
                (self.amount, self.target, self.wet, self.road_wet, self.car_wet),
                (&self.splats[..self.splat_count.min(SPLATS)], &self.drops, self.counts),
                (self.wind, self.gust_state, self.gust_wait, self.gust_time, self.gust_ramp, self.gust_target),
                (self.heading, self.stretch, self.jitter, self.pending),
            )
        )
    }

    /// The object as the track load leaves it (0x61E160, 0x61E100 →
    /// 0x613060(5, …)): dry, wetness 1, every drop hidden (type 5) at a
    /// random place in the box. Standalone (tests, tools): the lens set-up
    /// and the drops from a generator of its own at 0x12345678; the game
    /// builds it with [`Weather::with_lens`] and [`Weather::rain_init`] on
    /// its shared generator.
    pub fn new() -> Weather {
        let mut rng = GameRng::default();
        let mut w = Weather::with_lens(&LensSetup::draw(&mut rng));
        w.rain_set(&mut rng);
        w
    }

    /// The object with the lens drops' fixed values `lens` (0x61E160 →
    /// 0x6140A0, drawn once at start-up: [`LensSetup::boot`]) and no drops
    /// placed yet ([`Weather::rain_init`] places them at the world build).
    pub fn with_lens(lens: &LensSetup) -> Weather {
        let mut splats = [Splat::default(); SPLATS];
        for (s, &(life, speed, size)) in splats.iter_mut().zip(&lens.0) {
            (s.life, s.speed, s.size) = (life, speed, size);
        }
        let mut counts = [0; 6];
        counts[5] = DROPS as u32;
        Weather {
            covers: Default::default(),
            under_cover: false,
            cover_ahead: false,
            amount: 0.0,
            target: 0.0,
            wet: 1.0,
            road_wet: 0.0,
            car_wet: 0.0,
            splats,
            splat_count: 0,
            drops: vec![Drop { kind: 5, ..Default::default() }; DROPS],
            counts,
            wind: Vec3::ZERO,
            gust_state: 2,
            gust_wait: 0.0,
            gust_time: 0.0,
            gust_ramp: 0.0,
            gust_target: Vec3::ZERO,
            heading: None,
            stretch: 0.0,
            jitter: [Vec3::ZERO; 10],
            pending: 0.0,
        }
    }

    /// The world build's rain set-up (0x5FF6A5 → 0x61E100): 0x613060(5, 1.0)
    /// on view 1's object, then on view 2's (0x48B1E0(1) / (2)), each 400
    /// drops × {Rand_Int(2), Rand_Float(20), (7), (6)} and 10 motions ×
    /// {Rand_Float(0.02), (0.02), (0.03)} on the shared generator (1630
    /// draws a view, 3260 in all; **measured** c1/d1, docs/native-fixes/N-RNG.tsv).
    /// The clone's one weather object is view 1's (the one the frame steps,
    /// 0x5CB46E → 0x61F130); view 2's draws only move the generator on.
    pub fn rain_init(&mut self, rng: &mut GameRng) {
        self.rain_set(rng);
        Weather::with_lens(&LensSetup([(0.0, 0.0, 0.0); SPLATS])).rain_set(rng);
    }

    /// 0x613060(5, …) on this object: every drop hidden (type 5) at a
    /// random place in the box, and the 10 per-frame motions.
    fn rain_set(&mut self, rng: &mut GameRng) {
        for d in &mut self.drops {
            let sub = rng.below(2) as u8;
            let x = rng.unit(BOX_X.1) - BOX_X.1 * 0.5 + BOX_X.0;
            let y = rng.unit(BOX_Y) - BOX_Y * 0.5;
            let z = rng.unit(BOX_Z);
            let p = Vec3::new(x, y, z);
            *d = Drop { state: 0, kind: 5, sub, ahead: false, cur: p, prev: p };
        }
        for j in &mut self.jitter {
            // +0x4E10…+0x4E18 (0.02, 0.02, 0.03) and +0x4E70 (0.45).
            let x = rng.unit(0.02) - 0.01;
            let y = rng.unit(0.02) - 0.01;
            let z = -rng.unit(0.03) - 0.45;
            *j = Vec3::new(x, y, z);
        }
        self.counts = [0; 6];
        self.counts[5] = DROPS as u32;
    }

    #[cfg(test)]
    fn dev_state_test(&self) -> String {
        format!("{:?}", (&self.drops, self.jitter, self.counts, self.splats.map(|s| (s.life, s.speed, s.size))))
    }

    /// The flag the rain sound follows (CARSFX_Rain's update 0x4747A0
    /// starts it when +0x1F0 > 0.001, 0x78435C).
    pub fn raining(&self) -> bool {
        self.amount > 0.001
    }

    pub fn view(&self) -> RainView {
        RainView { amount: self.amount }
    }

    /// Advance by `dt` of game time in whole 60 Hz steps.
    pub fn update(&mut self, world: &World, frame: &Frame, dt: f32, rng: &mut GameRng) {
        self.pending += dt.max(0.0);
        // A long stall (a load) is not caught up step by step.
        if self.pending > 0.5 {
            self.pending = STEP;
        }
        while self.pending >= STEP {
            self.pending -= STEP;
            self.step(world, frame, STEP, rng);
        }
    }

    /// The cover zones (route triggers of kinds 5 and 6).
    pub fn set_covers(&mut self, covers: [Vec<Trigger>; 2]) {
        self.covers = covers;
    }

    /// Whether a place lies under one of the loaded cover zones (a kind 5
    /// or 6 route trigger holding it, higher than it): `update_cover`'s
    /// test for the car's place, without the chained search.
    pub fn covered_at(&self, at: Vec3) -> bool {
        self.covers.iter().flatten().any(|t| t.contains(at.truncate()) && at.z < t.position.z)
    }

    /// 0x5D9C80(at, mask, prev) over the cover kinds (5: mask 0x20, 6:
    /// 0x40; verified, the disassembly): the kinds from `prev`'s on whose
    /// mask bit is set, the first record holding `at` (0x5D9C00: after
    /// `prev` in `prev`'s own kind, from the first in the next ones). A
    /// mask with no bit at or above `prev`'s kind finds nothing.
    fn cover_find(&self, at: Vec2, kind: usize, prev: Option<(usize, usize)>) -> Option<(usize, usize)> {
        let start = prev.map_or(0, |p| p.0);
        if kind < start {
            return None;
        }
        let from = match prev {
            Some((k, i)) if k == kind => i + 1,
            _ => 0,
        };
        self.covers[kind].iter().enumerate().skip(from).find(|(_, t)| t.contains(at)).map(|(i, _)| (kind, i))
    }

    /// 0x61E710's cover tests (verified, the disassembly): +0x1E4 when a
    /// kind 5 or 6 route trigger holding the car's place (its +0x50 /
    /// +0x54) lies higher (+0x14) than the car (+0x58); +0x1E8 likewise for
    /// the point 9.5 m (0x7A67D8) along the car's unit forward axis (+0x90,
    /// its x and y). The four searches chain, each handed the last's result
    /// (0x5D9C80's third argument), as the game does.
    fn update_cover(&mut self, f: &Frame) {
        let here = f.car.truncate();
        let fw = f.forward.normalize_or_zero();
        let ahead = here + Vec2::new(fw.x, fw.y) * 9.5;
        let high = |c: Option<(usize, usize)>| c.is_some_and(|(k, i)| f.car.z < self.covers[k][i].position.z);
        self.under_cover = false;
        self.cover_ahead = false;
        let r = self.cover_find(here, 0, None);
        self.under_cover |= high(r);
        let r = self.cover_find(here, 1, r);
        self.under_cover |= high(r);
        let r = self.cover_find(ahead, 0, r);
        self.cover_ahead |= high(r);
        let r = self.cover_find(ahead, 1, r);
        self.cover_ahead |= high(r);
    }

    /// One frame of 0x61F130.
    pub fn step(&mut self, world: &World, f: &Frame, dt: f32, rng: &mut GameRng) {
        self.update_splats(f, dt, rng);
        // +0x5098 / +0x509C.
        let raining = self.amount != 0.0;
        self.wet = wet_step(self.wet, raining);
        self.road_wet = wet_step(self.road_wet, raining);
        self.car_wet = crate::render::wet_car::film_step(self.car_wet, if self.under_cover { 0.0 } else { self.amount });
        self.target = target_at(world, f.camera.truncate(), f.clock);
        self.amount += (self.target - self.amount) / (1.0 / dt * 1.0);
        if self.target == 0.0 && self.amount < 0.01 {
            self.amount = 0.0;
        }
        if self.amount > 1.0 {
            self.amount = 1.0;
        }
        self.update_drops(world, f, dt, rng);
    }

    /// 0x614100.
    fn update_splats(&mut self, f: &Frame, dt: f32, rng: &mut GameRng) {
        // None under cover (+0x1E4, last frame's: 0x614100 runs before
        // 0x61E710's tests).
        self.splat_count = if self.under_cover || self.amount == 0.0 { 0 } else { SPLATS };
        let push = f.camera_velocity.length() * 0.0001;
        for i in 0..self.splat_count {
            let s = &mut self.splats[i];
            s.timer -= dt;
            if s.timer > 0.0 {
                s.y += 0.2 * s.speed * dt;
                if s.y > 1.0 || s.x > 1.0 || s.x < 0.0 {
                    s.timer = 0.0;
                } else {
                    let d = Vec3::new(s.x - 0.5, s.y - 0.1, 0.0).normalize_or_zero();
                    s.x += d.x * push;
                }
            } else {
                s.timer = s.life;
                s.x = rng.unit(1.0);
                s.y = rng.unit(1.0);
            }
        }
    }

    /// The car's yaw frame: forward on the ground, z up.
    fn axes(forward: Vec3) -> (Vec3, Vec3) {
        let f = Vec3::new(forward.x, forward.y, 0.0).normalize_or(Vec3::X);
        (f, Vec3::new(-f.y, f.x, 0.0))
    }

    /// 0x6136C0 and 0x61E710.
    fn update_drops(&mut self, world: &World, f: &Frame, dt: f32, rng: &mut GameRng) {
        let (fx, fy) = Self::axes(f.forward);
        let to_local = |v: Vec3| Vec3::new(v.dot(fx), v.dot(fy), v.z);
        // 0x6136C0: the "Wind" map's direction (f1, degrees; f0 unused)
        // × 0.01 (0x803A84) in the car's frame.
        let mut base = Vec3::ZERO;
        if let Some(map) = world.parameter_maps.get(&bin_hash("Wind")) {
            let row = map.sample(f.camera.truncate());
            let a = binary_angle(row.get(1).copied().unwrap_or(0.0));
            base = to_local(Vec3::new(sin16(a), cos16(a), 0.0)) * 0.01;
        }
        match self.gust_state {
            0 => {
                self.gust_time += dt;
                if self.gust_time >= self.gust_wait {
                    self.gust_state = 2;
                }
            }
            1 => {
                self.wind.x += (self.gust_target.x - self.wind.x) * dt;
                self.wind.y += (self.gust_target.y - self.wind.y) * dt;
                self.gust_ramp += dt;
                if self.gust_ramp >= 1.0 {
                    self.gust_ramp = 0.0;
                    self.gust_state = 0;
                }
            }
            _ => {
                self.gust_wait = rng.unit(5.0) + 1.0;
                self.gust_time = 0.0;
                self.gust_state = 1;
                let x = (rng.unit(0.05) - 0.025) * 25.0;
                let y = (rng.unit(0.05) - 0.025) * 25.0;
                self.gust_target = Vec3::new(x, y, 0.0) + base;
            }
        }
        // 0x61E710: the quotas (0x613A40: type 0 all the active drops).
        let active = DROPS as u32 - ((1.0 - self.amount) * DROPS as f32) as u32;
        let quota = [active, 0, 0, 0, 0, DROPS as u32 - active];
        // The camera's velocity in the car's frame × −0.02 (0x803A14),
        // sideways dropped; its length is the streaks' stretch.
        let mut push = to_local(f.camera_velocity) * -0.02;
        push.y = 0.0;
        self.stretch = push.length();
        if self.stretch > 1.0 {
            self.stretch = 1.0;
            push = push.normalize();
        }
        let heading = binary_angle(fx.y.atan2(fx.x).to_degrees()) as u16;
        let turn = match self.heading {
            Some(h) => {
                let mut d = h.wrapping_sub(heading) as f32;
                if d >= 32768.0 {
                    d -= 65536.0;
                }
                d.clamp(-400.0, 400.0)
            }
            None => 0.0,
        };
        self.heading = Some(heading);
        let balanced = self.counts == quota;
        self.update_cover(f);
        let (x_lo, x_hi) = (BOX_X.0 - BOX_X.1 * 0.5, BOX_X.0 + BOX_X.1 * 0.5);
        for (k, d) in self.drops.iter_mut().enumerate() {
            if d.state == 0 {
                std::mem::swap(&mut d.cur, &mut d.prev);
                d.prev.y += turn * 5e-5 * d.prev.x;
                let fall = FALL[d.kind as usize][d.sub as usize];
                let mut p = d.prev + push + self.wind * fall + self.jitter[k % 10];
                if p.z < 0.0 {
                    d.state = 1;
                } else if p.x > x_hi {
                    p.x -= BOX_X.1;
                    d.prev.x -= BOX_X.1;
                } else if p.x < x_lo {
                    p.x += BOX_X.1;
                    d.prev.x += BOX_X.1;
                } else if p.y > BOX_Y * 0.5 {
                    p.y -= BOX_Y;
                    d.prev.y -= BOX_Y;
                } else if p.y < -BOX_Y * 0.5 {
                    p.y += BOX_Y;
                    d.prev.y += BOX_Y;
                }
                d.cur = p;
                d.ahead = p.x >= 0.0 && p.normalize_or_zero().x >= 0.96;
            } else {
                if !balanced {
                    let own = d.kind as usize;
                    if let Some(t) = (0..6).find(|&t| t != own && self.counts[t] < quota[t]) {
                        self.counts[own] -= 1;
                        self.counts[t] += 1;
                        d.kind = t as u8;
                    }
                }
                if d.kind != 5 && d.kind != 4 {
                    d.ahead = false;
                    if d.state == 1 {
                        d.cur.z = BOX_Z;
                        d.prev = d.cur;
                    }
                    d.state = 0;
                }
            }
            // Under cover the drops behind 9.5 m (ahead of it, when the
            // point ahead is covered) are hidden: type 5, state 2.
            if (self.cover_ahead && d.cur.x > 9.5) || (self.under_cover && d.cur.x < 9.5) {
                self.counts[d.kind as usize] -= 1;
                d.kind = 5;
                self.counts[5] += 1;
                d.state = 2;
            }
        }
    }

    /// The lens drops to draw (0x5C7AC0; none without rain).
    pub fn lens_drops(&self) -> impl Iterator<Item = LensDrop> + '_ {
        self.splats[..self.splat_count].iter().map(splat_drop)
    }

    /// The falling drops' quads in world space (0x613A80, type 0 with
    /// RAINDROP; the batch 0x5BB440 with the car's yaw frame as the world
    /// matrix): centre `c`, `A = (look × dir)·0.03` with `look` the
    /// camera's direction (camera +0x50) in the car's frame, `B = dir·(0.7 +
    /// stretch)` with `dir` from the newer to the older position, corners
    /// `c + A + B`, `c − A + B`, `c − A − B`, `c + A − B`.
    pub fn streaks(&self, f: &Frame) -> Vec<Streak> {
        streak_quads(
            self.drops.iter().filter(|d| d.kind == 0 && d.state == 0).map(|d| (d.cur, d.prev, d.ahead)),
            self.stretch,
            f,
        )
    }

    /// What the draw takes from this tick (the falling drops'
    /// two positions, the streaks' stretch, the lens drops) into `out`
    /// (its buffers reused: no allocation once they have their size).
    pub fn capture(&self, out: &mut RainSnapshot) {
        out.drops.clear();
        out.drops.extend(self.drops.iter().map(|d| DropView {
            cur: d.cur,
            prev: d.prev,
            drawn: d.kind == 0 && d.state == 0,
            ahead: d.ahead,
        }));
        out.stretch = self.stretch;
        out.splats.clear();
        out.splats.extend_from_slice(&self.splats[..self.splat_count.min(SPLATS)]);
    }
}

/// The quads of falling drops given as (newer, older position, near the
/// heading) in the car's yaw frame (0x613A80; see [`Weather::streaks`]).
fn streak_quads(drops: impl Iterator<Item = (Vec3, Vec3, bool)>, stretch: f32, f: &Frame) -> Vec<Streak> {
    let (fx, fy) = Weather::axes(f.forward);
    let to_world = |p: Vec3| f.car + fx * p.x + fy * p.y + Vec3::Z * p.z;
    let e = f.camera_forward;
    let eye = Vec3::new(e.dot(fx), e.dot(fy), e.z);
    let mut out = Vec::new();
    for (cur, prev, ahead) in drops {
        let dir = (prev - cur).normalize_or_zero();
        let a = eye.cross(dir) * STREAK.0;
        let b = dir * (STREAK.1 + stretch);
        let c = cur;
        out.push(Streak {
            corners: [to_world(c + a + b), to_world(c - a + b), to_world(c - a - b), to_world(c + a - b)],
            rgba: if ahead { COLOR_AHEAD } else { COLOR },
        });
    }
    out
}

/// One falling drop as the draw takes it from a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DropView {
    cur: Vec3,
    prev: Vec3,
    /// Falling and of the rain type (0x613A80 draws type 0, state 0).
    drawn: bool,
    ahead: bool,
}

/// Rate independence (docs/original/uncapped-fps.md "Drawn objects between ticks"): the
/// rain as one tick leaves it for the draw ([`Weather::capture`]), and
/// what a frame between two ticks draws ([`RainSnapshot::between`]): the
/// rain steps on the tick only; the drops and the lens drops are drawn at
/// the blend of their two ticks' positions.
#[derive(Clone, Debug, Default)]
pub struct RainSnapshot {
    drops: Vec<DropView>,
    stretch: f32,
    splats: Vec<Splat>,
}

/// A drop's move between two ticks beyond which it is not blended (m in
/// the car's frame): the drop box's wrap (20 m along, 7 m across) puts
/// its older position back by the box; one tick's fall is under 1 m.
const DROP_JUMP: f32 = 1.0;

impl RainSnapshot {
    /// The rain drawn `alpha` of the way from `prev` (S_{n−1}) to `cur`
    /// (S_n), into `out`: α ≤ 0 S_{n−1} as it is, α ≥ 1 S_n; between, a
    /// drop falling in both ticks at the blend of its two positions (a
    /// drop the box wrapped round: S_n), one that started falling in tick
    /// n at S_n, one that stopped not drawn; the stretch blended, the
    /// "ahead" colour held at S_{n−1}. A lens drop running in both at the
    /// blend of its place and timer; one started (or restarted) in tick n
    /// at S_n with its strength × α; one that ended not drawn.
    pub fn between(prev: &RainSnapshot, cur: &RainSnapshot, alpha: f32, out: &mut RainSnapshot) {
        use crate::render::native::fx::tick::lerp;
        out.drops.clear();
        out.splats.clear();
        if !(alpha > 0.0) {
            out.clone_from(prev);
            return;
        }
        if alpha >= 1.0 {
            out.clone_from(cur);
            return;
        }
        let v3 = |a: Vec3, b: Vec3| Vec3::new(lerp(a.x, b.x, alpha), lerp(a.y, b.y, alpha), lerp(a.z, b.z, alpha));
        out.stretch = lerp(prev.stretch, cur.stretch, alpha);
        for (k, b) in cur.drops.iter().enumerate() {
            let mut d = *b;
            match prev.drops.get(k) {
                Some(a) if a.drawn && b.drawn && b.prev.distance(a.cur) <= DROP_JUMP => {
                    d.cur = v3(a.cur, b.cur);
                    d.prev = v3(a.prev, b.prev);
                    d.ahead = a.ahead;
                }
                Some(a) if a.drawn && !b.drawn => d.drawn = false,
                _ => {}
            }
            out.drops.push(d);
        }
        for (i, b) in cur.splats.iter().enumerate() {
            let mut s = *b;
            match prev.splats.get(i) {
                Some(a) if b.timer > 0.0 && a.timer > b.timer => {
                    s.x = lerp(a.x, b.x, alpha);
                    s.y = lerp(a.y, b.y, alpha);
                    s.timer = lerp(a.timer, b.timer, alpha);
                }
                _ => s.timer *= alpha,
            }
            out.splats.push(s);
        }
    }

    /// The falling drops' quads (as [`Weather::streaks`] draws a tick's).
    pub fn streaks(&self, f: &Frame) -> Vec<Streak> {
        streak_quads(self.drops.iter().filter(|d| d.drawn).map(|d| (d.cur, d.prev, d.ahead)), self.stretch, f)
    }

    /// The lens drops (as [`Weather::lens_drops`] gives a tick's).
    pub fn lens_drops(&self) -> impl Iterator<Item = LensDrop> + '_ {
        self.splats.iter().map(splat_drop)
    }
}

/// A lens drop as 0x5C7AC0 draws it.
fn splat_drop(s: &Splat) -> LensDrop {
    LensDrop { x: s.x, y: s.y, size: s.size, alpha: if s.life > 0.0 { 3.0 * s.timer / s.life } else { 0.0 } }
}

/// The tint pass's colour for slot 2 (0x61F15F → 0x5D75F0, 0x5C7870): per
/// channel `⌊((⌊c⌋ / 128 − 1)·amount + 1)·127.5⌋`, drawn as
/// `dest·2·colour / 255` (SRCBLEND DESTCOLOR, DESTBLEND SRCCOLOR; texture
/// WHITE16X16_NOALPHA). `c` is the rain-blended fog row's f2, f3, f4 as
/// 0x613030 keeps them (+0x4EDC…: red f2, green f3, blue f4). The slot is
/// on while the amount is at least 0.01 (0x5D7701).
pub fn tint_multiplier(color: [f32; 3], amount: f32) -> Option<[f32; 3]> {
    if !(amount >= 0.01) {
        return None;
    }
    Some(color.map(|c| {
        let c = c as i32 as f32;
        let v = (((c * (1.0 / 128.0)) - 1.0) * amount + 1.0) * 127.5;
        2.0 * (v as i32 & 0xFF) as f32 / 255.0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The start-up's lens set-up (0x5C3041 / 0x5C30B4 → 0x6140A0)
    /// draws both views' 20 × 3 on the shared generator and keeps view 1's;
    /// the world build's rain set-up (0x61E100 → 0x613060 twice) draws
    /// 2 × 1630 and keeps view 1's drops.
    #[test]
    fn lens_and_rain_set_ups_draw_both_views_on_the_shared_generator() {
        let mut g = GameRng::default();
        let lens = LensSetup::boot(&mut g);
        let mut h = GameRng::default();
        let first = LensSetup::draw(&mut h);
        LensSetup::draw(&mut h);
        assert_eq!(g.0, h.0, "120 draws");
        assert_eq!(lens.0, first.0, "view 1's values");
        let mut w = Weather::with_lens(&lens);
        let before = g.0;
        w.rain_init(&mut g);
        let mut one = Weather::with_lens(&lens);
        let mut k = GameRng(before);
        one.rain_set(&mut k);
        let after_one = k.0;
        let mut n = GameRng(before);
        for _ in 0..3260 {
            n.below(1);
        }
        assert_eq!(g.0, n.0, "3260 draws");
        assert_ne!(after_one, g.0);
        assert_eq!(w.drops.iter().map(|d| d.cur).collect::<Vec<_>>(), one.drops.iter().map(|d| d.cur).collect::<Vec<_>>());
        assert_eq!(w.jitter, one.jitter);
        // The standalone object is the old one (lens, then view 1's rain).
        let a = Weather::new();
        let mut r = GameRng::default();
        let mut b = Weather::with_lens(&LensSetup::draw(&mut r));
        b.rain_set(&mut r);
        assert_eq!(a.dev_state_test(), b.dev_state_test());
    }

    #[test]
    fn rain_key_is_the_names_hash() {
        assert_eq!(RAIN_KEY, bin_hash("Rain"));
    }

    #[test]
    fn cycle_matches_the_original() {
        // Read from the original under Wine (0x89F048 and the clock, read
        // a few frames apart): clock 17.78, camera x 1689.4 → 0.811;
        // clock 12.53, x 1735.4 → 0.836.
        assert!((cycle(17.78, 1689.4) - 0.811).abs() < 0.006);
        assert!((cycle(12.53, 1735.4) - 0.836).abs() < 0.006);
        assert_eq!(cycle(0.0, 0.0), 1.0);
    }

    /// Wet Roads' road wetness: 0 at track load (the native +0x509C is
    /// 1 there and only sizes the reflection flares), up with the rain as
    /// +0x509C, then drying at its rate. The native value's decay
    /// matches the original under Wine (docs/evidence/s2-reflections:
    /// 0.0478 → 0.0393 in 2.0 s of a dry race, about 130 steps).
    #[test]
    fn road_wetness_follows_the_rain_only() {
        let w = Weather::new();
        assert_eq!((w.wet, w.road_wet), (1.0, 0.0));
        let mut road = 0.0;
        let mut native = 1.0;
        for _ in 0..600 {
            road = wet_step(road, false);
            native = wet_step(native, false);
        }
        assert_eq!(road, 0.0);
        assert!((native - 0.9985f32.powi(600)).abs() < 1e-4);
        let (a, b) = (0.0478f32, 0.0393f32);
        let steps = (b / a).ln() / 0.9985f32.ln();
        assert!((120.0..140.0).contains(&steps), "{steps}");
        for _ in 0..60 {
            road = wet_step(road, true);
        }
        assert!(road > 0.95);
        for _ in 0..667 {
            road = wet_step(road, false);
        }
        assert!((0.3..0.4).contains(&road), "{road}");
    }

    #[test]
    fn tint_is_neutral_without_rain_colour() {
        let m = tint_multiplier([128.0; 3], 1.0).unwrap();
        assert!((m[0] - 254.0 / 255.0).abs() < 1e-6);
        assert!(tint_multiplier([110.0; 3], 0.005).is_none());
    }

    fn quad_bits(q: &[Streak]) -> Vec<[u32; 12]> {
        q.iter().map(|s| std::array::from_fn(|k| s.corners[k / 3][k % 3].to_bits())).collect()
    }

    /// A raining run of ticks: the snapshot draws what the
    /// weather draws at each tick; between two ticks α = 0 is S_{n−1}, α =
    /// 1 S_n, and every drop drawn in both lies between its two places.
    #[test]
    fn rain_between_ticks_is_the_ticks_at_the_ends_and_between_inside() {
        let world = World::default();
        let mut rng = GameRng::default();
        let mut w = Weather::new();
        w.amount = 1.0;
        let frame = |i: usize| Frame {
            camera: Vec3::new(i as f32 * 0.4, 0.0, 1.5),
            camera_forward: Vec3::X,
            camera_velocity: Vec3::new(24.0, 0.0, 0.0),
            car: Vec3::new(i as f32 * 0.4, 0.0, 0.0),
            forward: Vec3::new(1.0, 0.002 * i as f32, 0.0),
            clock: 1.0,
        };
        let (mut prev, mut cur, mut out) = (RainSnapshot::default(), RainSnapshot::default(), RainSnapshot::default());
        let mut blended = 0;
        for i in 0..240 {
            w.amount = w.amount.max(0.9);
            w.step(&world, &frame(i), STEP, &mut rng);
            std::mem::swap(&mut prev, &mut cur);
            w.capture(&mut cur);
            let f = frame(i);
            assert_eq!(quad_bits(&cur.streaks(&f)), quad_bits(&w.streaks(&f)), "tick {i}");
            assert_eq!(cur.lens_drops().collect::<Vec<_>>(), w.lens_drops().collect::<Vec<_>>());
            if i == 0 {
                continue;
            }
            RainSnapshot::between(&prev, &cur, 0.0, &mut out);
            assert_eq!(quad_bits(&out.streaks(&f)), quad_bits(&prev.streaks(&f)));
            assert_eq!(out.lens_drops().collect::<Vec<_>>(), prev.lens_drops().collect::<Vec<_>>());
            RainSnapshot::between(&prev, &cur, 1.0, &mut out);
            assert_eq!(quad_bits(&out.streaks(&f)), quad_bits(&cur.streaks(&f)));
            RainSnapshot::between(&prev, &cur, 0.5, &mut out);
            for (k, d) in out.drops.iter().enumerate() {
                let (a, b) = (prev.drops[k], cur.drops[k]);
                if a.drawn && b.drawn && b.prev.distance(a.cur) <= DROP_JUMP {
                    blended += 1;
                    for c in 0..3 {
                        assert!(d.cur[c] >= a.cur[c].min(b.cur[c]) && d.cur[c] <= a.cur[c].max(b.cur[c]));
                    }
                } else if a.drawn && !b.drawn {
                    assert!(!d.drawn, "a drop that stopped falling is not drawn between");
                }
            }
            for (s, (a, b)) in out.splats.iter().zip(prev.splats.iter().zip(&cur.splats)) {
                if b.timer > 0.0 && a.timer > b.timer {
                    assert!(s.timer <= a.timer && s.timer >= b.timer);
                } else {
                    assert_eq!(s.timer, b.timer * 0.5, "a new lens drop fades in");
                }
            }
        }
        assert!(blended > 1000, "{blended}");
    }
}
