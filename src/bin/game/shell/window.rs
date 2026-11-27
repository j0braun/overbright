//! The window: its surface and swap chain (`WindowState`), creating it, the
//! display mode, the performance overlay's display line and winit's
//! `ApplicationHandler`.
use crate::*;

/// `surface.configure` that survives another thread's submission (the
/// preload thread builds the world renderer and submits its uploads while
/// the window is first resized): wgpu waits for the queue to drain before
/// reconfiguring and reports "Failed to wait for GPU to come idle" (a
/// validation error, fatal by default) when a submission landed during
/// that wait. Seen at the first windowed start on Wayland.
/// Retried after the queue drains; any other
/// error, or the same one 100 times running, is fatal as before.
pub(crate) fn configure_surface(surface: &wgpu::Surface, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
    for attempt in 0..100 {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        surface.configure(device, config);
        let Some(error) = pollster::block_on(scope.pop()) else { return };
        let text = format!("{error} {error:?}");
        if !surface_busy(&text) || attempt == 99 {
            panic!("surface configure: {text}");
        }
        log::warn!("surface configure: the GPU was busy (another thread submitting), attempt {}", attempt + 1);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// A swap-chain acquire this long (s) is logged as a display stall.
pub(crate) const ACQUIRE_STALL_S: f32 = 0.25;

/// The configure error [`configure_surface`] retries.
pub(crate) fn surface_busy(error: &str) -> bool {
    error.contains("Failed to wait for GPU to come idle")
}

#[cfg(test)]
mod configure_surface_tests {
    /// The error wgpu-core 30 gives (ConfigureSurfaceError::GpuWaitTimeout,
    /// as the default handler printed it at the crash) is retried; others
    /// stay fatal.
    #[test]
    fn only_the_busy_error_is_retried() {
        assert!(super::surface_busy(
            "Validation Error\n\nCaused by:\n  In Surface::configure\n    Failed to wait for GPU to come idle before reconfiguring the Surface\n"
        ));
        assert!(!super::surface_busy(
            "Validation Error\n\nCaused by:\n  In Surface::configure\n    `SurfaceOutput` must be dropped before a new `Surface` is made"
        ));
    }
}

pub(crate) struct WindowState {
    /// None for the headless benchmark (`--bench`), which draws into
    /// `offscreen` instead.
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) surface: Option<wgpu::Surface<'static>>,
    pub(crate) offscreen: Option<wgpu::Texture>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// Shared with the menus' preload thread ([`Preload`]).
    pub(crate) gpu: Arc<Gpu>,
    pub(crate) depth: wgpu::TextureView,
    pub(crate) ui: UiRenderer,
    /// The end-of-frame screen passes (render::postfx).
    pub(crate) post: Option<PostFx>,
    /// The last frame's end, still going to the GPU on a render thread
    /// ([`nfsu2::render::FrameEnd`]): waited for before anything is
    /// written to the GPU again ([`WindowState::settle`]).
    pub(crate) frame_end: Option<nfsu2::render::FrameEnd>,
    /// The frames ended so far, and the latest ones' last submissions
    /// (number, index), for the bench's frames in flight.
    pub(crate) frames_ended: u64,
    pub(crate) in_flight: std::collections::VecDeque<(u64, wgpu::SubmissionIndex)>,
    /// The frame's buffer uploads: copies from reused staging chunks,
    /// recorded ahead of the passes reading them (`render::upload`); None
    /// with `NFSU2_NO_BELT=1` (each a `queue.write_buffer`).
    pub(crate) belt: Option<wgpu::util::StagingBelt>,
    /// The frame's cars' values, uploaded together.
    pub(crate) cars: nfsu2::render::car::CarBatch,
    /// The surface's present modes.
    pub(crate) present_modes: Vec<wgpu::PresentMode>,
    /// The Resolution / Screen Mode last applied to the window.
    pub(crate) applied: Option<((u32, u32), bool)>,
    /// Winit on Wayland (no exclusive full screen there).
    pub(crate) wayland: bool,
    /// The last frame's wait for its swap chain image (the F3 overlay's
    /// "acquire": long when the GPU or the display is the limit), seconds.
    pub(crate) acquire: f32,
    /// Low Latency: the last frame's wait for the frame before it to be
    /// submitted and presented (`settle`), seconds.
    pub(crate) settle_wait: f32,
    /// The waits in `settle` since the frame loop last cleared it (the
    /// streak buffer's probes settle too), seconds.
    pub(crate) settle_sum: f32,
    /// The last frame's acquire and settle, their starts and ends.
    pub(crate) acquired: Option<(Instant, Instant)>,
    pub(crate) settled: Option<(Instant, Instant)>,
    /// Low Latency with vsync off: the frame's just-in-time GPU wait
    /// (`jit_wait`), its start and end.
    pub(crate) gpu_wait: Option<(Instant, Instant)>,
    /// HDR Output (docs/playing/hdr.md; clone deviation): the signal the surface
    /// offers (None: SDR only; headless: `NFSU2_HDR_FAKE`), and while it is
    /// on, the display transform and its frame texture.
    pub(crate) hdr_offered: Option<nfsu2::render::hdr_output::Signal>,
    pub(crate) hdr: Option<HdrState>,
}

/// HDR Output in force on a window: the display transform reads the SDR
/// frame (windowed: `frame`, drawn instead of the swap chain image; it
/// writes the swap chain) or writes `frame` (headless: the HDR image,
/// beside the SDR offscreen target).
pub(crate) struct HdrState {
    pub(crate) signal: nfsu2::render::hdr_output::Signal,
    pub(crate) transform: nfsu2::render::hdr_output::DisplayTransform,
    pub(crate) frame: Option<((u32, u32), wgpu::Texture)>,
}

impl WindowState {
    /// The surface and depth at a new window size.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.settle();
        self.config.width = width;
        self.config.height = height;
        nfsu2::qol::set_output_size((width, height));
        if let Some(surface) = &self.surface {
            configure_surface(surface, &self.gpu.device, &self.surface_config());
        }
        self.depth = create_depth(&self.gpu, width, height);
    }

    pub(crate) fn new(
        window: Option<Arc<Window>>,
        surface: Option<wgpu::Surface<'static>>,
        offscreen: Option<wgpu::Texture>,
        config: wgpu::SurfaceConfiguration,
        gpu: Gpu,
        depth: wgpu::TextureView,
        ui: UiRenderer,
    ) -> WindowState {
        nfsu2::qol::set_output_size((config.width, config.height));
        let belt = (!debug_env().no_belt)
            .then(|| wgpu::util::StagingBelt::new(gpu.device.clone(), 1 << 20));
        let cars = nfsu2::render::car::CarBatch::new(&gpu);
        WindowState {
            window,
            surface,
            offscreen,
            config,
            gpu: Arc::new(gpu),
            depth,
            ui,
            post: None,
            frame_end: None,
            frames_ended: 0,
            in_flight: Default::default(),
            belt,
            cars,
            present_modes: Vec::new(),
            applied: None,
            wayland: false,
            acquire: 0.0,
            settle_wait: 0.0,
            settle_sum: 0.0,
            acquired: None,
            settled: None,
            gpu_wait: None,
            hdr_offered: nfsu2::qol::hdr_output_signal(),
            hdr: None,
        }
    }

    /// The swap chain's configuration: `config` (the SDR frame's format,
    /// which every renderer draws in) with HDR Output's format and colour
    /// space while it is on.
    pub(crate) fn surface_config(&self) -> wgpu::SurfaceConfiguration {
        let mut c = self.config.clone();
        if let Some(h) = &self.hdr {
            c.format = h.signal.format();
            c.color_space = h.signal.color_space();
        }
        c
    }

    /// HDR Output switched on or off (the Display page): the swap chain
    /// configured again.
    pub(crate) fn sync_hdr(&mut self) {
        let want = if nfsu2::qol::hdr_output_active() { self.hdr_offered } else { None };
        if want == self.hdr.as_ref().map(|h| h.signal) {
            return;
        }
        self.settle();
        self.hdr = want.map(|signal| HdrState {
            signal,
            transform: nfsu2::render::hdr_output::DisplayTransform::new(&self.gpu),
            frame: None,
        });
        log::info!("hdr output: {}", want.map_or("off (SDR)", |s| s.name()));
        if let Some(surface) = &self.surface {
            configure_surface(surface, &self.gpu.device, &self.surface_config());
        }
    }

    /// HDR Output's frame texture at `size` (see [`HdrState`]).
    pub(crate) fn hdr_frame(&mut self, size: (u32, u32)) -> Option<wgpu::TextureView> {
        let headless = self.surface.is_none();
        let sdr_format = self.config.format;
        let gpu = self.gpu.clone();
        let h = self.hdr.as_mut()?;
        let format = if headless { h.signal.format() } else { sdr_format };
        // Made again for another size or role (a photo capture draws
        // without the swap chain, at the window's size for a 1x picture).
        if h.frame.as_ref().is_none_or(|(s, t)| *s != size || t.format() != format) {
            let (format, usage) = if headless {
                (h.signal.format(), wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC)
            } else {
                (
                    sdr_format,
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST,
                )
            };
            let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hdr output frame"),
                size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            });
            h.frame = Some((size, t));
        }
        h.frame.as_ref().map(|(_, t)| t.create_view(&Default::default()))
    }

    /// Low Latency with vsync off: waits until the GPU has finished the
    /// frame before the last one (the second newest of `in_flight`, the
    /// last one settled just before), and keeps the wait's span for the
    /// pacer (`gpu_wait`). Nothing otherwise.
    pub(crate) fn jit_wait(&mut self) {
        if !(nfsu2::extras::low_latency()
            && !matches!(self.config.present_mode, wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed))
            || self.surface.is_none()
        {
            self.gpu_wait = None;
            return;
        }
        let began = Instant::now();
        if let Some((_, index)) = self.in_flight.iter().rev().nth(1).cloned() {
            let _ = self.gpu.device.poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: Some(std::time::Duration::from_millis(100)),
            });
        }
        self.gpu_wait = Some((began, Instant::now()));
    }

    /// Waits for the last frame's submission and present (see
    /// `frame_end`), and closes its GPU timing.
    pub(crate) fn settle(&mut self) {
        profiling::scope!("settle (the last frame's end)");
        let started = Instant::now();
        let settled = self.settle_inner();
        self.settle_wait = started.elapsed().as_secs_f32();
        self.settle_sum += self.settle_wait;
        self.settled = Some((started, Instant::now()));
        settled
    }

    fn settle_inner(&mut self) {
        let Some(end) = self.frame_end.take() else {
            if let Some(b) = self.belt.as_mut() {
                b.recall();
            }
            return;
        };
        if let Some(index) = end.wait() {
            self.in_flight.push_back((self.frames_ended, index));
            while self.in_flight.len() > 8 {
                self.in_flight.pop_front();
            }
        }
        self.frames_ended += 1;
        // The belt's chunks back once the GPU has copied from them (the map
        // callbacks run in a poll).
        if let Some(b) = self.belt.as_mut() {
            b.recall();
        }
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
        nfsu2::prof::gpu::end_frame(&self.gpu.device, &self.gpu.queue);
    }
}

