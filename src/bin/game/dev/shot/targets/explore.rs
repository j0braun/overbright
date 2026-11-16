//! `--shot` targets: Free roam: pause menus, markers, shop entries, the drive itself, traffic and outruns.
//! One scene setup per target; the table is `TARGETS`.
use super::*;

/// This group's targets, in the old match's order.
pub(super) const TARGETS: &[Target] = &[
    Target::name("sms_tip", sms_tip),
    Target::name("sms", sms),
    Target::name("sms_rest", sms),
    Target::name("smsbox", sms),
    Target::name("smsread", sms),
    Target::name("pause", pause),
    Target::name("pause_quit", pause),
    Target::name("pause_undim", pause),
    Target::name("pause_options", pause),
    Target::name("pause_audio", pause),
    Target::name("pause_player", pause),
    Target::name("engage", pause),
    Target::name("reward", pause),
    Target::name("shop", pause),
    Target::name("ingamemap", pause),
    Target::name("status", pause),
    Target::name("status_region", pause),
    Target::name("status_dvd", pause),
    Target::name("challenge", pause),
    Target::name("shoot_dialog", pause),
    Target::name("challenge_retry", pause),
    Target::name("challenge_hud", pause),
    Target::name("showcase", pause),
    Target::name("showcase_picture", pause),
    Target::name("showcase_cycle", pause),
    Target::name("showcase_showoff", pause),
    Target::name("cash_popup", pause),
    Target::name("unique_part", pause),
    Target::name("unique_perf", pause),
    Target::name("mouse_map", pause),
    Target::name("mouse_map_gps", pause),
    Target::name("mouse_status", pause),
    Target::name("marker", marker),
    Target::name("prompt", marker),
    Target::name("shopmarker", marker),
    Target::name("shopenter", shopenter),
    Target::name("explore", explore),
    Target::name("race", explore),
    Target::name("vinyl_car", explore),
    Target::name("neon_car", explore),
    Target::name("hood", explore),
    Target::name("bumper", explore),
    Target::name("hydro", explore),
    Target::name("hydro_bounce", explore),
    Target::name("traffic_close", traffic_close),
    Target::name("outrun", outrun),
    Target::name("encounter", outrun),
    Target::name("outrun_won", outrun),
];

/// SMS: `sms` a message arriving in free roam (the HUD's mail
/// flashing; NFSU2_SHOT_MS under 1000 its state 2, else 3),
/// `sms_rest` the envelope resting after it; `smsbox` the mailbox
/// and `smsread` a message.
/// `sms_tip`: the car set in an engage tip's trigger (kind 0x12, the
/// nearest to the garage, FC25E6EE at (619, −175)); the tip's
/// message arrives (0x529C50) and the SMS slot flashes
/// (NFSU2_SHOT_MS: under 1000 its state 2, then 3, from 4000 at
/// rest), its sounds FE ids 16 then 15 (NFSU2_AUDIO_DEBUG prints them).
pub(super) fn sms_tip(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    let tip = game.sim.route_triggers.tips().find(|t| t.name == 0xFC25_E6EE).map(|t| t.position).context("the tip")?;
    game.explore(Some((tip, 0.0)))?;
    let unread = game.profile.as_ref().map_or(0, |p| p.inbox.unread());
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
    }
    let p = game.profile.as_ref().context("a career")?;
    anyhow::ensure!(p.inbox.unread() == unread + 1, "the tip's message arrived");
    finish_scene(game, gpu, format);
    game.pacing.started = Instant::now();
    for &t in &[0.0, 1.0] {
        game.overlay_quads(t);
    }
    println!(
        "sms_tip: unread {} cash {} slot {:?}",
        game.profile.as_ref().map_or(0, |p| p.inbox.unread()),
        game.profile.as_ref().map_or(0, |p| p.cash),
        game.hud_state.hud.sms_state()
    );
    Ok(Mode::Explore)
}

pub(super) fn sms(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    game.explore(None)?;
    if let Some(p) = game.profile.as_mut() {
        // SMS_INSTRUCTION (condition 3) comes at 10 s of driving.
        p.drive_time = 9.9;
    }
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
    }
    finish_scene(game, gpu, format);
    game.pacing.started = Instant::now();
    Ok(match screen {
        "sms" | "sms_rest" => {
            // The HUD frames before the shot's: the SMS element
            // queues the slot, the next frame starts it flashing.
            let frames: &[f32] = if screen == "sms" { &[0.0, 1.0] } else { &[0.0, 1.0, 2000.0] };
            for &t in frames {
                game.overlay_quads(t);
            }
            Mode::Explore
        }
        _ => {
            let messages = game.game_data.career.messages.clone();
            let p = game.profile.as_mut().context("a career")?;
            // Sample messages, not with a real save
            // (NFSU2_SHOT_SAVE: its own inbox, to compare with the
            // original's).
            let samples: &[&str] = if std::env::var_os("NFSU2_SHOT_SAVE").is_some() {
                &[]
            } else {
                &["SMS_MINIMAP_INSTRUCTION", "SMS_UNLOCK_01", "ENGAGE_TIPS_GPS", "SMS_OUTRUN_VICTORY"]
            };
            for &name in samples {
                if let Some(m) = messages.iter().find(|m| m.name == name) {
                    p.inbox.add(m.hash);
                }
            }
            let first = p
                .inbox
                .entries
                .iter()
                .position(|e| messages.iter().any(|m| m.hash == e.message && m.name == "SMS_INSTRUCTION"))
                .unwrap_or(0);
            if !samples.is_empty() || screen == "smsread" {
                p.inbox.read(first);
            }
            if screen == "smsread" {
                Mode::Menu(Menu::SmsViewer(nfsu2::frontend::sms::Viewer::new(
                    &game.game_data.assets,
                    &p.inbox,
                    &messages,
                    first,
                    true,
                    1,
                    0.0,
                )))
            } else {
                Mode::Menu(Menu::SmsMailbox(nfsu2::frontend::sms::Mailbox::new(&p.inbox, &messages, 1, None, 0.0)))
            }
        }
    })
}

