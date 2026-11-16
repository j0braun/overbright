use crate::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Real FrontB and magazine records; no GPU or user profile writes.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn cover_uses_selected_environment_light_hash() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("COVER_LIGHT_TEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(profile);
    let mut magazines = nfsu2::formats::magazines::Magazines::load(&game.game_data.root).unwrap();
    let envs = magazines.envmaps.clone();
    let milestone = game.game_data.career.milestones.iter().position(|m| m.kind == 3).unwrap();
    let raw = nfsu2::game_fs::read(&game.game_data.root, "FRONTEND/FrontB.lzc").unwrap();
    let setups = nfsu2::render::lighting::parse_light_setups(&nfsu2::formats::jdlz::decompress(&raw).unwrap());
    let fields = |rig: &[nfsu2::render::lighting::RigLight; 3]| {
        rig.map(|a| (a.camera_relative, a.theta, a.phi, a.colour, a.intensity))
    };
    for env in envs {
        for missing_cube in [false, true] {
            for setup in &mut magazines.setups {
                setup.envmaps = vec![env.hash];
                setup.counts[5] = 1;
            }
            let record = magazines.envmaps.iter_mut().find(|e| e.hash == env.hash).unwrap();
            record.name = if missing_cube { "__missing_test_cube__".into() } else { env.name.clone() };
            game.game_data.magazines = Some(magazines.clone());
            game.cover_view(milestone, false, 0.0);
            let shot = game.showcase.cover_car.as_ref().unwrap();
            assert_eq!(shot.envmap.is_none(), missing_cube);
            assert_eq!(
                fields(&shot.rig),
                fields(setups.get(&env.light).unwrap()),
                "env {:08X} light {:08X}",
                env.hash,
                env.light
            );
        }
    }
    // Unknown light and absent environment are separate fallback cases.
    for absent_environment in [false, true] {
        let hash = magazines.envmaps[0].hash;
        magazines.envmaps[0].light = 0;
        for setup in &mut magazines.setups {
            setup.envmaps = vec![if absent_environment { 0 } else { hash }];
            setup.counts[5] = 1;
        }
        game.game_data.magazines = Some(magazines.clone());
        game.cover_view(milestone, false, 0.0);
        assert_eq!(fields(&game.showcase.cover_car.as_ref().unwrap().rig), fields(&nfsu2::render::lighting::DEFAULT_RIG));
    }
}

