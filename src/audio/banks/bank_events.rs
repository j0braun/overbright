use super::cache::CarFxSounds;
use super::sink::Sink;
use crate::audio::aems::{Events, StartBlock, VoiceHost};
use std::collections::HashMap;

/// The player context's +0xF0: set by message 9, which the player car
/// sends right after an exhaust flame pop (car effect 10 from the shift
/// pop 0x5EBE10 at 0x5EBF0E and the neutral rev pop at 0x5F6059; not the
/// nitrous flame); read by CAR_Sputter's p8.
pub static BACKFIRE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Message 9 from the player's car ([`BACKFIRE`]).
pub fn note_backfire() {
    BACKFIRE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The library's voices for one bank's graphs, played on a [`Sink`]:
/// the start block's volume byte / 127 × the second volume byte / 127 as
/// the gain, live volumes v / 32767 (0x72085B: +0x38 = v · 1/32767, the
/// channel gain × the second byte / 127 at 0x724F5D), the pitch v / 4096
/// (0x724D0E), the azimuth (kind 3, the start block's +8) through the
/// pan law; a sample with loop tags loops.
pub struct BankVoices<'a> {
    pub sink: &'a mut dyn Sink,
    pub sounds: &'a mut CarFxSounds,
    pub path: &'a str,
    /// Per voice: (volume, second volume).
    /// Per voice (volume, second volume, send byte / 127).
    pub gains: &'a mut HashMap<u32, (f32, f32, f32)>,
    /// Voices started this call.
    pub started: usize,
}

impl VoiceHost for BankVoices<'_> {
    fn start(&mut self, slot: u32, b: &StartBlock) -> Option<u32> {
        let (sample, looped) = self.sounds.slot_sample(self.path, slot as usize)?;
        let (v, v2) = (b.volume.min(0x7F) as f32 / 127.0, b.volume2.min(0x7F) as f32 / 127.0);
        let id = if looped {
            self.sink.start_loop(&sample, v * v2, b.pitch)
        } else {
            self.sink.start_once(&sample, v * v2, b.pitch)
        };
        self.sink.pan(id, b.azimuth);
        // The start block's send byte (+5, kind 5): the engine ABK graphs
        // put p14 (the reverb's wet B) there.
        let s = b.send.min(0x7F) as f32 / 127.0;
        if s != 0.0 {
            self.sink.send(id, v * s);
        }
        self.gains.insert(id, (v, v2, s));
        self.started += 1;
        Some(id)
    }
    fn stop(&mut self, voice: u32) {
        self.sink.end(voice);
        self.gains.remove(&voice);
    }
    fn alive(&mut self, voice: u32) -> bool {
        let alive = self.sink.sounding(voice);
        if !alive {
            self.gains.remove(&voice);
        }
        alive
    }
    fn set(&mut self, voice: u32, kind: u8, value: i32) {
        let g = self.gains.entry(voice).or_insert((1.0, 1.0, 0.0));
        match kind {
            0 => self.sink.pitch(voice, value.clamp(0, 0xFFFF) as u16),
            2 => {
                g.0 = value.clamp(0, 0x7FFF) as f32 / 32767.0;
                self.sink.gain(voice, g.0 * g.1);
                if g.2 != 0.0 {
                    self.sink.send(voice, g.0 * g.2);
                }
            }
            // 0x720936: clamp(v, 0, 0x7FFF) >> 8 as the send byte (bus 0).
            5 => {
                g.2 = (value.clamp(0, 0x7FFF) >> 8) as f32 / 127.0;
                self.sink.send(voice, g.0 * g.2);
            }
            8 => {
                g.1 = (value.clamp(0, 0x7FFF) >> 8) as f32 / 127.0;
                self.sink.gain(voice, g.0 * g.1);
            }
            3 => self.sink.pan(voice, value as u16),
            _ => {}
        }
    }
    fn release(&mut self, voice: u32) {
        if let Some(g) = self.gains.get_mut(&voice) {
            g.0 = 0.0;
            if g.2 != 0.0 {
                self.sink.send(voice, 0.0);
            }
        }
        self.sink.gain(voice, 0.0);
    }
}

/// One bank's running event graphs.
pub struct EventBank {
    pub path: String,
    pub events: Events,
}

/// Point `slot` at the bank `path` (its graphs made on first use; the
/// old bank's instances stopped).
pub(crate) fn open_bank(
    slot: &mut Option<EventBank>,
    sounds: &mut CarFxSounds,
    gains: &mut HashMap<u32, (f32, f32, f32)>,
    sink: &mut dyn Sink,
    path: &str,
) {
    if slot.as_ref().is_some_and(|b| b.path == path) {
        return;
    }
    close_bank(slot, sounds, gains, sink);
    *slot = sounds.events(path).map(|events| EventBank { path: path.to_string(), events });
}

pub(crate) fn close_bank(
    slot: &mut Option<EventBank>,
    sounds: &mut CarFxSounds,
    gains: &mut HashMap<u32, (f32, f32, f32)>,
    sink: &mut dyn Sink,
) {
    if let Some(mut old) = slot.take() {
        let mut host = BankVoices { sink, sounds, path: &old.path, gains, started: 0 };
        old.events.kill_all(&mut host);
    }
}

/// Run a bank's library ticks for `dt`; the voices started.
pub(crate) fn run_bank(
    slot: &mut Option<EventBank>,
    sounds: &mut CarFxSounds,
    gains: &mut HashMap<u32, (f32, f32, f32)>,
    sink: &mut dyn Sink,
    dt: f32,
) -> usize {
    let Some(b) = slot.as_mut() else { return 0 };
    let mut host = BankVoices { sink, sounds, path: &b.path, gains, started: 0 };
    b.events.advance(dt, &mut host);
    host.started
}
