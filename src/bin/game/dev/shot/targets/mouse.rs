//! `--shot` targets: The front end driven by the mouse (`mouse::fe_mouse`).
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("mouse_main", mouse_main),
    Target::name("mouse_drag", mouse_drag),
    Target::name("mouse_parts", mouse_parts),
    Target::name("mouse_paint", mouse_paint),
    Target::name("mouse_options", mouse_options),
    Target::name("mouse_settings", mouse_settings),
    Target::name("mouse_perf", mouse_perf),
    Target::name("mouse_controls", mouse_controls),
    Target::name("mouse_lists", mouse_lists),
    Target::name("mouse_carlot", mouse_carlot),
];

/// The mouse on UI_Main (`mouse::fe_mouse`, 0x54EB90 / 0x538140): a
/// click on the strip's right arrow steps it, one on the selected
/// icon accepts it (both seen under Wine).
pub(super) fn mouse_main(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Main);
    let record = |game: &mut Game| {
        nfsu2::frontend::mouse::begin_frame(true);
        let _ = game.menu_quads(game.now() + 2000.0);
        nfsu2::frontend::mouse::begin_frame(true);
    };
    record(game);
    let frame = game.ui_frame().context("a frame")?;
    let at = |r: nfsu2::frontend::mouse::Rect| {
        let c = (r.lo + r.hi) * 0.5;
        Vec2::new(
            frame.origin.0 + c.x * frame.scale.0 + r.place.x as f32 * frame.reach_x(r.place),
            frame.origin.1 + c.y * frame.scale.1,
        )
    };
    let before = match &game.mode {
        Mode::Menu(Menu::Icons(m, _)) => m.selected,
        _ => anyhow::bail!("the main menu"),
    };
    let arrow = nfsu2::frontend::mouse::rect("UI_Main", nfsu2::formats::hash::fe_hash("ScrollBar_Arrow_2"))
        .context("the right arrow drawn")?;
    game.platform.mouse.at = Some(at(arrow));
    game.platform.mouse.last = game.platform.mouse.at;
    game.platform.mouse.pressed = true;
    game.fe_mouse()?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Icons(m, _)) if m.selected == before + 1),
        "the right arrow steps the strip"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The scroll bar's drag on UI_Main (0x538140 → 0x52E4F0): the handle
