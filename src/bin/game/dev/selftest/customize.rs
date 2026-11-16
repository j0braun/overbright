use crate::*;

/// The customize extras: Spinners in the car specialties shop open
/// UI_ChooseSpinner and Back returns on them; the uniques shop opens
/// UI_ChooseUniquePart whose Back asks first; a kind-3 photo shoot's
/// UI_MagazineReward asks to save the magazine.
pub(crate) fn selftest_customize_extras(game: &mut Game) -> Result<()> {
    let at = screens::SPECIALTIES.iter().position(|c| c.0 == 0x1D).context("Spinners")?;
    game.mode = game.specialties_browser(at, None);
    let locked = matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.categories[at].locked);
    if !locked {
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(
            matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ChooseSpinner"),
            "Spinners: UI_ChooseSpinner"
        );
        game.menu_key(Key::Back)?;
        anyhow::ensure!(
            matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.category == at),
            "back on Spinners"
        );
    }
    let uniques = game.game_data.career.shops.iter().position(|s| s.kind == 6).context("the uniques shop")?;
    game.mode = game.shop_front_end(uniques);
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ChooseUniquePart"),
        "UI_ChooseUniquePart"
    );
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.buttons.len() == 2),
        "the unique part's Back asks"
    );
    game.menu_key(Key::Right)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ChooseUniquePart"), "No stays");
    let shoot = game.game_data.career.milestones.iter().position(|m| m.kind == 3).context("a kind-3 shoot")?;
    game.mode = game.magazine_reward(shoot);
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Cover(v)) if v.reward && !v.dvd), "UI_MagazineReward");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Dialog(..))), "save this magazine?");
    // Yes: the cover's own file ("<NAME> Magazine <n>", 0x523F00), then
    // the library's message (saved, or unable without a drawn shot).
    answer_yes(game)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMessage(..))),
        "the cover's save message ({})",
        flow_mode(&game.mode)
    );
    let name = game.profile.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let n = game.game_data.career.milestones.iter().take(shoot + 1).filter(|m| matches!(m.kind, 1 | 3)).count();
    let file = profiles::root().join(profiles::cover_name(&name, false, n)).join(profiles::cover_name(&name, false, n));
    if let Ok(meta) = std::fs::metadata(&file) {
        anyhow::ensure!(meta.len() as usize == nfsu2::formats::native_save::PICTURE_FILE_LEN, "the cover file's size");
        anyhow::ensure!(!profiles::list().iter().any(|p| p.contains("Magazine")), "covers are not profiles");
        println!("cover saved: {}", file.display());
    }
    // UI_MagazineView shows the saved picture: "Load complete." then the
    // view with it bound; a cover never saved tells "Unable to load …" and
    // stays on the list (Wine).
    if !file.is_file() {
        let rgba = vec![200u8; 320 * 432 * 4];
        profiles::write_cover(
            &profiles::cover_name(&name, false, n),
            &nfsu2::formats::native_save::picture(&rgba, 320, 432),
        )?;
    }
    game.mode = game.covers_menu(false, 0);
    game.act(Action::Cover(shoot), MenuKind::Covers(false))?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMessage(m, _)) if m.text.contains("Load complete")),
        "the cover's load message ({})",
        flow_mode(&game.mode)
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Cover(v)) if v.picture && !v.reward && v.camera.is_none()),
        "UI_MagazineView on the saved picture"
    );
    anyhow::ensure!(game.showcase.cover_picture.as_ref().is_some_and(|p| p.len() == 512 * 512 * 4), "the picture loaded");
    std::fs::remove_file(&file)?;
    game.mode = game.covers_menu(false, 0);
    game.act(Action::Cover(shoot), MenuKind::Covers(false))?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMessage(m, _)) if m.text.contains("Unable to load") && m.text.contains("Magazine")),
        "the missing cover's message"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Covers(false)))),
        "back on the list ({})",
        flow_mode(&game.mode)
    );
    println!("magazine view: ok");
    // A kind-1 shoot's UI_ShowCase_Preview: the highlight starts on Take
    // Picture and stays within the four; Take Picture's Accept asks to save
    // and either answer leaves.
    let shoot = game.game_data.career.milestones.iter().position(|m| m.kind == 1).context("a kind-1 shoot")?;
    game.mode = Mode::Menu(Menu::Showcase(game.showcase_preview(shoot, false, game.now())));
    game.menu_key(Key::Down)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.option == 3 && !v.picture),
        "the showcase on Take Picture"
    );
    game.menu_key(Key::Up)?;
    game.menu_key(Key::Up)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.option == 1), "Up moves the highlight");
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.picture),
        "Take Picture: the picture state"
    );
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if !v.picture), "Back: the menu again");
    // Cycle Cameras (0x4B5EA0 → 0x510710) steps the set's camera and stays
    // in the menu; Show Off toggles the hood and the doors and Back leaves
    // them open; Control Car (0x4B5C70 → 0x499F10) shuts them.
    let before = match &game.mode {
        Mode::Menu(Menu::Showcase(v)) => (v.setup, v.picks.index),
        _ => (None, 0),
    };
    anyhow::ensure!(before.0.is_some() || game.showcase.cameras.is_none(), "the cover's camera set");
    game.menu_key(Key::Up)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.mode == screens::ShowcaseMode::Menu && (before.0.is_none() || v.picks.index != before.1)),
        "Cycle Cameras: the next camera"
    );
    game.menu_key(Key::Up)?;
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Up)?;
    game.menu_key(Key::Left)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.mode == screens::ShowcaseMode::ShowOff && v.open == [true, true, false]),
        "Show Off: hood and doors"
    );
    game.menu_key(Key::Back)?;
    game.menu_key(Key::Up)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.mode == screens::ShowcaseMode::Control && v.open == [false; 3]),
        "Control Car shuts them"
    );
    game.menu_key(Key::Back)?;
    for _ in 0..3 {
        game.menu_key(Key::Down)?;
    }
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Showcase(v)) if v.picture && (v.streamed || game.showcase.cameras.is_none())),
        "Take Picture: the masthead streamed"
    );
    game.menu_key(Key::Accept)?;
    let save = game.label(0x0331_2F33, "");
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o)) if d.buttons.len() == 2 && matches!(&o[0], Mode::Menu(Menu::LotDeal(LotDeal::SaveShowcase(_)))) && matches!(&o[1], Mode::Explore))
            && !save.is_empty(),
        "save this magazine? ({save})"
    );
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    println!(
        "customize extras: spinners {}, uniques, magazine reward, showcase: ok",
        if locked { "locked" } else { "open" }
    );
    Ok(())
}

