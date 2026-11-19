//! The AI racers' route layer: the FuturePoints a racer walks along the
//! track's route records (0x428720 → 0x428030 → 0x422220), the speed
//! profile on them (0x40A8C0, 0x422780, 0x40A6E0), the path handed to the
//! driver (AIActionJustDrive, 0x42B340), the target speed ("Win road race"
//! 0x41C5E0) with its catch-up (0x4172D0 / 0x4084A0), and the opponents'
//! upgrades from their skill (0x5260C0 → 0x5B6AE0).
//!
//! See docs/formats.md, "AI route layer", for what is verified and what is
//! inferred.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};

use super::junction_cache::JunctionCache;
use crate::formats::ai_routes::{RouteFile, lane, lane_offset};
use crate::formats::bundle::{chunks, read_f32, read_u16, read_u32, walk};
use crate::frontend::career::{Track, Trigger};
use crate::opponents::{Bounds, GameRng};

mod human;
mod lane_speeds;
mod path;
mod query;
mod target_speed;
mod walk;

pub use human::{HumanRoad, HumanStep, ROAD_PATH_SPEED, human_append};
pub use lane_speeds::{RaceLap, field_class, speed_mode};
use path::LinePoint;
pub use path::lane_room;
pub use query::Located;
pub use target_speed::{CatchUp, LineSpeed, SpeedTarget, catch_up_multiplier};
#[cfg(test)]
use walk::outrun_leader_junction;
use walk::{FuturePoint, curvature, hermite};
pub use walk::{RacerRoute, start_hold, travel};

/// The look-ahead (0x428720): at least 125 m (0x7E8E60), else 3 s of the
/// current speed (ctl slot 24 0x408240, 0x7E8E0C).
const LOOK_AHEAD: f32 = 125.0;
const LOOK_AHEAD_TIME: f32 = 3.0;
/// The most FuturePoints (0x422220) and those kept behind the racer
/// (0x428030: 20 outside race types 0 and 7; ctl slot 10 always asks).
const MAX_POINTS: usize = 40;
const KEEP_BEHIND: usize = 20;
/// The path the driver gets (0x415D50).
const PATH_POINTS: usize = 20;
/// The path's margin inside a lane's room (0x420D40 with a rate ≠ 999).
const ROOM_MARGIN: f32 = 1.0;
/// 0x420D40's margin in drift races (0x40A00000) for a racer (car +0x1C).
const ROOM_MARGIN_DRIFT: f32 = 5.0;
/// 0x41BFE0's re-plan distances: 1.5 m (0x7E8D94), 4.5 m in drift races.
const REPLAN: f32 = 1.5;
const REPLAN_DRIFT: f32 = 4.5;
/// 0x420D40's kept length: |v| × 0.5 (0x7837E0), at least 30 m (0x7837E8).
const KEEP_RATE: f32 = 0.5;
const KEEP_MIN: f32 = 30.0;
/// Action 0x13's kept length per m/s (0x783764 = 0.75; the minimum 0x783768 = 30 m, `KEEP_MIN`).
const OFFSET_KEEP_RATE: f32 = 0.75;
/// Braking (0x7A4748 / SUV 0x7A4768, m/s²) and grip (0x7A4758 / SUV
/// 0x7A4778, g) by performance class (0x5D4660, 0x5D4610).
const DECEL: [f32; 4] = [12.0, 10.6, 9.3, 8.0];
const DECEL_SUV: [f32; 4] = [10.0, 9.0, 8.0, 6.0];
const GRIP: [f32; 4] = [3.7, 3.4, 3.1, 2.8];
const GRIP_SUV: [f32; 4] = [3.0, 3.0, 3.0, 2.0];
/// A turn's highest speed, m/s (0x5D46B0's cap, 0x42700000).
const TURN_CAP: f32 = 60.0;
/// Off the road by this much the walk starts again (0x60C1B0 × 2).
const OFF_ROAD: f32 = 4.4;

/// 0x428720 without a list (0x428893..0x42895E): the car is on its road
/// node's own segment when, in the record's forward frame, its lateral
/// past the half width on its side (0x401B70: +0x16 right, +0x14 left)
/// is at most twice the car's half extent (0x60C1B0 × 2, [`OFF_ROAD`])
/// and its distance along lies within [−back, length] (0x401090 clamp;
/// back = the previous point's length, the node's own at a record's
/// first point, 0x40CCC0 → 0x5D8A00).
pub fn node_holds(f: &RouteFile, record: usize, cp: usize, position: Vec3) -> bool {
    let points = &f.records[record].points;
    let p = &points[cp];
    let d = position.truncate() - p.position.truncate();
    let along = p.tangent.x * d.x + p.tangent.y * d.y;
    let side = p.tangent.y * d.x - d.y * p.tangent.x;
    let past = if side > 0.0 { side - p.half_width[1] } else { -side - p.half_width[0] };
    if past > OFF_ROAD {
        return false;
    }
    let back = if cp > 0 { points[cp - 1].length } else { p.length };
    along.clamp(-back, p.length) == along
}

/// A track's route file for the race's direction.
#[derive(Clone, Debug)]
pub struct AiRoutes {
    pub file: RouteFile,
    /// The track's number (track +0x8A) and direction (0x89E7A4), for the
    /// drag lane-change table (0x40B110).
    pub track: u32,
    pub backward: bool,
    /// The race road is built (g_RouteCoordRace 0x88F2BE, set by
    /// Race_BuildRoute 0x5E5BC0 in every race): the racers' road node
    /// takes its direction from the record (0x40F750).
    pub race_coords: bool,
    /// The track's kind-4 route triggers (Paths<track>.bin chunk 0x3414A,
    /// loaded with the race by 0x5D9B40): the reset zones 0x6009B0 looks
    /// up first.
    pub resets: Vec<ResetZone>,
    /// The track's kind-3 route triggers not named PCDEMO (same chunk):
    /// the player's update 0x602660 → 0x600720 → 0x5DED20 places the car
    /// at one holding its road locator's point (see [`ResetZone::place`]).
    pub blocks: Vec<ResetZone>,
}

/// A kind-4 route trigger (0x3414A record): a human's reset in it puts
/// the car at its +0x04 / +0x08 facing its +0x0C / +0x10 direction
/// (0x6009B0 → 0x5D9C00(point, 4, 0)).
#[derive(Clone, Copy, Debug)]
pub struct ResetZone {
    pub trigger: Trigger,
    pub direction: Vec2,
}

impl ResetZone {
    /// The kind-4 records of the first 0x3414A chunk in `d` (the record
    /// layout of `career::RouteTriggers`).
    pub fn parse(d: &[u8]) -> Vec<ResetZone> {
        Self::parse_kind(d, 4)
    }

    /// The kind-3 records not named PCDEMO (0x5FBE10 compares +0x30 with
    /// the bin hash of "PCDEMO", 0x43DB50): in the retail data one unnamed
    /// 30 × 20 m zone at (835, −1214) in ROUTESL4RD's Paths4401 / 4402 /
    /// 4404 / FreeRoam; every Paths file's PCDEMO one does nothing.
    pub fn parse_blocks(d: &[u8]) -> Vec<ResetZone> {
        let demo = crate::formats::hash::bin_hash("PCDEMO");
        Self::parse_kind(d, 3).into_iter().filter(|z| z.trigger.name != demo).collect()
    }

