//! The window's frame loop: a redraw, the power and VRR frame caps, a
//! frame's update (`step`) and its drawing (`draw`).
use crate::*;

/// A frame interval this long (s) is an event of the diagnostics' ring.
const STALL_EVENT_S: f32 = 0.25;

impl Game {
    pub(crate) fn redraw(&mut self) -> Result<()> {
        // The game start's PC_Loading (only past 200 ms, or for the
        // opt-in minimum), until it comes down; the boot screens then start
        // from their first frame.
        let min_ms = self.settings.enh.boot_loading_min_ms;
        if let Some(b) = self.loads.boot_loading.as_mut() {
            if !b.done(min_ms) {
                if let Some(l) = b.frame(now_ms(self.pacing.started), min_ms) {
                    present_loading(&mut self.platform.window, &self.game_data.assets, l, now_ms(self.pacing.started));
                }
                return Ok(());
            }
            log::info!(
                "game start: done after {:.0} ms, PC_Loading {} (minimum {} ms)",
                b.since.elapsed().as_secs_f32() * 1000.0,
                if b.shown { "shown" } else { "not shown (under 200 ms)" },
                min_ms
            );
            self.loads.boot_loading = None;
            self.pacing.last = Instant::now();
        }
        // The frame limiter (`display::frame_period`): the optional
        // NFSU2_FRAME_CAP, waited out before input and the simulation, so
        // it adds no latency. A scene that holds the cars keeps the
        // original's 1/30 s world updates through the cadence (`advance`,
        // `frame_clock::SCENE`) and is drawn at the display's rate between
        // them: a render held to 30 fps judders on a 75 / 144 Hz display
        // (docs/notes/front-end/postrace-jitter.md).
        // Low Latency (docs/playing/modern-extras.md): with a VRR panel driven
        // variably, frames held 3 % under its top rate; then the pacer's
        // sleep, the wait the last frames spent after their input moved
        // before this one's.
        let low_latency = nfsu2::extras::low_latency();
        let mut period = nfsu2::render::display::frame_period(false);
        // Power Saving (opt-in; Off, the default, adds nothing): the
        // background / menu / battery cap, the longer of it and the above.
        if let Some(cap) = self.power_cap() {
            period = Some(period.map_or(cap, |p| p.max(cap)));
        }
        if low_latency {
            if let Some(vrr) = self.vrr_period() {
                period = Some(period.map_or(vrr, |p| p.max(vrr)));
            }
            self.pacing.pacer.set_period(self.pacing.refresh_hz.filter(|&hz| hz > 1.0).map(|hz| 1.0 / hz));
        }
        let limiter = Instant::now();
        if let Some(period) = period {
            nfsu2::render::display::wait_until(self.pacing.last + period);
        }
        let epoch = self.pacing.started;
        let secs = |t: Instant| (t - epoch).as_secs_f64();
        if low_latency {
            let due = self.pacing.pacer.due(secs(Instant::now()));
            nfsu2::render::display::wait_until(epoch + std::time::Duration::from_secs_f64(due.max(0.0)));
        }
        let now_t = Instant::now();
        // The frame's work starts: the reader ahead gives way to a frame
        // that runs long (docs/developing/read-ahead.md).
        nfsu2::cache::read_ahead::frame_started();
        let capped = (now_t - limiter).as_secs_f32();
        let elapsed = now_t - self.pacing.last;
        let interval = elapsed.as_secs_f32();
        self.pacing.last = now_t;
        if let Some(w) = self.platform.window.as_mut() {
            w.settle_sum = 0.0;
        }
        let alpha = self.advance_ns(u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX))?;
        let stepped = now_t.elapsed().as_secs_f32();
        let probes_in_ticks = self.render.streak.probe_wall;
        let result = self.draw_between(alpha);
        // After the first frame: the next minutes' files read ahead.
        if !self.loads.read_ahead_queued {
            self.read_ahead_after_start();
        }
        let busy = now_t.elapsed().as_secs_f32();
        // The diagnostics (docs/troubleshooting.md): the watchdog's
        // heartbeat, and a long frame as an event.
        nfsu2::diag::events::frame_drawn();
        if interval >= STALL_EVENT_S {
            nfsu2::diag::events::record(
                nfsu2::diag::events::Kind::Stall,
                format!("frame took {:.0} ms (its own work {:.0} ms)", interval * 1000.0, busy * 1000.0),
            );
        }
        self.waits_frame(interval * 1000.0);
        self.streak_window_sample(interval, busy - (stepped - probes_in_ticks).max(0.0), period);
        let record = if self.platform.perf_overlay.active(&self.settings.overlay) {
            Some(self.overlay_record(now_t, interval, capped, stepped, probes_in_ticks, busy))
        } else {
            None
        };
        if let Some(w) = self.platform.window.as_mut() {
            if low_latency {
                // The present goes out on a render thread right after;
                // the frame's end here stands in for it (**inferred**: the
                // GPU's own time is in the slack).
                // The longer of the frame's two waits (the last frame's
                // present, the image's acquire) is the display's: a
                // blocked one ends at a refresh; the other is taken off
                // the work.
                let end = Instant::now();
                let span = |p: Option<(Instant, Instant)>| p.filter(|(a, _)| *a >= now_t).unwrap_or((end, end));
                let (acq, set) = (span(w.acquired.take()), span(w.settled.take()));
                let ((ready, got), other) = if set.1 - set.0 > acq.1 - acq.0 { (set, acq) } else { (acq, set) };
                let present = secs(end) - (other.1 - other.0).as_secs_f64();
                self.pacing.pacer.record(nfsu2::latency::FrameTimes {
                    input: secs(now_t),
                    ready: secs(ready),
                    got: secs(got),
                    present: present.max(secs(got)),
                });
            } else if self.pacing.pacer.frames != 0 {
                self.pacing.pacer.reset();
            }
        }
        if let Some(record) = record {
            self.platform.perf_overlay.push(record, &self.settings.overlay);
        }
        if let Some(w) = self.platform.window.as_ref() {
            self.platform.perf_overlay.sync_gpu_timing(&w.gpu.device, &w.gpu.queue, &self.settings.overlay);
        }
        result
    }

    /// The performance overlay's record of this window frame
    /// (`overlay::stats::FrameRecord`): `now_t` its start, `interval` since
    /// the frame before began, `capped` the frame limiter's sleep before it,
    /// `stepped` the time to the end of its ticks (the probes between them
    /// included: `probes_in_ticks` of their wall time), `busy` to the end
    /// of its drawing. The main thread's parts: the ticks; the probes
    /// without their waits; the waits for the display (every `settle` of
    /// the frame and the swap-chain image's acquire); the drawing, the rest.
    fn overlay_record(
        &mut self,
        now_t: Instant,
        interval: f32,
        capped: f32,
        stepped: f32,
        probes_in_ticks: f32,
        busy: f32,
    ) -> nfsu2::overlay::stats::FrameRecord {
        let streak_exact = self.streak_in_use().then_some(!self.render.streak.cheap_frame);
        let s = &self.render.streak;
        let (settles, acquire) = self.platform.window.as_ref().map_or((0.0, 0.0), |w| {
            let acquire = w.acquired.filter(|(a, _)| *a >= now_t).map_or(0.0, |(a, b)| (b - a).as_secs_f32());
            (w.settle_sum, acquire)
        });
        let sim = (stepped - probes_in_ticks).max(0.0);
        let probe_time = (s.probe_wall - s.probe_settle).max(0.0);
        let wait = settles + acquire;
        nfsu2::overlay::stats::FrameRecord {
            interval,
            cap: capped,
            sim,
            ticks: u32::try_from(self.pacing.frame_updates).unwrap_or(u32::MAX),
            dropped: self.pacing.frame_dropped,
            draw: (busy - wait - sim - probe_time).max(0.0),
            probes: s.probes,
            probe_time,
            wait,
            acquire,
            streak_exact,
            ..Default::default()
        }
    }

    /// The light streaks' governor fed with this window frame
    /// (`render::streak`) while the trail buffer is in use (a scene's
    /// frames: the menus' are no scene render): its interval, the main
    /// thread's rendering (`rendering`: the frame's time to the end of its
    /// drawing without its ticks' simulation, here without the waits for
    /// the swap chain image and the frames before), the GPU time per scene
    /// render, and the period the frame waits for (the cap, or the refresh
    /// with vsync).
    fn streak_window_sample(&mut self, interval: f32, rendering: f32, cap: Option<std::time::Duration>) {
        if !self.streak_in_use() {
            return;
        }
        let tick_period = self.tick_period_now();
        let Some(w) = self.platform.window.as_ref() else { return };
        let cpu = (rendering - w.acquire - w.settle_sum).max(0.0);
        let vsync = matches!(w.config.present_mode, wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed);
        let s = &mut self.render.streak;
        if s.refresh.as_ref().is_none_or(|(_, at)| at.elapsed().as_secs_f32() > 2.0) {
            let hz = w
                .window
                .as_ref()
                .and_then(|w| w.current_monitor())
                .and_then(|m| m.refresh_rate_millihertz())
                .map(|mhz| mhz as f32 / 1000.0)
                .filter(|&hz| hz > 1.0);
            s.refresh = Some((hz, Instant::now()));
        }
        let refresh = s.refresh.and_then(|(hz, _)| hz).map(|hz| 1.0 / hz);
        let longer = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let cap = cap.map(|c| c.as_secs_f32());
        let target = longer(cap, refresh.filter(|_| vsync));
        // The period frames reach the screen at, vsync or not (the drawn
        // frames' own share of the machine).
        let display = longer(cap, refresh);
        let sample = crate::render::streak::FrameSample {
            interval,
            renders: 1 + s.probes,
            cpu,
            gpu: s.gpu_cost,
            target,
            tick_period,
            display,
        };
        self.streak_govern(sample);
    }

    /// Power Saving's cap for this frame (`nfsu2::extras::power_cap`):
    /// None with the row Off (no state read at all). The monitor's refresh
    /// rate and the battery are read again every few seconds.
    pub(crate) fn power_cap(&mut self) -> Option<std::time::Duration> {
        let level = nfsu2::extras::power_saving();
        if level == 0 {
            return None;
        }
        if self.pacing.battery.as_ref().is_none_or(|(_, at)| at.elapsed().as_secs_f32() > 5.0) {
            let battery = level >= 3 && nfsu2::extras::on_battery();
            self.pacing.refresh_hz = self
                .platform.window
                .as_ref()
                .and_then(|w| w.window.as_ref())
                .and_then(|w| w.current_monitor())
                .and_then(|m| m.refresh_rate_millihertz())
                .map(|mhz| f64::from(mhz) / 1000.0)
                .or(self.pacing.refresh_hz);
            self.pacing.battery = Some((battery, Instant::now()));
        }
        let state = nfsu2::extras::PowerState {
            foreground: self.pacing.focused && !self.pacing.occluded,
            menu: matches!(self.mode, Mode::Menu(_)),
            battery: self.pacing.battery.is_some_and(|b| b.0),
            refresh_hz: self.pacing.refresh_hz,
        };
        nfsu2::extras::power_cap(level, state)
    }

    /// The VRR cap's period (`latency::vrr_period`) while a panel is
    /// driven variably; the probe is asked again every 2 s (a switch to
    /// full screen turns VRR on).
    pub(crate) fn vrr_period(&mut self) -> Option<std::time::Duration> {
        if self.pacing.vrr.as_ref().is_none_or(|(_, at)| at.elapsed().as_secs_f32() > 2.0) {
            let v = nfsu2::latency::vrr::probe();
            if self.pacing.vrr.as_ref().is_none_or(|(old, _)| *old != v) {
                log::info!("low latency: {}", v.describe());
            }
            self.pacing.refresh_hz = self
                .platform.window
                .as_ref()
                .and_then(|w| w.window.as_ref())
                .and_then(|w| w.current_monitor())
                .and_then(|m| m.refresh_rate_millihertz())
                .map(|mhz| f64::from(mhz) / 1000.0);
            self.pacing.vrr = Some((v, Instant::now()));
        }
        let (v, _) = self.pacing.vrr.as_ref()?;
        if v.active() { nfsu2::latency::vrr_period(self.pacing.refresh_hz) } else { None }
    }

    /// A drawn frame counted (`frame_count`, presentation only; the env
    /// faces follow the ticks). The world counter counts ticks, in `Game::update`
    /// (`Pacing::clocks`).
    pub(crate) fn frame_drawn(&mut self) {
        self.pacing.frame_count = self.pacing.frame_count.wrapping_add(1);
    }

    /// A frame's simulation: the game, its sounds and particles.
    pub(crate) fn step(&mut self, dt: f32) -> Result<()> {
        profiling::scope!("step");
        self.pacing.frame_dt = dt;
        self.pacing.world_time += f64::from(dt);
        // The device stream kept alive (`Mixer::keep_alive`).
        if let Some(a) = self.audio.as_ref() {
            a.mixer.keep_alive();
        }
        dev_hooks::teleport(self);
        self.audio_step_begin();
        let span = nfsu2::prof::acc::span("tick: update");
        self.update(dt)?;
        drop(span);
        // The HUD's update on the tick, after the world update and
        // the package clock, before the tick's sounds (0x5811D0 steps 13,
        // 15, 16, 18); its sounds go out with them (`audio_sounds`).
        self.overlay_update(self.now());
        // The renderers of the cars this update dropped go with them.
        self.render.sweep();
        let span = nfsu2::prof::acc::span("tick: preload");
        self.preload_frame();
        self.programs_frame();
        self.cars_ahead_tick();
        drop(span);
        // GarageMain's update (0x4D82D0 from the front end's tick
        // 0x5663C0, step 15 of 0x5811D0) and the camera movers (0x44EFE0,
        // step 17: the dyno camera) before the AudioManager (step 18): the
        // garage's sounds go out with this tick's (docs/original/uncapped-fps.md,
        // "Front end out of the draw").
        self.fe_tick_before_sounds();
        let span = nfsu2::prof::acc::span("tick: audio");
        self.audio_frame(dt);
        drop(span);
        let tick_settings = self.pacing.tick_settings;
        let span = nfsu2::prof::acc::span("tick: particles");
        self.particle_effects(dt, &tick_settings);
        drop(span);
        // The front end's state the draw stepped before (photo
        // mode's close, the menus' hit map).
        self.fe_tick();
        // The native visuals at the tick's camera (render::visuals).
        self.visuals_tick();
        Ok(())
    }

    /// Photo mode closes once no pause menu is up. Called first
    /// in the tick's front-end step (`fe_tick`, not in `draw`).
    pub(crate) fn photo_close_update(&mut self) {
        if self.photo.is_some()
            && !matches!(&self.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause | MenuKind::RacePause)))
        {
            self.photo_close();
        }
    }

    /// A frame's drawing on the newest tick (α = 1), presented.
    pub(crate) fn draw(&mut self) -> Result<()> {
        self.draw_at(1.0)
    }

    /// A frame's drawing `alpha` of the way between the last two ticks
    /// (the frame's values from the tick snapshots, before
    /// anything of the frame runs), presented. Photo mode swaps its
    /// view in for the frame and puts the game's back; its camera is the
    /// frame's then.
    pub(crate) fn draw_at(&mut self, alpha: f32) -> Result<()> {
        // A Graphics row changed since the drive's programs were made (the
        // options behind the pause menu): made now, before this frame draws
        // with them (docs/developing/first-use-builds.md).
        self.warm_for_settings();
        // A frame between ticks shows the scratch over the
        // committed buffer: the tick it stands after is committed first,
        // from its own frame (a probe at α = 0, `render::streak`).
        if alpha > 0.0 && alpha < 1.0 {
            self.streak_probe(0.0)?;
        }
        self.draw_at_as(alpha, false)
    }

    /// [`Game::draw_at`] without its probe; `probe`: drawn offscreen for
    /// the streak buffer's commit only (`render::streak`: no
    /// output, no UI, not presented, not counted).
    pub(crate) fn draw_at_as(&mut self, alpha: f32, probe: bool) -> Result<()> {
        let poses = self.frame_poses(alpha);
        if !probe {
            dev_hooks::frame_trace(self, &poses);
        }
        let swapped = self.photo_begin(poses.photo_rig);
        let editor = self.free_camera_begin();
        let photo_camera = swapped.as_ref().is_some_and(|s| s.camera_swapped()) || editor.is_some();
        let r = self.draw_frame(poses, photo_camera, probe);
        if let Some(s) = editor {
            self.free_camera_end(s);
        }
        if let Some(s) = swapped {
            self.photo_end(s);
        }
        r
    }
}
