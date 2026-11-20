//! Controller extras (docs/playing/controller-support.md): the Enhancements
//! hub's Controller group. Clone deviations, every one opt-in, every
//! default the original's behaviour; kept in options.txt as `enh.pad_*`
//! lines (written only when not native) through
//! [`crate::qol::Enhancements`].
//!
//! The shaping ([`PadOptions::shape`]) works on the pad's own values
//! before they are presented to the native DirectInput model, once per
//! poll, as a pure function of the sample: the result does not depend on
//! the frame rate. The rumble model ([`Rumble`]) runs on the world's
//! fixed 1/60 s ticks.

use super::host::{Family, PadInputs};
use std::sync::atomic::{AtomicU8, Ordering};

/// The device the help texts name (Controller Layout Modern): 0 the
/// keyboard (native), else 1 + the [`Family`] of the pad used last.
static PROMPT: AtomicU8 = AtomicU8::new(0);

/// The last input came from a pad of `family` (None: the keyboard). Only
/// the Modern layout calls it with a pad, so with the extra off the texts
/// stay the keyboard's.
pub fn set_prompt(family: Option<Family>) {
    let v = match family {
        None => 0,
        Some(Family::Xbox) => 1,
        Some(Family::PlayStation) => 2,
        Some(Family::Nintendo) => 3,
        Some(Family::Other) => 4,
    };
    PROMPT.store(v, Ordering::Relaxed);
}

/// The Accept key's name in the footers ("Enter" on the keyboard, as the
/// PC build's; the pad's South button under the Modern layout).
pub fn accept_name() -> &'static str {
    match PROMPT.load(Ordering::Relaxed) {
        0 => "Enter",
        2 => Family::PlayStation.names()[0],
        3 => Family::Nintendo.names()[0],
        _ => Family::Xbox.names()[0],
    }
}

/// Controller Layout names: 0 Original (the DirectInput presentation), 1
/// Modern, 2 Auto (the default: Modern for a pad the mapping database
/// knows as a standard gamepad, Original for anything else).
pub const LAYOUT_NAMES: [&str; 3] = ["Original", "Modern", "Auto"];
pub const LAYOUT_ORIGINAL: u8 = 0;
pub const LAYOUT_MODERN: u8 = 1;
pub const LAYOUT_AUTO: u8 = 2;
/// The Controller Layout row's order (Auto first, the default).
pub const LAYOUT_STEPS: [u8; 3] = [LAYOUT_AUTO, LAYOUT_MODERN, LAYOUT_ORIGINAL];

/// What the attached pads resolved to (the hub's "Auto (Modern)" and the
/// deviations list): bit 0 a pad presented as Original, bit 1 one as
/// Modern; 0 no pad.
/// (Per thread in the library's tests, which run in parallel.)
#[cfg(not(test))]
static RESOLVED: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
thread_local!(static RESOLVED_T: AtomicU8 = const { AtomicU8::new(0) });

pub fn set_resolved(original: bool, modern: bool) {
    let v = u8::from(original) | (u8::from(modern) << 1);
    #[cfg(not(test))]
    RESOLVED.store(v, Ordering::Relaxed);
    #[cfg(test)]
    RESOLVED_T.with(|r| r.store(v, Ordering::Relaxed));
}

