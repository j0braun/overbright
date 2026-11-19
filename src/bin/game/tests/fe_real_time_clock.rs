//! The front end's C++ timers on the real-time clock 0x8651A8, the
//! specialties' and neon lists' purchase questions, the custom gauge picker
//! hidden under its dialogs, the garage's sounds before the tick's.
//! docs/original/uncapped-fps.md ("Menu timers on the real-time clock", "Front end
//! out of the draw"), docs/formats.md, docs/native-fixes/FE-3.tsv.
use super::*;
use crate::InputEvent;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

fn career(name: &str) -> Game {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new(name);
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.settings.autosave = false;
    game.settings.save_prompts = false;
    game
}

/// One 1/60 s display frame (one RACE tick).
fn tick(game: &mut Game) {
    game.advance(1.0 / 60.0).unwrap();
}

/// UI_PC_LAN_ServerSelect (0x4FB60E / 0x4FB8AE): the search ends on the
/// first tick whose 0x8651A8 is past +0x900 = the open's 0x8651A8 + 20 000
/// (5 s of the real-time clock: 300 or 301 RACE ticks), not 6000 package
/// ms (305 ticks).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn lan_search_ends_on_the_real_clock() {
    testkit::game();
    let mut game = Game::new().unwrap();
    for _ in 0..7 {
        tick(&mut game);
    }
    // Opened inside a tick, as the key's handler does (the input phase).
    game.pacing.clocks.real_tick = game.pacing.clocks.real;
    let start = game.real_now();
    game.act(Action::Lan, MenuKind::Main).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::LanServers(_, true))));
    let until = game.fe.lan_search_until;
    assert_eq!(until, start + 20_000, "+0x900 = 0x8651A8 + 5 s");
    let t0 = game.pacing.clocks.tick;
    let mut prev = start;
    for _ in 0..400 {
        tick(&mut game);
        let ended = matches!(game.mode, Mode::Menu(Menu::LanServers(_, false)));
        let read = game.pacing.clocks.real_tick;
        assert_eq!(ended, read > until, "tick {}: 0x8651A8 {read}, +0x900 {until}", game.pacing.clocks.tick - t0);
        if ended {
            assert!(prev <= until);
            // The open read the clock of tick t0 itself: n − 1 ticks after it.
            let n = game.pacing.clocks.tick - t0 - 1;
            assert!((300..=301).contains(&n), "ended {n} ticks after the open's");
            return;
        }
        prev = read;
    }
    panic!("the search never ended");
}

/// UG_LS_Splash (0x4C55F8, 0x4C564E): "Press Enter" 16 000 ticks of
/// 0x8651A8 after the package's screen-up message (sent at 0: the open),
/// the intro again past 30 s (120 000 ticks), both on the real-time clock.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn splash_timers_count_the_real_clock() {
    testkit::game();
    let mut game = Game::new().unwrap();
    tick(&mut game);
    game.mode = game.boot_screen(BootStep::Splash);
    let Mode::Menu(Menu::Splash(s)) = &game.mode else { panic!("the splash") };
    let opened = s.opened_real as u64;
    let (mut ready_at, mut gone_at) = (None, None);
    for _ in 0..2000 {
        tick(&mut game);
        let read = game.pacing.clocks.real_tick;
        match &game.mode {
            Mode::Menu(Menu::Splash(s)) => {
                if ready_at.is_none() && s.ready.is_some() {
                    ready_at = Some(read);
                }
            }
            _ => {
                gone_at = Some(read);
                break;
            }
        }
    }
    let ready = ready_at.expect("the prompt");
    assert!(ready - opened > 16_000 && ready - opened <= 16_000 + 69, "prompt at {} ticks", ready - opened);
    let gone = gone_at.expect("the intro again");
    assert!(nfsu2::game_clock::real_over(gone, opened, 30.0), "intro at {}", gone - opened);
    assert!(gone - opened <= 120_000 + 69, "intro at {} ticks", gone - opened);
}

