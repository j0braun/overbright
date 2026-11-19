//! The AI route files `TRACKS/ROUTES<region>/Routes<track><F|B>.bin`
//! (path built at 0x57F554, chunks taken by 0x5DEB90; free roam reads
//! `RoutesFreeRoam.bin`): the splines AI racers and traffic follow.
//!
//! * 0x34121 — route records, one after another: a 0x8C-byte header and
//!   +0x34 control points of 0x38 bytes. 0x5DE1D0 registers each by its id
//!   (+0x0A) in the route manager (0x88A334[id]); 0x5D9440 lists those with
//!   header +0x70 bit 6 set and, of them, the ones whose +0x0C is 0 are the
//!   race records the racers start on (0x88D218 / 0x88D21C).
//! * 0x34122 — junctions, 32 bytes each after 4 bytes of padding (0x883DB0,
//!   count (size − 4) / 32).
//! * 0x34123 — traffic intersections, 0x114 bytes each (0x883DBC, count
//!   size / 0x114): see [`Intersection`]. Only the traffic AI reads them.
//! * 0x34124 — race traffic teleporters, 0x108 bytes each (0x883DC4, count
//!   size / 0x108; in no shipped file); read by 0x4111C0 / 0x417980 but
//!   never present, so race traffic has nothing to place.
//!
//! A route is piecewise linear: a point `d` metres along a control point's
//! direction and `l` to its right is P + L·l + F·d, F = (tangent, grade)
//! and L = (tangent turned right, bank) (0x5DD370). Travelled against the
//! control points' order, a control point's direction is the previous
//! one's reversed (0x401AA0, its own when it is the first) and the right
//! side flips with it (0x5DD4A0).

use std::collections::HashMap;

use glam::{Vec2, Vec3};

use crate::formats::bundle::{chunks, walk};
use crate::formats::reader::{FormatResult, Reader};

/// A control point (0x38 bytes).
#[derive(Clone, Debug)]
pub struct ControlPoint {
    /// x, y (+0x00, +0x04) and z (+0x12, i16 / 8).
    pub position: Vec3,
    /// +0x08: a running coordinate along the route network (open).
    pub coord: f32,
    /// +0x0C: bits 0–10 the owning record's id; bit 11 a start point for
    /// racers (0x5E2E50 mask 4); bits 19–24 the traffic intersection the
    /// point stands at (0x3F none, 0x5D8960); bit 25 for mask 0x200, bit 27
    /// for 0x400.
    pub flags: u32,
    /// +0x10: length of the segment to the next control point (u16 / 256).
    pub length: f32,
    /// +0x14 / +0x16: the road's half widths to the left and right (u16 / 256).
    pub half_width: [f32; 2],
    /// +0x18: unit direction (i16 / 32767 each).
    pub tangent: Vec2,
    /// +0x1C / +0x1E: junction indices (0xFFFF none).
    pub junctions: [u16; 2],
    /// +0x20: lateral slope, +0x21: dz per metre (i8 / 128).
    pub bank: f32,
    pub grade: f32,
    /// +0x22 + lane: the lane's lateral position, a fraction (i8 / 128) of
    /// the wider half width. Lanes: 0 the racing line forward, 1 backward,
    /// 2–5 the forward traffic lanes, 6–9 the backward ones.
    pub lane_position: [i8; 10],
    /// +0x2C + lane: the lane's speed, m/s (the racing lines read 100).
    pub lane_speed: [u8; 10],
}

/// A route record: its header and control points.
#[derive(Clone, Debug)]
pub struct RouteRecord {
    /// +0x0A: the id the junctions and control point flags refer to.
    pub id: u16,
    /// +0x10: e.g. "TrackRoutesA31".
    pub name: String,
    /// +0x0C: 0 for the race records (0x5D9440).
    pub kind: u32,
    /// +0x38 / +0x3C: the first and last control points' coordinates.
    pub coord_range: (f32, f32),
    /// +0x60: min x, min y, max x, max y (0x43D410's box test, 0x417EFF).
    pub bounds: [f32; 4],
    /// +0x70: flags; bit 6 listed (0x5D9440), bit 10 never branched into by
    /// racers (0x413CC0).
    pub flags: u32,
    /// +0x74 / +0x75 forward, +0x7C / +0x7D backward: when either is set the
    /// lateral adjustments of points walked through fade (0x422220 →
    /// 0x40AA20).
    pub fade: [bool; 2],
    /// +0x82..+0x8B: whether lane 0–9 exists on this record (0x410CA0 reads
    /// it to place traffic on lanes 2–5 / 6–9).
    pub lanes: [bool; 10],
    pub points: Vec<ControlPoint>,
}

impl RouteRecord {
    /// A race record (0x5D9440): listed (+0x70 bit 6) with +0x0C 0.
    pub fn is_race(&self) -> bool {
        self.flags >> 6 & 1 != 0 && self.kind == 0
    }
}

/// A junction (32 bytes): where a control point links to another record.
#[derive(Clone, Copy, Debug)]
pub struct Junction {
    /// +0x04 / +0x06: the target control point (+0x06 for type 3).
    pub target_point: [i16; 2],
    /// +0x08 / +0x0A: the target record id (+0x0A for type 3).
    pub target_record: [i16; 2],
    /// +0x0C: low nibble the type (2 race link, 3 alternate link, 4 a
    /// branch racers may take, 5/7/10 traffic); bit 18 the direction it
    /// belongs to, bit 20 never for racers, bit 23 flips the direction, bit
    /// 26 a turn, bit 27 a branch taken a third of the time.
    pub flags: u32,
}

impl Junction {
    pub fn kind(&self) -> u32 {
        self.flags & 0xF
    }
    pub fn direction(&self) -> bool {
        self.flags >> 18 & 1 != 0
    }
    pub fn flips(&self) -> bool {
        self.flags >> 23 & 1 != 0
    }
    pub fn turn(&self) -> bool {
        self.flags >> 26 & 1 != 0
    }
    /// The target record id and control point (0x5D86B0 / 0x5D86E0).
    pub fn target(&self) -> (i16, i16) {
        let k = usize::from(self.kind() == 3);
        (self.target_record[k], self.target_point[k])
    }
}

/// A traffic intersection (chunk 0x34123, 0x114 bytes): where two roads
/// cross. +0x00 the kind (every one of L4RA's 38 free-roam intersections
/// is 1: no stop, the roads take turns; 2: road 1 stops first; 3: every
/// road stops first, 0x425150), +0x04 the two roads' record ids, +0x0A the
/// control point of each road at the crossing. The rest is run-time state
/// the game keeps in place: +0x08 per road the nearest registered car,
/// +0x10 the road holding the right of way (−1 none) and at +0x14 sixteen
/// 16-byte "IncomingTrafficCar" slots, one per traffic car (0x410750).
#[derive(Clone, Copy, Debug)]
pub struct Intersection {
    pub kind: u32,
    pub roads: [i16; 2],
    pub points: [i16; 2],
}

/// A parsed route file.
#[derive(Clone, Debug, Default)]
pub struct RouteFile {
    pub records: Vec<RouteRecord>,
    /// Record index by id.
    pub by_id: HashMap<u16, usize>,
    pub junctions: Vec<Junction>,
    pub intersections: Vec<Intersection>,
    /// Each record's four shorts at +0x44 (by record index): 0x5E41B0
    /// unlists, in free roam, a record naming 4000 there.
    pub record_tracks: Vec<[i16; 4]>,
}

