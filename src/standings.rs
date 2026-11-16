//! Race standings: the racers' order (0x5EEC90) and the time gaps the
//! leader board shows (0x5F82E0), read through the race's reference lap
//! (race status +0xC34, built by 0x5F6E80, read by 0x5EDDA0).

use glam::{Vec2, Vec3};

use crate::formats::ai_routes::RouteFile;
use crate::formats::roads::{Locator, RoadRoute};

/// The game's clock ticks 4000 times a second (0x865188).
pub const TICKS: f32 = 4000.0;

/// Seconds as clock ticks, the way the game stores a racer's finish
/// (+0x198) and the race clock.
pub fn ticks(seconds: f32) -> i32 {
    (seconds * TICKS) as i32
}

/// The Timer's rounding to whole 0.01 s on a tick count (0x577410(0.01)
/// → 0x577370; **verified** in the disassembly, x87 at 24-bit precision so
/// every step is an f32 operation): x = t · 0.00025 / 0.01; i = trunc(x),
/// less 1 when i > x; plus 1 when x − i ≥ 0.5 (half up); v = i · 0.01 ·
/// 4000, ceiled (0x5773E5); back to seconds (× 0.00025) and to ticks
/// (0x577410: × 4000 + 0.5, truncated). The result is a multiple of 40.
/// (The tick is exact as an f32 below 2^24 ticks, 70 minutes.)
pub fn round_centi(t: i32) -> i32 {
    let x = t as f32 * 0.00025 / 0.01;
    let mut i = x as i32 as f32;
    if i > x {
        i -= 1.0;
    }
    if !(x - i < 0.5) {
        i += 1.0;
    }
    let v = i * 0.01 * TICKS;
    let mut c = v as i32 as f32;
    if c < v {
        c += 1.0;
    }
    (c * 0.00025 * TICKS + 0.5) as i32
}

/// The reference lap (0x804 bytes): +0 the lap's time, then 256 bins of
/// the lap, each its rate (laps a second) and the time at its start.
#[derive(Clone, Debug)]
pub struct LapReference {
    pub lap_time: f32,
    pub bins: Vec<(f32, f32)>,
}

impl LapReference {
    /// 0x5F6E80's table: `found` gives rates (laps a second) where they
    /// start (a fraction of the lap, [`walk`]); the first rate found for a
    /// bin stays. A bin with none takes the bin before's rate, the first
    /// 40 m/s over the road's length (0x88F2B0, 0x5F7337); the bins' start
    /// times add up a 256th of the lap at each rate.
    pub fn new(road_length: f32, found: impl IntoIterator<Item = (f32, f32)>) -> LapReference {
        let road_length = road_length.max(1.0);
        let mut rates = [0.0f32; 256];
        for (at, rate) in found {
            if !(0.0..1.0).contains(&at) || rate <= 0.0 {
                continue;
            }
            let bin = &mut rates[(at * 256.0) as usize];
            if *bin == 0.0 {
                *bin = rate;
            }
        }
        let mut last = 40.0 / road_length;
        let mut time = 0.0;
        let mut bins = Vec::with_capacity(256);
        for rate in rates {
            let rate = if rate == 0.0 { last } else { rate };
            bins.push((rate, time));
            time += (1.0 / 256.0) / rate;
            last = rate;
        }
        LapReference { lap_time: time, bins }
    }

    /// 0x5EDDA0: the reference time (s) at `laps` laps into the race.
    pub fn time_at(&self, laps: f32) -> f32 {
        let laps = if laps >= 0.0 { laps } else { 0.0 };
        let whole = laps as i32;
        let frac = laps - whole as f32;
        let bin = ((frac * 256.0) as usize).min(255);
        let (rate, start) = self.bins[bin];
        whole as f32 * self.lap_time + start + (frac - bin as f32 / 256.0) / rate
    }
}

