use super::decode_thread::{new_buf, spawn_decoder};
use super::*;

/// The in-game score stream's volume byte for the score byte `v` the game
/// sets (0x47B647 / 0x47B524 → 0x46C800, and 0x47B820 storing v into MPF
/// slot 0's +0xC; v = the Ambience level >> 8, so the effects volume and
/// the master feed it): v itself. **Observed** under Wine (free roam,
/// effects 1, master 1, at the original's own pace and at a fixed 1/120 s
/// frame, two places, the score contexts' streams playing): the slot's +0xC
/// and the stream's library voice (2 channels, gain +0x38) both read 40 of
/// 127 at v = 40; and in the recorded mix the score's decoded stream
/// (event 27, MPF stream entry 0), found by cross-correlation (lag 0.4 ms,
/// steady over 30 s), fits at a gain of 0.30–0.33 per channel = 38–42 of
/// 127. An earlier reading of 79 / 39 / 19 / 27 at v = 40 / 20 / 10 / 14
/// did not reproduce (its state is not known).
pub(crate) fn in_game_score_volume(v: i32) -> i32 {
    v.clamp(0, 127)
}

impl Streams {
    /// The score's music context and event playing, if any.
    pub fn score(&self) -> Option<(u8, usize)> {
        self.score.as_ref().map(|s| (s.0, s.1))
    }

    pub fn stop_score(&mut self, mixer: &Mixer) {
        self.score_fade = false;
        self.score_since = None;
        if let Some((.., id, _)) = self.score.take() {
            mixer.stop_track(id);
        }
    }

    /// Whether the score's context-change fade runs (SFXObj_Ambience's
    /// field 2, +0xC: the mix map's event control 1 takes it down to
    /// −10000 mB over 11 frames, curve 2, and back up when it drops).
    pub fn score_fading(&self) -> bool {
        self.score_fade
    }

    /// The interactive score (0x47B260) for music context `zone`
    /// (+0xB4), its level `level` (SFXObj_Ambience's slot 0 in the front
    /// end, 1 in game; none: no mix map, full scale). Front end: a
    /// context other than the one playing (+0xB0) ends the piece at once
    /// (0x739320); in game it sets the fade (+0xD0 +0xC = 1, +0xC8) and
    /// the piece plays on at level >> 8 until the level is 0, then the
    /// new context's piece starts (+0xC = 0, +0x10 = 1). With nothing
    /// playing (0x82AFD4 = −1) the context's first event starts
    /// (0x7399E0 / 0x4580E0 on 0x7ECC10[+0xB4]) — a piece that ran out
    /// starts again. In game the steady volume is (trunc(+0x7C · 32767) ·
    /// level) >> 23: the "Volumes" offset's ramp (+0x60 → 0x468140,
    /// 300 ms, 0x459990) times the level.
    pub(super) fn score_frame(
        &mut self,
        mixer: &Mixer,
        zone: u8,
        in_game: bool,
        level: Option<i32>,
        volumes_mb: Option<i32>,
        dt: f32,
    ) {
        // The Volumes offset (0x46B330 → +0x60 in game).
        if in_game {
            if let Some(v) = volumes_mb {
                self.volumes.0 = v;
            }
            let lin = |mb: i32| crate::audio::mixmap::db_to_lin(mb) as f32 * 3.051851e-05;
            if self.volumes.0 != self.volumes.1 {
                // From a song (+0x14 < 0x1B) the ramp starts at the last
                // offset; while the score runs, at its value.
                let from = if self.score.is_none() { lin(self.volumes.1) } else { self.volumes.2.value };
                self.volumes.2 = crate::audio::transients::Ramp::set(from, lin(self.volumes.0), 300, 0);
                self.volumes.1 = self.volumes.0;
            }
            // The music manager 0x482350 steps it (0x46B100 at 0x482722)
            // once per audio update (AudioManager_Update 0x483B40, behind
            // its 15 ms real-time gate) with the frame's dt, 0 while the
            // audio is held ([0x82B884]+0x13): the clone's fixed 60 Hz
            // world update is that cadence. Not modelled: the hold's dt 0
            // and the game-state gate (0x8654A4 3 / 6).
            self.volumes.2.step(dt);
        }
        if let Some(cur) = self.score.as_ref().map(|s| s.0)
            && cur != zone
        {
            if in_game {
                // +0xB0 takes the new context now (again if it changes
                // during the fade); its piece waits for the fade.
                self.score_fade = true;
                if let Some(sc) = self.score.as_mut() {
                    sc.0 = zone;
                    sc.1 = usize::MAX;
                }
            } else {
                self.stop_score(mixer);
            }
        }
        if self.score_fade {
            let l = level.unwrap_or(0);
            if let Some((.., id, _)) = &self.score {
                mixer.set_track_gain(*id, vol(in_game_score_volume(l >> 8)));
            }
            if l > 0 {
                return;
            }
            // At 0 the new context's piece (+0xB0) starts.
            self.stop_score(mixer);
            self.score_fade = false;
        }
        let gain = match (level, in_game) {
            (None, _) => 1.0,
            (Some(l), false) => vol(l >> 8),
            (Some(l), true) => {
                vol(in_game_score_volume(((self.volumes.2.value * 32767.0) as i32).wrapping_mul(l) >> 23))
            }
        };
        if let Some((.., id, _)) = &self.score
            && mixer.track_playing(*id)
        {
            mixer.set_track_gain(*id, gain);
            return;
        }
        if self.score.is_some()
            && let Some(since) = self.score_since.take()
        {
            self.backoff.ended(since, self.line_now());
        }
        self.score = None;
        if self.backoff.held(self.line_now()) {
            return;
        }
        let (Some(viv), Some(mpf)) = (self.viv.clone(), self.mpf.clone()) else {
            return self.backoff.fail(self.line_now());
        };
        let Some(event) = music::score_event(zone) else {
            return;
        };
        let Some(sample) = mpf.event_sample(event) else {
            return self.backoff.fail(self.line_now());
        };
        let buf = new_buf();
        let b = buf.clone();
        let pace = (!mixer.is_offline()).then_some(music::Pace::LIVE);
        let started = spawn_decoder(false, move || {
            if music::decode_sample_paced(&viv, &mpf, sample, &b, pace).is_none()
                && let Ok(mut t) = b.lock()
            {
                t.done = true;
            }
        });
        if !started {
            buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner).done = true;
        }
        let id = mixer.play_track(buf.clone(), gain, 3);
        if crate::env_set!("NFSU2_AUDIO_DEBUG") {
            eprintln!("audio: score context {zone:#x} event {event} stream {sample} gain {gain:.3}");
        }
        self.score = Some((zone, event, id, buf));
        self.score_since = Some(self.line_now());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_in_game_score_plays_at_the_score_byte() {
        // Read under Wine (free roam, effects 1, master 1): v = 40 → the
        // stream voice at 40 of 127, the mix fitting the stream at 0.30–0.33.
        assert_eq!(super::in_game_score_volume(40), 40);
        assert!((super::super::vol(super::in_game_score_volume(40)) - 0.315).abs() < 0.01);
        assert_eq!(super::in_game_score_volume(0), 0);
        assert_eq!(super::in_game_score_volume(200), 127);
    }
}
