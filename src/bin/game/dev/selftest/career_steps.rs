//! The selftest's career steps, from the boot to the sub-selftests,
//! in the order `selftest` runs them.
use super::*;

/// The boot screens, the stream checks, the main menu's showroom, the car selects hammered, and a later career's free roam left for the main menu.
pub(super) fn boot_and_menus(game: &mut Game) -> Result<()> {
    selftest_boot(game)?;
    selftest_streams(game)?;
    // The main menu stands on GarageMain's SHOWROOM scene (0x4DF140,
    // 0x57FEFA) with the D-Day preset car before the career has a car.
    if matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
        let g = game.garage_request().context("the main menu's garage")?;
        let platform = nfsu2::garage::Platform::load(&game.game_data.root, &g.platform)?;
        anyhow::ensure!(
            g.platform == "SHOWROOM" && platform.envmap.is_some() && platform.world.placements.len() == 6,
            "the showroom scene"
        );
        println!("main menu garage: {} with {:?}", g.platform, g.car);
    }
    // Left / Right hammered on Quick Race's car select and Customize's:
    // the platform shows the selected car in front once the keys stop.
    if matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
        let (mode, game_mode) = (std::mem::replace(&mut game.mode, Mode::Explore), game.race_flow.game_mode);
        game.race_flow.game_mode = 2;
        game.mode = game.quick_car_select(None);
        selftest_lot_hammer(game, "Quick Race's car select")?;
        game.mode = mode;
        game.act(Action::Customize, MenuKind::Main)?;
        selftest_lot_hammer(game, "Customize's car select")?;
        game.race_flow.game_mode = game_mode;
        game.mode = game.icon_menu(MenuKind::Main);
        game.garage_request();
    }
    // A later career's free roam first, then back to the main menu (user
    // report 2026-11-01: the intro's GPS arrow at the airport pointed
    // right where the original's points left): the pool and its network,
    // pruned by stage 5's barriers (0x578070 → 0x5E9E90), stayed for the
    // new career, whose route to the car lot then ran south.
    if matches!(&game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
        let mode = std::mem::replace(&mut game.mode, Mode::Explore);
        let before = game.profile.take();
        let mut p = Profile::new("Later");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "COROLLA", &mut game.rng);
        p.stage = 5;
        game.profile = Some(p);
        game.explore(None)?;
        anyhow::ensure!(game.sim.traffic.as_ref().is_some_and(|t| t.stage == Some(5)), "stage 5's free-roam network");
        game.profile = before;
        game.career_world.gps = None;
        game.drop_drive();
        game.mode = mode;
    }
    game.act(Action::Career, MenuKind::Main)?;
    println!("career menu: {:?}", menu(game));
    Ok(())
}

/// Start New Career through MC_Main and the name editor, then the intro movie: the D-Day profile.
pub(super) fn new_career(game: &mut Game) -> Result<()> {
    // A new career starts the D-Day intro (0x495A30): in the city in
    // Rachel's car, the DDAY_PLAYER_CAR preset.
    // Start New Career opens MC_Main: Yes/No with No focused, Yes to the
    // name editor ("NAME", seven letters or digits at most), Enter, then
    // Yes to save. Typing appends to "NAME" (0x551280 never clears it;
    // the player backspaces it away), so "Ray-99" gives "NAMERay": "-"
    // is refused and the limit of seven drops "99".
    // A minute on the game's clock, as after a while in the menus: the
    // screens' script times must not depend on it (`check_texts_shown`).
    game.pacing.started = game.pacing.started.checked_sub(std::time::Duration::from_secs(60)).unwrap_or(game.pacing.started);
    game.act(Action::NewCareer, MenuKind::Career)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Ask && m.selected == 1),
        "MC_Main asks, No focused"
    );
    check_texts_shown(game, "MC_Main's question")?;
    game.menu_key(Key::Left)?;
    check_texts_shown(game, "MC_Main's question, Yes focused")?;
    game.menu_key(Key::Accept)?;
    check_texts_shown(game, "MC_Main's name editor")?;
    if let Mode::Menu(Menu::McMain(m)) = &mut game.mode {
        "Ray-99".chars().for_each(|c| m.char(c));
    }
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Name && m.name == "NAMERay"),
        "the name editor"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == McStep::Confirm),
        "save the profile named NAMERay?"
    );
    check_texts_shown(game, "MC_Main's save question")?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    // 0x495A30 → 0x53D720: the intro movie first (scene01, mode 6) on
    // IG_PlayMovie; Accept skips it only after 0.5 s (0x55AFF0), then the
    // tick starts D-Day (0x54AF90).
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.queue == ["scene01"] && m.mode == 6),
        "the career intro movie"
    );
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.player.is_some()), "Scene01 found and opened");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if !m.done), "no skip in the first 0.5 s");
    game.advance_for(0.3)?;
    game.advance_for(0.3)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Movie(m)) if m.player.as_ref().is_some_and(|p| p.width() == 512 && !p.rgba.is_empty())),
        "frames decoded"
    );
    game.menu_key(Key::Accept)?;
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "the intro in the city");
    anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.intro_played), "the intro movie is played (0x86347C)");
    anyhow::ensure!(game.profile.as_ref().is_some_and(|p| p.name == "NAMERay"), "the named profile");
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        p.intro == Some(career::DDAY_EVENT_A) && p.car.is_empty() && p.stage == 0,
        "D-Day A under way, no car of our own"
    );
    println!("intro car: {:?}", game.sim.drive.as_ref().map(|d| d.car.car_type.name.clone()));
    anyhow::ensure!(game.sim.drive.as_ref().is_some_and(|d| d.car.car_type.name == "350Z"), "Rachel's 350Z");
    anyhow::ensure!(p.cash == career::start_cash(), "native starting cash");
    Ok(())
}

/// The career's start scene (CareerNis1) over the car standing at the airport.
pub(super) fn start_scene(game: &mut Game) -> Result<()> {
    // The career's start scene (CareerNis1, 405 frames): the camera flies
    // over the airport with no HUD while the car stands at the start, and
    // the script waits for it.
    let car_before = game.here().context("the car")?.0;
    let scene = game.scenes.shop.as_ref().map(|r| (r.scene.kind, r.scene.length, r.camera.is_some()));
    anyhow::ensure!(
        scene == Some((nfsu2::shop_scene::SceneKind::Career, 405.0, true)),
        "the career's start scene: {scene:?}"
    );
    // The original's 350Z under CareerNis1 (Wine, a new career:
    // mode 5 from count 215 to the scene's end, standing, centre of mass
    // (1695.2516, −883.6426, 24.1127), forward (0.9817, −0.1903, −0.0018);
    // then mode 1 at the same place, docs/evidence/dyno-exit/README.md).
    let com_of = |game: &Game| {
        game.sim.drive.as_ref().map(|d| {
            (
                d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass),
                d.state.forward(),
            )
        })
    };
    let native_com = Vec3::new(1695.2516, -883.6426, 24.1127);
    let (com, fwd) = com_of(game).context("the drive")?;
    println!(
        "career scene car: centre of mass {com:?} ({:.3} m off the original's), forward {fwd:?}",
        com.distance(native_com)
    );
    anyhow::ensure!(
        com.distance(native_com) < 0.1 && fwd.dot(Vec3::new(0.9817, -0.1903, -0.0018)) > 0.999,
        "the career scene's car {com:?} / {fwd:?} vs the original's"
    );
    anyhow::ensure!(
        game.overlay_quads(0.0).iter().all(|q| q.texture.is_none() && q.color == [0.0, 0.0, 0.0, 1.0]),
        "no HUD under the start scene (the letterbox bars only)"
    );
    let cam = game.sim.drive.as_ref().map(|d| d.camera.position).context("the drive")?;
    anyhow::ensure!(cam.distance(car_before) > 10.0, "the scene's own camera: {cam:?} for the car at {car_before:?}");
    play_career_scene(game)?;
    anyhow::ensure!(
        game.here().context("the car")?.0.distance(car_before) < 0.5 && !game.career_world.dday.running(),
        "the car where it stood, the script not yet begun"
    );
    let (com, _) = com_of(game).context("the drive")?;
    anyhow::ensure!(
        com.distance(native_com) < 0.1,
        "after the career scene the car {com:?} vs the original's {native_com:?}"
    );
    Ok(())
}

