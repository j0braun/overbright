//! CARSFX_Hydraulics (registry 0x7ED468: name 0x788C40, factory 0x47DAE0,
//! ctor 0x4745B0, vtable 0x78AF88; car object 8, in the player's context
//! mask 0x5FFF, not the AI racers' 0x802D), traced in SPEED2.EXE
//! (verified unless marked). Bank `SOUND/NOS/Hydraulics_00_MB.abk`
//! (events FX_Hydraulic, slot 0x82C558, record 0x7EDDD0; FX_Hydr_Bounce,
//! slot 0x82C550, record 0x7EDDD8).
//!
//! * **Update** (slot 9, 0x47DD40, with the object on, +0xA0). For each
//!   corner: a voice (+0xB8 + 4i) whose instance ended (0x735070 count 1)
//!   while the corner's last state (+0xCC + 4i) was under 2 is freed
//!   (0x474660: +0xDC + 4i = the state); then the state is read again
//!   (0x5ED480: mover +0x74 + 4i) and the kick flag (car +0x408 + i) kept
//!   (+0xF8 + 4i). A bounce voice (+0xC8) whose instance ended is freed;
//!   with none, any kick starts FX_Hydr_Bounce, block {0, 0, 0x6F54, 0}
//!   ("SND: HydraulicsBounce"). Then the first corner whose state differs
//!   from its handled one (+0xDC + 4i) is handled, and no other this frame
//!   (the jump table 0x47DFC8): 0 or 1 frees its voice; 2 starts
//!   FX_Hydraulic with kind 1, 3 with kind 2 (0x47DC10, while sound is on,
//!   0x7FAAA8 = 1): its old voice freed, a block {0, 0, 0, kind, 0, 0}
//!   (node 0x457F20: p0 0..0x7FFF, p1 ±0x2000, p2 0..1, p3 1..2, p4
//!   0..0x7FFF, p5 0..0xFFFF), the state marked handled; then every rising
//!   corner, and every falling one without a voice, is marked handled too.
//! * **Levels** (slot 10, 0x4615E0, each frame after slot 9): each corner
//!   voice's p4 and the bounce voice's p2 become the object's level slot 0
//!   (vt+0x38, SFXObj_GetLevel: the mix map's class 8 slot 0), clamped to
//!   0..0x7FFF, and are sent (0x735580).
//! * In the bank's graphs (`aems`), FX_Hydraulic's p4 is its voice volume,
//!   p5 its azimuth (never set: 0), p1 its pitch; FX_Hydr_Bounce's p2 the
//!   volume, p3 the azimuth (0), p1 the pitch.
//!
// prov: inferred audio.hydraulics.module
//! **Inferred**: the object exists whenever the player drives (the
//! factory's pool test 0x43E910 ≥ 0x508 taken as met).

use crate::audio::carfx::{CarFxSounds, EventBank, Sink, close_bank, open_bank, run_bank};
use std::collections::HashMap;
use std::path::Path;

const BANK: &str = "SOUND/NOS/Hydraulics_00_MB.abk";

pub struct HydraulicsSfx {
    sounds: CarFxSounds,
    gains: HashMap<u32, (f32, f32, f32)>,
    bank: Option<EventBank>,
    /// +0xB8 + 4i with each voice's kind.
    voices: [Option<(u32, i32)>; 4],
    /// +0xCC + 4i, +0xDC + 4i.
    state: [u8; 4],
    handled: [u8; 4],
    /// +0xC8.
    bounce: Option<u32>,
    /// What started this frame (for the self-test): FX_Hydraulic, FX_Hydr_Bounce.
    pub started: (usize, usize),
}

impl HydraulicsSfx {
    pub fn new(root: &Path) -> HydraulicsSfx {
        HydraulicsSfx {
            sounds: CarFxSounds::new(root),
            gains: HashMap::new(),
            bank: None,
            voices: [None; 4],
            state: [0; 4],
            handled: [0; 4],
            bounce: None,
            started: (0, 0),
        }
    }