/// picked up and pulled right steps the strip on.
pub(super) fn mouse_drag(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.icon_menu(MenuKind::Main);
    let selected = |g: &Game| match &g.mode {
        Mode::Menu(Menu::Icons(m, _)) => m.selected,
        _ => usize::MAX,
    };
    game.mouse_record();
    let handle = game.mouse_at("UI_Main", nfsu2::formats::hash::fe_hash("ScrollBar_Handle"))?;
    let bar = nfsu2::frontend::mouse::bar("UI_Main", "ScrollBar").context("UI_Main's bar")?;
    let frame = game.ui_frame().context("a frame")?;
    let before = selected(game);
    let to = handle + Vec2::new(bar.step * 2.5 * frame.scale.0, 0.0);
    game.mouse_drag(handle, to, 12)?;
    anyhow::ensure!(
        selected(game) >= before + 2,
        "dragging the handle 2.5 steps right steps the strip on twice ({before} → {})",
        selected(game)
    );
    anyhow::ensure!(nfsu2::frontend::mouse::drag().is_none(), "the release ends the drag");
    // A click on the track left of the handle: one back (0x538B90).
    let after = selected(game);
    game.mouse_record();
    let back = nfsu2::frontend::mouse::rect_any("UI_Main", nfsu2::formats::hash::fe_hash("ScrollBar_Backing"))
        .context("the backing")?;
    let p = game.mouse_point(back, Vec2::new(0.02, 0.5)).context("a point")?;
    game.mouse_frame(p, true, 0.0)?;
    anyhow::ensure!(
        selected(game) + 1 == after,
        "a track click before the handle steps back one ({after} → {})",
        selected(game)
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// UI_GenericParts_Browser's mouse (0x5664A0): a track click right of
/// the strip's handle pages three categories on; in a category the
/// wheel steps the parts and the button on MINI_MASTER asks to fit.
pub(super) fn mouse_parts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.parts_browser(0, None);
    let at = |g: &Game| match &g.mode {
        Mode::Menu(Menu::Parts(b)) => (b.category, b.part),
        _ => (usize::MAX, None),
    };
    game.mouse_record();
    let back =
        nfsu2::frontend::mouse::rect_any("UI_GenericParts_Browser", nfsu2::formats::hash::fe_hash("ScrollBar_Backing"))
            .context("the strip's backing")?;
    let p = game.mouse_point(back, Vec2::new(0.98, 0.5)).context("a point")?;
    game.mouse_frame(p, true, 0.0)?;
    anyhow::ensure!(at(game).0 == 3, "a track click pages three categories ({:?})", at(game));
    for _ in 0..3 {
        game.menu_key(Key::Left)?;
    }
    game.menu_key(Key::Accept)?;
    let first = at(game).1.context("in a category")?;
    game.mouse_record();
    let mini = game.mouse_at("UI_GenericParts_Browser", 0x38E3_9AD7)?;
    game.mouse_frame(mini, false, -1.0)?;
    anyhow::ensure!(at(game).1 == Some(first + 1), "the wheel steps the parts ({:?})", at(game));
    // As Select: 0x5664A0 runs the same block as Accept, which asks
    // first (`Game::body_part_question`): a dialog, nothing fitted yet.
    let before = game.profile.as_ref().map(|p| p.installed.clone());
    if let Some(p) = game.profile.as_mut() {
        p.cash = 1_000_000;
    }
    game.mouse_frame(mini, true, 0.0)?;
    anyhow::ensure!(
        game.profile.as_ref().map(|p| p.installed.clone()) == before
            && matches!(game.mode, Mode::Menu(Menu::Dialog(..))),
        "the button on MINI_MASTER asks about the part ({:?})",
        at(game)
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// UI_Paint's swatches (0x55D420): a move over one selects it, a
/// click on the selected one asks to add it.
pub(super) fn mouse_paint(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let mut m = game.paint_shop(0, None, Some(0));
    if let Mode::Menu(Menu::Paint(p)) = &mut m {
        p.selected.get_or_insert(0);
    }
    game.mode = m;
    game.mouse_record();
    let swatch = game.mouse_at("UI_Paint", nfsu2::formats::hash::fe_hash("PAINT_SWATCH_3"))?;
    game.mouse_frame(swatch, false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Paint(p)) if p.selected == Some(2)),
        "a move over the third swatch selects it"
    );
    game.mouse_frame(swatch, true, 0.0)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "a click on the selected swatch asks to add it");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The option list (0x52FC50): a move selects the row under the
/// cursor, a click on its right arrow raises it.
pub(super) fn mouse_options(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Audio,
        &game.settings,
        FORCE_FEEDBACK,
        0.0,
    )));
    game.mouse_record();
    let rows = nfsu2::frontend::mouse::slider_rows();
    let row = rows.iter().find(|r| r.0 == 1).copied().context("a second row")?;
    let frame = game.ui_frame().context("a frame")?;
    let px = |c: Vec2| {
        Vec2::new(frame.origin.0 + (c.x + 320.0) * frame.scale.0, frame.origin.1 + (c.y + 240.0) * frame.scale.1)
    };
    game.mouse_frame(px(row.1 + row.2 * 0.5), false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(o)) if o.selected == 1),
        "a move selects the second row"
    );
    let before = game.settings.clone();
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 1).context("the row")?;
    game.mouse_frame(px(row.4.0 + row.4.1 * 0.5), true, 0.0)?;
    anyhow::ensure!(game.settings != before, "a click on the right arrow changes the value");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The settings page by mouse — on the hub a move selects Advanced and a
