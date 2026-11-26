//! Telemetry read from a recording: one car's speed over time, how far it was from the race's racing line
//! (left or right of it), and the events that touched it. Everything is worked out from the run file's records
//! (docs/modding/asset-format/runs.md); a channel the file does not carry (the drive channels of a run written
//! without them: gear, rpm and the pedals) is `None`. Plain data, so the
//! values are checked against the file without a window.

use glam::Vec2;

use super::format::{CarSample, EventKind, StateHash};
use super::record::Recording;

/// One channel over time: (time in s, value) in time order.
pub type Series = Vec<(f32, f32)>;

/// One car's telemetry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Telemetry {
    /// The car's speed (m/s) at each recorded frame.
    pub speed: Series,
    /// Its signed distance (m) from the racing line, left of the way it runs positive; none when the run's race
    /// is not in the open mod (no line to measure from).
    pub lateral: Option<Series>,
    /// The events of the car: time, kind, checkpoint.
    pub events: Vec<(f32, EventKind, u32)>,
    /// The drive channels (a run written with them): -1 reverse, 0 neutral, 1.. the gear.
    pub gear: Option<Series>,
    /// The shown engine speed (rpm).
    pub rpm: Option<Series>,
    /// The pedals, 0..1.
    pub throttle: Option<Series>,
    pub brake: Option<Series>,
    /// The steering input, -1 (left) .. 1 (right).
    pub steer: Option<Series>,
}

/// The racing line of a race: where it starts, its checkpoints in order, where it finishes; a circuit closes the
/// loop from the last checkpoint back to the first.
pub struct Line {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

impl Line {
    /// The line of a custom race (none under two points).
    pub fn of(race: &ob_map::tools::map::format::CustomRace) -> Option<Line> {
        let mut points: Vec<Vec2> = Vec::new();
        let closed = race.kind.loops();
        if !closed && let Some(s) = race.start.or(race.grid.first().copied()) {
            points.push(s.position.truncate());
        }
        points.extend(race.checkpoints.iter().copied());
        if !closed && let Some(f) = race.finish {
            points.push(f.position.truncate());
        }
        points.dedup();
        (points.len() >= 2).then_some(Line { points, closed })
    }

    /// The signed distance of `p` from the line: to its nearest piece, positive left of the way the piece runs.
    pub fn offset(&self, p: Vec2) -> f32 {
        let n = self.points.len();
        let pieces = if self.closed { n } else { n - 1 };
        let mut best = (f32::INFINITY, 0.0f32);
        for i in 0..pieces {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            let d = b - a;
            let len2 = d.length_squared();
            let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let off = p - (a + d * t);
            let dist = off.length();
            if dist < best.0 {
                // Left of the direction d: the perpendicular (-d.y, d.x).
                let side = if len2 > 0.0 { d.perp().dot(off).signum() } else { 1.0 };
                best = (dist, if dist == 0.0 { 0.0 } else { side * dist });
            }
        }
        best.1
    }
}

/// The telemetry of car `id` in `rec`.
pub fn of(rec: &Recording, id: u32, line: Option<&Line>) -> Telemetry {
    let mut t = Telemetry { lateral: line.map(|_| Vec::new()), ..Default::default() };
    for f in &rec.frames {
        let Some(c) = f.cars.iter().find(|c| c.id == id) else { continue };
        t.speed.push((f.time, c.speed));
        if let Some(d) = c.drive {
            let mut put = |s: &mut Option<Series>, v: f32| s.get_or_insert_with(Vec::new).push((f.time, v));
            put(&mut t.gear, d.gear as f32);
            put(&mut t.rpm, d.rpm);
            put(&mut t.throttle, d.throttle);
            put(&mut t.brake, d.brake);
            put(&mut t.steer, d.steer);
        }
        if let (Some(l), Some(v)) = (line, t.lateral.as_mut()) {
            v.push((f.time, l.offset(c.position.truncate())));
        }
    }
    t.events = rec.events.iter().filter(|e| e.car == id).map(|e| (e.time, e.kind, e.checkpoint)).collect();
    t
}

/// The value of `s` at `time` (the frame at or before it; none before the first).
pub fn value_at(s: &[(f32, f32)], time: f32) -> Option<f32> {
    let k = s.partition_point(|p| p.0 <= time);
    s.get(k.checked_sub(1)?).map(|p| p.1)
}

/// The least and the most of a series (none when empty).
pub fn range(s: &[(f32, f32)]) -> Option<(f32, f32)> {
    let mut it = s.iter().map(|p| p.1);
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
}

/// How two runs compare: where they part and how far apart their cars are.
#[derive(Clone, Debug, PartialEq)]
pub struct Parting {
    /// The tick of the first state hash that differs (the same seed and the same game give none). With the hash
    /// every tick it is the tick the runs parted at.
    pub hash_tick: Option<u64>,
    /// The time the runs first differ, when both have a frame of that tick.
    pub time: Option<f32>,
    /// The first time (s) the car `id` of B is over a metre from where A has it, at the same time.
    pub apart: Option<f32>,
}

/// Where run `b` parts from run `a` (compared at the same race time).
pub fn parting(a: &Recording, b: &Recording, id: u32) -> Parting {
    // By tick: two runs may carry the hash at another interval (every tick, or every 60), and a hash covers every
    // tick before it, so the first tick both have where it differs is where they had parted by.
    let hash_tick = a
        .hashes
        .iter()
        .find(|(tick, h)| b.hashes.binary_search_by_key(tick, |x| x.0).is_ok_and(|k| b.hashes[k].1 != *h))
        .map(|x| x.0);
    let time = hash_tick.and_then(|t| {
        // The hash at tick T covers every tick up to T: the run parts somewhere in the interval before (the
        // one tick itself when the runs carry the hash every tick).
        let every = |r: &Recording| r.header.as_ref().map_or(StateHash::EVERY, |h| h.hash_interval());
        let every = every(a).max(every(b));
        let from = t.saturating_sub(every.saturating_sub(1));
        let first = |r: &Recording| r.frames.iter().find(|f| f.tick >= from).map(|f| f.time);
        first(a).or(first(b))
    });
    let apart = a.frames.iter().find_map(|f| {
        let ca = f.cars.iter().find(|c| c.id == id)?;
        let cb: CarSample = *b.cars_at(f.time).iter().find(|c| c.id == id)?;
        (ca.position.distance(cb.position) > 1.0).then_some(f.time)
    });
    Parting { hash_tick, time, apart }
}

#[cfg(test)]
mod tests {
    use crate::runs::fixtures::race;
    use super::*;

