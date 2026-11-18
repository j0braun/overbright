//! `game --probe` (the launcher's hardware check, docs/launcher.md): the
//! adapter the game would pick, the primary monitor and the Resolution
//! row's list, the display values a first start would choose, the version
//! — printed as `key=value` lines (`nfsu2::launcher::probe::Probe`). No
//! window is opened and no device is made: the adapter is asked for with
//! the game's own options (`Gpu::adapter_options`) but without a window
//! surface (the window's adapter is asked for with that surface; on a
//! machine with one GPU, or where the high-performance GPU can present,
//! that is the same adapter — inferred, not checked on a multi-GPU
//! laptop). Exits 0, or 1 without an adapter.

use nfsu2::launcher::probe::{Adapter, Probe};

/// The primary monitor: size, refresh (mHz), video-mode sizes, and
/// whether it has no exclusive full screen (a Wayland session, or modes
/// that cannot be listed safely).
type Found = (u32, u32, u32, Vec<(u32, u32)>, bool);

/// The monitors, from an event loop that opens no window.
struct Monitors {
    found: Option<Found>,
}

impl winit::application::ApplicationHandler for Monitors {
    fn resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if let Some(m) = el.primary_monitor().or_else(|| el.available_monitors().next()) {
            // Read through `platform::monitor`: a mode without every field
            // (Wine on a virtual X display) never stops the probe.
            let facts = nfsu2::platform::monitor::facts(&m);
            let (sizes, (w, h), listed) = (facts.modes, facts.size, facts.exclusive_modes);
            // As the game window decides it (`shell::window`).
            let wayland = {
                #[cfg(all(unix, not(target_os = "macos")))]
                {
                    use winit::platform::wayland::ActiveEventLoopExtWayland;
                    el.is_wayland()
                }
                #[cfg(not(all(unix, not(target_os = "macos"))))]
                {
                    false
                }
            };
            // Modes that cannot be listed safely: no exclusive full screen
            // either (the game's window goes borderless, as on Wayland).
            self.found = Some((w, h, m.refresh_rate_millihertz().unwrap_or(0), sizes, wayland || !listed));
        }
        el.exit();
    }

    fn window_event(
        &mut self,
        _: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        _: winit::event::WindowEvent,
    ) {
    }
}

fn monitors() -> Option<Found> {
    let el = winit::event_loop::EventLoop::new().ok()?;
    let mut m = Monitors { found: None };
    el.run_app(&mut m).ok()?;
    m.found
}

/// The probe; the process's exit code.
pub(crate) fn run() -> i32 {
    let mut p = Probe { version: nfsu2::build_info::version_line(), ..Default::default() };
    match monitors() {
        Some((w, h, mhz, sizes, wayland)) => {
            p.monitor = Some((w, h, mhz));
            let mut all = sizes.clone();
            all.push((w, h));
            p.modes = nfsu2::render::display::mode_list(&all);
            p.monitor_modes = sizes;
            p.exclusive = !wayland;
        }
        None => p.modes = nfsu2::render::display::mode_list(&[]),
    }
    p.settings_file = nfsu2::frontend::options::Settings::path().exists();
    let instance = nfsu2::render::Gpu::new_instance();
    match pollster::block_on(instance.request_adapter(&nfsu2::render::Gpu::adapter_options(None))) {
        Ok(adapter) => {
            let info = adapter.get_info();
            nfsu2::upscale_pack::set_adapter(&info);
            let vram = nfsu2::upscale_pack::vram_bytes();
            p.gpu_class = nfsu2::qol::classify_gpu(info.device_type, vram).name().into();
            // Prepared for this build, adapter and driver, and for the
            // settings' programs as options.txt has them; the stamp's key
            // too, for the launcher to compare after its Graphics row.
            let key = nfsu2::frontend::enh::graphics_table::program_key(&nfsu2::frontend::options::Settings::load_text());
            p.prepared = Some(nfsu2::render::display::is_prepared(&info, key));
            p.prepared_programs = nfsu2::render::display::prepared_programs(&info);
            p.adapter = Some(Adapter {
                name: info.name.clone(),
                backend: format!("{:?}", info.backend),
                kind: format!("{:?}", info.device_type),
                vendor: info.vendor,
                device: info.device,
                driver: format!("{} {}", info.driver, info.driver_info).trim().to_string(),
                vram,
            });
            // What the window's first start does without options.txt
            // (`create_window`: `first_run_display(card)` on the defaults).
            // They are read with a table of the player's SPEED2.EXE: without a
            // usable game folder they are left out (the launcher runs the
            // probe again with the chosen folder in NFSU2_ROOT), never a
            // panic.
            p.first_run = nfsu2::qol::kept_paths::find_game()
                .and_then(|root| nfsu2::exe_tables::init(&root).ok())
                .map(|_| nfsu2::launcher::probe::first_run((info.vendor, info.device)));
        }
        // Linux: what is missing and the packages to install, in the
        // player's words (`platform::vulkan`); the launcher shows it. wgpu's
        // own words go to stderr (the launcher's log).
        Err(e) if cfg!(target_os = "linux") => {
            eprintln!("no GPU adapter: {e}");
            p.error = Some(nfsu2::platform::vulkan::no_adapter_line())
        }
        Err(e) => p.error = Some(format!("no GPU adapter: {e}")),
    }
    print!("{}", p.to_text());
    if p.ok() { 0 } else { 1 }
}
