//! The player car's road noise, wind and bottom-out sounds
//! (CARSFX_RoadNoise, CARSFX_WindNoise, CARSFX_WindWeather,
//! CARSFX_BottomOut), each through its bank's AEMS graph or the stitch
//! records, with the mix map's levels and azimuths
//! ([`super::mixmap::Levels`] object classes 6, 7, 10 and 11).
//!
//! * **RoadNoise** (vtable 0x78ACF8; attach 0x472E10: SFXCTL_Wheel →
//!   +0xB0; bank 0x4658F0 `ROADNOISE_00_MB.abk`). Start (0x472E50): two
//!   FX_ROADNOISE events (+0xB8, +0xBC) with the block {4, 0 …}. Update
//!   (0x47CD90): 0x4735E0 (owner kind 1 only, else all 0) with mph =
//!   car +0x430 × 2.237: B = trunc(graph(mph)) × 0x7FFF >> 15 (graph
//!   0x82C69C, points 0x7F09B0: (0, 0), (60, 28000), (100, 32500), (150,
//!   24500), (175, 18000)); r(x, y) = min(|(x, y)| × 0.01, 0.15); the
//!   right volume +0xFC = 1.1 · (1 + r(W90, W94)) · B, the left +0x100 =
//!   (1 + r(W98, W9C)) · (1 + min((WAC + WA0) · 0.05, 0.1)) · B (each at
//!   most 32000; the right one's spin term is the constant 0.1, as
//!   coded); O = ramp(mph: 0..100 → 1500..4500) (0x7F09D8); the left
//!   p2 +0x108 = O · (1 + min(|W98, W9C| · 0.01, 0.2)) · (1 + min((WAC +
//!   WA0) · 0.075, 0.15)), the right +0x104 likewise with W90 / W94 and
//!   WA8 + WA4 (each at most 6000). Then 0x45FAA0: the last surfaces
//!   +0xE0 / +0xE4, the change flags +0xE8 / +0xE9, the surfaces +0xD8 /
//!   +0xDC = Wheel +0xB0 / +0xB4.
//!   Per frame (0x47CDB0), on a side's change: the class 0x45F940
//!   (surface 1, 3 → 5; 2, 13 → 4; 7, 26 → 7; 8, 10, 15 → 0; 11, 12, 17 →
//!   2; 14 → 3; 16 → 1; 25 → 6; else −1) → +0xF8 / +0xF4: −1 releases the
//!   side's FX_ROADNOISE (+0xC4 left, +0xC0 right), else 0x45FB00 starts
//!   it with {class, 0, 0x1000, 0 …}; with the side's wheels down, a
//!   transition (0x4731C0) of variant 1 when the last surface was 11 or
//!   16, else of 0x45F9D0(surface) (11, 16 → 0; 14 → 2; else none). The
//!   surface events get p1 = level × ((T[class] × the side's volume) >>
//!   15) >> 15 (T = 0x7ECF4C: 15000, 24000, 32000, 24000, 24000, 5000,
//!   19000, 32500; 0 with the side's wheels up), p2 = the side's +0x108 /
//!   +0x104 (0..0x1FFF), p3 the azimuth, p5 = 1 on surfaces 8 and 9; the
//!   two base events p1 = level × ((+0x100 × T[4]) >> 15) >> 15 with the
//!   left / right p2. Levels / azimuths: left surface slot 1, right 0;
//!   +0xBC slot 5, +0xB8 slot 4; transitions 3 / 2; thumps 7 / 6.
//!   Transition (0x4731C0): v = {variants 0, 1: 12000; 2, 3: 0x5FB4; else
//!   0x7FFF} × trunc(ramp(mph: 12..60 → 20000..32767)) >> 15 (0x7F0978);
//!   FX_ROADNOISE_TRANS (releasing the last) {variant, level × v >> 15,
//!   trunc(ramp(mph: 12..60 → 3600..4400)) (0x7F0994), azimuth, 1}; and,
//!   when none runs, the stitch thump: collision record 0x4E + (counter
//!   & 3) (counters 0x82CB44 right, 0x82CB48 left).
//! * **WindNoise** (vtable 0x78ADB8; bank 0x465F60 `WIND_00_MB.abk`):
//!   start (0x45FE80) FX_WIND {0, 0, 0x1000, 0 …}; update (0x45FFB0): v =
//!   car +0x430 below 2 → 2, above 40 → 39.95, t = v / 40: +0x19C =
//!   trunc(1040 t), +0x190 = trunc(32767 t); per frame (0x473970): p1 /
//!   p4 = 0x7FFF × ((level[0 / 1] × +0x190) >> 15) >> 15, p2 = 0x1000,
//!   p3 / p5 the azimuths, p6 = +0x19C (0..0x410). The update also moves
//!   the left / right wind position objects (classes 13 / 14) to the car
//!   ± trunc(1280 + 19200 t) (16-bit angle) at radius 65 (1 − t); no
//!   installed mix map reads them, so they are not modelled.
//! * **WindWeather** (vtable 0x78AE00; bank 0x4664C0 WIND_00): start
//!   (0x460680, from the update 0x473A10 outside game flow 1)
//!   FX_WIND_Weather {0, 0, 0, 0}; +0xB4 = ramp(mph: 0..70 → 1.0..0.25)
//!   (0x7F09F4); per frame (0x460770): p0 = trunc(level[0] × +0xB4), p2 =
//!   4000, p3 = 0.
//! * **BottomOut** (vtable 0x78B4E8; 0x47ED80 / 0x47F100): the car's
//!   wheel pairs (0, 1), (2, 3), (1, 2), (0, 3) with "was down" flags
//!   and timers (set to the clock when the pair leaves the ground, then
//!   + dt each frame). The tipped flag +0xDC: set when the car's up z <
//!   0.9 (0x7ED564) with a wheel up, cleared with all four down. A pair
//!   (0, 1) or (2, 3) landing (down, not down last frame, timer > 0.05)
//!   untipped plays the knock with flag 0; tipped, any pair landing plays
//!   it with flag 1. The knock (0x47EA30): the volume v = Σ ramp(|wheel
//!   compression / travel|: 0.4..1 → 0.4..1) × 31.75 (0x78BC78; car
//!   +0x28 = 1, else 127), at most 127; the collision record 0x4E +
//!   (counter & 3) + 4 · trunc(v / 64) (flag 0, counter 0x82CBF4) or
//!   0x56 + (counter & 3) + 4 · trunc(v / 128) (flag 1, 0x82CBF8),
//!   replacing the last; +4 := 1. Per frame its level and azimuth are
//!   slot 0's. Under the crash / jump camera (0x459AC0: the nearest camera
//!   director, its event kind +0x58 = 6) a second knock (+0xB8, slot 2):
//!   record 0x2E + (0x82CBF0 & 3) − 4 · trunc(Rand_Float(127) · −1/64).
//!   **Not ported**: 0x47EC60 (the context's +0xD8 re-trigger).
//!
// prov: inferred audio.roadwind.module
//! **Inferred**: the wheel compression over the suspension's maximum
//! compression as 0x47ED80's ratio (car +0x34 → +0x1C over the tuning's
//! +0x184), objects updated before their per-frame dispatch, the pause
//! gates (0x82B884 +0x13 / +0x14) off.

