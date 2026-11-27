use super::decode_thread::{Decoding, new_buf, spawn_decoder};
use super::*;

/// Holds the music manager off after a song or score piece that failed to
/// play (a corrupt stream ends at once; a missing archive never starts), so
/// a broken archive stays quiet instead of picking again every tick (no
/// native counterpart). Timed on the lines' clock ([`LineClock`]: the
/// mixer frames the game hands per tick), so the next pick falls on the
/// same tick at any display rate.
#[derive(Default)]
pub(super) struct Backoff {
    /// Since when it holds.
    since: Option<LineStamp>,
}

impl Backoff {
    /// A piece that ended sooner than this (s) never really played.
    const SHORTEST: f32 = 1.0;
    /// How long a failure holds the manager off (s).
    const HOLD: f32 = 5.0;

    pub(super) fn fail(&mut self, now: LineStamp) {
        self.since = Some(now);
    }

    /// A piece started at `since` is over at `now`: a failure if it was short.
    pub(super) fn ended(&mut self, since: LineStamp, now: LineStamp) {
        if since.secs_to(now) < Self::SHORTEST {
            self.fail(now);
        }
    }

    pub(super) fn held(&self, now: LineStamp) -> bool {
        self.since.is_some_and(|s| s.secs_to(now) < Self::HOLD)
    }
}

impl Streams {
    /// Start song `i` (0x47AF90); its caption.
    pub fn play_song(&mut self, mixer: &Mixer, i: usize) -> Option<Caption> {
        let (Some(viv), Some(mpf)) = (self.viv.clone(), self.mpf.clone()) else {
            self.backoff.fail(self.line_now());
            return None;
        };
        if mpf.song_sample(i).is_none() {
            self.backoff.fail(self.line_now());
            return None;
        }
        self.stop_song(mixer);
        let buf = new_buf();
        // The decoder holds the buffer's only other strong count (it stops
        // once the song is dropped, `music::decode_sample`).
        let b = Decoding(buf.clone());
        // Always on a thread, also offline: the song decodes progressively
        // and nothing in the game reads its progress (only whether its
        // track still plays, which the mixer decides; the offline pump
        // waits for the samples it needs, `Mixer::pump`). Live, spread
        // over the song's playing time (`music::Pace`).
        let pace = (!mixer.is_offline()).then_some(music::Pace::LIVE);
        spawn_decoder(false, move || {
            music::decode_song_paced(&viv, &mpf, i, &b.0, pace);
        });
        let id = mixer.play_track(buf.clone(), 1.0, 3);
        self.song = Some((i, id, buf));
        self.song_since = Some(self.line_now());
        Some(Caption { song: i })
    }

    /// 0x479220 again with the same playlist (the jukebox's exit,
    /// 0x56B25A; a profile load): the pools refill, the positions go back.
    pub fn reset_playlist(&mut self) {
        self.playlist = None;
    }

    /// The title screen (UG_LS_Splash, 0x4A90F0) starts the front end's
    /// music: 0x480C30(−2) → 0x47AC50 state 1 stops the song (0x739320)
    /// and, with −2, makes the next front-end pick song 0
    /// ([`music::Playlist::title_start`], on the next running front-end
    /// frame, once the playlist is built). Only on the change into state 1:
    /// the boot movies and the attract movie before the title leave it.
    pub fn title_music(&mut self, mixer: &Mixer) {
        self.stop_song(mixer);
        self.title_pick = true;
    }

    /// The music object's playlist state (masks, pools, last positions)
    /// as the next pick will find it; none until the first frame that
    /// builds it.
    pub fn playlist(&self) -> Option<&music::Playlist> {
        self.playlist.as_ref()
    }

    pub fn stop_song(&mut self, mixer: &Mixer) {
        self.song_since = None;
        if let Some((_, id, _)) = self.song.take() {
            mixer.stop_track(id);
        }
    }

