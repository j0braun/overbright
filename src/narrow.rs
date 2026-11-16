//! The game's convex distance query (0x5B33A0 with its core 0x595050), as
//! used between the car's hull and a track polygon.
//!
//! It keeps a small set of vertices from each shape (at most four each, by
//! index) and, between calls, remembers them (pair +0x118) to start from.
//! Each round it asks both shapes for their extreme vertex along the
//! current separating direction (support functions 0x5AFBB0 for hulls,
//! 0x5A2070 for polygons), adds any new one to its set, and measures the
//! two sets again (0x595050). It stops when no vertex is new, or when the
//! distance stops shrinking (going back to the previous sets, 0x598250).
//!
//! 0x595050 finds the closest features of the two sets' hulls by trying, in
//! order: point/point; point against each segment; each segment against
//! point; every segment pair; every triangle of the first set against the
//! second's lowest vertex; every triangle of the second against the
//! first's highest. A candidate is taken when the sets are separated along
//! its direction and the extreme vertices lie on the candidate; the sets
//! then shrink to its vertices. Otherwise the nearest separated candidate
//! is kept as a fallback. Nothing separated means the hulls overlap.
//!
//! Worked in single precision on the world coordinates, every sum and
//! product in the order the original's code takes them: the game runs the
//! x87 at 24-bit precision (formats.md "Floating point"), so near ±1500 m
//! its gaps come out in float steps of up to 0.1 mm, and whether a squared
//! gap clears the warm boundary 1e-5 turns on those steps.
//!
//! A support taken in a call lands off the shape (see [`Raw`]): a hull
//! whose shape carries an offset (+0x3E / +0x40, the body's centre of
//! mass) ranks its vertices with the offset removed, but the point 0x5AFBB0
//! writes is the raw vertex through the body's transform (0x5AFF2A: its
//! corrected point is overwritten), R·c from the true vertex. Points
//! rebuilt from kept indices at the next call's start (0x5AAD30 →
//! 0x5A1980) do remove it. So the query of the call in which a vertex is
//! new to the pair's set measures it R·c off; the next call has it right.
//!
//! The hull's support (0x5AFBB0) is a climb, not a scan: from the vertex
//! the side's last support (or rebuilt kept vertex) reported — vertex 0 on
//! a cold start (0x59FB60 clears the buffers, 0x5AAD30(0)) — it takes that
//! vertex when none of its links is strictly greater along the direction;
//! otherwise (hulls of over 16 points) it queues the start's greater links
//! and takes the first of them that is such a local maximum; failing all, a
//! scan takes the first of the greatest. The index it reports is the
//! vertex record's +0xC: 0 for a vertex without links, so such a support
//! enters the set as vertex 0 and is rebuilt as vertex 0 next call. The
//! direction is turned into the shape's frame by the pair's inverse
//! rotation (+0x160 / +0x1E0, 0x43C9D0's transpose, no translation), so the
//! ranking equals the world's. Each call draws one number from the shared
//! generator for its visit stamp (0x43C1C0(0xFFFF) + 0x43BDF0, 0x5AFBF1;
//! the cached dot +0x20 counts only within a call): not ported (the draw
//! feeds nothing else here; it does shift the shared generator, open).

use crate::formats::collision::HullLinks;
use glam::Vec3;

/// One side of a query: the world offset its fresh supports carry
/// (0x5AFBB0, see the module notes: R·c for a hull with a shape offset c —
/// a car's `_CV` hull: the car's centre of mass, 0x5AEC30 / 0x5B5060 through
/// the body's vtable +0x1C; a prop's `ZCS_` volume: its info's centre of
/// mass, 0x5B1690 / 0x5B5060 —, zero for a polygon, a box or a hull without
/// one) and the hull's vertex links its support climbs over (none: the
/// first of the greatest, as a polygon's 0x5A2070).
#[derive(Clone, Copy, Debug, Default)]
pub struct Side<'a> {
    pub offset: Vec3,
    pub links: Option<&'a HullLinks>,
    /// The hull in its own frame, when the caller has it ([`Body`]): the
    /// support then ranks and places its vertices as 0x5AFBB0 does.
    /// Without it the ranking is on the world points and a fresh support
    /// is the true point plus `offset`.
    pub body: Option<Body<'a>>,
}

/// A hull side in its own frame: the vertices as stored (the shape's
/// offset not removed), that offset, and the body's transform (rotation
/// rows as columns, the position: the centre of mass).
///
/// 0x5AFBB0 turns the direction into the shape's frame through the pair's
/// inverse rotation (+0x160, the transpose; [`body_point`]'s order, a zero
/// translation), ranks each vertex less the offset by ((z + y) + x) of its
/// products (the cached dot +0x20), and writes the stored vertex through
/// the body's transform (0x5AFF2A, [`body_point`]).
#[derive(Clone, Copy, Debug)]
pub struct Body<'a> {
    pub stored: &'a [Vec3],
    pub offset: Vec3,
    pub rot: glam::Mat3,
    pub at: Vec3,
}

impl<'a> Side<'a> {
    /// A polygon's or a box's side: no offset, a plain scan.
    pub const NONE: Side<'static> = Side { offset: Vec3::ZERO, links: None, body: None };

