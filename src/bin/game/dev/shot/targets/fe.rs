//! `--shot` targets: The front end's menus: main, career and quick race menus, memory card, boot, loading, online and dialogs.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("main", main_menu),
    Target::name("career", career),
    Target::name("profile", profile),
    Target::name("customize", customize),
    Target::name("qrmode", qrmode),
    Target::name("online", online),
    Target::name("ealogin", ealogin),
    Target::name("ealogin_connect", ealogin_connect),
    Target::name("ealogin_lost", ealogin_lost),
    Target::name("ealogin_leave", ealogin_leave),
    Target::name("ealogin_cancel", ealogin_cancel),
    Target::name("lan", lan),
    Target::name("lan_list", lan_list),
    Target::name("controls", controls),
    Target::name("controls_capture", controls_capture),
    Target::name("controls_pad", controls_pad),
    Target::name("controls_header", controls_header),
    Target::name("jukebox", jukebox),
    Target::name("boot_loading", boot_loading),
    Target::name("loading", loading),
    Target::name("loading_late", loading),
    Target::name("credits", credits),
    Target::name("boot_ealogo", boot_ealogo),
    Target::name("boot_thx", boot_ealogo),
    Target::name("boot_psa", boot_ealogo),
    Target::name("boot_intro", boot_ealogo),
    Target::name("splash", splash),
    Target::name("movie", movie),
    Target::name("mcmain", mcmain),
    Target::name("mcmain_replace", mcmain),
    Target::name("mcmain_name", mcmain),
    Target::name("mcmain_confirm", mcmain),
    Target::name("quit", quit),
    Target::name("mc_main", mc_main),
    Target::name("quitgame", quitgame),
    Target::name("mc_boot", mc_boot),
    Target::name("mc_list", mc_list),
    Target::name("profile_load", profile_load),
    Target::name("mc_save", mc_save),
    Target::name("mc_done", mc_done),
    Target::name("mc_name", mc_name),
    Target::name("quickrace", quickrace),
    Target::name("qropts", qropts),
    Target::name("qrcars", qrcars),
    Target::name("qrsponsor", qrsponsor),
    Target::name("custcars", custcars),
    Target::name("dialog", dialog),
    Target::name("qrtrans", qrtrans),
    Target::name("dialog_sized", dialog_sized),
];

pub(super) fn main_menu(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Main))
}

pub(super) fn career(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Career))
}

pub(super) fn profile(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Profile))
}

pub(super) fn customize(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::CustomizeCategory))
}

pub(super) fn qrmode(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::QrMode))
}

pub(super) fn online(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::IspConnect(0.0)))
}

/// UI_OLEALogin after UI_OLISPConnect (with a network), its connect
/// dialog, and the lost-connection notice it ends in.
pub(super) fn ealogin(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::EaLogin(screens::EaLogin::new(0.0), None)))
}

pub(super) fn ealogin_connect(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::EaLogin(screens::EaLogin::new(0.0), Some((0x1448_49C1, 0.0)))))
}

pub(super) fn ealogin_lost(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::EaLogin(
        screens::EaLogin::new(0.0),
        Some((0x1448_49C1, game.now() - screens::EaLogin::CONNECT_MS)),
    ));
    game.menu_tick();
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn ealogin_leave(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::EaLogin(screens::EaLogin::new(0.0), None));
    let _ = game.menu_key(Key::Back);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn ealogin_cancel(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::EaLogin(screens::EaLogin::new(0.0), Some((0x1448_49C1, game.now()))));
    let _ = game.menu_key(Key::Accept);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn lan(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::LanServers(4000.0, true)))
}

pub(super) fn lan_list(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::LanServers(0.0, false)))
}

pub(super) fn controls(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Controls(screens::ControlsScreen::new(0.0))))
}

/// The first row's primary cell awaiting a key (Wine menus113 o6).
pub(super) fn controls_capture(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Controls(screens::ControlsScreen { capturing: true, ..screens::ControlsScreen::new(0.0) })))
}

/// A controller's page (with `NFSU2_PAD_SCRIPT` attaching one) and the
/// header selected.
pub(super) fn controls_pad(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Controls(screens::ControlsScreen { device: 1, ..screens::ControlsScreen::new(0.0) })))
}

pub(super) fn controls_header(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Controls(screens::ControlsScreen { header: true, ..screens::ControlsScreen::new(0.0) })))
}

pub(super) fn jukebox(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Jukebox(screens::Jukebox::new(0.0))))
}

