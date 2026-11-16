use crate::audio::mixer::{BAD_FORMAT, Mix, Mixer, warn_once};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// The live output: the cpal stream, its callbacks' beat ([`Beat`]) and
/// the watchdog's state ([`Mixer::keep_alive`]).
pub(crate) struct Device {
    pub(crate) stream: Option<cpal::Stream>,
    pub(crate) beat: Beat,
    /// The last check and the last restart.
    pub(crate) checked: std::time::Instant,
    pub(crate) restarted: Option<std::time::Instant>,
    pub(crate) restarts: u32,
    /// A stream being opened on a thread of its own ([`open_async`]),
    /// and since when.
    pub(crate) pending: Option<(Opening, std::time::Instant)>,
}

/// A stream being opened ([`open_async`]): the stream and its rate, or
/// None when no host opened one.
pub(crate) type Opening = std::sync::mpsc::Receiver<Option<(cpal::Stream, u32)>>;

/// How long `Mixer::new` waits for the stream (a sound server that
/// accepts but never answers would hold the game's start for good): the
/// game then starts silent and the stream comes in when it opens.
pub(crate) const OPEN_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// How long the watchdog waits for a reopened stream before it tries
/// again (the old opener left to finish or hang on its own).
const OPEN_GIVE_UP: std::time::Duration = std::time::Duration::from_secs(5);

/// [`open_stream`] on a thread of its own, so neither the game's start
/// nor its frames ever wait on the sound server (PipeWire's ALSA plug-in
/// waits for the format without a timeout; the pulse client for its
/// replies): `rate` first, then the device's own.
fn open_async(mix: &Arc<Mutex<Mix>>, rate: Option<u32>, beat: &Beat) -> Opening {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let (mix, beat) = (mix.clone(), beat.clone());
    let spawned = std::thread::Builder::new().name("audio_open".into()).spawn(move || {
        let s = open_stream(&mix, rate, &beat).or_else(|| rate.and_then(|_| open_stream(&mix, None, &beat)));
        // Nobody waiting any more (given up): the stream drops here.
        let _ = tx.send(s);
    });
    if let Err(e) = spawned {
        log::warn!("audio: no thread to open the stream: {e}");
    }
    rx
}

/// The device's callback silent this long, or three of its buffers and
/// 100 ms when those are longer (it runs every ~10–25 ms on PipeWire, a
/// buffer of ~0.1–0.5 s on other ALSA plug-ins): the stream is rebuilt
/// ([`Mixer::keep_alive`]).
const STALL_MS: u64 = 400;

/// Whether a callback silent for `since` ms with `period` ms buffers has
/// stopped ([`STALL_MS`]).
pub(crate) fn stalled(since: u64, period: u64) -> bool {
    since >= STALL_MS.max(3 * period + 100)
}

/// The device buffer asked for (frames), when the device's range has it.
const BUFFER_FRAMES: u32 = 1024;

/// The hosts tried, in order: cpal's default (on Linux the PulseAudio
/// host when a pulse server answers, PipeWire's included), then ALSA.
/// NFSU2_AUDIO_HOST (a cpal host name, e.g. `alsa` or `pulseaudio`)
/// picks one alone (a check aid).
fn hosts() -> Vec<cpal::Host> {
    if let Some(name) = std::env::var("NFSU2_AUDIO_HOST").ok().filter(|v| !v.is_empty()) {
        return name.parse::<cpal::HostId>().ok().and_then(|id| cpal::host_from_id(id).ok()).into_iter().collect();
    }
    let first = cpal::default_host();
    let mut v = vec![];
    #[cfg(target_os = "linux")]
    let alsa = (first.id() != cpal::HostId::Alsa).then(|| cpal::host_from_id(cpal::HostId::Alsa).ok()).flatten();
    v.push(first);
    #[cfg(target_os = "linux")]
    v.extend(alsa);
    v
}

/// The callbacks' shared state: `beat` stamped by each callback (ms since
/// `base`), `period` its buffer (ms), `failed` set by an error.
#[derive(Clone)]
pub(crate) struct Beat {
    pub(crate) beat: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) period: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) failed: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) base: std::time::Instant,
    pub(crate) stats: Arc<CallbackStats>,
}

/// A check aid ([`Mixer::take_stats`]): the device callbacks since the
/// last report — their count, the longest wait for the mix lock, the
/// longest render and the longest gap between two callbacks (µs).
#[derive(Default)]
pub(crate) struct CallbackStats {
    pub(crate) n: std::sync::atomic::AtomicU64,
    pub(crate) wait_max: std::sync::atomic::AtomicU64,
    pub(crate) render_max: std::sync::atomic::AtomicU64,
    pub(crate) gap_max: std::sync::atomic::AtomicU64,
    /// The last callback's start (µs since `Beat::base`).
    pub(crate) last: std::sync::atomic::AtomicU64,
}

