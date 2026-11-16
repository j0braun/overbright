use crate::*;

/// The finished-career save (tests/fixtures/native_saves/career_complete.sav:
/// the original's save, stage 6, every shop entered) as the current
/// profile, in memory only.
pub(crate) fn load_finished_career(game: &mut Game) -> Result<()> {
    let save = nfsu2::formats::native_save::NativeSave::parse(&crate::dev::private::require(
        crate::dev::private::FINISHED_CAREER,
        "the finished career (career_complete.sav)",
    )?)?;
    let ctx = career::NativeContext::get().context("the native context")?;
    let profile = Profile::from_native(&save, ctx);
    // The profile's own options too, as a real load applies them (the
    // finished-career save is metric: km/h and km under Wine).
    game.settings.apply_native(&save);
    profiles::set_current(&crate::dev::private::profile_name(&save), save);
    game.profile = Some(profile);
    Ok(())
}

/// Checks the shops' part previews and engage dialogs on the current
/// profile (`--shot finished_career_shop_checks`): in every open list of the body
/// shop, the specialties (UI_IcePartsOverlay), the neon, a rim brand's and
/// the spinners' style strips, the paints, the vinyls and a decal slot, a
/// step fits the highlighted item on the preview ride (0x55C180 / 0x55C200
/// / 0x55C380 / 0x569680) and the lists ask for their slot's view
/// (0x4C2100); entering a paint slot keeps the car's own paint (0x55CC60);
/// each shop's dialog asks the generic question once entered (0x512920).
pub(crate) fn shop_checks(game: &mut Game) -> Result<()> {
    fn ride(game: &Game) -> std::collections::BTreeMap<u8, u32> {
        game.profile.as_ref().map(|p| game.preview_ride(&p.installed)).unwrap_or_default()
    }
    // A step in the current list: the ride's slot follows the highlighted
    // item and differs from before.
    fn step(game: &mut Game, what: &str, key: Key, camera: bool) -> Result<()> {
        // The garage builds the list's neighbours ahead
        // (`GarageRequest::prefetch`); a step in a parts list must ask for
        // one of them.
        let ahead = game.garage_request().map(|r| r.prefetch).unwrap_or_default();
        let list = matches!(&game.mode, Mode::Menu(Menu::Parts(_))) && matches!(key, Key::Up | Key::Down);
        let before = ride(game);
        game.menu_key(key)?;
        let asked = game.garage_request().map(|r| r.installed);
        if list && !ahead.is_empty() {
            anyhow::ensure!(
                asked.as_ref().is_some_and(|a| ahead.contains(a)),
                "{what}: the step asked for a ride the garage did not build ahead"
            );
        }
        let item = game.shop_preview_item().with_context(|| format!("{what}: no highlighted item"))?;
        let after = ride(game);
        anyhow::ensure!(
            after.get(&item.0).copied() == Some(item.1).filter(|&h| h != 0)
                || (item.1 == 0 && !after.contains_key(&item.0)),
            "{what}: the ride lacks the item {item:x?}"
        );
        anyhow::ensure!(after != before, "{what}: the step left the ride as it was");
        if camera {
            anyhow::ensure!(
                matches!(game.garage_view_request(), Some(GarageViewRequest::Slot(..) | GarageViewRequest::Wheel(_))),
                "{what}: no slot view"
            );
        }
        println!(
            "shop check: {what}: slot {:#04x} part {:#010x}{}",
            item.0,
            item.1,
            if list && !ahead.is_empty() { " (built ahead)" } else { "" }
        );
        Ok(())
    }
    // Down a list (Up at its end).
    fn list_step(game: &mut Game, what: &str, camera: bool) -> Result<()> {
        let key = match &game.mode {
            Mode::Menu(Menu::Parts(b)) => match (b.part, b.categories.get(b.category)) {
                (Some(i), Some(c)) if i + 1 >= c.parts.len() => Key::Up,
                _ => Key::Down,
            },
            _ => Key::Down,
        };
        step(game, what, key, camera)
    }
    let label = |g: &Game, h: u32| g.game_data.assets.strings.get(h).unwrap_or("?").to_string();
    let shop_of = |g: &Game, name: &str| g.game_data.career.shops.iter().find(|s| s.name == name).map(|s| s.hash);
    let mut checked = 0;
    // The body shop's lists.
    game.shop.visit = shop_of(game, "IND_BODYSHOP_2");
    let Mode::Menu(Menu::Parts(b)) = game.parts_browser(0, None) else { anyhow::bail!("the body shop") };
    for (c, cat) in b.categories.iter().enumerate() {
        if cat.locked || cat.action != screens::CategoryAction::Parts || cat.parts.len() < 2 {
            continue;
        }
        game.mode = game.parts_browser(c, None);
        game.menu_key(Key::Accept)?;
        list_step(game, &format!("body shop {}", label(game, cat.label)), true)?;
        checked += 1;
    }
    // Carbon fibre's lists.
    let Mode::Menu(Menu::Parts(b)) = game.parts_browser_in(Some(0x5E), 0, None) else { anyhow::bail!("carbon fibre") };
    for (c, cat) in b.categories.iter().enumerate() {
        if cat.locked || cat.action != screens::CategoryAction::Parts || cat.parts.len() < 2 {
            continue;
        }
        game.mode = game.parts_browser_in(Some(0x5E), c, None);
        game.menu_key(Key::Accept)?;
        list_step(game, &format!("carbon {}", label(game, cat.label)), true)?;
        checked += 1;
    }
    // A rim brand's style strip and a style's sizes; the spinners' strip.
    let brand = (0..game.rim_brands().len())
        .find(|&i| matches!(game.rims_browser(i, None, None), Mode::Menu(Menu::Parts(ref b)) if b.categories.len() > 1))
        .context("a rim brand with two styles")?;
    game.mode = game.rims_browser(brand, None, None);
    step(game, "rim styles", Key::Right, true)?;
    game.shop.visit = shop_of(game, "IND_CAR_SPECIALTY_2");
    game.mode = game.spinner_browser(None, None);
    step(game, "spinner styles", Key::Right, true)?;
    // The specialties' lists (not Custom Gauges, 0xA3) and the neon's.
    let Mode::Menu(Menu::Parts(b)) = game.specialties_browser(0, None) else { anyhow::bail!("the specialties") };
    for (c, cat) in b.categories.iter().enumerate() {
        if cat.locked || cat.action != screens::CategoryAction::Parts || cat.parts.len() < 2 || cat.slot == 0xA3 {
            continue;
        }
        game.mode = game.specialties_browser(c, None);
        game.menu_key(Key::Accept)?;
        list_step(game, &format!("specialties {}", label(game, cat.label)), true)?;
        checked += 1;
    }
    let Mode::Menu(Menu::Parts(b)) = game.neon_browser(0, None) else { anyhow::bail!("the neon") };
    for (c, cat) in b.categories.iter().enumerate() {
        if cat.locked || cat.parts.len() < 2 {
            continue;
        }
        game.mode = game.neon_browser(c, Some(0));
        list_step(game, &format!("neon {}", label(game, cat.label)), true)?;
        checked += 1;
    }
    // The graphics shop: the base paint opens on the car's own (its finish
    // and swatch), then a step paints the ride; a vinyl; a decal.
    game.shop.visit = shop_of(game, "IND_GRAPHICS");
    let own = ride(game).get(&0x3F).copied();
    game.mode = game.paint_shop(0, None, None);
    game.menu_key(Key::Accept)?;
    let _ = game.garage_request();
    anyhow::ensure!(
        ride(game).get(&0x3F).copied() == own,
        "the base paint opened off the car's own ({own:x?} → {:x?})",
        ride(game).get(&0x3F)
    );
    step(game, "base paint", Key::Right, true)?;
    game.mode = game.vinyl_browser(0x40, None, None);
    game.menu_key(Key::Accept)?;
    list_step(game, "vinyls", true)?;
    game.mode = game.decal_shop(0, 0, Some(0));
    list_step(game, "windshield decals", true)?;
    println!("shop check: {checked} lists of the body shop, carbon fibre, the specialties and neon previewed");
    // The engage dialogs: entered shops ask the generic question.
    for shop in &game.game_data.career.shops {
        let entered = game.profile.as_ref().is_some_and(|p| p.entered.contains(&shop.hash));
        let text = screens::ShopEngage::message(&game.game_data.assets, shop, entered);
        let first = screens::ShopEngage::message(&game.game_data.assets, shop, false);
        println!("shop check: {} entered {entered}: {text:?} (first visit: {first:?})", shop.name);
        if entered && shop.kind != 0 {
            anyhow::ensure!(text == label(game, 0x15D6_B438), "{}: an entered shop's dialog", shop.name);
        }
    }
    Ok(())
}

