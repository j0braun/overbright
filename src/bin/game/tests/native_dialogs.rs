//! The "over the limit" (0x5165E0) and "not enough bank" (0x563D10)
//! dialogs reached through the live key path, on a sandbox profile (each
//! test thread's own empty data folder, `test_isolation`; never the
//! player's saves). docs/formats.md "Native dialogs: My Cars limit and
//! Custom Gauges bank".
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A career on the Peugeot past the intro, all 20 My Cars records used.
fn full_garage() -> Game {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("LIMIT");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    for k in 0..career::MY_CARS {
        p.my_cars[k] =
            Some(career::MyCar { id: career::tuned_car_id(k), car: "PEUGOT".into(), ..Default::default() });
    }
    game.profile = Some(p);
    game.settings.save_prompts = false;
    game.settings.autosave = false;
    game
}

fn dialog_text(game: &Game) -> Option<String> {
    match &game.mode {
        Mode::Menu(Menu::Dialog(d, _)) => Some(d.text.clone()),
        _ => None,
    }
}

/// 0x4FC2E5 → 0x4FC308 (game mode 0x20): Customize's Accept with all 20
/// records used shows 0x91921A8D with OK, and OK (0x34DC1BEC) stays on the
/// car select; nothing is customized.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn customize_with_twenty_my_cars_shows_the_limit_dialog() {
    let mut game = full_garage();
    let limit = game.label(0x9192_1A8D, "You have reached the limit of tuned cars.");
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::Customize, MenuKind::Main).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::Customize));
    game.menu_key(Key::Accept).unwrap();
    assert_eq!(dialog_text(&game).as_deref(), Some(limit.as_str()), "the limit dialog");
    assert!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 1));
    assert!(game.save_flow.customize.is_none(), "nothing customized");
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::Customize), "back on the car select");
    // One record freed: Customize goes on.
    game.profile.as_mut().unwrap().my_cars[7] = None;
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))), "customizing");
}

/// 0x4FCAF0-0x4FCB0E (game mode 2, BUTTON1): Quick Race's Customize tests
/// for a free record only on a stock car (0x516360); a My Cars record is
/// customized itself even with all 20 used.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn quick_race_customize_tests_the_limit_only_for_a_stock_car() {
    let mut game = full_garage();
    let limit = game.label(0x9192_1A8D, "You have reached the limit of tuned cars.");
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    let select = game.quick_car_select(Some(e));
    let Mode::Menu(Menu::CarLot(lot)) = select else { panic!("the car select") };
    let stock = lot.cars.iter().position(|c| c.my_car.is_none() && c.preset.is_none()).expect("a stock car");
    let mine = lot.cars.iter().position(|c| c.my_car.is_some()).expect("a My Cars record listed");
    let at = |game: &mut Game, i: usize| {
        let mut l = lot.clone();
        l.selected = i;
        game.mode = Mode::Menu(Menu::CarLot(l));
    };
    at(&mut game, stock);
    game.menu_key(Key::Button1).unwrap();
    assert_eq!(dialog_text(&game).as_deref(), Some(limit.as_str()), "a stock car: the limit dialog");
    at(&mut game, mine);
    game.menu_key(Key::Button1).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))), "a My Cars record: customized");
}

