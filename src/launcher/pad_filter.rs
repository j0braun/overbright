//! What counts as a deliberate pad input for the launcher (a pad's noise
//! must not switch the look between pad and mouse or move the focus).
//!
//! * [`Stick`]: one stick axis as a direction with two thresholds
//!   (hysteresis) around a resting value read when the pad connects, so an
//!   axis that rests away from 0 (some pads report -1.0 or 0.03 at rest) or
//!   that jitters around the threshold gives no repeated presses.
//! * [`NameDebounce`]: the shown pad name changes only after it has stayed
//!   the same for a moment (a pad that reconnects in a loop).
//! * [`mouse_move_counts`]: a pointer movement just after a pad input and
//!   of a pixel or two (the pointer hiding can make the system report one)
//!   does not switch back to the mouse look.

use std::time::{Duration, Instant};

/// A stick past this (from its resting value) is a direction.
pub const ENTER: f32 = 0.6;
/// ...and stays one until it falls back below this.
pub const LEAVE: f32 = 0.4;
/// A value past this at connect is the axis's resting value.
const CALIBRATE_FROM: f32 = 0.2;

/// One stick axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stick {
    rest: f32,
    dir: i8,
}

impl Stick {
    /// The value read when the pad connected: a resting value.
    pub fn calibrate(&mut self, v: f32) {
        self.rest = if v.abs() > CALIBRATE_FROM { v.clamp(-1.0, 1.0) } else { 0.0 };
        self.dir = 0;
    }

    /// The new direction (-1, 0, 1) when `v` changes it, else None.
    pub fn feed(&mut self, v: f32) -> Option<i8> {
        let e = v - self.rest;
        let new = if e > ENTER || (self.dir == 1 && e > LEAVE) {
            1
        } else if e < -ENTER || (self.dir == -1 && e < -LEAVE) {
            -1
        } else {
            0
        };
        (new != self.dir).then(|| {
            self.dir = new;
            new
        })
    }

    pub fn dir(&self) -> i8 {
        self.dir
    }
}

/// The pad name shown, changing only when a new one has held for `HOLD`.
#[derive(Clone, Debug, Default)]
pub struct NameDebounce {
    pending: Option<(Option<String>, Instant)>,
}

const HOLD: Duration = Duration::from_millis(400);

impl NameDebounce {
    /// `seen` is the name now, `shown` the one on screen; the name to show
    /// when it should change.
    pub fn update(&mut self, now: Instant, seen: &Option<String>, shown: &Option<String>) -> Option<Option<String>> {
        if seen == shown {
            self.pending = None;
            return None;
        }
        match &self.pending {
            Some((p, since)) if p == seen => (now.duration_since(*since) >= HOLD).then(|| {
                self.pending = None;
                seen.clone()
            }),
            _ => {
                self.pending = Some((seen.clone(), now));
                None
            }
        }
    }
}

/// Whether a pointer movement of `dx`, `dy` pixels is the player's: not a
/// movement of a few pixels within a moment of a pad input.
pub fn mouse_move_counts(dx: f32, dy: f32, since_pad: Option<Duration>) -> bool {
    since_pad.is_none_or(|d| d > Duration::from_millis(500)) || dx.abs().max(dy.abs()) >= 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Pro-Controller-like stream: noise around 0, a resting offset, and
    /// jitter right at the threshold; 300 frames give no direction.
    #[test]
    fn noisy_axes_give_no_direction() {
        let mut s = Stick::default();
        s.calibrate(0.03);
        let mut changes = 0;
        for f in 0..300 {
            let n = ((f * 7919) % 100) as f32 / 100.0 - 0.5; // -0.5..0.5
            if s.feed(0.03 + n * 0.3).is_some() {
                changes += 1;
            }
        }
        assert_eq!(changes, 0);
    }

    #[test]
    fn jitter_at_the_threshold_presses_once() {
        let mut s = Stick::default();
        let mut presses = 0;
        for f in 0..300 {
            let v = if f % 2 == 0 { 0.62 } else { 0.55 };
            if s.feed(v) == Some(1) {
                presses += 1;
            }
        }
        assert_eq!(presses, 1);
        assert_eq!(s.feed(0.1), Some(0));
    }

    #[test]
    fn a_resting_axis_is_calibrated() {
        let mut s = Stick::default();
        s.calibrate(-1.0);
        for _ in 0..300 {
            assert_eq!(s.feed(-1.0), None);
        }
        assert_eq!(s.dir(), 0);
        // A real push away from rest still works.
        assert_eq!(s.feed(-0.6), None);
        assert_eq!(s.feed(0.0), Some(1));
    }

    #[test]
    fn a_real_push_is_a_direction() {
        let mut s = Stick::default();
        assert_eq!(s.feed(0.9), Some(1));
        assert_eq!(s.feed(0.5), None);
        assert_eq!(s.feed(-0.9), Some(-1));
    }

    #[test]
    fn a_looping_name_does_not_change_the_screen() {
        let mut d = NameDebounce::default();
        let t0 = Instant::now();
        let a = Some("Pad".to_string());
        let mut shown = None;
        let mut changes = 0;
        // connected / lost every 100 ms for 300 frames
        for f in 0..300u64 {
            let seen = if f % 2 == 0 { &a } else { &None };
            if let Some(n) = d.update(t0 + Duration::from_millis(f * 50), seen, &shown) {
                shown = n;
                changes += 1;
            }
        }
        assert_eq!(changes, 0);
        // A name that stays is shown.
        assert!(d.update(t0 + Duration::from_secs(30), &a, &shown).is_none());
        assert_eq!(d.update(t0 + Duration::from_secs(31), &a, &shown), Some(a.clone()));
    }

    #[test]
    fn pointer_jitter_after_a_pad_input_is_not_the_mouse() {
        let soon = Some(Duration::from_millis(20));
        assert!(!mouse_move_counts(1.0, 0.0, soon));
        assert!(mouse_move_counts(30.0, 0.0, soon));
        assert!(mouse_move_counts(1.0, 0.0, None));
        assert!(mouse_move_counts(1.0, 0.0, Some(Duration::from_secs(2))));
    }
}
