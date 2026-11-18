//! The launcher's own sounds (Settings › Advanced › Launcher sounds, off by
//! default): a short soft tick when the focus moves and a brighter one on
//! a press, synthesised (no files). Played on the system's default output
//! through cpal, opened the first time a sound is asked for. Never in tests
//! or headless renders, and never when `NFSU2_AUDIO_HOST=none`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Sounds on or off (the setting).
pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed)
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// What to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// The focus moved.
    Move,
    /// Something was pressed.
    Press,
}

/// Whether sounds may be played at all in this process: not in tests,
/// not with `NFSU2_AUDIO_HOST=none` (headless checks), not when the
/// setting is off.
pub fn allowed() -> bool {
    !cfg!(test) && enabled() && std::env::var("NFSU2_AUDIO_HOST").map_or(true, |v| v != "none")
}

/// The cue's samples (mono, at `rate`): a decaying sine.
pub fn samples(cue: Cue, rate: u32) -> Vec<f32> {
    let (freq, ms, gain) = match cue {
        Cue::Move => (880.0, 28.0, 0.06),
        Cue::Press => (1320.0, 45.0, 0.08),
    };
    let n = (rate as f32 * ms / 1000.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let env = (1.0 - i as f32 / n as f32).powi(3);
            (t * freq * std::f32::consts::TAU).sin() * env * gain
        })
        .collect()
}

type Queue = std::sync::Arc<Mutex<std::collections::VecDeque<f32>>>;

/// The output: a thread that owns the stream (cpal streams stay on the
/// thread that made them) and the queue it plays from, with its rate.
#[cfg(not(test))]
static OUT: std::sync::OnceLock<Option<(Queue, u32)>> = std::sync::OnceLock::new();

#[cfg(not(test))]
fn open() -> Option<(Queue, u32)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("launcher-sounds".into())
        .spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
            let made = (|| {
                let dev = cpal::default_host().default_output_device()?;
                let supported = dev.default_output_config().ok()?;
                let format = supported.sample_format();
                let config: cpal::StreamConfig = supported.into();
                let (rate, ch) = (config.sample_rate, config.channels as usize);
                let queue: Queue = Default::default();
                let q = queue.clone();
                let next = move || q.lock().unwrap_or_else(|e| e.into_inner()).pop_front().unwrap_or(0.0);
                let err = |e| log::warn!("launcher sounds: {e}");
                let stream = match format {
                    cpal::SampleFormat::F32 => dev.build_output_stream(
                        config.clone(),
                        move |d: &mut [f32], _| d.chunks_mut(ch).for_each(|f| f.fill(next())),
                        err,
                        None,
                    ),
                    cpal::SampleFormat::I16 => dev.build_output_stream(
                        config.clone(),
                        move |d: &mut [i16], _| d.chunks_mut(ch).for_each(|f| f.fill((next() * 32767.0) as i16)),
                        err,
                        None,
                    ),
                    _ => return None,
                }
                .ok()?;
                stream.play().ok()?;
                Some((stream, queue, rate))
            })();
            match made {
                Some((stream, queue, rate)) => {
                    let _ = tx.send(Some((queue, rate)));
                    // Keep the stream for the life of the launcher.
                    let _keep = stream;
                    loop {
                        std::thread::park();
                    }
                }
                None => {
                    let _ = tx.send(None);
                }
            }
        })
        .ok()?;
    rx.recv_timeout(std::time::Duration::from_secs(2)).ok().flatten()
}

/// Plays `cue` when sounds are allowed (the output opened on first use).
pub fn play(cue: Cue) {
    if !allowed() {
        return;
    }
    #[cfg(not(test))]
    if let Some((queue, rate)) = OUT.get_or_init(|| {
        let o = open();
        if o.is_none() {
            log::info!("launcher sounds: no audio output");
        }
        o
    }) {
        let s = samples(cue, *rate);
        let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() < *rate as usize / 4 {
            q.extend(s);
        }
    }
    #[cfg(test)]
    let _ = cue;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_audible_in_tests_and_quiet() {
        set_enabled(true);
        assert!(!allowed(), "no sound in a test, even when switched on");
        set_enabled(false);
        let s = samples(Cue::Press, 48_000);
        assert!(!s.is_empty() && s.iter().all(|v| v.abs() <= 0.08));
    }
}