/// The performance shop's purchases and the save fields they write,
/// against what the original wrote after the same purchases under Wine
/// (`docs/perf-shop.md`, evidence `docs/evidence/perf-shop/`): an
/// unbranded part bought first appends no brand (slot 0x8389B4 still
/// 0), a part's brand screen picks slot 0, a package's each branded
/// part's slot in row order; a later unbranded part appends
/// 0xFFFFFFFF; owned bits +0x15C, spend +0x7F0 (+0x294 in the save,
/// full prices), the brand list +0x2A0; after the brand screen the
/// shop is back on Choose Individual Parts' first row, after a direct
/// buy the row stays; Back on the brand screen does nothing.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn perf_shop_individual_parts_packages_and_brands() {
    use nfsu2::formats::native_save::{EXTRA_BRANDS, EXTRA_PERF_OWNED, EXTRA_SPENT, SLOT_EXTRA};
    testkit::game();
    let bytes = &testkit::private("tests/fixtures/native_saves/career_end_won.sav");
    let save = nfsu2::formats::native_save::NativeSave::parse(bytes).unwrap();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    let p = Profile::from_native(&save, ctx);
    assert_eq!(p.car, "PEUGOT");
    let cash0 = p.cash;
    game.profile = Some(p);
    game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == "CC_PERF_SHOP").map(|s| s.hash);
    let keys = |game: &mut Game, keys: &[Key]| {
        for &k in keys {
            game.menu_key(k).unwrap();
        }
    };
    let list = |game: &Game| game.shop.perf_parts.as_ref().map(|l| (l.selected, l.row().map(|r| r.id))).unwrap();
    let on_parts =
        |game: &Game| matches!(&game.mode, Mode::Menu(Menu::Icons(m, MenuKind::PerfPackage)) if m.selected == 0);
    // Engine: Larger Diameter Downpipe (row 7, id 21, no brands) first.
    game.act(Action::PerfCategory(3), MenuKind::PerfCategory).unwrap();
    assert!(on_parts(&game));
    keys(&mut game, &[Key::Down; 6]);
    assert_eq!(list(&game), (6, Some(21)));
    keys(&mut game, &[Key::Accept, Key::Left, Key::Accept]);
    assert!(on_parts(&game) && list(&game) == (6, Some(21)), "a direct buy keeps the row");
    // Replace Headers (row 2, id 17): its brand screen, Back stays,
    // Select with the first brand (HP Racing).
    keys(&mut game, &[Key::Up; 5]);
    assert_eq!(list(&game), (1, Some(17)));
    keys(&mut game, &[Key::Accept, Key::Left, Key::Accept]);
    assert!(matches!(game.mode, Mode::Menu(Menu::BrandSelect(..))));
    keys(&mut game, &[Key::Back]);
    assert!(matches!(game.mode, Mode::Menu(Menu::BrandSelect(..))), "Back does nothing");
    keys(&mut game, &[Key::Accept]);
    assert!(on_parts(&game) && list(&game).0 == 0, "back on the first row");
    // Performance Exhaust (row 4, id 15): AEM → HP Racing → Jackson.
    keys(&mut game, &[Key::Down, Key::Down, Key::Down]);
    assert_eq!(list(&game), (3, Some(15)));
    keys(&mut game, &[Key::Accept, Key::Left, Key::Accept, Key::Right, Key::Right, Key::Accept]);
    // The Pro package (19, 20, 21 owned): Cat Back AEM → HP Racing,
    // High Flow Skunk2 → Jackson → DC Sports.
    keys(&mut game, &[Key::Right, Key::Right, Key::Accept, Key::Left, Key::Accept]);
    let Mode::Menu(Menu::BrandSelect(b, _)) = &game.mode else { panic!("no brand screen for the package") };
    assert_eq!(b.rows.len(), 2);
    keys(&mut game, &[Key::Right, Key::Down, Key::Right, Key::Right, Key::Accept]);
    assert!(on_parts(&game) && list(&game).0 == 0);
    // Transmission: Differential (row 3, id 8, no brands).
    game.act(Action::PerfCategory(1), MenuKind::PerfCategory).unwrap();
    keys(&mut game, &[Key::Down, Key::Down]);
    assert_eq!(list(&game), (2, Some(8)));
    keys(&mut game, &[Key::Accept, Key::Left, Key::Accept]);
    assert_eq!(list(&game), (2, Some(8)));
    let p = game.profile.as_ref().unwrap();
    let owned: Vec<u8> = p.perf_owned.iter().copied().collect();
    assert_eq!(owned, vec![8, 15, 17, 19, 20, 21]);
    assert_eq!(p.brands, vec![0xD03F_A52B, 0xF09A_0188, 0xD03F_A52B, 0x0000_04C6, 0xFFFF_FFFF]);
    assert_eq!(p.spent, 500 + 500 + 500 + 1500 + 500);
    assert_eq!(cash0 - p.cash, 500 + 500 + 500 + 1350 + 500);
    assert!(
        [15u8, 17, 19, 20, 21, 8].iter().all(|id| p.perf.contains(id) || *id == 15),
        "fitted (the exhaust replaced by the cat back)"
    );
    // The save fields, as the original's writer lays them out.
    let out = p.to_native(&save, ctx);
    let e = out.slot(0) + SLOT_EXTRA;
    let bits: Vec<u8> =
        (0..0x80u8).filter(|&id| out.u8(e + EXTRA_PERF_OWNED + id as usize / 8) >> (id % 8) & 1 != 0).collect();
    assert_eq!(bits, owned);
    assert_eq!(out.u32(e + EXTRA_SPENT), 3500);
    let brands: Vec<u32> = (0..6).map(|k| out.u32(e + EXTRA_BRANDS + 4 * k)).collect();
    assert_eq!(brands, vec![0xD03F_A52B, 0xF09A_0188, 0xD03F_A52B, 0x0000_04C6, 0xFFFF_FFFF, 0]);
}

