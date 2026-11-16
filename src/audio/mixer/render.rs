use super::*;
use crate::audio::fe::Sample;
use crate::audio::gin::Granular;

/// The option volumes the mix applies (0x45C210 makes them group-1
/// class-3 fields of the mix map, each × master × 32767: field 1 speech
/// (+4), 2 menu music (+8), 4 effects (+0x10), 5 engine (+0x14)): the
/// effects scale the one-shots and loops, the engine the engine voices,
/// speech the speech tracks and music the music tracks.
#[derive(Clone, Copy, Debug)]
pub struct Levels {
    pub effects: f32,
    pub engine: f32,
    pub speech: f32,
    pub music: f32,
}

impl Default for Levels {
    fn default() -> Levels {
        Levels { effects: 1.0, engine: 1.0, speech: 1.0, music: 1.0 }
    }
}

impl Levels {
    fn get(&self, kind: usize) -> f32 {
        [self.effects, self.engine, self.speech, self.music][kind.min(3)]
    }
}

/// What the stream plays; `render` fills interleaved frames.
pub struct Mix {
    pub(super) rate: u32,
    pub(super) voices: Vec<Voice>,
    /// The player engine's granular voices: the accel GIN and the decel
    /// `_DCL` GIN (0x474030's +0xE0 / +0xFC).
    engines: [Option<Engine>; 2],
    stream: Option<Stream>,
    tracks: Vec<Track>,
    pub levels: Levels,
    next_id: u32,
    /// The largest |sample| rendered since [`Mixer::take_peak`] (a check aid).
    pub(super) peak: f32,
    /// The loaded FX graph (SFXObj_Reverb's handle 0x82B08C) and its bus
    /// (0x8B47C4[0]), filled by the voices' sends each chunk.
    fx: Option<crate::audio::reverb::Graph>,
    bus: Vec<f32>,
    /// The largest |wet sample| since [`Mixer::take_fx_peak`] (a check aid).
    pub(super) fx_peak: f32,
    /// The stopped voices' last samples × their gains, faded out at the
    /// next chunk's start (0x728FD2 → 0x7292BC): library channels 0 / 1
    /// and the mono output (0x8B490C[c]), and the bus (0x8B4924[0]).
    acc: [f32; 3],
    acc_bus: f32,
    /// Frames rendered (a check aid's clock).
    rendered: u64,
    /// A check aid: the stops so far (frame, the largest |last sample ×
    /// gain| on a channel: the step a cut would leave), at most
    /// [`STOP_LOG`] (preallocated: the callback never allocates).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(super) stops: Vec<(u64, f32)>,
    /// A check aid ([`stems`]): each source's output energy (sum of
    /// squares, both channels, in the library's 16-bit scale).
    pub(super) stem_energy: Vec<f64>,
}

/// The stop log's capacity.
#[cfg(any(test, feature = "dev-harness"))]
pub(super) const STOP_LOG: usize = 1024;

impl Mix {
    /// See `Mixer::dev_posted` (dev harness).
    #[cfg(feature = "dev-harness")]
    pub(super) fn dev_posted(&self) -> Vec<u64> {
        let mut v = vec![u64::from(self.next_id), self.voices.len() as u64, self.tracks.len() as u64];
        v.extend(self.engines.iter().map(|e| u64::from(e.is_some())));
        for x in &self.voices {
            v.push(u64::from(x.tag) << 32 | u64::from(x.id));
            v.push(u64::from(x.gain.to_bits()) << 32 | u64::from(x.send.to_bits()));
            v.push(x.step.to_bits());
            v.push(u64::from(x.pan.0.to_bits()) << 32 | u64::from(x.pan.1.to_bits()) | u64::from(x.looped) << 63);
        }
        v
    }

    pub fn new(rate: u32) -> Mix {
        Mix {
            rate,
            voices: Vec::new(),
            engines: [None, None],
            stream: None,
            tracks: Vec::new(),
            levels: Levels::default(),
            next_id: 0,
            peak: 0.0,
            fx: None,
            bus: vec![0.0; crate::audio::reverb::BLOCK],
            fx_peak: 0.0,
            acc: [0.0; 3],
            acc_bus: 0.0,
            rendered: 0,
            #[cfg(any(test, feature = "dev-harness"))]
            stops: Vec::with_capacity(STOP_LOG),
            stem_energy: Vec::new(),
        }
    }

