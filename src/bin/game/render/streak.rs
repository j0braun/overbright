//! The light-streak buffer commits once per tick, from that tick's own
//! frame (docs/original/uncapped-fps.md, "Streak buffer on ticks").
//!
//! The original runs one streak pass per loop iteration (0x5CBB10 from the
//! world render 0x5CAAE0): decay, the frame's glow sprites, then the
//! overbright pass's A · factor (0x5CCB9A), with A made from that frame's
//! scene. At 60 fps a frame is a tick, so commit n is a function of the
//! frame tick n draws: its glows (drawn with its camera, depth-tested
//! against its scene), its overbright level, its scene. The commits are
//! keyed on the newest tick a draw stands after (the slot =
//! `GameClock::tick`): the frame drawn after tick n commits slot n with what
//! it draws, as the 60 Hz frame after tick n does, whatever α it is drawn
//! at (the harness's α = 1, the window's α = 0).
//!
//! Two forms:
//! - **Exact** (probes): a slot no drawn frame stands on exactly (a 30 Hz
//!   frame's first tick, a stall's, the tick before a frame between ticks
//!   at 75 / 144 / 200 Hz) is drawn as a **probe**: the 60 Hz frame of that
//!   slot, rendered offscreen (the world's scene and the post chain up to
//!   the commit, no output, no UI, no present) where the 60 Hz frame would
//!   be drawn — between two ticks of a frame (`Game::probe_between_ticks`)
//!   or before a frame between ticks (`Game::draw_at`). The probe's Glows
//!   pass gives the commit its tick's glow list and depth.
//! - **Cheap**: a frame commits the slots it missed itself (as many as the
//!   ticks it took, at most 3), each with its own glows and A. The number of
//!   commits per second is right at any rate (trail length and decay per
//!   second as at 60 fps); only the commit's scene is the frame's instead
//!   of its tick's: a sub-tick phase error, up to one frame interval.
//!
//! A frame between ticks shows the scratch B(α) over the committed buffer
//! (`StreakFrame::scratch`, not kept). The harness (shots, flows, the rate
//! gate, the tests: a deterministic clock) is always exact. The window
//! chooses per frame from measured work ([`Governor`]): exact while the
//! probes cost the machine little, cheap otherwise; the temporal pass
//! (TAA / temporal upscaling: a history per drawn frame a probe would feed)
//! is always cheap.
use crate::*;
use nfsu2::render::postfx::StreakFrame;

/// The streak buffer's commit bookkeeping (render side).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StreakTicks {
    /// The newest slot committed (`GameClock::tick` of the draw that
    /// committed it).
    pub(crate) seen: Option<u64>,
}

impl StreakTicks {
    /// Whether slot `tick` is not committed yet.
    pub(crate) fn due(&self, tick: u64) -> bool {
        self.seen.is_none_or(|s| tick > s)
    }

    /// What a draw after tick `tick` does to the buffer, and the slot
    /// marked committed. `probe`: a probe (one commit of its slot, if due);
    /// else a drawn frame at `alpha` (∈ (0, 1): between ticks, the scratch
    /// shown) that took `frame_ticks` ticks; `probed`: probes run for the
    /// slots before it (else it commits the missed ones itself).
    pub(crate) fn frame(&mut self, tick: u64, frame_ticks: u32, alpha: f32, probe: bool, probed: bool) -> StreakFrame {
        let missing = match self.seen {
            None => 1,
            Some(s) => tick.saturating_sub(s),
        };
        if missing > 0 {
            self.seen = Some(tick);
        }
        let commits = match missing {
            0 => 0,
            _ if probe || probed => 1,
            m => m.min(u64::from(frame_ticks.max(1))).min(3) as u32,
        };
        // Between ticks the scratch B(α) over the committed buffer; in the
        // cheap form a frame that commits shows what it committed (its own
        // glows are in it already), which saves the scratch's two copies
        // of the buffer on nearly every frame of a machine under 60 fps.
        let between = !probe && alpha > 0.0 && alpha < 1.0 && (probed || commits == 0);
        StreakFrame::tick(commits, if between { 0.5 } else { 0.0 })
    }
}

/// The streak buffer's state on the render side: the commits, the window's
/// choice of form and what it is measured from.
#[derive(Default)]
pub(crate) struct StreakState {
    pub(crate) ticks: StreakTicks,
    pub(crate) gov: Governor,
    /// This drawn frame commits the cheap way (no probes): set per window
    /// frame ([`Governor::frame_probes`]); never in the harness unless forced.
    pub(crate) cheap_frame: bool,
    /// Probes drawn in this window frame.
    pub(crate) probes: u32,
    /// This window frame's probes: their wall time and, of it, their
    /// waits for the frame before (`settle`), s (the performance overlay).
    pub(crate) probe_wall: f32,
    pub(crate) probe_settle: f32,
    /// GPU time per scene render (frame or probe), from timestamp queries.
    timer: Option<GpuTimer>,
    timer_tried: bool,
    pub(crate) gpu_cost: Option<f32>,
    /// Frames fed to the governor, those drawn in exact form, form changes
    /// and probes drawn (the bench's summary).
    pub(crate) frames: u64,
    pub(crate) exact_frames: u64,
    pub(crate) changes: u64,
    pub(crate) probes_total: u64,
    /// The monitor's refresh rate and when it was read.
    pub(crate) refresh: Option<(Option<f32>, std::time::Instant)>,
    /// The display period the bench's schedule stands for (its summary).
    pub(crate) bench_display: Option<f32>,
}

