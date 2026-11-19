//! Game-side audio glue: builds the mixer's inputs from the game state
//! (its sound posting, streams and tick clock).

use crate::*;

pub(crate) mod engine;
pub(crate) mod fe;
pub(crate) mod mix;
pub(crate) mod music_context;
pub(crate) mod soundfx;
pub(crate) mod world;

/// A started collision object: its kind's slot, its layers'
/// voices with the gain they started at, the start volume V they were
/// given, and a water drum's splash. Each frame V is the object's copy's
/// level of the slot again (SFXObj_Collision 0x461CE0 writes +0xC0 every
/// frame) and every playing layer is set to its volume × V >> 15
/// (0x47BB30 → 0x46CF70), the splash's p1 to V · 17000 >> 15 (0x481FD0).
pub(crate) struct ImpactLive {
    pub(crate) slot: usize,
    pub(crate) start: i32,
    pub(crate) voices: Vec<(u32, f32)>,
    pub(crate) splash: Option<u64>,
}

/// The output stream, the FE banks, and the engine entry whose recording
/// is loaded.
pub(crate) struct Audio {
    pub(crate) mixer: Mixer,
    /// SFXObj_Speech's fields f0 / f1 (+4 / +8, 0x4751A0): a line plays
    /// in the front end / in game.
    pub(crate) speech_fields: [i32; 2],
    /// The FE sound banks' events (`fe::FeEvents`).
    pub(crate) fe: nfsu2::audio::fe::FeEvents,
    /// SFXObj_FEHUD's slots this frame (mix map section 0, class 6;
    /// 0x7FFF each without a map).
    fe_levels: [i32; 8],
    /// The FE_COMMON player's stitch slots (the in-game car-part sounds,
    /// `stitch::CommonStitches`).
    pub(crate) part_stitches: nfsu2::audio::stitch::CommonStitches,
    /// The engine entry, its volume chain and the accel GIN's minimum rpm.
    pub(crate) engine: Option<(usize, nfsu2::audio::hybrid::HybridMotor, f32)>,
    /// This retained car context has used scripted audio (not the current gate).
    pub(crate) scripted_engine: bool,
    pub(crate) scripted_pattern: nfsu2::audio::scripted::Pattern,
    /// The race rivals' sound contexts' patterns (index = rival), scripted
    /// during a race start scene (0x47C490).
    pub(crate) ai_patterns: Vec<nfsu2::audio::scripted::Pattern>,
    collision: Option<nfsu2::audio::collision::CollisionSounds>,
    /// Impacts waiting for their collision object's first mix pass (their
    /// start volume is the mix's level of their kind at their point; see
    /// `audio::collision::voices`).
    impacts: Vec<nfsu2::audio::collision::Impact>,
    /// The impacts' collision objects kept in the mix after their first
    /// pass (point, seconds left) so their section-5 event controls (the
    /// impact duck, `collision::mix_fields`) run their envelopes, and, for
    /// a started object, while its layers or splash play: their level
    /// follows the copy's each frame (`ImpactLive`).
    impact_copies: Vec<([f32; 2], f32, Option<ImpactLive>)>,
    /// The collision manager's playing objects (`collision::Pool`).
    impact_pool: nfsu2::audio::collision::Pool,
    /// The player car's nitrous and shift sounds.
    pub(crate) car_sfx: nfsu2::audio::carfx::PlayerSfx,
    /// The mix map (0x486920: the file index it was loaded for), the
    /// player's Physics view slew, and this frame's player slots.
    mix: Option<(usize, nfsu2::audio::mixmap::Mix)>,
    /// The race kind whose mix map failed to load (`load_mix`).
    mix_failed: nfsu2::failed_load::FailedLoads<usize>,
    mix_slew: i32,
    pub(crate) levels: nfsu2::audio::mixmap::Levels,
    /// Section 0's EA Trax and score levels this frame (the music
    /// object's +0xCC and SFXObj_Ambience +0xD0, vtable +0x38(slot)):
    /// [EA Trax slot 0, slot 1, score slot 0, slot 1].
    pub(crate) music_levels: Option<[i32; 4]>,
    /// SFXObj_Speech's mix-map slots 0 / 1 (section 0, class 1: masters 2
    /// / 3, −450 / −300 mB plus the speech volume's control 2).
    speech_levels: Option<[i32; 2]>,
    /// The AI racers' and traffic cars' slots this frame (mix map
    /// sections 2 and 3, one per copy).
    ai_levels: Vec<nfsu2::audio::mixmap::Levels>,
    traffic_levels: Vec<nfsu2::audio::mixmap::Levels>,
    /// The fountains', planes' and trains' slots (mix map sections 4, 7
    /// and 8, one per state).
    enviro_levels: [Vec<nfsu2::audio::mixmap::Levels>; 3],
    /// The planes, trains and fountains (`audio::enviro`) and the race
    /// their managers were set up for.
    enviro: nfsu2::audio::enviro::Enviro,
    pub(crate) enviro_setup: nfsu2::audio::enviro::Setup,
    /// NisAudio_Request's SoundFX flags (0x484510): GB_NOS playing
    /// (0x82BB29), CollisionStream (0x82BB2B), BLOWNENGINE (0x82BB2C);
    /// SFXCTL_Physics +0x6C (BLOWNENGINE asked for this car) and a crash
    /// camera that started this frame (0x5A0117 → 0x486130).
    pub(crate) soundfx: SoundFxGate,
    /// The player's road noise, wind and bottom-out; the other cars.
    pub(crate) env: nfsu2::audio::roadwind::EnvSfx,
    pub(crate) others: nfsu2::audio::traffic::OtherCars,
    /// The collision objects' FX_Scrape (`audio::scrape`).
    pub(crate) scrapes: nfsu2::audio::scrape::Scrapes,
    /// CARSFX_Hydraulics (the player's car object 8).
    hydraulics: nfsu2::audio::hydraulics::HydraulicsSfx,
    /// CARSFX_DriveBy's zone whooshes (`audio::driveby`).
    pub(crate) driveby: nfsu2::audio::driveby::DriveBy,
    /// The minimap GPS text element's +0x35 (0x4C8410): the GPS was in
    /// state 3 (found) at its last update.
    gps_found: bool,
    /// FEGarage_UpdateCarTurn's turn sound (0x4D8BE6..0x4D8C5A): the last
    /// car angle (+0x170) and the playing / stopped flags (+0x16C / +0x16D).
    pub(crate) car_turn: (f32, bool, bool),
    /// The open engage dialog's "ready" sound (`Game::dialog_ready_sound`):
    /// the screen's opening time, when its package sends 0x35F8620B (ms
    /// from the opening), and whether it played.
    dialog_ready: Option<(f32, Option<f32>, bool)>,
    /// SDATA/sdat.viv's speech and EA Trax music (`audio::streams`).
    pub(crate) streams: nfsu2::audio::streams::Streams,
    /// SFXObj_Reverb / SFXCTL_Tunnel's reverb levels (`audio::reverb`)
    /// and the preset whose graph the mixer holds (None outside a drive).
    pub(crate) reverb: nfsu2::audio::reverb::ReverbCtl,
    reverb_loaded: Option<usize>,
    /// The rate the loaded FX graph was built for.
    fx_rate: u32,
    /// Frames since the reverb's debug line (NFSU2_AUDIO_DEBUG).
    reverb_debug: (i32,),
    /// The speech library's audio-start init (0x4866F0) made
    /// (`Game::speech_start`).
    pub(crate) speech_seeded: bool,
    /// The mix map kind (0x4869A0: drag 2, drift 1, else 0) of the last
    /// in-game audio load that re-seeded the speech library
    /// (`Game::audio_load_early`).
    pub(crate) mix_kind: Option<u8>,
    /// The audio's tick clock ([`Game::audio_tick_begin`]): mixer frames
    /// in all ticks so far, those of the tick running, and the part of a
    /// frame left over ([`nfsu2::frame_clock::UNITS`] × frames).
    pub(crate) tick_frames: (u64, usize, i64),
    /// `Game::step` runs this tick: its `audio_frame` posts the tick's
    /// sounds after the update (a headless capture's own pass in `update`
    /// stands aside then).
    pub(crate) stepping: bool,
}

