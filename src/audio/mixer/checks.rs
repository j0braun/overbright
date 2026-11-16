use super::*;
#[cfg(any(test, feature = "dev-harness"))]
use std::sync::{Arc, Mutex};

impl Mix {
    /// A check aid: note a stop whose cut would step by `step` (16-bit
    /// range; nothing for a silent one).
    #[cfg(any(test, feature = "dev-harness"))]
    pub(super) fn log_stop(&mut self, frame: u64, step: f32) {
        if step > 0.0 && step.is_finite() && self.stops.len() < STOP_LOG {
            self.stops.push((frame, step / 32768.0 * MASTER));
        }
    }

    /// Without the developer harness no stop is logged.
    #[cfg(not(any(test, feature = "dev-harness")))]
    pub(super) fn log_stop(&mut self, _frame: u64, _step: f32) {}

    /// The stops logged since the last call (see [`Mix::log_stop`]).
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn take_stops(&mut self) -> Vec<(u64, f32)> {
        let out = self.stops.clone();
        self.stops.clear();
        out
    }
}

impl Mixer {
    /// A check aid (`examples/audio_device_check`): hold the mix for `d`,
    /// as a stalled game would, so the device's callback waits and the
    /// device underruns.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn hold(&self, d: std::time::Duration) {
        let _m = self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        std::thread::sleep(d);
    }

    /// A check aid: the device stream dropped without a new one, as a
    /// worker stuck for good leaves it (no callbacks), so
    /// [`Mixer::keep_alive`] has to reopen it.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn kill_stream(&self) {
        if let Some(d) = self.device.as_ref() {
            d.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stream.take();
        }
    }

    /// A check aid: ms since the device's callback last ran, and its
    /// buffer length (ms).
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn beat(&self) -> Option<(u64, u64)> {
        use std::sync::atomic::Ordering::Relaxed;
        let d = self.device.as_ref()?.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Some((
            (d.beat.base.elapsed().as_millis() as u64).saturating_sub(d.beat.beat.load(Relaxed)),
            d.beat.period.load(Relaxed),
        ))
    }

    /// A check aid: the device callbacks since the last call (their
    /// count, the longest lock wait, render and gap between two, ms) and
    /// the restarts, as a line; None for an offline mixer.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn take_stats(&self) -> Option<String> {
        use std::sync::atomic::Ordering::Relaxed;
        let d = self.device.as_ref()?.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let st = &d.beat.stats;
        let n = st.n.swap(0, Relaxed);
        let ms = |a: &std::sync::atomic::AtomicU64| a.swap(0, Relaxed) as f64 / 1000.0;
        let (w, r, g) = (ms(&st.wait_max), ms(&st.render_max), ms(&st.gap_max));
        let silent = (d.beat.base.elapsed().as_millis() as u64).saturating_sub(d.beat.beat.load(Relaxed));
        Some(format!(
            "{n} callbacks, lock wait max {w:.2} ms, render max {r:.2} ms, gap max {g:.1} ms, silent now {silent} ms, restarts {}",
            d.restarts
        ))
    }

    /// A check aid: a mixer with no device, rendered by [`Mixer::pump`]
    /// (stereo at `rate`), so a headless run can record what it plays.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn offline(rate: u32) -> Mixer {
        Mixer { mix: Arc::new(Mutex::new(Mix::new(rate))), device: None }
    }

    /// A check aid: render this (offline) mixer from a thread at real
    /// time, `period` stereo frames at a time, as a device's callback does
    /// (no waiting for the decoders), each block given to `sink`.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn spawn_realtime(&self, period: usize, mut sink: impl FnMut(&[f32]) + Send + 'static) {
        let mix = self.mix.clone();
        std::thread::spawn(move || {
            let rate = mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).rate;
            let step = std::time::Duration::from_secs_f64(period as f64 / f64::from(rate));
            let mut next = std::time::Instant::now();
            let mut buf = vec![0.0f32; period * 2];
            // Per 5 s: the longest wait for the lock, the longest render
            // and the blocks done after their deadline (a device's
            // underruns), on stderr.
            let (mut wait_max, mut render_max, mut late, mut n) = (0f64, 0f64, 0u32, 0u32);
            loop {
                next += step;
                if let Some(wait) = next.checked_duration_since(std::time::Instant::now()) {
                    std::thread::sleep(wait);
                }
                buf.fill(0.0);
                let t0 = std::time::Instant::now();
                let mut m = mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let t1 = std::time::Instant::now();
                m.render(&mut buf, 2);
                drop(m);
                let t2 = std::time::Instant::now();
                wait_max = wait_max.max((t1 - t0).as_secs_f64() * 1000.0);
                render_max = render_max.max((t2 - t1).as_secs_f64() * 1000.0);
                if t2 > next + step {
                    late += 1;
                }
                n += 1;
                if f64::from(n) * period as f64 >= 5.0 * f64::from(rate) {
                    eprintln!(
                        "realtime mixer: lock wait max {wait_max:.2} ms, render max {render_max:.2} ms, {late} of {n} blocks late"
                    );
                    (wait_max, render_max, late, n) = (0.0, 0.0, 0, 0);
                }
                sink(&buf);
            }
        });
    }

    /// An offline mixer's next `frames` stereo frames, appended to `out`.
    #[cfg(any(test, feature = "dev-harness"))]
    pub fn pump(&self, frames: usize, out: &mut Vec<f32>) {
        // Offline the game runs faster than the decoder threads: wait until
        // each playing track holds the samples this pump needs, as a live
        // device's real-time pace would allow. The mix is then a function
        // of the posted events alone (the rate gate's WAVs); the
        // bound (60 s, a decoder that hangs) only keeps a check from
        // waiting for good, and says so.
        let t0 = std::time::Instant::now();
        while !self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).tracks_ready(frames) {
            if t0.elapsed().as_secs_f32() >= 60.0 {
                log::warn!("audio: offline pump: a track's decoder gave no samples for 60 s; mixed without them");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let start = out.len();
        out.resize(start + frames * 2, 0.0);
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).render(&mut out[start..], 2);
    }

    /// The output peak since the last call, and the mix's state (a check
    /// aid: `NFSU2_AUDIO_DEBUG`).
    pub fn take_peak(&self) -> (f32, String) {
        let mut m = self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let p = std::mem::take(&mut m.peak);
        (p, m.debug_state())
    }

    #[cfg(any(test, feature = "dev-harness"))]
    pub fn take_stops(&self) -> Vec<(u64, f32)> {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take_stops()
    }
}