/// The default output device's stream on `mix` at `rate` (the device's
/// default rate when None), from the first host that opens one
/// ([`hosts`]), and the rate it runs at. None without a device or a
/// usable format.
fn open_stream(mix: &Arc<Mutex<Mix>>, rate: Option<u32>, beat: &Beat) -> Option<(cpal::Stream, u32)> {
    for host in hosts() {
        // The 1024-frame buffer first, then the device's own choice.
        if let Some(s) =
            open_stream_with(&host, mix, rate, true, beat).or_else(|| open_stream_with(&host, mix, rate, false, beat))
        {
            log::info!("audio: {} output at {} Hz", host.id().name(), s.1);
            return Some(s);
        }
    }
    None
}

fn open_stream_with(
    host: &cpal::Host,
    mix: &Arc<Mutex<Mix>>,
    rate: Option<u32>,
    fixed_buffer: bool,
    beat: &Beat,
) -> Option<(cpal::Stream, u32)> {
    let device = host.default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let format = supported.sample_format();
    // A 1024-frame buffer (~23 ms) where the device takes it: PipeWire's
    // ALSA plug-in otherwise asks for 512 frames (11.6 ms at 44.1 kHz),
    // where one late callback is an xrun.
    let fixed = match *supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max }
            if fixed_buffer && min <= BUFFER_FRAMES && BUFFER_FRAMES <= max =>
        {
            Some(BUFFER_FRAMES)
        }
        _ => None,
    };
    if fixed_buffer && fixed.is_none() {
        return None;
    }
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(n) = fixed {
        config.buffer_size = cpal::BufferSize::Fixed(n);
    }
    if let Some(r) = rate {
        config.sample_rate = r;
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, mix, beat),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, mix, beat),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, mix, beat),
        cpal::SampleFormat::I8 => build::<i8>(&device, config, mix, beat),
        cpal::SampleFormat::I24 => build::<cpal::I24>(&device, config, mix, beat),
        cpal::SampleFormat::I64 => build::<i64>(&device, config, mix, beat),
        cpal::SampleFormat::U8 => build::<u8>(&device, config, mix, beat),
        cpal::SampleFormat::U16 => build::<u16>(&device, config, mix, beat),
        cpal::SampleFormat::U24 => build::<cpal::U24>(&device, config, mix, beat),
        cpal::SampleFormat::U32 => build::<u32>(&device, config, mix, beat),
        cpal::SampleFormat::U64 => build::<u64>(&device, config, mix, beat),
        cpal::SampleFormat::F64 => build::<f64>(&device, config, mix, beat),
        other => {
            // DSD and later formats: named once, so a silent game says why.
            warn_once(&BAD_FORMAT, || format!("the output device's sample format {other:?} is not supported"));
            None
        }
    }?;
    stream.play().ok()?;
    beat.beat.store(beat.base.elapsed().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
    beat.failed.store(false, std::sync::atomic::Ordering::Relaxed);
    Some((stream, config.sample_rate))
}

/// The output stream in sample format `T`: each callback stamps the
/// beat and renders the mix (f32) into it.
fn build<T: cpal::SizedSample + cpal::FromSample<f32> + 'static>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mix: &Arc<Mutex<Mix>>,
    beat: &Beat,
) -> Option<cpal::Stream> {
    use std::sync::atomic::Ordering::Relaxed;
    let channels = config.channels as usize;
    let sr = u64::from(config.sample_rate.max(1));
    let (m, b, f) = (mix.clone(), beat.clone(), beat.failed.clone());
    let mut tmp: Vec<f32> = Vec::new();
    let data = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        let t0 = b.base.elapsed();
        let us = t0.as_micros() as u64;
        b.beat.store(us / 1000, Relaxed);
        b.period.store((out.len() / channels.max(1)) as u64 * 1000 / sr, Relaxed);
        // Allocates only when the device's buffer grows.
        tmp.clear();
        tmp.resize(out.len(), 0.0);
        let mut mix = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let t1 = b.base.elapsed();
        mix.render(&mut tmp, channels);
        drop(mix);
        let t2 = b.base.elapsed();
        for (o, s) in out.iter_mut().zip(&tmp) {
            *o = T::from_sample(*s);
        }
        let st = &b.stats;
        let last = st.last.swap(us, Relaxed);
        if st.n.fetch_add(1, Relaxed) > 0 {
            st.gap_max.fetch_max(us.saturating_sub(last), Relaxed);
        }
        st.wait_max.fetch_max((t1 - t0).as_micros() as u64, Relaxed);
        st.render_max.fetch_max((t2 - t1).as_micros() as u64, Relaxed);
    };
    let err = move |e| {
        log::warn!("audio stream: {e}");
        f.store(true, Relaxed);
    };
    device.build_output_stream(config, data, err, None).map_err(|e| log::warn!("audio: {e}")).ok()
}

impl Mixer {
    /// The default output device's stream, or `None` (no device, an
    /// unsupported format, or the stream failing to start).
    pub fn new() -> Option<Mixer> {
        Mixer::start().finish()
    }