impl RouteFile {
    pub fn parse(data: &[u8]) -> RouteFile {
        let mut file = RouteFile::default();
        walk(chunks(data, 0), &mut |c| match c.id {
            0x34121 => {
                let mut b = Reader::new(c.body);
                let record = |b: &mut Reader<'_>| -> FormatResult<Option<(RouteRecord, [i16; 4])>> {
                    let at = b.position();
                    let count = b.i16_at(at + 0x34)?.max(0) as usize;
                    // A cut-off last record ends the table.
                    let Ok(r) = b.sub(0x8C + count * 0x38) else { return Ok(None) };
                    let points = (0..count).map(|k| point(&r.sub_at(0x8C + k * 0x38, 0x38)?)).collect::<FormatResult<_>>()?;
                    let flag = |at: usize| r.u8_at(at).map(|v| v != 0);
                    let record = RouteRecord {
                        id: r.u16_at(0x0A)?,
                        name: r.cstr_at(0x10, 0x20)?,
                        kind: r.u32_at(0x0C)?,
                        coord_range: (r.f32_at(0x38)?, r.f32_at(0x3C)?),
                        bounds: [r.f32_at(0x60)?, r.f32_at(0x64)?, r.f32_at(0x68)?, r.f32_at(0x6C)?],
                        flags: r.u32_at(0x70)?,
                        fade: [flag(0x74)? || flag(0x75)?, flag(0x7C)? || flag(0x7D)?],
                        lanes: r.array_at::<10>(0x82)?.map(|v| v != 0),
                        points,
                    };
                    Ok(Some((record, [r.i16_at(0x44)?, r.i16_at(0x46)?, r.i16_at(0x48)?, r.i16_at(0x4A)?])))
                };
                while b.remaining() >= 0x8C {
                    // Every read is inside a record whose size was checked.
                    let Ok(Some((record, tracks))) = record(&mut b) else { break };
                    file.by_id.insert(record.id, file.records.len());
                    file.records.push(record);
                    file.record_tracks.push(tracks);
                }
            }
            0x34122 => {
                let junction = |j: Reader<'_>| -> FormatResult<Junction> {
                    Ok(Junction { target_point: [j.i16_at(4)?, j.i16_at(6)?], target_record: [j.i16_at(8)?, j.i16_at(0x0A)?], flags: j.u32_at(0x0C)? })
                };
                let mut b = Reader::new(c.body);
                file.junctions = match b.skip(4) {
                    // Slot 0xFFFF means none, so at most 0xFFFE.
                    Ok(()) => b.whole_records(32).filter_map(|j| junction(j).ok()).take(0xFFFE).collect(),
                    Err(_) => Vec::new(),
                };
            }
            0x34123 => {
                let intersection = |e: Reader<'_>| -> FormatResult<Intersection> {
                    Ok(Intersection { kind: e.u32_at(0)?, roads: [e.i16_at(4)?, e.i16_at(6)?], points: [e.i16_at(0x0A)?, e.i16_at(0x0C)?] })
                };
                file.intersections = Reader::new(c.body).whole_records(0x114).filter_map(|e| intersection(e).ok()).collect();
            }
            _ => {}
        });
        file
    }

    /// The traffic intersection a control point stands at (0x5D8960: flag
    /// bits 19–24, not 0x3F and inside the table, and 0x410BF0: both its
    /// roads exist with record flag bit 6).
    pub fn intersection_at(&self, cp: &ControlPoint) -> Option<usize> {
        let i = (cp.flags >> 19 & 0x3F) as usize;
        let x = self.intersections.get(i).filter(|_| i != 0x3F)?;
        x.roads.iter().all(|&r| self.record(r).is_some_and(|k| self.records[k].flags >> 6 & 1 != 0)).then_some(i)
    }

    /// A road's control point at an intersection.
    pub fn crossing_point(&self, x: &Intersection, road: usize) -> Option<(usize, usize)> {
        let record = self.record(x.roads[road])?;
        let cp = usize::try_from(x.points[road]).ok().filter(|&c| c < self.records[record].points.len())?;
        Some((record, cp))
    }

    /// A record by id.
    pub fn record(&self, id: i16) -> Option<usize> {
        u16::try_from(id).ok().and_then(|id| self.by_id.get(&id).copied())
    }

    /// A control point's valid junctions, in slot order (0x5D4390 /
    /// 0x5D43D0: not 0xFFFF and inside the table).
    pub fn junctions_at(&self, cp: &ControlPoint) -> impl Iterator<Item = usize> + '_ {
        cp.junctions.into_iter().map(usize::from).filter(|&j| j != 0xFFFF && j < self.junctions.len())
    }

    /// The direction a control point is travelled in (0x401AA0 when
    /// backward: the previous point's direction reversed, its own reversed
    /// for the first).
    pub fn direction(&self, record: usize, cp: usize, backward: bool) -> Vec2 {
        let points = &self.records[record].points;
        if !backward {
            return points[cp].tangent;
        }
        -points[cp.checked_sub(1).unwrap_or(cp)].tangent
    }

    /// The segment length a point covers travelled each way (vt+0x40:
    /// its own forward, the previous point's backward, 0x40D200).
    pub fn segment(&self, record: usize, cp: usize, backward: bool) -> f32 {
        let points = &self.records[record].points;
        if backward { points[cp.checked_sub(1).unwrap_or(cp)].length } else { points[cp].length }
    }

    /// The point `d` metres along a control point's direction and `l` to
    /// its right (0x5DD370 forward, 0x5DD4A0 backward).
    pub fn point(&self, record: usize, cp: usize, backward: bool, d: f32, l: f32) -> Vec3 {
        let points = &self.records[record].points;
        let c = &points[cp];
        let (t, grade, bank) = if backward {
            let p = &points[cp.checked_sub(1).unwrap_or(cp)];
            (-p.tangent, -p.grade, -c.bank)
        } else {
            (c.tangent, c.grade, c.bank)
        };
        let forward = Vec3::new(t.x, t.y, grade);
        let right = Vec3::new(t.y, -t.x, bank);
        c.position + right * l + forward * d
    }

    /// How far a point lies along a control point's direction (vt+0x2C:
    /// 0x40D160 forward, 0x40D3D0 backward).
    pub fn along(&self, record: usize, cp: usize, backward: bool, p: Vec2) -> f32 {
        (p - self.records[record].points[cp].position.truncate()).dot(self.direction(record, cp, backward))
    }

    /// How far a point lies to the right of a control point's direction
    /// (vt+0x34: 0x40D1B0 forward, 0x40D410 backward).
    pub fn lateral(&self, record: usize, cp: usize, backward: bool, p: Vec2) -> f32 {
        let t = self.direction(record, cp, backward);
        (p - self.records[record].points[cp].position.truncate()).dot(Vec2::new(t.y, -t.x))
    }
}

/// A lane as a point travelled forward or backward reads it (FuturePoint
/// vt+8: forward as is; backward 0 → 1 and 2–5 → 6–9, 0x40D3A0).
pub fn lane(lane: usize, backward: bool) -> usize {
    match (backward, lane) {
        (true, 0) => 1,
        (true, 2..=5) => lane + 4,
        _ => lane,
    }
}

