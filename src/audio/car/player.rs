use super::nitrous::*;
use super::tables::*;
use super::turbo::*;
use crate::audio::banks::bank_events::*;
use crate::audio::banks::cache::*;
use crate::audio::banks::sink::*;
use std::collections::HashMap;
use std::path::Path;

/// What the engine chain's `SFXCTL_Shifting` / `SFXCTL_Engine`
/// ([`crate::audio::transients::Shifting`], run by [`crate::audio::hybrid::HybridMotor`])
/// hand the car's effect sounds this frame: the clunk flag +0x70 (set
/// when the clock passes the record's delay after a shift, 0x482D50),
/// the state +0x78 (CARSFX_Shift plays variant 1 in the up-shift states
/// 1–3), the flags +0x71 / +0x72 (the CAR_SWTN sweetener's), the context
/// rpm (ctx +0x48, the shift volume's) and the Engine throttle +0x130
/// (the turbo's boost value, slot 0x38).
#[derive(Clone, Copy, Debug, Default)]
pub struct ShiftSignal {
    pub play: bool,
    pub state: i32,
    pub swtn_start: bool,
    pub swtn_engage: bool,
    pub ctx_rpm: f32,
    pub eng_throttle: f32,
    /// The context throttle +0x44 (0..100), the Shifting's rpm at the
    /// shift (+0xA8), the engine entry (ctx +0x7C) and its id (engine
    /// record +8).
    pub ctx_throttle: f32,
    pub shift_rpm: f32,
    pub entry: usize,
    pub id: i32,
    /// Whether an engine chain ran this frame (else the effect sounds
    /// fall back to the car's own throttle for the turbo).
    pub valid: bool,
}

/// The player car's nitrous, turbo and shift sounds, frame by frame.
/// Each object starts its bank's AEMS events with the game's parameter
/// blocks and the graphs ([`crate::audio::aems`]) play the voices.
pub struct PlayerSfx {
    pub sounds: CarFxSounds,
    pub nitrous: Nitrous,
    /// The nitrous bank; FX_NITROUS (+0xB0: variant, volume, azimuth,
    /// the stop flag) and FX_PURGE (+0xB4: volume, azimuth, the stop
    /// flag, 0x45E3D0's 0x1000).
    nos_bank: Option<EventBank>,
    nos_event: Option<u32>,
    nos_block: [i32; 4],
    /// CARSFX_Nitrous +4 ([`PlayerSfx::nitrous_field`]).
    nitrous_on: bool,
    /// This frame's nitrous edge: Some(true) FX_NITROUS started (state 0
    /// → 1, where 0x484F20 also asks for GB_NOS), Some(false) it ended
    /// (state 1 → 0, where 0x484FB9 clears 0x82BB29).
    pub nos_edge: Option<bool>,
    purge_event: Option<u32>,
    purge_block: [i32; 4],
    pub turbo: Turbo,
    /// The turbo bank; the spool event (FX_TURBO_01 variant 0, +0xBC)
    /// and the blow-off (+0xC0) with their parameter blocks (the event
    /// +4 on: variant, volume, spool · 1024, azimuth, the sound state's
    /// +0xB0, rpm).
    turbo_bank: Option<EventBank>,
    whine_event: Option<u32>,
    blow_event: Option<u32>,
    whine_block: [i32; 6],
    blow_block: [i32; 6],
    /// The shift bank and its last FX_SHIFTING_01 (+0xB0).
    shift_bank: Option<EventBank>,
    shift_event: Option<u32>,
    /// The sweetener bank (SWTN_CAR_xx): CARSFX_Shift's CAR_SWTN events
    /// (+0xBC / +0xC0) and CARSFX_SparkChatter's CAR_Sputter (+0xB4).
    swtn_bank: Option<EventBank>,
    swtn_events: [Option<u32>; 2],
    sputter_event: Option<u32>,
    pub sputter_block: [i32; 11],
    /// CAR_SWTN events started this frame (for the self-test).
    pub sweeteners: usize,
    /// The skid bank, its two FX_SKID events (0x4608D0) and the
    /// controllers (record index, wheel, skid).
    skid_bank: Option<EventBank>,
    skid_events: [Option<u32>; 2],
    skid_ctl: Option<(usize, crate::audio::skids::WheelCtl, crate::audio::skids::SkidCtl)>,
    pub skid_blocks: [[i32; 17]; 2],
    /// The AI racers' CARSFX_Skids (one event each, made from the same
    /// FX_SKID module of this bank, 0x4608D0 on record 0x82C500): this
    /// frame's inputs ([`PlayerSfx::set_ai_skids`]) and each racer's
    /// controllers, event and block.
    ai_skid_in: Vec<AiSkidInput>,
    pub ai_skids: Vec<AiSkid>,
    /// The engine's EE bank and its "CAR" event (0x47D920: the +0xB0
    /// block), the steady engine layers and the rev-limiter layer.
    engine_bank: Option<EventBank>,
    engine_event: Option<u32>,
    /// The graphs' voices: (volume, second volume).
    gains: HashMap<u32, (f32, f32, f32)>,
    /// SFXCTL_Engine's running mean of the throttle (+0x90, 3 frames).
    boost: RunningMean3,
    /// The sound context's +0x44 (0x484C50: the pedal % slewed ±30 per
    /// update), the turbo's boost source through Engine +0x130's mean.
    ctx44: f32,
    /// What started this frame (for the self-test): nitrous voices, shift
    /// events.
    pub started_nitrous: usize,
    pub shifts: usize,
    /// Blow-offs started this frame (for the self-test).
    pub blow_offs: usize,
}
/// One AI racer's inputs to its skid sound this frame.
#[derive(Clone, Copy, Debug)]
pub struct AiSkidInput {
    pub skid: crate::audio::skids::SkidInput,
    /// The racer's car's engine sound index (the skid record's choice).
    pub sound_index: usize,
    /// The racer's copy of the mix map's section 2 (class 5).
    pub mix: crate::audio::mixmap::Levels,
}