/// UI_ICEMAIN's list for a slot at IND_CAR_SPECIALTY_2, opened on its
/// fitted item as Accept does (0x5606F0).
pub(crate) fn specials_list(game: &mut Game, slot: u8) -> Result<()> {
    game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == "IND_CAR_SPECIALTY_2").map(|s| s.hash);
    let at = screens::SPECIALTIES.iter().position(|c| c.0 == slot).context("the slot's item")?;
    game.mode = game.specialties_browser(at, None);
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("UI_ICEMAIN") };
    anyhow::ensure!(!b.categories[at].locked, "slot {slot:#x} locked on the finished-career profile");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_some()),
        "slot {slot:#x}: no list after Accept ({})",
        mode_name(&game.mode)
    );
    Ok(())
}

/// Down / Up the open list to the part `hash`.
pub(crate) fn specials_goto(game: &mut Game, hash: u32) -> Result<()> {
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("no list") };
    let c = &b.categories[b.category];
    let want = c.parts.iter().position(|p| p.0 == hash).context("the part is not listed")?;
    let at = b.part.unwrap_or(0);
    for _ in want..at {
        game.menu_key(Key::Up)?;
    }
    for _ in at..want {
        game.menu_key(Key::Down)?;
    }
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part == Some(want)),
        "the list did not reach the part"
    );
    Ok(())
}