    /// A hull's side: its offset in the world and its links (a hull loaded
    /// without them scans).
    pub fn hull(offset: Vec3, links: &'a HullLinks) -> Side<'a> {
        Side { offset, links: (!links.list.is_empty() && links.index.len() == links.first.len()).then_some(links), body: None }
    }

    /// The same side with its hull in its own frame ([`Body`]).
    pub fn with_body(self, body: Body<'a>) -> Side<'a> {
        Side { body: Some(body), ..self }
    }
}

/// A shape point through its body's transform as the original works it
/// (0x43C9C0 → 0x441000, the pair's transform +0x120 / +0x1A0): per world
/// axis, x: ((row 2 · v.z + row 1 · v.y) + row 0 · v.x) + the translation;
/// y and z: ((row 2 · v.z + row 0 · v.x) + row 1 · v.y) + the translation;
/// in single precision. `rot`'s columns are the rows (the body's axes),
/// `at` the translation (the body's position: a car's or prop's centre of
/// mass). The kept vertices of a query are rebuilt this way from the shape's
/// vertex less its offset (0x5AAD30 → 0x5A1980, a float subtraction), so
/// which of two vertices lies farther along a direction can turn on this
/// rounding: a front edge square to a wall ties under another order of the
/// sums, and the query then takes the second as a fresh support, R·c off
/// ([`Raw`]).
pub fn body_point(rot: glam::Mat3, at: Vec3, v: Vec3) -> Vec3 {
    let (x, y, z) = (rot.x_axis, rot.y_axis, rot.z_axis);
    Vec3::new(
        ((z.x * v.z + y.x * v.y) + x.x * v.x) + at.x,
        ((z.y * v.z + x.y * v.x) + y.y * v.y) + at.y,
        ((z.z * v.z + x.z * v.x) + y.z * v.y) + at.z,
    )
}

/// Both sides of a query (A, B).
pub type Raw<'a> = [Side<'a>; 2];

/// No offset and a plain scan on either side.
pub const EXACT: Raw<'static> = [Side::NONE; 2];

/// The vertex sets remembered between calls for one hull/polygon pair.
#[derive(Clone, Debug, Default)]
pub struct PairCache {
    a: Vec<usize>,
    b: Vec<usize>,
    /// The last answer and the two bodies' positions it was measured at
    /// (pair +0x10 / +0x14 / +0x15 with +0x20 / +0x30, see [`distance_at`]).
    memo: Option<(Vec3, Vec3, (f32, Vec3, Vec3))>,
    /// The pair's closest points as the last query left them (+0xC0 /
    /// +0xD0): a warm query that overlaps at once leaves them.
    at: Option<(Vec3, Vec3)>,
}

/// A pair is measured again only when it is dirty (pair +0x220, 0x5B3D30):
/// when either body's position has moved by more than 1 mm in some
/// component since the last measurement (0x598630, 0x43D190 against 0.001;
/// the orientation is not compared), or a shape changed (0x5985A0).
/// Otherwise the last distance, touch and overlap answers stay, and with
/// them the closest points (their transforms +0x120 / +0x1A0 are updated
/// only when dirty). Every query of the collision step goes through it:
/// the detection (0x5B5060 at 0x5B51BB), the time of impact (0x5B4BF0 at
/// 0x5B4CED) and the push-out (0x5B4300 at 0x5B4635). `at_a` / `at_b`:
/// the bodies' positions (a car's centre of mass, the box centre of its
/// collision body; a track polygon's is fixed).
pub fn distance_at(a: &[Vec3], b: &[Vec3], at_a: Vec3, at_b: Vec3, raw: Raw, cache: &mut Option<PairCache>) -> (f32, Vec3, Vec3) {
    let near = |p: Vec3, q: Vec3| (p - q).abs().cmple(Vec3::splat(0.001)).all();
    if let Some((ma, mb, answer)) = cache.as_ref().and_then(|c| c.memo) {
        if near(at_a, ma) && near(at_b, mb) {
            return answer;
        }
    }
    let answer = distance(a, b, raw, cache);
    cache.get_or_insert_with(PairCache::default).memo = Some((at_a, at_b, answer));
    answer
}

impl PairCache {
    /// A pair history holding these kept vertex indices (A, B), as a
    /// logged query left them (replays and tests).
    pub fn warm(a: Vec<usize>, b: Vec<usize>) -> PairCache {
        PairCache { a, b, memo: None, at: None }
    }

    /// The same with the closest points the pair's last query left
    /// (+0xC0 / +0xD0), which a warm query that overlaps at once keeps.
    pub fn warm_at(a: Vec<usize>, b: Vec<usize>, at: Option<(Vec3, Vec3)>) -> PairCache {
        PairCache { a, b, memo: None, at }
    }

    /// The kept vertex indices of A and B (pair +0xF0 / +0x104).
    pub fn kept(&self) -> (&[usize], &[usize]) {
        (&self.a, &self.b)
    }

    /// The last measured distance (pair +0x10), for logs.
    pub fn last_distance(&self) -> Option<f32> {
        self.memo.map(|m| m.2 .0)
    }

