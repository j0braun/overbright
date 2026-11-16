use super::*;

/// The ground blown debris and grit land on: the upward road and pavement
/// triangles in a 2D grid.
pub struct Ground {
    tris: Vec<[Vec3; 3]>,
    gravel: Vec<bool>,
    grid: Grid,
}

impl Ground {
    pub(crate) fn new(s: &Surfaces) -> Ground {
        let tris: Vec<[Vec3; 3]> = s.pave.iter().chain(&s.road).chain(&s.other).map(|t| t.p).collect();
        let grid = Grid::new(tris.iter(), 4.0);
        Ground { tris, gravel: s.gravel.clone(), grid }
    }

    /// The highest surface under `p` at most `above` metres over it: its
    /// height, normal and whether it is loose ground.
    pub fn at(&self, p: Vec3, above: f32) -> Option<(f32, Vec3, bool)> {
        let mut best: Option<(f32, Vec3, bool)> = None;
        for &i in self.grid.map.get(&self.grid.cell(p)).into_iter().flatten() {
            let t = &self.tris[i as usize];
            let (a, b, c) = (t[0], t[1], t[2]);
            let v0 = (b - a).truncate();
            let v1 = (c - a).truncate();
            let v2 = (p - a).truncate();
            let den = v0.perp_dot(v1);
            if den.abs() < 1e-9 {
                continue;
            }
            let u = v2.perp_dot(v1) / den;
            let v = v0.perp_dot(v2) / den;
            if u < -1e-4 || v < -1e-4 || u + v > 1.0001 {
                continue;
            }
            let z = a.z + (b.z - a.z) * u + (c.z - a.z) * v;
            if z <= p.z + above && best.is_none_or(|(bz, _, _)| z > bz) {
                let mut n = (b - a).cross(c - a).normalize_or(Vec3::Z);
                if n.z < 0.0 {
                    n = -n;
                }
                best = Some((z, n, self.gravel.get(i as usize).copied().unwrap_or(false)));
            }
        }
        best
    }

    pub fn len(&self) -> usize {
        self.tris.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Its triangles (as bits, in order) and loose-ground flags, folded
    /// into `h` (FNV-1a; the tests compare grounds).
    pub(crate) fn digest(&self, h: u64) -> u64 {
        let words = self.tris.iter().flat_map(|t| t.iter().flat_map(|p| p.to_array().map(f32::to_bits)));
        words
            .chain(self.gravel.iter().map(|&g| u32::from(g)))
            .fold(h ^ self.tris.len() as u64, |h, w| (h ^ u64::from(w)).wrapping_mul(0x100_0000_01b3))
    }
}
