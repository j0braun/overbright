//! The Graphics page's Car Shadow / Neon **Ultra** tier for the underglow
//! (a clone deviation, docs/neon-quality.md): a cleaned, upsampled,
//! high-precision copy of the car's `%s_NEON` texture.
//!
//! The native texture (render info +0x344, drawn by the shadow grid's second
//! pass 0x622D66 with SRCALPHA / ONE) is a 128 × 64 DXT1 grey glow mask.
//! DXT1 stores each 4 × 4 block as two RGB565 end points and 2-bit indices,
//! so the decoded gradient has 5-bit red / blue and 6-bit green steps (8/255
//! and 4/255; world.efx's `2·tex·colour` doubles them on screen), a
//! green / magenta tint where the channels round apart, and block-shaped
//! plateaus. Magnified on the ground (about 3.5 cm a texel, 15–25 px at
//! 4K close up) those show as contour bands and stair-stepped glow edges.
//!
//! [`enhance`] rebuilds the mask: grey texels (channel spread up to
//! [`GREY_SPREAD`]) to their luma, a small Gaussian across the block
//! seams, a 4× Mitchell–Netravali upsample, a full box-filtered mip chain,
//! half-float texels. The glow's shape, size, overall brightness and the
//! draw (grid, UVs, colour, blend) stay the native ones.
//!
//! The rebuild also fades the mask to 0 over its outer [`EDGE_TEXELS`] (the reported "cutoff line", docs/neon-quality.md): the
//! native `%s_NEON` masks still hold 8–18 / 255 on their border texels
//! (doubled by world.efx), so the glow stops in a straight line at the
//! shadow grid's edge (box × 1.05) instead of reaching 0. The original shows
//! the same line (Original stays native).

/// The upsampling factor.
pub const SCALE: u32 = 4;
/// Channel spread (out of 255) under which a texel counts as grey; DXT1's
/// 565 rounding spreads a grey texel by up to about 8.
pub const GREY_SPREAD: u8 = 12;
/// The deblocking Gaussian's σ, in source texels.
pub const SIGMA: f32 = 0.85;

/// Width (in source texels) of the fade to 0 along each border, capped at
/// an eighth of the axis.
pub const EDGE_TEXELS: f32 = 6.0;

/// The edge weight of texel `i` of `n` for a fade `width` texels wide
/// (smoothstep; the outermost texel 0).
fn edge_weight(i: usize, n: usize, width: f32) -> f32 {
    if width <= 0.0 || n < 2 {
        return 1.0;
    }
    let t = (i.min(n - 1 - i) as f32 / width).min(1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Multiply `img` (`w` × `h`) by the border fade, `wx` / `wy` texels wide.
fn fade_edges(img: &mut [[f32; 4]], w: usize, h: usize, wx: f32, wy: f32) {
    for y in 0..h {
        let fy = edge_weight(y, h, wy);
        for x in 0..w {
            let f = fy * edge_weight(x, w, wx);
            let p = &mut img[y * w + x];
            for c in 0..3 {
                p[c] *= f;
            }
        }
    }
}

/// The fade's width in source texels for an axis of `n` texels.
fn edge_texels(n: usize) -> f32 {
    EDGE_TEXELS.min(n as f32 / 8.0)
}

/// The rebuilt texture: `levels[i]` is mip `i`, RGBA f32 rows top down.
pub struct Enhanced {
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<[f32; 4]>>,
}

impl Enhanced {
    /// The levels as `Rgba16Float` texels back to back (the layout the
    /// texture uploads take: level 0 first, rows unpadded).
    pub fn rgba16f_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.levels.iter().map(|l| l.len() * 8).sum());
        for level in &self.levels {
            for px in level {
                for c in px {
                    out.extend_from_slice(&f16_bits(*c).to_le_bytes());
                }
            }
        }
        out
    }

    pub fn mips(&self) -> u32 {
        self.levels.len() as u32
    }
}

