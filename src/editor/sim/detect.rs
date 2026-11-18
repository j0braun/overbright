//! The problems a simulation reports, found from each car's motion tick by
//! tick: a hard hit (speed lost in one tick), a car that barely moves for
//! a while after the green, a car driving against the race's direction.
//! The game's simulate mode runs this on every racer; it is plain maths, so
//! it is tested here without the game.

use super::format::{Event, EventKind};

/// Speed lost in one tick that counts as a hard hit, m/s.
pub const CRASH_LOSS: f32 = 6.0;
/// Below this a racer counts as not moving, m/s.
pub const STUCK_SPEED: f32 = 1.5;
/// Not moving this long after the green is "stuck", s.
pub const STUCK_AFTER: f32 = 4.0;
/// Moving again above this ends a stuck spell, m/s.
pub const MOVING_AGAIN: f32 = 5.0;
/// Against the road (direction · forward below this) for this long is
/// "wrong way", above `WRONG_SPEED`.
pub const WRONG_DOT: f32 = -0.3;
pub const WRONG_AFTER: f32 = 2.0;
pub const WRONG_SPEED: f32 = 4.0;
/// A second hit within this many seconds is the same crash.
pub const CRASH_GAP: f32 = 1.0;

/// One car's watch.
#[derive(Clone, Debug, Default)]
pub struct Watch {
    last_speed: Option<f32>,
    slow: f32,
    stuck: bool,
    wrong: f32,
    wrong_said: bool,
    /// Seconds since the last crash (None: none yet).
    since_crash: Option<f32>,
}

/// What the watch is told each tick.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub tick: u64,
    pub time: f32,
    pub dt: f32,
    pub car: u32,
    pub speed: f32,
    /// The road's driving direction · the car's forward (None: unknown).
    pub along_road: Option<f32>,
    /// Racing: after the green and not finished.
    pub racing: bool,
    /// The next checkpoint (1-based) for the events' place.
    pub next_checkpoint: u32,
}

impl Watch {
    /// This tick's events (none before the green or after the finish; a
    /// teleport — a reset or a traffic car placed again — reads as a jump,
    /// not a crash, because the caller passes `racing` false then or the
    /// speed rises).
    pub fn step(&mut self, t: Tick) -> Vec<Event> {
        let mut out = Vec::new();
        let ev =
            |kind, value| Event { tick: t.tick, time: t.time, kind, car: t.car, checkpoint: t.next_checkpoint, value };
        if let Some(c) = self.since_crash.as_mut() {
            *c += t.dt;
        }
        if !t.racing {
            self.last_speed = Some(t.speed);
            self.slow = 0.0;
            self.wrong = 0.0;
            return out;
        }
        if let Some(before) = self.last_speed {
            let lost = before - t.speed;
            if lost >= CRASH_LOSS && self.since_crash.is_none_or(|c| c >= CRASH_GAP) {
                out.push(ev(EventKind::Crash, lost));
                self.since_crash = Some(0.0);
            }
        }
        self.last_speed = Some(t.speed);
        if t.speed < STUCK_SPEED {
            self.slow += t.dt;
            if self.slow >= STUCK_AFTER && !self.stuck {
                self.stuck = true;
                out.push(ev(EventKind::Stuck, self.slow));
            }
        } else if t.speed > MOVING_AGAIN {
            self.slow = 0.0;
            self.stuck = false;
        }
        match t.along_road {
            Some(d) if d < WRONG_DOT && t.speed > WRONG_SPEED => {
                self.wrong += t.dt;
                if self.wrong >= WRONG_AFTER && !self.wrong_said {
                    self.wrong_said = true;
                    out.push(ev(EventKind::WrongWay, self.wrong));
                }
            }
            Some(d) if d > 0.3 => {
                self.wrong = 0.0;
                self.wrong_said = false;
            }
            _ => {}
        }
        out
    }
}

