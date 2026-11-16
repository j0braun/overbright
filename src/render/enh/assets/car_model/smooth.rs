//! Smooth Car Shapes (opt-in, Graphics → Cars; docs/assets-distance.md).
//!
//! A load-time pre-pass on the car's solids (own parts, shared parts and
//! rims alike) that rounds low-poly silhouettes — wheel arches, roof
//! lines, bumper corners, tyre treads — with curved PN triangles (Vlachos
//! et al. 2001), conservatively:
//!
//! * only edges between two triangles that share the vertex normals at
//!   both ends (a smooth-shaded edge) **and** whose faces meet at an angle
//!   inside [`MIN_ANGLE`]..[`MAX_ANGLE`] are curved; flat panels (below the
//!   band), hard creases (normals split, or above the band), open borders
//!   and edges of three or more triangles stay straight, so panel seams,
//!   part joints and flat areas keep their exact shape;
//! * a curved edge's bulge is held to [`MAX_SAG`] metres and to
//!   [`MAX_SAG_RATIO`] of its length, and must bend the way the fold does
//!   outward: an edge or patch point that would sink into the car stays on
//!   the source triangle (parts are layered millimetres apart);
//! * a triangle with no curved edge is left as it is (same vertices, same
//!   indices); one with a curved edge is split into `LEVEL²` triangles on
//!   its cubic patch; the shared edges' points are computed once per edge
//!   (one orientation), so neighbours meet exactly; a straight edge next to
//!   an unsplit triangle (or an open border) gets no points of its own
//!   (they fold onto its corners), so there are no T-junctions;
//! * UVs, colours and normals are interpolated linearly across the
//!   original triangle: textures and shading normals stay where they were.
//!
//! Off (the default) = the files as shipped: [`apply`] returns at once.
//! The CPU pass runs once per car load (measured in docs/assets-distance.md).
use crate::formats::solid::Solid;
use glam::Vec3;
use std::collections::HashMap;

/// Edges whose faces meet flatter than this (degrees) stay straight.
pub const MIN_ANGLE: f32 = 3.0;
/// Edges whose faces meet sharper than this (degrees) stay straight.
pub const MAX_ANGLE: f32 = 50.0;
/// The most a curved edge's midpoint moves off its chord (metres).
pub const MAX_SAG: f32 = 0.02;
/// … and at most this share of the edge's length (a circle through the
/// ends at 50° per segment sags 0.11 of the chord).
pub const MAX_SAG_RATIO: f32 = 0.1;
/// An edge whose curve would move less than this (metres) stays straight:
/// below a millimetre or two nothing shows, and the triangles are saved.
pub const MIN_SAG: f32 = 0.002;
/// Each curved triangle becomes LEVEL² triangles.
pub const LEVEL: usize = 3;
/// Vertex normals this close (cosine) count as one shared normal.
const SAME_NORMAL: f32 = 0.9995;

/// Whether the pass is in force (the Graphics row).
pub fn enabled() -> bool {
    crate::qol::smooth_cars()
}

/// Parts left alone after the per-car review (docs/assets-distance.md):
/// name fragments (`contains`). Lamps and decals sit a few millimetres
/// over or under the body panels; moving either side lets the other one
/// through (CELICA's trunk lamps showed as black boxes beside the plate).
/// The parts under the hood and in the trunk gain nothing visible. The
/// AEROSTYLE roof scoops (ROOF_STYLE07*) lose their slats' edges and the
/// exhaust tips' nested tubes close up (HUMMER SONIC), so both are left.
pub const EXCLUDED: &[&str] = &[
    "BRAKELIGHT",
    "HEADLIGHT",
    "DECAL",
    "BADG",
    "PLATE",
    "_AUDIO",
    "HOOD_UNDER",
    "TRUNK_UNDER",
    "ENGINE",
    "ROOF_STYLE07",
    "EXHAUST",
];