    /// The last measured answer: the distance and the closest points (pair
    /// +0x10, +0xC0, +0xD0). 0x5AF0F0's point and normal (0x594010) take
    /// these as they stand: 0x5B2620 measures nothing between the time of
    /// impact's search (or the push-out) and the impulse.
    pub fn last(&self) -> Option<(f32, Vec3, Vec3)> {
        self.memo.map(|m| m.2)
    }
}

/// One set: (vertex index, position).
type Set = Vec<(usize, Vec3)>;

struct Closest {
    dist2: f32,
    on_a: Vec3,
    on_b: Vec3,
    /// The direction the next supports are taken along (A toward B).
    direction: Vec3,
}

// The query's arithmetic, in single precision and in the original's order
// of operations (the game runs the x87 at 24-bit precision, formats.md
// "Floating point", so each x87 operation rounds as an f32 one). A dot
// product's three terms are summed in the order the code at each place sums
// them; the names say which (`xzy`: (x + z) + y).

fn xzy(p: Vec3, v: Vec3) -> f32 {
    (p.x * v.x + p.z * v.z) + p.y * v.y
}
fn xyz(p: Vec3, v: Vec3) -> f32 {
    (p.x * v.x + p.y * v.y) + p.z * v.z
}
fn zyx(p: Vec3, v: Vec3) -> f32 {
    (p.z * v.z + p.y * v.y) + p.x * v.x
}
fn yzx(p: Vec3, v: Vec3) -> f32 {
    (p.y * v.y + p.z * v.z) + p.x * v.x
}
/// The squared length the candidates of 0x595050 report: (z² + y²) + x².
fn len2_zyx(v: Vec3) -> f32 {
    zyx(v, v)
}
/// a × b (0x43D1E0): each component one difference of two products.
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}
/// 0x43CF10: v / |v| with |v| = √((x² + y²) + z²) and one reciprocal;
/// (1, 0, 0) for a zero vector.
fn normalize(v: Vec3) -> Vec3 {
    let s = ((v.x * v.x + v.y * v.y) + v.z * v.z).sqrt();
    if s == 0.0 {
        return Vec3::X;
    }
    let inv = 1.0 / s;
    Vec3::new(inv * v.x, v.y * inv, v.z * inv)
}
/// The component of `w` across direction `d`, unnormalised: d × (w × d)
/// (two calls of 0x43D1E0).
fn across(w: Vec3, d: Vec3) -> Vec3 {
    cross(d, cross(w, d))
}
/// p + d·t (0x43D0C0 and the inline copies: (d·t) + p per component).
fn along(p: Vec3, d: Vec3, t: f32) -> Vec3 {
    Vec3::new(d.x * t + p.x, d.y * t + p.y, d.z * t + p.z)
}

/// Index of the greatest (first wins) projection of `set` on `v`: the
/// first point's dot in `first`'s order, the rest (x + z) + y.
fn arg_max(set: &Set, v: Vec3, first: fn(Vec3, Vec3) -> f32) -> (usize, f32) {
    let mut best = (0, first(set[0].1, v));
    for (i, p) in set.iter().enumerate().skip(1) {
        let x = xzy(p.1, v);
        if best.1 < x {
            best = (i, x);
        }
    }
    best
}

/// Index of the least (first wins) projection of `set` on `v`.
fn arg_min(set: &Set, v: Vec3, first: fn(Vec3, Vec3) -> f32) -> (usize, f32) {
    let mut best = (0, first(set[0].1, v));
    for (i, p) in set.iter().enumerate().skip(1) {
        let x = xzy(p.1, v);
        if x < best.1 {
            best = (i, x);
        }
    }
    best
}

/// Which vertices of a segment (i, j) a parameter keeps.
fn on_segment(i: usize, j: usize, t: f32) -> Vec<usize> {
    if t == 0.0 {
        vec![i]
    } else if t == 1.0 {
        vec![j]
    } else {
        vec![i, j]
    }
}

/// A candidate: closest points, direction, and the vertices it keeps.
struct Candidate {
    closest: Closest,
    keep_a: Vec<usize>,
    keep_b: Vec<usize>,
}

