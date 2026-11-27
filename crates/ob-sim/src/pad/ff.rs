//! Force feedback as SPEED2.EXE drives it (docs/formats/front-end.md,
//! **Force feedback**): a pure model that turns the player's car into
//! DirectInput 8 effect commands. It sends nothing to a device; a backend
//! plays [`Command`]s.
//!
//! The chain in the original: the human driver's update 0x41E2C0 →
//! 0x41DFC0 (the steering) → 0x406460, once per world update 0x609CF0, so
//! once per drawn frame. The game here updates the world on fixed 1/60 s
//! ticks with the 60 fps tick pattern, so [`Original::driving`] runs once
//! per world tick: the per-call speed test (more than 4 m/s since the last
//! call) is per 1/60 s, as the original at 60 fps, at every display rate.
//! The 0.15 s spacing reads the sim clock 0x865188 and the 150 ms wall-clock
//! spacing (ftime in 0x5C9770) reads the real-time clock on ticks
//! ([`Clock`]), both as the original sees them at 60 fps.
//!
//! The effect functions (0x5C9160 spring, 0x5C95A0 damper, 0x5C9390
//! constant, 0x5C9FE0 periodic, 0x5C9DA0 puddle damper, 0x5C9770 jolt)
//! share one pattern: upload the parameters when they differ from the last
//! upload (or create the effect), and start it when it is not playing.
//! Spring and damper do nothing while the puddle damper plays; nothing but
//! the jolt plays while the effects are suspended (both front wheels off
//! the ground).
//!
//! Units are DirectInput's: levels, coefficients and saturations in
//! ±10000, periods and durations in ms.

use super::DevType;

/// DirectInput's full scale (DI_FFNOMINALMAX).
pub const FULL: i32 = 10_000;
/// The spring while driving at full lock (0x4064F6) and with Feedback off at
/// full lock (0x40699F).
pub const SPRING_FULL_LOCK: i32 = 10_000;
/// The spring with Feedback off below full lock (0x4069C9: 0x1B92).
pub const SPRING_FEEDBACK_OFF: i32 = 0x1B92;
/// The spring in the front end (0x5CFFAD: 0x1B58).
pub const SPRING_FRONT_END: i32 = 0x1B58;
/// The puddle damper's strength (0x4068ED); played negated (0x5C9DA0).
pub const PUDDLE: i32 = 10_000;
/// Surface 13, PUDDLE (surface table 0x800EC0 + 0x98·s, name at +0x20).
pub const SURFACE_PUDDLE: u32 = 13;
/// The jolt: GUID_Square, 150 ms, period 75 ms, fade to 0 over the last
/// 20 ms (0x5C9845..0x5C9877).
pub const JOLT_MS: i32 = 150;
pub const JOLT_PERIOD_MS: i32 = 75;
pub const JOLT_FADE_MS: i32 = 20;
/// The periodic effect's period cap (0x5C9FE0), ms.
pub const TERRAIN_PERIOD_MAX_MS: i32 = 150;

/// 0x784504: 1/13, the speed (m/s) at which the damper reaches 0 and the
/// terrain its full magnitude.
const PER_SPEED: f32 = 0.076_923_08;
/// 0x7844FC: the damper at standstill.
const DAMPER_MAX: f32 = 7_058.823_5;
/// 0x7844F8: the speed drop per call that jolts, m/s.
const JOLT_DROP: f32 = 4.0;
/// 0x7844EC: 1/15.
const JOLT_DROP_SCALE: f32 = 0.066_666_67;
/// 0x7844F4: the bump-stop total that jolts.
const BUMP_MIN: f32 = 0.05;
/// 0x783AD0: the bump-stop total for a full jolt.
const BUMP_FULL: f32 = 4.0;
/// 0x7844E8: the bump jolt's full magnitude.
const BUMP_MAX: f32 = 6_666.666_5;
/// 0x7844F0: the jolts' spacing on the sim clock, s.
const JOLT_SPACING: f32 = 0.15;
/// 0x784268: the sim clock's tick, s.
const SIM_TICK: f32 = 0.000_25;
/// 0x7844E4: the terrain magnitude per unit of the surface table.
const TERRAIN_SCALE: f32 = 39.215_687;
/// 0x7844E0: the puddle damper's speed, m/s.
const PUDDLE_SPEED: f32 = 23.0;
/// 0x5C9560's angle for the constant force (0x4065D4: 270).
const CONSTANT_ANGLE: usize = 270;
/// The jolts' wall-clock spacing in 0x5C9770, ms.
const JOLT_WALL_MS: u64 = 150;

/// The periodic waves 0x5C9FE0 picks by its argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    /// 0: GUID_Sine (0x7BFCD0).
    Sine,
    /// 1: GUID_Square (0x7BFCE0).
    Square,
    /// 2: GUID_Triangle (0x7BFCC0).
    Triangle,
    /// 3: GUID_SawtoothUp (0x7BFCB0); 0x406460 never asks for it.
    SawUp,
    /// 4: GUID_SawtoothDown (0x7BFCA0); 0x406460 never asks for it.
    SawDown,
}

/// The effects 0x406460 drives, by their table slot in the FF object
/// (*0x870760: conditions +0x2498, constant +0x24E8, periodic +0x2538).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// Condition slot 0.
    Spring,
    /// Condition slot 1.
    Damper,
    /// Condition slot 2.
    Puddle,
    /// Constant slot 0.
    Constant,
    /// Periodic slot 3.
    Terrain,
    /// Periodic slot 0.
    Jolt,
}