pub(super) fn pause(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, env, .. } = cx;
    game.explore(None)?;
    for _ in 0..30 {
        game.advance_for(1.0 / 30.0)?;
    }
    finish_scene(game, gpu, format);
    // The menu opens at the front end's time 0, so NFSU2_SHOT_MS
    // is its time since opening (loading the world took a while).
    game.pacing.started = Instant::now();
    Ok(match screen {
        "pause" => game.icon_menu(MenuKind::ExplorePause),
        // Quit to Main Menu's UI_InGameDialog over the pause.
        "pause_quit" => {
            game.mode = game.icon_menu(MenuKind::ExplorePause);
            game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        // Cancel on "Quit to Main Menu?": the pause's UNDIM,
        // NFSU2_SHOT_MS after the answer (the DIM a second before).
        "pause_undim" => {
            game.mode = game.icon_menu(MenuKind::ExplorePause);
            game.act(Action::QuitToMainMenu, MenuKind::ExplorePause)?;
            game.track_undim();
            game.menu_key(Key::Back)?;
            game.track_undim();
            let u = game.fe.undim.as_mut().context("the pause's UNDIM")?;
            u.dim_at = -1000.0;
            u.undim_at = Some(0.0);
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        "pause_options" => game.icon_menu(MenuKind::PauseOptions(false)),
        "pause_audio" | "pause_player" => {
            let c = if screen == "pause_audio" { options::Category::Audio } else { options::Category::Player };
            Mode::Menu(Menu::Options(options::OptionsScreen::new_in(
                c,
                &game.settings,
                FORCE_FEEDBACK,
                Some(game.pause_context(false)),
                0.0,
            )))
        }
        // The World Map key's UI_InGame_WorldMap; NFSU2_MAP_ZOOM
        // picks the zoom record and NFSU2_MAP_SELECT steps the
        // selection right that many times.
        "ingamemap" => {
            game.mode = Mode::Explore;
            let mut m = game.in_game_map().context("the in-game map (a career)")?;
            if let Mode::Menu(Menu::InGameMap(map)) = &mut m {
                let zoom: usize = std::env::var("NFSU2_MAP_ZOOM").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                map.zoom = zoom.min(map.max_zoom);
                let steps: usize = std::env::var("NFSU2_MAP_SELECT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                for _ in 0..steps {
                    map.key(Key::Right, game.game_data.assets.package("UI_InGame_WorldMap"));
                }
            }
            m
        }
        // UI_InGame_WorldMap's zones (0x4CBEE0): a move onto a legend
        // row focuses it, off every zone drops the focus; a click on
        // the left zoom arrow zooms (0x4CBDA0).
        "mouse_map" => {
            game.mode = Mode::Explore;
            game.mode = game.in_game_map().context("the in-game map (a career)")?;
            let map = |g: &Game| match &g.mode {
                Mode::Menu(Menu::InGameMap(m)) => (m.focus, m.zoom),
                _ => (None, usize::MAX),
            };
            game.mouse_record();
            let row = game.mouse_at("UI_InGame_WorldMap", nfsu2::formats::hash::fe_hash("ICON_BACKING_1"))?;
            game.mouse_frame(row, false, 0.0)?;
            anyhow::ensure!(map(game).0 == Some(0), "a move onto the first legend row focuses it ({:?})", map(game));
            let frame = game.ui_frame().context("a frame")?;
            game.mouse_frame(Vec2::new(frame.origin.0 + 4.0, frame.origin.1 + 4.0), false, 0.0)?;
            anyhow::ensure!(map(game).0.is_none(), "off every zone the focus drops");
            let zoom = map(game).1;
            game.mouse_record();
            let left = game.mouse_at("UI_InGame_WorldMap", 0xCF6F_0439)?;
            game.mouse_frame(left, true, 0.0)?;
            anyhow::ensure!(map(game).1 != zoom, "the left arrow zooms");
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        // Setting a destination on UI_InGame_WorldMap with the mouse
        // (0x4CBEE0 → 0x4CBE70 kind 0 → 0x4B10C0; checked in the
        // original under Wine). The map has no free waypoint:
        // the GPS goal (0x413120, only from 0x529490) is always a
        // marker entry's GPS point. With the GPS on (a new career
        // aims it at the car lot), a click on a marker asks
        // "Deactivating GPS"; OK clears it. A click on the map away
        // from every marker does nothing. A click on a marker asks
        // "Are you sure you want to engage the GPS system?"; Yes
        // engages it on that marker and the map closes; the route
        // search then finds its path. The shot is free roam with the
        // GPS on that marker (the HUD arrow and the minimap route).
        "mouse_map_gps" => {
            const PKG: &str = "UI_InGame_WorldMap";
            const TRACK2000_MAP: u32 = 0x90FE_80F3;
            game.mode = Mode::Explore;
            let open = |g: &mut Game| -> anyhow::Result<()> {
                g.mode = g.in_game_map().context("the in-game map (a career)")?;
                g.mouse_record();
                Ok(())
            };
            // A marker on the map: its index, target and window point.
            let marker = |g: &Game| -> anyhow::Result<(usize, u32, nfsu2::frontend::career::Trigger, Vec2)> {
                let Mode::Menu(Menu::InGameMap(m)) = &g.mode else { anyhow::bail!("not on the map") };
                let pkg = g.game_data.assets.package(PKG).context("UI_InGame_WorldMap")?;
                for (i, h) in m.mouse_markers(pkg) {
                    if let Some((t, trig)) = m.icons[i].target {
                        return Ok((i, t, trig, g.mouse_at(PKG, h)?));
                    }
                }
                anyhow::bail!("no marker on the map")
            };
            open(game)?;
            if game.career_world.gps.as_ref().is_some_and(|g| g.on()) {
                let (_, _, _, p) = marker(game)?;
                game.mouse_frame(p, true, 0.0)?;
                anyhow::ensure!(
                    matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("Deactivating")),
                    "a marker click with the GPS on: Deactivating GPS"
                );
                game.menu_key(Key::Accept)?;
                anyhow::ensure!(
                    game.career_world.gps.is_none() && matches!(&game.mode, Mode::Menu(Menu::InGameMap(m)) if !m.gps),
                    "the GPS off, back on the map"
                );
                game.mouse_record();
            }
            // A point of the map away from every marker (the
            // candidate farthest from the markers' centres).
            let markers: Vec<Vec2> = match &game.mode {
                Mode::Menu(Menu::InGameMap(m)) => {
                    let pkg = game.game_data.assets.package(PKG).context("UI_InGame_WorldMap")?;
                    m.mouse_markers(pkg).into_iter().filter_map(|(_, h)| game.mouse_at(PKG, h).ok()).collect()
                }
                _ => anyhow::bail!("not on the map"),
            };
            let rect = nfsu2::frontend::mouse::rect_any(PKG, TRACK2000_MAP).context("the map object laid out")?;
            let empty = (1..10)
                .flat_map(|a| (1..10).map(move |b| Vec2::new(a as f32 / 10.0, b as f32 / 10.0)))
                .filter_map(|f| game.mouse_point(rect, f))
                .max_by(|a, b| {
                    let d = |p: &Vec2| markers.iter().map(|m| m.distance(*p)).fold(f32::MAX, f32::min);
                    d(a).total_cmp(&d(b))
                })
                .context("a point on the map")?;
            game.mouse_frame(empty, true, 0.0)?;
            game.mouse_frame(empty, false, 0.0)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::InGameMap(_))) && game.career_world.gps.is_none(),
                "a click away from the markers does nothing ({})",
                flow_mode(&game.mode)
            );
            let (i, target, trigger, p) = marker(game)?;
            game.mouse_record();
            game.mouse_frame(p, true, 0.0)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("engage the GPS")),
                "a marker click: the GPS question ({})",
                flow_mode(&game.mode)
            );
            answer_yes(game)?;
            anyhow::ensure!(matches!(game.mode, Mode::Explore), "Yes closes the map");
            anyhow::ensure!(
                game.career_world.gps.as_ref().is_some_and(|g| g.target == target && g.at == trigger.gps && g.on()),
                "the GPS on the clicked marker"
            );
            for _ in 0..90 {
                game.advance_for(1.0 / 30.0)?;
            }
            let state = game.career_world.gps.as_ref().map(|g| (g.state(), g.error()));
            anyhow::ensure!(
                game.career_world.gps.as_ref().is_some_and(|g| g.on() && g.error().is_none()),
                "the route to the clicked marker ({state:?})"
            );
            println!("mouse_map_gps: marker {i} {target:08X} at {:?}, GPS {state:?}", trigger.gps);
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        // UI_Status_Master (0x4B1F20): its right arrow turns the page.
        "mouse_status" => {
            game.mode = Mode::Explore;
            game.mode = game.status_screen().context("the status screen (a career)")?;
            game.mouse_record();
            let right = game.mouse_at("UI_Status_Master", 0x4449_69FE)?;
            game.mouse_frame(right, true, 0.0)?;
            anyhow::ensure!(
                matches!(&game.mode, Mode::Menu(Menu::Status(st)) if st.screen.page == 1),
                "the right arrow turns to the second page"
            );
            if let Mode::Menu(Menu::Status(st)) = &mut game.mode {
                st.screen.opened = 0.0;
            }
            std::mem::replace(&mut game.mode, Mode::Explore)
        }
        // The Career Statistics key's UI_Status_Master on its first,
        // second or third page; NFSU2_STATUS_SCROLL scrolls the
        // career page down that many rows.
        "status" | "status_region" | "status_dvd" => {
            game.mode = Mode::Explore;
            let mut m = game.status_screen().context("the status screen (a career)")?;
            if let Mode::Menu(Menu::Status(st)) = &mut m {
                let rows = st.rows.len();
                let page = match screen {
                    "status_region" => 1,
                    "status_dvd" => 2,
                    _ => 0,
                };
                for _ in 0..page {
                    st.screen.key(Key::Right, rows);
                }
                let scroll: usize = std::env::var("NFSU2_STATUS_SCROLL").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                for _ in 0..scroll {
                    st.screen.key(Key::Down, rows);
                }
                st.screen.opened = 0.0;
            }
            m
        }
        "engage" => {
            // NFSU2_SHOT_EVENT=<name>: that event's dialog.
            let named =
                std::env::var("NFSU2_SHOT_EVENT").ok().and_then(|n| game.game_data.career.events.iter().position(|e| e.name == n));
            let e = match named {
                Some(e) => e,
                None => game.nearest_event().context("an event")?.0,
            };
            game.game_data.assets.track_map(&game.game_data.root, game.game_data.career.events[e].track);
            let purse = game
                .profile
                .as_ref()
                .map_or(game.game_data.career.events[e].reward, |p| p.race_bank(&game.game_data.career, &game.game_data.career.events[e]));
            Mode::Menu(Menu::Engage(Engage { event: e, message: 3, opened: 0.0, focus: Default::default(), purse }))
        }
        // NFSU2_SHOT_SHOP=<name>: that shop's dialog.
        "shop" => {
            let name = std::env::var("NFSU2_SHOT_SHOP").unwrap_or_else(|_| "CC_BODYSHOP_1".into());
            let shop = game.game_data.career.shops.iter().position(|s| s.name == name).context("a body shop")?;
            Mode::Menu(Menu::ShopEngage(ShopEngage { shop, opened: 0.0, focus: Default::default() }))
        }
        // UI_ChooseUniquePart for STAGE_4_VISUAL_3 (its goal shop
        // reached, state 4): the unique spoilers; `unique_perf`
        // STAGE_3_PERF_1's performance uniques.
        "unique_part" | "unique_perf" => {
            let name = if screen == "unique_part" { "STAGE_4_VISUAL_3" } else { "STAGE_3_PERF_1" };
            let index = game
                .game_data.career
                .mag_triggers
                .iter()
                .position(|t| t.name == nfsu2::formats::hash::bin_hash(name))
                .context("the challenge")?;
            game.career_world.challenge = Some(WorldChallenge { index, time: 0.0, state: 4, failed: false, heading: 0.0 });
            game.unique_part_screen()
        }
        // A world challenge (STAGE_2_SHOW_1): `challenge` its
        // UI_EngageEventDialog, `challenge_retry` the retry after
        // its time ran out, `challenge_hud` free roam with its
        // clock at 9.5 s (Animate_RED).
        // UI_ShowCase_Preview for STAGE_2_SHOW_1_PICTURE (kind 1):
        // the menu, or `showcase_picture` its picture state.
        "showcase" | "showcase_picture" | "showcase_cycle" | "showcase_showoff" => {
            let i = game.game_data.career.milestones.iter().position(|m| m.kind == 1).context("a kind-1 shoot")?;
            let mut v = game.showcase_preview(i, false, 0.0);
            // `showcase_picture` its picture state;
            // `showcase_cycle` the set's next camera,
            // `showcase_showoff` Show Off with the hood and doors
            // open (NFSU2_SHOT_MS from their opening).
            match screen {
                "showcase_picture" => {
                    let k = v.key(Key::Accept, 0.0);
                    game.showcase_key(&mut v, k);
                }
                "showcase_cycle" => {
                    v.option = 2;
                    let k = v.key(Key::Accept, 0.0);
                    game.showcase_key(&mut v, k);
                }
                "showcase_showoff" => {
                    v.option = 1;
                    for key in [Key::Accept, Key::Up, Key::Left] {
                        let k = v.key(key, 0.0);
                        game.showcase_key(&mut v, k);
                    }
                }
                _ => {}
            }
            game.mode = Mode::Menu(Menu::Showcase(v.clone()));
            let at = env.ms.unwrap_or(5000.0);
            game.showcase_frame(screens::ShowcaseMode::Menu, v.picks.camera, Controls::default(), 0.0, at / 1000.0);
            Mode::Menu(Menu::Showcase(v))
        }
        // The photo shoot's cash popup (0x4AC170 → 0x4A9FF0) on the
        // free-roam HUD.
        "cash_popup" => {
            game.hud_state.cash_popup = Some((3, 1500, 1));
            game.mode = Mode::Explore;
            // Its ZoomIn (1.8 s) 600 ms in at the shot's time.
            let at = env.ms.unwrap_or(5000.0);
            game.overlay_quads(at - 600.0);
            Mode::Explore
        }
        // A DVD's photo-shoot dialog (UI_EngageEventDialog's
        // milestone branch; under Wine "From: Carol").
        "shoot_dialog" => Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
            kind: screens::ChallengeDialogKind::Milestone {
                milestone: nfsu2::formats::hash::bin_hash("STAGE_2_DVD_1"),
            },
            opened: 0.0,
            focus: Default::default(),
            from: "Carol".into(),
        })),
        "challenge" | "challenge_retry" | "challenge_hud" => {
            let index = game
                .game_data.career
                .mag_triggers
                .iter()
                .position(|t| t.name == nfsu2::formats::hash::bin_hash("STAGE_2_SHOW_1"))
                .context("STAGE_2_SHOW_1")?;
            match screen {
                "challenge" => Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
                    kind: screens::ChallengeDialogKind::Offer { challenge: index, message: 2 },
                    opened: 0.0,
                    focus: Default::default(),
                    from: "Lola".into(),
                })),
                "challenge_retry" => Mode::Menu(Menu::Challenge(screens::ChallengeDialog {
                    kind: screens::ChallengeDialogKind::Retry,
                    opened: 0.0,
                    focus: Default::default(),
                    from: String::new(),
                })),
                _ => {
                    game.career_world.challenge = Some(WorldChallenge { index, time: 9.5, state: 1, failed: false, heading: 0.0 });
                    if let Some(p) = game.profile.as_mut() {
                        p.stage = 2;
                    }
                    for &t in &[0.0, 1.0] {
                        game.overlay_quads(t);
                    }
                    Mode::Explore
                }
            }
        }
        _ => Mode::Menu(Menu::Reward(Reward { reputation: 250, bank: 300, name: "Player".into(), opened: 0.0 })),
    })
}

