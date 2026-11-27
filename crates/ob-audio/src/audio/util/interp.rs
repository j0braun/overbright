/// The interpolator 0x459990 / 0x46B100 ({elapsed, ms, from, to, shape,
/// value, done}): `set(from, to, ms, shape)`; each step adds dt (ms) and,
/// past the duration, holds `to` and sets done. Shapes: 0 linear, 1 hold,
/// 2 ease-in (t²), 3 ease-out (1 − (t − 1)²).
#[derive(Clone, Copy, Debug)]
pub struct Interp {
    pub elapsed: f32,
    pub duration: f32,
    pub from: f32,
    pub to: f32,
    pub shape: u8,
    pub value: f32,
    pub done: bool,
}

impl Interp {
    pub fn at(v: f32) -> Interp {
        Interp { elapsed: 0.0, duration: 0.0, from: v, to: v, shape: 0, value: v, done: true }
    }

    pub fn set(&mut self, from: f32, to: f32, ms: i32, shape: u8) {
        *self = Interp { elapsed: 0.0, duration: ms as f32, from, to, shape, value: from, done: false };
    }

    pub fn step(&mut self, dt_ms: f32) {
        if self.done {
            return;
        }
        self.elapsed += dt_ms;
        if self.elapsed > self.duration {
            self.value = self.to;
            self.done = true;
            return;
        }
        let u = self.elapsed / self.duration;
        self.value = match self.shape {
            1 => self.value,
            2 => (self.to - self.from) * u * u + self.from,
            3 => (self.to - self.from) * (1.0 - (u - 1.0) * (u - 1.0)) + self.from,
            _ => u * (self.to - self.from) + self.from,
        };
    }
}

/// The 4-tap moving average 0x579F30(4): push (0x576980) or fill
/// (0x576A20, every tap), the mean of the taps filled so far (as the
/// 3-tap [`RunningMean3`] of the same class).
#[derive(Clone, Copy, Debug, Default)]
pub struct Mean4 {
    taps: [f32; 4],
    at: usize,
    count: usize,
}

impl Mean4 {
    pub fn push(&mut self, v: f32) {
        self.taps[self.at] = v;
        self.at = (self.at + 1) % 4;
        self.count = (self.count + 1).min(4);
    }
    pub fn fill(&mut self, v: f32) {
        self.taps = [v; 4];
        self.count = 4;
    }
    pub fn mean(&self) -> f32 {
        if self.count == 0 { 0.0 } else { self.taps.iter().sum::<f32>() / self.count as f32 }
    }
}