/// A lane's lateral position at a control point (0x401C40): the wider half
/// width times the lane's fraction.
pub fn lane_offset(cp: &ControlPoint, lane: usize) -> f32 {
    cp.half_width[0].max(cp.half_width[1]) * f32::from(cp.lane_position.get(lane).copied().unwrap_or(0)) / 128.0
}

/// A paths-file barrier as the route post-load reads it (0x88F298 +0x49C,
/// 0x24 bytes): its two ends (+0x00, +0x08), closed (+0x10) and +0x12.
#[derive(Clone, Copy, Debug)]
pub struct RouteBarrier {
    pub ends: [Vec2; 2],
    pub closed: bool,
    pub held: bool,
}

impl RouteBarrier {
    /// The barriers of a parsed paths file after its `close_barriers`.
    pub fn from_net(net: &crate::formats::roads::RoadNet) -> Vec<RouteBarrier> {
        net.barrier_ends
            .iter()
            .enumerate()
            .map(|(k, &ends)| RouteBarrier { ends, closed: net.closed.get(k).copied().unwrap_or(false), held: net.barrier_held.get(k).copied().unwrap_or(false) })
            .collect()
    }
}

impl RouteFile {
    /// The free-roam post-load of the route network (0x5E9E90 with
    /// g_RouteCoordRace 0x88F2BE clear), run once the paths file's barriers
    /// are closed (0x578070: `BARRIERS_<track>`, `PLAYER_BARRIERS_<track>`,
    /// `BARRIERS_CAREER<stage>`). It retypes junctions, drops the ones
    /// touching unlisted records (the table keeps only the live ones,
    /// 0x883DB4), reorders the table and the control points' junction
    /// indices with it, and sets the records' +0x70 bits and the points'
    /// +0x0C bits 11/18/25–28. See `docs/formats.md` (AI routes).
    pub fn finalize_free_roam(&mut self, barriers: &[RouteBarrier]) {
        let live = PostLoad::run(self, barriers).live;
        self.junctions.truncate(live);
    }

    /// [`Self::finalize_free_roam`] with the barriers of the paths file
    /// (`PathsFreeRoam.bin`'s bytes) that 0x578070 closes with no race:
    /// `BARRIERS_<track>`, `PLAYER_BARRIERS_<track>` (free roam 4000) and
    /// `BARRIERS_CAREER<stage>` (0x862838; 5 in game modes 0x200 and 8 with
    /// 0x850088 = 8).
    pub fn finalize_free_roam_paths(&mut self, paths: &[u8], track: u32, stage: u32) {
        let mut net = crate::formats::roads::RoadNet::parse(paths);
        net.close_barriers(&[&format!("BARRIERS_{track}"), &format!("PLAYER_BARRIERS_{track}"), &format!("BARRIERS_CAREER{stage}")]);
        self.finalize_free_roam(&RouteBarrier::from_net(&net));
    }
}

/// g_RouteLapLength (0x88F298 +0x18 = 0x88F2B0) during the free-roam
/// post-load: the junctions dropped there are keyed 10 · lap + index, and
/// the original's table after the intro holds 2083.0 where the index is
/// 2073 (observed). Without a race 0x57A6CF calls 0x5D9A70 on the route
/// data, which stores 1.0 there; a race's road build (0x5E5BC0) stores
/// its length instead, but races never run this pruning (0x88A330 is
/// never 1, see `docs/formats.md`).
const FREE_ROAM_LAP: f32 = 1.0;

/// A control point: record index, point index.
type Pt = (usize, usize);

/// The state of 0x5E9E90: the junction sort keys (+0x00) and the live
/// count 0x883DB4; the table length is 0x883DB8.
struct PostLoad<'a> {
    f: &'a mut RouteFile,
    keys: Vec<f32>,
    live: usize,
    barriers: &'a [RouteBarrier],
    /// The records 0x5D9440 listed (+0x70 bit 6), by id.
    listed: Vec<usize>,
}

/// A junction's type `k` (0 and 2 the low nibble, 1 bits 4–7).
fn ktype(flags: u32, k: usize) -> u32 {
    if k == 1 { flags >> 4 & 0xF } else { flags & 0xF }
}

/// 0x5DCBA0: type `k` is a dead link (7; 8 too for k = 0).
fn dead(flags: u32, k: usize) -> bool {
    match k {
        0 => matches!(flags & 0xF, 7 | 8),
        1 => flags >> 4 & 0xF == 7,
        _ => flags & 0xF == 7,
    }
}

/// 0x5DCBF0: type `k` is a link racers take (2; 8 too for k = 2).
fn link(flags: u32, k: usize) -> bool {
    match k {
        0 => flags & 0xF == 2,
        1 => flags >> 4 & 0xF == 2,
        _ => matches!(flags & 0xF, 2 | 8),
    }
}

fn bit(flags: u32, b: u32) -> bool {
    flags >> b & 1 != 0
}

/// 0x5D4D60: whether segments a1–a2 and b1–b2 cross (both parameters in
/// [0, 1]; parallel never).
fn crosses(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let (a1, a2, b1, b2) = (a1.as_dvec2(), a2.as_dvec2(), b1.as_dvec2(), b2.as_dvec2());
    let d = (a2.x - a1.x) * (b2.y - b1.y) - (b2.x - b1.x) * (a2.y - a1.y);
    if d == 0.0 {
        return false;
    }
    let ua = ((a1.y - b1.y) * (b2.x - b1.x) - (a1.x - b1.x) * (b2.y - b1.y)) * (1.0 / d);
    if !(0.0..=1.0).contains(&ua) {
        return false;
    }
    let ub = ((a1.y - b1.y) * (a2.x - a1.x) - (a1.x - b1.x) * (a2.y - a1.y)) * (1.0 / d);
    (0.0..=1.0).contains(&ub)
}

/// 0x5D4100: the heap sort (Numerical Recipes' hpsort) over `a` by
/// `less`, as the game runs it (not stable).
fn heap_sort(a: &mut [usize], less: impl Fn(usize, usize) -> bool) {
    let n = a.len();
    if n < 2 {
        return;
    }
    let (mut l, mut ir) = (n / 2 + 1, n);
    loop {
        let rra;
        if l > 1 {
            l -= 1;
            rra = a[l - 1];
        } else {
            rra = a[ir - 1];
            a[ir - 1] = a[0];
            ir -= 1;
            if ir == 1 {
                a[0] = rra;
                return;
            }
        }
        let (mut i, mut j) = (l, l * 2);
        while j <= ir {
            if j < ir && less(a[j - 1], a[j]) {
                j += 1;
            }
            if less(rra, a[j - 1]) {
                a[i - 1] = a[j - 1];
                i = j;
                j *= 2;
            } else {
                j = ir + 1;
            }
        }
        a[i - 1] = rra;
    }
}

