//! The road network of a Paths<track>.bin (read at 0x5D9AA0) and the race
//! route the game builds through it from a checkpoint list (0x5E5BC0).
//!
//! * 0x34148 nodes, 0x18 bytes: f32 x, y, u16 segment, …, f32 at +0x14.
//! * 0x34149 segments, 0xDC bytes: i16 first node +0x1C, i16 end node +0x1E
//!   (exclusive), u8 connection counts +0x22 / +0x23 for the two ends,
//!   connections at +0x24 / +0x44 (4 × 8 bytes: u16 segment, u8 the
//!   direction it is entered in, …; 0x5D5640 turns the index into a
//!   pointer), f32 length +0x64, i16 barrier count +0xBC and barrier
//!   indices +0xBE.
//! * 0x3414D barriers, 0x24 bytes (0x88F298 +0x498 count, +0x49C): two
//!   ends (f32 x, y at +0x00 and +0x08), u8 closed at +0x10 (0 on disk),
//!   i8 count +0x11, u8 +0x12 (set with +0x10 by 0x5D5020 and not cleared
//!   by 0x5D4FF0; 0x5E1CB0 ignores barriers with it set for a junction's
//!   second type), that many barrier-group name hashes from +0x14.
//!
//! A segment is run in direction 0 from its first node to its last, and
//! leaves through its +0x44 connections; direction 1 the other way, out
//! through +0x24 (0x5D53E0's `k + (dir == 0) * 4`).

use glam::{Vec2, Vec4};

use crate::formats::bundle::{chunks, walk};
use crate::formats::reader::{FormatResult, Reader};

#[derive(Clone, Debug)]
pub struct Segment {
    pub first: usize,
    pub end: usize,
    pub length: f32,
    /// The spline length as exported (+0x6C; rebuilt for the race by
    /// 0x5D50A0 with the route's neighbours).
    pub spline_length: f32,
    /// Connections out of the first node's end (+0x24) and the last's
    /// (+0x44): segment, direction.
    pub exits: [Vec<(usize, u8)>; 2],
    /// Barrier records (0x3414D) that close the segment.
    pub barriers: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct RoadNet {
    pub nodes: Vec<(Vec2, usize)>,
    /// Each node's locator bytes (+0x0A, +0x0B: twice the distances in
    /// m that keep it / try its links; 0xFF +0x0A: not usable) and links
    /// to other nodes (+0x0C, +0x0E, +0x10, i16, -1 none).
    pub links: Vec<(u8, u8, [i16; 3])>,
    /// Each node's place along a road as exported (+0x14), until a race
    /// road through it is built.
    pub along: Vec<f32>,
    pub segments: Vec<Segment>,
    /// Barrier closed flags (0x3414D +0x10).
    pub closed: Vec<bool>,
    /// Each barrier's two ends (+0x00, +0x08).
    pub barrier_ends: Vec<[Vec2; 2]>,
    /// Barrier +0x12: set with +0x10 when closed, never cleared.
    pub barrier_held: Vec<bool>,
    /// The barrier-group name hashes each record answers to (+0x14).
    pub barrier_names: Vec<Vec<u32>>,
    /// How many segment ranges and node-to-segment links [`RoadNet::parse`]
    /// had to repair (0 for every shipped file).
    pub repaired: usize,
}

/// The search state 0x5D53E0 copies per branch (28 dwords): the distance
/// so far, the bound, the depth limit, the target and the segments taken.
#[derive(Clone)]
struct Search {
    distance: f32,
    bound: f32,
    depth: usize,
    target: usize,
    taken: Vec<(usize, u8)>,
}

impl RoadNet {
    pub fn parse(data: &[u8]) -> RoadNet {
        let mut net = RoadNet::default();
        // Whole records of each table; every read is inside its record, so
        // the `filter_map`s below drop nothing.
        fn table<'a, T>(body: &'a [u8], stride: usize, f: impl Fn(&Reader<'a>) -> FormatResult<T>) -> Vec<T> {
            Reader::new(body).whole_records(stride).filter_map(|r| f(&r).ok()).collect()
        }
        let vec2 = |r: &Reader<'_>, at: usize| -> FormatResult<Vec2> { Ok(Vec2::new(r.f32_at(at)?, r.f32_at(at + 4)?)) };
        walk(chunks(data, 0), &mut |c| match c.id {
            0x34148 => {
                net.nodes = table(c.body, 0x18, |r| Ok((vec2(r, 0)?, r.u16_at(8)? as usize)));
                net.along = table(c.body, 0x18, |r| r.f32_at(0x14));
                net.links = table(c.body, 0x18, |r| Ok((r.u8_at(0xA)?, r.u8_at(0xB)?, [r.i16_at(0xC)?, r.i16_at(0xE)?, r.i16_at(0x10)?])));
            }
            0x34149 => {
                net.segments = table(c.body, 0xDC, |r| {
                    let group = |at: usize, count: u8| (0..count.min(4) as usize).map(|k| Ok((r.u16_at(at + k * 8)? as usize, r.u8_at(at + k * 8 + 2)?))).collect::<FormatResult<_>>();
                    let barriers = (0..r.i16_at(0xBC)?.clamp(0, 14) as usize).map(|k| Ok(r.u16_at(0xBE + k * 2)? as usize)).collect::<FormatResult<_>>()?;
                    Ok(Segment {
                        first: r.u16_at(0x1C)? as usize,
                        end: r.u16_at(0x1E)? as usize,
                        length: r.f32_at(0x64)?,
                        spline_length: r.f32_at(0x6C)?,
                        exits: [group(0x24, r.u8_at(0x22)?)?, group(0x44, r.u8_at(0x23)?)?],
                        barriers,
                    })
                });
            }
            0x3414D => {
                net.closed = table(c.body, 0x24, |r| Ok(r.u8_at(0x10)? != 0));
                net.barrier_ends = table(c.body, 0x24, |r| Ok([vec2(r, 0)?, vec2(r, 8)?]));
                net.barrier_held = table(c.body, 0x24, |r| Ok(r.u8_at(0x12)? != 0));
                net.barrier_names = table(c.body, 0x24, |r| (0..(r.u8_at(0x11)? as i8).clamp(0, 4) as usize).map(|k| r.u32_at(0x14 + 4 * k)).collect());
            }
            _ => {}
        });
        net.repair();
        net
    }

