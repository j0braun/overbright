use super::shifting::Physics;
use crate::audio::util::ramp::Ramp;

/// `SFXCTL_AccelTrans` (vtable 0x789FB8): the launch (0x45CC30) and tip-in
/// (0x45CB70) transients; +0xC8 the state (1 tip-in, 2 launch, 3 launch
/// settling, 4 lifted during one).
#[derive(Clone, Copy, Debug, Default)]
pub struct AccelTrans {
    pub clock: f32,
    /// +0xD0 / +0xD1 on throttle now / last; +0xD8 a lift at high rpm.
    pub(super) on: bool,
    pub(super) last: bool,
    pub lift: bool,
    pub state: i32,
    /// +0xCC when the last transient started.
    pub(super) started: f32,
    /// +0x74 the rpm ramp (value +0x88), +0x90 a second ramp (value
    /// +0xA4; no reader found in the sound code), +0xAC the throttle
    /// (value +0xC0).
    pub rpm: Ramp,
    pub aux: Ramp,
    pub thr: Ramp,
}

impl AccelTrans {
    /// 0x4831A0. `shifting` is the Shifting state, `racer` whether the
    /// car's owner kind is 2 (an AI racer: 0x470030 lets those tip in in
    /// any gear), `eng_rpm` the Engine's rpm (slot 0x34) and `rec` the
    /// launch record.
    pub fn update(
        &mut self,
        p: &Physics,
        shifting: i32,
        racer: bool,
        eng_rpm: f32,
        rec: (f32, i32, i32, i32, i32),
        dt: f32,
    ) {
        self.clock += dt;
        self.last = self.on;
        self.lift = false;
        self.on = p.on;
        let jump = p.input - p.prev_input >= 30.0;
        if self.on {
            if !self.last {
                // 0x45CD30: a launch from standing in first.
                let launch = !p.scripted
                    && p.mph <= 15.0
                    && jump
                    && self.state == 0
                    && shifting == 0
                    && p.gear == 2
                    && !p.race_type_8
                    && p.rpm <= 1500.0;
                // 0x470030: a tip-in in gear.
                let tip = jump
                    && self.state == 0
                    && shifting == 0
                    && p.rpm <= p.up_rpm - 250.0
                    && p.rpm >= 3000.0
                    && self.clock - self.started >= 2.0
                    && (racer || p.gear > 2);
                if launch {
                    self.started = self.clock;
                    self.state = 2;
                    self.rpm = Ramp::set(p.rpm, p.rpm + rec.1 as f32, rec.2, 6);
                    self.aux = Ramp::set(0.0, rec.0, rec.2, 0);
                    self.thr = Ramp::raw(p.throttle, p.throttle, 0.01, 0);
                } else if tip {
                    self.rpm = Ramp::raw(p.rpm + 1000.0, p.rpm, 0.5, 6);
                    self.state = 1;
                    self.aux = Ramp::raw(0.8, 0.0, 0.2, 0);
                    self.thr = Ramp::raw(100.0, 100.0, 0.01, 0);
                    self.started = self.clock;
                }
            }
        } else if self.last && self.state == 0 && shifting == 0 && p.rpm >= 6000.0 && p.gear > 2 {
            // 0x45CDC0.
            self.lift = true;
        }
        self.step(p, eng_rpm, rec, dt);
    }

    /// 0x47C290.
    pub(super) fn step(&mut self, p: &Physics, eng_rpm: f32, rec: (f32, i32, i32, i32, i32), dt: f32) {
        self.aux.step(dt);
        // 0x47C1A0.
        match self.state {
            1 | 3 | 4 => self.rpm.track(p.rpm, dt),
            2 => self.rpm.step(dt),
            _ => {}
        }
        // 0x47C220: the throttle follows the Physics throttle.
        if (1..5).contains(&self.state) {
            self.thr.track(p.throttle, dt);
            if self.thr.done {
                self.thr = Ramp::set(p.throttle, p.throttle, 0, 0);
            }
        }
        if self.state == 0 {
            return;
        }
        if !p.on && self.state != 4 {
            self.rpm = Ramp::set(eng_rpm, p.rpm, rec.4, 6);
            self.state = 4;
            self.aux = Ramp::set(self.aux.value, 0.0, rec.4, 0);
        }
        match self.state {
            1 | 3 | 4 if self.rpm.done => self.state = 0,
            2 if self.rpm.done => {
                self.state = 3;
                self.rpm = Ramp::set(self.rpm.value, p.rpm, rec.3, 0);
                self.aux = Ramp::set(self.aux.value, 0.0, rec.3, 0);
            }
            _ => {}
        }
    }
}
