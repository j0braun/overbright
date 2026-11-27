use super::decode_thread::{Decoding, new_buf, spawn_decoder};
use super::*;

/// The speech channel's state ([`Streams::speech_channel`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineState {
    Gone,
    Queued,
    Playing(i64),
}

/// The clock the lines' "over" bound ([`within_length`]) reads (for
/// frame-rate independence): the wall clock (today's default), or the mixer
/// frames the game has pumped, handed in once per tick
/// ([`Streams::set_line_clock`]; 800 frames a RACE tick at 48 kHz), so a
/// line is over at the same tick at any display rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineClock {
    Wall,
    /// `frames` mixed so far at `rate` frames a second.
    Mixer { frames: u64, rate: u32 },
}

/// When a line started (or the streams were held), on the clock in use then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LineStamp {
    Wall(std::time::Instant),
    Frames(u64, u32),
}

impl LineStamp {
    /// Seconds from `self` to `now`. A stamp on the other clock than `now`
    /// (the clock switched while a line played): the wall stamp's own
    /// elapsed time, a frame stamp counts as just started.
    pub(super) fn secs_to(self, now: LineStamp) -> f32 {
        match (self, now) {
            (LineStamp::Wall(t), _) => t.elapsed().as_secs_f32(),
            (LineStamp::Frames(f, _), LineStamp::Frames(n, rate)) => {
                (n.saturating_sub(f) as f64 / f64::from(rate.max(1))) as f32
            }
            (LineStamp::Frames(..), LineStamp::Wall(_)) => 0.0,
        }
    }
}

/// A line's bound in seconds: its decoded length (+0.5 s), or 30 s while
/// it is still decoding.
pub fn line_limit_secs(frames: usize, channels: usize, rate: u32, done: bool) -> f32 {
    if done { frames as f32 / channels.max(1) as f32 / rate.max(1) as f32 + 0.5 } else { 30.0 }
}

/// The tick form's bound: a line started at mixer frame s is over from
/// mixer frame s + N on, N the least count whose seconds at `mix_rate`
/// (as [`LineStamp`] measures them) reach [`line_limit_secs`]. `samples`
/// is the decoded buffer's length (all channels).
pub fn line_limit_frames(samples: usize, channels: usize, rate: u32, done: bool, mix_rate: u32) -> u64 {
    let limit = line_limit_secs(samples, channels, rate, done);
    let secs = |n: u64| (n as f64 / f64::from(mix_rate.max(1))) as f32;
    let mut n = (f64::from(limit) * f64::from(mix_rate.max(1))).ceil().max(0.0) as u64;
    while n > 0 && secs(n - 1) >= limit {
        n -= 1;
    }
    while secs(n) < limit {
        n += 1;
    }
    n
}

/// Whether a line started at `start` can still be playing at `now`: within
/// its decoded length (+0.5 s), or within 30 s while it is still decoding.
fn within_length(buf: &TrackBuf, start: LineStamp, now: LineStamp) -> bool {
    let t = buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let limit = line_limit_secs(t.samples.len(), t.channels, t.rate, t.done);
    start.secs_to(now) < limit
}

