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
//! * [`PadNav`]: a pad's events as the launcher's keys. What a pad reports
//!   in its first moment after it connects is its state, not a press (some
//!   pads report many buttons as pressed then and never release them), and
//!   a pad that reports more buttons held at once than a hand presses is not
//!   read correctly: neither moves the focus or repeats.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use super::ui::Key;

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

/// A held pad direction: the first repeat after this, then one every
/// [`REPEAT_EVERY`] (about a keyboard's).
pub const REPEAT_DELAY: Duration = Duration::from_millis(400);
pub const REPEAT_EVERY: Duration = Duration::from_millis(110);

/// What a pad reports within this of connecting (or of first being seen)
/// is its state: held buttons are noted, nothing is pressed.
pub const SETTLE: Duration = Duration::from_millis(500);
/// More buttons than this held at once: the pad is not read correctly
/// (a Switch Pro Controller on Windows without its driver reports
/// eight and more); its input counts again once they are let go.
pub const MAX_HELD: usize = 3;

/// One pad event, as gilrs reports it (the button's code tells apart the
/// buttons gilrs names `Unknown`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadEv {
    Connected,
    Disconnected,
    Pressed(gilrs::Button, u32),
    Released(gilrs::Button, u32),
    Axis(gilrs::Axis, f32),
}

#[derive(Clone, Debug)]
struct PadState {
    /// Input counts from here on.
    live_from: Instant,
    /// The buttons held now (their codes).
    held: BTreeSet<u32>,
    /// Too many held: told once until it is readable again.
    unreadable: bool,
}

/// Pads' events as the launcher's keys, with the held direction's repeat.
#[derive(Debug)]
pub struct PadNav {
    pads: HashMap<usize, PadState>,
    /// The left stick's last direction per axis (all pads share it).
    stick: (Stick, Stick),
    /// A direction held (d-pad or stick) and when it repeats next.
    held: Option<(Key, Instant)>,
    settle: Duration,
    max_held: usize,
    /// Lines for the log (a pad that is not read correctly, and again).
    pub notes: Vec<String>,
}

impl Default for PadNav {
    fn default() -> Self {
        Self {
            pads: HashMap::new(),
            stick: Default::default(),
            held: None,
            settle: SETTLE,
            max_held: MAX_HELD,
            notes: Vec::new(),
        }
    }
}

fn is_move(k: Key) -> bool {
    matches!(k, Key::Up | Key::Down | Key::Left | Key::Right)
}

/// A button's key in the launcher.
pub fn button_key(b: gilrs::Button) -> Option<Key> {
    use gilrs::Button as B;
    Some(match b {
        B::DPadUp => Key::Up,
        B::DPadDown => Key::Down,
        B::DPadLeft => Key::Left,
        B::DPadRight => Key::Right,
        B::South | B::Start => Key::Enter,
        B::East => Key::Escape,
        B::LeftTrigger => Key::PagePrev,
        B::RightTrigger => Key::PageNext,
        B::LeftTrigger2 => Key::MoveUp,
        B::RightTrigger2 => Key::MoveDown,
        B::West => Key::ActionX,
        B::North => Key::ActionY,
        _ => return None,
    })
}

impl PadNav {
    /// The sticks' resting values (read when a pad is shown).
    pub fn calibrate(&mut self, x: f32, y: f32) {
        self.stick.0.calibrate(x);
        self.stick.1.calibrate(y);
    }

    /// Forget a held direction (the window lost focus).
    pub fn release(&mut self) {
        self.held = None;
    }

    fn pad(&mut self, id: usize, now: Instant) -> &mut PadState {
        let settle = self.settle;
        self.pads.entry(id).or_insert_with(|| PadState {
            live_from: now + settle,
            held: BTreeSet::new(),
            unreadable: false,
        })
    }

    /// Whether pad `id`'s input counts now (settled, readable).
    fn live(&self, id: usize, now: Instant) -> bool {
        self.pads.get(&id).is_some_and(|p| now >= p.live_from && !p.unreadable)
    }

