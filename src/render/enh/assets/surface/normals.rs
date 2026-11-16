use super::*;

/// Why a texture gets no normal map, or that it gets one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalVerdict {
    Map,
    /// Its class has no map (glass, signs, foliage, emissive).
    Class,
    /// Alpha-tested cut-outs (the holes' edges would emboss).
    Cutout,
    /// Too small to carry grain (under 32 texels a side).
    Small,
    /// Little high-frequency detail: nothing to gain.
    Flat,
    /// Its detail is painted structure (edges of drawn shapes, painted
    /// shadows), not surface grain: it would emboss the art.
    Painted,
}

/// The high-pass Gaussian's σ in texels for a texture `w` wide (the baked
/// light and the art's large shapes are below it).
fn high_pass_sigma(w: usize) -> f32 {
    (w as f32 / 64.0).clamp(1.5, 4.0)
}

/// Luma (gamma-space, Rec. 709 weights) of RGBA8 texels, 0..1.
fn luma(rgba: &[u8], w: usize, h: usize) -> Vec<f32> {
    (0..w * h)
        .map(|i| {
            let p = &rgba[i * 4..i * 4 + 4];
            (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0
        })
        .collect()
}

/// A separable Gaussian blur with wrap-around (the world's textures tile).
fn blur_wrap(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as isize;
    let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    let k: Vec<f32> = k.iter().map(|v| v / sum).collect();
    let mut tmp = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut a = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let xx = (x as isize + j as isize - r).rem_euclid(w as isize) as usize;
                a += src[y * w + xx] * kv;
            }
            tmp[y * w + x] = a;
        }
    }
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut a = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let yy = (y as isize + j as isize - r).rem_euclid(h as isize) as usize;
                a += tmp[yy * w + x] * kv;
            }
            out[y * w + x] = a;
        }
    }
    out
}

/// The detail measures of a texture's luma: the high-pass band's RMS and
/// the gradient's "edginess" (the 99th percentile of the gradient
/// magnitude over its median; grain is near-Gaussian, ~3–5, drawn shapes
/// are mostly flat with sharp edges, ≫ 8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub rms: f32,
    pub edginess: f32,
    /// The low band's (baked light, large shapes) RMS about its mean.
    pub low_rms: f32,
}

/// The high-passed luma and its [`Detail`].
fn detail(rgba: &[u8], w: usize, h: usize) -> (Vec<f32>, Detail) {
    let l = luma(rgba, w, h);
    let low = blur_wrap(&l, w, h, high_pass_sigma(w));
    let hp: Vec<f32> = l.iter().zip(&low).map(|(a, b)| a - b).collect();
    let n = (w * h) as f32;
    let rms = (hp.iter().map(|v| v * v).sum::<f32>() / n).sqrt();
    let mean_low = low.iter().sum::<f32>() / n;
    let low_rms = (low.iter().map(|v| (v - mean_low) * (v - mean_low)).sum::<f32>() / n).sqrt();
    let mut g: Vec<f32> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let at = |dx: isize, dy: isize| {
                hp[((y as isize + dy).rem_euclid(h as isize) as usize) * w
                    + (x as isize + dx).rem_euclid(w as isize) as usize]
            };
            let gx = at(1, 0) - at(-1, 0);
            let gy = at(0, 1) - at(0, -1);
            (gx * gx + gy * gy).sqrt()
        })
        .collect();
    g.sort_by(|a, b| a.total_cmp(b));
    let median = g[g.len() / 2].max(1e-4);
    let p99 = g[(g.len() * 99 / 100).min(g.len() - 1)];
    (hp, Detail { rms, edginess: p99 / median, low_rms })
}

/// Thresholds of [`normal_verdict`] (calibrated on Bayview's most visible
/// textures, docs/surface-maps.md).
pub const MIN_RMS: f32 = 0.012;
pub const MAX_EDGINESS: f32 = 9.0;

