//! Low Latency mode and VRR-aware pacing (docs/playing/modern-extras.md;
//! clone deviation, opt-in, nothing of it is in SPEED2.EXE).
//!
//! The original samples its input at the top of its loop (0x5811D0) and
//! presents at the bottom with D3DPRESENT_INTERVAL_ONE when vsync is on
//! (0x5B9B13): with the driver's queue full the wait for a free back
//! buffer comes *after* the input was read, so every queued frame adds a
//! refresh of latency. The clone's window loop has the same shape
//! (`Game::redraw`: input and the world at the top, the swap chain's image
//! acquired in `draw`). Low Latency moves that wait before the input:
//!
//! * the swap chain is asked for one frame in flight instead of two
//!   (`desired_maximum_frame_latency`), so no hidden queue builds;
//! * [`Pacer`]: the time each frame was blocked after its input (the last
//!   frame's end and the image's acquire) is fed back; the next frame
//!   sleeps that long, less a small margin, *before* it reads the input,
//!   so the input is as fresh as the display allows (an integrator on the
//!   blocked time; it settles where the frame meets its refresh with
//!   [`SLACK`] to spare);
//! * with a variable-refresh display active ([`vrr`]) the frames are held
//!   [`VRR_HEADROOM`] under its top rate, so the swap chain never queues
//!   and the panel stays in its variable range.
//!
//! The world's cadence (`frame_clock::Cadence`, fixed 1/60 s updates, drawn
//! frames interpolated) is not touched: the pacer only moves *when* a frame
//! starts; the cadence takes the real time that passed, as before.

use std::time::{Duration, Instant};

/// Time a paced frame keeps in hand before its deadline (s): its blocked
/// time settles here, not at zero, so a frame a little slower than the
/// last still makes its refresh.
pub const SLACK: f32 = 0.0015;

/// How much of each frame's error the delay takes (the integrator's gain).
pub const GAIN: f32 = 0.35;

/// The longest delay before the input (s): no display waits longer for a
/// free image (a 20 Hz refresh).
pub const MAX_DELAY: f32 = 0.05;

/// A VRR display's frames are capped this share under its top rate: the
/// usual 3 %, so a frame never waits for the panel's fastest refresh.
pub const VRR_HEADROOM: f64 = 0.97;

/// One frame's times (seconds on any clock): the input sample, the
/// acquire's start and end (`ready`, `got`: the end is a refresh when the
/// acquire blocked), and the present (the frame handed to the display).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTimes {
    pub input: f64,
    pub ready: f64,
    pub got: f64,
    pub present: f64,
}

/// The Low Latency pacer: when a frame reads its input.
///
/// With the display's refresh period known (the window's monitor), a
/// blocked acquire returns at a refresh: that is the refresh grid's phase.
/// Each frame then starts so that it is presented [`SLACK`] before the
/// next refresh it can make: input = refresh − the recent frames' work
/// (their 90th percentile, input to acquire plus acquire to present) −
/// [`SLACK`]. A frame that still blocks (the work grew, the period drifted)
/// gives the grid again. Without a known period the fallback is an
/// integrator on the blocked time (the sleep moves toward the time the
/// frames were blocked, less the slack).
#[derive(Clone, Debug, Default)]
pub struct Pacer {
    period: Option<f64>,
    /// A refresh (when a blocked acquire returned).
    anchor: Option<f64>,
    /// The last frame's present.
    last_present: Option<f64>,
    work: std::collections::VecDeque<f64>,
    delay: f32,
    /// Frames fed back, and the blocked time's running mean.
    pub frames: u64,
    pub blocked_mean: f32,
}

/// Frames of work the prediction looks back on.
const WORK_FRAMES: usize = 60;

impl Pacer {
    /// The display's refresh period (s), when known.
    pub fn set_period(&mut self, period: Option<f64>) {
        if self.period != period {
            self.period = period;
            self.anchor = None;
        }
    }

    /// The period the pacer locks to under the present mode: the refresh
    /// with vsync (FIFO: a frame can only go out at a refresh), none with
    /// vsync off (Mailbox / Immediate: locking to the refresh there held
    /// the frame rate at the refresh rate, report #349; the integrator on
    /// the just-in-time GPU wait paces those).
    pub fn for_present(vsync: bool, refresh: Option<f64>) -> Option<f64> {
        if vsync { refresh } else { None }
    }