    /// 0x5FBE10 for a kind-3 trigger not named PCDEMO (verified, the
    /// disassembly): with its z (+0x14) 0, or car +0x5CC set and the car's
    /// z (+0x68) below it, the car goes to (+0x04, +0x08, z 0) facing
    /// Atan2_16(+0x0C, +0x10) at 8 m/s (0x8021CC; Car_PlaceAt 0x5FBD70
    /// with flag 1). Returns the place and heading; None when the z test
    /// fails (car +0x5CC is not traced, so a trigger with a z is skipped,
    /// **inferred**; the retail one has z 0).
    pub fn place(&self) -> Option<(Vec2, f32)> {
        (self.trigger.position.z == 0.0)
            .then(|| (self.trigger.position.truncate(), self.direction.y.atan2(self.direction.x)))
    }

    fn parse_kind(d: &[u8], want: u32) -> Vec<ResetZone> {
        let key = 0x3414Au32.to_le_bytes();
        let Some(at) = d.windows(4).position(|w| w == key) else { return Vec::new() };
        if at + 8 > d.len() {
            return Vec::new();
        }
        let end = (at + 8 + read_u32(d, at + 4) as usize).min(d.len());
        let mut out = Vec::new();
        let mut p = at + 8;
        while p + 0x44 <= end {
            let (kind, n, size) = (read_u32(d, p), read_u16(d, p + 0x40) as usize, read_u16(d, p + 0x42) as usize);
            if size < 0x44 || p + size > end {
                break;
            }
            if kind == want {
                let mut polygon = [Vec2::ZERO; 8];
                let fits = n <= 8 && 0x44 + n * 8 <= size;
                if fits {
                    for (k, v) in polygon.iter_mut().enumerate().take(n) {
                        *v = Vec2::new(read_f32(d, p + 0x44 + k * 8), read_f32(d, p + 0x48 + k * 8));
                    }
                }
                let trigger = Trigger {
                    name: read_u32(d, p + 0x30),
                    position: Vec3::new(read_f32(d, p + 4), read_f32(d, p + 8), read_f32(d, p + 0x14)),
                    gps: Vec3::ZERO,
                    min: Vec2::new(read_f32(d, p + 0x20), read_f32(d, p + 0x24)),
                    max: Vec2::new(read_f32(d, p + 0x28), read_f32(d, p + 0x2C)),
                    polygon,
                    points: if fits { n as u8 } else { 0 },
                };
                out.push(ResetZone { trigger, direction: Vec2::new(read_f32(d, p + 0x0C), read_f32(d, p + 0x10)) });
            }
            p += size;
        }
        out
    }
}

impl AiRoutes {
    /// `TRACKS/ROUTES<region>/Routes<track><F|B>.bin` (0x57F554).
    pub fn load(root: &Path, track: &Track, backward: bool) -> Result<AiRoutes> {
        let path = crate::game_fs::path(root, &format!(
            "TRACKS/ROUTES{}/Routes{}{}.bin",
            track.region,
            track.number,
            if backward { "B" } else { "F" }
        ));
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let paths = crate::game_fs::read(root, &format!("TRACKS/ROUTES{}/Paths{}.bin", track.region, track.number))
            .unwrap_or_default();
        Ok(AiRoutes {
            file: RouteFile::parse(&data),
            track: u32::from(track.number),
            backward,
            race_coords: false,
            resets: ResetZone::parse(&paths),
            blocks: ResetZone::parse_blocks(&paths),
        })
    }
}

/// The racers' top speed and acceleration by performance class (GLOBALB
/// chunk 0x34A33, kept at 0x86B338: rows of 0x10C bytes, +0x00 the top
/// speed, m/s, +0x0C 64 accelerations, m/s², over 0..top speed). 0x5A78F0
/// makes each row a 64-sample curve for the route manager's table
/// (+0x4670, 0x88E990).
#[derive(Clone, Debug)]
pub struct ClassCurves {
    pub top: [f32; 4],
    pub accel: [[f32; 64]; 4],
}

impl ClassCurves {
    pub fn parse(globalb: &[u8]) -> Option<ClassCurves> {
        let mut out = None;
        walk(chunks(globalb, 0), &mut |c| {
            if c.id == 0x34A33 && c.body.len() >= 4 * 0x10C {
                let mut curves = ClassCurves { top: [0.0; 4], accel: [[0.0; 64]; 4] };
                for k in 0..4 {
                    let r = &c.body[k * 0x10C..];
                    curves.top[k] = read_f32(r, 0);
                    for i in 0..64 {
                        curves.accel[k][i] = read_f32(r, 0x0C + 4 * i);
                    }
                }
                out = Some(curves);
            }
        });
        out
    }

    /// 0x5764E0: the class's acceleration at `v`, linear between the 64
    /// samples (63 / top speed apart), the first below 0, the last above.
    pub fn accel(&self, class: usize, v: f32) -> f32 {
        let (c, top) = (&self.accel[class.min(3)], self.top[class.min(3)]);
        let x = v * 63.0 / top;
        if x < 0.0 {
            return c[0];
        }
        let i = x as usize;
        if i >= 63 {
            return c[63];
        }
        let t = x - i as f32;
        c[i] * (1.0 - t) + c[i + 1] * t
    }
}

/// The AI tuning (GLOBALB chunk 0x34B00, 0xAE0 bytes, kept at 0x81CB60).
#[derive(Clone, Debug)]
pub struct AiTuning {
    data: Vec<u8>,
}

impl AiTuning {
    pub fn parse(globalb: &[u8]) -> Option<AiTuning> {
        let mut out = None;
        walk(chunks(globalb, 0), &mut |c| {
            if c.id == 0x34B00 && c.body.len() >= 0xAE0 {
                out = Some(AiTuning { data: c.body[..0xAE0].to_vec() });
            }
        });
        out
    }

    fn f(&self, at: usize) -> f32 {
        read_f32(&self.data, at)
    }

    /// A skill row's float (rows of 0x68 bytes; `at` is skill 0's offset).
    fn row(&self, skill: usize, at: usize) -> f32 {
        self.f(at + skill.min(9) * 0x68)
    }

    /// The ranges of a drag AI racer's random shift offsets (0x5B6214..
    /// 0x5B626A): [up +0x558..+0x55C, down +0x550..+0x554, 2nd gear
    /// +0x560..+0x564] (rows 0–9: up / 2nd −900..300, −800..260, −650..200,
    /// −500..140, −390..100, −290..85, −205..45, −200..30, −150..20,
    /// −110..15; down 0..0.01). `None` for a row outside 0..=9: the set-up
    /// then leaves the drivetrain's own.
    pub fn shift_ranges(&self, row: usize) -> Option<[[f32; 2]; 3]> {
        if row > 9 {
            return None;
        }
        let at = |o: usize| self.f(o + row * 0x68);
        Some([[at(0x558), at(0x55C)], [at(0x550), at(0x554)], [at(0x560), at(0x564)]])
    }

