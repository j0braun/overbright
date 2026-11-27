use super::EnvSfx;
use super::road::ROAD;
use crate::audio::carfx::{Sink, open_bank};

/// The drum's per-frame pitch for its three requests (0x48209F..0x4820BE).
pub const SPLASH_PITCH: [i32; 3] = [2000, 4000, 7000];

/// 0x7ECF90: the drum's level scale (17000 / 32768).
pub const SPLASH_SCALE: i32 = 17000;

impl EnvSfx {
    /// A water drum's splash (the prop object's start 0x46AE00 and its
    /// per-frame 0x481FD0): three FX_ROADNOISE_TRANS requests {3, 0, 0, 0,
    /// 1} (ROADNOISE_00_MB plays variant 3 as sample 14, 1.99 s), then each
    /// frame p1 = clamp(+0xC0 · 17000 >> 15, 0, 0x7FFF), p2 = 2000 / 4000 /
    /// 7000 by the request (clamped to 0x1FFF), p3 = the object's azimuth
    /// +0xC8; a request whose instance has ended is released and the rest
    /// wait for the next frame. `start` is the object's +0xC0, the start
    /// volume its impact's layers take (the copy's kind slot,
    /// `collision::splash_level`, **verified**). Held at its start
    // prov: inferred audio.roadwind.splash
    /// value (**inferred**: natively +0xC0 follows the copy's level each
    /// frame while the object lives, as for the impacts' layers); the ramp
    // prov: open audio.roadwind.splash-2
    /// by +0x11C when +0x128 has bit 0x10 (0x4599E0 on 0x82BCD0) is not
    /// applied (**open**: that flag's writer), the azimuth is 0 (the mixer
    /// is mono).
    ///
    /// Returns the splash's id for [`EnvSfx::splash_start`].
    pub fn splash(&mut self, sink: &mut dyn Sink, start: i32) -> u64 {
        open_bank(&mut self.road_bank, &mut self.sounds, &mut self.gains, sink, ROAD);
        self.splash_ids += 1;
        let id = self.splash_ids;
        let Some(b) = self.road_bank.as_mut() else { return id };
        let ev = &mut b.events;
        let Some(m) = ev.module("FX_ROADNOISE_TRANS") else { return id };
        let ids = [0; 3].map(|_| ev.play(m, &[3, 0, 0, 0, 1, 0, 0]));
        let level = crate::audio::collision::splash_level(start);
        self.splashed += ids.iter().any(|i| i.is_some()) as usize;
        self.splashes.push((id, ids, level));
        id
    }

    /// The object's start volume this frame (0x481FD0 reads +0xC0 each
    /// frame, which its mix copy rewrites): the splash's p1 follows it.
    /// Whether the splash still runs.
    pub fn splash_start(&mut self, id: u64, start: i32) -> bool {
        match self.splashes.iter_mut().find(|s| s.0 == id) {
            Some(s) => {
                s.2 = crate::audio::collision::splash_level(start);
                true
            }
            None => false,
        }
    }

    /// Whether a splash still runs.
    pub fn splash_alive(&self, id: u64) -> bool {
        self.splashes.iter().any(|s| s.0 == id)
    }

    /// The splash requests still running.
    pub fn splashing(&self) -> usize {
        self.splashes.iter().map(|(_, ids, _)| ids.iter().flatten().count()).sum()
    }

    /// 0x481FD0's loop over the requests.
    pub(super) fn splash_frame(&mut self) {
        let Some(b) = self.road_bank.as_mut() else { return };
        let ev = &mut b.events;
        for (_, ids, level) in &mut self.splashes {
            for (k, slot) in ids.iter_mut().enumerate() {
                let Some(e) = *slot else { continue };
                if !ev.alive(e) {
                    ev.release(e);
                    *slot = None;
                    break;
                }
                ev.set(e, &[3, *level, SPLASH_PITCH[k].clamp(0, 0x1FFF), 0, 1]);
            }
        }
        self.splashes.retain(|(_, ids, _)| ids.iter().any(|i| i.is_some()));
    }
}