    /// Start a looping voice at a gain (0–1); its handle for
    /// [`Mix::set_gain`] and [`Mix::stop`].
    pub fn play_loop(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let base = sample.0 as f64 / self.rate as f64;
        self.voices.push(Voice::new(sample, base, pitch, gain, 0, self.next_id, true));
        self.next_id
    }

    /// Start a one-shot with a handle, so its gain can follow a parameter;
    /// it ends with its sample.
    pub fn play_handle(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let base = sample.0 as f64 / self.rate as f64;
        self.voices.push(Voice::new(sample, base, pitch, gain, 0, self.next_id, false));
        self.next_id
    }

    pub fn set_gain(&mut self, id: u32, gain: f32) {
        for v in self.voices.iter_mut().filter(|v| v.id == id) {
            v.gain = unit_gain(gain);
        }
    }

    /// A handled voice's reverb send gain (0–1, bus 0).
    pub fn set_send(&mut self, id: u32, gain: f32) {
        for v in self.voices.iter_mut().filter(|v| v.id == id) {
            v.send = unit_gain(gain);
        }
    }

    /// A handled voice's pitch (0x1000 = the recorded rate; the library's
    /// 0x724D0E: step = rate · 16 / output rate · pitch, 16.16 fixed).
    pub fn set_pitch(&mut self, id: u32, pitch: u16) {
        for v in self.voices.iter_mut().filter(|v| v.id == id) {
            v.step = v.base * pitch as f64 / 4096.0;
        }
    }

    /// Stop a handled voice: its last sample fades out over the next
    /// chunk's first 16 samples ([`declick`]).
    pub fn stop(&mut self, id: u32) {
        let mut acc = self.acc;
        let mut step = 0f32;
        self.voices.retain(|v| {
            if v.id != id {
                return true;
            }
            declick(&mut acc, v.cur, v.last);
            if v.cur_send.is_finite() {
                self.acc_bus += v.cur_send * v.last;
            }
            step = step.max(v.cur.iter().map(|g| (g * v.last).abs()).fold(0.0, f32::max));
            false
        });
        self.acc = acc;
        self.log_stop(self.rendered, step);
    }

    /// A handled voice's azimuth word (the player parameter kind 3, the
    /// library's 0x7208D8: the angle word rec +0x1C → the pan tables'
    /// row, [`pan_weights`]).
    pub fn set_pan(&mut self, id: u32, azimuth: u16) {
        for v in self.voices.iter_mut().filter(|v| v.id == id) {
            v.pan = pan_weights(azimuth);
        }
    }

    /// Engine voice `slot`'s azimuth (0x473E60 → 0x724093 with the
    /// object's slot 0x3C angle).
    pub fn engine_pan(&mut self, slot: usize, azimuth: u16) {
        if let Some(e) = self.engines.get_mut(slot).and_then(|e| e.as_mut()) {
            e.pan = pan_weights(azimuth);
        }
    }

    /// Whether a looping voice is playing.
    pub fn playing(&self, id: u32) -> bool {
        self.voices.iter().any(|v| v.id == id)
    }

    /// Voices sounding (one-shots and loops).
    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// Start a one-shot at a volume byte (0–0x7F) and pitch (0x1000 = 1).
    pub fn play(&mut self, sample: &Sample, volume: u8, pitch: u16) {
        self.play_after(sample, volume.min(0x7F) as f32 / 127.0, pitch, 0.0);
    }

    /// Start a one-shot at a gain (0–1) and pitch (0x1000 = 1) after a
    /// delay in seconds.
    pub fn play_after(&mut self, sample: &Sample, gain: f32, pitch: u16, delay: f32) {
        let base = sample.0 as f64 / self.rate as f64;
        let wait = (delay.max(0.0) * self.rate as f32) as u32;
        self.voices.push(Voice::new(sample, base, pitch, gain, wait, 0, false));
    }