/// One AI racer's CARSFX_Skids: the controllers (record index, wheel,
/// skid), its FX_SKID event and the last block.
#[derive(Clone, Debug, Default)]
pub struct AiSkid {
    ctl: Option<(usize, crate::audio::skids::WheelCtl, crate::audio::skids::SkidCtl)>,
    event: Option<u32>,
    pub block: [i32; 17],
}

/// The car's inputs to its sounds this frame.
#[derive(Clone, Copy, Debug)]
pub struct CarInput {
    /// The running shot's stage (0x59FA40).
    pub stage: usize,
    /// Speed, m/s.
    pub speed: f32,
    pub gear: usize,
    pub rpm: f32,
    /// The nitrous level (sound context +0xAC) and the shift record.
    pub nitrous_level: usize,
    pub shift_record: usize,
    /// The pedal, 0–1: the sound context's +0x54 = the input block's gas
    /// (car +0x30 → +0x30 → +0x20C) × 100 (SFXCTL_Physics 0x484C50 →
    /// 0x484E75); +0x44 is that slewed by at most ±30 an update (0x484E2C,
    /// `PlayerSfx::ctx44`). In a scripted scene the pattern's (0x46FA30).
    pub throttle: f32,
    /// The input block's gas (car +0x30 → +0x20C), for 0x5ECDC0.
    pub gas: f32,
    /// The turbo record (0x7F3338 index).
    pub turbo_record: usize,
    /// Whether the car has a turbo: CARSFX_Turbo's reset (vtable +0x1C,
    /// 0x45EBC0) takes the largest point of the tuning's boost curve
    /// (tuning +0x1F0 → 0x457B00: +0x34 … from 0; record +0x334, the
    /// handling's `curve_334`) over 0.25 (0x788814) and, at 0, clears the
    /// object's +0xA0, so its update (0x47CAD0) never starts FX_TURBO_01
    /// (**verified**; under Wine the stock 206 in free roam has no
    /// FX_TURBO_01 instance, 2026-11-05, docs/notes/parity/world-systems-parity.md).
    pub turbo: bool,
    /// The car type's sound index (0x459240), the drift flag (0x89E7D9 /
    /// 0x890104) and the skid controllers' inputs.
    pub sound_index: usize,
    pub drift: bool,
    /// The player's camera setting is 0, the bumper view (ctx +0xA8 == 2,
    /// 0x459200 → 0x5EB410): FX_SHIFTING_01's last parameter (0x45E8C8).
    pub bumper_cam: bool,
    pub skid: crate::audio::skids::SkidInput,
    /// The engine chain's shift outputs this frame.
    pub shift: ShiftSignal,
    /// The mix map's slots for this car (`mixmap::Levels`: object class
    /// 0 engine, 1 shift, 2 turbo, 3 nitrous, 4 SparkChatter, 5 skids, …).
    pub mix: crate::audio::mixmap::Levels,
}

impl PlayerSfx {
    pub fn new(root: &Path) -> PlayerSfx {
        PlayerSfx {
            sounds: CarFxSounds::new(root),
            nitrous: Nitrous::default(),
            nos_bank: None,
            nos_event: None,
            nos_block: [0; 4],
            nitrous_on: false,
            nos_edge: None,
            purge_event: None,
            purge_block: [0; 4],
            turbo: Turbo::default(),
            turbo_bank: None,
            whine_event: None,
            blow_event: None,
            whine_block: [0; 6],
            blow_block: [0; 6],
            shift_bank: None,
            shift_event: None,
            swtn_bank: None,
            swtn_events: [None; 2],
            sputter_event: None,
            sputter_block: [0; 11],
            sweeteners: 0,
            skid_bank: None,
            skid_events: [None; 2],
            skid_ctl: None,
            skid_blocks: [[0; 17]; 2],
            ai_skid_in: Vec::new(),
            ai_skids: Vec::new(),
            engine_bank: None,
            engine_event: None,
            gains: HashMap::new(),
            boost: RunningMean3::default(),
            ctx44: 0.0,
            started_nitrous: 0,
            shifts: 0,
            blow_offs: 0,
        }
    }

