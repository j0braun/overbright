use super::*;

/// The placer's walk starts this far along the road (0x7E9720) …
pub const SPAWN_START: f32 = 250.0;

/// … and gives up past this straight-line distance (0x42E9F0).
pub const SPAWN_MAX: f32 = 500.0;

/// Control points the placer tries before giving up (0x42E9F0, > 99).
pub(super) const SPAWN_TRIES: usize = 100;

/// Seconds between placement passes (0x4098D0: 0x783BA0; 0.3 s,
/// 0x783BA4, in a drag race).
pub const PLACE_INTERVAL: f32 = 1.0;

pub const PLACE_INTERVAL_DRAG: f32 = 0.3;

/// Cars a pass places: 1, or 16 in a drag race (0x4018F0 on the race
/// parameters: +0x38 = 0x89E7D8, or its online twin 0x890108).
pub const PLACE_DRAG: usize = 16;

/// At or above this speed (m/s, 55 mph) the player gets traffic ahead —
/// unless the race counts him as going the wrong way (player object
/// +0x248: then the walk byte is clear) — below it ahead or behind
/// with even odds (0x42E9F0).
pub const AHEAD_SPEED: f32 = 24.5865;

/// The traffic driver's throttle cap (CarDriver +0x22C, set in its ctor
/// 0x425AB0 at 0x425B44: 0x783A28 by difficulty only for a racer's
/// controller, class 2; 1.0, 0x784250, for the rest).
pub const TRAFFIC_THROTTLE_CAP: f32 = 1.0;

/// The last placement's outcome (probe use, `TRAFH_PLACER`).
#[cfg(feature = "dev-harness")]
pub mod place_probe {
    use std::cell::RefCell;
    thread_local! {
        pub(super) static WHY: RefCell<String> = const { RefCell::new(String::new()) };
        pub(super) static STARTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    }
    /// The last placement's start (cursor, number of starts).
    pub fn starts() -> (usize, usize) {
        STARTS.with(|x| x.get())
    }
    /// The last placement's outcome, taken.
    pub fn take() -> String {
        WHY.with(|w| std::mem::take(&mut *w.borrow_mut()))
    }
}

#[cfg(feature = "dev-harness")]
fn why(w: &str) {
    place_probe::WHY.with(|x| {
        let mut x = x.borrow_mut();
        x.clear();
        x.push_str(w);
    });
}

#[cfg(not(feature = "dev-harness"))]
fn why(_: &str) {}

/// The drag placement's search reach (0x7E9720, 250 m; 0x42E620 hands it
/// to 0x417C60).
pub const DRAG_REACH: f32 = 250.0;

/// 0x5D8CA0 (verified, the disassembly): a record's control point nearest
/// beyond `reach` of the racer and ahead of it. Per point the measure is
/// max(the 2D distance to `pos`, |Δ race coordinate| − 50 (0x785B80))
/// (0x5D8A90) less `reach`; with no position (a null `pos`, as the race
/// set-up's fill 0x42E4A0 passes) it is the point's race coordinate less
/// `coord` (lap-wrapped, 0x88F2B0 / 0x88F2B4) less `reach`. A point counts
/// when that is ≥ 0, the least so far, and its race coordinate (+0x08) is
/// ahead of `coord` (0x5D5CE0 > 0; the drag roads do not loop, so no lap
/// wrap). After each point the scan skips −2 − trunc(measure × −0.02)
/// (0x7852AC; `_ftol`) points more when that is positive.
pub(super) fn reach_point(
    r: &crate::formats::ai_routes::RouteRecord,
    pos: Option<Vec2>,
    coord: f32,
    reach: f32,
) -> Option<usize> {
    let mut best = (999_999.0f32, None);
    let mut i = 0usize;
    while i < r.points.len() {
        let c = &r.points[i];
        let ahead = c.coord - coord;
        let g = match pos {
            Some(pos) => c.position.truncate().distance(pos).max(ahead.abs() - 50.0),
            None => ahead,
        } - reach;
        if g >= 0.0 && g < best.0 && ahead > 0.0 {
            best = (g, Some(i));
        }
        let skip = -2 - (g * -0.02) as i32;
        if skip > 0 {
            i += skip as usize;
        }
        i += 1;
    }
    best.1
}

/// 0x5DCC90 (verified, the disassembly): whether the placement gate
/// 0x418110 measures a control point by race coordinate (0x409890,
/// 0x411060) rather than in 2D (0x409840, 0x410FC0): the race road is
/// built (0x88F2BE; always in the clone's races), the point's record has
/// +0x70 bit 9 clear and the point has flag bit 29.
pub fn coordinate_point(f: &RouteFile, record: usize, cp: usize) -> bool {
    f.records[record].flags & 0x200 == 0 && f.records[record].points[cp].flags >> 29 & 1 != 0
}

/// 0x417C60 (verified): over the race records in id order (the route
/// manager's list 0x88D21C), each with the flag flipped where its +0x70 has
/// bit 1 (in a coordinate race) and a lane on that side (0x5D4490: bytes
/// +0x84..+0x87 for a set flag, +0x88..+0x8B clear), the [`reach_point`]
/// that carries flag bit 25 (flag set) or 26 (clear, the flag as drawn),
/// is not the record's last (set) or first (clear) point (0x409520) and
/// whose race coordinate is the least ahead of the racer's.
pub fn drag_point(f: &RouteFile, pos: Option<Vec2>, coord: f32, reach: f32, flag: bool) -> Option<(usize, usize)> {
    let mut races: Vec<usize> = f.records.iter().enumerate().filter(|(_, r)| r.is_race()).map(|(i, _)| i).collect();
    races.sort_by_key(|&i| f.records[i].id);
    let mut best = (999_999.0f32, None);
    for ri in races {
        let r = &f.records[ri];
        let side = flag ^ (r.flags & 2 != 0);
        let group = if side { 2..6 } else { 6..10 };
        if !r.lanes[group].iter().any(|&l| l) {
            continue;
        }
        let Some(k) = reach_point(r, pos, coord, reach) else { continue };
        let c = &r.points[k];
        let bit = if flag { 25 } else { 26 };
        let inner = if side { k + 1 < r.points.len() } else { k > 0 };
        let gap = c.coord - coord;
        if c.flags >> bit & 1 != 0 && inner && gap > 0.0 && gap < best.0 {
            best = (gap, Some((ri, k)));
        }
    }
    best.1
}

