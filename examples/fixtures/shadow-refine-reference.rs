// Frozen c170c12 Shadow::refine; comparison oracle, not production.
fn reference(mut shadow: Shadow, n: usize, ground: &mut Ground) -> Shadow {
        if n <= 1 {
            return shadow;
        }
        let lift: Vec<f32> = shadow.vertices.iter().map(|v| match ground(v.position) {
            Some((z, _)) => v.position.z - z,
            None => 0.0,
        }).collect();
        let mut cache: std::collections::HashMap<(usize, usize), ShadowVertex> = Default::default();
        let mut vertex = |row: usize, col: usize| -> ShadowVertex {
            *cache.entry((row, col)).or_insert_with(|| {
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
                let p = mix(&|k| shadow.vertices[k].position);
                let uv = mix(&|k| Vec3::new(shadow.vertices[k].uv[0], shadow.vertices[k].uv[1], lift[k]));
                let z = match ground(p) {
                    Some((g, _)) if (g + uv.z - p.z).abs() < 1.0 => g + uv.z,
                    _ => p.z,
                };
                ShadowVertex { position: p.with_z(z), uv: [uv.x, uv.y] }
            })
        };
        let mut out = Vec::new();
        for j in (0..3).filter(|&j| shadow.strips[j]) {
            for c in 0..3 {
                for a in 0..n {
                    for b in 0..n {
                        let (row, col) = (j * n + a, c * n + b);
                        out.push([vertex(row + 1, col), vertex(row, col), vertex(row, col + 1), vertex(row + 1, col + 1)]);
                    }
                }
            }
        }
        shadow.refined = Some(out);
        shadow
    }