impl Slot {
    const ALL: [Slot; 6] = [Slot::Spring, Slot::Damper, Slot::Puddle, Slot::Constant, Slot::Terrain, Slot::Jolt];

    fn index(self) -> usize {
        self as usize
    }
}

/// An effect's parameters. Every effect: DIEFF_OBJECTOFFSETS |
/// DIEFF_CARTESIAN (0x12), axes X then Y (as many as the device's
/// actuators), gain 10000, no trigger button. A second axis gets the
/// condition {coefficients ±8000, saturations 8000, deadband 500}
/// (observed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// GUID_Spring: offset 0, both coefficients and saturations `coef`,
    /// deadband 0, infinite.
    Spring { coef: i32 },
    /// GUID_Damper: both coefficients `coef`, saturations 10000, infinite.
    Damper { coef: i32 },
    /// GUID_Damper with both coefficients −`coef` (a negative damper: the
    /// wheel goes light), saturations 10000, infinite.
    Puddle { coef: i32 },
    /// GUID_ConstantForce: magnitude `level`, infinite.
    Constant { level: i32 },
    /// The periodic `wave`: magnitude, offset 0, phase 0, period, infinite.
    Terrain { wave: Wave, magnitude: i32, period_ms: i32 },
    /// GUID_Square: magnitude, [`JOLT_MS`] long, period [`JOLT_PERIOD_MS`],
    /// fading to 0 over [`JOLT_FADE_MS`].
    Jolt { magnitude: i32 },
}

impl Effect {
    pub fn slot(&self) -> Slot {
        match self {
            Effect::Spring { .. } => Slot::Spring,
            Effect::Damper { .. } => Slot::Damper,
            Effect::Puddle { .. } => Slot::Puddle,
            Effect::Constant { .. } => Slot::Constant,
            Effect::Terrain { .. } => Slot::Terrain,
            Effect::Jolt { .. } => Slot::Jolt,
        }
    }
}

/// What the device is asked to do, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Upload these parameters (CreateEffect the first time, SetParameters
    /// after; a periodic effect whose wave changes is released and created
    /// again, and a [`Command::Start`] follows).
    Set(Effect),
    /// Start(1, 0): play once from the beginning.
    Start(Slot),
    /// Stop.
    Stop(Slot),
}

/// The FF device on the player's channel: its class (0x5BFB70) and the
/// actuator count (0x86FF80, up to 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Device {
    pub class: DevType,
    pub actuators: u8,
}

/// What 0x406460 reads, once per world tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Feed {
    /// Its first argument: the player's steering as 0x41DFC0 returns it
    /// (the shaped input, −1..1, positive left). Full lock is |steer| = 1.0
    /// exactly.
    pub steer: f32,
    /// |car +0x70|, m/s.
    pub speed: f32,
    /// The front tyres' force across the car (0x59DB00's field +4 on car
    /// +0x34 → +0x38 and +0x3C: sin δ · +0x124 + cos δ · +0x128 per wheel),
    /// kN, summed.
    pub front_sideways: f32,
    /// The bump stops' impulses this frame, car +0x370..+0x37C (0x5ABB10
    /// adds 0x59E1B0's 0.225 · mass · closing speed per sub-step; the
    /// frame start 0x5B731B zeroes them).
    pub bump: [f32; 4],
    /// The front wheels touch the ground (car +0x390 / +0x391 = wheel
    /// +0x11C, exported by 0x5A57FC).
    pub front_down: [bool; 2],
    /// The front wheels' surfaces (car +0x394 / +0x398 = wheel +0x114; 1 in
    /// drag and Street X races, 0x59CF75).
    pub front_surface: [u32; 2],
    /// 0x4018F0: a drag race (0x89E7D8) or its online twin 0x890108. The
    /// effects then run as with Feedback off.
    pub drag: bool,
}

/// The player's options (player block 0x83AA64 + 0x28·p).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// +4 Feedback (default on).
    pub feedback: bool,
    /// +0x20 Road Feedback (2–20, default 12.4): copied to the steering
    /// object's +0xC (0x41E25E), the constant force's divisor.
    pub road_feedback: f32,
}

impl Default for Options {
    fn default() -> Options {
        Options { feedback: true, road_feedback: 12.4 }
    }
}

/// The clocks the jolts read, as the tick in progress sees them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    /// 0x865188: the sim clock, 1/4000 s.
    pub sim: u32,
    /// The wall clock 0x5C9770 reads (ftime), ms: the real-time clock on
    /// ticks (0x8651A8 / 4).
    pub real_ms: u64,
}

/// DIPROP_FFGAIN from Overall Force Strength (0x41E28D → 0x5C9130 →
/// 0x5C00C0): `min(100, _ftol(strength × 255)) × 100`, the minimum taken
/// unsigned. Any strength from 0.4 up is the full 10000; a negative one
/// too.
pub fn gain(strength: f32) -> i32 {
    let t = ftol(f64::from(strength) * 255.0) as u32;
    t.min(100) as i32 * 100
}

/// MSVC's _ftol (0x75BED4): truncation; out of range or NaN gives
/// 0x80000000.
fn ftol(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 { i32::MIN } else { x.trunc() as i32 }
}