/// The D-Day script holding the car, then Rachel's SMS_INSTRUCTION read.
pub(super) fn dday_script(game: &mut Game) -> Result<()> {
    // The D-Day script (0x5342A0): it starts on the first free-roam frame
    // (0x513BC0) and holds the car (input mode 0xA); the HUD shows only
    // the prompt (0x60031E: high bit 2) flashing kind 0x14.
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(game.career_world.dday.running() && game.career_world.dday.state == 2, "the D-Day script under way");
    anyhow::ensure!(
        game.career_world.dday.hud_features() == Some((0, 2)) && game.career_world.dday_prompt.is_some(),
        "the script's HUD and prompt"
    );
    let car_at = game.here().context("the car")?.0;
    game.platform.keys.insert(KeyCode::ArrowUp);
    // Two seconds on, SMS_INSTRUCTION arrives and the SMS screens open on
    // it (0x5338A0, 0x529710), reading it (0x514DC0 → +8).
    let mut frames = 0;
    while matches!(game.mode, Mode::Explore) && frames < 400 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    game.platform.keys.clear();
    println!("SMS_INSTRUCTION after {:.2} s", frames as f32 / 60.0);
    // Rachel's message on its own: the viewer opens on it (0x497850 with
    // one unread), not the mailbox.
    let unread: Vec<String> = game
        .profile
        .as_ref()
        .map_or(Vec::new(), |p| p.inbox.entries.iter().filter(|e| !e.read).map(|e| format!("{e:?}")).collect());
    println!(
        "instruction SMS: {} (unread now {unread:?})",
        match &game.mode {
            Mode::Menu(Menu::SmsViewer(_)) => "viewer",
            Mode::Menu(Menu::SmsMailbox(_)) => "MAILBOX",
            _ => "other",
        }
    );
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::SmsViewer(_))), "the instruction SMS opens in the viewer");
    anyhow::ensure!((2.0..2.2).contains(&(frames as f32 / 60.0)), "the instruction 2 s in");
    let moved = game.here().map_or(f32::MAX, |(at, _)| (at - car_at).truncate().length());
    println!(
        "held with the throttle down: moved {moved:.2} m, speed {:.2}",
        game.sim.drive.as_ref().map_or(0.0, |d| d.state.speed())
    );
    // (The spawn settles and rolls a little; full throttle would be well
    // past 5 m/s by now.)
    anyhow::ensure!(
        moved < 3.0 && game.sim.drive.as_ref().is_some_and(|d| d.state.speed().abs() < 3.0),
        "the car takes no throttle while the script runs"
    );
    let instruction = game
        .profile
        .as_ref()
        .unwrap()
        .inbox
        .entries
        .iter()
        .position(|e| e.message == dday::SMS_INSTRUCTION)
        .context("SMS_INSTRUCTION in the inbox")?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::SmsViewer(v)) if v.entry == instruction),
        "the viewer on SMS_INSTRUCTION"
    );
    check_texts_shown(game, "SMS_INSTRUCTION")?;
    anyhow::ensure!(game.career_world.dday.read && game.career_world.gps.is_none(), "read, no GPS yet");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Explore) && game.career_world.dday.closed,
        "back in the city, the screens closed (+9)"
    );
    Ok(())
}

/// The script's end: the GPS's route to the car lot and its aid on the minimap.
pub(super) fn gps_to_car_lot(game: &mut Game) -> Result<()> {
    // Two seconds later state 6, two more the GPS to the car lot (state 7),
    // then the script ends and the car is free.
    for _ in 0..300 {
        game.advance_for(1.0 / 60.0)?;
    }
    let lot_point = game
        .sim.route_triggers
        .find(game.game_data.career.shops.iter().find(|s| s.name == "CC_CAR_LOT_1").context("the car lot")?.trigger)
        .context("its trigger")?
        .gps;
    // The route search (0x4268A0 each frame, 0.6 ms a frame) finds the
    // way to the car lot; the follower takes it (0x41EA30, +0x3E4 = 1).
    let mut frames = 0;
    while game.career_world.gps.as_ref().is_some_and(|g| g.state() == 0) && frames < 600 {
        game.advance_for(1.0 / 60.0)?;
        frames += 1;
    }
    let gps = game.career_world.gps.clone().context("the GPS engaged")?;
    println!(
        "GPS: target {:08X} at {:?}, state {} after {frames} frames, car {:?}",
        gps.target,
        gps.at,
        gps.state(),
        game.car_pose()
    );
    anyhow::ensure!(gps.target == career::DDAY_EVENT_A && gps.at == lot_point && gps.on(), "the GPS to the car lot");
    let routes = &game.sim.traffic.as_ref().context("free roam routes")?.routes;
    let route = gps.route(routes);
    let length: f32 = route.windows(2).map(|w| (w[1] - w[0]).truncate().length()).sum();
    let car = game.here().context("the car")?.0;
    let straight = (car - gps.at).truncate().length();
    println!(
        "GPS route: {} points, {length:.0} m (straight {straight:.0} m), g {:.0}",
        route.len(),
        gps.follower.node.as_ref().map_or(0.0, |n| n.g)
    );
    anyhow::ensure!(gps.state() == 3 && gps.follower.state == 1, "a route found to the car lot");
    anyhow::ensure!(length >= straight - 60.0 && length < 4.0 * straight + 500.0, "the route's length is sane");
    anyhow::ensure!(
        route.first().is_some_and(|p| (*p - car).truncate().length() < 60.0) && route.last() == Some(&gps.at),
        "the route runs from the car to the lot"
    );
    anyhow::ensure!(
        game.map_view().is_some_and(|m| !m.gps_searching && m.gps_error.is_none()),
        "no search group once found"
    );
    anyhow::ensure!(game.career_world.dday.state == 9 && !game.career_world.dday.holds_input(), "the script done");
    // The minimap shows the GPS icon and, in stage 0 with the GPS on,
    // every marker (the intro's goal being the only one).
    anyhow::ensure!(game.map_view().is_some_and(|m| m.gps == Some(gps.map)), "the GPS icon on the minimap");
    // MARKER_GPS_AID's look-ahead (0x5F3CC0): a point on the route at
    // least 100 m on from the car (0x40A2A0).
    let aid = gps.aid.context("the GPS aid's look-ahead point")?;
    anyhow::ensure!(
        route.iter().any(|p| (*p - aid).length() < 200.0) && (aid - car).length() > 20.0,
        "the GPS aid ahead on the route: {aid:?}"
    );
    anyhow::ensure!(game.sim.drive.as_ref().is_some_and(|d| d.world.gps_aid.is_some()), "MARKER_GPS_AID loaded");
    // Over stage 0's network, not the earlier career's: the way north, the
    // aid left of the car (the original: (1725.2, −787.3) from the car at
    // (1695.3, −883.6) facing (0.98, −0.19)).
    anyhow::ensure!(game.sim.traffic.as_ref().is_some_and(|t| t.stage == Some(0)), "the new career's network is stage 0's");
    let fwd = game.car_pose().context("the car")?.1;
    let left = fwd.perp_dot((aid - car).truncate());
    println!("GPS aid {aid:?} from the car {car:?} facing {fwd:?}: {}", if left > 0.0 { "left" } else { "right" });
    anyhow::ensure!(left > 0.0, "the GPS aid points left at the airport, as the original's");
    Ok(())
}

