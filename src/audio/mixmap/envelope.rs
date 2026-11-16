use super::*;

/// 0x468580 for one variant with the position object's fields (f0 the
/// distance to the player's car, f1 to the camera, f2 / f3 the azimuths
/// from them).
pub(super) fn threed_step(st: &mut TdState, v: &Variant, f: [i32; 4]) {
    let d = match (v[0] >> 12) & 0xF {
        0 => f[1] as f64,
        1 => f[0] as f64,
        _ => -1.0,
    };
    match (v[0] >> 8) & 0xF {
        0 => st.az = f[3],
        1 => st.az = f[2],
        _ => {}
    }
    let q = (st.az >> 14) & 3;
    let ang = st.az - q * 0x4000;
    let (a, b, ta) = match q {
        0 => (v[2], v[3], v[1] >> 28),
        1 => (v[3], v[4], v[1] >> 16),
        2 => (v[4], v[5], v[1] >> 24),
        _ => (v[5], v[2], v[1] >> 20),
    };
    let ta = ta & 0xF;
    let (lo1, hi1) = ((a & 0x7FFF) as f64, ((a >> 16) & 0x7FFF) as f64);
    let (lo2, hi2) = ((b & 0x7FFF) as f64, ((b >> 16) & 0x7FFF) as f64);
    if d > hi1 && d > hi2 {
        st.mb = -10000;
        return;
    }
    let d1 = d.max(lo1).min(hi1);
    let d2 = (d as f32 as f64).max(lo2).min(hi2);
    let ix = ((d1 - lo1) / (hi1 - lo1) * 32767.0) as i32;
    let iy = (((d2 - lo2) / (hi2 - lo2)) as f32 as f64 * 32767.0) as i32;
    // 0x468380: both directions through the first direction's curve type
    // (as coded), crossfaded by the angle within the quadrant.
    let ca = curve(ta, ix, false);
    let cb = if ang != 0 { curve(ta, iy, false) } else { 0x7FFF };
    let wgt = 2 * ang;
    let lin = (((0x7FFF - wgt) * ca) >> 15) + ((wgt * cb) >> 15);
    st.mb = lin_to_db(lin);
}

/// The time word of an event control: (ms, curve).
pub(super) fn time(w: u32) -> (f64, u32) {
    ((w & 0xFFF) as f32 as f64 * 16.666_67_f32 as f64, (w >> 12) & 0xF)
}

/// The envelope level for an attack (x rising) or release (x falling)
/// point: V ≥ 0 → 0x7FFF − curve, else the curve (0x468960 / 0x468A70 /
/// 0x468BD0).
pub(super) fn env_of(v: i32, t: u32, x: i32) -> i32 {
    let c = curve(t, x, false);
    if v >= 0 { 0x7FFF - c } else { c }
}

/// 0x468D40 for one event control: the timer runs while triggered (or
/// until its envelope ends); modes 0 attack / release, 1 attack / hold /
/// release, 2 attack / sustain while the trigger holds / release; the
/// output is V (millibels) scaled by 1 − envelope / 32768 and by the
/// scale input / 32767.
pub(super) fn evt_step(st: &mut EvtState, w: &[u32; 7], trig: i32, scale: i32, dt: f32) {
    if st.timer == 0.0 && trig == 0 {
        st.rel = 0.0;
        st.out = 0;
        st.latch = false;
        return;
    }
    let v = {
        let x = w[1] & 0xFFFF;
        if x > 10000 { (x | 0xFFFF_0000) as i32 } else { x as i32 }
    };
    st.timer = (dt as f64 * 1000.0 + st.timer as f64) as f32;
    let tm = st.timer as f64;
    let (a, ta) = time(w[4]);
    let (h, _) = time(w[5]);
    let (r, tr) = time(w[6]);
    match (w[0] >> 24) & 0xF {
        0 | 1 => {
            let hold = if (w[0] >> 24) & 0xF == 1 { h } else { 0.0 };
            if a > tm {
                st.env = env_of(v, ta, (tm * 32767.0 / a) as i32);
            } else if (w[0] >> 24) & 0xF == 1 && tm - a <= hold {
                st.env = 0;
            } else {
                let t2 = tm - (a + hold);
                if t2 >= r {
                    st.timer = 0.0;
                    st.env = 0x7FFF;
                } else {
                    let x = ((32767.0 - t2 * 32767.0 / r) as i32).clamp(0, 0x7FFF);
                    st.env = env_of(v, tr, x);
                }
            }
        }
        2 => {
            if a > tm {
                if trig == 0 {
                    st.timer = a as f32;
                    st.env = 0;
                    st.latch = true;
                    st.held = st.out;
                    return finish(st, v, trig, scale);
                }
                st.env = env_of(v, ta, (tm * 32767.0 / a) as i32);
            } else if trig == 0 {
                if st.rel == 0.0 {
                    st.rel = st.timer;
                }
                let t2 = tm - st.rel as f64;
                if t2 > r {
                    st.latch = false;
                    st.held = 0;
                    st.timer = 0.0;
                    st.rel = 0.0;
                    st.env = 0x7FFF;
                } else {
                    let x = (32767.0 - t2 * 32767.0 / r) as i32;
                    st.env = env_of(v, tr, x);
                }
            } else if trig == 1 && st.rel != 0.0 {
                st.timer = 0.0;
                st.rel = 0.0;
                st.latch = true;
                st.held = st.out;
                st.env = 0x7FFF;
            } else {
                st.timer = a as f32;
                st.latch = false;
                st.held = 0;
                st.rel = 0.0;
                st.env = 0;
            }
        }
        _ => {
            st.out = 0;
        }
    }
    finish(st, v, trig, scale);
}

pub(super) fn finish(st: &mut EvtState, v: i32, trig: i32, scale: i32) {
    let f = (32767 - st.env) as f64 * (1.0 / 32768.0);
    let out = if !st.latch {
        (f * v as f64) as i32
    } else if trig == 0 {
        (f * st.held as f64) as i32
    } else {
        (f * (v - st.held) as f64) as i32 + st.held
    };
    st.out = (scale as f64 * (1.0 / 32767.0) * out as f64) as i32;
}