/// The walk generator a new traffic lane takes (the clone's own: the
/// original's route extension draws its junction odds on the world
/// generator, 0x413C8E, which the clone's lanes do not share yet — open,
/// docs/original/rng.md). Derived from the world generator's state without a draw:
/// a placement makes no `Rand_Int(0x7FFFFFFF)` (**measured**, the sequence
/// log: the fill's and the placer's world draws end with the lane picks).
pub(super) fn lane_stream(rng: &GameRng) -> GameRng {
    GameRng(rng.0 ^ 0x5A5A_A5A5)
}

/// 0x410CA0 in a drag race (0x89E7D8 / 0x890108; verified): with odds ½
/// (0x5EA510) the lane of a random human racer (0x5EA4F0 over 0x8900BC ×
/// 100000; its controller's lane, vt+0x1C, 2–9, taken on this side) when
/// the record has it; else [`pick_lane`]. With no human racer listed (the race set-up's fill,
/// `humans` false) neither draw is made.
pub(super) fn drag_lane(
    f: &RouteFile,
    s: Step,
    player_lane: Option<usize>,
    humans: bool,
    rng: &mut GameRng,
) -> Option<usize> {
    if humans && rng.unit(1.0) < 0.5 {
        rng.below(100_000);
        if let Some(l) = player_lane.filter(|l| (2..=5).contains(l) && has_lane(f, s, *l)) {
            return Some(l);
        }
    }
    pick_lane(f, s, rng)
}

/// The next control point walking a direction (0x41EC20 with the traffic
/// goal "Be traffic car", vtable 0x7848A0; "Be ambient traffic car",
/// 0x784900, has the same slots here). At a point's junctions
/// of the direction (bit 18 = travelled backward), in order: a type 3 (low
/// nibble) ends the walk; the traffic type is the high nibble (bits 4–7):
/// 2 is always taken (slot +0x2C, 0x40CB70); 4 is taken with odds ½ (slot
/// +0x28, 0x413BB0; 0x5EA510 on the traffic manager's random numbers), or
/// never when it has bit 26 unless the target record has the car's lane in
/// the direction it arrives and that lane is the highest (5 down to 2) the
/// current record has (0x40B0B0); 5, 7, 10 and the rest never. Taking one
/// flips the direction by bit 23; else the record's next point. None at a
/// dead end. The lane then follows 0x415B70 (`keep_lane`).
/// The most starts 0x417D80 gathers for the placer (0x42E9F0 asks for 3).
pub(super) const PLACER_STARTS: usize = 3;

/// 0x417D80: the placer's walk starts. The first is the player's own road
/// point (`own`); then, record by record (the route manager's list
/// 0x88D21C, count 0x88D218; the clone takes the file's order) until
/// `max` are found, a record other than `own`'s whose box (+0x60 / +0x68)
/// holds the point within max(left, right half width) + 30 m
/// (0x43D410) gives its first point that
/// * runs parallel: |cos| between the two tangents over 0.85 (0x783BE8);
/// * carries flag bit 25 when the start goes backward, else bit 26 — the
///   start's direction is `own`'s, flipped where the tangents oppose;
/// * lies in the point's frame (along the tangent, right of it) within
///   ±max(the segment +0x10, 20 m) along and 2–30 m (0x783BE0 / BE4)
///   beyond the left (+0x14) or right (+0x16) half width across — a road
///   running beside the player's.
///
/// A point that fails moves the scan on max(1, ⌊0.02 × its straight
/// distance to the player's point⌋) points (0x7851C0, `_ftol`). One match
/// per record.
pub(super) fn starts(f: &RouteFile, own: Step, max: usize) -> Vec<Step> {
    let mut out = vec![own];
    let me = &f.records[own.record].points[own.cp];
    let p = me.position.truncate();
    let (fx, fy) = (me.tangent.x, me.tangent.y);
    let reach = me.length.max(20.0);
    let [left, right] = me.half_width;
    let margin = left.max(right) + 30.0;
    for (ri, r) in f.records.iter().enumerate() {
        if out.len() >= max {
            break;
        }
        let [x0, y0, x1, y1] = r.bounds;
        if ri == own.record || p.x < x0 - margin || p.x > x1 + margin || p.y < y0 - margin || p.y > y1 + margin {
            continue;
        }
        let mut k = 0;
        while k < r.points.len() {
            let c = &r.points[k];
            let cos = c.tangent.x * fx + c.tangent.y * fy;
            let backward = own.backward ^ (cos < 0.0);
            if cos.abs() > 0.85 && c.flags >> if backward { 25 } else { 26 } & 1 != 0 {
                let d = c.position.truncate() - p;
                let (along, across) = (d.y * fy + d.x * fx, d.x * fy - d.y * fx);
                let inside = |lo: f32, hi: f32| (-reach..=reach).contains(&along) && (lo..=hi).contains(&across);
                if inside(-left - 30.0, -left - 2.0) || inside(right + 2.0, right + 30.0) {
                    out.push(Step { record: ri, cp: k, backward, lane: 0 });
                    break;
                }
            }
            k += ((c.position.truncate().distance(p) * 0.02) as usize).max(1);
        }
    }
    out
}

