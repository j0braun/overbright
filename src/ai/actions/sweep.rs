use super::*;

/// 0x428CE0 → 0x4275E0: the first road entry the car meets within
/// `look` seconds on `path`, and when. The entries tested are 0x403820's
/// (not the car, not wholly behind it: gap > −(both +0x0C), −0.5 m more
/// for the human); none is ignored by the controller's vt+0x84 (0x41BEA0:
/// the goal's slot +0x40, 0x50D510, names none). Each is swept by
/// [`swept`] (0x4275E0); the least time is kept. A path of no point (or
/// more than 20) meets nothing (0x428CE0's count test).
pub(super) fn sweep(path: &[Vec3], list: &[Entry], own: usize, me: &RoadCar, look: f32) -> Option<(usize, f32)> {
    if path.is_empty() {
        return None;
    }
    let mine = &list[own];
    let own_path = CarPath::from_points(path);
    let mut best: Option<(usize, f32)> = None;
    for (i, e) in list.iter().enumerate() {
        if i == own {
            continue;
        }
        let behind = if e.car.human { -0.5 } else { 0.0 };
        if gap(list, own, e) <= -(mine.along + e.along) + behind {
            continue;
        }
        if let Some(t) = swept(me, &own_path, &e.car, look)
            && best.is_none_or(|b| t < b.1)
        {
            best = Some((e.car.id, t));
        }
    }
    best
}

/// The AI tuning's +0x24 (0x81CB84, 0.02 in the shipped GLOBALB; squared
/// into 0x827A20 by 0x77A6F0): 0x404EF0's least distance to a path point
/// ahead.
pub(super) const PATH_AHEAD: f32 = 0.02;
/// 0x4275E0 sweeps a traffic car along its driver's path while its
/// controller's vt+0x2C (0x40D4F0: how far it is past its road's edge at
/// its route point; negative on the road) is at most this (0x785B6C),
/// else along the prediction (0x425A40).
pub const TRAFFIC_OWN_PATH_EDGE: f32 = -0.25;
/// 0x420B80: a traffic car's predicted path, one point per 0.1 s
/// (0x3DCCCCCD) stepped by its velocity and acceleration; 0x420A60's
/// straight line instead when |a| > 0.75 |v|, five points L / 5 apart.
pub(super) const PREDICT_STEP: f32 = 0.1;
pub(super) const PREDICT_LINE: f32 = 0.75;
/// 0x4275E0: a moving traffic car's boxes grown by 1.3 for a car that is
/// not traffic (car +0x24 ≠ 4), not under 2 m/s.
pub(super) const TRAFFIC_BOX: f32 = 1.3;
pub(super) const TRAFFIC_BOX_SPEED: f32 = 2.0;

/// An oriented box of 0x4040A0 (+0x10 / +0x20 its axes, +0x40 / +0x44
/// the half extents along them).
#[derive(Clone, Copy, Debug)]
pub(super) struct Obb {
    pub(super) u: Vec2,
    pub(super) w: Vec2,
    pub(super) half: Vec2,
}

/// 0x4040A0: a box per path point: the first along the car's heading, each
/// next along the segment from the point before (p[i−1] − p[i]); the half
/// extents are the car's (length along, width across) times `scale`.
pub(super) fn path_boxes(points: &[Vec2], heading: Vec2, half: Vec2, scale: f32) -> Vec<Obb> {
    let half = half.abs() * scale;
    let mk = |u: Vec2| Obb { u, w: Vec2::new(-u.y, u.x), half };
    let mut out = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        out.push(if i == 0 { mk(heading) } else { mk((points[i - 1] - points[i]).normalize_or_zero()) });
    }
    out
}

/// 0x404EF0: the first path point ahead of the car along its velocity and
/// more than 0.02 m away; the last when there is none.
pub(super) fn first_ahead(points: &[Vec2], at: Vec2, velocity: Vec2) -> usize {
    let dir = velocity.normalize_or_zero();
    points
        .iter()
        .position(|&p| {
            (p - at).normalize_or_zero().dot(dir) > 0.0 && (p - at).length_squared() > PATH_AHEAD * PATH_AHEAD
        })
        .unwrap_or(points.len().saturating_sub(1))
}

/// 0x4043F0: when the projection p + v·t lies within ±r.
pub(super) fn axis_interval(v: f32, p: f32, r: f32) -> (f32, f32) {
    if v.abs() >= 0.0001 {
        let exit = (r - p) * (1.0 / v);
        let enter = -((p + r) * (1.0 / v));
        if v < 0.0 { (exit, enter) } else { (enter, exit) }
    } else if r < p.abs() {
        (9999.0, 9999.0)
    } else {
        (-9999.0, 9999.0)
    }
}

/// 0x404240: two intervals' overlap (the second least of the four ends,
/// within both, to the third).
pub(super) fn overlap(a: (f32, f32), b: (f32, f32)) -> Option<(f32, f32)> {
    let mut v = [a.0, a.1, b.0, b.1];
    v.sort_by(f32::total_cmp);
    (a.0 <= v[1] && v[1] <= a.1 && b.0 <= v[1] && v[1] <= b.1).then_some((v[1], v[2]))
}

