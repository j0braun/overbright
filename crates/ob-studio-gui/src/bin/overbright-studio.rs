//! `overbright-studio`: Overbright Studio's window (src/studio/, docs/modding/studio.md), a preview: the shell
//! with Home, and the map editor as its World and Races workspaces (the same editor `overbright-editor` shows in
//! its own window). The game folder is `NFSU2_ROOT`, else the one the launcher kept.
//!
//! ```text
//! overbright-studio [--mod <name|folder>] [--maps <dir>]  the window, with that mod open (its folder name in the
//!                                                        mods folder, or its folder's path)
//! overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
//!                   [--workspace home|world|races|mod|settings|changes] [--input mouse|keyboard|xbox|playstation|deck]
//!                   [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--files] [--warning] [--find] [--address]
//!                   [--select N] [--new-mod NAME]
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
use ob_studio_gui::editor::app::App;
use ob_studio_gui::studio::nav::{Area, Pad};
use ob_studio_gui::studio::workspace::Workspace;
use ob_studio_gui::studio::{Studio, layout, shot};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const USAGE: &str = "usage: overbright-studio [--mod <folder>] [--maps <dir>]\n       overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>] [--workspace home|world|races|mod|settings|changes] [--input mouse|keyboard|xbox|playstation|deck] [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--files] [--warning] [--find] [--address] [--select N] [--new-mod NAME] [--tutorial 1-6] [--race-tutorial 1-7]";

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
    /// The Mod page: its Files fold open (`--files`), the warning before exporting textures from the original
    /// game's pixels up (`--warning`).
    files: bool,
    warning: bool,
    find: bool,
    address: bool,
    /// The placement selected in World (a `--shot`).
    select: Option<usize>,
    /// The New mod dialog open with this name typed (a `--shot`).
    new_mod: Option<String>,
    /// The first tutorial ("Your first mod") at this step (1 … 5, 6: the last card) on the open mod (a `--shot`).
    tutorial: Option<usize>,
    /// The race tutorial at this step (1 … 6, 7: the last card) on the open mod (a `--shot`).
    race_tutorial: Option<usize>,
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
                        .chain(Workspace::PAGES)
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
            "--files" => c.files = true,
            "--warning" => c.warning = true,
            "--find" => c.find = true,
            "--address" => c.address = true,
            "--select" => c.select = Some(val("--select")?.parse()?),
            "--new-mod" => c.new_mod = Some(val("--new-mod")?),
            "--race-tutorial" => {
                let n: usize = val("--race-tutorial")?.parse()?;
                if !(1..=7).contains(&n) {
                    bail!("--race-tutorial: 1 … 7");
                }
                c.race_tutorial = Some(n);
            }
            "--tutorial" => {
                let n: usize = val("--tutorial")?.parse()?;
                if !(1..=6).contains(&n) {
                    bail!("--tutorial: 1 … 6");
                }
                c.tutorial = Some(n);
            }
            "--version" => {}
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(c)
}

fn game_root() -> PathBuf {
    nfsu2::studio_api::qol::setup::game_root().unwrap_or_else(|| PathBuf::from("."))
}

