use crate::*;

/// `NFSU2_AUDIO_WAV`'s capture: the samples so far (the second field is
/// unused: the capture renders each tick's whole frames).
pub(crate) static AUDIO_CAPTURE: std::sync::Mutex<Option<(Vec<f32>, f64)>> = std::sync::Mutex::new(None);

/// The offline mixer's rate: 48 kHz, or `NFSU2_AUDIO_RATE` (a check aid:
/// a live device's own rate, e.g. 44100 as cpal's ALSA default picks).
pub(crate) fn audio_capture_rate() -> u32 {
    static RATE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| {
        std::env::var("NFSU2_AUDIO_RATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&r: &u32| (8000..=192_000).contains(&r))
            .unwrap_or(48000)
    })
}

/// `NFSU2_AUDIO_METER=s` (with a capture): per `s` seconds of game time the
/// output's peak and RMS and the game's audio state (`Game::audio_state`)
/// on stdout, the samples dropped once measured (no WAV), so a capture can
/// run for hours: (frames in the window, sum of squares, peak, frames in
/// all).
pub(crate) static AUDIO_METER: std::sync::Mutex<(usize, f64, f32, usize)> = std::sync::Mutex::new((0, 0.0, 0.0, 0));

/// The meter's window in frames, if on.
pub(crate) fn audio_meter_window() -> Option<usize> {
    let s: f32 = std::env::var("NFSU2_AUDIO_METER").ok()?.parse().ok()?;
    Some(((s.max(0.1) * audio_capture_rate() as f32) as usize).max(1))
}

/// The offline mixer rendered for a tick: its `frames` (`Game::audio_tick_frames`:
/// the tick's real-time length at the mix rate, 800 a RACE tick and 1600 a
/// SCENE tick at 48 kHz, whatever the display rate) (no-op without
/// a capture); `state` describes the game's audio for the meter's lines.
pub(crate) fn pump_audio_capture(mixer: &Mixer, frames: usize, state: &dyn Fn() -> String) {
    let mut c = AUDIO_CAPTURE.lock().unwrap();
    let Some((samples, _)) = c.as_mut() else { return };
    mixer.pump(frames, samples);
    // NFSU2_AUDIO_STEMS=file: each source's RMS (dBFS) per 0.5 s of capture.
    if nfsu2::audio::mixer::stems::enabled() {
        static STEMS: std::sync::Mutex<(usize, Vec<(String, f64)>)> = std::sync::Mutex::new((0, Vec::new()));
        let mut st = STEMS.lock().unwrap();
        st.0 += frames;
        for (name, e) in mixer.take_stems() {
            match st.1.iter_mut().find(|x| x.0 == name) {
                Some(x) => x.1 += e,
                None => st.1.push((name, e)),
            }
        }
        let window = audio_capture_rate() as usize / 2;
        if st.0 >= window {
            let t = samples.len() as f64 / 2.0 / f64::from(audio_capture_rate());
            let n = st.0 as f64 * 2.0;
            let line: String =
                st.1.iter().map(|(k, e)| format!("\t{k}={:.1}", 10.0 * (e / n).max(1e-20).log10())).collect();
            if let Ok(path) = std::env::var("NFSU2_AUDIO_STEMS") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{t:.2}{line}");
                }
            }
            st.0 = 0;
            st.1.clear();
        }
    }
    nfsu2::audio::aems::log::set_log_time(samples.len() as f64 / 2.0 / f64::from(audio_capture_rate()));
    if let Some(window) = audio_meter_window() {
        let mut m = AUDIO_METER.lock().unwrap();
        for f in samples[samples.len() - frames * 2..].chunks(2) {
            m.1 += f.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
            m.2 = f.iter().fold(m.2, |p, s| p.max(s.abs()));
            m.0 += 1;
            m.3 += 1;
            if m.0 >= window {
                let rms = (m.1 / (2 * m.0) as f64).sqrt();
                println!(
                    "meter {:7.1} s: peak {:.4} rms {:.5} | {}",
                    m.3 as f32 / audio_capture_rate() as f32,
                    m.2,
                    rms,
                    state()
                );
                (m.0, m.1, m.2) = (0, 0.0, 0.0);
            }
        }
        samples.clear();
        return;
    }
    // NFSU2_AUDIO_DEBUG: the voices playing, every 5 s of capture.
    let n = samples.len() / 2;
    let every = audio_capture_rate() as usize * 5;
    if nfsu2::env_set!("NFSU2_AUDIO_DEBUG") && n / every != (n - frames) / every {
        eprintln!("audio: {} s, {} voices", n / audio_capture_rate() as usize, mixer.voices());
    }
}

/// The capture as a 16-bit stereo WAV, and its level per second of game
/// time on stdout.
pub(crate) fn write_audio_capture(path: &str) -> Result<()> {
    let Some((samples, _)) = AUDIO_CAPTURE.lock().unwrap().take() else { return Ok(()) };
    let pcm: Vec<i16> = samples.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    let data = (pcm.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    for v in [16u32, 1 | (2 << 16), audio_capture_rate(), audio_capture_rate() * 4, 4 | (16 << 16)] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in &pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, out)?;
    let per_second: Vec<String> = samples
        .chunks(audio_capture_rate() as usize * 2)
        .map(|c| format!("{:.3}", (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt()))
        .collect();
    println!(
        "audio: {:.1} s captured, rms per second [{}]",
        samples.len() as f32 / 2.0 / audio_capture_rate() as f32,
        per_second.join(" ")
    );
    Ok(())
}
