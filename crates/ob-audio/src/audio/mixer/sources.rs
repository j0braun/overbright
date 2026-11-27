use super::*;
use crate::audio::fe::Sample;
use crate::audio::gin::Granular;
use std::sync::{Arc, Mutex};

pub(super) struct Voice {
    /// The source that started it ([`stems`]).
    pub(super) tag: u16,
    pub(super) sample: Sample,
    /// The channel weights at the voice's azimuth ([`pan_weights`]).
    pub(super) pan: (f32, f32),
    pub(super) pos: f64,
    pub(super) step: f64,
    /// The step at pitch 0x1000 (the sample's rate over the device's).
    pub(super) base: f64,
    pub(super) gain: f32,
    /// Output frames still to wait before the sample starts.
    pub(super) wait: u32,
    /// A looping voice's handle (0: a one-shot). Loops restart at 0 (the
    /// car-effect samples' loop tags are 0 / length − 1).
    pub(super) id: u32,
    /// Whether the voice restarts at its end (a handled one-shot does not).
    pub(super) looped: bool,
    /// The gains last mixed (library channels 0 / 1, the mono output):
    /// the voice's current gains (+0x4), ramped to the targets (+0x1C)
    /// over a chunk's first 16 samples ([`mix_gain`]).
    pub(super) cur: [f32; 3],
    /// The last sample it gave (+0x3C), for the stop's fade ([`declick`]).
    pub(super) last: f32,
    /// Ended this chunk (removed after it).
    pub(super) done: bool,
    /// The reverb send gain (bus 0; the send byte / 127 × the volume,
    /// 0x729E24) before the effects level, and the current one.
    pub(super) send: f32,
    pub(super) cur_send: f32,
}

impl Voice {
    pub(super) fn new(sample: &Sample, base: f64, pitch: u16, gain: f32, wait: u32, id: u32, looped: bool) -> Voice {
        Voice {
            tag: stems::current(),
            sample: sample.clone(),
            pan: pan_weights(0),
            pos: 0.0,
            step: base * pitch as f64 / 4096.0,
            base,
            gain: unit_gain(gain),
            wait,
            id,
            looped,
            cur: [f32::NAN; 3],
            last: 0.0,
            done: false,
            send: 0.0,
            cur_send: f32::NAN,
        }
    }
}

/// The engine: the granular player's blocks, read out at the device's
/// rate.
pub(super) struct Engine {
    pub(super) player: Granular,
    pub(super) buf: Vec<i16>,
    pub(super) pos: f64,
    pub(super) step: f64,
    pub(super) gain: f32,
    /// The channel weights at the voice's azimuth.
    pub(super) pan: (f32, f32),
    /// The reverb send byte / 127 (0x723BC1, bus 0) and the dry "second
    /// volume" byte / 127 (0x723C41); the send is × the voice's volume
    /// (0x729E24), as `gain` is in the sample.
    pub(super) send: f32,
    pub(super) dry: f32,
    /// The current dry gains and send (see [`Voice::cur`]).
    pub(super) cur: [f32; 3],
    pub(super) cur_send: f32,
    pub(super) last: f32,
}

impl Engine {
    fn at(&mut self, k: usize) -> f32 {
        // Filled in place (no allocation in the callback).
        while self.buf.len() <= k {
            let at = self.buf.len();
            self.buf.resize(at + self.player.block(), 0);
            self.player.fill(&mut self.buf[at..]);
        }
        self.buf[k] as f32
    }

    pub(super) fn next(&mut self) -> f32 {
        let i = self.pos as usize;
        let f = (self.pos - i as f64) as f32;
        let (a, b) = (self.at(i), self.at(i + 1));
        self.pos += self.step;
        let used = self.pos as usize;
        if used > 4096 {
            self.buf.drain(..used);
            self.pos -= used as f64;
        }
        a + (b - a) * f
    }
}

/// A streamed source (a movie's soundtrack): interleaved samples in the
/// 16-bit range at their own rate, played as they arrive.
pub(super) struct Stream {
    /// The movie voice's volume (0x6FEFEF: the 0..127 byte × 1/127),
    /// set when the movie opens ([`movie_volume`]).
    pub(super) gain: f32,
    pub(super) channels: usize,
    pub(super) buf: std::collections::VecDeque<f32>,
    pub(super) pos: f64,
    pub(super) step: f64,
}

impl Stream {
    /// The source frame `i`'s channel `c` (the last channel for more).
    fn at(&self, i: usize, c: usize) -> f32 {
        let c = c.min(self.channels - 1);
        self.buf.get(i * self.channels + c).copied().unwrap_or(0.0)
    }

    pub(super) fn next(&mut self) -> (f32, f32) {
        let i = self.pos as usize;
        let f = (self.pos - i as f64) as f32;
        let lerp = |c| {
            let (a, b) = (self.at(i, c), self.at(i + 1, c));
            a + (b - a) * f
        };
        let out = (lerp(0), lerp(1));
        if (i + 1) * self.channels < self.buf.len() {
            self.pos += self.step;
        }
        let used = self.pos as usize;
        if used > 0 {
            self.buf.drain(..(used * self.channels).min(self.buf.len()));
            self.pos -= used as f64;
        }
        out
    }
}

/// A whole decoded stream (speech, a NIS line, a song): interleaved
/// samples in −1..1 that may still be growing while a decoder thread
/// appends to them; it ends when its buffer is done and played out.
pub struct TrackPcm {
    pub channels: usize,
    pub rate: u32,
    pub samples: Vec<f32>,
    pub done: bool,
}

pub type TrackBuf = Arc<Mutex<TrackPcm>>;

pub(super) struct Track {
    pub(super) buf: TrackBuf,
    pub(super) pos: f64,
    pub(super) step: f64,
    pub(super) gain: f32,
    pub(super) id: u32,
    pub(super) ended: bool,
    /// Held where it is (the PathFinder's pause, 0x739030 with 1).
    pub(super) paused: bool,
    /// [`Levels`] index: 2 speech, 3 music.
    pub(super) kind: usize,
    /// The current gain (× 32768: the library's 16-bit range; see
    /// [`Voice::cur`]) and the last frame (left, right).
    pub(super) cur: f32,
    pub(super) last: (f32, f32),
}