/// Whether `info` (its top level decoded as RGBA8, `w` × `h`) gets a map.
pub fn normal_verdict(info: &TextureInfo, class: Class, rgba: &[u8], w: u32, h: u32) -> NormalVerdict {
    if class.params().normal <= 0.0 {
        return NormalVerdict::Class;
    }
    if info.alpha_test {
        return NormalVerdict::Cutout;
    }
    if w < 32 || h < 32 || rgba.len() < (w * h * 4) as usize {
        return NormalVerdict::Small;
    }
    let (_, d) = detail(rgba, w as usize, h as usize);
    verdict_of(&d)
}

fn verdict_of(d: &Detail) -> NormalVerdict {
    if d.rms < MIN_RMS {
        return NormalVerdict::Flat;
    }
    if d.edginess > MAX_EDGINESS {
        return NormalVerdict::Painted;
    }
    NormalVerdict::Map
}

/// A generated map: [`MAP_MIPS`] levels of RG8 snorm texels (x, y of the
/// tangent-space normal; z = sqrt(1 − x² − y²)), level 0 first.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalMap {
    pub levels: Vec<Vec<[i8; 2]>>,
}

impl NormalMap {
    /// All levels back to back (the upload's layout, rows unpadded).
    pub fn bytes(&self) -> Vec<u8> {
        self.levels.iter().flat_map(|l| l.iter().flat_map(|p| [p[0] as u8, p[1] as u8])).collect()
    }

    pub fn from_bytes(b: &[u8]) -> Option<NormalMap> {
        let mut levels = Vec::new();
        let mut at = 0;
        for m in 0..MAP_MIPS {
            let s = (MAP_SIZE >> m).max(1) as usize;
            let n = s * s * 2;
            let l = b.get(at..at + n)?;
            levels.push(l.chunks_exact(2).map(|p| [p[0] as i8, p[1] as i8]).collect());
            at += n;
        }
        (at == b.len()).then_some(NormalMap { levels })
    }
}

/// The slope scale: a height step of one RMS of the texture's high-pass
/// band between neighbours tilts the normal by atan(SLOPE · strength)
/// (about 13° at strength 1). Small on purpose: the art's own shading
/// stays the main relief.
pub const SLOPE: f32 = 0.24;

/// The normal map of a texture (top level RGBA8, `w` × `h`), at
/// [`MAP_SIZE`]² with its mips; `strength` from [`Params::normal`].
pub fn build_normals(rgba: &[u8], w: u32, h: u32, strength: f32) -> NormalMap {
    let (w, h) = (w as usize, h as usize);
    let (hp, d) = detail(rgba, w, h);
    // Height: the high-passed luma, soft-clipped at 2.5 × its RMS (painted
    // edges and paint lines do not become cliffs), in RMS units.
    let clip = 2.5 * d.rms.max(1e-4);
    let height: Vec<f32> = hp.iter().map(|v| clip * (v / clip).tanh()).collect();
    // Gradient (central differences with a [1 2 1] cross smoothing,
    // Sobel), wrap-around, per texel; scaled so the tilt does not depend
    // on the texture's resolution (per 1/128 of the texture).
    let scale = SLOPE * strength;
    let at = |x: isize, y: isize| height[(y.rem_euclid(h as isize) as usize) * w + x.rem_euclid(w as isize) as usize];
    let mut n = vec![[0.0f32; 3]; w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let gx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1))
                / 8.0;
            let gy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1))
                / 8.0;
            // Bright = raised: the normal leans away from the uphill side.
            let k = scale / d.rms.max(MIN_RMS);
            let v = [-gx * k, -gy * k, 1.0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            n[y as usize * w + x as usize] = [v[0] / l, v[1] / l, v[2] / l];
        }
    }
    // To MAP_SIZE² (bilinear, wrap-around), then a box mip chain of the
    // unnormalised vectors (their shortening is the mip's spread; stored
    // renormalised: the shader widens the roughness by the derivative).
    let s = MAP_SIZE as usize;
    let mut top = vec![[0.0f32; 3]; s * s];
    for y in 0..s {
        for x in 0..s {
            let fx = (x as f32 + 0.5) * w as f32 / s as f32 - 0.5;
            let fy = (y as f32 + 0.5) * h as f32 / s as f32 - 0.5;
            let (x0, y0) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - x0, fy - y0);
            let g = |xx: f32, yy: f32| {
                n[((yy as isize).rem_euclid(h as isize) as usize) * w + (xx as isize).rem_euclid(w as isize) as usize]
            };
            let (a, b, c, e) = (g(x0, y0), g(x0 + 1.0, y0), g(x0, y0 + 1.0), g(x0 + 1.0, y0 + 1.0));
            let mut v = [0.0; 3];
            for k in 0..3 {
                v[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + e[k] * tx) * ty;
            }
            top[y * s + x] = v;
        }
    }
    let mut levels_f = vec![top];
    for m in 1..MAP_MIPS as usize {
        let prev = &levels_f[m - 1];
        let ps = s >> (m - 1);
        let ns = (s >> m).max(1);
        let mut next = vec![[0.0f32; 3]; ns * ns];
        for y in 0..ns {
            for x in 0..ns {
                let mut v = [0.0; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(2 * y + dy).min(ps - 1) * ps + (2 * x + dx).min(ps - 1)];
                    for k in 0..3 {
                        v[k] += p[k] * 0.25;
                    }
                }
                next[y * ns + x] = v;
            }
        }
        levels_f.push(next);
    }
    let q = |v: f32| (v * 127.0).round().clamp(-127.0, 127.0) as i8;
    let levels = levels_f
        .iter()
        .map(|l| {
            l.iter()
                .map(|v| {
                    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
                    [q(v[0] / len), q(v[1] / len)]
                })
                .collect()
        })
        .collect();
    NormalMap { levels }
}