/// A tick's real-time length in [`nfsu2::frame_clock::UNITS`]: a RACE tick
/// 1/60 s, a SCENE tick 1/30 s (the tick clock hands a SCENE tick 60 or 61
/// /1800 s, a RACE tick 29–31 /1800 s; the two never meet: 1/1800 s ticks
/// ≥ 45 are a scene's). No time for a tick of no time (an update of dt 0).
pub(crate) fn tick_units(dt: f32) -> i64 {
    use nfsu2::frame_clock::{RACE, SCENE};
    if dt <= 0.0 {
        0
    } else if dt * 1800.0 >= 45.0 {
        SCENE.period
    } else {
        RACE.period
    }
}

/// The mixer frames of a tick of `units` at `rate`, the remainder `rem`
/// (UNITS × frames) carried: whole frames, their sum exact (48 kHz: 800 a
/// RACE tick, 1600 a SCENE tick; 44.1 kHz: 735 / 1470).
pub(crate) fn tick_frames(units: i64, rate: u32, rem: &mut i64) -> usize {
    let t = units * i64::from(rate) + *rem;
    *rem = t % nfsu2::frame_clock::UNITS;
    (t / nfsu2::frame_clock::UNITS) as usize
}

impl Game {
    /// A tick's sounds, in the order `step` posts them after the update:
    /// the player engine (which runs the mix map first, `mix_sounds`: the
    /// levels every later sound reads), the impacts, the car's effects, the
    /// GPS voice, the dialog-ready cue and the streams. (A headless capture
    /// driven by `update` alone, `dev::hooks::audio_capture_frame`, runs the
    /// same list, [`Game::audio_sounds`], at the update's end.)
    pub(crate) fn audio_frame(&mut self, dt: f32) {
        self.audio_sounds(dt);
        #[cfg(test)]
        self.dev.fe_heard.push(self.dev.fe_played.len());
        // A headless capture renders the tick's frames now, after its
        // sounds (`dev_hooks`).
        dev_hooks::audio_tick_end(self);
        if let Some(a) = self.audio.as_mut() {
            a.stepping = false;
        }
    }

