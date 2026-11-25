//! "What went wrong": the problems of one simulated run, or of several runs
//! with different seeds, grouped by kind and place ("Stuck before checkpoint
//! 2: 3 of 5 runs"), each with the spot, the time and the car to show. Built
//! from what the run file already records (the events, the frames, the
//! results), the mod's props and the base track's roads; plain data, tested
//! without a window.

use glam::{Vec2, Vec3};

use super::detect::MOVING_AGAIN;
use super::format::{CarKind, EventKind};
use super::record::Recording;
use crate::editor::state::Level;

/// A crash this close to a mod prop (horizontal, m) is a crash into it.
pub const PROP_NEAR: f32 = 6.0;
/// A crash this close to another car (m) is a crash into it.
pub const CAR_NEAR: f32 = 5.5;
/// Farther than this from the base track's roads (m) is off them (as a
/// checkpoint is in Validation).
pub const OFF_ROAD: f32 = ob_studio::validate::CHECKPOINT_OFF_ROAD_M;
/// Off the roads this long counts (s).
pub const OFF_ROAD_FOR: f32 = 2.0;
/// The racers' places are checked for the roads this often (s).
const OFF_ROAD_STEP: f32 = 0.25;
/// Trouble this soon after the green (s) and this close to the car's grid
/// spot (m) is trouble at the start (a start line facing the wrong way).
pub const START_SECONDS: f32 = 6.0;
pub const START_NEAR: f32 = 40.0;
/// Occurrences listed under a group at most (the rest counted).
pub const SHOWN: usize = 8;

/// What a finding is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Cars in trouble right after the green near their grid spots.
    Start,
    Stuck,
    /// A crash into a mod prop (its label).
    CrashProp(String),
    CrashCar,
    CrashWall,
    WrongWay,
    /// A checkpoint skipped (passed over in one jump) or never reached.
    Missed,
    OffRoad,
    /// The first to the last finish, and who did not finish (information).
    Spread,
}

impl Kind {
    pub fn level(&self) -> Level {
        if *self == Kind::Spread { Level::Info } else { Level::Warning }
    }
}

/// One thing that went wrong in one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub kind: Kind,
    /// The checkpoint the car was heading for (1-based; 0 none).
    pub checkpoint: u32,
    /// The run's seed.
    pub seed: u64,
    pub car: u32,
    /// The race clock to show.
    pub time: f32,
    pub at: Option<Vec3>,
    /// The occurrence in words, without the group's ("Nissan 350Z: 12 s").
    pub text: String,
}

/// The findings of one run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunFindings {
    pub seed: u64,
    pub findings: Vec<Finding>,
    /// First to last finish (s) and the racers that did not finish.
    pub spread: Option<f32>,
    pub dnf: u32,
}

/// What the summary needs from the mod and the world: the mod's props
/// (label, place) and the distance to the base track's roads.
pub struct Context<'a> {
    pub props: &'a [(String, Vec3)],
    pub road: Option<&'a dyn Fn(Vec2) -> f32>,
}

fn clock(t: f32) -> String {
    let sign = if t < 0.0 { "-" } else { "" };
    let t = t.abs();
    format!("{sign}{}:{:04.1}", (t / 60.0) as u32, t % 60.0)
}

/// The racer's place at `time` (the frame at or after it).
fn place_at(rec: &Recording, car: u32, time: f32) -> Option<Vec3> {
    let k = rec.frames.partition_point(|f| f.time < time);
    rec.frames.get(k).or(rec.frames.last())?.cars.iter().find(|c| c.id == car).map(|c| c.position)
}

