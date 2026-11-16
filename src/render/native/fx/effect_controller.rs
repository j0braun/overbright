//! The NIS SCREEN_FLASH event: the white flashes of the race-start scenes
//! (IntroNis05 / 10 / 22 / 24–27), drawn by the tint pass.
//!
//! Traced in SPEED2.EXE (docs/nis-mover.md "SCREEN_FLASH"):
//!
//! * Parser 0x432D00: the name 0x785E48 maps to event type 10 (0x430B20);
//!   the line's two values go to +0x08 (int) and +0x14 (float). Every
//!   shipped line reads `SCREEN_FLASH 1 1.0f`.
//! * The scene update 0x4395F0 adds its dt to the scene clock (+0x0C) and
//!   dispatches (0x4392E0) every event whose time × 1/30 (0x033333335) is
//!   below it. Type 10 ignores both values: for views 1 and 2 with the
//!   view active (0x48B1E0(i) +0x08), 0x5DBD80(1) on the view's effect
//!   controller (+0x60) applies descriptor 1 (0x8769DC, built by 0x5DBDE0:
//!   one record, controller slot 0, callback 0x5D7A70, table 0, state 3)
//!   through 0x5D7730 → 0x5D75F0.
//! * 0x5D75F0 copies the record into slot 0, keeps the slot's step (+0x08)
//!   unless it is 6 or more (then 0), enables the slot and calls 0x5D7A70,
//!   which sets the colour from the tables at 0x800E54.. for the step:
//!   amount (+0x80) = 1, 1, 1, 0.7, 0.3, 0.1 (0x800E54), red / green / blue
//!   255 (0x800E6C / 0x800E84 / 0x800E9C), +0x7C = 128; from step 6 the
//!   amount is 0 and the slot is off.
//! * Twice a drawn frame 0x5DBE90 → 0x5D7590(1/30) steps an enabled slot
//!   in state 3 (+0x08 += 1) and calls the callback again: at the end of
//!   the screen passes (0x5CBAEC, in 0x5CAAE0) and after the present
//!   (0x5D2C31: Render_Present runs it whether or not the device was lost;
//!   the `jne` at 0x5D2BDE only skips the reset). So the frame the event
//!   fires in draws step 0, the next two steps 2 and 4, and the slot is off
//!   from step 6: three drawn frames, amounts 1, 1, 0.3 — 0.1 s at the
//!   scenes' 30 frames a second. **Observed** under Wine (IntroNis05,
//!   the maintainer's private captures): the step reads 0, 2, 4, 6 on four
//!   consecutive frame counts (0x865148, one per present), the controller's
//!   update count (+0x3AC × 30) goes up 2 per frame, and the grabbed frames
//!   brighten × 2.1, × 2.1, × 1.4 for three frames at each flash.
//! * The tint pass 0x5C7870 (only with g_TintingEnable 0x870D04, set at
//!   0x5CB7D9 when slot 0 or slot 2 is on) takes slot 0's colour and amount
//!   over the rain's slot 2 and multiplies the frame by
//!   `2·⌊((⌊c⌋/128 − 1)·amount + 1)·127.5⌋ / 255` per channel (DESTCOLOR /
//!   SRCCOLOR blend), [`crate::weather::tint_multiplier`]: × 1.99 at
//!   amount 1, × 1.29 at 0.3 — a brightening of the scene, not a white
//!   overlay. (Steps 1, 3 and 5 — 1, 0.7, 0.1 — are never drawn.)
//!
//! The clone steps the flash by scene frames (1/30 s of game time, two
//! steps each) where the original steps it by drawn frames; the race-start
//! scenes are drawn at 30 frames a second by the scene limiter (0x435F40,
//! `SCENE_FRAME`), so the two agree there.
//!
//! The in-game screen tint: the bleach-bypass amount and grey pull that
//! screenfilter.efx gets each frame, from the world's "Sunrise Screen Tint"
//! and "Night Screen Tint" parameter maps.
//!
//! Traced in SPEED2.EXE:
//!
//! * Every frame 0x5DBE90 (called from the frame function at 0x5CBAEC, after
//!   the screen filters, so the values are used on the next frame) walks
//!   views 1 and 2 and, for each active view, applies the effect descriptor
//!   0x876B00 to the view's effect controller (`*(view + 0x60)`) through
//!   0x5D7730 → 0x5D75F0. The descriptor (filled by 0x5DBDE0) names slot 3,
//!   state 2 and the callback 0x5D7800. 0x5D75F0 sets the slot's enable
//!   (`controller + 3·12 + 0x0C` = +0x30), calls the callback, then clears
//!   the enable again when the slot's amount (+0x188) is below 0.01
//!   (0x78425C) or NaN.
//! * 0x5D7800 reads the view's camera position (0x5F0C60: camera +0x40,
//!   +0x44), smooths each map's row on its own with 0x5FA670 over 50 units
//!   (0x7A47FC; the map objects are 0x88EA08 / 0x88F210 + 0x30·(view − 1)),
//!   reads fields 0–6 (0x5F0B50) and blends `sunrise·w + night·(1 − w)` with
//!   `w = 0x60FA90()` (the time-of-day weight, always 1 in the shipped game;
//!   see [`crate::sky`]). It writes slot 3's record (`controller + 0x70 +
//!   3·0x58`): +0x178…+0x180 = f0…f2, +0x184 = 128, +0x188 = f3,
//!   +0x1C4…+0x1CC = f4…f6. A map that did not load (+0x0C = 0) gives zeros.
//! * 0x5CB91B: when +0x30 is set, the bleach amount (FILTERBLEND) is +0x188
//!   and TINTCOLOR.w (0x7FF188) is +0x1CC; TINTCOLOR.rgb is +0x178…+0x180
//!   / 255 (0x784548). Otherwise the amount is 0 and TINTCOLOR keeps its
//!   last value (the front end's 0.2, 0x5CB8E6).
//!
//! Fields f4 and f5 (+0x1C4, +0x1C8) are not read by the screen filters
// prov: open screen_tint.module
//! (**open**: their reader, if any).

