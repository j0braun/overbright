//! `game --prepare` (docs/prepare.md): what a first drive would load and
//! make while the player waits, done ahead with no window — the launcher
//! offers it as a step with a progress bar, and the game never needs it.
//!
//! * the programs of the settings in force (`enh::warm::drive_programs`:
//!   every pipeline and shader module a drive's frames take that depends
//!   on the settings and the window's format alone; the 2-D renderer's
//!   for the menus and the HUD), into the driver's
//!   pipeline cache on disk where the backend has one (Vulkan; on DX12
//!   wgpu keeps none, and making them once still fills the driver's own
//!   shader cache);
//! * free roam's world: its track file parsed into the asset cache, its
//!   renderer's data (the texture arrays as uploaded) beside it, the
//!   renderer's own programs and the world's effect objects' made once;
//! * the first career race's region the same way, when it is another;
//! * the current profile's car (with a profile).
//!
//! Progress goes to stdout as `key=value` lines (`nfsu2::launcher::prepare`
//! reads them): `prepare=1`, then `step=<n>/<of> <what>` as each step
//! starts, `ok=1` at the end (`error=…`, `ok=0` and exit code 1
//! otherwise). The last step writes the stamp the probe reads
//! (`prepared.txt` in the cache folder: the build, the adapter and its
//! driver), after the cache writes are flushed: a run stopped half way
//! leaves whole cache files (they are renamed into place) and no stamp,
//! and the next run makes only what is missing.
use crate::*;

/// The steps, in order.
const STEPS: [&str; 5] = ["graphics device", "shader programs", "the city", "the first race's track", "saving"];

fn step(n: usize) {
    use std::io::Write;
    println!("step={}/{} {}", n + 1, STEPS.len(), STEPS[n]);
    let _ = std::io::stdout().flush();
}

/// The window's format the programs are made for: the one the game's
/// window last used on this machine (`window-format.txt` in the cache
/// folder, written when the window is made), else the first of the
/// window's preference order (what a surface offers nearly everywhere).
fn window_format() -> wgpu::TextureFormat {
    nfsu2::render::display::recorded_window_format().unwrap_or(nfsu2::render::display::SDR_FORMAT_ORDER[0])
}

pub(crate) fn run(game: &mut Game) -> Result<()> {
    println!("prepare=1");
    step(0);
    let gpu = Arc::new(pollster::block_on(Gpu::new(None, Gpu::new_instance()))?);
    let info = gpu.adapter.get_info();
    println!("adapter={} ({:?})", info.name, info.backend);
    println!("pipeline_cache={}", if gpu.has_pipeline_cache() { 1 } else { 0 });
    let format = window_format();
    let opts = nfsu2::render::RenderOptions::in_force();
    let samples = nfsu2::render::display::drive_msaa_samples(&game.settings.custom, &gpu, &opts);
    let mirror = game.settings.rearview && game.settings.players[0].camera <= 1;
    let set = nfsu2::render::enh::warm::ProgramSet::new(&opts, format, samples, mirror, game.settings.enh.photo_mode);
    let built = nfsu2::render::pipelines_built();

    step(1);
    // The programs on a thread of their own, the city beside them (as the
    // menus do it: `ProgramsAhead`, `Preload::world`).
    let programs = {
        let gpu = gpu.clone();
        std::thread::Builder::new()
            .name("programs".into())
            .spawn(move || nfsu2::render::enh::warm::drive_programs(&gpu, &set))
            .context("the programs thread")?
    };
    // The window's own programs (the menus' and the HUD's 2-D renderer).
    drop(UiRenderer::new(&gpu, format));
    let (caches, root) = (game.game_data.caches.clone(), game.game_data.root.clone());
    let world = |region: &str| -> Result<()> {
        let h =
            Preload::world(caches.clone(), root.clone(), region.into(), Some((gpu.clone(), format)), None, Some(set));
        let (world, _renderer) = h.join().map_err(|_| anyhow::anyhow!("loading {region} failed"))?;
        world.map(|_| ())
    };
    let car = game.profile.is_some().then(|| game.spawn_car_load(&game.car_key(0)));
    programs.join().map_err(|_| anyhow::anyhow!("making the programs failed"))?;
    gpu.save_pipeline_cache();

    step(2);
    world(EXPLORE_REGION)?;
    if let Some(car) = car {
        let _ = car.join();
    }

    step(3);
    let first = game
        .game_data
        .career
        .stage_events(&game.game_data.root, 1)
        .first()
        .and_then(|e| game.game_data.career.tracks.get(&e.races.first().map_or(e.track, |r| r.0)))
        .map(|t| t.region.clone());
    if let Some(region) = first.filter(|r| r != EXPLORE_REGION) {
        world(&region)?;
    }

    step(4);
    let index = gpu.queue.submit([]);
    let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
    gpu.save_pipeline_cache();
    nfsu2::cache::flush();
    nfsu2::render::display::write_prepared_stamp(&info, nfsu2::frontend::enh::graphics_table::program_key(&game.settings));
    println!("pipelines={}", nfsu2::render::pipelines_built() - built);
    println!("ok=1");
    Ok(())
}
