use super::tables::ShiftRec;
use crate::audio::util::ramp::Ramp;

/// What the transients read of the sound side this update: SFXCTL_Physics
/// (+0x70 the context rpm, +0x84 the slewed throttle, +0x8C / +0x90 the
/// input % now and last update, +0xC8 / +0xCC the gear now and last) and
/// the Engine's rpm / throttle before its own update (slots 0x34 / 0x38).
#[derive(Clone, Copy, Debug, Default)]
pub struct Physics {
    pub rpm: f32,
    pub throttle: f32,
    pub input: f32,
    pub prev_input: f32,
    pub gear: i32,
    pub prev_gear: i32,
    /// ctx +0x4C: the input above 30 %.
    pub on: bool,
    /// Car speed, mph (car +0x430 × 2.237).
    pub mph: f32,
    /// The drivetrain's up-shift rpm for the current gear (0x5ECD40 →
    /// 0x5947C0: drivetrain +0x84 + 4 · gear; a real rpm compared with
    /// the context's 1000..10000 rpm, as the game does).
    pub up_rpm: f32,
    /// Race type 8 (0x45CD30 skips the launch there; `g_RaceType`).
    pub race_type_8: bool,
    /// Physics +0xD8: scripted input suppresses the native launch trigger.
    pub scripted: bool,
}

/// `SFXCTL_Shifting` (vtable 0x789F28): the triggers (0x46EC70 up,
/// 0x46EE60 down), the state machine 0x482D50 and its ramps.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shifting {
    pub clock: f32,
    /// +0x70 the clunk (CARSFX_Shift plays it), +0x71 an up-shift started,
    /// +0x72 the clutch engaged (state 1 → 2); all cleared each update.
    pub play: bool,
    pub swtn_start: bool,
    pub swtn_engage: bool,
    /// +0x78 the state, +0x7C the state entered this update (else 0).
    pub state: i32,
    pub changed: i32,
    /// +0x80 / +0x84 the volume wobble's amplitude and period (ms), +0x88 /
    /// +0x8C the throttle's (never set), +0x90 / +0x94 the rpm's.
    pub wob: [i32; 6],
    /// +0x98 the clunk armed, +0x9C up, +0xA0 when, +0xA8 the rpm at the
    /// shift.
    pub(super) armed: bool,
    pub up: bool,
    pub(super) at: f32,
    pub shift_rpm: f32,
    /// +0xAC the throttle ramp (value +0xC0), +0xC8 the rpm ramp (value
    /// +0xDC, done +0xE0), +0xE4 the volume bump (value +0xF8), +0x104 /
    /// +0x120 the rpm / volume wobble decays, +0x13C the wobble on, +0x140
    /// a second rpm ramp (value +0x154, Engine's +0xB8 filter).
    pub thr: Ramp,
    pub rpm: Ramp,
    pub bump: Ramp,
    pub(super) wob_rpm: Ramp,
    pub(super) wob_vol: Ramp,
    pub(super) wob_on: bool,
    pub rpm2: Ramp,
    /// +0x64: the context's transmission level (+0xA4).
    pub level: i32,
}

impl Shifting {
    /// 0x45C430: the shift ends.
    pub(super) fn end(&mut self) {
        self.wob_on = false;
        self.wob = [0; 6];
        self.state = 0;
        self.changed = 0;
    }

    /// 0x46EC70 / 0x46EE60 share the start: the wobble cleared, the
    /// Engine's rpm put back to the last update's (+0x128 ↔ +0x12C), the
    /// throttle ramped from the Physics throttle to 0.
    pub(super) fn begin(&mut self, p: &Physics, eng: &mut (f32, f32), state: i32, thr_s: f32) {
        self.state = state;
        self.changed = state;
        self.shift_rpm = eng.1;
        *eng = (eng.1, eng.0);
        self.thr = Ramp::raw(p.throttle, 0.0, thr_s, 0);
        self.bump = Ramp::raw(0.0, 0.0, 0.001, 0);
    }

    /// 0x483130: one update.
    pub fn update(&mut self, p: &Physics, rec: &ShiftRec, eng: &mut (f32, f32), eng_throttle: f32, dt: f32) {
        self.clock += dt;
        self.play = false;
        self.swtn_start = false;
        self.swtn_engage = false;
        if p.gear > p.prev_gear {
            // 0x46EC70.
            self.wob_on = false;
            self.wob = [0; 6];
            self.state = 0;
            self.changed = 0;
            if p.rpm >= 3000.0 {
                self.begin(p, eng, 1, 0.1);
                let a = self.shift_rpm;
                self.rpm = Ramp::set(a, a - rec.up_drop as f32, rec.up_drop_ms, 0);
                self.rpm2 = Ramp::set(a, a - 1200.0, rec.up_add_ms + rec.up_drop_ms, 6);
                self.armed = true;
                self.up = true;
                self.swtn_start = true;
                self.at = rec.up_delay + self.clock;
            }
        } else if p.gear < p.prev_gear && p.gear != 1 {
            // 0x46EE60 (not into neutral).
            self.wob_on = false;
            self.wob = [0; 6];
            self.begin(p, eng, 4, 0.05);
            let a = self.shift_rpm;
            let to = (a - rec.down_drop as f32).clamp(1000.0, 10000.0);
            self.rpm = Ramp::set(a, to, rec.down_drop_ms, 0);
            self.up = false;
            self.armed = true;
            self.at = rec.down_delay + self.clock;
        }
        self.machine(p, rec, eng, eng_throttle, dt);
    }