/// 0x4044C0: when box `b`, at `p` from box `a` and moving at `v`
/// relative to it, first touches it (the four axes' intervals, 0x4043F0,
/// overlapped by 0x404240): the entry time when positive, 0 when the boxes
/// already overlap and part later, else 99999.
pub(super) fn contact_time(a: &Obb, b: &Obb, p: Vec2, v: Vec2) -> f32 {
    let (uu, uw, wu, ww) = (a.u.dot(b.u), a.u.dot(b.w), a.w.dot(b.u), a.w.dot(b.w));
    let i1 = axis_interval(v.dot(a.w), p.dot(a.w), (ww * b.half.y).abs() + (wu * b.half.x).abs() + a.half.y.abs());
    let i2 = axis_interval(v.dot(a.u), p.dot(a.u), (uw * b.half.y).abs() + (uu * b.half.x).abs() + a.half.x.abs());
    let i3 = axis_interval(v.dot(b.w), p.dot(b.w), (ww * a.half.y).abs() + (uw * a.half.x).abs() + b.half.y.abs());
    let i4 = axis_interval(v.dot(b.u), p.dot(b.u), (wu * a.half.y).abs() + (uu * a.half.x).abs() + b.half.x.abs());
    let Some(r) = overlap(i1, i2).and_then(|r| overlap(r, i3)).and_then(|r| overlap(r, i4)) else { return 99_999.0 };
    if r.0 > 0.0 {
        r.0
    } else if r.0 < 0.0 && r.1 > 0.0 {
        0.0
    } else {
        99_999.0
    }
}

/// 0x404CD0 with its fifth argument set (0x420B80's appends): a point under
/// 1 m from the last is refused, and within 20 m one that turns back
/// against the last segment.
pub(super) fn append_point(points: &mut Vec<Vec2>, q: Vec2) {
    if points.len() >= PATH_MAX {
        return;
    }
    if let Some(&last) = points.last() {
        let d = (q - last).length();
        if d < 1.0 {
            return;
        }
        if d < 20.0 && points.len() >= 2 && (q - last).dot(last - points[points.len() - 2]) < 0.0 {
            return;
        }
    }
    points.push(q);
}

/// 0x420A60: five points `length` / 5 apart from the car along its
/// velocity (or its heading under 1 m/s), the first at the car (0x404CD0
/// without the turn test: points under 1 m apart are refused).
pub(super) fn line_points(at: Vec2, velocity: Vec2, heading: Vec2, length: f32) -> Vec<Vec2> {
    let dir = if velocity.length() >= 1.0 { velocity.normalize() } else { heading };
    let mut out: Vec<Vec2> = Vec::new();
    for k in 0..5 {
        let q = at + dir * (k as f32 * length * 0.2);
        if out.last().is_none_or(|&l| (q - l).length() >= 1.0) {
            out.push(q);
        }
    }
    out
}

/// The human controller's path below 10 m/s (0x42CAA0: car +0x42C < 10.0
/// → 0x425910 → 0x420A60, with or without a road node): 0x420A60's five
/// points from the car along its velocity (its heading under 1 m/s), the
/// length the reach 0x404910 = max(125 m, the human controller's vt+0x60
/// = 0x40A240 = 2.0 s (0x783C38) × car +0x430), **verified**; observed
/// under Wine: 5 points 25 m apart standing and up to ~8 m/s.
/// At 10 m/s and above the road path (0x4221F0) is
/// `route::HumanRoad::road_path` (it needs the human walk): None here.
pub fn human_path(position: Vec3, velocity: Vec3, forward: Vec2, speed: f32, forward_speed: f32) -> Option<CarPath> {
    if forward_speed >= HUMAN_LINE_SPEED {
        return None;
    }
    let length = REACH.max(HUMAN_REACH_TIME * speed);
    let line = line_points(position.truncate(), velocity.truncate(), forward, length);
    Some(CarPath::from_points(&line.iter().map(|q| q.extend(0.0)).collect::<Vec<_>>()))
}

/// 0x42CAA0's switch between the line and the road path (car +0x42C, m/s).
const HUMAN_LINE_SPEED: f32 = 10.0;
/// The human controller's reach time (vt+0x60 = 0x40A240: 2.0 s).
const HUMAN_REACH_TIME: f32 = 2.0;

/// 0x425A40 → 0x420B80: a traffic car's path for the sweep, from its place
/// (+0x60), velocity (+0x70) and acceleration (+0x80): 0x420A60's line
/// when |a| > 0.75 |v|, else a point every 0.1 s (v += a·0.1, p += v·0.1)
/// until its velocity turns against the first, the length is run or 20
/// points. The length is 0x60BF20: the controller's reach 0x404910,
/// max(125 m, speed × vt+0x60; the second term **open**, the clone takes
// prov: open ai.actions.predicted
/// 125 m).
pub(super) fn predicted(c: &RoadCar) -> Vec<Vec2> {
    let length = REACH;
    let (v0, a) = (c.velocity.truncate(), c.acceleration.truncate());
    if v0.length() * PREDICT_LINE < a.length() {
        return line_points(c.position.truncate(), v0, c.forward, length);
    }
    let (mut p, mut v) = (c.position.truncate(), v0);
    let mut out = Vec::new();
    let mut run = 0.0;
    for _ in 0..PATH_MAX {
        if out.len() >= PATH_MAX {
            break;
        }
        v += a * PREDICT_STEP;
        let q = p + v * PREDICT_STEP;
        append_point(&mut out, q);
        run += (p - q).length();
        p = q;
        if v.dot(v0) < 0.0 || length <= run {
            break;
        }
    }
    out
}