/// Headless audio: the mix map (`audio::mixmap`).
pub(super) fn audio_mixmap(game: &mut Game) -> Result<()> {
    // The mix map (`audio::mixmap`): MAPOUTPUT gives the player's engine
    // a level below full with the camera behind it, and a car 200 m off
    // to the side is out of the AI section's 3D ranges.
    {
        use nfsu2::audio::mixmap::{CarFields, Listener, Mix, MixMap, Scene};
        let map = MixMap::load(&game.game_data.root, 0).context("SOUND/MixMaps/MAPOUTPUT.mxb")?;
        let mut mix = Mix::new(map);
        let l = Listener { cam: [-5.0, 0.0], cam_dir: [1.0, 0.0], car: [0.0, 0.0], car_dir: [1.0, 0.0] };
        let car = CarFields { ctx_rpm: 1000.0, view_slew: 0x7D00, ..Default::default() };
        let far = CarFields { pos: [0.0, -200.0], ..car };
        let scene = Scene { listener: Some(l), player: Some(car), ai: vec![far], ..Default::default() };
        for _ in 0..5 {
            mix.frame(&scene, 1.0 / 60.0);
        }
        let (engine, ai) = (mix.levels(1, 0).level(0, 0), mix.levels(2, 0).level(0, 0));
        anyhow::ensure!(
            engine > 0x2000 && engine < 0x7FFF && ai == 0,
            "mix map levels: player engine {engine}, far AI engine {ai}"
        );
        println!("mix map: player engine level {engine}");
    }
    Ok(())
}

/// Headless audio: the car sounds (`audio::carfx`).
pub(super) fn audio_car_sounds(game: &mut Game) -> Result<()> {
    // The car sounds (`audio::carfx`) into a headless mix: a nitrous shot
    // at speed starts the FX_NITROUS voices and a loop; an up-shift at
    // 6000 rpm plays FX_SHIFTING_01 after the record's delay.
    {
        use nfsu2::audio::carfx::{CarInput, PlayerSfx};
        game.advance_for(1.0 / 60.0)?;
        let base = game.car_sfx_input().with_context(|| {
            format!(
                "the car's sound inputs (drive {}, drivetrain {})",
                game.sim.drive.is_some(),
                game.sim.drive.as_ref().is_some_and(|d| d.state.drivetrain.is_some())
            )
        })?;
        println!("car sounds: nitrous level {} shift record {}", base.nitrous_level, base.shift_record);
        let mut mix = nfsu2::audio::mixer::Mix::new(44100);
        let mut sfx = PlayerSfx::new(&game.game_data.root);
        // The engine chain's SFXCTL_Shifting gives the clunk.
        let mut h = nfsu2::audio::hybrid::HybridMotor::new(0);
        let mut at = |stage, gear: usize| {
            let e = nfsu2::audio::hybrid::EngineInput {
                throttle: 1.0,
                rpm: 6000.0,
                idle_rpm: 1000.0,
                max_rpm: 8000.0,
                gear: gear as i32,
                shift_record: base.shift_record,
                ..Default::default()
            };
            h.update(&e, 1.0 / 60.0);
            CarInput { stage, speed: 30.0, gear, rpm: 6000.0, shift: h.shift_signal(), turbo: true, ..base }
        };
        sfx.update(&mut mix, at(0, 3), 1.0 / 60.0);
        sfx.update(&mut mix, at(1, 3), 1.0 / 60.0);
        anyhow::ensure!(sfx.started_nitrous > 0 && mix.voices() > 0, "a nitrous shot starts the NOS voices");
        let before = mix.voices();
        let mut shifted = 0;
        for _ in 0..30 {
            sfx.update(&mut mix, at(1, 4), 1.0 / 60.0);
            shifted += sfx.shifts;
        }
        anyhow::ensure!(
            shifted == 1 && mix.voices() > 0,
            "an up-shift plays one shift sample ({shifted}, voices {before})"
        );
        // The shot ends: the loop fades over 1 s and stops.
        for _ in 0..90 {
            sfx.update(&mut mix, at(0, 4), 1.0 / 60.0);
        }
        anyhow::ensure!(!sfx.nitrous.on, "the nitrous released");
        // The turbo (CARSFX_Turbo): a second of full throttle spools the
        // whine loops up; lifting off blows off once.
        let mut mix = nfsu2::audio::mixer::Mix::new(44100);
        let mut sfx = PlayerSfx::new(&game.game_data.root);
        for _ in 0..60 {
            sfx.update(&mut mix, CarInput { throttle: 1.0, rpm: 6000.0, stage: 0, turbo: true, ..base }, 1.0 / 60.0);
        }
        anyhow::ensure!(
            sfx.turbo.whine > 0 && mix.voices() >= 1,
            "the turbo whine spools up ({}, voices {})",
            sfx.turbo.whine,
            mix.voices()
        );
        let mut blown = 0;
        for _ in 0..5 {
            sfx.update(&mut mix, CarInput { throttle: 0.0, rpm: 6000.0, stage: 0, turbo: true, ..base }, 1.0 / 60.0);
            blown += sfx.blow_offs;
        }
        anyhow::ensure!(blown == 1 && sfx.turbo.blowing, "lifting off blows off once ({blown})");
        println!("turbo record {}: blow-off at spool {:.2}", base.turbo_record, sfx.turbo.fraction);
    }
    Ok(())
}