use glam::Vec2;

use crate::formats::hash::bin_hash;
use crate::world::World;

/// 0x800E54: slot 0's amount by step (table 0 of 0x5D7A70).
pub const RAMP: [f32; 6] = [1.0, 1.0, 1.0, 0.7, 0.3, 0.1];
/// 0x800E6C / 0x800E84 / 0x800E9C: the colour by step (all 255).
pub const COLOR: [f32; 3] = [255.0; 3];

/// The SCREEN_FLASH lines of an event track: their times in scene frames.
pub fn parse(track: &str) -> Vec<f32> {
    let mut out: Vec<f32> = track
        .lines()
        .filter_map(|line| {
            let w: Vec<_> = line.split("//").next().unwrap_or_default().split_whitespace().collect();
            (w.len() >= 3 && w[0] == "EVENT:" && w[2] == "SCREEN_FLASH")
                .then(|| w[1].trim_end_matches('f').parse::<f32>().ok())
                .flatten()
        })
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

/// 0x5D7590 runs twice a drawn frame (0x5CBAEC and 0x5D2C31).
pub const STEPS_PER_FRAME: f32 = 2.0;

/// Controller slot 0 as the flash drives it: its step (0x5D7A70's table
/// index, fractional between drawn frames) since it last restarted, or None
/// while off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenFlash {
    age: Option<f32>,
}

impl ScreenFlash {
    /// The scene clock went from `was` to `now` (scene frames): the flashes
    /// in between fire (0x4395F0: time < clock); time passes first, as the
    /// previous frame's end stepped the slot before this update.
    pub fn advance(&mut self, flashes: &[f32], was: f32, now: f32) {
        self.step(now - was);
        if flashes.iter().any(|&t| t >= was && t < now) {
            self.fire();
        }
    }

    /// 0x5D75F0 on slot 0: the step restarts unless a flash is still on.
    pub fn fire(&mut self) {
        if !self.age.is_some_and(|a| a < RAMP.len() as f32) {
            self.age = Some(0.0);
        }
    }

    /// `frames` scene frames pass (0x5D7590 twice per drawn frame).
    pub fn step(&mut self, frames: f32) {
        if let Some(a) = self.age.as_mut() {
            *a += frames.max(0.0) * STEPS_PER_FRAME;
            if *a + 1e-3 >= RAMP.len() as f32 {
                self.age = None;
            }
        }
    }

    /// Slot 0's amount now (+0x80; 0 when off).
    pub fn amount(&self) -> f32 {
        // The step drawn: the even one reached (the two updates land
        // together at a frame's end).
        self.age.map_or(0.0, |a| {
            RAMP[(((a + 1e-3) / STEPS_PER_FRAME) as usize * STEPS_PER_FRAME as usize).min(RAMP.len() - 1)]
        })
    }