/// The placer's road search (0x415130 with one result wanted and its
/// sixth argument 0, as 0x42E9F0 calls it; **verified** from the
/// decompile and the asm 0x4154CA–0x415582): from a control point walked
/// forward (the walk byte set: 0x401CC0 then steps to the next index) or
/// backward, the first point where `metres` run out, the metres counted
/// by each point's own segment (+0x10; a backward walk starts with its
/// first point's added back). It is a search, not one car's drive: at a
/// point's junctions of the walk's direction (bit 18 ≠ the walk byte) a
/// low nibble 3 ends the walk there; on a point with flag bit 25 a high
/// nibble 7 is passed by and a 2 is followed (the target point, the
/// direction flipped by bit 23, the metres corrected by the target's offset
/// along the point's tangent, ×1/32767 at 0x81E7FC); any other junction
/// (every one on a point without bit 25) is a branch: the next point along
/// goes to the front of a 20-entry ring (0x40C720, dropped when full) and
/// the junction's target to the front or, with odds ½ (0x5EA510; in a
/// coordinate race when its low nibble is not 0), the back (0x40C6B0), and
/// the walk goes on from the ring's front (0x4132E0). A point stepped onto
/// must carry, for the walk's starting direction, one of bits 11, 25, 27
/// (forward, 0x5D8C00) or 18, 26, 28 (backward, 0x5D8C50), else that path
/// ends there and the ring's next is taken. None when the ring runs dry,
/// after more pops than the route's junctions (0x883DB4), or after the
/// records × 1024 steps (0x88D218 << 10).
///
/// `racing` is the walk's sixth argument (1 from the encounter placer
/// 0x42A5E0, at 0x42A69A and 0x42A73C; **verified**, the decompile and
/// the asm): the pass-by / follow test then takes the point's flag bit 11
/// (+0x0C & 0x800) for bit 25 and the junction's low nibble for its high
/// one (7 passed by, 2 followed); everything else is the same.
pub(super) fn placer_walk(
    f: &RouteFile,
    start: Step,
    metres: f32,
    coordinate: bool,
    racing: bool,
    rng: &mut GameRng,
) -> Option<Step> {
    const RING: usize = 20;
    // {point, metres left, walk byte} (12-byte entries at ring +0, head
    // +0xF0, tail +0xF4, full +0xF8).
    let mut ring = [(Step { record: 0, cp: 0, backward: false, lane: 0 }, 0.0f32); RING];
    let (mut head, mut tail, mut full) = (0usize, 0usize, false);
    let start_forward = !start.backward;
    let pt = |s: Step| &f.records[s.record].points[s.cp];
    let next = |s: Step| -> Option<Step> {
        let n = f.records[s.record].points.len();
        let cp = if s.backward { s.cp.checked_sub(1)? } else { Some(s.cp + 1).filter(|&c| c < n)? };
        Some(Step { cp, ..s })
    };
    let direction_ok = |s: Step| {
        let flags = pt(s).flags;
        let bits: [u32; 3] = if start_forward { [11, 25, 27] } else { [18, 26, 28] };
        bits.iter().any(|&b| flags >> b & 1 != 0)
    };
    let mut cur = Some(start);
    let mut left = metres + if start.backward { pt(start).length } else { 0.0 };
    let mut pops = 0usize;
    for _ in 0..f.records.len() * 1024 {
        let Some(mut s) = cur else { return None };
        let c = pt(s);
        left -= c.length;
        if left < 0.0 {
            return Some(Step { lane: 0, ..s });
        }
        let (mut along, mut branched, mut ended) = (true, false, false);
        let shift =
            |t: Step| (pt(t).position.x - c.position.x) * c.tangent.x + (pt(t).position.y - c.position.y) * c.tangent.y;
        for j in f.junctions_at(c).map(|j| f.junctions[j]) {
            if j.direction() != s.backward {
                continue;
            }
            along = false;
            if j.kind() == 3 {
                left = -1.0;
                ended = true;
                break;
            }
            let nibble = if racing { j.flags & 0xF } else { j.flags >> 4 & 0xF };
            let gate = if racing { 11 } else { 25 };
            let target = {
                let (rid, tcp) = j.target();
                f.record(rid).and_then(|r| {
                    usize::try_from(tcp).ok().filter(|&p| p < f.records[r].points.len()).map(|p| Step {
                        record: r,
                        cp: p,
                        backward: s.backward ^ j.flips(),
                        lane: 0,
                    })
                })
            };
            if c.flags >> gate & 1 != 0 {
                if nibble == 7 {
                    along = true;
                    continue;
                }
                if nibble == 2 {
                    if let Some(t) = target {
                        left = c.length + left - shift(t);
                        s = t;
                    }
                    ended = true;
                    break;
                }
            }
            // A branch: the next point along, then the junction's target.
            let mut push_front = |e: (Step, f32)| {
                if !full {
                    head = if head == 0 { RING - 1 } else { head - 1 };
                    full = tail == head;
                    ring[head] = e;
                }
            };
            if let Some(n) = next(s) {
                push_front((n, left));
            }
            if let Some(t) = target {
                let e = (t, c.length + left - shift(t));
                let back = if coordinate { j.kind() != 0 } else { rng.unit(1.0) >= 0.5 };
                if !back {
                    push_front(e);
                } else if !full {
                    ring[tail] = e;
                    tail = (tail + 1) % RING;
                    full = tail == head;
                }
            }
            branched = true;
        }
        let _ = ended;
        cur = if branched {
            None
        } else if along {
            next(s)
        } else {
            Some(s)
        };
        cur = cur.filter(|&t| direction_ok(t));
        if cur.is_none() {
            if !full && head == tail {
                return None;
            }
            let (e, l) = ring[head];
            head = (head + 1) % RING;
            full = false;
            left = l;
            cur = Some(e);
            pops += 1;
            if pops > f.junctions.len() {
                return None;
            }
        }
    }
    None
}