use crate::audio::carfx::{CarFxSounds, EventBank, Sink, close_bank};
use crate::audio::mixmap::Levels;
use crate::audio::skids::WheelCtl;
use crate::audio::stitch::StitchBanks;
use road::Road;
use std::collections::HashMap;
use std::path::Path;

mod bottom_out;
mod rain;
mod road;
mod splash;
mod wind;

pub(crate) use bottom_out::*;
pub use rain::*;
pub use road::*;
pub use splash::*;

/// 0x4599E0: y0 + (y1 − y0) · clamp((x − x0) / (x1 − x0)), y0 at or below
/// x0.
pub fn ramp(y0: f32, y1: f32, x0: f32, x1: f32, x: f32) -> f32 {
    let f = (x - x0) / (x1 - x0);
    if f > 0.0 { (y1 - y0) * f.min(1.0) + y0 } else { y0 }
}

/// The player's inputs this frame.
#[derive(Clone, Debug)]
pub struct EnvInput {
    /// Car +0x430 (m/s).
    pub speed: f32,
    /// SFXCTL_Wheel (from the skid controllers; `None` before the first
    /// frame).
    pub wheel: Option<WheelCtl>,
    /// Wheels on the ground (FL, FR, RR, RL: car +0x390..+0x393).
    pub down: [bool; 4],
    /// The car's up vector's z (+0xB8).
    pub up_z: f32,
    /// Each wheel's compression over its travel (**inferred**, see the
    /// module notes).
    pub compression: [f32; 4],
    /// The mix map's slots for this car.
    pub mix: Levels,
    /// The running event kind (+0x58) of the camera director nearest the
    /// car (0x459AC0: the only one with one player); 6 is the crash / jump
    /// camera (`event_camera`).
    pub event_kind: u8,
    /// The weather object's rain (+0x1F0, `Weather::amount`; 0 dry):
    /// CARSFX_Rain ([`Rain`]).
    pub rain: f32,
    /// A NIS stream is queued or playing (0x46C9A0 on the speech
    /// object's +8): the thunder waits.
    pub stream_busy: bool,
}

/// The four objects of the player's car.
pub struct EnvSfx {
    sounds: CarFxSounds,
    gains: HashMap<u32, (f32, f32, f32)>,
    stitches: Option<StitchBanks>,
    road_bank: Option<EventBank>,
    wind_bank: Option<EventBank>,
    road: Road,
    wind: Option<u32>,
    weather: Option<u32>,
    bottom: Bottom,
    clock: f32,
    /// Stitch thumps and knocks started this frame (for the self-test).
    pub thumps: usize,
    pub knocks: usize,
    rain_bank: Option<EventBank>,
    pub rain: Rain,
    /// The water drums' splashes: their three FX_ROADNOISE_TRANS requests
    /// (+0x14C, +0x150, +0x154) and the level they are sent.
    splashes: Vec<(u64, [Option<u32>; 3], i32)>,
    /// The last splash's id.
    splash_ids: u64,
    /// Splashes started so far (for the self-test).
    pub splashed: usize,
}