/// A region of a car texture: the UV triangles of the draws of one class.
#[derive(Clone, Debug, PartialEq)]
pub struct CarRegion {
    pub class: Class,
    pub tris: Vec<[[f32; 2]; 3]>,
}

/// A car map: RG8 snorm tangent-space normals at the source's size (at
/// most [`PACK_MAX`]), full mip chain, level 0 first. Texels outside every
/// accepted region are (0, 0): flat, the native shading.
#[derive(Clone, Debug, PartialEq)]
pub struct CarMap {
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<[i8; 2]>>,
}

impl CarMap {
    pub fn mips(w: u32, h: u32) -> u32 {
        32 - w.max(h).max(1).leading_zeros()
    }

    /// The cache file's bytes: width, height (u32 LE), then the levels.
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(8 + self.levels.iter().map(|l| l.len() * 2).sum::<usize>());
        b.extend(self.width.to_le_bytes());
        b.extend(self.height.to_le_bytes());
        for l in &self.levels {
            b.extend(l.iter().flat_map(|p| [p[0] as u8, p[1] as u8]));
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<CarMap> {
        let width = u32::from_le_bytes(b.get(0..4)?.try_into().ok()?);
        let height = u32::from_le_bytes(b.get(4..8)?.try_into().ok()?);
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return None;
        }
        let mut levels = Vec::new();
        let mut at = 8;
        for m in 0..CarMap::mips(width, height) {
            let n = ((width >> m).max(1) * (height >> m).max(1)) as usize * 2;
            levels.push(b.get(at..at + n)?.chunks_exact(2).map(|p| [p[0] as i8, p[1] as i8]).collect());
            at += n;
        }
        (at == b.len()).then_some(CarMap { width, height, levels })
    }
}