/// 0x4275E0: the swept test of the car along its path (`path`, at its speed
/// +0x430 grown by its acceleration 0x5EC3B0) against another car along its
/// own: a traffic car's predicted path (0x425A40, `predicted`), a racer's
/// driven path (`RoadCar::path`; the human's, which the clone does not
// prov: inferred ai.actions.swept
/// keep, is taken as a traffic car's prediction, **inferred**), at |v|
/// grown by its acceleration (0x60BE40). Boxes per path point (0x4040A0,
/// a moving traffic car's 1.3 times as large), both cars start at their
/// first point ahead (0x404EF0); each step runs until the first of the two
/// reaches its point (that car then aims at its next one), the boxes of the
/// points they aim at tested by 0x4044C0 at the cars' relative place and
/// velocity. The time of contact when it is under `look` (3 s).
/// **Not ported**: the paths of a car going backwards (0x425910 / 0x425980:
/// a straight line; taken only when its path is empty).
pub(super) fn swept(me: &RoadCar, own: &CarPath, other: &RoadCar, look: f32) -> Option<f32> {
    let a_pts = own.points();
    // 0x4275E0 copies the other car's path (0x60BE20: car +0x30 → +0x10)
    // and replaces it with the prediction (0x425A40) only for traffic (kind
    // 3) whose controller's vt+0x2C (0x40D4F0, the distance past its road's
    // edge) is above −0.25 (0x785B6C; 0x427677..0x4276C2): a traffic car on
    // its road is swept along its driver's path, which the caller gives it
    // only then ([`TRAFFIC_OWN_PATH_EDGE`]); one at or past the edge, or
    // without a route point (0 m), along the prediction. A racer's empty
    // path gives no contact (the loop runs `i < count`, observed under
    // Wine). The human's path is its controller's per-frame build
    // (0x42CAA0, [`human_path`]); above 10 m/s that is the road path
    // 0x4221F0 (`route::HumanRoad::road_path`, set by the game's caller;
    // without a human walk the clone falls back to the prediction).
    let b_pts: Vec<Vec2> = if (other.traffic || other.human) && other.path.n == 0 {
        predicted(other)
    } else {
        other.path.points().to_vec()
    };
    if a_pts.is_empty() || b_pts.is_empty() {
        return None;
    }
    let mut sb = other.velocity.length();
    let mut scale = if other.traffic && !me.traffic { TRAFFIC_BOX } else { 1.0 };
    if sb < TRAFFIC_BOX_SPEED {
        scale = 1.0;
    }
    let mut sa = me.velocity.length().max(0.001);
    sb = sb.max(0.001);
    let boxes_a = path_boxes(a_pts, me.forward, me.half, 1.0);
    let boxes_b = path_boxes(&b_pts, other.forward, other.half, scale);
    let (mut i, mut j) = (
        first_ahead(a_pts, me.position.truncate(), me.velocity.truncate()),
        first_ahead(&b_pts, other.position.truncate(), other.velocity.truncate()),
    );
    let (mut pa, mut pb) = (me.position.truncate(), other.position.truncate());
    let mut elapsed = 0.0f32;
    let mut found = 99_999.0f32;
    while i < a_pts.len() {
        if j >= b_pts.len() || elapsed > look {
            break;
        }
        let (da, db) = (a_pts[i] - pa, b_pts[j] - pb);
        let (la, lb) = (da.length(), db.length());
        let ta = la / sa;
        let dt = ta.min(lb / sb);
        let va = da.normalize_or_zero() * sa;
        let vb = db.normalize_or_zero() * sb;
        let t = contact_time(&boxes_a[i], &boxes_b[j], pb - pa, vb - va);
        if t <= dt && t >= 0.0 {
            found = t + elapsed;
            break;
        }
        if la > 1e-5 {
            pa += da * (dt * sa / la);
        }
        sa = (sa + dt * me.forward_accel).max(0.001);
        if lb > 1e-5 {
            pb += db * (dt * sb / lb);
        }
        sb = (sb + dt * other.accel).max(0.001);
        if (ta - dt).abs() > 0.001 {
            j += 1;
        } else {
            i += 1;
        }
        elapsed += dt;
    }
    (found < look && (found - 99_999.0).abs() > 0.001).then_some(found)
}

/// 0x428DF0: the path clear of the road entries for `look` seconds.
/// A path that could not be built (0x420D40 false; the clone: no point) is
/// not clear.
pub(super) fn clear(path: &[Vec3], list: &[Entry], own: usize, me: &RoadCar, look: f32) -> bool {
    !path.is_empty() && sweep(path, list, own, me, look).is_none()
}
