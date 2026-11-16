use super::*;

/// 0x467FF0: millibels (≤ 0) to a level 0..0x7FFF: 0 → 0x7FFF, else q =
/// trunc(mB / −602), r = mB % 602 (≤ 0), the table at 601 + r shifted
/// right by q; below 50 → 0. (0x477AA0 maps mB < −10000 to 0 and > 0 to
/// 0x7FFF before calling it.)
pub fn db_to_lin(mb: i32) -> i32 {
    if mb == 0 {
        return 0x7FFF;
    }
    let q = mb / -602;
    let r = mb % 602;
    let v = DB_TO_LIN[(601 + r).clamp(0, 601) as usize] >> (q & 31);
    if v < 50 { 0 } else { v }
}

/// 0x468050: a level (0..0x7FFF) to millibels (−10000 for 0): e = the
/// count of leading zeros below bit 14, the mantissa below the top bit
/// indexes the table (shifted to 9 bits), plus (−1 − e) · 602; clamped at
/// −10000.
pub fn lin_to_db(x: i32) -> i32 {
    if x == 0 {
        return -10000;
    }
    let mut e = 0;
    while x & (0x4000 >> e) == 0 {
        if e == 16 {
            return -10000;
        }
        e += 1;
    }
    let m = x - (0x4000 >> e);
    let idx = if e >= 5 { (m << (e - 5)) | ((1 << (e - 5)) - 1) } else { m >> (5 - e) };
    let v = (-1 - e) * 602 + LIN_TO_DB[(idx as usize).min(511)];
    v.max(-10000)
}

pub(super) fn fade(k: i32) -> i32 {
    if !(0..=511).contains(&k) {
        return 0;
    }
    super::super::carfx::fade(k as usize)
}

/// 0x468160(t, x, db): the mixer's curves over x in 0..0x7FFF. 0 the fade
/// table (cos, 32767 → 0) blended to the next entry by bits 1–4 of x
/// (unless the entry is 0); 2 its square (>> 15); 4 the mirrored fade (0
/// → 32767); 6 its square; 8 0x7FFF − x; the odd types run the even one
/// below on 0x7FFF − x (as levels) or negate its mB (with `db`); > 9 → 0.
/// With `db` the even types give millibels (types 2 / 6 as twice the
/// base type's mB, −10000 under −9600).
pub fn curve(t: u32, x: i32, db: bool) -> i32 {
    match t {
        1 | 3 | 5 | 7 | 9 => {
            if db {
                -curve(t - 1, x, true)
            } else {
                curve(t - 1, 0x7FFF - x, false)
            }
        }
        0 | 4 => {
            let i = x >> 6;
            let f = ((x & 0x1E) << 9) | 0x3FF;
            let v = if t == 0 {
                let a = fade(i);
                if a == 0 { 0 } else { a + (((fade(i + 1) - a) * f) >> 15) }
            } else {
                let j = 0x1FF - i;
                let a = 0x7FFF - fade(j);
                let b = 0x7FFF - fade(j + 1);
                a + (((b - a) * f) >> 15)
            };
            if db {
                let d = lin_to_db(v);
                if d < -0x2580 { -10000 } else { d }
            } else {
                v
            }
        }
        2 | 6 => {
            if db {
                let d = curve(t - 2, x, true) * 2;
                if d < -0x2580 { -10000 } else { d }
            } else {
                let c = curve(t - 2, x, false);
                (c * c) >> 15
            }
        }
        8 => {
            if db {
                lin_to_db(0x7FFF - x)
            } else {
                0x7FFF - x
            }
        }
        _ => 0,
    }
}

/// 0x43C490: the 16-bit arc sine (0x4000 = 90°) by the table 0x7EA628:
/// 1 or more → 0x4000, a negative input mirrored (0x10000 − a).
pub fn asin16(x: f32) -> u16 {
    let neg = x < 0.0;
    let x = x.abs();
    if x >= 1.0 {
        return if neg { 0xC000 } else { 0x4000 };
    }
    let v = (x as f64 * 65536.0) as i32;
    let (mut edx, mut ecx, mut esi) = (0x8000i32, 0x8000i32, 0i32);
    if v >= ecx {
        while esi < 11 {
            edx >>= 1;
            ecx += edx;
            esi += 1;
            if v < ecx {
                break;
            }
        }
    }
    let edi = ecx - edx;
    let frac = (v - edi) >> (11 - esi);
    let base = (edx >> 4) * frac + edi;
    let (a, s) = ASIN[((esi << 4) + frac).clamp(0, 191) as usize];
    let t = ((x as f64 - base as f64 / 65536.0) * s as f64 * -65536.0) as i32;
    let r = a.wrapping_sub(t as u16);
    if neg { (0x10000 - r as i32) as u16 } else { r }
}

/// 0x45DE60: the azimuth word of a point seen from `from` facing the unit
/// vector `dir` (2D, the game's x / y): 0x4000 − asin16(n · dir) with n
/// the normalised offset (0x43CE50: (1, 0) for a zero offset), mirrored
/// (0xFFFF − a) when n.x · dir.y − n.y · dir.x < 0. 0 straight ahead,
/// 0x4000 with the cross product ≥ 0, 0x8000 behind.
pub fn azimuth(point: [f32; 2], from: [f32; 2], dir: [f32; 2]) -> u16 {
    let (dx, dy) = (point[0] - from[0], point[1] - from[1]);
    let l = (dx * dx + dy * dy).sqrt();
    let n = if l == 0.0 { [1.0, 0.0] } else { [dx * (1.0 / l), dy * (1.0 / l)] };
    let a = 0x4000u16.wrapping_sub(asin16(n[0] * dir[0] + n[1] * dir[1]));
    if n[0] * dir[1] - n[1] * dir[0] < 0.0 { 0xFFFF - a } else { a }
}
