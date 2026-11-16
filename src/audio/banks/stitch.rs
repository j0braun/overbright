//! Stitch sounds that follow their owner: the sample-layer records of
//! `GLOBAL/InGameCommon.bun` (see [`super::collision`]) started by a
//! sound object and updated every frame with the object's level and
//! azimuth (the road-surface thumps, the bottom-out knocks, the traffic
//! whooshes).
//!
//! * An object (0x45B1F0, 0x5C bytes, vtable 0x789EC0) holds one record
//!   (list 0 = collision, list 1 = whoosh; 0x46CD60 / 0x43BE60 on the
//!   manager's +0xD0 lists) and a parameter block {+0 unused, +4 volume,
//!   +8 pitch offset p, +0xC azimuth, +0x10}. Its start (0x46CDD0) and
//!   every update (0x47BB30) copy the block in and scale the volume by
//!   the record's gain (0x7F34A0 for list 0, 0x7F36E8 for list 1, by the
//!   record's +6) >> 15; byte +0x1C says a layer is still sounding.
//! * Each layer (0x45B260 / 0x45B280) starts an event of its bank kind
//!   (byte +1: 0 "AEMS_StichCollision" of `MB_Stich_Collision.abk`, 1
//!   "AEMS_StichWoosh" of `MB_Stitch_Whoosh.abk`) with sound byte +0,
//!   volume (+2 − rand(+0xA)) × the block's volume >> 15, pitch (+4 −
//!   rand(|+0xC|)) × (0x1000 − p) / 0x1000, azimuth +8 + the block's,
//!   and +6 as the delay. Each update (0x46CF70) resends volume = the
//!   layer volume × the block's >> 15 (0..0x7FFF), the azimuth (layer
//!   +8 + the block's, 0..0x10000) and the start pitch (0..0x2000); a
//!   layer whose event ended is dropped (0x46CEF0).
//!
// prov: inferred audio.stitch.module
//! **Inferred** (as in `collision`): the sound byte as the bank's sound
//! list entry, the delay in milliseconds, the record gain +4 applied.

use super::cache::StitchBanks;
use crate::audio::carfx::Sink;
use crate::audio::collision::{GAIN, Stitch, rand};
use crate::audio::fe::Sample;
use std::path::Path;

/// The per-record gain of list 1 (0x7F36E8: 60 × 26028).
pub const GAIN_WHOOSH: u16 = 26028;

struct Layer {
    sample: Option<Sample>,
    volume: i32,
    pitch: u16,
    azimuth: u16,
    delay: f32,
    voice: Option<u32>,
    done: bool,
}

/// One running stitch object.
pub struct StitchVoice {
    gain: i32,
    layers: Vec<Layer>,
}

impl StitchVoice {
    /// 0x45B1F0 + 0x46CDD0: record `index` of `list` (0 collision, 1
    /// whoosh) with the start block's pitch offset 0.
    pub fn start(banks: &mut StitchBanks, list: usize, index: usize) -> Option<StitchVoice> {
        let st: Stitch = banks.stitches.get(list)?.get(index)?.clone();
        let table = if list == 0 { *GAIN.get(st.index as usize).unwrap_or(&0x7FFF) } else { GAIN_WHOOSH } as i32;
        let gain = (table * st.gain as i32) >> 15;
        let layers = st
            .layers
            .iter()
            .map(|l| Layer {
                sample: banks.sample(l.bank as usize, l.sound as usize),
                volume: l.volume as i32 - rand(l.volume_rand as u32) as i32,
                pitch: (l.pitch as i32 - rand(l.pitch_rand.unsigned_abs() as u32) as i32).clamp(0, 0x2000) as u16,
                azimuth: l.azimuth,
                delay: l.delay.min(4000) as f32 / 1000.0,
                voice: None,
                done: false,
            })
            .collect();
        Some(StitchVoice { gain, layers })
    }

