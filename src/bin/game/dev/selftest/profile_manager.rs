use crate::*;

/// The Profile Manager on native profile files: Create (named, saved,
/// "Save complete."), the boot's handling of one and of two profiles, Load
/// with its question, Save with the overwrite question, Delete.
pub(crate) fn selftest_profiles(game: &mut Game) -> Result<()> {
    let mc_text = |game: &Game| match &game.mode {
        Mode::Menu(Menu::McMessage(m, _)) => m.text.clone(),
        _ => String::new(),
    };
    let accept_yes = |game: &mut Game| -> Result<()> {
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)
    };
    game.mode = game.icon_menu(MenuKind::Profile);
    // Create: Yes, the name "NAME" + "AB", Yes to save.
    game.act(Action::ProfileCreate, MenuKind::Profile)?;
    accept_yes(game)?;
    if let Mode::Menu(Menu::McMain(m)) = &mut game.mode {
        m.keys = 1;
        m.char('A');
        m.char('B');
    }
    game.menu_key(Key::Accept)?;
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Save complete.", "create: {:?}", mc_text(game));
    anyhow::ensure!(
        profiles::file("NAMEAB").is_file() && profiles::current_name().as_deref() == Some("NAMEAB"),
        "NAMEAB saved"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Profile))), "back to the Profile Manager");
    // Save asks twice now that the file exists.
    if let Some(p) = game.profile.as_mut() {
        p.cash = 4321;
    }
    game.act(Action::ProfileSave, MenuKind::Profile)?;
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "overwrite? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Save complete.", "saved");
    let saved = nfsu2::formats::native_save::NativeSave::parse(&std::fs::read(profiles::file("NAMEAB"))?)?;
    anyhow::ensure!(
        saved.u32(nfsu2::formats::native_save::CASH) == 4321 && saved.name() == "NAMEAB",
        "the file holds the bank"
    );
    game.menu_key(Key::Accept)?;
    // A second profile, then Load lists both and asks first.
    let mut other = Profile::new("RAY");
    other.cash = 77;
    other.save()?;
    profiles::set_current("NAMEAB", saved);
    game.act(Action::ProfileLoad, MenuKind::Profile)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::McList(l)) if l.names == ["NAMEAB", "RAY"]), "the list");
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(mc_text(game).starts_with("Loading a profile"), "load? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Load complete.", "loaded");
    anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.name == "RAY" && p.cash == 77), "RAY current");
    game.menu_key(Key::Accept)?;
    // Delete RAY.
    game.act(Action::ProfileDelete, MenuKind::Profile)?;
    game.menu_key(Key::Down)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(mc_text(game) == "Are you sure you want to delete RAY?", "delete? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Save game deleted." && !profiles::file("RAY").exists(), "deleted");
    anyhow::ensure!(game.profile.is_none(), "the current profile went with it");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(profiles::list() == ["NAMEAB"], "one left");
    // The save points (0x4A3100 operation 0x40 / 0x50; see `save_point`).
    profiles::load("NAMEAB")?;
    game.profile = Profile::load();
    let cash_on_disk =
        |_: &Game| profiles::read("NAMEAB").map(|s| s.u32(nfsu2::formats::native_save::CASH)).unwrap_or(0);
    // A commit keeps the payload in memory only.
    if let Some(p) = game.profile.as_mut() {
        p.cash = 555;
        p.commit()?;
    }
    anyhow::ensure!(
        cash_on_disk(game) == 4321 && Profile::load().is_some_and(|p| p.cash == 555),
        "commit: memory only"
    );
    // Options with Prompt: a change asks on the way out, then UI_Main.
    game.settings.save_prompts = true;
    game.settings.autosave = false;
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::Options, MenuKind::Main)?;
    if let Some(p) = game.profile.as_mut() {
        p.cash = 556;
    }
    game.menu_key(Key::Back)?;
    anyhow::ensure!(mc_text(game).starts_with("Are you sure you want to save"), "options: save? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "options: overwrite? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Save complete." && cash_on_disk(game) == 556, "options: saved");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "options: UI_Main");
    // No at the overwrite question: "Are you sure…" again (MC_Main state
    // 5, Wine); No there leaves unsaved.
    game.act(Action::Options, MenuKind::Main)?;
    if let Some(p) = game.profile.as_mut() {
        p.cash = 560;
    }
    game.menu_key(Key::Back)?;
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "options: overwrite? {:?}", mc_text(game));
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        mc_text(game).starts_with("Are you sure you want to save"),
        "overwrite No: save? {:?}",
        mc_text(game)
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) && cash_on_disk(game) == 556,
        "No, No: UI_Main unsaved"
    );
    // The "No" holds: the progress stays in memory (0x5347D0 restores no
    // snapshot here, docs/save-points.md) and the clone's own points that
    // used to save silently (a shop, Car Select, tuning, the quit to the
    // main menu) only commit, so nothing reaches the file before the next
    // save point.
    if let Some(p) = game.profile.as_mut() {
        p.commit()?;
    }
    anyhow::ensure!(
        cash_on_disk(game) == 556 && Profile::load().is_some_and(|p| p.cash == 560),
        "No: kept in memory, not written by a later commit"
    );
    if let Some(p) = game.profile.as_mut() {
        p.cash = 556;
    }
    // Unchanged: no question.
    game.act(Action::Options, MenuKind::Main)?;
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))), "options unchanged: straight on");
    // Autosave: the first save asks to overwrite, the next writes at once.
    game.settings.autosave = true;
    game.save_flow.autosaved = false;
    for (cash, first) in [(557, true), (558, false)] {
        game.act(Action::Options, MenuKind::Main)?;
        if let Some(p) = game.profile.as_mut() {
            p.cash = cash;
        }
        game.menu_key(Key::Back)?;
        if first {
            anyhow::ensure!(mc_text(game).contains("overwrite"), "autosave: overwrite? {:?}", mc_text(game));
            accept_yes(game)?;
        }
        anyhow::ensure!(
            cash_on_disk(game) == cash && matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
            "autosave {cash}"
        );
    }
    // The first autosave's overwrite question answered No: Autosave off
    // (the Save option at Prompt), then "Are you sure…"; Yes asks to
    // overwrite again and saves with "Save complete." (Wine).
    game.save_flow.autosaved = false;
    game.act(Action::Options, MenuKind::Main)?;
    if let Some(p) = game.profile.as_mut() {
        p.cash = 559;
    }
    game.menu_key(Key::Back)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "first autosave: overwrite? {:?}", mc_text(game));
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(!game.settings.autosave && game.settings.save_prompts, "first autosave No: Prompt");
    anyhow::ensure!(
        mc_text(game).starts_with("Are you sure you want to save"),
        "first autosave No: save? {:?}",
        mc_text(game)
    );
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "then overwrite? {:?}", mc_text(game));
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game) == "Save complete." && cash_on_disk(game) == 559, "then saved");
    game.menu_key(Key::Accept)?;
    // The Profile Manager's Save with Autosave on (session flag set): "Are
    // you sure…", the overwrite question all the same; No there asks "Are
    // you sure…" again and keeps Autosave; Yes saves with no message.
    game.settings.autosave = true;
    game.save_flow.autosaved = true;
    if let Some(p) = game.profile.as_mut() {
        p.cash = 561;
    }
    game.mode = game.icon_menu(MenuKind::Profile);
    game.act(Action::ProfileSave, MenuKind::Profile)?;
    accept_yes(game)?;
    anyhow::ensure!(mc_text(game).contains("overwrite"), "manager autosave: overwrite? {:?}", mc_text(game));
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        mc_text(game).starts_with("Are you sure you want to save") && game.settings.autosave,
        "manager No: save? {:?}",
        mc_text(game)
    );
    accept_yes(game)?;
    accept_yes(game)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Profile))) && cash_on_disk(game) == 561,
        "manager autosave: saved, no message"
    );
    game.mode = game.icon_menu(MenuKind::Main);
    game.settings.autosave = false;
    // Delete Tuned Car (game mode 0x20, BUTTON1): asks, Yes clears the
    // record and nothing is saved.
    if let Some(p) = game.profile.as_mut() {
        p.my_cars[3] = Some(career::MyCar { id: career::tuned_car_id(3), car: "PEUGOT".into(), ..Default::default() });
        p.menu_car_id = Some(career::tuned_car_id(3));
    }
    game.act(Action::Customize, MenuKind::Main)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.cars[l.selected].my_car == Some(3)),
        "the car select on player 0's car"
    );
    game.menu_key(Key::Button1)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.starts_with("Would you like to delete")),
        "delete?"
    );
    accept_yes(game)?;
    anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.my_cars[3].is_none()), "deleted");
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::CarLot(l)) if l.state == LotState::Customize && l.cars.iter().all(|c| c.my_car.is_none())),
        "the list again"
    );
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) && cash_on_disk(game) == 561,
        "Back: UI_Main, nothing saved"
    );
    println!("profiles: save points ok");
    // A name that differs only in case is that profile (Windows' file
    // system; under Wine "name" over "NAME" asked to overwrite NAME/NAME).
    anyhow::ensure!(profiles::exists("nameab") && profiles::resolve("nameab") == "NAMEAB", "case-insensitive lookup");
    let mut lower = Profile::new("nameab");
    lower.cash = 99;
    lower.save()?;
    anyhow::ensure!(profiles::list() == ["NAMEAB"], "no second folder: {:?}", profiles::list());
    let s = profiles::read("NAMEAB")?;
    anyhow::ensure!(s.u32(nfsu2::formats::native_save::CASH) == 99 && s.name() == "nameab", "saved into NAMEAB");
    // Folders differing only in case need a case-sensitive file system
    // (Linux): on Windows (and Wine) "nameAB" is the NAMEAB folder.
    if cfg!(target_os = "linux") {
        // Folders differing only in case (Linux only): one entry, the newest
        // file's folder for every operation; the other is left alone.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(profiles::root().join("nameAB"))?;
        std::fs::write(profiles::file("nameAB"), s.to_bytes())?;
        anyhow::ensure!(
            profiles::list() == ["nameAB"] && profiles::resolve("NAMEAB") == "nameAB",
            "one entry, the newest: {:?}",
            profiles::list()
        );
        profiles::delete("NAMEAB")?;
        anyhow::ensure!(
            profiles::file("NAMEAB").is_file() && !profiles::file("nameAB").exists(),
            "deleted the newest only"
        );
        anyhow::ensure!(profiles::list() == ["NAMEAB"], "the other again: {:?}", profiles::list());
    }
    profiles::delete("nameab")?;
    anyhow::ensure!(profiles::list().is_empty(), "both gone");
    game.mode = game.icon_menu(MenuKind::Main);
    println!("profiles: ok");
    Ok(())
}