    /// The tick's sounds ([`Game::audio_frame`]'s list; a headless
    /// capture driven by `update` alone runs it too).
    pub(crate) fn audio_sounds(&mut self, dt: f32) {
        self.speech_start();
        // The audio objects' Rand_Int / Rand_Float (turbo, stitch layers,
        // stereo, horns, bottom-out; EA Trax's pick) draw on the game's
        // shared generator (0x7EA624, docs/original/rng.md), as the original's do
        // (docs/native-fixes/N-RNG.tsv): the audio's generator is the game's for the
        // tick's sounds (the game draws nothing on its own copy in
        // between). Only with sound, as natively.
        let saved = self.audio.is_some().then(|| nfsu2::audio::collision::enter_game_rng(self.rng.0));
        self.engine_sound(dt);
        self.impact_sounds();
        self.car_sounds(dt);
        self.gps_sound();
        self.dialog_ready_sound();
        self.stream_sounds(dt);
        if let Some(saved) = saved {
            self.rng.0 = nfsu2::audio::collision::leave_game_rng(saved);
        }
        // The main menu's car after the sound's start-up draws.
        self.menu_car_draw();
        // The sounds the tick's HUD update queued (it runs
        // before the audio manager, 0x5811D0 steps 16 / 18), last as they
        // were posted from the draw right after the tick before.
        self.hud_sounds_update();
    }

