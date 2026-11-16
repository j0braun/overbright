//! The streamed audio the game plays from `SDATA/sdat.viv`: the cut
//! scenes' speech (NIS bank, [`super::speech`]) and the EA Trax music
//! ([`super::music`]), decoded on worker threads into mixer tracks.

mod decode_thread;
mod lines;
mod music_manager;
mod score;

use super::mixer::{Levels, Mixer, TrackBuf, TrackPcm};
use super::music::{self, Mpf};
use super::speech::{self, Bank};
use crate::formats::viv::Viv;
use lines::{Backup, LineStamp};
pub use lines::{LineClock, line_limit_frames, line_limit_secs};
use music_manager::Backoff;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A song caption to show (Chyron_ShowSong 0x4AC9F0 → 0x4AC950: title,
/// artist, album of the EA Trax table entry).
#[derive(Clone, Debug, PartialEq)]
pub struct Caption {
    pub song: usize,
}

/// Where the music manager is (0x482350 → 0x482180 each frame, the
/// states of 0x47AC50 on the player's +0xC0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MusicContext {
    /// No music manager pass (boot, before the title).
    #[default]
    Off,
    /// The front end (audio +0x90 = 1): the menu playlist.
    FrontEnd,
    /// In game with EA Trax (game music on, not a drag or drift race,
    /// 0x401910 / 0x4018F0): the race playlist.
    Game,
    /// The tuning session on the dyno track (g_RaceType 0x89E7B0 = 8;
    /// audio +0x90 = 5, game state 6): the game's rules (0x482180's in-game
    /// branch: game music and master on and the race mask +0x8C not
    /// empty, the volume at slot 1), but the song comes from the front
    /// end's pool (0x46B6F0 takes +0x84 / +0x88 for race type 8). Under
    /// Wine (L1, the finished-career save's Lancer) a new song started as the dyno track
    /// loaded and played on through DynoEnter and the tuning menus.
    Dyno,
    /// The song held where it is (state 5, 0x739030(…, 1)): the pause
    /// menu, the countdown (flow states 3 / 6 / 9, 0x481210), PC_Loading
    /// over a running flow (0x482350), a movie in game.
    Paused,
    /// The interactive score in game (0x482180 → 0x47B260 alone): a drag
    /// or drift race (0x4018F0 / 0x401910), or the game music or the
    /// master at 0 (0x482243).
    Score,
    /// The interactive score in the front end: the menu music at 0
    /// (0x4821D7). Its level is SFXObj_Ambience's, which follows the
    /// effects volume, so it is heard.
    ScoreFrontEnd,
    /// The song stopped (state 7 on the flow's LOADING / CAMERA_FLYBY,
    /// 0x481210 with 1 / 2; state 0xB for a movie in the front end).
    Stopped,
}

/// The speech library's seed before the game runs its init
/// ([`speech::Library::init`]; 0x82BA34 is .bss 0, not a static 9).
pub const SPEECH_SEED: u32 = speech::UNINIT_SEED;