/// Headless audio: road noise, wind, bottom-out (`audio::roadwind`) and the other cars (`audio::traffic`).
pub(super) fn audio_road_and_traffic(game: &mut Game) -> Result<()> {
    // The road noise, wind and bottom-out (`audio::roadwind`) and the
    // other cars (`audio::traffic`) into a headless mix: on asphalt
    // (surface 1, class 5) the FX_ROADNOISE and FX_WIND graphs sound;
    // rolling onto surface 11 thumps (collision record 0x4E..0x51); the
    // front wheels landing knocks; an AI engine runs its SPU "CAR"
    // graph; a traffic car passing within 5 m at 25 m/s whooshes.
    {
        use nfsu2::audio::roadwind::{EnvInput, EnvSfx};
        use nfsu2::audio::traffic::{AiInput, OtherCars, TrafficInput};
        let mut mix = nfsu2::audio::mixer::Mix::new(44100);
        let mut env = EnvSfx::new(&game.game_data.root);
        let wheel = |surface: u8| nfsu2::audio::skids::WheelCtl {
            surface: [surface; 2],
            down: [true; 2],
            ..Default::default()
        };
        let mut input = EnvInput {
            speed: 25.0,
            wheel: Some(wheel(1)),
            down: [true; 4],
            up_z: 1.0,
            compression: [0.5; 4],
            mix: Default::default(),
            event_kind: 0,
            rain: 0.0,
            stream_busy: false,
        };
        for _ in 0..30 {
            env.update(&mut mix, &input, 1.0 / 60.0);
        }
        let road = mix.voices();
        anyhow::ensure!(road >= 2, "road noise and wind sound at 25 m/s ({road} voices)");
        input.wheel = Some(wheel(11));
        let mut thumps = 0;
        for _ in 0..5 {
            env.update(&mut mix, &input, 1.0 / 60.0);
            thumps += env.thumps;
        }
        anyhow::ensure!(thumps == 2, "onto surface 11 both sides thump ({thumps})");
        let mut knocks = 0;
        for down in [[false, false, true, true], [false, false, true, true], [true; 4], [true; 4]] {
            env.update(&mut mix, &EnvInput { down, ..input.clone() }, 1.0 / 60.0);
            knocks += env.knocks;
        }
        anyhow::ensure!(knocks == 1, "the front wheels landing knock once ({knocks})");
        // A water drum's splash (0x46AE00): three FX_ROADNOISE_TRANS
        // variant-3 requests that run out with the sample (1.99 s).
        env.splash(&mut mix, 100);
        for _ in 0..3 {
            env.update(&mut mix, &input, 1.0 / 60.0);
        }
        let running = env.splashing();
        anyhow::ensure!(
            env.splashed == 1 && running == 3,
            "a water drum splashes ({} started, {running} requests)",
            env.splashed
        );
        // (The mix is rendered so the voices run out.)
        let mut frames = 0;
        let mut buf = vec![0.0f32; 735 * 2];
        while env.splashing() > 0 && frames < 600 {
            mix.render(&mut buf, 2);
            env.update(&mut mix, &input, 1.0 / 60.0);
            frames += 1;
        }
        println!("water drum splash: 3 requests, ended after {:.2} s", frames as f32 / 60.0);
        anyhow::ensure!(env.splashing() == 0, "the splash's requests end ({} left after 10 s)", env.splashing());
        let mut mix = nfsu2::audio::mixer::Mix::new(44100);
        let mut others = OtherCars::new(&game.game_data.root);
        let ai = [AiInput {
            entry: 0,
            ctx_rpm: 6000.0,
            throttle: 1.0,
            position: Vec3::new(20.0, 0.0, 0.0),
            mix: Default::default(),
        }];
        let mut whooshes = 0;
        for k in 0..120 {
            let x = -30.0 + 25.0 * k as f32 / 60.0;
            let car = TrafficInput {
                position: Vec3::new(x, 3.0, 0.0),
                velocity: Vec3::new(25.0, 0.0, 0.0),
                mix: Default::default(),
                down: [true; 4],
                up_z: 1.0,
                event_kind: 0,
                key: 0,
                taxi: false,
            };
            others.update(&mut mix, &ai, &[car], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
            whooshes += others.whooshes;
        }
        anyhow::ensure!(
            whooshes == 1 && mix.voices() >= 2,
            "a passing traffic car whooshes once ({whooshes}), engines sound ({} voices)",
            mix.voices()
        );
        // An oncoming traffic car near a fast player honks (0x47E6E0).
        others.player_forward = Vec3::X;
        others.player_speed = 20.0;
        let mut horns = 0;
        // (3 s after the clock's start, then 30 % or 5 s since the last.)
        for _ in 0..300 {
            let car = TrafficInput {
                position: Vec3::new(12.0, 3.0, 0.0),
                velocity: Vec3::new(-10.0, 0.0, 0.0),
                mix: Default::default(),
                down: [true; 4],
                up_z: 1.0,
                event_kind: 0,
                key: 0,
                taxi: false,
            };
            others.update(&mut mix, &[], &[car], (Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0)), 1.0 / 60.0);
            horns += others.horns;
        }
        anyhow::ensure!(horns >= 1, "an oncoming traffic car honks ({horns})");
        // Car stereos (CARSFX_TrafficStereo, 0x476580): each attach within
        // 70 m rolls 1/3; taxis never; out of range nothing attaches.
        let (mut stereos, mut taxis, mut far) = (0, 0, 0);
        for key in 0..60 {
            let car = |key, taxi, x| TrafficInput {
                position: Vec3::new(x, 3.0, 0.0),
                velocity: Vec3::ZERO,
                mix: Default::default(),
                down: [true; 4],
                up_z: 1.0,
                event_kind: 0,
                key,
                taxi,
            };
            others.update(&mut mix, &[], &[car(key, false, 30.0)], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
            stereos += others.stereos;
            others.update(&mut mix, &[], &[car(1000 + key, true, 30.0)], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
            taxis += others.stereos;
            others.update(&mut mix, &[], &[car(2000 + key, false, 80.0)], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
            far += others.stereos;
            others.update(&mut mix, &[], &[], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
        }
        anyhow::ensure!(
            (8..=35).contains(&stereos) && taxis == 0 && far == 0,
            "about a third of the traffic cars play a stereo ({stereos} of 60, taxis {taxis}, beyond 70 m {far})"
        );
        // Six traffic contexts (0x476AF0): ten cars in range, six engines.
        let mut ctx = OtherCars::new(&game.game_data.root);
        let cars: Vec<_> = (0..10)
            .map(|key| TrafficInput {
                position: Vec3::new(10.0 + 3.0 * key as f32, 3.0, 0.0),
                velocity: Vec3::ZERO,
                mix: Default::default(),
                down: [true; 4],
                up_z: 1.0,
                event_kind: 0,
                key,
                taxi: false,
            })
            .collect();
        ctx.traffic_cars = 10;
        ctx.update(&mut mix, &[], &cars, (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
        let held = ctx.traffic_contexts();
        anyhow::ensure!(
            held == (0..6).map(Some).collect::<Vec<_>>(),
            "the six contexts go to the first six cars in range ({held:?})"
        );
        // Racers' stereos (CARSFX_Stereo, 0x4765F0): every sixth racer.
        let racers: Vec<_> = (0..12)
            .map(|k| AiInput {
                entry: 0,
                ctx_rpm: 3000.0,
                throttle: 0.0,
                position: Vec3::new(10.0 * k as f32, 0.0, 0.0),
                mix: Default::default(),
            })
            .collect();
        ctx.update(&mut mix, &racers, &[], (Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
        anyhow::ensure!(ctx.racer_stereos == 2, "every sixth racer plays a stereo ({} of 12)", ctx.racer_stereos);
        ctx.silence(&mut mix);
        println!(
            "road / wind voices {road}, thumps {thumps}, knocks {knocks}, whooshes {whooshes}, stereos {stereos} of 60"
        );
    }
    Ok(())
}

/// Free roam traffic, stage 0's races and Rachel's complaints closing them.
pub(super) fn stage0_races(game: &mut Game) -> Result<()> {
    // Free roam traffic: the placer puts a car a second (where the player's
    // traffic zone and the spacing allow), each driven by its lane.
    for _ in 0..600 {
        game.advance_for(1.0 / 60.0)?;
    }
    let placed =
        game.sim.traffic.as_ref().map_or(0, |t| t.cars.iter().filter(|c| c.lane.is_some() && c.plan.is_some()).count());
    println!("traffic after 10 s: {placed} cars");
    anyhow::ensure!(placed > 0, "free roam traffic placed");
    // Stage 0's races: 15 s of driving after DDAY_EVENT_A became current
    // Rachel's kind-2 hint reveals them (0x524250 → 0x53E0C0 case 2:
    // career +0x873F, then 0x527D40 opens them).
    anyhow::ensure!(game.profile.as_ref().unwrap().open.is_empty(), "no races before the reveal");
    for _ in 0..360 {
        if game.profile.as_ref().unwrap().intro_races {
            break;
        }
        game.advance_for(1.0 / 60.0)?;
    }
    let p = game.profile.as_ref().unwrap();
    println!("stage 0 races revealed after {:.1} s of driving: {:08X?}", p.drive_time, p.open);
    anyhow::ensure!(
        p.intro_races && !p.open.is_empty() && p.open.iter().all(|h| dday::STAGE0_RACES.contains(h)),
        "stage 0's races open"
    );
    // Outrun encounters are offered in stage 0 while the player drives
    // Rachel's car and the races are not closed (0x513D30).
    anyhow::ensure!(p.outruns_offered(&game.game_data.career, false, -1.0), "stage 0 outruns offered during the intro");
    // Each stage-0 race over (won or lost, 0x5242D0) brings a complaint a
    // second of driving later; the third closes the races (+0x873E).
    for n in 1..=3u8 {
        game.profile.as_mut().unwrap().hints.race_over(dday::STAGE0_RACES[0], false, false, 0, false);
        for _ in 0..70 {
            game.advance_for(1.0 / 60.0)?;
        }
        anyhow::ensure!(game.profile.as_ref().unwrap().hints.complaints == n, "complaint {n}");
    }
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        p.intro_closed && p.open.is_empty() && !p.outruns_offered(&game.game_data.career, false, -1.0),
        "Rachel closes stage 0's races and outruns"
    );
    Ok(())
}

/// D-Day A: the car lot engaged, a stock car bought, its save point.
pub(super) fn car_lot(game: &mut Game) -> Result<()> {
    // D-Day A: the car lot (CC_CAR_LOT_1), the intro's goal, engages as
    // the car enters it (0x514FD0); it wins the event and opens the lot in
    // its first state: take a stock car into the open slot.
    let (shop, at) = shop_at(game, "CC_CAR_LOT_1")?;
    put(game, at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Explore) && game.career_world.marker == Some(MarkerTarget::Shop(shop)),
        "the car lot current, no dialog yet"
    );
    anyhow::ensure!(game.engage(), "the Engage key");
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == shop),
        "the car lot's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    // The first entry plays the lot's movie (+0x20 "SCENE05") first.
    skip_shop_movie(game, "scene05")?;
    selftest_lot_hammer(game, "the career car lot")?;
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { anyhow::bail!("the car lot") };
    anyhow::ensure!(
        game.profile.as_ref().is_some_and(|p| p.hints.garage),
        "Rachel's car lot line (0x5243E0(4), once-flag +9)"
    );
    anyhow::ensure!(lot.state == LotState::Take, "take a stock car");
    println!("car lot: {:?}", lot.cars.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
    anyhow::ensure!(game.profile.as_ref().unwrap().intro == Some(career::DDAY_EVENT_B), "D-Day A won, B under way");
    // Back is refused while Rachel's car is the only one.
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "Rachel needs her car back");
    check_texts_shown(game, "Rachel needs her car back")?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::CarLot(_))), "still in the lot");
    game.menu_key(Key::Right)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "do you want this stock car");
    // The lot plays its DIM under the question (0x4EDCBC).
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.dim_under.is_some() && d.under.is_some()),
        "the lot dims under the stock-car question"
    );
    check_texts_shown(game, "do you want this stock car")?;
    // No: back to the lot, which runs its UNDIM from the answer.
    game.track_undim();
    game.menu_key(Key::Back)?;
    game.track_undim();
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::CarLot(_)))
            && game.fe.undim.is_some_and(|u| u.package == "UI_CareerCarLot" && u.undim_at.is_some()),
        "No on the stock question: the lot's UNDIM"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "do you want this stock car (again)");
    game.menu_key(Key::Accept)?;
    // The lot's save point (0x4ED945, mode 0x141; with Prompt, the fresh
    // profile's option): "Are you sure you want to save the profile named
    // …?", No focused; No goes on to the city.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::McMessage(m, _)) if m.text.starts_with("Are you sure you want to save")),
        "the car lot's save question"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "back in the city");
    let p = game.profile.as_ref().unwrap();
    println!("picked {}, cash ${}", p.car, p.cash);
    anyhow::ensure!(!p.car.is_empty() && p.slots[0].car.as_deref() == Some(p.car.as_str()), "the stock car in slot 0");
    Ok(())
}

