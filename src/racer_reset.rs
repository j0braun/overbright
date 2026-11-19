//! What the reset key may do (docs/native-fixes/N-CAM.tsv).
//!
//! The key is JOY event 0xA1 in the human player's event handler 0x602BE0
//! (the same handler as SMS 0xC2 and pause 0xBD). It checks no race type or
//! mode: free roam takes the race's path. It calls Racer_ResetCar 0x6009B0
//! directly (not through car +0x96C) when all of these hold (**verified**,
//! 0x60346E..0x6034BA):
//!
//! * no engage under way (0x5012C0 on g_EngageState 0x85EF50 +0x1C clear);
//! * the key is down;
//! * not (car +0x24 == 1 and car +0x30 ≠ 0 and [car +0x30] +0x2B4 ≠ 0) —
//!   the meaning of these fields is **open**; the caller passes the result;
//! * 0x5F3F10 ([`may_reset`]).
//!
//! 0x6009B0 then places the car (kept speed, kind-4 reset zone, the human's
//! road node within 150 m, else the oldest saved place); every placement
//! (0x5FBD70 / 0x5FBEE0 with the stamp flag) stamps car +0x974 = now and
//! +0x978 = [`PLACE_HOLD`].

/// The hold after a placement, s (0x7A55C8 / 0x7A55CC = 2.0, stored in car
/// +0x978 by 0x5FBD70 at 0x5FBDD9 and 0x5FBEE0 at 0x5FC027 → 0x5EBF80).
pub const PLACE_HOLD: f32 = 2.0;

/// The speed over which the key does nothing, mph (0x5F3F10: car +0x430 ×
/// 2.237 must be at most 60).
pub const RESET_MAX_MPH: f32 = 60.0;

/// What 0x5F3F10 reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResetCheck {
    /// The player's event camera kind (player +0x58) when one runs and its
    /// byte of 0x8020E8 is set (a blocking kind); kinds 6 and 7 never block.
    pub event_camera: Option<u32>,
    /// |v|, m/s (car +0x430).
    pub speed: f32,
    /// The race status *0x890118 is 4 (racing), with its clock
    /// (status[5] × 0.00025, s); None outside a race.
    pub racing_clock: Option<f32>,
    /// The race type 0x89E7B0 is 7 (outrun).
    pub outrun: bool,
    /// Seconds since the car's last stamped placement ((0x865188 − car
    /// +0x974) × 0.00025) and that placement's hold (+0x978); None when
    /// never stamped (+0x974 = 0 or 0x7FFFFFFF).
    pub since_placed: Option<(f32, f32)>,
}

/// 0x5F3F10 (**verified**, decompiled): whether the reset is allowed.
/// False while a blocking event camera runs (0x8020E8[kind] set, kinds 6
/// and 7 excepted), above 60 mph, in the first 3 s of race status 4
/// (except race type 7), and within the hold of the last placement.
pub fn may_reset(c: &ResetCheck) -> bool {
    if c.event_camera.is_some_and(|k| k != 6 && k != 7) {
        return false;
    }
    if !(c.speed * 2.237 <= RESET_MAX_MPH) {
        return false;
    }
    if c.racing_clock.is_some_and(|t| t < 3.0) && !c.outrun {
        return false;
    }
    if c.since_placed.is_some_and(|(since, hold)| since < hold) {
        return false;
    }
    true
}

/// The whole key test of 0x602BE0's case 0xA1: the key down, no engage,
/// the controller condition (car +0x24 / +0x30 / +0x2B4, passed in as
/// `held`) clear, and [`may_reset`].
pub fn reset_key(down: bool, engaging: bool, held: bool, c: &ResetCheck) -> bool {
    down && !engaging && !held && may_reset(c)
}

/// The native draw rate the blink of a placed car is tied to, Hz
/// (the frame cap 120: 0x870818 counts drawn frames, so at the cap the
/// car is shown on every other one, 60 Hz on and off).
pub const BLINK_RATE: f64 = 120.0;

/// The draw of a car within the hold of its last stamped placement
/// (**verified**: 0x60D500(car, view), called by the car lists 0x61B234,
/// 0x60D5A4, 0x6317B2 and 0x6154E3, hides the car when 0x5EC000(view)
/// holds, i.e. the car is ghosted by the hold, (0x865188 − +0x974) ×
/// 0.00025 < +0x978, the same test as the collision ghost 0x5EBFB0, and
/// the frame time 0x865150 is over 0.001 s (not paused): then hidden =
/// the drawn-frame counter 0x870818 & 1, incremented once a frame at
/// 0x5D2C59). **Observed** under Wine: the reset car shows on alternate
/// frames for 2.0 s at the 120 fps cap (a 60 Hz flicker that reads as a
/// see-through ghost), its lights and shadow steady.
///
/// The clone ties the phase to the drawn world time instead of the
/// frame count (any frame rate): the car's visibility flips on every
/// drawn frame that enters a new 1/120 s slot of that time. Up to 120 fps
/// that is every frame, as the original; above it the car still flips at
/// most 120 times a second (60 Hz), as at the original's cap. A frame
/// whose world time moved 0.001 s or less (paused) draws the car.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaceBlink {
    /// The last blinking frame's world time and 1/120 s slot.
    last: Option<(f64, i64)>,
    hidden: bool,
}