impl StreakState {
    /// The start of a world frame's (or probe's) GPU work: a command
    /// buffer of its own holding the first timestamp, submitted before
    /// every other buffer of the frame (the world's views and the
    /// environment cube are encoded on render threads and go first). None:
    /// no timestamp queries.
    pub(crate) fn gpu_begin(&mut self, gpu: &Gpu) -> Option<wgpu::CommandBuffer> {
        if !self.timer_tried {
            self.timer_tried = true;
            self.timer = GpuTimer::new(gpu);
        }
        let t = self.timer.as_mut()?;
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("streak cost: start") });
        t.begin(&mut encoder).then(|| encoder.finish())
    }

    /// Where the measured work ends: the post chain writes it right after
    /// the streak buffer's commit (`PostFx::set_commit_stamp`), so a drawn
    /// frame is timed over what a probe draws, without its output, screen
    /// passes and UI.
    pub(crate) fn commit_stamp(&self) -> Option<(wgpu::QuerySet, u32)> {
        self.timer.as_ref()?.end_stamp()
    }

    /// The end of that work, in the post passes' command buffer: the
    /// second timestamp written here unless the chain wrote it (`written`),
    /// then read back.
    pub(crate) fn gpu_end(&mut self, encoder: &mut wgpu::CommandEncoder, written: bool) {
        if let Some(t) = self.timer.as_mut() {
            t.end(encoder, written);
        }
    }

    /// After the last frame's submission: the timings read back so far
    /// into `gpu_cost` (never waits).
    pub(crate) fn gpu_harvest(&mut self, gpu: &Gpu) {
        let Some(t) = self.timer.as_mut() else { return };
        for secs in t.harvest(&gpu.device) {
            self.gpu_cost = Some(match self.gpu_cost {
                Some(c) => c + (secs - c) * 0.2,
                None => secs,
            });
        }
    }
}

/// One window frame's measurements for the [`Governor`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameSample {
    /// Wall time since the last frame began (s).
    pub(crate) interval: f32,
    /// Scene renders in the frame: 1 + its probes.
    pub(crate) renders: u32,
    /// The main thread's rendering in the frame: its draw and its probes,
    /// without its ticks' simulation (a probe simulates nothing) and the
    /// waits for the display and the frame cap, s.
    pub(crate) cpu: f32,
    /// GPU time per scene render (timestamp queries; None without them), s.
    pub(crate) gpu: Option<f32>,
    /// The display's or the frame cap's period, when the frame waits for
    /// one (vsync with a known refresh rate, a cap), s.
    pub(crate) target: Option<f32>,
    /// The tick period at the state's rate (1/60 s, 1/30 s in a scene).
    pub(crate) tick_period: f32,
    /// The period at which drawn frames reach the screen, when known: the
    /// display's refresh (with or without vsync), or the frame cap's when
    /// longer; the bench's display schedule. None: unknown (no frames'
    /// share is counted), s.
    pub(crate) display: Option<f32>,
}

/// Exact or cheap, from measured work. The cost of one scene render c is
/// the larger of its GPU time (timestamps around exactly what a probe
/// draws) and the main thread's rendering per render; without timestamps,
/// when the frame is not held by the display or a cap, also the frame's
/// interval per render (the machine's throughput). Exact mode spends up to
/// one render per tick on probes, so the predicted load of the probes is
/// c / tick period. c is the median of the frames of each quarter second
/// (a typical render: hitches — streaming, a busy machine's scheduling, a
/// load inside a frame — are rare frames and do not move it, however long
/// they are), smoothed over 0.5 s. Cheap when the load passes
/// [`Governor::ENTER`]; exact again only under [`Governor::LEAVE`], after a
/// minimum dwell in each form (the cheap one doubles when exact did not
/// hold, up to [`Governor::MAX_CHEAP`]). A frame over 4 ticks long, or
/// whose own work is, is not a cost sample.
///
/// The drawn frames take their own share of the machine too: one render
/// per displayed frame, c / display period ([`FrameSample::display`]).
/// The probes come on top of it, so exact mode also ends when the two
/// together pass [`Governor::BUSY_ENTER`] (and comes back only under
/// [`Governor::BUSY_LEAVE`]): a small GPU whose frames already fill most
/// of each refresh cannot fit a probe in, although the probes alone look
/// cheap against the tick. With the display period unknown (uncapped, no
/// refresh read) only the probe load counts.
#[derive(Clone, Debug)]
pub(crate) struct Governor {
    pub(crate) exact: bool,
    /// Seconds per scene render, smoothed.
    pub(crate) cost: Option<f32>,
    in_mode: f32,
    cheap_dwell: f32,
    clock: f32,
    left_cheap: Option<f32>,
    /// This quarter second's costs per render and its length so far.
    window: Vec<f32>,
    window_time: f32,
    /// Forced by the developer knob: Some(true) exact, Some(false) cheap.
    pub(crate) forced: Option<bool>,
}

impl Default for Governor {
    fn default() -> Self {
        // The first decision at the end of the first quarter second (the
        // cost is known from its frames): a machine that cannot afford
        // probes leaves them at once.
        Governor {
            exact: true,
            cost: None,
            in_mode: Self::MIN_EXACT - Self::WINDOW,
            cheap_dwell: Self::MIN_CHEAP,
            clock: 0.0,
            left_cheap: None,
            window: Vec::new(),
            window_time: 0.0,
            forced: None,
        }
    }
}

impl Governor {
    /// Probe load (share of the machine) above which exact mode ends.
    /// Set against the cost of all a probe draws: 4K Original on a strong
    /// GPU (2.1–2.5 ms a render, 13–15 %) and 1080p Extreme (2.6–2.8 ms)
    /// stay exact and come back to it after a load's first heavy seconds;
    /// 4K Extreme (10 ms, 60 %) is cheap.
    pub(crate) const ENTER: f32 = 0.30;
    /// Probe load under which it comes back.
    pub(crate) const LEAVE: f32 = 0.18;
    /// Probe load plus the drawn frames' own share above which exact mode
    /// ends: the frames' output, screen passes and UI are not in c, so a
    /// tenth is left for them.
    pub(crate) const BUSY_ENTER: f32 = 0.90;
    /// The two together under which it comes back (with the probe load
    /// under [`Governor::LEAVE`]).
    pub(crate) const BUSY_LEAVE: f32 = 0.75;
    pub(crate) const MIN_EXACT: f32 = 1.0;
    pub(crate) const MIN_CHEAP: f32 = 2.0;
    pub(crate) const MAX_CHEAP: f32 = 32.0;
    /// The cost's smoothing time (s).
    const TAU: f32 = 0.5;
    /// The span of frames whose median is one cost sample (s).
    const WINDOW: f32 = 0.25;

