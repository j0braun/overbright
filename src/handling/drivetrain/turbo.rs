/// The turbo's boost (engine +0x98..+0xAC; 0x5A0F40, run by the engine's
/// tidy-up 0x5AA770 after the limiter test with the sub-step's dt
/// 0x7FC800). Only the HUD's turbo needle (0x5ECED0 → 0x4C6B80) reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Turbo {
    /// +0x98: the largest turbo curve point (physics +0x334, from 0;
    /// 0x457B00 at 0x5AA6D4). The turbo runs only above 0.
    pub max: f32,
    /// +0x9C: the boost (the needle).
    pub boost: f32,
    /// +0xA0: 0 off the gas, 1 spooling, 2 blowing off.
    pub(in crate::handling) state: u8,
    /// +0xA4: the boost when the blow-off began.
    pub(in crate::handling) blow_from: f32,
    /// +0xA8: the blow-off's time, s.
    pub(in crate::handling) blow_time: f32,
    /// +0xAC: the blow-off's last turning point (0 none yet).
    pub(in crate::handling) turn: f32,
}

impl Turbo {
    /// 0x5A0F40: `rpm` the engine's (+0x14 × 9.549304), `throttle` power ×
    /// pedal (0x5942F0, 0 at the limiter), `t` the rpm's place between idle
    /// and max (engine data +0 / +8).
    pub fn update(&mut self, rpm: f32, idle: f32, max: f32, throttle: f32, limiter: bool, dt: f32) {
        if self.max <= 0.0 || dt == 0.0 {
            return;
        }
        let full = self.max / 0.19;
        let vacuum = -(full * 0.25);
        let t = (rpm - idle) / (max - idle);
        match self.state {
            0 => {
                if throttle >= 0.3 {
                    self.state = 1;
                } else {
                    self.boost = (t + t).min(1.0) * vacuum;
                }
            }
            1 => {
                if throttle >= 0.3 {
                    self.boost = full.min(t * throttle.min(1.0) * 2.0 * dt + self.boost);
                } else if !limiter {
                    self.state = 2;
                    self.blow_time = dt;
                    self.blow_from = self.boost;
                }
            }
            2 => {
                let x = (self.blow_time + 1.1) * (self.blow_time + 1.1);
                let wave = crate::audio::transients::sin16(
                    ((x * x * 6.5 + std::f32::consts::FRAC_PI_2) * 10430.378) as i32 as u16,
                );
                // ClampF (0x401090): lo first.
                let v = (self.blow_from / full / self.blow_time * 0.015 * wave + vacuum)
                    .max(vacuum + vacuum)
                    .min(self.blow_from);
                let mut done = false;
                if self.turn == 0.0 {
                    self.turn = v;
                } else if (self.boost < v && v < self.turn) || (v < self.boost && self.turn < v) {
                    if (self.boost - self.turn).abs() >= 0.15 {
                        self.turn = self.boost;
                    } else {
                        done = true;
                    }
                }
                self.blow_time += dt;
                self.boost = v;
                if done || self.blow_time > 0.5 {
                    self.state = 0;
                    self.turn = 0.0;
                }
            }
            _ => {}
        }
    }
}