/// Region ids per texel (0 none, i + 1 region i): the triangles
/// rasterised in texel space (wrapping), texels whose alpha is below the
/// alpha test's reference (holes, cut-outs) left out.
fn car_mask(rgba: &[u8], w: usize, h: usize, regions: &[CarRegion]) -> Vec<u8> {
    let mut mask = vec![0u8; w * h];
    for (r, region) in regions.iter().enumerate() {
        let id = r as u8 + 1;
        for t in &region.tris {
            let p: Vec<(f32, f32)> = t.iter().map(|uv| (uv[0] * w as f32 - 0.5, uv[1] * h as f32 - 0.5)).collect();
            let (x0, x1) = (
                p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor(),
                p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil(),
            );
            let (y0, y1) = (
                p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor(),
                p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil(),
            );
            if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) {
                continue;
            }
            if (x1 - x0) * (y1 - y0) >= (w * h) as f32 {
                // Tiled across the whole texture (the carbon weave).
                mask.iter_mut().for_each(|m| *m = id);
                continue;
            }
            let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
            let area = edge(p[0], p[1], p[2].0, p[2].1);
            // Grown by a texel (the map's bilinear footprint at the edges).
            let grow = 1.0;
            for y in y0 as i64 - 1..=y1 as i64 + 1 {
                for x in x0 as i64 - 1..=x1 as i64 + 1 {
                    let (fx, fy) = (x as f32, y as f32);
                    let inside = if area.abs() < 1e-6 {
                        false
                    } else {
                        let s = area.signum();
                        [(0, 1, 2), (1, 2, 0), (2, 0, 1)].iter().all(|&(a, b, _)| {
                            let (pa, pb) = (p[a], p[b]);
                            let len = ((pb.0 - pa.0).powi(2) + (pb.1 - pa.1).powi(2)).sqrt().max(1e-6);
                            edge(pa, pb, fx, fy) * s >= -grow * len
                        })
                    };
                    if inside {
                        mask[y.rem_euclid(h as i64) as usize * w + x.rem_euclid(w as i64) as usize] = id;
                    }
                }
            }
        }
    }
    for (i, m) in mask.iter_mut().enumerate() {
        if rgba[i * 4 + 3] < 95 {
            *m = 0;
        }
    }
    mask
}

/// Each region's [`Detail`] over its own texels: (region, verdict, detail).
pub fn car_verdicts(rgba: &[u8], w: u32, h: u32, regions: &[CarRegion]) -> Vec<(Class, NormalVerdict, Detail)> {
    let (w, h) = (w as usize, h as usize);
    if w < 32 || h < 32 || rgba.len() < w * h * 4 {
        return regions
            .iter()
            .map(|r| (r.class, NormalVerdict::Small, Detail { rms: 0.0, edginess: 0.0, low_rms: 0.0 }))
            .collect();
    }
    let mask = car_mask(rgba, w, h, regions);
    let hp = car_high_pass(rgba, w, h, &mask);
    regions
        .iter()
        .enumerate()
        .map(|(r, region)| {
            let d = region_detail(&hp, w, h, &mask, r as u8 + 1);
            (region.class, car_verdict_of(region.class, &d), d)
        })
        .collect()
}

fn car_verdict_of(class: Class, d: &Detail) -> NormalVerdict {
    if class.params().normal <= 0.0 {
        return NormalVerdict::Class;
    }
    if d.rms < MIN_RMS {
        return NormalVerdict::Flat;
    }
    if d.edginess > car_max_edginess(class) {
        return NormalVerdict::Painted;
    }
    NormalVerdict::Map
}

/// The high-passed luma of the masked texels; the others take the masked
/// mean first (a region's border with the paint or a hole is no edge).
fn car_high_pass(rgba: &[u8], w: usize, h: usize, mask: &[u8]) -> Vec<f32> {
    let mut l = luma(rgba, w, h);
    let (sum, n) = l.iter().zip(mask).filter(|(_, m)| **m > 0).fold((0.0f32, 0usize), |(s, n), (v, _)| (s + v, n + 1));
    let mean = if n > 0 { sum / n as f32 } else { 0.5 };
    for (v, m) in l.iter_mut().zip(mask) {
        if *m == 0 {
            *v = mean;
        }
    }
    let low = blur_wrap(&l, w, h, high_pass_sigma(w.min(h)));
    l.iter().zip(&low).zip(mask).map(|((a, b), m)| if *m > 0 { a - b } else { 0.0 }).collect()
}

