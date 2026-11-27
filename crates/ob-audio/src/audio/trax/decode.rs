use super::Mpf;
use crate::formats::viv::Viv;

/// How a decoder thread spreads a stream's decode over its playing time
/// instead of decoding it whole at once (a song is 0.6–1.1 s of a full
/// core; on a small machine that burst competes with the frames): the
/// first `lead` seconds of the stream at once, then at most `speed`
/// seconds of the stream per second of wall time. The live mixer plays
/// one second per second, so the decoded part stays ahead of it by the
/// lead and more; the samples and the blocks they are appended in are
/// the same as an unpaced decode's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    /// Seconds of the stream decoded before any wait.
    pub lead: f64,
    /// Seconds of the stream decoded per second of wall time after that.
    pub speed: f64,
}

impl Pace {
    /// The live game's (a device mixer). An offline mix (the harness's
    /// captures) decodes unpaced: it renders faster than real time.
    pub const LIVE: Pace = Pace { lead: 6.0, speed: 4.0 };

    /// How long (s) to wait before decoding further with `decoded` seconds
    /// of the stream decoded `elapsed` seconds after the start (0: go on).
    pub fn wait(&self, decoded: f64, elapsed: f64) -> f64 {
        ((decoded - self.lead - elapsed * self.speed) / self.speed).max(0.0)
    }
}

/// Decode song `i` progressively into `buf` (run on a worker thread).
pub fn decode_song(viv: &Viv, mpf: &Mpf, i: usize, buf: &crate::audio::mixer::TrackBuf) -> Option<()> {
    decode_song_paced(viv, mpf, i, buf, None)
}

/// [`decode_song`], spread over the song's playing time with `pace`.
pub fn decode_song_paced(
    viv: &Viv,
    mpf: &Mpf,
    i: usize,
    buf: &crate::audio::mixer::TrackBuf,
    pace: Option<Pace>,
) -> Option<()> {
    decode_sample_paced(viv, mpf, mpf.song_sample(i)?, buf, pace)
}

/// Decode the `.mus` stream `s` (an index of the `.mpf`'s sample table)
/// progressively into `buf`.
pub fn decode_sample(viv: &Viv, mpf: &Mpf, s: usize, buf: &crate::audio::mixer::TrackBuf) -> Option<()> {
    decode_sample_paced(viv, mpf, s, buf, None)
}

/// [`decode_sample`], spread over the stream's playing time with `pace`.
pub fn decode_sample_paced(
    viv: &Viv,
    mpf: &Mpf,
    s: usize,
    buf: &crate::audio::mixer::TrackBuf,
    pace: Option<Pace>,
) -> Option<()> {
    let started = std::time::Instant::now();
    let (off, _) = *mpf.samples.get(s)?;
    let next = mpf.samples.get(s + 1).map(|n| n.0);
    let e = viv.find("pfdata/MusicSFx.mus")?;
    let len = next.unwrap_or(e.size).saturating_sub(off) as usize;
    let d = viv.read_range(e, off, len)?;
    let len = crate::audio::schl::stream_len(&d)?;
    let h = crate::audio::schl::header(&d)?;
    let ch = h.channels.max(1) as usize;
    {
        // The whole song's room up front (the header's sample count and a
        // frame's slack), made outside the lock: a growing buffer would
        // otherwise be moved while the lock is held, which the mixer's
        // callback waits on.
        let room = Vec::with_capacity((h.samples.min(48_000 * 60 * 20) as usize + 2304) * ch);
        let mut b = buf.lock().ok()?;
        b.rate = h.rate;
        b.channels = h.channels as usize;
        if b.samples.is_empty() {
            b.samples = room;
        }
    }
    let mut dec = crate::audio::ealayer3::Decoder::new();
    let mut out = Vec::new();
    for (o, size) in crate::audio::schl::blocks(&d[..len], &h) {
        let mut p = o + 0xC + 4 * ch;
        while p < o + size {
            match dec.frame(&d[p..o + size], ch, &mut out) {
                Some(n) if n > 0 => p += n,
                _ => break,
            }
        }
        if out.len() >= 32000 * ch {
            let decoded = {
                let mut b = buf.lock().ok()?;
                b.samples.extend(out.drain(..).map(|s| s.clamp(-1.0, 1.0)));
                (b.samples.len() / ch) as f64 / f64::from(h.rate.max(1))
            };
            if Arc_strong_one(buf) {
                return None;
            }
            if let Some(pace) = pace {
                // In short sleeps, so a dropped song's decoder ends soon.
                loop {
                    let wait = pace.wait(decoded, started.elapsed().as_secs_f64());
                    if wait <= 0.0 {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs_f64(wait.min(0.05)));
                    if Arc_strong_one(buf) {
                        return None;
                    }
                }
            }
        }
    }
    let mut b = buf.lock().ok()?;
    b.samples.extend(out.drain(..).map(|s| s.clamp(-1.0, 1.0)));
    b.done = true;
    Some(())
}

/// Nobody else holds the buffer any more: the song was dropped.
#[allow(non_snake_case)]
fn Arc_strong_one(buf: &crate::audio::mixer::TrackBuf) -> bool {
    std::sync::Arc::strong_count(buf) == 1
}
