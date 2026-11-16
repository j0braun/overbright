/// 0x7F3760: the doppler depth per user.
pub const DOPPLER_DEPTH: [f32; 4] = [0.35, 0.33, 0.6, 0.3];

/// A position object's doppler state (+0xA0, +0xA4, the mean +0xA8).
#[derive(Clone, Debug)]
pub struct Doppler {
    last: f32,
    taps: Vec<f32>,
    next: usize,
    pub mean: f32,
}

impl Default for Doppler {
    fn default() -> Self {
        Doppler { last: -1.0, taps: Vec::new(), next: 0, mean: 1.0 }
    }
}

impl Doppler {
    /// 0x45E1F0 with this frame's integer distance and dt.
    pub fn update(&mut self, distance: i32, dt: f32) {
        let d = distance as f32;
        let r = if d < 0.0 || self.last < 0.0 || dt <= 0.0 { 1.0 } else { 343.0 / ((d - self.last) / dt + 343.0) };
        if self.taps.len() < 4 {
            self.taps.push(r);
        } else {
            self.taps[self.next] = r;
        }
        self.next = (self.next + 1) % 4;
        self.mean = self.taps.iter().sum::<f32>() / self.taps.len() as f32;
        self.last = d;
    }

    /// 0x45E290(k).
    pub fn ratio(&self, k: usize) -> f32 {
        let v = (self.mean - 1.0) * DOPPLER_DEPTH[k & 3] + 1.0;
        if v <= 0.8 {
            0.8
        } else if v > 1.2 {
            1.2
        } else {
            v
        }
    }
}
