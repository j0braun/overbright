//! `--shot` targets: The career shops: car lot, body, paint, rims, specialties, performance and graphics shops.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("parts_bumper", parts_bumper),
    Target::name("parts_hood", parts_hood),
    Target::name("paint_base", paint_base),
    Target::name("paint_engine", paint_engine),
    Target::name("storage", storage),
    Target::name("storage_perf", storage),
    Target::name("storage_pkg", storage),
    Target::name("storage_spec", storage),
    Target::name("bodyshop", bodyshop),
    Target::name("spinners", spinners),
    Target::name("parts", parts),
    Target::name("rims", rims),
    Target::name("rimstyles", rimstyles),
    Target::name("rimsizes", rimsizes),
    Target::name("perf", perf),
    Target::name("graphics", graphics),
    Target::name("graphics_setup", graphics_setup),
    Target::name("vinyl_layers", graphics_setup),
    Target::name("vinyl_colours", graphics_setup),
    Target::name("vinyl_colour_ask", graphics_setup),
    Target::name("vinyls", vinyls),
    Target::name("vinyls_sponsor", vinyls_sponsor),
    Target::name("vinyl_parts", vinyl_parts),
    Target::name("vinyl_ask", vinyl_parts),
    Target::name("decals", decals),
    Target::name("decal_windshield", decal_windshield),
    Target::name("decal_door", decal_door),
    Target::name("decal_parts", decal_parts),
    Target::name("decal_layouts", decal_layouts),
    Target::name("paint", paint),
    Target::name("specialties", specialties),
    Target::name("specialtyparts", specialtyparts),
    Target::name("hydroshop", hydroshop),
    Target::name("specials_hydro3", specials_hydro3_scene),
    Target::name("specials_purge", specials_purge_scene),
    Target::name("customgauges", customgauges),
    Target::name("choosehud", choosehud),
    Target::name("neon", neon),
    Target::name("neon_rating", neon_rating),
    Target::name("audio", audio),
    Target::name("perfpackage", perfpackage),
    Target::name("perfparts_new", perfparts_new),
    Target::name("perfparts", perfparts),
    Target::name("brandselect_part", brandselect_part),
    Target::name("brandselect_pkg", brandselect_part),
    Target::name("perf_part_ask", perf_part_ask),
    Target::name("brandselect", brandselect),
    Target::name("carbon", carbon),
    Target::name("carlot", carlot),
    Target::name("carlot_exit", carlot_exit),
    Target::name("graphics_exit", graphics_exit),
    Target::name("perf_exit", perf_exit),
    Target::name("specialties_exit", perf_exit),
    Target::name("bodyshop_exit", perf_exit),
    Target::name("purge_notice", purge_notice),
    Target::name("hood_notice", purge_notice),
    Target::name("carlot_stock", carlot_stock),
    Target::name("carlot_next", carlot_next),
    Target::name("garage_neon", garage_neon),
    Target::name("cribcars", cribcars),
    Target::name("shop_checks", shop_checks_scene),
];

/// In a category: the slot's view (0x53BA10 → 0x4C2100).
pub(super) fn parts_bumper(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.parts_browser(0, Some(0)))
}

pub(super) fn parts_hood(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let hood = match game.parts_browser(0, None) {
        Mode::Menu(Menu::Parts(b)) => b.categories.iter().position(|c| c.slot == 0x09).unwrap_or(0),
        _ => 0,
    };
    Ok(game.parts_browser(hood, Some(0)))
}

/// UI_Paint in the base paint's swatches (0x55CAD0 → 0x4C2100(0x3F)).
pub(super) fn paint_base(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(shot_paint_slot(game, 0)?)
}

/// UI_Paint on Engine Accents (slot 0x44): the engine view, the
/// hood opening (0x55CC60).
pub(super) fn paint_engine(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(shot_paint_slot(game, 7)?)
}