    /// One event of pad `id`: the key it gives, if any.
    pub fn event(&mut self, id: usize, ev: PadEv, now: Instant) -> Option<Key> {
        match ev {
            PadEv::Connected => {
                let settle = self.settle;
                self.pads.insert(id, PadState { live_from: now + settle, held: BTreeSet::new(), unreadable: false });
                self.held = None;
                None
            }
            PadEv::Disconnected => {
                self.pads.remove(&id);
                self.held = None;
                None
            }
            PadEv::Pressed(b, code) => {
                let max = self.max_held;
                let p = self.pad(id, now);
                p.held.insert(code);
                if p.held.len() > max && !p.unreadable {
                    p.unreadable = true;
                    let n = p.held.len();
                    self.notes.push(format!(
                        "gamepad {id}: {n} buttons held at once, so it is not read correctly; its input is ignored until they are let go"
                    ));
                    self.held = None;
                }
                if !self.live(id, now) {
                    return None;
                }
                let k = button_key(b)?;
                if is_move(k) {
                    self.held = Some((k, now + REPEAT_DELAY));
                }
                Some(k)
            }
            PadEv::Released(b, code) => {
                let max = self.max_held;
                let p = self.pad(id, now);
                p.held.remove(&code);
                if p.unreadable && p.held.len() <= max {
                    p.unreadable = false;
                    self.notes.push(format!("gamepad {id}: buttons let go, its input counts again"));
                }
                if let Some(k) = button_key(b)
                    && self.held.is_some_and(|(h, _)| h == k)
                {
                    self.held = None;
                }
                None
            }
            PadEv::Axis(axis, v) => {
                self.pad(id, now);
                let live = self.live(id, now);
                let (s, keys) = match axis {
                    gilrs::Axis::LeftStickX => (&mut self.stick.0, (Key::Right, Key::Left)),
                    // gilrs: up is positive.
                    gilrs::Axis::LeftStickY => (&mut self.stick.1, (Key::Up, Key::Down)),
                    // Other axes (motion sensor, triggers, right stick) are not input here.
                    _ => return None,
                };
                let dir = s.feed(v)?;
                if self.held.is_some_and(|(h, _)| h == keys.0 || h == keys.1) {
                    self.held = None;
                }
                if dir == 0 || !live {
                    return None;
                }
                let k = if dir > 0 { keys.0 } else { keys.1 };
                self.held = Some((k, now + REPEAT_DELAY));
                Some(k)
            }
        }
    }

    /// The held direction's repeat, when it is due.
    pub fn tick(&mut self, now: Instant) -> Option<Key> {
        let (k, at) = self.held?;
        (now >= at).then(|| {
            self.held = Some((k, now + REPEAT_EVERY));
            k
        })
    }
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

    use gilrs::{Axis as A, Button as B};

    const DPAD_RIGHT: u32 = u32::MAX - 2;
    const DPAD_DOWN: u32 = u32::MAX - 1;

    /// Issue 309's log (a Switch Pro Controller on Windows, read through
    /// Windows.Gaming.Input with its SDL mapping): the 30 events the
    /// launcher logged, all in its first poll after the pad connected.
    fn switch_pro_burst() -> Vec<PadEv> {
        use PadEv::*;
        vec![
            Connected,
            Axis(A::LeftStickY, 0.3749661),
            Pressed(B::South, 0),
            Pressed(B::West, 2),
            Pressed(B::RightTrigger, 5),
            Pressed(B::LeftTrigger2, 6),
            Pressed(B::Select, 8),
            Pressed(B::Unknown, 14),
            Pressed(B::Unknown, 15),
            Pressed(B::Unknown, 16),
            Pressed(B::DPadRight, DPAD_RIGHT),
            Pressed(B::DPadDown, DPAD_DOWN),
            Axis(A::LeftStickY, 0.36628556),
            Axis(A::RightStickY, 0.09457737),
            Released(B::South, 0),
            Released(B::West, 2),
            Pressed(B::North, 3),
        ]
    }