    /// The predicted share of the machine exact mode's probes take.
    pub(crate) fn load(&self, tick_period: f32) -> f32 {
        self.cost.map_or(0.0, |c| c / tick_period)
    }

    /// The drawn frames' predicted share of the machine: one render a
    /// displayed frame (`display`: the period frames reach the screen at).
    /// 0 when the period is unknown.
    pub(crate) fn frames_share(&self, display: Option<f32>) -> f32 {
        match (self.cost, display) {
            (Some(c), Some(p)) if p > 0.0 => c / p,
            _ => 0.0,
        }
    }

    /// The predicted share of the machine in exact mode: the probes' load
    /// and the drawn frames' own share.
    pub(crate) fn busy(&self, tick_period: f32, display: Option<f32>) -> f32 {
        self.load(tick_period) + self.frames_share(display)
    }

    /// One frame's measurements in; the form for the frames after it.
    pub(crate) fn sample(&mut self, s: FrameSample) {
        let dt = s.interval.max(0.0);
        self.clock += dt;
        self.in_mode += dt;
        if let Some(f) = self.forced {
            self.exact = f;
            return;
        }
        let long = 4.0 * s.tick_period;
        if dt > long || dt <= 0.0 || s.cpu > long {
            return;
        }
        let n = s.renders.max(1) as f32;
        let mut c = (s.cpu / n).max(s.gpu.unwrap_or(0.0));
        let held = s.target.is_some_and(|t| dt <= 1.3 * t);
        if s.gpu.is_none() && !held {
            c = c.max(dt / n);
        }
        self.window.push(c);
        self.window_time += dt;
        if self.window_time < Self::WINDOW {
            return;
        }
        let mid = self.window.len() / 2;
        let median = *self.window.select_nth_unstable_by(mid, f32::total_cmp).1;
        let k = 1.0 - (-self.window_time / Self::TAU).exp();
        self.window.clear();
        self.window_time = 0.0;
        self.cost = Some(self.cost.map_or(median, |old| old + (median - old) * k));
        let load = self.load(s.tick_period);
        let busy = self.busy(s.tick_period, s.display);
        if self.exact && self.in_mode >= Self::MIN_EXACT && (load > Self::ENTER || busy > Self::BUSY_ENTER) {
            self.exact = false;
            self.in_mode = 0.0;
            // Back to cheap soon after leaving it: exact did not hold.
            self.cheap_dwell = match self.left_cheap {
                Some(t) if self.clock - t < 10.0 => (self.cheap_dwell * 2.0).min(Self::MAX_CHEAP),
                _ => Self::MIN_CHEAP,
            };
        } else if !self.exact && self.in_mode >= self.cheap_dwell && load < Self::LEAVE && busy < Self::BUSY_LEAVE {
            self.exact = true;
            self.in_mode = 0.0;
            self.left_cheap = Some(self.clock);
        }
    }

    /// Whether a frame `interval` s long may draw probes: exact mode, and
    /// not a hitch (more ticks than a steady 30 Hz frame holds: the frame
    /// after a stall commits its missed ticks itself, no burst of probes).
    pub(crate) fn frame_probes(&self, interval: f32, tick_period: f32) -> bool {
        self.exact && interval <= 2.5 * tick_period
    }
}

/// Timestamps around each world frame's (and probe's) GPU work — from
/// before its first command buffer (the world's views, the environment
/// cube) to the streak buffer's commit — a pair per render in a small
/// ring, read back without waiting.
struct GpuTimer {
    set: wgpu::QuerySet,
    slots: Vec<TimerSlot>,
    period: f64,
    open: Option<usize>,
}

struct TimerSlot {
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    state: SlotState,
}

enum SlotState {
    Free,
    Begun,
    Ended,
    Mapping(std::sync::Arc<std::sync::atomic::AtomicBool>),
}

impl GpuTimer {
    const SLOTS: usize = 8;

