use super::{EnvInput, EnvSfx};
use crate::audio::carfx::{Sink, open_bank, run_bank};

/// CARSFX_Rain (id 0x10090: section 1 class 9; factory 0x47DFE0, ctor
/// 0x4746D0, vtable 0x78AFD0; verified, sound5). Update (vtable +0x24,
/// 0x474770(dt)), with a weather object (0x48B1E0(1)) and the flow state
/// not 1: +0xB0 = the rain (weather +0x64 → +0x1F0); above 0.001
/// (0x78435C) with no event (+0xD0) FX_Weather starts (0x461750: bank
/// `FX_Rain_MB`, slot 0x82C508, the block +4..+0x18 all 0, +0xCC = 0);
/// at or below it a playing event that is not stopping stops (0x461840:
/// +0xC8 = 10 s, +0xCC = 1, block +0x18 = 1 dispatched). The thunder
/// timer: +0xB8 += dt; when the rain is above 0.25 (0x78B020), 0x8A329C
/// and 0x88EC68 are 0 and +0xB8 > +0xBC: with +0xC4 = 0, +0xB8 = 0 and,
/// when no NIS stream is busy (0x46C9A0), the thunder (vtable +0x44
/// 0x4852A0) plays, +0xBC = Rand_Int(20) + 25 (0x43C1C0, 0x78B018) and
/// +0xC4 = 1; with +0xC4 ≠ 0 it counts up and wraps to 0 past 32. A
/// stopping event's +0xC8 runs down by dt and at 0 the event is released
/// (vtable +0x20, 0x474720). The parameters (vtable +0x28, 0x4618C0,
/// while +0xD0 is set): +0xC = slot 0 (slot 1 and +4 = 1 when the sound
// prov: open audio.roadwind.stop
/// context +0x9C has +0xAC set and +0xA8 = 2 or 4; **open**, not
/// modelled), +8 = 0, +0x10 = 7000, +0x14 = 0. Mix field 2 (+0xC) = 1
/// while the event plays and is not stopping (vtable +0x34, 0x461890).
/// The thunder (0x4852A0): unless one is playing (+0xC0, cleared by the
/// stream's callback 0x461880), the flow state is 2, 3 or 7, or
/// g_RaceType is 8: NisAudio_Request(4, "THUNDERSTRM", 1 below rain 0.5
/// (0x7864E0) else 2, 0x461880) when the stream object is ready → 0x484510
/// (unless GB_NOS plays, 0x82BB29): SoundFX_Select = (4, variant)
/// (0x45A770), stream kind 4.
#[derive(Default)]
pub struct Rain {
    pub(super) event: Option<u32>,
    pub(super) stopping: bool,
    /// +0xC8, +0xB8, +0xBC, +0xC4.
    pub(super) fade: f32,
    pub(super) timer: f32,
    pub(super) next: f32,
    pub(super) count: u32,
    /// +0xC0: a thunder line plays (cleared by [`Rain::thunder_done`]).
    pub thunder_playing: bool,
    pub(super) rng: crate::opponents::GameRng,
    /// The FX_Weather events started (checks).
    pub started: usize,
}

impl Rain {
    /// Mix field 2 (0x461890).
    pub fn field(&self) -> i32 {
        (self.event.is_some() && !self.stopping) as i32
    }

    /// The THUNDERSTRM line ended (0x461880).
    pub fn thunder_done(&mut self) {
        self.thunder_playing = false;
    }
}

const RAIN: &str = "SOUND/IG_GLOBAL/FX_Rain_MB.abk";

impl EnvSfx {
    /// CARSFX_Rain's update and parameters for one frame ([`Rain`]); the
    /// THUNDERSTRM variant to request (1 or 2) when the thunder fires.
    pub fn rain_update(&mut self, sink: &mut dyn Sink, i: &EnvInput, dt: f32) -> Option<u8> {
        open_bank(&mut self.rain_bank, &mut self.sounds, &mut self.gains, sink, RAIN);
        let r = &mut self.rain;
        r.started = 0;
        let mut thunder = None;
        if let Some(b) = self.rain_bank.as_mut() {
            let ev = &mut b.events;
            if i.rain > 0.001 {
                if r.event.is_none() {
                    r.event = ev.module("FX_Weather").and_then(|m| ev.play(m, &[0; 6]));
                    r.started += r.event.is_some() as usize;
                    r.stopping = false;
                    if crate::env_set!("NFSU2_AUDIO_DEBUG") {
                        eprintln!("audio: rain FX_Weather starts (rain {:.3}, level {})", i.rain, i.mix.level(9, 0));
                    }
                }
            } else if let Some(id) = r.event
                && !r.stopping
            {
                r.fade = 10.0;
                r.stopping = true;
                ev.set(id, &[0, 0, i.mix.level(9, 0).clamp(0, 0x7FFF), 7000, 0, 1]);
            }
            r.timer += dt;
            // prov: inferred audio.roadwind.rain_update
            // 0x8A329C / 0x88EC68 taken as 0 (**inferred**: their writers
            // are not traced).
            if i.rain > 0.25 && r.timer > r.next {
                if r.count == 0 {
                    r.timer = 0.0;
                    if !i.stream_busy {
                        // +0xC0 is cleared just before 0x4852A0 checks it,
                        // so the thunder always goes (its flow-state and
                        // race-type gates are the caller's).
                        thunder = Some(if i.rain < 0.5 { 1 } else { 2 });
                        r.thunder_playing = true;
                        r.next = r.rng.below(20) as f32 + 25.0;
                        r.count += 1;
                    }
                } else {
                    r.count += 1;
                    if r.count > 0x20 {
                        r.count = 0;
                    }
                }
            }
            if r.stopping {
                r.fade -= dt;
                if r.fade <= 0.0 {
                    if let Some(id) = r.event.take() {
                        ev.release(id);
                    }
                    r.stopping = false;
                }
            }
            if let Some(id) = r.event {
                ev.set(id, &[0, 0, i.mix.level(9, 0).clamp(0, 0x7FFF), 7000, 0, r.stopping as i32]);
            }
        }
        run_bank(&mut self.rain_bank, &mut self.sounds, &mut self.gains, sink, dt);
        thunder
    }
}
