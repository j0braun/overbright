use super::nitrous::Ramp;
use super::tables::{TURBO_BANKS, TURBO_DECAY, TURBO_FULL, fade};

/// The running mean of the last three values (0x579F30 init with 3,
/// 0x576980 push: the window fills up first, so the mean is over the
/// values pushed so far, at most 3).
#[derive(Clone, Debug, Default)]
pub struct RunningMean3 {
    values: [f32; 3],
    count: u8,
    next: u8,
    sum: f32,
}

impl RunningMean3 {
    pub fn push(&mut self, v: f32) -> f32 {
        if self.count < 3 {
            self.count += 1;
        }
        self.sum += v - self.values[self.next as usize];
        self.values[self.next as usize] = v;
        self.next = (self.next + 1) % 3;
        self.sum / self.count as f32
    }

    /// 0x576A20: every tap set to `v`.
    pub fn fill(&mut self, v: f32) -> f32 {
        self.values = [v; 3];
        self.sum = 3.0 * v;
        self.count = 3;
        v
    }
}

/// What the turbo object asks for this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TurboEvent {
    /// The blow-off (0x45EE70): FX_TURBO_01 with this variant (1, or 2–3
    /// at more than 3/4 spool) at this volume (+0xE8).
    BlowOff { variant: usize, volume: i32 },
}

/// `CARSFX_Turbo` (ctor 0x472480, vtable 0x78AC10): update 0x47CAD0 with
/// the spool 0x472770 and the blow-off step 0x4726E0.
#[derive(Clone, Debug, Default)]
pub struct Turbo {
    /// +0x180: 0 idle, 1 on boost, 2 blowing off.
    pub state: u8,
    /// +0x184: the spool rate (boost value × 0.01 above 20, else 0).
    pub rate: f32,
    /// +0xF4: the spool, 0..40; +0xF8 its fraction.
    pub spool: f32,
    pub fraction: f32,
    /// +0xE4: the whine's volume (0..0x7FFF before the mix level).
    pub whine: i32,
    /// +0x100: the full-spool fade running; +0x104 its envelope (+0x178
    /// the value: a fade-table index).
    fading: bool,
    fade_time: f32,
    /// +0xC0 the blow-off playing; +0xE8 its volume; +0xC4 its ramp
    /// (value +0xD8).
    pub blowing: bool,
    pub blow_volume: i32,
    pub blow_ramp: Ramp,
}

impl Turbo {
    /// One sound tick. `boost` is SFXCTL_Engine's slot 0x38 value (+0x130:
    /// the 3-frame running mean of the sound state's +0x44 plus the shift
    /// offset +0x148, 0 outside shifts), `throttle` the sound state's +0x54,
    /// `rpm` its +0x48, `record` the turbo record, `shifting` the shift
    /// controller's state +0x78 ≠ 0, `blow_done` whether the blow-off event
    /// has ended (0x45EFF0's event-state test), `dt` the tick in seconds.
    pub fn update(
        &mut self,
        boost: f32,
        throttle: f32,
        rpm: f32,
        record: usize,
        shifting: bool,
        blow_done: bool,
        dt: f32,
    ) -> Option<TurboEvent> {
        let (_, whine_vol, blow1, blow23) = TURBO_BANKS[record.min(14)];
        // 0x47CAD0: the rate.
        self.rate = if boost <= 20.0 { 0.0 } else { boost * 0.01 };
        // 0x4726E0: while a blow-off plays, throttle back above 20 with the
        // ramp full and no shift fades it out over 150 ms; once done it is
        // released (0x45F050).
        if self.blowing {
            self.blow_ramp.step(dt);
            if throttle > 20.0 && self.blow_ramp.value == 1.0 && !shifting {
                self.blow_ramp.set(1.0, 0.0, 150);
            }
            if blow_done || self.blow_ramp.value == 0.0 {
                self.blowing = false;
            }
        }
        self.spool_tick(rpm, whine_vol, dt, dt > 0.0);
        // The state machine (0x47CAD0).
        match self.state {
            0 if boost > 20.0 => self.state = 1,
            1 if boost < 20.0 => {
                self.state = 2;
                let ev = self.blow_off(blow1, blow23);
                // The spool drops to 0 at once (+0xF4 := 0) and is
                // re-evaluated (0x472770 again).
                self.spool = 0.0;
                self.spool_tick(rpm, whine_vol, 0.0, dt > 0.0);
                return ev;
            }
            2 if !self.blowing => self.state = 0,
            _ => {}
        }
        None
    }