/// How many of the race's checkpoints lie behind `along` metres on the
/// race's road (`cps` their places on it, `start` the start's, `length`
/// the road's; a circuit's road wraps).
pub fn checkpoints_passed(cps: &[f32], start: f32, length: f32, along: f32, loops: bool) -> u32 {
    let rel = |a: f32| if loops && length > 0.0 { (a - start).rem_euclid(length) } else { a - start };
    let me = rel(along);
    cps.iter().filter(|&&c| rel(c) <= me + 0.5).count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(n: u64, speed: f32, along_road: Option<f32>) -> Tick {
        Tick {
            tick: n,
            time: n as f32 / 60.0,
            dt: 1.0 / 60.0,
            car: 2,
            speed,
            along_road,
            racing: true,
            next_checkpoint: 3,
        }
    }

    #[test]
    fn a_hard_hit_is_one_crash() {
        let mut w = Watch::default();
        assert!(w.step(tick(0, 40.0, None)).is_empty());
        let e = w.step(tick(1, 25.0, None));
        assert_eq!(e.len(), 1);
        assert_eq!((e[0].kind, e[0].car, e[0].checkpoint), (EventKind::Crash, 2, 3));
        assert!((e[0].value - 15.0).abs() < 1e-4);
        // The rest of the same hit within a second: no second crash.
        assert!(w.step(tick(2, 10.0, None)).is_empty());
        // Braking hard is not a crash.
        let mut w = Watch::default();
        for n in 0..120 {
            assert!(w.step(tick(n, 40.0 - n as f32 * 0.25, None)).is_empty());
        }
    }

    #[test]
    fn standing_still_after_the_green_is_stuck_once() {
        let mut w = Watch::default();
        let mut stuck = 0;
        for n in 0..(STUCK_AFTER * 60.0 * 3.0) as u64 {
            stuck += w.step(tick(n, 0.2, Some(1.0))).iter().filter(|e| e.kind == EventKind::Stuck).count();
        }
        assert_eq!(stuck, 1);
        // Before the green: never.
        let mut w = Watch::default();
        for n in 0..600 {
            assert!(w.step(Tick { racing: false, ..tick(n, 0.0, None) }).is_empty());
        }
        // Moving again re-arms it.
        let mut w = Watch::default();
        let mut stuck = 0;
        for (n, v) in (0..300).map(|_| 0.0).chain((0..60).map(|_| 20.0)).chain((0..300).map(|_| 0.0)).enumerate() {
            stuck += w.step(tick(n as u64, v, None)).iter().filter(|e| e.kind == EventKind::Stuck).count();
        }
        assert_eq!(stuck, 2);
    }

    #[test]
    fn driving_against_the_road_is_wrong_way() {
        let mut w = Watch::default();
        let mut said = Vec::new();
        for n in 0..300 {
            said.extend(w.step(tick(n, 20.0, Some(-0.9))));
        }
        assert_eq!(said.iter().filter(|e| e.kind == EventKind::WrongWay).count(), 1);
        // Slow turning on the spot is not.
        let mut w = Watch::default();
        for n in 0..300 {
            assert!(w.step(tick(n, 2.0, Some(-0.9))).iter().all(|e| e.kind != EventKind::WrongWay));
        }
    }

    #[test]
    fn checkpoints_count_along_the_road() {
        let cps = [100.0, 300.0, 500.0];
        assert_eq!(checkpoints_passed(&cps, 0.0, 600.0, 50.0, false), 0);
        assert_eq!(checkpoints_passed(&cps, 0.0, 600.0, 301.0, false), 2);
        // A circuit starting at 400 m: 500 is the first, 100 and 300 after the wrap.
        assert_eq!(checkpoints_passed(&cps, 400.0, 600.0, 450.0, true), 0);
        assert_eq!(checkpoints_passed(&cps, 400.0, 600.0, 520.0, true), 1);
        assert_eq!(checkpoints_passed(&cps, 400.0, 600.0, 150.0, true), 2);
    }
}