impl PlaceBlink {
    /// Whether the car is drawn this frame: `ghosted` the car within its
    /// placement hold (`CarState::ghost_timer` > 0), `now` the frame's
    /// drawn world time (s). Draw-only; nothing of the simulation reads it.
    pub fn drawn(&mut self, ghosted: bool, now: f64) -> bool {
        if !ghosted {
            *self = PlaceBlink::default();
            return true;
        }
        let slot = (now * BLINK_RATE).floor() as i64;
        match self.last {
            // The first frame (a still, too) takes the slot's parity.
            None => self.hidden = slot & 1 == 1,
            // The world stood still: 0x5EC000's frame-time test.
            Some((t, _)) if (now - t).abs() <= 0.001 => return true,
            Some((_, s)) if s != slot => self.hidden = !self.hidden,
            Some(_) => {}
        }
        self.last = Some((now, slot));
        !self.hidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reset_key_is_gated_as_0x5f3f10() {
        let free = ResetCheck { speed: 10.0, ..Default::default() };
        assert!(reset_key(true, false, false, &free));
        assert!(!reset_key(false, false, false, &free));
        assert!(!reset_key(true, true, false, &free));
        assert!(!reset_key(true, false, true, &free));
        // 60 mph is still allowed, above it not.
        assert!(may_reset(&ResetCheck { speed: 60.0 / 2.237, ..free }));
        assert!(!may_reset(&ResetCheck { speed: 60.5 / 2.237, ..free }));
        // The first 3 s of racing, except in an outrun.
        assert!(!may_reset(&ResetCheck { racing_clock: Some(2.9), ..free }));
        assert!(may_reset(&ResetCheck { racing_clock: Some(3.0), ..free }));
        assert!(may_reset(&ResetCheck { racing_clock: Some(1.0), outrun: true, ..free }));
        // The 2 s hold after a placement.
        assert!(!may_reset(&ResetCheck { since_placed: Some((1.9, PLACE_HOLD)), ..free }));
        assert!(may_reset(&ResetCheck { since_placed: Some((2.0, PLACE_HOLD)), ..free }));
        // A blocking event camera; kinds 6 and 7 do not block.
        assert!(!may_reset(&ResetCheck { event_camera: Some(3), ..free }));
        assert!(may_reset(&ResetCheck { event_camera: Some(6), ..free }));
        assert!(may_reset(&ResetCheck { event_camera: Some(7), ..free }));
    }

    /// The drawn pattern of a car ghosted for `PLACE_HOLD` at `fps`.
    fn pattern(fps: f64) -> Vec<bool> {
        let mut b = PlaceBlink::default();
        let frames = (PLACE_HOLD as f64 * fps) as usize;
        (0..frames + 10).map(|i| b.drawn(i < frames, 100.0 + i as f64 / fps)).collect()
    }

    #[test]
    fn a_placed_car_blinks_for_the_hold_at_any_frame_rate() {
        for fps in [30.0, 60.0, 120.0, 144.0, 200.0, 240.0] {
            let p = pattern(fps);
            let frames = (PLACE_HOLD as f64 * fps) as usize;
            let hold = &p[..frames];
            // About half the frames of the hold show the car.
            let shown = hold.iter().filter(|d| **d).count() as f64 / frames as f64;
            assert!((0.4..=0.6).contains(&shown), "{fps} fps: {shown}");
            // It flips at most 120 times a second, and at least once a
            // frame up to 120 fps.
            let flips = hold.windows(2).filter(|w| w[0] != w[1]).count() as f64 / PLACE_HOLD as f64;
            assert!(flips <= BLINK_RATE + 1.0, "{fps} fps: {flips} flips/s");
            if fps <= BLINK_RATE {
                assert!(flips >= fps - 1.0, "{fps} fps: {flips} flips/s");
            } else {
                assert!(flips >= BLINK_RATE * 0.9, "{fps} fps: {flips} flips/s");
            }
            // Steady after the hold.
            assert!(p[frames..].iter().all(|d| *d), "{fps} fps");
        }
    }

    #[test]
    fn a_paused_world_draws_the_placed_car() {
        let mut b = PlaceBlink::default();
        let mut t = 50.0;
        // Run until a hidden frame.
        while b.drawn(true, t) {
            t += 1.0 / 60.0;
        }
        // Paused: the same world time every frame.
        for _ in 0..10 {
            assert!(b.drawn(true, t));
        }
        // Unpaused, it blinks on.
        let next: Vec<bool> = (1..=4).map(|i| b.drawn(true, t + i as f64 / 60.0)).collect();
        assert!(next.contains(&false) && next.contains(&true));
    }
}
