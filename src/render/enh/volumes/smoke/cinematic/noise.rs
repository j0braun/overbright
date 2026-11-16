use super::*;

/// The noise volume's edge (texels; it tiles).
pub const NOISE: usize = 64;

fn hash(x: i32, y: i32, z: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F)
        ^ s.wrapping_mul(0x165_667B1);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^ (h >> 15)
}

/// Periodic gradient noise (`period` cells along each axis) at `p` (cells).
fn perlin(p: Vec3, period: i32, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = Vec3::new(fade(f.x), fade(f.y), fade(f.z));
    let grad = |cx: i32, cy: i32, cz: i32, d: Vec3| {
        let h = hash(cx.rem_euclid(period), cy.rem_euclid(period), cz.rem_euclid(period), seed);
        let g = [
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(-1.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(0.0, -1.0, 1.0),
            Vec3::new(0.0, 1.0, -1.0),
            Vec3::new(0.0, -1.0, -1.0),
        ][(h % 12) as usize];
        g.dot(d)
    };
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let mut v = [0.0f32; 8];
    for (k, item) in v.iter_mut().enumerate() {
        let o = [(k & 1) as i32, ((k >> 1) & 1) as i32, (k >> 2) as i32];
        *item = grad(x + o[0], y + o[1], z + o[2], f - Vec3::new(o[0] as f32, o[1] as f32, o[2] as f32));
    }
    let a = lerp(lerp(v[0], v[1], u.x), lerp(v[2], v[3], u.x), u.y);
    let b = lerp(lerp(v[4], v[5], u.x), lerp(v[6], v[7], u.x), u.y);
    lerp(a, b, u.z)
}

/// Periodic Worley noise (`period` cells): 1 − the distance to the
/// nearest feature point (billowy cells).
fn worley(p: Vec3, period: i32, seed: u32) -> f32 {
    let i = p.floor();
    let mut best = 9.0f32;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = Vec3::new(i.x + dx as f32, i.y + dy as f32, i.z + dz as f32);
                let h = hash(
                    (c.x as i32).rem_euclid(period),
                    (c.y as i32).rem_euclid(period),
                    (c.z as i32).rem_euclid(period),
                    seed ^ 0xA5A5,
                );
                let o = Vec3::new((h & 1023) as f32, ((h >> 10) & 1023) as f32, ((h >> 20) & 1023) as f32) / 1023.0;
                best = best.min((c + o - p).length_squared());
            }
        }
    }
    1.0 - best.sqrt().min(1.0)
}

/// The billow density at `p` (texels of the volume): Perlin fbm shaped by
/// inverted Worley cells (a "Perlin-Worley" cloud noise), 0..1; tiles.
pub fn billow(p: Vec3) -> f32 {
    let n = NOISE as f32;
    let fbm =
        perlin(p * (4.0 / n), 4, 1) * 0.6 + perlin(p * (8.0 / n), 8, 2) * 0.3 + perlin(p * (16.0 / n), 16, 3) * 0.15;
    let w = worley(p * (4.0 / n), 4, 4) * 0.65 + worley(p * (8.0 / n), 8, 5) * 0.35;
    let perlin01 = (fbm * 0.5 + 0.5).clamp(0.0, 1.0);
    // Remap the fbm by the cells: rounded lumps with wispy gaps.
    let lo = 1.0 - w;
    ((perlin01 - lo * 0.75) / (1.0 - lo * 0.75).max(1e-3)).clamp(0.0, 1.0) * 0.55 + w * 0.45
}

/// The noise volume (RGBA8, `NOISE`³): r the billow density, gba its
/// gradient (central differences, scaled to ±1 by `GRAD_SCALE`).
pub fn noise_volume() -> &'static [u8] {
    static V: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let n = NOISE;
        let mut d = vec![0.0f32; n * n * n];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    d[(z * n + y) * n + x] = billow(Vec3::new(x as f32, y as f32, z as f32));
                }
            }
        }
        // Equalised to its rank (uniform over 0..1): the shader's erosion
        // thresholds then read as the share of the puff kept.
        let mut order: Vec<u32> = (0..d.len() as u32).collect();
        order.sort_by(|&a, &b| d[a as usize].total_cmp(&d[b as usize]));
        let scale = 1.0 / (d.len() - 1) as f32;
        for (rank, &i) in order.iter().enumerate() {
            d[i as usize] = rank as f32 * scale;
        }
        let at = |x: usize, y: usize, z: usize| d[(z % n * n + y % n) * n + x % n];
        let mut out = vec![0u8; n * n * n * 4];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let g = Vec3::new(
                        at(x + 1, y, z) - at(x + n - 1, y, z),
                        at(x, y + 1, z) - at(x, y + n - 1, z),
                        at(x, y, z + 1) - at(x, y, z + n - 1),
                    ) * (0.5 * GRAD_SCALE);
                    let i = ((z * n + y) * n + x) * 4;
                    let q = |v: f32| ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0).round() as u8;
                    out[i] = (at(x, y, z) * 255.0).round() as u8;
                    out[i + 1] = q(g.x);
                    out[i + 2] = q(g.y);
                    out[i + 3] = q(g.z);
                }
            }
        }
        out
    })
}

/// The gradient's scale in the volume (density per texel × this ≈ ±1).
pub const GRAD_SCALE: f32 = 16.0;