/// The NIS stream object's backup line (0x46CD20 / 0x46CA10 and the NIS
/// stream object's update 0x475330; **verified** in the disassembly,
/// docs/native-fixes/N-AUD2.tsv): when a race start's line (kind 2) starts while NisAudio's
/// last request (+4) is still kind 2, NIS_Select_Start is set to the
/// scene's and the track's values with 4 as the third (0x46CD20 reads
/// NisAudio +0xC / +0x10, stored by the kind-2 request at 0x484A56) and a
/// kind-7 request is queued on the NIS stream ([`Streams::race_start_line_started`]):
/// +0x4C = 1 (0x48414E), the
/// "backup stream armed" both speech gates read (0x46D100). When the
/// scene is skipped ("button through", 0x43B460 → 0x479200) and the
/// backup is ready (0x462020), it replaces the playing line (0x45AD40(7)).
/// It is cleaned up (0x46CA10: its stream stopped, +0x4C = 0) once, by
/// [`Streams::backup_update`] or [`Streams::nis_over`].
#[derive(Default)]
pub(super) struct Backup {
    /// The kind-7 request: its line and decoded buffer (+0x4C while set).
    line: Option<(Arc<Bank>, speech::Pick, TrackBuf)>,
    /// 0x82BC49: the scene was skipped.
    skipped: bool,
    /// NIS stream object +0xB1: cleaned up since the last kind-2 start.
    cleaned: bool,
    /// NisAudio +0xC / +0x10 of the last kind-2 request.
    start: Option<Vec<u32>>,
}

impl Streams {
    /// The clock the lines' bounds read from now on ([`LineClock`]). The
    /// tick form: the game calls this once per tick, before the audio
    /// update, with the mixer frames pumped so far.
    pub fn set_line_clock(&mut self, clock: LineClock) {
        self.line_clock = clock;
    }

    /// Decode the speech and NIS lines on the caller's thread, finished
    /// before the request returns (on for an offline mix, `Mixer::offline`:
    /// the harness's captures). On a thread a line is "queued, not yet
    /// playing" (`speech_free` no, `speech_playing` no) until its decoder
    /// finishes, at a moment no tick determines; inline it plays at once
    /// (docs/original/uncapped-fps.md "Audio on ticks"). The songs and the score keep their threads (nothing in
    /// the game reads their progress; the offline pump waits for it).
    pub fn set_inline_decode(&mut self, on: bool) {
        self.inline_decode = on;
    }

    /// Now on the lines' clock.
    pub(super) fn line_now(&self) -> LineStamp {
        match self.line_clock {
            LineClock::Wall => LineStamp::Wall(std::time::Instant::now()),
            LineClock::Mixer { frames, rate } => LineStamp::Frames(frames, rate),
        }
    }

    /// Decode a picked record into a fresh track buffer.
    fn decode_line(&self, bank: &Arc<Bank>, pick: speech::Pick) -> Option<TrackBuf> {
        let viv = self.viv.clone()?;
        let bank = bank.clone();
        let buf = new_buf();
        let b = Decoding(buf.clone());
        spawn_decoder(self.inline_decode, move || {
            let p = bank.decode(&viv, pick.entry, pick.record);
            let mut t = b.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(p) = p {
                t.rate = p.rate;
                t.channels = p.channels as usize;
                t.samples = p.samples;
            }
            // Done with the samples, under the same lock: marked by the
            // `Decoding` hold's drop alone, a reader could see the
            // samples and not yet "done" (a line Queued for a moment, its
            // end held by the mixer).
            t.done = true;
        });
        Some(buf)
    }

    /// Play a NIS line: event `event` requested with `tuple` through the
    /// speech library ([`speech::Library::request`]: the gate, the
    /// alternatives, the fitting records and the keyed pick — natively the
    /// NIS bank goes through the same library as Rachel's), at the
    /// stream's full volume (0x484060: +0x1C / 127). A race start (kind 2)
    /// arms the backup line ([`Backup`]).
    pub fn play_nis(&mut self, mixer: &Mixer, event: u16, tuple: &[u32], kind: u8) -> bool {
        let Some(bank) = self.nis.clone() else {
            return false;
        };
        let Some(ev) = bank.events.iter().find(|e| e.id == event) else {
            return false;
        };
        let Some(pick) = self.lib.request(&bank, speech::BankId::Nis, ev, tuple) else {
            return false;
        };
        let Some(buf) = self.decode_line(&bank, pick) else {
            return false;
        };
        self.lib.played(&bank, speech::BankId::Nis, pick);
        self.stop_nis(mixer);
        self.nis_line = Some((buf.clone(), self.line_now()));
        self.nis_kind = kind;
        self.nis_track = Some(mixer.play_track(buf, 1.0, 2));
        if kind == 2 {
            // NisAudio +0xC / +0x10 (0x484A56): the scene's and the
            // track's values, for the backup.
            self.backup.start = tuple.get(..2).map(<[u32]>::to_vec);
        }
        true
    }