/// The player's road node as the placer reads it (see
/// [`TrafficLane::spawn_from`]): the first point of the nearest segment
// prov: inferred ai.traffic.road_node
/// (point k to k + 1) of any record, in 2D (**inferred**).
pub fn road_node(f: &RouteFile, p: Vec2) -> Option<(usize, usize)> {
    let mut best: Option<(f32, usize, usize)> = None;
    for (ri, r) in f.records.iter().enumerate() {
        for k in 0..r.points.len().saturating_sub(1) {
            let (a, b) = (r.points[k].position.truncate(), r.points[k + 1].position.truncate());
            let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = p.distance(a + (b - a) * t);
            if best.is_none_or(|x| d < x.0) {
                best = Some((d, ri, k));
            }
        }
    }
    best.map(|x| (x.1, x.2))
}

/// [`placer_walk`] from a control point (free roam: no coordinate race),
/// for probes: the found point's record, index and backward flag.
pub fn placer_search(
    f: &RouteFile,
    record: usize,
    cp: usize,
    backward: bool,
    metres: f32,
    rng: &mut GameRng,
) -> Option<(usize, usize, bool)> {
    placer_walk(f, Step { record, cp, backward, lane: 0 }, metres, false, false, rng)
        .map(|s| (s.record, s.cp, s.backward))
}

/// The encounter placer's road search (0x415130 with count 1 and its
/// sixth argument 1, 0x42A6A8 / 0x42A748; see [`placer_walk`]): the found
/// point's record, index and backward flag. `coordinate` is 0x88F2BE (the
/// coordinate race), clear in free roam.
pub fn racer_search(
    f: &RouteFile,
    record: usize,
    cp: usize,
    backward: bool,
    metres: f32,
    coordinate: bool,
    rng: &mut GameRng,
) -> Option<(usize, usize, bool)> {
    placer_walk(f, Step { record, cp, backward, lane: 0 }, metres, coordinate, true, rng)
        .map(|s| (s.record, s.cp, s.backward))
}

/// A traffic zone: a route trigger of kind 0x10 (chunk 0x3414A of the
/// region's Paths file, the record layout of `career::RouteTriggers`;
/// L4RA has four). 0x5D9C00 finds the one holding a point: in its box,
/// then in its polygon (even-odd). +0x2C + 4·level is the spacing row of
/// the AI tuning (levels 1–3: +0x30, +0x34, +0x38; L4RA's read 0–2, 3–5,
/// 6–8), +0x3C the zone of the car types' traffic caps (0x412950).
#[derive(Clone, Debug)]
pub struct TrafficZone {
    pub(super) min: Vec2,
    pub(super) max: Vec2,
    pub(super) polygon: Vec<Vec2>,
    pub(super) rows: [u32; 3],
    pub caps: usize,
}

impl TrafficZone {
    pub fn parse(paths: &[u8]) -> Vec<TrafficZone> {
        use crate::formats::bundle::{read_f32, read_u16, read_u32};
        let key = 0x3414Au32.to_le_bytes();
        let Some(at) = paths.windows(4).position(|w| w == key) else { return Vec::new() };
        if at + 8 > paths.len() {
            return Vec::new();
        }
        let end = (at + 8 + read_u32(paths, at + 4) as usize).min(paths.len());
        let mut out = Vec::new();
        let mut p = at + 8;
        while p + 0x44 <= end {
            let (kind, n, size) =
                (read_u32(paths, p), read_u16(paths, p + 0x40) as usize, read_u16(paths, p + 0x42) as usize);
            if size < 0x44 || p + size > end {
                break;
            }
            if kind == 0x10 && 0x44 + n * 8 <= size {
                out.push(TrafficZone {
                    min: Vec2::new(read_f32(paths, p + 0x20), read_f32(paths, p + 0x24)),
                    max: Vec2::new(read_f32(paths, p + 0x28), read_f32(paths, p + 0x2C)),
                    polygon: (0..n)
                        .map(|k| Vec2::new(read_f32(paths, p + 0x44 + k * 8), read_f32(paths, p + 0x48 + k * 8)))
                        .collect(),
                    rows: [read_u32(paths, p + 0x30), read_u32(paths, p + 0x34), read_u32(paths, p + 0x38)],
                    caps: read_u32(paths, p + 0x3C) as usize,
                });
            }
            p += size;
        }
        out
    }

    /// 0x5D9C00 with kind 0x10: the first zone holding a point.
    pub fn at(zones: &[TrafficZone], p: Vec2) -> Option<&TrafficZone> {
        zones.iter().find(|z| {
            p.cmpge(z.min).all() && p.cmple(z.max).all() && {
                let mut inside = false;
                let n = z.polygon.len();
                for i in 0..n {
                    let (a, b) = (z.polygon[i], z.polygon[(i + n - 1) % n]);
                    if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                        inside = !inside;
                    }
                }
                inside
            }
        })
    }

    /// The spacing row at a traffic level (1–3).
    pub fn row(&self, level: usize) -> Option<usize> {
        level.checked_sub(1).and_then(|l| self.rows.get(l)).map(|&r| r as usize)
    }
}

/// 0x418240: how far apart the placer keeps traffic around a player: the
/// player's traffic zone's row of the AI tuning's spacing table (min, max)
/// at the level, min + rand(max − min) (0x5EA510 on the traffic manager's
/// numbers). None — and the placement fails (0x42F800 refuses 99999) —
/// outside every zone, or when 0x4112A0 finds a jam: the player under
/// 30 m/s (0x783BF4) with a placed car under 1 m/s (0x783BF0) within 250 m
/// (0x7E9720; of race coordinate, the clone: 2D, inferred).
pub fn spacing(
    zones: &[TrafficZone],
    table: &[(f32, f32)],
    level: usize,
    player: Vec2,
    jammed: bool,
    rng: &mut GameRng,
) -> Option<f32> {
    let (lo, hi) = TrafficZone::at(zones, player).and_then(|z| z.row(level)).and_then(|r| table.get(r)).copied()?;
    if jammed {
        return None;
    }
    Some(rng.unit(hi - lo) + lo)
}