/// Read one run.
pub fn run_findings(rec: &Recording, cx: &Context) -> RunFindings {
    let seed = rec.header.as_ref().map_or(0, |h| h.seed);
    let racers = rec.racers();
    let mut out: Vec<Finding> = Vec::new();
    let find = |kind: Kind, checkpoint: u32, car: u32, time: f32, at: Option<Vec3>, text: String| Finding {
        kind,
        checkpoint,
        seed,
        car,
        time,
        at,
        text,
    };
    let green = rec.events.iter().find(|e| e.kind == EventKind::Start).map_or(0.0, |e| e.time);
    let grid = |car: u32| place_at(rec, car, green);
    let mut start_trouble: Vec<(u32, f32, Option<Vec3>, &'static str)> = Vec::new();

    for e in &rec.events {
        let name = rec.name(e.car);
        let at = place_at(rec, e.car, e.time);
        let near_start = e.time - green <= START_SECONDS
            && matches!((at, grid(e.car)), (Some(a), Some(g)) if a.truncate().distance(g.truncate()) <= START_NEAR);
        match e.kind {
            EventKind::Stuck => {
                // From when it stopped to when it moved again (or the end).
                let from = e.time - e.value;
                let moved = rec
                    .frames
                    .iter()
                    .filter(|f| f.time > e.time)
                    .find(|f| f.cars.iter().any(|c| c.id == e.car && c.speed > MOVING_AGAIN))
                    .map(|f| f.time);
                let end = rec.span().map_or(e.time, |s| s.1);
                let long = moved.unwrap_or(end) - from;
                let still = if moved.is_none() { ", never moved again" } else { "" };
                if near_start {
                    start_trouble.push((e.car, e.time, at, "stuck"));
                }
                out.push(find(
                    Kind::Stuck,
                    e.checkpoint,
                    e.car,
                    e.time,
                    at,
                    format!("{name}: {:.0} s{still}", long.max(e.value)),
                ));
            }
            EventKind::Crash => {
                let p = at.unwrap_or(Vec3::ZERO);
                let prop = at.and_then(|a| {
                    cx.props
                        .iter()
                        .map(|(n, q)| (n, q.truncate().distance(a.truncate())))
                        .filter(|(_, d)| *d <= PROP_NEAR)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(n, _)| n.clone())
                });
                let k = rec.frames.partition_point(|f| f.time < e.time);
                let other = at.and(rec.frames.get(k)).and_then(|f| {
                    f.cars
                        .iter()
                        .filter(|c| c.id != e.car)
                        .map(|c| (c.id, c.position.distance(p)))
                        .filter(|(_, d)| *d <= CAR_NEAR)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(id, _)| id)
                });
                let kmh = e.value * 3.6;
                let (kind, text) = match (prop, other) {
                    (Some(n), _) => (Kind::CrashProp(n), format!("{name}: {kmh:.0} km/h lost")),
                    (None, Some(o)) => (Kind::CrashCar, format!("{name} into {}: {kmh:.0} km/h lost", rec.name(o))),
                    _ => (Kind::CrashWall, format!("{name}: {kmh:.0} km/h lost")),
                };
                if near_start {
                    start_trouble.push((e.car, e.time, at, "crashed"));
                }
                out.push(find(kind, e.checkpoint, e.car, e.time, at, text));
            }
            EventKind::WrongWay => {
                out.push(find(Kind::WrongWay, e.checkpoint, e.car, e.time, at, name));
            }
            _ => {}
        }
    }

    // Checkpoints: skipped in one jump, or never reached by a racer that
    // did not finish.
    let ended = rec.end.is_some();
    let total = rec.header.as_ref().map_or(0, |h| h.checkpoints);
    for &car in &racers {
        let mut last = 0u32;
        for e in rec.events.iter().filter(|e| e.car == car && e.kind == EventKind::Checkpoint) {
            if e.checkpoint < last {
                last = 0; // a new lap
            }
            for skipped in last + 1..e.checkpoint {
                let at = place_at(rec, car, e.time);
                out.push(find(
                    Kind::Missed,
                    skipped,
                    car,
                    e.time,
                    at,
                    format!("{}: passed over it at {}", rec.name(car), clock(e.time)),
                ));
            }
            last = e.checkpoint;
        }
        let finished = rec.events.iter().any(|e| e.car == car && e.kind == EventKind::Finish)
            || rec.results.iter().any(|r| r.car == car && r.place > 0);
        if ended && !finished && (total == 0 || last < total) {
            let t = rec.span().map_or(0.0, |s| s.1);
            out.push(find(
                Kind::Missed,
                last + 1,
                car,
                t,
                place_at(rec, car, t),
                format!("{}: never reached it", rec.name(car)),
            ));
        }
    }

    // Off the base track's roads: the farthest spot of each spell.
    if let Some(road) = cx.road {
        for &car in &racers {
            let mut spell: Option<(f32, f32, f32, Vec3, u32)> = None; // start, last, farthest, there, checkpoint
            let mut next_t = f32::NEG_INFINITY;
            let close = |spell: &mut Option<(f32, f32, f32, Vec3, u32)>, out: &mut Vec<Finding>| {
                if let Some((t0, t1, d, p, cp)) = spell.take()
                    && t1 - t0 >= OFF_ROAD_FOR
                {
                    out.push(find(
                        Kind::OffRoad,
                        cp,
                        car,
                        t0,
                        Some(p),
                        format!("{}: {d:.0} m off for {:.0} s", rec.name(car), t1 - t0),
                    ));
                }
            };
            for f in rec.frames.iter().filter(|f| f.time > green) {
                if f.time < next_t {
                    continue;
                }
                next_t = f.time + OFF_ROAD_STEP;
                let Some(c) = f.cars.iter().find(|c| c.id == car) else { continue };
                let d = road(c.position.truncate());
                if d > OFF_ROAD {
                    let s = spell.get_or_insert((f.time, f.time, d, c.position, c.checkpoint + 1));
                    s.1 = f.time;
                    if d > s.2 {
                        (s.2, s.3) = (d, c.position);
                    }
                } else {
                    close(&mut spell, &mut out);
                }
            }
            close(&mut spell, &mut out);
        }
    }

    // Trouble at the start: one line per car.
    let mut seen = Vec::new();
    for (car, time, at, what) in start_trouble {
        if seen.contains(&car) {
            continue;
        }
        seen.push(car);
        out.push(find(
            Kind::Start,
            0,
            car,
            time,
            at,
            format!("{}: {what} {:.1} s after the green", rec.name(car), time - green),
        ));
    }

    // The finish spread.
    let finishes: Vec<(u32, f32)> = rec
        .events
        .iter()
        .filter(|e| e.kind == EventKind::Finish && racers.contains(&e.car))
        .map(|e| (e.car, e.value))
        .collect();
    let spread = (finishes.len() >= 2).then(|| {
        let lo = finishes.iter().map(|f| f.1).fold(f32::MAX, f32::min);
        let hi = finishes.iter().map(|f| f.1).fold(f32::MIN, f32::max);
        hi - lo
    });
    let dnf = if ended {
        racers
            .iter()
            .filter(|&&c| !finishes.iter().any(|f| f.0 == c))
            .filter(|&&c| rec.cars.get(&c).is_some_and(|i| i.kind != CarKind::Traffic))
            .count() as u32
    } else {
        0
    };
    if let Some(sp) = spread {
        let last = finishes.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let t = rec.events.iter().find(|e| e.kind == EventKind::Finish && e.car == last.0).map_or(last.1, |e| e.time);
        let first = finishes.iter().map(|f| f.1).fold(f32::MAX, f32::min);
        out.push(find(
            Kind::Spread,
            0,
            last.0,
            t,
            place_at(rec, last.0, t),
            format!("{:.1} s: first {}, last {} ({})", sp, clock(first), clock(last.1), rec.name(last.0)),
        ));
    }
    RunFindings { seed, findings: out, spread, dnf }
}

