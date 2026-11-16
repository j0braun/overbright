//! Nitrous (verified unless noted). The car's tank is the block at car
//! +0x40 + 0x414.. (0x591CE0 fills it from the tuning block +0x2A0 /
//! +0x2A4 = car-type record +0x3B0 / +0x3B4; the shot's push is block
//! +0x2AC = record +0x3BC). Times are game ticks (1/4000 s, 0x784264 /
//! 0x784268), rounded with + 0.5.

/// Ticks per second (0x784264).
pub const TICKS: f32 = 4000.0;

/// The nitrous torque curve (0x7FBF38, the engine's curve at +0x60 set up
/// by 0x5AA5xx over idle..redline): a factor on the full-throttle push.
pub const CURVE: [f32; 9] = [0.3, 0.4, 0.45, 0.5, 0.55, 0.6, 0.7, 1.0, 0.7];

/// The stage thresholds on the tank's fraction (0x7A1798: a stage is 1 +
/// the highest i with fraction > T[i]).
const STAGES: [f32; 3] = [0.0, 1.4, 1.7];

/// The torque factor by stage (0x7A17A0[stage], 0x594372).
pub const TORQUE: [f32; 4] = [1.7, 1.0, 1.4, 1.7];

/// The wheels' grip factor by stage (0x7A17AC[stage], 0x5A510F).
pub const GRIP: [f32; 4] = [1.7, 1.0, 1.4, 1.7];

/// The tank (car +0x454..+0x460).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tank {
    /// Its size, ticks (+0x414): capacity × 4000.
    pub size: i32,
    /// What is left, ticks, as of the shot's start (+0x41C).
    pub left: i32,
    /// The tank at the shot's start (+0x418: capacity / shots at fill,
    /// `left` when a shot starts with 0x7FBEDC set, 0 when it stops); no
    /// reader was found.
    pub shot: i32,
    /// Seconds into the running shot (+0x420 holds its start tick; 0 = no
    /// shot).
    pub running: Option<f32>,
}

fn ticks(s: f32) -> i32 {
    (s * TICKS + 0.5) as i32
}

impl Tank {
    /// Fill the tank (0x591CE0): `capacity` seconds (+0x2A0), `shots`
    /// (+0x2A4). Called at the car's set-up (0x59F9DA) and when a car's
    /// size no longer matches its record (0x5F6C8C).
    pub fn new(capacity: f32, shots: i32) -> Tank {
        Tank {
            size: ticks(capacity),
            left: ticks(capacity),
            shot: if shots > 0 { ticks(capacity / shots as f32) } else { 0 },
            running: None,
        }
    }

    /// Seconds left (0x592490): `left` less the running shot's time.
    pub fn remaining(&self) -> f32 {
        let used = self.running.map_or(0.0, |t| t);
        if self.left != 0 { self.left as f32 / TICKS - used } else { -used }
    }

    /// The tank's fraction (0x5925C0): remaining / size; past full (the
    /// style-point refill 0x592650 gets there, up to 2) the excess counts
    /// at rate 1 (0x7A1794).
    pub fn fraction(&self) -> f32 {
        if self.size == 0 {
            return 0.0;
        }
        let size = self.size as f32 / TICKS;
        if size <= 0.0 {
            return 0.0;
        }
        let r = self.remaining();
        if r <= size { r / size } else { size / size + (r - size) / 1.0 / size }
    }

    fn stage_of(&self) -> usize {
        let f = self.fraction();
        (0..3).rev().find(|&i| f > STAGES[i]).map_or(0, |i| i + 1)
    }

    /// The stage a shot would start at (0x59FAB0): 0 while one runs.
    pub fn ready(&self) -> usize {
        if self.running.is_some() { 0 } else { self.stage_of() }
    }

    /// The running shot's stage (0x59FA40, each frame from the mover
    /// 0x5ABD75 into engine +0x84 and from the grip set-up 0x5A5106): 0
    /// without one; an empty one stops.
    pub fn level(&mut self) -> usize {
        if self.running.is_none() {
            return 0;
        }
        let s = self.stage_of();
        if s == 0 {
            self.stop();
        }
        s
    }

    /// Start a shot (0x59FB00) when one may (0x59FAB0); `shot` takes the
    /// tank (0x7FBEDC is 1).
    pub fn start(&mut self) -> bool {
        if self.ready() == 0 {
            return false;
        }
        self.running = Some(0.0);
        self.shot = self.left;
        true
    }

    /// End the shot (0x592510): `left` becomes what remains (at least 0).
    pub fn stop(&mut self) {
        if self.running.is_some() {
            self.left = ticks(self.remaining().max(0.0));
            self.shot = 0;
            self.running = None;
        }
    }