/// The garage's Parts Storage (Change Parts), its performance
/// categories and packages, and its specialties.
pub(super) fn storage(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    game.mode = game.icon_menu(MenuKind::Crib);
    game.act(Action::ChangeParts, MenuKind::Crib)?;
    match screen {
        "storage_perf" => game.act(Action::CustomizeCategory(1), MenuKind::CustomizeCategory)?,
        "storage_pkg" => {
            game.act(Action::CustomizeCategory(1), MenuKind::CustomizeCategory)?;
            game.act(Action::PerfCategory(nfsu2::performance::CATEGORIES[0].0), MenuKind::PerfCategory)?;
        }
        "storage_spec" => game.act(Action::CustomizeCategory(3), MenuKind::CustomizeCategory)?,
        _ => {}
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn bodyshop(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.parts_browser(0, None))
}

/// The car's spinners owned, so the screen lists them (their cost
/// category's unlock is not met by the shot's profile).
pub(super) fn spinners(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if let Some(car) = game.profile.as_ref().map(|p| p.car.clone()) {
        let mut after = None;
        let mut owned = Vec::new();
        while let Some(x) = game.game_data.parts.find_after(&car, 0x1D, after) {
            after = Some(x);
            if game.game_data.parts.attribute(x, 0xEBB0_3E66) == Some(nfsu2::formats::hash::bin_hash("SPINNER")) {
                owned.push(x.hash);
            }
        }
        if let Some(p) = game.profile.as_mut() {
            p.owned.extend(owned.into_iter().map(|h| (0x1D, h)));
        }
    }
    Ok(game.spinner_browser(None, None))
}

pub(super) fn parts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.parts_browser(0, Some(0)))
}

pub(super) fn rims(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.rim_brand_menu(game.fitted_rim_brand()))
}

pub(super) fn rimstyles(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.rims_browser(0, None, None))
}

pub(super) fn rimsizes(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.rims_browser(0, None, Some(0)))
}

pub(super) fn perf(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.perf_category_menu(0))
}

pub(super) fn graphics(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.icon_menu(MenuKind::Graphics))
}

/// The graphics shop with a vinyl and a paint in the setup.
/// vinyl_colour_ask: Select on the swatch, "Would you like to add
/// this vinyl to your setup?" over UI_Paint's DIM (Wine menus103
/// vinyl_colour_ask).
pub(super) fn graphics_setup(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
    let vinyl = game.game_data.parts.find_after(&car, 0x40, None).map(|p| p.hash).context("a vinyl")?;
    let paint = game.game_data.parts.find_after(&car, 0x3F, None).map(|p| p.hash).context("a paint")?;
    game.shop.setup.put_vinyl(0x40, vinyl, false, false);
    game.shop.setup.put(0x3F, paint);
    game.shop.vinyl = Some(vinyl);
    Ok(match screen {
        "vinyl_layers" => game.vinyl_layer_menu(0),
        // One colour: the swatches open on the slot's (stock) colour.
        // The swatches' view requests (0x55CAD0 / 0x55D670 →
        // 0x4C2100 with a colour slot 0x4D.., which has no record)
        // leave the view the vinyl list set (Wine menus113 a17 /
        // a18), so the shot stands on the vinyl's view, settled.
        "vinyl_colours" | "vinyl_colour_ask" => {
            let group = game.car_part(0x40, vinyl).map(|p| p.kind);
            let mut view = nfsu2::garage::View::new(0);
            view.set_slot(0x40, false, false, group, -60.0, 1.0 / 60.0);
            game.garage_fe.view = Some((game.frontend_scene(), Some(GarageViewRequest::Slot(0x40, group)), view));
            let m = game.vinyl_colours(0);
            if screen == "vinyl_colour_ask" {
                game.mode = m;
                // Enough bank for the question (else the bank
                // notice), and the garage as the screen left it.
                if let Some(p) = game.profile.as_mut() {
                    p.cash = p.cash.max(100_000);
                }
                let _ = game.garage_request();
                if let Mode::Menu(Menu::Paint(p)) = &game.mode
                    && p.selected.is_none()
                {
                    game.menu_key(Key::Accept)?;
                }
                game.menu_key(Key::Accept)?;
                anyhow::ensure!(
                    matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.dim_under.is_some()),
                    "the colour question over UI_Paint's DIM"
                );
                std::mem::replace(&mut game.mode, Mode::Explore)
            } else {
                m
            }
        }
        _ => game.graphics_menu(0),
    })
}

pub(super) fn vinyls(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.vinyl_browser(0x40, None, None))
}

/// The Sponsor group's list (NFSU2_CAT=26: Contest) with every code
/// typed at the title (`NFSU2_CHEATS`, default all).
pub(super) fn vinyls_sponsor(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if std::env::var("NFSU2_CHEATS").is_err() {
        type_cheats("all");
    }
    let cat = std::env::var("NFSU2_CAT").ok().and_then(|v| v.parse().ok()).unwrap_or(27);
    Ok(game.vinyl_browser(0x40, Some(cat), Some(0)))
}