/// `rgba` (8-bit, `width` × `height`, as `Texture::decode_rgba`) rebuilt
/// at [`SCALE`]× the size. `None` for an empty or mismatched image.
/// The border is faded to 0 on level 0 and again on every mip level the
/// fade still spans a texel of (the box mips would pull the glow back onto
/// the border otherwise).
pub fn enhance(rgba: &[u8], width: u32, height: u32) -> Option<Enhanced> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return None;
    }
    let scale = SCALE as usize;
    let (fx, fy) = (edge_texels(w) * scale as f32, edge_texels(h) * scale as f32);
    // Grey texels to their luma (the 565 tint out), as 0..1 floats.
    let src: Vec<[f32; 4]> = rgba
        .chunks_exact(4)
        .map(|p| {
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            let f = |v: u8| v as f32 / 255.0;
            if hi - lo <= GREY_SPREAD {
                let y = (f(p[0]) + 2.0 * f(p[1]) + f(p[2])) * 0.25;
                [y, y, y, f(p[3])]
            } else {
                [f(p[0]), f(p[1]), f(p[2]), f(p[3])]
            }
        })
        .collect();
    let smooth = gaussian(&src, w, h, SIGMA);
    let (uw, uh) = (w * scale, h * scale);
    let mut top = upsample(&smooth, w, h, uw, uh);
    fade_edges(&mut top, uw, uh, fx, fy);
    Some(mip_chain(top, uw, uh, fx, fy))
}

/// Level 0 `top` (`uw` × `uh`) and its box mips down to 1 × 1, each level's
/// border faded again while the fade (`fx` / `fy` level-0 texels) is at
/// least a texel wide there.
fn mip_chain(top: Vec<[f32; 4]>, uw: usize, uh: usize, fx: f32, fy: f32) -> Enhanced {
    let mut levels = vec![top];
    let (mut lw, mut lh) = (uw, uh);
    let (mut fx, mut fy) = (fx, fy);
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let prev = levels.last().unwrap();
        let mut next = vec![[0.0f32; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                let mut n = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(lw - 1), (2 * y + dy).min(lh - 1));
                    let p = prev[sy * lw + sx];
                    for c in 0..4 {
                        acc[c] += p[c];
                    }
                    n += 1.0;
                }
                next[y * nw + x] = acc.map(|v| v / n);
            }
        }
        (fx, fy) = (fx * nw as f32 / lw as f32, fy * nh as f32 / lh as f32);
        if fx >= 1.0 && fy >= 1.0 {
            fade_edges(&mut next, nw, nh, fx, fy);
        }
        levels.push(next);
        (lw, lh) = (nw, nh);
    }
    Enhanced { width: uw as u32, height: uh as u32, levels }
}

/// Separable Gaussian, edges clamped (the mask's border is black).
fn gaussian(src: &[[f32; 4]], w: usize, h: usize, sigma: f32) -> Vec<[f32; 4]> {
    let r = (sigma * 3.0).ceil() as i32;
    let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    let k: Vec<f32> = k.iter().map(|v| v / sum).collect();
    let pass = |img: &[[f32; 4]], horizontal: bool| -> Vec<[f32; 4]> {
        let mut out = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for (j, kv) in k.iter().enumerate() {
                    let o = j as i32 - r;
                    let (sx, sy) = if horizontal {
                        ((x as i32 + o).clamp(0, w as i32 - 1) as usize, y)
                    } else {
                        (x, (y as i32 + o).clamp(0, h as i32 - 1) as usize)
                    };
                    let p = img[sy * w + sx];
                    for c in 0..4 {
                        acc[c] += p[c] * kv;
                    }
                }
                out[y * w + x] = acc;
            }
        }
        out
    };
    pass(&pass(src, true), false)
}

/// Mitchell–Netravali (B = C = 1/3): smooth, little ringing.
fn mitchell(x: f32) -> f32 {
    let (b, c) = (1.0 / 3.0, 1.0 / 3.0);
    let x = x.abs();
    if x < 1.0 {
        ((12.0 - 9.0 * b - 6.0 * c) * x * x * x + (-18.0 + 12.0 * b + 6.0 * c) * x * x + (6.0 - 2.0 * b)) / 6.0
    } else if x < 2.0 {
        ((-b - 6.0 * c) * x * x * x + (6.0 * b + 30.0 * c) * x * x + (-12.0 * b - 48.0 * c) * x + (8.0 * b + 24.0 * c))
            / 6.0
    } else {
        0.0
    }
}

