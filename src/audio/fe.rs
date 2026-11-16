//! The front end's sounds: a menu event on a kind of control picks a
//! sound id (the table at 0x837FE0, filled by 0x50CF40, played by
//! 0x50D150), and PlayFESound (0x458520) plays the id from one of two
//! banks: ids under 23 from GLOBAL/FE_COMMON_MB.abk's PlayCommonSample
//! (0x469760), ids 23–93 from FE/FE_MB.abk's PlayFrontEndSample (0x469520)
//! as id − 23. Both pass the index clamped to 0–0x4B, the master FE
//! volume, pitch 0x1000 and pan 0.

use super::abk::Bank;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The menu events (enum names at 0x7F9438).
pub mod event {
    pub const SUMMON_DIALOG: u8 = 1;
    pub const SUMMON_HELP: u8 = 2;
    pub const SUMMON_CUSTOMIZE: u8 = 3;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 5;
    pub const UP: u8 = 6;
    pub const DOWN: u8 = 7;
    pub const SELECT: u8 = 8;
    pub const START: u8 = 9;
    pub const BACK: u8 = 10;
    pub const FOCUS: u8 = 11;
    pub const UNFOCUS: u8 = 12;
    pub const INSTALL: u8 = 13;
    pub const ROTATION_SMALL: u8 = 14;
    pub const ROTATION_MED: u8 = 15;
    pub const ROTATION_LARGE: u8 = 16;
    pub const LTRIGGER: u8 = 17;
    pub const RTRIGGER: u8 = 18;
    pub const WRONG: u8 = 19;
}

/// The kinds of control (enum names at 0x7F9500).
pub mod control {
    pub const GENERIC: u8 = 0;
    pub const SLIDER: u8 = 1;
    pub const TOGGLE: u8 = 2;
    pub const INPUT: u8 = 3;
    pub const BUTTON: u8 = 4;
    pub const BROWSER: u8 = 8;
    pub const ICONSCROLLER: u8 = 9;
    pub const PAINT: u8 = 10;
    pub const PAINT_TYPE: u8 = 11;
    pub const CARLOT: u8 = 12;
    pub const ENGAGE_PHONE: u8 = 13;
    pub const ENGAGE_DPAD: u8 = 14;
    pub const ENGAGE_SMS: u8 = 15;
    pub const SMS_POPUP: u8 = 16;
    pub const EXHAUST: u8 = 17;
    pub const TURBO: u8 = 18;
    pub const NOS: u8 = 19;
    pub const TIRES: u8 = 20;
    pub const TRANSMISSION: u8 = 21;
    pub const CARSEL: u8 = 23;
    pub const ROTATION_1: u8 = 24;
    pub const ROTATION_3: u8 = 26;
}

/// The sound id for an event on a control (0x50CF40's table; every other
/// cell is −1, no sound).
pub fn sound(ev: u8, ctl: u8) -> Option<u8> {
    use super::fe::control as c;
    use super::fe::event as e;
    Some(match (ev, ctl) {
        (e::SUMMON_DIALOG | e::SUMMON_HELP, _) => 10,
        (e::LEFT, c::SLIDER) => 8,
        (e::LEFT, c::PAINT_TYPE) => 41,
        (e::LEFT, c::CARSEL) => 70,
        (e::LEFT | e::LTRIGGER, _) => 2,
        (e::RIGHT, c::SLIDER) => 9,
        (e::RIGHT, c::PAINT_TYPE) => 42,
        (e::RIGHT, c::CARSEL) => 69,
        (e::RIGHT | e::RTRIGGER, _) => 3,
        (e::UP, c::PAINT) => 39,
        (e::UP, _) => 0,
        (e::DOWN, c::PAINT) => 40,
        (e::DOWN, _) => 1,
        (e::SELECT, _) => 4,
        (e::START, _) => 6,
        (e::BACK, _) => 5,
        (e::FOCUS, c::ENGAGE_PHONE..=c::SMS_POPUP) => [14, 15, 16, 13][(ctl - c::ENGAGE_PHONE) as usize],
        (e::INSTALL, c::PAINT) => 38,
        (e::INSTALL, c::EXHAUST) => 49,
        (e::INSTALL, c::TURBO) => 51,
        (e::INSTALL, c::NOS) => 52,
        (e::INSTALL, c::TIRES) => 54,
        (e::INSTALL, c::TRANSMISSION) => 53,
        (e::INSTALL, _) => 50,
        (e::ROTATION_SMALL..=e::ROTATION_LARGE, c::ROTATION_1..=c::ROTATION_3) => 63 + ctl - c::ROTATION_1,
        (e::WRONG, _) => 7,
        _ => return None,
    })
}