    pub fn song(&self) -> Option<usize> {
        self.song.as_ref().map(|s| s.0)
    }
    /// Each frame (0x482350 → 0x482180 → 0x47B080): the song follows the
    /// music manager's context. Front end ↔ game stops the song (0x481440
    /// → state 2 leaving the game; state 7 as the track load's LOADING
    /// starts), `Paused` holds it and the next running context lets it go
    /// on (state 6), `Stopped` ends it. With no song playing (0x82AFD4 =
    /// −1, the stream ran out or was stopped) the front end and the game
    /// start the next of their playlist (0x46B6F0): a song is never
    /// looped, its end starts the next.
    ///
    /// `zone` is the score's music context (+0xB4: the front end's 0x1C,
    /// in game the Ambience map's at the player car): the score plays in
    /// `Score`, and when the playlist has no song for the side (0x47B080
    /// returns −1, 0x482180 then runs 0x47B260).
    ///
    /// `order` / `random` are the profile's playlist and play mode: when
    /// they differ from the ones the playlist was built from, it is built
    /// again (0x479220 on each jukebox change).
    ///
    /// `levels`: section 0's [EA Trax slot 0, slot 1, score slot 0, slot
    /// 1] (none without a mix map: full scale); `volumes_mb` the
    /// "Volumes" map at the player car (in game).
    #[allow(clippy::too_many_arguments)]
    pub fn music_frame(
        &mut self,
        mixer: &Mixer,
        context: MusicContext,
        order: &[(usize, u8)],
        random: bool,
        zone: u8,
        levels: Option<[i32; 4]>,
        volumes_mb: Option<i32>,
        dt: f32,
    ) -> Option<Caption> {
        self.context = context;
        let pause = |s: &Option<(usize, u32, TrackBuf)>, on: bool| {
            if let Some((_, id, _)) = s {
                mixer.pause_track(*id, on);
            }
        };
        let pause_score = |s: &Option<(u8, usize, u32, TrackBuf)>, on: bool| {
            if let Some((.., id, _)) = s {
                mixer.pause_track(*id, on);
            }
        };
        match context {
            MusicContext::Off | MusicContext::Stopped => {
                self.stop_song(mixer);
                self.stop_score(mixer);
                return None;
            }
            MusicContext::Paused => {
                pause(&self.song, true);
                pause_score(&self.score, true);
                return None;
            }
            // 0x482180 in a drag or drift race: a song playing is
            // ended (0x739320 while +0x14 < 0x1B), the score runs.
            MusicContext::Score | MusicContext::ScoreFrontEnd => {
                let game = context == MusicContext::Score;
                self.stop_song(mixer);
                if self.side.is_some_and(|b| b != game) {
                    self.stop_score(mixer);
                }
                self.side = Some(game);
                pause_score(&self.score, false);
                self.score_frame(mixer, zone, game, levels.map(|l| l[2 + game as usize]), volumes_mb, dt);
                return None;
            }
            MusicContext::FrontEnd | MusicContext::Game | MusicContext::Dyno => {}
        }
        if self.playlist.as_ref().is_none_or(|p| p.order != order || p.random != random) {
            self.playlist = Some(music::Playlist::new(order, random));
        }
        if std::mem::take(&mut self.title_pick)
            && let Some(p) = self.playlist.as_mut()
        {
            p.title_start();
        }
        // EA Trax again: the score ends (0x739320 while +0x14 > 0x1A).
        let side_now = matches!(context, MusicContext::Game | MusicContext::Dyno);
        if self.side.is_some_and(|b| b != side_now) {
            self.stop_score(mixer);
        }
        // Between the front end and the game the song is stopped, not
        // carried over (a pause in between keeps the side it came from).
        let side = side_now;
        if self.side.is_some_and(|b| b != side) {
            self.stop_song(mixer);
        }
        self.side = Some(side);
        pause(&self.song, false);
        // The song's volume each frame (0x482180: the EA Trax object's
        // vtable +0x38(slot 0 front end / 1 game) → +0xBC, 0x46C800(>> 8)).
        let song_gain = levels.map_or(1.0, |l| vol(l[side as usize] >> 8));
        if let Some((_, id, _)) = &self.song
            && mixer.track_playing(*id)
        {
            mixer.set_track_gain(*id, song_gain);
            return None;
        }
        if self.song.is_some()
            && let Some(since) = self.song_since.take()
        {
            self.backoff.ended(since, self.line_now());
        }
        self.song = None;
        if self.backoff.held(self.line_now()) {
            return None;
        }
        // 0x46B6F0 takes the song out of the pool before it plays, so one
        // with no stream is passed over, not tried again each frame.
        // The dyno (race type 8): 0x482180 asks for the race mask, 0x46B6F0
        // then draws from the front end's pool.
        let next = self
            .playlist
            .as_mut()
            .filter(|p| context != MusicContext::Dyno || p.has_songs(true))
            .and_then(|p| crate::audio::collision::with_rng(|rng| p.next(context == MusicContext::Game, rng)));
        if crate::env_set!("NFSU2_AUDIO_DEBUG") {
            eprintln!(
                "audio: trax pick {next:?} ({}, {})",
                match context {
                    MusicContext::Game => "game",
                    MusicContext::Dyno => "dyno",
                    _ => "front end",
                },
                if random { "random" } else { "sequential" }
            );
        }
        let Some(next) = next else {
            // No song for this side: the score (0x47B080 → −1 → 0x47B260).
            pause_score(&self.score, false);
            self.score_frame(mixer, zone, side, levels.map(|l| l[2 + side as usize]), volumes_mb, dt);
            return None;
        };
        self.stop_score(mixer);
        let c = self.play_song(mixer, next);
        if let Some((_, id, _)) = &self.song {
            mixer.set_track_gain(*id, song_gain);
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::LineStamp;

    #[test]
    fn a_failed_piece_holds_the_manager_off() {
        // On the mixer-frame clock at 48 kHz.
        let at = |s: f64| LineStamp::Frames((s * 48000.0) as u64, 48000);
        let mut b = super::Backoff::default();
        assert!(!b.held(at(0.0)));
        // A piece that ran for a minute is a normal end.
        b.ended(at(0.0), at(60.0));
        assert!(!b.held(at(60.0)));
        // One that ended at once is a failure: held for 5 s from its end.
        b.ended(at(60.0), at(60.005));
        assert!(b.held(at(61.0)) && b.held(at(65.0)));
        assert!(!b.held(at(65.005)) && !b.held(at(66.0)));
    }
}
