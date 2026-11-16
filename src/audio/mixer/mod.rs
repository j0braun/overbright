//! The output: one cpal stream mixing one-shot samples and the engine's
//! granular players. The game mixes in its audio library (volume bytes
//! 0–0x7F, pitch 0x1000 = 1.0: 0x721A6B's defaults); volume is linear in
//! the byte. Each library voice reaches the two output channels through
//! the stereo pan tables (0x72559B, two channels: [`pan_weights`]) at its
//! azimuth word, which the mix map (`super::mixmap`) gives each sound
//! object (slot 0x3C) and the event graphs pass on as parameter kind 3.
//!
//! `Mixer::new` returns `None` without an output device, so nothing
//! changes for headless runs.

mod checks;
mod device;
mod facade;
mod gain;
mod render;
mod sources;

use self::device::*;
pub use self::device::MixerOpening;
pub use self::facade::*;
pub use self::gain::*;
pub use self::render::*;
pub use self::sources::*;

/// A check aid (`NFSU2_AUDIO_STEMS`): each voice carries the source that
/// started it (an AEMS event's name, `GIN`, `speech`, …), and the offline
/// mix sums each source's output energy, so a capture can be compared per
/// source. Off (and free) unless enabled.
pub mod stems {
    use std::cell::Cell;
    use std::sync::Mutex;
    static NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
    thread_local!(static CURRENT: Cell<u16> = const { Cell::new(0) });
    /// Whether `NFSU2_AUDIO_STEMS` is set (read once).
    pub fn enabled() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("NFSU2_AUDIO_STEMS").is_some())
    }
    /// The index of a source name (0 = "other").
    pub fn index(name: &str) -> u16 {
        let mut n = NAMES.lock().unwrap();
        if n.is_empty() {
            n.push("other".into());
        }
        if let Some(i) = n.iter().position(|x| x == name) {
            return i as u16;
        }
        n.push(name.into());
        (n.len() - 1) as u16
    }
    pub fn name(i: usize) -> String {
        NAMES.lock().unwrap().get(i).cloned().unwrap_or_else(|| "other".into())
    }
    /// The source the voices started on this thread from now on belong to.
    pub fn set(name: &str) {
        if enabled() {
            let i = index(name);
            CURRENT.with(|c| c.set(i));
        }
    }
    pub fn clear() {
        CURRENT.with(|c| c.set(0));
    }
    /// `NFSU2_AUDIO_STEMS_SPLIT`: finer source names (the player's own CAR
    /// instance as `CAR.player`, the two engine voices as `GIN.accel` /
    /// `GIN.decel`, the AI racers' skids as `FX_SKID.ai`); off, the names
    /// stay `CAR`, `GIN` and `FX_SKID`.
    pub fn split() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| enabled() && std::env::var_os("NFSU2_AUDIO_STEMS_SPLIT").is_some())
    }
    /// `NFSU2_AUDIO_SOLO=name,name,…` (with stems on): only these sources
    /// reach the output (an event's name, `CAR.player`, `GIN`, `GIN.accel`,
    /// `GIN.decel`, `speech`, `music`); the reverb's output stays unless
    /// `-reverb` is listed. A check aid for per-layer recordings of the
    /// offline mix.
    fn solo_list() -> Option<&'static Vec<String>> {
        static SOLO: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
        SOLO.get_or_init(|| {
            if !enabled() {
                return None;
            }
            std::env::var("NFSU2_AUDIO_SOLO")
                .ok()
                .map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        })
        .as_ref()
    }
    /// Whether the source `tag` is heard under `NFSU2_AUDIO_SOLO`.
    pub(super) fn keep(tag: u16) -> bool {
        let Some(list) = solo_list() else { return true };
        solo_matches(list, &name(tag as usize))
    }
    /// Whether the source `name` is one of the solo `list`'s: an exact
    /// name, or `GIN` / `CAR` for every split name under them.
    pub(super) fn solo_matches(list: &[String], name: &str) -> bool {
        list.iter().any(|x| {
            x == name
                || (x == "GIN" && name.starts_with("GIN."))
                || (x == "CAR" && name.starts_with("CAR."))
                || (x == "FX_SKID" && name.starts_with("FX_SKID."))
        })
    }
    /// Whether the reverb's output is heard under `NFSU2_AUDIO_SOLO`.
    pub(super) fn keep_reverb() -> bool {
        solo_list().is_none_or(|l| !l.iter().any(|x| x == "-reverb"))
    }
    pub(super) fn current() -> u16 {
        CURRENT.with(Cell::get)
    }
    /// Adds `src · g` energy (both channels) to `acc[tag]`.
    pub(super) fn add(acc: &mut Vec<f64>, tag: u16, src: &[f32], g: (f32, f32)) {
        let t = tag as usize;
        if acc.len() <= t {
            acc.resize(t + 1, 0.0);
        }
        let e: f64 = src.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
        acc[t] += e * (f64::from(g.0) * f64::from(g.0) + f64::from(g.1) * f64::from(g.1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-layer check aid's names: a split name answers to its own
    /// name and to the group's (`GIN`, `CAR`), nothing else.
    #[test]
    fn solo_names_match_exactly_or_by_group() {
        let list: Vec<String> = ["GIN", "CAR.player", "FX_SKID"].iter().map(|s| s.to_string()).collect();
        for n in ["GIN", "GIN.accel", "GIN.decel", "CAR.player", "FX_SKID", "FX_SKID.ai"] {
            assert!(stems::solo_matches(&list, n), "{n}");
        }
        for n in ["CAR", "FX_SKID2", "music", "speech", "GINX", "reverb"] {
            assert!(!stems::solo_matches(&list, n), "{n}");
        }
        let cars = vec!["CAR".to_string()];
        assert!(stems::solo_matches(&cars, "CAR") && stems::solo_matches(&cars, "CAR.player"));
        assert!(!stems::solo_matches(&cars, "CAR_Sputter"));
    }
    use crate::audio::fe::Sample;
    use std::sync::{Arc, Mutex};

    /// unwrap audit: no output device (here: a host that does not
    /// exist, `NFSU2_AUDIO_HOST`) is `None`, which the game runs without
    /// sound on, never a panic; promptly, not after the open wait. In a
    /// child process: the variable is process-wide.
    #[test]
    fn no_audio_device_is_none() {
        const CHILD: &str = "NFSU2_NO_DEVICE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "audio::mixer::tests::no_audio_device_is_none"])
                .env(CHILD, "1")
                .env("NFSU2_AUDIO_HOST", "no-such-host")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let t0 = std::time::Instant::now();
        assert!(Mixer::new().is_none());
        assert!(t0.elapsed() < OPEN_WAIT, "{:?}", t0.elapsed());
    }

    #[test]
    fn a_silent_device_callback_is_a_stall() {
        // PipeWire's 11.6 ms buffers: 0.4 s without a callback.
        assert!(!stalled(399, 12) && stalled(400, 12));
        // Long plug-in buffers (0.5 s) wait three of them and 0.1 s.
        assert!(!stalled(1500, 500) && stalled(1600, 500));
    }

    #[test]
    fn voices_resample_and_finish() {
        let mut mix = Mix::new(44100);
        let s: Sample = Arc::new((22050, vec![16384.0; 100]));
        mix.play(&s, 0x7F, 0x1000);
        let mut out = vec![0.0; 2 * 150];
        mix.render(&mut out, 2);
        // Centre: 180 / 255 on each channel.
        assert!((out[0] - 0.5 * MASTER * 180.0 / 255.0).abs() < 1e-6);
        assert_eq!(out[0], out[1]);
        // 100 samples at half the rate last about 200 frames.
        assert_eq!(mix.voices.len(), 1);
        mix.render(&mut out, 2);
        assert!(mix.voices.is_empty());
    }

    #[test]
    fn pan_rows_match_the_tables() {
        let row = |b: u16| {
            let (a, c) = pan_weights(b << 8);
            ((a * 255.0).round() as i32, (c * 255.0).round() as i32)
        };
        assert_eq!(
            [row(0), row(32), row(64), row(96), row(128), row(160), row(192), row(224)],
            [(180, 180), (236, 98), (255, 0), (236, 98), (180, 180), (98, 236), (0, 255), (98, 236)]
        );
    }

    #[test]
    fn loops_run_until_stopped() {
        let mut mix = Mix::new(22050);
        let s: Sample = Arc::new((22050, vec![1000.0; 10]));
        let id = mix.play_loop(&s, 1.0, 0x1000);
        let mut out = vec![0.0; 2 * 100];
        mix.render(&mut out, 2);
        assert!(mix.playing(id) && out[198] > 0.0);
        mix.stop(id);
        assert!(!mix.playing(id));
    }

    #[test]
    fn gain_ramps_over_sixteen_samples() {
        // 0x728957: 1/17 of the change a step, the block ends 1/17 short.
        let src = [1.0f32; 40];
        let mut dst = [0f32; 40];
        let mut cur = 0.0;
        assert!(mix_gain(&mut dst, &src, &mut cur, 1.0));
        for i in 0..16 {
            assert!((dst[i] - (i + 1) as f32 / 17.0).abs() < 1e-6, "{i} {}", dst[i]);
        }
        assert!(dst[16..].iter().all(|&x| x == 1.0) && cur == 1.0);
        // Unchanged: the target throughout, no ramp; a voice's first chunk
        // (cur NaN) starts at its target.
        let mut dst = [0f32; 40];
        assert!(!mix_gain(&mut dst, &src, &mut cur, 1.0));
        assert!(dst.iter().all(|&x| x == 1.0));
        let mut cur = f32::NAN;
        let mut dst = [0f32; 40];
        assert!(!mix_gain(&mut dst, &src, &mut cur, 0.5) && dst[0] == 0.5);
    }

    #[test]
    fn set_gain_ramps_at_the_next_chunk() {
        let mut mix = Mix::new(22050);
        let s: Sample = Arc::new((22050, vec![10000.0; 4000]));
        let id = mix.play_loop(&s, 1.0, 0x1000);
        let mut out = vec![0.0; 2 * 256];
        mix.render(&mut out, 2);
        let full = out[0];
        mix.set_gain(id, 0.0);
        mix.render(&mut out, 2);
        // Frame i of the chunk: full · (1 − (i + 1) / 17), then silence.
        for i in 0..16 {
            let want = full * (1.0 - (i + 1) as f32 / 17.0);
            assert!((out[2 * i] - want).abs() < 1e-5, "{i}: {} {want}", out[2 * i]);
        }
        assert!(out[32..].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn stops_fade_out_over_sixteen_samples() {
        // 0x728FD2 / 0x72886C: the last sample × the gains, × 16/17 … 1/17
        // at the next chunk's start.
        let mut mix = Mix::new(22050);
        let s: Sample = Arc::new((22050, vec![10000.0; 4000]));
        let id = mix.play_loop(&s, 1.0, 0x1000);
        let mut out = vec![0.0; 2 * 100];
        mix.render(&mut out, 2);
        let last = out[198];
        mix.stop(id);
        mix.render(&mut out, 2);
        for i in 0..16 {
            let want = last * (16 - i) as f32 / 17.0;
            assert!((out[2 * i] - want).abs() < 1e-5 && out[2 * i + 1] == out[2 * i], "{i}");
        }
        assert!(out[32..].iter().all(|&x| x == 0.0));
        // Once only.
        mix.render(&mut out, 2);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn stopping_a_tone_has_no_step() {
        // A 440 Hz tone stopped at its peak: no output step larger than the
        // tone's own largest step (2π · 440 / 44100 of its amplitude) plus
        // the fade's (1/17 of it).
        let rate = 44100;
        let s: Sample = Arc::new((
            rate,
            (0..44100).map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / rate as f32).sin() * 20000.0).collect(),
        ));
        for cut in [100usize, 125, 131, 256, 300] {
            let mut mix = Mix::new(rate);
            let id = mix.play_loop(&s, 1.0, 0x1000);
            let mut out = vec![0.0; 2 * cut];
            mix.render(&mut out, 2);
            mix.stop(id);
            let mut tail = vec![0.0; 2 * 64];
            mix.render(&mut tail, 2);
            let amp = 20000.0 / 32768.0 * MASTER * 180.0 / 255.0;
            let bound = amp * (2.0 * std::f32::consts::PI * 440.0 / rate as f32 + 1.0 / 17.0) * 1.01;
            let left: Vec<f32> = out.iter().chain(&tail).step_by(2).copied().collect();
            let worst = left.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
            assert!(worst <= bound, "cut {cut}: step {worst} > {bound}");
        }
    }

    #[test]
    fn mono_tracks_take_the_centre_pan() {
        // A mono stream is one library voice at azimuth 0 (0x8B4680):
        // 180/255 on each side, as a centred one-shot; a stereo one is two
        // voices at 0xC000 / 0x4000, full on its own side.
        assert_eq!(stream_pan(1), (180.0 / 255.0, 180.0 / 255.0));
        assert_eq!(stream_pan(2), (1.0, 1.0));
        let mut mix = Mix::new(32000);
        let buf: TrackBuf =
            Arc::new(Mutex::new(TrackPcm { channels: 1, rate: 32000, samples: vec![0.5; 64], done: true }));
        mix.play_track(buf, 1.0, 2);
        let mut out = vec![0.0; 2 * 8];
        mix.render(&mut out, 2);
        assert!((out[0] - 0.5 * MASTER * 180.0 / 255.0).abs() < 1e-6, "{}", out[0]);
        assert_eq!(out[0], out[1]);
    }

    #[test]
    fn movie_volume_is_its_byte_times_master_and_speech() {
        // 0x525E20: clamp(trunc(byte · master · speech), 0, 127) / 127.
        assert_eq!(movie_volume(Some(127), 1.0, 1.0), 1.0);
        assert_eq!(movie_volume(Some(40), 1.0, 1.0), 40.0 / 127.0);
        assert_eq!(movie_volume(Some(105), 0.5, 1.0), 52.0 / 127.0);
        assert_eq!(movie_volume(Some(127), 0.1, 0.5), 6.0 / 127.0);
        // Not in the table: 0xFF read as −1 → silence.
        assert_eq!(movie_volume(None, 1.0, 1.0), 0.0);
        // The stream plays at it.
        let mut mix = Mix::new(32000);
        mix.set_stream(Some((2, 32000)));
        mix.set_stream_gain(0.5);
        mix.push_stream(&[16384.0, -16384.0, 16384.0, -16384.0, 16384.0, -16384.0]);
        let mut out = vec![0.0; 4];
        mix.render(&mut out, 2);
        assert!((out[0] - 0.25 * MASTER).abs() < 1e-6, "{}", out[0]);
    }

    #[test]
    fn a_rate_change_keeps_the_pitch() {
        let mut mix = Mix::new(48000);
        let s: Sample = Arc::new((24000, vec![0.0; 1000]));
        mix.play(&s, 0x7F, 0x1000);
        assert_eq!(mix.voices[0].step, 0.5);
        mix.set_rate(44100);
        assert!((mix.voices[0].step - 24000.0 / 44100.0).abs() < 1e-12);
        assert!((mix.voices[0].base - 24000.0 / 44100.0).abs() < 1e-12);
    }

    #[test]
    fn surround_channels_get_silence() {
        let mut mix = Mix::new(32000);
        let s: Sample = Arc::new((32000, vec![16384.0; 100]));
        mix.play(&s, 0x7F, 0x1000);
        let mut out = vec![1.0; 6 * 4];
        mix.render(&mut out, 6);
        assert!(out[0] > 0.0 && out[1] > 0.0);
        assert!(out[2..6].iter().all(|&x| x == 0.0), "{:?}", &out[..6]);
    }

    #[test]
    fn stream_plays_stereo_at_its_rate() {
        let mut mix = Mix::new(32000);
        mix.set_stream(Some((2, 32000)));
        mix.push_stream(&[16384.0, -16384.0, 16384.0, -16384.0, 16384.0, -16384.0]);
        let mut out = vec![0.0; 4];
        mix.render(&mut out, 2);
        assert!((out[0] - 0.5 * MASTER).abs() < 1e-6 && (out[1] + 0.5 * MASTER).abs() < 1e-6);
    }
}

#[cfg(test)]
mod cost {
    use super::*;
    use crate::audio::fe::Sample;
    use std::sync::Arc;

    /// The callback's cost: 24 loops whose gains move every 735 frames
    /// (a 60 Hz game frame), the Simple_Tunnel graph, 10 s at 44100 in
    /// 512-frame callbacks. `cargo test --release mixer::cost -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark: the mixer callback cost"]
    fn render_cost() {
        let mut mix = Mix::new(44100);
        let s: Sample = Arc::new((22050, (0..4000).map(|i| ((i as f32) * 0.05).sin() * 8000.0).collect()));
        let ids: Vec<u32> = (0..24).map(|i| mix.play_loop(&s, 0.5, 0x1000 + i * 40)).collect();
        if let Some(g) = super::super::reverb::Graph::load(&crate::game_root(), 5, 44100) {
            mix.set_fx(Some(g));
        }
        let mut out = vec![0f32; 1024];
        let t = std::time::Instant::now();
        let mut frames = 0usize;
        let mut n = 0u32;
        while frames < 441000 {
            mix.render(&mut out, 2);
            frames += 512;
            if frames % 735 < 512 {
                n += 1;
                for (k, id) in ids.iter().enumerate() {
                    mix.set_gain(*id, 0.3 + 0.2 * ((n as f32 + k as f32) * 0.1).sin());
                }
            }
        }
        let el = t.elapsed().as_secs_f64();
        println!("render_cost: {:.2} ms for 10 s ({:.3} % of one core)", el * 1000.0, el * 10.0);
    }
}