    /// The tint pass's multiplier for this frame: None when the slot is off;
    /// Reduced Flashing (an opt-in deviation, `extras`) caps the amount.
    pub fn multiplier(&self) -> Option<[f32; 3]> {
        let a = self.amount();
        if a <= 0.0 {
            return None;
        }
        let a = if crate::extras::reduced_flashing() { a.min(crate::extras::FLASH_AMOUNT) } else { a };
        crate::weather::tint_multiplier(COLOR, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_intro_tracks_lines() {
        let t = "EVENT: 35.0f    CAR_AUDIO_PATTERN     2       10       1.0f\r\nEVENT: 36.0      SCREEN_FLASH\t     1               1.0f\r\n// EVENT: 50.0 SCREEN_FLASH 1 1.0f\r\nEVENT: 200.0      SCREEN_FLASH\t     1               1.0f\r\n";
        assert_eq!(parse(t), vec![36.0, 200.0]);
    }

    /// Three drawn frames from the event's frame on: steps 0, 2, 4 — 1, 1,
    /// 0.3 — as multipliers 2·254/255, 2·254/255, 2·165/255 (0x5C7870's
    /// rounding); the Wine poll read 0, 2, 4, 6.
    #[test]
    fn a_flash_draws_three_scene_frames() {
        let mut f = ScreenFlash::default();
        let flashes = [36.0];
        let mut seen = Vec::new();
        for n in 30..45 {
            f.advance(&flashes, n as f32, (n + 1) as f32);
            seen.push(f.amount());
        }
        assert_eq!(seen, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let mut f = ScreenFlash::default();
        let mut m = Vec::new();
        f.fire();
        for _ in 0..3 {
            m.push((f.multiplier().unwrap()[0] * 255.0 / 2.0).round() as i32);
            f.step(1.0);
        }
        assert_eq!(m, [254, 254, 165]);
        assert_eq!(f.multiplier(), None);
    }

    /// At 60 updates a second each scene frame takes two updates: the same
    /// three frames' worth of time (0.1 s).
    #[test]
    fn the_flash_follows_scene_time_not_updates() {
        let mut f = ScreenFlash::default();
        let mut on = 0;
        let mut clock = 30.0f32;
        for _ in 0..40 {
            let next = clock + 0.5;
            f.advance(&[36.0], clock, next);
            clock = next;
            on += usize::from(f.amount() > 0.0);
        }
        assert_eq!(on, 6);
    }

    /// Reduced Flashing (opt-in) holds the amount to 0.25: × 1.25 instead of
    /// × 1.99; off, the native multiplier.
    #[test]
    fn reduced_flashing_caps_the_flash() {
        let _g = crate::qol::test_globals();
        let mut f = ScreenFlash::default();
        f.fire();
        assert_eq!(f.multiplier().unwrap()[0], 2.0 * 254.0 / 255.0);
        crate::extras::Extras { reduced_flashing: true, ..Default::default() }.apply();
        let m = f.multiplier().unwrap()[0];
        assert_eq!(m, 2.0 * 159.0 / 255.0);
        f.step(2.0);
        assert_eq!(f.multiplier().unwrap()[0], m, "0.3 is capped to 0.25 as well");
    }

    /// A second flash while one is on keeps the step (0x5D75F0 resets it
    /// only from 6 on).
    #[test]
    fn a_running_flash_is_not_restarted() {
        let mut f = ScreenFlash::default();
        f.fire();
        f.step(2.0);
        f.fire();
        assert_eq!(f.amount(), 0.3);
    }
}

/// 0x7A47FC: the distance over which 0x5FA670 blends to a new sample.
const BLEND_DISTANCE: f32 = 50.0;
/// 0x78425C: 0x5D75F0 disables the slot below this amount.
const MIN_AMOUNT: f32 = 0.01;
/// TINTCOLOR.w as the front end leaves it (0x5CB8E6).
const FRONT_END_GREY: f32 = 0.2;

const MAPS: [&str; 2] = ["Sunrise Screen Tint", "Night Screen Tint"];

/// One frame's screen-tint values (slot 3 of the main view's controller).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenTint {
    /// +0x30: whether the bleach bypass gets this record's amount.
    pub enabled: bool,
    /// +0x188 (f3): FILTERBLEND, the bleach bypass amount (0 when disabled).
    pub bleach: f32,
    /// +0x1CC (f6): TINTCOLOR.w, the grey pull of the bleach passes.
    pub grey: f32,
    /// +0x178…+0x180 (f0…f2) / 255: TINTCOLOR.rgb (read only by
    /// screenfilter.efx's second technique).
    pub color: [f32; 3],
}

/// Smoothed samples that follow the camera, one per map (0x5FA670).
#[derive(Clone)]
pub struct ScreenTintState {
    current: [Option<[f32; 7]>; 2],
    last_position: [Vec2; 2],
    /// 0x7FF188 persists between frames: a disabled slot leaves it alone.
    grey: f32,
}

#[cfg(feature = "dev-harness")]
impl ScreenTintState {
    /// Dev harness (rate gate, `state_hash` `vis`): the smoothed rows and the grey as bits.
    pub fn dev_bits(&self) -> Vec<u32> {
        let mut v: Vec<u32> =
            self.current.iter().flat_map(|r| r.map_or([u32::MAX; 7], |r| r.map(f32::to_bits))).collect();
        v.push(self.grey.to_bits());
        v
    }
}

#[cfg(test)]
impl ScreenTintState {
    /// The whole state as bits (tests).
    fn dev_bits_raw(&self) -> Vec<u32> {
        let mut v: Vec<u32> =
            self.current.iter().flat_map(|r| r.map_or([u32::MAX; 7], |r| r.map(f32::to_bits))).collect();
        v.extend(self.last_position.iter().flat_map(|p| [p.x.to_bits(), p.y.to_bits()]));
        v.push(self.grey.to_bits());
        v
    }
}

impl Default for ScreenTintState {
    fn default() -> Self {
        ScreenTintState { current: [None; 2], last_position: [Vec2::ZERO; 2], grey: FRONT_END_GREY }
    }
}

impl ScreenTintState {
    /// Sample at the camera's ground `position`; `sunrise_weight` is
    /// 0x60FA90's factor. The per-frame path over the tick form: one
    /// [`ScreenTintState::step`], then the draw at α = 0.
    pub fn update(&mut self, world: &World, position: Vec2, sunrise_weight: f32) -> ScreenTint {
        let s = self.step(world, position, sunrise_weight);
        TintSample::drawn(&s, &s, 0.0)
    }

    /// Tick form (frame-rate independence): one tick of 0x5D7800 at the **tick**
    /// camera's ground `position` (each map smoothed over 50 units by
    /// t = min(|p_n − p_{n−1}| / 50, 1)). -> S_n, what tick n's frame draws.
    pub fn step(&mut self, world: &World, position: Vec2, sunrise_weight: f32) -> TintSample {
        let mut rows = [[0.0f32; 7]; 2];
        for (i, name) in MAPS.iter().enumerate() {
            let Some(map) = world.parameter_maps.get(&bin_hash(name)) else { continue };
            let row = map.sample(position);
            let target: [f32; 7] = std::array::from_fn(|k| row.get(k).copied().unwrap_or(0.0));
            let current = match self.current[i] {
                None => target,
                Some(old) => {
                    let moved = position.distance(self.last_position[i]);
                    let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            self.current[i] = Some(current);
            self.last_position[i] = position;
            rows[i] = current;
        }
        let w = sunrise_weight;
        let f: [f32; 7] = std::array::from_fn(|k| rows[1][k] * (1.0 - w) + rows[0][k] * w);
        // 0x5D7701: the slot stays on unless the amount is below 0.01 or NaN.
        if f[3] >= MIN_AMOUNT {
            self.grey = f[6];
        }
        TintSample { f, grey: self.grey }
    }
}

/// The screen tint's part of a tick snapshot: slot 3's blended fields
/// f0…f6 and 0x7FF188 (TINTCOLOR.w as it persists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TintSample {
    pub f: [f32; 7],
    pub grey: f32,
}

impl TintSample {
    /// Slot 3 before 0x5D7800 has run for this world (its first tick: the
    /// values are written after the screen passes, 0x5CBAEC): zeros, so the
    /// slot is off, and TINTCOLOR.w at the front end's 0.2 (0x5CB8E6).
    pub const UNSAMPLED: TintSample = TintSample { f: [0.0; 7], grey: FRONT_END_GREY };

    /// The values a frame at α between ticks `prev` (S_{n−1}) and `cur`
    /// (S_n) draws: the fields and the grey interpolated (`prev` exactly at
    /// α = 0), then 0x5D7701's enable test and the /255 on them.
    pub fn drawn(prev: &TintSample, cur: &TintSample, alpha: f32) -> ScreenTint {
        use super::tick::{lerp, lerp_n};
        let f = lerp_n(prev.f, cur.f, alpha);
        let enabled = f[3] >= MIN_AMOUNT;
        ScreenTint {
            enabled,
            bleach: if enabled { f[3] } else { 0.0 },
            grey: lerp(prev.grey, cur.grey, alpha),
            color: [f[0] / 255.0, f[1] / 255.0, f[2] / 255.0],
        }
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;
    use crate::render::native::fx::tick::{testkit, within};

    /// The per-frame code before the tick form (verbatim), the oracle.
    fn legacy_update(s: &mut ScreenTintState, world: &World, position: Vec2, sunrise_weight: f32) -> ScreenTint {
        let mut rows = [[0.0f32; 7]; 2];
        for (i, name) in MAPS.iter().enumerate() {
            let Some(map) = world.parameter_maps.get(&bin_hash(name)) else { continue };
            let row = map.sample(position);
            let target: [f32; 7] = std::array::from_fn(|k| row.get(k).copied().unwrap_or(0.0));
            let current = match s.current[i] {
                None => target,
                Some(old) => {
                    let moved = position.distance(s.last_position[i]);
                    let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            s.current[i] = Some(current);
            s.last_position[i] = position;
            rows[i] = current;
        }
        let w = sunrise_weight;
        let f: [f32; 7] = std::array::from_fn(|k| rows[1][k] * (1.0 - w) + rows[0][k] * w);
        let enabled = f[3] >= MIN_AMOUNT;
        if enabled {
            s.grey = f[6];
        }
        ScreenTint {
            enabled,
            bleach: if enabled { f[3] } else { 0.0 },
            grey: s.grey,
            color: [f[0] / 255.0, f[1] / 255.0, f[2] / 255.0],
        }
    }

    fn bits(t: &ScreenTint) -> (bool, u32, u32, [u32; 3]) {
        (t.enabled, t.bleach.to_bits(), t.grey.to_bits(), t.color.map(f32::to_bits))
    }

    /// 600 ticks stepped = 600 frames of the per-frame code, bit for bit
    /// (rows that switch the slot on and off); α = 0 draws the old output;
    /// between ticks every value lies within the two snapshots.
    #[test]
    fn tick_form_is_the_per_frame_path() {
        let sunrise: [&[f32]; 4] = [
            &[200.0, 180.0, 160.0, 0.3, 0.0, 0.0, 0.4],
            &[10.0, 20.0, 30.0, 0.005, 0.0, 0.0, 0.9],
            &[90.0, 90.0, 255.0, 0.76, 1.0, 2.0, 0.12],
            &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let night: [&[f32]; 4] = [
            &[1.0, 2.0, 3.0, 0.5, 0.0, 0.0, 0.2],
            &[100.0, 100.0, 100.0, 0.2, 0.0, 0.0, 0.3],
            &[5.0, 5.0, 5.0, 0.008, 0.0, 0.0, 0.6],
            &[50.0, 60.0, 70.0, 0.9, 0.0, 0.0, 0.05],
        ];
        let world = testkit::world(&[("Sunrise Screen Tint", sunrise), ("Night Screen Tint", night)]);
        for w in [1.0f32, 0.35] {
            let (mut new, mut old) = (ScreenTintState::default(), ScreenTintState::default());
            let mut prev: Option<TintSample> = None;
            for i in 0..600 {
                let p = testkit::path(i);
                let want = legacy_update(&mut old, &world, p, w);
                let mut per_frame = new.clone();
                assert_eq!(bits(&per_frame.update(&world, p, w)), bits(&want), "frame {i}");
                let cur = new.step(&world, p, w);
                assert_eq!(new.dev_bits_raw(), old.dev_bits_raw(), "tick {i}");
                assert_eq!(bits(&TintSample::drawn(&cur, &cur, 0.0)), bits(&want));
                if let Some(prev) = prev {
                    let at0 = TintSample::drawn(&prev, &cur, 0.0);
                    assert_eq!(bits(&at0), bits(&TintSample::drawn(&prev, &prev, 0.0)));
                    for a in testkit::ALPHAS {
                        let d = TintSample::drawn(&prev, &cur, a);
                        for k in 0..3 {
                            assert!(within(d.color[k], prev.f[k] / 255.0, cur.f[k] / 255.0));
                        }
                        assert!(within(d.grey, prev.grey, cur.grey));
                        if d.enabled {
                            assert!(within(d.bleach, prev.f[3], cur.f[3]));
                        }
                    }
                }
                prev = Some(cur);
            }
        }
    }
}