    /// The route speed factor (+0x538).
    pub fn speed_factor(&self, skill: usize) -> f32 {
        self.row(skill, 0x538)
    }

    /// The fewest seconds of nitrous an AI racer starts a shot with
    /// (+0x570; 0.5, 0.5, 1, 1, 1, 1.5, 1.5, 2, 2, 0.5 by row).
    pub fn nitrous_seconds(&self, skill: usize) -> f32 {
        self.row(skill, 0x570)
    }

    /// The lowest gear (0x40D850: car +0x434 index, 2 = first) an AI
    /// racer fires nitrous in: the byte +0x588 + 1 in a drag race (0x89E7D8
    /// or 0x890108; rows 5, 5, 5, 5, 5, 4, 3, 3, 3, 3), else 0x7E8D90 = 2.
    pub fn nitrous_gear(&self, skill: usize, drag: bool) -> usize {
        if drag { self.data[0x588 + skill.min(9) * 0x68] as usize + 1 } else { 2 }
    }

    /// 0x416F70's skill-row values outside drag: the least time between
    /// shots (+0x568, 10 … 5 s), the same on the last lap (+0x56C,
    /// 5 … 2 s), the longest shot (+0x574, 1.5 … 4 s) and the byte +0x589
    /// (rows 5–9: no shot with under 1 s in the tank before the end).
    pub fn nitrous_gap(&self, skill: usize, last_lap: bool) -> f32 {
        self.row(skill, if last_lap { 0x56C } else { 0x568 })
    }
    pub fn nitrous_longest(&self, skill: usize) -> f32 {
        self.row(skill, 0x574)
    }
    pub fn nitrous_keeps_reserve(&self, skill: usize) -> bool {
        self.data[0x589 + skill.min(9) * 0x68] != 0
    }

    /// The odds the controller's vt+0x9C (0x408280) takes a pass or an
    /// avoid with (+0x530).
    pub fn pass_odds(&self, skill: usize) -> f32 {
        self.row(skill, 0x530)
    }

    /// The odds a racer takes a retyped junction with (+0x524, +0x528,
    /// +0x52C; 0x413CC0, see `RacerRoute::junction_odds`).
    pub fn junction_odds(&self, skill: usize) -> [f32; 3] {
        [self.row(skill, 0x524), self.row(skill, 0x528), self.row(skill, 0x52C)]
    }

    /// The random speed-sample offset's half range, m (+0x584).
    /// The Block action's priority (0x414130's return, the chooser's gate
    /// 0x42D355): the i32 at +0x934 + 8·row, row = racer +0x902 (three
    /// rows: 200, 200, 400).
    pub fn block_priority(&self, row: usize) -> i32 {
        let at = 0x934 + row.min(2) * 8;
        i32::from_le_bytes(self.data[at..at + 4].try_into().unwrap())
    }

    /// Whether Block measures the follower's gap bumper to bumper (the
    /// byte at +0x938 + 8·row; 1 in every row).
    pub fn block_bumper(&self, row: usize) -> bool {
        self.data[0x938 + row.min(2) * 8] != 0
    }

    pub fn jitter(&self, skill: usize) -> f32 {
        self.row(skill, 0x584)
    }

    /// Seconds into the race before the catch-up applies (+0x580).
    pub fn catch_up_hold(&self, skill: usize) -> f32 {
        self.row(skill, 0x580)
    }

    /// 0x4FED00: a skill row drawn for a difficulty: the first of the ten
    /// cumulative floats at +0x9F4 + 0x28 × difficulty that the random
    /// fraction `r` (0x43C1F0(1.0)) is at most, else 0. Career free roam's
    /// encounter racers take theirs so (0x53F010).
    pub fn random_row(&self, difficulty: usize, r: f32) -> usize {
        (0..10).find(|&i| r <= self.f(0x9F4 + difficulty.min(2) * 0x28 + i * 4)).unwrap_or(0)
    }

    /// How often the offset is redrawn (+0xAD4, 5 s) and the catch-up
    /// refreshed (+0xAD8, 0.3 s).
    pub fn jitter_period(&self) -> f32 {
        self.f(0xAD4)
    }
    pub fn catch_up_period(&self) -> f32 {
        self.f(0xAD8)
    }

    /// The Quick Race catch-up scale of a difficulty and the human car's
    /// performance class (+0x94C + 4·(4·difficulty + class); 0x53F306).
    pub fn catch_up_scale(&self, difficulty: usize, class: usize) -> f32 {
        self.f(0x94C + 4 * (4 * difficulty.min(2) + class.min(3)))
    }

    /// The traffic spacing range of a traffic zone's row (+0xA8C + 8·row:
    /// min, max metres; 0x418240): rows 0–8 read 240–300, 110–145, 80–95,
    /// 90–110, 70–100, 50–65, 50–70, 35–45, 20–25.
    pub fn traffic_spacing(&self, row: usize) -> Option<(f32, f32)> {
        (row < 9).then(|| (self.f(0xA8C + row * 8), self.f(0xA90 + row * 8)))
    }

    /// An 11-point curve at `at` (0x5767A0).
    fn curve(&self, at: usize, points: usize, x: f32) -> f32 {
        let p: Vec<(f32, f32)> = (0..points).map(|i| (self.f(at + i * 8), self.f(at + i * 8 + 4))).collect();
        pairs(&p, x)
    }
}

/// 0x5767A0: linear through (x, y) pairs, clamped at the ends.
pub(crate) fn pairs(p: &[(f32, f32)], x: f32) -> f32 {
    let Some(first) = p.first() else { return 0.0 };
    if x <= first.0 {
        return first.1;
    }
    let last = p[p.len() - 1];
    if x >= last.0 {
        return last.1;
    }
    for w in p.windows(2) {
        if w[0].0 <= x && x < w[1].0 {
            let dx = w[1].0 - w[0].0;
            if dx.abs() <= 1e-6 {
                return (w[1].1 - w[0].1) * 0.5 + w[0].1;
            }
            return (x - w[0].0) / dx * (w[1].1 - w[0].1) + w[0].1;
        }
    }
    first.1
}

/// One upgrade preset (GLOBALB 0x34A31, 16 bytes): a rating and the
/// performance parts it fits.
#[derive(Clone, Debug)]
pub struct Preset {
    pub rating: f32,
    pub parts: BTreeSet<u8>,
}

/// The presets of every car type, in car-type order (0x34A31 chunks in the
/// 0x80034A30 container; 0x86B0C8 counts, 0x86B0CC pointers).
#[derive(Clone, Debug, Default)]
pub struct Presets(pub Vec<Vec<Preset>>);

impl Presets {
    pub fn parse(globalb: &[u8]) -> Presets {
        let mut out = Vec::new();
        walk(chunks(globalb, 0), &mut |c| {
            if c.id == 0x34A31 {
                let n = crate::formats::bundle::read_u32(c.body, 0) as usize;
                out.push(
                    (0..n)
                        .filter_map(|i| c.body.get(4 + i * 16..4 + i * 16 + 16))
                        .map(|r| Preset {
                            rating: read_f32(r, 0),
                            parts: (0..0x44u8).filter(|&b| r[5 + b as usize / 8] >> (7 - b % 8) & 1 != 0).collect(),
                        })
                        .collect(),
                );
            }
        });
        Presets(out)
    }

