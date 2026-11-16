use super::{DECELS, ENGINES, Interp, Mean4, Rec80, Rec84};
use crate::audio::carfx::RunningMean3;
use crate::audio::transients::{AccelTrans, LAUNCH_INDEX, LAUNCHES, Physics, SHIFT_RECS, Shifting, sin16};

/// x87 `fistp` / `_ftol`: the chain's int conversions are `_ftol`
/// (truncation, 0x75BED4).
fn t(x: f32) -> i32 {
    x as i32
}

/// The player's car this frame, as SFXCTL_Physics reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineInput {
    /// Throttle input, 0–1 (car +0x30 → +0x20C).
    pub throttle: f32,
    /// Engine rpm (car +0x440) and the car type's idle / max rpm (the
    /// tuning's +0x1F0 / +0x1F8 through 0x5B5360(1), 0x45D370).
    pub rpm: f32,
    pub idle_rpm: f32,
    pub max_rpm: f32,
    /// The gear (car +0x434: ctx +0x50; 0 reverse, 1 neutral, 2 first).
    pub gear: i32,
    /// Speed, mph (car +0x430 × 2.237; the launch test).
    pub mph: f32,
    /// The drivetrain's up-shift rpm in this gear (0x5ECD40; the tip-in
    /// test).
    pub up_rpm: f32,
    /// The shift record (`carfx::shift_record`) and the transmission level
    /// (ctx +0xA4 → Shifting +0x64).
    pub shift_record: usize,
    pub transmission: i32,
    /// Race type 8 (no launch transient).
    pub race_type_8: bool,
    /// 0x5ECD80: the human car is changing gear (car +0x28 == 1 and the
    /// transmission's +0x48, the drivetrain's `pending`; read live under
    /// Wine: 1 for 0.35–0.51 s around every shift).
    pub shifting: bool,
}

/// The sound context fields Physics writes (0x484CEE–0x484E75).
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    /// +0x44: the throttle %, moved at most 30 per update toward the input.
    pub throttle: f32,
    /// +0x48: the rpm mapped to 1000..10000 over idle..max.
    pub rpm: f32,
    /// +0x4C: on throttle (the input above 30 %).
    pub on: bool,
    /// +0x54: the input throttle %.
    pub input: f32,
}

/// One frame of the chain; `voices()` gives the player's two GIN voices.
#[derive(Clone, Debug)]
pub struct HybridMotor {
    pub entry: usize,
    pub ctx: Context,
    /// Engine +0x130 (3-frame mean of ctx +0x44), +0x128 / +0x12C (rpm).
    mean: RunningMean3,
    pub eng_throttle: f32,
    eng_rpm: f32,
    eng_prev: f32,
    /// Engine +0xE4 (limiting), +0x104 → +0x118 (volume) and +0xE8 → +0xFC.
    limiting: bool,
    pub limiter: Interp,
    limiter_b: Interp,
    /// HM +0xF4, +0xC4 (filter), +0xF8, +0x100 / +0xFC, +0xBC.
    prev_ctx_rpm: f32,
    filter: Mean4,
    pub d0: f32,
    pub rpm_map: f32,
    pub gain_a: f32,
    pub gain_b: f32,
    /// HM +0x7C / +0x80 / +0x84 / +0xB8 (0..0x7FFF).
    pub out: [i32; 4],
    /// The transients: SFXCTL_Shifting and SFXCTL_AccelTrans, the
    /// Physics fields they read, Engine +0xE0 (volume, 0x7FFF outside
    /// shifts), the wobble outputs +0x140 (volume) / +0x144 (rpm) with
    /// their phases +0x150 / +0x14C, and the idle rpm limiter flag +0x13C.
    pub shift: Shifting,
    pub accel: AccelTrans,
    pub phys: Physics,
    started: bool,
    pub e0: i32,
    wob_vol: f32,
    wob_rpm: f32,
    phases: [u16; 3],
    idle: bool,
    /// The last update's 0x5ECD80 ([`EngineInput::shifting`]).
    pub(super) shifting: bool,
}