    /// 0x475330 as the race start's line (kind 2) starts (its "NIS audio
    /// ready" 0x475530 and "NIS ready to play" 0x4791D0 both set; the
    /// clone starts the line at once in [`Self::play_nis`]): +0xB1
    /// cleared, and while NisAudio's last request (+4) is still kind 2,
    /// 0x46CD20 sets NIS_Select_Start = (scene, track, 4) and queues the
    /// kind-7 backup request. A game that calls this must also call
    /// [`Self::backup_update`] each audio update (the cleanups).
    pub fn race_start_line_started(&mut self) {
        self.backup.cleaned = false;
        let Some(bank) = self.nis.clone() else {
            return;
        };
        let Some([scene, track]) = self.backup.start.as_deref().and_then(|t| <[u32; 2]>::try_from(t).ok()) else {
            return;
        };
        if self.nis_kind != 2 {
            return;
        }
        let bank = &bank;
        let Some(ev) = bank.events.iter().find(|e| e.id == speech::nis::EVENT_START) else {
            return;
        };
        let values = [scene, track, 4];
        if let Some(pick) = self.lib.request(bank, speech::BankId::Nis, ev, &values)
            && let Some(buf) = self.decode_line(bank, pick)
        {
            self.backup.line = Some((bank.clone(), pick, buf));
        }
    }

    /// 0x479200 (from the scene's skip, 0x43B460): the player pressed
    /// through the race start's scene (0x82BC49 = 1); the next
    /// [`Self::backup_update`] starts the backup line if it is armed.
    pub fn nis_button_through(&mut self) {
        self.backup.skipped = true;
    }

    /// 0x46CA10: the channel of the stream object's +0x48 stopped
    /// (0x7227CB), a kind-7 request freed, +0x4C = 0. That +0x48 is the
    /// backup line's own channel, so only a backup already playing stops,
    // prov: inferred audio.streams.cleanup_backup
    /// is **inferred** (its writer was not traced).
    fn cleanup_backup(&mut self, mixer: &Mixer) {
        if self.backup.line.take().is_some() && self.nis_kind == 7 {
            self.stop_nis(mixer);
        }
    }