/// 0x595050: the closest features of the hulls of two vertex sets; the
/// sets shrink to them. `None` when the hulls overlap (the sets then stay
/// as they are, and so do the pair's closest points +0xC0 / +0xD0: no
/// candidate wrote them).
fn closest(set_a: &mut Set, set_b: &mut Set) -> Option<Closest> {
    let (na, nb) = (set_a.len(), set_b.len());
    let shrink = |set: &mut Set, keep: &[usize]| {
        let kept: Set = keep.iter().map(|&k| set[k]).collect();
        *set = kept;
    };
    let mut fallback: Option<Candidate> = None;
    // The nearest separated candidate so far (esp+0x68, from FLT_MAX): a
    // later one replaces it only when strictly nearer.
    let consider = |fallback: &mut Option<Candidate>, c: Candidate| {
        if fallback.as_ref().is_none_or(|f| c.closest.dist2 < f.closest.dist2) {
            *fallback = Some(c);
        }
    };

    if na == 1 && nb == 1 {
        let (x, y) = (set_a[0].1, set_b[0].1);
        let v = y - x;
        return Some(Closest { dist2: len2_zyx(v), on_a: x, on_b: y, direction: v });
    }

    if na == 1 {
        // The point against each segment of B (0x595137..0x59568B).
        let p = set_a[0].1;
        for i in 0..nb - 1 {
            let w = set_b[i].1 - p;
            for j in i + 1..nb {
                let (bi, bj) = (set_b[i].1, set_b[j].1);
                let d = bj - bi;
                let mut t = -xzy(d, w) / xzy(d, d);
                let v = if t < 0.0 {
                    t = 0.0;
                    bi - p
                } else if t > 1.0 {
                    t = 1.0;
                    bj - p
                } else {
                    across(w, d)
                };
                let pv = xzy(v, p);
                let (k, low) = arg_min(set_b, v, xyz);
                if pv < low {
                    let q = along(bi, d, t);
                    let c = Closest { dist2: len2_zyx(q - p), on_a: p, on_b: q, direction: v };
                    if k == i || k == j {
                        shrink(set_b, &on_segment(i, j, t));
                        return Some(c);
                    }
                    consider(&mut fallback, Candidate { closest: c, keep_a: vec![0], keep_b: on_segment(i, j, t) });
                }
            }
        }
    } else if nb == 1 {
        // Each segment of A against the point (0x59594F..0x595E71).
        let p = set_b[0].1;
        for i in 0..na - 1 {
            let w = p - set_a[i].1;
            for j in i + 1..na {
                let (ai, aj) = (set_a[i].1, set_a[j].1);
                let d = aj - ai;
                let mut t = xzy(d, w) / xzy(d, d);
                let v = if t < 0.0 {
                    t = 0.0;
                    p - ai
                } else if t > 1.0 {
                    t = 1.0;
                    p - aj
                } else {
                    across(w, d)
                };
                let (k, high) = arg_max(set_a, v, xzy);
                if high < zyx(p, v) {
                    let q = along(ai, d, t);
                    let c = Closest { dist2: len2_zyx(p - q), on_a: q, on_b: p, direction: v };
                    if k == i || k == j {
                        shrink(set_a, &on_segment(i, j, t));
                        return Some(c);
                    }
                    consider(&mut fallback, Candidate { closest: c, keep_a: on_segment(i, j, t), keep_b: vec![0] });
                }
            }
        }
    } else {
        // Every segment pair (0x595F64..0x596E1F).
        for i in 0..na - 1 {
            for j in i + 1..na {
                let (ai, aj) = (set_a[i].1, set_a[j].1);
                let d1 = aj - ai;
                let a = xzy(d1, d1);
                for k in 0..nb - 1 {
                    let bk = set_b[k].1;
                    let r = bk - ai;
                    let f = xzy(d1, r);
                    for l in k + 1..nb {
                        let bl = set_b[l].1;
                        let d2 = bl - bk;
                        let e = xzy(d2, d2);
                        let b = xzy(d1, d2);
                        let c = xzy(d2, r);
                        let denom = a * e - b * b;
                        let numer = f * e - b * c;
                        let mut s = if denom.abs() <= 1e-10 {
                            if numer * denom > 0.0 { 1.0 } else { 0.0 }
                        } else {
                            let s = numer / denom;
                            if s < 0.0 {
                                0.0
                            } else if s > 1.0 {
                                1.0
                            } else {
                                s
                            }
                        };
                        let mut u = (b * s - c) / e;
                        let v;
                        if u <= 0.0 || u >= 1.0 {
                            u = if u > 0.0 { 1.0 } else { 0.0 };
                            let s2 = (b * u + f) / a;
                            if s2 <= 0.0 || s2 >= 1.0 {
                                s = if s2 > 0.0 { 1.0 } else { 0.0 };
                                v = along(bk, d2, u) - along(ai, d1, s);
                            } else {
                                s = s2;
                                v = across(along(bk, d2, u) - ai, d1);
                            }
                        } else if s <= 0.0 || s >= 1.0 {
                            v = across(bk - along(ai, d1, s), d2);
                        } else {
                            // Both inside: the best of three axes by the gap
                            // between the segments' lowest ends.
                            let mut n = normalize(cross(d1, d2));
                            if (r.y * n.y + r.x * n.x) + r.z * n.z < 0.0 {
                                n = -n;
                            }
                            let one = across(bk - along(ai, d1, s), d2);
                            let two = across(along(bk, d2, u) - ai, d1);
                            let gap = |w: Vec3| yzx(bl, w).min(yzx(bk, w)) - yzx(aj, w).min(yzx(ai, w));
                            let mut best = (n, gap(n));
                            let g = gap(one);
                            if best.1 < g {
                                best = (one, g);
                            }
                            let g = gap(two);
                            if best.1 < g {
                                best = (two, g);
                            }
                            v = best.0;
                        }
                        let (ka, high) = arg_max(set_a, v, xzy);
                        let (kb, low) = arg_min(set_b, v, |p, v| (p.y * v.y + p.x * v.x) + p.z * v.z);
                        if high < low {
                            let (x, y) = (along(ai, d1, s), along(bk, d2, u));
                            let c = Closest { dist2: len2_zyx(y - x), on_a: x, on_b: y, direction: v };
                            let (keep_a, keep_b) = (on_segment(i, j, s), on_segment(k, l, u));
                            if (ka == i || ka == j) && (kb == k || kb == l) {
                                shrink(set_a, &keep_a);
                                shrink(set_b, &keep_b);
                                return Some(c);
                            }
                            consider(&mut fallback, Candidate { closest: c, keep_a, keep_b });
                        }
                    }
                }
            }
        }
    }

    // Whether p lies over the triangle: each edge's inward side by the
    // triangle's own normal (t1 − t0) × (t2 − t0), before the direction is
    // turned toward the other set (0x595050: the edge cross products, A's
    // triangles at 0x597245 / 0x597304 / 0x5973C3 and B's at 0x597B17 /
    // 0x597BD6 / 0x597C95, take the normalised face kept at esp+0xB0; the
    // turned copy is written only to the output direction, e.g.
    // 0x5977CD–0x5977FA).
    let inside = |n: Vec3, t: [Vec3; 3], p: Vec3| -> bool { (0..3).all(|e| zyx(cross(n, t[(e + 1) % 3] - t[e]), p - t[e]) > 0.0) };
    let fourth = |n: usize, i: usize, j: usize, k: usize| -> Option<usize> { (n == 4).then(|| 6 - i - j - k) };

    // Each triangle of A against B's lowest vertex (0x5956A8..0x597630).
    if na >= 3 {
        for i in 0..na - 2 {
            for j in i + 1..na - 1 {
                for k in j + 1..na {
                    let t = [set_a[i].1, set_a[j].1, set_a[k].1];
                    let face = normalize(cross(t[1] - t[0], t[2] - t[0]));
                    let n = if zyx(set_b[0].1 - t[0], face) < 0.0 { -face } else { face };
                    let plane = (n.y * t[0].y + t[0].x * n.x) + t[0].z * n.z;
                    let (mut ka, mut high) = (i, plane);
                    if let Some(m) = fourth(na, i, j, k) {
                        let x = xzy(set_a[m].1, n);
                        if plane < x {
                            ka = m;
                            high = x;
                        }
                    }
                    let (kb, low) = arg_min(set_b, n, zyx);
                    if high < low && inside(face, t, set_b[kb].1) {
                        let p = set_b[kb].1;
                        let dist = low - plane;
                        let nd = n * dist;
                        let c = Closest { dist2: len2_zyx(nd), on_a: p - nd, on_b: p, direction: n };
                        if ka == i {
                            shrink(set_a, &[i, j, k]);
                            shrink(set_b, &[kb]);
                            return Some(c);
                        }
                        consider(&mut fallback, Candidate { closest: c, keep_a: vec![i, j, k], keep_b: vec![kb] });
                    }
                }
            }
        }
    }
    // Each triangle of B against A's highest vertex (0x597636..0x597EFF).
    if nb >= 3 {
        for i in 0..nb - 2 {
            for j in i + 1..nb - 1 {
                for k in j + 1..nb {
                    let t = [set_b[i].1, set_b[j].1, set_b[k].1];
                    let face = normalize(cross(t[1] - t[0], t[2] - t[0]));
                    let n = if zyx(t[0] - set_a[0].1, face) < 0.0 { -face } else { face };
                    let plane = zyx(t[0], n);
                    let (mut kb, mut low) = (i, plane);
                    if let Some(m) = fourth(nb, i, j, k) {
                        let x = xzy(set_b[m].1, n);
                        if x < plane {
                            kb = m;
                            low = x;
                        }
                    }
                    let (ka, high) = arg_max(set_a, n, xzy);
                    if high < low && inside(face, t, set_a[ka].1) {
                        let p = set_a[ka].1;
                        let dist = plane - high;
                        let nd = n * dist;
                        let c = Closest { dist2: len2_zyx(nd), on_a: p, on_b: nd + p, direction: n };
                        if kb == i {
                            shrink(set_a, &[ka]);
                            shrink(set_b, &[i, j, k]);
                            return Some(c);
                        }
                        consider(&mut fallback, Candidate { closest: c, keep_a: vec![ka], keep_b: vec![i, j, k] });
                    }
                }
            }
        }
    }

    let f = fallback?;
    shrink(set_a, &f.keep_a);
    shrink(set_b, &f.keep_b);
    Some(f.closest)
}