/// 0x401070: min(a, b) on floats.
fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// The FF object's angle table +0x25F0 (0x5BFD30): `_ftol(sin(d · π / 180)
/// × 1000)` with π as a float, so 90° gives 999 and 270° −999.
fn sine_table(degrees: usize) -> i32 {
    let pi = f64::from(std::f32::consts::PI);
    ftol((pi * degrees as f64 * (1.0 / 180.0)).sin() * 1000.0)
}

/// The surface table of 0x406460 (jump tables 0x4069E4 / 0x406A0C).
/// Returns (amplitude, period, wave) from the two front surfaces.
pub fn terrain(s1: u32, s2: u32) -> (i32, i32, Wave) {
    // The first front wheel's surface sets the pair.
    let (a1, p1) = match s1 {
        8 => (130, 80),   // DIRT
        9 => (110, 80),   // GRAVEL
        10 => (70, 90),   // ROUGH_ROAD
        11 => (100, 100), // COBBLESTONE
        15 => (100, 40),  // GRASS
        16 => (80, 100),  // SIDEWALK
        17 => (80, 140),  // WOOD
        _ => (0, 0),
    };
    // The second adds its pair and picks the wave; SIDEWALK and WOOD
    // replace the sums.
    match s2 {
        8 => (a1 + 130, p1 + 80, Wave::Sine),
        9 => (a1 + 110, p1 + 80, Wave::Sine),
        10 => (a1 + 70, p1 + 90, Wave::Square),
        11 => (a1 + 100, p1 + 100, Wave::Square),
        15 => (a1 + 100, p1 + 40, Wave::Triangle),
        16 => (80, 100, Wave::Square),
        17 => (80, 140, Wave::Square),
        _ => (a1, p1, Wave::Sine),
    }
}

/// The FF object for the player's channel (0x870760's per-channel state)
/// and 0x406460's globals.
#[derive(Clone, Debug)]
pub struct Original {
    device: Device,
    /// The last parameters uploaded per slot (+0x2BA0.., +0x2BB8, +0x2BC8,
    /// +0x2BE0, +0x2BF8, +0x2C00..).
    cache: [Option<Effect>; 6],
    /// GetEffectStatus as the model keeps it: started and not stopped.
    playing: [bool; 6],
    /// +0x2220: suspended (0x5CDA20); the slots to start again on resume
    /// (+0x25DC..).
    suspended: bool,
    resume: [bool; 6],
    /// +0x25D8 / +0x25DA: the damper and the spring the puddle damper
    /// stopped, restarted when it ends (0x5CD920).
    held_damper: bool,
    held_spring: bool,
    /// 0x827AAC: the speed at the last call, m/s.
    prev_speed: f32,
    /// 0x827AA4: the last jolt on the sim clock, s.
    last_jolt: f32,
    /// +0x2B90 / +0x2B98: the last jolt on the wall clock, ms.
    wall_jolt: Option<u64>,
}

impl Original {
    /// The model for a device with FF actuators (0x86FF80 ≠ 0); None
    /// without: every effect function checks the count and does nothing.
    pub fn new(device: Device) -> Option<Original> {
        (device.actuators > 0).then_some(Original {
            device,
            cache: [None; 6],
            playing: [false; 6],
            suspended: false,
            resume: [false; 6],
            held_damper: false,
            held_spring: false,
            prev_speed: 0.0,
            last_jolt: 0.0,
            wall_jolt: None,
        })
    }

    pub fn device(&self) -> Device {
        self.device
    }

    /// Whether `slot` plays (as far as the commands go).
    pub fn playing(&self, slot: Slot) -> bool {
        self.playing[slot.index()]
    }

    /// The slots playing now.
    pub fn active(&self) -> Vec<Slot> {
        Slot::ALL.into_iter().filter(|&s| self.playing(s)).collect()
    }

    fn gamepad(&self) -> bool {
        self.device.class == DevType::Gamepad
    }

    /// One world tick while driving (0x406460; the caller has checked its
    /// gates: car +0x28 = 1, a player, the channel's device, game state 4).
    pub fn driving(&mut self, feed: &Feed, options: &Options, clock: Clock) -> Vec<Command> {
        let mut out = Vec::new();
        if !options.feedback || feed.drag {
            self.feedback_off(&mut out, feed.steer);
            return out;
        }
        let full_lock = feed.steer.abs() == 1.0;
        if full_lock {
            self.spring(&mut out, SPRING_FULL_LOCK);
        } else {
            self.stop(&mut out, Slot::Spring);
        }
        let v = feed.speed;
        let share = min(1.0, v * PER_SPEED);
        // The constant force: only called while the tyres push (a sum of
        // exactly 0 leaves the last level playing), never on a gamepad.
        let sideways = feed.front_sideways;
        if sideways != 0.0 {
            let level = if v > 1.0 {
                ftol(f64::from(sideways) / f64::from(options.road_feedback) * f64::from(FULL as f32)).clamp(-FULL, FULL)
            } else {
                0
            };
            if !self.gamepad() {
                self.constant(&mut out, sine_table(CONSTANT_ANGLE) * level / 1000);
            }
        }
        self.damper(&mut out, ftol(f64::from(1.0 - share) * f64::from(DAMPER_MAX)));
        if !feed.front_down[0] && !feed.front_down[1] {
            self.suspend(&mut out);
        } else {
            self.resume(&mut out);
        }
        self.jolts(&mut out, v, feed.bump, clock);
        let (amp, per, wave) = terrain(feed.front_surface[0], feed.front_surface[1]);
        let magnitude = ftol(f64::from(amp / 2) * f64::from(share) * f64::from(TERRAIN_SCALE));
        self.periodic(&mut out, wave, magnitude, per / 2);
        if feed.front_surface.contains(&SURFACE_PUDDLE) && v > PUDDLE_SPEED {
            self.puddle(&mut out, PUDDLE);
        } else {
            self.end_puddle(&mut out);
        }
        self.prev_speed = v;
        out
    }