/// Back in the city: the GPS to the garage, found once the car is on the road.
pub(super) fn gps_to_garage(game: &mut Game) -> Result<()> {
    // Back in the city (0x60A690 → 0x5412E0): the GPS to the garage; the
    // car lot's hint (0x5243E0(4)) sends SMS_LOC_GO_TO_GARAGE a second of
    // driving later.
    let crib_point = game
        .sim.route_triggers
        .find(game.game_data.career.shops.iter().find(|s| s.name == "CRIB_1").context("the garage")?.trigger)
        .context("its trigger")?
        .gps;
    anyhow::ensure!(
        game.career_world.gps.as_ref().is_some_and(|g| g.target == career::DDAY_EVENT_B && g.at == crib_point),
        "the GPS to the garage"
    );
    // The GPS must find its route once the lot's drive-out is over (user
    // report: "no signal" right after leaving the car dealer).
    for _ in 0..600 {
        if game.scenes.shop.is_none() {
            break;
        }
        game.advance_for(1.0 / 60.0)?;
    }
    for _ in 0..180 {
        game.advance_for(1.0 / 60.0)?;
    }
    let st = game.career_world.gps.as_ref().map(|g| (g.state(), g.error()));
    eprintln!(
        "selftest: GPS after the lot's drive-out {st:?}, car {:?}",
        game.sim.drive.as_ref().map(|d| d.state.position)
    );
    // At the lot's door no race record's stretch lies within 5 m (0x422B90),
    // so the search finds no start node ("Unable to contact satellite",
    // state 1); the follower (0x42A360, state 3) starts it again every
    // frame the router is idle, so the GPS finds its route once the car is
    // on the road (player report: "no signal" for good after the car dealer).
    let g = game.career_world.gps.as_ref().context("the GPS to the garage")?;
    anyhow::ensure!(
        st == Some((1, Some(0x1554_09E8))) && g.follower.state == 3 && g.router.state != 0,
        "at the door: no start node, searching again ({st:?}, follower {}, router {})",
        g.follower.state,
        g.router.state
    );
    // Driven south off the lot (the original under Wine, 2026-10-29: its
    // drive-out also ends at (−724.8, −876.5, 18.3) reading "Unable to
    // contact satellite", the router restarting from the car each pass;
    // the search that found the garage started at (−726.8, −897.4, 18.3)
    // heading south).
    if let Some(d) = game.sim.drive.as_mut() {
        d.place(Vec3::new(-726.8, -897.4, 18.3), -std::f32::consts::FRAC_PI_2);
    }
    for _ in 0..30 {
        game.advance_for(1.0 / 60.0)?;
    }
    let g = game.career_world.gps.as_ref().context("the GPS to the garage")?;
    println!(
        "GPS on the road by the lot: state {}, follower {}, {} route points",
        g.state(),
        g.follower.state,
        g.route(&game.sim.traffic.as_ref().context("routes")?.routes).len()
    );
    anyhow::ensure!(g.state() == 3 && g.follower.state == 1, "on the road the GPS finds the garage");
    anyhow::ensure!(!game.career_world.dday.running(), "no script after the movie session's first drive");
    play_shop_scene(game)?;
    for _ in 0..70 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(game.profile.as_ref().unwrap().inbox.contains(dday::SMS_LOC_GO_TO_GARAGE), "SMS_LOC_GO_TO_GARAGE");
    if !matches!(game.mode, Mode::Explore) {
        game.menu_key(Key::Back)?;
    }
    Ok(())
}