/// Distance between hull `a` and polygon `b` (0x5B33A0): the distance and
/// the closest point on each; 0 when they overlap. `a` and `b` are the
/// shapes' true points in the world; `raw` the points of their fresh
/// supports. Worked in single precision on the world coordinates in the
/// original's order of operations (its gaps near ±1500 m come out in
/// float steps of 0.1 mm, and the warm / cold test of a squared gap
/// against 1e-5 turns on them).
pub fn distance(a: &[Vec3], b: &[Vec3], raw: Raw, cache: &mut Option<PairCache>) -> (f32, Vec3, Vec3) {
    let shapes = [Shape { points: a, side: raw[0] }, Shape { points: b, side: raw[1] }];
    query(&shapes, cache)
}

/// One side as the query works on it: its true points and its side.
struct Shape<'a> {
    points: &'a [Vec3],
    side: Side<'a>,
}

impl Shape<'_> {
    /// The point a support taken in this call writes for vertex `k`
    /// (0x5AFBB0: the raw vertex through the body's transform, 0x5AFF2A).
    fn fresh(&self, k: usize) -> Vec3 {
        match self.side.body {
            Some(b) => body_point(b.rot, b.at, b.stored[k]),
            None => self.points[k] + self.side.offset,
        }
    }

    /// The ranking value of each vertex along `dir` (0x5AFBB0's +0x20, or
    /// the world point's (x + z) + y without a [`Body`]).
    fn ranks(&self, dir: Vec3) -> Vec<f32> {
        match self.side.body {
            Some(b) => {
                // The transpose's rows are the rotation's columns.
                let (r0, r1, r2) = (b.rot.x_axis, b.rot.y_axis, b.rot.z_axis);
                let d = Vec3::new(
                    ((r0.z * dir.z + r0.y * dir.y) + r0.x * dir.x) + 0.0,
                    ((r1.z * dir.z + r1.x * dir.x) + r1.y * dir.y) + 0.0,
                    ((r2.z * dir.z + r2.x * dir.x) + r2.y * dir.y) + 0.0,
                );
                b.stored.iter().map(|v| zyx(*v - b.offset, d)).collect()
            }
            None => self.points.iter().map(|p| xzy(*p, dir)).collect(),
        }
    }

    /// 0x5AFBB0 (0x5A2070 for a polygon): the support along `dir` from the
    /// vertex `start` last reported: (the vertex, the index it reports).
    fn support(&self, dir: Vec3, start: usize) -> (usize, usize) {
        let Some(links) = self.side.links else {
            let k = support(self.points, dir);
            return (k, k);
        };
        let n = self.points.len();
        let rank = self.ranks(dir);
        let dot = |k: usize| rank[k];
        let mut seen = vec![false; n];
        let mut queue = std::collections::VecDeque::from([start.min(n - 1)]);
        // Only the first vertex queues its greater links (+0x14 > 16; then
        // the flag takes 0x86B2D8, which nothing writes).
        let mut more = n > 16;
        let mut found = None;
        while let Some(v) = queue.pop_front() {
            seen[v] = true;
            let near = links.of(v);
            if near.is_empty() {
                continue;
            }
            let mut top = true;
            for &m in near {
                let m = (m as usize).min(n - 1);
                if dot(v) < dot(m) {
                    top = false;
                    if more && !seen[m] {
                        queue.push_back(m);
                    }
                }
                seen[m] = true;
            }
            more = false;
            if top {
                found = Some(v);
                break;
            }
        }
        let k = found.unwrap_or_else(|| first_greatest(&rank));
        (k, links.index.get(k).map_or(k, |&i| (i as usize).min(n - 1)))
    }
}