    /// Feeds `evs` at `t0` (+1 ms each), then ticks a 60 Hz frame for
    /// `secs`: every key that came out.
    fn run(nav: &mut PadNav, t0: Instant, evs: &[PadEv], secs: u64) -> Vec<Key> {
        let mut keys = Vec::new();
        for (i, e) in evs.iter().enumerate() {
            keys.extend(nav.event(0, *e, t0 + Duration::from_millis(i as u64)));
        }
        let mut t = t0;
        while t < t0 + Duration::from_secs(secs) {
            t += Duration::from_millis(16);
            keys.extend(nav.tick(t));
        }
        keys
    }

    /// Before: every press counted and the stuck d-pad Down repeated, so
    /// the focus ran down the screen on its own (the flicker of #309).
    #[test]
    fn the_switch_pro_burst_moved_the_focus_without_a_player() {
        let mut old = PadNav { settle: Duration::ZERO, max_held: usize::MAX, ..Default::default() };
        let keys = run(&mut old, Instant::now(), &switch_pro_burst(), 3);
        assert_eq!(
            &keys[..7],
            &[Key::Enter, Key::ActionX, Key::PageNext, Key::MoveUp, Key::Right, Key::Down, Key::ActionY]
        );
        let repeats = keys[7..].iter().filter(|k| **k == Key::Down).count();
        assert!(repeats >= 20, "{repeats} repeats in 3 s");
    }

    #[test]
    fn the_switch_pro_burst_moves_nothing() {
        let t0 = Instant::now();
        let mut nav = PadNav::default();
        assert_eq!(run(&mut nav, t0, &switch_pro_burst(), 5), vec![]);
        assert_eq!(nav.notes.len(), 1, "{:?}", nav.notes);
        // It keeps toggling buttons later: still nothing.
        let later = t0 + Duration::from_secs(6);
        for (i, e) in [PadEv::Released(B::North, 3), PadEv::Pressed(B::North, 3), PadEv::Pressed(B::South, 0)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(nav.event(0, e, later + Duration::from_millis(i as u64)), None);
        }
        assert_eq!(nav.tick(later + Duration::from_secs(1)), None);
        // Once it reports its buttons let go, it is a pad again.
        for (b, c) in [
            (B::South, 0),
            (B::North, 3),
            (B::RightTrigger, 5),
            (B::LeftTrigger2, 6),
            (B::Select, 8),
            (B::Unknown, 14),
            (B::Unknown, 15),
            (B::Unknown, 16),
            (B::DPadRight, DPAD_RIGHT),
            (B::DPadDown, DPAD_DOWN),
        ] {
            assert_eq!(nav.event(0, PadEv::Released(b, c), later), None);
        }
        assert_eq!(nav.notes.len(), 2, "{:?}", nav.notes);
        let t = later + Duration::from_secs(2);
        assert_eq!(nav.event(0, PadEv::Pressed(B::South, 0), t), Some(Key::Enter));
    }