pub struct Streams {
    /// The failed-piece hold-off and when the song / score started.
    backoff: Backoff,
    song_since: Option<LineStamp>,
    score_since: Option<LineStamp>,
    viv: Option<Arc<Viv>>,
    nis: Option<Arc<Bank>>,
    /// The in-game speech bank (UG2_Speech, the language table 0x7ECFA8's
    /// English name) and the line playing on the speech stream (0x82BA30
    /// +4).
    speech: Option<Arc<Bank>>,
    speech_track: Option<u32>,
    /// The lines' buffers and start times: a line counts as over once its
    /// length has passed even if the device never played it out (a stalled
    /// or silent device must not hold the game; the original skips speech
    /// when it has no speech object).
    speech_line: Option<(TrackBuf, LineStamp)>,
    nis_line: Option<(TrackBuf, LineStamp)>,
    /// The clock those bounds read ([`LineClock`]): the wall
    /// clock until the game hands the mixer-frame clock per tick.
    line_clock: LineClock,
    /// Decode the lines on the caller's thread ([`Streams::set_inline_decode`]).
    inline_decode: bool,
    /// The speech library (both banks' requests, 0x734E70..0x738A40).
    lib: speech::Library,
    /// The NIS stream's backup line (the race start's kind 7, 0x46CD20).
    backup: Backup,
    mpf: Option<Arc<Mpf>>,
    nis_track: Option<u32>,
    /// The NIS line's stream kind (NisAudio_Request 0x484510's first
    /// argument, queued as is by 0x46CA60): 1 GB_NOS, 2 a race start, 4
    /// THUNDERSTRM, 5 CollisionStream, 6 a shop / dyno / ending scene, 8
    /// BLOWNENGINE, 9 the career start.
    pub nis_kind: u8,
    /// Since when the speech and NIS streams are held (0x46C8D0, the
    /// game's pause), if they are.
    held: Option<LineStamp>,
    /// The song playing: (index, track, its buffer).
    song: Option<(usize, u32, TrackBuf)>,
    /// The music object's playlist state (0x46BAA0's masks and pools).
    /// Its Random mode draws Rand_Int (0x43C1C0) on the game's one state
    /// 0x7EA624 (docs/native-fixes/N-RNG.tsv, 0x46B86F / 0x46BA2F): the audio's
    /// generator, which is the game's while its tick sounds run
    /// (`collision::enter_game_rng`).
    playlist: Option<music::Playlist>,
    /// The title screen started the music ([`Streams::title_music`]): the
    /// playlist takes [`music::Playlist::title_start`] on the next frame,
    /// once it is built from the profile's.
    title_pick: bool,
    context: MusicContext,
    /// The side the song belongs to (true in game).
    side: Option<bool>,
    /// The score playing (0x47B260): (music context +0xB0, event, track,
    /// its buffer).
    score: Option<(u8, usize, u32, TrackBuf)>,
    /// The in-game context change's fade (+0xC8; SFXObj_Ambience +0xC
    /// set): the piece plays on until the Ambience level reaches 0.
    score_fade: bool,
    /// The "Volumes" offset (+0x60 the target mB, +0x64 the last one,
    /// the ramp +0x68..+0x80 with its value +0x7C; 0x459CB0's defaults
    /// {0, 0.001 s, 0 → 1}).
    volumes: (i32, i32, super::transients::Ramp),
}

/// The stream volume 0x46C800 takes (0..127) as a gain.
pub(super) fn vol(v: i32) -> f32 {
    v.clamp(0, 127) as f32 * (1.0 / 127.0)
}

impl Streams {
    /// Mount the archive (the audio start-up 0x486510) and read the NIS
    /// bank's index files and the music map.
    pub fn new(root: &Path) -> Streams {
        let viv = Viv::mount_game(root).map(Arc::new);
        let nis = viv.as_ref().and_then(|v| Bank::load(v, "nisaudio")).map(Arc::new);
        let speech = viv.as_ref().and_then(|v| Bank::load(v, "UG2_Speech")).map(Arc::new);
        let mpf = viv.as_ref().and_then(|v| v.read("pfdata/MusicSFx.mpf")).and_then(|d| Mpf::parse(&d)).map(Arc::new);
        Streams {
            backoff: Backoff::default(),
            song_since: None,
            score_since: None,
            viv,
            nis,
            speech,
            speech_track: None,
            speech_line: None,
            nis_line: None,
            line_clock: LineClock::Wall,
            inline_decode: false,
            lib: speech::Library::default(),
            backup: Backup::default(),
            mpf,
            nis_track: None,
            nis_kind: 0,
            held: None,
            song: None,
            playlist: None,
            title_pick: false,
            context: MusicContext::Off,
            side: None,
            score: None,
            score_fade: false,
            volumes: (-1, -2, super::transients::Ramp::raw(0.0, 1.0, 0.001, 0)),
        }
    }

    /// The options' levels (0x45C210): each × master.
    pub fn levels(master: f32, effects: f32, engine: f32, speech: f32, music: f32) -> Levels {
        Levels { effects: effects * master, engine: engine * master, speech: speech * master, music: music * master }
    }
}