    /// The fallback's sleep before the input (s).
    pub fn delay(&self) -> f32 {
        self.delay
    }

    /// The recent frames' work (s): their 90th percentile.
    pub fn work(&self) -> f64 {
        if self.work.is_empty() {
            return 0.0;
        }
        let mut v: Vec<f64> = self.work.iter().copied().collect();
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * 0.9).round() as usize]
    }

    /// When the frame starting at `now` reads its input.
    pub fn due(&self, now: f64) -> f64 {
        match (self.period, self.anchor) {
            (Some(period), Some(anchor)) => {
                let lead = self.work() + f64::from(SLACK);
                if lead >= period {
                    return now;
                }
                // The refresh that shows the last frame: with one frame in
                // flight its image is free only then, so this frame aims
                // at the one after (and no sooner than it can make).
                let grid = |t: f64| anchor + ((t - anchor) / period).ceil() * period;
                let shown = grid(self.last_present.unwrap_or(now) - 1e-9);
                let refresh = grid(now + lead).max(shown + period);
                (refresh - lead).max(now)
            }
            _ => now + f64::from(self.delay),
        }
    }

    /// A frame's end.
    pub fn record(&mut self, t: FrameTimes) {
        let blocked = (t.got - t.ready).max(0.0) as f32;
        if blocked > 0.0003 {
            self.anchor = Some(t.got);
        }
        let work = (t.ready - t.input).max(0.0) + (t.present - t.got).max(0.0);
        self.last_present = Some(t.present);
        self.work.push_back(work);
        while self.work.len() > WORK_FRAMES {
            self.work.pop_front();
        }
        // The fallback integrator.
        let err = blocked - SLACK;
        self.delay = (self.delay + GAIN * err).clamp(0.0, MAX_DELAY);
        if blocked < SLACK * 0.25 {
            self.delay = (self.delay - SLACK).max(0.0);
        }
        self.frames += 1;
        let k = 1.0 / (self.frames.min(120) as f32);
        self.blocked_mean += (blocked - self.blocked_mean) * k;
    }

    /// Starts over (a new swap chain, a mode change); the period is kept.
    pub fn reset(&mut self) {
        *self = Pacer { period: self.period, ..Pacer::default() };
    }
}

/// The swap chain's frames in flight (`desired_maximum_frame_latency`):
/// two, and one under Low Latency with vsync. With vsync off Low Latency
/// keeps two: with one, a Mailbox swap chain has two images and each
/// acquire waits for a refresh, and on Vulkan / DX12 the CPU waits for the
/// GPU to finish the last frame before it starts the next (no overlap);
/// the just-in-time GPU wait keeps the queue short instead.
pub fn frames_in_flight(low_latency: bool, vsync: bool) -> u32 {
    if low_latency && vsync { 1 } else { 2 }
}

/// The frame period a VRR display gets under Low Latency: the top rate
/// less [`VRR_HEADROOM`]. None without a known rate.
pub fn vrr_period(refresh_hz: Option<f64>) -> Option<Duration> {
    refresh_hz.filter(|&hz| hz > 1.0).map(|hz| Duration::from_secs_f64(1.0 / (hz * VRR_HEADROOM)))
}

/// One frame's latency proxies: from the input sample to the present call
/// (the design's "input to present submit"), and in the display model of
/// [`Display`] to the refresh that shows it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub to_present: f32,
    pub to_photon: f32,
    pub blocked: f32,
}

/// A model of the swap chain and the panel (the bench's "display"): FIFO
/// presentation with at most `queue` frames presented and not yet shown
/// (Vulkan's FIFO with `queue + 1` images), on a fixed refresh, or VRR (a
/// frame is shown when presented, no sooner than one fastest refresh after
/// the last). Times in seconds from any origin.
#[derive(Clone, Debug)]
pub struct Display {
    pub period: f64,
    pub vrr: bool,
    pub queue: usize,
    /// When each queued frame goes on screen.
    shown: std::collections::VecDeque<f64>,
    last_shown: f64,
}