    /// 0x45EE70: variant 1, or 2 + rand(2) above 3/4 spool; volume =
    /// trunc(record volume × fraction); the ramp set to 1.0. Only when no
    /// blow-off plays.
    fn blow_off(&mut self, blow1: i32, blow23: i32) -> Option<TurboEvent> {
        if self.blowing {
            return None;
        }
        let variant = if self.fraction > 0.75 { 2 + crate::audio::collision::rand(2) as usize } else { 1 };
        let vol = if variant == 1 { blow1 } else { blow23 };
        self.blow_volume = (vol as f32 * self.fraction) as i32;
        self.blowing = true;
        self.blow_ramp.set(1.0, 1.0, 1);
        self.blow_ramp.value = 1.0;
        Some(TurboEvent::BlowOff { variant, volume: self.blow_volume })
    }

    /// 0x472770 (with the spool event started): spool up by the rate above
    /// 0.01, else down by 0.5; clamp to 0..40 and to (0.4 + 0.6 ·
    /// clamp((trunc(rpm) − 1000) · 0.01, 0, 1)) · 40; at that cap the
    /// whine holds for 1 s and then fades over 2 s to fade-table index
    /// 307.2 (0x4598F0 segments (0, 0, 1000 ms) and (0, 0.6 · 512,
    /// 2000 ms), linear); the volume is trunc(fraction · 32767) × record
    /// +4 >> 15, times fade[index] >> 15 while fading.
    fn spool_tick(&mut self, rpm: f32, whine_vol: i32, dt: f32, step: bool) {
        // One step per call, whatever the dt (0x472770 does not read it;
        // nothing while the audio is paused, [0x82B884]+0x13): once per
        // audio update, and again at a blow-off's start (0x47CBAF).
        if step {
            self.spool += if self.rate > 0.01 { self.rate } else { -TURBO_DECAY };
        }
        self.spool = self.spool.clamp(0.0, TURBO_FULL);
        let x = (((rpm as i32) - 1000) as f32 * 0.01).clamp(0.0, 1.0);
        let k = x * 0.6 + 0.4;
        let cap = k * TURBO_FULL;
        self.spool = self.spool.clamp(0.0, cap);
        if self.spool == cap && !self.fading {
            self.fading = true;
            self.fade_time = 0.0;
        } else if self.spool != TURBO_FULL && self.fading {
            self.fading = false;
        }
        self.fraction = self.spool / TURBO_FULL;
        self.whine = ((self.fraction * 32767.0) as i32 * whine_vol) >> 15;
        if self.fading {
            self.fade_time += dt;
            let t = self.fade_time;
            let index = if t <= 1.0 { 0.0 } else { (0.6 * 512.0) * ((t - 1.0) / 2.0).min(1.0) };
            self.whine = (fade(index as usize) * self.whine) >> 15;
        }
    }

    /// The blow-off's volume this tick (0x45ECE0): ((+0xE8 × level) >> 15)
    /// × the ramp, clamped 0..0x7FFF.
    pub fn blow_level(&self, level: i32) -> i32 {
        ((((self.blow_volume * level) >> 15) as f32 * self.blow_ramp.value) as i32).clamp(0, 0x7FFF)
    }

    /// The whine's volume (0x45ECE0): (+0xE4 × level) >> 15, clamped.
    pub fn whine_level(&self, level: i32) -> i32 {
        ((self.whine * level) >> 15).clamp(0, 0x7FFF)
    }
}