impl<'a> PostLoad<'a> {
    /// 0x5E9E90's free-roam branch.
    fn run(f: &'a mut RouteFile, barriers: &'a [RouteBarrier]) -> PostLoad<'a> {
        let n = f.junctions.len();
        let mut s = PostLoad { f, keys: vec![0.0; n], live: n, barriers, listed: Vec::new() };
        s.reset_types();
        s.list_free_roam();
        s.list();
        let removed = s.unlink_unlisted();
        s.sort();
        s.live -= removed;
        s.order_slots();
        s.mark_barred();
        while s.prune() {}
        // 0x5D9600: records lose +0x70 bit 8, points their marks.
        for r in &mut s.f.records {
            r.flags &= !0x100;
            r.points.iter_mut().for_each(|p| p.flags &= 0xE1FB_F7FF);
        }
        s.mark_reachable();
        // 0x5DE8E0: records no junction reaches are unlisted (bits 6, 7, 9).
        for r in &mut s.f.records {
            if !bit(r.flags, 8) {
                r.flags &= 0xFFFF_FD3F;
            }
        }
        s.list();
        let removed = s.drop_unlisted();
        s.sort();
        s.live -= removed;
        // Traffic junctions (types 5, 10) mark the record they lead to:
        // bit 2 when the junction's direction bit matches its own record's
        // bit 1, else bit 3.
        for j in 0..s.live {
            let jn = s.f.junctions[j];
            if matches!(jn.flags & 0xF, 5 | 10) {
                let b = s.rec(jn.target_record[1]).is_some_and(|r| bit(s.f.records[r].flags, 1));
                let mask = if bit(jn.flags, 18) == b { 1 << 2 } else { 1 << 3 };
                if let Some(r) = s.rec(jn.target_record[0]) {
                    s.f.records[r].flags |= mask;
                }
            }
        }
        s
    }

    /// g_RouteManager (0x88A334)[id].
    fn rec(&self, id: i16) -> Option<usize> {
        self.f.record(id)
    }

    fn rflags(&self, id: i16) -> u32 {
        self.rec(id).map_or(0, |r| self.f.records[r].flags)
    }

    fn jflags(&self, j: usize) -> u32 {
        self.f.junctions[j].flags
    }

    /// 0x5D8730: set junction `j`'s type `k`.
    fn set_type(&mut self, j: usize, k: usize, v: u32) {
        let f = &mut self.f.junctions[j].flags;
        *f = if k == 1 { *f & !0xF0 | v << 4 } else { *f & !0xF | v };
    }

    fn point(&self, rec: Option<usize>, p: i16) -> Option<Pt> {
        let r = rec?;
        usize::try_from(p).ok().filter(|&p| p < self.f.records[r].points.len()).map(|p| (r, p))
    }

    /// The junction's own control point (+0x0A, +0x06).
    fn own(&self, j: usize) -> Option<Pt> {
        let jn = self.f.junctions[j];
        self.point(self.rec(jn.target_record[1]), jn.target_point[1])
    }

    /// 0x5D86E0: the control point the junction leads to.
    fn target(&self, j: usize) -> Option<Pt> {
        let (r, p) = self.f.junctions[j].target();
        self.point(self.rec(r), p)
    }

    /// The id of the record the junction leads to (+0x0A for type 3).
    fn target_id(&self, j: usize) -> i16 {
        self.f.junctions[j].target().0
    }

    /// 0x5D43D0 over a point: its junctions below 0x883DB4, slot order.
    fn slots(&self, (r, p): Pt) -> Vec<usize> {
        self.f.records[r].points[p].junctions.iter().map(|&j| usize::from(j)).filter(|&j| j != 0xFFFF && j < self.live).collect()
    }

    /// The next control point along the record (none past either end).
    fn step(&self, (r, p): Pt, forward: bool) -> Option<Pt> {
        if forward {
            (p + 1 < self.f.records[r].points.len()).then_some((r, p + 1))
        } else {
            p.checked_sub(1).map(|p| (r, p))
        }
    }

    fn xy(&self, (r, p): Pt) -> Vec2 {
        self.f.records[r].points[p].position.truncate()
    }

    /// 0x5D98D0 (and the record flag resets of 0x5E41B0): the live count
    /// is the whole table; each junction's key 0 and its two types taken
    /// from the file's byte 1 (bits 8–11 → 0–3, 12–15 → 4–7).
    fn reset_types(&mut self) {
        for (j, key) in self.f.junctions.iter_mut().zip(&mut self.keys) {
            *key = 0.0;
            j.flags = j.flags & !0xFF | j.flags >> 8 & 0xF | (j.flags >> 12 & 0xF) << 4;
        }
        for r in &mut self.f.records {
            r.flags &= !2;
        }
    }

    /// 0x5E41B0, free roam: 0x5D9660(1) lists every record (bit 6; bits 7,
    /// 9, 11 clear); then each record's bit 1, its points' coordinates
    /// (+0x08) and +0x38/+0x3C are cleared and it stays listed unless a
    /// short at +0x44 is 4000.
    fn list_free_roam(&mut self) {
        for (r, tracks) in self.f.records.iter_mut().zip(&self.f.record_tracks) {
            r.flags = r.flags & 0xFFFF_F53F | 0x40;
            r.flags &= !2;
            r.points.iter_mut().for_each(|p| p.coord = 0.0);
            r.coord_range = (0.0, 0.0);
            if tracks.contains(&4000) {
                r.flags &= !0x40;
            }
        }
    }

    /// 0x5D9440: the listed records (+0x70 bit 6), by id.
    fn list(&mut self) {
        let mut ids: Vec<u16> = self.f.by_id.keys().copied().filter(|&id| id < 0x5DC).collect();
        ids.sort_unstable();
        self.listed = ids.into_iter().map(|id| self.f.by_id[&id]).filter(|&r| bit(self.f.records[r].flags, 6)).collect();
    }

    /// 0x5E27E0 (free roam): a junction on an unlisted record becomes 0x77
    /// and leaves; one into an unlisted record has its types 7 (a race
    /// link 2 on k = 0/2 is cut back through 0x5E1600; on k = 1, or with
    /// the record's bit 7, both types become 3) and leaves when all three
    /// read 7. Each junction's key is the race road distance of its own
    /// point (0x5D9E40 +0x24: 0 with no race road), a leaving one's
    /// 10 · lap + index (the lap 0x88F2B0, 1.0 in free roam). Returns how
    /// many left.
    fn unlink_unlisted(&mut self) -> usize {
        let mut removed = 0;
        for j in 0..self.live {
            let own = self.rflags(self.f.junctions[j].target_record[1]);
            let tgt = self.rflags(self.target_id(j));
            self.keys[j] = 0.0;
            if !bit(own, 6) {
                self.f.junctions[j].flags = self.jflags(j) & 0xFFFF_FF77 | 0x77;
                self.keys[j] = FREE_ROAM_LAP * 10.0 + j as f32;
                removed += 1;
            } else if !bit(tgt, 6) {
                let mut all7 = true;
                for k in 0..3 {
                    match ktype(self.jflags(j), k) {
                        0 | 1 | 4 | 5 | 6 | 8 | 9 | 10 => self.set_type(j, k, 7),
                        2 => {
                            if !bit(own, 7) && k != 1 {
                                self.set_type(j, k, 7);
                                self.cut(j, k, 1);
                            } else {
                                self.f.junctions[j].flags = self.jflags(j) & 0xFFFF_FF33 | 0x33;
                            }
                        }
                        _ => {}
                    }
                    all7 &= ktype(self.jflags(j), k) == 7;
                }
                if all7 {
                    self.keys[j] = FREE_ROAM_LAP * 10.0 + j as f32;
                    removed += 1;
                }
            }
            // The bit 9 branches (race road records) cannot run: 0x5D9660
            // cleared bit 9 on every record.
        }
        removed
    }

    /// 0x5E2BE0: sort the whole table (0x883DB8 entries) by key (0x5D4100
    /// with 0x5D42C0, key a − key b) and renumber the control points'
    /// junction slots (+0x1C/+0x1E, those below the table length).
    fn sort(&mut self) {
        let n = self.f.junctions.len();
        let mut order: Vec<usize> = (0..n).collect();
        let keys = &self.keys;
        heap_sort(&mut order, |a, b| keys[a] - keys[b] < 0.0);
        let mut new = vec![0u16; n];
        for (k, &old) in order.iter().enumerate() {
            new[old] = k as u16;
        }
        for r in &mut self.f.records {
            if r.id >= 0x5DC {
                continue;
            }
            for p in &mut r.points {
                for s in &mut p.junctions {
                    if *s != 0xFFFF && usize::from(*s) < n {
                        *s = new[usize::from(*s)];
                    }
                }
            }
        }
        self.f.junctions = order.iter().map(|&o| self.f.junctions[o]).collect();
        self.keys = order.iter().map(|&o| self.keys[o]).collect();
    }

    /// 0x5DE9A0: on the listed race records (+0x0C 0), a point's junction
    /// slots are swapped so that its first race link (low or high type 2,
    /// or low type 3) with direction bit 0 is in the last valid slot and
    /// the first with bit 1 in the one before (with one direction only,
    /// that one goes last).
    fn order_slots(&mut self) {
        for &r in &self.listed.clone() {
            if self.f.records[r].kind != 0 {
                continue;
            }
            for p in 0..self.f.records[r].points.len() {
                let valid = self.slots((r, p));
                let n = valid.len();
                if n == 0 {
                    continue;
                }
                let (mut back, mut fwd) = (None, None);
                for (i, &j) in valid.iter().enumerate() {
                    let fl = self.jflags(j);
                    if fl & 0xF == 2 || fl >> 4 & 0xF == 2 || fl & 0xF == 3 {
                        if !bit(fl, 18) {
                            back.get_or_insert(i);
                        } else {
                            fwd.get_or_insert(i);
                        }
                    }
                }
                let (first, second) = match (back, fwd) {
                    (None, None) => continue,
                    (None, f) => (f, None),
                    (b, f) => (b, f),
                };
                let slots = &mut self.f.records[r].points[p].junctions;
                if let Some(a) = first {
                    slots.swap(n - 1, a);
                }
                if let Some(b) = second {
                    slots.swap(n - 2, b);
                }
            }
        }
    }

    /// 0x5D9720: records lose bit 12; a listed record whose box (+0x60)
    /// overlaps a closed barrier's box (0x43D4C0, strict) gets it.
    fn mark_barred(&mut self) {
        for r in &mut self.f.records {
            r.flags &= !0x1000;
        }
        for b in self.barriers.iter().filter(|b| b.closed) {
            let (lo, hi) = (b.ends[0].min(b.ends[1]), b.ends[0].max(b.ends[1]));
            let mut ids: Vec<u16> = self.f.by_id.keys().copied().filter(|&id| id < 0x5DC).collect();
            ids.sort_unstable();
            for id in ids {
                let r = &mut self.f.records[self.f.by_id[&id]];
                if bit(r.flags, 6) && !bit(r.flags, 12) {
                    let [x0, y0, x1, y1] = r.bounds;
                    if x0 < hi.x && lo.x < x1 && y0 < hi.y && lo.y < y1 {
                        r.flags |= 0x1000;
                    }
                }
            }
        }
    }

    /// 0x5D2E70: the first closed barrier crossing the segment a–b.
    fn barrier(&self, a: Vec2, b: Vec2) -> Option<&RouteBarrier> {
        // Segments whose boxes are apart cannot cross: that cheap test
        // first (the same first barrier is found).
        let (lo, hi) = (a.min(b), a.max(b));
        let near = |x: &RouteBarrier| {
            let (blo, bhi) = (x.ends[0].min(x.ends[1]), x.ends[0].max(x.ends[1]));
            blo.x <= hi.x && bhi.x >= lo.x && blo.y <= hi.y && bhi.y >= lo.y
        };
        self.barriers.iter().find(|x| x.closed && near(x) && crosses(x.ends[0], x.ends[1], a, b))
    }

    /// 0x5DD560 with "any" set and no type-0 links: the point after `p`
    /// going `dir` (1 forward): through the first junction against `dir`
    /// whose type `k` is a link (flipping `dir` on bit 23) unless it leads
    /// back to `p`, else along the record.
    fn next(&self, p: Pt, dir: &mut bool, k: usize) -> Option<Pt> {
        let mut at = p;
        for j in self.slots(p) {
            let fl = self.jflags(j);
            if bit(fl, 18) != *dir && link(fl, k) {
                let t = self.target(j);
                if bit(fl, 23) {
                    *dir = !*dir;
                }
                if t != Some(p) {
                    return t;
                }
                at = t?;
                break;
            }
        }
        self.step(at, *dir)
    }

    /// 0x5E1CB0 (ECX `p`): whether the way from `p` going `dir` is barred
    /// for junction `j`'s type `k`: the step from `q` (if given) to `p`
    /// crosses a closed barrier, or, on a record with bit 12, a step
    /// within 200 m does before a junction off to another record against
    /// `dir`. For k = 1 barriers with +0x12 do not count.
    fn barred(&self, p: Option<Pt>, k: usize, mut dir: bool, j: usize, q: Option<Pt>) -> bool {
        let counts = |b: Option<&RouteBarrier>| b.is_some_and(|b| k != 1 || !b.held);
        let Some(mut p) = p else { return false };
        if let Some(q) = q
            && counts(self.barrier(self.xy(q), self.xy(p)))
        {
            return true;
        }
        let r = (self.f.records[p.0].points[p.1].flags & 0x7FF) as i16;
        if !bit(self.rflags(r), 12) {
            return false;
        }
        let mut dist = 0.0f32;
        // Zero-length or NaN cycles never reach 200 m. A walk can visit each
        // (point, direction) state once; more steps than that (and than a
        // floor that spares the per-call sum) is a cycle.
        let mut steps = 0usize;
        loop {
            steps += 1;
            if steps > 4096 && steps > 2 * self.f.records.iter().map(|r| r.points.len()).sum::<usize>() + 2 {
                return false;
            }
            for j2 in self.slots(p) {
                let fl = self.jflags(j2);
                if j2 == j || dead(fl, k) {
                    continue;
                }
                if bit(fl, 18) != dir && self.f.junctions[j2].target_record[1] != self.target_id(j2) {
                    return false;
                }
            }
            let next = self.next(p, &mut dir, k);
            if let Some(n) = next {
                if counts(self.barrier(self.xy(p), self.xy(n))) {
                    return true;
                }
                dist += (self.xy(p) - self.xy(n)).length();
            }
            let Some(n) = next else { return false };
            p = n;
            if dist >= 200.0 {
                return false;
            }
        }
    }

    /// 0x5D8770: the junction at `j`'s target that leads back to `j`'s
    /// own point in the opposite direction (`j` itself for a loop).
    fn reverse(&self, j: usize) -> Option<usize> {
        let own = self.own(j);
        let t = self.target(j)?;
        if Some(t) == own {
            return Some(j);
        }
        let fl = self.jflags(j);
        let d = bit(fl, 23) ^ bit(fl, 18);
        self.slots(t).into_iter().find(|&j2| bit(self.jflags(j2), 18) != d && self.target(j2) == own && own.is_some())
    }

    /// 0x5E14C0 (ECX `p`): of the point's first two junctions, those not
    /// dead for `k`, off to another record and with direction bit ≠ `d`;
    /// with two, the one whose 30 m point (0x5E1220) has the larger
    /// coordinate — every coordinate is 0 here (0x5E41B0), so the first.
    fn turn_pick(&self, p: Pt, d: bool, k: usize) -> Option<usize> {
        let s = self.slots(p);
        let ok = |j: usize| {
            let fl = self.jflags(j);
            !dead(fl, k) && self.f.junctions[j].target_record[1] != self.target_id(j) && bit(fl, 18) != d
        };
        let a = s.first().copied().filter(|&j| ok(j));
        let b = s.get(1).copied().filter(|&j| ok(j));
        a.or(b)
    }

    /// 0x5E1600 (ECX `j`): a dead end cut back from junction `j` (type
    /// `k`): walking its own record from its own point (forward with
    /// direction bit 18 set), junctions the same way into unlisted records
    /// die (7) until one into a listed record becomes the link (2; 0x5E14C0
    /// picks among turns, bit 26) — or is one already — and the walk ends;
    /// junctions the other way whose reverse is live and leads from a
    /// listed record die too, recursing on links (depth ≤ 100).
    fn cut(&mut self, j: usize, k: usize, depth: u32) {
        if depth > 100 {
            return;
        }
        let jr = self.rec(self.f.junctions[j].target_record[1]);
        let Some(jr) = jr else { return };
        if !bit(self.f.records[jr].flags, 6) {
            return;
        }
        let dirb = !bit(self.jflags(j), 18);
        let forward = bit(self.jflags(j), 18);
        let mut p = self.own(j);
        while let Some(pt) = p {
            let pf = self.f.records[pt.0].points[pt.1].flags;
            if !bit(pf, 29) && bit(self.rflags((pf & 0x7FF) as i16), 7) {
                return;
            }
            for j2 in self.slots(pt) {
                let fl = self.jflags(j2);
                if j2 == j || bit(fl, 18) != !dirb || dead(fl, k) || ktype(fl, k) == 10 {
                    continue;
                }
                let t = self.rec(self.target_id(j2));
                if t == Some(jr) {
                    continue;
                }
                if t.is_some_and(|t| bit(self.f.records[t].flags, 6)) {
                    if link(fl, k) {
                        return;
                    }
                    let x = if bit(fl, 26) { self.turn_pick(pt, dirb, k) } else { Some(j2) };
                    if let Some(x) = x {
                        self.set_type(x, k, 2);
                    }
                    return;
                }
                self.set_type(j2, k, 7);
            }
            for j2 in self.slots(pt) {
                let fl = self.jflags(j2);
                if j2 == j || bit(fl, 18) != dirb {
                    continue;
                }
                let Some(j3) = self.reverse(j2) else { continue };
                let f3 = self.jflags(j3);
                if dead(f3, k) || ktype(f3, k) == 10 || fl & 0xF == 3 {
                    continue;
                }
                if !bit(self.rflags(self.f.junctions[j3].target_record[1]), 6) {
                    continue;
                }
                let was_link = link(f3, k);
                self.set_type(j3, k, 7);
                if was_link {
                    self.cut(j3, k, depth + 1);
                }
            }
            p = self.step(pt, forward);
        }
    }

    /// 0x5E2140: one pass of barrier pruning over the live junctions; for
    /// each type k not dead: types 0/1/4/6/9 become 2 when barred from
    /// their own point but not from the target, 7 when barred from the
    /// target (cut back through 0x5E1600 when barred both ways); a link
    /// (2) barred from the target becomes 7 and is cut back. Returns
    /// whether anything changed.
    fn prune(&mut self) -> bool {
        let mut changed = false;
        for j in 0..self.live {
            for k in 0..3 {
                let fl = self.jflags(j);
                if dead(fl, k) {
                    continue;
                }
                let own = self.own(j);
                let tgt = self.target(j);
                let d1 = !bit(fl, 18);
                let d2 = bit(fl, 23) ^ d1;
                let (mut cut, mut to2, mut to7) = (false, false, false);
                match ktype(fl, k) {
                    0 | 1 | 4 | 6 | 9 => {
                        let r1 = self.barred(own, k, d1, j, None);
                        let r2 = self.barred(tgt, k, d2, j, own);
                        match (r1, r2) {
                            (true, false) => to2 = true,
                            (true, true) => (cut, to7) = (true, true),
                            (false, true) => to7 = true,
                            _ => {}
                        }
                    }
                    2 => {
                        if self.barred(tgt, k, d2, j, own) {
                            (cut, to7) = (true, true);
                        }
                    }
                    _ => {}
                }
                if to2 {
                    self.set_type(j, k, 2);
                }
                if to7 {
                    self.set_type(j, k, 7);
                    if cut {
                        self.cut(j, k, 1);
                    }
                }
                changed |= cut || to2 || to7;
            }
        }
        changed
    }

    /// 0x5DE5D0 over the live table: from each live junction type k's
    /// target (types 0–2, 4, 5, 8–10 going !bit 23 ^ bit 18; type 3 going
    /// bit 18 when its file type is 3, k = 1 or the record has bit 7),
    /// 0x5DE250 marks the way. (Its second half, on records with bit 9,
    /// changes nothing here: bit 9 is clear on every record.)
    fn mark_reachable(&mut self) {
        for k in 0..3 {
            for j in 0..self.live {
                let fl = self.jflags(j);
                if dead(fl, k) {
                    continue;
                }
                let dir = match ktype(fl, k) {
                    0 | 1 | 2 | 4 | 5 | 8 | 9 | 10 => !bit(fl, 23) ^ bit(fl, 18),
                    3 => {
                        if fl >> 8 & 0xF != 3 && k != 1 && !bit(self.rflags(self.f.junctions[j].target_record[1]), 7) {
                            continue;
                        }
                        bit(fl, 18)
                    }
                    _ => continue,
                };
                if let Some(t) = self.target(j) {
                    self.mark_from(t, dir, k);
                }
            }
        }
    }

    /// 0x5DE250: walk the record from `t` going `dir` (1 forward), marking
    /// the points passed (+0x0C: k 0 bit 18/11, 1 bit 26/25, 2 bit 28/27
    /// for dir ^ record bit 1 clear/set) and the record (+0x70 bit 8), in
    /// stretches ending at each point with a live junction; it stops at a
    /// point already marked, at the record's end (that last stretch is not
    /// marked) or when the last junction checked at a point is a link or
    /// type 3 against `dir`.
    fn mark_from(&mut self, t: Pt, dir: bool, k: usize) {
        let mark = bit(self.f.records[t.0].flags, 1) ^ dir;
        let mask = match (k, mark) {
            (0, false) => 1 << 18,
            (0, true) => 1 << 11,
            (1, false) => 1 << 26,
            (1, true) => 1 << 25,
            (_, false) => 1 << 28,
            (_, true) => 1 << 27,
        };
        let (mut start, mut p) = (t, Some(t));
        let mut pending = false;
        loop {
            let mut done = false;
            p = p.and_then(|q| self.step(q, dir));
            match p {
                None => done = true,
                Some(q) if self.f.records[q.0].points[q.1].flags & mask != 0 => {
                    done = true;
                    pending = true;
                }
                Some(q) => {
                    for j in self.slots(q) {
                        let fl = self.jflags(j);
                        if dead(fl, k) {
                            continue;
                        }
                        let is3 = fl & 0xF == 3;
                        done = bit(fl, 18) != dir && (is3 || link(fl, k));
                        let keep = fl >> 8 & 0xF == 3 || k == 1 || bit(self.rflags(self.f.junctions[j].target_record[1]), 7);
                        pending = if is3 { keep } else { true };
                    }
                }
            }
            if pending {
                pending = false;
                let q = p.expect("a stretch ends at a point");
                let r = start.0;
                self.f.records[r].flags |= 0x100;
                let (a, b) = (start.1.min(q.1), start.1.max(q.1));
                for pt in &mut self.f.records[r].points[a..=b] {
                    pt.flags |= mask;
                }
                start = q;
            }
            if done {
                return;
            }
        }
    }

    /// 0x5D8870: junctions whose own or target record is unlisted leave
    /// (key 10 · lap + index). Returns how many.
    fn drop_unlisted(&mut self) -> usize {
        let mut removed = 0;
        for j in 0..self.live {
            let own = self.rflags(self.f.junctions[j].target_record[1]);
            if !bit(own, 6) || !bit(self.rflags(self.target_id(j)), 6) {
                self.keys[j] = FREE_ROAM_LAP * 10.0 + j as f32;
                removed += 1;
            }
        }
        removed
    }
}

