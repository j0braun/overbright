//! The front-end and HUD fixes of part 2 against SPEED2.EXE
//! (docs/native-fixes/N-FE-2.tsv; docs/formats.md "Native front-end fixes,
//! part 2"), on sandbox profiles (`test_isolation`).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
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

/// UI_OLISPConnect (0x4B97D0): 0x49DF20 never returns −1 / −2, and under
/// Wine offline EA Login follows at once: the first
/// tick after Online leaves for UI_OLEALogin, no error dialog.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn online_goes_on_to_ea_login_at_the_next_tick() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.act(Action::Online, MenuKind::Main).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::IspConnect(_))));
    game.menu_tick();
    assert!(matches!(game.mode, Mode::Menu(Menu::EaLogin(_, None))), "EA Login at the next tick");
}

/// 0x512620 (from 0x4B551B): stage-0 events name their own message (no
/// draw); an SUV event (+0x0D) uses ENGAGE_SUV_RANDOM_n (9) before any
/// sponsor or kind set; the number is one 0x43C1C0(n) + 1 draw on the
/// game's generator at the dialog's set-up.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn engage_message_sets_and_draw() {
    let mut game = career("ENGAGE");
    let events = &game.game_data.career.events;
    let stage0 = events.iter().find(|e| e.stage == 0).expect("a stage-0 event");
    assert_eq!(screens::message_set(stage0), None);
    let suv = events.iter().find(|e| e.stage != 0 && e.suv).expect("an SUV event");
    assert_eq!(screens::message_set(suv), Some(("SUV", 9)));
    let event = events
        .iter()
        .position(|e| e.stage != 0 && !e.suv && e.category == career::Category::Normal && e.kind == career::EventKind::Circuit)
        .expect("a circuit");
    assert_eq!(screens::message_set(&events[event]), Some(("CIRCUIT", 18)));
    // The draw: the same generator state gives the same number.
    let unlock = game.game_data.career.events[event].unlock;
    if !game.profile.as_ref().unwrap().open.contains(&unlock) {
        game.profile.as_mut().unwrap().open.push(unlock);
    }
    // On the shared generator (0x4B551B → 0x512620: 0x43C1C0 on 0x7EA624;
    // The clone's career generator is gone).
    let mut expect = game.rng.clone();
    let want = expect.below(18) + 1;
    game.career_world.marker = Some(MarkerTarget::Event(event));
    game.career_world.marker_trigger = Some(0);
    assert!(game.engage(), "the marker engages");
    assert!(matches!(&game.mode, Mode::Menu(Menu::Engage(e)) if e.message == want));
    assert_eq!(game.rng.0, expect.0, "one draw");
}

/// 0x50E960 on UI_ChooseUniquePart (0x5618C0): a rim's brand prints the
/// career brand table's name ("5ZIGEN" for 0x352D08D1) before the label.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn unique_rim_names_use_the_brand_table() {
    let game = career("RIMNAME");
    let car = game.profile.as_ref().unwrap().car.clone();
    let mut after = None;
    let mut seen = false;
    while let Some(p) = game.game_data.parts.find_after(&car, 0x1D, after) {
        after = Some(p);
        if game.game_data.parts.attribute(p, 0xEBB0_3E66) == Some(0x352D_08D1) {
            assert!(game.unique_part_name(0x1D, p.hash).starts_with("5ZIGEN "), "{}", game.unique_part_name(0x1D, p.hash));
            seen = true;
            break;
        }
    }
    assert!(seen, "a 5Zigen rim");
}