/// A performance purchase stores the ride into the car record as the
/// original does (`docs/perf-shop.md`): replayed on a
/// community save as the original resaved it without a purchase (LANCEREVO8,
/// slot 1, every part fitted, the level floats zero), the two Wine runs
/// give the career slot byte for byte as the original wrote it — the
/// ten level floats, the fitted parts with category 9 refitted (0x3A,
/// 0x3D, 0x40, 0x43), slot 0x0D (ENGINE STYLE02 with the level-2
/// engine, ECU and turbo packages on), 0x1B (BRAKE STYLE01), and the
/// wide body's door sill, hood under and decal slots — and the clone
/// loading the original's save rates the car as it did after buying.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn perf_purchases_store_the_ride_as_the_original() {
    use nfsu2::formats::native_save::{NativeSave, SLOT_LEN};
    testkit::game();
    let control = NativeSave::parse(&testkit::private("tests/fixtures/native_saves/community_perf_control.sav"))
    .unwrap();
    drop(Game::new().unwrap());
    let ctx = career::NativeContext::get().unwrap();
    let keys = |game: &mut Game, keys: &[Key]| {
        for &k in keys {
            game.menu_key(k).unwrap();
        }
    };
    let goto = |game: &mut Game, id: u8| {
        while game.shop.perf_parts.as_ref().is_some_and(|l| l.selected > 0) {
            game.menu_key(Key::Up).unwrap();
        }
        for _ in 0..16 {
            if game.shop.perf_parts.as_ref().and_then(|l| l.row().map(|r| r.id)) == Some(id) {
                return;
            }
            game.menu_key(Key::Down).unwrap();
        }
        panic!("no row for part {id}");
    };
    let category = |game: &mut Game, c: u8| game.act(Action::PerfCategory(c), MenuKind::PerfCategory).unwrap();
    // Return To Stock, Yes, then Left back to Choose Individual Parts
    // (Wine: Left ×4).
    let stock = |game: &mut Game| {
        game.mode = game.perf_confirm(PerfPending::Stock, false);
        for _ in 0..4 {
            game.menu_key(Key::Left).unwrap();
        }
        assert!(matches!(&game.mode, Mode::Menu(Menu::Icons(m, MenuKind::PerfPackage)) if m.selected == 0));
    };
    // A part: Accept, Yes; on its brand screen `brand` then Select.
    let buy = |game: &mut Game, id: u8, brand: &[Key]| {
        goto(game, id);
        keys(game, &[Key::Accept, Key::Left, Key::Accept]);
        if matches!(game.mode, Mode::Menu(Menu::BrandSelect(..))) {
            keys(game, brand);
            keys(game, &[Key::Accept]);
        }
    };
    let run = |steps: &dyn Fn(&mut Game)| {
        let mut game = Game::new().unwrap();
        let p = Profile::from_native(&control, ctx);
        assert_eq!(p.car, "LANCEREVO8");
        game.profile = Some(p);
        game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == "CC_PERF_SHOP").map(|s| s.hash);
        steps(&mut game);
        let p = game.profile.as_ref().unwrap();
        (p.to_native(&control, ctx), game.perf_ratings(&game.perf_fitted()), game.visual_rating())
    };
    let check = |name: &str, got: &NativeSave, want: &[u8]| {
        let want = NativeSave::parse(want).unwrap();
        let o = got.slot(1);
        let diff: Vec<String> = (o..o + SLOT_LEN)
            .filter(|&i| got.payload[i] != want.payload[i])
            .map(|i| format!("+{:#x}: {:02x} want {:02x}", i - o, got.payload[i], want.payload[i]))
            .collect();
        assert!(diff.is_empty(), "{name}: slot 1 differs at {diff:?}");
    };
    // Run 1: Engine back to stock, Replace Headers (HP Racing), the
    // Performance Exhaust (Jackson Racing), the Pro package (Cat Back
    // HP Racing, High Flow DC Sports).
    let (out, ratings, visual) = run(&|game| {
        category(game, 3);
        stock(game);
        buy(game, 17, &[]);
        buy(game, 15, &[Key::Right, Key::Right]);
        keys(game, &[Key::Right, Key::Right, Key::Accept, Key::Left, Key::Accept]);
        let Mode::Menu(Menu::BrandSelect(b, _)) = &game.mode else { panic!("no brand screen for the package") };
        assert_eq!(b.rows.len(), 2);
        keys(game, &[Key::Right, Key::Down, Key::Right, Key::Right, Key::Accept]);
    });
    let after =
        &testkit::private("tests/fixtures/native_saves/community_perf_after_buys.sav");
    check("after_buys", &out, after);
    // Run 2: Engine to stock, the downpipe (no brands), the Cat Back
    // (AEM); Transmission to stock, the Differential (no brands).
    let (out2, _, _) = run(&|game| {
        category(game, 3);
        stock(game);
        buy(game, 21, &[]);
        buy(game, 19, &[]);
        category(game, 1);
        stock(game);
        buy(game, 8, &[]);
    });
    check(
        "unbranded",
        &out2,
        &testkit::private("tests/fixtures/native_saves/community_perf_unbranded.sav"),
    );
    // Loading the original's save gives the state the purchases left:
    // the same shop ratings and visual rating.
    let mut game = Game::new().unwrap();
    game.profile = Some(Profile::from_native(&NativeSave::parse(after).unwrap(), ctx));
    let loaded = game.perf_ratings(&game.perf_fitted());
    assert_eq!(
        (loaded.acceleration, loaded.top_speed, loaded.handling),
        (ratings.acceleration, ratings.top_speed, ratings.handling)
    );
    assert_eq!(game.visual_rating(), visual);
}