fn point(e: &Reader<'_>) -> FormatResult<ControlPoint> {
    Ok(ControlPoint {
        position: Vec3::new(e.f32_at(0)?, e.f32_at(4)?, f32::from(e.i16_at(0x12)?) / 8.0),
        coord: e.f32_at(8)?,
        flags: e.u32_at(0x0C)?,
        length: f32::from(e.u16_at(0x10)?) / 256.0,
        half_width: [f32::from(e.u16_at(0x14)?) / 256.0, f32::from(e.u16_at(0x16)?) / 256.0],
        tangent: Vec2::new(f32::from(e.i16_at(0x18)?), f32::from(e.i16_at(0x1A)?)) / 32767.0,
        junctions: [e.u16_at(0x1C)?, e.u16_at(0x1E)?],
        bank: f32::from(e.u8_at(0x20)? as i8) / 128.0,
        grade: f32::from(e.u8_at(0x21)? as i8) / 128.0,
        lane_position: e.array_at::<10>(0x22)?.map(|v| v as i8),
        lane_speed: e.array_at::<10>(0x2C)?,
    })
}

/// The control points of a route file in a grid of square cells, for the
/// point nearest a place (3D, as a scan over every point finds it: the
/// first of equal distances in record and point order).
pub struct PointGrid {
    cell: f32,
    origin: (i32, i32),
    size: (i32, i32),
    /// Per cell (row-major), the points in it: record, point, position.
    cells: Vec<Vec<(u32, u32, Vec3)>>,
    records: usize,
    /// Some point is not finite: every query scans.
    scan: bool,
}