/// PC_Loading as a track load opens it (`loading`: the controls
/// list at the start; `loading_late`: 2 s on, "LOADING" and dots).
/// The boot variant: the game start's PC_Loading as `run` puts it up (over the
/// boot's first screen), 2 s after it opened ("LOADING" and its
/// dots), drawn with the same quads `present_loading` draws.
pub(super) fn boot_loading(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { env, .. } = cx;
    let at: f32 = env.ms.unwrap_or(5000.0);
    game.loads.loading = Some(BootLoading::new(at - 2000.0).loading);
    Ok(game.boot_screen(BootStep::Blank))
}

pub(super) fn loading(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, env, .. } = cx;
    let at: f32 = env.ms.unwrap_or(5000.0);
    let opened = if screen == "loading" { at } else { at - 2000.0 };
    game.loads.loading = Some(screens::Loading { package: screens::Loading::PC, opened });
    Ok(game.icon_menu(MenuKind::Main))
}

pub(super) fn credits(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::Credits(screens::Credits::load(&game.game_data.root, 0.0))))
}

/// The boot screens: `boot_<step>` a movie some seconds in, `splash`
/// with its prompt up.
pub(super) fn boot_ealogo(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let (step, at) = match screen {
        "boot_ealogo" => (BootStep::EaLogo, 3.0),
        "boot_thx" => (BootStep::Thx, 2.0),
        "boot_psa" => (BootStep::Psa, 12.0),
        _ => (BootStep::Intro, 30.0),
    };
    // BOOT_AT=<s>: another moment of the movie.
    let at = std::env::var("BOOT_AT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(at);
    let mut mode = game.boot_screen(step);
    if let Mode::Menu(Menu::Movie(m)) = &mut mode
        && let Some(p) = m.player.as_mut()
    {
        p.advance(at);
    }
    Ok(mode)
}

pub(super) fn splash(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let now = game.now();
    let mut s = screens::Splash::new(now, game.real_now(), false);
    s.ready = Some(now);
    s.age(3000.0);
    s.opened -= 5000.0;
    Ok(Mode::Menu(Menu::Splash(s)))
}

/// The career intro 83.5 s in (frame 2087, twelve after a key frame).
pub(super) fn movie(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mut m = MovieScreen::new(&game.game_data.root, 6, &["scene01"]);
    if let Some(p) = m.player.as_mut() {
        p.advance(83.5);
    }
    Ok(Mode::Menu(Menu::Movie(m)))
}

/// MC_Main for Start New Career: `mcmain` the first Yes/No,
/// `mcmain_name` the editor, `mcmain_confirm` the save question.
pub(super) fn mcmain(_game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let mut m = McMain::new(screen == "mcmain_replace", 0.0);
    if screen == "mcmain_name" || screen == "mcmain_confirm" {
        m.key(Key::Left, 0.0);
        m.key(Key::Accept, 0.0);
    }
    if screen == "mcmain_confirm" {
        m.key(Key::Accept, 0.0);
        m.key(Key::Accept, 0.0);
    }
    Ok(Mode::Menu(Menu::McMain(m)))
}

pub(super) fn quit(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.act(Action::MainMenu, MenuKind::Crib)?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn mc_main(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::McMain(McMain::new(false, 0.0))))
}

/// 'Q' on UI_Main: "Are you sure you want to quit?" (0x558B80).
pub(super) fn quitgame(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Main);
    game.garage_request();
    game.key(winit::keyboard::KeyCode::KeyQ, &mut || {})?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The boot's create question over MC_Background (no profile yet).
pub(super) fn mc_boot(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.fe.boot_profiles = Some(nfsu2::frontend::profiles::Boot::None);
    Ok(game.boot_profile_step())
}

pub(super) fn mc_list(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::McList(McList::new(McOp::Load, vec!["CLONE".into(), "NAME".into()], 0.0))))
}

/// The Profile Manager's Load: MC_List over the dimmed Profile
/// Manager (0x4A3100 mode 0xC11; Wine menus13 profile_load_list).
pub(super) fn profile_load(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Profile);
    game.act(Action::ProfileLoad, MenuKind::Profile)?;
    anyhow::ensure!(game.fe.mc_under.is_some(), "the Profile Manager under the list");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn mc_save(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.mc_save_question("NAME", MenuKind::Profile))
}

pub(super) fn mc_done(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.mc_tell(&game.mc_text(nfsu2::frontend::profiles::mc_text::LOAD_DONE, ""), McAct::To(MenuKind::Main)))
}

pub(super) fn mc_name(_game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mut m = McMain::new(false, 0.0);
    m.key(Key::Left, 0.0);
    m.key(Key::Accept, 0.0);
    "Ray".chars().for_each(|c| m.char(c));
    Ok(Mode::Menu(Menu::McMain(m)))
}

/// Circuit's track list, as capture.py reaches it in the original.
pub(super) fn quickrace(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(Mode::Menu(Menu::QuickRace(game.quick_track_select(screens::QrMode::Circuit, 0.0).context("no circuit track")?)))
}