/// D-Day B: the garage ends the intro and the stage; its drive-out; stage 1's map.
pub(super) fn garage_first_visit(game: &mut Game) -> Result<()> {
    // D-Day B: the garage (CRIB_1) ends the intro and the stage.
    let (shop, at) = shop_at(game, "CRIB_1")?;
    put(game, at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(game.engage(), "the Engage key at the garage");
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == shop),
        "the garage's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    // The garage's first entry: "SCENE06" first.
    skip_shop_movie(game, "scene06")?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "the garage");
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.intro.is_none() && p.stage == 1, "intro over, stage 1");
    let crib_scene = game.garage_request().map(|g| g.platform).context("the garage's scene")?;
    game.act(Action::Explore, MenuKind::Crib)?;
    // The question is pushed over the garage: its scene stays behind it,
    // not free roam with the car parked inside the building.
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "Return to Explore Mode asks");
    check_texts_shown(game, "Return to Explore Mode?")?;
    anyhow::ensure!(
        game.garage_request().is_some_and(|g| g.platform == crib_scene),
        "the garage scene behind its dialog"
    );
    anyhow::ensure!(
        matches!(game.music_context(), MusicContext::FrontEnd | MusicContext::Stopped),
        "the garage's dialog keeps the garage's music (no new song)"
    );
    // No is focused (0x5587A0's 1 at 0x4AF644): Left to Yes.
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1),
        "Return to Explore Mode opens on No"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "into explore mode");
    // Out of the garage: the drive-out (scene type 4), the door open at
    // once and shut from frame 40.
    anyhow::ensure!(
        game.scenes.shop.as_ref().is_some_and(|r| r.scene.kind == nfsu2::shop_scene::SceneKind::Exit),
        "the garage's drive-out"
    );
    let door = game.scenes.shop.as_ref().map(|r| r.origin).context("the drive-out's frame")?;
    play_shop_scene(game)?;
    // The new chase view after it stands outside the shut door (scene y
    // 0; the car ends at y −4.8): its world push (0x444530).
    let eye =
        game.sim.drive.as_ref().map(|d| door.inverse().transform_point3(d.camera.position)).context("the chase view")?;
    println!("after the drive-out the eye is {:.2} m out of the door", -eye.y);
    anyhow::ensure!(eye.y < 0.0, "the chase eye outside the door after the drive-out ({eye:?})");
    for _ in 0..60 {
        game.advance_for(1.0 / 60.0)?;
    }
    // Stage 1's map: the announced shops (+0x51) wait to be found.
    let shops: Vec<_> = game.open_shops().iter().map(|&(s, _)| game.game_data.career.shops[s].name.clone()).collect();
    println!("open shops on the map: {shops:?}");
    anyhow::ensure!(!shops.iter().any(|s| s == "CC_BODYSHOP_1"), "the body shop not found yet");
    Ok(())
}

/// The free-roam sub-selftests, then an event's marker engaged.
pub(super) fn free_roam_checks(game: &mut Game) -> Result<()> {
    selftest_sms(game)?;
    selftest_markers(game)?;
    selftest_challenge(game)?;
    selftest_unique_part(game)?;
    selftest_world_clock(game)?;
    selftest_outrun(game)?;
    // Driving into an open event's marker trigger makes it current; the
    // Engage key opens its engage dialog. Its marker effect stands there.
    let (event, trigger) =
        game.career_world.event_starts.as_ref().and_then(|(_, s)| s.first().copied()).context("an open event trigger")?;
    put(game, trigger.position);
    game.advance_for(1.0 / 60.0)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Explore) && game.career_world.marker == Some(MarkerTarget::Event(event)),
        "the event's marker current, no dialog yet"
    );
    let lit: Vec<_> = game.sim.drive.as_ref().map_or_else(Vec::new, |d| {
        d.effects
            .markers
            .iter()
            .zip(MARKER_EFFECTS)
            .filter(|(m, _)| m.as_ref().is_some_and(|(_, e)| !e.is_empty()))
            .map(|(_, e)| e.2)
            .collect()
    });
    println!("marker effects on: {lit:?}");
    // The shops' markers: the track's placed emitters under FREE_ROAM,
    // held in career free roam (0x571170 → 0x5D7480).
    let (placed, held) = game.sim.drive.as_ref().map_or((0, false), |d| {
        (
            d.world.placed_emitters.iter().filter(|p| p.switch == nfsu2::formats::hash::bin_hash("FREE_ROAM")).count(),
            d.world.switches.contains(&nfsu2::formats::hash::bin_hash("FREE_ROAM")),
        )
    });
    println!("placed emitters under FREE_ROAM: {placed}, held {held}");
    anyhow::ensure!(placed > 0 && held, "the shops' markers run in free roam");
    anyhow::ensure!(!lit.is_empty(), "the event's marker effect");
    game.engage();
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Engage(ref e)) if e.event == event),
        "the event's engage dialog"
    );
    println!("engaged {} at its marker", game.game_data.career.events[event].name);
    game.menu_key(Key::Back)?;
    Ok(())
}

/// The body shop found and engaged; its browser and the rims' brands.
pub(super) fn body_shop_and_rims(game: &mut Game) -> Result<()> {
    // And into the Hotel Plaza body shop's route trigger: found, so open,
    // then engaged: its dialog, then the body shop.
    let (shop, at) = shop_at(game, "CC_BODYSHOP_1")?;
    put(game, at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(game.open_shops().iter().any(|&(s, _)| s == shop), "the body shop found");
    game.engage();
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == shop),
        "the shop's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    // The drive-in first (0x499470): the car on the scene's path, the
    // shop's door opening from frame 21; its end opens the shop.
    anyhow::ensure!(matches!(game.mode, Mode::Explore) && game.scenes.shop.is_some(), "the body shop's drive-in");
    let key = game.game_data.career.shops[shop].trigger;
    let door = |g: &Game| {
        g.sim.drive.as_ref().and_then(|d| d.world.doors.iter().find(|x| x.keys.contains(&key)).map(|x| x.state.frame))
    };
    anyhow::ensure!(door(game) == Some(0.0), "the body shop's door shut");
    let mut frames = 0;
    while game.scenes.shop.is_some() && frames < 200 {
        game.advance_for(1.0 / 30.0)?;
        frames += 1;
    }
    println!("drive-in: {frames} frames, the door at {:?}", door(game));
    anyhow::ensure!(
        (139..=141).contains(&frames) && door(game).is_some_and(|f| f > 70.0),
        "the drive-in played and opened the door"
    );
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("the body shop's browser") };
    let open: Vec<_> =
        b.categories.iter().filter(|c| !c.locked).map(|c| game.game_data.assets.strings.get(c.label).unwrap_or("?")).collect();
    println!("body shop: {} categories, open {:?}", b.categories.len(), open);
    // Into the first category (front bumpers), on its installed part.
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("still the browser") };
    anyhow::ensure!(b.part.is_some_and(|i| b.categories[0].parts[i].4), "in a category on the installed part");
    game.menu_key(Key::Back)?;
    // Rims open their brands (0x546530); Back returns to them.
    let rims = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == 0x1D).unwrap();
    for _ in 0..rims {
        game.menu_key(Key::Right)?;
    }
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Icons(m, MenuKind::RimBrand)) = &game.mode else { anyhow::bail!("the rim brands") };
    anyhow::ensure!(
        m.options.len() == 16 && matches!(m.options[15].action, Action::RimBrand(15)),
        "rim brands, stock open"
    );
    let open: Vec<_> = m
        .options
        .iter()
        .filter(|o| !matches!(o.action, Action::Unavailable))
        .map(|o| game.game_data.assets.strings.get(o.label).unwrap_or("?"))
        .collect();
    println!("rim brands open: {open:?}");
    // A brand: its styles, then a style's sizes.
    let first = m.options.iter().position(|o| matches!(o.action, Action::RimBrand(_))).unwrap();
    for _ in 0..first {
        game.menu_key(Key::Right)?;
    }
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("a brand's rims") };
    let styles: Vec<_> = b.categories.iter().map(|c| (c.text.clone().unwrap_or_default(), c.parts.len())).collect();
    println!("rim styles: {styles:?}");
    anyhow::ensure!(!styles.is_empty(), "styles");
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("a style") };
    anyhow::ensure!(b.part == Some(0), "in a style");
    game.menu_key(Key::Back)?;
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RimBrand))), "back to the brands");
    game.menu_key(Key::Back)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("back to the body shop") };
    anyhow::ensure!(b.sub.is_none() && b.category == rims, "on rims again");
    // Back on the body shop's strip in a career: "exit back to Explore
    // Mode?" (0x56721E, No focused, MENUFADEOUT under it); Yes leaves.
    game.menu_key(Key::Back)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.selected == 1 && d.dim_scripts.is_some()),
        "the body shop's exit question"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    while let Mode::Menu(Menu::McMessage(m, _)) = &game.mode {
        if m.buttons.len() == 2 {
            game.menu_key(Key::Right)?;
        }
        game.menu_key(Key::Accept)?;
    }
    Ok(())
}