impl HybridMotor {
    pub fn new(entry: usize) -> HybridMotor {
        HybridMotor {
            entry: entry.min(ENGINES.len() - 1),
            ctx: Context::default(),
            mean: RunningMean3::default(),
            eng_throttle: 0.0,
            eng_rpm: 0.0,
            eng_prev: 0.0,
            limiting: false,
            limiter: Interp::at(1.0),
            limiter_b: Interp::at(0.0),
            prev_ctx_rpm: 0.0,
            filter: Mean4::default(),
            d0: 0.0,
            rpm_map: 0.0,
            gain_a: 0.0,
            gain_b: 0.0,
            out: [0; 4],
            shift: Shifting::default(),
            accel: AccelTrans::default(),
            phys: Physics::default(),
            started: false,
            e0: 0x7FFF,
            wob_vol: 0.0,
            wob_rpm: 0.0,
            phases: [0; 3],
            idle: false,
            shifting: false,
        }
    }

    pub fn rec80(&self) -> &'static Rec80 {
        &ENGINES[self.entry]
    }

    /// The decel record: rec80 +0x14, or entry 0 when the entry is not
    /// hybrid (SetCar 0x4594F5; the [0x850078] = 4 case is not taken).
    pub fn rec84(&self) -> &'static Rec84 {
        let r = self.rec80();
        &DECELS[if r.hybrid { r.decel.min(DECELS.len() - 1) } else { 0 }]
    }

    /// The trapezoid 0x45CF90 builds from rec84 at HM +0x88, read by
    /// 0x5767A0 (linear between points, the end values outside).
    pub fn trapezoid(&self, x: f32) -> f32 {
        let r = self.rec84();
        let (a, b) = (r.rpm0 as f32, r.rpm1 as f32);
        let pts = [(0.0, 0.0), (a, 0.0), (a + (b - a) * r.down, 1.0), (b - (b - a) * r.up, 1.0), (b, 0.0)];
        if x <= pts[0].0 {
            return pts[0].1;
        }
        for w in pts.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            if x <= x1 {
                return if x1 > x0 { y0 + (y1 - y0) * (x - x0) / (x1 - x0) } else { y1 };
            }
        }
        pts[4].1
    }

    /// Scripted sound-physics input; body/drivetrain state is not changed.
    pub fn update_scripted(&mut self, input: &EngineInput, pattern: &crate::audio::scripted::Pattern, dt: f32) {
        self.update_with(input, dt, Some(pattern));
    }

    /// One update; `dt` in seconds.
    pub fn update(&mut self, input: &EngineInput, dt: f32) {
        self.update_with(input, dt, None);
    }

    fn update_with(&mut self, input: &EngineInput, dt: f32, scripted: Option<&crate::audio::scripted::Pattern>) {
        if let Some(pattern) = scripted {
            // 484CDA returns before ordinary input/gear refresh. 46FA30
            // writes only these Physics/context fields; preserve the rest.
            self.ctx.rpm = pattern.rpm;
            self.ctx.throttle = pattern.throttle;
            self.ctx.input = pattern.throttle;
            self.ctx.on = pattern.throttle > 30.0;
            self.phys.rpm = pattern.rpm;
            self.phys.throttle = pattern.throttle;
            self.phys.input = pattern.throttle;
            self.phys.on = self.ctx.on;
            self.phys.scripted = true;
        } else {
            // SFXCTL_Physics 0x484C50 (+0xD8 clear: the player drives).
            let raw = input.throttle * 100.0;
            let old = self.ctx.throttle;
            self.ctx.throttle = if raw > old + 30.0 {
                old + 30.0
            } else if raw < old - 30.0 {
                old - 30.0
            } else {
                raw
            };
            let span = input.max_rpm - input.idle_rpm;
            self.ctx.rpm = if span != 0.0 { (input.rpm - input.idle_rpm) / span * 9000.0 + 1000.0 } else { 1000.0 };
            self.ctx.on = raw > 30.0;
            // +0x90 = the last +0x8C, +0xCC = the last gear +0xC8. The
            // constructor leaves +0xC8 = 1 and +0x8C = 0, so the first update
            // sees gear 1 and input 0 as the last (0x484CEE, read live under
            // Wine); scripted updates (0x46FA30) write +0x8C, not +0xC8.
            let (prev_input, prev_gear) = (self.phys.input, if self.started { self.phys.gear } else { 1 });
            self.started = true;
            self.ctx.input = raw;
            self.phys = Physics {
                rpm: self.ctx.rpm,
                throttle: self.ctx.throttle,
                input: raw,
                prev_input,
                gear: input.gear,
                prev_gear,
                on: self.ctx.on,
                mph: input.mph,
                up_rpm: input.up_rpm,
                race_type_8: input.race_type_8,
                scripted: false,
            };
        }
        // SFXCTL_Shifting 0x483130 and SFXCTL_AccelTrans 0x4831A0.
        let rec = SHIFT_RECS[input.shift_record.min(15)];
        self.shift.level = input.transmission;
        // 0x470540 / 0x4701A0 test Physics +0xD8 (scripted) before 0x5ECD80.
        self.shifting = input.shifting && !self.phys.scripted;
        let mut eng = (self.eng_rpm, self.eng_prev);
        self.shift.update(&self.phys, &rec, &mut eng, self.eng_throttle, dt);
        (self.eng_rpm, self.eng_prev) = eng;
        let launch = LAUNCHES[LAUNCH_INDEX[self.entry] as usize];
        self.accel.update(&self.phys, self.shift.state, false, self.eng_rpm, launch, dt);
        // SFXCTL_Engine 0x47C010.
        self.engine(dt);
        self.rev_limiter(self.phys.gear, dt * 1000.0);
        let e0 = self.e0;
        // HM 0x45D0A0: the rpm change filter.
        let ec = self.ctx.rpm - self.prev_ctx_rpm;
        let f0 = self.eng_rpm - self.eng_prev;
        self.prev_ctx_rpm = self.ctx.rpm;
        match (self.accel.state, self.shift.state) {
            (1, _) => self.filter.fill(ec),
            (a, _) if a != 0 => self.filter.push(f0),
            (_, 0) => self.filter.push(ec),
            (_, 5) => self.filter.fill(f0),
            _ if self.shift.changed == 2 => self.filter.fill(ec * 0.2),
            _ => self.filter.push(ec),
        }
        self.d0 = self.filter.mean();
        // +0xF8: the recording's rpm.
        let r = *self.rec80();
        self.rpm_map =
            r.rpm_min + (self.eng_rpm.clamp(1000.0, 10000.0) - 1000.0) * (1.0 / 9000.0) * (r.rpm_max - r.rpm_min);
        if r.hybrid {
            self.hybrid(e0)
        } else {
            self.single(e0)
        }
        // 0x45D190.
        let l = self.limiter.value;
        self.out[3] = t(self.limiter_b.value * self.out[3] as f32);
        for k in 0..3 {
            self.out[k] = t(self.out[k] as f32 * l);
        }
    }

    /// SFXCTL_Engine's slots before the rev limiter: the idle flag
    /// (0x46F9D0), the shift wobble (slot 0x3C, 0x45C7A0), the rpm (slot
    /// 0x24, 0x46F7D0), the throttle (slot 0x28, 0x45C710) and the volume
    /// +0xE0 (slot 0x2C, 0x45C6C0).
    fn engine(&mut self, dt: f32) {
        // 0x46F9D0: the player (owner kind 1, racer +0x900 clear:
        // prov: inferred audio.hybrid.engine
        // **inferred** clear) with AccelTrans idle at ≤ 2500 rpm (0x7885D0).
        self.idle = self.accel.state == 0 && self.eng_rpm <= 2500.0;
        // 0x45C7A0: the wobble oscillators from the Shifting's +0x80..+0x94;
        // the phase steps by dt / period · 65535 (mod 65535) and the output
        // is Sin16(phase) × the amplitude; outside shifts all 0 and the
        // phases 0x4097.
        if self.shift.state != 0 {
            let w = self.shift.wob;
            let osc = |phase: &mut u16, amp: i32, period: i32| -> Option<f32> {
                if amp == 0 {
                    return None;
                }
                let p = period.clamp(1, 10000) as f32;
                let d = (dt / (p * 0.001) * -65535.0) as i32;
                *phase = ((phase.wrapping_sub(d as u16)) as u32 % 0xFFFF) as u16;
                Some(sin16(*phase) * amp as f32)
            };
            if let Some(v) = osc(&mut self.phases[0], w[4], w[5]) {
                self.wob_rpm = v;
            }
            // The throttle wobble (+0x88 / +0x8C, phase +0x14E) is never
            // set: +0x148 stays 0.
            if let Some(v) = osc(&mut self.phases[2], w[0], w[1]) {
                self.wob_vol = v;
            }
        } else {
            self.wob_rpm = 0.0;
            self.wob_vol = 0.0;
            self.phases = [0x4097; 3];
        }
        // 0x46F7D0: the shift's rpm ramp, else AccelTrans's, else the
        // context's; with the idle flag outside shifts the context's moves
        // at most −60 / +999 an update (0x7885CC, 0x78AAB0).
        let mut src = if self.shift.state != 0 {
            self.shift.rpm.value
        } else if self.accel.state != 0 {
            self.accel.rpm.value
        } else {
            self.ctx.rpm
        };
        if self.idle && self.shift.state == 0 {
            src = self.ctx.rpm.clamp(self.eng_rpm - 60.0, self.eng_rpm + 999.0);
        }
        self.eng_prev = self.eng_rpm;
        // + 2 · +0x144 + +0x1C8 (+0x1C8: no writer in the class, 0).
        self.eng_rpm = src + self.wob_rpm + self.wob_rpm;
        // 0x45C710: the 3-frame mean, reset to a transient's throttle ramp.
        self.eng_throttle = if self.shift.state != 0 {
            self.mean.fill(self.shift.thr.value)
        } else if self.accel.state != 0 {
            self.mean.fill(self.accel.thr.value)
        } else {
            self.mean.push(self.ctx.throttle)
        };
        // 0x45C6C0: 0x7FFF − trunc(bump · −32767) in a shift, + trunc(+0x140).
        self.e0 = 0x7FFF;
        if self.shift.state != 0 {
            self.e0 = 0x7FFF - t(self.shift.bump.value * -32767.0);
        }
        self.e0 += t(self.wob_vol);
    }

    /// The rev limiter (0x46F610): above 9800 rpm outside shifts the
    /// volume ramps to 0.15 over 450 · v · s ms (s = the gear clamped to
    /// 1..5, 0x7F0050) and +0xFC to 0.85 over (1 − b) · 120 · s ms; back
    /// under it, to 1.0 over (1 − v) · 50 ms and 0 over 50 · b ms.
    fn rev_limiter(&mut self, gear: i32, dt_ms: f32) {
        let shifting = self.shift.state != 0;
        if !self.limiting && !shifting && self.eng_rpm > 9800.0 {
            self.limiting = true;
            let s = (gear as f32).clamp(1.0, 5.0);
            let v = self.limiter.value;
            self.limiter.set(v, 1.0 - 0.85, t(450.0 * v * s), 0);
            let b = self.limiter_b.value;
            self.limiter_b.set(b, 0.85, t((1.0 - b) * 120.0 * s), 0);
        } else if self.limiting && (shifting || self.eng_rpm < 9800.0) {
            self.limiting = false;
            let v = self.limiter.value;
            self.limiter.set(v, 1.0, t((1.0 - v) * 50.0), 0);
            let b = self.limiter_b.value;
            self.limiter_b.set(b, 0.0, t(50.0 * b), 0);
        }
        self.limiter.step(dt_ms);
        self.limiter_b.step(dt_ms);
    }

    /// ±0.2 per update toward the targets (0x470718), or a snap.
    fn smooth(&mut self, a: f32, b: f32, snap: bool) {
        if snap {
            (self.gain_a, self.gain_b) = (a, b);
            return;
        }
        self.gain_a = a.clamp(self.gain_a - 0.2, self.gain_a + 0.2);
        self.gain_b = b.clamp(self.gain_b - 0.2, self.gain_b + 0.2);
    }

    /// 0x470540 (Physics +0xD8 clear). In a shift, or while 0x5ECD80 says
    /// the human car is changing gear (jump table 0x470924, taken even with
    /// the Shifting state 0: state − 2 > 5 goes to the default 0x4706ED):
    /// state 2 the rec80 lerp snapped, 3 / 5 the rec80 lerp, 7 the rec80
    /// lerp (snapped on throttle), 0 / 1 / 4 / 6 the rec84 lerp. Else by
    /// AccelTrans: idle, on throttle the rec80 lerp by t1 and off it
    /// rec84's by t2; a transient the rec80 lerp, snapped in a tip-in
    /// (state 1).
    fn hybrid(&mut self, e0: i32) {
        let (r, d) = (*self.rec80(), *self.rec84());
        let t1 = (1.0 - (r.d0 - self.d0.abs()) / r.d0).clamp(0.0, 1.0);
        let t2 = (1.0 - (d.d0 - self.d0.abs()) / d.d0).clamp(0.0, 1.0);
        let accel = ((r.a1 - r.a0) * t1 + r.a0, (r.b1 - r.b0) * t1 + r.b0);
        let decel = ((d.a1 - d.a0) * t2 + d.a0, (d.b1 - d.b0) * t2 + d.b0);
        let ((a, b), snap) = match (self.shifting || self.shift.state != 0, self.shift.state) {
            (false, _) => match self.accel.state {
                0 => (if self.ctx.on { accel } else { decel }, false),
                s => (accel, s == 1),
            },
            (_, 2) => (accel, true),
            (_, 3 | 5) => (accel, false),
            (_, 7) => (accel, self.ctx.on),
            _ => (decel, false),
        };
        self.smooth(a, b, snap);
        let v = self.eng_throttle;
        let s = self.trapezoid(self.rpm_map);
        let f = v * 0.01 + (1.0 - v * 0.01) * s;
        self.gain_a = (1.0 - f) + f * self.gain_a;
        self.gain_b *= f;
        let g = e0 as f32 * (1.0 / 32767.0);
        self.out[0] = t((d.vol_a - t(v * (r.vol_a - d.vol_a) as f32 * -0.01)) as f32 * self.gain_a * g);
        self.out[1] = t(v * r.vol_b as f32 * g * self.gain_b * 0.01);
        self.out[2] = t((1.0 - 0.01 * v) * d.vol_d as f32 * g * self.gain_b);
        self.out[3] = t(r.vol_a as f32 * g);
    }

    /// 0x4701A0 (entries 9 and 38): the lerp by t (rising rpm only) or the
    /// fixed gains. In a shift or while 0x5ECD80 is set (0x47024C, as
    /// 0x470540): 2 lerp snapped, 3 / 5 lerp, 7 lerp snapped on throttle
    /// else fixed, 0 / 1 / 4 / 6 fixed; else AccelTrans idle: the
    /// lerp above 5 % input, else fixed; tip-in: lerp snapped; launch: lerp.
    fn single(&mut self, e0: i32) {
        let r = *self.rec80();
        let tt = (self.d0 / r.d0).clamp(0.0, 1.0);
        let lerp = (r.a0 + tt * (r.a1 - r.a0), r.b0 + tt * (r.b1 - r.b0));
        let fixed = (r.fixed_a, r.fixed_b);
        let ((a, b), snap) = match (self.shifting || self.shift.state != 0, self.shift.state) {
            (false, _) => match self.accel.state {
                0 => (if self.ctx.input > 5.0 { lerp } else { fixed }, false),
                s => (lerp, s == 1),
            },
            (_, 2) => (lerp, true),
            (_, 3 | 5) => (lerp, false),
            (_, 7) if self.ctx.on => (lerp, true),
            _ => (fixed, false),
        };
        self.smooth(a, b, snap);
        let g = e0 as f32 * (1.0 / 32000.0);
        self.out[0] = t(r.vol_a as f32 * self.gain_a * g);
        self.out[1] = t(r.vol_b as f32 * self.gain_b * g);
        self.out[2] = 0;
        self.out[3] = t(r.vol_a as f32 * g);
    }

    /// The shift outputs the car's effect sounds read
    /// ([`crate::audio::carfx::ShiftSignal`]).
    pub fn shift_signal(&self) -> crate::audio::carfx::ShiftSignal {
        crate::audio::carfx::ShiftSignal {
            play: self.shift.play,
            state: self.shift.state,
            swtn_start: self.shift.swtn_start,
            swtn_engage: self.shift.swtn_engage,
            ctx_rpm: self.ctx.rpm,
            eng_throttle: self.eng_throttle,
            ctx_throttle: self.ctx.throttle,
            shift_rpm: self.shift.shift_rpm,
            entry: self.entry,
            id: self.abk_id(),
            valid: true,
        }
    }

    /// The engine record's EE bank (rec80 +0x00; [0x850078] ≠ 4 picks EE
    /// over SPU at 0x465C80) and its id (+0x08: the entry, 41 and 42
    /// reuse 14 and 12).
    pub fn abk_id(&self) -> i32 {
        match self.entry {
            41 => 14,
            42 => 12,
            e => e as i32,
        }
    }

    pub fn abk_path(&self) -> String {
        format!("SOUND/ENGINE/CAR_{:02}_ENG_MB_EE.abk", self.abk_id())
    }

    /// The "CAR" event's creation block (0x47D920: +4 the id clamped
    /// 0..60, +0x34 = ([0x850078] ≠ 4) = 1, the rest 0), as event
    /// parameters from +4.
    pub fn abk_first_block(&self) -> [i32; 26] {
        let mut b = [0; 26];
        b[0] = self.abk_id().clamp(0, 60);
        b[12] = 1;
        b
    }

    /// The per-frame block (0x474310 → 0x735580), parameters from +4:
    /// p1 (+0x08) = clamp(trunc(+0xF8), 0, 10000); p3 (+0x10) =
    /// clamp(trunc(v · 10.24), 0, 0x400); p4 / p5 (+0x14 / +0x18) =
    /// clamp(((HM +0x7C · level0) >> 15) − 0x7FFF, −32767, 0) (the main
    /// layers' attenuation); p13 (+0x38) = clamp([0x82B880] − 0x7FFF,
    /// −0x7FFF, 0) and p14 (+0x3C) = ([0x82B070] · level0) >> 15 (the
    /// tunnel / master mix globals of 0x4717xx: 0x7FFF outside tunnels,
    // prov: inferred audio.hybrid.abk_block
    /// **inferred**); p15 (+0x40) the azimuth (0); p16 (+0x44) = 0 (+0xBC
    // prov: open audio.hybrid.abk_block-2
    /// = 1.0); p17 (+0x48) = Engine +0x134 (0 here, **open**; the graph
    /// does not read it); p21 (+0x58) the cutoff Engine +0x138 (24000
    // prov: open audio.hybrid.abk_block-3
    /// here, **open**: the graph sends it as kind 6, not mixed); p25
    /// (+0x68) = clamp((HM +0xB8 · level0) >> 15, 0, 0x7FFF): the rev
    /// limiter layer (0 until +0xFC ramps up).
    pub fn abk_block(&self, level0: i32) -> [i32; 26] {
        self.abk_block_fx(level0, 0x7FFF, 0x7FFF)
    }

    /// [`Self::abk_block`] with the reverb's wet B ([0x82B070], p14) and
    /// dry B ([0x82B880], p13) levels (0..0x7FFF, `reverb::ReverbCtl::out`).
    pub fn abk_block_fx(&self, level0: i32, wet_b: i32, dry_b: i32) -> [i32; 26] {
        let mut b = self.abk_first_block();
        let lv = |x: i32| ((x as i64 * level0 as i64) >> 15) as i32;
        b[1] = t(self.rpm_map).clamp(0, 10000);
        b[3] = t(self.eng_throttle * 10.24).clamp(0, 0x400);
        b[4] = (lv(self.out[0]) - 0x7FFF).clamp(-32767, 0);
        b[5] = b[4];
        b[13] = (dry_b - 0x7FFF).clamp(-0x7FFF, 0);
        b[14] = lv(wet_b).clamp(0, 0x7FFF);
        b[21] = 24000;
        b[25] = lv(self.out[3]).clamp(0, 0x7FFF);
        b
    }

    /// Engine +0x138, the cutoff the abk block sends as p21 (0x46F460, each
    /// SFXCTL_Engine update at 0x47C186): `dist` the 3DCarPos field f1 =
    /// trunc(2D distance car → camera), −1 without one (0x45DF00); x =
    /// (dist − 6.5) / D, D = 22, or 90 for the human player's car under
    /// the camera director's kinds 6 / 7 (`far_cam`), clamped to 0..1; v =
    /// trunc(x · 32766); mB = curve 4 (as mB) of v, −10000 under −9600
    /// (doubled, as 0x468160's types 2 / 6); cutoff = trunc(lin(mB) / 32767
    /// · (24000 − 725) + 725), clamped to 0..0x7FFF: 23920 within 6.5 m
    /// (curve 4's first entry is lin 32655), toward 725 by 28.5 m.
    /// (Rec80 +0x48 is not read on this path.)
    pub fn engine_cutoff(dist: i32, far_cam: bool) -> i32 {
        let d = if far_cam { 90.0f32 } else { 22.0 };
        let x = ((dist as f32 - 6.5) / d).clamp(0.0, 1.0);
        let v = (x * 32766.0) as i32;
        let mut mb = crate::audio::mixmap::curve(4, v, true) * 2;
        if mb < -9600 {
            mb = -10000;
        }
        let lin = crate::audio::mixmap::db_to_lin(mb);
        ((lin as f32 * (1.0 / 32767.0) * (24000.0 - 725.0) + 725.0) as i32).clamp(0, 0x7FFF)
    }

    /// The player's voices (0x474030 / 0x473E60): the accel GIN with
    /// +0x80 and (hybrid) the decel GIN with +0x84, each volume byte
    /// clamp(((x · level0) >> 15) >> 8, 0, 127); both SetRPM(max(+0xF8,
    /// the accel GIN's minimum rpm), 60 ms).
    pub fn voices(&self, level0: i32, gin_min_rpm: f32) -> (f32, [u8; 2]) {
        let byte = |x: i32| (((x as i64 * level0 as i64) >> 15) >> 8).clamp(0, 127) as u8;
        let second = if self.rec80().hybrid { byte(self.out[2]) } else { 0 };
        // Check aid (NFSU2_EVENT_LOG): the bytes and their inputs.
        crate::audio::aems::log::check_row(
            "GIN",
            &[
                byte(self.out[1]) as i32,
                second as i32,
                self.out[1],
                self.out[2],
                level0,
                // The rpm change filter HM +0xD0 (×1000) and the B gain (×10000).
                t(self.d0 * 1000.0),
                t(self.gain_b * 10000.0),
                t(self.ctx.rpm),
            ],
        );
        (self.rpm_map.max(gin_min_rpm), [byte(self.out[1]), second])
    }
}