    /// One tick outside game state 4 (the input poll, 0x5CFE40..0x5CFFBA):
    /// everything but the spring off, the spring at 7000 (stopped on a
    /// gamepad; the original tests channel 0's class for every channel).
    pub fn front_end(&mut self) -> Vec<Command> {
        let mut out = Vec::new();
        self.stop(&mut out, Slot::Constant);
        self.stop(&mut out, Slot::Terrain);
        self.stop(&mut out, Slot::Damper);
        self.resume(&mut out);
        if self.playing(Slot::Puddle) {
            self.end_puddle(&mut out);
        }
        if self.gamepad() {
            self.stop(&mut out, Slot::Spring);
        } else {
            self.spring(&mut out, SPRING_FRONT_END);
        }
        out
    }

    /// Feedback off or a drag race (0x40692A): only the centring spring.
    /// The speed of this call is not kept.
    fn feedback_off(&mut self, out: &mut Vec<Command>, steer: f32) {
        self.stop(out, Slot::Constant);
        self.stop(out, Slot::Terrain);
        self.stop(out, Slot::Damper);
        self.resume(out);
        self.end_puddle(out);
        if self.gamepad() {
            self.stop(out, Slot::Spring);
        } else if steer.abs() == 1.0 {
            self.spring(out, SPRING_FULL_LOCK);
        } else {
            self.spring(out, SPRING_FEEDBACK_OFF);
        }
    }

    /// The speed and bump jolts (0x406656..0x4067AB).
    fn jolts(&mut self, out: &mut Vec<Command>, v: f32, bump: [f32; 4], clock: Clock) {
        let t = f64::from(clock.sim) * f64::from(SIM_TICK);
        // A drop of more than 4 m/s since the last call; a rise as well,
        // with a negative magnitude past −1 (not clamped).
        let drop = self.prev_speed - v;
        if drop.abs() > JOLT_DROP {
            let gap = t - f64::from(self.last_jolt);
            if gap < 0.0 {
                self.last_jolt = 0.0;
            } else if gap >= f64::from(JOLT_SPACING) {
                self.last_jolt = t as f32;
                let k = min(1.0, (drop - JOLT_DROP) * JOLT_DROP_SCALE);
                self.jolt(out, ftol(f64::from(k) * f64::from(FULL as f32)), clock);
            }
        }
        let sum = bump.iter().fold(0.0f64, |s, &b| s + f64::from(b));
        if sum > f64::from(BUMP_MIN) {
            let k = min(1.0, (sum / f64::from(BUMP_FULL)) as f32);
            let gap = t - f64::from(self.last_jolt);
            // A clock behind the last jolt (a world load reset it) resets
            // the stamp and jolts all the same (0x406718..0x406727).
            let fire = if gap < 0.0 {
                self.last_jolt = 0.0;
                true
            } else if gap >= f64::from(JOLT_SPACING) {
                self.last_jolt = t as f32;
                true
            } else {
                false
            };
            if fire {
                self.jolt(out, ftol(f64::from(k) * f64::from(BUMP_MAX)), clock);
            }
        }
    }

    /// Play `e` (upload when changed, start when not playing).
    fn set_and_play(&mut self, out: &mut Vec<Command>, e: Effect) {
        let s = e.slot().index();
        let restart = match (self.cache[s], e) {
            (Some(Effect::Terrain { wave: old, .. }), Effect::Terrain { wave, .. }) => old != wave,
            _ => false,
        };
        if self.cache[s] != Some(e) {
            out.push(Command::Set(e));
            self.cache[s] = Some(e);
        }
        if !self.playing[s] || restart {
            out.push(Command::Start(e.slot()));
            self.playing[s] = true;
        }
    }

    fn stop(&mut self, out: &mut Vec<Command>, slot: Slot) {
        if self.playing[slot.index()] {
            out.push(Command::Stop(slot));
            self.playing[slot.index()] = false;
        }
    }

    /// 0x5C9160.
    fn spring(&mut self, out: &mut Vec<Command>, coef: i32) {
        if self.playing(Slot::Puddle) || self.suspended {
            return;
        }
        self.set_and_play(out, Effect::Spring { coef });
    }

    /// 0x5C95A0.
    fn damper(&mut self, out: &mut Vec<Command>, coef: i32) {
        if self.playing(Slot::Puddle) || self.suspended {
            return;
        }
        self.set_and_play(out, Effect::Damper { coef });
    }

    /// 0x5C9390 (through 0x5C9560's angle scaling).
    fn constant(&mut self, out: &mut Vec<Command>, level: i32) {
        if self.suspended {
            return;
        }
        self.set_and_play(out, Effect::Constant { level });
    }

    /// 0x5C9FE0.
    fn periodic(&mut self, out: &mut Vec<Command>, wave: Wave, magnitude: i32, period_ms: i32) {
        if self.suspended {
            return;
        }
        let period_ms = period_ms.min(TERRAIN_PERIOD_MAX_MS);
        self.set_and_play(out, Effect::Terrain { wave, magnitude, period_ms });
    }