/// The race's road (the route object 0x88F298) with its start and finish
/// lines along it (race status +0xBF8 by +0xBF6 / +0xBF7, as distances).
pub struct RaceRoad<'a> {
    pub road: &'a RoadRoute,
    pub start_at: f32,
    pub finish_at: f32,
}

impl RaceRoad<'_> {
    /// 0x5D5CE0: `a − b` along the road, a circuit's taken the short way
    /// (past half its length, ± its length).
    pub fn difference(&self, a: f32, b: f32) -> f32 {
        let (l, d) = (self.road.length, a - b);
        if !self.road.loops {
            d
        } else if d < -l * 0.5 {
            d + l
        } else if d > l * 0.5 {
            d - l
        } else {
            d
        }
    }

    /// 0x5EE9C0: a place on the road as a fraction of the lap: a circuit's
    /// from the start line round to it (0x5D5D20, below 0 + 1), a sprint's
    /// from start to finish (past 1, − 1).
    pub fn fraction(&self, along: f32) -> f32 {
        if self.road.loops {
            let d = self.difference(along, self.start_at) / self.road.length.max(1.0);
            if d < 0.0 { d + 1.0 } else { d }
        } else {
            let f = (along - self.start_at) / (self.finish_at - self.start_at);
            if f > 1.0 { f - 1.0 } else { f }
        }
    }

    /// The locator (0x5DA340 → 0x5D9E40, `RoadRoute::locate`) from
    /// fresh: where a point lies along the road and the road segment of
    /// the node it chose.
    fn locate(&self, p: Vec2) -> (f32, Option<usize>) {
        let mut loc = Locator::default();
        self.road.locate(&mut loc, p);
        (loc.along, loc.node.map(|n| self.road.nodes[n].segment).or_else(|| self.road.segment_at(loc.along)))
    }
}

/// 0x5E4090 (radius 40 m, mask 4): the nearest racer point (flag bit 11)
/// to `at`, over the race records then the other listed ones (0x88D218 /
/// 0x88D21C), each record's best (0x5E2E50: a point whose road holds `at`
/// first, then the nearest) shrinking the radius to its 3D distance.
fn nearest(f: &RouteFile, at: Vec3, mut radius: f32) -> Option<(usize, usize)> {
    let listed = |r: &crate::formats::ai_routes::RouteRecord| r.flags >> 6 & 1 != 0;
    let order = f.records.iter().enumerate().filter(|(_, r)| r.is_race()).chain(f.records.iter().enumerate().filter(|(_, r)| listed(r) && !r.is_race()));
    let mut found = None;
    for (ri, r) in order {
        let mut best: Option<(bool, f32, usize)> = None;
        for (ci, c) in r.points.iter().enumerate() {
            let d = c.position.distance(at);
            if c.flags & 0x800 == 0 || d >= radius {
                continue;
            }
            let along = f.along(ri, ci, false, at.truncate());
            let side = f.lateral(ri, ci, false, at.truncate());
            let inside = (0.0..=c.length).contains(&along) && side >= -c.half_width[0] && side <= c.half_width[1];
            if best.is_none_or(|(bi, bd, _)| (inside && !bi) || (inside == bi && d < bd)) {
                best = Some((inside, d, ci));
            }
        }
        if let Some((_, d, ci)) = best {
            radius = d;
            found = Some((ri, ci));
        }
    }
    found
}

/// 0x5DD560 for a racer (type 0, links on): from a racer point (flag bit
/// 11), a junction of the other direction bit (bit 18 ≠ forward) of type
/// 2 or 0 to its target (bit 23 flips the direction), else the record's
/// next point that way, itself a racer point; None at a record's end.
fn next_point(f: &RouteFile, record: usize, cp: usize, mut forward: bool) -> Option<(usize, usize)> {
    let c = &f.records[record].points[cp];
    if c.flags >> 11 & 1 == 0 {
        return None;
    }
    let (mut record, mut cp) = (record, cp);
    for j in f.junctions_at(c) {
        let jn = &f.junctions[j];
        if jn.direction() == forward || !matches!(jn.kind(), 0 | 2) {
            continue;
        }
        let (rid, pid) = jn.target();
        let r = f.record(rid)?;
        let pid = usize::try_from(pid).ok().filter(|&p| p < f.records[r].points.len())?;
        if jn.flips() {
            forward = !forward;
        }
        if (r, pid) != (record, cp) {
            return Some((r, pid));
        }
        (record, cp) = (r, pid);
        break;
    }
    let n = f.records[record].points.len();
    let cp = if forward { (cp + 1 < n).then_some(cp + 1)? } else { cp.checked_sub(1)? };
    (f.records[record].points[cp].flags >> 11 & 1 != 0).then_some((record, cp))
}

