//! `overbright-editor`: the desktop map editor (src/editor/, docs/modding/map-editor.md).
//!
//! ```text
//! overbright-editor [--mod <folder>] [--region <code>] [--maps <dir>]
//!                                   the window, with that map mod open
//! overbright-editor --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--region <code>]
//!                   [--mode props|races] [--tool <name>] [--view 3d|map|street|whole|top] [--tab browser|validation|simulate|log] [--select N,M]
//!                   [--light work|game] [--sim stream.txt[@seconds[:car]]] [--hover simulate|drive-here|test-race]
//!                   [--dialog how-to-test|controls] [--run test-race|drive-here] (--run: OB_GAME_EXE a stand-in)
//!                                   the whole window drawn off-screen into a PNG (no window)
//! ```
//! The game folder is `NFSU2_ROOT`, else the one the launcher kept.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result};
use ob_editor::editor::app::{self, App, Cli};
use nfsu2::render::Gpu;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

fn game_root() -> PathBuf {
    if let Some(r) = std::env::var_os("NFSU2_ROOT").filter(|r| !r.is_empty()) {
        return PathBuf::from(r);
    }
    nfsu2::qol::setup::saved_root().unwrap_or_else(|| PathBuf::from("."))
}

fn new_app(cli: &Cli) -> App {
    let maps = cli.maps.clone().unwrap_or_else(nfsu2::tools::map::store::maps_dir);
    App::new(game_root(), maps, Box::new(|root, region| nfsu2::world::World::load(root, region)))
}

fn main() -> ExitCode {
    // A portable install keeps the driver's shader caches in its data
    // folder too, as the game does (`platform::dirs::driver_cache_vars`;
    // nothing otherwise).
    let portable = nfsu2::platform::dirs::portable_base();
    for (name, dir) in nfsu2::platform::dirs::driver_cache_vars(
        nfsu2::platform::dirs::Os::CURRENT,
        &|n: &str| std::env::var_os(n),
        portable.as_deref(),
    ) {
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var(name, dir) };
    }
    nfsu2::console::attach();
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).try_init();
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{} map editor (preview)", nfsu2::build_info::version_line());
        return ExitCode::SUCCESS;
    }
    log::info!("{} map editor (preview)", nfsu2::build_info::version_line());
    let cli = match Cli::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            nfsu2::console::fatal(&format!("{e:#}"));
            return ExitCode::from(2);
        }
    };
    let r = if cli.shot.is_some() { shot(&cli) } else { window(cli) };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            nfsu2::console::fatal(&format!("overbright-editor: {e:#}"));
            ExitCode::FAILURE
        }
    }
}

fn shot(cli: &Cli) -> Result<()> {
    let out = cli.shot.clone().context("--shot needs a path")?;
    let mut app = new_app(cli);
    if let Some(f) = &cli.folder {
        app.open(f, cli.region.as_deref())?;
    }
    app::apply_cli_view(&mut app, cli);
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    app::shot(&mut app, &gpu, cli.size, cli.scale, &out)?;
    println!("wrote {} ({}x{})", out.display(), cli.size.0, cli.size.1);
    Ok(())
}

struct Win {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    egui: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    title: String,
}

struct Runner {
    app: App,
    cli: Cli,
    win: Option<Win>,
    /// When egui wants the next frame.
    repaint_at: Option<Instant>,
    error: Option<anyhow::Error>,
}

impl Runner {
    fn open_window(&mut self, el: &ActiveEventLoop) -> Result<Win> {
        let attrs = Window::default_attributes()
            .with_title(self.app.title())
            .with_window_icon(nfsu2::platform::icon::window_icon())
            .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(960.0, 600.0));
        let window = Arc::new(el.create_window(attrs)?);
        let instance = Gpu::new_instance();
        let surface = instance.create_surface(window.clone())?;
        let gpu = pollster::block_on(Gpu::new(Some(&surface), instance))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("the window cannot be drawn on this GPU")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        // egui blends in gamma space: a non-sRGB format, as the game's.
        if let Some((format, space)) = nfsu2::render::display::sdr_surface_format(&caps) {
            config.format = format;
            config.color_space = space;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);
        let max_tex = gpu.device.limits().max_texture_dimension_2d as usize;
        let egui = egui_winit::State::new(
            self.app.ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(max_tex),
        );
        let renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            config.format,
            egui_wgpu::RendererOptions { msaa_samples: 1, ..Default::default() },
        );
        Ok(Win { window, surface, config, gpu, egui, renderer, title: String::new() })
    }

    fn redraw(&mut self) {
        let Some(w) = self.win.as_mut() else { return };
        self.app.tick(Instant::now());
        let raw = w.egui.take_egui_input(&w.window);
        let mut full = self.app.frame(raw);
        let platform = std::mem::take(&mut full.platform_output);
        w.egui.handle_platform_output(&w.window, platform);
        self.repaint_at =
            full.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| Instant::now().checked_add(v.repaint_delay));
        let title = self.app.title();
        if title != w.title {
            w.window.set_title(&title);
            w.title = title;
        }
        let frame = match w.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                w.surface.configure(&w.gpu.device, &w.config);
                w.window.request_redraw();
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let bufs = self.app.paint(&w.gpu, &mut w.renderer, &view, (w.config.width, w.config.height), full);
        w.gpu.queue.submit(bufs);
        w.window.pre_present_notify();
        w.gpu.queue.present(frame);
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.win.is_some() {
            return;
        }
        match self.open_window(el) {
            Ok(w) => {
                w.window.request_redraw();
                self.win = Some(w);
                if let Some(f) = self.cli.folder.clone()
                    && let Err(e) = self.app.open(&f, self.cli.region.as_deref())
                {
                    self.app.dialog =
                        Some(app::Dialog::Message { title: "Could not open the mod".into(), text: format!("{e:#}") });
                }
            }
            Err(e) => {
                self.error = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(w) = self.win.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => {
                if self.app.close_requested() {
                    el.exit();
                } else {
                    w.window.request_redraw();
                }
                return;
            }
            WindowEvent::Resized(size) => {
                w.config.width = size.width.max(1);
                w.config.height = size.height.max(1);
                w.surface.configure(&w.gpu.device, &w.config);
                w.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.app.quit {
                    el.exit();
                }
                return;
            }
            _ => {}
        }
        let r = w.egui.on_window_event(&w.window, &event);
        if r.repaint {
            w.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let Some(w) = self.win.as_ref() else { return };
        let now = Instant::now();
        // Redraw on input, when egui asks, and for the autosave.
        let mut wake = self.app.next_tick();
        if let Some(t) = self.repaint_at {
            if t <= now {
                self.repaint_at = None;
                w.window.request_redraw();
            } else {
                wake = wake.min(t);
            }
        }
        if wake <= now {
            w.window.request_redraw();
        }
        el.set_control_flow(ControlFlow::WaitUntil(wake.max(now)));
    }
}

fn window(cli: Cli) -> Result<()> {
    let el = EventLoop::new()?;
    let app = new_app(&cli);
    let mut r = Runner { app, cli, win: None, repaint_at: None, error: None };
    el.run_app(&mut r)?;
    match r.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