    /// 0x46F080: the wobble decays start (gears below 5; 0.85 in gear 3,
    /// 0.7 in gear 4 on the rpm wobble).
    pub(super) fn wobble_start(&mut self, p: &Physics, rec: &ShiftRec) {
        if p.gear >= 5 {
            return;
        }
        let k = match p.gear {
            3 => 0.85,
            4 => 0.7,
            _ => 1.0,
        };
        self.wob_on = true;
        self.wob_rpm = Ramp::set(k * rec.wob_rpm as f32, 0.0, rec.wob_rpm_ms, 0);
        self.wob_vol = Ramp::set(rec.wob_vol as f32, 0.0, rec.wob_vol_ms, 0);
    }

    /// 0x46F000: the decays stepped; the rpm one ending ends the shift.
    pub(super) fn wobble_step(&mut self, rec: &ShiftRec, dt: f32) {
        if !self.wob_on {
            return;
        }
        self.wob_rpm.step(dt);
        self.wob_vol.step(dt);
        if self.wob_rpm.done {
            self.end();
            return;
        }
        self.wob[4] = self.wob_rpm.value as i32;
        self.wob[5] = rec.wob_rpm_period;
        self.wob[0] = self.wob_vol.value as i32;
        self.wob[1] = rec.wob_vol_period;
    }

    /// 0x482D50.
    pub(super) fn machine(&mut self, p: &Physics, rec: &ShiftRec, eng: &mut (f32, f32), eng_throttle: f32, dt: f32) {
        if self.clock > self.at && self.armed {
            self.play = true;
            self.armed = false;
        }
        self.changed = 0;
        if self.state == 0 {
            return;
        }
        // 0x47BF20: the second rpm ramp, then the rpm ramp (tracking the
        // context rpm in states 2, 3, 7).
        if !self.rpm2.done {
            self.rpm2.track(p.rpm, dt);
        }
        match self.state {
            2 | 3 | 7 => self.rpm.track(p.rpm, dt),
            _ => self.rpm.step(dt),
        }
        // 0x47BEA0: the throttle ramp likewise with the Physics throttle.
        match self.state {
            2 | 3 | 7 => self.thr.track(p.throttle, dt),
            _ => self.thr.step(dt),
        }
        self.bump.step(dt);
        self.wobble_step(rec, dt);
        let done = self.rpm.done;
        match self.state {
            1 if done => {
                self.state = 2;
                self.changed = 2;
                let from = (rec.up_add as f32 + p.rpm).clamp(1000.0, 10000.0);
                self.rpm = Ramp::set(from, p.rpm, rec.up_add_ms, 0);
                // Tables 0x7885A0 (1.0 × 4) and 0x7885B0 (0 ms × 4) by
                // clamp(gear − 2, 0, 3).
                self.thr = Ramp::set(p.throttle * 1.0, p.throttle, 0, 0);
                self.bump = Ramp::set(rec.bump, 0.0, rec.bump_ms, 0);
                self.wobble_start(p, rec);
                self.wobble_step(rec, dt);
                self.swtn_engage = true;
            }
            2 if done => {
                self.state = 3;
                self.changed = 3;
            }
            3 => {
                if !(self.wob_rpm.done && self.wob_vol.done) && self.wob_on {
                    return;
                }
                self.end();
            }
            4 if done => {
                self.state = 5;
                self.changed = 5;
                let mut ms = rec.down_blip_ms;
                if p.gear < 3 {
                    ms = (ms as f32 * 0.7) as i32;
                }
                let to = (p.rpm + rec.down_add as f32).clamp(1000.0, 10000.0);
                self.rpm = Ramp::set(eng.0, to, ms, 6);
                // 0x7ED29C = 100.0, 0x7ED2A0 = 215 ms.
                self.thr = Ramp::set(eng_throttle, 100.0, 215, 0);
            }
            5 if done => {
                self.state = 6;
                self.changed = 6;
                let mut ms = rec.down_fall_ms;
                if p.gear < 3 && self.level <= 1 {
                    ms = (ms as f32 * 0.7) as i32;
                }
                self.rpm = Ramp::set(eng.0, eng.0 - rec.down_fall as f32, ms, 0);
                self.thr = Ramp::set(100.0, 0.0, 215, 0);
            }
            6 if done => {
                self.state = 7;
                self.changed = 7;
                let ms = (((eng.0 - p.rpm) * rec.settle).abs().clamp(0.0, 800.0)) as i32;
                self.rpm = Ramp::set(eng.0, p.rpm, ms, 6);
                self.thr = Ramp::set(0.0, p.throttle, 60, 0);
            }
            7 if done => self.end(),
            _ => {}
        }
    }

    /// CARSFX_Shift's variant: 1 while the up-shift states 1–3 run
    /// (0x45E730 reads +0x78).
    pub fn upshifting(&self) -> bool {
        (1..=3).contains(&self.state)
    }
}