/// click opens it; there a click on Compare's right arrow switches it, a
/// click on the Image Quality row opens that group page, and a click on a
/// group page's row arrow changes the value.
pub(super) fn mouse_settings(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::Options(options::OptionsScreen::new(
        options::Category::Enhancements,
        &game.settings,
        FORCE_FEEDBACK,
        0.0,
    )));
    game.mouse_record();
    let frame = game.ui_frame().context("a frame")?;
    let px = |c: Vec2| {
        Vec2::new(frame.origin.0 + (c.x + 320.0) * frame.scale.0, frame.origin.1 + (c.y + 240.0) * frame.scale.1)
    };
    let advanced = options::rows_in(options::Category::Enhancements, &game.settings, 0, FORCE_FEEDBACK, None)
        .iter()
        .position(|r| r.id == options::RowId::AdvancedPage)
        .context("the Advanced row")?;
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == advanced).context("the Advanced row")?;
    game.mouse_frame(px(row.1 + row.2 * 0.5), false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(o)) if o.selected == advanced),
        "a move selects Advanced"
    );
    game.mouse_frame(px(row.1 + row.2 * 0.5), true, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(o)) if o.category == options::Category::Advanced),
        "a click on Advanced opens it"
    );
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 0).context("the Compare row")?;
    game.mouse_frame(px(row.4.0 + row.4.1 * 0.5), true, 0.0)?;
    anyhow::ensure!(nfsu2::qol::comparing(), "a click on Compare's right arrow switches it on");
    nfsu2::frontend::settings_page::reset_session(&game.settings);
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 3).context("the Image Quality row")?;
    game.mouse_frame(px(row.1 + row.2 * 0.5), false, 0.0)?;
    game.mouse_frame(px(row.1 + row.2 * 0.5), true, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Options(o)) if o.category == options::Category::Settings(nfsu2::frontend::settings_model::UxGroup::ImageQuality)),
        "a click on Image Quality opens it"
    );
    let before = game.settings.clone();
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 0).context("the first row")?;
    game.mouse_frame(px(row.4.0 + row.4.1 * 0.5), true, 0.0)?;
    anyhow::ensure!(game.settings != before, "a click on the right arrow changes the value");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The performance shop (Wine, perf-shop round): a move over
/// UI_BuyPerformanceParts' third row highlights it (the readouts
/// stay); on the brand screen a click on the right arrow steps the
/// brand back (Performance Exhaust: AEM → the table's last).
pub(super) fn mouse_perf(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.perf_category = 3;
    game.shop.perf_parts = Some(game.perf_parts_list(0));
    game.mode = game.perf_package_menu(0);
    let preview = game.shop.perf_parts.as_ref().map(|l| l.preview);
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 2).context("the third parts row")?;
    let frame = game.ui_frame().context("a frame")?;
    let px = |c: Vec2| {
        Vec2::new(frame.origin.0 + (c.x + 320.0) * frame.scale.0, frame.origin.1 + (c.y + 240.0) * frame.scale.1)
    };
    game.mouse_frame(px(row.1 + row.2 * 0.5), false, 0.0)?;
    anyhow::ensure!(
        game.shop.perf_parts.as_ref().is_some_and(|l| l.selected == 2 && Some(l.preview) == preview),
        "a move selects the third row, the readouts stay"
    );
    let pending = PerfPending::Part(15, true);
    let b = game.brand_select(pending, 0.0).context("the exhaust's brands")?;
    let n = b.rows[0].1.len();
    game.mode = Mode::Menu(Menu::BrandSelect(b, pending));
    game.mouse_record();
    let row = nfsu2::frontend::mouse::slider_rows().into_iter().find(|r| r.0 == 0).context("the brand row")?;
    game.mouse_frame(px(row.4.0 + row.4.1 * 0.5), true, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::BrandSelect(b, _)) if b.rows[0].2 == n - 1),
        "a click on the right arrow steps the brand back"
    );
    game.mouse_record();
    game.mouse_frame(px(row.3.0 + row.3.1 * 0.5), true, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::BrandSelect(b, _)) if b.rows[0].2 == 0),
        "the left arrow forward again"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// UI_Options_PC_Controller (0x4ED140): a move over a key cell
/// selects it, a click on it waits for the key.
pub(super) fn mouse_controls(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::Controls(screens::ControlsScreen::new(0.0)));
    game.mouse_record();
    let cell = game.mouse_at("UI_Options_PC_Controller", nfsu2::frontend::mouse::CONTROL_CELL + 2 * 2 + 1)?;
    game.mouse_frame(cell, false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Controls(c)) if c.selected == 2 && c.column == 1 && !c.header),
        "a move selects the third row's secondary key"
    );
    game.mouse_frame(cell, true, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Controls(c)) if c.capturing),
        "a click on it starts the capture"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The icon-scroller lists (0x54EB90 with +0xA4 / +0xA8 = 0): a move