    fn new(gpu: &Gpu) -> Option<GpuTimer> {
        if !gpu.device.features().contains(nfsu2::prof::gpu::FEATURES) {
            return None;
        }
        let set = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("streak cost"),
            ty: wgpu::QueryType::Timestamp,
            count: (Self::SLOTS * 2) as u32,
        });
        let buffer = |usage| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("streak cost"), size: 16, usage, mapped_at_creation: false })
        };
        let slots = (0..Self::SLOTS)
            .map(|_| TimerSlot {
                resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
                read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
                state: SlotState::Free,
            })
            .collect();
        Some(GpuTimer { set, slots, period: f64::from(gpu.queue.get_timestamp_period()), open: None })
    }

    /// The first timestamp of a render into `encoder`; false without a free
    /// slot (nothing written).
    fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) -> bool {
        // A render that never reached its end (no post passes) frees its slot.
        if let Some(i) = self.open.take() {
            self.slots[i].state = SlotState::Free;
        }
        let Some(i) = self.slots.iter().position(|s| matches!(s.state, SlotState::Free)) else { return false };
        encoder.write_timestamp(&self.set, (2 * i) as u32);
        self.slots[i].state = SlotState::Begun;
        self.open = Some(i);
        true
    }

    /// The open render's second timestamp (query set, index).
    fn end_stamp(&self) -> Option<(wgpu::QuerySet, u32)> {
        self.open.map(|i| (self.set.clone(), (2 * i + 1) as u32))
    }

    /// The open render's end: its second timestamp (unless `written`
    /// already), resolved and copied for the read-back.
    fn end(&mut self, encoder: &mut wgpu::CommandEncoder, written: bool) {
        let Some(i) = self.open.take() else { return };
        let q = (2 * i) as u32;
        if !written {
            encoder.write_timestamp(&self.set, q + 1);
        }
        let slot = &mut self.slots[i];
        encoder.resolve_query_set(&self.set, q..q + 2, &slot.resolve, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, 16);
        slot.state = SlotState::Ended;
    }

    /// The renders read back since the last call, s each. Called after the
    /// last frame's submission (its ended slots are submitted).
    fn harvest(&mut self, device: &wgpu::Device) -> Vec<f32> {
        for slot in &mut self.slots {
            if matches!(slot.state, SlotState::Ended) {
                let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let flag = done.clone();
                slot.read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    if r.is_ok() {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                    }
                });
                slot.state = SlotState::Mapping(done);
            }
        }
        let _ = device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for slot in &mut self.slots {
            let SlotState::Mapping(done) = &slot.state else { continue };
            if !done.load(std::sync::atomic::Ordering::Acquire) {
                continue;
            }
            let t: Vec<u64> = slot
                .read
                .slice(..)
                .get_mapped_range()
                .expect("mapped timestamps")
                .chunks_exact(8)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                .collect();
            slot.read.unmap();
            slot.state = SlotState::Free;
            let secs = (t[1].saturating_sub(t[0]) as f64 * self.period / 1e9) as f32;
            if secs > 0.0 && secs < 1.0 {
                out.push(secs);
            }
        }
        out
    }
}

impl Game {
    /// Whether a probe for the streak buffer would commit anything: the
    /// light streaks on, a scene to draw (a world or the garage's), no
    /// temporal pass, exact form this frame, the newest slot not committed.
    fn streak_probe_due(&self) -> bool {
        self.streak_in_use()
            && !self.render.streak.cheap_frame
            && self.render.streak.ticks.due(self.pacing.clocks.tick)
    }

    /// Between two ticks of one drawn frame (30 Hz, a stall): the 60 Hz
    /// frame of the newest tick drawn as a probe. At α = 0, as the frames
    /// of the window and of `ticks::Frames` are drawn at a tick (the
    /// harness's `draw()` on the tick, α = 1, takes no tick loop).
    pub(crate) fn probe_between_ticks(&mut self) -> Result<()> {
        self.streak_probe(0.0)
    }

    /// A probe at `alpha` (0 or 1), when one is due.
    pub(crate) fn streak_probe(&mut self, alpha: f32) -> Result<()> {
        if !self.streak_probe_due() {
            return Ok(());
        }
        self.render.streak.probes += 1;
        // Timed for the performance overlay: the probe's wall time and its
        // waits for the frame before (`settle`).
        let started = Instant::now();
        let settled = self.platform.window.as_ref().map_or(0.0, |w| w.settle_sum);
        let r = self.draw_at_as(alpha, true);
        let s = &mut self.render.streak;
        s.probe_wall += started.elapsed().as_secs_f32();
        s.probe_settle += self.platform.window.as_ref().map_or(0.0, |w| w.settle_sum) - settled;
        r
    }

    /// Whether the light-trail buffer is in use (probes may be drawn when
    /// the form allows): the trails on, a scene, no temporal pass.
    pub(crate) fn streak_in_use(&self) -> bool {
        self.platform.window.is_some()
            && Switches::from_custom(&self.settings.custom).streaks()
            && (self.sim.drive.is_some() || self.garage_fe.request.is_some())
            && !nfsu2::qol::temporal()
    }