    /// 0x47BB30 → 0x46CF70 with the owner's level (0..0x7FFF) and
    /// azimuth: starts the layers whose delay has run out, resends the
    /// others; false once every layer has ended (byte +0x1C clear).
    pub fn update(&mut self, sink: &mut dyn Sink, level: i32, azimuth: i32, dt: f32) -> bool {
        let v = (level.clamp(0, 0x7FFF) * self.gain) >> 15;
        for l in self.layers.iter_mut().filter(|l| !l.done) {
            let gain = ((l.volume * v) >> 15).clamp(0, 0x7FFF) as f32 / 32767.0;
            let az = ((l.azimuth as i32 + azimuth) & 0xFFFF) as u16;
            match l.voice {
                None => {
                    l.delay -= dt;
                    if l.delay <= 0.0 {
                        match &l.sample {
                            Some(s) => {
                                let id = sink.start_once(s, gain, l.pitch);
                                sink.pan(id, az);
                                l.voice = Some(id);
                            }
                            None => l.done = true,
                        }
                    }
                }
                Some(id) => {
                    if sink.sounding(id) {
                        sink.gain(id, gain);
                        sink.pan(id, az);
                    } else {
                        l.done = true;
                    }
                }
            }
        }
        self.layers.iter().any(|l| !l.done)
    }

    /// The object's release (vtable +0): its layers stop.
    pub fn stop(&mut self, sink: &mut dyn Sink) {
        for l in &mut self.layers {
            if let Some(id) = l.voice.take() {
                sink.end(id);
            }
            l.done = true;
        }
    }
}

/// The FE_COMMON player's two stitch slots (+0x28 / +0x2C), which the
/// in-game car-part sounds use (0x481500(n), from CarPart_RequestAnim
/// 0x61F830 outside the front end): record `n` of the collision list
/// (0x43BE60 on [0x82B884]+0xD0's list +4) goes to the first free slot —
/// none when both are taken — started with a zeroed block. The player's
/// update (vtable 0x789D28 +0x14, 0x4815B0) runs its loop twice over slot
/// +0x28 only: the slot's level is SFXObj_FEHUD's mix-map slot 4 (its +0x24
/// vtable +0x38(4)), azimuth and pitch 0, and an ended stitch is released.
/// Slot +0x2C is never updated or released, so a second sound started
/// while the first plays stays at volume 0 and holds that slot for good.
pub struct CommonStitches {
    banks: Option<StitchBanks>,
    slots: [Option<StitchVoice>; 2],
}

impl CommonStitches {
    pub fn new(root: &Path) -> CommonStitches {
        CommonStitches { banks: StitchBanks::load(root), slots: [None, None] }
    }

    /// 0x481500(n).
    pub fn start(&mut self, n: usize) {
        let Some(slot) = self.slots.iter().position(|s| s.is_none()) else { return };
        let Some(banks) = self.banks.as_mut() else { return };
        self.slots[slot] = StitchVoice::start(banks, 0, n);
    }

    /// 0x4815B0 with SFXObj_FEHUD's slot 4 level.
    pub fn update(&mut self, sink: &mut dyn Sink, level: i32, dt: f32) {
        if let Some(v) = self.slots[0].as_mut()
            && !v.update(sink, level, 0, dt)
        {
            self.slots[0] = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn whoosh_list_uses_the_whoosh_bank() {
        let mut b = StitchBanks::load(&crate::testkit::game_root()).expect("the stitch banks");
        assert_eq!(b.stitches[1].len(), 60);
        assert!(b.stitches[1].iter().all(|s| s.layers.iter().all(|l| l.bank == 1)));
        for i in 9..27 {
            assert!(
                StitchVoice::start(&mut b, 1, i).is_some_and(|v| v.layers.iter().all(|l| l.sample.is_some())),
                "{i}"
            );
        }
        for i in 0x4E..0x5A {
            assert!(StitchVoice::start(&mut b, 0, i).is_some_and(|v| !v.layers.is_empty()), "{i}");
        }
    }

    /// 0x61F830's in-game records (0x481500(n), n 0..5) are the collision
    /// list's first six, with samples; a second start while the first plays
    /// takes slot +0x2C, a third finds none.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn car_part_records_and_common_slots() {
        let root = crate::testkit::game_root();
        let mut b = StitchBanks::load(&root).expect("the stitch banks");
        for n in 0..6 {
            assert!(
                StitchVoice::start(&mut b, 0, n)
                    .is_some_and(|v| !v.layers.is_empty() && v.layers.iter().all(|l| l.sample.is_some())),
                "record {n}"
            );
        }
        let mut c = CommonStitches::new(&root);
        c.start(3);
        c.start(2);
        assert!(c.slots[0].is_some() && c.slots[1].is_some());
        c.start(5);
        assert!(c.slots.iter().all(|s| s.is_some()));
    }
}