/// Body-shop installs store the career ride as the original does
/// (`docs/shop-record-store.md`): 0x55C240's fit by 0x631D30 (which
/// runs 0x61BCE0) and 0x503950's copy of the ride into the record —
/// the derived wide-body / door / HOODUNDER slots, the performance
/// parts' 0x0D / 0x1B, and the upgrade state as the ride was built on
/// entering the shop. Each run is one Wine visit to CC_BODYSHOP_1 on
/// a community save (records with stale derived slots), saved by the
/// original on leaving: its car slot must come out byte for byte.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn body_shop_installs_store_the_ride_as_the_original() {
    use nfsu2::formats::native_save::{NativeSave, SLOT_LEN};
    testkit::game();
    drop(Game::new().unwrap());
    let ctx = career::NativeContext::get().unwrap();
    // (input, the original's save after the visit, career slot, car,
    // body-shop slot, part fitted).
    let runs: [(&[u8], &[u8], usize, &str, u8, u32); 4] = [
        // LANCEREVO8 (wide body kind 4, TRAP hood with HOODUNDER): the
        // ERASER hood (none): 0x19 the first part, the stale sill and
        // wide-body decals rewritten, ENGINE / BRAKE styles.
        (
            &testkit::private("tests/fixtures/native_saves/community_perf_control.sav"),
            &testkit::private("tests/fixtures/native_saves/community_body_hood.sav"),
            1,
            "LANCEREVO8",
            9,
            0x26EB_0659,
        ),
        // ESCALADE (no wide body): the front bumper back to stock (the
        // record keeps 0x3A, the ride's aero as built on entering).
        (
            &testkit::private("tests/fixtures/native_saves/community_body_escalade.sav"),
            &testkit::private("tests/fixtures/native_saves/community_body_bumper_stock.sav"),
            4,
            "ESCALADE",
            1,
            0xB369_824A,
        ),
        // Next visit, the ORION bumper (level 1): the ride rebuilt on
        // entry has 0x3A off and category 9 at 2.25.
        (
            &testkit::private("tests/fixtures/native_saves/community_body_bumper_stock.sav"),
            &testkit::private("tests/fixtures/native_saves/community_body_bumper_orion.sav"),
            4,
            "ESCALADE",
            1,
            0x2239_7DF1,
        ),
        // RX7 (wide body, HOODUNDER hood, every derived slot stale):
        // the JORGEN spoiler repairs them all.
        (
            &testkit::private("tests/fixtures/native_saves/community_body_rx7.sav"),
            &testkit::private("tests/fixtures/native_saves/community_body_rx7_spoiler.sav"),
            2,
            "RX7",
            0x0C,
            0x45F0_8571,
        ),
    ];
    for (input, want, slot, car, part_slot, hash) in runs {
        let input = NativeSave::parse(input).unwrap();
        let want = NativeSave::parse(want).unwrap();
        let mut game = Game::new().unwrap();
        let mut p = Profile::from_native(&input, ctx);
        assert_eq!((p.slot, p.car.as_str()), (slot, car));
        p.build_shop_ride();
        game.profile = Some(p);
        game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == "CC_BODYSHOP_1").map(|s| s.hash);
        let category = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == part_slot).unwrap();
        let Mode::Menu(Menu::Parts(b)) = game.parts_browser(category, None) else { panic!("no body shop") };
        let i = b.categories[b.category]
            .parts
            .iter()
            .position(|x| x.0 == hash)
            .unwrap_or_else(|| panic!("{car}: no part {hash:08X}"));
        game.mode = game.parts_browser(category, Some(i));
        game.menu_key(Key::Accept).unwrap();
        if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            game.menu_key(Key::Left).unwrap();
            game.menu_key(Key::Accept).unwrap();
        }
        let got = game.profile.as_ref().unwrap().to_native(&input, ctx);
        let o = got.slot(slot);
        let diff: Vec<String> = (o..o + SLOT_LEN)
            .filter(|&k| got.payload[k] != want.payload[k])
            .map(|k| format!("+{:#x}: {:02x} want {:02x}", k - o, got.payload[k], want.payload[k]))
            .collect();
        assert!(diff.is_empty(), "{car}: slot {slot} differs at {diff:?}");
        // The other records stay as loaded (stale ones too).
        for k in (0..5).filter(|&k| k != slot) {
            let o = got.slot(k);
            assert_eq!(got.payload[o..o + SLOT_LEN], input.payload[o..o + SLOT_LEN], "{car}: slot {k} changed");
        }
    }
    // Loading a record with stale derived slots and saving it without
    // a shop action keeps them, as the original's control resave of
    // the community save (0x503880 fixes the ride, not the record).
    let control = NativeSave::parse(&testkit::private("tests/fixtures/native_saves/community_perf_control.sav"))
    .unwrap();
    let resaved = Profile::from_native(&control, ctx).to_native(&control, ctx);
    for k in 0..5 {
        let o = control.slot(k);
        assert_eq!(resaved.payload[o..o + SLOT_LEN], control.payload[o..o + SLOT_LEN], "control slot {k}");
    }
    // The ride built from the stale record has the derived slots the
    // original's own store wrote (a purchase on the same car).
    let game = Game::new().unwrap();
    let stale = Profile::from_native(&control, ctx);
    let stored = Profile::from_native(
        &NativeSave::parse(&testkit::private("tests/fixtures/native_saves/community_perf_after_buys.sav"))
        .unwrap(),
        ctx,
    );
    let ride = nfsu2::car::with_derived_slots(&stale.installed, &game.game_data.parts, &stale.car);
    for slot in [0x11u8, 0x12, 0x13, 0x14, 0x15, 0x16, 0x19, 0x3B, 0x3C, 0x3D, 0x3E] {
        assert_eq!(ride.get(&slot), stored.installed.get(&slot), "ride slot {slot:#x}");
    }
}

