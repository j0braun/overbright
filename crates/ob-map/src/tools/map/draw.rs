//! The faces of a map mod's drawn collision (docs/modding/map-editor.md
//! "Removed and drawn collision"): a drawn wall is a closed box, so it
//! blocks from either side and a car's sweep meets a face whatever its
//! step; drawn ground is its outline cut into triangles. The world writes
//! these faces into the collision packs as it writes moved scenery's
//! (`world::prop_collision`); the editor's preview draws the same ones.

use glam::{Vec2, Vec3};

use super::format::CollisionEdit;

/// A drawn wall's thickness, metres (the box's width across its foot).
pub const WALL_THICKNESS: f32 = 0.5;
/// How far a drawn wall's box reaches below its ends' heights, metres, so
/// a road that falls away across the box leaves no gap under it.
pub const WALL_FOOT: f32 = 0.5;

/// One face of drawn collision, world space: a quad's 4 corners in order,
/// or a triangle's 3 (its corner 3 repeating corner 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnFace {
    pub corners: [Vec3; 4],
    pub quad: bool,
    /// A wall (else a floor).
    pub wall: bool,
    pub surface: u8,
}

/// The faces `e` draws (none for a removal): a wall's four sides, each
/// foot then top, or a ground's triangles counter-clockwise from above
/// (as the original's floors, so their normal points up).
pub fn faces(e: &CollisionEdit) -> Vec<DrawnFace> {
    match e {
        CollisionEdit::Remove { .. } => Vec::new(),
        CollisionEdit::Wall { from, to, height, surface, .. } => wall_box(*from, *to, *height)
            .into_iter()
            .map(|corners| DrawnFace { corners, quad: true, wall: true, surface: *surface })
            .collect(),
        CollisionEdit::Ground { outline, surface, .. } => {
            let ccw = ccw(outline);
            triangulate(outline)
                .unwrap_or_default()
                .into_iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| ccw[i]);
                    DrawnFace { corners: [a, b, c, c], quad: false, wall: false, surface: *surface }
                })
                .collect()
        }
    }
}

/// A wall's four sides along `from` → `to`: the two long ones, then the
/// end at `from` and the end at `to`.
pub fn wall_box(from: Vec3, to: Vec3, height: f32) -> [[Vec3; 4]; 4] {
    let d = (to - from).truncate().normalize_or_zero();
    let n = Vec2::new(-d.y, d.x) * (WALL_THICKNESS * 0.5);
    let foot = |p: Vec3, side: Vec2| (p.truncate() + side).extend(p.z - WALL_FOOT);
    let up = Vec3::Z * (height + WALL_FOOT);
    let (fl, fr, tl, tr) = (foot(from, n), foot(from, -n), foot(to, n), foot(to, -n));
    [[tl, fl, fl + up, tl + up], [fr, tr, tr + up, fr + up], [fl, fr, fr + up, fl + up], [tr, tl, tl + up, tr + up]]
}

/// `outline` counter-clockwise from above.
fn ccw(outline: &[Vec3]) -> Vec<Vec3> {
    let mut v = outline.to_vec();
    if area(&v) < 0.0 {
        v.reverse();
    }
    v
}

fn area(p: &[Vec3]) -> f32 {
    (0..p.len()).map(|i| p[i].truncate().perp_dot(p[(i + 1) % p.len()].truncate())).sum::<f32>() * 0.5
}

/// The outline's triangles (indices into its counter-clockwise order, see
/// [`faces`]) by ear clipping; None when it crosses itself or has no area.
pub fn triangulate(outline: &[Vec3]) -> Option<Vec<[usize; 3]>> {
    let p: Vec<Vec2> = ccw(outline).iter().map(|v| v.truncate()).collect();
    let n = p.len();
    if n < 3 || area(&ccw(outline)) < 0.01 {
        return None;
    }
    // Edges that cross (not at a shared corner): not a simple outline.
    let cross = |a: Vec2, b: Vec2, c: Vec2, d: Vec2| {
        let o = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
        (o(a, b, c) * o(a, b, d) < 0.0) && (o(c, d, a) * o(c, d, b) < 0.0)
    };
    for i in 0..n {
        for j in i + 1..n {
            if (j + 1) % n == i || (i + 1) % n == j {
                continue;
            }
            if cross(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return None;
            }
        }
    }
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 {
        guard += 1;
        if guard > 4 * n * n {
            return None;
        }
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (a, b, c) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let turn = (p[b] - p[a]).perp_dot(p[c] - p[a]);
            if turn.abs() < 1e-6 {
                // A corner on a straight line: no triangle, just dropped.
                idx.remove(k);
                clipped = true;
                break;
            }
            if turn < 0.0 {
                continue;
            }
            let inside = |q: Vec2| {
                let s = |u: Vec2, v: Vec2| (v - u).perp_dot(q - u);
                s(p[a], p[b]) >= 0.0 && s(p[b], p[c]) >= 0.0 && s(p[c], p[a]) >= 0.0
            };
            if idx
                .iter()
                .any(|&o| o != a && o != b && o != c && p[o] != p[a] && p[o] != p[b] && p[o] != p[c] && inside(p[o]))
            {
                continue;
            }
            out.push([a, b, c]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            return None;
        }
    }
    let (a, b, c) = (idx[0], idx[1], idx[2]);
    if (p[b] - p[a]).perp_dot(p[c] - p[a]) > 1e-6 {
        out.push([a, b, c]);
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_is_a_closed_box_facing_out() {
        let e = CollisionEdit::Wall {
            id: "w".into(),
            region: "L4RA".into(),
            from: Vec3::ZERO,
            to: Vec3::new(10.0, 0.0, 0.0),
            height: 2.0,
            surface: 3,
        };
        let f = faces(&e);
        assert_eq!(f.len(), 4);
        let centre = Vec3::new(5.0, 0.0, 0.5);
        for face in &f {
            let c = face.corners;
            let normal = (c[1] - c[0]).cross(c[2] - c[0]);
            let mid = (c[0] + c[1] + c[2] + c[3]) / 4.0;
            assert!(normal.dot(mid - centre) > 0.0, "{c:?}");
            assert!(face.wall && face.quad && face.surface == 3);
            assert!((c[0].z + WALL_FOOT).abs() < 1e-6 && (c[2].z - 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn ground_cuts_into_upward_triangles() {
        // An L, given clockwise.
        let l = [(0.0, 0.0), (0.0, 10.0), (5.0, 10.0), (5.0, 5.0), (10.0, 5.0), (10.0, 0.0)]
            .map(|(x, y)| Vec3::new(x, y, 1.0));
        let e = CollisionEdit::Ground { id: "g".into(), region: "L4RA".into(), outline: l.to_vec(), surface: 0 };
        let f = faces(&e);
        assert_eq!(f.len(), 4);
        let mut a = 0.0;
        for t in &f {
            let c = t.corners;
            let z = (c[1] - c[0]).cross(c[2] - c[0]).z;
            assert!(z > 0.0);
            a += z * 0.5;
            assert!(!t.wall && !t.quad);
        }
        assert!((a - 75.0).abs() < 1e-3);
        let bow = [(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)].map(|(x, y)| Vec3::new(x, y, 0.0));
        assert!(triangulate(&bow).is_none());
    }
}