impl Display {
    pub fn new(hz: f64, vrr: bool, queue: usize) -> Display {
        Display { period: 1.0 / hz, vrr, queue: queue.max(1), shown: Default::default(), last_shown: f64::NEG_INFINITY }
    }

    /// The acquire at `now`: when an image is free (the oldest queued frame
    /// goes on screen and frees the one before it).
    pub fn acquire(&mut self, now: f64) -> f64 {
        self.shown.retain(|&t| t > now);
        if self.shown.len() < self.queue {
            return now;
        }
        let free = self.shown[self.shown.len() - self.queue];
        self.shown.retain(|&t| t > free);
        free
    }

    /// A frame presented at `at`: when it goes on screen.
    pub fn present(&mut self, at: f64) -> f64 {
        let after = self.last_shown + self.period;
        let t = if self.vrr {
            at.max(after)
        } else {
            // The next refresh after the present and after the last
            // frame's (one new frame per refresh).
            let first = (at / self.period).ceil() * self.period;
            let mut t = first;
            while t < after - 1e-9 {
                t += self.period;
            }
            t
        };
        self.last_shown = t;
        self.shown.push_back(t);
        t
    }
}

/// Runs `frames` frames of `work(i)` s each (input to present) on a model
/// display, with or without the pacer (and the VRR cap `cap`, s), from a
/// loop that starts the next frame as soon as the last one is presented
/// (the clone's uncapped loop). Returns each frame's proxies. The pure
/// model the tests check; `--bench latency` runs the game's real frames
/// through the same [`Display`] on the real clock.
pub fn simulate(display: &mut Display, frames: usize, low_latency: bool, cap: Option<f64>, work: impl Fn(usize) -> f64) -> Vec<Sample> {
    let mut pacer = Pacer::default();
    pacer.set_period(Some(display.period));
    let mut now = 0.0f64;
    let mut last_start = f64::NEG_INFINITY;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        if let Some(c) = cap {
            now = now.max(last_start + c);
        }
        if low_latency {
            now = pacer.due(now);
        }
        let input = now;
        last_start = input;
        // The work up to the acquire, the wait, the rest.
        let w = work(i);
        let ready = input + w * 0.5;
        let got = display.acquire(ready);
        let present = got + w * 0.5;
        let photon = display.present(present);
        let blocked = (got - ready) as f32;
        if low_latency {
            pacer.record(FrameTimes { input, ready, got, present });
        }
        out.push(Sample { to_present: (present - input) as f32, to_photon: (photon - input) as f32, blocked });
        now = present;
    }
    out
}

/// The swap chain's present mode in the GPU pipeline model ([`pipeline`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Present {
    /// Vsync: every frame shown, in order, one per refresh.
    Fifo,
    /// Vsync off without tearing: the newest finished frame is shown at
    /// each refresh, older ones are dropped.
    Mailbox,
    /// Vsync off, tearing: a frame goes on screen when it is finished.
    Immediate,
}

/// How a frame starts in the pipeline model ([`pipeline`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Low Latency off: right after the last frame was submitted; the
    /// swap chain's acquire (two frames in flight) is the only wait.
    Off,
    /// Low Latency as 0.7.0 had it, kept as a model of report #349's
    /// cause: one frame in flight in every present mode and the
    /// refresh-locked [`Pacer`] even with vsync off.
    Refresh,
    /// Low Latency now ([`Pacer::for_present`]): with vsync the same
    /// refresh-locked pacer and one frame in flight; with vsync off two
    /// frames in flight, the just-in-time GPU wait before the acquire (the
    /// frame before the last one's fence) and the integrator's sleep before
    /// the input, which takes that wait's time.
    JustInTime,
}

/// One run of [`pipeline`]: frames finished per second on the GPU, the GPU's
/// busy share, and the time from a frame's input to its frame finished on
/// the GPU and to the refresh that shows it (means after the settling).
#[derive(Clone, Copy, Debug, Default)]
pub struct PipelineRun {
    pub fps: f64,
    pub gpu_busy: f64,
    pub input_to_done: f64,
    pub input_to_photon: f64,
}