/// docs/cheats.md: a bonus code typed on the title, the boot's "Would
/// you like to create a new profile?" declined, then a profile made
/// later (the Profile Manager's Create with no profile current, state
/// 2): it keeps the career in memory, the bank the code paid (Wine,
/// regmybank, No, then Customize's Back save point's create:
/// saved with 200, `tests/fixtures/native_saves/cheats_later_create.sav`).
/// docs/save-points.md: main-menu Customize's Dyno Yes (0x55E530 on
/// 0x34DC1BEC). A stock car customized gets a new My Cars record after
/// the snapshot, so the first Yes asks "Are you sure you want to save
/// the profile named …?" (mode 0x1141) and either answer goes on to the
/// dyno; the snapshot is gone after it, so the second Yes goes to the
/// dyno at once (both seen under Wine).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn customize_dyno_asks_to_save_only_when_changed() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("DYNO");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.settings.save_prompts = true;
    game.settings.autosave = false;
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::Customize, MenuKind::Main).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    let made = game.save_flow.customize_car.expect("a My Cars record");
    game.act(Action::Tuning, MenuKind::CustomizeCategory).unwrap();
    game.customize_dyno_yes().unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMessage(m, _)) if m.text.contains("save")), "the save point asks");
    assert!(
        game.profile.as_ref().is_some_and(|p| !p.scratch && p.my_cars[made.0].is_some()),
        "it shows the loaded profile with the record"
    );
    // No (focused): on to the dyno, the session's profile back.
    game.menu_key(Key::Accept).unwrap();
    assert!(game.profile.as_ref().is_some_and(|p| p.scratch));
    assert!(matches!(game.save_flow.customize, Some(Some(_))));
    assert!(game.dyno.tuning_session, "the dyno");
    game.dyno.tuning_session = false;
    game.mode = game.icon_menu(MenuKind::CustomizeCategory);
    game.act(Action::Tuning, MenuKind::CustomizeCategory).unwrap();
    game.customize_dyno_yes().unwrap();
    assert!(!matches!(game.mode, Mode::Menu(Menu::McMessage(..))), "unchanged: no question");
    assert!(game.dyno.tuning_session);
}