/// The performance shop's engine parts: bought, fitted, stock, fitted again.
pub(super) fn perf_shop(game: &mut Game) -> Result<()> {
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "back to explore");
    // The performance shop's engine parts: Choose Individual Parts lists
    // the open ones; Accept asks, Yes buys and fits; Accept again says it
    // is fitted; Return To Stock takes it off; fitting it again is free.
    game.profile.as_mut().unwrap().cash = 2000;
    game.mode = game.perf_category_menu(0);
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Icons(m, MenuKind::PerfPackage)) = &game.mode else { anyhow::bail!("the engine packages") };
    // On Choose Individual Parts (0x556E70; Wine round 3B).
    anyhow::ensure!(m.selected == 0 && m.bars.len() == 3, "on Choose Individual Parts with three bars");
    let list = game.shop.perf_parts.clone().context("the parts list")?;
    let rows: Vec<_> = list.rows.iter().map(|r| (game.game_data.assets.strings.get(r.name).unwrap_or("?"), r.price)).collect();
    println!("individual engine parts: {rows:?}; torque {:.4} kN·m, power {:.1} kW", list.current.0, list.current.1);
    let row = list.rows.first().cloned().context("an engine part")?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "asked to buy the part");
    // No focused (0x55A3A8 pushes 1): Left to Yes.
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    // A branded part picks its brand first (UI_PerformanceBrandSelect).
    let mut brand = None;
    if let Mode::Menu(Menu::BrandSelect(b, _)) = &game.mode {
        brand = b.picked().first().copied();
        game.menu_key(Key::Accept)?;
    }
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(
        p.cash == 2000 - row.price && p.perf.contains(&row.id) && p.perf_owned.contains(&row.id),
        "bought and fitted"
    );
    anyhow::ensure!(brand.is_none_or(|b| p.brands.contains(&b)), "its brand on the car's list");
    anyhow::ensure!(game.shop.perf_parts.as_ref().is_some_and(|l| l.rows[0].installed), "listed as installed");
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Dialog(ref d, _)) if d.buttons.len() == 1),
        "already installed"
    );
    game.menu_key(Key::Accept)?;
    for _ in 0..4 {
        game.menu_key(Key::Right)?;
    }
    // Both questions focus No (0x55A211 / 0x55A3A8 push 1).
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(!game.profile.as_ref().unwrap().perf.contains(&row.id), "back to stock");
    for _ in 0..4 {
        game.menu_key(Key::Left)?;
    }
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    let p = game.profile.as_ref().unwrap();
    anyhow::ensure!(p.cash == 2000 - row.price && p.perf.contains(&row.id), "fitted again for free");
    println!(
        "bought {} for ${}, back to stock, fitted again",
        game.game_data.assets.strings.get(row.name).unwrap_or("?"),
        row.price
    );
    game.explore(game.here())?;
    // The graphics shop: a vinyl on the bottom layer with its colours,
    // then Purchase Setup (with some bank to pay for it).
    if let Some(p) = game.profile.as_mut() {
        p.cash += 5000;
    }
    Ok(())
}

/// The graphics shop: a vinyl with its colours, Purchase Setup.
pub(super) fn graphics_shop(game: &mut Game) -> Result<()> {
    let cash = game.profile.as_ref().map_or(0, |p| p.cash);
    // Found by driving in (an announced shop), so its vinyl groups open
    // (0x515360's kind 1 is 0x513110).
    let (shop, at) = shop_at(game, "CC_GRAPHICS")?;
    put(game, at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    game.engage();
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == shop),
        "the graphics shop's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Graphics))), "the graphics shop");
    game.menu_key(Key::Right)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::VinylLayer))), "the vinyl layers");
    game.menu_key(Key::Accept)?;
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { anyhow::bail!("the vinyl browser") };
    let open = b.categories.iter().position(|c| !c.locked).context("an open vinyl group")?;
    println!(
        "vinyl groups open: {:?}",
        b.categories
            .iter()
            .filter(|c| !c.locked)
            .map(|c| game.game_data.assets.strings.get(c.label).unwrap_or("?"))
            .collect::<Vec<_>>()
    );
    for _ in 0..open {
        game.menu_key(Key::Right)?;
    }
    game.menu_key(Key::Accept)?;
    game.menu_key(Key::Accept)?;
    // "Would you like to add this vinyl to your setup?" (0x559250), No
    // focused: Left to Yes.
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Dialog(ref d, _)) if d.selected == 1),
        "the vinyl question, No focused"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    let vinyl = game.shop.setup.get(0x40).context("the vinyl in the setup")?;
    // Its colours: each a swatch and Yes, back to the layers after the last
    // (one colour opens its swatches at once, 0x563C79).
    for _ in 0..4 {
        let Mode::Menu(Menu::Paint(p)) = &game.mode else { break };
        if p.selected.is_none() {
            game.menu_key(Key::Accept)?;
        }
        game.menu_key(Key::Accept)?;
        anyhow::ensure!(
            matches!(game.mode, Mode::Menu(Menu::Dialog(ref d, _)) if d.selected == 1),
            "the colour question, No focused"
        );
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::VinylLayer))), "back to the layers");
    game.menu_key(Key::Back)?;
    let total = game.graphics_total().unwrap_or(0);
    println!("graphics setup: {:?}, total ${total}", game.shop.setup.items);
    for _ in 0..2 {
        game.menu_key(Key::Right)?;
    }
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "the purchase question");
    // No focused (0x562C1D pushes 1).
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    let p = game.profile.as_ref().context("profile")?;
    anyhow::ensure!(game.shop.setup.is_empty() && p.installed.get(&0x40) == Some(&vinyl), "the vinyl fitted");
    anyhow::ensure!(p.cash + total.max(0) as u32 == cash, "paid {total}");
    game.menu_key(Key::Back)?;
    // "exit back to Explore Mode?", No focused (0x562F49 pushes 1).
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "out of the graphics shop");
    Ok(())
}