/// Carbon fibre's Convert Trunk (0x5664A0 at 0x566686, +0x1D5 set): the
/// Accept asks first, No focused, the browser dimmed — in a career with
/// the stock trunk not owned the purchase question (0x3175F2B0), else
/// "Do you want to convert your trunk to carbon fiber?" (0x2B950C36). No
/// fits nothing; Yes (0x51123592 → 0x566F12) fits the carbon trunk, and
/// the next Accept asks to revert (0xA645A29E).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn carbon_convert_asks_first() {
    let mut game = career("CARBQ");
    game.profile.as_mut().unwrap().cash = 1_000_000;
    let open = |game: &Game| {
        let mut m = game.parts_browser_in(Some(0x5E), 0, None);
        let Mode::Menu(Menu::Parts(b)) = &mut m else { panic!("carbon fibre") };
        let at = b.categories.iter().position(|c| c.slot == 0x60).expect("Convert Trunk");
        b.categories[at].locked = false;
        b.category = at;
        (m, at)
    };
    let (m, at) = open(&game);
    game.mode = m;
    let before = game.profile.as_ref().unwrap().installed.get(&0x60).copied();
    game.menu_key(Key::Accept).unwrap();
    let text = game.label(0x3175_F2B0, "?");
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _))
            if d.text == text && d.selected == 1
                && d.dim_under.is_some_and(|(p, _)| p == "UI_GenericParts_Browser")),
        "the purchase question, No focused"
    );
    assert_eq!(game.profile.as_ref().unwrap().installed.get(&0x60).copied(), before, "nothing fitted yet");
    // No: back on the strip, nothing fitted.
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.category == at && b.part.is_none()), "No: the strip");
    assert_eq!(game.profile.as_ref().unwrap().installed.get(&0x60).copied(), before, "No fits nothing");
    // Outside a career (or with the stock part owned) the plain question.
    game.profile.as_mut().unwrap().owned.insert((0x60, 0xC8D3_13AC));
    assert_eq!(game.carbon_question(0x60), 0x2B95_0C36);
    let (m, _) = open(&game);
    game.mode = m;
    game.menu_key(Key::Accept).unwrap();
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.selected = 0;
    }
    game.menu_key(Key::Accept).unwrap();
    let fitted = game.profile.as_ref().unwrap().installed.get(&0x60).copied();
    assert!(fitted.is_some_and(|h| h != 0xC8D3_13AC) && fitted != before, "Yes: the carbon trunk");
    assert_eq!(game.carbon_question(0x60), 0xA645_A29E, "then the revert question");
}

/// Carbon fibre's Convert Trunk leaves GarageMain's view where it was: no
/// path of 0x5664A0 for the question, its Yes or its No calls 0x4C2100
/// (the view's only way in besides the idle tour), and under Wine the
/// question's frames match the strip's frame outside the dialog (dimmed to
/// 0.40, correlation 0.99 to 1.00), Yes leaves the previous list's view on
/// (docs/car-materials.md, "The camera under the convert question").
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn carbon_convert_keeps_the_view() {
    let mut game = career("CARBV");
    game.profile.as_mut().unwrap().cash = 1_000_000;
    let car = game.profile.as_ref().unwrap().car.clone();
    let stock = (0x60, 0xC8D3_13AC);
    game.profile.as_mut().unwrap().owned.insert(stock);
    let mut m = game.parts_browser_in(Some(0x5E), 0, None);
    let Mode::Menu(Menu::Parts(b)) = &mut m else { panic!("carbon fibre") };
    let at = b.categories.iter().position(|c| c.slot == 0x60).expect("Convert Trunk");
    b.categories[at].locked = false;
    b.category = at;
    game.mode = m;
    let view = |game: &mut Game| {
        game.garage_request().expect("GarageMain under the browser");
        let (_, _, v) = game.garage_fe.view.as_ref().expect("a view");
        (v.camera, v.car_angle, v.steer)
    };
    let before = view(&mut game);
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Dialog(..))), "the convert question");
    assert_eq!(view(&mut game), before, "the question keeps the view ({car})");
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.selected = 0;
    }
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.category == at), "Yes: the strip");
    assert_eq!(view(&mut game), before, "Yes keeps the view");
}