    /// The preset nearest a rating (0x5B6AE0's search).
    pub fn nearest(&self, car: usize, rating: f32) -> Option<&Preset> {
        let list = self.0.get(car)?;
        if list.len() <= 1 {
            return list.first();
        }
        let mut i = 0;
        while i + 1 < list.len() - 1 && rating >= list[i + 1].rating {
            i += 1;
        }
        let (a, b) = (&list[i], &list[i + 1]);
        Some(if (a.rating - rating).abs() < (b.rating - rating).abs() { a } else { b })
    }
}

/// The handling score of a car-type record (0x5B54A0): nine normalised
/// tuning measures weighted by 0x7A1A9C, clamped to 0.03..1.
pub fn handling_score(record: &[u8], bounds: &Bounds) -> f32 {
    const W: [f32; 9] = [0.14, 0.10, 0.15, 0.15, 0.10, 0.15, 0.15, 0.08, 0.08];
    let t = |o: usize| read_f32(record, 0x110 + o);
    let m = t(0x110);
    let n = |v: f32, i: usize| {
        let (lo, hi) = bounds.score[i];
        (v - lo) / (hi - lo)
    };
    let f4 = if t(0x1C0) <= 0.6 { (t(0x1C0) + 0.2) * 1.25 } else { (1.0 - t(0x1C0)) * 2.5 };
    let f = [
        n((t(0xE0) + t(0xDC) + t(0x100) + t(0xFC)) * 0.5 / m * 0.5, 0),
        n((t(0x274) * 0.65 + t(0x278) * 0.35) / m, 1),
        n((t(0x174) * t(0x170) + t(0x180) + t(0x194) * t(0x190) + t(0x1A0)) * 0.5 / m * 0.5, 2),
        1.0 - n(m, 3),
        f4,
        1.0 - n(t(0x08), 5),
        1.0 - n(t(0x160), 6),
        n(t(0x264), 7),
        n(t(0x268), 8),
    ];
    W.iter().zip(f).map(|(w, f)| w * f).sum::<f32>().clamp(0.03, 1.0)
}

/// An opponent's fitted performance parts from its skill (0x5260C0 for
/// skill sources 0 and 3, offline → 0x5B6AE0): a rating between the
/// player's car's first and last presets by skill / 10, the opponent car's
/// nearest preset fitted, then once more with the rating moved by 150 ×
/// (the skill's target score − the fitted car's handling score), the
/// target running from `bounds.spread.0` to `.1` by skill / 10.
/// In a drag race (0x89E7D8 or 0x890108 set) the target score stays −1
/// (0x526309) and 0x5B6AE0 skips the second fit: the nearest preset to
/// the rating stands (observed under Wine, the finished-career save's Skyline range
/// 273.8–591.0 at skill 4 → 400.5, the stage-4 drag rivals' ratings
/// 397.5 / 398.4 / 402.4 are the COROLLA's, PEUGOT's and RX7's nearest
/// presets).
/// `record` is the opponent's stock car-type record (0x890 bytes).
pub fn fit_performance(
    presets: &Presets,
    bounds: &Bounds,
    car: usize,
    player_car: usize,
    skill: f32,
    record: &[u8],
    packages: &[crate::performance::Package],
    drag: bool,
) -> BTreeSet<u8> {
    let s = skill.clamp(0.0, 10.0) / 10.0;
    let Some(range) = presets.0.get(player_car).filter(|l| !l.is_empty()) else { return BTreeSet::new() };
    let (lo, hi) = (range[0].rating, range[range.len() - 1].rating);
    let mut rating = (hi - lo) * s + lo;
    let target = (bounds.spread.1 - bounds.spread.0) * s + bounds.spread.0;
    let Some(first) = presets.nearest(car, rating) else { return BTreeSet::new() };
    let mut fitted = first.parts.clone();
    if drag {
        return fitted;
    }
    let mut upgraded = record.to_vec();
    crate::performance::upgrade(&mut upgraded, packages, &|id| fitted.contains(&id));
    rating += (target - handling_score(&upgraded, bounds)) * 150.0;
    if let Some(p) = presets.nearest(car, rating) {
        fitted = p.parts.clone();
    }
    fitted
}

/// The best human racer's performance as an AI racer is matched to it
/// (0x5B7580's argument: its performance object, racer info + 0x20).
#[derive(Clone, Copy, Debug)]
pub struct HumanPerf {
    /// Its rating (+8, the raw area 0x5B4DD0 keeps).
    pub rating: f32,
    /// Its handling score (0x5B54A0).
    pub score: f32,
    /// Its nitrous level: the upgrade state's float 2 (+0x2E8) + 0.5,
    /// truncated.
    pub nitrous: u8,
}

