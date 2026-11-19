//! docs/developing/frame-settings.md, "A tick's settings and a frame's": a setting
//! changed between a tick and a drawn frame is drawn with the value the
//! tick-stepped state was stepped with (the newest tick's), and from the
//! next tick on with the new one; the chase camera's Field Of View offset
//! is the tick's.
use super::*;

/// The rows the drive starts with, then the ones changed between a tick
/// and a draw: each tick-stepped setting the draw reads.
const BEFORE: &[(&str, &str)] = &[
    ("enh.motion_blur", "0"),
    ("enh.particles", "0"),
    ("enh.sparks_fire", "0"),
    ("enh.wet_interaction", "0"),
    ("enh.lens_drops", "0"),
    ("enh.fov", "0"),
];
const AFTER: &[(&str, &str)] = &[
    ("enh.motion_blur", "2"),
    ("enh.particles", "2"),
    ("enh.sparks_fire", "1"),
    ("enh.wet_interaction", "2"),
    ("enh.lens_drops", "1"),
    ("enh.fov", "10"),
];

fn apply(game: &mut Game, rows: &[(&str, &str)]) {
    for (k, v) in rows {
        assert!(game.settings.enh.parse(k, v), "{k} {v}");
    }
    game.settings.apply_enhancements();
}

/// Free roam, a few ticks; the rows changed after a tick: the frame drawn
/// before the next tick draws the tick-stepped state with the tick's
/// values (not the frame's), the next tick takes the new ones; the chase
/// camera keeps the tick's angle until then.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_setting_changed_between_a_tick_and_a_draw_is_drawn_from_the_next_tick() {
    super::testkit::game();
    super::test_isolation::exclusive();
    eprintln!(
        "FrameSettings {} B (RenderOptions {} B), TickSettings {} B",
        std::mem::size_of::<nfsu2::render::FrameSettings>(),
        std::mem::size_of::<nfsu2::render::RenderOptions>(),
        std::mem::size_of::<crate::sim::TickSettings>()
    );
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("SETTINGS_MOMENTS_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    apply(&mut game, BEFORE);
    game.explore(None).unwrap();
    for _ in 0..5 {
        game.advance(1.0 / 60.0).unwrap();
    }
    let tick = crate::render::frame::Stepped::of_tick(&game.frame_poses(1.0).stepped.expect("a captured tick"));
    let fov = game.sim.drive.as_ref().unwrap().camera.game_fov.unwrap();
    // Changed between the tick and the draw (as a menu's mouse or pad
    // input changes it after the tick took its settings).
    apply(&mut game, AFTER);
    let frame = crate::render::frame::Stepped::of_frame(&nfsu2::render::FrameSettings::new(
        &game.settings.custom,
        nfsu2::render::RenderOptions::in_force(),
    ));
    let drawn = crate::render::frame::Stepped::of_tick(&game.frame_poses(1.0).stepped.unwrap());
    assert_eq!(drawn, tick, "the draw shows the tick-stepped state with the tick's values");
    assert_ne!(drawn, frame, "the frame's settings hold the new values");
    for (what, t, f) in [
        ("motion blur", drawn.motion_blur, frame.motion_blur),
        ("particles", drawn.particles, frame.particles),
        ("sparks & fire", drawn.sparks_fire, frame.sparks_fire),
        ("wet road interaction", drawn.wet_interaction, frame.wet_interaction),
        ("lens drops", u8::from(drawn.lens_drops), u8::from(frame.lens_drops)),
    ] {
        assert_ne!(t, f, "{what}: the tick's value is drawn until the next tick");
    }
    assert_eq!(game.sim.drive.as_ref().unwrap().camera.game_fov.unwrap().to_bits(), fov.to_bits());
    // The next tick takes the new values, the draw with it.
    game.advance(1.0 / 60.0).unwrap();
    let next = crate::render::frame::Stepped::of_tick(&game.frame_poses(1.0).stepped.unwrap());
    assert_eq!(next, frame, "from the next tick on, the new values");
    let d = game.sim.drive.as_ref().unwrap();
    assert_eq!(d.fov_offset.to_bits(), nfsu2::qol::fov_offset().to_bits(), "the tick's Field Of View offset");
    assert!(d.camera.game_fov.unwrap() > fov, "the chase camera widened on the tick");
    apply(&mut game, BEFORE);
}