    fn straight() -> Line {
        Line { points: vec![Vec2::new(-100.0, 0.0), Vec2::new(1300.0, 0.0)], closed: false }
    }

    #[test]
    fn offset_is_signed_left_positive() {
        let l = straight();
        let near = |a: f32, b: f32| assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        near(l.offset(Vec2::new(10.0, 0.0)), 0.0);
        near(l.offset(Vec2::new(10.0, 40.0)), 40.0);
        near(l.offset(Vec2::new(10.0, -3.0)), -3.0);
        // Past the end: the distance to the end point.
        assert!((l.offset(Vec2::new(1304.0, 3.0)).abs() - 5.0).abs() < 1e-4);
        // A loop closes: a square run counter-clockwise has its inside on the left.
        let sq = Line {
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)],
            closed: true,
        };
        assert!((sq.offset(Vec2::new(5.0, 2.0)) - 2.0).abs() < 1e-3);
        assert!(sq.offset(Vec2::new(0.0, 5.0)).abs() < 1e-3, "the closing piece");
    }

    #[test]
    fn values_match_the_run_file() {
        let rec = race(1, true, false);
        let l = straight();
        // The player's slot leaves the road (y = 40) from 30 s to 34 s at 20 m/s.
        let t = of(&rec, 0, Some(&l));
        assert_eq!(t.speed.len(), rec.frames.len());
        for (f, s) in rec.frames.iter().zip(&t.speed) {
            assert_eq!(*s, (f.time, f.cars[0].speed));
        }
        let lat = t.lateral.as_ref().unwrap();
        let at = |t: f32| value_at(lat, t).unwrap();
        for (t, want) in [(29.75, 0.0), (30.0, 40.0), (34.0, 40.0), (34.25, 0.0)] {
            assert!((at(t) - want).abs() < 1e-3, "{t}: {}", at(t));
        }
        let (lo, hi) = range(lat).unwrap();
        assert!(lo.abs() < 1e-3 && (hi - 40.0).abs() < 1e-3);
        assert_eq!(value_at(&t.speed, -5.0), None);
        assert_eq!(value_at(&t.speed, 0.0), Some(20.0));
        // Racer 1 stops from 10 s; its events are its own.
        let t1 = of(&rec, 1, None);
        assert_eq!(value_at(&t1.speed, 12.0), Some(0.0));
        assert!(t1.lateral.is_none());
        assert_eq!(
            t1.events.iter().map(|e| (e.1, e.2)).collect::<Vec<_>>(),
            [(EventKind::Checkpoint, 1), (EventKind::Checkpoint, 2), (EventKind::Stuck, 3)]
        );
    }

    #[test]
    fn two_runs_part_where_their_cars_do() {
        let a = race(1, true, false);
        assert_eq!(parting(&a, &a, 2), Parting { hash_tick: None, time: None, apart: None });
        // The second run has racer 2 stuck (the first crash at 1.5 s is not in A): no hash lines, so by position.
        let b = race(1, true, true);
        let p = parting(&a, &b, 0);
        assert_eq!(p.apart, None, "the player's slot drove the same");
        let mut c = a.clone();
        for f in c.frames.iter_mut().filter(|f| f.time >= 12.0) {
            f.cars[2].position.y += 3.0;
        }
        assert_eq!(parting(&a, &c, 2).apart, Some(12.0));
        // A hash that differs names its tick, and the time of the frames it covers.
        let mut d = a.clone();
        d.hashes = vec![(59, 1), (119, 2)];
        let mut e = a.clone();
        e.hashes = vec![(59, 1), (119, 3)];
        let p = parting(&d, &e, 0);
        assert_eq!(p.hash_tick, Some(119));
        assert!(p.time.is_some());
    }

    /// A run of `n` ticks of one car, with the drive channels when `drive`, and the state hash every tick when
    /// `every` is 1 (every 60 otherwise); `hash(tick)` is the hash it carries.
    fn driven(n: u64, drive: bool, every: u64, hash: impl Fn(u64) -> u64) -> Recording {
        use crate::runs::format::{CarKind, Drive, Frame, Header, Line as L};
        let mut r = Recording::default();
        r.push(L::Header(Header { version: 1, hash_every: if every == 1 { 1 } else { 0 }, ..Default::default() }));
        for tick in 0..n {
            let d = Drive { gear: (tick / 2) as i32 - 1, rpm: 3000.0 + tick as f32, throttle: 1.0, brake: 0.0, handbrake: 0.0, steer: -0.5, nitrous: false };
            let car = CarSample {
                id: 0,
                kind: CarKind::Player,
                position: glam::Vec3::new(tick as f32, 0.0, 0.0),
                rotation: glam::Quat::IDENTITY,
                speed: 5.0,
                lap: 0,
                checkpoint: 0,
                place: 1,
                wheels: None,
                drive: drive.then_some(d),
            };
            r.push(L::Frame(Frame { tick, time: tick as f32 / 60.0, cars: vec![car] }));
            if (tick + 1) % every == 0 {
                r.push(L::Hash { tick, hash: crate::runs::format::StateHash(hash(tick)) });
            }
        }
        r
    }

    #[test]
    fn the_drive_channels_are_series_of_the_car_and_a_run_without_them_has_none() {
        let with = of(&driven(6, true, 60, |_| 0), 0, None);
        assert_eq!(with.gear.as_deref().map(|g| g.iter().map(|p| p.1).collect::<Vec<_>>()), Some(vec![-1.0, -1.0, 0.0, 0.0, 1.0, 1.0]));
        assert_eq!(with.throttle.as_ref().map(Vec::len), Some(6));
        assert_eq!(value_at(with.steer.as_ref().unwrap(), 0.05), Some(-0.5));
        assert_eq!(value_at(with.rpm.as_ref().unwrap(), 3.0 / 60.0), Some(3003.0));
        let without = of(&driven(6, false, 60, |_| 0), 0, None);
        assert!(without.gear.is_none() && without.rpm.is_none() && without.throttle.is_none() && without.brake.is_none());
        assert_eq!(without.speed.len(), 6, "the poses are there either way");
    }

    #[test]
    fn a_hash_every_tick_names_the_tick_the_runs_parted_at() {
        let a = driven(200, true, 1, |t| t);
        // B has the same state until tick 123, then another hash for every tick after (a hash covers all before it).
        let b = driven(200, true, 1, |t| if t >= 123 { t + 1_000_000 } else { t });
        let p = parting(&a, &b, 0);
        assert_eq!(p.hash_tick, Some(123));
        assert_eq!(p.time, Some(123.0 / 60.0), "the tick itself, not a minute's window");
        // Against a run that carries the hash every 60 ticks the common ticks decide, within that interval.
        let c = driven(200, false, 60, |t| if t >= 123 { t + 1_000_000 } else { t });
        let p = parting(&a, &c, 0);
        assert_eq!(p.hash_tick, Some(179));
        assert!(p.time.is_some_and(|t| t <= 179.0 / 60.0 && t >= 120.0 / 60.0), "{:?}", p.time);
        assert_eq!(parting(&a, &a, 0).hash_tick, None);
    }
}