    /// 0x5C9DA0: stops the damper and the spring (kept for 0x5CD920), then
    /// plays the negative damper.
    fn puddle(&mut self, out: &mut Vec<Command>, coef: i32) {
        if self.playing(Slot::Damper) {
            self.stop(out, Slot::Damper);
            self.held_damper = true;
        }
        if self.playing(Slot::Spring) {
            self.stop(out, Slot::Spring);
            self.held_spring = true;
        }
        if self.suspended {
            return;
        }
        self.set_and_play(out, Effect::Puddle { coef });
    }

    /// 0x5CD920: the puddle damper off, the damper and spring it stopped
    /// back with their last parameters.
    fn end_puddle(&mut self, out: &mut Vec<Command>) {
        self.stop(out, Slot::Puddle);
        if std::mem::take(&mut self.held_damper)
            && let Some(Effect::Damper { coef }) = self.cache[Slot::Damper.index()]
        {
            self.damper(out, coef);
        }
        if std::mem::take(&mut self.held_spring)
            && let Some(Effect::Spring { coef }) = self.cache[Slot::Spring.index()]
        {
            self.spring(out, coef);
        }
    }

    /// 0x5C9770: the jolt, at most once per 150 ms of wall clock; it plays
    /// while suspended too.
    fn jolt(&mut self, out: &mut Vec<Command>, magnitude: i32, clock: Clock) {
        if self.wall_jolt.is_some_and(|last| clock.real_ms < last + JOLT_WALL_MS) {
            return;
        }
        self.wall_jolt = Some(clock.real_ms);
        let e = Effect::Jolt { magnitude };
        let s = Slot::Jolt.index();
        if self.cache[s] != Some(e) {
            out.push(Command::Set(e));
            self.cache[s] = Some(e);
        }
        // Started on every jolt; it ends by its duration, so the model
        // never counts it as playing.
        out.push(Command::Start(Slot::Jolt));
    }

    /// 0x5CDA20: both front wheels off the ground. Stops what plays and
    /// keeps the set for [`Original::resume`].
    fn suspend(&mut self, out: &mut Vec<Command>) {
        if self.suspended {
            return;
        }
        self.suspended = true;
        for slot in [Slot::Spring, Slot::Constant, Slot::Damper] {
            if self.playing(slot) {
                self.stop(out, slot);
                self.resume[slot.index()] = true;
            }
        }
        if self.playing(Slot::Puddle) {
            // 0x5CD920 inside the suspension: the held damper and spring
            // are forgotten, not restarted.
            self.end_puddle(out);
            self.resume[Slot::Puddle.index()] = true;
        }
        if self.playing(Slot::Terrain) {
            self.stop(out, Slot::Terrain);
            self.resume[Slot::Terrain.index()] = true;
        }
    }