    /// Add `f` tanks to what is left (0x592650; `f` < 0 takes away). With
    /// `L` the size in seconds and `F` the fraction (0x5925C0): adding
    /// fills to full first (min(1 − F, f) · L), then past full up to a
    /// fraction of 2 (min(2 − F', f') · 1.0 · L, 0x7A1794); taking away
    /// empties the excess first (max(1 − F, f) · L), then down to 0
    /// (max(−F', f') · L). Each step rounds `left` to ticks (+ 0.5, then
    /// truncated, 0x75BED4). A refill during a shot adds to `left`, so the
    /// remaining time moves by the same amount.
    pub fn refill(&mut self, mut f: f32) {
        let secs = |v: i32| if v != 0 && v != i32::MAX { v as f32 / TICKS } else { 0.0 };
        let size = secs(self.size);
        let mut frac = self.fraction();
        if f > 0.0 {
            if frac < 1.0 {
                let a = (1.0 - frac).min(f);
                self.left = ticks(a * size + secs(self.left));
                frac += a;
                f -= a;
            }
            if f > 0.0 {
                let b = (2.0 - frac).min(f);
                self.left = ticks(1.0 * b * size + secs(self.left));
            }
        } else {
            if frac > 1.0 {
                let c = (1.0 - frac).max(f);
                self.left = ticks(1.0 * c * size + secs(self.left));
                frac += c;
                f -= c;
            }
            if f < 0.0 {
                let d = (-frac).max(f);
                self.left = ticks(d * size + secs(self.left));
            }
        }
    }

    /// The shot's clock (the game reads the race clock 0x865188 against
    /// the start tick).
    pub fn advance(&mut self, dt: f32) {
        // The shot's time is the clock (whole ticks, 0x865188) less its
        // start tick (+0x420), so it counts whole ticks: summing frame
        // steps in seconds would leave 40 × 0.05 s just under 2 s and run
        // the shot a frame long.
        if let Some(t) = &mut self.running {
            *t = ((*t * TICKS).round() + (dt * TICKS).round()) / TICKS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_drains_the_tank_and_stops_when_empty() {
        let mut t = Tank::new(5.0, 0);
        assert_eq!((t.size, t.left, t.shot), (20000, 20000, 0));
        assert_eq!(t.ready(), 1);
        assert!(t.start());
        assert_eq!(t.ready(), 0);
        t.advance(2.0);
        assert_eq!(t.level(), 1);
        assert!((t.fraction() - 0.6).abs() < 1e-6);
        t.stop();
        assert_eq!(t.left, 12000);
        assert!(t.start());
        t.advance(3.5);
        assert_eq!(t.level(), 0);
        assert_eq!((t.left, t.running), (0, None));
        assert!(!t.start());
    }

    #[test]
    fn style_points_refill_past_full_up_to_twice_the_tank() {
        // Measured under Wine (2026-10-30, Rachel's 350Z, 4 s tank): a
        // SPRAY SHOW's 100 points took left 16000 → 17067 during a shot
        // (100 / 15 × 0.01 of the tank, 0x601737).
        let mut t = Tank::new(4.0, 0);
        assert!(t.start());
        t.advance(0.85);
        t.refill(crate::style::refill_fraction(100.0));
        assert_eq!(t.left, 17067);
        t.stop();
        let mut t = Tank::new(4.0, 0);
        t.left = 8000;
        t.refill(0.75);
        assert_eq!(t.left, 20000);
        assert!((t.fraction() - 1.25).abs() < 1e-6);
        t.refill(5.0);
        assert_eq!(t.left, 32000);
        assert_eq!(t.ready(), 3);
        t.refill(-0.5);
        assert_eq!(t.left, 24000);
        t.refill(-0.75);
        assert_eq!(t.left, 12000);
        t.refill(-9.0);
        assert_eq!(t.left, 0);
        assert_eq!(t.ready(), 0);
    }

    #[test]
    fn negative_style_points_drain_past_full_down_to_empty() {
        // Measured under Wine (2026-10-31, nosneg: Rachel's 350Z, 4 s tank
        // = 16000 ticks, free roam, TRAFFIC HITs; left written first). A
        // negative Δ takes the extra first, then goes on below one full tank
        // (0x592650's second step), down to 0 and never below.
        let run = |left: i32, deltas: &[f32]| {
            let mut t = Tank::new(4.0, 0);
            t.left = left;
            deltas
                .iter()
                .map(|&d| {
                    t.refill(crate::style::refill_fraction(d));
                    t.left
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(28000, &[-42.0, -45.0, -46.0, -47.0]), [27552, 27072, 26581, 26080]);
        assert_eq!(run(16800, &[-78.0, -88.0, -45.0]), [15968, 15029, 14549]);
        assert_eq!(run(12000, &[-61.0]), [11349]);
        assert_eq!(run(300, &[-30.0]), [0]);
        assert_eq!(run(0, &[-30.0, -31.0]), [0, 0]);
    }

    #[test]
    fn no_capacity_no_nitrous() {
        let mut t = Tank::new(0.0, 0);
        assert!(!t.start());
        assert_eq!(t.fraction(), 0.0);
        assert_eq!(Tank::new(6.0, 3).shot, 8000);
    }
}