/// over MC_List's second row selects it; then EA Trax's: a move over
/// the third row selects it, a click on its right arrow steps its
/// state.
pub(super) fn mouse_lists(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::McList(McList::new(McOp::Load, vec!["CLONE".into(), "NAME".into()], 0.0)));
    game.mouse_record();
    let row = game.mouse_at("MC_List", screens::mc_list::NAME + 1)?;
    game.mouse_frame(row, false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McList(l)) if l.selected == 1),
        "a move over MC_List's second row selects it"
    );
    game.mode = Mode::Menu(Menu::Jukebox(screens::Jukebox::new(0.0)));
    game.mouse_record();
    let row = game.mouse_at("EA_TRAX_Jukebox", screens::jukebox::GROUPS[0] + 2)?;
    game.mouse_frame(row, false, 0.0)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Jukebox(j)) if j.selected == 2),
        "a move over the third track selects it"
    );
    let before = game.settings.trax;
    game.mouse_record();
    let right = game.mouse_at("EA_TRAX_Jukebox", screens::jukebox::RIGHT + 2)?;
    game.mouse_frame(right, true, 0.0)?;
    anyhow::ensure!(game.settings.trax[2] != before[2], "a click on its right arrow steps its state");
    // The play-mode arrows (+0x10C / +0x110) flip Random.
    let random = game.settings.trax_random;
    game.mouse_record();
    let arrow = game.mouse_at("EA_TRAX_Jukebox", 0xABA9_3D3F)?;
    game.mouse_frame(arrow, true, 0.0)?;
    anyhow::ensure!(game.settings.trax_random != random, "the play-mode arrow flips Random");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// UI_QRTrackSelect / UI_QRCarSelect's mouse (0x4EFA10 / 0x4FC1F0):
/// the bar's right arrow steps one, a track click right of the handle
/// three; the car select's right filter arrow steps the filter.
pub(super) fn mouse_carlot(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = Mode::Menu(Menu::QuickRace(
        game.quick_track_select(screens::QrMode::Circuit, 0.0).context("no circuit track")?,
    ));
    let track = |g: &Game| match &g.mode {
        Mode::Menu(Menu::QuickRace(q)) => q.selected,
        _ => usize::MAX,
    };
    game.mouse_record();
    let arrow = game.mouse_at("UI_QRTrackSelect", nfsu2::formats::hash::fe_hash("ScrollBar_Arrow_2"))?;
    game.mouse_frame(arrow, true, 0.0)?;
    anyhow::ensure!(track(game) == 1, "the track select's right arrow steps one ({})", track(game));
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let e = game.quick_event(screens::QrMode::Circuit);
    game.fe.help_name = game.profile.take().map(|p| p.name);
    game.mode = game.quick_car_select(Some(e));
    let car = |g: &Game| match &g.mode {
        Mode::Menu(Menu::CarLot(l)) => (l.selected, l.filter),
        _ => (usize::MAX, 0),
    };
    game.mouse_record();
    let back = nfsu2::frontend::mouse::rect_any("UI_QRCarSelect", nfsu2::formats::hash::fe_hash("ScrollBar_Backing"))
        .context("the car bar's backing")?;
    let p = game.mouse_point(back, Vec2::new(0.98, 0.5)).context("a point")?;
    game.mouse_frame(p, true, 0.0)?;
    anyhow::ensure!(car(game).0 == 3, "a track click right of the handle steps three cars ({:?})", car(game));
    let filter = car(game).1;
    game.mouse_record();
    let right = game.mouse_at("UI_QRCarSelect", 0xABA9_3D3F)?;
    game.mouse_frame(right, true, 0.0)?;
    anyhow::ensure!(car(game).1 != filter, "the right filter arrow steps the filter ({filter} → {})", car(game).1);
    Ok(match std::mem::replace(&mut game.mode, Mode::Explore) {
        Mode::Menu(Menu::CarLot(mut lot)) => {
            lot.opened = 0.0;
            Mode::Menu(Menu::CarLot(lot))
        }
        m => m,
    })
}