/// Each Customize list's opening entry with a known install (Wine,
/// the finished-career save): Custom Gauges on the top (0x56C220 → 0x53C200), the other
/// specialties and neon lists on the fitted part (0x5606F0, 0x53BA10), a
/// vinyl layer's list on the car's vinyl over the setup's (0x53A4C0), a
/// decal slot's on the setup's decal, else (none, or "No Decal" in the
/// setup) the car's (0x560260).
pub(crate) fn selftest_list_openings(game: &mut Game) -> Result<()> {
    let saved = (
        game.profile.clone().context("a career")?,
        game.shop.setup.clone(),
        std::mem::replace(&mut game.mode, Mode::Explore),
    );
    let unlock = session::UnlockAll::set(true);
    let result = (|| -> Result<()> {
        let fit = |game: &mut Game, slot: u8, hash: u32| {
            game.profile.as_mut().unwrap().installed.insert(slot, hash);
        };
        let opened = |game: &mut Game| -> Option<usize> {
            let _ = game.menu_key(Key::Accept);
            match &game.mode {
                Mode::Menu(Menu::Parts(b)) => b.part,
                // Split Hoods with a hood decal: the warning over the list.
                Mode::Menu(Menu::Dialog(_, next)) => match next.first() {
                    Some(Mode::Menu(Menu::Parts(b))) => b.part,
                    _ => None,
                },
                _ => None,
            }
        };
        // The specialties' lists: Custom Gauges on its top, the others on
        // the fitted part.
        // Nitrous Purge without a nitrous part opens its notice (UI_ICEMAIN
        // 0x568750 → 0x558440), the trunk audio its own menu: not lists here.
        for (at, &(slot, ..)) in screens::SPECIALTIES.iter().enumerate().filter(|(_, c)| !matches!(c.0, 0xA2 | 0x22)) {
            let Mode::Menu(Menu::Parts(b)) = game.specialties_browser(at, None) else { continue };
            let Some(c) = b
                .categories
                .get(at)
                .filter(|c| c.action == screens::CategoryAction::Parts && !c.locked && c.parts.len() > 2)
            else {
                continue;
            };
            let hash = c.parts[2].0;
            fit(game, slot, hash);
            game.mode = game.specialties_browser(at, None);
            let part = opened(game);
            let want = if slot == 0xA3 { 0 } else { 2 };
            anyhow::ensure!(part == Some(want), "specialties slot {slot:#x}: opened on {part:?}, want {want}");
            println!("list opening: specialties {slot:#x} on {want}");
            // Lists Open on Installed (clone deviation): Custom Gauges on
            // the fitted gauge too, the others unchanged.
            let mut enh = game.settings.enh;
            enh.lists_open_installed = true;
            let applied = session::EnhApplied::apply(enh, game.settings.enh);
            game.mode = game.specialties_browser(at, None);
            let part = opened(game);
            drop(applied);
            anyhow::ensure!(
                part == Some(2),
                "Lists Open on Installed: specialties slot {slot:#x} opened on {part:?}, want 2"
            );
        }
        // A neon slot's list on the fitted neon.
        let neon = game.slot_parts(0x9A, false).get(1).map(|p| p.hash);
        if let Some(hash) = neon {
            fit(game, 0x9A, hash);
            game.mode = game.neon_browser(0, None);
            anyhow::ensure!(opened(game) == Some(1), "the underglow list on the fitted neon");
        }
        // A vinyl layer: the car's vinyl, not the setup's pick.
        let vinyls: Vec<(u32, u8)> = game.slot_parts(0x40, false).iter().map(|p| (p.hash, p.kind)).collect();
        let pair =
            vinyls.iter().find_map(|&(a, k)| vinyls.iter().find(|&&(b, j)| j == k && b != a).map(|&(b, _)| (a, b, k)));
        if let Some((car, pick, kind)) = pair {
            fit(game, 0x40, car);
            game.shop.setup.put_vinyl(0x40, pick, false, false);
            anyhow::ensure!(game.vinyl_list_current(0x40) == Some(car), "the layer's list on the car's vinyl");
            let group = graphics::VINYL_GROUPS.iter().position(|g| g.0 == kind).context("the vinyl's group")?;
            game.mode = game.vinyl_browser(0x40, None, None);
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.category == group),
                "the strip on the car's vinyl's group"
            );
            let part = opened(game);
            let want = match &game.mode {
                Mode::Menu(Menu::Parts(b)) => b.categories[group].parts.iter().position(|p| p.0 == car),
                _ => None,
            };
            anyhow::ensure!(part.is_some() && part == want, "the vinyl list on the car's vinyl ({part:?} / {want:?})");
            game.shop.setup.drop_layer(0x40);
            game.profile.as_mut().unwrap().installed.remove(&0x40);
            game.shop.setup.put_vinyl(0x40, pick, false, false);
            anyhow::ensure!(game.vinyl_list_current(0x40) == Some(pick), "no car vinyl: the setup's");
            game.shop.setup.drop_layer(0x40);
        }
        // A decal slot: the setup's decal, else the car's.
        let decals: Vec<u32> = game.decal_list(0x61).iter().map(|p| p.hash).collect();
        if decals.len() > 1 {
            fit(game, 0x61, decals[0]);
            game.shop.setup.put(0x61, decals[1]);
            anyhow::ensure!(game.decal_list_current(0x61) == Some(decals[1]), "the decal list on the setup's decal");
            game.shop.setup.remove(0x61, true);
            anyhow::ensure!(game.decal_list_current(0x61) == Some(decals[0]), "No Decal in the setup: the car's decal");
        }
        Ok(())
    })();
    drop(unlock);
    (game.profile, game.shop.setup, game.mode) = (Some(saved.0), saved.1, saved.2);
    result
}