    /// The nitrous (`CARSFX_Nitrous`): FX_NITROUS starts (0x472AF0 →
    /// 0x457E70(0, level[0], angle, 0), releasing the last one) with the
    /// shot and gets its stop flag (block +0xC = 1, 0x45F2A0) when it
    /// ends; FX_PURGE starts with an all-zero block (0x45F2D0) and gets
    /// its stop flag at +8 (0x45F3E0). Per frame (0x45F4B0) the nitrous
    /// event's volume is level[0] and the purge's level[1], with the
    /// azimuths (0 here) and the purge's +0xC = 0x1000 (0x45E3D0); an
    /// event whose instance has ended is dropped. The graph does the
    /// fades. Levels and azimuths: the mix map's (`CarInput::mix`).
    fn nitrous_update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        let path = nitrous_path(car.nitrous_level);
        open_bank(&mut self.nos_bank, &mut self.sounds, &mut self.gains, sink, &path);
        let events = self.nitrous.update(car.stage, car.speed, car.gas);
        self.nos_edge = None;
        for e in &events {
            match e {
                NosEvent::StartNitrous => self.nos_edge = Some(true),
                NosEvent::StopNitrous => self.nos_edge = Some(false),
                _ => {}
            }
        }
        if let Some(b) = self.nos_bank.as_mut() {
            let ev = &mut b.events;
            for e in events {
                match e {
                    NosEvent::StartNitrous => {
                        self.nitrous_on = true;
                        if let Some(id) = self.nos_event.take() {
                            ev.release(id);
                        }
                        self.nos_block = [0, car.mix.level(3, 0), car.mix.az(3, 0), 0];
                        self.nos_event = ev.module("FX_NITROUS").and_then(|m| ev.play(m, &self.nos_block));
                    }
                    NosEvent::StopNitrous => {
                        self.nitrous_on = false;
                        self.nos_block[3] = 1;
                        if let Some(id) = self.nos_event {
                            ev.set(id, &self.nos_block);
                        }
                    }
                    NosEvent::StartPurge => {
                        if let Some(id) = self.purge_event.take() {
                            ev.release(id);
                        }
                        self.purge_block = [0; 4];
                        self.purge_event = ev.module("FX_PURGE").and_then(|m| ev.play(m, &self.purge_block));
                    }
                    NosEvent::StopPurge => {
                        self.purge_block[2] = 1;
                        if let Some(id) = self.purge_event {
                            ev.set(id, &self.purge_block);
                        }
                    }
                }
            }
            self.nos_event = self.nos_event.filter(|&id| ev.alive(id));
            self.purge_event = self.purge_event.filter(|&id| ev.alive(id));
            if let Some(id) = self.nos_event {
                self.nos_block[1] = car.mix.level(3, 0);
                self.nos_block[2] = car.mix.az(3, 0);
                ev.set(id, &self.nos_block);
            }
            if let Some(id) = self.purge_event {
                self.purge_block = [car.mix.level(3, 1), car.mix.az(3, 1), self.purge_block[2], 0x1000];
                ev.set(id, &self.purge_block);
            }
        }
        self.started_nitrous += run_bank(&mut self.nos_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// The turbo (`CARSFX_Turbo`) through its bank's FX_TURBO_01 graph:
    /// the spool event (0x45F0A0, variant 0, started at volume 0 when none
    /// runs) and the blow-off (0x45EE70 → 0x458260: variant 1–3, volume 0,
    /// spool · 1024, the rpm), released when the turbo object ends it
    /// (0x45F050). Per frame (0x45ECE0) the spool event gets volume
    /// (+0xE4 × level[0]) >> 15, spool · 1024 (0..0x2000), the azimuth
    /// (0 here), the sound state's +0xB0 (0 here; the graph does not read
    /// it) and the rpm (0..10000); the blow-off its volume. The graph
    /// picks the samples, pitches and volumes. The mix map's levels
    /// (slot 0x38: 0, 1, 2) and the azimuth (slot 0) come from `CarInput::mix`.
    fn turbo_update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        let path = turbo_path(car.turbo_record);
        if self.turbo_bank.as_ref().is_none_or(|b| b.path != path) {
            self.whine_event = None;
            self.blow_event = None;
        }
        open_bank(&mut self.turbo_bank, &mut self.sounds, &mut self.gains, sink, &path);
        // The boost value: SFXCTL_Engine +0x130 (slot 0x38).
        let pedal = car.throttle * 100.0;
        self.ctx44 = pedal.clamp(self.ctx44 - 30.0, self.ctx44 + 30.0);
        let mean = self.boost.push(self.ctx44);
        let boost = if car.shift.valid { car.shift.eng_throttle } else { mean };
        let Some(b) = self.turbo_bank.as_mut() else { return };
        let ev = &mut b.events;
        let module = ev.module("FX_TURBO_01");
        let spool = |t: &Turbo| ((t.fraction * 1024.0) as i32).clamp(0, 0x2000);
        // The sound context's rpm +0x48 (1000..10000 over idle..max).
        let ctx_rpm = if car.shift.valid { car.shift.ctx_rpm } else { car.rpm };
        let rpm = (ctx_rpm as i32).clamp(0, 10000);
        if let Some(m) = module.filter(|_| self.whine_event.is_none_or(|id| !ev.alive(id))) {
            self.whine_block = [0, 0, spool(&self.turbo), 0, 0, rpm];
            self.whine_event = ev.play(m, &self.whine_block);
        }
        let blow_done = self.blow_event.is_none_or(|id| !ev.alive(id));
        let shifting = car.shift.state != 0;
        if let Some(TurboEvent::BlowOff { variant, .. }) =
            self.turbo.update(boost, car.throttle * 100.0, ctx_rpm, car.turbo_record, shifting, blow_done, dt)
        {
            if let Some(m) = module {
                self.blow_block = [variant as i32, 0, spool(&self.turbo), 0, car.mix.az(2, 0), rpm];
                self.blow_event = ev.play(m, &self.blow_block);
                self.blow_offs += 1;
            }
        }
        if !self.turbo.blowing {
            if let Some(id) = self.blow_event.take() {
                ev.release(id);
            }
        }
        if let Some(id) = self.blow_event {
            // 0x45ECE0: level[1] for variant 1, else level[2].
            let slot = if self.blow_block[0] == 1 { 1 } else { 2 };
            self.blow_block[1] = self.turbo.blow_level(car.mix.level(2, slot));
            ev.set(id, &self.blow_block);
        }
        if let Some(id) = self.whine_event {
            self.whine_block =
                [0, self.turbo.whine_level(car.mix.level(2, 0)), spool(&self.turbo), car.mix.az(2, 0), 0, rpm];
            ev.set(id, &self.whine_block);
        }
        run_bank(&mut self.turbo_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// The shift (`CARSFX_Shift`, 0x47C890 → 0x45E730): on the
    /// Shifting's clunk flag, FX_SHIFTING_01 with the block (variant 1 in
    /// the up-shift states 1–3 else 0, the volume, 0x1000, the azimuth, 0,
    // prov: open audio.carfx.shift_update
    /// the sound context's +0xA8 = 2 flag (0 here: **open**)), releasing
    /// the last one.
    fn shift_update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        let path = shift_path(car.shift_record);
        open_bank(&mut self.shift_bank, &mut self.sounds, &mut self.gains, sink, &path);
        if car.shift.play {
            let up = (1..=3).contains(&car.shift.state);
            // 0x45E730: level[0] up, level[1] down; the azimuth slot 0.
            let v = shift_volume(car.shift.ctx_rpm, up, car.shift_record, car.mix.level(1, !up as usize));
            if let Some(b) = self.shift_bank.as_mut() {
                let ev = &mut b.events;
                if let Some(id) = self.shift_event.take() {
                    ev.release(id);
                }
                self.shift_event = ev
                    .module("FX_SHIFTING_01")
                    .and_then(|m| ev.play(m, &[up as i32, v, 0x1000, car.mix.az(1, 0), 0, car.bumper_cam as i32]));
                self.shifts += self.shift_event.is_some() as usize;
            }
        }
        run_bank(&mut self.shift_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// The sweetener bank's two events. CARSFX_Shift's flags +0x71 / +0x72
    /// play CAR_SWTN ([`swtn_block`]). CARSFX_SparkChatter (registry
    /// 0x7ED3C8, vtable 0x78ACB0; despite its name the engine's decel
    /// crackle) starts CAR_Sputter once (slot 0x1C, 0x485130: {id, the
    /// object, 0 …}, and adds its listener 0x47CCD0 to CAR_SputOutput, the
    /// graph's node-38 output: the exhaust pops' messages, no sound) and
    /// each frame (0x45F800) sets p2 = clamp(trunc(ctx rpm), 0, 10000), p3
    /// = clamp((level[0] · record +8) >> 15, 0, 0x8013), p4 the azimuth,
    /// p5 = p6 = 0, p7 = clamp(trunc(ctx throttle · 10.24), 0, 0x400) and
    /// p10 = the Shifting state ≠ 0; p8 is toggled by 0x45F780 from ctx
    /// +0xF0 (writer not traced: 0 here). The graph pops on a lift at
    /// high rpm.
    fn swtn_update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        let sh = car.shift;
        if !sh.valid {
            return;
        }
        let path = swtn_path(sh.entry);
        if self.swtn_bank.as_ref().is_none_or(|b| b.path != path) {
            self.swtn_events = [None; 2];
            self.sputter_event = None;
        }
        open_bank(&mut self.swtn_bank, &mut self.sounds, &mut self.gains, sink, &path);
        if let Some(b) = self.swtn_bank.as_mut() {
            let ev = &mut b.events;
            for (k, (flag, rpm)) in
                [(sh.swtn_start, sh.ctx_rpm), (sh.swtn_engage, sh.shift_rpm)].into_iter().enumerate()
            {
                // 0x4720D0 level[3], 0x4722B0 level[2]; azimuth slot 0.
                if let Some(mut block) =
                    swtn_block(k as i32, sh.id, rpm, sh.entry, car.mix.level(1, 3 - k)).filter(|_| flag)
                {
                    block[4] = car.mix.az(1, 0);
                    self.swtn_events[k] = ev.module("CAR_SWTN").and_then(|m| ev.play(m, &block));
                    self.sweeteners += self.swtn_events[k].is_some() as usize;
                }
            }
            if self.sputter_event.is_none_or(|id| !ev.alive(id)) {
                let mut first = [0; 11];
                first[0] = sh.id.clamp(0, 60);
                first[1] = 1;
                self.sputter_event = ev.module("CAR_Sputter").and_then(|m| ev.play(m, &first));
            }
            let blk = &mut self.sputter_block;
            blk[0] = sh.id.clamp(0, 60);
            blk[1] = 1;
            blk[2] = (sh.ctx_rpm as i32).clamp(0, 10000);
            blk[3] = ((car.mix.level(4, 0) * SWTN[sh.entry.min(42)].2) >> 15).clamp(0, 0x8013);
            blk[4] = car.mix.az(4, 0);
            blk[5] = 0;
            blk[6] = 0;
            blk[7] = ((sh.ctx_throttle * 10.24) as i32).clamp(0, 0x400);
            blk[10] = (sh.state != 0) as i32;
            // p8 (+0x24): CARSFX_SparkChatter (0x45F780) in neutral or
            // first (ctx +0x50 = 1 / 2) after a flame pop's message 9 (ctx
            // +0xF0, 0x469BC0): 1 → 0, else the flag cleared and 1 — a pop
            // toggles p8 over one or two updates.
            if BACKFIRE.load(std::sync::atomic::Ordering::Relaxed) && matches!(car.gear, 1 | 2) {
                if blk[8] == 1 {
                    blk[8] = 0;
                } else {
                    BACKFIRE.store(false, std::sync::atomic::Ordering::Relaxed);
                    blk[8] = 1;
                }
            }
            if let Some(id) = self.sputter_event {
                ev.set(id, blk);
            }
        }
        run_bank(&mut self.swtn_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// The skids (`CARSFX_Skids`): the bank by drift flag and class, two
    /// FX_SKID events made with the creator's block (p7 the drift flag,
    /// p8–p10 0x1000), the wheel and skid controllers each frame and the
    /// per-frame blocks ([`crate::audio::skids::event_blocks`]).
    fn skid_update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        use crate::audio::skids::*;
        let path = bank_path(car.sound_index, car.drift);
        if self.skid_bank.as_ref().is_none_or(|b| b.path != path) {
            self.skid_events = [None; 2];
        }
        open_bank(&mut self.skid_bank, &mut self.sounds, &mut self.gains, sink, &path);
        let k = record_index(car.sound_index, car.drift);
        if self.skid_ctl.as_ref().is_none_or(|c| c.0 != k) {
            self.skid_ctl = Some((k, WheelCtl::default(), SkidCtl::new(RECORDS[k])));
        }
        let (_, w, s) = self.skid_ctl.as_mut().unwrap();
        w.update(&car.skid);
        s.update(w, &car.skid, dt);
        self.skid_blocks = event_blocks(w, s, &car.skid, car.drift, &car.mix);
        if let Some(b) = self.skid_bank.as_mut() {
            let ev = &mut b.events;
            for (e, block) in self.skid_events.iter_mut().zip(self.skid_blocks) {
                if e.is_none_or(|id| !ev.alive(id)) {
                    let mut first = [0; 17];
                    first[7] = (car.drift as i32).clamp(0, 3);
                    first[8..11].fill(0x1000);
                    *e = ev.module("FX_SKID").and_then(|m| ev.play(m, &first));
                }
                if let Some(id) = *e {
                    ev.set(id, &block);
                }
            }
        }
        self.ai_skid_update(car.drift, dt);
        run_bank(&mut self.skid_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// The AI racers' skid inputs for the next update, in the racers'
    /// order (none outside a race).
    pub fn set_ai_skids(&mut self, ai: Vec<AiSkidInput>) {
        self.ai_skid_in = ai;
    }

    /// The AI racers' skids: one CARSFX_Skids per racer with one event
    /// (count +0xC4 = 1, 0x473BA0), its SFXCTL_Wheel / SFXCTL_Skid on the
    /// racer's car (the record by the racer's own sound index and the
    /// drift flag, 0x45BD10), the block of 0x460A70 / 0x460C70 for event 0
    /// ([`crate::audio::skids::single_event_block`]) with the racer's mix
    /// levels, played from this bank's FX_SKID module.
    fn ai_skid_update(&mut self, drift: bool, dt: f32) {
        use crate::audio::skids::*;
        let ai = std::mem::take(&mut self.ai_skid_in);
        let mut events = self.skid_bank.as_mut().map(|b| &mut b.events);
        while self.ai_skids.len() > ai.len() {
            let a = self.ai_skids.pop().unwrap();
            if let (Some(id), Some(ev)) = (a.event, events.as_deref_mut()) {
                ev.release(id);
            }
        }
        while self.ai_skids.len() < ai.len() {
            self.ai_skids.push(AiSkid::default());
        }
        for (a, inp) in self.ai_skids.iter_mut().zip(&ai) {
            let k = record_index(inp.sound_index, drift);
            if a.ctl.as_ref().is_none_or(|c| c.0 != k) {
                a.ctl = Some((k, WheelCtl::default(), SkidCtl::new(RECORDS[k])));
            }
            let (_, w, s) = a.ctl.as_mut().unwrap();
            w.update(&inp.skid);
            s.update(w, &inp.skid, dt);
            a.block = single_event_block(w, s, &inp.skid, drift, &inp.mix);
            let Some(ev) = events.as_deref_mut() else { continue };
            if a.event.is_none_or(|id| !ev.alive(id)) {
                let mut first = [0; 17];
                first[7] = (drift as i32).clamp(0, 3);
                first[8..11].fill(0x1000);
                a.event = ev.module("FX_SKID").and_then(|m| ev.play(m, &first));
            }
            if let Some(id) = a.event {
                ev.set(id, &a.block);
            }
        }
    }

    /// The AI racers' SFXCTL_Skid outputs for the mix map (slot +0x14 →
    /// +0x10, as [`PlayerSfx::skid_level`]), in the racers' order.
    pub fn ai_skid_levels(&self) -> Vec<i32> {
        self.ai_skids
            .iter()
            .map(|a| {
                a.ctl.as_ref().map_or(0, |(_, _, s)| {
                    ((0..3).map(|j| (s.out[0][j] + s.out[1][j]) >> 1).max().unwrap_or(0) << 5).clamp(0, 0x7FFF)
                })
            })
            .collect()
    }

    /// The engine's abk event (CARSFX_PlayerEngine): "CAR" of `path`
    /// started with `first` (0x47D920; 0x7358C0 at 0x47DA5E), then each
    /// frame the block (0x474310's dispatch); the graph mixes the
    /// recordings. Returns the voices started.
    pub fn engine_abk(&mut self, sink: &mut dyn Sink, path: &str, first: &[i32], block: &[i32], dt: f32) -> usize {
        if self.engine_bank.as_ref().is_none_or(|b| b.path != path) {
            self.engine_event = None;
        }
        open_bank(&mut self.engine_bank, &mut self.sounds, &mut self.gains, sink, path);
        if let Some(b) = self.engine_bank.as_mut() {
            let ev = &mut b.events;
            if self.engine_event.is_none_or(|id| !ev.alive(id)) {
                self.engine_event = ev.module("CAR").and_then(|m| ev.play(m, first));
            }
            if let Some(id) = self.engine_event {
                ev.set(id, block);
            }
        }
        run_bank(&mut self.engine_bank, &mut self.sounds, &mut self.gains, sink, dt)
    }

    /// The engine's abk "CAR" event's sound players (state, list index,
    /// parameters; a probe for level comparisons with the original).
    pub fn engine_players(&self) -> Vec<(i32, i32, Vec<(u8, i32)>)> {
        let (Some(id), Some(b)) = (self.engine_event, self.engine_bank.as_ref()) else { return Vec::new() };
        let Some(i) = b.events.get(id) else { return Vec::new() };
        (0..).map_while(|k| i.player_state(k)).collect()
    }

    /// Whether the engine's abk "CAR" event plays.
    pub fn engine_playing(&self) -> bool {
        self.engine_event.is_some_and(|id| self.engine_bank.as_ref().is_some_and(|b| b.events.alive(id)))
    }

    /// Stop the engine's abk event (off the road).
    pub fn engine_silence(&mut self, sink: &mut dyn Sink) {
        self.engine_event = None;
        close_bank(&mut self.engine_bank, &mut self.sounds, &mut self.gains, sink);
    }

    /// SFXCTL_Skid's fields (slot 0x14, 0x47BDF0): f0..f2 = (right + left)
    /// >> 1 per layer, f3 (+0x10) = clamp(max(f0, f1, f2) << 5, 0,
    /// 0x7FFF) — the mix map's skid control input.
    pub fn skid_level(&self) -> i32 {
        self.skid_ctl.as_ref().map_or(0, |(_, _, s)| {
            ((0..3).map(|j| (s.out[0][j] + s.out[1][j]) >> 1).max().unwrap_or(0) << 5).clamp(0, 0x7FFF)
        })
    }

    /// SFXCTL_Wheel's state this frame (the skid controllers'), for the
    /// road noise.
    pub fn wheel_ctl(&self) -> Option<&crate::audio::skids::WheelCtl> {
        self.skid_ctl.as_ref().map(|c| &c.1)
    }

    /// CARSFX_Nitrous +4 (field 0 for the mix map): 1 from FX_NITROUS's
    /// start (0x472AF0, outside game flow states 2, 3, 7) to its stop
    /// (0x45F2A0).
    pub fn nitrous_field(&self) -> i32 {
        self.nitrous_on as i32
    }

    /// Stop every voice (leaving the road).
    pub fn silence(&mut self, sink: &mut dyn Sink) {
        self.skid_events = [None; 2];
        self.skid_ctl = None;
        self.swtn_events = [None; 2];
        self.sputter_event = None;
        for slot in
            [&mut self.nos_bank, &mut self.turbo_bank, &mut self.shift_bank, &mut self.skid_bank, &mut self.swtn_bank]
        {
            close_bank(slot, &mut self.sounds, &mut self.gains, sink);
        }
        self.nos_event = None;
        self.nitrous_on = false;
        self.purge_event = None;
        self.whine_event = None;
        self.blow_event = None;
        self.shift_event = None;
        self.turbo = Turbo::default();
        self.boost = RunningMean3::default();
    }

    /// The turbo object switched off (0x45EBC0 with no boost curve): its
    /// events end and its state is reset.
    fn turbo_off(&mut self, sink: &mut dyn Sink, dt: f32) {
        if let Some(b) = self.turbo_bank.as_mut() {
            for id in [self.whine_event.take(), self.blow_event.take()].into_iter().flatten() {
                b.events.release(id);
            }
        }
        if self.turbo_bank.is_some() {
            run_bank(&mut self.turbo_bank, &mut self.sounds, &mut self.gains, sink, dt);
        }
        self.turbo = Turbo::default();
    }

    /// One frame.
    pub fn update(&mut self, sink: &mut dyn Sink, car: CarInput, dt: f32) {
        self.started_nitrous = 0;
        self.shifts = 0;
        self.blow_offs = 0;
        self.nitrous_update(sink, car, dt);
        if car.turbo {
            self.turbo_update(sink, car, dt);
        } else {
            self.turbo_off(sink, dt);
        }
        self.sweeteners = 0;
        self.shift_update(sink, car, dt);
        self.swtn_update(sink, car, dt);
        self.skid_update(sink, car, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::aems::{StartBlock, VoiceHost};

    #[test]
    fn fade_matches_the_table() {
        assert_eq!(fade(0), 32767);
        assert_eq!(fade(256), 23134);
        assert_eq!(fade(460), 5115);
        assert_eq!(fade(500), 1107);
        assert_eq!(fade(511), 0);
    }

    #[test]
    fn nitrous_starts_stops_and_purges() {
        let mut n = Nitrous::default();
        assert!(n.update(0, 20.0, 1.0).is_empty());
        assert_eq!(n.update(2, 20.0, 1.0), vec![NosEvent::StartNitrous]);
        assert_eq!(n.ramp.to, 1.0);
        assert!((n.ramp.duration - 0.4).abs() < 1e-6);
        assert!(n.update(2, 20.0, 1.0).is_empty());
        n.ramp.step(0.2);
        assert_eq!(n.update(0, 20.0, 1.0), vec![NosEvent::StopNitrous]);
        assert!((n.ramp.from - 0.5).abs() < 1e-6 && n.ramp.to == 0.0 && (n.ramp.duration - 1.0).abs() < 1e-6);
        // Held while standing still: the purge, not the nitrous.
        assert_eq!(n.update(1, 0.5, 1.0), vec![NosEvent::StartPurge]);
        assert_eq!(n.update(0, 0.5, 1.0), vec![NosEvent::StopPurge]);
        // Held off the gas at speed (0x5ECDC0's |+0x20C| ≤ 0.001): the purge too.
        assert_eq!(n.update(1, 20.0, 0.0), vec![NosEvent::StartPurge]);
        assert_eq!(n.update(1, 20.0, 0.001), Vec::new());
        assert_eq!(n.update(1, 20.0, 0.0011), vec![NosEvent::StartNitrous, NosEvent::StopPurge]);
        assert!(!purging(0, 20.0, 0.0) && purging(2, 20.0, -0.0005) && !purging(2, 20.0, -0.5) && purging(2, 2.0, 1.0));
        assert_eq!(nitrous_bank(1), "Nitrous_00_MB.abk");
        assert_eq!(nitrous_bank(3), "Nitrous_01_MB.abk");
    }

    /// The AI racers' skids: one CARSFX_Skids per racer, whose one event
    /// takes the mean of both sides; at a drag's launch the original's
    /// three read p3 = 1024 while their driven wheels spun (car +0x3B4
    /// 9–13 m/s on the driven side, 1/120 s frames, two runs), 0 once the
    /// tyres gripped again; the objects go with the racers.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn ai_racers_skid_with_their_wheels() {
        use crate::audio::skids::{SkidInput, WheelIn};
        let mut sfx = PlayerSfx::new(Path::new("/nonexistent"));
        let spin = |slip: f32| {
            let mut i = SkidInput { speed: 1.0, forward: 1.0, local_dir: (1.0, 0.0), ..Default::default() };
            for (k, w) in i.wheels.iter_mut().enumerate() {
                let driven = k < 2;
                *w = WheelIn { on_ground: true, surface: 3, slip: if driven { slip } else { 0.0 }, lateral: 0.0, spin_slip: slip };
            }
            AiSkidInput { skid: i, sound_index: 1, mix: Default::default() }
        };
        for _ in 0..6 {
            sfx.set_ai_skids(vec![spin(11.4), spin(10.8), spin(12.1)]);
            sfx.ai_skid_update(false, 1.0 / 60.0);
        }
        assert_eq!(sfx.ai_skids.len(), 3);
        assert!(sfx.ai_skids.iter().all(|a| a.block[3] == 1024), "{:?}", sfx.ai_skids.iter().map(|a| a.block[3]).collect::<Vec<_>>());
        assert!(sfx.ai_skid_levels().iter().all(|&l| l == 0x7FFF));
        // Gripping again: the export is 0 and the mean runs out in five updates.
        for _ in 0..6 {
            sfx.set_ai_skids(vec![spin(0.0), spin(0.0), spin(0.0)]);
            sfx.ai_skid_update(false, 1.0 / 60.0);
        }
        assert!(sfx.ai_skids.iter().all(|a| a.block[3] == 0));
        sfx.set_ai_skids(Vec::new());
        sfx.ai_skid_update(false, 1.0 / 60.0);
        assert!(sfx.ai_skids.is_empty());
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn shift_volume_follows_rpm() {
        assert_eq!(shift_volume(8000.0, true, 0, 0x7FFF), (32767 * 10000 >> 15) * 0x7FFF >> 15);
        assert!(shift_volume(1000.0, true, 0, 0x7FFF) < shift_volume(6000.0, true, 0, 0x7FFF));
        assert_eq!(shift_record(3, 0), 0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn sweeteners_need_high_revs() {
        assert_eq!(swtn_block(0, 3, 6999.0, 0, 0x7FFF), None);
        let b = swtn_block(1, 3, 8000.0, 0, 0x7FFF).unwrap();
        assert_eq!(b, [1, 3, 8000, ((32767 * 15000) >> 15) * 0x7FFF >> 15, 0, 0, 0]);
        assert_eq!(swtn_path(41), "SOUND/ENGINE/SWTN_CAR_14_MB.abk");
    }

    struct Count(u32, Vec<u32>);
    impl VoiceHost for Count {
        fn start(&mut self, _: u32, _: &StartBlock) -> Option<u32> {
            self.0 += 1;
            self.1.push(self.0);
            Some(self.0)
        }
        fn stop(&mut self, v: u32) {
            self.1.retain(|&x| x != v);
        }
        fn alive(&mut self, v: u32) -> bool {
            self.1.contains(&v)
        }
        fn set(&mut self, _: u32, _: u8, _: i32) {}
        fn release(&mut self, _: u32) {}
    }

    /// Sound round 10: every `CAR_nn_ENG_MB_EE.abk`'s "CAR" event, run
    /// with a revving block, starts its voices with the send byte p14 >>
    /// 8 (the reverb's wet B, block +0x3C) and sets no send live.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn engine_abk_graphs_send_wet_b() {
        struct Sends(u32, Vec<u32>, u32);
        impl VoiceHost for Sends {
            fn start(&mut self, _: u32, b: &StartBlock) -> Option<u32> {
                self.0 += 1;
                self.1.push(self.0);
                // Every start: the send byte (+1 when it is not 32).
                self.2 += (b.send != 32) as u32;
                Some(self.0)
            }
            fn stop(&mut self, v: u32) {
                self.1.retain(|&x| x != v);
            }
            fn alive(&mut self, v: u32) -> bool {
                self.1.contains(&v)
            }
            fn set(&mut self, _: u32, kind: u8, _: i32) {
                self.2 += 1000 * (kind == 5) as u32;
            }
            fn release(&mut self, _: u32) {}
        }
        let mut s = CarFxSounds::new(&crate::testkit::game_root());
        let mut banks = 0;
        for id in 0..61 {
            let path = format!("SOUND/ENGINE/CAR_{id:02}_ENG_MB_EE.abk");
            let Some(mut ev) = s.events(&path) else { continue };
            let Some(m) = ev.module("CAR") else { continue };
            banks += 1;
            let mut first = [0i32; 26];
            first[0] = id;
            first[12] = 1;
            let e = ev.play(m, &first).unwrap();
            let mut host = Sends(0, Vec::new(), 0);
            for k in 0..600 {
                let mut b = first;
                b[1] = 1000 + (k * 13) % 8000;
                b[3] = if k % 200 < 120 { 0x400 } else { 0 };
                b[13] = 0;
                b[14] = 0x2000;
                b[21] = 24000;
                ev.set(e, &b);
                ev.tick(&mut host);
            }
            assert!(host.0 > 0, "{path}: no voice");
            assert_eq!(host.2, 0, "{path}: a start send other than 0x2000 >> 8, or a live send");
        }
        assert!(banks >= 20, "{banks} banks");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn sputter_pops_on_a_lift() {
        let mut s = CarFxSounds::new(&crate::testkit::game_root());
        let mut ev = s.events(&swtn_path(0)).expect("the SWTN bank");
        let m = ev.module("CAR_Sputter").unwrap();
        let mut pops = |block: [i32; 11]| {
            let id = ev.play(m, &block).unwrap();
            let mut host = Count(0, Vec::new());
            for _ in 0..150 {
                ev.tick(&mut host);
            }
            ev.kill(id, &mut host);
            host.0
        };
        // Off the throttle at 8000 rpm: pops; on it: none.
        assert!(pops([0, 1, 8000, 18500, 0, 0, 0, 0, 0, 0, 0]) > 3);
        assert_eq!(pops([0, 1, 8000, 18500, 0, 0, 0, 1024, 0, 0, 0]), 0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn turbo_spools_and_blows_off() {
        // Type 0 → sound 0x16 → [3, 3, 4, 5]; type 9 (sound 0x15) a truck.
        assert_eq!(turbo_record(0, 0), 3);
        assert_eq!(turbo_record(0, 3), 5);
        assert_eq!(turbo_path(turbo_record(0, 3)), "SOUND/TURBO/TURBO_SML2_2_MB.abk");
        assert_eq!(TURBO_BANKS[turbo_record(9, 2)].0, "TURBO_TRUCK_1_MB");
        let mut m = RunningMean3::default();
        assert_eq!(m.push(30.0), 30.0);
        assert_eq!(m.push(0.0), 15.0);
        m.push(90.0);
        assert_eq!(m.push(90.0), 60.0);
        let mut t = Turbo::default();
        let tick = 1.0 / 60.0;
        // Off boost: nothing.
        assert_eq!(t.update(10.0, 10.0, 5000.0, 0, false, true, tick), None);
        assert_eq!(t.state, 0);
        // Full throttle spools 1.0 a tick to the cap (40 above 1100 rpm).
        for _ in 0..30 {
            assert_eq!(t.update(100.0, 100.0, 5000.0, 0, false, true, tick), None);
        }
        assert_eq!(t.state, 1);
        assert!((t.spool - 30.0).abs() < 1e-3, "{}", t.spool);
        assert_eq!(t.whine, ((0.75f32 * 32767.0) as i32 * 6500) >> 15);
        for _ in 0..20 {
            t.update(100.0, 100.0, 5000.0, 0, false, true, tick);
        }
        assert_eq!(t.spool, 40.0);
        // At 1000 rpm the cap is 0.4 · 40.
        let mut low = Turbo::default();
        for _ in 0..60 {
            low.update(100.0, 100.0, 1000.0, 0, false, true, tick);
        }
        assert_eq!(low.spool, 16.0);
        // Lifting: a blow-off of variant 2 or 3 (full spool), the spool
        // to 0.
        match t.update(0.0, 0.0, 5000.0, 0, false, false, tick) {
            Some(TurboEvent::BlowOff { variant, volume }) => {
                assert!(variant == 2 || variant == 3);
                // The spool has already lost 0.5 this tick (0x472770 runs
                // before the state machine): 16500 × 39.5 / 40.
                assert_eq!(volume, (16500.0f32 * 0.9875) as i32);
            }
            e => panic!("{e:?}"),
        }
        assert_eq!((t.state, t.spool), (2, 0.0));
        // Back on the throttle: the blow-off ramps out over 150 ms, then
        // the turbo is idle again.
        for _ in 0..12 {
            t.update(0.0, 100.0, 5000.0, 0, false, false, tick);
        }
        assert!(!t.blowing);
        t.update(0.0, 100.0, 5000.0, 0, false, true, tick);
        assert_eq!(t.state, 0);
    }

    /// 0x472770 steps once per call, whatever the dt (traced in the
    /// binary): a 30 Hz caller spools half as fast in time as a
    /// 60 Hz one, as the original at 30 fps; dt 0 (the audio held) none.
    #[test]
    fn turbo_spool_steps_per_update() {
        let (mut a, mut b) = (Turbo::default(), Turbo::default());
        for _ in 0..10 {
            a.update(100.0, 100.0, 5000.0, 0, false, true, 1.0 / 60.0);
            b.update(100.0, 100.0, 5000.0, 0, false, true, 1.0 / 30.0);
        }
        assert_eq!(a.spool, b.spool);
        let before = a.spool;
        a.update(100.0, 100.0, 5000.0, 0, false, true, 0.0);
        assert_eq!(a.spool, before);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn banks_have_players() {
        let mut s = CarFxSounds::new(&crate::testkit::game_root());
        let p = s.players(&nitrous_path(0), "FX_NITROUS");
        assert!(!p.is_empty(), "no FX_NITROUS players in the nitrous bank");
        assert_eq!(p, vec![(2, false), (1, false), (0, true)]);
        assert!(s.sample(&nitrous_path(0), 0).is_some());
        assert_eq!(s.players(&nitrous_path(0), "FX_PURGE"), vec![(5, false), (3, true), (4, false)]);
        assert!(!s.players(&shift_path(0), "FX_SHIFTING_01").is_empty());
        // FX_TURBO_01's two sound players: the whine loops 0 and 4.
        assert_eq!(s.players(&turbo_path(0), "FX_TURBO_01"), vec![(0, true), (4, true)]);
        assert!(s.sample(&turbo_path(0), 3).is_some());
    }
}