/// The jam test's speeds and reach (0x4112A0).
pub const JAM_PLAYER: f32 = 30.0;

pub const JAM_CAR: f32 = 1.0;

pub const JAM_REACH: f32 = 250.0;

/// 0x42F800's draws for each car the pass tries (**verified**, the
/// decompile and the RNG port list's draw sites, docs/native-fixes/N-RNG.tsv): which human racer the car is placed for
/// (0x42F812, 0x5EA4F0 on the human count 0x8900BC) and the teleporter
/// odds (0x42F852, 0x5EA510(1.0), compared with 1.0 or 0.35 under 20 m/s),
/// both before the spacing is looked at; 0x42E790 then returns at once
/// without a draw (no teleporters in the retail routes, 0x883DC8 = 0).
/// The spacing itself (0x418240, [`spacing`]) is drawn once a pass for each
/// human racer before the slots are walked (0x42F910), whether or not a car
/// is then tried.
pub fn placer_try_draws(rng: &mut GameRng, humans: u32) {
    rng.below(humans);
    rng.unit(1.0);
}

/// 0x42E9F0's measure of a try's point in a coordinate race (
/// **verified**, asm 0x42EBBB..0x42EC0A): for a point measured by race
/// coordinate (0x5DCC90) the signed 0x5D5CE0(point +0x08, racer +0x8FC),
/// ahead positive, no FABS; for any other the 2D distance to the player.
/// Over [`SPAWN_MAX`] ends the placement; 0x42ED4C compares it with the
/// car's own place.
pub fn reach_measure(f: &RouteFile, record: usize, cp: usize, player: Vec2, race: &RacePlace) -> f32 {
    let c = &f.records[record].points[cp];
    if coordinate_point(f, record, cp) {
        (race.diff)(c.coord, race.coord)
    } else {
        c.position.truncate().distance(player)
    }
}

/// What the placement 0x42E9F0 reads in a coordinate race (0x88F2BE)
/// beyond the free-roam inputs (see [`TrafficLane::spawn_race`]).
pub struct RacePlace<'a> {
    /// 0x5D5CE0(a, b): a − b on the race road (lap-wrapped on a closed road).
    pub diff: &'a dyn Fn(f32, f32) -> f32,
    /// The player's race coordinate (racer +0x8FC).
    pub coord: f32,
    /// The placed car's race coordinate (car +0x8FC) when it is not parked
    /// (controller +0x77C clear); None for a parked car.
    pub own: Option<f32>,
    /// 0x418110 on a point: its place, race coordinate and whether it is
    /// measured by race coordinate (0x5DCC90); true lets it.
    pub gate: &'a dyn Fn(Vec2, f32, bool) -> bool,
    /// The human's wrong-way byte (the player object's +0x248, car
    /// +0x18, written each frame by 0x602688 from 0x600720): at
    /// [`AHEAD_SPEED`] or more 0x42EAFB takes "ahead" only while it is clear.
    pub wrong_way: bool,
}