/// The extreme vertex along `dir`: the first of the greatest (0x5A2070:
/// each point's (x + z) + y).
fn support(points: &[Vec3], dir: Vec3) -> usize {
    first_greatest(&points.iter().map(|p| xzy(*p, dir)).collect::<Vec<_>>())
}

/// The first of the greatest values.
fn first_greatest(values: &[f32]) -> usize {
    let mut best = 0;
    for (i, v) in values.iter().enumerate().skip(1) {
        if *v > values[best] {
            best = i;
        }
    }
    best
}

/// The cold / warm boundary of the squared gap (0x7A1934: 1e-5 as a float).
const WARM: f32 = 1e-5;

fn query(shapes: &[Shape; 2], cache: &mut Option<PairCache>) -> (f32, Vec3, Vec3) {
    let (a, b) = (shapes[0].points, shapes[1].points);
    // The vertex each side's support starts its climb from: the last one
    // reported (vertex 0 when cold), or the last kept one rebuilt.
    let (mut start_a, mut start_b): (usize, usize);
    let mut set_a: Set;
    let mut set_b: Set;
    let mut last: Closest;
    // The cache is made against these hulls: its owner drops it when a
    // hull changes (see `smackable::Props::sync_cars`), so its indices are
    // in range.
    // The pair's vertices count only once kept (+0x118 set, 0x5B33A0):
    // a first query whose supports overlap at once returns 0 with +0x118
    // still clear, so the next starts afresh. `distance_at` keeps such a
    // pair's answer with no vertices; taken as remembered, its empty sets
    // wrapped `0..n - 1` into a loop that never ended (the playthrough
    // bot's hang at S2_DRAG_2's start).
    // Kept vertices are rebuilt true (0x5AAD30 → 0x5A1980 less the shape
    // offset); every support taken below is its side's raw point
    // (0x5AFBB0), the cold start's first two included.
    let keep = |cache: &mut Option<PairCache>, set_a: &Set, set_b: &Set, at: (Vec3, Vec3)| {
        let memo = cache.as_ref().and_then(|c| c.memo);
        *cache = Some(PairCache { a: set_a.iter().map(|p| p.0).collect(), b: set_b.iter().map(|p| p.0).collect(), memo, at: Some(at) });
    };
    match cache.as_mut().filter(|c| !c.a.is_empty() && !c.b.is_empty()) {
        Some(c) => {
            (start_a, start_b) = (*c.a.last().unwrap(), *c.b.last().unwrap());
            set_a = c.a.iter().map(|&i| (i, a[i])).collect();
            set_b = c.b.iter().map(|&i| (i, b[i])).collect();
            match closest(&mut set_a, &mut set_b) {
                Some(r) if r.dist2 > WARM => last = r,
                found => {
                    // An overlap leaves the pair's closest points +0xC0 /
                    // +0xD0 as the last call left them.
                    let (x, y) = found.map_or(c.at.unwrap_or((Vec3::ZERO, Vec3::ZERO)), |r| (r.on_a, r.on_b));
                    c.a = set_a.iter().map(|p| p.0).collect();
                    c.b = set_b.iter().map(|p| p.0).collect();
                    c.at = Some((x, y));
                    return (0.0, x, y);
                }
            }
        }
        None => {
            // Along vertex 0 to vertex 0, both true (0x5AAD30(0)).
            let dir = b[0] - a[0];
            let (ka, ia) = shapes[0].support(dir, 0);
            let (kb, ib) = shapes[1].support(-dir, 0);
            (start_a, start_b) = (ia, ib);
            let (x, y) = (shapes[0].fresh(ka), shapes[1].fresh(kb));
            let v = y - x;
            let dist2 = xzy(v, v);
            if dist2 <= WARM {
                // +0x118 stays clear: the next call is cold again.
                if let Some(c) = cache.as_mut() {
                    c.a.clear();
                    c.b.clear();
                    c.at = Some((x, y));
                }
                return (0.0, x, y);
            }
            set_a = vec![(ia, x)];
            set_b = vec![(ib, y)];
            last = Closest { dist2, on_a: x, on_b: y, direction: v };
        }
    }

    let result = loop {
        let saved = (set_a.clone(), set_b.clone());
        let (ka, ia) = shapes[0].support(last.direction, start_a);
        let (kb, ib) = shapes[1].support(-last.direction, start_b);
        (start_a, start_b) = (ia, ib);
        let new_a = !set_a.iter().any(|p| p.0 == ia) && set_a.len() < 4;
        let new_b = !set_b.iter().any(|p| p.0 == ib) && set_b.len() < 4;
        if !new_a && !new_b {
            break (last.dist2.sqrt(), last.on_a, last.on_b);
        }
        if new_a {
            set_a.push((ia, shapes[0].fresh(ka)));
        }
        if new_b {
            set_b.push((ib, shapes[1].fresh(kb)));
        }
        let Some(next) = closest(&mut set_a, &mut set_b) else {
            break (0.0, last.on_a, last.on_b);
        };
        if next.dist2 == 0.0 {
            break (0.0, next.on_a, next.on_b);
        }
        if last.dist2 <= next.dist2 {
            (set_a, set_b) = saved;
            break (last.dist2.sqrt(), last.on_a, last.on_b);
        }
        last = next;
    };
    keep(cache, &set_a, &set_b, (result.1, result.2));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(c: Vec3) -> Vec<Vec3> {
        (0..8).map(|i| c + Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32) - 0.5).collect()
    }

    #[test]
    fn matches_exact_distances() {
        let mut cache = None;
        let (d, pa, pb) = distance(&cube(Vec3::ZERO), &cube(Vec3::new(3.0, 0.2, 0.0)), EXACT, &mut cache);
        assert!((d - 2.0).abs() < 1e-4, "{d}");
        assert!((pa.x - 0.5).abs() < 1e-4 && (pb.x - 2.5).abs() < 1e-4);
        // Warm-started again from the remembered vertices.
        let (d, _, _) = distance(&cube(Vec3::ZERO), &cube(Vec3::new(3.0, 0.3, 0.1)), EXACT, &mut cache);
        assert!((d - 2.0).abs() < 1e-4, "{d}");
        let mut cache = None;
        assert_eq!(distance(&cube(Vec3::ZERO), &cube(Vec3::new(0.5, 0.5, 0.0)), EXACT, &mut cache).0, 0.0);
        // A point over a triangle's face; a cube edge over a quad.
        let tri = [Vec3::ZERO, Vec3::X * 4.0, Vec3::Y * 4.0];
        let (d, _, pb) = distance(&[Vec3::new(1.0, 1.0, 2.0)], &tri, EXACT, &mut None);
        assert!((d - 2.0).abs() < 1e-4 && (pb - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-4, "{d} {pb}");
        let quad = [Vec3::new(-5.0, -5.0, -1.0), Vec3::new(5.0, -5.0, -1.0), Vec3::new(5.0, 5.0, -1.0), Vec3::new(-5.0, 5.0, -1.0)];
        let (d, _, _) = distance(&cube(Vec3::ZERO), &quad, EXACT, &mut None);
        assert!((d - 0.5).abs() < 1e-4, "{d}");
    }

    /// A pair whose first query overlapped at once, then moved apart: the
    /// next query measures it (an empty remembered set looped for good).
    #[test]
    fn overlap_at_first_then_apart() {
        let mut cache = None;
        let (d, _, _) = distance_at(&cube(Vec3::ZERO), &cube(Vec3::new(0.5, 0.5, 0.0)), Vec3::ZERO, Vec3::new(0.5, 0.5, 0.0), EXACT, &mut cache);
        assert_eq!(d, 0.0);
        let (d, _, _) = distance_at(&cube(Vec3::ZERO), &cube(Vec3::new(3.0, 0.2, 0.0)), Vec3::ZERO, Vec3::new(3.0, 0.2, 0.0), EXACT, &mut cache);
        assert!((d - 2.0).abs() < 1e-4, "{d}");
        let mut empty = Some(PairCache::default());
        let (d, _, _) = distance(&cube(Vec3::ZERO), &cube(Vec3::new(3.0, 0.2, 0.0)), EXACT, &mut empty);
        assert!((d - 2.0).abs() < 1e-4, "{d}");
    }

    #[test]
    fn segment_over_a_polygon_from_remembered_vertices() {
        // A hull edge 0.4 m over a floor, warm-started from an edge of each
        // (as a car riding over a road face keeps them): the floor's
        // triangle turns its direction toward the hull, and its inside test
        // still takes the face's own winding (0x595050), so the two are
        // apart, not overlapping.
        let floor = [Vec3::new(4.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO];
        let edge = [Vec3::new(1.0, 1.0, 0.4), Vec3::new(1.2, 1.3, 0.45)];
        for floor in [floor, [floor[1], floor[0], floor[2]]] {
            let mut cache = Some(PairCache { a: vec![0, 1], b: vec![0, 2], memo: None, at: None });
            let (d, _, _) = distance(&edge, &floor, EXACT, &mut cache);
            assert!((d - 0.4).abs() < 1e-4, "{d}");
        }
    }

    /// 0x5AFBB0 / 0x5A1980 (docs/prop-hits.md §7): a support taken in a
    /// call lands its side's offset off the shape, a vertex kept from an
    /// earlier call is rebuilt true. Two turned cubes (one closest vertex
    /// each): the pair's first query sees A 0.3 m nearer along the gap;
    /// the next, warm-started from the same vertices and finding none new,
    /// sees the true distance. An offset across the gap leaves it.
    #[test]
    fn fresh_supports_carry_the_offset_kept_ones_do_not() {
        let turn = |q: glam::Quat, c: Vec3| -> Vec<Vec3> { cube(Vec3::ZERO).iter().map(|p| c + q * *p).collect() };
        let a = turn(glam::Quat::from_euler(glam::EulerRot::ZYX, 0.3, 0.2, 0.1), Vec3::ZERO);
        let b = turn(glam::Quat::from_euler(glam::EulerRot::ZYX, -0.4, 0.1, 0.25), Vec3::new(3.0, 0.2, 0.1));
        let mut exact_cache = None;
        let (exact, x, y) = distance(&a, &b, EXACT, &mut exact_cache);
        let gap = (y - x).normalize();
        let raw = [Side { offset: gap * 0.3, links: None, body: None }, Side::NONE];
        let mut cache = None;
        let first = distance(&a, &b, raw, &mut cache).0;
        assert!((first - (exact - 0.3)).abs() < 1e-3, "{first} vs {exact}");
        let again = distance(&a, &b, raw, &mut cache).0;
        assert!((again - exact).abs() < 1e-4, "{again} vs {exact}");
        let side = gap.cross(Vec3::Z).normalize() * 0.3;
        let across = distance(&a, &b, [Side { offset: side, links: None, body: None }, Side::NONE], &mut None).0;
        assert!(across >= exact - 1e-4 && across < exact + 0.05, "{across} vs {exact}");
    }

    /// 0x598630 / 0x5B3D30: a pair is measured again only once a body has
    /// moved by more than 1 mm in some component; a turn on the spot keeps
    /// the old answer.
    #[test]
    fn pair_kept_until_a_body_moves_a_millimetre() {
        let mut cache = None;
        let wall = cube(Vec3::new(3.0, 0.0, 0.0));
        let d0 = distance_at(&cube(Vec3::ZERO), &wall, Vec3::ZERO, Vec3::ZERO, EXACT, &mut cache).0;
        assert!((d0 - 2.0).abs() < 1e-4);
        // 0.9 mm on: the last answer stays.
        let at = Vec3::new(0.0009, 0.0, 0.0);
        assert_eq!(distance_at(&cube(at), &wall, at, Vec3::ZERO, EXACT, &mut cache).0, d0);
        // A turn with the position held: still the last answer.
        let turned: Vec<Vec3> = cube(Vec3::ZERO).iter().map(|p| glam::Quat::from_rotation_z(0.3) * *p).collect();
        assert_eq!(distance_at(&turned, &wall, Vec3::ZERO, Vec3::ZERO, EXACT, &mut cache).0, d0);
        // 1.1 mm: measured again.
        let at = Vec3::new(0.0011, 0.0, 0.0);
        let d = distance_at(&cube(at), &wall, at, Vec3::ZERO, EXACT, &mut cache).0;
        assert!((d - (2.0 - 0.0011)).abs() < 1e-5, "{d}");
    }
}