impl PointGrid {
    const CELL: f32 = 64.0;
    /// Rings searched before a plain scan is cheaper.
    const MAX_RING: i32 = 12;

    pub fn new(f: &RouteFile) -> PointGrid {
        let cell = Self::CELL;
        let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32);
        let points = || f.records.iter().enumerate().flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri as u32, ci as u32, c.position)));
        let scan = points().any(|p| !p.2.is_finite());
        let (mut lo, mut hi) = ((i32::MAX, i32::MAX), (i32::MIN, i32::MIN));
        for p in points().filter(|_| !scan) {
            let k = key(p.2);
            lo = (lo.0.min(k.0), lo.1.min(k.1));
            hi = (hi.0.max(k.0), hi.1.max(k.1));
        }
        // The bounding box of the points sizes the grid; one
        // stray point must not (no grid past 2^22 cells: every query scans).
        let span = |lo: i32, hi: i32| i64::from(hi) - i64::from(lo) + 1;
        let (scan, size) = if lo.0 > hi.0 || scan {
            (scan, (0, 0))
        } else if span(lo.0, hi.0) * span(lo.1, hi.1) > 1 << 22 {
            (true, (0, 0))
        } else {
            (scan, (span(lo.0, hi.0) as i32, span(lo.1, hi.1) as i32))
        };
        let mut cells = vec![Vec::new(); (size.0 * size.1) as usize];
        if size.0 > 0 {
            for p in points() {
                let k = key(p.2);
                cells[((k.1 - lo.1) * size.0 + (k.0 - lo.0)) as usize].push(p);
            }
        }
        PointGrid { cell, origin: lo, size, cells, records: f.records.len(), scan }
    }

    /// The point of `f` (the file the grid was built from) nearest `at`.
    pub fn nearest(&self, f: &RouteFile, at: Vec3) -> Option<(usize, usize)> {
        let scan = || {
            f.records
                .iter()
                .enumerate()
                .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position)))
                .min_by(|a, b| a.2.distance_squared(at).total_cmp(&b.2.distance_squared(at)))
                .map(|(r, c, _)| (r, c))
        };
        if self.scan || self.size.0 == 0 || !at.is_finite() || f.records.len() != self.records {
            return scan();
        }
        let (cx, cy) = ((at.x / self.cell).floor() as i32, (at.y / self.cell).floor() as i32);
        let mut best: Option<(f32, u32, u32)> = None;
        // Ring k: the cells k from the place's cell; every point beyond it
        // is at least k cells' width away.
        for k in 0..=Self::MAX_RING {
            for y in cy - k..=cy + k {
                for x in cx - k..=cx + k {
                    if (y - cy).abs() != k && (x - cx).abs() != k {
                        continue;
                    }
                    let (gx, gy) = (x - self.origin.0, y - self.origin.1);
                    if gx < 0 || gy < 0 || gx >= self.size.0 || gy >= self.size.1 {
                        continue;
                    }
                    for &(r, c, p) in &self.cells[(gy * self.size.0 + gx) as usize] {
                        let d = p.distance_squared(at);
                        if best.is_none_or(|b| d.total_cmp(&b.0).then((r, c).cmp(&(b.1, b.2))).is_lt()) {
                            best = Some((d, r, c));
                        }
                    }
                }
            }
            // All cells searched, or nothing past this ring can be nearer
            // (a metre's margin for rounding).
            let all = cx - k <= self.origin.0 && cy - k <= self.origin.1 && cx + k >= self.origin.0 + self.size.0 - 1 && cy + k >= self.origin.1 + self.size.1 - 1;
            let reach = (k as f32 * self.cell - 1.0).max(0.0);
            if all || best.is_some_and(|b| reach * reach > b.0) {
                return best.map(|b| (b.1 as usize, b.2 as usize));
            }
        }
        scan()
    }
}