/// Bicubic resample on texel centres (as a GPU samples), clamped to 0..1.
fn upsample(src: &[[f32; 4]], w: usize, h: usize, uw: usize, uh: usize) -> Vec<[f32; 4]> {
    let weights = |out: usize, size_in: usize, size_out: usize| -> Vec<Vec<(usize, f32)>> {
        (0..out)
            .map(|i| {
                let s = (i as f32 + 0.5) * size_in as f32 / size_out as f32 - 0.5;
                let base = s.floor() as i32;
                let taps: Vec<(usize, f32)> = (base - 1..=base + 2)
                    .map(|t| (t.clamp(0, size_in as i32 - 1) as usize, mitchell(s - t as f32)))
                    .collect();
                let sum: f32 = taps.iter().map(|t| t.1).sum();
                taps.into_iter().map(|(t, wgt)| (t, wgt / sum)).collect()
            })
            .collect()
    };
    let (wx, wy) = (weights(uw, w, uw), weights(uh, h, uh));
    // Across, then down.
    let mut mid = vec![[0.0f32; 4]; uw * h];
    for y in 0..h {
        for (x, taps) in wx.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for &(t, k) in taps {
                let p = src[y * w + t];
                for c in 0..4 {
                    acc[c] += p[c] * k;
                }
            }
            mid[y * uw + x] = acc;
        }
    }
    let mut out = vec![[0.0f32; 4]; uw * uh];
    for (y, taps) in wy.iter().enumerate() {
        for x in 0..uw {
            let mut acc = [0.0f32; 4];
            for &(t, k) in taps {
                let p = mid[t * uw + x];
                for c in 0..4 {
                    acc[c] += p[c] * k;
                }
            }
            out[y * uw + x] = acc.map(|v| v.clamp(0.0, 1.0));
        }
    }
    out
}