pub(super) fn marker(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, env, .. } = cx;
    game.explore(None)?;
    let (_, t) = game
        .career_world.event_starts
        .as_ref()
        .and_then(|(_, s)| s.first().copied())
        .or_else(|| {
            game.refresh_event_triggers();
            game.career_world.event_starts.as_ref().and_then(|(_, s)| s.first().copied())
        })
        .context("an open event trigger")?;
    let back = if screen == "marker" { 20.0 } else { 0.0 };
    let mut at = t.position + Vec3::new(-back, 0.0, 0.0);
    // A shop's marker instead: 25 m short of the car lot's trigger.
    if screen == "shopmarker"
        && let Some(lot) =
            game.game_data.career.shops.iter().find(|s| s.name == "CC_CAR_LOT_1").and_then(|s| game.sim.route_triggers.find(s.trigger))
    {
        at = ((lot.min + lot.max) * 0.5).extend(lot.position.z);
    }
    // NFSU2_SHOT_AT=x,y,z,deg: the car there instead, heading deg
    // from +x (for side-by-sides with the original's place).
    let mut heading = 0.0;
    if let Some(v) = env.at
    {
        at = Vec3::new(v[0], v[1], v[2]);
        heading = v[3].to_radians();
    }
    if let Some(d) = game.sim.drive.as_mut() {
        d.place(at, heading);
    }
    for _ in 0..60 {
        game.advance_for_each(1.0 / 30.0, |g, dt| {
            g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            Ok(())
        })?;
    }
    // NFSU2_SHOT_HUD_MS: the HUD that long after an update of its
    // own (e.g. a cash pickup's Cash_Animate, started there); the
    // shot renders at NFSU2_SHOT_MS (default 5000).
    if let Some(ms) = std::env::var("NFSU2_SHOT_HUD_MS").ok().and_then(|v| v.parse::<f32>().ok()) {
        let now = game.now();
        let _ = game.hud(now);
        let at: f32 = env.ms.unwrap_or(5000.0);
        game.hud_state.hud.retime(now, at - ms);
    }
    // NFSU2_SHOT_HUD_DUMP=1: the HUD's quads on stderr (texture hash and
    // name, canvas rectangle, colour, z), for naming what draws where
    // (the graphics issues list).
    if std::env::var_os("NFSU2_SHOT_HUD_DUMP").is_some() {
        let now = game.now();
        for q in game.hud(now) {
            let name = q.texture.and_then(|h| game.game_data.assets.textures.get(&h)).map_or("-", |t| t.name.as_str());
            eprintln!(
                "hud quad tex {:08X} {name:32} rect {:?} colour {:?} z {}",
                q.texture.unwrap_or(0),
                q.rect,
                q.color,
                q.z
            );
        }
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The car lot's drive-in scene (ShopEnter1) in free roam with the
/// shot profile's car, `NFSU2_FRAMES` frames at 30 fps in; for the
/// side-by-side with the original's shop entry (scenes75).
/// `NFSU2_SHOP=<name>` another shop (`list` prints them with their
/// markers), `NFSU2_SHOP_EXIT=1` its drive-out (ShopExit1) instead.
pub(super) fn shopenter(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    game.explore(None)?;
    for _ in 0..10 {
        game.advance_for(1.0 / 30.0)?;
    }
    let name = std::env::var("NFSU2_SHOP").unwrap_or_else(|_| "CC_CAR_LOT_1".into());
    if name == "list" {
        let markers =
            career::load_markers(&nfsu2::game_fs::path(&game.game_data.root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/TrackPosMarkersFreeRoam.bin")))
                .unwrap_or_default();
        for s in &game.game_data.career.shops {
            eprintln!("shop {} {:?}", s.name, markers.iter().find(|m| m.name == s.hash).map(|m| m.position));
        }
    }
    let lot = game.game_data.career.shops.iter().position(|s| s.name == name).context("the shop")?;
    let kind = if std::env::var_os("NFSU2_SHOP_EXIT").is_some() {
        nfsu2::shop_scene::SceneKind::Exit
    } else {
        nfsu2::shop_scene::SceneKind::Enter
    };
    anyhow::ensure!(game.start_shop_scene(kind, lot), "the shop scene");
    let frames = std::env::var("NFSU2_FRAMES").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(30);
    for _ in 0..frames {
        if game.scenes.shop.is_none() {
            break;
        }
        game.advance_for(1.0 / 30.0)?;
    }
    // `NFSU2_AFTER_FRAMES=n`: n more frames once the scene has
    // ended (the car handed back to the player, no input).
    let after = std::env::var("NFSU2_AFTER_FRAMES").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    for i in 0..after {
        game.advance_for(1.0 / 30.0)?;
        if std::env::var_os("NFSU2_PROBE").is_some() {
            let d = game.sim.drive.as_ref().unwrap();
            let h = &d.car.car_type.handling;
            let com = d.state.position + d.state.orientation * Vec3::from(h.centre_of_mass);
            let comp: Vec<f32> = d.state.wheels.iter().map(|w| w.compression).collect();
            eprintln!(
                "PROBE after {i} car {:?} com {com:?} com_local {:?} v {:?} rest {} held {} compression {comp:?} road {:?}",
                d.state.position,
                h.centre_of_mass,
                d.state.velocity,
                d.state.rest,
                d.state.held,
                d.world.road_height_below(com + Vec3::Z)
            );
        }
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// `hood` / `bumper`: free roam as `explore`, from that camera
/// setting (1 / 0), with the rear-view mirror (the option's default).
pub(super) fn explore(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, env, .. } = cx;
    if screen.starts_with("hydro") {
        install_shot_hydraulics(game);
    }
    match screen {
        "hood" => game.settings.players[0].camera = 1,
        "bumper" => game.settings.players[0].camera = 0,
        _ => {}
    }
    if screen == "neon_car" {
        install_shot_neon(game);
    }
    // NFSU2_GAUGES: the second custom gauge (FLAMES) with the
    // colour parts RED / LIME / BLUE in 0xA4 / 0xA5 / 0xA6.
    if std::env::var_os("NFSU2_GAUGES").is_some() {
        let car = game.profile.as_ref().map(|p| p.car.clone()).unwrap_or_default();
        let gauge = game.game_data.parts.find_after(&car, 0xA3, game.game_data.parts.find_after(&car, 0xA3, None)).map(|p| p.hash);
        let colour = |slot: u8, name: &str| {
            game.game_data.parts.find(&car, slot as usize, Some(nfsu2::formats::hash::bin_hash(name)), None).map(|p| p.hash)
        };
        let fit =
            [(0xA3, gauge), (0xA4, colour(0xA4, "RED")), (0xA5, colour(0xA5, "LIME")), (0xA6, colour(0xA6, "BLUE"))];
        if let Some(p) = game.profile.as_mut() {
            for (slot, h) in fit {
                if let Some(h) = h {
                    p.installed.insert(slot, h);
                }
            }
        }
    }
    if screen == "vinyl_car" {
        install_shot_vinyl(game);
    }
    // NOS_SHOT: a level-1 nitrous package fitted, so the car has a
    // tank (the shot's profile car has none).
    if std::env::var_os("NOS_SHOT").is_some() {
        let (car, installed, perf) = game.player_car();
        let ids: Vec<u8> = game
            .game_data.career
            .packages
            .iter()
            .filter(|p| p.category == 2 && p.level == 1)
            .flat_map(|p| p.parts.iter().map(|x| x.id))
            .collect();
        if let Some(p) = game.profile.as_mut() {
            (p.car, p.installed, p.perf) = (car, installed, perf);
            p.perf.extend(ids);
        }
    }
    // NFSU2_FX_TURBO=n: the turbo packages up to level n fitted
    // (the shift / rev flame pops need level 2, `carfx_pops`).
    if let Some(level) = env.fx.turbo {
        let (car, installed, perf) = game.player_car();
        let ids: Vec<u8> = game
            .game_data.career
            .packages
            .iter()
            .filter(|p| p.category == 4 && p.level >= 1 && p.level <= level)
            .flat_map(|p| p.parts.iter().map(|x| x.id))
            .collect();
        if let Some(p) = game.profile.as_mut() {
            if p.car.is_empty() {
                (p.car, p.installed, p.perf) = (car, installed, perf);
            }
            p.perf.extend(ids);
        }
    }
    if screen != "race" {
        game.explore(None)?;
        // NFSU2_SHOT_AT=x,y,z,deg (as the marker shot): start the
        // drive there (e.g. in a tunnel, for a reverb capture).
        if let Some(v) = env.at
            && let Some(d) = game.sim.drive.as_mut()
        {
            d.place(Vec3::new(v[0], v[1], v[2]), v[3].to_radians());
            // Where `place` put it (NFSU2_SHOT_LOOK cameras are set from
            // NFSU2_SHOT_AT, whose height may differ from the road's).
            log::info!("shot: car placed at {:?} (asked {:?})", d.state.position, &v[..3]);
            // NFSU2_FX_SPEED=km/h (shot aid, not native): the
            // placed car rolling forward at that speed (the wall
            // scrapes and hits at speed, docs/sparks-fire.md).
            // `km/h,down`: also falling at `down` m/s (a bottom-out).
            let fx_speed: Vec<f32> = env.fx.speed.clone();
            if let Some(&kmh) = fx_speed.first() {
                let v = kmh / 3.6;
                d.state.velocity = glam::Mat3::from_quat(d.state.orientation) * Vec3::X * v
                    - Vec3::Z * fx_speed.get(1).copied().unwrap_or(0.0);
                for (w, spec) in d.state.wheels.iter_mut().zip(d.car.car_type.wheels.iter()) {
                    w.spin = v / spec.radius.max(0.1);
                }
            }
        }
    } else {
        // NFSU2_SHOT_EVENT=<name>: that career event (e.g. the final
        // race, S5_CIRCUIT_36), else stage 1's first circuit.
        let named =
            std::env::var("NFSU2_SHOT_EVENT").ok().and_then(|n| game.game_data.career.events.iter().position(|e| e.name == n));
        let e = match named {
            Some(e) => e,
            None => game
                .game_data.career
                .stage_events(&game.game_data.root, 1)
                .iter()
                .find(|e| e.kind == career::EventKind::Circuit)
                .map(|e| e.index)
                .context("a circuit")?,
        };
        game.start_race(e, false, false)?;
    }
    // `hydro` / `hydro_bounce`: the car standing, then the
    // original's reference key timelines (docs/formats.md,
    // "Hydraulics"), 1/120 s frames; HYDRO_LOG=1 prints each frame.
    let script: &[(&[KeyCode], f32)] = match screen {
        "hydro" => &[
            (&[], 1.0),
            (&[KeyCode::KeyH], 0.5),
            (&[KeyCode::KeyH, KeyCode::ArrowUp], 0.3),
            (&[KeyCode::KeyH], 1.0),
            (&[KeyCode::KeyH, KeyCode::ArrowDown], 0.3),
            (&[KeyCode::KeyH], 1.0),
        ],
        "hydro_bounce" => &[(&[], 1.0), (&[KeyCode::KeyJ], 0.5), (&[KeyCode::KeyJ, KeyCode::ArrowUp], 1.3)],
        _ => &[],
    };
    let log = std::env::var_os("HYDRO_LOG").is_some();
    let mut t = 0.0f32;
    for &(keys, secs) in script {
        game.platform.keys.clear();
        game.platform.keys.extend(keys.iter().copied());
        for _ in 0..(secs * 120.0).round() as u32 {
            game.advance_for(1.0 / 120.0)?;
            t += 1.0 / 120.0;
            if log && let Some(d) = game.sim.drive.as_ref() {
                let s = &d.state;
                let com = s.position
                    + glam::Mat3::from_quat(s.orientation) * Vec3::from(d.car.car_type.handling.centre_of_mass);
                let fwd = glam::Mat3::from_quat(s.orientation) * Vec3::X;
                let hy = &s.hydraulics;
                eprintln!(
                    "hydro {t:6.3} keys {:?} mode {} st {:?} h {:.3} {:.3} {:.3} {:.3} kick {:?} z {:.4} fz {:.4} v {:.2}",
                    keys.len(),
                    d.hydro.mode,
                    hy.state,
                    hy.height[0],
                    hy.height[1],
                    hy.height[2],
                    hy.height[3],
                    hy.kicked,
                    com.z,
                    fwd.z,
                    s.velocity.length() * 2.237
                );
            }
        }
    }
    // Sparks and fire (docs/sparks-fire.md): NFSU2_FX=1 steps the particles like
    // NOS_SHOT; NFSU2_FX_STEER=a|d holds a steering key from
    // NFSU2_FX_STEER_AT (frame, default 0); NFSU2_FX_FRAMES the
    // frame count (default 90).
    let fx_shot = env.fx.on;
    let fx_steer = env.fx.steer;
    let fx_steer_at: u32 = env.fx.steer_at.unwrap_or(0);
    // NFSU2_FX_STEER_FOR: release the steering key after that
    // many frames (a shallow approach: a scrape, not a hit).
    let fx_steer_until = fx_steer_at
        .saturating_add(env.fx.steer_for.unwrap_or(u32::MAX));
    let frames: u32 = env.fx.frames.unwrap_or(90);
    for i in 0..if script.is_empty() { frames } else { 0 } {
        if let Some(k) = fx_steer.filter(|_| i >= fx_steer_at && i < fx_steer_until) {
            game.platform.keys.insert(k);
        } else if let Some(k) = fx_steer {
            game.platform.keys.remove(&k);
        }
        // NOS_PURGE=1 (with NOS_SHOT): the car stands, so the
        // nitrous key purges (0x5ECDC0 under 5 mph).
        // NFSU2_SHOT_HOLD=1: no gas either, the car stays where
        // NFSU2_SHOT_AT put it (a mark-this-spot report's command).
        if std::env::var_os("NOS_PURGE").is_none() && !env.hold {
            game.platform.keys.insert(KeyCode::KeyW);
        }
        // NOS_SHOT: hold the nitrous key (Left Alt) from 1 s on and
        // log the tank.
        if std::env::var_os("NOS_SHOT").is_some() && i >= 30 {
            game.platform.keys.insert(KeyCode::AltLeft);
        }
        // NFSU2_FX_POST=surface,slide,impulse[,from,to] (shot aid):
        // a contact posted at the car's right flank every frame in
        // [from, to) — car-to-car (surface 28) or a light touch at a
        // chosen push, without another car or a wall to hit.
        if let Some(v) = env.fx.post.as_ref()
            && (i as f32) >= v.get(3).copied().unwrap_or(0.0)
            && (i as f32) < v.get(4).copied().unwrap_or(f32::MAX)
            && let Some(d) = game.sim.drive.as_ref()
        {
            let rot = glam::Mat3::from_quat(d.state.orientation);
            let at = d.state.position + rot * Vec3::new(0.2, -0.9, 0.45);
            nfsu2::particles::post_contact(v[0] as u8, at, v[1], v[2], d.state.velocity, rot * Vec3::Y);
        }
        // NFSU2_FX_GEAR=n (shot aid, with NFSU2_FX_SPEED): the
        // gearbox held in that gear (1 = first) over the first
        // frames, so a fast start is not braked by first gear.
        if i < 3
            && let Some(g) = env.fx.gear
            && let Some(t) = game.sim.drive.as_mut().and_then(|d| d.state.drivetrain.as_mut())
        {
            t.gear = g + nfsu2::handling::NEUTRAL;
            t.target_gear = t.gear;
        }
        // The flames need the particle step (other shots keep
        // their frames as they were): each tick's.
        let fx = std::env::var_os("NOS_SHOT").is_some() || fx_shot;
        game.advance_for_each(1.0 / 30.0, |g, dt| {
            if fx {
                g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
            }
            Ok(())
        })?;
        if fx_shot && let Some(d) = game.sim.drive.as_ref() {
            let fx = &d.effects.sparks_fire.sim;
            let flames = d.effects.system.particles.iter().filter(|p| p.key >= FLAME_KEY).count();
            if env.fx.dump {
                for s in fx.sparks.iter().take(6) {
                    eprintln!(
                        "  frag rel {:?} vel {:?} age {:.2}/{:.2} t {:.2}",
                        s.position - d.state.position,
                        s.velocity,
                        s.age,
                        s.life,
                        s.temperature()
                    );
                }
            }
            eprintln!(
                "fx frame {i} speed {:.1} km/h gear {} fragments {} native {} flames {flames} turbo {} at {:?}",
                d.state.speed() * 3.6,
                d.state.drivetrain.as_ref().map_or(0, |t| t.gear),
                fx.sparks.len(),
                d.effects.system.particles.len(),
                d.car.car_type.turbo_level,
                d.state.position
            );
        }
        if std::env::var_os("NOS_SHOT").is_some() && i % 15 == 0 {
            let d = game.sim.drive.as_ref().unwrap();
            eprintln!("compression {:?}", d.state.wheel_poses(&d.car).map(|w| w.lift));
            eprintln!(
                "t {:.1} speed {:.1} km/h nitrous {:?} capacity {} shots {} stage {} particles {}",
                i as f32 / 30.0,
                d.state.speed() * 3.6,
                d.state.nitrous.as_ref().map(|t| t.size),
                d.car.car_type.handling.nitrous_capacity,
                d.car.car_type.handling.nitrous_shots,
                d.state.drivetrain.as_ref().map_or(0, |t| t.nitrous),
                d.effects.system.particles.len()
            );
            // The purge's share (NOS_PURGE: car effect kinds 13 / 14).
            let purge = d.effects.system.particles.iter().filter(|p| p.palette != 0).count();
            eprintln!("purge particles {purge} groups {:08X?}", d.car.purge.as_ref().map(|f| f.groups));
        }
    }
    // `NFSU2_TRICK_SHOT=i[,rate[,frames]]`: trick table entry i fed
    // once (rate 1 by default), then `frames` more frames (3): its
    // popup on screen.
    if let Some(v) = std::env::var("NFSU2_TRICK_SHOT").ok() {
        let mut it = v.split(',');
        let i: usize = it.next().and_then(|x| x.parse().ok()).context("NFSU2_TRICK_SHOT=index")?;
        let rate: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(1.0);
        let frames: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(3);
        match &mut game.mode {
            Mode::Race(r) => {
                let now = r.timing.clock;
                if let Some(st) = r.scoring.style.as_mut() {
                    st.feed(i, rate, now);
                }
            }
            _ => {
                if let Some(roam) = game.sim.roam_style.as_mut() {
                    roam.0.feed(i, rate, roam.2);
                }
            }
        }
        // The HUD each frame, up to the shot's own HUD time (5000 ms).
        for k in 0..frames {
            game.advance_for(1.0 / 30.0)?;
            let _ = game.hud(5000.0 - (frames - k) as f32 * 33.0);
        }
    }
    // `NFSU2_BRAKE=n`: then n frames on the brake (the brake
    // lights' glow, 0x615640 with car +0x944).
    if let Some(n) = std::env::var("NFSU2_BRAKE").ok().and_then(|v| v.parse::<u32>().ok()) {
        game.platform.keys.remove(&KeyCode::KeyW);
        for _ in 0..n {
            game.platform.keys.insert(KeyCode::KeyS);
            game.platform.keys.insert(KeyCode::ArrowDown);
            game.advance_for(1.0 / 30.0)?;
        }
    }
    let d = game.sim.drive.as_mut().unwrap();
    // NFSU2_SHOT_HOLD with NFSU2_SHOT_AT: the settled car moved
    // back onto the given x, y (it rolls a little while it settles),
    // so a mark-this-spot report's pose comes back exactly.
    if env.hold
        && let Some(v) = env.at
    {
        d.state.position.x = v[0];
        d.state.position.y = v[1];
    }
    // `NFSU2_WHEEL_SHOT=i`: the camera close to wheel i (table
    // order: 0 front left, 1 front right, 2 rear right, 3 rear
    // left), a little ahead of it, to see the rim and the brake.
    if let Some(i) = std::env::var("NFSU2_WHEEL_SHOT").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&i| i < 4) {
        let w = d.car.car_type.wheels[i].position;
        let t = d.state.transform();
        let eye = t.transform_point3(w + Vec3::new(0.9, w.y.signum() * 1.1, 0.15));
        d.camera = nfsu2::render::camera::Camera::look_at(eye, t.transform_point3(w), Vec3::Z, 40f32.to_radians());
    }
    // `NFSU2_SHOT_ORBIT=deg,dist,height`: the camera `dist` m from
    // the car at `deg` from its nose (90 = its left), `height` m
    // above its position, looking at it (the neon shots,
    // docs/neon-quality.md).
    if let Some(v) = std::env::var("NFSU2_SHOT_ORBIT")
        .ok()
        .map(|s| s.split(',').filter_map(|f| f.trim().parse::<f32>().ok()).collect::<Vec<_>>())
        .filter(|v| v.len() == 3)
    {
        let t = d.state.transform();
        let a = v[0].to_radians();
        let eye = t.transform_point3(Vec3::new(a.cos() * v[1], a.sin() * v[1], 0.0)) + Vec3::Z * v[2];
        d.camera = nfsu2::render::camera::Camera::look_at(eye, t.w_axis.truncate(), Vec3::Z, 50f32.to_radians());
    }
    // `NFSU2_SHOT_LOOK=ex,ey,ez,tx,ty,tz[,fov]`: the camera at the
    // eye, looking at the target (world, Z up; the water crops,
    // docs/water.md).
    if let Some(v) = std::env::var("NFSU2_SHOT_LOOK")
        .ok()
        .map(|s| s.split(',').filter_map(|f| f.trim().parse::<f32>().ok()).collect::<Vec<_>>())
        .filter(|v| v.len() >= 6)
    {
        // A mark-this-spot report's camera: `,game_fov,ux,uy,uz`
        // after the fov: the game camera's own FOV (degrees, 0 none)
        // and up vector.
        let up = if v.len() >= 11 { Vec3::new(v[8], v[9], v[10]) } else { Vec3::Z };
        d.camera = nfsu2::render::camera::Camera::look_at(
            Vec3::new(v[0], v[1], v[2]),
            Vec3::new(v[3], v[4], v[5]),
            up,
            v.get(6).copied().unwrap_or(50.0).to_radians(),
        );
        d.camera.game_fov = v.get(7).copied().filter(|f| *f > 0.0).map(f32::to_radians);
    }
    game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
    game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
    // NFSU2_SHOT_MARK=1: a mark-this-spot report of the shot's
    // spot (docs/mark-spot.md: the report's command rebuilds it).
    if std::env::var_os("NFSU2_SHOT_MARK").is_some() {
        game.mark_spot();
    }
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// The nearest free-roam traffic car (TRAFFIC_MODEL picks a model),
/// seen from 8 m behind it after 20 s of traffic.
pub(super) fn traffic_close(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, .. } = cx;
    game.explore(None)?;
    let want = std::env::var("TRAFFIC_MODEL").ok();
    for f in 0..600 {
        game.advance_for(1.0 / 30.0)?;
        if std::env::var_os("TRAFFIC_POSE_CHECK").is_some() {
            for (i, c) in game.sim.traffic.iter().flat_map(|t| t.cars.iter().enumerate()).filter(|(_, c)| c.lane.is_some())
            {
                let p = c.state.wheel_poses(&c.car);
                if p.iter()
                    .any(|w| !(w.lift.is_finite() && w.steer.is_finite() && w.roll.is_finite()) || w.lift.abs() > 0.2)
                    || !c.state.position.is_finite()
                {
                    eprintln!(
                        "frame {f} car {i} {}: {:?} mover {}",
                        c.car.car_type.name,
                        p.map(|w| (w.lift, w.steer, w.roll)),
                        c.mover.is_some()
                    );
                }
            }
        }
    }
    let at = game.sim.drive.as_ref().unwrap().state.position;
    let t = game.sim.traffic.as_ref().context("traffic")?;
    let c = t
        .cars
        .iter()
        .filter(|c| c.lane.is_some() && want.as_ref().is_none_or(|w| c.car.car_type.name == *w))
        .min_by(|a, b| (a.state.position - at).length().total_cmp(&(b.state.position - at).length()))
        .context("a traffic car")?;
    let (pos, heading) = (c.state.position, c.state.heading());
    eprintln!(
        "traffic {} at {pos:?}: wheels {:?}",
        c.car.car_type.name,
        c.state.wheel_poses(&c.car).map(|w| (w.lift, w.roll))
    );
    let back = Vec3::new(heading.cos(), heading.sin(), 0.0) * 8.0;
    // `TRAFFIC_VIEW=oncoming` with `TRAFFIC_DIST=<m>` (default 25): the
    // player standing that far ahead of the car, one lane (3.5 m) to its
    // left, facing it (its headlights coming at the chase camera).
    let oncoming = std::env::var("TRAFFIC_VIEW").is_ok_and(|v| v == "oncoming");
    if let Some(d) = game.sim.drive.as_mut() {
        if oncoming {
            let dist = std::env::var("TRAFFIC_DIST").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(25.0);
            let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
            let left = Vec3::new(-heading.sin(), heading.cos(), 0.0);
            // `TRAFFIC_LANE=<m>` (default 3.5): the player's side offset (0 = in the car's
            // own lane, the camera on its headlights' axis).
            let lane = std::env::var("TRAFFIC_LANE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(3.5);
            d.place(pos + fwd * dist + left * lane + Vec3::Z, heading + std::f32::consts::PI);
        } else {
            d.place(pos - back + Vec3::Z, heading);
        }
    }
    let d = game.sim.drive.as_mut().unwrap();
    // `TRAFFIC_VIEW=side`: the camera 6 m off the car's left side
    // at wheel height, to see the wheels in their arches.
    if std::env::var("TRAFFIC_VIEW").is_ok_and(|v| v == "side") {
        let right = Vec3::new(heading.sin(), -heading.cos(), 0.0);
        d.camera = nfsu2::render::camera::Camera::look_at(
            pos - right * 6.0 + Vec3::Z * 0.8,
            pos + Vec3::Z * 0.6,
            Vec3::Z,
            50f32.to_radians(),
        );
    }
    // `TRAFFIC_VIEW=far` with `TRAFFIC_DIST=<m>`: the camera that far
    // off, behind and 25° to the left at 1.5 m (the LOD steps, see
    // `car_lod`), with the chase camera's lens.
    if std::env::var("TRAFFIC_VIEW").is_ok_and(|v| v == "far") {
        let dist = std::env::var("TRAFFIC_DIST").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(30.0);
        let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
        let right = Vec3::new(heading.sin(), -heading.cos(), 0.0);
        let a = 25f32.to_radians();
        let fov = d.camera.game_fov;
        d.camera = nfsu2::render::camera::Camera::look_at(
            pos - (fwd * a.cos() + right * a.sin()) * dist + Vec3::Z * 1.5,
            pos + Vec3::Z * 0.6,
            Vec3::Z,
            50f32.to_radians(),
        );
        d.camera.game_fov = fov;
    }
    game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &nfsu2::render::RenderOptions::in_force()));
    game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &nfsu2::render::RenderOptions::in_force()));
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}