/// 0x5F6E80's walk: from the racer point nearest the start line (+0xC10,
/// ForwardStartLine / BackwardStartLine) along the racer links, each point
/// of a record set for its direction (+0x74/+0x75, +0x7C/+0x7D; record
/// +0x70 bit 1 backward) gives, at its racing line's offset (+0x22 +
/// direction of the wider half width), a rate from 0 m to 1 m along it:
/// the lap fraction between them over 1 m at the racing line's speed
/// (+0x2C + direction), kept only on one road segment and with the road
/// distance 0.25–4 times the straight one. It ends past 3/4 of the road
/// once the last point is 0–200 m past the finish, at a dead end, or
/// after 10,001 points.
pub fn walk(f: &RouteFile, race: &RaceRoad, start: Vec3) -> Vec<(f32, f32)> {
    let mut found = Vec::new();
    let Some((mut record, mut cp)) = nearest(f, start, 40.0) else { return found };
    let mut travelled = 0.0;
    let mut last = race.start_at;
    for _ in 0..=10_000 {
        let r = &f.records[record];
        let back = r.flags >> 1 & 1 != 0;
        let dir = usize::from(back);
        let c = &r.points[cp];
        if r.fade[dir] {
            let lateral = f32::from(c.lane_position[dir]) / 128.0 * c.half_width[0].max(c.half_width[1]);
            let a = f.point(record, cp, back, 0.0, lateral).truncate();
            let b = f.point(record, cp, back, 1.0, lateral).truncate();
            let ((la, sa), (lb, sb)) = (race.locate(a), race.locate(b));
            last = lb;
            let (mut fa, mut fb) = (race.fraction(la), race.fraction(lb));
            if fb - fa > 0.5 {
                fa += 1.0;
            } else if fa - fb > 0.5 {
                fb += 1.0;
            }
            let straight = a.distance(b);
            let speed = f32::from(c.lane_speed[dir]);
            let ratio = race.difference(la, lb).abs() / straight;
            if speed > 0.0 && sa == sb && (0.25..=4.0).contains(&ratio) {
                let rate = (fb - fa) / (straight / speed);
                let at = fa.max(0.0);
                found.push((at - at.trunc(), rate));
            }
        }
        let Some((nr, nc)) = next_point(f, record, cp, !back) else { break };
        travelled += c.position.truncate().distance(f.records[nr].points[nc].position.truncate());
        if travelled > race.road.length * 0.75 && (0.0..200.0).contains(&race.difference(last, race.finish_at)) {
            break;
        }
        (record, cp) = (nr, nc);
    }
    found
}

/// The race lines (race status +0xBF8, percent of the road, count
/// +0xBF4; built by 0x5EE630 through 0x5EE500): line 0 the start line
/// (+0xBF6), the split lines (SplitTimeBoundary1–3), and the finish line
/// (+0xBF7), which is the start line on a circuit and its own line, last,
/// on a point to point race (0x89E7B4).
#[derive(Clone, Copy, Debug)]
pub struct RaceLines {
    pub at: [f32; 5],
    pub count: usize,
    pub finish: usize,
    /// The road closes on itself (route +0x24, read by 0x5D5D20).
    pub loops: bool,
}

impl RaceLines {
    /// The lines of `race` with no split markers, run forward.
    pub fn new(race: &RaceRoad) -> RaceLines {
        Self::with_splits(race, [None; 3], false)
    }