/// IEEE half from an f32 in 0..1 (round to nearest; below the smallest
/// normal half, 6.1e-5, to 0).
pub fn f16_bits(v: f32) -> u16 {
    if !(v > 6.103_515_6e-5) {
        return 0;
    }
    let b = v.min(65504.0).to_bits();
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let man = b & 0x7F_FFFF;
    // Round the 23-bit mantissa to 10 bits (a carry bumps the exponent).
    let rounded = (((exp as u32) << 10) | (man >> 13)) + ((man >> 12) & 1);
    rounded as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn half_to_f32(h: u16) -> f32 {
        let exp = ((h >> 10) & 0x1F) as i32;
        let man = (h & 0x3FF) as f32;
        if exp == 0 {
            return man * 2f32.powi(-24);
        }
        (1.0 + man / 1024.0) * 2f32.powi(exp - 15)
    }

    #[test]
    fn half_round_trip() {
        for v in [0.0f32, 1.0, 0.5, 0.25, 0.123, 0.0039, 0.9999] {
            let back = half_to_f32(f16_bits(v));
            assert!((back - v).abs() <= v * 0.001 + 1e-4, "{v} -> {back}");
        }
    }

    /// A DXT1-like ramp (565 steps, green rounding apart, flat 4-texel
    /// plateaus) comes back as a smooth grey ramp: no tint, steps far below
    /// one 8-bit level per output texel, the same overall brightness.
    #[test]
    fn quantised_ramp_comes_back_smooth() {
        let (w, h) = (64u32, 8u32);
        let mut rgba = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                // Plateaus of 4 texels, 5-bit red / blue, 6-bit green.
                let v = (x / 4) as f32 * 4.0 / (w - 1) as f32 * 0.42;
                let q5 = ((v * 31.0).round() / 31.0 * 255.0).round() as u8;
                let q6 = ((v * 63.0).round() / 63.0 * 255.0).round() as u8;
                rgba.extend_from_slice(&[q5, q6, q5, 255]);
            }
        }
        let e = enhance(&rgba, w, h).unwrap();
        assert_eq!((e.width, e.height), (256, 32));
        assert_eq!(e.mips(), 9);
        let row = &e.levels[0][16 * 256..17 * 256];
        // Grey.
        assert!(row.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
        // Largest step between neighbours, in 8-bit levels: native plateaus
        // jump by a whole 565 step (8 levels) at once.
        let native_max = rgba
            .chunks_exact(4)
            .take(w as usize)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|p| p[1][0].abs_diff(p[0][0]))
            .max()
            .unwrap();
        // (Inside the border fade, EDGE_TEXELS = 6 source texels = 24 here.)
        let inner = &row[24..256 - 24];
        let step = inner.windows(2).map(|p| (p[1][0] - p[0][0]).abs() * 255.0).fold(0.0f32, f32::max);
        assert!(native_max >= 8, "the fixture's native steps ({native_max})");
        assert!(step < 1.5, "largest enhanced step {step} levels");
        // Brightness kept inside the border fade (mean within 2 %).
        let mean_native: f32 = rgba
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, _)| (8..56).contains(&(i % w as usize)))
            .map(|(_, p)| (p[0] as f32 + 2.0 * p[1] as f32 + p[2] as f32) / (4.0 * 255.0))
            .sum::<f32>()
            / (48 * h) as f32;
        let mean: f32 = e.levels[0]
            .iter()
            .enumerate()
            .filter(|(i, _)| (32..224).contains(&(i % 256)) && (4..28).contains(&(i / 256)))
            .map(|(_, p)| p[0])
            .sum::<f32>()
            / (192 * 24) as f32;
        assert!((mean - mean_native).abs() < 0.02 * mean_native.max(0.01), "{mean} vs {mean_native}");
        // The chain ends at 1 × 1 and the bytes match it.
        assert_eq!(e.levels.last().unwrap().len(), 1);
        assert_eq!(e.rgba16f_bytes().len(), e.levels.iter().map(|l| l.len() * 8).sum::<usize>());
    }

    /// Coloured texels (a channel spread above GREY_SPREAD) keep their hue.
    #[test]
    fn coloured_texels_keep_their_hue() {
        let rgba: Vec<u8> = (0..16).flat_map(|_| [200u8, 20, 180, 255]).collect();
        let e = enhance(&rgba, 4, 4).unwrap();
        let p = e.levels[0][8 * 16 + 8];
        assert!((p[0] - 200.0 / 255.0).abs() < 0.01 && (p[1] - 20.0 / 255.0).abs() < 0.01);
    }

    /// The "cutoff line": a mask that is still lit on its border (the native
    /// `%s_NEON` hold 8–18 / 255 there) comes back 0 on the border of every
    /// level the fade spans, ramps up smoothly, and is untouched inside.
    #[test]
    fn border_fades_to_zero() {
        let (w, h) = (128u32, 64u32);
        // Flat grey 18 / 255 (the brightest native border texel), alpha 255.
        let rgba: Vec<u8> = (0..w * h).flat_map(|_| [18u8, 18, 18, 255]).collect();
        {
            let e = enhance(&rgba, w, h).unwrap();
            let s = SCALE as usize;
            assert_eq!((e.width, e.height), (w * s as u32, h * s as u32));
            let (lw, lh) = (e.width as usize, e.height as usize);
            let l0 = &e.levels[0];
            // The four borders are black.
            for x in 0..lw {
                assert_eq!(l0[x][0], 0.0, "top {x}");
                assert_eq!(l0[(lh - 1) * lw + x][0], 0.0, "bottom {x}");
            }
            for y in 0..lh {
                assert_eq!(l0[y * lw][0], 0.0, "left {y}");
                assert_eq!(l0[y * lw + lw - 1][0], 0.0, "right {y}");
            }
            // Inside the fade the native value is kept.
            let mid = l0[(lh / 2) * lw + lw / 2][0];
            assert!((mid - 18.0 / 255.0).abs() < 0.002, "centre {mid}");
            // The ramp along the middle row rises monotonically, no step
            // above a quarter of the value.
            let row = &l0[(lh / 2) * lw..(lh / 2 + 1) * lw];
            let ramp = &row[..(EDGE_TEXELS as usize * s) + 1];
            assert!(ramp.windows(2).all(|p| p[1][0] >= p[0][0] && p[1][0] - p[0][0] < 0.25 * mid), "ramp");
            // Mips that still span the fade keep a black border.
            for (k, l) in e.levels.iter().enumerate().skip(1) {
                let (mw, mh) = ((lw >> k).max(1), (lh >> k).max(1));
                let fx = EDGE_TEXELS * s as f32 * mw as f32 / lw as f32;
                let fy = EDGE_TEXELS * s as f32 * mh as f32 / lh as f32;
                if fx >= 1.0 && fy >= 1.0 {
                    assert_eq!(l[0][0], 0.0, "mip {k} corner");
                    assert_eq!(l[(mh / 2) * mw][0], 0.0, "mip {k} left edge");
                }
            }
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert!(enhance(&[], 0, 0).is_none());
        assert!(enhance(&[0; 12], 2, 2).is_none());
    }
}