impl TrafficLane {
    /// 0x42E9F0's placement: from the player's road node ([`road_node`],
    /// inferred) search ([`placer_walk`], 0x415130) ahead — the next index
    /// (at `AHEAD_SPEED` or more while the player is not going the wrong
    /// way, [`RacePlace::wrong_way`]; else ahead or behind on a coin flip) —
    /// `SPAWN_START` along the roads, then try points until one is
    /// `SPAWN_MAX` away in a straight line or `SPAWN_TRIES` are spent, each
    /// try at least `spacing` metres (0x418240, [`spacing`]) and at least
    /// the point's segment further along the road (0x415130). A car drives
    /// against the walk (towards the player) with odds 1 − `with`, the
    /// track's per-level float (track record +0x108 + 4·level; ½ for track
    /// 4000); 0x410CA0 picks its lane on that side or else the other, the
    /// point must carry flag bit 25 (against) or 26 (with), and 0x418110
    /// refuses a point within `DESPAWN` of the player or within `spacing`
    /// (0x410FC0) of another placed car (`others`). Distances go through
    /// `dist`: 2D outside coordinate races (0x409840, 0x410FC0); in a
    /// coordinate race (0x88F2BE, set once a race's road is built) the
    /// game measures race coordinate instead (0x409890, 0x411060 on
    /// 0x5D5CE0) for points that have one (0x5DCC90), and the caller
    /// passes that. Returns the lane, the place (the lane's centre,
    /// 0x5FBEE0 → 0x5DD370) and the heading.
    ///
    /// This is [`TrafficLane::spawn_from`] from the player's own road (the
    /// first of the starts) every time.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        f: &RouteFile,
        player: Vec3,
        heading: Vec2,
        speed: f32,
        with: f32,
        spacing: f32,
        others: &[Vec2],
        dist: &dyn Fn(Vec2, Vec2) -> f32,
        rng: &mut GameRng,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        let mut cursor = usize::MAX;
        Self::spawn_from(f, player, heading, speed, with, spacing, others, dist, rng, &mut cursor, false, None)
    }

    /// [`TrafficLane::spawn`] with the placer's start cursor (0x42E9F0:
    /// placer +0x0C): the walk starts from the player's road or from one
    /// of up to two roads running beside it ([`starts`], 0x417D80), taken
    /// in turn — the cursor moves to (cursor + 1) mod the number of starts
    /// on each try.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_from(
        f: &RouteFile,
        player: Vec3,
        heading: Vec2,
        speed: f32,
        with: f32,
        spacing: f32,
        others: &[Vec2],
        dist: &dyn Fn(Vec2, Vec2) -> f32,
        rng: &mut GameRng,
        cursor: &mut usize,
        coordinate: bool,
        node: Option<(usize, usize)>,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        Self::spawn_race(f, player, heading, speed, with, spacing, others, dist, rng, cursor, coordinate, node, None)
    }

    /// [`TrafficLane::spawn_from`] with what 0x42E9F0 reads in a coordinate
    /// race (0x88F2BE; `race`, [`RacePlace`], **verified** in the asm
    /// 0x42EBBB..0x42EC19 and 0x42ED4C..0x42EDA9): per point the reach test
    /// takes, for a point measured by race coordinate (0x5DCC90), the
    /// signed race-coordinate difference point − player (0x5D5CE0 on point
    /// +0x08 and racer +0x8FC, no FABS: a point behind the player never
    /// fails it), else the 2D distance to the player, and refuses over
    /// 500 m; the gate is 0x418110 (`race.gate`); and a car that is not
    /// parked (+0x77C clear) whose race coordinate lies ahead of the
    /// player's (car +0x8FC − racer +0x8FC > 0) is not placed ahead
    /// farther than that (the measure above it), nor one behind placed
    /// behind farther back (the measure under it): the placement fails
    /// (0x42F910 then parks the car, 0x4020D0 / +0x77C). "Ahead" is the
    /// walk byte before the coordinate race's flip (0x42EB04).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_race(
        f: &RouteFile,
        player: Vec3,
        heading: Vec2,
        speed: f32,
        with: f32,
        spacing: f32,
        others: &[Vec2],
        dist: &dyn Fn(Vec2, Vec2) -> f32,
        rng: &mut GameRng,
        cursor: &mut usize,
        coordinate: bool,
        node: Option<(usize, usize)>,
        race: Option<&RacePlace>,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        let _ = heading;
        // 0x42E9F0: the draw (0x5EA510 under 1 − the track's float), then
        // ahead (the walk byte: 0x401CC0 steps to the next index) at 55 mph
        // or more, else on a coin flip — along the record's own order, not
        // the player's heading (**verified**: no heading is read).
        let against = rng.unit(1.0) < 1.0 - with;
        // **Verified** (asm 0x42EAB3..0x42EAFF; **observed** under
        // Wine, `docs/evidence/traf-5/`): at 55 mph or more the byte is set
        // only while the human's player object (car +0x18) has its
        // wrong-way byte +0x248 clear (0x600720, the race's wrong-way test;
        // 0 outside races, where it has no road node); no draw then. The
        // clone ignored the byte, so a player the game counts as going the
        // wrong way got traffic behind him instead of ahead.
        let wrong_way = race.is_some_and(|r| r.wrong_way);
        let ahead = if speed >= AHEAD_SPEED { !wrong_way } else { rng.unit(1.0) >= 0.5 };
        // The player's road node (controller +0x0C, 0x5EC0C0 / 0x5EC0B0):
        // the human controller's node, kept by 0x42A190 and searched by
        // 0x411490 / 0x40F180 (`HumanRoad`), handed in as `node` —
        // the same record / control point indices, the traffic's route file
        // being the same file (free roam: RoutesFreeRoam.bin, whose pruning
        // drops junctions only; a race: Routes<track><F|B>.bin). The clone
        // updates it after the traffic's think, so the placer reads the
        // previous update's (the order of the controllers' pass and the
        // placer 0x42F910 within the update is **inferred**). Without one
        // (free roam's human road runs with the encounters; before the first
        // update) the start of the nearest segment of any record
        // ([`road_node`]): under Wine at career free roam's start the node
        // was record 515's point 6, a record without traffic lanes (13.137 m,
        // against 13.154 m for record 692's segment 7 and 15.6 m for 692's
        // prov: inferred ai.traffic.spawn_from
        // point 8, the nearest point; **inferred** from that observation).
        let near = node.or_else(|| road_node(f, player.truncate()));
        why("node");
        let near = near?;
        // 0x42ED4C reads the byte before the flip ([esp+0xE]).
        let forward = ahead;
        // In a coordinate race (0x88F2BE, the race road built) the byte
        // flips on a node record with +0x70 bit 1 (0x42EB04).
        let ahead = ahead != (coordinate && f.records[near.0].flags & 2 != 0);
        let all = starts(f, Step { record: near.0, cp: near.1, backward: !ahead, lane: 0 }, PLACER_STARTS);
        *cursor = cursor.wrapping_add(1) % all.len();
        #[cfg(feature = "dev-harness")]
        place_probe::STARTS.with(|x| x.set((*cursor, all.len())));
        why("walk0");
        let mut s = placer_walk(f, all[*cursor], SPAWN_START, coordinate, false, rng)?;
        for _ in 0..SPAWN_TRIES {
            let cp = &f.records[s.record].points[s.cp];
            // 0x42EBBB: by race coordinate (signed) for a coordinate point in
            // a coordinate race, else in 2D.
            let by_coord = race.is_some() && coordinate_point(f, s.record, s.cp);
            let measure = match race {
                Some(r) => reach_measure(f, s.record, s.cp, player.truncate(), r),
                None => dist(cp.position.truncate(), player.truncate()),
            };
            if measure > SPAWN_MAX {
                why("reach");
                return None;
            }
            // 0x410CA0's side flag is (walk byte clear) ≠ the draw, set
            // meaning lanes 2–5; the other side when that fails. The point
            // needs bit 25 with the draw, else 26.
            let side = |backward: bool| Step { backward, ..s };
            let first = s.backward == against;
            let placed = pick_lane(f, side(first), rng)
                .map(|l| (side(first), l))
                .or_else(|| pick_lane(f, side(!first), rng).map(|l| (side(!first), l)));
            let at = cp.position.truncate();
            let clear = match race {
                Some(r) => (r.gate)(at, cp.coord, by_coord),
                None => dist(at, player.truncate()) >= DESPAWN && others.iter().all(|&o| dist(o, at) >= spacing),
            };
            if let Some((start, l)) = placed.filter(|_| cp.flags >> if against { 25 } else { 26 } & 1 != 0 && clear) {
                // 0x42ED4C: a car not parked keeps to its side of the player.
                if let Some(old) = race.and_then(|r| r.own.map(|o| (r.diff)(o, r.coord))) {
                    let farther = if forward { old > 0.0 && measure > old } else { old < 0.0 && measure < old };
                    if farther {
                        why("keep");
                        return None;
                    }
                }
                let start = Step { lane: l, ..start };
                let mut t = TrafficLane {
                    steps: VecDeque::from([start]),
                    rng: lane_stream(rng),
                    obey: None,
                    avoid: None,
                    roomed: 0,
                    took_avoid: false,
                    behind: None,
                    kept: Vec::new(),
                    heading: None,
                    walked: 0.0,
                    last_turn: -99_999.0,
                    turned: None,
                    lead: (0, 0.0),
                    xs: VecDeque::new(),
                    drag: false,
                };
                t.extend(f, ENTRY_REACH);
                t.roomed = t.steps.len();
                let at = point(f, start, 0.0);
                let dir = f.direction(start.record, start.cp, start.backward);
                #[cfg(feature = "dev-harness")]
                why(&format!("ok:{measure:.1}"));
                return Some((t, at, dir.y.atan2(dir.x)));
            }
            // On by max(the point's segment, the spacing); a walk that
            // fails or stays put ends the placement (the walk at 0x42ED06; 0x42ED29 returns 0).
            let step = cp.length.max(spacing);
            why("walk");
            let n = placer_walk(f, s, step, coordinate, false, rng)?;
            if (n.record, n.cp) == (s.record, s.cp) {
                why("stay");
                return None;
            }
            s = n;
        }
        why("tries");
        None
    }

    /// The drag race placement (0x42E620; verified): draws the flag
    /// (0x5EA510 under 1 − the track's per-level float, `with`), finds the
    /// point ([`drag_point`], 0x417C60 with 250 m, 0x7E9720), flips the flag
    /// when the point's record has +0x70 bit 1 (0x5D89A0, in a coordinate
    /// race 0x88F2BE — always in the clone's races), picks a lane on that
    /// side or else the other ([`drag_lane`], 0x410CA0: flag set → lanes
    /// 2–5, clear → 6–9), and places the car there when `gate` (0x418110,
    /// on the point's place and race coordinate) lets it: on the lane at
    /// the point, moving at the lane's speed byte (0x42C950 with the cap
    /// 0x41C4B127, 55 mph). `player_lane` is the human racer's lane (2–5,
    /// the forward index of its controller's lane, vt+0x1C).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_drag(
        f: &RouteFile,
        pos: Vec2,
        coord: f32,
        player_lane: Option<usize>,
        with: f32,
        gate: &dyn Fn(Vec2, f32, bool) -> bool,
        rng: &mut GameRng,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        let flag = rng.unit(1.0) < 1.0 - with;
        let (record, cp) = drag_point(f, Some(pos), coord, DRAG_REACH, flag)?;
        let flag = flag ^ (f.records[record].flags & 2 != 0);
        let s = |flag: bool| Step { record, cp, backward: !flag, lane: 0 };
        let (start, l) = drag_lane(f, s(flag), player_lane, true, rng)
            .map(|l| (s(flag), l))
            .or_else(|| drag_lane(f, s(!flag), player_lane, true, rng).map(|l| (s(!flag), l)))?;
        let c = &f.records[record].points[cp];
        if !gate(c.position.truncate(), c.coord, coordinate_point(f, record, cp)) {
            return None;
        }
        let start = Step { lane: l, ..start };
        let mut t = TrafficLane {
            steps: VecDeque::from([start]),
            rng: lane_stream(rng),
            obey: None,
            avoid: None,
            roomed: 0,
            took_avoid: false,
            behind: None,
            kept: Vec::new(),
            heading: None,
            walked: 0.0,
            last_turn: -99_999.0,
            turned: None,
            lead: (0, 0.0),
            xs: VecDeque::new(),
            drag: true,
        };
        t.extend(f, ENTRY_REACH);
        t.roomed = t.steps.len();
        let at = point(f, start, 0.0);
        let dir = f.direction(start.record, start.cp, start.backward);
        Some((t, at, dir.y.atan2(dir.x)))
    }

    /// The race set-up's drag fill (0x42E4A0; verified, the disassembly):
    /// the set-up 0x6065C0 hands each traffic entry of the car table
    /// (0x89E810, 0x940 bytes, +4 = 3) to 0x42F7A0, which in a drag race
    /// (0x89E7D8 / 0x890108) tries 0x42E4A0 first. For k = 0..99 it draws
    /// the flag (0x5EA510 under 1 − `with`), finds the point with no
    /// position, race coordinate 0 and reach (entry +1 − 50 + k) × `step`
    /// ([`drag_point`]; `step` is the track's float +0x100 + 4·direction,
    /// the entry byte +1 is 50, 51, … for the traffic entries in table
    /// order, **observed** under Wine, so `index` + k), flips the flag by
    /// the record's +0x70 bit 1, takes a lane on that side or the other
    /// (0x410CA0) and stops at the first point `gate` (0x418110 with the
    /// spacing `step` and no car) lets: the car goes there (0x42C670: on
    /// the lane at the point, moving at the lane's speed byte, race
    /// coordinate +0x8FC the point's +0x08). None after 100 tries.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_drag(
        f: &RouteFile,
        index: usize,
        step: f32,
        player_lane: Option<usize>,
        humans: bool,
        with: f32,
        gate: &dyn Fn(Vec2, f32, bool) -> bool,
        rng: &mut GameRng,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        for k in 0..100 {
            let flag = rng.unit(1.0) < 1.0 - with;
            let Some((record, cp)) = drag_point(f, None, 0.0, (index + k) as f32 * step, flag) else { continue };
            let flag = flag ^ (f.records[record].flags & 2 != 0);
            let s = |flag: bool| Step { record, cp, backward: !flag, lane: 0 };
            let Some((start, l)) = drag_lane(f, s(flag), player_lane, humans, rng)
                .map(|l| (s(flag), l))
                .or_else(|| drag_lane(f, s(!flag), player_lane, humans, rng).map(|l| (s(!flag), l)))
            else {
                continue;
            };
            let c = &f.records[record].points[cp];
            if !gate(c.position.truncate(), c.coord, coordinate_point(f, record, cp)) {
                continue;
            }
            let start = Step { lane: l, ..start };
            let mut t = TrafficLane {
                steps: VecDeque::from([start]),
                rng: lane_stream(rng),
                obey: None,
                avoid: None,
                roomed: 0,
                took_avoid: false,
                behind: None,
                kept: Vec::new(),
                heading: None,
                walked: 0.0,
                last_turn: -99_999.0,
                turned: None,
                lead: (0, 0.0),
                xs: VecDeque::new(),
                drag: true,
            };
            t.extend(f, ENTRY_REACH);
            t.roomed = t.steps.len();
            let at = point(f, start, 0.0);
            let dir = f.direction(start.record, start.cp, start.backward);
            return Some((t, at, dir.y.atan2(dir.x)));
        }
        None
    }

    /// The race set-up's fill outside drag races (0x42F7A0 → 0x42E2C0;
    /// verified, the disassembly). The placer's +0x14 is the number of
    /// control points over the race records (0x88D21C, id order) that have
    /// a lane on the set side (0x409900 from the placer ctor 0x4179F0 with
    /// flag 1: 0x5D4490, bytes +0x84..+0x87). With that non-zero, for
    /// k = 0..99: the flag drawn (0x5EA510 under 1 − `with`); **0x411110**
    /// draws r = 0x5EA4F0(total) and walks the race records, each with the
    /// flag flipped by its +0x70 bit 1 (coordinate race) and counted only
    /// when it has a lane on that side, summing their +0x34; at the first
    /// sum above r the point is index sum − r (1..=count) of that record.
    /// When no record reaches r the previous try's pick stays (the
    /// function's locals start null once, before the loop). The point must
    /// belong to the record (+0x0C bits 0–10 = record +0x0A), carry both
    /// bits 25 and 26 and have index ≤ count − 2; then 0x418110(&{record,
    /// point, …, its place}, `step` = track +0x100 + 4·direction, no car)
    /// gates it (`gate`, which holds that spacing); the flag flips by the record's +0x70 bit 1; 0x417C30 takes
    /// a lane on that side or else the other (0x410CA0, ten random picks
    /// then the first, outside drag races); 0x42C670 creates the car there.
    /// The flips by +0x70 bit 1 are made only with the race road built
    /// (`road`; free roam's entry fill has none: docs/formats/ai.md, "Fill
    /// outside drag races", **verified**).
    pub fn fill_race(
        f: &RouteFile,
        road: bool,
        with: f32,
        gate: &dyn Fn(Vec2, f32, bool) -> bool,
        rng: &mut GameRng,
    ) -> Option<(TrafficLane, Vec3, f32)> {
        let mut races: Vec<usize> = f.records.iter().enumerate().filter(|(_, r)| r.is_race()).map(|(i, _)| i).collect();
        races.sort_by_key(|&i| f.records[i].id);
        let side_has = |ri: usize, side: bool| f.records[ri].lanes[if side { 2..6 } else { 6..10 }].iter().any(|&l| l);
        let total: usize = races.iter().filter(|&&ri| side_has(ri, true)).map(|&ri| f.records[ri].points.len()).sum();
        if total == 0 {
            return None;
        }
        let mut pick: Option<(usize, usize)> = None;
        for _ in 0..100 {
            let flag = rng.unit(1.0) < 1.0 - with;
            let r = rng.below(total as u32) as usize;
            let mut sum = 0usize;
            for &ri in &races {
                let rec = &f.records[ri];
                if !side_has(ri, flag ^ (road && rec.flags & 2 != 0)) {
                    continue;
                }
                sum += rec.points.len();
                if sum > r {
                    pick = Some((ri, sum - r));
                    break;
                }
            }
            let Some((record, cp)) = pick else { continue };
            let rec = &f.records[record];
            if cp + 2 > rec.points.len() {
                continue;
            }
            let c = &rec.points[cp];
            if c.flags & 0x7FF != u32::from(rec.id) || c.flags >> 25 & 3 != 3 {
                continue;
            }
            if !gate(c.position.truncate(), c.coord, coordinate_point(f, record, cp)) {
                continue;
            }
            let flag = flag ^ (road && rec.flags & 2 != 0);
            let s = |flag: bool| Step { record, cp, backward: !flag, lane: 0 };
            let Some((start, l)) = pick_lane(f, s(flag), rng)
                .map(|l| (s(flag), l))
                .or_else(|| pick_lane(f, s(!flag), rng).map(|l| (s(!flag), l)))
            else {
                continue;
            };
            let start = Step { lane: l, ..start };
            let mut t = TrafficLane {
                steps: VecDeque::from([start]),
                rng: lane_stream(rng),
                obey: None,
                avoid: None,
                roomed: 0,
                took_avoid: false,
                behind: None,
                kept: Vec::new(),
                heading: None,
                walked: 0.0,
                last_turn: -99_999.0,
                turned: None,
                lead: (0, 0.0),
                xs: VecDeque::new(),
                drag: false,
            };
            t.extend(f, ENTRY_REACH);
            t.roomed = t.steps.len();
            let at = point(f, start, 0.0);
            let dir = f.direction(start.record, start.cp, start.backward);
            return Some((t, at, dir.y.atan2(dir.x)));
        }
        None
    }
}
