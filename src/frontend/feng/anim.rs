//! Track sampling (0x535500 → 0x52BDD0) and the slerp (0x519B00).

use super::Track;

/// A track's value at `t` ms (0x535500 → 0x52BDD0, linear tracks; the
/// game's only interpolation in the shipped packages but one).
///
/// The first `Kd` key is the base (track +0x18); the others are a list of
/// {time, delta} keys. 0x5052B0 finds the first key whose time is not
/// below `t` (else the last); with `f` the float fraction between the
/// previous key and it (1 when there is no previous key or `t` is at or
/// past it), the value is the base plus the delta: floats `a + (b − a)·f`
/// (0x505150), colours `a + ftol((b − a)·f + 0.5)` on the integers
/// (0x5051A0; ftol truncates), and a rotation's deltas are quaternions
/// slerped (0x519B00: the nearer of ±b; nlerp past a dot of 0.999)
/// and composed `delta · base` (0x4FE6B0), not added.
///
/// The track's loop mode (`FI` byte 3 & 0x7F): 0 holds the end keys; 1
/// wraps — past the last key it runs towards the first, which it reaches
/// at the track's length plus the first key's time, and before the first
/// key it comes from the last; 2 (ping-pong) reflects `t` past the length.
pub fn sample(track: &Track, t: u32) -> Vec<u32> {
    let mut out = track.base.clone();
    sample_into(track, t, &mut out);
    out
}

/// [`sample`] into `out`, which holds the track's base on entry (its
/// length): no allocation (the HUD samples every track every frame).
pub fn sample_into(track: &Track, t: u32, out: &mut [u32]) {
    let keys = &track.keys;
    if keys.is_empty() {
        return;
    }
    let len = track.length as i64;
    let mut t = t as i64;
    let mode = track.flags & 0x7F;
    if mode == 2 && t > len {
        t = 2 * len - t;
    }
    // 0x5052B0: the first key at or after t, else the last.
    let k = keys.iter().position(|k| k.0 as i32 as i64 >= t).unwrap_or(keys.len() - 1);
    let time = |i: usize| keys[i].0 as i32 as i64;
    let frac = |num: i64, den: i64| {
        let den = den as f32;
        if den <= 1e-5 { 0.0 } else { num as f32 / den }
    };
    // (from, to, f): f 0 takes `from`, 1 takes `to`.
    let (a, b, f) = match mode {
        1 => {
            if time(k) < t {
                // Past the last key: towards the first, over the rest of
                // the track's length.
                (k, 0, frac(t - time(k), len + time(0) - time(k)))
            } else if time(k) == t {
                (k, k, 0.0)
            } else if k == 0 {
                // Before the first key: from the last (0x505260(length)).
                let last = keys.iter().position(|q| q.0 as i32 as i64 >= len).unwrap_or(keys.len() - 1);
                let rest = len - time(last);
                (last, 0, frac(rest + t, rest + time(0)))
            } else {
                (k - 1, k, frac(t - time(k - 1), time(k) - time(k - 1)))
            }
        }
        _ => {
            if k == 0 || t >= time(k) {
                (k, k, 1.0)
            } else {
                (k - 1, k, frac(t - time(k - 1), time(k) - time(k - 1)))
            }
        }
    };
    let (ka, kb) = (&keys[a].1, &keys[b].1);
    let n = out.len().min(ka.len()).min(kb.len());
    let fl = |w: u32| f32::from_bits(w);
    match track.param {
        5 if n >= 4 => {
            let q = |k: &[u32]| glam::Quat::from_xyzw(fl(k[0]), fl(k[1]), fl(k[2]), fl(k[3]));
            let delta = if f == 0.0 {
                q(ka)
            } else if f == 1.0 {
                q(kb)
            } else {
                slerp(q(ka), q(kb), f)
            };
            let r = delta * q(&track.base);
            for (i, v) in [r.x, r.y, r.z, r.w].into_iter().enumerate() {
                out[i] = v.to_bits();
            }
        }
        6 | 1 => {
            for i in 0..n {
                let (x, y, base) = (ka[i] as i32, kb[i] as i32, track.base[i] as i32);
                let d = if f == 0.0 {
                    x
                } else if f == 1.0 {
                    y
                } else {
                    x.wrapping_add(((y.wrapping_sub(x)) as f32 * f + 0.5) as i32)
                };
                out[i] = base.wrapping_add(d) as u32;
            }
        }
        _ => {
            for i in 0..n {
                let d = if f == 0.0 {
                    fl(ka[i])
                } else if f == 1.0 {
                    fl(kb[i])
                } else {
                    (fl(kb[i]) - fl(ka[i])) * f + fl(ka[i])
                };
                out[i] = (d + fl(track.base[i])).to_bits();
            }
        }
    }
}

/// 0x519B00: the slerp of two quaternions, taking −b when the dot is
/// negative and a normalised lerp when it is above 0.999 (0x79B160). The
/// game's angle is a 16-bit fixed-point one (0x43C490, 2π / 65536 a step);
/// here it is a float.
fn slerp(a: glam::Quat, b: glam::Quat, f: f32) -> glam::Quat {
    let mut b = b;
    let mut d = a.dot(b);
    if d < 0.0 {
        b = -b;
        d = -d;
    }
    if d > 0.999 {
        return (a * (1.0 - f) + b * f).normalize();
    }
    let th = d.min(1.0).acos();
    let s = th.sin();
    a * (((1.0 - f) * th).sin() / s) + b * ((f * th).sin() / s)
}