/// The specialties' lists (UI_IcePartsOverlay, 0x568D40 → 0x568E2F) ask
/// as the body shop does: the bank dialog when short, else the purchase
/// question (No focused) over the overlay dimmed, Yes buys and fits it
/// and returns to the categories, and the fitted part only says "already
/// installed".
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn specialties_and_neon_lists_ask_to_buy() {
    let mut game = career("ICEQ");
    game.profile.as_mut().unwrap().cash = 0;
    // A list part not owned, above level 0, in the specialties (not the
    // gauges, not a category with a screen of its own) or the neon.
    let pick = |game: &Game, m: Mode| -> Option<(usize, usize, u8, u32)> {
        let Mode::Menu(Menu::Parts(b)) = m else { return None };
        b.categories.iter().enumerate().find_map(|(k, c)| {
            if c.locked || c.action != screens::CategoryAction::Parts || c.slot == 0xA3 {
                return None;
            }
            c.parts
                .iter()
                .position(|x| !x.3 && !x.4 && game.car_part(c.slot, x.0).is_some_and(|p| p.level != 0))
                .map(|i| (k, i, c.slot, c.parts[i].0))
        })
    };
    for (overlay, open) in [
        ("UI_IcePartsOverlay", (|g: &Game, c, p| g.specialties_browser(c, p)) as fn(&Game, usize, Option<usize>) -> Mode),
        ("UI_NeonPartsOverlay", |g: &Game, c, p| g.neon_browser(c, p)),
    ] {
        let (category, part, slot, hash) = pick(&game, open(&game, 0, None)).expect("a part to buy");
        game.profile.as_mut().unwrap().cash = 0;
        game.mode = open(&game, category, Some(part));
        game.menu_key(Key::Accept).unwrap();
        let bank = game.label(0xD1AC_60BE, "");
        assert!(
            matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o))
                if d.text == bank && d.package == "GenericDialog_SMALL" && d.buttons.len() == 1 && o.len() == 1
                    && d.dim_under.is_some_and(|(p, _)| p == overlay)),
            "{overlay}: the bank dialog"
        );
        game.menu_key(Key::Accept).unwrap();
        assert!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part == Some(part)), "{overlay}: OK, the list");
        game.profile.as_mut().unwrap().cash = 10_000_000;
        game.menu_key(Key::Accept).unwrap();
        let want = if slot == 0x22 {
            if game.trunk_layout_level() == 0 { 0xF58C_0CD9 } else { 0x4FAF_B114 }
        } else {
            0x44B7_167F
        };
        let question = game.label(want, "?");
        assert!(
            matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o))
                if d.text == question && d.package == "GenericDialog_SMALL" && d.selected == 1
                    && d.dim_under.is_some_and(|(p, _)| p == overlay)
                    && matches!(&o[0], Mode::Menu(Menu::LotDeal(LotDeal::FitPart { .. })))),
            "{overlay}: the purchase question, No focused"
        );
        assert!(!game.profile.as_ref().unwrap().owned.contains(&(slot, hash)), "{overlay}: nothing bought yet");
        if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
            d.selected = 0;
        }
        game.dev.fe_played.clear();
        game.menu_key(Key::Accept).unwrap();
        let p = game.profile.as_ref().unwrap();
        assert!(p.owned.contains(&(slot, hash)) && p.installed.get(&slot) == Some(&hash), "{overlay}: Yes buys and fits");
        assert!(!game.dev.fe_played.is_empty(), "{overlay}: the install sound (0x521290)");
        // The overlay closes after Yes (+0x114 / +0x115 → 0x8ACAA8DF):
        // the categories again, on the category.
        assert!(
            matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_none() && b.category == category),
            "{overlay}: Yes leaves the list for the categories"
        );
        game.mode = open(&game, category, Some(part));
        game.menu_key(Key::Accept).unwrap();
        let text = game.label(0xDA15_D7E5, "");
        assert!(
            matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text == text && d.buttons.len() == 1),
            "{overlay}: already installed"
        );
    }
}