/// (a pad is presented as Original, one as Modern).
pub fn resolved() -> (bool, bool) {
    #[cfg(not(test))]
    let v = RESOLVED.load(Ordering::Relaxed);
    #[cfg(test)]
    let v = RESOLVED_T.with(|r| r.load(Ordering::Relaxed));
    (v & 1 != 0, v & 2 != 0)
}
/// Stick dead zone steps, percent of the travel (0 native).
pub const DEADZONE_STEPS: [u8; 6] = [0, 5, 10, 15, 20, 25];
/// Trigger dead zone steps, percent (0 native).
pub const TRIGGER_DEADZONE_STEPS: [u8; 5] = [0, 5, 10, 15, 20];
/// Saturation (outer dead zone) steps, percent of the travel that reads
/// full (100 native).
pub const SATURATION_STEPS: [u8; 5] = [100, 95, 90, 85, 80];
/// Steering linearity: the exponent × 100 on the steering axis (100 the
/// original's linear response; above 100 finer near the centre).
pub const STEER_CURVE_STEPS: [u16; 5] = [100, 125, 150, 200, 75];
/// Rumble strengths, percent (0 off, native).
pub const RUMBLE_STEPS: [u8; 4] = [0, 35, 70, 100];
pub const RUMBLE_NAMES: [&str; 4] = ["Off", "Low", "Medium", "High"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadOptions {
    /// Controller Layout: 0 Original (the DirectInput presentation and
    /// defaults Windows and SPEED2.EXE give the pad), 1 Modern (triggers as
    /// analog pedals, Start pauses, shoulder buttons page the menus, exact
    /// analog, d-pad name entry, button names of the pad's family), 2 Auto
    /// (the default; per pad, [`PadOptions::modern_for`]). A device setting
    /// like the resolution: Restore Native and the presets leave it alone.
    pub layout: u8,
    /// Stick dead zone, percent ([`DEADZONE_STEPS`]).
    pub deadzone: u8,
    /// Trigger dead zone, percent ([`TRIGGER_DEADZONE_STEPS`]).
    pub trigger_deadzone: u8,
    /// Saturation, percent ([`SATURATION_STEPS`]).
    pub saturation: u8,
    /// Steering linearity exponent × 100 ([`STEER_CURVE_STEPS`]).
    pub steer_curve: u16,
    /// Rumble, percent ([`RUMBLE_STEPS`]).
    pub rumble: u8,
    /// Pause a race / free roam when the driving pad is unplugged.
    pub pause_on_disconnect: bool,
    /// Safe Axis Binding: a capture on the Controls page takes only the
    /// axis that moved most and leaves the others' roles alone (the
    /// original, 0x5CA730, switches off the roles of every other axis that
    /// moved past the limit in the same frame).
    pub safe_bind: bool,
    /// Quit with Both Sticks: pressing both sticks (L3 + R3) together on
    /// the main menu asks the game's quit question (the original has no
    /// pad shortcut for it; its 'Q' key asks the same, 0x558B80).
    pub sticks_quit: bool,
}

impl Default for PadOptions {
    fn default() -> PadOptions {
        PadOptions { layout: LAYOUT_AUTO, deadzone: 0, trigger_deadzone: 0, saturation: 100, steer_curve: 100, rumble: 0, pause_on_disconnect: false, safe_bind: false, sticks_quit: false }
    }
}

/// The next / previous of `steps` from `v` (an unknown value: the first),
/// wrapping, for the rows' left / right.
pub fn cycle<T: Copy + PartialEq>(steps: &[T], v: T, right: bool) -> T {
    let n = steps.len();
    let at = steps.iter().position(|&s| s == v).unwrap_or(0);
    steps[if right { (at + 1) % n } else { (at + n - 1) % n }]
}

fn step_of<T: Copy + PartialOrd>(steps: &[T], v: T) -> T {
    // The nearest offered step at or below `v` (the first one when none).
    steps.iter().copied().filter(|&s| s <= v).fold(steps[0], |a, s| if s > a { s } else { a })
}

impl PadOptions {
    /// Whether a pad is presented as Modern: always under Modern, never
    /// under Original; under Auto when the mapping database knows it as a
    /// standard gamepad (`standard`) and the profile holds no custom native
    /// map for it (`native_custom`: its saved Original bindings keep
    /// working).
    pub fn modern_for(&self, standard: bool, native_custom: bool) -> bool {
        match self.layout {
            LAYOUT_MODERN => true,
            LAYOUT_ORIGINAL => false,
            _ => standard && !native_custom,
        }
    }

    pub fn is_native(&self) -> bool {
        *self == PadOptions::default()
    }

    /// Whether the sample shaping changes anything.
    pub fn shapes(&self) -> bool {
        self.deadzone != 0 || self.trigger_deadzone != 0 || self.saturation != 100 || self.steer_curve != 100
    }

    pub fn deviations(&self, v: &mut Vec<String>) {
        // Modern by choice, or Auto with a pad it made Modern.
        if self.layout == LAYOUT_MODERN || (self.layout == LAYOUT_AUTO && resolved().1) {
            v.push("modern controller layout".into());
        }
        if self.deadzone != 0 {
            v.push(format!("stick dead zone {}%", self.deadzone));
        }
        if self.trigger_deadzone != 0 {
            v.push(format!("trigger dead zone {}%", self.trigger_deadzone));
        }
        if self.saturation != 100 {
            v.push(format!("saturation {}%", self.saturation));
        }
        if self.steer_curve != 100 {
            v.push(format!("steering linearity {:.2}", f32::from(self.steer_curve) / 100.0));
        }
        if self.rumble != 0 {
            v.push(format!("rumble {}%", self.rumble));
        }
        if self.pause_on_disconnect {
            v.push("pause on controller disconnect".into());
        }
        if self.safe_bind {
            v.push("safe axis binding".into());
        }
        if self.sticks_quit {
            v.push("quit with both sticks".into());
        }
    }

    pub fn write(&self, out: &mut String) {
        let d = PadOptions::default();
        if self.layout != d.layout {
            out.push_str(&format!("enh.pad_layout {}\n", self.layout));
        }
        if self.deadzone != d.deadzone {
            out.push_str(&format!("enh.pad_deadzone {}\n", self.deadzone));
        }
        if self.trigger_deadzone != d.trigger_deadzone {
            out.push_str(&format!("enh.pad_trigger_deadzone {}\n", self.trigger_deadzone));
        }
        if self.saturation != d.saturation {
            out.push_str(&format!("enh.pad_saturation {}\n", self.saturation));
        }
        if self.steer_curve != d.steer_curve {
            out.push_str(&format!("enh.pad_steer_curve {}\n", self.steer_curve));
        }
        if self.rumble != d.rumble {
            out.push_str(&format!("enh.pad_rumble {}\n", self.rumble));
        }
        if self.pause_on_disconnect != d.pause_on_disconnect {
            out.push_str("enh.pad_pause_on_disconnect 1\n");
        }
        if self.safe_bind != d.safe_bind {
            out.push_str("enh.pad_safe_bind 1\n");
        }
        if self.sticks_quit != d.sticks_quit {
            out.push_str("enh.pad_sticks_quit 1\n");
        }
    }

    pub fn parse(&mut self, key: &str, value: &str) -> bool {
        match key {
            "enh.pad_layout" => self.layout = value.parse::<u8>().map_or(self.layout, |v| v.min(LAYOUT_AUTO)),
            "enh.pad_deadzone" => self.deadzone = value.parse::<u8>().map_or(self.deadzone, |v| step_of(&DEADZONE_STEPS, v)),
            "enh.pad_trigger_deadzone" => self.trigger_deadzone = value.parse::<u8>().map_or(self.trigger_deadzone, |v| step_of(&TRIGGER_DEADZONE_STEPS, v)),
            "enh.pad_saturation" => self.saturation = value.parse::<u8>().map_or(self.saturation, |v| if SATURATION_STEPS.contains(&v) { v } else { self.saturation }),
            "enh.pad_steer_curve" => self.steer_curve = value.parse::<u16>().map_or(self.steer_curve, |v| if STEER_CURVE_STEPS.contains(&v) { v } else { self.steer_curve }),
            "enh.pad_rumble" => self.rumble = value.parse::<u8>().map_or(self.rumble, |v| step_of(&RUMBLE_STEPS, v)),
            "enh.pad_pause_on_disconnect" => self.pause_on_disconnect = value != "0",
            "enh.pad_safe_bind" => self.safe_bind = value != "0",
            "enh.pad_sticks_quit" => self.sticks_quit = value != "0",
            _ => return false,
        }
        true
    }

    /// The pad's sample shaped: per stick axis the dead zone and the
    /// saturation (the travel between them rescaled to 0..1), the steering
    /// axis (left x) then through the linearity exponent; the triggers by
    /// their own dead zone and the saturation. Identity at the defaults.
    pub fn shape(&self, p: &PadInputs) -> PadInputs {
        if !self.shapes() {
            return *p;
        }
        let sat = f32::from(self.saturation) / 100.0;
        let ramp = |v: f32, dz: f32| -> f32 {
            let a = v.abs();
            if a <= dz {
                return 0.0;
            }
            let t = ((a - dz) / (sat - dz).max(1e-3)).min(1.0);
            t.copysign(v)
        };
        let dz = f32::from(self.deadzone) / 100.0;
        let mut out = *p;
        for (i, s) in out.sticks.iter_mut().enumerate() {
            let v = ramp(p.sticks[i], dz);
            *s = if i == 0 && self.steer_curve != 100 { v.abs().powf(f32::from(self.steer_curve) / 100.0).copysign(v) } else { v };
        }
        let tdz = f32::from(self.trigger_deadzone) / 100.0;
        for (i, t) in out.triggers.iter_mut().enumerate() {
            *t = ramp(p.triggers[i].max(0.0), tdz);
        }
        out
    }
}

/// What the rumble model reads from the player's car each world tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RumbleFeed {
    /// The car's speed, m/s.
    pub speed: f32,
    /// The wheels' suspension travel this tick, m: the sum of each
    /// wheel's compression change since the last tick (the original's
    /// bump test sums the bump stops' totals at car +0x370..+0x37C,
    /// formats/front-end.md; the travel stands in for them, **inferred**).
    pub bump: f32,
    /// The surfaces under the wheels (collision face byte 2): the road
    /// texture's weak-motor buzz ([`surface_buzz`]).
    pub surfaces: [u8; 4],
    /// A front or rear wheel on surface 13 (PUDDLE).
    pub rough: bool,
    /// A nitrous shot is running.
    pub nitrous: bool,
    /// The gear changed this tick.
    pub shifted: bool,
    /// The hardest new hit since the last tick (`drive::CarState::take_hit`):
    /// a contact's first push |J| against a wall, another car or a
    /// knock-over prop; 0 none.
    pub hit: f32,
    /// The player drives (not a scene, replay, menu or pause).
    pub driving: bool,
}

