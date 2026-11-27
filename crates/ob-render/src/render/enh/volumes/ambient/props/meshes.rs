use super::*;

/// A mesh vertex (ambient_detail.wgsl `vs_main` locations 0..2).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// The meshes' vertices and indices, and each mesh's (first index, index
/// count, base vertex). Lying meshes: x along the axis, resting on z = 0,
/// centred in x and y. Made in code (nothing from the game or a mod).
pub fn meshes() -> (Vec<Vertex>, Vec<u16>, [(u32, u32, i32); MESHES]) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let mut ranges = [(0u32, 0u32, 0i32); MESHES];
    let mut add = |m: Mesh, (mv, mi): (Vec<Vertex>, Vec<u16>)| {
        ranges[m as usize] = (i.len() as u32, mi.len() as u32, v.len() as i32);
        v.extend(mv);
        i.extend(mi);
    };
    // Can: 12 sides, r 3.3 cm, 12.2 cm long, lying.
    add(
        Mesh::Can,
        lathe(
            &[
                (-0.061, 0.0),
                (-0.061, 0.028),
                (-0.058, 0.033),
                (0.052, 0.033),
                (0.058, 0.028),
                (0.061, 0.022),
                (0.061, 0.0),
            ],
            12,
            0.033,
        ),
    );
    // Bottle: 10 sides, body r 3.2 cm, neck r 1.2 cm, 24 cm long.
    add(
        Mesh::Bottle,
        lathe(
            &[
                (-0.12, 0.0),
                (-0.12, 0.03),
                (-0.117, 0.032),
                (0.03, 0.032),
                (0.065, 0.014),
                (0.105, 0.012),
                (0.108, 0.014),
                (0.12, 0.013),
                (0.12, 0.0),
            ],
            10,
            0.032,
        ),
    );
    // Cup: a paper cup, 11 cm, r 4.2 → 3 cm (with its lid), lying.
    add(
        Mesh::Cup,
        lathe(&[(-0.055, 0.0), (-0.055, 0.03), (0.05, 0.042), (0.055, 0.043), (0.058, 0.035), (0.058, 0.0)], 10, 0.043),
    );
    add(Mesh::Box, cuboid(Vec3::new(0.36, 0.26, 0.16)));
    add(Mesh::Crumple, crumple(0.06));
    add(Mesh::Card, card(6, 4));
    add(Mesh::Quad, card(1, 1));
    (v, i, ranges)
}

/// A solid of revolution about x: the profile (x, radius) from end to end
/// (radius 0 closes an end), `sides` around, its axis `lift` above z = 0.
fn lathe(profile: &[(f32, f32)], sides: u32, lift: f32) -> (Vec<Vertex>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    let rows = profile.len() as u32;
    // Normals per ring from the profile's slope (smooth around).
    let len: f32 = profile.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum();
    let mut along = 0.0;
    for (k, &(x, r)) in profile.iter().enumerate() {
        if k > 0 {
            along += ((x - profile[k - 1].0).powi(2) + (r - profile[k - 1].1).powi(2)).sqrt();
        }
        let prev = profile[k.saturating_sub(1)];
        let next = profile[(k + 1).min(profile.len() - 1)];
        let (dx, dr) = (next.0 - prev.0, next.1 - prev.1);
        for s in 0..=sides {
            let a = s as f32 / sides as f32 * std::f32::consts::TAU;
            let (sa, ca) = a.sin_cos();
            let radial = Vec3::new(0.0, ca, sa);
            // Normal: perpendicular to the profile's tangent (dx, dr).
            let n = (radial * dx - Vec3::X * dr).normalize_or(Vec3::X * dx.signum());
            v.push(Vertex {
                pos: [x, r * ca, lift + r * sa],
                normal: n.to_array(),
                uv: [along / len, s as f32 / sides as f32],
            });
        }
    }
    let ring = sides + 1;
    for k in 0..rows - 1 {
        for s in 0..sides {
            let a = (k * ring + s) as u16;
            let b = (k * ring + s + 1) as u16;
            let c = ((k + 1) * ring + s) as u16;
            let d = ((k + 1) * ring + s + 1) as u16;
            idx.extend([a, c, b, b, c, d]);
        }
    }
    (v, idx)
}

fn cuboid(size: Vec3) -> (Vec<Vertex>, Vec<u16>) {
    let h = size * 0.5;
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for (n, u, w) in [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, -Vec3::Y, Vec3::Z),
        (Vec3::Y, -Vec3::X, Vec3::Z),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::X, -Vec3::Y),
    ] {
        let base = v.len() as u16;
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let p = (n + u * a + w * b) * h + Vec3::Z * h.z;
            v.push(Vertex { pos: p.to_array(), normal: n.to_array(), uv: [0.5 + 0.5 * a, 0.5 + 0.5 * b] });
        }
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (v, idx)
}

/// A crumpled ball: an icosahedron split once, each vertex pushed in or
/// out by a fixed hash, flat-shaded (its facets are the creases).
fn crumple(r: f32) -> (Vec<Vertex>, Vec<u16>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut p: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let faces: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    let mut mid = std::collections::HashMap::new();
    let mut tris = Vec::new();
    let mut m = |a: usize, b: usize, p: &mut Vec<Vec3>| -> usize {
        *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
            p.push(((p[a] + p[b]) * 0.5).normalize());
            p.len() - 1
        })
    };
    for f in faces {
        let (a, b, c) = (m(f[0], f[1], &mut p), m(f[1], f[2], &mut p), m(f[2], f[0], &mut p));
        tris.extend([[f[0], a, c], [f[1], b, a], [f[2], c, b], [a, b, c]]);
    }
    let pts: Vec<Vec3> = p
        .iter()
        .enumerate()
        .map(|(k, d)| {
            *d * r
                * (0.7 + 0.45 * (hash(k as u64 * 7919) >> 40) as f32 / (1u64 << 24) as f32)
                * Vec3::new(1.0, 1.0, 0.8)
                + Vec3::Z * r * 0.93
        })
        .collect();
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for t in tris {
        let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        let n = (b - a).cross(c - a).normalize_or(Vec3::Z);
        for (k, q) in [a, b, c].into_iter().enumerate() {
            idx.push(v.len() as u16);
            v.push(Vertex { pos: q.to_array(), normal: n.to_array(), uv: [k as f32 * 0.5, (q.z / r).fract()] });
        }
    }
    (v, idx)
}

/// A unit card (x, y in ±0.5) of `nx` × `ny` quads, curled up at its edges
/// (z = 0.10 y² + 0.04 x², a leaf's or a sheet's curl; the shader flattens
/// it by the instance's z scale). uv 0..1.
fn card(nx: u32, ny: u32) -> (Vec<Vertex>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    let curl = nx > 1;
    for j in 0..=ny {
        for i in 0..=nx {
            let (u, w) = (i as f32 / nx as f32, j as f32 / ny as f32);
            let (x, y) = (u - 0.5, w - 0.5);
            let (z, n) = if curl {
                let z = 0.10 * y * y + 0.04 * x * x;
                (z + 0.004, Vec3::new(-0.08 * x, -0.2 * y, 1.0).normalize())
            } else {
                (0.0, Vec3::Z)
            };
            v.push(Vertex { pos: [x, y, z], normal: n.to_array(), uv: [u, w] });
        }
    }
    let row = nx + 1;
    for j in 0..ny {
        for i in 0..nx {
            let a = (j * row + i) as u16;
            idx.extend([a, a + 1, a + row as u16, a + 1, a + row as u16 + 1, a + row as u16]);
        }
    }
    (v, idx)
}