    /// [`Mixer::new`] in two halves: the stream's opening started on its
    /// thread now (the game's start, beside its data and its window), and
    /// taken by [`MixerOpening::finish`], which waits what is left of
    /// [`OPEN_WAIT`] since this start. The stream renders the mix from its
    /// opening on, as before (an empty mix: silence until a sound is
    /// queued).
    pub fn start() -> MixerOpening {
        let base = std::time::Instant::now();
        let beat = Beat {
            beat: Default::default(),
            period: Default::default(),
            failed: Default::default(),
            base,
            stats: Default::default(),
        };
        // The rate is the device's (set again from the stream's config).
        let mix = Arc::new(Mutex::new(Mix::new(48000)));
        let opening = open_async(&mix, None, &beat);
        MixerOpening { mix, beat, opening, base }
    }
}

/// A device stream being opened for a new mixer ([`Mixer::start`]).
pub struct MixerOpening {
    mix: Arc<Mutex<Mix>>,
    beat: Beat,
    opening: Opening,
    base: std::time::Instant,
}

impl MixerOpening {
    /// The mixer, its stream taken when it opened within [`OPEN_WAIT`] of
    /// [`Mixer::start`] (else it comes in later, `Device::pending`); None
    /// without a device, as [`Mixer::new`].
    pub fn finish(self) -> Option<Mixer> {
        let MixerOpening { mix, beat, opening, base } = self;
        let mut device = Device { stream: None, beat, checked: base, restarted: None, restarts: 0, pending: None };
        match opening.recv_timeout(OPEN_WAIT.saturating_sub(base.elapsed())) {
            Ok(Some((stream, rate))) => {
                mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_rate(rate);
                device.stream = Some(stream);
            }
            Ok(None) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                log::warn!(
                    "audio: the sound server has not answered in {OPEN_WAIT:?}: starting without sound until it does"
                );
                device.pending = Some((opening, base));
            }
        }
        Some(Mixer { mix, device: Some(Mutex::new(device)) })
    }
}

impl Mixer {
    /// Once a frame: a device stream that went quiet or failed is rebuilt
    /// on the same mix (at most once a second), from the first host that
    /// opens one ([`hosts`]). The last resort: cpal 0.17.3's ALSA worker
    /// spun for good after an xrun (PipeWire's plug-in gives POLLERR while
    /// the PCM is in XRUN; cpal reported it and polled again, never calling
    /// prepare), the "no in-game sound" report of 2026-11-01; 0.18 recovers
    /// by itself and on Linux the PulseAudio host comes first. Still caught
    /// here: a sound server that restarts or a stream that stops calling
    /// back for any other reason. Not native (the original's DirectSound
    /// has no such state); the check keeps the clone's output alive.
    pub fn keep_alive(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        let Some(d) = self.device.as_ref() else { return };
        let mut d = d.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = std::time::Instant::now();
        // A stream being opened: taken when it is there, given up after
        // OPEN_GIVE_UP (the next check opens another).
        if let Some((opening, since)) = d.pending.take() {
            match opening.try_recv() {
                Ok(Some((stream, r))) => {
                    let mut m = self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    m.set_rate(r);
                    drop(m);
                    d.stream = Some(stream);
                    log::info!("audio: the device stream is open ({:.0} ms)", since.elapsed().as_secs_f32() * 1000.0);
                }
                Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    log::warn!("audio: could not reopen the device stream")
                }
                Err(std::sync::mpsc::TryRecvError::Empty) if since.elapsed() < OPEN_GIVE_UP => {
                    d.pending = Some((opening, since))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    log::warn!("audio: the device stream did not open in {OPEN_GIVE_UP:?}: trying again")
                }
            }
            d.checked = now;
            return;
        }
        // The game's own frames stalled (a load, a breakpoint): the
        // callback's silence then says nothing.
        let gap = now - d.checked;
        d.checked = now;
        if gap.as_millis() as u64 > STALL_MS / 2 {
            return;
        }
        let since = (d.beat.base.elapsed().as_millis() as u64).saturating_sub(d.beat.beat.load(Relaxed));
        // An error alone (an underrun cpal recovered from) is no reason:
        // only a callback that stopped.
        if !stalled(since, d.beat.period.load(Relaxed)) {
            return;
        }
        let failed = d.beat.failed.swap(false, Relaxed);
        if d.restarted.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) {
            return;
        }
        d.restarted = Some(now);
        d.restarts += 1;
        log::warn!(
            "audio: the device stream {} ({} ms without a callback): reopening it (restart {})",
            if failed { "reported an error" } else { "stalled" },
            since,
            d.restarts
        );
        // The old stream's drop joins its worker, which may be stuck: on a
        // thread of its own, so the game never waits for it.
        // Without a thread (the process at its limit) the closure, and the
        // stream with it, drops here instead: a wait, not a panic.
        if let Some(old) = d.stream.take()
            && let Err(e) = std::thread::Builder::new().spawn(move || drop(old))
        {
            log::warn!("audio: no thread to close the old stream ({e}): closed here");
        }
        // Opened on a thread of its own (taken by a later check).
        let rate = self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).rate;
        d.pending = Some((open_async(&self.mix, Some(rate), &d.beat), now));
    }

    /// The device stream's restarts so far ([`Mixer::keep_alive`]).
    pub fn restarts(&self) -> u32 {
        self.device.as_ref().map_or(0, |d| d.lock().unwrap_or_else(std::sync::PoisonError::into_inner).restarts)
    }
}