/// The road texture by surface, after the original's periodic terrain
/// effect 0x5C9FE0 (formats/front-end.md, **Force feedback**): the surface
/// table's amplitude (8: 130, 9: 110, 10: 70, 11: 100, 15: 100, 16: 80,
/// 17: 80, others none), magnitude amp / 2 × min(1, v / 13) × 39.2157 of
/// DirectInput's 10000, here 0..1 on the weak motor; the strongest of the
/// wheels' surfaces.
pub fn surface_buzz(surfaces: &[u8; 4], speed: f32) -> f32 {
    let amp = |s: u8| match s {
        8 => 130.0,
        9 => 110.0,
        10 => 70.0,
        11 | 15 => 100.0,
        16 | 17 => 80.0,
        _ => 0.0f32,
    };
    let a = surfaces.iter().map(|&s| amp(s)).fold(0.0, f32::max);
    (a / 2.0 * (speed / 13.0).clamp(0.0, 1.0) * 39.2157 / 10000.0).clamp(0.0, 1.0)
}

/// The rumble model, Overbright's own, loosely after the original's
/// force-feedback effects (formats/front-end.md, **Force feedback**; the
/// faithful model is [`super::ff::Original`]): a strong motor pulse when the
/// speed changes by more than 4 m/s within 0.15 s or the bump value passes
/// 0.05, scaled by the speed change; the weak motor for the road texture
/// ([`surface_buzz`]), a puddle (by speed), a nitrous shot and a gear change.
/// A collision (the game's own contact events: a wall, another car, a
/// knock-over prop, [`RumbleFeed::hit`]) always pulses the strong motor,
/// from [`HIT_FLOOR`] for the lightest touch to full at [`HIT_FULL`],
/// held [`HIT_HOLD`] ticks (0.1 s) before it fades.
/// One step per 1/60 s world tick, so the same at every frame rate.
#[derive(Clone, Debug, Default)]
pub struct Rumble {
    /// Speeds of the last 0.15 s (9 ticks), oldest first.
    history: std::collections::VecDeque<f32>,
    strong: f32,
    /// World ticks a hit's pulse still holds before it fades.
    hold: u8,
    shift: f32,
}