    /// `Game::step` is about to run a tick (`update`, then `audio_frame`).
    pub(crate) fn audio_step_begin(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            a.stepping = true;
        }
    }

    /// At a tick's start (`update`): the audio's tick clock. The
    /// speech and NIS lines' "over" bound and the music manager's
    /// hold-off read it ([`nfsu2::audio::streams::LineClock::Mixer`]) as the
    /// mixer frames of the ticks before this one, so a line is over on the
    /// same tick at any display rate; this tick's frames
    /// ([`tick_frames`] of [`tick_units`]) are the ones a headless capture
    /// renders for it.
    pub(crate) fn audio_tick_begin(&mut self, dt: f32) {
        let Some(a) = self.audio.as_mut() else { return };
        let rate = a.mixer.rate();
        a.streams.set_line_clock(nfsu2::audio::streams::LineClock::Mixer { frames: a.tick_frames.0, rate });
        let n = tick_frames(tick_units(dt), rate, &mut a.tick_frames.2);
        a.tick_frames.0 += n as u64;
        a.tick_frames.1 = n;
    }

    /// The mixer frames of the tick running ([`Game::audio_tick_begin`]; a
    /// headless capture renders them).
    #[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
    pub(crate) fn audio_tick_frames(&self) -> usize {
        self.audio.as_ref().map_or(0, |a| a.tick_frames.1)
    }

    /// The game's sounds on `mixer` (the device's, or an offline one).
    pub(crate) fn new_audio(&self, mixer: Mixer) -> Audio {
        let mut streams = nfsu2::audio::streams::Streams::new(&self.game_data.root);
        // An offline mix (the harness's captures) decodes the lines inline,
        // so a line plays from the tick that asked for it.
        streams.set_inline_decode(mixer.is_offline());
        Audio {
            mixer,
            speech_fields: [0; 2],
            fe: nfsu2::audio::fe::FeEvents::new(&self.game_data.root),
            fe_levels: [0x7FFF; 8],
            part_stitches: nfsu2::audio::stitch::CommonStitches::new(&self.game_data.root),
            engine: None,
            scripted_engine: false,
            scripted_pattern: Default::default(),
            ai_patterns: Vec::new(),
            collision: nfsu2::audio::collision::CollisionSounds::load(&self.game_data.root),
            impacts: Vec::new(),
            impact_copies: Vec::new(),
            impact_pool: Default::default(),
            car_sfx: nfsu2::audio::carfx::PlayerSfx::new(&self.game_data.root),
            mix: None,
            mix_failed: Default::default(),
            mix_slew: 0,
            levels: Default::default(),
            music_levels: None,
            speech_levels: None,
            ai_levels: Vec::new(),
            traffic_levels: Vec::new(),
            enviro_levels: Default::default(),
            enviro: nfsu2::audio::enviro::Enviro::new(&self.game_data.root),
            enviro_setup: Default::default(),
            soundfx: Default::default(),
            env: nfsu2::audio::roadwind::EnvSfx::new(&self.game_data.root),
            others: nfsu2::audio::traffic::OtherCars::new(&self.game_data.root),
            scrapes: nfsu2::audio::scrape::Scrapes::new(&self.game_data.root),
            hydraulics: nfsu2::audio::hydraulics::HydraulicsSfx::new(&self.game_data.root),
            driveby: nfsu2::audio::driveby::DriveBy::new(&self.game_data.root),
            gps_found: false,
            car_turn: (0.0, false, false),
            dialog_ready: None,
            streams,
            reverb: Default::default(),
            reverb_loaded: None,
            fx_rate: 0,
            reverb_debug: (0,),
            speech_seeded: false,
            mix_kind: None,
            tick_frames: (0, 0, 0),
            stepping: false,
        }
    }
}

#[cfg(test)]
mod tick_clock_tests {
    use super::{tick_frames, tick_units};

    /// The audio tick clock: a tick is its real-time length in
    /// mixer frames, whatever its game dt in the native pattern (RACE 30,
    /// 29, 31 /1800 s; SCENE 60, 60, 60, 60, 61 /1800 s), and the frames
    /// sum exactly at any mix rate.
    #[test]
    fn ticks_render_their_real_time_in_whole_frames() {
        let race: Vec<f32> = [30u32, 29, 31].iter().map(|&t| t as f32 * (1.0 / 1800.0)).collect();
        let scene: Vec<f32> = [60u32, 60, 60, 60, 61].iter().map(|&t| t as f32 * (1.0 / 1800.0)).collect();
        let mut rem = 0;
        for &dt in &race {
            assert_eq!(tick_frames(tick_units(dt), 48000, &mut rem), 800);
            assert_eq!(tick_frames(tick_units(dt), 44100, &mut rem), 735);
        }
        for &dt in &scene {
            assert_eq!(tick_frames(tick_units(dt), 48000, &mut rem), 1600);
        }
        assert_eq!(tick_units(1.0 / 60.0), nfsu2::frame_clock::RACE.period);
        assert_eq!(tick_units(0.0), 0);
        // 22.05 kHz: 367.5 a RACE tick, alternating 367 / 368, exact over two.
        let mut rem = 0;
        let n: Vec<usize> = (0..4).map(|_| tick_frames(tick_units(race[0]), 22050, &mut rem)).collect();
        assert_eq!(n, [367, 368, 367, 368]);
    }
}