/// What the window presents with, for the performance overlay.
pub(crate) fn overlay_display(
    window: Option<&WindowState>,
    settings: &options::Settings,
) -> nfsu2::overlay::DisplayInfo {
    let Some(w) = window else { return Default::default() };
    let info = w.gpu.adapter.get_info();
    nfsu2::overlay::DisplayInfo {
        present: format!("{:?}", w.config.present_mode),
        latency: w.config.desired_maximum_frame_latency,
        low_latency: nfsu2::extras::low_latency(),
        frame_cap: settings.enh.frame_cap.into(),
        size: (w.config.width, w.config.height),
        adapter: format!("{} ({:?})", info.name, info.backend),
    }
}

impl Game {
    pub(crate) fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        // The window at the Resolution's size in physical pixels (any
        // HiDPI scale factor leaves it so: `ScaleFactorChanged`), full
        // screen with the Screen Mode row (`apply_display_mode`).
        let (rw, rh) = self.settings.resolution;
        let made = nfsu2::prof::span("window: made");
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Overbright — Need for Speed Underground 2")
                    .with_window_icon(nfsu2::platform::icon::window_icon())
                    .with_inner_size(winit::dpi::PhysicalSize::new(rw, rh))
                    // Never smaller than the original's 640 × 480.
                    .with_min_inner_size(winit::dpi::PhysicalSize::new(
                        nfsu2::render::display::MIN_WINDOW.0,
                        nfsu2::render::display::MIN_WINDOW.1,
                    )),
            )?,
        );
        // The system cursor is hidden over the window: the game draws its
        // own (0x50B460) where it shows one. The original hides it in full
        // screen only (0x5D269F: SetCursor(0), ShowCursor(0) when 0x87098C
        // is 0); windowed too here (agreed deviation, docs/playing/native-deviations.md "Mouse cursor": no system arrow over
        // the game, none at all before the menus and while driving).
        window.set_cursor_visible(false);
        drop(made);
        nfsu2::diag::events::record(
            nfsu2::diag::events::Kind::Window,
            format!("window made, asked {rw}x{rh}, got {:?}", window.inner_size()),
        );
        let wayland = {
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                use winit::platform::wayland::ActiveEventLoopExtWayland;
                event_loop.is_wayland()
            }
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            {
                false
            }
        };
        // The Resolution row's lists per path from the window's monitor
        // (`display::set_monitor`; rebuilt when the window moves to
        // another one, `refresh_monitor`).
        // Read through `platform::monitor`: a display driver's odd answer
        // (a mode without its fields) leaves a fact unknown, never a panic.
        if let Some(m) = window.current_monitor().or_else(|| event_loop.primary_monitor()) {
            let facts = nfsu2::platform::monitor::facts(&m);
            let (name, size) = (facts.name.clone(), facts.size);
            nfsu2::render::display::set_monitor(nfsu2::render::display::MonitorInfo::from_facts(facts, wayland));
            self.settings.resolution = nfsu2::render::display::fit_mode(
                &nfsu2::render::display::modes_on(self.settings.fullscreen),
                self.settings.resolution,
            );
            nfsu2::diag::events::record(
                nfsu2::diag::events::Kind::Window,
                format!("monitor {:?}: {}x{}, scale {}, wayland {wayland}", name, size.0, size.1, m.scale_factor()),
            );
            log::info!(
                "monitor {:?}: {}x{} at {:.2} Hz, scale {}",
                name,
                size.0,
                size.1,
                m.refresh_rate_millihertz().unwrap_or(0) as f32 / 1000.0,
                m.scale_factor()
            );
        }
        // The device opened since the start (`shell::start_ahead`) when it
        // is the one this window's surface would get; else opened here.
        let ahead = self.platform.ahead.as_mut().and_then(|a| a.gpu.take()).and_then(|j| {
            let _t = nfsu2::prof::span("window: gpu ahead joined");
            j.join().ok()
        });
        let t = nfsu2::prof::span("window: instance");
        let instance = ahead.as_ref().map_or_else(Gpu::new_instance, |a| a.instance.clone());
        drop(t);
        let t = nfsu2::prof::span("window: surface");
        // Without a Vulkan backend (no loader, or no driver) Linux fails
        // here already: the player's line (`platform::vulkan`), wgpu's after.
        let surface = instance.create_surface(window.clone()).map_err(|e| {
            if cfg!(target_os = "linux") {
                anyhow::anyhow!("{}: {e}", nfsu2::platform::vulkan::no_adapter_line())
            } else {
                anyhow::Error::new(e)
            }
        })?;
        drop(t);
        // The overlay's GPU passes need timestamp queries on the device
        // (asked for only with the overlay's options set).
        if self.settings.overlay != nfsu2::overlay::Settings::default() || debug_env().overlay {
            nfsu2::prof::gpu::want(true);
        }
        let t = nfsu2::prof::span("window: gpu");
        let gpu = match ahead.and_then(|a| a.take(&surface)) {
            Some(gpu) => gpu,
            None => pollster::block_on(Gpu::new(Some(&surface), instance))?,
        };
        drop(t);
        let info = gpu.adapter.get_info();
        self.card = (info.vendor, info.device);
        self.platform.perf_overlay.pci = Some(self.card);
        // A fresh run (no saved settings): World Detail from the hardware
        // and the Customize Display values for it (0x5CECA0), kept in
        // options.txt at once so the profile loads' `Settings::load` keep it.
        if !options::Settings::path().exists() {
            self.settings.first_run_display(self.card);
            let _ = self.settings.save();
        }
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("surface not supported")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        // The SDR frame's format, picked by the game (8-bit first), not the
        // surface's first (`display::sdr_surface_format`).
        if let Some((format, space)) = nfsu2::render::display::sdr_surface_format(&caps) {
            config.format = format;
            config.color_space = space;
            // Kept for `game --prepare`, which has no window to ask.
            nfsu2::render::display::record_window_format(format);
        }
        // HDR Output (docs/playing/hdr.md): the HDR signal this surface can carry
        // (the Display page's row is greyed without one) and what the
        // display reports for its calibration's Auto values.
        let offered = nfsu2::render::hdr_output::choose(&caps);
        nfsu2::qol::set_hdr_output_support(offered);
        let info = surface.display_hdr_info(&gpu.adapter);
        nfsu2::qol::set_hdr_display_info(
            info.luminance.and_then(|l| l.max_nits),
            info.luminance.and_then(|l| l.sdr_white_nits),
        );
        log::info!(
            "hdr output: surface offers {}; display reports {:?}",
            offered.map_or("no HDR signal", |s| s.name()),
            info
        );
        // g_VSyncOn (0x5B9B13).
        let present_modes = surface.get_capabilities(&gpu.adapter).present_modes;
        config.present_mode = nfsu2::render::display::present_mode_for(&self.settings.custom, &present_modes);
        // Opaque where the surface offers it (any desktop one does): the
        // frame is opaque anyway, and the UI then needs no pass setting its
        // alpha back to 1 (`ui::Alpha::opaque`).
        if surface.get_capabilities(&gpu.adapter).alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        }
        let t = nfsu2::prof::span("window: configure");
        configure_surface(&surface, &gpu.device, &config);
        drop(t);
        let info = gpu.adapter.get_info();
        nfsu2::diag::events::record(
            nfsu2::diag::events::Kind::Surface,
            format!(
                "swap chain on {} ({:?}): {:?} {:?} at {}x{}",
                info.name, info.backend, config.format, config.present_mode, config.width, config.height
            ),
        );
        nfsu2::qol::log_file::fact(&format!(
            "display: {} ({:?}, {} {}); swap chain {:?} {:?} {:?} at {}x{}; surface formats {}",
            info.name,
            info.backend,
            info.driver,
            info.driver_info,
            config.format,
            config.color_space,
            config.present_mode,
            config.width,
            config.height,
            nfsu2::render::display::surface_formats_line(&caps, config.format)
        ));
        nfsu2::render::display::set_texture_filter(self.settings.custom[20]);
        let depth = create_depth(&gpu, config.width, config.height);
        let t = nfsu2::prof::span("window: ui renderer");
        let ui = UiRenderer::new(&gpu, config.format);
        drop(t);
        if let Some(d) = self.sim.drive.as_mut() {
            self.render.world = Some(WorldRenderer::new(&gpu, &d.world, config.format, &nfsu2::render::RenderOptions::in_force()));
            self.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, config.format, &nfsu2::render::RenderOptions::in_force()));
        }
        let mut w = WindowState::new(Some(window), Some(surface), None, config, gpu, depth, ui);
        w.present_modes = present_modes;
        w.wayland = wayland;
        self.platform.window = Some(w);
        let _t = nfsu2::prof::span("window: display mode");
        self.apply_display_mode();
        Ok(())
    }

    /// The window's monitor again (moved, a new scale factor,
    /// resized: it may be on another monitor, or the monitor set changed);
    /// when it differs, the Resolution row's lists are rebuilt and the
    /// size moves to one the active path can output (applied by
    /// `apply_display_mode` on the next frame).
    pub(crate) fn refresh_monitor(&mut self) {
        let Some(w) = self.platform.window.as_ref() else { return };
        let Some(m) = w.window.as_ref().and_then(|w| w.current_monitor()) else { return };
        if nfsu2::render::display::set_monitor(nfsu2::render::display::MonitorInfo::from_winit(&m, w.wayland)) {
            self.settings.resolution = nfsu2::render::display::fit_mode(
                &nfsu2::render::display::modes_on(self.settings.fullscreen),
                self.settings.resolution,
            );
        }
    }

    /// The Resolution and Screen Mode rows (clone options; the game
    /// resets its device for a new mode, 0x8709D1): applied to the window
    /// when they change. Windowed: the window at that size in physical
    /// pixels. Full screen: borderless on the window's monitor at the
    /// desktop size when that is the size, else the monitor's mode of that
    /// size at its highest refresh rate (exclusive; not on Wayland, which
    /// has none: borderless at the desktop size there, or when the monitor
    /// lacks the size). The surface follows the window's real size
    /// (`Resized`).
    pub(crate) fn apply_display_mode(&mut self) {
        let want = (self.settings.resolution, self.settings.fullscreen);
        let Some(w) = self.platform.window.as_mut() else { return };
        if w.applied == Some(want) {
            return;
        }
        w.applied = Some(want);
        let Some(window) = w.window.clone() else { return };
        let (size, fullscreen) = want;
        if fullscreen {
            let monitor = window.current_monitor();
            let facts = monitor.as_ref().map(nfsu2::platform::monitor::facts);
            let desktop = facts.as_ref().map(|f| f.size);
            // Not when the monitor's modes cannot be listed safely
            // (`platform::monitor::exclusive_mode`): borderless then.
            let exclusive = monitor
                .as_ref()
                .zip(facts.as_ref())
                .filter(|_| !w.wayland && desktop != Some(size))
                .and_then(|(m, f)| nfsu2::platform::monitor::exclusive_mode(m, f, size));
            match exclusive {
                Some(mode) => {
                    log::info!(
                        "full screen {}x{} at {:.2} Hz",
                        size.0,
                        size.1,
                        mode.refresh_rate_millihertz() as f32 / 1000.0
                    );
                    nfsu2::diag::events::record(
                        nfsu2::diag::events::Kind::Window,
                        format!("exclusive full screen {}x{}", size.0, size.1),
                    );
                    window.set_fullscreen(Some(winit::window::Fullscreen::Exclusive(mode)));
                }
                None => {
                    nfsu2::diag::events::record(
                        nfsu2::diag::events::Kind::Window,
                        format!("borderless full screen (asked {}x{}, desktop {desktop:?})", size.0, size.1),
                    );
                    window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)))
                }
            }
        } else {
            nfsu2::diag::events::record(nfsu2::diag::events::Kind::Window, format!("windowed {}x{}", size.0, size.1));
            window.set_fullscreen(None);
            if let Some(now) = window.request_inner_size(winit::dpi::PhysicalSize::new(size.0, size.1)) {
                w.resize(now.width, now.height);
            }
        }
    }
}

