//! `overbright-studio`: Overbright Studio's window (src/studio/, docs/modding/studio.md), a preview: the shell
//! with Home; the map editor (`overbright-editor`) moves in as its World workspace later.
//!
//! ```text
//! overbright-studio [--mod <folder>] [--maps <dir>]      the window, with that mod open
//! overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
//!                   [--workspace home|world|races|mod|settings] [--input mouse|keyboard|xbox|playstation|deck]
//!                   [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--find] [--address]
//!                                   the whole window drawn off-screen into a PNG (no window)
//! ```
//! A `--shot` prints one line "studio layout: fits …" or "studio layout: clipped: …" (the clip and overlap check).

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result, bail};
use nfsu2::studio_api::render::Gpu;
use ob_look::legends::InputKind;
use ob_studio_gui::studio::nav::{Area, Pad};
use ob_studio_gui::studio::workspace::Workspace;
use ob_studio_gui::studio::{Studio, layout, shot};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const USAGE: &str = "usage: overbright-studio [--mod <folder>] [--maps <dir>]\n       overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>] [--workspace home|world|races|mod|settings] [--input mouse|keyboard|xbox|playstation|deck] [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--find] [--address]";

#[derive(Default)]
struct Cli {
    folder: Option<String>,
    maps: Option<PathBuf>,
    shot: Option<PathBuf>,
    size: Option<(u32, u32)>,
    scale: Option<f32>,
    workspace: Option<Workspace>,
    input: Option<InputKind>,
    focus: Option<Area>,
    drawer: bool,
    find: bool,
    address: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Cli> {
    let mut c = Cli::default();
    let mut it = args;
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().with_context(|| format!("{name} needs a value\n{USAGE}"));
        match a.as_str() {
            "--mod" => c.folder = Some(val("--mod")?),
            "--maps" => c.maps = Some(PathBuf::from(val("--maps")?)),
            "--shot" => c.shot = Some(PathBuf::from(val("--shot")?)),
            "--size" => {
                let v = val("--size")?;
                let (w, h) = v.split_once('x').context("--size: WxH")?;
                c.size = Some((w.parse()?, h.parse()?));
            }
            "--scale" => c.scale = Some(val("--scale")?.parse()?),
            "--workspace" => {
                let v = val("--workspace")?;
                c.workspace = Some(
                    Workspace::ALL
                        .into_iter()
                        .find(|w| w.name().eq_ignore_ascii_case(&v))
                        .with_context(|| format!("--workspace {v}?"))?,
                );
            }
            "--input" => {
                c.input = Some(match val("--input")?.as_str() {
                    "mouse" => InputKind::Mouse,
                    "keyboard" => InputKind::Keyboard,
                    "xbox" => InputKind::Xbox,
                    "playstation" => InputKind::PlayStation,
                    "deck" => InputKind::Deck,
                    v => bail!("--input {v}?"),
                })
            }
            "--focus" => {
                let v = val("--focus")?;
                c.focus = Some(
                    Area::ALL
                        .into_iter()
                        .find(|a| a.name().to_ascii_lowercase().starts_with(&v))
                        .with_context(|| format!("--focus {v}?"))?,
                );
            }
            "--drawer" => c.drawer = true,
            "--find" => c.find = true,
            "--address" => c.address = true,
            "--version" => {}
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(c)
}

fn new_studio(cli: &Cli) -> Studio {
    let maps = cli.maps.clone().unwrap_or_else(ob_map::tools::map::store::maps_dir);
    Studio::new(maps, cli.folder.as_deref())
}

fn main() -> ExitCode {
    ob_core::console::attach();
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).try_init();
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{} studio (preview)", ob_core::build_info::version_line());
        return ExitCode::SUCCESS;
    }
    let cli = match parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            ob_core::console::fatal(&format!("{e:#}"));
            return ExitCode::from(2);
        }
    };
    let r = if cli.shot.is_some() { run_shot(&cli) } else { window(cli) };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            ob_core::console::fatal(&format!("overbright-studio: {e:#}"));
            ExitCode::FAILURE
        }
    }
}

fn run_shot(cli: &Cli) -> Result<()> {
    let out = cli.shot.clone().context("--shot needs a path")?;
    let size = cli.size.unwrap_or((1920, 1080));
    let scale = cli.scale.unwrap_or_else(|| layout::ui_scale(size.0, size.1));
    let mut s = new_studio(cli);
    if let Some(w) = cli.workspace {
        s.nav.workspace = w;
    }
    if let Some(i) = cli.input {
        s.nav.input = i;
    }
    if let Some(a) = cli.focus {
        s.nav.area = a;
    }
    s.nav.drawer_open = cli.drawer;
    if cli.find {
        s.nav.find_open = true;
    }
    if cli.address {
        s.command(ob_studio_gui::studio::nav::Command::EditAddress, None);
        s.nav.input = cli.input.unwrap_or(InputKind::Keyboard);
    }
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let l = shot::shot(&mut s, &gpu, size, scale, &out)?;
    let p = l.problems();
    if l.too_small {
        println!("studio layout: under the smallest window: one line");
    } else if p.is_empty() {
        println!("studio layout: fits ({:.0} x {:.0} points, scale {scale})", l.size.x, l.size.y);
    } else {
        println!("studio layout: clipped: {}", p.join("; "));
    }
    println!("wrote {} ({}x{})", out.display(), size.0, size.1);
    Ok(())
}