    /// The device's rate, once its stream is open (the mix starts at a
    /// guessed 48 kHz): the voices', engines' and stream's steps follow
    /// (their source rates over the new one); the FX graph, built for a
    /// rate, is reloaded by its owner ([`Mixer::rate`]).
    pub fn set_rate(&mut self, rate: u32) {
        if rate == 0 || rate == self.rate {
            return;
        }
        let k = f64::from(self.rate) / f64::from(rate);
        for v in &mut self.voices {
            v.base *= k;
            v.step *= k;
            v.wait = (f64::from(v.wait) / k) as u32;
        }
        for e in self.engines.iter_mut().flatten() {
            e.step *= k;
        }
        if let Some(st) = self.stream.as_mut() {
            st.step *= k;
        }
        self.rate = rate;
    }

    /// [`Mix::play_after`] with a handle (for [`Mix::stop`] /
    /// [`Mix::playing`]); it ends with its sample.
    pub fn play_handle_after(&mut self, sample: &Sample, gain: f32, pitch: u16, delay: f32) -> u32 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let base = sample.0 as f64 / self.rate as f64;
        let wait = (delay.max(0.0) * self.rate as f32) as u32;
        self.voices.push(Voice::new(sample, base, pitch, gain, wait, self.next_id, false));
        self.next_id
    }

    /// Put a granular player in engine voice `slot` (0 accel, 1 decel).
    pub fn set_engine(&mut self, slot: usize, player: Option<Granular>) {
        let rate = self.rate as f64;
        if let Some(e) = self.engines.get_mut(slot) {
            // The old voice's stop (0x728FD2): its last sample into the
            // channel and bus accumulators.
            if let Some(old) = e.as_ref() {
                declick(&mut self.acc, old.cur, old.last);
                if !old.cur_send.is_nan() {
                    self.acc_bus += old.cur_send * old.last;
                }
                #[cfg(any(test, feature = "dev-harness"))]
                let step = old.cur.iter().map(|g| (g * old.last).abs()).fold(0.0, f32::max);
                #[cfg(any(test, feature = "dev-harness"))]
                if step > 0.0 && step.is_finite() && self.stops.len() < STOP_LOG {
                    self.stops.push((self.rendered, step / 32768.0 * MASTER));
                }
            }
            *e = player.map(|p| Engine {
                step: p.rate() as f64 / rate,
                player: p,
                buf: Vec::new(),
                pos: 0.0,
                gain: 0.0,
                pan: pan_weights(0),
                send: 0.0,
                dry: 1.0,
                cur: [f32::NAN; 3],
                cur_send: f32::NAN,
                last: 0.0,
            });
        }
    }

    /// Aim engine voice `slot` at an rpm at a volume (0–1). The engine
    /// sound's three SetRPM calls (0x473FD3, 0x4741A3, 0x4742F5) all glide
    /// over 60 ms.
    pub fn engine(&mut self, slot: usize, rpm: f32, volume: f32) {
        if let Some(e) = self.engines.get_mut(slot).and_then(|e| e.as_mut()) {
            e.player.set_rpm(rpm, 60.0);
            e.gain = volume;
        }
    }

    /// Engine voice `slot`'s reverb send and dry bytes (0–0x7F; 0x473F0E /
    /// 0x473ECF, [`super::reverb::ReverbCtl::engine_bytes`]).
    pub fn engine_send(&mut self, slot: usize, send: u8, dry: u8) {
        if let Some(e) = self.engines.get_mut(slot).and_then(|e| e.as_mut()) {
            e.send = send.min(0x7F) as f32 / 127.0;
            e.dry = dry.min(0x7F) as f32 / 127.0;
        }
    }

    /// Load (or with `None` unload) the FX graph (0x721A3C / 0x7219EC).
    pub fn set_fx(&mut self, graph: Option<crate::audio::reverb::Graph>) {
        self.fx = graph;
    }

    /// Whether the FX graph ran in the last chunk (a check aid).
    pub fn fx_running(&self) -> bool {
        self.fx.as_ref().is_some_and(|g| g.running())
    }

    /// Start (or with `None` stop) a streamed source of `channels` at
    /// `rate` Hz.
    pub fn set_stream(&mut self, format: Option<(u8, u32)>) {
        self.stream = format.map(|(channels, rate)| Stream {
            gain: 1.0,
            channels: channels.max(1) as usize,
            buf: Default::default(),
            pos: 0.0,
            step: rate as f64 / self.rate as f64,
        });
    }

    /// The streamed source's volume (0..1; [`movie_volume`]).
    pub fn set_stream_gain(&mut self, gain: f32) {
        if let Some(s) = self.stream.as_mut() {
            s.gain = unit_gain(gain);
        }
    }

    /// Queue interleaved samples on the streamed source.
    pub fn push_stream(&mut self, samples: &[f32]) {
        if let Some(s) = self.stream.as_mut() {
            s.buf.extend(samples.iter().copied());
        }
    }

    /// Start a track at a gain (0–1) under level `kind` (2 speech, 3
    /// music); its handle.
    pub fn play_track(&mut self, buf: TrackBuf, gain: f32, kind: usize) -> u32 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let rate = buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner).rate.max(1);
        self.tracks.push(Track {
            buf,
            pos: 0.0,
            step: rate as f64 / self.rate as f64,
            gain: unit_gain(gain),
            id: self.next_id,
            ended: false,
            paused: false,
            kind,
            cur: f32::NAN,
            last: (0.0, 0.0),
        });
        self.next_id
    }

    pub fn set_track_gain(&mut self, id: u32, gain: f32) {
        for t in self.tracks.iter_mut().filter(|t| t.id == id) {
            t.gain = unit_gain(gain);
        }
    }

    /// Stop a track, its last frame faded out as a voice's ([`Mix::stop`]).
    pub fn stop_track(&mut self, id: u32) {
        let mut acc = self.acc;
        let mut step = 0f32;
        self.tracks.retain(|t| {
            if t.id != id {
                return true;
            }
            declick_track(&mut acc, t.cur, t.last);
            step = (t.cur * t.last.0).abs().max((t.cur * t.last.1).abs());
            false
        });
        self.acc = acc;
        let frame = self.rendered;
        self.log_stop(frame, step);
    }

    /// Hold a track where it is, or let it go on.
    pub fn pause_track(&mut self, id: u32, paused: bool) {
        for t in self.tracks.iter_mut().filter(|t| t.id == id) {
            t.paused = paused;
        }
    }

    pub fn track_playing(&self, id: u32) -> bool {
        self.tracks.iter().any(|t| t.id == id)
    }

    /// A track's position (its stream's frames) and whether it is held.
    pub fn track_pos(&self, id: u32) -> Option<(f64, bool)> {
        self.tracks.iter().find(|t| t.id == id).map(|t| (t.pos, t.paused))
    }

    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let ch = channels.max(1);
        // The library mixes in chunks of at most 256 frames (0x7295A8) and
        // runs the FX graph after each (0x7294CC).
        for chunk in out.chunks_mut(ch * crate::audio::reverb::BLOCK) {
            self.render_chunk(chunk, ch);
            self.rendered += (chunk.len() / ch) as u64;
        }
        self.peak = out.iter().fold(self.peak, |m, s| m.max(s.abs()));
    }

    /// One chunk (≤ 256 frames, 0x7292BC): the stopped voices' fade-outs,
    /// then each voice's samples into the two library channels (and the
    /// mono output) and the bus at its gains ([`mix_gain`]), the FX
    /// graph's output added, then the device frames.
    fn render_chunk(&mut self, out: &mut [f32], channels: usize) {
        const B: usize = crate::audio::reverb::BLOCK;
        let n = out.len() / channels;
        let stereo = channels >= 2;
        let mut lib = [[0f32; B]; 2];
        let mut mono = [0f32; B];
        let fx_on = self.fx.is_some();
        // 0x7292BC: the accumulators of the voices stopped since the last
        // chunk fade out into the bus (with a graph, 0x8B4804[0]) and the
        // channel buffers before any voice is mixed.
        {
            let [l0, l1] = &mut lib;
            fade_out(&mut l0[..n], &mut self.acc[0]);
            fade_out(&mut l1[..n], &mut self.acc[1]);
            fade_out(&mut mono[..n], &mut self.acc[2]);
        }
        let bus = &mut self.bus[..n];
        bus.fill(0.0);
        if fx_on {
            fade_out(bus, &mut self.acc_bus);
        }
        let mut sent = false;
        let mut src = [0f32; B];
        let mut acc = self.acc;
        let effects = self.levels.effects;
        for v in &mut self.voices {
            let start = (v.wait as usize).min(n);
            v.wait -= start as u32;
            if start == n {
                continue;
            }
            let pcm = &v.sample.1;
            let mut end = n;
            for (k, s) in src[start..n].iter_mut().enumerate() {
                let i = v.pos as usize;
                if i + 1 >= pcm.len() {
                    end = start + k;
                    break;
                }
                let f = (v.pos - i as f64) as f32;
                let x = pcm[i] + (pcm[i + 1] - pcm[i]) * f;
                // A voice gone non-finite (a NaN pitch) would poison the
                // whole mix for good: dropped, and named once.
                if !x.is_finite() || !v.step.is_finite() {
                    warn_once(&BAD_VOICE, || {
                        format!(
                            "dropped a voice with a non-finite sample (id {}, {} samples, gain {}, step {}, pos {})",
                            v.id,
                            pcm.len(),
                            v.gain,
                            v.step,
                            v.pos
                        )
                    });
                    v.looped = false;
                    v.pos = pcm.len() as f64;
                    v.last = 0.0;
                    end = start + k;
                    break;
                }
                *s = x;
                v.pos += v.step;
                if v.looped && (v.pos as usize) + 1 >= pcm.len() && pcm.len() > 1 {
                    v.pos -= (pcm.len() - 1) as f64;
                }
            }
            let g = if stems::enabled() && !stems::keep(v.tag) { 0.0 } else { v.gain * effects };
            let tgt = [g * v.pan.0, g * v.pan.1, g * (127.0 / 255.0)];
            if end > start {
                let s = &src[start..end];
                if stems::enabled() {
                    stems::add(&mut self.stem_energy, v.tag, s, (tgt[0], tgt[1]));
                }
                if stereo {
                    let [l0, l1] = &mut lib;
                    mix_gain(&mut l0[start..end], s, &mut v.cur[0], tgt[0]);
                    mix_gain(&mut l1[start..end], s, &mut v.cur[1], tgt[1]);
                    v.cur[2] = tgt[2];
                } else {
                    mix_gain(&mut mono[start..end], s, &mut v.cur[2], tgt[2]);
                    v.cur[..2].copy_from_slice(&tgt[..2]);
                }
                // The send (bus 0) only with a graph; see the engines'.
                if fx_on {
                    let ts = if g == 0.0 { 0.0 } else { v.send * effects };
                    let ramped = mix_gain(&mut bus[start..end], s, &mut v.cur_send, ts);
                    if ts != 0.0 && (!ramped || end - start > RAMP) {
                        sent = true;
                    }
                }
                v.last = src[end - 1];
            }
            if end < n {
                // The voice's end (its pull gives no more, 0x7293C9 →
                // 0x7292AC): the stop's fade.
                v.done = true;
                if v.cur[0].is_nan() {
                    v.cur = tgt;
                }
                declick(&mut acc, v.cur, v.last);
                if v.cur_send.is_finite() {
                    self.acc_bus += v.cur_send * v.last;
                }
                #[cfg(any(test, feature = "dev-harness"))]
                let step = v.cur.iter().map(|g| (g * v.last).abs()).fold(0.0, f32::max);
                #[cfg(any(test, feature = "dev-harness"))]
                if step > 0.0 && step.is_finite() && self.stops.len() < STOP_LOG {
                    self.stops.push((self.rendered + end as u64, step / 32768.0 * MASTER));
                }
            }
        }
        self.voices.retain(|v| !v.done);
        let engine_level = self.levels.engine;
        for (slot, e) in self.engines.iter_mut().enumerate() {
            let Some(e) = e.as_mut() else { continue };
            let tag = if stems::enabled() {
                stems::index(if !stems::split() { "GIN" } else if slot == 0 { "GIN.accel" } else { "GIN.decel" })
            } else {
                0
            };
            for s in &mut src[..n] {
                let x = e.next();
                *s = if x.is_finite() {
                    x
                } else {
                    warn_once(&BAD_ENGINE, || format!("engine voice gave a non-finite sample (level {engine_level})"));
                    0.0
                };
            }
            e.last = src[n - 1];
            let g = if !(e.gain * engine_level).is_finite() || (stems::enabled() && !stems::keep(tag)) {
                0.0
            } else {
                e.gain * engine_level
            };
            let d = g * e.dry;
            let tgt = [d * e.pan.0, d * e.pan.1, d * (127.0 / 255.0)];
            let s = &src[..n];
            if stems::enabled() {
                stems::add(&mut self.stem_energy, tag, s, (tgt[0], tgt[1]));
            }
            if stereo {
                let [l0, l1] = &mut lib;
                mix_gain(&mut l0[..n], s, &mut e.cur[0], tgt[0]);
                mix_gain(&mut l1[..n], s, &mut e.cur[1], tgt[1]);
                e.cur[2] = tgt[2];
            } else {
                mix_gain(&mut mono[..n], s, &mut e.cur[2], tgt[2]);
                e.cur[..2].copy_from_slice(&tgt[..2]);
            }
            // The send (bus 0) only with a graph (0x728BD7 / 0x729458);
            // the idle count resets when the chunk's samples after the
            // ramp go in at a nonzero send (0x729499).
            if fx_on {
                let ts = g * e.send;
                let ramped = mix_gain(bus, s, &mut e.cur_send, ts);
                if ts != 0.0 && (!ramped || n > RAMP) {
                    sent = true;
                }
            }
        }
        // The streamed source (a movie's soundtrack) at the movie's
        // volume, each channel a library voice panned as a stream's
        // ([`stream_pan`]).
        if let Some(st) = self.stream.as_mut() {
            let (wl, wr) = stream_pan(st.channels);
            let g = st.gain;
            for k in 0..n {
                let (l, r) = st.next();
                let (l, r) = if l.is_finite() && r.is_finite() { (l * g, r * g) } else { (0.0, 0.0) };
                mono[k] += (l + r) * 0.5;
                // pan.0 (the 90° speaker, library channel 0) right, pan.1
                // (270°) left.
                lib[0][k] += r * wr;
                lib[1][k] += l * wl;
            }
        }
        self.render_tracks(&mut lib, &mut mono, n, stereo, &mut acc);
        self.acc = acc;
        if let Some(g) = self.fx.as_mut() {
            let [c0, c1] = &mut lib;
            let (mut w0, mut w1) = ([0f32; B], [0f32; B]);
            if g.process(&self.bus[..n], sent, [&mut w0[..n], &mut w1[..n]]) && (!stems::enabled() || stems::keep_reverb()) {
                if stems::enabled() {
                    let fx = stems::index("reverb");
                    stems::add(&mut self.stem_energy, fx, &w0[..n], (1.0, 0.0));
                    stems::add(&mut self.stem_energy, fx, &w1[..n], (1.0, 0.0));
                }
                for k in 0..n {
                    let (a, b) =
                        (if w0[k].is_finite() { w0[k] } else { 0.0 }, if w1[k].is_finite() { w1[k] } else { 0.0 });
                    c0[k] += a;
                    c1[k] += b;
                    self.fx_peak = self.fx_peak.max(a.abs()).max(b.abs());
                    mono[k] += (a + b) * 0.5;
                }
            }
        }
        let conv = |v: f32| if v.is_finite() { (v / 32768.0 * MASTER).clamp(-1.0, 1.0) } else { 0.0 };
        for (k, frame) in out.chunks_mut(channels).enumerate() {
            if stereo {
                // A device with more than two channels gets the library's
                // two channels on its front pair and silence elsewhere
                // (the game mixes for two speakers, 0x8B46E8 = 2; Windows
                // does not upmix a stereo DirectSound buffer).
                frame.fill(0.0);
                frame[0] = conv(lib[1][k]);
                frame[1] = conv(lib[0][k]);
            } else {
                frame.fill(conv(mono[k]));
            }
        }
    }

    /// The tracks' frames of this chunk into the library channels (left
    /// into channel 1, right into 0) or the mono output, at their gains
    /// ramped as a voice's ([`mix_gain`]).
    /// Whether every playing track has `frames` output frames decoded
    /// past its position (or its decoder is done).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(super) fn tracks_ready(&self, frames: usize) -> bool {
        self.tracks.iter().filter(|t| !t.paused).all(|t| {
            let pcm = t.buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let need = t.pos + f64::from(pcm.rate.max(1)) / f64::from(self.rate) * frames as f64 + 2.0;
            pcm.done || (pcm.samples.len() / pcm.channels.max(1)) as f64 >= need
        })
    }

    fn render_tracks(
        &mut self,
        lib: &mut [[f32; crate::audio::reverb::BLOCK]; 2],
        mono: &mut [f32],
        n: usize,
        stereo: bool,
        acc: &mut [f32; 3],
    ) {
        let levels = self.levels;
        let rate = self.rate;
        let (mut sl, mut sr) = ([0f32; crate::audio::reverb::BLOCK], [0f32; crate::audio::reverb::BLOCK]);
        for t in self.tracks.iter_mut().filter(|t| !t.paused) {
            let solo = !stems::enabled() || stems::keep(stems::index(if t.kind == 2 { "speech" } else { "music" }));
            let tgt = if solo { t.gain * levels.get(t.kind) * 32768.0 } else { 0.0 };
            let channels = t.buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner).channels.max(1);
            let (wl, wr) = stream_pan(channels);
            // A decoder that died holding the lock leaves it poisoned: the
            // samples it put in still play and the track still ends.
            let pcm = t.buf.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            // The decoder sets the stream's own rate after the track starts
            // (a buffer starts at 32000 Hz): the step follows it.
            t.step = f64::from(pcm.rate.max(1)) / f64::from(rate);
            let c = pcm.channels.max(1);
            let frames = pcm.samples.len() / c;
            let mut end = n;
            for k in 0..n {
                let i = t.pos as usize;
                if i + 1 >= frames {
                    if pcm.done {
                        t.ended = true;
                    }
                    end = k;
                    break;
                }
                let f = (t.pos - i as f64) as f32;
                let at = |k: usize, ch: usize| pcm.samples[k * c + ch.min(c - 1)];
                let (l, r) = (at(i, 0) + (at(i + 1, 0) - at(i, 0)) * f, at(i, 1) + (at(i + 1, 1) - at(i, 1)) * f);
                let ok = l.is_finite() && r.is_finite();
                if !ok {
                    warn_once(&BAD_TRACK, || "a stream track gave a non-finite sample".to_string());
                }
                sl[k] = if ok { l } else { 0.0 };
                sr[k] = if ok { r } else { 0.0 };
                t.pos += t.step;
            }
            drop(pcm);
            if end > 0 && stems::enabled() {
                let tag = stems::index(if t.kind == 2 { "speech" } else { "music" });
                stems::add(&mut self.stem_energy, tag, &sl[..end], (tgt * wl, 0.0));
                stems::add(&mut self.stem_energy, tag, &sr[..end], (tgt * wr, 0.0));
            }
            if end > 0 {
                if stereo {
                    // The gain ramps as one value (`cur`); each channel's
                    // weight is applied to its samples.
                    for k in 0..end {
                        sl[k] *= wl;
                        sr[k] *= wr;
                    }
                    let mut c1 = t.cur;
                    mix_gain(&mut lib[1][..end], &sl[..end], &mut c1, tgt);
                    mix_gain(&mut lib[0][..end], &sr[..end], &mut t.cur, tgt);
                } else {
                    for k in 0..end {
                        sl[k] = (sl[k] + sr[k]) * 0.5;
                    }
                    mix_gain(&mut mono[..end], &sl[..end], &mut t.cur, tgt);
                    sr[end - 1] = sl[end - 1];
                }
                t.last = (sl[end - 1], sr[end - 1]);
            }
            if t.ended {
                if t.cur.is_nan() {
                    t.cur = tgt;
                }
                declick_track(acc, t.cur, t.last);
            }
        }
        self.tracks.retain(|t| !t.ended);
    }

    /// Voices, tracks and the levels, for a debug line.
    pub fn debug_state(&self) -> String {
        format!(
            "voices {} tracks {} engines {} levels {:?} track gains {:?}",
            self.voices.len(),
            self.tracks.len(),
            self.engines.iter().flatten().count(),
            self.levels,
            self.tracks.iter().map(|t| (t.kind, t.gain * 127.0)).collect::<Vec<_>>()
        )
    }
}