/// A decoded sample: its rate and 16-bit-scale samples.
pub type Sample = Arc<(u32, Vec<f32>)>;

/// The two FE banks and the samples decoded from them so far.
pub struct FeSounds {
    common: Bank,
    fe: Bank,
    cache: HashMap<u8, Option<Sample>>,
}

impl FeSounds {
    pub fn load(root: &Path) -> Option<FeSounds> {
        let read = |p: &str| Bank::parse(crate::game_fs::read(root, p).ok()?);
        Some(FeSounds { common: read("SOUND/GLOBAL/FE_COMMON_MB.abk")?, fe: read("SOUND/FE/FE_MB.abk")?, cache: HashMap::new() })
    }

    /// The sample sound `id` plays: the event module's sound list entry
    /// (node 27's list) names the bank sample. The banks name the events
    /// "PlayCommonSample", "PlayFrontEndSample" and "PlayFrontEndSample_loop".
    pub fn sample(&mut self, id: u8) -> Option<Sample> {
        if let Some(s) = self.cache.get(&id) {
            return s.clone();
        }
        let (bank, event, index) = match id {
            0..23 => (&self.common, "PlayCommonSample", id),
            23..=93 => (&self.fe, if id - 23 == 50 { "PlayFrontEndSample_loop" } else { "PlayFrontEndSample" }, id - 23),
            _ => return None,
        };
        let index = index.min(0x4B) as usize;
        let s = bank.module(event).and_then(|m| bank.sound_list(m).get(index).copied()).and_then(|(_, slot)| bank.decode(slot as usize)).map(|(info, pcm)| Arc::new((info.rate, pcm)));
        self.cache.insert(id, s.clone());
        s
    }
}

/// The FE sound banks' AEMS events, as SFXObj_FEHUD's two players run
/// them (verified, sound5): ids under 23 start FE_COMMON_MB's
/// PlayCommonSample (0x469760, vtable 0x789D28 +0xC), ids 23–93 FE_MB's
/// PlayFrontEndSample (0x469520, vtable 0x78A744 +0xC) with the block
/// {index clamped 0..0x4B, level, 0x1000, 0}, the level SFXObj_FEHUD's
/// mix-map slot (0x45E390: section 0 class 6) 0 for FE_COMMON and 1 for
/// FE_MB, read as the sound starts; the wrapper is freed at once
/// (0x45FA60(1) → 0x735550 releases the handle) and the graph plays out.
/// The graphs layer the sample (PlayCommonSample: two detuned side voices
/// at a quarter of the level and the centre one). FE_COMMON plays nothing
/// while 0x82B924 is set (not ported: its writer is not traced).
///
/// Id 0x49 (FE_MB 50, the FE car turn) is the one loop: PlayFrontEndSample_loop
/// started once (its handle at +0x10, +0xEC = −1); StopFESound (0x469490)
/// only marks it (+0xEC = 0, +0xE8 the time), and the player's update
/// (0x4793D0, vtable +0x14) stops it once 100 ms (0x784258) have passed
/// since. While it plays, each update sets its level to slot 6 and its
/// second parameter to 3000 + 500 · clamp(m, 0, 1) (the ramp 0x7EFFFC,
/// clamped 0..0x1FFF), m the mean of the last five |turn rate| values
/// (0x82C6E8, 5 entries, 0x576980 / 0x576920) the FE garage stores in
/// 0x7EC858 (|car +0x248|, 0x4D8C31).
pub struct FeEvents {
    sounds: super::carfx::CarFxSounds,
    gains: HashMap<u32, (f32, f32, f32)>,
    common: Option<super::carfx::EventBank>,
    fe: Option<super::carfx::EventBank>,
    /// The turn loop: its event, the stop mark's age (s), the averager.
    turn: Option<u32>,
    turn_stop: Option<f32>,
    turn_rates: [f32; 5],
    turn_count: usize,
    turn_next: usize,
    /// Events started since the last [`FeEvents::take_started`] (checks).
    started: usize,
    /// The shop's Test N2O Purge (0x458B30, "SND: FEPurge"): FX_PURGE in
    /// a nitrous bank, the FE_MB player's +4 event.
    purge_bank: Option<super::carfx::EventBank>,
    purge: Option<u32>,
    purge_stop: bool,
}

const FE_COMMON: &str = "SOUND/GLOBAL/FE_COMMON_MB.abk";
const FE_MB: &str = "SOUND/FE/FE_MB.abk";