/// One line of the summary: a kind at a place, over every run.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub kind: Kind,
    pub checkpoint: u32,
    /// "Stuck before checkpoint 2: 3 of 5 runs, 4 times".
    pub text: String,
    /// The runs it happened in, of all.
    pub runs: usize,
    pub of: usize,
    pub items: Vec<Finding>,
}

impl Group {
    pub fn level(&self) -> Level {
        self.kind.level()
    }
}

fn before(cp: u32) -> String {
    if cp > 0 { format!(" before checkpoint {cp}") } else { String::new() }
}

/// The group's own words, without the counts.
fn head(kind: &Kind, cp: u32) -> String {
    match kind {
        Kind::Start => "Cars in trouble at the start: check the start line's heading and the grid".to_string(),
        Kind::Stuck => format!("Stuck{}", before(cp)),
        Kind::CrashProp(n) => format!("Crashed into prop {n}{}", before(cp)),
        Kind::CrashCar => format!("Crashed into another car{}", before(cp)),
        Kind::CrashWall => format!("Crashed into a wall or the scenery{}", before(cp)),
        Kind::WrongWay => format!("Drove the wrong way{}", before(cp)),
        Kind::Missed => format!("Missed checkpoint {cp}"),
        Kind::OffRoad => format!("Left the base track's roads{}", before(cp)),
        Kind::Spread => "Finish spread".to_string(),
    }
}

