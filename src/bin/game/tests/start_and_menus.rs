//! The start and the menus' waits (docs/start-and-menus.md): Skip Title
//! Screen's three values; the main menu's garage scene made while the
//! title is up draws the frames the menu made it for itself draws.
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

const SIZE: (u32, u32) = (320, 240);

/// One 1/60 s display frame.
fn tick(game: &mut Game) {
    game.advance(1.0 / 60.0).unwrap();
}

fn is_movie(mode: &Mode) -> bool {
    matches!(mode, Mode::Menu(Menu::Movie(_)))
}

fn is_title(mode: &Mode) -> bool {
    matches!(mode, Mode::Menu(Menu::Splash(_)))
}

/// Off: the movies, then the title (0x4C564E's hold). Title Screen: the
/// movies, then the profile step where the title stood. Movies and Title
/// Screen: the profile step on the start's first tick.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn skip_title_screen_leaves_out_what_it_names() {
    testkit::game();
    let mut native = Game::new().unwrap();
    assert_eq!(native.settings.enh.skip_title, 0, "native by default");
    native.mode = native.boot_screen(BootStep::Splash);
    assert!(is_title(&native.mode));
    native.mode = native.boot_screen(BootStep::Intro);
    assert!(matches!(&native.mode, Mode::Menu(Menu::Movie(m)) if m.player.is_some()), "the intro plays");

    let mut title = Game::new().unwrap();
    title.settings.enh.skip_title = 1;
    title.mode = title.boot_screen(BootStep::Intro);
    assert!(matches!(&title.mode, Mode::Menu(Menu::Movie(m)) if m.player.is_some()), "the movies still play");
    title.mode = title.boot_screen(BootStep::Splash);
    assert!(!is_title(&title.mode) && !is_movie(&title.mode), "the profile step where the title stood");

    let mut all = Game::new().unwrap();
    all.settings.enh.skip_title = 2;
    all.mode = all.boot_screen(BootStep::Blank);
    assert!(matches!(&all.mode, Mode::Menu(Menu::Movie(m)) if m.player.is_none()), "no movie opened");
    tick(&mut all);
    assert!(!is_title(&all.mode) && !is_movie(&all.mode), "the profile step on the first tick");
}

/// The frame just drawn, read back once its end is submitted (as
/// `tests::first_use`'s).
fn picture(game: &mut Game) -> Vec<u8> {
    let w = game.platform.window.as_mut().unwrap();
    w.settle();
    nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), SIZE.0, SIZE.1)
}

/// The main menu's garage scene read while the title is up (platform,
/// lights, FE car) is taken by the menu's first tick, and the menu's
/// frames are the same pictures as a menu that made its scene itself.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_main_menus_scene_made_during_the_title_draws_the_same() {
    testkit::game();
    // Alone: the front end's cube (`fe_cube_texels`) and the output size
    // are process-wide, and another test's garage scenes or window size
    // between the two games' first ticks would change their pictures.
    super::test_isolation::exclusive();
    let menu_frames = |ahead: bool| {
        let mut game = Game::new().unwrap();
        let mut p = Profile::new("SCENE_AHEAD_TEST");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        game.profile = Some(p);
        super::tick_effects::offscreen_window(&mut game, SIZE.0, SIZE.1);
        game.pacing.clock = Some(0.0);
        if ahead {
            game.mode = game.boot_screen(BootStep::Splash);
            assert!(game.loads.garage_ahead.is_some(), "made while the title is up");
            // The title's seconds: the scene is done before the menu.
            let t = std::time::Instant::now();
            while !game.loads.garage_ahead.as_ref().unwrap().is_done() {
                assert!(t.elapsed().as_secs() < 60, "the scene ahead never ended");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        game.mode = game.icon_menu(MenuKind::Main);
        let mut frames = Vec::new();
        for n in 0..6 {
            game.pacing.clock = Some(n as f64 / 60.0);
            game.step_for(1.0 / 60.0).unwrap();
            game.draw().unwrap();
            frames.push(picture(&mut game));
            if n == 0 {
                assert!(game.loads.garage_ahead.is_none(), "taken or dropped by the first tick");
                assert_eq!(game.loads.garage_ahead_taken, u32::from(ahead), "taken");
                let g = game.render.garage.as_ref().expect("the menu's scene");
                assert!(g.car.as_ref().is_some_and(|c| c.3.is_some()), "its FE car and renderer");
                assert!(g.renderer.is_some(), "its platform renderer");
            }
        }
        frames
    };
    let own = menu_frames(false);
    let ahead = menu_frames(true);
    for (n, (a, b)) in own.iter().zip(&ahead).enumerate() {
        assert!(a == b, "frame {n}: the pictures differ");
    }
    assert!(own.iter().any(|f| f.iter().any(|&v| v != 0)), "something drawn");
}
