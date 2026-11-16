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
}