/// A model of the CPU, the GPU and the swap chain together (report #349):
/// each frame takes `cpu` s on the main thread (input to its submission,
/// half before the image's acquire, half after) and `gpu` s on the GPU,
/// in order. The swap chain has `frames in flight + 1` images; the
/// acquire of an image waits (as wgpu's Vulkan swap chain does) for the
/// GPU work of the frame that used it last and for the display to give it
/// back. The pure model the regression tests check; the window loop runs
/// the same [`Pacer`].
pub fn pipeline(hz: f64, present: Present, start: Start, frames: usize, cpu: f64, gpu: f64) -> PipelineRun {
    let period = 1.0 / hz;
    let vsync = present == Present::Fifo;
    let in_flight = match start {
        Start::Off => frames_in_flight(false, vsync),
        Start::Refresh => 1,
        Start::JustInTime => frames_in_flight(true, vsync),
    } as usize;
    let images = in_flight + 1;
    let mut pacer = Pacer::default();
    pacer.set_period(match start {
        Start::Refresh => Some(period),
        _ => Pacer::for_present(vsync, Some(period)),
    });
    let vblank = |t: f64| (t / period).ceil() * period;
    // Per frame: its input, the GPU's end, when it is shown and when its
    // image is free again.
    let (mut input, mut done, mut shown, mut free) =
        (vec![0.0f64; frames], vec![0.0f64; frames], vec![f64::NAN; frames], vec![f64::INFINITY; frames]);
    let mut holder: Vec<Option<usize>> = vec![None; images];
    let mut t = 0.0f64;
    for k in 0..frames {
        // Low Latency's sleep before the input.
        if start != Start::Off {
            t = pacer.due(t);
        }
        input[k] = t;
        let at_acquire = t + cpu * 0.5;
        // Vsync off: the just-in-time wait for the frame before the last
        // one (after the last frame's end went to the GPU, before the
        // acquire: `WindowState::jit_wait`).
        let jit = if start == Start::JustInTime && !vsync && k >= 2 { at_acquire.max(done[k - 2]) } else { at_acquire };
        // The image of frame k − images: its GPU work done and the display
        // done with it.
        // wgpu waits for the GPU work of the acquire `images` back (its
        // semaphore's last use), then the display gives the image that is
        // free first (the last frame's is not: it waits for this one).
        let fence = if k >= images { done[k - images] } else { 0.0 };
        let (slot, image_free) = holder
            .iter()
            .enumerate()
            .filter(|(_, h)| h.is_none_or(|j| j + 1 < k))
            .map(|(i, h)| (i, h.map_or(0.0, |j| free[j])))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("an image");
        holder[slot] = Some(k);
        let acquired = jit.max(fence).max(image_free);
        // The time blocked after the input (with vsync off the GPU wait and
        // the acquire together), which the pacer moves before it.
        let (ready, got) = (at_acquire, acquired);
        let submit = acquired + cpu * 0.5;
        let gpu_start = if k > 0 { submit.max(done[k - 1]) } else { submit };
        done[k] = gpu_start + gpu;
        // The display: when this frame is shown and the frame before it
        // gives its image back.
        match present {
            Present::Immediate => {
                shown[k] = done[k];
                if k > 0 {
                    free[k - 1] = done[k];
                }
            }
            Present::Fifo => {
                let after = if k > 0 { shown[k - 1] + period } else { 0.0 };
                shown[k] = vblank(done[k]).max(after);
                if k > 0 {
                    free[k - 1] = shown[k];
                }
            }
            Present::Mailbox => {
                shown[k] = vblank(done[k]);
                if k > 0 {
                    // Frame k − 1 dropped when k replaces it before its
                    // refresh, else let go when k goes on screen.
                    if done[k] < shown[k - 1] {
                        free[k - 1] = done[k];
                        shown[k - 1] = f64::NAN;
                    } else {
                        free[k - 1] = shown[k];
                    }
                }
            }
        }
        if start != Start::Off {
            pacer.record(FrameTimes { input: input[k], ready, got, present: submit });
        }
        t = submit;
    }
    let skip = frames / 4;
    let n = (frames - skip) as f64;
    let span = done[frames - 1] - done[skip];
    let shown_after: Vec<f64> = (skip..frames).filter(|&k| shown[k].is_finite()).map(|k| shown[k] - input[k]).collect();
    PipelineRun {
        fps: (frames - 1 - skip) as f64 / span,
        gpu_busy: gpu * (frames - 1 - skip) as f64 / span,
        input_to_done: (skip..frames).map(|k| done[k] - input[k]).sum::<f64>() / n,
        input_to_photon: shown_after.iter().sum::<f64>() / shown_after.len().max(1) as f64,
    }
}

