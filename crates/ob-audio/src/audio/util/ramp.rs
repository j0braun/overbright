use crate::audio::carfx::fade;

/// The curve 0x468160(0, p, 0): the fade table 0x7EF7F8 at p >> 6, with
/// the next entry blended by `((p & 0x1E) << 9) | 0x3FF` / 32768 (as the
/// code does: only bits 1–4 of p) unless the entry is 0.
pub fn curve0(p: i32) -> i32 {
    let i = (p >> 6).clamp(0, 511) as usize;
    let v = fade(i);
    if v == 0 {
        return 0;
    }
    v + (((fade(i + 1) - v) * (((p & 0x1E) << 9) | 0x3FF)) >> 15)
}

/// 0x468160(2, p, 0): curve0 squared, >> 15.
pub fn curve2(p: i32) -> i32 {
    let c = curve0(p);
    (c * c) >> 15
}

/// The interpolator 0x459990 / 0x46B100 ({elapsed, duration, from, to,
/// shape, value, done}, durations in seconds: 0x459990 takes ms × 0.001
/// and uses 0.01 s for a duration ≤ 0). Each step adds dt; past the
/// duration (strictly) it holds `to` and sets done. Shapes (jump table
/// 0x46B21C): 1 hold, 2 t², 3 1 − (t − 1)², 6 the curve
/// curve2(0x7FFF − clamp(trunc(t · 32767), 0, 0x7FFF)) / 32767, else
/// linear.
#[derive(Clone, Copy, Debug)]
pub struct Ramp {
    pub elapsed: f32,
    pub duration: f32,
    pub from: f32,
    pub to: f32,
    pub shape: u8,
    pub value: f32,
    pub done: bool,
}

impl Default for Ramp {
    fn default() -> Ramp {
        Ramp::raw(0.0, 0.0, 0.0, 0)
    }
}

impl Ramp {
    /// A ramp written field by field (seconds as given).
    pub fn raw(from: f32, to: f32, duration: f32, shape: u8) -> Ramp {
        Ramp { elapsed: 0.0, duration, from, to, shape, value: from, done: false }
    }

    /// 0x459990(from, to, ms, shape).
    pub fn set(from: f32, to: f32, ms: i32, shape: u8) -> Ramp {
        let d = ms as f32 * 0.001;
        Ramp::raw(from, to, if d <= 0.0 { 0.01 } else { d }, shape)
    }

    /// 0x46B100(dt), dt in seconds.
    pub fn step(&mut self, dt: f32) {
        if self.done {
            return;
        }
        self.elapsed += dt;
        let e = self.elapsed;
        if e > self.duration {
            self.value = self.to;
            self.done = true;
            return;
        }
        let u = e / self.duration;
        self.value = match self.shape {
            1 => self.value,
            2 => (self.to - self.from) * u * u + self.from,
            3 => {
                let w = (1.0 / self.duration) * e - 1.0;
                (self.to - self.from) * (1.0 - w * w) + self.from
            }
            6 => {
                let k = ((u * 32767.0) as i32).clamp(0, 0x7FFF);
                curve2(0x7FFF - k) as f32 * (1.0 / 32767.0) * (self.to - self.from) + self.from
            }
            _ => u * (self.to - self.from) + self.from,
        };
    }

    /// Step toward a moving target (0x47C1A0 / 0x47BF20 / 0x47AAA0): `to`
    /// is replaced first, and once done the value is the target.
    pub fn track(&mut self, target: f32, dt: f32) {
        self.to = target;
        self.step(dt);
        if self.done {
            self.value = target;
        }
    }
}
