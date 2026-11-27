/// The world's slow motion (+0x108 rate, +0x10C seconds left).
#[derive(Clone, Copy, Debug)]
pub struct SlowMotion {
    pub rate: f32,
    pub remaining: f32,
}

impl Default for SlowMotion {
    fn default() -> Self {
        SlowMotion { rate: 1.0, remaining: 0.0 }
    }
}

impl SlowMotion {
    /// 0x5EA290 (the callback is always 0x6022E0, an event's end).
    pub fn start(&mut self, rate: f32, seconds: f32) {
        self.rate = rate;
        self.remaining = seconds;
    }

    /// 0x5EA370 for a real frame time: the world's step, and whether the
    /// slow motion ran out this frame (its callback is due).
    pub fn advance(&mut self, dt: f32) -> (f32, bool) {
        let mut expired = false;
        if self.remaining > 0.0 {
            self.remaining -= dt;
            if self.remaining <= 0.0 {
                self.remaining = 0.0;
                self.rate = 1.0;
                expired = true;
            }
        }
        (dt * self.rate, expired)
    }
}

/// (rate, seconds) of each kind's slow motion (0x602524..0x6025D0).
pub fn slow_motion(kind: u8) -> Option<(f32, f32)> {
    match kind {
        3 => Some((f32::from_bits(0x3EAA_7EFA), 8.0)),
        4 => Some((f32::from_bits(0x3EAA_7EFA), 2.0)),
        5 => Some((2.0, 4.0)),
        6 | 7 => Some((0.4, 8.0)),
        _ => None,
    }
}