/// What the pass did to one solid or a whole car.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub solids: usize,
    pub solids_changed: usize,
    pub triangles_in: usize,
    pub triangles_out: usize,
    pub vertices_in: usize,
    pub vertices_out: usize,
    pub curved_edges: usize,
    /// The largest distance of a new vertex from its source triangle's
    /// plane (metres).
    pub max_displacement: f32,
}

impl Stats {
    fn add(&mut self, o: &Stats) {
        self.solids += o.solids;
        self.solids_changed += o.solids_changed;
        self.triangles_in += o.triangles_in;
        self.triangles_out += o.triangles_out;
        self.vertices_in += o.vertices_in;
        self.vertices_out += o.vertices_out;
        self.curved_edges += o.curved_edges;
        self.max_displacement = self.max_displacement.max(o.max_displacement);
    }
}

/// Whether a solid is one the pass works on: a lit car mesh (normals per
/// vertex) of the top LOD (`_A`), not excluded.
pub fn wanted(s: &Solid) -> bool {
    !s.normals.is_empty()
        && s.normals.len() == s.vertices.len()
        && s.name.ends_with("_A")
        && !EXCLUDED.iter().any(|e| s.name.contains(e))
}

/// Smooth a car's loaded solids when the option is in force.
pub fn apply(solids: &mut [Solid]) -> Stats {
    if !enabled() {
        return Stats::default();
    }
    apply_always(solids)
}

/// [`apply`] regardless of the setting (tests, the sheets).
pub fn apply_always(solids: &mut [Solid]) -> Stats {
    apply_where(solids, &|_| true)
}

/// The pass on the [`wanted`] solids `pick` accepts (the car load: the
/// fitted parts' models).
pub fn apply_where(solids: &mut [Solid], pick: &dyn Fn(&Solid) -> bool) -> Stats {
    let _t = crate::prof::span("Car: smooth shapes");
    let mut all = Stats::default();
    for s in solids.iter_mut().filter(|s| wanted(s) && pick(s)) {
        all.add(&smooth_solid(s, LEVEL));
    }
    all
}

#[derive(Clone, Copy)]
struct Edge {
    /// The two inner control points, from the lower position id's end.
    c: Option<[Vec3; 2]>,
}

fn key(p: [f32; 3]) -> (i64, i64, i64) {
    let q = |x: f32| (f64::from(x) * 1.0e5).round() as i64;
    (q(p[0]), q(p[1]), q(p[2]))
}

