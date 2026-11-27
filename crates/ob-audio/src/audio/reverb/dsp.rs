pub(super) const PI: f32 = std::f32::consts::PI; // 0x786D98
pub(super) const TWO_PI: f32 = 2.0 * std::f32::consts::PI; // 0x786D9C
/// The denormal guard 0x7C6A34 (1e−20).
pub(super) const GUARD: f32 = 1e-20;

/// 0x72A032: cos x by its Taylor series to x¹² (2π subtracted while x >
/// 2π). Computed as the x87 does, in extended precision (here f64).
pub fn cos_approx(x: f32) -> f64 {
    let mut x = x as f64;
    while x > TWO_PI as f64 {
        x -= TWO_PI as f64;
    }
    let a = x * x;
    let (b, c) = (a * a, a * a * a);
    let d = b * b;
    let k = |v: f32| v as f64;
    1.0 - a * 0.5 + b * k(0.041_666_668) - c * k(0.001_388_888_9) + d * k(2.480_158_8e-5) - d * a * k(2.755_732e-7)
        + c * c * k(2.087_675_8e-9)
}

/// 0x72A0AE: sin x by its Taylor series to x¹³ (x¹¹ stored as a float).
pub fn sin_approx(x: f32) -> f64 {
    let mut x = x as f64;
    while x > TWO_PI as f64 {
        x -= TWO_PI as f64;
    }
    let a = x * x;
    let b = a * x;
    let c = b * a;
    let d = c * a;
    let e = d * a;
    let f = (e * a) as f32 as f64;
    let k = |v: f32| v as f64;
    x - b * k(0.166_666_67) + c * k(0.008_333_33) - d * k(1.984_127e-4) + e * k(2.755_731_9e-6) - f * k(2.505_210_8e-8)
        + f * a * k(1.6059e-10)
}

/// 0x72D581: √(1 + x) by the binomial series to x⁷.
pub fn sqrt1p_approx(x: f32) -> f64 {
    let x = x as f64;
    let t1 = 0.5 * x;
    let t2 = t1 * x * -0.25;
    let t3 = t2 * x * -0.5;
    let t4 = t3 * x * -0.625;
    let t5 = t4 * x * (-0.7f32 as f64);
    let t6 = t5 * x * -0.75;
    let t7 = t6 * x * (-0.7857f32 as f64);
    t7 + t6 + t5 + t4 + t3 + t2 + t1 + 1.0
}

/// 0x7339E9's search: the smallest prime ≥ x (x ≥ 3; divisors 2 ..
/// isqrt(x) + 1, 0x7339D1).
fn next_prime(mut x: i32) -> i32 {
    'next: loop {
        let limit = (x as f64).sqrt() as i32 + 1;
        for d in 2..=limit {
            if x % d == 0 {
                x += 1;
                continue 'next;
            }
            if d == limit {
                break;
            }
        }
        return x;
    }
}

/// 0x72D567(sr, ms): an allpass delay in samples, 16 · the smallest prime
/// ≥ max(3, sr · (ms / 16) / 1000) (integer divisions).
pub fn allpass_len(rate: i32, ms: i32) -> usize {
    let x = (rate.wrapping_mul(ms / 16) / 1000).max(3);
    16 * next_prime(x) as usize
}

/// 0x72D697: the 9-tap FIR's five coefficients {c0 (taps ±4) … c4 (centre)}
/// for type 2 (low-pass at f2), 3 (high-pass at f1) or 4 (band f1..f2),
/// Hamming-windowed (0.54 − 0.46 cos(kπ/4)) and normalised to unit gain at
/// DC (2, 4) or at Nyquist (3). Frequencies are fractions of Nyquist.
pub fn fir_design(kind: u8, mut f1: f32, f2: f32) -> [f32; 5] {
    if (kind == 3 || kind == 4) && 0.8 < f1 {
        f1 = 0.8;
    }
    let mut c = [0f32; 5];
    for k in 1..=4 {
        let w = (k as f32 * PI) as f64 as f32;
        let wd = w as f64;
        c[4 - k] = match kind {
            2 => (sin_approx((wd * f2 as f64) as f32) / wd) as f32,
            3 => (sin_approx((wd * f1 as f64) as f32) / wd) as f32,
            _ => {
                let s = (sin_approx(((f2 as f64 - f1 as f64) * wd * 0.5) as f32) / wd) as f32;
                let cs = cos_approx(((f2 as f64 + f1 as f64) * wd * 0.5) as f32);
                (cs * s as f64 * 2.0) as f32
            }
        };
    }
    c[4] = match kind {
        2 => f2,
        3 => f1,
        _ => (f2 as f64 - f1 as f64) as f32,
    };
    let mut sum = 0f32;
    for (k, ck) in c.iter_mut().enumerate() {
        let win = 0.54f32 as f64 - 0.46f32 as f64 * cos_approx((k as f32 * std::f32::consts::FRAC_PI_4) as f64 as f32);
        if kind == 3 {
            *ck = (-(win * *ck as f64)) as f32;
            sum = if k % 2 == 0 { (*ck as f64 + sum as f64) as f32 } else { (sum as f64 - *ck as f64) as f32 };
        } else {
            *ck = (win * *ck as f64) as f32;
            sum = (*ck as f64 + sum as f64) as f32;
        }
    }
    if kind == 3 {
        c[4] = (c[4] as f64 + 1.0) as f32;
        sum = (sum as f64 + 1.0) as f32;
    }
    let mut total = sum as f64 * 2.0 - c[4] as f64;
    if total < 0.0 {
        total = -total;
    }
    let s = 1.0 / total;
    for ck in &mut c {
        *ck = (s * *ck as f64) as f32;
    }
    c
}