/// The garage engaged past the intro and its save question on the way out.
pub(super) fn garage_save_question(game: &mut Game) -> Result<()> {
    // Past the intro the garage engages like any shop: driving in only
    // makes it current, the Engage key opens "Enter Garage".
    let (shop, at) = shop_at(game, "CRIB_1")?;
    put(game, at);
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Explore) && game.career_world.marker == Some(MarkerTarget::Shop(shop)),
        "the garage current, no dialog yet"
    );
    game.engage();
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::ShopEngage(ref e)) if e.shop == shop),
        "the garage's engage dialog"
    );
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    play_shop_scene(game)?;
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "into the garage from the city");
    // The garage's INIT took the snapshot (0x4AF515); a change since asks
    // to save on the way out (0x4AF360 → 0x52A4D0 → 0x4A3100 mode 0x151),
    // as the original did after a Car Select (Wine), and
    // nothing unchanged (seen twice there). The change: the bank + 1.
    game.advance_for(1.0 / 60.0)?;
    if let Some(p) = game.profile.as_mut() {
        p.cash += 1;
        let _ = p.commit();
    }
    game.act(Action::Explore, MenuKind::Crib)?;
    answer_yes(game)?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::McMessage(..))),
        "a change in the garage asks to save ({})",
        flow_mode(&game.mode)
    );
    // No to the save question: on to the city.
    for _ in 0..4 {
        match &game.mode {
            Mode::Menu(Menu::McMessage(m, _)) => {
                anyhow::ensure!(m.buttons.len() < 2 || m.selected == 1, "the save question opens on No");
                game.menu_key(Key::Accept)?;
            }
            _ => break,
        }
    }
    if let Some(p) = game.profile.as_mut() {
        p.cash -= 1;
        let _ = p.commit();
    }
    for _ in 0..2 {
        game.advance_for(1.0 / 60.0)?;
    }
    if !matches!(game.mode, Mode::Explore) {
        play_shop_scene(game)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "back out of the garage ({})", flow_mode(&game.mode));
    Ok(())
}

/// The in-game World Map: its legend, Activate GPS and its route.
pub(super) fn in_game_map_and_gps(game: &mut Game) -> Result<()> {
    // The World Map key (code 0xC0): UI_InGame_WorldMap over paused free
    // roam; `]` zooms within the stage's records, Back resumes.
    let Some(Mode::Menu(Menu::InGameMap(mut map))) = game.in_game_map() else { anyhow::bail!("the in-game world map") };
    map.key(Key::RTrigger, None);
    anyhow::ensure!(map.zoom == 1.min(map.max_zoom), "the map zooms in");
    game.mode = Mode::Menu(Menu::InGameMap(map));
    anyhow::ensure!(!game.menu_quads(game.now()).is_empty(), "the in-game map draws");
    // The legend (0x4EE320): a row per group the markers show, the first
    // focused; Accept hides the focused group (0x4B0EF0) and drops a
    // selection in it, Accept again shows it and selects its first
    // marker. The player's icon (type 0x10) is on the map.
    {
        let Mode::Menu(Menu::InGameMap(map)) = &mut game.mode else { anyhow::bail!("the map") };
        anyhow::ensure!(!map.legend.is_empty() && map.focus == Some(0), "legend rows, the first focused");
        anyhow::ensure!(map.icons.iter().any(|i| i.group == 0x10), "the player's icon");
        let g = map.legend[0].0;
        map.key(Key::Accept, None);
        anyhow::ensure!(map.is_hidden(g) && map.selected.is_none_or(|s| map.icons[s].group != g), "the group hidden");
        map.key(Key::Accept, None);
        anyhow::ensure!(
            !map.is_hidden(g) && map.selected.is_some_and(|s| map.icons[s].group == g),
            "shown again, its marker selected"
        );
        map.key(Key::Down, None);
        anyhow::ensure!(map.focus == Some(1 % map.legend.len()), "Down moves the legend focus");
        println!(
            "map legend: {:?}",
            map.legend
                .iter()
                .map(|&(_, k)| game.game_data.assets.strings.get(screens::map_label(k)).unwrap_or("?"))
                .collect::<Vec<_>>()
        );
    }
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "Back returns to free roam");
    // Activate GPS (C, 0x4B10C0): an icon selected, "Are you sure you want
    // to engage the GPS system?" Yes engages it on the icon's entry
    // (0x529490) and the map closes; C again: "Deactivating GPS", OK
    // clears it (0x496CE0).
    let Some(mut m) = game.in_game_map() else { anyhow::bail!("the in-game world map again") };
    if let Mode::Menu(Menu::InGameMap(map)) = &mut m {
        map.key(Key::Right, game.game_data.assets.package("UI_InGame_WorldMap"));
    }
    game.mode = m;
    let Mode::Menu(Menu::InGameMap(map)) = &game.mode else { anyhow::bail!("the map") };
    let (target, trigger) = map.target().context("a selected marker")?;
    anyhow::ensure!(!map.gps, "Activate GPS offered");
    game.menu_key(Key::Button1)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("engage the GPS")),
        "the GPS question"
    );
    check_texts_shown(game, "the GPS question")?;
    // No focused (0x5587A0's 1) and the map drawn under it: Enter answers
    // No and the map stays, the GPS off (Wine).
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, o)) if d.selected == 1 && d.under.is_some_and(|u| matches!(o.get(u), Some(Mode::Menu(Menu::InGameMap(_)))))),
        "the GPS question: No focused, over the map"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::InGameMap(m)) if !m.gps) && !game.career_world.gps.as_ref().is_some_and(|g| g.on()),
        "No keeps the map, the GPS off"
    );
    game.menu_key(Key::Button1)?;
    game.menu_key(Key::Left)?;
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(matches!(game.mode, Mode::Explore), "the map closes");
    anyhow::ensure!(
        game.career_world.gps.as_ref().is_some_and(|g| g.target == target && g.at == trigger.gps && g.on()),
        "the GPS on the chosen marker"
    );
    println!("GPS from the map: {target:08X} at {:?}", trigger.gps);
    // The route search runs over the next frames: it must find a path, not
    // end in "Unable to contact satellite" / "Unable to find a path".
    for _ in 0..90 {
        game.advance_for(1.0 / 30.0)?;
    }
    let state = game.career_world.gps.as_ref().map(|g| (g.state(), g.error()));
    anyhow::ensure!(
        game.career_world.gps.as_ref().is_some_and(|g| g.error().is_none() && g.on()),
        "the GPS found its route from the map (state {state:?})"
    );
    let Some(m) = game.in_game_map() else { anyhow::bail!("the map with the GPS on") };
    anyhow::ensure!(matches!(&m, Mode::Menu(Menu::InGameMap(map)) if map.gps), "Deactivate GPS offered");
    game.mode = m;
    game.menu_key(Key::Button1)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Dialog(d, _)) if d.text.contains("Deactivating")),
        "Deactivating GPS"
    );
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        game.career_world.gps.is_none() && matches!(&game.mode, Mode::Menu(Menu::InGameMap(map)) if !map.gps),
        "the GPS off, back on the map"
    );
    game.menu_key(Key::Back)?;
    Ok(())
}

/// The sub-selftests: status, custom HUD, list openings, dialogs, pause options, customize extras, garage returns.
pub(super) fn sub_selftests(game: &mut Game) -> Result<()> {
    selftest_status(game)?;
    selftest_custom_hud(game)?;
    selftest_list_openings(game)?;
    selftest_dialogs_loading(game)?;
    // Esc: the explore pause menu; Jump to Garage.
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    println!("explore pause: {:?}", menu(game));
    selftest_pause_options(game)?;
    selftest_customize_extras(game)?;
    selftest_garage_returns(game)?;
    Ok(())
}
