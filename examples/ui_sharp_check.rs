//! ui_sharp_check [FACTOR ...]: HD UI Art's sharp bilinear (ui.wgsl
//! `sharp`, the same arithmetic on the CPU) over every front-end texture
//! drawn magnified, against the native bilinear filter, at each FACTOR
//! target pixels per texel (default 2.25 and 4.5: 1080p and 2160p).
//!
//! Per texture and factor, every output pixel of the texture magnified
//! (phase as the game's: pixel i at texel coordinate i / f + 1/2):
//! * `centre`: at each texel's centre the output is that texel
//!   (premultiplied: equal alpha, equal colour where alpha > 0);
//! * `inside`: no output value outside its 2 × 2 source texels' range
//!   (premultiplied, per channel): no new extremes, halos or overshoot;
//! * `exact`: the share of output pixels equal to one source texel (to
//!   the 8-bit value); the rest lie on the 1/k-texel step between texels.
//!
//! Exits non-zero if any centre or range check fails.
fn main() -> anyhow::Result<()> {
    let root = std::path::PathBuf::from(std::env::var("NFSU2_ROOT")?);
    let factors: Vec<f32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let factors = if factors.is_empty() { vec![2.25, 4.5] } else { factors };
    let assets = nfsu2::frontend::Assets::load(&root)?;
    let mut textures: Vec<_> = assets.textures.values().filter(|t| t.width >= 2 && t.height >= 2 && t.width.max(t.height) <= 1024).collect();
    textures.sort_by_key(|t| t.name.clone());
    let mut failures = 0usize;
    for &f in &factors {
        let (mut px, mut centres, mut bad_centre, mut bad_range) = (0u64, 0u64, 0u64, 0u64);
        let (mut exact_sharp, mut exact_bilinear, mut new_sharp, mut new_bilinear) = (0u64, 0u64, 0u64, 0u64);
        for t in &textures {
            let (w, h) = (t.width as i64, t.height as i64);
            let rgba = t.decode_rgba();
            let pre: Vec<[f32; 4]> = rgba
                .chunks_exact(4)
                .map(|c| {
                    let a = c[3] as f32 / 255.0;
                    [c[0] as f32 / 255.0 * a, c[1] as f32 / 255.0 * a, c[2] as f32 / 255.0 * a, a]
                })
                .collect();
            let at = |x: i64, y: i64| pre[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize];
            let palette: std::collections::HashSet<[u8; 4]> = pre.iter().map(|p| q8(*p)).collect();
            let k = (f + 1e-3).floor().max(1.0);
            // One axis: (texel left, weight of the right one) of the sharp
            // filter and of plain bilinear at texel coordinate x.
            let sharp_w = |x: f32| {
                let b = (x + 0.5).floor();
                (b as i64 - 1, ((x - b) * k + 0.5).clamp(0.0, 1.0))
            };
            let bil_w = |x: f32| {
                let p = x - 0.5;
                (p.floor() as i64, p - p.floor())
            };
            let filt = |(x0, wx): (i64, f32), (y0, wy): (i64, f32)| {
                let mut o = [0.0f32; 4];
                let (mut lo, mut hi) = ([1.0f32; 4], [0.0f32; 4]);
                for (dx, dy, ww) in [(0, 0, (1.0 - wx) * (1.0 - wy)), (1, 0, wx * (1.0 - wy)), (0, 1, (1.0 - wx) * wy), (1, 1, wx * wy)] {
                    let c = at(x0 + dx, y0 + dy);
                    for i in 0..4 {
                        o[i] += c[i] * ww;
                        lo[i] = lo[i].min(c[i]);
                        hi[i] = hi[i].max(c[i]);
                    }
                }
                (o, lo, hi)
            };
            let (ow, oh) = ((w as f32 * f).floor() as i64, (h as f32 * f).floor() as i64);
            for oy in 0..oh {
                let y = oy as f32 / f + 0.5;
                for ox in 0..ow {
                    let x = ox as f32 / f + 0.5;
                    let (s, lo, hi) = filt(sharp_w(x), sharp_w(y));
                    px += 1;
                    if (0..4).any(|i| s[i] < lo[i] - 1e-5 || s[i] > hi[i] + 1e-5) {
                        bad_range += 1;
                    }
                    let qs = q8(s);
                    if palette.contains(&qs) {
                        exact_sharp += 1;
                    } else {
                        new_sharp += 1;
                    }
                    let (b, _, _) = filt(bil_w(x), bil_w(y));
                    if palette.contains(&q8(b)) {
                        exact_bilinear += 1;
                    } else {
                        new_bilinear += 1;
                    }
                }
            }
            // Texel centres, exactly.
            for ty in 0..h {
                for tx in 0..w {
                    let (s, _, _) = filt(sharp_w(tx as f32 + 0.5), sharp_w(ty as f32 + 0.5));
                    centres += 1;
                    if q8(s) != q8(at(tx, ty)) {
                        bad_centre += 1;
                    }
                }
            }
        }
        failures += (bad_centre + bad_range) as usize;
        let pc = |n: u64| 100.0 * n as f64 / px.max(1) as f64;
        println!(
            "factor {f}: {} textures, {px} output pixels; texel centres {centres}, not the texel {bad_centre}; outside the 2x2 range {bad_range}; \
             sharp: {:.2} % a source texel, {:.2} % on a step ({new_sharp}); native bilinear: {:.2} % a source texel, {:.2} % blended ({new_bilinear})",
            textures.len(),
            pc(exact_sharp),
            pc(new_sharp),
            pc(exact_bilinear),
            pc(new_bilinear)
        );
    }
    anyhow::ensure!(failures == 0, "{failures} failures");
    Ok(())
}

/// Premultiplied 0..1 → 8-bit (the comparison's resolution).
fn q8(p: [f32; 4]) -> [u8; 4] {
    p.map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8)
}