/// NFSU2_CAT=c / NFSU2_PART=p: another group / vinyl (default
/// Flames, its first).
/// `vinyl_ask`: Select on it, the setup question (0x559250); the
/// groups open with NFSU2_UNLOCK_ALL=1 (as the Wine capture's
/// poked unlock-all byte, menus103).
pub(super) fn vinyl_parts(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    // A fresh career has every vinyl group locked: their cost
    // levels unlock by a shop (0x515360 type 1) that is announced
    // and so open only once driven into (0x513110). The question
    // needs an open group, so the shot drives into the shops of
    // the career's stage (0x512FF0's visit flag), as the selftest
    // reaches the graphics shop.
    if screen == "vinyl_ask"
        && let Some(p) = game.profile.as_mut()
    {
        for shop in game.game_data.career.shops.iter() {
            p.visit_shop(shop);
        }
    }
    game.mode = game.vinyl_browser(0x40, Some(env("NFSU2_CAT", 9)), Some(env("NFSU2_PART", 0)));
    if screen == "vinyl_ask" {
        // The group locked in this career: the first open one.
        if let Mode::Menu(Menu::Parts(b)) = &mut game.mode
            && b.categories.get(b.category).is_none_or(|c| c.locked)
            && let Some(open) = b.categories.iter().position(|c| !c.locked)
        {
            b.category = open;
            b.part = None;
        }
        if let Some(p) = game.profile.as_mut() {
            p.cash = p.cash.max(100_000);
        }
        let _ = game.garage_request();
        // Into the group's list, then Select on its vinyl (as the
        // selftest's keys).
        for _ in 0..2 {
            if !matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
                game.menu_key(Key::Accept)?;
            }
        }
        anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "the vinyl question");
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn decals(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.decal_shop(0, 0, None))
}

/// The Windshield's decals, on "No Decal".
pub(super) fn decal_windshield(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.decal_shop(0, 0, Some(0)))
}

pub(super) fn decal_door(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.decal_node = 0x10001;
    Ok(game.decal_shop(0x10001, 0, None))
}

pub(super) fn decal_parts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.decal_node = 0x10001;
    Ok(game.decal_shop(0x10001, 1, Some(1)))
}

pub(super) fn decal_layouts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.decal_node = 0x10003;
    Ok(game.decal_shop(0x10003, 0, None))
}

pub(super) fn paint(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(shot_paint_slot(game, 0)?)
}

pub(super) fn specialties(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.specialties_browser(0, None))
}

pub(super) fn specialtyparts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.specialties_browser(2, Some(0)))
}

/// Hydraulics (slot 0xA1) on its second part, with a level-2 part
/// fitted (`install_shot_hydraulics`): the FE car's garage motion.
pub(super) fn hydroshop(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    // The category is a career unlock: every part listed.
    career::UNLOCK_ALL.store(true, std::sync::atomic::Ordering::Relaxed);
    install_shot_hydraulics(game);
    Ok(game.specialties_browser(5, Some(1)))
}

/// A player report (finished career: "level 3 hydraulics in the shop not
/// working, also nitro purge"): `finished_career_specials_hydro3` buys and
/// fits LVL 3 HYDRAULICS at IND_CAR_SPECIALTY_2 and shows its list
/// with the car pumping (mode 3, 0x77D010's record);
/// `finished_career_specials_purge` the Nitrous Purge list with Test N2O
/// Purge on (C), after buying and fitting a purge.
pub(super) fn specials_hydro3_scene(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(specials_hydro3(game)?)
}

pub(super) fn specials_purge_scene(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(specials_purge(game)?)
}

/// Custom Gauges: the list on its second gauge with
/// UI_ChooseCustomHUD's preview; `choosehud` its colour steps
/// (NFSU2_HUD_STEP 0..2) with the swatch moved right three times.
pub(super) fn customgauges(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.specialties_browser(0, Some(1)))
}