impl FeEvents {
    pub fn new(root: &Path) -> FeEvents {
        FeEvents { sounds: super::carfx::CarFxSounds::new(root), gains: HashMap::new(), common: None, fe: None, turn: None, turn_stop: None, turn_rates: [0.0; 5], turn_count: 0, turn_next: 0, started: 0, purge_bank: None, purge: None, purge_stop: false }
    }

    fn open(&mut self, sink: &mut dyn super::carfx::Sink) {
        super::carfx::open_bank(&mut self.common, &mut self.sounds, &mut self.gains, sink, FE_COMMON);
        super::carfx::open_bank(&mut self.fe, &mut self.sounds, &mut self.gains, sink, FE_MB);
    }

    /// PlayFESound (0x458520) for a one-shot id; `levels` SFXObj_FEHUD's
    /// slots (0x7FFF each without a mix map).
    pub fn play(&mut self, sink: &mut dyn super::carfx::Sink, id: u8, levels: &[i32; 8]) {
        if id == 0x49 {
            return self.turn_start(sink);
        }
        self.open(sink);
        let (bank, event, index, level) = match id {
            0..23 => (&mut self.common, "PlayCommonSample", id, levels[0]),
            23..=93 => (&mut self.fe, "PlayFrontEndSample", id - 23, levels[1]),
            _ => return,
        };
        let Some(b) = bank.as_mut() else { return };
        let Some(m) = b.events.module(event) else { return };
        if let Some(h) = b.events.play(m, &[index.min(0x4B) as i32, level.clamp(0, 0x7FFF), 0x1000, 0]) {
            b.events.release(h);
            self.started += 1;
            if crate::env_set!("NFSU2_AUDIO_DEBUG") {
                eprintln!("audio: FE sound {id} ({event} {index}) level {level}");
            }
        }
    }

    /// PlayFESound 0x49: the loop starts once (0x469595..0x469632) with
    /// the block {50, 0, 0, 0} (its level and second parameter come with
    /// the next update); a pending stop is cancelled (+0xEC = −1 again).
    fn turn_start(&mut self, sink: &mut dyn super::carfx::Sink) {
        self.turn_stop = None;
        if self.turn.is_some() {
            return;
        }
        self.open(sink);
        let Some(b) = self.fe.as_mut() else { return };
        let Some(m) = b.events.module("PlayFrontEndSample_loop") else { return };
        // 0x576A20(0) on the averager as the loop starts.
        self.turn_rates = [0.0; 5];
        self.turn_count = 0;
        self.turn_next = 0;
        self.turn = b.events.play(m, &[50, 0, 0, 0]);
        self.started += self.turn.is_some() as usize;
    }

    /// 0x458B30: a running purge event is let go first, then FX_PURGE
    /// starts with the block {0, 0, 0, 0}; the update (0x4793D0, +4) fills
    // prov: inferred audio.fe.purge_start
    /// it. `bank` the nitrous bank it is found in (**inferred**: 0x458B30
    /// loads none; the FE car's nitrous bank by the in-game rule).
    pub fn purge_start(&mut self, sink: &mut dyn super::carfx::Sink, bank: &str) {
        if let (Some(h), Some(b)) = (self.purge.take(), self.purge_bank.as_mut()) {
            b.events.release(h);
        }
        super::carfx::open_bank(&mut self.purge_bank, &mut self.sounds, &mut self.gains, sink, bank);
        let Some(b) = self.purge_bank.as_mut() else { return };
        let Some(m) = b.events.module("FX_PURGE") else { return };
        self.purge = b.events.play(m, &[0, 0, 0, 0]);
        self.purge_stop = false;
        self.started += self.purge.is_some() as usize;
    }

    /// 0x458C40: the event's stop flag (+0xC) set.
    pub fn purge_end(&mut self) {
        if self.purge.is_some() {
            self.purge_stop = true;
        }
    }

    pub fn purge_playing(&self) -> bool {
        self.purge.is_some() && !self.purge_stop
    }

    /// StopFESound 0x49 (0x469490): the stop is marked, the update ends it.
    pub fn turn_stop(&mut self) {
        if self.turn.is_some() && self.turn_stop.is_none() {
            self.turn_stop = Some(0.0);
        }
    }

    pub fn turn_playing(&self) -> bool {
        self.turn.is_some()
    }