    /// The NIS stream object's update 0x475330 after its kind-2 part: (1)
    /// with the scene skipped and the backup ready (decoded), the backup
    /// line replaces the NIS line (0x45AD40(7): the channel stopped, kind
    /// 7 plays; 0x82BC49 cleared); (2) with the race status object
    /// (0x890118) `race` = (its state +0, its clock +0x14 · 0.00025 s): in
    /// state 3 before 15 s, an armed backup is cleaned up once the speech
    /// stream (0x45A550 on the speech object's stream) is not playing;
    /// after 15 s (any state) an armed backup is cleaned up; either once
    /// per race start (+0xB1).
    pub fn backup_update(&mut self, mixer: &Mixer, race: Option<(i32, f32)>) {
        if self.backup.skipped
            && let Some((_, pick, buf)) = self.backup.line.as_ref()
            && buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner).done
        {
            let (pick, buf) = (*pick, buf.clone());
            let bank = self.backup.line.as_ref().map(|l| l.0.clone());
            self.stop_nis(mixer);
            if let Some(bank) = bank {
                self.lib.played(&bank, speech::BankId::Nis, pick);
            }
            self.nis_line = Some((buf.clone(), self.line_now()));
            self.nis_kind = 7;
            self.nis_track = Some(mixer.play_track(buf, 1.0, 2));
            self.backup.skipped = false;
        }
        let Some((state, secs)) = race else {
            return;
        };
        let armed = self.backup.line.is_some();
        let clean = if secs <= 15.0 {
            // fcom/jp at 0x4753F9: an unordered clock takes the other arm.
            !self.backup.cleaned && state == 3 && armed && !self.speech_stream_playing(mixer)
        } else {
            secs > 15.0 && !self.backup.cleaned && armed
        };
        if clean {
            self.cleanup_backup(mixer);
            self.backup.cleaned = true;
        }
    }

    /// 0x4754C0 (the NIS stream object's "NIS IS OVER"): an armed backup
    /// is cleaned up when the speech stream is not playing; with `purge`
    /// and a request on the NIS stream, the NIS stream is purged
    /// (0x46C9D0).
    pub fn nis_over(&mut self, mixer: &Mixer, purge: bool) {
        if self.backup.line.is_some() && !self.speech_stream_playing(mixer) {
            self.cleanup_backup(mixer);
            self.backup.cleaned = true;
        }
        if purge && self.nis_track.is_some() {
            self.stop_nis(mixer);
        }
    }

    /// The speech library's init (0x486220) with the game's draw
    /// Rand_Int(0xFFFFFFFF) on its generator 0x7EA624 (0x4866F0 /
    /// 0x486A2C): see [`speech::Library::init`]. Returns the seed.
    pub fn init_speech_library(&mut self, draw: u32) -> u32 {
        self.lib.init(draw)
    }

    /// Set a speech switch's tuple and play its line (the setters
    /// 0x46D150…0x46D580: 0x735880, then stream kind 3 through 0x46CA60 →
    /// 0x45B480 → 0x45AD40, which starts the queued request in place of
    /// what the speech stream plays). The switch is the setter's
    /// ([`speech::lines::switch_of`]) and its event is requested through
    /// the library ([`speech::Library::request`]); full volume, the speech
    /// level applies (`Mixer` kind 2). False without the bank, for a tuple
    /// no setter makes, or when the request plays nothing.
    pub fn say(&mut self, mixer: &Mixer, tuple: &[u32]) -> bool {
        let Some(bank) = self.speech.clone() else {
            return false;
        };
        let Some(ev) = speech::lines::switch_of(tuple).and_then(|s| bank.switch_event(s)) else {
            return false;
        };
        let Some(pick) = self.lib.request(&bank, speech::BankId::Speech, ev, tuple) else {
            return false;
        };
        if crate::env_set!("NFSU2_AUDIO_DEBUG") {
            eprintln!("audio: speech {tuple:X?} → event {:#x} entry {:#x}, record {}", pick.event, pick.entry, pick.record);
        }
        let Some(buf) = self.decode_line(&bank, pick) else {
            return false;
        };
        self.lib.played(&bank, speech::BankId::Speech, pick);
        self.stop_speech(mixer);
        self.speech_line = Some((buf.clone(), self.line_now()));
        self.speech_track = Some(mixer.play_track(buf, 1.0, 2));
        true
    }

    /// 0x45B490: the speech stream stopped (0x7227CB), before a front-end
    /// line (0x5243E0's cases).
    pub fn stop_speech(&mut self, mixer: &Mixer) {
        if let Some(id) = self.speech_track.take() {
            mixer.stop_track(id);
        }
    }

    /// The speech stream's channel as 0x722ECE / 0x722DB5 report it to
    /// 0x46C790 and 0x45A550: nothing queued or the stream gone (count 0, or
    /// state 3), queued but not yet playing (state 0 / 1: the clone's line
    /// still decoding) or playing (state 2) with the remaining ms, (end −
    /// pos) · 1000 / rate. A line past its wall-clock bound
    /// ([`within_length`]) counts as gone.
    fn speech_channel(&self, mixer: &Mixer) -> LineState {
        let (Some(id), Some((buf, start))) = (self.speech_track, self.speech_line.as_ref()) else {
            return LineState::Gone;
        };
        if !mixer.track_playing(id) || !within_length(buf, *start, self.line_now()) {
            return LineState::Gone;
        }
        let (frames, rate, done) = {
            let t = buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            (t.samples.len() / t.channels.max(1), t.rate.max(1), t.done)
        };
        if !done {
            return LineState::Queued;
        }
        if frames == 0 {
            return LineState::Gone;
        }
        let pos = mixer.track_pos(id).map_or(0.0, |p| p.0);
        LineState::Playing(((frames as f64 - pos).max(0.0) * 1000.0 / rate as f64) as i64)
    }

    /// The NIS stream object's +0x4C (the backup stream armed), which
    /// 0x46D100 tests after both readings: set at 0x48414E when the kind-7
    /// request is queued, cleared by 0x46CA10 ([`Backup`]).
    pub fn backup_armed(&self) -> bool {
        self.backup.line.is_some()
    }

    /// 0x45A550 on the speech object's stream, without the backup gate
    /// (the cleanups of 0x475330 / 0x4754C0 read it so).
    fn speech_stream_playing(&self, mixer: &Mixer) -> bool {
        matches!(self.speech_channel(mixer), LineState::Playing(_))
    }

    /// 0x46D100(0), Rachel may speak (0x525A00 / the test in 0x54BD40):
    /// with no speech object, yes; else 0x46C790 on the speech stream —
    /// nothing queued, or the one queued stream gone (state 3), or playing
    /// (state 2) with under 100 ms left; a stream queued but not started
    /// (state 0 / 1) or more than one queued: no — and no while the backup
    /// stream is armed. The NIS stream's own lines do not enter it
    /// (**verified** in the disassembly, 0x46D100 / 0x46C790 / 0x722ECE /
    /// 0x722DB5; the speech object's +0x5D, which answers "free" outright,
    /// is not identified and taken as clear).
    pub fn speech_free(&self, mixer: &Mixer) -> bool {
        if self.speech.is_none() {
            return true;
        }
        let ready = match self.speech_channel(mixer) {
            LineState::Gone => true,
            LineState::Queued => false,
            LineState::Playing(left_ms) => left_ms < 100,
        };
        ready && !self.backup_armed()
    }

    /// The speech line's and the NIS line's track gains (both the
    /// streams' volumes from the mix map, 0x46CB48 / 0x46CBAE).
    pub fn set_speech_gains(&self, mixer: &Mixer, speech: f32, nis: f32) {
        if let Some(id) = self.speech_track {
            mixer.set_track_gain(id, speech);
        }
        if let Some(id) = self.nis_track {
            mixer.set_track_gain(id, nis);
        }
    }

    /// 0x46D100(1), a speech line is playing: 0x45A550 on the speech
    /// stream (the channel's current stream in state 2, playing; a line
    /// still decoding is not), and not while the backup stream is armed
    /// (**verified** in the disassembly, as [`Self::speech_free`]).
    pub fn speech_playing(&self, mixer: &Mixer) -> bool {
        self.speech_stream_playing(mixer) && !self.backup_armed()
    }

    /// A NIS line is still playing (the clone's reading of 0x46C9A0, the
    // prov: inferred audio.streams.nis_busy
    /// stream object busy: **inferred**).
    pub fn nis_busy(&self, mixer: &Mixer) -> bool {
        self.nis_track.is_some_and(|id| mixer.track_playing(id))
            && self.nis_line.as_ref().is_none_or(|(buf, start)| within_length(buf, *start, self.line_now()))
    }

    /// 0x4810F0 → 0x46C8D0 / 0x481210 → 0x46C930 (**verified**; seen under
    /// Wine, 2026-11-01): the game's pause holds the speech
    /// stream (+4) and the NIS stream (+8) where they are (+0x2C = 1, the
    /// rate 0 by 0x7230A7) and the resume lets them go on (+0x2C = 0, rate
    /// 0x1000). A line held for 6 s ended 6 s later than played through.
    /// The lines' clocks (`within_length`) move by the held time.
    pub fn hold_speech(&mut self, mixer: &Mixer, on: bool) {
        if on == self.held.is_some() {
            return;
        }
        for id in [self.speech_track, self.nis_track].into_iter().flatten() {
            mixer.pause_track(id, on);
        }
        if on {
            self.held = Some(self.line_now());
        } else if let Some(since) = self.held.take() {
            let now = self.line_now();
            for (_, start) in [self.speech_line.as_mut(), self.nis_line.as_mut()].into_iter().flatten() {
                match (since, &mut *start, now) {
                    (LineStamp::Wall(since), LineStamp::Wall(start), _) => *start += since.elapsed(),
                    (LineStamp::Frames(since, _), LineStamp::Frames(start, _), LineStamp::Frames(now, _)) => {
                        *start += now.saturating_sub(since)
                    }
                    // The clock switched while held: nothing to shift by.
                    _ => {}
                }
            }
        }
    }

    /// 0x481210 with flow mode 3, 6 or 9 (the flow event 0xB, Return to
    /// Explore Mode from a race, passes 9; **verified**, 0x606740 case 0xB):
    /// with the speech banks loaded (0x82BA30 +0x14) both streams are
    /// stopped and their queued requests dropped (0x46C9D0 "PurgeStreams",
    /// 0x7227CB). Under Wine the ending scene's NIS line stopped as the
    /// post-race exit left for free roam (2026-11-01).
    pub fn purge_speech(&mut self, mixer: &Mixer) {
        self.stop_nis(mixer);
        self.stop_speech(mixer);
    }

    /// Rachel's line's track: its position (frames) and whether it is held.
    pub fn speech_pos(&self, mixer: &Mixer) -> Option<(f64, bool)> {
        self.speech_track.and_then(|id| mixer.track_pos(id))
    }

    /// The career start scene's line (CareerNis1, 0x484AF4).
    pub fn play_career_nis(&mut self, mixer: &Mixer) -> bool {
        self.play_nis(mixer, speech::nis::EVENT_END, &speech::nis::CAREER, 9)
    }

    pub fn stop_nis(&mut self, mixer: &Mixer) {
        if let Some(id) = self.nis_track.take() {
            mixer.stop_track(id);
        }
    }

    /// 0x481210 with 3 / 6 / 9 (the speech object's streams up, +0x14):
    /// the NIS stream (+8) and the speech stream (+4) purged (0x46C9D0:
    /// 0x7227CB stops the stream, the queued request is freed), at once,
    /// no fade. Its callers: UI_PostRace's destructor (0x4E9540, 3) on
    /// any exit from the post-race menu, and the flow's PAUSE state left
    /// with reason 3 (Restart), 6 (Quit to Main Menu, Back to World Map,
    /// Jump to Garage) or by message 0xB (9: Return to Explore Mode).
    /// Observed under Wine: a race's ending line stops as UI_PostRace is
    /// left by Restart or Quit to Main Menu, and plays on through Event
    /// Results and Post Race until then (`docs/formats.md`, "Race lines
    /// and the post-race exit").
    pub fn purge_lines(&mut self, mixer: &Mixer) {
        self.stop_nis(mixer);
        self.stop_speech(mixer);
    }

    /// Whether a NIS or speech line's track is still in the mix (a check
    /// aid for the tests: nothing of a race may outlive its exit).
    pub fn lines_active(&self, mixer: &Mixer) -> bool {
        [self.nis_track, self.speech_track].into_iter().flatten().any(|id| mixer.track_playing(id))
    }

    /// The NIS line's track and its stream kind, if one was started.
    pub fn nis_line(&self) -> Option<(u32, u8)> {
        self.nis_track.map(|id| (id, self.nis_kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rachel_is_free_as_0x46d100_reads_the_speech_channel() {
        // 0x46D100(0) → 0x46C790: free with nothing queued or the stream
        // gone, or playing with under 100 ms left; not while queued and not
        // yet playing (the line still decoding). 0x46D100(1) → 0x45A550:
        // playing only in state 2. A NIS line does not enter either.
        let mixer = Mixer::offline(48000);
        let mut s = Streams::new(std::path::Path::new("/nonexistent"));
        s.speech = Some(Arc::new(speech::Bank::default()));
        assert!(s.speech_free(&mixer) && !s.speech_playing(&mixer));
        let line = |s: &mut Streams, secs: f32, done: bool| {
            let n = (22050.0 * secs) as usize;
            let buf = Arc::new(Mutex::new(TrackPcm { channels: 1, rate: 22050, samples: vec![0.1; n], done }));
            s.speech_line = Some((buf.clone(), LineStamp::Wall(std::time::Instant::now())));
            s.speech_track = Some(mixer.play_track(buf.clone(), 1.0, 2));
            buf
        };
        let buf = line(&mut s, 1.0, false);
        assert!(!s.speech_free(&mixer) && !s.speech_playing(&mixer), "queued, not started");
        buf.lock().unwrap().done = true;
        assert!(!s.speech_free(&mixer) && s.speech_playing(&mixer), "1000 ms left");
        line(&mut s, 0.09, true);
        assert!(s.speech_free(&mixer) && s.speech_playing(&mixer), "90 ms left");
        s.stop_speech(&mixer);
        assert!(s.speech_free(&mixer) && !s.speech_playing(&mixer), "stopped");
        // A NIS line playing leaves Rachel free.
        let nis = Arc::new(Mutex::new(TrackPcm { channels: 1, rate: 22050, samples: vec![0.1; 22050], done: true }));
        s.nis_line = Some((nis.clone(), LineStamp::Wall(std::time::Instant::now())));
        s.nis_track = Some(mixer.play_track(nis, 1.0, 2));
        assert!(s.nis_busy(&mixer) && s.speech_free(&mixer));
    }

    #[test]
    fn the_backup_line_gates_rachel_and_is_cleaned_up_as_0x475330_does() {
        // +0x4C armed: 0x46D100 answers neither free nor busy.
        let mixer = Mixer::offline(48000);
        let mut s = Streams::new(std::path::Path::new("/nonexistent"));
        s.speech = Some(Arc::new(speech::Bank::default()));
        let pick = speech::Pick { event: speech::nis::EVENT_START, entry: 0x4023, record: 5 };
        let line = || {
            Arc::new(Mutex::new(TrackPcm { channels: 1, rate: 22050, samples: vec![0.1; 22050], done: true }))
        };
        let arm = |s: &mut Streams| {
            s.backup.line = Some((Arc::new(speech::Bank::default()), pick, line()));
            s.backup.cleaned = false;
        };
        arm(&mut s);
        assert!(!s.speech_free(&mixer) && !s.speech_playing(&mixer));
        // 0x4753FB: race status 3 within 15 s, the speech stream silent.
        s.backup_update(&mixer, Some((2, 5.0)));
        assert!(s.backup_armed(), "status 2: kept");
        s.backup_update(&mixer, Some((3, 5.0)));
        assert!(!s.backup_armed() && s.speech_free(&mixer));
        // Once per kind-2 start (+0xB1).
        s.backup.line = Some((Arc::new(speech::Bank::default()), pick, line()));
        s.backup_update(&mixer, Some((3, 20.0)));
        assert!(s.backup_armed());
        // After 15 s in any state.
        arm(&mut s);
        s.backup_update(&mixer, Some((2, 15.5)));
        assert!(!s.backup_armed());
        // Rachel speaking holds the early cleanup (0x45A550 on her stream).
        arm(&mut s);
        let rachel = line();
        s.speech_line = Some((rachel.clone(), LineStamp::Wall(std::time::Instant::now())));
        s.speech_track = Some(mixer.play_track(rachel, 1.0, 2));
        s.backup_update(&mixer, Some((3, 1.0)));
        assert!(s.backup_armed());
        // NIS IS OVER (0x4754C0) waits for her too.
        s.nis_over(&mixer, false);
        assert!(s.backup_armed());
        s.stop_speech(&mixer);
        // Button through (0x479200 → 0x45AD40(7)): the backup replaces
        // the NIS line, kind 7.
        let start = line();
        s.nis_line = Some((start.clone(), LineStamp::Wall(std::time::Instant::now())));
        s.nis_track = Some(mixer.play_track(start, 1.0, 2));
        s.nis_kind = 2;
        s.nis_button_through();
        s.backup_update(&mixer, None);
        assert_eq!(s.nis_kind, 7);
        assert!(s.nis_busy(&mixer) && s.backup_armed());
        // The cleanup stops a playing backup.
        s.nis_over(&mixer, false);
        assert!(!s.backup_armed() && !s.nis_busy(&mixer));
    }

    /// The tick form: on the mixer-frame clock a line is over at a
    /// fixed mixer frame, whatever the wall clock does, and a hold shifts
    /// it by the frames held; the wall form (the default) is unchanged.
    #[test]
    fn lines_end_on_the_mixer_frame_clock() {
        let mixer = Mixer::offline(48000);
        let mut s = Streams::new(std::path::Path::new("/nonexistent"));
        s.speech = Some(Arc::new(speech::Bank::default()));
        assert_eq!(s.line_clock, LineClock::Wall);
        // 1 s at 22050 Hz: over 1.5 s on, 72 000 frames at 48 kHz.
        let n = line_limit_frames(22050, 1, 22050, true, 48000);
        assert_eq!(n, 72_000);
        assert_eq!(line_limit_frames(22050, 1, 22050, false, 48000), 30 * 48000);
        // A 2-channel buffer of 0.3 s at 32 kHz at 44.1 kHz: the least N with
        // N / 44100 ≥ 0.8 (as f32).
        let m = line_limit_frames(2 * 9600, 2, 32000, true, 44100);
        assert!(((m as f64 / 44100.0) as f32) >= 0.8 && (((m - 1) as f64 / 44100.0) as f32) < 0.8);
        let start = 123_456u64;
        let at = |s: &mut Streams, f: u64| {
            s.set_line_clock(LineClock::Mixer { frames: f, rate: 48000 });
        };
        at(&mut s, start);
        let buf = Arc::new(Mutex::new(TrackPcm { channels: 1, rate: 22050, samples: vec![0.1; 22050], done: true }));
        s.speech_line = Some((buf.clone(), s.line_now()));
        s.speech_track = Some(mixer.play_track(buf.clone(), 1.0, 2));
        // The offline mixer is never pumped: the track stays at 0 and
        // "playing", so only the bound decides.
        for f in (start..start + n).step_by(800) {
            at(&mut s, f);
            assert!(s.speech_playing(&mixer), "frame {f}");
        }
        at(&mut s, start + n - 1);
        assert!(s.speech_playing(&mixer));
        at(&mut s, start + n);
        assert!(!s.speech_playing(&mixer) && s.speech_free(&mixer), "over at s + N");
        // Held for 10 000 frames: over 10 000 frames later.
        at(&mut s, start);
        s.speech_line = Some((buf.clone(), s.line_now()));
        at(&mut s, start + 1000);
        s.hold_speech(&mixer, true);
        at(&mut s, start + 11_000);
        s.hold_speech(&mixer, false);
        at(&mut s, start + n + 9_999);
        assert!(s.speech_playing(&mixer));
        at(&mut s, start + n + 10_000);
        assert!(!s.speech_playing(&mixer));
    }
}