/// The summary over every run, the worst first: by how many runs it
/// happened in, then its kind and checkpoint; the finish spread last.
pub fn summarize(runs: &[RunFindings]) -> Vec<Group> {
    let of = runs.len();
    let mut groups: Vec<Group> = Vec::new();
    for r in runs {
        for f in &r.findings {
            let cp = if f.kind == Kind::Start || f.kind == Kind::Spread { 0 } else { f.checkpoint };
            let g = match groups.iter_mut().position(|g| g.kind == f.kind && g.checkpoint == cp) {
                Some(k) => &mut groups[k],
                None => {
                    groups.push(Group {
                        kind: f.kind.clone(),
                        checkpoint: cp,
                        text: String::new(),
                        runs: 0,
                        of,
                        items: Vec::new(),
                    });
                    groups.last_mut().unwrap()
                }
            };
            g.items.push(f.clone());
        }
    }
    for g in &mut groups {
        let mut seeds: Vec<u64> = g.items.iter().map(|f| f.seed).collect();
        seeds.sort();
        seeds.dedup();
        g.runs = seeds.len();
        let n = g.items.len();
        let times = if n == 1 { "once".to_string() } else { format!("{n} times") };
        g.text = if g.kind == Kind::Spread {
            let sp: Vec<f32> = runs.iter().filter_map(|r| r.spread).collect();
            let dnf: Vec<&RunFindings> = runs.iter().filter(|r| r.dnf > 0).collect();
            let lo = sp.iter().copied().fold(f32::MAX, f32::min);
            let hi = sp.iter().copied().fold(f32::MIN, f32::max);
            let range =
                if of == 1 || (hi - lo) < 0.05 { format!("{hi:.1} s") } else { format!("{lo:.1} to {hi:.1} s") };
            let dnf = match (of, dnf.len()) {
                (_, 0) => String::new(),
                (1, _) => format!("; {} did not finish", dnf[0].dnf),
                (_, k) => format!("; did not finish in {k} of {of} runs"),
            };
            format!("{}: {range}{dnf}", head(&g.kind, 0))
        } else if of == 1 {
            let cars = g.items.iter().map(|f| f.car).collect::<std::collections::BTreeSet<_>>().len();
            let who = if cars == 1 { "1 car".to_string() } else { format!("{cars} cars") };
            format!("{}: {who}", head(&g.kind, g.checkpoint))
        } else {
            format!("{}: {} of {of} runs, {times}", head(&g.kind, g.checkpoint), g.runs)
        };
        g.items.sort_by(|a, b| (a.seed, a.time.to_bits()).cmp(&(b.seed, b.time.to_bits())));
    }
    groups.sort_by(|a, b| {
        let key = |g: &Group| {
            (g.kind == Kind::Spread, g.kind != Kind::Start, std::cmp::Reverse(g.runs), std::cmp::Reverse(g.items.len()))
        };
        key(a).cmp(&key(b)).then(a.checkpoint.cmp(&b.checkpoint)).then(a.kind.cmp(&b.kind))
    });
    groups
}

/// An occurrence as listed under its group (the seed first with several runs).
pub fn item_text(f: &Finding, runs: usize) -> String {
    let at = clock(f.time);
    if runs > 1 { format!("Seed {}, {} at {at}", f.seed, f.text) } else { format!("{} at {at}", f.text) }
}

#[cfg(test)]
#[path = "summary_tests.rs"]
pub(crate) mod tests;