/// Custom Gauges: a custom gauge's accept opens UI_ChooseCustomHUD's
/// steps; the picks are fitted to 0xA4 / 0xA6 / 0xA5 with the gauge and
/// reach the race HUD's colours (0x4F5100). The profile is put back.
pub(crate) fn selftest_custom_hud(game: &mut Game) -> Result<()> {
    let saved_mode = std::mem::replace(&mut game.mode, Mode::Explore);
    let saved = game.profile.clone().context("a career")?;
    // The purchase drops the drive (the car rebuilt); keep the free roam.
    let drive = game.sim.drive.take();
    game.profile.as_mut().unwrap().cash = 100_000;
    game.mode = game.choose_custom_hud(0, 1);
    let Mode::Menu(Menu::ChooseHud(c)) = &game.mode else { anyhow::bail!("UI_ChooseCustomHUD") };
    anyhow::ensure!(
        c.hud_index == 1 && c.palettes.iter().all(|p| p.len() == 12),
        "FLAMES (HUDINDEX 1), twelve swatches a step"
    );
    let palettes = c.palettes.clone();
    let fitted_needle = c.chosen[2];
    // Backing: two right (OCEAN); dial: one right (BLUE); needle: no move.
    for k in [Key::Right, Key::Right, Key::Accept, Key::Right, Key::Accept] {
        game.menu_key(k)?;
    }
    let Mode::Menu(Menu::ChooseHud(c)) = &game.mode else { anyhow::bail!("on the needle colour") };
    anyhow::ensure!(c.step == 2 && c.index == [3, 2, 1], "the steps' indices");
    game.menu_key(Key::Accept)?;
    // The purchase question (0x44B7167F, 0x564248), No focused: Yes.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1 && d.buttons.len() == 2),
        "the purchase question"
    );
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.selected = 0;
    }
    game.menu_key(Key::Accept)?;
    let p = game.profile.as_ref().unwrap();
    let gauge = game.specialties_gauge(0, 1);
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_none()), "back to the categories");
    anyhow::ensure!(p.installed.get(&0xA3) == Some(&gauge), "the gauge fitted");
    anyhow::ensure!(
        p.installed.get(&0xA4) == Some(&palettes[0][2].hash) && p.installed.get(&0xA6) == Some(&palettes[1][1].hash),
        "backing and dial colours fitted"
    );
    anyhow::ensure!(p.installed.get(&0xA5).copied() == fitted_needle, "the needle's unmoved pick kept");
    let colours = [0xA4, 0xA5, 0xA6].map(|slot| game.gauge_colour(slot).map(|c| c.1));
    anyhow::ensure!(
        colours[0] == Some(palettes[0][2].rgb) && colours[2] == Some(palettes[1][1].rgb),
        "the race HUD's colours"
    );
    // Lists Open on Installed (clone deviation): the steps on the fitted
    // colours (backing OCEAN, dial BLUE); off, native swatch 1.
    game.mode = game.choose_custom_hud(0, 1);
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::ChooseHud(c)) if c.index == [1, 1, 1]),
        "native: every step on swatch 1"
    );
    let mut enh = game.settings.enh;
    enh.lists_open_installed = true;
    let applied = session::EnhApplied::apply(enh, game.settings.enh);
    game.mode = game.choose_custom_hud(0, 1);
    drop(applied);
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::ChooseHud(c)) if c.index[..2] == [3, 2]),
        "Lists Open on Installed: the steps on the fitted colours"
    );
    // Back from the backing colour: the gauge list on the gauge.
    game.mode = game.choose_custom_hud(0, 1);
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part == Some(1)), "back to the gauge list");
    println!("custom gauges: FLAMES with {colours:?}");
    let _ = saved.save();
    game.profile = Some(saved);
    game.sim.drive = drive;
    game.mode = saved_mode;
    Ok(())
}