/// Studio on the mods folder, with the map editor hosted on the same egui context. `--mod` takes a mod's folder
/// name or the path of its folder; one that is neither, or does not read, is an error, not a window with no mod.
fn new_studio(cli: &Cli) -> Result<Studio> {
    let mut maps = cli.maps.clone().unwrap_or_else(ob_map::tools::map::store::maps_dir);
    let mut folder = None;
    if let Some(arg) = cli.folder.as_deref() {
        let (m, f) = ob_studio_gui::studio::home::find_mod(&maps, arg).map_err(anyhow::Error::msg)?;
        maps = m;
        folder = Some(f);
    }
    let mut s = Studio::new(maps.clone(), folder.as_deref());
    if let Some(f) = &folder
        && s.open.is_none()
    {
        let why = ob_studio_gui::studio::home::summary(&maps, f).err().map_or_else(String::new, |e| format!(": {e:#}"));
        bail!("the mod {f} in {} does not read{why}", maps.display());
    }
    let editor = App::new_in(
        s.ctx.clone(),
        game_root(),
        maps,
        Box::new(|root, region| nfsu2::studio_api::world::World::load(root, region)),
    );
    s.attach_editor(editor);
    Ok(s)
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
    let mut s = new_studio(cli)?;
    if let Some(w) = cli.workspace {
        s.nav.workspace = w;
        if w == Workspace::Changes {
            let folder = s.open_mod().map(|m| m.folder.clone());
            s.changes.ensure(folder.as_deref());
        }
        if w == Workspace::Mod {
            let folder = s.open_mod().map(|m| m.folder.clone());
            s.mod_page.load(folder.as_deref());
            s.mod_page.files_open = cli.files;
            s.mod_page.confirm = cli.warning;
        }
    }
    if let Some(n) = cli.race_tutorial {
        use ob_studio_gui::studio::guide::{Guide, Tutorial};
        let ws = s.nav.workspace;
        s.start_tutorial(Tutorial::FirstRace);
        let folder = s.open_mod().map(|m| m.folder.clone());
        // The steps' own buttons up to step n (in Races: the race made, its start and points placed as they do).
        if let Some(g) = s.guide.as_mut() {
            g.made = folder;
        }
        s.nav.workspace = ws;
        if ws == Workspace::Races {
            for k in 2..n.min(5) {
                if let Some(g) = s.guide.as_mut() {
                    g.step = Guide::step_at(Tutorial::FirstRace, k);
                }
                s.guide_main();
                shot::frames(&mut s, size, scale, 3);
            }
        }
        if let Some(g) = s.guide.as_mut() {
            g.step = Guide::step_at(Tutorial::FirstRace, n);
        }
    }
    if let Some(n) = cli.tutorial {
        use ob_studio_gui::studio::guide::{Guide, Step, Tutorial};
        let ws = s.nav.workspace;
        s.start_tutorial(Tutorial::FirstMod);
        let folder = s.open_mod().map(|m| m.folder.clone());
        if let Some(g) = s.guide.as_mut() {
            g.made = folder;
            g.step = Guide::step_at(Tutorial::FirstMod, n);
        }
        s.nav.workspace = ws;
        // Past Go to a street in World: Take me there, as the step's button does.
        if n >= 3 && ws == Workspace::World {
            if let Some(g) = s.guide.as_mut() {
                g.step = Step::GoToStreet;
            }
            s.guide_main();
            shot::frames(&mut s, size, scale, 2);
            if let Some(g) = s.guide.as_mut() {
                g.step = Guide::step_at(Tutorial::FirstMod, n);
            }
        }
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
    if let Some(i) = cli.select {
        // One frame so World opens the mod, then the selection.
        shot::frames(&mut s, size, scale, 1);
        if let Some(st) = s.editor.as_mut().and_then(|h| h.app.state.as_mut()) {
            st.selection = vec![i];
        }
    }
    if let Some(name) = &cli.new_mod {
        s.open_new_mod();
        if let Some(d) = s.new_mod.as_mut() {
            d.name = name.clone();
        }
    }
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let l = shot::shot(&mut s, &gpu, size, scale, &out)?;
    // The editor's history closed as a window closes (its lock released).
    s.close_requested();
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
        let bufs = shot::paint(&mut self.studio, &w.gpu, &mut w.renderer, &view, (w.config.width, w.config.height), full);
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
                // The map editor saves first (and asks only when that fails).
                if self.studio.close_requested() {
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
        // The map editor's history: saving at quiet points, steps from other programs.
        if pads || self.studio.tick(now) {
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
    let studio = new_studio(&cli)?;
    let pads = gilrs::Gilrs::new().ok();
    let mut r = Runner { studio, win: None, pads, repaint_at: None, error: None };
    el.run_app(&mut r)?;
    match r.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