/// A hit's pulse for the lightest touch (a glancing car, a small prop).
pub const HIT_FLOOR: f32 = 0.35;
/// The push |J| that gives a full pulse (30: the push that totals a car
/// in a drag race, 0x59FB80).
pub const HIT_FULL: f32 = 30.0;
/// World ticks (1/60 s) a hit's pulse holds before its fade: 0.1 s.
pub const HIT_HOLD: u8 = 6;

/// A hit's pulse strength 0..1 for its push |J|.
pub fn hit_strength(push: f32) -> f32 {
    if !(push > 0.0) {
        return 0.0;
    }
    HIT_FLOOR + (1.0 - HIT_FLOOR) * (push / HIT_FULL).min(1.0).sqrt()
}

impl Rumble {
    /// Ticks in the jolt window (0.15 s at 60 Hz).
    const WINDOW: usize = 9;

    pub fn reset(&mut self) {
        *self = Rumble::default();
    }

    /// One world tick: the motors' strengths 0..1 (strong, weak) before
    /// the option's scale.
    pub fn step(&mut self, f: &RumbleFeed) -> (f32, f32) {
        const DT: f32 = 1.0 / 60.0;
        if !f.driving {
            self.reset();
            return (0.0, 0.0);
        }
        self.history.push_back(f.speed);
        if self.history.len() > Self::WINDOW {
            self.history.pop_front();
        }
        let (lo, hi) = self.history.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        let change = hi - lo;
        // A jolt: the original's test (speed change over 4 in 0.15 s, or the
        // travel sum over 0.05); the pulse holds the larger, then fades in
        // 0.25 s.
        if change > 4.0 || f.bump > 0.05 {
            let k = (change / 15.0).max(f.bump * 4.0).clamp(0.25, 1.0);
            if k > self.strong {
                self.strong = k;
                // A new jolt restarts the window so one hit is one pulse.
                self.history.clear();
                self.history.push_back(f.speed);
            }
        }
        // A collision: a pulse by the hit's strength, at least the floor.
        let hit = hit_strength(f.hit);
        if hit > 0.0 && hit >= self.strong {
            self.strong = hit;
            self.hold = HIT_HOLD;
            self.history.clear();
            self.history.push_back(f.speed);
        }
        let strong = self.strong;
        if self.hold > 0 {
            self.hold -= 1;
        } else {
            self.strong = (self.strong - DT / 0.25).max(0.0);
        }
        if f.shifted {
            self.shift = 0.35;
        }
        let shift = if self.shift > 0.0 { 0.5 } else { 0.0 };
        self.shift = (self.shift - DT * 3.5).max(0.0);
        let rough = if f.rough { (f.speed / 30.0).clamp(0.0, 1.0) * 0.45 } else { 0.0 };
        let nitrous = if f.nitrous { 0.2 } else { 0.0 };
        let texture = surface_buzz(&f.surfaces, f.speed);
        (strong, (rough + nitrous + texture).max(shift).min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_identity_and_round_trip() {
        let d = PadOptions::default();
        let mut p = PadInputs::default();
        p.sticks = [0.03, -0.5, 0.97, 1.0];
        p.triggers = [0.02, 0.9];
        assert_eq!(d.shape(&p), p);
        let mut out = String::new();
        d.write(&mut out);
        assert!(out.is_empty() && d.is_native());
        // Auto is the default; Original is now written.
        assert_eq!(d.layout, LAYOUT_AUTO);
        let mut o = String::new();
        PadOptions { layout: LAYOUT_ORIGINAL, ..d }.write(&mut o);
        assert_eq!(o, "enh.pad_layout 0\n");
        // Auto is a deviation only while it makes a pad Modern.
        let mut v = Vec::new();
        set_resolved(true, false);
        d.deviations(&mut v);
        assert!(v.is_empty());
        set_resolved(false, true);
        d.deviations(&mut v);
        assert_eq!(v, ["modern controller layout"]);
        set_resolved(false, false);
        assert!(PadOptions::default().modern_for(true, false) && !PadOptions::default().modern_for(false, false) && !PadOptions::default().modern_for(true, true));
        assert!(PadOptions { layout: LAYOUT_MODERN, ..d }.modern_for(false, true) && !PadOptions { layout: LAYOUT_ORIGINAL, ..d }.modern_for(true, false));
        let e = PadOptions { layout: 1, deadzone: 10, trigger_deadzone: 5, saturation: 90, steer_curve: 150, rumble: 70, pause_on_disconnect: true, safe_bind: true, sticks_quit: true };
        e.write(&mut out);
        let mut back = PadOptions::default();
        for l in out.lines() {
            let (k, v) = l.split_once(' ').unwrap();
            assert!(back.parse(k, v));
        }
        assert_eq!(back, e);
        let mut v = Vec::new();
        e.deviations(&mut v);
        assert_eq!(v.len(), 9);
        // Odd values snap to the steps.
        assert!(back.parse("enh.pad_deadzone", "12") && back.deadzone == 10);
        assert!(back.parse("enh.pad_saturation", "93") && back.saturation == 90);
    }

    #[test]
    fn shaping() {
        let o = PadOptions { deadzone: 10, saturation: 90, steer_curve: 200, trigger_deadzone: 10, ..Default::default() };
        let mut p = PadInputs::default();
        p.sticks = [0.05, 0.5, -0.95, 0.1];
        p.triggers = [0.05, 0.95];
        let s = o.shape(&p);
        assert_eq!(s.sticks[0], 0.0);
        // (0.5 − 0.1) / 0.8 = 0.5 on a non-steering axis.
        assert!((s.sticks[1] - 0.5).abs() < 1e-6);
        assert_eq!(s.sticks[2], -1.0);
        assert_eq!(s.triggers, [0.0, 1.0]);
        // The steering curve: 0.55 → (0.45 / 0.8)² ≈ 0.316.
        p.sticks[0] = -0.55;
        let s = o.shape(&p);
        assert!((s.sticks[0] + 0.31640625).abs() < 1e-5);
    }

    #[test]
    fn rumble_pulses_on_a_jolt_and_fades() {
        let mut r = Rumble::default();
        let mut f = RumbleFeed { speed: 30.0, driving: true, ..Default::default() };
        for _ in 0..20 {
            assert_eq!(r.step(&f), (0.0, 0.0));
        }
        f.speed = 20.0;
        let (s, _) = r.step(&f);
        assert!(s > 0.6);
        let mut last = s;
        for _ in 0..15 {
            let (s, _) = r.step(&f);
            assert!(s <= last);
            last = s;
        }
        assert_eq!(last, 0.0);
        f.rough = true;
        f.speed = 30.0;
        assert!(r.step(&f).1 > 0.4);
        f.driving = false;
        assert_eq!(r.step(&f), (0.0, 0.0));
    }

    /// A bump (the wheels' travel over 0.05 m in a tick) pulses the strong
    /// motor; a textured surface buzzes the weak one by speed, plain
    /// asphalt not at all.
    #[test]
    fn rumble_on_bumps_and_road_texture() {
        let mut r = Rumble::default();
        let mut f = RumbleFeed { speed: 20.0, driving: true, ..Default::default() };
        assert_eq!(r.step(&f), (0.0, 0.0));
        f.bump = 0.08;
        let (s, _) = r.step(&f);
        assert!(s >= 0.25, "{s}");
        f.bump = 0.0;
        r.reset();
        f.surfaces = [8, 8, 0, 0];
        let (_, w) = r.step(&f);
        assert!((w - 0.2549).abs() < 1e-3, "{w}");
        // Half of 13 m/s: half the buzz (a fresh model: no speed jolt).
        r.reset();
        f.speed = 6.5;
        assert!((r.step(&f).1 - 0.1275).abs() < 1e-3);
        f.surfaces = [0; 4];
        assert_eq!(r.step(&f), (0.0, 0.0));
    }

    /// A collision (the game's own contact events) always pulses the strong
    /// motor, from the floor for the lightest touch, harder for harder hits,
    /// held 0.1 s and then faded as the jolt; no hit, no change.
    #[test]
    fn rumble_on_hits_by_strength() {
        let peak = |hit: f32| {
            let mut r = Rumble::default();
            let mut f = RumbleFeed { speed: 20.0, driving: true, ..Default::default() };
            r.step(&f);
            f.hit = hit;
            let (first, _) = r.step(&f);
            f.hit = 0.0;
            let mut held = 0;
            let mut last = first;
            for _ in 0..40 {
                let (s, _) = r.step(&f);
                held += usize::from(s == first && s > 0.0);
                assert!(s <= last);
                last = s;
            }
            assert_eq!(last, 0.0);
            (first, held)
        };
        assert_eq!(peak(0.0), (0.0, 0));
        let (light, held) = peak(0.05);
        assert!(light >= HIT_FLOOR && light < 0.45, "{light}");
        assert_eq!(held, usize::from(HIT_HOLD));
        let mut last = light;
        for j in [1.0, 5.0, 15.0, 30.0] {
            let (k, _) = peak(j);
            assert!(k > last, "{j}: {k} after {last}");
            last = k;
        }
        assert_eq!(last, 1.0);
        assert_eq!(peak(80.0).0, 1.0);
        // A weaker hit during a stronger pulse does not cut it short.
        let mut r = Rumble::default();
        let mut f = RumbleFeed { speed: 20.0, driving: true, hit: 30.0, ..Default::default() };
        r.step(&f);
        f.hit = 0.1;
        assert_eq!(r.step(&f).0, 1.0);
    }
}