/// UI_ChooseCustomHUD at step 0 (the needle's Accept, 0x56455C: the
/// chooser's package colour 0): the picker is hidden under the purchase's
/// dialogs, only the overlay drawn (dimmed); No (0xB4623F67 → step 4)
/// shows it again.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_gauge_picker_hides_under_its_dialogs() {
    let mut game = career("HUDQ");
    game.profile.as_mut().unwrap().cash = 10_000_000;
    let Mode::Menu(Menu::Parts(b)) = game.specialties_browser(0, None) else { panic!("the specialties browser") };
    let category = b.categories.iter().position(|c| c.slot == 0xA3).expect("the Custom Gauges category");
    let part = b.categories[category]
        .parts
        .iter()
        .position(|x| game.gauge_hud_index(x.0) != 0 && !x.3)
        .expect("a custom gauge not owned");
    let Mode::Menu(m) = game.specialties_browser(category, Some(part)) else { unreachable!() };
    game.fe.hud_overlay = Some(Box::new(m));
    game.mode = game.choose_custom_hud(category, part);
    let Mode::Menu(Menu::ChooseHud(c)) = &game.mode else { unreachable!() };
    assert!(!c.hidden && !c.quads(&game.game_data.assets, game.now()).is_empty(), "the picker up");
    for _ in 0..3 {
        game.menu_key(Key::Accept).unwrap();
    }
    let Mode::Menu(Menu::Dialog(d, o)) = &game.mode else { panic!("the purchase question") };
    let Some(Mode::Menu(Menu::ChooseHud(under))) = d.under.and_then(|i| o.get(i)) else { panic!("the picker under") };
    assert!(under.hidden && under.quads(&game.game_data.assets, game.now()).is_empty(), "step 0: the picker hidden");
    // The overlay is still drawn under the dialog.
    let quads = game.menu_quads(game.now());
    assert!(!quads.is_empty());
    // No (focused): back on the needle colour, the picker shown again on
    // the tick.
    game.menu_key(Key::Accept).unwrap();
    tick(&mut game);
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::ChooseHud(c)) if c.step == 2 && !c.hidden),
        "No: the picker again on the needle colour"
    );
}

/// GarageMain's update (0x4D82D0, step 15 of 0x5811D0) runs before the
/// AudioManager (step 18): every FE sound the garage plays on a tick goes
/// out with that tick's sounds (`audio_frame`), none after them, at
/// 60 Hz and at 144 Hz.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_sounds_go_out_with_their_tick() {
    for hz in [60.0f32, 144.0] {
        let mut game = career("GSND");
        game.mode = game.parts_browser(0, None);
        let base = game.pacing.clocks.tick;
        for (t, k) in [(30, KeyCode::Enter), (90, KeyCode::ArrowRight), (150, KeyCode::ArrowRight), (240, KeyCode::Escape)] {
            game.input_at(base + t, InputEvent::press(k));
            game.input_at(base + t + 1, InputEvent::KeyUp(k));
        }
        let mut heard_ticks = 0;
        for _ in 0..(400.0 * hz / 60.0) as usize {
            let before = game.dev.fe_played.len();
            game.advance(1.0 / hz).unwrap();
            if game.pacing.frame_updates == 0 {
                continue;
            }
            assert_eq!(
                game.dev.fe_heard.last().copied(),
                Some(game.dev.fe_played.len()),
                "{hz} Hz, tick {}: an FE sound after the tick's sounds",
                game.pacing.clocks.tick - base
            );
            heard_ticks += usize::from(game.dev.fe_played.len() > before);
        }
        assert!(heard_ticks > 0, "{hz} Hz: the garage played sounds");
    }
}