pub(super) fn choosehud(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let step: usize = std::env::var("NFSU2_HUD_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(0).min(2);
    if let Mode::Menu(m) = game.specialties_browser(0, Some(1)) {
        game.fe.hud_overlay = Some(Box::new(m));
    }
    let mut mode = game.choose_custom_hud(0, 1);
    if let Mode::Menu(Menu::ChooseHud(c)) = &mut mode {
        for _ in 0..step {
            c.key(Key::Accept, 0.0);
        }
        for _ in 0..3 {
            c.key(Key::Right, 0.0);
        }
    }
    Ok(mode)
}

pub(super) fn neon(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if std::env::var("NFSU2_NEON").is_ok() {
        install_shot_neon(game);
    }
    // NFSU2_PART=p: another row (default "NO NEON").
    Ok(game.neon_browser(0, Some(std::env::var("NFSU2_PART").ok().and_then(|v| v.parse().ok()).unwrap_or(0))))
}

/// A step down the neon list (NO NEON → the first colour): the
/// Visual Rating widget pushed by 0x55CAD0 → 0x55C180 (Wine
/// menus103 neon_cust: 0.14 on RED).
pub(super) fn neon_rating(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.neon_browser(0, Some(0));
    game.menu_key(Key::Down)?;
    anyhow::ensure!(game.fe.overlay_rating == Some("UI_CustomNeonMain"), "the neon step's Visual Rating widget");
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn audio(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.audio_browser(false, 1, Some(0)))
}

pub(super) fn perfpackage(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.perf_parts = Some(game.perf_parts_list(0));
    Ok(game.perf_package_menu(1))
}

/// Choose Individual Parts with UI_BuyPerformanceParts over it: one
/// engine part fitted, one bought, the third row highlighted.
/// As reached on a new car: nothing fitted, the first row.
pub(super) fn perfparts_new(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.shop.perf_parts = Some(game.perf_parts_list(0));
    Ok(game.perf_package_menu(0))
}

pub(super) fn perfparts(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if let Some(p) = game.profile.as_mut() {
        p.cash = 12_000;
        p.perf.insert(16);
        p.perf_owned.extend([16, 17]);
    }
    game.shop.perf_parts = Some(game.perf_parts_list(2));
    Ok(game.perf_package_menu(0))
}

/// UI_PerformanceBrandSelect on buying the category's level-3
/// package, the second row highlighted.
/// The brand screen for one part (Replace Headers, id 17: HP
/// Racing first) and for the Pro package (two branded rows, the
/// second highlighted), as reached under Wine (perf-shop round).
pub(super) fn brandselect_part(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    game.shop.perf_category = 3;
    game.shop.perf_parts = Some(game.perf_parts_list(0));
    let pending =
        if screen == "brandselect_part" { PerfPending::Part(17, true) } else { PerfPending::Package(2, true) };
    let mut b = game.brand_select(pending, 0.0).context("a branded purchase")?;
    b.selected = if screen == "brandselect_pkg" { 1.min(b.rows.len() - 1) } else { 0 };
    Ok(Mode::Menu(Menu::BrandSelect(b, pending)))
}

/// "Would you like to purchase this part?" over the parts list
/// (Replace Headers highlighted, No focused).
pub(super) fn perf_part_ask(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if let Some(p) = game.profile.as_mut() {
        p.cash = p.cash.max(12_000);
    }
    game.shop.perf_category = 3;
    game.shop.perf_parts = Some(game.perf_parts_list(1));
    game.mode = game.perf_package_menu(0);
    // A frame's garage first (the scene stays under the dialog).
    let _ = game.garage_request();
    Ok(game.perf_part_accept())
}

pub(super) fn brandselect(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let pending = PerfPending::Package(3, true);
    let mut b = game.brand_select(pending, 0.0).context("a branded package")?;
    b.selected = 1.min(b.rows.len() - 1);
    Ok(Mode::Menu(Menu::BrandSelect(b, pending)))
}

pub(super) fn carbon(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.parts_browser_in(Some(0x5E), 0, None))
}

pub(super) fn carlot(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    Ok(game.career_lot())
}