/// The Game Options on the first circuit, as capture.py reaches it.
pub(super) fn qropts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    Ok(game.quick_options_screen(screens::QrMode::Circuit, 0.0))
}

pub(super) fn qrcars(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    // Reached from the main menu (Wine): its car stood on the
    // platform first (the boot's swing, 0x8363F0 0 → 120), so the
    // car select's car of another type swings in once more
    // (120 → 240) and the floor's tiles stand as natively. The main
    // menu's car is asked a second before the shot's clock.
    // No profile (the original's fresh one has no car): the main
    // menu's D-Day 350Z.
    game.fe.help_name = game.profile.take().map(|p| p.name);
    let clock = game.pacing.clock;
    game.pacing.clock = Some(clock.unwrap_or_else(|| game.seconds() as f64) - 1.0);
    game.mode = game.icon_menu(MenuKind::Main);
    let _ = game.garage_request();
    game.pacing.clock = clock;
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    Ok(match game.quick_car_select(Some(e)) {
        Mode::Menu(Menu::CarLot(mut lot)) => {
            lot.opened = 0.0;
            // NFSU2_SHOT_QRCAR=240SX: that car type on the platform
            // as the list opens (no swing), for still comparisons
            // with the original's car select (docs/car-shading-compare.md).
            // A stock car type the no-profile list does not hold
            // (the original's profile had it unlocked, e.g. the finished-career save's
            // TIBURON) is put at the list's end.
            if let Ok(want) = std::env::var("NFSU2_SHOT_QRCAR") {
                if !lot.cars.iter().any(|c| c.name.eq_ignore_ascii_case(&want))
                    && let Some(t) = game.game_data.car_types.iter().find(|t| t.stock_car && t.name.eq_ignore_ascii_case(&want))
                {
                    lot.cars.push(CarChoice::new(&game.game_data.assets, &t.name, None));
                }
                lot.selected = lot
                    .cars
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(&want))
                    .with_context(|| format!("NFSU2_SHOT_QRCAR: no car {want}"))?;
            }
            Mode::Menu(Menu::CarLot(lot))
        }
        m => m,
    })
}

/// The car select on the SPONSOR CARS filter with every code typed
/// at the title (`NFSU2_CHEATS`, default all): the first sponsor car,
/// or NFSU2_SHOT_KEYS to step along.
pub(super) fn qrsponsor(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if std::env::var("NFSU2_CHEATS").is_err() {
        type_cheats("all");
    }
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.fe.help_name = game.profile.take().map(|p| p.name);
    game.quick.filter = 8;
    Ok(match game.quick_car_select(Some(e)) {
        Mode::Menu(Menu::CarLot(mut lot)) => {
            lot.opened = 0.0;
            Mode::Menu(Menu::CarLot(lot))
        }
        m => m,
    })
}

/// Main-menu Customize's car select (UI_QRCarSelect in game mode 0x20).
pub(super) fn custcars(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Main);
    let _ = game.act(Action::Customize, MenuKind::Main);
    if let Mode::Menu(Menu::CarLot(lot)) = &mut game.mode {
        lot.opened = 0.0;
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn dialog(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.dialog("Welcome to Bayview.^Your Peugeot 206 is ready.", Mode::Explore))
}

/// Quick race's Transmission question over its car select (the
/// Ask for Transmission path, 0x4FC2E5).
pub(super) fn qrtrans(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.fe.help_name = game.profile.take().map(|p| p.name);
    game.mode = game.quick_car_select(Some(e));
    if let Mode::Menu(Menu::CarLot(lot)) = &mut game.mode {
        lot.opened = 0.0;
    }
    // The garage up under the car select, then its Accept (the
    // real path: the question over the dimmed select, 0x4FC2E5).
    game.settings.players[0].ask_transmission = true;
    game.garage_request();
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "the Transmission question");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The car lot's swap question (0x1231C38F with two car names, no
/// package named at 0x4DFEC6): sized by 0x526C50.
pub(super) fn dialog_sized(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    // `dialog_sized`: the longest of these no-name questions.
    let q = [
        0xD76E_0B5A,
        0x22BC_94AF,
        0xF356_8B86,
        0xA232_3032,
        0x5D33_6F96,
        0xEC94_88BB,
        0xF0A6_E6B5,
        0x6347_7036,
        0x3000_3D27,
    ]
    .into_iter()
    .max_by_key(|&h| game.label(h, "").chars().count())
    .unwrap();
    let m = game.ask(q, Mode::Explore, Mode::Explore);
    if let Mode::Menu(Menu::Dialog(d, _)) = &m {
        println!("{} characters: {}", d.text.chars().count(), d.package);
    }
    Ok(m)
}
