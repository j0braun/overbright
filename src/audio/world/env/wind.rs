use super::{EnvInput, EnvSfx, ramp};
use crate::audio::carfx::{Sink, open_bank, run_bank};

const WIND: &str = "SOUND/IG_GLOBAL/WIND_00_MB.abk";

impl EnvSfx {
    pub(super) fn wind_update(&mut self, sink: &mut dyn Sink, i: &EnvInput, dt: f32) {
        open_bank(&mut self.wind_bank, &mut self.sounds, &mut self.gains, sink, WIND);
        let v = i.speed;
        let v = if v < 2.0 {
            2.0
        } else if v > 40.0 {
            40.0 - 0.05
        } else {
            v
        };
        let t = v / 40.0;
        let p6 = (1040.0 * t) as i32;
        let level = (32767.0 * t) as i32;
        let mph = i.speed * 2.237;
        let weather = ramp(1.0, 0.25, 0.0, 70.0, mph);
        if let Some(b) = self.wind_bank.as_mut() {
            let ev = &mut b.events;
            if self.wind.is_none_or(|id| !ev.alive(id)) {
                self.wind = ev.module("FX_WIND").and_then(|m| ev.play(m, &[0, 0, 0x1000, 0, 0, 0, 0]));
            }
            if self.weather.is_none_or(|id| !ev.alive(id)) {
                self.weather = ev.module("FX_WIND_Weather").and_then(|m| ev.play(m, &[0, 0, 0, 0]));
            }
            if let Some(id) = self.wind {
                let vol = |k: usize| ((0x7FFF * ((i.mix.level(7, k) * level) >> 15)) >> 15).clamp(0, 0x7FFF);
                ev.set(
                    id,
                    &[
                        0,
                        vol(0),
                        0x1000,
                        i.mix.az(7, 0).clamp(0, 0xFFFF),
                        vol(1),
                        i.mix.az(7, 1).clamp(0, 0xFFFF),
                        p6.clamp(0, 0x410),
                    ],
                );
            }
            if let Some(id) = self.weather {
                ev.set(id, &[((i.mix.level(10, 0) as f32 * weather) as i32).clamp(0, 0x7FFF), 0, 4000, 0]);
            }
        }
        run_bank(&mut self.wind_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}