    /// An ordinary pad (Xbox, DualSense, Steam Deck): presses after the
    /// first moment count, a held direction repeats and stops on release.
    #[test]
    fn a_normal_pad_navigates_and_repeats() {
        let t0 = Instant::now();
        let mut nav = PadNav::default();
        assert_eq!(nav.event(0, PadEv::Connected, t0), None);
        let t = t0 + Duration::from_secs(1);
        assert_eq!(nav.event(0, PadEv::Pressed(B::DPadDown, 13), t), Some(Key::Down));
        assert_eq!(nav.tick(t + Duration::from_millis(300)), None);
        assert_eq!(nav.tick(t + Duration::from_millis(400)), Some(Key::Down));
        assert_eq!(nav.tick(t + Duration::from_millis(510)), Some(Key::Down));
        assert_eq!(nav.event(0, PadEv::Released(B::DPadDown, 13), t + Duration::from_millis(520)), None);
        assert_eq!(nav.tick(t + Duration::from_secs(2)), None);
        // A chord of three (LB + RB + A) still counts.
        let t = t0 + Duration::from_secs(3);
        assert_eq!(nav.event(0, PadEv::Pressed(B::LeftTrigger, 4), t), Some(Key::PagePrev));
        assert_eq!(nav.event(0, PadEv::Pressed(B::RightTrigger, 5), t), Some(Key::PageNext));
        assert_eq!(nav.event(0, PadEv::Pressed(B::South, 0), t), Some(Key::Enter));
        // The stick.
        let t = t0 + Duration::from_secs(4);
        assert_eq!(nav.event(0, PadEv::Axis(A::LeftStickY, -0.9), t), Some(Key::Down));
        assert_eq!(nav.event(0, PadEv::Axis(A::LeftStickY, 0.0), t), None);
        assert_eq!(nav.tick(t + Duration::from_secs(1)), None);
    }

    /// A button held while the pad is plugged in is no press; let go and
    /// pressed again it is. Unplugged and plugged in again (hot-plug), the
    /// pad settles again and then works.
    #[test]
    fn held_at_connect_and_hot_plug() {
        let t0 = Instant::now();
        let mut nav = PadNav::default();
        nav.event(1, PadEv::Connected, t0);
        assert_eq!(nav.event(1, PadEv::Pressed(B::South, 0), t0 + Duration::from_millis(5)), None);
        assert_eq!(nav.event(1, PadEv::Axis(A::LeftStickX, 1.0), t0 + Duration::from_millis(5)), None);
        let t = t0 + Duration::from_secs(1);
        assert_eq!(nav.event(1, PadEv::Pressed(B::DPadRight, 16), t), Some(Key::Right));
        assert_eq!(nav.event(1, PadEv::Released(B::DPadRight, 16), t), None);
        assert_eq!(nav.event(1, PadEv::Released(B::South, 0), t), None);
        assert_eq!(nav.event(1, PadEv::Pressed(B::South, 0), t), Some(Key::Enter));
        assert_eq!(nav.event(1, PadEv::Axis(A::LeftStickX, 0.0), t), None);
        assert_eq!(nav.event(1, PadEv::Axis(A::LeftStickX, 0.9), t), Some(Key::Right));
        nav.event(1, PadEv::Disconnected, t);
        assert_eq!(nav.tick(t + Duration::from_secs(1)), None, "an unplugged pad's held stick does not repeat");
        let t = t0 + Duration::from_secs(5);
        nav.event(1, PadEv::Connected, t);
        assert_eq!(nav.event(1, PadEv::Pressed(B::East, 1), t + Duration::from_millis(100)), None);
        assert_eq!(nav.event(1, PadEv::Released(B::East, 1), t + Duration::from_millis(200)), None);
        assert_eq!(nav.event(1, PadEv::Pressed(B::East, 1), t + Duration::from_millis(700)), Some(Key::Escape));
    }

    /// A pad first seen without a Connected event also settles first; a
    /// second pad's burst does not stop the first.
    #[test]
    fn two_pads() {
        let t0 = Instant::now();
        let mut nav = PadNav::default();
        assert_eq!(nav.event(0, PadEv::Pressed(B::South, 0), t0), None);
        let t = t0 + Duration::from_secs(1);
        assert_eq!(nav.event(0, PadEv::Released(B::South, 0), t), None);
        for (i, e) in switch_pro_burst().into_iter().enumerate() {
            nav.event(1, e, t + Duration::from_millis(i as u64));
        }
        let t = t + Duration::from_secs(1);
        assert_eq!(nav.event(0, PadEv::Pressed(B::South, 0), t), Some(Key::Enter));
        assert_eq!(nav.event(1, PadEv::Pressed(B::East, 1), t), None);
    }
}