/// Wine, the finished-career save's Lancer: the dyno keeps the game's music rules
/// with the front end's pool, its Quit question holds the song, and
/// the garage after it swings its car in with FE sound 69 (FE_MB
/// 0x2E), as a fresh GarageMain's first car does (0x4C17DE → 0x4D82D0
/// state 1 → 2).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn dyno_music_and_the_garage_swing_after_it() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("SWING");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.settings.game_music = 1.0;
    game.settings.menu_music = 1.0;
    game.settings.master = 1.0;
    game.mode = game.icon_menu(MenuKind::Crib);
    assert_eq!(game.music_context(), MusicContext::FrontEnd);
    let _ = game.garage_request().expect("the garage");
    assert!(
        game.dev.fe_played.contains(&69),
        "a fresh garage swings its first car in (RIGHT on CARSEL): {:?}",
        game.dev.fe_played
    );
    let now = game.now() / 1000.0;
    assert!(game.garage_fe.view.as_ref().is_some_and(|v| v.2.changing(now)), "the platform turns");
    // The same car again: no second swing.
    game.dev.fe_played.clear();
    let _ = game.garage_request();
    assert!(!game.dev.fe_played.contains(&69));
    // The dyno: in game with the front end's pool.
    game.dyno.tuning_mode = 0;
    game.mode = game.enter_dyno();
    assert!(game.dyno.tuning_session && game.on_dyno_track());
    assert_eq!(game.music_context(), MusicContext::Dyno);
    game.settings.game_music = 0.0;
    assert_eq!(game.music_context(), MusicContext::Score, "game music 0: the score (0x482180)");
    game.settings.game_music = 1.0;
    // Back on the dyno menu: "Quit to Garage?", which holds the song.
    game.mode = game.icon_menu(MenuKind::Dyno);
    assert!(game.garage_request().is_none(), "no GarageMain over the dyno");
    game.menu_key(Key::Back).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))));
    assert_eq!(game.music_context(), MusicContext::Paused);
    game.quit_tuning();
    game.mode = game.after_tuning();
    game.dev.fe_played.clear();
    assert_eq!(game.music_context(), MusicContext::FrontEnd);
    let _ = game.garage_request().expect("the garage again");
    assert_eq!(
        game.dev.fe_played.iter().filter(|&&i| i == 69).count(),
        1,
        "the garage after the dyno swings its car in: {:?}",
        game.dev.fe_played
    );
}