impl ApplicationHandler for Game {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.window.is_none()
            && let Err(e) = self.create_window(event_loop)
        {
            // The kind first, for the launcher's failure screen
            // (`launch::error::ErrorKind`), then the line it also knows.
            let text = format!("{e:#}");
            eprintln!("{}", nfsu2::launch::error::ErrorKind::of_window_error(&text).line());
            eprintln!("could not open a window: {text}");
            // In the log and, on Windows with no console, in a message box:
            // a start that ends here otherwise leaves no word.
            log::error!("could not open a window: {text}");
            nfsu2::diag::events::record(nfsu2::diag::events::Kind::Error, format!("could not open a window: {text}"));
            nfsu2::diag::events::dump_to_log("no window");
            nfsu2::qol::log_file::flush(std::time::Duration::from_millis(300));
            nfsu2::console::show_if_unseen(&format!("could not open a window: {text}"));
            event_loop.exit();
        }
        self.loads.waits.start_part("window");
        // The game start's PC_Loading over the rest of the start, only
        // once it has run [`LOADING_DELAY_MS`] (or with the opt-in minimum).
        let min_ms = self.settings.enh.boot_loading_min_ms;
        if let Some(l) =
            self.loads.boot_loading.as_mut().filter(|b| !b.ready).and_then(|b| b.frame(now_ms(self.pacing.started), min_ms))
        {
            present_loading(&mut self.platform.window, &self.game_data.assets, l, now_ms(self.pacing.started));
        }
        if self.audio.is_none() {
            let _t = nfsu2::prof::span("start: audio device");
            // The stream asked for since the start (`shell::start_ahead`).
            let opening = self.platform.ahead.as_mut().and_then(|a| a.audio.take());
            let mixer = match opening {
                Some(o) => o.finish(),
                None => Mixer::new(),
            };
            self.audio = mixer.map(|mixer| self.new_audio(mixer));
        }
        self.platform.ahead = None;
        self.loads.waits.start_part("audio");
        if let Some(b) = self.loads.boot_loading.as_mut() {
            b.ready = true;
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                nfsu2::diag::events::record(nfsu2::diag::events::Kind::Window, "close asked");
                event_loop.exit()
            }
            // Power Saving's background test.
            // Either way the held keys and modifiers are let go
            // (`InputEvent::Focus`): their releases may have gone to another
            // window (Alt-Tab).
            // In the background's Pause reads them too (`background_pause`).
            WindowEvent::Focused(f) => {
                self.pacing.focused = f;
                nfsu2::diag::events::set_shown(f, self.pacing.occluded);
                crate::shell::support::modifiers(false);
                self.input_now(InputEvent::Focus(f));
            }
            WindowEvent::Occluded(o) => {
                self.pacing.occluded = o;
                nfsu2::diag::events::set_shown(self.pacing.focused, o);
            }
            WindowEvent::Resized(size) => {
                nfsu2::diag::events::record(
                    nfsu2::diag::events::Kind::Window,
                    format!("resized to {}x{}", size.width, size.height),
                );
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if let Some(w) = self.platform.window.as_mut() {
                    w.resize(size.width, size.height);
                }
                self.refresh_monitor();
            }
            // Moved, perhaps to another monitor.
            WindowEvent::Moved(_) => self.refresh_monitor(),
            // A new HiDPI scale (another monitor, a changed setting): the
            // window keeps its size in physical pixels, the Resolution's
            // (winit would scale it).
            WindowEvent::ScaleFactorChanged { mut inner_size_writer, scale_factor } => {
                nfsu2::diag::events::record(nfsu2::diag::events::Kind::Window, format!("scale factor {scale_factor}"));
                if let Some(w) = self.platform.window.as_ref().filter(|w| !w.applied.is_some_and(|a| a.1)) {
                    let _ = inner_size_writer
                        .request_inner_size(winit::dpi::PhysicalSize::new(w.config.width, w.config.height));
                }
                self.refresh_monitor();
            }
            // Keys (modifiers, which the game reads as held
            // keys, and the typed text with them) and the clicking button,
            // the wheel and the cursor go through the tick queue: applied
            // at the start of the tick their arrival falls in
            // (`input::queue`), never between ticks.
            WindowEvent::ModifiersChanged(m) => {
                crate::shell::support::modifiers(m.state().control_key());
                self.input_now(InputEvent::Alt(m.state().alt_key()));
            }
            // The system's made-up presses and releases at a focus change
            // (X11) are not the player's: the focus lets every key go and
            // the modifiers come with `ModifiersChanged`.
            WindowEvent::KeyboardInput { is_synthetic: true, .. } => {}
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    // Ctrl+F8: the recent events to the log and a support
                    // file (docs/troubleshooting.md). The keys still go to
                    // the game as any other.
                    crate::shell::support::key(code, event.state == ElementState::Pressed, event.repeat);
                    if event.state == ElementState::Pressed && !event.repeat {
                        self.loads.waits.key();
                    }
                    let e = match event.state {
                        ElementState::Pressed => InputEvent::KeyDown {
                            code,
                            text: event.text.as_ref().map(|t| t.to_string()),
                            repeat: event.repeat,
                        },
                        ElementState::Released => InputEvent::KeyUp(code),
                    };
                    self.input_now(e);
                }
            }
            WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } => {
                if matches!(event, WindowEvent::MouseInput { state: ElementState::Pressed, .. }) {
                    self.loads.waits.key();
                }
                if let Some(e) = crate::input::mouse::mouse_input(&event) {
                    self.input_now(e);
                }
            }
            // Paused in the background: an uncovered window is drawn again
            // as it stood (no time passes; a hidden one is not drawn).
            WindowEvent::RedrawRequested if self.pacing.background_paused => {
                if !self.pacing.occluded {
                    self.pacing.last = Instant::now();
                    if let Err(e) = self.redraw() {
                        log::error!("frame: {e:#}");
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.redraw() {
                    log::error!("frame: {e:#}");
                    nfsu2::diag::events::record(nfsu2::diag::events::Kind::Error, format!("frame: {e:#}"));
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        nfsu2::diag::events::loop_turned();
        // The quit question's Quit Game (`quit_app`, set on a tick by a
        // key, a click or a controller) and a controller's Back on the
        // main menu end the event loop.
        if self.platform.exit_requested || self.platform.quit_app {
            event_loop.exit();
        }
        // In the background's Pause: the loop sleeps until the next window
        // event (SPEED2.EXE's pump 0x5BF1D0 waits in GetMessage).
        if self.background_pause() {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
            return;
        }
        // The next frame's waits (the frame cap, Low Latency's pacing) are
        // slept here, in the event loop, so the input that comes in during
        // them is read for that frame's ticks (`Game::wait_for_frame`).
        if self.platform.window.as_ref().is_some_and(|w| w.window.is_some())
            && let Some(wake) = self.wait_for_frame()
        {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(wake));
            return;
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        if let Some(window) = self.platform.window.as_ref().and_then(|w| w.window.as_ref()) {
            window.request_redraw();
        }
    }
}

/// The window, the input devices and the per-frame UI uploads (were
/// `Game` fields).
pub(crate) struct Platform {
    pub(crate) keys: HashSet<KeyCode>,
    /// The system's Alt modifier (`InputEvent::Alt`), for Alt+Enter.
    pub(crate) alt: bool,
    pub(crate) window: Option<WindowState>,
    /// The quit question was answered Quit Game: the event loop ends.
    pub(crate) quit_app: bool,
    /// F3: the performance overlay (not a game feature; the clone's own,
    /// `nfsu2::overlay`, docs/notes/performance/overlay.md).
    pub(crate) perf_overlay: nfsu2::overlay::Overlay,
    /// The front end's mouse, sampled once a frame (`mouse::fe_mouse`).
    pub(crate) mouse: nfsu2::frontend::mouse::Mouse,
    /// The front end mouse's held, mute, arrow and map-zone state.
    pub(crate) mouse_state: crate::input::mouse::MouseState,
    /// The window's key and mouse events waiting for their tick
    /// (`input::queue`).
    pub(crate) input: crate::input::InputQueue,
    /// The mirror's image as bound in the UI renderer.
    pub(crate) rear_view_bound: Option<wgpu::TextureView>,
    /// The controllers (`nfsu2::pad`): gilrs in the window, the synthetic
    /// script (`NFSU2_PAD_SCRIPT`) everywhere.
    pub(crate) pads: nfsu2::pad::host::Host,
    /// A controller's Back on the main menu asked to quit.
    pub(crate) exit_requested: bool,
    /// Streamed textures for the front end's next draw (the showcase's
    /// masthead and extra).
    pub(crate) ui_uploads: Vec<nfsu2::formats::texture::Texture>,
    /// The EA Trax song whose caption is up and when it started
    /// (Chyron_ShowSong 0x4AC9F0; `audio::music::Chyron`).
    pub(crate) chyron: Option<nfsu2::audio::music::Chyron>,
    /// The window's graphics and sound devices being made since the start
    /// (`shell::start_ahead`), taken when the window is made.
    pub(crate) ahead: Option<crate::shell::start_ahead::StartAhead>,
}

impl Platform {
    /// The values `Game::new` starts with; opens the controllers
    /// (`Host::new`, which reads `NFSU2_PAD_SCRIPT`).
    pub(crate) fn new(perf_overlay: nfsu2::overlay::Overlay) -> Platform {
        Platform {
            keys: HashSet::new(),
            alt: false,
            window: None,
            quit_app: false,
            perf_overlay,
            mouse: Default::default(),
            mouse_state: Default::default(),
            input: Default::default(),
            rear_view_bound: None,
            pads: nfsu2::pad::host::Host::new(false),
            exit_requested: false,
            ui_uploads: Vec::new(),
            chyron: None,
            ahead: None,
        }
    }
}