    /// The lines of `race` (0x5EE630): 0x5EE500 appends a marker's line,
    /// placed by a fresh locator (0x5DEDA0) as its fraction × 100, else
    /// its default percent (past 100 less 100) unless that is negative.
    /// Forward: the start line, SplitTimeBoundary1 (default start + 33),
    /// 2 (start + 66), 3 (none); run backward (0x89E7A4) 3, then 2 at
    /// start + 33 and 1 at start + 66. A point to point race adds its
    /// finish line last and moves a split still at its default (equal to
    /// start + 33 / + 66, past 100 less 100) to start + 0.33 / 0.66 ×
    /// (finish − start). `splits` are the SplitTimeBoundary1–3 markers'
    /// distances along the road, where found.
    pub fn with_splits(race: &RaceRoad, splits: [Option<f32>; 3], backward: bool) -> RaceLines {
        let pct = |m: f32| m / race.road.length.max(1.0) * 100.0;
        let wrap = |p: f32| if p > 100.0 { p - 100.0 } else { p };
        let mut lines = RaceLines { at: [0.0; 5], count: 0, finish: 0, loops: race.road.loops };
        let add = |lines: &mut RaceLines, marker: Option<f32>, default: f32| -> Option<usize> {
            let at = match marker {
                Some(m) => pct(m),
                None if wrap(default) < 0.0 => return None,
                None => wrap(default),
            };
            lines.at[lines.count] = at;
            lines.count += 1;
            Some(lines.count - 1)
        };
        let start = pct(race.start_at);
        add(&mut lines, Some(race.start_at), 5.0);
        let (near, far) = if backward {
            add(&mut lines, splits[2], -1.0);
            (add(&mut lines, splits[1], start + 33.0), add(&mut lines, splits[0], start + 66.0))
        } else {
            let near = add(&mut lines, splits[0], start + 33.0);
            let far = add(&mut lines, splits[1], start + 66.0);
            add(&mut lines, splits[2], -1.0);
            (near, far)
        };
        if !race.road.loops {
            let finish = pct(race.finish_at);
            lines.finish = add(&mut lines, Some(race.finish_at), finish).unwrap_or(0);
            for (line, by, k) in [(near, 33.0, 0.33), (far, 66.0, 0.66)] {
                if let Some(i) = line
                    && lines.at[i] == wrap(start + by)
                {
                    lines.at[i] = (finish - start) * k + start;
                }
            }
        }
        lines
    }

    /// 0x5D5D20: `a − b` in percent of the road, a loop's past ±50 taken
    /// the other way round.
    pub fn difference(&self, a: f32, b: f32) -> f32 {
        let d = a - b;
        if !self.loops {
            d
        } else if d < -50.0 {
            d + 100.0
        } else if d > 50.0 {
            d - 100.0
        } else {
            d
        }
    }

    /// 0x5EE160: did a racer going from `last` to `p` (percent) cross
    /// `line`? None when `p` is more than 25% from it; Some(true) forward
    /// (from behind it to on or past it), Some(false) backward.
    pub fn crossing(&self, p: f32, last: f32, line: f32) -> Option<bool> {
        let (a, b) = (self.difference(p, line), self.difference(last, line));
        if a.abs() > 25.0 {
            None
        } else if b < 0.0 && a >= 0.0 {
            Some(true)
        } else if b >= 0.0 && a < 0.0 {
            Some(false)
        } else {
            None
        }
    }

    /// 0x5EE9C0: `p` (percent) as a fraction of the lap: with the start
    /// and finish lines the same, from the line round to it (below 0 +
    /// 100) / 100; else from start to finish, past 1 less 1.
    pub fn fraction(&self, p: f32) -> f32 {
        let (start, finish) = (self.at[0], self.at[self.finish]);
        if start == finish {
            let d = self.difference(p, finish);
            (if d < 0.0 { d + 100.0 } else { d }) * 0.01
        } else {
            let f = (p - start) / (finish - start);
            if f > 1.0 { f - 1.0 } else { f }
        }
    }
}