#[cfg(test)]
mod tests {
    /// Two coincident control points whose junctions point at each other
    /// are a zero-length cycle: `barred`'s 200 m test never fires, so it
    /// used to loop forever. It stops and reports "not barred".
    #[test]
    fn barred_terminates_on_a_zero_length_cycle() {
        let point = |junction: u16| ControlPoint {
            position: Vec3::ZERO,
            coord: 0.0,
            flags: 1,
            length: 0.0,
            half_width: [0.0; 2],
            tangent: Vec2::ZERO,
            junctions: [junction, 0xFFFF],
            bank: 0.0,
            grade: 0.0,
            lane_position: [0; 10],
            lane_speed: [0; 10],
        };
        let junction = |to: i16| Junction { target_point: [to, to], target_record: [1, 1], flags: 0x4_0002 };
        let record = RouteRecord {
            id: 1,
            name: String::new(),
            kind: 0,
            coord_range: (0.0, 0.0),
            bounds: [0.0; 4],
            flags: 1 << 12,
            fade: [false; 2],
            lanes: [false; 10],
            points: vec![point(0), point(1)],
        };
        let mut f = RouteFile { records: vec![record], junctions: vec![junction(1), junction(0)], ..Default::default() };
        f.by_id.insert(1, 0);
        let s = PostLoad { f: &mut f, keys: vec![0.0; 2], live: 2, barriers: &[], listed: Vec::new() };
        assert!(!s.barred(Some((0, 0)), 0, false, 99, None));
    }

    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn reads_routes_4001_forward() {
        let path = crate::testkit::game_root().join("TRACKS/ROUTESL4RA/Routes4001F.bin");
        let data = std::fs::read(path).unwrap();
        let f = RouteFile::parse(&data);
        assert_eq!(f.records.len(), 136);
        assert_eq!(f.junctions.len(), 455);
        let r = &f.records[f.by_id[&401]];
        assert_eq!(r.name, "TrackRoutesA30");
        // The first segment's stored length matches the points' distance.
        let d = r.points[0].position.truncate().distance(r.points[1].position.truncate());
        assert!((r.points[0].length - d).abs() < 0.1, "{} vs {d}", r.points[0].length);
        assert!(f.records.iter().all(RouteRecord::is_race));
    }
}