/// Wine (fresh profile NAME, 0x8363F0 read under each screen): the
/// boot's main menu swings its first car in (0 → 120), UI_QRCarSelect's
/// first car (the 206, another type than the 350Z) swings again
/// (120 → 240) and the main menu after Back once more (240 → 0): every
/// FE car change of type turns the platform +120° (GarageMain +0x148
/// stays 0). The floor's tiles turn with the platform (disc i at
/// Rz(car) · T(disc) · Rz(platform)), so a wrong platform shows the
/// SHR_SUBDISK grout at another angle.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn quick_race_car_select_turns_the_platform_a_third() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.profile = None;
    game.pacing.clock = Some(10.0);
    let step = |game: &mut Game| {
        let _ = game.garage_request();
        game.pacing.clock = game.pacing.clock.map(|c| c + 1.0);
        let _ = game.garage_request();
        game.pacing.clock = game.pacing.clock.map(|c| c + 1.0);
        let _ = game.garage_request();
        let now = game.now() / 1000.0;
        let v = game.garage_fe.view.as_ref().expect("the garage view").2.at(now);
        (v.platform_angle.rem_euclid(360.0).round(), v.disc)
    };
    game.mode = game.icon_menu(MenuKind::Main);
    assert_eq!(step(&mut game), (120.0, 1), "the boot's first car swings in");
    game.act(Action::QuickRace, MenuKind::Main).unwrap();
    assert_eq!(step(&mut game), (120.0, 1), "the mode select keeps the car");
    game.act(Action::QrMode(screens::QrMode::Circuit), MenuKind::QrMode).unwrap();
    step(&mut game);
    game.menu_key(Key::Accept).unwrap();
    step(&mut game);
    game.menu_key(Key::Accept).unwrap();
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if matches!(l.state, LotState::QuickRace(Some(_)))),
        "the car select ({})",
        mode_name(&game.mode)
    );
    assert_eq!(step(&mut game), (240.0, 2), "the 206 swings in");
    for _ in 0..8 {
        if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
            break;
        }
        game.menu_key(Key::Back).unwrap();
        step(&mut game);
    }
    assert!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
        "back on the main menu ({})",
        mode_name(&game.mode)
    );
    assert_eq!(step(&mut game), (0.0, 0), "the 350Z swings back in");
}

/// L1 (Wine, a fresh copy with No Profile): Customize's Dyno Yes asks
/// "Would you like to create a new profile?"; No → "You cannot enter
/// Performance Tuning / Dyno without an active profile." and the
/// categories; Yes → the name → "save the profile named NAME?" → No →
/// the dyno with NAME current.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn customize_dyno_without_a_profile_asks_to_create_one() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.profile = None;
    game.settings.autosave = false;
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::Customize, MenuKind::Main).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    game.act(Action::Tuning, MenuKind::CustomizeCategory).unwrap();
    game.customize_dyno_yes().unwrap();
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask && m.selected == 1),
        "the create question, No focused"
    );
    game.menu_key(Key::Accept).unwrap();
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("without an active profile")),
        "no profile: the dyno is refused"
    );
    check_texts_shown(&game, "without an active profile").unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    assert!(!game.dyno.tuning_session);
    // Again, Yes: the name, then No to saving it.
    game.act(Action::Tuning, MenuKind::CustomizeCategory).unwrap();
    game.customize_dyno_yes().unwrap();
    game.menu_key(Key::Left).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Name));
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm));
    game.menu_key(Key::Accept).unwrap();
    assert!(game.dyno.tuning_session, "on to the dyno");
    assert!(matches!(&game.save_flow.customize, Some(Some(p)) if p.name == "NAME"), "NAME is the profile set aside");
    // The session's car (the memory's My Cars record 0) is NAME's.
    assert!(
        matches!(&game.save_flow.customize, Some(Some(p)) if p.my_cars[0].is_some() && p.menu_car_id == Some(nfsu2::frontend::career::tuned_car_id(0))),
        "the customized car kept"
    );
    assert!(game.profile.as_ref().is_some_and(|p| p.scratch), "the session's profile back");
}