    /// 0x5CDBC0: a front wheel down again; the stopped effects start with
    /// their last parameters. The constant force comes back at level 0:
    /// 0x5CDBC0 replays it through 0x5C9560 with the angle +0x2BBC, which
    /// nothing writes (**inferred** 0, sin 0 = 0); the next tick sets it
    /// again.
    fn resume(&mut self, out: &mut Vec<Command>) {
        if !self.suspended {
            return;
        }
        self.suspended = false;
        let marks = std::mem::take(&mut self.resume);
        let cached = self.cache;
        if marks[Slot::Spring.index()]
            && let Some(Effect::Spring { coef }) = cached[Slot::Spring.index()]
        {
            self.spring(out, coef);
        }
        if marks[Slot::Constant.index()] && cached[Slot::Constant.index()].is_some() {
            self.constant(out, 0);
        }
        if marks[Slot::Damper.index()]
            && let Some(Effect::Damper { coef }) = cached[Slot::Damper.index()]
        {
            self.damper(out, coef);
        }
        if marks[Slot::Puddle.index()]
            && let Some(Effect::Puddle { coef }) = cached[Slot::Puddle.index()]
        {
            self.puddle(out, coef);
        }
        if marks[Slot::Terrain.index()]
            && let Some(Effect::Terrain { wave, magnitude, period_ms }) = cached[Slot::Terrain.index()]
        {
            self.periodic(out, wave, magnitude, period_ms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHEEL: Device = Device { class: DevType::Driving, actuators: 1 };
    const PAD: Device = Device { class: DevType::Gamepad, actuators: 2 };

    fn on_road(speed: f32) -> Feed {
        Feed { speed, front_down: [true, true], front_surface: [1, 1], ..Feed::default() }
    }

    fn sets(c: &[Command]) -> Vec<Effect> {
        c.iter()
            .filter_map(|c| match c {
                Command::Set(e) => Some(*e),
                _ => None,
            })
            .collect()
    }

    fn damper_at(speed: f32) -> i32 {
        let mut ff = Original::new(WHEEL).unwrap();
        let c = ff.driving(&on_road(speed), &Options::default(), Clock::default());
        sets(&c)
            .into_iter()
            .find_map(|e| match e {
                Effect::Damper { coef } => Some(coef),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn no_actuators_no_model() {
        assert!(Original::new(Device { class: DevType::Driving, actuators: 0 }).is_none());
    }

    #[test]
    fn gain_quirk() {
        assert_eq!(gain(1.0), 10_000);
        assert_eq!(gain(0.4), 10_000);
        assert_eq!(gain(0.3), 7_600);
        assert_eq!(gain(0.1), 2_500);
        assert_eq!(gain(0.0), 0);
        // _ftol's negative result compares unsigned: full gain.
        assert_eq!(gain(-0.5), 10_000);
    }

    #[test]
    fn damper_law() {
        // (1 − min(1, v/13)) × 7058.8, truncated.
        assert_eq!(damper_at(0.0), 7_058);
        assert_eq!(damper_at(6.5), 3_529);
        assert_eq!(damper_at(13.0), 0);
        assert_eq!(damper_at(40.0), 0);
        for v in [1.0f32, 3.3, 9.0, 12.0] {
            let want = (1.0 - v / 13.0) * 7058.8;
            assert!((damper_at(v) as f32 - want).abs() <= 1.5, "{v}");
        }
    }

    #[test]
    fn spring_cases() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        // Driving below full lock: no spring.
        let c = ff.driving(&on_road(10.0), &o, Clock::default());
        assert!(!sets(&c).iter().any(|e| matches!(e, Effect::Spring { .. })));
        // Full lock: 10000, started.
        let c = ff.driving(&Feed { steer: -1.0, ..on_road(10.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Spring { coef: 10_000 })));
        assert!(c.contains(&Command::Start(Slot::Spring)));
        // Off full lock again: stopped.
        let c = ff.driving(&Feed { steer: 0.99, ..on_road(10.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Stop(Slot::Spring)));
        // Feedback off: 7058 below full lock, 10000 at it.
        let off = Options { feedback: false, ..o };
        let c = ff.driving(&on_road(10.0), &off, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Spring { coef: 7_058 })));
        let c = ff.driving(&Feed { steer: 1.0, ..on_road(10.0) }, &off, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Spring { coef: 10_000 })));
        // The front end: 7000.
        let c = ff.front_end();
        assert!(c.contains(&Command::Set(Effect::Spring { coef: 7_000 })));
        // A gamepad-class device gets no spring with Feedback off or in the
        // front end, but does at full lock while driving.
        let mut pad = Original::new(PAD).unwrap();
        assert!(sets(&pad.front_end()).is_empty());
        assert!(sets(&pad.driving(&on_road(10.0), &off, Clock::default())).is_empty());
        let c = pad.driving(&Feed { steer: 1.0, ..on_road(10.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Spring { coef: 10_000 })));
    }

    #[test]
    fn unchanged_parameters_are_not_sent_again() {
        let mut ff = Original::new(WHEEL).unwrap();
        let o = Options::default();
        let first = ff.driving(&on_road(0.0), &o, Clock::default());
        assert!(first.contains(&Command::Start(Slot::Damper)));
        let again = ff.driving(&on_road(0.0), &o, Clock::default());
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn constant_force() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        // 6.2 kN across at 20 m/s: 6.2 / 12.4 × 10000 = 5000, through the
        // angle table's sin 270° = −999 / 1000.
        let c = ff.driving(&Feed { front_sideways: 6.2, ..on_road(20.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Constant { level: -4_995 })));
        // Clamped to ±10000 before the table.
        let c = ff.driving(&Feed { front_sideways: -30.0, ..on_road(20.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Constant { level: 9_990 })));
        // At 1 m/s or less: 0.
        let c = ff.driving(&Feed { front_sideways: 3.0, ..on_road(1.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Constant { level: 0 })));
        // A sum of exactly 0 leaves it alone.
        let c = ff.driving(&Feed { front_sideways: 0.0, ..on_road(20.0) }, &o, Clock::default());
        assert!(!c.iter().any(|c| matches!(c, Command::Set(Effect::Constant { .. }))));
        assert!(ff.playing(Slot::Constant));
        // Road Feedback is the divisor.
        let mut ff = Original::new(WHEEL).unwrap();
        let o = Options { road_feedback: 2.0, ..o };
        let c = ff.driving(&Feed { front_sideways: 1.0, ..on_road(20.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Set(Effect::Constant { level: -4_995 })));
        // Never on a gamepad.
        let mut pad = Original::new(PAD).unwrap();
        let c = pad.driving(&Feed { front_sideways: 6.2, ..on_road(20.0) }, &o, Clock::default());
        assert!(!c.iter().any(|c| matches!(c, Command::Set(Effect::Constant { .. }))));
    }

    #[test]
    fn angle_table() {
        assert_eq!(sine_table(0), 0);
        assert_eq!(sine_table(90), 999);
        assert_eq!(sine_table(180), 0);
        assert_eq!(sine_table(270), -999);
    }

    #[test]
    fn surface_table() {
        assert_eq!(terrain(1, 1), (0, 0, Wave::Sine));
        assert_eq!(terrain(8, 8), (260, 160, Wave::Sine));
        assert_eq!(terrain(9, 1), (110, 80, Wave::Sine));
        assert_eq!(terrain(1, 9), (110, 80, Wave::Sine));
        assert_eq!(terrain(10, 11), (170, 190, Wave::Square));
        assert_eq!(terrain(11, 10), (170, 190, Wave::Square));
        assert_eq!(terrain(15, 15), (200, 80, Wave::Triangle));
        assert_eq!(terrain(8, 16), (80, 100, Wave::Square));
        assert_eq!(terrain(16, 16), (80, 100, Wave::Square));
        assert_eq!(terrain(17, 17), (80, 140, Wave::Square));
        assert_eq!(terrain(16, 1), (80, 100, Wave::Sine));
        assert_eq!(terrain(12, 13), (0, 0, Wave::Sine));
        // Magnitude amp/2 × min(1, v/13) × 39.2157, period per/2 (≤ 150).
        let mut ff = Original::new(WHEEL).unwrap();
        let feed = Feed { front_surface: [16, 16], ..on_road(20.0) };
        let c = ff.driving(&feed, &Options::default(), Clock::default());
        assert!(c.contains(&Command::Set(Effect::Terrain { wave: Wave::Square, magnitude: 1_568, period_ms: 50 })));
        // Observed under Wine on surface 16 with one wheel on it: Sine, 50
        // ms, 1568.
        let feed = Feed { front_surface: [16, 1], ..on_road(20.0) };
        let c = ff.driving(&feed, &Options::default(), Clock::default());
        assert!(c.contains(&Command::Set(Effect::Terrain { wave: Wave::Sine, magnitude: 1_568, period_ms: 50 })));
        assert!(c.contains(&Command::Start(Slot::Terrain)), "a new wave starts again");
        // Half speed, half magnitude.
        let feed = Feed { front_surface: [8, 8], ..on_road(6.5) };
        let c = ff.driving(&feed, &Options::default(), Clock::default());
        assert!(c.contains(&Command::Set(Effect::Terrain { wave: Wave::Sine, magnitude: 2_549, period_ms: 80 })));
    }

    #[test]
    fn puddle_damper() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let c = ff.driving(&Feed { steer: 1.0, ..on_road(30.0) }, &o, Clock::default());
        assert!(ff.playing(Slot::Spring) && ff.playing(Slot::Damper), "{c:?}");
        // A front wheel in a puddle above 23 m/s: damper and spring stop,
        // the negative damper plays.
        let wet = Feed { steer: 1.0, front_surface: [1, SURFACE_PUDDLE], ..on_road(30.0) };
        let c = ff.driving(&wet, &o, Clock::default());
        assert!(c.contains(&Command::Stop(Slot::Damper)) && c.contains(&Command::Stop(Slot::Spring)));
        assert!(c.contains(&Command::Set(Effect::Puddle { coef: 10_000 })));
        assert_eq!(ff.active(), vec![Slot::Puddle, Slot::Terrain]);
        // Spring and damper wait while it plays.
        let c = ff.driving(&wet, &o, Clock::default());
        assert!(c.is_empty(), "{c:?}");
        // At 23 m/s or out of the puddle: it ends, the two come back.
        let c = ff.driving(&Feed { front_surface: [1, SURFACE_PUDDLE], ..on_road(23.0) }, &o, Clock::default());
        assert!(c.contains(&Command::Stop(Slot::Puddle)));
        assert!(c.contains(&Command::Start(Slot::Damper)) && c.contains(&Command::Start(Slot::Spring)));
    }

    #[test]
    fn airborne_suspends_and_resumes() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let road = Feed { front_sideways: 6.2, front_surface: [8, 8], ..on_road(20.0) };
        ff.driving(&road, &o, Clock::default());
        assert_eq!(ff.active(), vec![Slot::Damper, Slot::Constant, Slot::Terrain]);
        // One front wheel down keeps everything.
        let one = Feed { front_down: [false, true], ..road };
        assert!(ff.driving(&one, &o, Clock::default()).is_empty());
        // Both up: all stop.
        let air = Feed { front_down: [false, false], ..road };
        let c = ff.driving(&air, &o, Clock::default());
        assert!(ff.active().is_empty(), "{c:?}");
        assert!(ff.driving(&air, &o, Clock::default()).is_empty());
        // Down again: back, the constant force at level 0 first.
        let c = ff.driving(&road, &o, Clock::default());
        assert_eq!(c[0], Command::Set(Effect::Constant { level: 0 }));
        assert_eq!(ff.active(), vec![Slot::Damper, Slot::Constant, Slot::Terrain]);
    }

    /// Ticks of the 4000 Hz sim clock at 60 fps (67, 64, 69 …).
    fn sim_ticks(n: usize) -> Vec<u32> {
        let steps = [67u32, 64, 69];
        let mut t = 4_000;
        (0..n)
            .map(|i| {
                t += steps[i % 3];
                t
            })
            .collect()
    }

    fn jolts(c: &[Command]) -> Vec<i32> {
        sets(c)
            .into_iter()
            .filter_map(|e| match e {
                Effect::Jolt { magnitude } => Some(magnitude),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn speed_jolt_per_tick_and_spacing() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let ticks = sim_ticks(40);
        let clock = |i: usize| Clock { sim: ticks[i], real_ms: u64::from(ticks[i]) / 4 };
        // The first call jolts from the stored 0 m/s unless the spacing
        // holds it: at sim time 0 it does.
        assert!(jolts(&ff.driving(&on_road(30.0), &o, Clock::default())).is_empty());
        ff.driving(&on_road(30.0), &o, clock(0));
        // A drop of 4 m/s in one tick: nothing; of 4.5: (0.5/15) × 10000.
        let c = ff.driving(&on_road(26.0), &o, clock(1));
        assert!(jolts(&c).is_empty());
        let c = ff.driving(&on_road(21.5), &o, clock(2));
        assert_eq!(jolts(&c), vec![333]);
        assert!(c.contains(&Command::Start(Slot::Jolt)));
        // Speed swings of 100 m/s every tick: one jolt per 0.15 s of sim
        // clock at most.
        let mut fired = vec![];
        for i in 3..30 {
            let v = if i % 2 == 0 { 100.0 } else { 0.0 };
            if ff.driving(&on_road(v), &o, clock(i)).contains(&Command::Start(Slot::Jolt)) {
                fired.push(i);
            }
        }
        // 0.15 s = 600 sim ticks = 9 ticks of 67/64/69, but 600 × 0.00025
        // less the stamp kept as a float is just under the float 0.15: the
        // next jolt comes 10 ticks after tick 2.
        assert_eq!(fired.first(), Some(&12), "{fired:?}");
        assert!(fired.windows(2).all(|w| w[1] - w[0] >= 9), "{fired:?}");
    }

    #[test]
    fn speed_jolt_magnitudes() {
        let o = Options::default();
        let run = |from: f32, to: f32| {
            let mut ff = Original::new(WHEEL).unwrap();
            ff.driving(&on_road(from), &o, Clock { sim: 4_000, real_ms: 1_000 });
            jolts(&ff.driving(&on_road(to), &o, Clock { sim: 8_000, real_ms: 2_000 }))
        };
        assert_eq!(run(30.0, 11.0), vec![10_000]);
        assert_eq!(run(30.0, 19.0), vec![4_666]);
        // A rise: (Δ − 4)/15 with Δ negative, not clamped below.
        assert_eq!(run(10.0, 29.0), vec![-15_333]);
    }

    #[test]
    fn bump_jolt() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let c =
            ff.driving(&Feed { bump: [0.01, 0.01, 0.01, 0.02], ..on_road(0.0) }, &o, Clock { sim: 8_000, real_ms: 0 });
        assert!(jolts(&c).is_empty(), "0.05 is not over 0.05");
        let c = ff.driving(&Feed { bump: [1.0, 0.0, 0.0, 0.0], ..on_road(0.0) }, &o, Clock { sim: 8_000, real_ms: 0 });
        assert_eq!(jolts(&c), vec![1_666]);
        let c =
            ff.driving(&Feed { bump: [9.0, 0.0, 0.0, 0.0], ..on_road(0.0) }, &o, Clock { sim: 8_700, real_ms: 200 });
        assert_eq!(jolts(&c), vec![6_666]);
        // The sim clock behind the last jolt (a world load): it jolts and
        // the stamp goes to 0.
        let c =
            ff.driving(&Feed { bump: [2.0, 0.0, 0.0, 0.0], ..on_road(0.0) }, &o, Clock { sim: 4_000, real_ms: 400 });
        assert_eq!(jolts(&c), vec![3_333]);
        // Within 150 ms of wall clock: nothing even with the sim gap.
        let c =
            ff.driving(&Feed { bump: [2.0, 0.0, 0.0, 0.0], ..on_road(0.0) }, &o, Clock { sim: 9_000, real_ms: 500 });
        assert!(jolts(&c).is_empty() && !c.contains(&Command::Start(Slot::Jolt)));
    }

    #[test]
    fn jolt_plays_while_suspended() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let air = Feed { front_down: [false, false], bump: [1.0, 0.0, 0.0, 0.0], ..on_road(5.0) };
        let c = ff.driving(&air, &o, Clock { sim: 8_000, real_ms: 0 });
        assert!(c.contains(&Command::Start(Slot::Jolt)), "{c:?}");
    }

    #[test]
    fn feedback_off_set() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        let rough = Feed { front_sideways: 6.2, front_surface: [8, 8], ..on_road(20.0) };
        ff.driving(&rough, &o, Clock::default());
        assert_eq!(ff.active(), vec![Slot::Damper, Slot::Constant, Slot::Terrain]);
        let off = Options { feedback: false, ..o };
        let c = ff.driving(&rough, &off, Clock::default());
        assert_eq!(
            c,
            vec![
                Command::Stop(Slot::Constant),
                Command::Stop(Slot::Terrain),
                Command::Stop(Slot::Damper),
                Command::Set(Effect::Spring { coef: SPRING_FEEDBACK_OFF }),
                Command::Start(Slot::Spring),
            ]
        );
        // A drag race runs the same way, and no jolt fires.
        let mut ff = Original::new(WHEEL).unwrap();
        ff.driving(&on_road(40.0), &o, Clock { sim: 4_000, real_ms: 0 });
        let c = ff.driving(&Feed { drag: true, ..on_road(10.0) }, &o, Clock { sim: 9_000, real_ms: 2_000 });
        assert_eq!(
            c,
            vec![
                // The terrain plays on plain road too, at magnitude 0.
                Command::Stop(Slot::Terrain),
                Command::Stop(Slot::Damper),
                Command::Set(Effect::Spring { coef: 7_058 }),
                Command::Start(Slot::Spring)
            ]
        );
        // The speed is not kept with Feedback off: back on, the drop from
        // 40 counts.
        let c = ff.driving(&on_road(10.0), &o, Clock { sim: 14_000, real_ms: 4_000 });
        assert_eq!(jolts(&c), vec![10_000]);
    }

    #[test]
    fn front_end_turns_the_driving_effects_off() {
        let o = Options::default();
        let mut ff = Original::new(WHEEL).unwrap();
        ff.driving(&Feed { front_sideways: 6.2, front_surface: [8, 8], ..on_road(20.0) }, &o, Clock::default());
        let c = ff.front_end();
        assert_eq!(
            c,
            vec![
                Command::Stop(Slot::Constant),
                Command::Stop(Slot::Terrain),
                Command::Stop(Slot::Damper),
                Command::Set(Effect::Spring { coef: SPRING_FRONT_END }),
                Command::Start(Slot::Spring),
            ]
        );
        assert!(ff.front_end().is_empty());
    }
}