/// Back in the career car lot: "exit back to Explore Mode?" over
/// the lot (menus107 c24).
pub(super) fn carlot_exit(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.career_lot();
    game.garage_request();
    game.menu_key(Key::Back)?;
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// Back in the career performance shop / UI_ICEMAIN: "exit back to
/// Explore Mode?" over the screen, DIM from the ask (0x55C715 /
/// 0x568B66).
/// `bodyshop_exit`: the body shop's (0x56721E), the browser under it
/// fading out by MENUFADEOUT, its strip items hidden.
/// Customize's graphics shop left with a changed setup: "…install
/// your new graphics setup?" over the dimmed shop on Decals (Wine
/// menus103 `graphics_exit`).
pub(super) fn graphics_exit(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
    let mut after = None;
    while let Some(p) = game.game_data.parts.find_after(&car, 0x40, after) {
        after = Some(p);
        if p.kind == 8 {
            game.shop.setup.put(0x40, p.hash);
            break;
        }
    }
    game.mode = game.graphics_menu(2);
    game.garage_request();
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.under.is_some() && d.selected == 1),
        "the setup question over the shop"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

pub(super) fn perf_exit(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    game.mode = match screen {
        "perf_exit" => game.perf_category_menu(0),
        "bodyshop_exit" => game.parts_browser(0, None),
        _ => game.specialties_browser(0, None),
    };
    // The shop's garage up first, as in play (a dialog keeps the
    // scene it opened over).
    game.garage_request();
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.dim_under.is_some() && d.under.is_some() && d.selected == 1),
        "the shop's exit question over its dimmed screen"
    );
    anyhow::ensure!(
        screen != "bodyshop_exit" || matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.dim_scripts.is_some()),
        "the body shop's MENUFADEOUT"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// UI_ICEMAIN's notices: Nitrous Purge without nitrous (0x20B30CD8)
/// and Split Hoods over a hood decal (0x903854F0, the list under).
pub(super) fn purge_notice(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { screen, .. } = cx;
    let slot = if screen == "purge_notice" { 0xA2 } else { 9 };
    if slot == 9 {
        let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
        let decal = game.game_data.parts.find_after(&car, 0x61, None).map_or(1, |p| p.hash);
        if let Some(p) = game.profile.as_mut() {
            p.installed.insert(0x61, decal);
        }
    }
    let at = screens::SPECIALTIES.iter().position(|c| c.0 == slot).unwrap_or(0);
    game.mode = game.specialties_browser(at, None);
    // The item open as later in the career (the shot's profile has
    // nothing beyond stock unlocked there).
    if let Mode::Menu(Menu::Parts(b)) = &mut game.mode
        && let Some(c) = b.categories.get_mut(at)
    {
        c.locked = false;
    }
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.dim_under.is_some()),
        "the notice over UI_ICEMAIN, dimmed"
    );
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// Accept in the career car lot: "Do you want this stock %s?" with
/// the lot's DIM (gone after 100 ms; menus107 c14).
pub(super) fn carlot_stock(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.career_lot();
    game.garage_request();
    game.menu_key(Key::Accept)?;
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut game.mode {
        d.opened -= 1.0;
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// Right in the car lot: the platform turning to the next car
/// (0x4D82D0 state 2; NFSU2_SHOT_MS under 500 shows it turning).
pub(super) fn carlot_next(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    game.mode = game.career_lot();
    game.garage_request();
    if let Mode::Menu(Menu::CarLot(lot)) = &mut game.mode {
        lot.selected = (lot.selected + 1) % lot.cars.len().max(1);
    }
    // The step, then the lot's 0.3 s wait before it asks for the
    // car (the swing starts then; the shot's clock stays there).
    game.garage_request();
    game.pacing.clock = Some(f64::from(game.now()) / 1000.0 + f64::from(nfsu2::audio::fe_rules::CRIB_SWING_DELAY));
    game.garage_request();
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// NFSU2_VINYL=1: the profile's car with `vinyl_car`'s vinyl, for
/// the slot's look in the preview.
/// The crib's Car Select on the finished-career save's Corolla (career slot 0, WHITE
/// PULSE underglow, BLUE PULSE engine and trunk neon).
/// NFSU2_SHOT_SLOT=i: that slot's car highlighted instead.
pub(super) fn garage_neon(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    let slot = game.profile.as_ref().map(|p| p.slot);
    let mut mode = game.own_cars(None);
    if let Mode::Menu(Menu::CarLot(lot)) = &mut mode {
        lot.selected = lot.cars.iter().position(|c| c.slot == slot).unwrap_or(0);
        lot.opened = 0.0;
    }
    Ok(mode)
}

pub(super) fn cribcars(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    if std::env::var_os("NFSU2_VINYL").is_some() {
        install_shot_vinyl(game);
    }
    Ok(game.own_cars(None))
}

/// The shops' previews and engage dialogs checked (`shop_checks`;
/// with `finished_career_` on the finished career), then the parts browser.
pub(super) fn shop_checks_scene(game: &mut Game, _cx: &ShotCtx) -> Result<Mode> {
    shop_checks(game)?;
    game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == "IND_BODYSHOP_2").map(|s| s.hash);
    Ok(game.parts_browser(0, None))
}
