use super::super::*;

impl Instance {
    /// Node 28 (0x720E48): an oscillator, period +8 ms, amplitude +0xC,
    /// phase f32 +4, shape byte +0 (0 sine, 1 square, 2 saw, else
    /// triangle).
    pub(super) fn lfo(&mut self, b: i32) -> i32 {
        let period = self.rd(b + 8);
        if period <= 0 {
            return 0;
        }
        let inc = TICK_MS as f64 / period as f64;
        let amp = self.rd(b + 0xC) as f64;
        let mut ph = self.rf(b + 4) as f64;
        while ph >= 1.0 {
            ph -= 1.0;
        }
        self.wf(b + 4, ph as f32);
        let v = match self.rb(b) {
            0 => sine(fist((ph * 1024.0) as f32 as f64)) as f64 * amp * (1.0 / 65536.0),
            1 => {
                if ph < 0.5 {
                    0.0
                } else {
                    amp
                }
            }
            2 => amp * ph,
            _ => {
                if ph > 0.5 {
                    (1.0 - ph) * amp * 2.0
                } else {
                    ph * amp * 2.0
                }
            }
        };
        self.wf(b + 4, (inc + ph) as f32);
        fist(v as f32 as f64)
    }
}
