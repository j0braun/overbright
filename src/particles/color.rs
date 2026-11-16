use super::*;

/// 0x488270: RGB (0..1) to HSL (hue 0..1), the extremes taken against 1
/// and 0 first.
pub(super) fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let lo = 1.0f32.min(r).min(g).min(b);
    let hi = 0.0f32.max(r).max(g).max(b);
    let l = (hi + lo) * 0.5;
    if lo == hi {
        return (0.0, 0.0, l);
    }
    let d = hi - lo;
    let s = if l >= 0.5 { d / ((2.0 - hi) - lo) } else { d / (hi + lo) };
    let h = if hi == r {
        (g - b) / d
    } else if hi == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 0.16666667, s, l)
}

/// 0x4883F0: HSL back to RGB (0..1).
pub(super) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l >= 0.5 { (s + l) - s * l } else { (s + 1.0) * l };
    let p = (l + l) - q;
    let wrap = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        t
    };
    let channel = |t: f32| {
        if t * 6.0 < 1.0 {
            (q - p) * t * 6.0 + p
        } else if t + t < 1.0 {
            q
        } else if t * 3.0 < 2.0 {
            (0.6666667 - t) * (q - p) * 6.0 + p
        } else {
            p
        }
    };
    (channel(wrap(h + 0.33333334)), channel(wrap(h)), channel(wrap(h - 0.33333334)))
}

/// 0x48B920 for one description: its four colour keys (+0xA0) shifted in
/// HSL — hue by `hue` (clamped ±1, wrapped into 0..1), saturation by `sat`
/// (clamped ±1; the result 0..1), luminance by `lum` (the result 0..1) —
/// alpha kept, then 0x487940 fits the cubic through them at the key times
/// (+0x90 × 1/3). The clone solves that cubic directly (Lagrange through
/// the four points); the game inverts the 4 × 4 power matrix (0x4874E0).
pub fn recolor(d: &Description, hue: f32, sat: f32, lum: f32) -> Cubic {
    let hue = hue.clamp(-1.0, 1.0);
    let sat = sat.clamp(-1.0, 1.0);
    let keys: [[f32; 4]; 4] = d.key_colors.map(|k| {
        let (mut h, s, l) = rgb_to_hsl(k[0] / 255.0, k[1] / 255.0, k[2] / 255.0);
        h += hue;
        while h < 0.0 {
            h += 1.0;
        }
        while h > 1.0 {
            h -= 1.0;
        }
        let s = (s + sat).clamp(0.0, 1.0);
        let l = (l + lum).clamp(0.0, 1.0);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        [r * 255.0, g * 255.0, b * 255.0, k[3]]
    });
    let t = d.key_times.map(|x| x * 0.33333334);
    // The cubic's rows: t³, t², t, 1 (Cubic::at).
    let mut m = [[0.0f32; 4]; 4];
    for i in 0..4 {
        // Lagrange basis i: Π (x − t_j) / (t_i − t_j).
        let others: Vec<f32> = (0..4).filter(|&j| j != i).map(|j| t[j]).collect();
        let den: f32 = others.iter().map(|&tj| t[i] - tj).product();
        if den == 0.0 {
            continue;
        }
        let (a, b, c) = (others[0], others[1], others[2]);
        let poly = [1.0, -(a + b + c), a * b + a * c + b * c, -(a * b * c)];
        for (row, coef) in poly.iter().enumerate() {
            for ch in 0..4 {
                m[row][ch] += coef / den * keys[i][ch];
            }
        }
    }
    Cubic(m)
}