/// Carbon fibre's Hood (0x5664A0 at 0x566786): with a vinyl of kind 0x14 on
/// a layer, opening the hoods first warns "Installing a carbon fiber hood
/// will cause any hood vinyls to be removed" (0xB3BBB11D; with a decal on
/// 0x61..0x68 the decals one, 0x9796DFB4), one OK, over the opened list;
/// without such a vinyl the list opens at once.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn carbon_hood_warns_about_hood_vinyls() {
    let mut game = career("CARBH");
    game.profile.as_mut().unwrap().cash = 1_000_000;
    let car = game.profile.as_ref().unwrap().car.clone();
    let vinyl = game.game_data.parts.parts_for(&car, 0x40).into_iter().find(|p| p.kind & 0x1F == 0x14).map(|p| p.hash);
    let vinyl = vinyl.expect("a vinyl of kind 0x14 for the car");
    // A hood to list (a stage-1 car has no carbon one unlocked yet).
    let hood = game.game_data.parts.parts_for(&car, 9).into_iter().find(|p| p.level != 0).expect("a hood").hash;
    let hood = game.browser_part(9, game.car_part(9, hood).unwrap());
    let open = |game: &mut Game| {
        let mut m = game.parts_browser_in(Some(0x5E), 0, None);
        let Mode::Menu(Menu::Parts(b)) = &mut m else { panic!("carbon fibre") };
        let at = b.categories.iter().position(|c| c.slot == 9).expect("Hood");
        b.categories[at].locked = false;
        if b.categories[at].parts.is_empty() {
            b.categories[at].parts.push(hood.clone());
        }
        b.category = at;
        game.mode = m;
        game.menu_key(Key::Accept).unwrap();
    };
    assert_eq!(game.carbon_hood_warning(), None);
    open(&mut game);
    assert!(!matches!(&game.mode, Mode::Menu(Menu::Dialog(..))), "no hood vinyl: no warning");
    game.profile.as_mut().unwrap().installed.insert(0x40, vinyl);
    assert_eq!(game.carbon_hood_warning(), Some(0xB3BB_B11D));
    open(&mut game);

    let text = game.label(0xB3BB_B11D, "?");
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o))
            if d.text == text && d.buttons.len() == 1
                && matches!(o.first(), Some(Mode::Menu(Menu::Parts(b))) if b.part.is_some())),
        "the warning over the opened list"
    );
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_some()), "OK: the hoods' list");
}

/// 0x5664A0 → 0x566D98 (the Select and the Accept block): in a career an
/// unowned part above level 0 asks "Would you like to purchase this
/// part?" (0x44B7167F, No focused, the browser dimmed); Yes (0x09F00F07)
/// buys and fits it and shows the categories again; the fitted part only says "already installed"
/// (0xDA15D7E5).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn body_shop_accept_asks_to_buy() {
    let mut game = career("BODYQ");
    game.profile.as_mut().unwrap().cash = 1_000_000;
    let Mode::Menu(Menu::Parts(b)) = game.parts_browser(0, None) else { panic!("the body shop") };
    let (category, part) = b
        .categories
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.locked && c.action == screens::CategoryAction::Parts)
        .find_map(|(k, c)| {
            c.parts
                .iter()
                .position(|x| !x.3 && !x.4 && game.car_part(c.slot, x.0).is_some_and(|p| p.level != 0))
                .map(|i| (k, i))
        })
        .expect("an unowned part above level 0");
    let (slot, hash) = (b.categories[category].slot, b.categories[category].parts[part].0);
    game.mode = game.parts_browser_in(None, category, Some(part));
    game.menu_key(Key::Accept).unwrap();
    let question = game.label(0x44B7_167F, "?");
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _))
            if d.text == question && d.selected == 1
                && d.dim_under.is_some_and(|(p, _)| p == "UI_GenericParts_Browser")),
        "the purchase question"
    );
    assert!(!game.profile.as_ref().unwrap().owned.contains(&(slot, hash)), "nothing bought yet");
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.selected = 0;
    }
    game.menu_key(Key::Accept).unwrap();
    let p = game.profile.as_ref().unwrap();
    assert!(p.owned.contains(&(slot, hash)) && p.installed.get(&slot) == Some(&hash), "Yes: bought and fitted");
    // 0x5665AA → 0x55EF90(0): the list closes, the categories again.
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_none() && b.category == category),
        "Yes leaves the list for the categories"
    );
    // Accept on it again: already installed.
    game.mode = game.parts_browser_in(None, category, Some(part));
    game.menu_key(Key::Accept).unwrap();
    let text = game.label(0xDA15_D7E5, "");
    assert!(matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text == text && d.buttons.len() == 1));
}