/// L2 (Wine, a fresh copy with No Profile): the Options' Back after a
/// volume change asks "Would you like to create a new profile?" (No
/// focused); No → the main menu, no profile; Yes → the name → "save
/// the profile named NAME?" → No → the main menu with NAME current.
/// Unchanged, Back asks nothing.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn options_back_without_a_profile_asks_to_create_one() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.profile = None;
    game.settings.save_prompts = true;
    game.settings.autosave = false;
    let open_options = |game: &mut Game| {
        game.mode = game.icon_menu(MenuKind::Main);
        game.act(Action::Options, MenuKind::Main).unwrap();
    };
    open_options(&mut game);
    game.menu_key(Key::Back).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "unchanged: no question");
    open_options(&mut game);
    game.settings.master = (game.settings.master - 0.1).max(0.0);
    game.menu_key(Key::Back).unwrap();
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask && m.selected == 1),
        "the create question, No focused"
    );
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "No: the main menu");
    assert!(game.profile.is_none());
    open_options(&mut game);
    game.settings.master = (game.settings.master + 0.1).min(1.0);
    game.menu_key(Key::Back).unwrap();
    game.menu_key(Key::Left).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Name));
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm));
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "the main menu");
    assert_eq!(game.profile.as_ref().map(|p| p.name.as_str()), Some("NAME"));
}

/// L2 (Wine, No Profile): Quick Race → Peugeot 206 → Customize → a
/// front bumper → Back asks to create a profile; Yes → NAME → No to
/// saving: the car select on the tuned 206, NAME current with it.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn quick_race_customize_back_without_a_profile_keeps_the_car() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.profile = None;
    game.settings.save_prompts = true;
    game.settings.autosave = false;
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.mode = game.quick_car_select(Some(e));
    game.menu_key(Key::Button1).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::CustomizeCategory))));
    let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap();
    game.profile.as_mut().unwrap().installed.insert(0x17, 0x1234_5678);
    game.menu_key(Key::Back).unwrap();
    assert!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask && m.selected == 1),
        "the create question"
    );
    game.menu_key(Key::Left).unwrap();
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm));
    game.menu_key(Key::Accept).unwrap();
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { panic!("the car select") };
    let p = game.profile.as_ref().expect("NAME current");
    assert_eq!(p.name, "NAME");
    let rec = p.my_cars[0].as_ref().expect("the tuned car kept");
    assert_eq!(rec.car, car);
    assert_eq!(rec.installed.get(&0x17), Some(&0x1234_5678));
    assert_eq!(lot.cars[lot.selected].my_car, Some(0), "on the tuned car");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn profile_made_after_declining_the_boot_keeps_the_bonus_bank() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.profile = None;
    // regmybank's payment into the boot's career (0x5760B0), without
    // touching the shared code table other tests read.
    game.fe.boot_cash = nfsu2::frontend::cheats::pay_now(200, false, game.fe.boot_cash);
    game.mode = game.icon_menu(MenuKind::Profile);
    game.act(Action::ProfileCreate, MenuKind::Profile).unwrap();
    // As selftest_profiles: Yes to create, the name NAME, then No
    // (focused) to "Are you sure you want to save…" (the editor's first
    // Enter does nothing, 0x5512A9).
    game.menu_key(Key::Left).unwrap();
    for _ in 0..4 {
        game.menu_key(Key::Accept).unwrap();
    }
    assert_eq!(game.profile.as_ref().map(|p| p.cash), Some(200));
}