/// An outrun a moment after the challenge: the first encounter
/// racer placed, lined up 20 m ahead and challenged ("outrun"), or
/// its icon before the key ("encounter"); "outrun_won": then the
/// challenged car put 320 m behind (past 0x7A56B4's 300 m), the
/// race over 1 s on ("You Won the Outrun!" / time / bank).
pub(super) fn outrun(game: &mut Game, cx: &ShotCtx) -> Result<Mode> {
    let &ShotCtx { gpu, format, screen, .. } = cx;
    // Stage 0 offers none (its intro flags are not modelled).
    if let Some(p) = game.profile.as_mut()
        && p.stage == 0
    {
        p.stage = 1;
    }
    game.explore(None)?;
    // On the longest road of the free-roam routes, at its middle and
    // along it (the garage's driveway walks into a dead end).
    if let (Some(e), Some(d)) = (game.sim.encounters.as_ref(), game.sim.drive.as_mut()) {
        let f = &e.routes.file;
        if let Some((ri, r)) = f.records.iter().enumerate().max_by_key(|(_, r)| r.points.len()) {
            let c = r.points.len() / 2;
            let dir = f.direction(ri, c, false);
            d.place(r.points[c].position, dir.y.atan2(dir.x));
        }
    }
    for _ in 0..30 * 60 {
        game.advance_for(1.0 / 30.0)?;
        if game.sim.encounters.as_ref().is_some_and(|e| e.state == 3 && e.cars.iter().any(|c| c.active)) {
            break;
        }
    }
    let i = game.sim.encounters.as_ref().and_then(|e| e.cars.iter().position(|c| c.active)).with_context(|| {
        format!(
            "no encounter racer placed: {:?}, stage {:?}",
            game.sim.encounters.as_ref().map(|e| (e.state, e.career, e.timer, e.cars.len())),
            game.profile.as_ref().map(|p| p.stage)
        )
    })?;
    let (heading, at) = {
        let d = game.sim.drive.as_ref().unwrap();
        (d.state.heading(), d.state.position)
    };
    let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
    for frame in 0..12 {
        if frame == 0 || screen == "encounter" {
            let d = game.sim.drive.as_mut().unwrap();
            d.state = CarState::new(at, heading);
            d.state.velocity = fwd * 20.0;
            // Explicit synthetic/replay motion initialization; no native placement offset.
            d.state.publish_forward_speed();
            let e = game.sim.encounters.as_mut().unwrap();
            e.cars[i].state = CarState::new(at + fwd * 20.0, heading);
            e.cars[i].state.velocity = fwd * 20.0;
            // Explicit synthetic/replay motion initialization; no native placement offset.
            e.cars[i].state.publish_forward_speed();
        }
        if frame == 1 && screen != "encounter" {
            game.sim.encounters.as_mut().unwrap().engage();
        }
        game.advance_for(1.0 / 30.0)?;
        // Frames up to the shot's own HUD time (5000 ms), so the
        // challenge's ZoomIn still runs.
        let _ = game.hud(5000.0 - (12 - frame) as f32 * 33.0);
    }
    if screen == "outrun_won" {
        // The player takes the lead first (the challenged car 30 m
        // behind for 1 s), then the car drops 320 m back.
        for frame in 0..90 {
            if frame < 30 || frame == 30 {
                let d = game.sim.drive.as_ref().unwrap();
                let (at, heading, v) = (d.state.position, d.state.heading(), d.state.velocity);
                let back = Vec3::new(heading.cos(), heading.sin(), 0.0) * if frame < 30 { -30.0 } else { -320.0 };
                let e = game.sim.encounters.as_mut().unwrap();
                e.cars[i].state = CarState::new(at + back, heading);
                e.cars[i].state.velocity = if frame < 30 { v } else { Vec3::ZERO };
                // Explicit synthetic/replay motion initialization; no native placement offset.
                e.cars[i].state.publish_forward_speed();
            }
            game.advance_for(1.0 / 30.0)?;
            let _ = game.hud(5000.0 - (90 - frame) as f32 * 33.0);
        }
        eprintln!("outrun_won: {:?}", game.sim.encounters.as_ref().and_then(|e| e.race_over()));
    }
    finish_scene(game, gpu, format);
    Ok(std::mem::replace(&mut game.mode, Mode::Explore))
}
