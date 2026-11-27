use super::AiInput;
use super::doppler::Doppler;
use super::others::{ENV_COMMON, OtherCars};
use super::stereo::Stereo;
use crate::audio::carfx::{Sink, open_bank, run_bank};
use crate::audio::hybrid::ENGINES;
use glam::Vec3;

pub(super) struct Ai {
    pub(super) path: String,
    pub(super) event: Option<u32>,
    pub(super) offset: i32,
    pub(super) doppler: Doppler,
    /// CARSFX_Stereo (0x200F0) on the racer's context.
    pub(super) stereo: Stereo,
}

/// The AI engine's bank: the engine record's SPU bank (rec80 +4).
pub fn ai_bank(entry: usize) -> String {
    let id = match entry {
        41 => 14,
        42 => 12,
        e => e,
    };
    format!("SOUND/ENGINE/CAR_{id:02}_ENG_MB_SPU.abk")
}

impl OtherCars {
    pub(super) fn ai_update(&mut self, sink: &mut dyn Sink, ai: &[AiInput], player: Vec3, dt: f32) {
        if !ai.is_empty() || !self.ai.is_empty() {
            open_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink, ENV_COMMON);
        }
        // Objects come and go with the racers (0x473CE0 per creation).
        while self.ai.len() > ai.len() {
            let mut a = self.ai.pop().unwrap();
            if let (Some(id), Some(Some(b))) = (a.event, self.banks.get_mut(&a.path)) {
                b.events.release(id);
            }
            a.stereo.release(self.env_bank.as_mut().map(|b| &mut b.events));
        }
        while self.ai.len() < ai.len() {
            self.ai_counter = (self.ai_counter + 100) % 600;
            // The world's start rolls each racer's stereo (0x4765F0).
            let mut stereo = Stereo::default();
            stereo.racer_roll(&mut self.stereo_counter);
            self.racer_stereos += stereo.on as usize;
            self.ai.push(Ai {
                path: String::new(),
                event: None,
                offset: self.ai_counter - 300,
                doppler: Doppler::default(),
                stereo,
            });
        }
        for (a, inp) in self.ai.iter_mut().zip(ai) {
            let path = ai_bank(inp.entry);
            if a.path != path {
                if let (Some(id), Some(Some(b))) = (a.event.take(), self.banks.get_mut(&a.path)) {
                    b.events.release(id);
                }
                a.path = path.clone();
            }
            if !self.banks.contains_key(&path) {
                let mut slot = None;
                open_bank(&mut slot, &mut self.sounds, &mut self.gains, sink, &path);
                self.banks.insert(path.clone(), slot);
            }
            // CARSFX_Stereo: the mix map's AI class 15 slot 0 (section 2).
            if let Some(ev) = self.env_bank.as_mut().map(|b| &mut b.events) {
                self.stereos += a.stereo.frame(ev, inp.mix.level(15, 0), inp.mix.az(15, 0)) as usize;
            }
            let rec = &ENGINES[inp.entry.min(ENGINES.len() - 1)];
            let id = match inp.entry {
                41 => 14,
                42 => 12,
                e => e as i32,
            };
            let d = player.truncate().distance(inp.position.truncate()) as i32;
            a.doppler.update(d, dt);
            let Some(Some(b)) = self.banks.get_mut(&path) else { continue };
            let ev = &mut b.events;
            if a.event.is_none_or(|e| !ev.alive(e)) {
                let mut first = [0; 26];
                first[0] = id.clamp(0, 60);
                a.event = ev.module("CAR").and_then(|m| ev.play(m, &first));
            }
            if let Some(e) = a.event {
                let mut blk = [0; 26];
                blk[0] = id.clamp(0, 60);
                blk[1] = ((inp.ctx_rpm - 1000.0) * 0.000_111_111_11 * (rec.rpm_max - rec.rpm_min) + rec.rpm_min) as i32;
                blk[1] = blk[1].clamp(0, 10000);
                blk[3] = ((inp.throttle * 100.0 * 10.24) as i32).clamp(0, 0x400);
                let l = (((0x7FFF * inp.mix.level(0, 0)) >> 15) - 0x7FFF).clamp(-0x7FFF, 0);
                blk[4] = l;
                blk[5] = l;
                blk[15] = inp.mix.az(0, 0).clamp(0, 0x10000);
                blk[16] = ((a.doppler.ratio(2) * (a.offset + 16000) as f32 - 16000.0) as i32).clamp(-0x3FFF, 0x3FFF);
                blk[17] = 0;
                blk[21] = 24000;
                ev.set(e, &blk);
            }
        }
        for slot in self.banks.values_mut() {
            run_bank(slot, &mut self.sounds, &mut self.gains, sink, dt);
        }
    }
}