/// [`Detail`] of region `id`'s texels of the high-passed `hp`.
fn region_detail(hp: &[f32], w: usize, h: usize, mask: &[u8], id: u8) -> Detail {
    let texels: Vec<usize> = (0..w * h).filter(|&i| mask[i] == id).collect();
    if texels.len() < 64 {
        return Detail { rms: 0.0, edginess: 0.0, low_rms: 0.0 };
    }
    let n = texels.len() as f32;
    let rms = (texels.iter().map(|&i| hp[i] * hp[i]).sum::<f32>() / n).sqrt();
    let mut g: Vec<f32> = texels
        .iter()
        .map(|&i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let at = |dx: isize, dy: isize| {
                hp[((y + dy).rem_euclid(h as isize) as usize) * w + (x + dx).rem_euclid(w as isize) as usize]
            };
            let gx = at(1, 0) - at(-1, 0);
            let gy = at(0, 1) - at(0, -1);
            (gx * gx + gy * gy).sqrt()
        })
        .collect();
    g.sort_by(|a, b| a.total_cmp(b));
    let median = g[g.len() / 2].max(1e-4);
    let p99 = g[(g.len() * 99 / 100).min(g.len() - 1)];
    Detail { rms, edginess: p99 / median, low_rms: 0.0 }
}

/// The map of a car texture (top level RGBA8, `w` × `h`, at most
/// [`PACK_MAX`] a side) for its relief `regions`; None when no region is
/// accepted ([`car_verdicts`]).
pub fn build_car_normals(rgba: &[u8], w: u32, h: u32, regions: &[CarRegion]) -> Option<CarMap> {
    let verdicts = car_verdicts(rgba, w, h, regions);
    if verdicts.iter().all(|v| v.1 != NormalVerdict::Map) {
        return None;
    }
    let (wu, hu) = (w as usize, h as usize);
    let mask = car_mask(rgba, wu, hu, regions);
    let hp = car_high_pass(rgba, wu, hu, &mask);
    // Per texel: its region's tilt scale and soft clip (0 = flat).
    let per: Vec<(f32, f32)> = verdicts
        .iter()
        .map(|(c, v, d)| {
            if *v == NormalVerdict::Map {
                (SLOPE * c.params().normal / d.rms.max(MIN_RMS), 2.5 * d.rms.max(1e-4))
            } else {
                (0.0, 1.0)
            }
        })
        .collect();
    let height: Vec<f32> = hp
        .iter()
        .zip(&mask)
        .map(|(v, m)| {
            if *m > 0 {
                let c = per[*m as usize - 1].1;
                c * (v / c).tanh()
            } else {
                0.0
            }
        })
        .collect();
    let at =
        |x: isize, y: isize| height[(y.rem_euclid(hu as isize) as usize) * wu + x.rem_euclid(wu as isize) as usize];
    let mut top = vec![[0.0f32, 0.0, 1.0]; wu * hu];
    for y in 0..hu as isize {
        for x in 0..wu as isize {
            let m = mask[y as usize * wu + x as usize];
            if m == 0 || per[m as usize - 1].0 == 0.0 {
                continue;
            }
            let k = per[m as usize - 1].0;
            let gx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x - 1, y)
                - at(x - 1, y + 1))
                / 8.0;
            let gy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                - at(x - 1, y - 1)
                - 2.0 * at(x, y - 1)
                - at(x + 1, y - 1))
                / 8.0;
            let v = [-gx * k, -gy * k, 1.0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            top[y as usize * wu + x as usize] = [v[0] / l, v[1] / l, v[2] / l];
        }
    }
    let q = |v: f32| (v * 127.0).round().clamp(-127.0, 127.0) as i8;
    let pack = |l: &[[f32; 3]]| -> Vec<[i8; 2]> {
        l.iter()
            .map(|v| {
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
                [q(v[0] / len), q(v[1] / len)]
            })
            .collect()
    };
    let mut levels = vec![pack(&top)];
    let (mut cur, mut cw, mut ch) = (top, wu, hu);
    while cw > 1 || ch > 1 {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![[0.0f32; 3]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut v = [0.0; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = cur[(2 * y + dy).min(ch - 1) * cw + (2 * x + dx).min(cw - 1)];
                    for k in 0..3 {
                        v[k] += p[k] * 0.25;
                    }
                }
                next[y * nw + x] = v;
            }
        }
        levels.push(pack(&next));
        (cur, cw, ch) = (next, nw, nh);
    }
    Some(CarMap { width: w, height: h, levels })
}