/// Accept on the highlighted part, answering Yes: bought and fitted.
pub(crate) fn specials_buy(game: &mut Game, slot: u8, hash: u32) -> Result<u32> {
    let cash = game.profile.as_ref().map_or(0, |p| p.cash);
    game.menu_key(Key::Accept)?;
    for _ in 0..3 {
        let Mode::Menu(Menu::Dialog(d, _)) = &game.mode else { break };
        println!("specials: dialog {:?}", d.text);
        if d.buttons.len() == 2 {
            game.menu_key(Key::Left)?;
        }
        game.menu_key(Key::Accept)?;
    }
    let p = game.profile.as_ref().context("profile")?;
    anyhow::ensure!(
        p.installed.get(&slot) == Some(&hash),
        "slot {slot:#x}: {hash:#010x} not fitted (fitted {:x?})",
        p.installed.get(&slot)
    );
    Ok(cash - p.cash)
}

pub(crate) fn specials_hydro3(game: &mut Game) -> Result<Mode> {
    let car = game.profile.as_ref().context("profile")?.car.clone();
    let lvl3 = game.game_data.parts.slot_parts(&car, 0xA1).into_iter().find(|p| p.level == 3).context("LVL 3 HYDRAULICS")?.hash;
    specials_list(game, 0xA1)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("list") };
    let listed: Vec<(String, i32)> = b.categories[b.category].parts.iter().map(|p| (p.1.clone(), p.2)).collect();
    println!("specials: hydraulics list {listed:?}");
    anyhow::ensure!(
        b.categories[b.category].parts.iter().any(|p| p.0 == lvl3 && p.2 == 1600),
        "LVL 3 HYDRAULICS listed at 1,600 (Wine)"
    );
    specials_goto(game, lvl3)?;
    // The preview fits it on the FE car, whose garage motion is mode 3
    // (0x8363F8 set by the list's entry): the front and rear pump.
    let r = game.garage_request().context("garage")?;
    anyhow::ensure!(r.hydraulics.0 == 3, "the FE car's hydraulics level {} in the list (want 3)", r.hydraulics.0);
    let mut g = nfsu2::hydraulics::Garage::default();
    let (mut lo, mut hi) = ([f32::MAX; 4], [f32::MIN; 4]);
    for _ in 0..240 {
        g.update(3, r.hydraulics.1, 1.0 / 60.0);
        for i in 0..4 {
            lo[i] = lo[i].min(g.corner[i]);
            hi[i] = hi[i].max(g.corner[i]);
        }
    }
    println!("specials: mode 3 corners over 4 s: {lo:?} … {hi:?}");
    anyhow::ensure!(g.mode == 3 && (0..4).all(|i| hi[i] - lo[i] > 0.3), "mode 3 does not pump");
    let paid = specials_buy(game, 0xA1, lvl3)?;
    println!("specials: LVL 3 HYDRAULICS bought for {paid}");
    anyhow::ensure!(paid == 1600, "paid {paid}, want 1600");
    // Out of the list the FE car stands (0x8363F8 cleared).
    game.mode = game.specialties_browser(screens::SPECIALTIES.iter().position(|c| c.0 == 0xA1).unwrap_or(0), None);
    anyhow::ensure!(game.garage_request().is_some_and(|r| r.hydraulics.0 == 0), "the strip shows hydraulics motion");
    // In game: car +0x581 = 3, so the pressure mode's left / right axes
    // work (level 2 or 3, 0x4078A0).
    let level = game
        .profile
        .as_ref()
        .and_then(|p| p.installed.get(&0xA1).and_then(|&h| game.game_data.parts.find(&p.car, 0xA1, Some(h), None)))
        .map_or(0, |x| x.level);
    anyhow::ensure!(level == 3, "the fitted level {level}");
    let mut c = nfsu2::hydraulics::Controller::default();
    let ctx = nfsu2::hydraulics::Context::default();
    let h = nfsu2::hydraulics::Keys { pressure: true, ..Default::default() };
    for _ in 0..2 {
        c.update(level, nfsu2::hydraulics::Keys::default(), &ctx, 1.0 / 120.0);
    }
    for _ in 0..40 {
        c.update(level, h, &ctx, 1.0 / 120.0);
    }
    let left = c.update(level, nfsu2::hydraulics::Keys { lr: 1.0, ..h }, &ctx, 1.0 / 120.0);
    anyhow::ensure!(
        c.mode == 1 && left.pressure == [1.0, 0.0, 0.0, 1.0],
        "level 3's pressure mode: mode {} pressure {:?}",
        c.mode,
        left.pressure
    );
    // The shot: the list again on the fitted LVL 3, the car mid-pump.
    specials_list(game, 0xA1)?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(crate) fn specials_purge(game: &mut Game) -> Result<Mode> {
    let car = game.profile.as_ref().context("profile")?.car.clone();
    let type3 = game
        .game_data.parts
        .slot_parts(&car, 0xA2)
        .into_iter()
        .find(|p| game.game_data.parts.string(p.name as u32).as_deref() == Some("TYPE 3 RED"))
        .context("TYPE 3 RED")?
        .hash;
    specials_list(game, 0xA2)?;
    // Test N2O Purge (0x80409's 0x80000) on the help bar.
    let mask = match &game.mode {
        Mode::Menu(Menu::Parts(b)) => b.help_mask().unwrap_or(0),
        _ => 0,
    };
    anyhow::ensure!(screens::help_labels(mask).contains(&0xBA0E_1020), "no Test N2O Purge on the help bar ({mask:#x})");
    specials_goto(game, type3)?;
    let paid = specials_buy(game, 0xA2, type3)?;
    println!("specials: TYPE 3 RED purge bought for {paid}");
    // The car's purge set-up (0x634810): the hood markers and both
    // wheels, the part's colour.
    let installed = game.profile.as_ref().context("profile")?.installed.clone();
    let built = nfsu2::car::Car::load_with(&game.game_data.root, &car, &installed)?;
    let fx = built.purge.clone().context("no purge set-up")?;
    println!("specials: purge groups {:08X?} hsl {:?} markers {:?}", fx.groups, fx.hsl, fx.markers);
    anyhow::ensure!(fx.markers[0].len() == 2 && fx.markers[1].len() == 2, "TYPE 3: two hood markers and two wheels");
    // Test N2O Purge: C toggles it on (0xC519BFC0), the FE car fires
    // kinds 13 / 14, its release stops it (0xD871B0A4).
    specials_list(game, 0xA2)?;
    game.menu_key(Key::Button1)?;
    anyhow::ensure!(
        game.shop.purge_test && game.garage_request().is_some_and(|r| r.purge),
        "C did not start the purge test"
    );
    game.key_released(KeyCode::KeyC);
    anyhow::ensure!(!game.shop.purge_test, "C's release did not stop it");
    // The help bar's Test N2O Purge toggles (no release).
    anyhow::ensure!(
        matches!(mouse::help_message(0x8_0000, false, false), Some(mouse::HelpMsg::Key(Key::Button1))),
        "the help bar's 0x80000"
    );
    // NFSU2_PURGE_SHOW=<name>: the list on that part for the picture
    // (the preview fits it; the test stays on).
    if let Some(name) = std::env::var("NFSU2_PURGE_SHOW").ok()
        && let Some(p) = game
            .game_data.parts
            .slot_parts(&car, 0xA2)
            .into_iter()
            .find(|p| game.game_data.parts.string(p.name as u32).as_deref() == Some(name.as_str()))
    {
        specials_goto(game, p.hash)?;
    }
    game.menu_key(Key::Button1)?;
    anyhow::ensure!(game.shop.purge_test, "the second toggle");
    // The smoke: the FE car's emitters burst at its hood and wheels.
    let mut lib = nfsu2::particles::Library::default();
    let globalb = nfsu2::game_fs::read(&game.game_data.root, "GLOBAL/GLOBALB.BUN")?;
    lib.add(&globalb);
    lib.add_templates(&globalb);
    let mut sys = nfsu2::particles::System::default();
    let mut smoke = PurgeSmoke::default();
    smoke.burst(&mut sys, &lib, &fx, glam::Mat4::IDENTITY, Vec3::ZERO, 0, &mut nfsu2::particles::Rng(7));
    let palettes: std::collections::BTreeSet<u16> = sys.particles.iter().map(|p| p.palette).collect();
    println!("specials: one purge tick: {} particles, colour slots {palettes:?}", sys.particles.len());
    anyhow::ensure!(sys.particles.len() >= 8 && !palettes.contains(&0), "the purge burst");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