struct Win {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    egui: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

struct Runner {
    studio: Studio,
    win: Option<Win>,
    pads: Option<gilrs::Gilrs>,
    repaint_at: Option<Instant>,
    error: Option<anyhow::Error>,
}

/// A pad button as Studio's role (the same places on every pad).
fn pad_button(b: gilrs::Button) -> Option<Pad> {
    use gilrs::Button as G;
    Some(match b {
        G::South => Pad::A,
        G::East => Pad::B,
        G::West => Pad::X,
        G::North => Pad::Y,
        G::LeftTrigger => Pad::LB,
        G::RightTrigger => Pad::RB,
        G::LeftTrigger2 => Pad::LT,
        G::RightTrigger2 => Pad::RT,
        G::DPadUp => Pad::Up,
        G::DPadDown => Pad::Down,
        G::DPadLeft => Pad::Left,
        G::DPadRight => Pad::Right,
        G::Start => Pad::Start,
        G::Select => Pad::View,
        _ => return None,
    })
}

/// The glyph set a pad wants, from its name.
fn pad_kind(name: &str) -> InputKind {
    let n = name.to_ascii_lowercase();
    if n.contains("steam deck") {
        InputKind::Deck
    } else if n.contains("playstation") || n.contains("dualshock") || n.contains("dualsense") || n.contains("sony") {
        InputKind::PlayStation
    } else {
        InputKind::Xbox
    }
}

impl Runner {
    fn open_window(&mut self, el: &ActiveEventLoop) -> Result<Win> {
        let attrs = Window::default_attributes()
            .with_title("Overbright Studio (preview)")
            .with_window_icon(nfsu2::studio_api::platform::icon::window_icon())
            .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(layout::WINDOW_MIN.0, layout::WINDOW_MIN.1));
        let window = Arc::new(el.create_window(attrs)?);
        let instance = Gpu::new_instance();
        let surface = instance.create_surface(window.clone())?;
        let gpu = pollster::block_on(Gpu::new(Some(&surface), instance))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("the window cannot be drawn on this GPU")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some((format, space)) = nfsu2::studio_api::render::display::sdr_surface_format(&caps) {
            config.format = format;
            config.color_space = space;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);
        let max_tex = gpu.device.limits().max_texture_dimension_2d as usize;
        let egui = egui_winit::State::new(
            self.studio.ctx.clone(),
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
        Ok(Win { window, surface, config, gpu, egui, renderer })
    }

    fn redraw(&mut self) {
        let Some(w) = self.win.as_mut() else { return };
        // The UI scale by the window's size (Studio's rule) times the system's.
        let phys = w.window.inner_size();
        let sys = w.window.scale_factor() as f32;
        let ui = layout::ui_scale((phys.width as f32 / sys) as u32, (phys.height as f32 / sys) as u32);
        self.studio.ctx.set_zoom_factor(ui);
        let raw = w.egui.take_egui_input(&w.window);
        let mut full = self.studio.frame(raw);
        let platform = std::mem::take(&mut full.platform_output);
        w.egui.handle_platform_output(&w.window, platform);
        self.repaint_at =
            full.viewport_output.get(&egui::ViewportId::ROOT).and_then(|v| Instant::now().checked_add(v.repaint_delay));
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
        let bufs = shot::paint(&self.studio, &w.gpu, &mut w.renderer, &view, (w.config.width, w.config.height), full);
        w.gpu.queue.submit(bufs);
        w.window.pre_present_notify();
        w.gpu.queue.present(frame);
    }

    fn poll_pads(&mut self) -> bool {
        let Some(g) = self.pads.as_mut() else { return false };
        let mut any = false;
        while let Some(ev) = g.next_event() {
            if let gilrs::EventType::ButtonPressed(b, _) = ev.event
                && let Some(p) = pad_button(b)
            {
                let kind = pad_kind(g.gamepad(ev.id).name());
                self.studio.pad(p, kind);
                any = true;
            }
        }
        any
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
                el.exit();
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
                if self.studio.quit {
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
        let pads = self.poll_pads();
        let Some(w) = self.win.as_ref() else { return };
        let now = Instant::now();
        if pads {
            w.window.request_redraw();
        }
        // Pads are polled: wake at least ten times a second.
        let mut wake = now + std::time::Duration::from_millis(100);
        if let Some(t) = self.repaint_at {
            if t <= now {
                self.repaint_at = None;
                w.window.request_redraw();
            } else {
                wake = wake.min(t);
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

fn window(cli: Cli) -> Result<()> {
    let el = EventLoop::new()?;
    let studio = new_studio(&cli);
    let pads = gilrs::Gilrs::new().ok();
    let mut r = Runner { studio, win: None, pads, repaint_at: None, error: None };
    el.run_app(&mut r)?;
    match r.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