#[cfg(test)]
mod post_load_tests {
    use super::*;
    use crate::formats::bundle::read_u32 as u32_at;
    use crate::formats::bundle::{read_f32, read_i16, read_u16};

    /// The free-roam post-load against the original's memory after the
    /// intro (career stage 1, L4RA), read under Wine: the private set's
    /// the maintainer's private captures holds junctions_mem.bin (the
    /// table, 32 bytes each) and records_mem.bin (u16 id, u32 length, the
    /// record's bytes).
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn free_roam_matches_the_original() {
        let jm = crate::testkit::private("docs/evidence/free-roam-routes/junctions_mem.bin");
        let rm = crate::testkit::private("docs/evidence/free-roam-routes/records_mem.bin");
        let root = crate::testkit::game_root().join("TRACKS/ROUTESL4RA");
        let read = |p: std::path::PathBuf| std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let (routes, paths) = (read(root.join("RoutesFreeRoam.bin")), read(root.join("PathsFreeRoam.bin")));
        let mut f = RouteFile::parse(&routes);
        let mut net = crate::formats::roads::RoadNet::parse(&paths);
        net.close_barriers(&["BARRIERS_4000", "PLAYER_BARRIERS_4000", "BARRIERS_CAREER1"]);
        let barriers = RouteBarrier::from_net(&net);
        assert!(barriers.iter().any(|b| b.closed));
        let s = PostLoad::run(&mut f, &barriers);
        let live = s.live;
        let keys = s.keys.clone();
        assert_eq!(f.junctions.len() * 32, jm.len());
        let mut bad = 0;
        for (k, (j, m)) in f.junctions.iter().zip(jm.chunks_exact(32)).enumerate() {
            let mine = (keys[k], j.target_point, j.target_record, j.flags);
            let theirs = (read_f32(m, 0), [read_i16(m, 4), read_i16(m, 6)], [read_i16(m, 8), read_i16(m, 0xA)], u32_at(m, 0xC));
            if mine != theirs {
                if bad < 10 {
                    eprintln!("junction {k:#x}: {mine:x?} vs {theirs:x?}");
                }
                bad += 1;
            }
        }
        let mut at = 0;
        let (mut bad_rec, mut bad_pt) = (0, 0);
        while at + 6 <= rm.len() {
            let id = read_u16(&rm, at);
            let len = u32_at(&rm, at + 2) as usize;
            let m = &rm[at + 6..at + 6 + len];
            at += 6 + len;
            let r = &f.records[f.by_id[&id]];
            if r.flags != u32_at(m, 0x70) {
                if bad_rec < 10 {
                    eprintln!("record {id}: {:#x} vs {:#x}", r.flags, u32_at(m, 0x70));
                }
                bad_rec += 1;
            }
            for (k, p) in r.points.iter().enumerate() {
                let e = &m[0x8C + k * 0x38..];
                if p.junctions != [read_u16(e, 0x1C), read_u16(e, 0x1E)] || p.flags != u32_at(e, 0xC) || p.coord != read_f32(e, 8) {
                    if bad_pt < 10 {
                        eprintln!("record {id} point {k}: {:x?} {:#x} vs {:x?} {:#x}", p.junctions, p.flags, [read_u16(e, 0x1C), read_u16(e, 0x1E)], u32_at(e, 0xC));
                    }
                    bad_pt += 1;
                }
            }
        }
        assert_eq!((live, bad, bad_rec, bad_pt), (3245, 0, 0, 0));
    }
}

#[cfg(test)]
mod point_grid_tests {
    use super::*;

    /// The grid finds the point a scan over every point finds, near the
    /// roads, between them, off the map and on ties.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn grid_matches_scan() {
        let root = crate::testkit::game_root();
        let data = std::fs::read(root.join("TRACKS/ROUTESL4RA/RoutesFreeRoam.bin")).unwrap();
        let f = RouteFile::parse(&data);
        let grid = PointGrid::new(&f);
        assert!(!grid.scan && grid.size.0 > 0);
        let scan = |at: Vec3| {
            f.records
                .iter()
                .enumerate()
                .flat_map(|(ri, r)| r.points.iter().enumerate().map(move |(ci, c)| (ri, ci, c.position)))
                .min_by(|a, b| a.2.distance_squared(at).total_cmp(&b.2.distance_squared(at)))
                .map(|(r, c, _)| (r, c))
        };
        let mut x = 12345u32;
        let mut tries: Vec<Vec3> = f.records.iter().flat_map(|r| r.points.iter().map(|c| c.position)).step_by(7).collect();
        for _ in 0..3000 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let u = (x >> 8) as f32 / (1u32 << 24) as f32;
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let v = (x >> 8) as f32 / (1u32 << 24) as f32;
            tries.push(Vec3::new(-6000.0 + u * 12000.0, -6000.0 + v * 12000.0, 40.0 * u));
        }
        assert!(tries.len() > 3000);
        for at in tries {
            assert_eq!(grid.nearest(&f, at), scan(at), "{at:?}");
        }
    }
}