    /// One frame (the FE_MB player's update 0x4793D0, then the library
    /// ticks): `turn_rate` the garage's |turn rate| (0x7EC858).
    pub fn update(&mut self, sink: &mut dyn super::carfx::Sink, levels: &[i32; 8], turn_rate: f32, dt: f32) {
        if let (Some(h), Some(b)) = (self.turn, self.fe.as_mut()) {
            if let Some(t) = self.turn_stop.as_mut() {
                *t += dt;
                // prov: inferred audio.fe.update
                // 0x43E5C0's elapsed time against 100 (**inferred** ms).
                if *t >= 0.1 {
                    b.events.release(h);
                    self.turn = None;
                    self.turn_stop = None;
                }
            } else {
                self.turn_rates[self.turn_next] = turn_rate;
                self.turn_next = (self.turn_next + 1) % 5;
                self.turn_count = (self.turn_count + 1).min(5);
                let mean = self.turn_rates.iter().sum::<f32>() / self.turn_count.max(1) as f32;
                let p2 = ((3000.0 + 500.0 * mean.clamp(0.0, 1.0)) as i32).clamp(0, 0x1FFF);
                b.events.set(h, &[50, levels[6].clamp(0, 0x7FFF), p2, 0]);
            }
        }
        // 0x4793D0 on the purge event (+4): {SFXObj level slot 5 (vt
        // +0x38(5), 0..0x7FFF), 0, the stop flag, 0x1000}, then the
        // event's update (0x735580). Under Wine the block read
        // {0x62E, 0, 0, 0x1000} while purging.
        if let (Some(h), Some(b)) = (self.purge, self.purge_bank.as_mut()) {
            if b.events.alive(h) {
                b.events.set(h, &[levels[5].clamp(0, 0x7FFF), 0, self.purge_stop as i32, 0x1000]);
            } else {
                self.purge = None;
            }
        }
        super::carfx::run_bank(&mut self.purge_bank, &mut self.sounds, &mut self.gains, sink, dt);
        super::carfx::run_bank(&mut self.common, &mut self.sounds, &mut self.gains, sink, dt);
        super::carfx::run_bank(&mut self.fe, &mut self.sounds, &mut self.gains, sink, dt);
    }

    /// Events started since the last call.
    pub fn take_started(&mut self) -> usize {
        std::mem::take(&mut self.started)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_events_pick_the_games_sounds() {
        assert_eq!(sound(event::UP, control::GENERIC), Some(0));
        assert_eq!(sound(event::LEFT, control::SLIDER), Some(8));
        assert_eq!(sound(event::RIGHT, control::CARSEL), Some(69));
        assert_eq!(sound(event::BACK, control::ICONSCROLLER), Some(5));
        assert_eq!(sound(event::FOCUS, control::SMS_POPUP), Some(13));
        assert_eq!(sound(event::ROTATION_MED, 25), Some(64));
        assert_eq!(sound(event::UNFOCUS, control::GENERIC), None);
    }

    /// PlayCommonSample at FE_COMMON's level (slot 0, 4235 at the default
    /// volumes) starts the graph's three voices, the side ones at a
    /// quarter level; the turn loop starts once and ends 100 ms after
    /// its stop.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn fe_events_play_at_the_hud_levels() {
        #[derive(Default)]
        struct Log(Vec<f32>, usize);
        impl super::super::carfx::Sink for Log {
            fn one_shot(&mut self, _: &Sample, _: f32, _: u16) {}
            fn start_loop(&mut self, _: &Sample, g: f32, _: u16) -> u32 {
                self.0.push(g);
                self.0.len() as u32
            }
            fn gain(&mut self, _: u32, _: f32) {}
            fn end(&mut self, _: u32) {
                self.1 += 1;
            }
            fn start_once(&mut self, _: &Sample, g: f32, _: u16) -> u32 {
                self.0.push(g);
                self.0.len() as u32
            }
            fn sounding(&mut self, _: u32) -> bool {
                true
            }
            fn pitch(&mut self, _: u32, _: u16) {}
            fn pan(&mut self, _: u32, _: u16) {}
        }
        let root = crate::testkit::game_root();
        let levels = [4235, 6157, 6522, 6157, 6522, 4486, 3085, 0];
        let mut f = FeEvents::new(&root);
        let mut log = Log::default();
        f.play(&mut log, 3, &levels);
        f.update(&mut log, &levels, 0.0, 0.03);
        // Volume bytes 4, 4 and 16 (level >> 8) of 127.
        assert_eq!(log.0.len(), 3, "{:?}", log.0);
        assert!((log.0[2] - 16.0 / 127.0).abs() < 1e-4, "{:?}", log.0);
        f.play(&mut log, 0x49, &levels);
        f.play(&mut log, 0x49, &levels);
        f.update(&mut log, &levels, 0.0, 0.03);
        assert!(f.turn_playing());
        f.turn_stop();
        f.update(&mut log, &levels, 0.0, 0.05);
        assert!(f.turn_playing());
        f.update(&mut log, &levels, 0.0, 0.06);
        assert!(!f.turn_playing());
    }
}
