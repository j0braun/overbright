/// 0x5ECDC0 (**verified**, disassembled 2026-11-04): the running shot's
/// stage (0x59FA40 on car +0x40) when the gas of the car's input block
/// (car +0x30 → +0x20C) is at most 0.001 in size or the speed (car +0x430
/// × 2.237, 0x78426C) is under 5 mph (0x7A5668), else 0. Its non-zero
/// result is the purge (`FX_PURGE`, the `CARFX_NOS_BLOWOFF` smoke), its
/// zero with a shot running the nitrous proper (the flame, kind 10 at
/// 0x5F5F5A). So a shot held off the gas purges at any speed.
pub fn purging(stage: usize, speed: f32, gas: f32) -> bool {
    stage != 0 && (gas.abs() <= 0.001 || !(speed.abs() * 2.237 >= 5.0))
}

/// The ramp 0x459990 / 0x4599E0 (+0xCC: elapsed, duration, from, to; the
/// value at +0xE0).
#[derive(Clone, Copy, Debug, Default)]
pub struct Ramp {
    pub elapsed: f32,
    pub duration: f32,
    pub from: f32,
    pub to: f32,
    pub value: f32,
}

impl Ramp {
    /// 0x459990(from, to, ms): a duration of ms / 1000 (0.01 when not
    /// positive), starting at `from`.
    pub fn set(&mut self, from: f32, to: f32, ms: i32) {
        let d = ms as f32 * 0.001;
        *self = Ramp { elapsed: 0.0, duration: if d > 0.0 { d } else { 0.01 }, from, to, value: from };
    }

    /// Advance and lerp (the clone's tick: **inferred**, the ramp's
    /// stepping is not traced).
    pub fn step(&mut self, dt: f32) -> f32 {
        self.elapsed += dt;
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        self.value = self.from + (self.to - self.from) * t;
        self.value
    }
}

/// What the nitrous object asks for this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NosEvent {
    StartNitrous,
    StopNitrous,
    StartPurge,
    StopPurge,
}

/// `CARSFX_Nitrous`'s update (0x484F20, vtable 0x78AC58 +0x24).
#[derive(Clone, Debug, Default)]
pub struct Nitrous {
    /// +0xB8: 1 while the nitrous event plays.
    pub on: bool,
    /// +0xB4: the purge event (its +0xC set when released).
    pub purge: bool,
    /// +0xCC: the gain ramp.
    pub ramp: Ramp,
}

impl Nitrous {
    /// One frame with the running shot's stage (0x59FA40 on car +0x40) and
    /// the car's speed (m/s).
    pub fn update(&mut self, stage: usize, speed: f32, gas: f32) -> Vec<NosEvent> {
        let mut out = Vec::new();
        let purge = purging(stage, speed, gas);
        if !self.on {
            // State 0: a shot running and no purge starts FX_NITROUS
            // (0x472AF0) and ramps 0 → 1 over 400 ms.
            if stage != 0 && !purge {
                self.on = true;
                self.ramp.set(0.0, 1.0, 400);
                out.push(NosEvent::StartNitrous);
            }
        } else if stage == 0 || purge {
            // State 1: the shot gone or a purge releases it (0x45F2A0:
            // the event's +0x10 = 1) and ramps the current value → 0
            // over 1000 ms.
            self.on = false;
            self.ramp.set(self.ramp.value, 0.0, 1000);
            out.push(NosEvent::StopNitrous);
        }
        // The purge: started (0x45F2D0, FX_PURGE) while 0x5ECDC0 is
        // non-zero and none plays; released (0x45F3E0) once it is zero.
        if purge && !self.purge {
            self.purge = true;
            out.push(NosEvent::StartPurge);
        } else if !purge && self.purge {
            self.purge = false;
            out.push(NosEvent::StopPurge);
        }
        out
    }
}