/// An AI racer's performance parts matched to the best human's (0x532020
/// → 0x5B7580(human, 1, offline), for Quick Race 0x53EEA0 and the career
/// free roam's encounter racers 0x53F010): 0x5B6AE0(the human's rating,
/// unscaled, the human's handling score) — the AI car's nearest preset,
/// then once more with the rating moved by 150 × (the human's score − the
/// fitted car's) — then nitrous fitted up to the human's level
/// (0x5A64B0(level, 2): nitrous off, levels 1 to it on), then the
/// category-9 refit of 0x6399E0(1, 1) ([`crate::performance::ai_refit`]).
/// `record` is the AI's stock car-type record.
pub fn match_to_human(
    presets: &Presets,
    bounds: &Bounds,
    car: usize,
    human: &HumanPerf,
    record: &[u8],
    packages: &[crate::performance::Package],
) -> BTreeSet<u8> {
    let mut fitted = presets.nearest(car, human.rating).map(|p| p.parts.clone()).unwrap_or_default();
    if presets.nearest(car, human.rating).is_some() {
        let mut upgraded = record.to_vec();
        crate::performance::upgrade(&mut upgraded, packages, &|id| fitted.contains(&id));
        let rating = (human.score - handling_score(&upgraded, bounds)) * 150.0 + human.rating;
        if let Some(p) = presets.nearest(car, rating) {
            fitted = p.parts.clone();
        }
    }
    crate::performance::fit_category(&mut fitted, packages, 2, human.nitrous);
    crate::performance::ai_refit(&mut fitted, packages);
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn chase_import_walk_and_goal_exit_keep_native_decisions() {
        let routes = AiRoutes {
            file: RouteFile::parse(&crate::testkit::private("tests/fixtures/outrun-native/routes.bin")),
            track: 4000,
            backward: false,
            race_coords: false,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        let mut checked = 0;
        for line in crate::testkit::private_str("tests/fixtures/outrun-native/search.csv")
            .lines()
            .skip(1)
        {
            let values: Vec<_> = line.split(',').collect();
            if values[10] != "1" {
                continue;
            }
            let number = |i: usize| values[i].parse::<f32>().unwrap();
            let position = Vec3::new(number(2), number(3), number(4));
            let direction = Vec2::new(number(5), number(6));
            let target = Vec3::new(number(7), number(8), number(9));
            let record_id = values[11].parse::<u16>().unwrap();
            let record = routes.file.records.iter().position(|r| r.id == record_id).unwrap();
            let point = values[12].parse::<usize>().unwrap();
            let forward = values[13] == "1";
            let decisions: Vec<_> = values[15]
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let word = s.parse::<usize>().unwrap();
                    (word & 8191, word & 8192 != 0)
                })
                .collect();
            let mut route = RacerRoute::from_node(&routes, position, record, point, !forward);
            route.set_outrun_chaser(true);
            let rng = route.rng.0;
            assert!(route.search_outrun_chaser(&routes, position, direction, target));
            assert_eq!(route.node(), Some((record, point, !forward)), "native input {line}");
            route.update(&routes, position, 20.0, Vec2::X, 0, false, &|_| 1.0);
            assert!(!route.search_rebuild_pending);
            assert_eq!(route.rng.0, rng, "chase walk must not draw ordinary junction odds");
            for &(junction, accepted) in &decisions {
                assert_eq!(route.decisions.get(junction), Some(accepted), "native import survived rebuild {line}");
            }
            let before_failure = format!("{route:?}");
            assert!(!route.search_outrun_chaser(&routes, Vec3::splat(1e8), direction, target));
            assert_eq!(format!("{route:?}"), before_failure, "double failure changes nothing");
            route.set_outrun_chaser(false);
            for &(junction, _) in &decisions {
                assert!(route.decisions.get(junction).is_none());
            }
            checked += 1;
        }
        assert!(checked > 30);
    }

    /// 0x428720 / 0x42CDC0: a failed road-node search leaves
    /// the node null and the list empty (0x42CE9C, 0x411BE0); the next
    /// frame's 0x428720 sees the empty list and the null node and searches
    /// again (vt+0x30(1)), every frame until a node is found, then 0x428030
    /// rebuilds the list from it.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn a_failed_node_search_searches_again_every_frame() {
        let routes = AiRoutes {
            file: RouteFile::parse(&crate::testkit::private("tests/fixtures/outrun-native/routes.bin")),
            track: 4000,
            backward: false,
            race_coords: false,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        let mut found = 0;
        for line in crate::testkit::private_str("tests/fixtures/outrun-native/search.csv")
            .lines()
            .skip(1)
            .take(40)
        {
            let values: Vec<_> = line.split(',').collect();
            let number = |i: usize| values[i].parse::<f32>().unwrap();
            let position = Vec3::new(number(2), number(3), number(4));
            let way = Vec2::new(number(5), number(6));
            let Some(expected) = RacerRoute::find(&routes, position, way, None, true) else { continue };
            let mut route = RacerRoute::from_node(&routes, position, expected.0, expected.1, expected.2);
            // A frame far off every road: the search fails, the list stays empty.
            route.points.clear();
            route.update(&routes, Vec3::splat(1e7), 0.0, way, 0, false, &|_| 1.0);
            assert!(route.node().is_none());
            route.update(&routes, Vec3::splat(1e7), 0.0, way, 0, false, &|_| 1.0);
            assert!(route.node().is_none());
            // Back on the road: the next frame finds the node again.
            route.update(&routes, position, 0.0, way, 0, false, &|_| 1.0);
            let p = &route.points[0];
            assert_eq!((p.record, p.cp, p.backward), expected, "{line}");
            assert!(route.points.len() > 1, "the list rebuilt from the node");
            found += 1;
        }
        assert!(found > 5, "{found}");
    }

    /// 0x413CC0 / 0x5EF000: the hold is the same function for
    /// every controller and holds only racer numbers 100…105, so the human
    /// (number 0, observed) has none; a human controller (kind 1) also takes
    /// junction types 2 and 8 before any other test (0x5DCBF0(2)).
    #[test]
    fn the_human_walk_has_no_start_hold_and_takes_types_2_and_8() {
        assert_eq!(start_hold(0), f32::NEG_INFINITY);
        assert_eq!(start_hold(100), 12.0);
        assert_eq!(start_hold(102), 11.0);
        assert_eq!(start_hold(106), f32::NEG_INFINITY);
        let (_, mut route) = outrun_speed_fixture();
        let mut file = RouteFile::default();
        file.junctions =
            vec![crate::formats::ai_routes::Junction { target_point: [0, 0], target_record: [-1, -1], flags: 8 }; 2];
        route.clock = 0.0;
        route.branch_hold = 12.0;
        assert!(!route.accept(&file, 0), "a racer in its hold: no type 8");
        route.human = true;
        route.decisions = JunctionCache::default();
        assert!(route.accept(&file, 1), "the human controller takes type 8 first");
    }

    #[test]
    fn outrun_leader_cache_overwrites_zero_on_seventeenth_miss() {
        let (_, mut route) = outrun_speed_fixture();
        let mut file = RouteFile::default();
        file.junctions =
            vec![crate::formats::ai_routes::Junction { target_point: [0, 0], target_record: [-1, -1], flags: 0 }; 18];
        route.outrun_leader_positions = Some((Vec2::ZERO, Vec2::new(149.0, 0.0)));
        for j in 0..17 {
            route.accept(&file, j);
        }
        let after = route.rng.0;
        route.accept(&file, 1);
        assert_eq!(route.rng.0, after, "overflow retains slot1");
        route.accept(&file, 0);
        assert_ne!(route.rng.0, after, "slot0 was replaced and must consume a new draw");
        let after = route.rng.0;
        route.accept(&file, 16);
        assert_ne!(route.rng.0, after, "each full miss overwrites slot0 again");
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn outrun_leader_junction_matches_original_instructions() {
        // Original 0x413F50 + actual RNG/distance/record helpers, executed
        // in a private Wine PE32. No expected outcomes computed by this test.
        let cases = &crate::testkit::private_str("docs/evidence/outrun/leader-native.csv");
        for (index, line) in cases.lines().skip(1).enumerate() {
            let v: Vec<&str> = line.split(',').collect();
            let flags = v[0].parse::<u32>().unwrap();
            let blocked = v[1] == "1";
            let second = Vec2::new(v[2].parse().unwrap(), v[3].parse().unwrap());
            let mut rng = GameRng(v[4].parse().unwrap());
            let accepted = outrun_leader_junction(flags, blocked, Vec2::ZERO, second, &mut rng);
            assert_eq!(accepted, v[5] == "1", "native case {index}: {line}");
            assert_eq!(rng.0, v[6].parse::<u32>().unwrap(), "RNG case {index}: {line}");
        }
    }

    #[test]
    fn outrun_leader_route_uses_goal_filter_and_cache_eligibility() {
        let (_, mut route) = outrun_speed_fixture();
        let mut file = RouteFile::default();
        file.junctions.push(crate::formats::ai_routes::Junction {
            target_point: [0, 0],
            target_record: [-1, -1],
            flags: 0,
        });
        route.outrun_leader_positions = Some((Vec2::ZERO, Vec2::new(149.0, 0.0)));
        route.rng = GameRng(0x4000_0000); // .5 < .75, despite ordinary row odds 0.
        assert!(route.accept(&file, 0), "goal 6 has no ordinary start hold");
        let after = route.rng.0;
        route.outrun_leader_positions = Some((Vec2::ZERO, Vec2::new(150.0, 0.0)));
        assert!(route.accept(&file, 0), "the cached ordinary decision is retained");
        assert_eq!(route.rng.0, after, "cache reuse consumes no RNG");
        file.junctions[0].flags = 9;
        route.rng = GameRng(0x4000_0000);
        assert!(!route.accept(&file, 0), "type 9 ignores the existing cache entry");
        assert_ne!(route.rng.0, 0x4000_0000);
        route.outrun_leader_positions = Some((Vec2::ZERO, Vec2::new(149.0, 0.0)));
        route.rng = GameRng(0x4000_0000);
        assert!(route.accept(&file, 0), "type 9 is reevaluated at the new distance");
        route.outrun_leader_positions = None;
        route.decisions.clear();
        assert!(!route.accept(&file, 0), "ordinary routing retains its own hold and odds");
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn outrun_leader_cache_expires_before_update_and_redraws_on_revisit() {
        let mut file = RouteFile::parse(&crate::testkit::private("tests/fixtures/outrun-native/routes.bin"));
        // Deliberately distinct destination and source; native 5D3F90 reads
        // the second pair (+06/+0A), not Junction::target's first pair.
        file.junctions[0] = crate::formats::ai_routes::Junction {
            target_point: [0, 0],
            target_record: [file.records[1].id as i16, file.records[0].id as i16],
            flags: 0,
        };
        let position = file.records[0].points[0].position;
        let routes =
            AiRoutes { file, track: 4000, backward: false, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
        let (_, mut route) = outrun_speed_fixture();
        route.points = (0..MAX_POINTS)
            .map(|i| FuturePoint {
                record: 0,
                cp: 0,
                backward: i % 2 == 0,
                s: i as f32 * 100.0,
                offset: 0.0,
                adjust: 0.0,
                lane_adjust: [0.0; 4],
                speed: [30.0; 5],
                inserted: None,
            })
            .collect();
        route.outrun_leader_positions = Some((Vec2::ZERO, Vec2::new(149.0, 0.0)));
        route.rng = GameRng(0x4000_0000);
        assert!(route.accept(&routes.file, 0));
        let after_first = route.rng.0;
        route.update(&routes, position, 0.0, Vec2::X, 0, false, &|_| 1.0);
        assert_eq!(route.decisions.get(0), Some(true), "a source still on the list retains its decision");
        assert_eq!(route.rng.0, after_first);
        route.points.clear();
        route.update(&routes, position, 0.0, Vec2::X, 0, false, &|_| 1.0);
        assert!(
            route.decisions.is_empty(),
            "even an empty route expires ordinary goal-6 entries before update returns"
        );
        assert_eq!(route.rng.0, after_first, "pruning itself consumes no RNG");
        let mut expected_rng = route.rng.clone();
        let expected = outrun_leader_junction(0, false, Vec2::ZERO, Vec2::new(149.0, 0.0), &mut expected_rng);
        assert_eq!(route.accept(&routes.file, 0), expected);
        assert_eq!(route.rng.0, expected_rng.0, "a revisited expired fork takes a fresh draw");
        assert_ne!(route.rng.0, after_first);
    }

    /// 0x415D50: every path point, a junction gap's inserted one (+0x74)
    /// too, is 0x4122C0 at its own offset with the lateral, so a car off
    /// its racing line gets a path parallel to the line. The clone took an
    /// inserted point's smoothed place without the lateral, and the path
    /// zigzagged every 8 m (docs/notes/outrun/ai-strength.md: the encounter
    /// rival steered into walls at junctions).
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn inserted_points_keep_the_lateral_on_the_path() {
        let file = RouteFile::parse(&crate::testkit::private("tests/fixtures/outrun-native/routes.bin"));
        let routes =
            AiRoutes { file, track: 4000, backward: false, race_coords: false, resets: Vec::new(), blocks: Vec::new() };
        let (_, mut route) = outrun_speed_fixture();
        let n = routes.file.records[0].points.len();
        assert!(n >= 3, "fixture record 0 has {n} points");
        let len0 = routes.file.records[0].points[0].length;
        let plain = |cp: usize, s: f32| FuturePoint {
            record: 0,
            cp,
            backward: false,
            s,
            offset: 0.0,
            adjust: 0.0,
            lane_adjust: [0.0; 4],
            speed: [30.0; 5],
            inserted: None,
        };
        let mut gap = plain(0, len0.min(8.0));
        gap.offset = len0.min(8.0);
        // The smoothed place the clone kept: on the racing line (lateral 0).
        gap.inserted = Some(route.line_point(&routes.file, &gap, gap.offset, 0.0));
        route.points =
            vec![plain(0, 0.0), gap, plain(1, len0), plain(2, len0 + routes.file.records[0].points[1].length)];
        route.current = 0;
        route.distance = 0.0;
        route.lateral = 1.5;
        let start = route.line_point(&routes.file, &route.points[0], 0.0, 1.5);
        let path = route.line_path(
            &routes,
            0.0,
            start,
            routes.file.direction(0, 0, false).extend(0.0),
            20.0,
            false,
            true,
            1.5,
        );
        assert_eq!(path.len(), 3);
        for (k, (q, l)) in path.iter().zip(route.line.clone()).enumerate() {
            let p = &route.points[k + 1];
            let want = route.line_point(&routes.file, p, p.offset, l.lateral);
            assert!(q.distance(want) < 1e-4, "point {k}: {q:?} vs {want:?}");
            assert!((l.lateral - 1.5).abs() < 1e-4);
        }
        let on_line = route.points[1].inserted.unwrap();
        assert!(path[0].distance(on_line) > 1.0, "the inserted point is offset by the lateral, not its line place");
    }

    fn outrun_speed_fixture() -> (AiTuning, RacerRoute) {
        let mut tuning = AiTuning { data: vec![0; 0xAE0] };
        for skill in 0..10 {
            for (offset, value) in [(0x538, 0.4 + skill as f32 * 0.1), (0x584, 20.0)] {
                tuning.data[offset + skill * 0x68..offset + skill * 0x68 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        tuning.data[0xAD4..0xAD8].copy_from_slice(&1.0f32.to_le_bytes());
        let route = RacerRoute {
            points: [20.0, 40.0]
                .into_iter()
                .enumerate()
                .map(|(cp, speed)| FuturePoint {
                    record: 0,
                    cp,
                    backward: false,
                    s: cp as f32 * 100.0,
                    offset: 0.0,
                    adjust: 0.0,
                    lane_adjust: [0.0; 4],
                    speed: [speed; 5],
                    inserted: None,
                })
                .collect(),
            turns: Vec::new(),
            current: 0,
            distance: 50.0,
            lateral: 0.0,
            off_road: 0.0,
            backward: false,
            decisions: JunctionCache::default(),
            last_turn: f32::MIN,
            dead_end: false,
            clock: 0.0,
            branch_hold: 12.0,
            drag: false,
            human: false,
            rng: GameRng::default(),
            race_coord: None,
            junction_odds: [0.0, 0.0, 1.0],
            outrun_leader_positions: None,
            outrun_chaser: false,
            search_rebuild_pending: false,
            line: Vec::new(),
            replanned: false,
        };
        (tuning, route)
    }

    /// 0x41C5E0's race-state-7 branch: route distance is unjittered,
    /// table 0x78357C gives .84 for place 1 and 0x783598 gives 1 otherwise.
    #[test]
    fn outrun_speed_uses_place_without_skill_or_distance_jitter() {
        let (tuning, route) = outrun_speed_fixture();
        for skill in [0, 5, 9] {
            let mut speed = SpeedTarget::new(skill);
            let mut rng = GameRng::default();
            let leader = speed.update_outrun(&tuning, &route, &mut rng, 0.0, true, 0.016);
            assert!((leader - 25.2).abs() < 0.00001, "skill {skill}: {leader}");
            let chaser = speed.update_outrun(&tuning, &route, &mut rng, 0.25, false, 0.016);
            assert_eq!(chaser, 30.0);
            let leader = speed.update_outrun(&tuning, &route, &mut rng, 2.0, true, 0.016);
            assert!((leader - 25.2).abs() < 0.00001);
        }
    }

    /// 0x41C5E0 (0x41C6E0..0x41C731): the distance offset is redrawn from
    /// the world's generator (0x5EA510 with ECX = [0x890080]) once the
    /// elapsed (sim tick − +0x700) × 0.00025 is strictly above the period
    /// [0x81CB60] +0xAD4; the racers draw in turn from that one generator
    /// (observed: the pinned circuit's world generator steps three times on
    /// the first racing frame, once per rival).
    #[test]
    fn speed_offset_draws_from_the_shared_world_generator() {
        let (tuning, route) = outrun_speed_fixture();
        let mut world = GameRng(923_776_375);
        let mut expected = world.clone();
        let (mut a, mut b) = (SpeedTarget::new(3), SpeedTarget::new(3));
        a.update(&tuning, &route, &mut world, 0.0, None, 0.016);
        b.update(&tuning, &route, &mut world, 0.0, None, 0.016);
        assert_eq!(a.jitter, expected.unit(40.0) - 20.0, "first racer: the first draw");
        assert_eq!(b.jitter, expected.unit(40.0) - 20.0, "second racer: the next draw of the same generator");
        // Exactly one period later: no redraw (strict compare, whole ticks).
        let before = world.0;
        a.update(&tuning, &route, &mut world, 1.0, None, 0.016);
        assert_eq!(world.0, before);
        a.update(&tuning, &route, &mut world, 1.0003, None, 0.016);
        assert_ne!(world.0, before);
    }

    #[test]
    fn outrun_speed_preserves_rng_and_zero_route_fallback() {
        let (tuning, mut route) = outrun_speed_fixture();
        for p in &mut route.points {
            p.speed = [0.0; 5];
        }
        let mut ordinary = SpeedTarget::new(5);
        let mut outrun = ordinary.clone();
        let mut race_rng = GameRng::default();
        let mut outrun_rng = race_rng.clone();
        let mut expected_rng = race_rng.clone();
        for (clock, leader) in [(0.0, true), (0.25, false), (2.0, true)] {
            ordinary.update(&tuning, &route, &mut race_rng, clock, None, 0.016);
            let target = outrun.update_outrun(&tuning, &route, &mut outrun_rng, clock, leader, 0.016);
            if clock == 0.0 || clock == 2.0 {
                expected_rng.unit(40.0);
            }
            assert_eq!(outrun_rng.0, expected_rng.0, "one draw at 0 and 2 seconds, none at .25");
            assert_eq!(outrun_rng.0, race_rng.0, "the unused jitter draw still advances the racer RNG");
            assert_eq!(outrun.jitter, ordinary.jitter);
            assert_eq!(outrun.jitter_at, ordinary.jitter_at);
            assert!((target - if leader { 25.2 } else { 30.0 }).abs() < 0.00001);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    /// The kind-3 route triggers: L4RD's Paths4401 has the unnamed zone at
    /// (835.05, −1214.87) facing (0.26, −0.96) with z 0, which places;
    /// L4RA's only kind-3 record is PCDEMO, which does not.
    fn kind3_blocks() {
        let tracks = crate::testkit::game_root().join("TRACKS");
        let d = std::fs::read(tracks.join("ROUTESL4RD/Paths4401.bin")).expect("ROUTESL4RD/Paths4401.bin");
        let b = ResetZone::parse_blocks(&d);
        assert_eq!(b.len(), 1);
        let (at, h) = b[0].place().expect("z 0 places");
        assert!((at - Vec2::new(835.05, -1214.87)).length() < 0.1 && (h - (-0.96f32).atan2(0.26)).abs() < 0.05);
        assert!(b[0].trigger.contains(Vec2::new(845.0, -1200.0)) && !b[0].trigger.contains(at));
        let d = std::fs::read(tracks.join("ROUTESL4RA/Paths4001.bin")).expect("ROUTESL4RA/Paths4001.bin");
        assert!(ResetZone::parse_blocks(&d).is_empty());
    }

    /// The road entries file an object behind the list's first point too
    /// (0x419C10 → 0x419B20 → 0x4198C0 with flags 0): its s is the
    /// nearest point's +0x54 − +0x58 + the along (0x40AE30), extrapolated;
    /// only an empty list files nothing. `locate` (the clone's span-limited
    /// projection) gives none there.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn objects_behind_the_list_are_filed_with_an_extrapolated_distance() {
        let root = crate::testkit::game_root();
        let routes = std::fs::read(root.join("TRACKS/ROUTESL4RA/Routes4014F.bin")).expect("Routes4014F.bin");
        let r = AiRoutes {
            file: RouteFile::parse(&routes),
            track: 4014,
            backward: false,
            race_coords: true,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        let at = Vec3::new(-224.25, 87.09, 7.32);
        let route = RacerRoute::start_human(&r, at, Vec2::new(-1.0, 0.0)).expect("a node");
        let (first, _) = route.list_span().unwrap();
        // 40 m behind the car, along its way back (+x).
        let behind = at + Vec3::new(40.0, 0.0, 0.0);
        assert!(route.locate(&r, behind).is_none());
        let filed = route.locate_object(&r, behind).expect("filed");
        assert!(filed.s < first - 30.0 && filed.s > first - 50.0, "s {} first {first}", filed.s);
        assert_eq!(route.nearest_index(&r, behind), Some(0));
    }

    /// The human car's list on the Quick Race grid of 4014 forward, as read
    /// under Wine (car +0x2C → +0x68, node +0x54 and the inserted
    /// flag +0x74): 0, 8*, 20.6, 28.6*, 38.7, 53.3, 61.3*, 70.3, 84.4,
    /// 92.4*, 104.1, 118.9, 126.9*, 138.6 — the 8 m points 0x418DA0 puts
    /// into every gap of 16 m or more, inside a record too; facing the
    /// other way the list runs the other way (records 655 → 476).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn human_list_matches_the_original_grid() {
        let root = crate::testkit::game_root();
        let routes = std::fs::read(root.join("TRACKS/ROUTESL4RA/Routes4014F.bin")).expect("Routes4014F.bin");
        let r = AiRoutes {
            file: RouteFile::parse(&routes),
            track: 4014,
            backward: false,
            race_coords: true,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        let at = Vec3::new(-224.25, 87.09, 7.32);
        let route = RacerRoute::start_human(&r, at, Vec2::new(-1.0, 0.0)).expect("a node");
        let s: Vec<f32> = route.points.iter().map(|p| p.s - route.points[0].s).take(14).collect();
        let want = [0.0, 8.0, 20.6, 28.6, 38.7, 53.3, 61.3, 70.3, 84.4, 92.4, 104.1, 118.9, 126.9, 138.6];
        assert_eq!(s.len(), want.len(), "{s:?}");
        for (a, b) in s.iter().zip(want) {
            assert!((a - b).abs() < 0.1, "{s:?}");
        }
        let id = |p: &FuturePoint| r.file.records[p.record].id;
        assert!(
            route.points.iter().take(4).all(|p| id(p) == 476),
            "{:?}",
            route.points.iter().take(4).map(id).collect::<Vec<_>>()
        );
        // At (−270.3, 85.9) the original's list read 655/5, 655/4, 655/4*,
        // 655/3 facing −x and 655/5, 655/6, 655/6*, 476/2 facing +x.
        let at = Vec3::new(-270.3, 85.9, 8.6);
        let nodes = |dir: f32| {
            RacerRoute::start_human(&r, at, Vec2::new(dir, 0.0))
                .expect("a node")
                .points
                .iter()
                .take(4)
                .map(|p| (id(p), p.cp, p.inserted.is_some()))
                .collect::<Vec<_>>()
        };
        assert_eq!(nodes(-1.0), vec![(655, 5, false), (655, 4, false), (655, 4, true), (655, 3, false)]);
        assert_eq!(nodes(1.0), vec![(655, 5, false), (655, 6, false), (655, 6, true), (476, 2, false)]);
    }

    /// The original's route state during a Quick Race circuit on 4014
    /// forward (class 3, speed mode 2; memory read under Wine, see
    /// `docs/formats.md`): records unchanged from the file but for the
    /// lane speeds, the junctions but for 0x5E8790's type adjustment.
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn race_4014_speeds_match_the_original() {
        let root = crate::testkit::game_root();
        let read = |p: std::path::PathBuf| std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let (lanes, jm, routes, g) = (
            crate::testkit::private("docs/evidence/race-routes/race4014F_lanes.bin"),
            crate::testkit::private("docs/evidence/race-routes/race4014F_junctions.bin"),
            read(root.join("TRACKS/ROUTESL4RA/Routes4014F.bin")),
            read(root.join("GLOBAL/GLOBALB.BUN")),
        );
        let curves = ClassCurves::parse(&g).unwrap();
        let mut r = AiRoutes {
            file: RouteFile::parse(&routes),
            track: 4014,
            backward: false,
            race_coords: true,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        r.racing_speeds(&curves, 3, false, 2, Some(RaceLap { length: 3001.135, wrap: true }));
        let (mut at, mut points, mut bad) = (0, 0, 0);
        while at + 8 <= lanes.len() {
            let id = u16::from_le_bytes([lanes[at], lanes[at + 1]]);
            let n = usize::from(u16::from_le_bytes([lanes[at + 2], lanes[at + 3]]));
            let flags = u32::from_le_bytes(lanes[at + 4..at + 8].try_into().unwrap());
            let rec = &r.file.records[r.file.record(id as i16).unwrap()];
            assert_eq!(rec.flags, flags, "record {id} flags");
            for k in 0..n {
                let m = &lanes[at + 8 + 4 * k..at + 12 + 4 * k];
                if rec.points[k].lane_speed[..4] != m[..4] {
                    if bad < 400 {
                        eprintln!("record {id} point {k}: {:?} vs {:?}", &rec.points[k].lane_speed[..4], m);
                    }
                    bad += 1;
                }
                points += 1;
            }
            at += 8 + 4 * n;
        }
        let mut bad_j = 0;
        for (k, (j, m)) in r.file.junctions.iter().zip(jm.chunks_exact(32)).enumerate() {
            let theirs = u32::from_le_bytes(m[12..16].try_into().unwrap());
            if j.flags != theirs {
                eprintln!("junction {k}: {:#x} vs {theirs:#x}", j.flags);
                bad_j += 1;
            }
        }
        eprintln!("{bad} of {points} points, {bad_j} junctions differ");
        assert_eq!(r.file.junctions.len() * 32, jm.len());
        assert_eq!(bad_j, 0);
        assert_eq!(bad, 0);
    }

    #[test]
    fn class_curves_interpolate_and_clamp() {
        let mut c = ClassCurves { top: [63.0; 4], accel: [[0.0; 64]; 4] };
        for i in 0..64 {
            c.accel[1][i] = i as f32;
        }
        assert_eq!(c.accel(1, -1.0), 0.0);
        assert_eq!(c.accel(1, 10.5), 10.5);
        assert_eq!(c.accel(1, 200.0), 63.0);
    }

    #[test]
    fn speed_mode_follows_the_track_flags() {
        assert_eq!(speed_mode(0x71, 4013), 2);
        assert_eq!(speed_mode(0x2, 4013), 0);
        assert_eq!(speed_mode(0x8, 4013), 1);
        assert_eq!(speed_mode(0x800, 4013), 4);
        assert_eq!(speed_mode(0x2000, 0x1262), 6);
        assert_eq!(speed_mode(0, 4013), 3);
    }

    /// The field's class for the racing lines (0x5DDFD0): 0 in a career
    /// race with AI racers, the cars' own in a Quick Race, 3 without AI.
    #[test]
    fn field_class_by_start() {
        assert_eq!(field_class(true, Some(2)), 0);
        assert_eq!(field_class(true, Some(3)), 0);
        assert_eq!(field_class(false, Some(2)), 2);
        assert_eq!(field_class(true, None), 3);
        assert_eq!(field_class(false, None), 3);
    }

    /// A stage-4 career URL circuit (track 4714 forward, speed mode 6): the
    /// rivals' path points under Wine (record 411, control points 36–41,
    /// 48–50, 54–58) carry lane-0 speeds equal to the class-0 line bytes;
    /// the cars' own class (2) gives 2–4 m/s less.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn career_url_lines_use_class_0() {
        let root = crate::testkit::game_root();
        let read = |p: &str| std::fs::read(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
        let curves = ClassCurves::parse(&read("GLOBAL/GLOBALB.BUN")).unwrap();
        let routes = read("TRACKS/ROUTESL4RG/Routes4714F.bin");
        let lane0 = |class: usize| {
            let mut r = AiRoutes {
                file: RouteFile::parse(&routes),
                track: 4714,
                backward: false,
                race_coords: true,
                resets: Vec::new(),
                blocks: Vec::new(),
            };
            r.racing_speeds(&curves, class, false, speed_mode(0x2000, 4714), None);
            let rec = &r.file.records[r.file.record(411).unwrap()];
            [36..=41, 48..=50, 54..=58].into_iter().flatten().map(|k| rec.points[k].lane_speed[0]).collect::<Vec<u8>>()
        };
        let wine = [47, 47, 46, 44, 43, 41, 41, 39, 40, 45, 43, 43, 43, 43];
        assert_eq!(lane0(field_class(true, Some(2))), wine);
        assert!(lane0(2).iter().zip(wine).all(|(a, b)| *a < b), "class 2: {:?}", lane0(2));
    }
}