    /// The developer knob `NFSU2_STREAK_FORM` (dev-harness builds):
    /// `exact` or `cheap` forces that form everywhere (the window's choice
    /// and the harness), `auto` lets the bench's frames choose as the
    /// window's do (a measuring aid). None: the default (window: measured;
    /// harness: exact).
    pub(crate) fn streak_form_knob() -> Option<&'static str> {
        match debug_env().streak_form.as_deref() {
            Some("exact") => Some("exact"),
            Some("cheap") => Some("cheap"),
            Some("auto") => Some("auto"),
            _ => None,
        }
    }

    /// The window's frame begins (`advance_ns`): the form for it, from the
    /// governor and the frame's length; the probe count reset.
    pub(crate) fn streak_frame_begin(&mut self, interval: f32) {
        let period = self.tick_period_now();
        let s = &mut self.render.streak;
        s.probes = 0;
        s.probe_wall = 0.0;
        s.probe_settle = 0.0;
        s.gov.forced = match Self::streak_form_knob() {
            Some("exact") => Some(true),
            Some("cheap") => Some(false),
            _ => None,
        };
        s.cheap_frame = match s.gov.forced {
            Some(f) => !f,
            None => !s.gov.frame_probes(interval, period),
        };
    }

    #[cfg(any(test, feature = "dev-harness"))]
    /// The harness's frames (`advance`, `ticks::Frames`): exact, unless the
    /// knob forces the cheap form (with `auto` the bench sets the form).
    pub(crate) fn streak_harness_form(&mut self) {
        if Self::streak_form_knob() == Some("auto") {
            return;
        }
        self.render.streak.probes = 0;
        self.render.streak.cheap_frame = Self::streak_form_knob() == Some("cheap");
    }

    /// A window frame's measurements into the governor (logs a change of
    /// form with the predicted probe load).
    pub(crate) fn streak_govern(&mut self, sample: FrameSample) {
        let st = &mut self.render.streak;
        st.frames += 1;
        st.probes_total += u64::from(st.probes);
        if st.gov.exact {
            st.exact_frames += 1;
        }
        let g = &mut st.gov;
        let was = g.exact;
        g.sample(sample);
        if g.exact != was {
            st.changes += 1;
        }
        let g = &st.gov;
        // Every sample and its outcome (`RUST_LOG=game::render::streak=debug`).
        log::debug!(
            "streak governor: interval {:.3} ms, renders {}, cpu {:.3} ms, gpu {}, target {}, display {}, tick {:.2} ms -> cost {:.3} ms, load {:.1} %, with the frames {:.1} %, {} ({:.2} s in it, cheap dwell {:.0} s)",
            sample.interval * 1000.0,
            sample.renders,
            sample.cpu * 1000.0,
            sample.gpu.map_or("-".into(), |g| format!("{:.3} ms", g * 1000.0)),
            sample.target.map_or("-".into(), |t| format!("{:.2} ms", t * 1000.0)),
            sample.display.map_or("-".into(), |t| format!("{:.2} ms", t * 1000.0)),
            sample.tick_period * 1000.0,
            g.cost.unwrap_or(0.0) * 1000.0,
            g.load(sample.tick_period) * 100.0,
            g.busy(sample.tick_period, sample.display) * 100.0,
            if g.exact { "exact" } else { "cheap" },
            g.in_mode,
            g.cheap_dwell
        );
        if g.exact != was {
            log::info!(
                "light streaks: {} form (predicted probe load {:.0} %, with the drawn frames {:.0} %, {:.2} ms per scene render)",
                if g.exact { "exact" } else { "cheap" },
                g.load(sample.tick_period) * 100.0,
                g.busy(sample.tick_period, sample.display) * 100.0,
                g.cost.unwrap_or(0.0) * 1000.0
            );
        }
    }

    /// The bench's frame at `hz` with `NFSU2_STREAK_FORM=auto`: the last
    /// frame's wall time (offscreen, no vsync; the display period the
    /// schedule's) into the governor, then this frame's form.
    #[cfg(feature = "dev-harness")]
    pub(crate) fn streak_bench_frame(&mut self, last: Option<f32>, hz: f32) {
        match Self::streak_form_knob() {
            Some("auto") => {}
            // A forced form: the probes counted for the summary.
            Some(_) => {
                let s = &mut self.render.streak;
                if last.is_some() {
                    s.frames += 1;
                    s.probes_total += u64::from(s.probes);
                }
                self.streak_frame_begin(1.0 / hz);
                return;
            }
            None => return,
        }
        self.render.streak.bench_display = Some(1.0 / hz);
        if let Some(interval) = last {
            let s = &self.render.streak;
            let sample = FrameSample {
                interval,
                renders: 1 + s.probes,
                cpu: interval,
                gpu: s.gpu_cost,
                target: None,
                tick_period: self.tick_period_now(),
                // The display the bench's schedule stands for.
                display: Some(1.0 / hz),
            };
            self.streak_govern(sample);
        }
        self.streak_frame_begin(1.0 / hz);
    }

    /// The bench's line on the governor (with `NFSU2_STREAK_FORM` set; the
    /// GPU time is the streak cost timer's, per scene render).
    #[cfg(feature = "dev-harness")]
    pub(crate) fn streak_bench_summary(&self) -> Option<String> {
        Self::streak_form_knob()?;
        let s = &self.render.streak;
        Some(format!(
            "streaks  form: exact in {} of {} frames, {} changes, {} probes; predicted probe load {:.1} % ({:.3} ms a scene render, GPU {:.3} ms), with the drawn frames {:.1} %",
            s.exact_frames,
            s.frames,
            s.changes,
            s.probes_total,
            s.gov.load(self.tick_period_now()) * 100.0,
            s.gov.cost.unwrap_or(0.0) * 1000.0,
            s.gpu_cost.unwrap_or(0.0) * 1000.0,
            s.gov.busy(self.tick_period_now(), s.bench_display) * 100.0
        ))
    }

    /// The tick period at the state's rate now (s).
    pub(crate) fn tick_period_now(&self) -> f32 {
        let rate = if self.scene_frame_cap() { nfsu2::frame_clock::SCENE } else { nfsu2::frame_clock::RACE };
        rate.update as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commits(f: StreakFrame) -> (u32, bool) {
        (f.commits, f.scratch)
    }

    /// One tick a frame (60 Hz, either α convention): one commit each,
    /// as the per-frame path.
    #[test]
    fn one_tick_a_frame_commits_once() {
        let mut s = StreakTicks::default();
        for t in 1..100 {
            assert_eq!(commits(s.frame(t, 1, 0.0, false, true)), (1, false));
        }
        let mut s = StreakTicks::default();
        for t in 1..100 {
            assert_eq!(commits(s.frame(t, 1, 1.0, false, true)), (1, false));
        }
    }

    /// No new tick: nothing committed; between ticks the scratch.
    #[test]
    fn no_tick_commits_nothing() {
        let mut s = StreakTicks::default();
        assert_eq!(commits(s.frame(5, 1, 0.0, false, true)), (1, false));
        assert_eq!(commits(s.frame(5, 0, 0.0, false, true)), (0, false));
        assert_eq!(commits(s.frame(5, 0, 0.5, false, true)), (0, true));
        assert_eq!(commits(s.frame(5, 0, 1.0, false, true)), (0, false));
    }

    /// 30 Hz: the probe between the frame's ticks commits the first slot,
    /// the frame the second; a 200 Hz frame between ticks after its probe
    /// commits nothing and shows the scratch.
    #[test]
    fn probes_take_the_slots_frames_do_not_stand_on() {
        let mut s = StreakTicks::default();
        assert_eq!(commits(s.frame(1, 1, 0.0, false, true)), (1, false));
        for k in 1..50u64 {
            assert_eq!(commits(s.frame(2 * k, 0, 0.0, true, true)), (1, false), "probe {k}");
            assert_eq!(commits(s.frame(2 * k + 1, 2, 0.0, false, true)), (1, false), "frame {k}");
        }
        assert_eq!(commits(s.frame(200, 0, 0.0, true, true)), (1, false));
        assert_eq!(commits(s.frame(200, 1, 0.3, false, true)), (0, true));
        // A probe of a slot already committed does nothing.
        assert_eq!(commits(s.frame(200, 0, 0.0, true, true)), (0, false));
    }

    /// Without probes (cheap form): the ticks a frame took, at most 3;
    /// ticks without a drawn frame (the harness's warm-ups) are not made up.
    #[test]
    fn without_probes_a_frame_commits_its_ticks() {
        let mut s = StreakTicks::default();
        assert_eq!(commits(s.frame(1, 1, 0.0, false, false)), (1, false));
        assert_eq!(commits(s.frame(3, 2, 0.0, false, false)), (2, false));
        assert_eq!(commits(s.frame(6, 3, 0.5, false, false)), (3, false), "shows its commits");
        assert_eq!(commits(s.frame(6, 0, 0.7, false, false)), (0, true), "the scratch without a tick");
        assert_eq!(commits(s.frame(40, 1, 0.0, false, false)), (1, false));
    }

    /// A machine whose scene render costs `render` s (GPU), the display
    /// held to `target` s (vsync; None: uncapped, no vsync), run
    /// for `secs` s of frames; `gpu`: timestamp queries there. The frames'
    /// length follows the form: exact mode draws a probe for each tick
    /// in the frame (the rates where frames do not land on ticks).
    /// -> (exact at the end, form changes, share of time in exact mode).
    fn run(render: f32, target: Option<f32>, secs: f32, gpu: bool, stall_at: Option<f32>) -> (bool, u32, f32) {
        let t = 1.0 / 60.0;
        let mut g = Governor::default();
        let (mut clock, mut acc, mut probes, mut changes, mut exact_time) = (0.0f32, 0.0f32, 0u32, 0u32, 0.0f32);
        let mut stalled = false;
        while clock < secs {
            let renders = 1 + probes;
            // Held to the refresh (a missed one waits for the next).
            let work = render * renders as f32 + 0.0002;
            let mut interval = target.map_or(work, |p| (work / p).ceil() * p);
            if let Some(at) = stall_at.filter(|&at| clock >= at && !stalled) {
                let _ = at;
                stalled = true;
                interval = 0.5;
            }
            let was = g.exact;
            let probing = g.frame_probes(interval, t);
            g.sample(FrameSample {
                interval,
                renders,
                cpu: 0.0002 * renders as f32,
                gpu: gpu.then_some(render),
                target,
                tick_period: t,
                display: target,
            });
            if g.exact != was {
                changes += 1;
            }
            if g.exact {
                exact_time += interval;
            }
            acc += interval;
            let ticks = (acc / t).floor();
            acc -= ticks * t;
            probes = if probing && g.exact { (ticks as u32).min(3) } else { 0 };
            clock += interval;
        }
        (g.exact, changes, exact_time / clock)
    }

    /// Uncapped and GPU-bound at 40 / 75 / 140 fps: probes would take a
    /// large share of the machine, so the cheap form, entered once and kept.
    #[test]
    fn gpu_bound_without_vsync_goes_cheap() {
        for fps in [40.0f32, 75.0, 140.0] {
            for gpu in [true, false] {
                let (exact, changes, _) = run(1.0 / fps, None, 60.0, gpu, None);
                assert!(!exact, "{fps} fps (timestamps {gpu}): cheap");
                assert_eq!(changes, 1, "{fps} fps (timestamps {gpu}): one change");
            }
        }
    }

    /// Vsync at 60 Hz with frames missed (a render over the refresh): cheap.
    #[test]
    fn vsync_missing_frames_goes_cheap() {
        for gpu in [true, false] {
            let (exact, changes, _) = run(0.018, Some(1.0 / 60.0), 60.0, gpu, None);
            assert!(!exact && changes == 1, "timestamps {gpu}");
        }
    }

    /// Lots of headroom: a 50 Hz display, a 30 fps cap, 144 and 200 Hz, and
    /// uncapped on a fast machine (0.7 ms and 2.5 ms a render): exact, never
    /// left.
    #[test]
    fn headroom_stays_exact() {
        for (render, target) in [
            (0.002, Some(1.0 / 50.0)),
            (0.002, Some(1.0 / 30.0)),
            (0.001, Some(1.0 / 144.0)),
            (0.001, Some(1.0 / 200.0)),
            (0.0007, None),
            (0.0025, None),
        ] {
            let (exact, changes, share) = run(render, target, 60.0, true, None);
            assert!(exact && changes == 0 && share == 1.0, "{render} s a render, target {target:?}");
        }
    }

    /// One long frame (a 0.5 s stall) does not change the form, and that
    /// frame draws no probes (its ticks are committed by the frame itself).
    #[test]
    fn a_stall_does_not_flip_the_form() {
        let (exact, changes, _) = run(0.001, Some(1.0 / 144.0), 20.0, true, Some(5.0));
        assert!(exact && changes == 0);
        let g = Governor::default();
        assert!(!g.frame_probes(0.5, 1.0 / 60.0), "no burst of probes after a hitch");
        assert!(g.frame_probes(1.0 / 30.0, 1.0 / 60.0), "a steady 30 Hz frame probes its first tick");
        assert!(g.frame_probes(1.0 / 144.0, 1.0 / 60.0));
    }

    /// The thresholds' gap and the dwells keep a borderline machine from
    /// flapping: a render cost that puts the load between LEAVE and ENTER
    /// keeps whatever form it is in; one just over ENTER switches once
    /// (uncapped: the frames' length follows the work).
    #[test]
    fn borderline_does_not_flap() {
        let t = 1.0f32 / 60.0;
        for render in [0.20 * t, 0.27 * t] {
            let (exact, changes, _) = run(render, None, 120.0, true, None);
            assert!(exact && changes == 0, "{render}");
        }
        let (exact, changes, _) = run(0.32 * t, None, 120.0, true, None);
        assert!(!exact && changes == 1);
    }

    /// Back to exact when the load drops (a menu, a lighter scene), after
    /// the cheap form's dwell, which doubles when exact did not hold.
    #[test]
    fn returns_to_exact_and_backs_off() {
        let t = 1.0f32 / 60.0;
        let mut g = Governor::default();
        // Frames until the form changes: s taken.
        let until_change = |g: &mut Governor, render: f32| {
            let (was, mut c) = (g.exact, 0.0f32);
            while g.exact == was && c < 100.0 {
                g.sample(FrameSample { interval: 1.0 / 144.0, renders: 1, cpu: 0.0002, gpu: Some(render), target: Some(1.0 / 144.0), tick_period: t, display: Some(1.0 / 144.0) });
                c += 1.0 / 144.0;
            }
            c
        };
        assert!(until_change(&mut g, 0.006) < 2.0);
        assert!(!g.exact);
        let first = until_change(&mut g, 0.001);
        assert!(g.exact && (Governor::MIN_CHEAP..Governor::MIN_CHEAP + 1.0).contains(&first), "{first}");
        assert!(until_change(&mut g, 0.006) < 2.0);
        let second = until_change(&mut g, 0.001);
        assert!(g.exact && second >= 2.0 * Governor::MIN_CHEAP, "{second}: the dwell doubled");
    }

    /// A window run: `frame(k, clock, renders)` gives frame k's sample
    /// (its interval, CPU, GPU, target) for a frame with `renders` scene
    /// renders; the probes follow the form and the ticks as in the window
    /// (a frame spanning n ticks probes n − 1 of them, one between ticks
    /// the tick before it; α within 2 % of a tick snaps to it).
    /// -> (exact at the end, form changes, share of time exact, probes).
    fn window_run(secs: f32, mut frame: impl FnMut(u64, f32, u32) -> FrameSample) -> (bool, u32, f32, u64) {
        let mut g = Governor::default();
        let (mut clock, mut acc, mut changes, mut exact_time, mut probes, mut total) = (0.0f32, 0.0f64, 0u32, 0.0f32, 0u32, 0u64);
        let mut k = 0u64;
        while clock < secs {
            let s = frame(k, clock, 1 + probes);
            let t = s.tick_period;
            let was = g.exact;
            let probing = g.frame_probes(s.interval, t);
            g.sample(s);
            changes += u32::from(g.exact != was);
            if g.exact {
                exact_time += s.interval;
            }
            acc += f64::from(s.interval / t);
            let mut ticks = acc.floor();
            acc -= ticks;
            if acc > 0.98 {
                ticks += 1.0;
                acc = 0.0;
            }
            let between = acc > 0.02;
            let ticks = (ticks as u32).min(3);
            probes = if probing && g.exact && ticks > 0 { ticks - 1 + u32::from(between) } else { 0 };
            total += u64::from(probes);
            clock += s.interval;
            k += 1;
        }
        (g.exact, changes, exact_time / clock, total)
    }

    /// The free-roam failure seen in a window: a machine whose scene render
    /// costs about a millisecond, uncapped (mailbox: no target), with a
    /// hitch every hundredth frame (streaming, a busy machine's scheduling:
    /// 30 ms of interval and of the frame's own work) and, first, a frame
    /// with a load in it (0.9 s of work in a frame that began 0.2 ms after
    /// the one before). Time-weighted, the hitches made the cost several
    /// times the typical render's (the cheap form at 700 fps); the load
    /// frame's work passed the stall filter, which looked only at the
    /// interval. The typical render decides: exact, never left, with and
    /// without timestamps.
    #[test]
    fn hitches_and_a_load_do_not_make_a_fast_machine_cheap() {
        // (interval, CPU, GPU) of a typical frame: 700 fps on the
        // developer's machine, the private Xvfb window's 2.75 ms.
        for (interval, cpu, gpu) in [(0.00143f32, 0.0003f32, 0.0006f32), (0.00275, 0.0008, 0.0017)] {
            for timestamps in [true, false] {
                let r = window_run(60.0, |k, _, renders| {
                    let probe = 0.7 * gpu.max(cpu) * (renders - 1) as f32;
                    let (interval, cpu) = match k {
                        0 => (0.0002, 0.9),
                        _ if k % 100 == 50 => (0.03, 0.03),
                        _ => (interval + probe, cpu * renders as f32),
                    };
                    FrameSample { interval, renders, cpu, gpu: timestamps.then_some(gpu), target: None, tick_period: 1.0 / 60.0, display: None }
                });
                let (exact, changes, share, _) = r;
                assert!(exact && changes == 0 && share == 1.0, "{interval} s frames, timestamps {timestamps}: {r:?}");
            }
        }
    }

    /// Vsync at exactly 60 and 120 Hz (the 120 Hz frames alternate on a
    /// tick and half way, the 60 Hz ones land on ticks, a little jitter):
    /// no probe is needed and none is drawn, and the form never changes,
    /// cheap render or expensive (a 60 Hz machine using 13 ms of each
    /// 16.7 ms frame goes cheap once, then stays).
    #[test]
    fn vsync_at_the_tick_rate_and_twice_it_needs_no_probes_and_does_not_flap() {
        for hz in [60.0f32, 120.0] {
            for (gpu, cheap) in [(0.001f32, false), (0.004, false), (0.013, true)] {
                if hz == 120.0 && gpu > 1.0 / 120.0 {
                    continue;
                }
                let r = window_run(60.0, |k, _, renders| FrameSample {
                    interval: 1.0 / hz + [0.0002, -0.0002, 0.0001, -0.0001][k as usize % 4],
                    renders,
                    cpu: 0.0005,
                    gpu: Some(gpu),
                    target: Some(1.0 / hz),
                    tick_period: 1.0 / 60.0,
                    display: Some(1.0 / hz),
                });
                let (exact, changes, _, probes) = r;
                assert_eq!(probes, 0, "{hz} Hz, {gpu} s a render: {r:?}");
                assert!(exact != cheap && changes == u32::from(cheap), "{hz} Hz, {gpu} s a render: {r:?}");
            }
        }
    }

    /// A 50 Hz display and a frame cap of 30 with headroom (2 ms a render):
    /// exact, with its probes, through hitches.
    #[test]
    fn fifty_hertz_and_a_thirty_cap_stay_exact() {
        for hz in [50.0f32, 30.0] {
            let r = window_run(60.0, |k, _, renders| FrameSample {
                interval: if k % 100 == 50 { 0.05 } else { 1.0 / hz },
                renders,
                cpu: 0.0005 * renders as f32,
                gpu: Some(0.002),
                target: Some(1.0 / hz),
                tick_period: 1.0 / 60.0,
                display: Some(1.0 / hz),
            });
            let (exact, changes, share, probes) = r;
            assert!(exact && changes == 0 && share == 1.0 && probes > 0, "{hz}: {r:?}");
        }
    }

    /// A starved machine (uncapped, 30 ms a render, every frame) leaves the
    /// exact form within a second of the start, and within a second of the
    /// load arriving after a light start.
    #[test]
    fn a_starved_machine_goes_cheap_within_a_second() {
        let starved = |from: f32| {
            let mut changed_at = None;
            let mut g = Governor::default();
            let mut clock = 0.0f32;
            while clock < from + 5.0 {
                let render: f32 = if clock < from { 0.001 } else { 0.03 };
                g.sample(FrameSample {
                    interval: render.max(0.004),
                    renders: 1,
                    cpu: 0.0005,
                    gpu: Some(render),
                    target: None,
                    tick_period: 1.0 / 60.0,
                    display: None,
                });
                clock += render.max(0.004);
                if !g.exact && changed_at.is_none() {
                    changed_at = Some(clock);
                }
            }
            changed_at.map(|t| t - from)
        };
        for from in [0.0f32, 5.0] {
            let after = starved(from).expect("cheap");
            assert!(after <= 1.0, "load from {from} s: cheap {after} s after");
        }
    }

    /// A small GPU whose frames already fill most of each refresh: a
    /// 144 Hz display (vsync, and mailbox: frames back to back with the
    /// refresh known), 4.6–5.0 ms a scene render, measured on an integrated
    /// GPU at 1366 × 768 Original (free roam; the race start's 7.1–7.8 ms
    /// against a 30 Hz tick). The probes alone are 21–30 % of the tick, under
    /// ENTER; with the frames' 66–112 % of each refresh the machine is
    /// full: cheap within a second, kept. The same renders with the display
    /// unknown (uncapped) decide as before (exact), and a strong GPU at
    /// 144 Hz (1 ms a render) stays exact.
    #[test]
    fn frames_filling_the_refresh_go_cheap() {
        let p = 1.0f32 / 144.0;
        for (render, tick) in [(0.0046f32, 1.0f32 / 60.0), (0.005, 1.0 / 60.0), (0.0074, 1.0 / 30.0)] {
            for vsync in [true, false] {
                let r = window_run(30.0, |_, _, renders| {
                    let work = render * renders as f32 + 0.0002;
                    let interval = if vsync { (work / p).ceil() * p } else { work };
                    FrameSample {
                        interval,
                        renders,
                        cpu: 0.0003 * renders as f32,
                        gpu: Some(render),
                        target: vsync.then_some(p),
                        tick_period: tick,
                        display: Some(p),
                    }
                });
                let (exact, changes, share, _) = r;
                assert!(!exact && changes == 1, "{render} s a render, vsync {vsync}: {r:?}");
                assert!(share * 30.0 <= 1.5, "{render} s a render, vsync {vsync}: cheap after {} s", share * 30.0);
                let g = Governor { cost: Some(render), ..Default::default() };
                assert!(g.load(tick) < Governor::ENTER, "the probes alone are under the threshold");
            }
            // Uncapped with the refresh unknown: the probe load alone, as before.
            let r = window_run(30.0, |_, _, renders| FrameSample {
                interval: render * renders as f32 + 0.0002,
                renders,
                cpu: 0.0003 * renders as f32,
                gpu: Some(render),
                target: None,
                tick_period: tick,
                display: None,
            });
            assert!(r.0 && r.1 == 0, "{render} s a render, display unknown: {r:?}");
        }
        // A strong GPU at 144 Hz (1 ms; 4K Original's 2.3 ms): exact, never left.
        for render in [0.001f32, 0.0023] {
            let r = window_run(30.0, |_, _, renders| FrameSample {
                interval: p,
                renders,
                cpu: 0.0003 * renders as f32,
                gpu: Some(render),
                target: Some(p),
                tick_period: 1.0 / 60.0,
                display: Some(p),
            });
            assert!(r.0 && r.1 == 0 && r.2 == 1.0, "{render} s a render at 144 Hz: {r:?}");
        }
    }

    /// The knob's forced form wins over the measurements.
    #[test]
    fn forced_form_wins() {
        let mut g = Governor { forced: Some(true), ..Default::default() };
        for _ in 0..1000 {
            g.sample(FrameSample { interval: 0.03, renders: 2, cpu: 0.03, gpu: Some(0.015), target: None, tick_period: 1.0 / 60.0, display: None });
        }
        assert!(g.exact);
        g.forced = Some(false);
        g.sample(FrameSample { interval: 0.001, renders: 1, cpu: 0.0, gpu: Some(0.0001), target: None, tick_period: 1.0 / 60.0, display: None });
        assert!(!g.exact);
    }
}