/// A racer's laps (the racer status block, 0x1F0 bytes from race status
/// +0x30): set up by 0x5FC2B0, started by the green light (0x5FC910 state
/// 4), moved each frame by 0x5F7C40.
#[derive(Clone, Debug)]
pub struct Laps {
    /// +0x08: the lap count, −1 from 0x5FC2B0 (a circuit's grid is behind
    /// the line, so its first crossing makes it 0); a point to point race
    /// sets it 0 at the green light.
    pub lap: i32,
    /// +0x09: the most laps done; the race is over for the racer when it
    /// reaches the race's laps (+0xBE8, 0x608C11).
    pub most: i32,
    /// +0x0D: this frame `lap` passed `most` at the finish line.
    pub new_lap: bool,
    /// +0x04 / +0x06: the last line crossed forward as line + 5 × lap,
    /// the furthest so far (−1, 0 from the green light); +0x0E the frame
    /// it grows.
    pub reached: i32,
    pub best: i32,
    pub new_best: bool,
    /// +0x14 / +0x20: progress in laps, the lap count + 0x5EE9C0.
    pub progress: f32,
    /// +0x18: the progress rate, laps a second: each frame with a time
    /// step 0.333 of the old plus 0.667 of (new − old progress) / dt
    /// (0x5F7E84, 0x7A5694); 0 until then.
    pub rate: f32,
    /// The race clock of the last update (the frame time 0x865150).
    stamp: Option<f32>,
    /// +0x1C: last frame's place, percent of the road.
    pub last: f32,
    /// +0x30 + 4·line: the race clock (s) of each line's furthest
    /// crossing; line 0's set at the green light (none before).
    pub crossed: [Option<f32>; 5],
    /// +0x44 + 4·line: each line's time at its furthest crossing, from
    /// line 0's crossing (the split times; the finish line's is the
    /// lap's).
    pub split: [Option<f32>; 5],
}

impl Laps {
    /// 0x5FC2B0 with the car at `p` percent of the road; its 0x5F7C40
    /// call crosses nothing (+0x1C is `p` already) and leaves +0x14 /
    /// +0x20 at the lap count + the fraction.
    pub fn new(lines: &RaceLines, p: f32) -> Laps {
        Laps { lap: -1, most: 0, new_lap: false, reached: -1, best: -1, new_best: false, progress: -1.0 + lines.fraction(p), rate: 0.0, stamp: None, last: p, crossed: [None; 5], split: [None; 5] }
    }

    /// The green light (0x5FC910 state 4) at race clock `clock`: line 0's
    /// crossing time is now, the furthest line 0, and a point to point
    /// race's lap count and last line 0.
    pub fn go(&mut self, lines: &RaceLines, clock: f32) {
        self.crossed[0] = Some(clock);
        self.best = 0;
        if !lines.loops {
            self.lap = 0;
            self.reached = 0;
        }
    }

    /// 0x5F7C40 with the car at `p` percent of the road at race clock
    /// `clock`. A forward crossing of the finish line counts a lap
    /// (raising the most), a backward one takes one off (not below the
    /// most − 1). A forward crossing further than any before records the
    /// time since line 0's (+0x44 + 4·line); at the finish line that is
    /// the lap's time (racer +0x94 + 4·lap, laps 0–20), returned with
    /// the lap. Line 0's crossing time then moves on.
    pub fn update(&mut self, lines: &RaceLines, p: f32, clock: f32) -> Option<(i32, f32)> {
        self.new_lap = false;
        self.new_best = false;
        let mut stamped = None;
        for i in 0..lines.count {
            let Some(forward) = lines.crossing(p, self.last, lines.at[i]) else { continue };
            if !forward {
                if i == lines.finish {
                    self.lap = (self.lap - 1).max(self.most - 1);
                }
                continue;
            }
            if i == lines.finish {
                self.lap += 1;
                if self.lap > self.most {
                    self.most = self.lap;
                    self.new_lap = true;
                }
            }
            self.reached = i as i32 + self.lap * 5;
            if self.reached > self.best {
                self.best = self.reached;
                self.new_best = true;
                if let Some(since) = self.crossed[0] {
                    self.split[i] = Some(clock - since);
                    if i == lines.finish {
                        stamped = Some((self.lap, clock - since));
                    }
                }
                self.crossed[i] = Some(clock);
            }
        }
        let old = self.progress;
        self.progress = self.lap as f32 + lines.fraction(p);
        let dt = self.stamp.map_or(0.0, |t| clock - t);
        if dt > 0.0 {
            self.rate = (1.0 - 0.667) * self.rate + 0.667 * ((self.progress - old) / dt);
        }
        self.stamp = Some(clock);
        self.last = p;
        stamped
    }

