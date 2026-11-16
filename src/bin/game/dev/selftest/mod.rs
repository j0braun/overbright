use crate::*;

pub(crate) mod audio_lines;
mod career_steps;
pub(crate) mod customize;
pub(crate) mod dday_paths;
pub(crate) mod drift;
pub(crate) mod free_roam;
pub(crate) mod garage;
pub(crate) mod menus;
pub(crate) mod profile_manager;
mod race_steps;
pub(crate) mod resume;
pub(crate) mod session;
pub(crate) mod showcase;

use career_steps::*;
use race_steps::*;

/// Headless run through the career flow on the game's own menus: new
/// career, car lot, explore, pause → garage, World Map race with a pause,
/// reward and post-race menu, the engage dialog at a marker, quitting to
/// the main menu through the Yes/No dialog, resuming, and a Quick Race
/// drag race in its own region.
pub(crate) fn selftest(game: &mut Game) -> Result<()> {
    boot_and_menus(game)?;
    new_career(game)?;
    start_scene(game)?;
    dday_script(game)?;
    gps_to_car_lot(game)?;
    audio_mixmap(game)?;
    audio_car_sounds(game)?;
    audio_road_and_traffic(game)?;
    stage0_races(game)?;
    car_lot(game)?;
    gps_to_garage(game)?;
    garage_first_visit(game)?;
    free_roam_checks(game)?;
    body_shop_and_rims(game)?;
    perf_shop(game)?;
    graphics_shop(game)?;
    garage_save_question(game)?;
    in_game_map_and_gps(game)?;
    sub_selftests(game)?;
    let race = world_map_race(game)?;
    race_results(game, &race)?;
    back_at_engage_place(game, &race)?;
    quit_and_resume(game)?;
    quick_race_drag(game)?;
    quick_race_circuit(game)?;
    let saved = free_run(game)?;
    tournament(game)?;
    particles(game)?;
    anyhow::ensure!(saved.races_entered > 0, "career races entered counted (0x541170) and saved");
    selftest_drift(game)?;
    selftest_music_after_race_pause(game)?;
    selftest_race_lines_end(game)?;
    selftest_dday_paths(game)?;
    selftest_menus13(game)?;
    selftest_resume_points(game)?;
    println!(
        "selftest passed: cash ${}, reputation {}, stage {}, wins {}",
        saved.cash,
        saved.reputation,
        saved.stage,
        saved.completed.len()
    );
    Ok(())
}

/// The icon menu on screen: its kind and its options' labels.
fn menu(g: &Game) -> Option<(MenuKind, Vec<String>)> {
    match &g.mode {
        Mode::Menu(Menu::Icons(m, k)) => Some((
            *k,
            m.options.iter().map(|o| g.game_data.assets.strings.get(o.label).unwrap_or("?").to_string()).collect::<Vec<_>>(),
        )),
        _ => None,
    }
}

/// Arriving at a spot from elsewhere: no marker trigger held yet.
fn put(g: &mut Game, at: Vec3) {
        // A teleport cuts a drive-out short.
        g.scenes.shop = None;
        if let Some(d) = g.sim.drive.as_mut() {
            d.state.position = at;
        }
        g.career_world.marker_trigger = None;
}

/// A shop's route trigger (its +0x3C name, kind 14), mid-box.
fn shop_at(g: &Game, name: &str) -> Result<(usize, Vec3)> {
        let shop = g.game_data.career.shops.iter().position(|s| s.name == name).with_context(|| format!("shop {name}"))?;
        let t = g.sim.route_triggers.find(g.game_data.career.shops[shop].trigger).with_context(|| format!("{name}'s trigger"))?;
        Ok((shop, ((t.min + t.max) * 0.5).extend(t.position.z)))
}