    pub fn silence(&mut self, sink: &mut dyn Sink) {
        close_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink);
        self.voices = [None; 4];
        self.bounce = None;
        self.state = [0; 4];
        self.handled = [0; 4];
    }

    /// One frame: the corners' states (mover +0x74 + 4i), this frame's
    /// kicks (car +0x408 + i) and the object's level (class 8, slot 0).
    pub fn update(&mut self, sink: &mut dyn Sink, states: [u8; 4], kicked: [bool; 4], level: i32, dt: f32) {
        self.started = (0, 0);
        open_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink, BANK);
        let Some(bank) = self.bank.as_mut() else { return };
        let ev = &mut bank.events;
        for i in 0..4 {
            if let Some((id, _)) = self.voices[i]
                && ev.get(id).is_none()
                && self.state[i] < 2
            {
                self.handled[i] = self.state[i];
                ev.release(id);
                self.voices[i] = None;
            }
            self.state[i] = states[i];
        }
        if self.bounce.is_some_and(|id| ev.get(id).is_none()) {
            self.bounce = None;
        }
        if self.bounce.is_none() && kicked.iter().any(|&k| k) {
            self.bounce = ev.module("FX_Hydr_Bounce").and_then(|m| ev.play(m, &[0, 0, 0x6F54, 0]));
            self.started.1 += self.bounce.is_some() as usize;
        }
        if let Some(i) = (0..4).find(|&i| self.state[i] != self.handled[i] && self.state[i] <= 3) {
            match self.state[i] {
                0 | 1 => {
                    self.handled[i] = self.state[i];
                    if let Some((id, _)) = self.voices[i].take() {
                        ev.release(id);
                    }
                }
                s => {
                    let kind = if s == 2 { 1 } else { 2 };
                    self.handled[i] = self.state[i];
                    if let Some((id, _)) = self.voices[i].take() {
                        ev.release(id);
                    }
                    self.voices[i] =
                        ev.module("FX_Hydraulic").and_then(|m| ev.play(m, &[0, 0, 0, kind, 0, 0])).map(|id| (id, kind));
                    self.started.0 += self.voices[i].is_some() as usize;
                    for j in 0..4 {
                        if self.state[j] == 2 || (self.state[j] == 3 && self.voices[j].is_none()) {
                            self.handled[j] = self.state[j];
                        }
                    }
                }
            }
        }
        // Slot 10 (0x4615E0).
        let level = level.clamp(0, 0x7FFF);
        for &(id, kind) in self.voices.iter().flatten() {
            ev.set(id, &[0, 0, 0, kind, level, 0]);
        }
        if let Some(id) = self.bounce {
            ev.set(id, &[0, 0, level, 0]);
        }
        run_bank(&mut self.bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Null;
    impl Sink for Null {
        fn one_shot(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) {}
        fn start_loop(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
            1
        }
        fn gain(&mut self, _: u32, _: f32) {}
        fn end(&mut self, _: u32) {}
        fn start_once(&mut self, _: &crate::audio::fe::Sample, _: f32, _: u16) -> u32 {
            1
        }
        fn sounding(&mut self, _: u32) -> bool {
            true
        }
        fn pitch(&mut self, _: u32, _: u16) {}
        fn pan(&mut self, _: u32, _: u16) {}
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn one_pump_for_a_rising_pair_and_one_bounce() {
        let root = crate::testkit::game_root();
        let mut s = HydraulicsSfx::new(&root);
        s.update(&mut Null, [0; 4], [false; 4], 0x4000, 0.016);
        s.update(&mut Null, [2, 2, 0, 0], [false; 4], 0x4000, 0.016);
        assert_eq!(s.started, (1, 0));
        s.update(&mut Null, [2, 2, 0, 0], [false; 4], 0x4000, 0.016);
        assert_eq!(s.started, (0, 0), "the second corner was marked handled");
        s.update(&mut Null, [1, 1, 0, 0], [true, true, false, false], 0x4000, 0.016);
        assert_eq!(s.started, (0, 1));
        s.update(&mut Null, [1, 1, 0, 0], [true, false, false, false], 0x4000, 0.016);
        assert_eq!(s.started.1, 0, "one bounce at a time");
    }
}
