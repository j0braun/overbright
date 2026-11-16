/// The output scale: 1, as SndMix_Render (0x729574) mixes into 16-bit
/// channel buffers that 0x72CD6A only interleaves into the device buffer
/// (no headroom; a loud mix saturates). Under Wine the FE clicks (the
/// same PlayCommonSample block {id, 4235, 0x1000, 0}) measured ~6 dB above
/// the clone's old 0.5 (soundcmp, 2026-11-01).
pub(super) const MASTER: f32 = 1.0;

/// The library's two-channel pan law (0x72559B with N_t = 2, speakers at
/// 90° and 270°, 0x7258E6): row b = the azimuth word's high byte, θ = b ·
/// 360 / 256, d = θ + 90 (θ ≤ 90), 270 − θ (θ < 270) or θ − 270, T = d ·
/// π / 360, and the channel weights round(255 · sin T) / 255 (channel 0,
/// the 90° speaker) and round(255 · cos T) / 255 (channel 1) (K2 =
/// 1/255, 0x724FC5). The azimuth word (0x45DE60, `mixmap::azimuth`) is
/// 0x4000 for a point on the listener's right (n.x · dir.y − n.y · dir.x
/// ≥ 0 in the game's right-handed, z-up world), so channel 0, the 90°
/// speaker, is the right one: it goes to the device's channel 1 and
/// channel 1 to channel 0 (the library's interleave was not traced; the
/// side measured under Wine in round 2 of the sound comparison: a traffic
/// car at azimuth ≈ 49 600 is 15–20 dB louder on the left, one at ≈ 12 700
/// 11–13 dB louder on the right).
pub fn pan_weights(azimuth: u16) -> (f32, f32) {
    let theta = (azimuth >> 8) as f64 * 360.0 / 256.0;
    let d = if theta <= 90.0 {
        theta + 90.0
    } else if theta < 270.0 {
        270.0 - theta
    } else {
        theta - 270.0
    };
    let t = d * std::f64::consts::PI / 360.0;
    let w = |x: f64| ((255.0 * x).round_ties_even() as f32) * (1.0 / 255.0);
    (w(t.sin()), w(t.cos()))
}

/// A stream's channel weights (left, right): the stream player gives each
/// of its channels a library voice at the azimuth word of the table at
/// 0x8B4680 for its channel count (0x7259xx: one channel 0, two 0xC000 /
/// 0x4000; read under Wine: a stereo song's two voices at 49152 / 16384,
/// weights 0 / 1 and 1 / 0), mixed through the pan law ([`pan_weights`],
/// the channel gains of 0x724F5D). A mono stream (speech, a NIS line) is
/// one voice at azimuth 0: 180/255 on each side, like any centred voice.
pub fn stream_pan(channels: usize) -> (f32, f32) {
    if channels == 1 {
        let (r, l) = pan_weights(0);
        (l, r)
    } else {
        (pan_weights(0xC000).1, pan_weights(0x4000).0)
    }
}

/// A movie's sound volume (0x531E00 at the movie's first frame → 0x525E20
/// → 0x6FEFEF): clamp(trunc(the movie's volume byte · master · speech),
/// 0, 127) / 127. The byte is the +0x1F of the movie's entry (chunk
/// 0x39020 in GLOBALB.BUN, found by name with 0x51C580, case-blind;
/// `None` → 0xFF read as −1 → silence).
pub fn movie_volume(byte: Option<u8>, master: f32, speech: f32) -> f32 {
    let b = byte.map_or(-1, |b| i32::from(b as i8));
    ((b as f32 * (master * speech)).trunc() as i32).clamp(0, 127) as f32 / 127.0
}

/// The gain ramp's length: a voice's 16-sample block (0x728B48).
pub(super) const RAMP: usize = 16;

/// The ramp's step, 1/17 (0x7C69E4).
const STEP: f32 = 1.0 / 17.0;

/// 0x72886C's weights 16/17 … 1/17 (0x7C6A20 … 0x7C69E4, each the float
/// nearest k / 17).
const FADE: [f32; RAMP] = {
    let mut w = [0f32; RAMP];
    let mut i = 0;
    while i < RAMP {
        w[i] = (RAMP - i) as f32 / 17.0;
        i += 1;
    }
    w
};

/// One voice's samples into a buffer at its gain (0x728B48's per-bus /
/// per-channel step): a gain that moved since the last chunk ramps over
/// the first 16 samples, dst[i] += src[i] · (cur + (i + 1) · (tgt − cur)
/// / 17) (0x728957: the block ends 1/17 short of the target), and the rest
/// go in at the target (0x8B4A04: dst += src · g, skipped at 0); then cur
/// = tgt. A voice's first chunk starts at its targets (0x728F75 copies
/// them): `cur` NaN. Returns whether it ramped. (The library's block is
/// always 16 samples; a chunk shorter than 16 frames ramps over its
/// length here.)
pub(super) fn mix_gain(dst: &mut [f32], src: &[f32], cur: &mut f32, tgt: f32) -> bool {
    let c = if cur.is_nan() { tgt } else { *cur };
    *cur = tgt;
    if c != tgt {
        let d = (tgt - c) * STEP;
        let r = RAMP.min(src.len());
        for (i, (o, s)) in dst[..r].iter_mut().zip(&src[..r]).enumerate() {
            *o += s * ((i + 1) as f32 * d + c);
        }
        for (o, s) in dst[r..].iter_mut().zip(&src[r..]) {
            *o += s * tgt;
        }
        true
    } else {
        if tgt != 0.0 {
            for (o, s) in dst.iter_mut().zip(src) {
                *o += s * tgt;
            }
        }
        false
    }
}

/// 0x72886C: a stopped voices' accumulator faded out over 16 samples,
/// dst[i] += acc · (16 − i) / 17, then cleared (only when nonzero,
/// 0x7292E7 / 0x729334).
pub(super) fn fade_out(dst: &mut [f32], acc: &mut f32) {
    if *acc != 0.0 {
        for (o, w) in dst.iter_mut().zip(FADE) {
            *o += *acc * w;
        }
        *acc = 0.0;
    }
}

/// A voice's stop (0x728FD2): its last sample × its current gains into the
/// channel accumulators (library channels 0 / 1, mono).
pub(super) fn declick(acc: &mut [f32; 3], cur: [f32; 3], last: f32) {
    if last != 0.0 && cur.iter().all(|g| g.is_finite()) {
        for (a, g) in acc.iter_mut().zip(cur) {
            *a += g * last;
        }
    }
}

/// A track's stop: its left sample into channel 1, its right into 0 (as
/// it is mixed), both halves into mono.
pub(super) fn declick_track(acc: &mut [f32; 3], cur: f32, (l, r): (f32, f32)) {
    if cur.is_finite() {
        acc[0] += cur * r;
        acc[1] += cur * l;
        acc[2] += cur * (l + r) * 0.5;
    }
}

/// A gain into 0..1; a NaN one is silence (it would poison the mix).
pub(super) fn unit_gain(gain: f32) -> f32 {
    if gain.is_finite() { gain.clamp(0.0, 1.0) } else { 0.0 }
}

pub(super) static BAD_VOICE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static BAD_ENGINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static BAD_TRACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static BAD_FORMAT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A mixer fault, on stderr the first time only (the audio callback must
/// not spam).
pub(super) fn warn_once(flag: &std::sync::atomic::AtomicBool, msg: impl FnOnce() -> String) {
    if !flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("audio: {}", msg());
    }
}