/// How a test tick changes an option.
#[derive(Clone, Copy, Debug)]
enum Input {
    Key,
    Pad,
    Mouse,
}

/// An option changed in a tick — by a key, a pad button or a mouse click
/// on the Graphics page's Effects group (the Particles row, one step
/// right) — is in force in that same tick: the tick's settings
/// (`Pacing::tick_settings`, what the simulation and the tick's effects
/// read) carry the new value after that one update, whichever input
/// changed it. The settings are taken after all of the tick's input.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn an_option_changed_by_any_input_is_in_force_in_its_tick() {
    use nfsu2::pad::host::Btn;
    super::testkit::game();
    super::test_isolation::exclusive();
    let mut game = Game::new().unwrap();
    game.pacing.clock = Some(100.0);
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
    let category = options::Category::GraphicsGroup(options::GraphicsGroup::Effects);
    let tick = |game: &mut Game| {
        game.pacing.clock = game.pacing.clock.map(|t| t + 1.0 / 60.0);
        game.update(1.0 / 60.0).unwrap();
    };
    for input in [Input::Key, Input::Pad, Input::Mouse] {
        apply(&mut game, &[("enh.particles", "0")]);
        let row = options::rows(category, &game.settings, 0, false)
            .iter()
            .position(|r| r.id == options::RowId::Particles)
            .expect("the Particles row");
        let mut page = options::OptionsScreen::new(category, &game.settings, false, 0.0);
        page.selected = row;
        game.mode = Mode::Menu(Menu::Options(page));
        // Settled: the pad attached, the screen up, nothing pressed.
        for _ in 0..30 {
            tick(&mut game);
        }
        assert_eq!(nfsu2::qol::particles(), 0, "{input:?}: before");
        assert_eq!(game.pacing.tick_settings.particles, 0, "{input:?}: before");
        let at = game.pacing.clocks.tick;
        match input {
            Input::Key => game.input_at(at, InputEvent::press(KeyCode::ArrowRight)),
            Input::Pad => game.platform.pads.devices[0].inputs.buttons[Btn::Right as usize] = true,
            Input::Mouse => {
                game.mouse_record();
                let r = nfsu2::frontend::mouse::slider_rows()
                    .into_iter()
                    .find(|r| r.0 == row)
                    .unwrap_or_else(|| panic!("the Particles row laid out: {:?}", nfsu2::frontend::mouse::slider_rows()));
                let frame = game.ui_frame().unwrap();
                let c = r.4.0 + r.4.1 * 0.5;
                let p = Vec2::new(
                    frame.origin.0 + (c.x + 320.0) * frame.scale.0,
                    frame.origin.1 + (c.y + 240.0) * frame.scale.1,
                );
                game.input_at(at, InputEvent::CursorMoved(p));
                game.input_at(at, InputEvent::Button(true));
            }
        }
        // The one tick the input belongs to.
        tick(&mut game);
        let now = nfsu2::qol::particles();
        assert_ne!(now, 0, "{input:?}: the input changed the Particles row");
        assert_eq!(game.pacing.tick_settings.particles, now, "{input:?}: the change is in force in its own tick");
        // Let go.
        let at = game.pacing.clocks.tick;
        match input {
            Input::Key => game.input_at(at, InputEvent::KeyUp(KeyCode::ArrowRight)),
            Input::Pad => game.platform.pads.devices[0].inputs.buttons[Btn::Right as usize] = false,
            Input::Mouse => game.input_at(at, InputEvent::Button(false)),
        }
        tick(&mut game);
    }
    apply(&mut game, BEFORE);
}