/// Mean of a slice of samples' fields (skipping the first `skip`, the
/// settling).
pub fn mean(samples: &[Sample], skip: usize, f: impl Fn(&Sample) -> f32) -> f32 {
    let s = &samples[skip.min(samples.len())..];
    if s.is_empty() {
        return 0.0;
    }
    s.iter().map(f).sum::<f32>() / s.len() as f32
}

/// Sleeps until `due` as the frame limiter does (`frame_clock::wait_until`).
pub fn wait_until(due: Instant) {
    crate::frame_clock::wait_until(due);
}

/// Variable refresh detection. wgpu exposes no VRR state (its surface
/// capabilities list present modes only), so the platform is asked:
///
/// * **Linux** (X11 and Wayland alike): the DRM connector property
///   `vrr_capable` (immutable, set by the kernel from the panel's EDID /
///   DPCD) on every connected connector, and the CRTC property
///   `VRR_ENABLED` (set by the compositor or X server while it drives the
///   panel variably: KWin / Mutter / gamescope for a full-screen window,
///   Xorg with `VariableRefresh` for a full-screen window carrying
///   `_VARIABLE_REFRESH`, which Mesa's Vulkan WSI sets). Read through the
///   KMS ioctls on `/dev/dri/card*` without DRM master (read-only
///   GETRESOURCES / OBJ_GETPROPERTIES / GETPROPERTY; no connector probe).
/// * **Windows / macOS**: not probed (**open**): VRR is reported unknown,
///   so the VRR cap does not engage; the blocked-time pacer still works.
pub mod vrr {
    /// What the probe found.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Vrr {
        /// Connected outputs whose panel can refresh variably.
        pub capable: Vec<String>,
        /// CRTCs driven variably right now.
        pub active: usize,
        /// Whether the platform could be asked at all.
        pub known: bool,
    }

    impl Vrr {
        pub fn capable(&self) -> bool {
            !self.capable.is_empty()
        }
        pub fn active(&self) -> bool {
            self.active > 0
        }
        pub fn describe(&self) -> String {
            if !self.known {
                return "VRR unknown on this platform".into();
            }
            match (self.capable(), self.active()) {
                (false, _) => "no VRR-capable output".into(),
                (true, false) => format!("VRR-capable ({}), not active", self.capable.join(", ")),
                (true, true) => format!("VRR active ({})", self.capable.join(", ")),
            }
        }
    }

    /// Asks the platform (see the module).
    pub fn probe() -> Vrr {
        #[cfg(target_os = "linux")]
        {
            linux::probe()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Vrr::default()
        }
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use super::Vrr;
        use std::os::fd::AsRawFd;

        const OBJ_CONNECTOR: u32 = 0xc0c0_c0c0;
        const OBJ_CRTC: u32 = 0xcccc_cccc;

        #[repr(C)]
        #[derive(Default)]
        struct CardRes {
            fb_id_ptr: u64,
            crtc_id_ptr: u64,
            connector_id_ptr: u64,
            encoder_id_ptr: u64,
            count_fbs: u32,
            count_crtcs: u32,
            count_connectors: u32,
            count_encoders: u32,
            min_width: u32,
            max_width: u32,
            min_height: u32,
            max_height: u32,
        }

        #[repr(C)]
        #[derive(Default)]
        struct ObjGetProperties {
            props_ptr: u64,
            prop_values_ptr: u64,
            count_props: u32,
            obj_id: u32,
            obj_type: u32,
            pad: u32,
        }

        #[repr(C)]
        struct GetProperty {
            values_ptr: u64,
            enum_blob_ptr: u64,
            prop_id: u32,
            flags: u32,
            name: [u8; 32],
            count_values: u32,
            count_enum_blobs: u32,
        }

        /// _IOWR('d', nr, size).
        const fn iowr(nr: u32, size: usize) -> u64 {
            ((3u64) << 30) | ((size as u64) << 16) | (0x64 << 8) | nr as u64
        }

        fn ioctl<T>(fd: i32, request: u64, arg: &mut T) -> bool {
            // SAFETY: `arg` is the kernel's struct for `request` (sizes in
            // the request match the #[repr(C)] layouts above).
            unsafe { libc::ioctl(fd, request as _, arg as *mut T) == 0 }
        }

        /// (name, value) of every property of a KMS object.
        fn properties(fd: i32, id: u32, kind: u32) -> Vec<(String, u64)> {
            let mut q = ObjGetProperties { obj_id: id, obj_type: kind, ..Default::default() };
            if !ioctl(fd, iowr(0xB9, std::mem::size_of::<ObjGetProperties>()), &mut q) || q.count_props == 0 {
                return Vec::new();
            }
            let mut ids = vec![0u32; q.count_props as usize];
            let mut values = vec![0u64; q.count_props as usize];
            q.props_ptr = ids.as_mut_ptr() as u64;
            q.prop_values_ptr = values.as_mut_ptr() as u64;
            if !ioctl(fd, iowr(0xB9, std::mem::size_of::<ObjGetProperties>()), &mut q) {
                return Vec::new();
            }
            let n = (q.count_props as usize).min(ids.len());
            (0..n)
                .filter_map(|i| {
                    let mut p = GetProperty { values_ptr: 0, enum_blob_ptr: 0, prop_id: ids[i], flags: 0, name: [0; 32], count_values: 0, count_enum_blobs: 0 };
                    ioctl(fd, iowr(0xAA, std::mem::size_of::<GetProperty>()), &mut p).then(|| {
                        let end = p.name.iter().position(|&b| b == 0).unwrap_or(32);
                        (String::from_utf8_lossy(&p.name[..end]).into_owned(), values[i])
                    })
                })
                .collect()
        }

        pub fn probe() -> Vrr {
            let mut out = Vrr::default();
            let Ok(dir) = std::fs::read_dir("/sys/class/drm") else { return out };
            let mut cards: Vec<String> = Vec::new();
            let mut connectors: Vec<(String, String, u32)> = Vec::new();
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let Some(rest) = name.strip_prefix("card") else { continue };
                match rest.split_once('-') {
                    None if rest.chars().all(|c| c.is_ascii_digit()) => cards.push(name.clone()),
                    Some((card, conn)) => {
                        let status = std::fs::read_to_string(e.path().join("status")).unwrap_or_default();
                        let id = std::fs::read_to_string(e.path().join("connector_id")).ok().and_then(|s| s.trim().parse::<u32>().ok());
                        if status.trim() == "connected"
                            && let Some(id) = id
                        {
                            connectors.push((format!("card{card}"), conn.to_string(), id));
                        }
                    }
                    _ => {}
                }
            }
            cards.sort();
            for card in cards {
                let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(format!("/dev/dri/{card}")).or_else(|_| std::fs::File::open(format!("/dev/dri/{card}"))) else { continue };
                let fd = file.as_raw_fd();
                out.known = true;
                for (_, conn, id) in connectors.iter().filter(|(c, _, _)| *c == card) {
                    if properties(fd, *id, OBJ_CONNECTOR).iter().any(|(n, v)| n == "vrr_capable" && *v != 0) {
                        out.capable.push(conn.clone());
                    }
                }
                let mut res = CardRes::default();
                if !ioctl(fd, iowr(0xA0, std::mem::size_of::<CardRes>()), &mut res) || res.count_crtcs == 0 {
                    continue;
                }
                let mut crtcs = vec![0u32; res.count_crtcs as usize];
                let mut res2 = CardRes { crtc_id_ptr: crtcs.as_mut_ptr() as u64, count_crtcs: crtcs.len() as u32, ..Default::default() };
                if !ioctl(fd, iowr(0xA0, std::mem::size_of::<CardRes>()), &mut res2) {
                    continue;
                }
                let n = (res2.count_crtcs as usize).min(crtcs.len());
                out.active += crtcs[..n].iter().filter(|&&c| properties(fd, c, OBJ_CRTC).iter().any(|(n, v)| n == "VRR_ENABLED" && *v != 0)).count();
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_layouts() {
        // The kernel's sizes (drm_mode.h): the requests encode them.
        assert_eq!(std::mem::size_of::<[u64; 4]>() + 8 * 4, 64);
        assert_eq!(vrr_period(Some(144.0)).map(|d| (d.as_secs_f64() * 1e6).round()), Some(7159.0));
        assert_eq!(vrr_period(None), None);
    }

    /// The platform probe runs without DRM master and never panics
    /// (prints what it found: on the dev machine DP-1 / DP-3 capable).
    #[test]
    fn vrr_probe_runs() {
        let v = vrr::probe();
        eprintln!("vrr probe: {} {:?}", v.describe(), v);
        if !v.known {
            assert!(!v.capable() && !v.active());
        }
    }

    #[test]
    fn fifo_queue_and_vrr_models() {
        // FIFO 60 Hz, 2 frames queued, 2 ms of work: the loop fills the
        // queue, then every frame waits about a refresh after its input.
        let mut d = Display::new(60.0, false, 2);
        let off = simulate(&mut d, 400, false, None, |_| 0.002);
        let photon_off = mean(&off, 100, |s| s.to_photon);
        assert!(photon_off > 0.040 && photon_off < 0.052, "{photon_off}");
        // Shown frames one per refresh.
        assert!((mean(&off, 100, |s| s.to_present) - 0.0167).abs() < 0.002);
    }

    #[test]
    fn low_latency_moves_the_wait_before_the_input() {
        for hz in [60.0, 144.0] {
            for work in [0.002, 0.005] {
                let period = 1.0 / hz;
                let mut d = Display::new(hz, false, 2);
                let off = simulate(&mut d, 600, false, None, |_| work);
                let mut d = Display::new(hz, false, 1);
                let on = simulate(&mut d, 600, true, None, |_| work);
                let (a, b) = (mean(&off, 200, |s| s.to_photon), mean(&on, 200, |s| s.to_photon));
                // On: about the work and the slack, well under a refresh.
                assert!(b < a * 0.5 && f64::from(b) < work + 0.0045, "{hz} Hz {work}: off {a} on {b}");
                // Still one frame per refresh: no missed refreshes.
                let presents: Vec<f32> = on[200..].iter().map(|s| s.to_present).collect();
                assert!(presents.iter().all(|&p| f64::from(p) < period), "{hz}: {presents:?}");
            }
        }
    }

    #[test]
    fn jittery_work_keeps_the_refresh() {
        // ±1 ms of noise on 4 ms of work at 144 Hz: the slack holds.
        let mut d = Display::new(144.0, false, 1);
        let on = simulate(&mut d, 2000, true, None, |i| 0.004 + 0.001 * (((i * 7919) % 13) as f64 / 6.0 - 1.0));
        let photons: Vec<f64> = {
            let mut d2 = Display::new(144.0, false, 1);
            let mut t = 0.0;
            on.iter().map(|s| { t += f64::from(s.to_present); d2.present(t) }).collect()
        };
        let _ = photons;
        let lat = mean(&on, 500, |s| s.to_photon);
        assert!(f64::from(lat) < 0.0095, "{lat}");
    }

    #[test]
    fn vrr_cap_keeps_the_queue_empty() {
        // 144 Hz VRR, 2 ms of work, uncapped: the queue fills at the top
        // rate. With the cap 3 % under it nothing waits.
        let mut d = Display::new(144.0, true, 2);
        let off = simulate(&mut d, 600, false, None, |_| 0.002);
        let mut d = Display::new(144.0, true, 1);
        let cap = vrr_period(Some(144.0)).map(|p| p.as_secs_f64());
        let on = simulate(&mut d, 600, true, cap, |_| 0.002);
        let (a, b) = (mean(&off, 200, |s| s.to_photon), mean(&on, 200, |s| s.to_photon));
        assert!(b < 0.0025 && a > 0.010, "off {a} on {b}");
        assert!(mean(&on, 200, |s| s.blocked) < 0.0005);
    }

    #[test]
    fn mailbox_like_displays_are_left_alone() {
        // A display that never blocks: the pacer stays at zero.
        let mut p = Pacer::default();
        let frame = |t: f64, blocked: f64| FrameTimes { input: t, ready: t + 0.001, got: t + 0.001 + blocked, present: t + 0.002 + blocked };
        for i in 0..100 {
            p.record(frame(f64::from(i) * 0.003, 0.0));
        }
        assert_eq!(p.delay(), 0.0);
        assert_eq!(p.due(5.0), 5.0);
        p.record(frame(1.0, 0.010));
        assert!(p.delay() > 0.0);
        // With a period, the grid from the blocked acquire: input lands
        // the work and the slack before a refresh.
        p.set_period(Some(1.0 / 60.0));
        p.record(frame(1.0, 0.010));
        let due = p.due(1.02);
        let at = (due + p.work() + f64::from(SLACK) - 1.011) * 60.0;
        assert!(due >= 1.02 && (at - at.round()).abs() < 1e-6, "{due} {at}");
        p.reset();
        assert_eq!(p.delay(), 0.0);
    }
    /// Report #349: GPU-bound with vsync off, the old Low Latency held the
    /// frame rate at the refresh (one frame in flight, the pacer locked to
    /// the refresh); now it stays within a few % of Off and the time from
    /// the input to the GPU's end drops.
    #[test]
    fn vsync_off_low_latency_keeps_the_frame_rate() {
        for present in [Present::Mailbox, Present::Immediate] {
            for hz in [60.0, 144.0] {
                for (cpu, gpu) in [(0.0015, 0.0055), (0.002, 0.005), (0.004, 0.0045), (0.006, 0.003)] {
                    let off = pipeline(hz, present, Start::Off, 2000, cpu, gpu);
                    let old = pipeline(hz, present, Start::Refresh, 2000, cpu, gpu);
                    let on = pipeline(hz, present, Start::JustInTime, 2000, cpu, gpu);
                    eprintln!(
                        "{present:?} {hz} Hz cpu {cpu} gpu {gpu}: off {off:.4?}\n  old {old:.4?}\n  new {on:.4?}"
                    );
                    assert!(on.fps > off.fps * 0.95, "{present:?} {hz} {cpu} {gpu}: on {} off {}", on.fps, off.fps);
                    assert!(
                        on.input_to_done <= off.input_to_done + 1e-4,
                        "{present:?} {hz} {cpu} {gpu}: on {} off {}",
                        on.input_to_done,
                        off.input_to_done
                    );
                    if gpu > cpu * 1.5 {
                        // GPU-bound: the queue behind the input shorter by a millisecond or more.
                        assert!(
                            on.input_to_done < off.input_to_done - 0.001,
                            "{present:?} {hz} {cpu} {gpu}: {}",
                            on.input_to_done
                        );
                        assert!(
                            on.gpu_busy > off.gpu_busy * 0.95,
                            "{present:?} {hz} {cpu} {gpu}: busy {}",
                            on.gpu_busy
                        );
                    }
                    // The cause: the old pacing held the rate near the refresh.
                    if gpu > cpu && 1.0 / gpu > hz * 1.2 {
                        assert!(old.fps < hz * 1.05, "{present:?} {hz} {cpu} {gpu}: old {}", old.fps);
                    }
                }
            }
        }
    }

    /// With vsync Low Latency is as before: one frame in flight, the
    /// refresh kept, the latency down.
    #[test]
    fn vsync_low_latency_is_unchanged() {
        for hz in [60.0, 144.0] {
            // A quiet GPU (the pacer's slack holds the GPU's time).
            let old = pipeline(hz, Present::Fifo, Start::Refresh, 1200, 0.002, 0.0005);
            let on = pipeline(hz, Present::Fifo, Start::JustInTime, 1200, 0.002, 0.0005);
            let off = pipeline(hz, Present::Fifo, Start::Off, 1200, 0.002, 0.0005);
            assert!((on.fps - old.fps).abs() < 1e-6 && (on.input_to_photon - old.input_to_photon).abs() < 1e-9);
            assert!(
                (on.fps - hz).abs() < hz * 0.03 && on.input_to_photon < off.input_to_photon * 0.6,
                "{hz}: on {on:?} off {off:?}"
            );
        }
        assert_eq!(frames_in_flight(true, true), 1);
        assert_eq!(frames_in_flight(true, false), 2);
        assert_eq!(frames_in_flight(false, true), 2);
        assert_eq!(Pacer::for_present(false, Some(0.01)), None);
    }
}