    /// The u16 node ranges of a segment and the segment of a
    /// node come straight from the file. A range is cut to the node table
    /// and an empty or backwards one made empty; a node whose segment does
    /// not exist or does not hold it gets `usize::MAX` ("no segment", every
    /// consumer looks segments up with `get`). The tables stay as long as
    /// they were, since nodes and segments refer to each other by index.
    fn repair(&mut self) {
        let n = self.nodes.len();
        for s in &mut self.segments {
            let (first, end) = (s.first, s.end.min(n));
            let (first, end) = if first < end { (first, end) } else { (0, 0) };
            if (first, end) != (s.first, s.end) {
                self.repaired += 1;
            }
            (s.first, s.end) = (first, end);
        }
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if !self.segments.get(node.1).is_some_and(|s| (s.first..s.end).contains(&i)) {
                node.1 = usize::MAX;
                self.repaired += 1;
            }
        }
    }

    /// The barriers a track load switches on (0x578070): every record
    /// opened (0x5D4FF0), then those naming one of `names` closed
    /// (0x5D5020, by name hash): a race's `BARRIERS_<n>` and
    /// `PLAYER_BARRIERS_<n>`, and with no race `BARRIERS_CAREER<stage>`.
    /// The race road is searched after this (0x57A660 → 0x5E6A50).
    pub fn close_barriers(&mut self, names: &[&str]) {
        let hashes: Vec<u32> = names.iter().map(|n| crate::formats::hash::bin_hash(n)).collect();
        for (k, (closed, listed)) in self.closed.iter_mut().zip(&self.barrier_names).enumerate() {
            *closed = listed.iter().any(|h| hashes.contains(h));
            if *closed && let Some(h) = self.barrier_held.get_mut(k) {
                *h = true;
            }
        }
    }

