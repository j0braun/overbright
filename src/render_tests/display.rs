//! Display tests that name the menus' option table (the top layer's tests).

use ob_render::render::display::*;

#[test]
fn step_mode_over_the_mode_list() {
    let list = mode_list(&[(3440, 1440), (1920, 1080), (720, 400), (5120, 1440)]);
    assert_eq!(crate::frontend::options::step_mode(&list, (1600, 1200), true), (1920, 1080));
    assert_eq!(crate::frontend::options::step_mode(&list, (5120, 1440), true), (5120, 1440));
}

#[test]
fn from_custom() {
    let top = crate::frontend::options::custom_presets(5);
    let d = Display::from_custom(&top);
    assert_eq!(
        d,
        Display { env_rate: 3, env_world: true, shadows: true, headlight: true, world_lod_b: false, fog: true }
    );
    let low = Display::from_custom(&crate::frontend::options::custom_presets(1));
    assert_eq!(
        low,
        Display { env_rate: 0, env_world: false, shadows: false, headlight: false, world_lod_b: true, fog: false }
    );
}
