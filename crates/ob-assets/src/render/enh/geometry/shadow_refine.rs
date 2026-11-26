use glam::Vec3;

use crate::render::native::fx::shadow::{Ground, Shadow, ShadowVertex};

impl Shadow {
    /// The Graphics page's Shadow Quality High / Ultra (a **clone
    /// deviation**; the native shadow is this texture on the 4 × 4 grid,
    /// there is no shadow map to sharpen): each quad of the kept strips
    /// split `n` × `n`, positions and texture coordinates interpolated, and
    /// each new vertex put at the native grid's height above the ground
    /// (interpolated from the quad's corners) over the ground at its own
    /// spot, so the shadow follows kerbs and slopes between the native
    /// vertices instead of cutting through them or floating. A vertex whose
    /// query misses, or lands more than 1 m from the interpolated height,
    /// keeps the interpolated one. At the native vertices nothing changes;
    /// the texture, colour and strips are the native ones.
    pub fn refine(mut self, n: usize, ground: &mut Ground) -> Shadow {
        if n <= 1 {
            return self;
        }
        let lift = self.vertices.map(|v| match ground(v.position) {
            Some((z, _)) => v.position.z - z,
            None => 0.0,
        });
        // The regular grid has one unique key per (row, column). Keep the
        // original first-use/query order; only replace the hash lookup.
        let side = 3 * n + 1;
        let mut cache = vec![None; side * side];
        let mut vertex = |row: usize, col: usize| -> ShadowVertex {
            *cache[row * side + col].get_or_insert_with(|| {
                // Grid position in native cells.
                let (r, c) = (row as f32 / n as f32, col as f32 / n as f32);
                let (r0, c0) = ((r.floor() as usize).min(2), (c.floor() as usize).min(2));
                let (fr, fc) = (r - r0 as f32, c - c0 as f32);
                let at = |rr: usize, cc: usize| rr * 4 + cc;
                let mix = |f: &dyn Fn(usize) -> Vec3| {
                    let top = f(at(r0, c0)).lerp(f(at(r0, c0 + 1)), fc);
                    let bottom = f(at(r0 + 1, c0)).lerp(f(at(r0 + 1, c0 + 1)), fc);
                    top.lerp(bottom, fr)
                };
                let p = mix(&|k| self.vertices[k].position);
                let uv = mix(&|k| Vec3::new(self.vertices[k].uv[0], self.vertices[k].uv[1], lift[k]));
                let z = match ground(p) {
                    Some((g, _)) if (g + uv.z - p.z).abs() < 1.0 => g + uv.z,
                    _ => p.z,
                };
                ShadowVertex { position: p.with_z(z), uv: [uv.x, uv.y] }
            })
        };
        let mut out = Vec::with_capacity(self.strips.iter().filter(|&&kept| kept).count() * 3 * n * n);
        for j in (0..3).filter(|&j| self.strips[j]) {
            for c in 0..3 {
                for a in 0..n {
                    for b in 0..n {
                        let (row, col) = (j * n + a, c * n + b);
                        out.push([
                            vertex(row + 1, col),
                            vertex(row, col),
                            vertex(row, col + 1),
                            vertex(row + 1, col + 1),
                        ]);
                    }
                }
            }
        }
        self.refined = Some(out);
        self
    }
}