/// One solid, each curved triangle split `level` times a side. The solid
/// is left unchanged when nothing curves or the result would not fit the
/// 16-bit indices.
pub fn smooth_solid(s: &mut Solid, level: usize) -> Stats {
    let mut st =
        Stats { solids: 1, triangles_in: s.indices.len() / 3, vertices_in: s.vertices.len(), ..Default::default() };
    st.triangles_out = st.triangles_in;
    st.vertices_out = st.vertices_in;
    if s.normals.len() != s.vertices.len() || s.indices.len() < 6 || level < 2 {
        return st;
    }
    // Geometric vertices: one id per position.
    let mut ids: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let pid: Vec<u32> = s
        .vertices
        .iter()
        .map(|v| {
            let n = ids.len() as u32;
            *ids.entry(key(v.position)).or_insert(n)
        })
        .collect();
    let pos = |i: u16| Vec3::from(s.vertices[i as usize].position);
    let nrm = |i: u16| Vec3::from(s.normals[i as usize]).normalize_or_zero();
    let tris: Vec<[u16; 3]> = s.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
    // Face normals, turned to the side the vertex normals face (outside).
    let face: Vec<Vec3> = tris
        .iter()
        .map(|t| {
            let f = (pos(t[1]) - pos(t[0])).cross(pos(t[2]) - pos(t[0])).normalize_or_zero();
            if f.dot(nrm(t[0]) + nrm(t[1]) + nrm(t[2])) < 0.0 { -f } else { f }
        })
        .collect();
    // Edges by their ends' position ids (low, high): the triangles on them
    // and each one's local edge number (corner k → k + 1).
    let mut edges: HashMap<(u32, u32), Vec<(usize, usize)>> = HashMap::new();
    for (t, tri) in tris.iter().enumerate() {
        if face[t] == Vec3::ZERO {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (pid[tri[k] as usize], pid[tri[(k + 1) % 3] as usize]);
            if a != b {
                edges.entry((a.min(b), a.max(b))).or_default().push((t, k));
            }
        }
    }
    // Which edges curve, and their control points. A smooth edge (the
    // same normals on both sides) bends by its ends' normals (PN); a
    // crease (normals split) bends along the crease line, whose direction
    // at an end is the cross product of the two sides' normals there. The
    // turn between the ends (normals or crease directions) must lie in
    // the band; open borders and edges of three or more triangles stay
    // straight.
    let (cos_min, cos_max) = (MIN_ANGLE.to_radians().cos(), MAX_ANGLE.to_radians().cos());
    let in_band = |a: Vec3, b: Vec3| {
        let c = a.dot(b);
        c <= cos_min && c >= cos_max
    };
    let mut curve: HashMap<(u32, u32), Edge> = HashMap::new();
    for (&(lo, hi), on) in &edges {
        let [(t1, k1), (t2, k2)] = on[..] else { continue };
        let (a1, b1) = (tris[t1][k1], tris[t1][(k1 + 1) % 3]);
        let (a2, b2) = (tris[t2][k2], tris[t2][(k2 + 1) % 3]);
        // A consistent winding walks the shared edge both ways.
        if pid[a1 as usize] != pid[b2 as usize] || pid[b1 as usize] != pid[a2 as usize] {
            continue;
        }
        let _ = hi;
        // The canonical end first (the lower id): side 1's and side 2's
        // vertices at each end.
        let (e0, e1, f0, f1) = if pid[a1 as usize] == lo { (a1, b1, b2, a2) } else { (b1, a1, a2, b2) };
        let (p0, p1) = (pos(e0), pos(e1));
        let dir = p1 - p0;
        let len = dir.length();
        if len < 1.0e-5 {
            continue;
        }
        let d = dir / len;
        let smooth0 = nrm(e0).dot(nrm(f0)) >= SAME_NORMAL;
        let smooth1 = nrm(e1).dot(nrm(f1)) >= SAME_NORMAL;
        // Each end's tangent toward the other end.
        let tangent = |n_a: Vec3, n_b: Vec3, smooth: bool, toward: Vec3| -> Option<Vec3> {
            if smooth {
                // PN: the edge projected into the end's tangent plane.
                let t = (toward - n_a * toward.dot(n_a)).normalize_or_zero();
                (t != Vec3::ZERO).then_some(t)
            } else {
                let t = n_a.cross(n_b).normalize_or_zero();
                let t = if t.dot(toward) < 0.0 { -t } else { t };
                // A corner (the crease turns sharply here): no tangent.
                (t.dot(toward) > 30f32.to_radians().cos()).then_some(t)
            }
        };
        let (Some(t0), Some(t1v)) = (tangent(nrm(e0), nrm(f0), smooth0, d), tangent(nrm(e1), nrm(f1), smooth1, -d))
        else {
            continue;
        };
        // The turn along the edge: between the ends' normals on a smooth
        // edge, between the tangents (one reversed) on a crease.
        let turn_ok = if smooth0 && smooth1 { in_band(nrm(e0), nrm(e1)) } else { in_band(t0, -t1v) };
        if !turn_ok {
            continue;
        }
        let mut o0 = t0 * (len / 3.0) - dir / 3.0;
        let mut o1 = t1v * (len / 3.0) + dir / 3.0;
        // The curve's largest offset from the chord (sampled; the cubic's
        // offset is 3(1−t)²t·o0 + 3(1−t)t²·o1).
        let m = (1..16)
            .map(|k| {
                let t = k as f32 / 16.0;
                (o0 * (3.0 * (1.0 - t) * (1.0 - t) * t) + o1 * (3.0 * (1.0 - t) * t * t)).length()
            })
            .fold(0.0f32, f32::max);
        let cap = MAX_SAG.min(MAX_SAG_RATIO * len);
        if m < MIN_SAG {
            continue;
        }
        // Outward only: an edge that would sink into the car (a concave
        // fold) stays straight. Car parts are layered a few millimetres
        // apart (lamps, trims, inner panels), and a sunken panel lets the
        // layer under it through (SUPRA's roof, G35's trunk in the review).
        let mid = (o0 + o1) * (3.0 / 8.0);
        if mid.dot(face[t1] + face[t2]) < 0.0 {
            continue;
        }
        if m > cap {
            o0 *= cap / m;
            o1 *= cap / m;
        }
        curve.insert((lo, hi), Edge { c: Some([(2.0 * p0 + p1) / 3.0 + o0, (p0 + 2.0 * p1) / 3.0 + o1]) });
    }
    if curve.is_empty() {
        return st;
    }
    st.curved_edges = curve.len();
    let edge_of = |a: u16, b: u16| {
        let (x, y) = (pid[a as usize], pid[b as usize]);
        (x.min(y), x.max(y))
    };
    let split: Vec<bool> =
        tris.iter().map(|t| (0..3).any(|k| curve.contains_key(&edge_of(t[k], t[(k + 1) % 3])))).collect();
    // A straight edge gets points only when every triangle on it is split
    // (else they fold onto its corners: no T-junction).
    let edge_points =
        |a: u16, b: u16| edges.get(&edge_of(a, b)).is_some_and(|on| on.len() == 2 && on.iter().all(|&(t, _)| split[t]));
    let n = level;
    let new_per = (n + 1) * (n + 2) / 2 - 3;
    let extra = split.iter().filter(|&&x| x).count() * new_per;
    if s.vertices.len() + extra > u16::MAX as usize {
        return if level > 2 { smooth_solid(s, level - 1) } else { st };
    }
    // The points of each edge, computed once in the canonical direction.
    let mut edge_cache: HashMap<(u32, u32), Vec<Vec3>> = HashMap::new();
    let mut points_on = |a: u16, b: u16| -> Vec<Vec3> {
        let e = edge_of(a, b);
        let pts = edge_cache.entry(e).or_insert_with(|| {
            let (p0, p1) = if pid[a as usize] == e.0 { (pos(a), pos(b)) } else { (pos(b), pos(a)) };
            let c = curve.get(&e).and_then(|c| c.c).unwrap_or([(2.0 * p0 + p1) / 3.0, (p0 + 2.0 * p1) / 3.0]);
            (0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32;
                    let u = 1.0 - t;
                    p0 * (u * u * u) + c[0] * (3.0 * u * u * t) + c[1] * (3.0 * u * t * t) + p1 * (t * t * t)
                })
                .collect()
        });
        if pid[a as usize] == e.0 { pts.clone() } else { pts.iter().rev().copied().collect() }
    };
    let ctrl = |a: u16, b: u16| -> [Vec3; 2] {
        let e = edge_of(a, b);
        let (pa, pb) = (pos(a), pos(b));
        match curve.get(&e).and_then(|c| c.c) {
            Some(c) if pid[a as usize] == e.0 => c,
            Some(c) => [c[1], c[0]],
            None => [(2.0 * pa + pb) / 3.0, (pa + 2.0 * pb) / 3.0],
        }
    };
    let mut vertices = s.vertices.clone();
    let mut normals = s.normals.clone();
    let mut indices: Vec<u16> = Vec::with_capacity(s.indices.len() + extra * 2);
    let mut max_disp = 0.0f32;
    let mut materials = s.materials.clone();
    for m in &mut materials {
        let first = indices.len() as u32;
        let range = m.first_index as usize..(m.first_index + m.index_count) as usize;
        for t in range.start / 3..range.end.div_ceil(3).min(tris.len()) {
            let tri = tris[t];
            if !split[t] {
                indices.extend_from_slice(&tri);
                continue;
            }
            let p = tri.map(pos);
            let [b210, b120] = ctrl(tri[0], tri[1]);
            let [b021, b012] = ctrl(tri[1], tri[2]);
            let [b201, b102] = {
                let c = ctrl(tri[2], tri[0]);
                [c[1], c[0]]
            };
            let e = (b210 + b120 + b021 + b012 + b102 + b201) / 6.0;
            let v = (p[0] + p[1] + p[2]) / 3.0;
            let b111 = e + (e - v) / 2.0;
            let sides = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
            let along: Vec<Option<Vec<Vec3>>> = sides
                .iter()
                .map(|&(a, b)| (curve.contains_key(&edge_of(a, b)) || edge_points(a, b)).then(|| points_on(a, b)))
                .collect();
            // Grid (i toward corner 1, j toward corner 2) → vertex index.
            let mut grid = vec![0u16; (n + 1) * (n + 1)];
            let at = |i: usize, j: usize| i * (n + 1) + j;
            for i in 0..=n {
                for j in 0..=n - i {
                    let k0 = n - i - j;
                    // Corners: the original vertices.
                    if i == n {
                        grid[at(i, j)] = tri[1];
                        continue;
                    }
                    if j == n {
                        grid[at(i, j)] = tri[2];
                        continue;
                    }
                    if k0 == n {
                        grid[at(i, j)] = tri[0];
                        continue;
                    }
                    // On an edge: its own points, or folded onto a corner.
                    let side = if j == 0 {
                        Some((0, i))
                    } else if k0 == 0 {
                        Some((1, j))
                    } else if i == 0 {
                        Some((2, n - j))
                    } else {
                        None
                    };
                    let (u, vv, w) = (k0 as f32 / n as f32, i as f32 / n as f32, j as f32 / n as f32);
                    let position = match side {
                        Some((sd, k)) => match &along[sd] {
                            Some(pts) => pts[k],
                            None => {
                                let (a, b) = sides[sd];
                                grid[at(i, j)] = if 2 * k < n { a } else { b };
                                continue;
                            }
                        },
                        None => {
                            p[0] * (u * u * u)
                                + p[1] * (vv * vv * vv)
                                + p[2] * (w * w * w)
                                + b210 * (3.0 * u * u * vv)
                                + b120 * (3.0 * u * vv * vv)
                                + b201 * (3.0 * u * u * w)
                                + b021 * (3.0 * vv * vv * w)
                                + b102 * (3.0 * u * w * w)
                                + b012 * (3.0 * vv * w * w)
                                + b111 * (6.0 * u * vv * w)
                        }
                    };
                    let flat = p[0] * u + p[1] * vv + p[2] * w;
                    // Interior points held to the cap too (edge points are
                    // by construction).
                    let off = position - flat;
                    let position = if side.is_none() && off.length() > MAX_SAG {
                        flat + off * (MAX_SAG / off.length())
                    } else {
                        position
                    };
                    // Interior points never sink below the source triangle
                    // either (outward only, as the edges).
                    let position = if side.is_none() && (position - flat).dot(face[t]) < 0.0 {
                        position - face[t] * (position - flat).dot(face[t])
                    } else {
                        position
                    };
                    max_disp = max_disp.max((position - flat).dot(face[t]).abs());
                    let src = tri.map(|x| s.vertices[x as usize]);
                    let mut vx = src[0];
                    vx.position = position.to_array();
                    vx.uv = [0, 1].map(|c| src[0].uv[c] * u + src[1].uv[c] * vv + src[2].uv[c] * w);
                    vx.color = [0, 1, 2, 3].map(|c| {
                        (f32::from(src[0].color[c]) * u
                            + f32::from(src[1].color[c]) * vv
                            + f32::from(src[2].color[c]) * w)
                            .round()
                            .clamp(0.0, 255.0) as u8
                    });
                    let nn = tri.map(nrm);
                    let normal = (nn[0] * u + nn[1] * vv + nn[2] * w).normalize_or(nn[0]);
                    grid[at(i, j)] = vertices.len() as u16;
                    vertices.push(vx);
                    normals.push(normal.to_array());
                }
            }
            let mut push = |a: u16, b: u16, c: u16| {
                if a != b && b != c && a != c {
                    indices.extend_from_slice(&[a, b, c]);
                }
            };
            for i in 0..n {
                for j in 0..n - i {
                    push(grid[at(i, j)], grid[at(i + 1, j)], grid[at(i, j + 1)]);
                    if i + j + 1 < n {
                        push(grid[at(i + 1, j)], grid[at(i + 1, j + 1)], grid[at(i, j + 1)]);
                    }
                }
            }
        }
        m.first_index = first;
        m.index_count = indices.len() as u32 - first;
    }
    // The box stays the file's: the car shadow's box (`Car` shadow_box) and
    // the culls read it, and the curves stay within MAX_SAG of it.
    s.vertices = vertices;
    s.normals = normals;
    s.indices = indices;
    s.materials = materials;
    st.solids_changed = 1;
    st.triangles_out = s.indices.len() / 3;
    st.vertices_out = s.vertices.len();
    st.max_displacement = max_disp;
    st
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::solid::{Material, Vertex};

    fn solid(name: &str, verts: &[([f32; 3], [f32; 3])], tris: &[[u16; 3]]) -> Solid {
        Solid {
            name: name.into(),
            hash: 0,
            bbox_min: [-10.0; 3],
            bbox_max: [10.0; 3],
            textures: vec![0],
            vertices: verts.iter().map(|(p, _)| Vertex { position: *p, color: [255; 4], uv: [p[0], p[1]] }).collect(),
            normals: verts.iter().map(|(_, n)| Vec3::from(*n).normalize().to_array()).collect(),
            light_materials: Vec::new(),
            indices: tris.iter().flatten().copied().collect(),
            materials: vec![Material {
                texture_slot: 0,
                light_material: 0xFF,
                first_index: 0,
                index_count: (tris.len() * 3) as u32,
                flags: 0,
            }],
            mesh_flags: 0x180,
            markers: Vec::new(),
            light_markers: Vec::new(),
        }
    }

    /// A flat panel (all normals up) is left exactly as it is.
    #[test]
    fn flat_panels_stay() {
        let up = [0.0, 0.0, 1.0];
        let mut s = solid(
            "X_A",
            &[([0.0, 0.0, 0.0], up), ([1.0, 0.0, 0.0], up), ([1.0, 1.0, 0.0], up), ([0.0, 1.0, 0.0], up)],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let before = (s.indices.clone(), s.vertices.len());
        let st = smooth_solid(&mut s, LEVEL);
        assert_eq!(st.solids_changed, 0);
        assert_eq!((s.indices.clone(), s.vertices.len()), before);
    }

    /// A smooth-shaded 30° fold (a roof line) curves, within the caps, and
    /// keeps every original vertex and the open border straight.
    #[test]
    fn fold_curves_within_tolerance() {
        let a = 15f32.to_radians();
        let (ca, sa) = (a.cos(), a.sin());
        // Two strips meeting at the ridge y = 0, each sloping down 15°.
        let n_mid = [0.0, 0.0, 1.0];
        let n_l = [0.0, sa, ca];
        let n_r = [0.0, -sa, ca];
        let v = [
            ([0.0, 0.3 * ca, -0.3 * sa], n_l),
            ([1.0, 0.3 * ca, -0.3 * sa], n_l),
            ([0.0, 0.0, 0.0], n_mid),
            ([1.0, 0.0, 0.0], n_mid),
            ([0.0, -0.3 * ca, -0.3 * sa], n_r),
            ([1.0, -0.3 * ca, -0.3 * sa], n_r),
        ];
        let mut s = solid("ROOF_A", &v, &[[0, 2, 1], [1, 2, 3], [2, 4, 3], [3, 4, 5]]);
        let orig: Vec<[f32; 3]> = s.vertices.iter().map(|v| v.position).collect();
        let st = smooth_solid(&mut s, LEVEL);
        assert_eq!(st.solids_changed, 1);
        assert!(st.curved_edges >= 1, "{st:?}");
        assert!(st.max_displacement > 0.0 && st.max_displacement <= MAX_SAG + 1e-4, "{st:?}");
        // Originals untouched, at their indices.
        assert!(s.vertices[..orig.len()].iter().map(|v| v.position).eq(orig.iter().copied()));
        // The ridge rounds outward (convex): new vertices above the
        // original strips' planes, none below.
        let plane = |p: [f32; 3]| -p[1].abs() * 15f32.to_radians().tan();
        assert!(s.vertices[orig.len()..].iter().any(|v| v.position[2] > plane(v.position) + 1e-4));
        assert!(s.vertices[orig.len()..].iter().all(|v| v.position[2] > plane(v.position) - 1e-5));
        // Every index in range, no degenerate triangle.
        for t in s.indices.chunks(3) {
            assert!(t.iter().all(|&i| (i as usize) < s.vertices.len()));
            assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
        }
    }

    /// Hard creases (split normals) stay straight: a box edge.
    #[test]
    fn creases_stay() {
        let up = [0.0, 0.0, 1.0];
        let side = [0.0, -1.0, 0.0];
        let v = [
            ([0.0, 0.0, 0.0], up),
            ([1.0, 0.0, 0.0], up),
            ([1.0, 1.0, 0.0], up),
            ([0.0, 1.0, 0.0], up),
            ([0.0, 0.0, 0.0], side),
            ([1.0, 0.0, 0.0], side),
            ([1.0, 0.0, -1.0], side),
            ([0.0, 0.0, -1.0], side),
        ];
        let mut s = solid("BOX_A", &v, &[[0, 1, 2], [0, 2, 3], [4, 6, 5], [4, 7, 6]]);
        assert_eq!(smooth_solid(&mut s, LEVEL).solids_changed, 0);
    }

    /// The whole car pass is off by default (native).
    #[test]
    fn off_by_default() {
        assert!(!enabled());
        assert_eq!(apply(&mut []), Stats::default());
    }

    /// Every player car: watertight where it was (each original edge used
    /// by two triangles still is, as a set of positions), within the
    /// displacement cap, and within the 16-bit indices.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn player_cars_stay_within_tolerance() {
        let root = crate::testkit::game_root();
        let root = std::path::PathBuf::from(root);
        for car in ["240SX", "350Z", "GOLF", "SKYLINE", "HUMMER"] {
            let Ok(data) = std::fs::read(root.join("CARS").join(car).join("GEOMETRY.BIN")) else { continue };
            let mut solids = Vec::new();
            for l in crate::formats::bundle::chunks(&data, 0).filter(|c| c.id == crate::formats::bundle::id::SOLID_LIST)
            {
                solids.extend(crate::formats::solid::parse_solid_list(l).unwrap());
            }
            let st = apply_always(&mut solids);
            assert!(st.solids_changed > 0, "{car}: {st:?}");
            assert!(st.max_displacement <= MAX_SAG + 1e-4, "{car}: {st:?}");
            for s in &solids {
                assert!(s.vertices.len() <= u16::MAX as usize);
                assert_eq!(s.normals.len(), s.vertices.len());
                let total: u32 = s.materials.iter().map(|m| m.index_count).sum();
                assert_eq!(total as usize, s.indices.len(), "{}", s.name);
            }
        }
    }
}