/// 0x563D10 at 0x5641FB-0x564246: in a career, a custom gauge (HUDINDEX
/// ≠ 0) bought after the needle colour with the bank short of its price
/// shows GenericDialog_SMALL with 0xD1AC60BE and OK; nothing is bought,
/// and OK (0xB4623F67) puts the picker back on the needle colour. With
/// the bank equal to the price it is bought (`jge`).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn custom_gauge_short_of_bank_shows_the_bank_dialog() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("BANK");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    p.cash = 0;
    game.profile = Some(p);
    game.settings.autosave = false;
    let Mode::Menu(Menu::Parts(b)) = game.specialties_browser(0, None) else { panic!("the specialties browser") };
    let category = b.categories.iter().position(|c| c.slot == 0xA3).expect("the Custom Gauges category");
    let part = b.categories[category]
        .parts
        .iter()
        .position(|x| game.gauge_hud_index(x.0) != 0 && !x.3)
        .expect("a custom gauge not owned");
    let hash = b.categories[category].parts[part].0;
    let picker = |game: &mut Game| {
        let browser = game.specialties_browser(category, Some(part));
        let Mode::Menu(m) = browser else { unreachable!() };
        game.fe.hud_overlay = Some(Box::new(m));
        game.mode = game.choose_custom_hud(category, part);
        // Backing, dial, needle: the needle's Accept buys (0x9E382ECD).
        for _ in 0..3 {
            game.menu_key(Key::Accept).unwrap();
        }
    };
    picker(&mut game);
    let bank = game.label(0xD1AC_60BE, "You do not have enough bank to purchase this item.");
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o))
            if d.package == "GenericDialog_SMALL" && d.text == bank && d.buttons.len() == 1 && o.len() == 1),
        "the bank dialog"
    );
    let p = game.profile.as_ref().unwrap();
    assert!(p.cash == 0 && !p.owned.contains(&(0xA3, hash)), "nothing bought");
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::ChooseHud(c)) if c.step == 2), "back on the needle colour");
    // The bank equal to the price: bought.
    let Some(x) = game.profile.as_ref().and_then(|p| game.game_data.parts.find(&p.car, 0xA3, Some(hash), None)).copied() else {
        panic!("the gauge part")
    };
    let price = game.browser_part(0xA3, &x).2;
    assert!(price > 0, "a priced gauge");
    game.profile.as_mut().unwrap().cash = price as u32;
    picker(&mut game);
    // The bank equal to the price (`jge` at 0x56421F): the purchase
    // question 0x44B7167F (0x564248), No focused, the overlay dimmed.
    let question = game.label(0x44B7_167F, "?");
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o))
            if d.text == question && d.selected == 1 && d.under == Some(1)
                && d.dim_under.is_some_and(|(p, _)| p == "UI_CustomHUDOverlay")
                && matches!(&o[0], Mode::Menu(Menu::LotDeal(LotDeal::FitGauge(_))))),
        "the purchase question"
    );
    // No (0xB4623F67): back on the needle colour, nothing bought.
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::ChooseHud(c)) if c.step == 2), "No: back on the needle colour");
    assert!(!game.profile.as_ref().unwrap().owned.contains(&(0xA3, hash)), "No: nothing bought");
    // Yes (0x9F00F07 → 0x563DA4): bought and fitted.
    game.menu_key(Key::Accept).unwrap();
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.selected = 0;
    }
    game.menu_key(Key::Accept).unwrap();
    let p = game.profile.as_ref().unwrap();
    assert!(p.cash == 0 && p.owned.contains(&(0xA3, hash)), "bought");
    assert_eq!(p.installed.get(&0xA3), Some(&hash), "fitted");
}

/// 0x563D10 at 0x564159–0x564192: the overlay's step is 1 (0x56C220,
/// 0x56C283), so Accept on the stock gauge (HUDINDEX 0) returns at
/// 0x5644D9: no picker, no dialog, nothing fitted or bought.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_stock_gauge_accept_does_nothing() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("STOCKG");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.settings.autosave = false;
    let Mode::Menu(Menu::Parts(b)) = game.specialties_browser(0, None) else { panic!("the specialties browser") };
    let category = b.categories.iter().position(|c| c.slot == 0xA3).expect("the Custom Gauges category");
    let stock = b.categories[category].parts.iter().position(|x| game.gauge_hud_index(x.0) == 0).expect("the stock gauge");
    game.mode = game.specialties_browser(category, Some(stock));
    let before = (game.profile.as_ref().unwrap().installed.clone(), game.profile.as_ref().unwrap().cash);
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part == Some(stock)), "still on the list");
    let p = game.profile.as_ref().unwrap();
    assert_eq!((p.installed.clone(), p.cash), before, "nothing fitted or bought");
}