    /// 0x608C11 (every race type but 7, each frame for each racer still
    /// running): done once the most laps reach the race's (+0xBE8; none
    /// when it is 0).
    pub fn done(&self, race_laps: u32) -> bool {
        race_laps != 0 && self.most == race_laps as i32
    }

    /// When the lap under way started: line 0's crossing.
    pub fn lap_start(&self) -> f32 {
        self.crossed[0].unwrap_or(0.0)
    }
}

/// What the standings read of a racer: its progress in laps (racer
/// +0x20) and its finish on the race clock, s (+0x198; finishing also
/// sets the finish order +0x0C).
#[derive(Clone, Copy, Debug, Default)]
pub struct Standing {
    pub laps: f32,
    pub finished: Option<f32>,
}

/// 0x5EEC10: does `b` go ahead of `a`? Two finishers by their finish,
/// else by progress. (Race type 7 compares +0x24 instead; the clone has
/// no such race.)
fn ahead(a: &Standing, b: &Standing) -> bool {
    match (a.finished, b.finished) {
        (Some(x), Some(y)) => x > y,
        _ => b.laps > a.laps,
    }
}

/// 0x5EEC90: bubble the racers' order (race status +0xBD0, kept from
/// frame to frame) until no pair swaps; a racer's place is its index + 1
/// (+0x0A).
pub fn rank(order: &mut [usize], racers: &[Standing]) {
    loop {
        let mut swapped = false;
        for i in 1..order.len() {
            if ahead(&racers[order[i - 1]], &racers[order[i]]) {
                order.swap(i - 1, i);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
}

/// 0x5F82E0: `row`'s gap to the player, ticks, negative when `row` is
/// ahead: their finishes once both are in, else their reference times at
/// their progress (a finisher at `laps`), each side still racing timed
/// from the race clock `clock`.
pub fn gap(row: &Standing, me: &Standing, reference: &LapReference, laps: u32, clock: f32) -> i32 {
    let t = |x: f32| reference.time_at(x);
    let ftol = |s: f32| (s * TICKS + 0.5) as i32;
    let full = laps as f32;
    match (row.finished, me.finished) {
        (Some(r), Some(p)) => ticks(r) - ticks(p),
        (Some(r), None) => ftol(t(me.laps) - t(full)) + ticks(r) - ticks(clock),
        (None, Some(p)) => ftol(t(full) - t(row.laps)) - ticks(p) + ticks(clock),
        (None, None) => ftol(t(me.laps) - t(row.laps)),
    }
}

#[cfg(test)]
mod tests {
    /// 0x577410(0.01): half up to 40 ticks (measured on the
    /// original: green tick 47522 → +0x10 47520, 32419 → 32400; the race
    /// clock 69, 133, 202, 269 ticks past it → 80, 120, 200, 280).
    #[test]
    fn the_timer_rounds_half_up_to_hundredths() {
        let r: Vec<i32> = [47522, 32419, 69, 133, 202, 269, 0, 19, 20, 60].into_iter().map(super::round_centi).collect();
        assert_eq!(r, [47520, 32400, 80, 120, 200, 280, 0, 0, 40, 80]);
    }

    use super::*;

    #[test]
    fn an_empty_reference_runs_at_forty() {
        let r = LapReference::new(4000.0, []);
        assert!((r.lap_time - 100.0).abs() < 1e-3);
        assert!((r.time_at(1.5) - 150.0).abs() < 1e-2);
        assert_eq!(r.time_at(-1.0), 0.0);
    }

    #[test]
    fn empty_bins_take_the_rate_before() {
        let r = LapReference::new(1000.0, [(0.0, 0.1)]);
        assert!((r.lap_time - 10.0).abs() < 1e-3);
    }

    fn circuit() -> RaceLines {
        RaceLines { at: [2.0, 0.0, 0.0, 0.0, 0.0], count: 1, finish: 0, loops: true }
    }

    /// Drive `laps` from `from` to `to` percent in 1% steps, a frame a
    /// second; the lap times stamped.
    fn drive(laps: &mut Laps, lines: &RaceLines, from: f32, to: f32, clock: &mut f32) -> Vec<(i32, f32)> {
        let mut out = Vec::new();
        let mut p = from;
        while p + 1.0 <= to {
            p += 1.0;
            *clock += 1.0;
            out.extend(laps.update(lines, p.rem_euclid(100.0), *clock));
        }
        out
    }

    #[test]
    fn a_circuit_counts_laps_at_the_finish_line() {
        let lines = circuit();
        // The grid just behind the line: lap −1, the first crossing makes
        // it 0 and is no lap; each one after is.
        let mut laps = Laps::new(&lines, 99.0);
        assert_eq!((laps.lap, laps.most), (-1, 0));
        assert!((laps.progress - -0.03).abs() < 1e-4);
        laps.go(&lines, 0.0);
        let mut clock = 0.0;
        let stamps = drive(&mut laps, &lines, 99.0, 104.0, &mut clock);
        assert_eq!((laps.lap, laps.most, laps.new_lap), (0, 0, false));
        assert!(stamps.is_empty());
        assert!(!laps.done(2));
        let stamps = drive(&mut laps, &lines, 104.0, 202.0, &mut clock);
        // Lap 1: the line crossed 103 frames after the green light.
        assert_eq!(stamps, [(1, 103.0)]);
        assert_eq!((laps.lap, laps.most), (1, 1));
        assert!((laps.progress - 1.0).abs() < 1e-4);
        let stamps = drive(&mut laps, &lines, 202.0, 302.0, &mut clock);
        assert_eq!(stamps, [(2, 100.0)]);
        assert!(laps.done(2));
    }

    #[test]
    fn backing_over_the_line_takes_a_lap_off_once() {
        let lines = circuit();
        let mut laps = Laps::new(&lines, 99.0);
        laps.go(&lines, 0.0);
        let mut clock = 0.0;
        drive(&mut laps, &lines, 99.0, 202.0, &mut clock);
        assert_eq!((laps.lap, laps.most), (1, 1));
        // Back over the line, round the loop out of the window and back
        // over it again: no lower than the most − 1.
        for p in [1.0, 50.0, 3.0, 1.0] {
            clock += 1.0;
            laps.update(&lines, p, clock);
        }
        assert_eq!((laps.lap, laps.most), (0, 1));
        // Forward again: the lap comes back but is no new one, no time.
        clock += 1.0;
        assert_eq!(laps.update(&lines, 3.0, clock), None);
        assert_eq!((laps.lap, laps.most, laps.new_lap), (1, 1, false));
    }

    #[test]
    fn a_jump_past_the_window_crosses_nothing() {
        let lines = circuit();
        let mut laps = Laps::new(&lines, 99.0);
        laps.go(&lines, 0.0);
        // 99% to 30%: 28% past the line, outside the 25% window.
        assert_eq!(laps.update(&lines, 30.0, 1.0), None);
        assert_eq!(laps.lap, -1);
    }

    #[test]
    fn split_lines_take_their_defaults() {
        // No shipped track places SplitTimeBoundary markers.
        let straight = RoadRoute::new(vec![Vec2::ZERO, Vec2::new(1000.0, 0.0)], false);
        let race = RaceRoad { road: &straight, start_at: straight.length * 0.05, finish_at: straight.length * 0.95 };
        let lines = RaceLines::with_splits(&race, [None; 3], false);
        assert_eq!((lines.count, lines.finish), (4, 3));
        // A sprint's splits move to a third and two thirds of the way.
        for (got, want) in lines.at[..4].iter().zip([5.0, 5.0 + 0.33 * 90.0, 5.0 + 0.66 * 90.0, 95.0]) {
            assert!((got - want).abs() < 1e-3, "{:?}", lines.at);
        }
        let ring = RoadRoute::new(vec![Vec2::ZERO, Vec2::new(500.0, 0.0), Vec2::new(500.0, 500.0), Vec2::new(0.0, 500.0)], true);
        let race = RaceRoad { road: &ring, start_at: ring.length * 0.5, finish_at: ring.length * 0.5 };
        let lines = RaceLines::with_splits(&race, [None; 3], false);
        assert_eq!((lines.count, lines.finish), (3, 0));
        // A circuit's at start + 33 and + 66, past 100 less 100.
        for (got, want) in lines.at[..3].iter().zip([50.0, 83.0, 16.0]) {
            assert!((got - want).abs() < 1e-3, "{:?}", lines.at);
        }
        // The splits are stamped from line 0's crossing.
        let mut laps = Laps::new(&lines, 49.0);
        laps.go(&lines, 0.0);
        let mut clock = 0.0;
        drive(&mut laps, &lines, 49.0, 120.0, &mut clock);
        assert_eq!(laps.split[1], Some(34.0));
        assert_eq!(laps.split[2], Some(67.0));
    }

    #[test]
    fn a_sprint_finishes_at_its_finish_line() {
        let lines = RaceLines { at: [5.0, 95.0, 0.0, 0.0, 0.0], count: 2, finish: 1, loops: false };
        let mut laps = Laps::new(&lines, 4.0);
        laps.go(&lines, 0.0);
        assert_eq!((laps.lap, laps.most), (0, 0));
        let mut clock = 0.0;
        let stamps = drive(&mut laps, &lines, 4.0, 94.0, &mut clock);
        assert!(stamps.is_empty() && !laps.done(1));
        assert!((laps.progress - (94.0 - 5.0) / 90.0).abs() < 1e-4);
        let stamps = drive(&mut laps, &lines, 94.0, 95.0, &mut clock);
        assert_eq!(stamps, [(1, 91.0)]);
        assert!(laps.done(1) && laps.new_lap);
        laps.update(&lines, 96.0, clock + 1.0);
        assert!(!laps.new_lap);
        // Past the finish 0x5EE9C0 wraps the fraction: 1 + (f − 1).
        assert!((laps.progress - (1.0 + (96.0 - 5.0) / 90.0 - 1.0)).abs() < 1e-4);
    }

    #[test]
    fn ranking_and_gaps() {
        let r = LapReference::new(4000.0, []);
        let me = Standing { laps: 0.5, finished: None };
        let rival = Standing { laps: 0.6, finished: None };
        let mut order = vec![0, 1];
        rank(&mut order, &[me, rival]);
        assert_eq!(order, [1, 0]);
        // 0.1 lap ahead at 40 m/s over 4 km: 10 s.
        assert_eq!(gap(&rival, &me, &r, 1, 30.0), -39_999);
        let done = Standing { laps: 1.0, finished: Some(80.0) };
        rank(&mut order, &[me, done]);
        assert_eq!(order, [1, 0]);
        // The finisher's 80 s against the player's 60 s + 50 s to go.
        assert_eq!(gap(&done, &me, &r, 1, 60.0), ticks(80.0) - ticks(60.0) + (-50.0 * TICKS + 0.5) as i32);
    }
}