    /// The node nearest `at` over every segment (0x5D57B0 with no limit).
    pub fn nearest_node(&self, at: Vec2) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for s in &self.segments {
            for n in s.first..s.end.min(self.nodes.len()) {
                let d = self.nodes[n].0.distance(at);
                if best.is_none_or(|b| d < b.1) {
                    best = Some((n, d));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// The depth-first search 0x5D53E0: take `seg` in `dir`, then the
    /// shortest way on to the target within the depth limit and the bound.
    fn search(&self, s: &mut Search, seg: usize, dir: u8, depth: usize) -> bool {
        let Some(segment) = self.segments.get(seg) else { return false };
        if s.taken.len() == 16 {
            return false;
        }
        if s.taken.iter().any(|t| t.0 == seg) && seg != s.target {
            return false;
        }
        if segment.barriers.iter().any(|&b| self.closed.get(b).copied().unwrap_or(false)) {
            return false;
        }
        s.taken.push((seg, dir));
        s.distance += segment.length;
        if seg == s.target {
            return true;
        }
        if depth + 1 > s.depth || s.distance > s.bound {
            return false;
        }
        let mut best: Option<Search> = None;
        for &(next, next_dir) in &segment.exits[if dir == 0 { 1 } else { 0 }] {
            let mut branch = s.clone();
            if self.search(&mut branch, next, next_dir, depth + 1) && best.as_ref().is_none_or(|b| branch.distance < b.distance) {
                s.bound = branch.distance;
                branch.bound = branch.distance;
                best = Some(branch);
            }
        }
        match best {
            Some(b) => {
                *s = b;
                true
            }
            None => false,
        }
    }

    /// The race route through the checkpoints (0x5E5BC0): between each
    /// pair, from the segment nearest the first to the one nearest the
    /// second; the first pair's direction is the segment's that does not
    /// run against the pair, the later ones carry on in the last segment's.
    /// A segment already on the route ends the build, except the one just
    /// taken and, on a loop, the first.
    pub fn route(&self, points: &[Vec2], loops: bool) -> Option<Vec<(usize, u8)>> {
        let mut route: Vec<(usize, u8)> = Vec::new();
        for (i, pair) in points.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            let node = self.nearest_node(a)?;
            let seg = self.nodes[node].1;
            let segment = self.segments.get(seg)?;
            let dir = if i == 0 {
                let along = (b - a).normalize_or_zero();
                let tangent = if node + 1 < segment.end {
                    self.nodes[node + 1].0 - self.nodes[node].0
                } else {
                    self.nodes[node].0 - self.nodes[node.checked_sub(1)?].0
                };
                u8::from(along.dot(tangent.normalize_or_zero()) <= 0.0)
            } else {
                route.last()?.1
            };
            let target = self.nodes[self.nearest_node(b)?].1;
            let mut s = Search { distance: 0.0, bound: 9_999_999.0, depth: 10, target, taken: Vec::new() };
            if !self.search(&mut s, seg, dir, 0) {
                return None;
            }
            for t in s.taken {
                if route.last().is_some_and(|l| l.0 == t.0) || (loops && route.first().is_some_and(|f| f.0 == t.0)) {
                    continue;
                }
                if route.iter().any(|r| r.0 == t.0) {
                    return None;
                }
                route.push(t);
            }
        }
        Some(route)
    }

    /// The route's nodes in driving order.
    pub fn polyline(&self, route: &[(usize, u8)]) -> Vec<Vec2> {
        self.pieces(route).0
    }

    /// The route's nodes in driving order and, for each node, the road
    /// segment it came from.
    pub fn pieces(&self, route: &[(usize, u8)]) -> (Vec<Vec2>, Vec<usize>) {
        let mut points: Vec<Vec2> = Vec::new();
        let mut segments = Vec::new();
        for &(seg, dir) in route {
            let s = &self.segments[seg];
            let nodes = s.first..s.end.min(self.nodes.len());
            let mut run: Vec<Vec2> = nodes.map(|n| self.nodes[n].0).collect();
            if dir == 1 {
                run.reverse();
            }
            for p in run {
                if points.last().is_none_or(|l| l.distance(p) > 0.01) {
                    points.push(p);
                    segments.push(seg);
                }
            }
        }
        (points, segments)
    }
}

impl RoadNet {
    /// The race road through `route` (0x5E48C0 from 0x5E5BC0, scale 1):
    /// a spline per route segment, each starting where the last ended
    /// (its +0x84), the road's length (route +0x18) the last one's end;
    /// each node's place along the road (+0x14, 0x5D52D0). The polyline
    /// is the nodes in driving order at those places.
    pub fn race_road(&self, route: &[(usize, u8)], loops: bool) -> RoadRoute {
        // A route names segments of this network.
        let route: Vec<(usize, u8)> = route.iter().copied().filter(|&(seg, _)| seg < self.segments.len()).collect();
        let route = &route[..];
        let count = route.len();
        let node = |k: usize| self.nodes.get(k).map_or(Vec2::ZERO, |n| n.0);
        // The way a route segment leaves its far end and enters its near
        // end, in driving order (0x5E48C0: the neighbour's second node
        // less its first, or its last but one less its last).
        let lead = |seg: usize, dir: u8, out: bool| {
            let s = &self.segments[seg];
            if (dir == 0) != out { node(s.end.saturating_sub(2)) - node(s.end.saturating_sub(1)) } else { node(s.first + 1) - node(s.first) }
        };
        // While building, the route's length (+0x18) is 1 (0x5E5BC0), so
        // a locator's fraction is its distance.
        let nodes = (0..self.nodes.len()).map(|j| RoadNode { at: node(j), segment: self.nodes[j].1, near: self.links.get(j).map_or(0xFF, |l| l.0), far: self.links.get(j).map_or(0, |l| l.1), links: self.links.get(j).map_or([-1; 3], |l| l.2), along: self.along.get(j).copied().unwrap_or(0.0) }).collect();
        let bounds = self.segments.iter().map(|s| (s.first..s.end.min(self.nodes.len())).fold((Vec2::MAX, Vec2::MIN), |(lo, hi), j| (lo.min(node(j)), hi.max(node(j))))).collect();
        let mut road = RoadRoute { length: 1.0, loops, piece_of: vec![None; self.segments.len()], nodes, bounds, ..Default::default() };
        let mut end = 0.0;
        for (k, &(seg, dir)) in route.iter().enumerate() {
            let s = &self.segments[seg];
            let (a, b) = (node(s.first), node(s.end.max(s.first + 1) - 1));
            road.piece_of[seg] = None;
            // The segment starts where a fresh locator (0x5DEDA0) puts its
            // driving start (+0x88, or +0x90 run backward) on the splines
            // built so far: 0 for the first, else the last one's end and
            // the gap to it.
            let start = road.locate_once(if dir == 0 { a } else { b });
            // The phantom control points before the first node and after
            // the last (0x5D50A0): from the neighbours along the route,
            // else the end piece carried on.
            let mut before = None;
            let mut after = None;
            if k > 0 || loops {
                let (ps, pd) = route[(count + k - 1) % count];
                let v = lead(ps, pd, false);
                if dir == 0 { before = Some(a + v) } else { after = Some(b + v) }
            }
            if k + 1 < count || loops {
                let (ns, nd) = route[(k + 1) % count];
                let v = lead(ns, nd, true);
                if dir == 0 { after = Some(b + v) } else { before = Some(a + v) }
            }
            let mut cps = vec![before.unwrap_or(a - (node(s.first + 1) - a))];
            cps.extend((s.first..s.end).map(node));
            cps.push(after.unwrap_or(b + (b - node(s.end.max(2) - 2))));
            let mut spline = RoadSpline { segment: seg, dir, first: s.first, exported: s.spline_length, cps: cps.iter().map(|p| Vec4::new(p.x, p.y, 0.0, 0.0)).collect(), ..Default::default() };
            spline.measure(16);
            let n = s.end.saturating_sub(s.first) as f32;
            spline.w1 = spline.eval(1.0, false).w;
            spline.length = spline.eval(n, false).w - spline.w1;
            // 0x5D52D0: direction 1 runs from the far end back.
            (spline.start, spline.scale) = if dir == 0 { (start, 1.0) } else { (start + spline.length, -1.0) };
            end = start + spline.length;
            for j in s.first..s.end.min(self.nodes.len()) {
                road.nodes[j].along = spline.along((j - s.first + 1) as f32);
            }
            road.piece_of[seg] = Some(road.splines.len());
            road.splines.push(spline);
        }
        let along: Vec<f32> = road.nodes.iter().map(|n| n.along).collect();
        let start = end;
        let mut points: Vec<Vec2> = Vec::new();
        let mut at = Vec::new();
        let mut segments = Vec::new();
        for &(seg, dir) in route {
            let s = &self.segments[seg];
            let mut run: Vec<usize> = (s.first..s.end.min(self.nodes.len())).collect();
            if dir == 1 {
                run.reverse();
            }
            for j in run {
                if points.last().is_none_or(|l: &Vec2| l.distance(node(j)) > 0.01) && at.last().is_none_or(|&l| along[j] >= l) {
                    points.push(node(j));
                    at.push(along[j]);
                    segments.push(seg);
                }
            }
        }
        if loops && let Some(&first) = points.first() {
            points.push(first);
            at.push(start);
            segments.push(segments[0]);
        }
        RoadRoute { points, at, length: start, segments, ..road }
    }
}

/// A route segment's spline (0x57A310 type 2: a uniform Catmull-Rom
/// through (x, y, 0, w) points, the 0x865230 / 0x865270 matrices; past
/// either end the end pieces carry on in a line, 0x570250 / 0x570520).
/// The control points are the phantom before the first node, the nodes
/// in their own order, the phantom after the last; w is the arc length
/// to each (0x5778E0 with 16 steps a span).
#[derive(Clone, Debug, Default)]
pub struct RoadSpline {
    pub segment: usize,
    pub dir: u8,
    pub first: usize,
    cps: Vec<Vec4>,
    /// +0x70: w at the first node; +0x6C: the arc length first to last
    /// node; +0x74 / +0x7C: the road distance at the parameter's origin
    /// and its sign.
    pub w1: f32,
    pub length: f32,
    pub start: f32,
    pub scale: f32,
    /// The segment's exported spline length (+0x6C as read).
    pub exported: f32,
}

impl RoadSpline {
    fn cp(&self, j: i32) -> Vec4 {
        let n = self.cps.len() as i32;
        if j < 0 {
            self.cps[0] + (self.cps[0] - self.cps[1]) * (-j) as f32
        } else if j >= n {
            self.cps[n as usize - 1] + (self.cps[n as usize - 1] - self.cps[n as usize - 2]) * (j - n + 1) as f32
        } else {
            self.cps[j as usize]
        }
    }

    /// 0x570650: the point (or, `slope`, the derivative) at parameter `t`
    /// (piece `t` truncated, as _ftol does).
    pub fn eval(&self, t: f32, slope: bool) -> Vec4 {
        let i = t as i32;
        let u = t - i as f32;
        let (u2, u3) = (u * u, u * u * u);
        let w = if slope {
            [-0.5 + 2.0 * u - 1.5 * u2, -5.0 * u + 4.5 * u2, 0.5 + 4.0 * u - 4.5 * u2, -u + 1.5 * u2]
        } else {
            [-0.5 * u + u2 - 0.5 * u3, 1.0 - 2.5 * u2 + 1.5 * u3, 0.5 * u + 2.0 * u2 - 1.5 * u3, -0.5 * u2 + 0.5 * u3]
        };
        (0..4).map(|k| self.cp(i - 1 + k) * w[k as usize]).sum()
    }

    /// 0x5778E0: each control point's w is the arc length to it, summed
    /// over `steps` chords a span.
    fn measure(&mut self, steps: usize) {
        let mut total = 0.0;
        for i in 0..self.cps.len() {
            self.cps[i].w = total;
            if i + 1 < self.cps.len() {
                let mut last = self.eval(i as f32, false).truncate();
                for k in 1..=steps {
                    let p = self.eval(i as f32 + k as f32 / steps as f32, false).truncate();
                    total += p.distance(last);
                    last = p;
                }
            }
        }
    }

    /// The road distance at parameter `t` (0x5D9E40 / 0x5D52D0).
    pub fn along(&self, t: f32) -> f32 {
        (self.eval(t, false).w - self.w1) * self.scale + self.start
    }

    /// 0x5774D0: the parameter nearest `q`, from `t`, until the step's
    /// miss is under `tolerance` (at most 998 steps; the step capped at a
    /// bound starting at 1 and halved when it turns back; the slope the
    /// derivative's length, then the last two misses').
    pub fn nearest(&self, q: Vec2, mut t: f32, tolerance: f32) -> f32 {
        let (mut bound, mut last_step, mut last_miss) = (1.0f32, 0.0f32, 0.0f32);
        for _ in 0..998 {
            let p = self.eval(t, false).truncate();
            let d = self.eval(t, true).truncate();
            let mut slope = d.length();
            if slope == 0.0 {
                return t;
            }
            let miss = (q.extend(0.0) - p).dot(d) / slope;
            if miss.abs() < tolerance {
                return t;
            }
            if last_step != 0.0 {
                slope = -((miss - last_miss) / last_step);
                if slope == 0.0 {
                    return t;
                }
            }
            let step = (miss / slope).max(-bound).min(bound);
            t += step;
            if step * last_step < 0.0 {
                bound *= 0.5;
            }
            last_step = step;
            last_miss = miss;
        }
        t
    }
}

/// A node as the locator sees it (see `RoadNet::links`), with its place
/// along the race road (+0x14).
#[derive(Clone, Copy, Debug, Default)]
pub struct RoadNode {
    pub at: Vec2,
    pub segment: usize,
    pub near: u8,
    pub far: u8,
    pub links: [i16; 3],
    pub along: f32,
}

/// A locator (0x5D9E40's object): the node it last chose (+0x18), the
/// spline parameter (+0x1C), where it was asked last (+0x08), the road
/// distance (+0x24) and fraction of the road (+0x20), the point on the
/// road (+0x10) and its driving direction (+0x28); +0x38 how far
/// (|Δx| + |Δy|, m) a point must be from the last ask before 0x5DA340
/// locates again (0 from 0x5DA300 / 0x5DEDA0: every time).
#[derive(Clone, Copy, Debug, Default)]
pub struct Locator {
    pub node: Option<usize>,
    pub t: f32,
    pub last: Vec2,
    pub along: f32,
    pub fraction: f32,
    pub point: Vec2,
    pub direction: Vec2,
    pub threshold: f32,
}

/// Distance along a polyline: the points, the distance at each.
#[derive(Clone, Debug, Default)]
pub struct RoadRoute {
    pub points: Vec<Vec2>,
    pub at: Vec<f32>,
    pub length: f32,
    pub loops: bool,
    /// The road segment of each node (empty when not built from the
    /// network); a piece belongs to its end node's segment.
    pub segments: Vec<usize>,
    /// The route segments' splines (empty when not built from the
    /// network), the spline of each network segment on the route, the
    /// network's nodes and each segment's node bounds (+0x98 / +0xA0).
    pub splines: Vec<RoadSpline>,
    pub piece_of: Vec<Option<usize>>,
    pub nodes: Vec<RoadNode>,
    pub bounds: Vec<(Vec2, Vec2)>,
}

impl RoadRoute {
    pub fn new(mut points: Vec<Vec2>, loops: bool) -> RoadRoute {
        if loops && let (Some(&first), Some(&last)) = (points.first(), points.last())
            && first.distance(last) > 0.01
        {
            points.push(first);
        }
        let mut at = vec![0.0];
        for w in points.windows(2) {
            at.push(at.last().unwrap() + w[0].distance(w[1]));
        }
        let length = *at.last().unwrap_or(&0.0);
        RoadRoute { points, at, length, loops, ..Default::default() }
    }

    /// The route through the network's segments, remembering them.
    pub fn with_segments(points: Vec<Vec2>, segments: Vec<usize>, loops: bool) -> RoadRoute {
        let mut segments = segments;
        if let Some(&first) = segments.first() {
            segments.push(first);
        }
        RoadRoute { segments, ..RoadRoute::new(points, loops) }
    }

    /// 0x5D5A10: the distance from `p` to the road pieces either side of
    /// node `n` within its segment (0x5D4E40), and the nearer end node of
    /// the nearer piece.
    fn piece_distance(&self, p: Vec2, n: usize) -> (f32, Option<usize>) {
        let seg = self.nodes[n].segment;
        let (first, end) = self.segment_nodes(seg);
        let mut best = (9_999_999.0, None);
        if n != first {
            let (d, k) = to_piece(self.nodes[n - 1].at, self.nodes[n].at, p);
            if d < 9_999_999.0 {
                best = (d, Some(n - 1 + k));
            }
        }
        if n + 1 == end || n + 1 >= self.nodes.len() {
            return best;
        }
        let (d, k) = to_piece(self.nodes[n].at, self.nodes[n + 1].at, p);
        if best.0 <= d { best } else { (d, Some(n + k)) }
    }

    fn segment_nodes(&self, seg: usize) -> (usize, usize) {
        self.piece_of.get(seg).copied().flatten().map_or((0, 0), |k| {
            let s = &self.splines[k];
            (s.first, s.first + s.cps.len() - 2)
        })
    }

    /// 0x5D57B0 over the route's segments: the node nearest `p` among the
    /// segments whose node bounds, widened by `radius`, hold it.
    fn nearest_on_route(&self, p: Vec2, radius: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (k, s) in self.splines.iter().enumerate() {
            if self.piece_of[s.segment] != Some(k) {
                continue;
            }
            let (lo, hi) = self.bounds[s.segment];
            if p.x < lo.x - radius || p.y < lo.y - radius || p.x > hi.x + radius || p.y > hi.y + radius {
                continue;
            }
            for j in s.first..s.first + s.cps.len() - 2 {
                let d = self.nodes[j].at.distance(p);
                if best.is_none_or(|b| d < b.1) {
                    best = Some((j, d));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// 0x5D5CE0 on node places: `a − b` the short way round a circuit.
    pub fn difference(&self, a: f32, b: f32) -> f32 {
        let (l, d) = (self.length, a - b);
        if !self.loops { d } else if d < -l * 0.5 { d + l } else if d > l * 0.5 { d - l } else { d }
    }

    /// The locator 0x5D9E40: keep the last node unless `p` is over 128 m
    /// from where it was last asked (else the nearest route node). Within
    /// half of the node's +0x0A of its pieces it stays; within half of
    /// +0x0B its links compete, farther a search within 128 m does: a
    /// node's cost is its pieces' distance + 1 + (|its place − the
    /// node's| − 40) / 4 past 40 m. The place on the road is then the
    /// nearest point of the node's spline (0x5774D0 from the node's
    /// parameter, or the last one for the same node, to 0.05 m).
    pub fn locate(&self, loc: &mut Locator, p: Vec2) {
        if self.nodes.is_empty() {
            loc.along = self.project(p, None, 0.0);
            loc.fraction = loc.along / self.length.max(1.0);
            return;
        }
        let mut node = loc.node;
        if node.is_none() || p.distance(loc.last) > 128.0 {
            node = self.nearest_on_route(p, 1e8);
        }
        let Some(mut n) = node else {
            // 0x5D9ECE: node, place and fraction 0, the direction +x; the
            // parameter (+0x1C) and threshold (+0x38) stay.
            *loc = Locator { last: p, point: p, direction: Vec2::X, t: loc.t, threshold: loc.threshold, ..Default::default() };
            return;
        };
        let (mut best, moved) = self.piece_distance(p, n);
        n = moved.unwrap_or(n);
        let here = self.nodes[n];
        let cost = |m: usize, d: f32, from: f32| d + 1.0 + ((self.difference(from, self.nodes[m].along).abs() - 40.0) * 0.25).max(0.0);
        if here.near != 0xFF {
            if best + best < here.far as f32 {
                if best + best < here.near as f32 {
                    // Close enough: keep it.
                } else {
                for l in here.links.into_iter().filter(|&l| l >= 0).map(|l| l as usize) {
                    if l >= self.nodes.len() || self.piece_of.get(self.nodes[l].segment).copied().flatten().is_none() {
                        continue;
                    }
                    let (d, m) = self.piece_distance(p, l);
                    let Some(m) = m else { continue };
                    if self.nodes[m].near == 0xFF {
                        continue;
                    }
                    let c = cost(m, d, self.nodes[n].along);
                    if c < best {
                        best = c;
                        n = m;
                    }
                }
                }
            } else if let Some(m) = self.nearest_on_route(p, 128.0).filter(|&m| self.nodes[m].near != 0xFF) {
                let (d, m2) = self.piece_distance(p, m);
                if let Some(m2) = m2 && cost(m2, d, self.nodes[n].along) < best {
                    n = m2;
                }
            }
        }
        let Some(k) = self.piece_of.get(self.nodes[n].segment).copied().flatten() else { return };
        let s = &self.splines[k];
        let t0 = if loc.node == Some(n) && loc.t != 0.0 { loc.t } else { (n - s.first + 1) as f32 };
        let t = s.nearest(p, t0, 0.05);
        let d = s.eval(t, true).truncate().truncate().normalize_or_zero();
        loc.node = Some(n);
        loc.t = t;
        loc.last = p;
        loc.point = s.eval(t, false).truncate().truncate();
        loc.direction = if s.scale < 0.0 { -d } else { d };
        loc.along = s.along(t);
        loc.fraction = loc.along / self.length.max(1.0);
    }

    /// 0x5DA340: locate `p` (0x5D9E40) once it is the locator's
    /// threshold (+0x38) or more from the last ask (+0x08), by |Δx| + |Δy|.
    pub fn relocate(&self, loc: &mut Locator, p: Vec2) {
        let d = p - loc.last;
        if loc.threshold <= d.x.abs() + d.y.abs() {
            self.locate(loc, p);
        }
    }

    /// A fresh locator's place for `p` (0x5DEDA0).
    pub fn locate_once(&self, p: Vec2) -> f32 {
        let mut loc = Locator::default();
        self.locate(&mut loc, p);
        loc.along
    }

    /// The road segment at a distance along the route (the piece's end
    /// node's), if known.
    pub fn segment_at(&self, along: f32) -> Option<usize> {
        if self.points.len() < 2 || self.at.len() < self.points.len() {
            return None;
        }
        let piece = self.at.partition_point(|&a| a <= along).clamp(1, self.points.len().max(2) - 1);
        self.segments.get(piece).copied()
    }

    /// The distance along the route of the point nearest `p`, preferring
    /// pieces within `window` m of `near` (when given) so crossing roads
    /// don't jump the progress.
    pub fn project(&self, p: Vec2, near: Option<f32>, window: f32) -> f32 {
        let mut best: Option<(f32, f32)> = None;
        for (i, w) in self.points.windows(2).enumerate() {
            if let Some(n) = near {
                let mut d = (self.at[i] - n).abs().min((self.at[i + 1] - n).abs());
                if self.loops {
                    d = d.min(self.length - d);
                }
                if d > window {
                    continue;
                }
            }
            let ab = w[1] - w[0];
            let t = if ab.length_squared() > 0.0 { ((p - w[0]).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let d = (w[0] + ab * t).distance(p);
            if best.is_none_or(|b| d < b.1) {
                best = Some((self.at[i] + t * (self.at[i + 1] - self.at[i]), d));
            }
        }
        match best {
            Some(b) => b.0,
            None if near.is_some() => self.project(p, None, window),
            None => 0.0,
        }
    }

    /// The point a distance along the route (wrapping on a loop).
    pub fn point_at(&self, along: f32) -> Vec2 {
        // Fewer than two points has no pieces.
        if self.points.len() < 2 || self.at.len() < self.points.len() {
            return self.points.first().copied().unwrap_or(Vec2::ZERO);
        }
        // (`max`/`min`, not `clamp`: a NaN length from NaN nodes must not panic.)
        let along = if self.loops && self.length > 0.0 { along.rem_euclid(self.length) } else { along.max(0.0).min(self.length) };
        let i = self.at.partition_point(|&a| a <= along).clamp(1, self.points.len().max(2) - 1);
        let span = (self.at[i] - self.at[i - 1]).max(1e-6);
        self.points[i - 1].lerp(self.points[i], ((along - self.at[i - 1]) / span).clamp(0.0, 1.0))
    }

    /// The route's direction at a distance along it.
    pub fn direction(&self, along: f32) -> Vec2 {
        if self.points.len() < 2 || self.at.len() < self.points.len() {
            return Vec2::ZERO;
        }
        let along = if self.loops && self.length > 0.0 { along.rem_euclid(self.length) } else { along };
        let i = self.at.partition_point(|&a| a <= along).clamp(1, self.points.len().max(2) - 1);
        (self.points[i] - self.points[i - 1]).normalize_or_zero()
    }
}

/// 0x5D4E40: the distance from `p` to the piece `a`–`b`, and which end
/// is nearer (1 for `b`); past either end, the distance to that end.
fn to_piece(a: Vec2, b: Vec2, p: Vec2) -> (f32, usize) {
    let ab = b - a;
    let along_a = ab.dot(p - a);
    let along_b = ab.dot(b - p);
    if along_a <= 0.0 {
        (p.distance(a), 0)
    } else if along_b <= 0.0 {
        (p.distance(b), 1)
    } else {
        ((ab.y * (p.x - a.x) - ab.x * (p.y - a.y)).abs() / ab.length(), (along_b < along_a) as usize)
    }
}