impl EnvSfx {
    pub fn new(root: &Path) -> EnvSfx {
        EnvSfx {
            sounds: CarFxSounds::new(root),
            gains: HashMap::new(),
            stitches: StitchBanks::load(root),
            road_bank: None,
            wind_bank: None,
            road: Road::default(),
            wind: None,
            weather: None,
            bottom: Bottom::default(),
            clock: 0.0,
            thumps: 0,
            knocks: 0,
            rain_bank: None,
            rain: Rain::default(),
            splashes: Vec::new(),
            splash_ids: 0,
            splashed: 0,
        }
    }

    /// Whether FX_WIND is running (the player's wind object).
    pub fn wind_on(&self) -> bool {
        self.wind.is_some()
    }

    /// CARSFX_BottomOut +4 (mix map field 0): 1 on the frame a knock
    /// starts.
    pub fn bottom_out_field(&self) -> i32 {
        self.bottom.started as i32
    }

    /// Everything stops (leaving the road).
    pub fn silence(&mut self, sink: &mut dyn Sink) {
        for t in self.road.thump.iter_mut().chain([&mut self.bottom.knock, &mut self.bottom.knock2]) {
            if let Some(mut v) = t.take() {
                v.stop(sink);
            }
        }
        close_bank(&mut self.road_bank, &mut self.sounds, &mut self.gains, sink);
        close_bank(&mut self.wind_bank, &mut self.sounds, &mut self.gains, sink);
        close_bank(&mut self.rain_bank, &mut self.sounds, &mut self.gains, sink);
        self.rain = Rain { rng: self.rain.rng.clone(), ..Default::default() };
        self.splashes.clear();
        self.road = Road::default();
        self.bottom = Bottom::default();
        self.wind = None;
        self.weather = None;
    }

    pub fn update(&mut self, sink: &mut dyn Sink, i: &EnvInput, dt: f32) {
        self.clock += dt;
        self.thumps = 0;
        self.knocks = 0;
        self.road_update(sink, i, dt);
        self.wind_update(sink, i, dt);
        self.bottom_update(sink, i, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CARSFX_TrafficBottomOut: a traffic car (volume 127, any mover but
    /// the physics one) knocks when a wheel pair lands after more than
    /// 0.05 s in the air (0x47ED80), record 0x4E + n + 4 (0x47EA30).
    #[test]
    fn traffic_bottom_out_knocks_on_landing() {
        struct Null;
        impl Sink for Null {
            fn one_shot(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) {}
            fn start_loop(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn gain(&mut self, _: u32, _: f32) {}
            fn end(&mut self, _: u32) {}
            fn start_once(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
                1
            }
            fn sounding(&mut self, _: u32) -> bool {
                true
            }
            fn pitch(&mut self, _: u32, _: u16) {}
            fn pan(&mut self, _: u32, _: u16) {}
        }
        let mut bo = Bottom::default();
        let input = |down| BottomIn { down, up_z: 1.0, volume: 127.0, event_kind: 0, levels: [(0, 0); 2] };
        let mut clock = 0.0;
        let mut step = |bo: &mut Bottom, down| {
            clock += 1.0 / 60.0;
            bottom_out(bo, None, &mut Null, input(down), clock, 1.0 / 60.0);
            bo.started
        };
        assert!(!step(&mut bo, [true; 4]));
        for _ in 0..6 {
            assert!(!step(&mut bo, [false; 4]));
        }
        assert!(step(&mut bo, [true; 4]));
        assert_eq!(bo.counter[0], 1);
        // Staying down does not knock again.
        assert!(!step(&mut bo, [true; 4]));
    }

    #[test]
    fn road_curves() {
        assert_eq!(road_graph(30.0), 14000.0);
        assert_eq!(road_graph(200.0), 18000.0);
        assert_eq!(ramp(1500.0, 4500.0, 0.0, 100.0, 50.0), 3000.0);
        assert_eq!(ramp(20000.0, 32767.0, 12.0, 60.0, 5.0), 20000.0);
        let w = WheelCtl::default();
        // 60 mph, no slip: B = 28000 · 0x7FFF >> 15.
        let ((l, r), (lp, rp)) = {
            let (a, b) = road_levels(60.0 / 2.237, &w);
            ((a[0], a[1]), (b[0], b[1]))
        };
        let b = (28000 * 0x7FFF) >> 15;
        assert!((l - b).abs() <= 1 && (r - (b as f32 * 1.1) as i32).abs() <= 2, "{l} {r}");
        assert!((lp - 3300).abs() <= 1 && (rp - 3300).abs() <= 1, "{lp} {rp}");
        assert_eq!((surface_class(14), surface_transition(14), surface_class(99)), (3, 2, -1));
    }
}
