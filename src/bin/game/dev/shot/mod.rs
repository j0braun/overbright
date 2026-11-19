//! `--shot <screen> <out.png>` (docs/checks.md): the screenshot harness.
//! Transitional: `shot` is one long fn, moved whole (the maintainer's private notes); its
//! target table is cut into files later.
use crate::*;

mod env;
pub(crate) mod fit;
pub(crate) mod hdr;
pub(crate) mod targets;

use env::ShotEnv;
use targets::ShotCtx;

/// When the options shots' pages open (and their keys are pressed), ms
/// before the shot's time 0: the highlighted row's HIGHLIGHT (UI_Options
/// OPTION_NAME_n / OPTION_DATA_n: a 1.2 s loop of the colour
/// 138,244,204 whose alpha goes 0 → 255 over 0–440 ms, holds to 760 and
/// falls back to 0 at 1200) is then on its hold. At time 0 its alpha is 0
/// and the highlighted row's text was blank in the shots (as in the game
/// the instant a row is highlighted).
pub(crate) const OPTIONS_SHOT_AT: f32 = -600.0;

/// Headless: draw one screen to a PNG.
pub(crate) fn shot(game: &mut Game, screen: &str, out: &str) -> Result<()> {
    // Check targets (scripts/checks): `photo`, photo mode's first frame
    // (the photo check's help-bar still, free roam); `explore_rain`, free
    // roam in the shower at world clock 162 s (the bench's rain scene).
    if screen == "photo" {
        let dir = nfsu2::frontend::profiles::data_home().join("photo-shot");
        return photo::photo_check(game, &dir.to_string_lossy(), "explore", Some(out));
    }
    let screen = if screen == "explore_rain" {
        SHOT_RAIN_CLOCK.store(162f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
        "explore"
    } else {
        screen
    };
    let env = ShotEnv::from_env();
    let s = setup(game, screen, &env)?;
    let cx = ShotCtx { gpu: &s.gpu, format: s.format, screen: s.screen, env: &env };
    game.mode = targets::run(game, &cx)?;
    let keys = post_setup(game, s.screen, s.cust)?;
    let drawn = render_offscreen(game, &s, &env)?;
    write_outputs(game, s, keys, drawn, out)
}

/// What [`setup`] made for the rest of the shot.
struct Setup<'s> {
    gpu: Gpu,
    format: wgpu::TextureFormat,
    ui: UiRenderer,
    /// The screen without its prefix and suffix variants.
    screen: &'s str,
    /// `<screen>_cust`: reached from the main menu's Customize.
    cust: bool,
    width: u32,
    height: u32,
}

/// The shot's state before its target's setup: the target size, the
/// clock, the GPU, the profile (and its variants), the shop and the
/// tuning session.
fn setup<'s>(game: &mut Game, screen: &'s str, env: &ShotEnv) -> Result<Setup<'s>> {
    // NFSU2_SHOT_SIZE=WxH: another target size (default 1280x960), e.g. a
    // wide one to check how the front end's 640×480 canvas stretches.
    let (width, height) = env.size.unwrap_or((1280u32, 960u32));
    nfsu2::qol::set_output_size((width, height));
    headless_aspect(game);
    fake_hdr();
    // NFSU2_SHOT_CLOCK=s (a measuring aid; docs/perf-gates.md): the
    // game's clock pinned at s seconds for the whole shot, as the bench
    // pins it, so the front-end shots that otherwise animate on the wall
    // clock (main, career, options, worldmap, ending) come out the same
    // every run and can be compared byte for byte. Unset: the wall clock,
    // as before.
    if let Some(c) = std::env::var("NFSU2_SHOT_CLOCK").ok().and_then(|v| v.parse::<f64>().ok()) {
        game.pacing.clock = Some(c);
    }
    // The script's controllers present at the start (`NFSU2_PAD_SCRIPT`).
    game.platform.pads.poll(0.0);
    game.apply_pad_maps();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    // NFSU2_SHOT_FORMAT=rgba16float|rgb10a2: the target in that window format (an HDR-capable or
    // 10-bit swap chain) instead of 8-bit RGBA; the PNG is its RGBA8 read-back, and a float target also
    // prints how much of the frame lies over white (`shot target:` line), which an 8-bit target clamps away.
    let format = match std::env::var("NFSU2_SHOT_FORMAT").as_deref() {
        Ok("rgba16float") => wgpu::TextureFormat::Rgba16Float,
        Ok("rgb10a2") => wgpu::TextureFormat::Rgb10a2Unorm,
        _ => wgpu::TextureFormat::Rgba8Unorm,
    };
    let ui = UiRenderer::new(&gpu, format);
    // NFSU2_UNLOCK_ALL=1: the unlock-all byte 0x838464 set, to compare
    // with an original whose byte was poked (`career::UNLOCK_ALL`).
    if std::env::var("NFSU2_UNLOCK_ALL").is_ok_and(|v| v == "1") {
        nfsu2::frontend::career::UNLOCK_ALL.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // NFSU2_CHEATS="yodogg,gottaedge" (or "all"): those codes typed at the
    // title, for shots of what they open.
    if let Ok(codes) = std::env::var("NFSU2_CHEATS") {
        type_cheats(&codes);
    }
    // NFSU2_SHOT_PRESET=Original|Balanced|High|Ultra|Extreme: a Graphics
    // page preset (its name on the page); NFSU2_SHOT_LEVEL=Low|Medium|High:
    // an Enhanced level. Both as the bench's, before NFSU2_SHOT_ENH.
    if let Ok(name) = std::env::var("NFSU2_SHOT_PRESET") {
        let preset = nfsu2::qol::PRESETS
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(&name))
            .with_context(|| format!("no preset {name}"))?;
        preset.apply(&mut game.settings.enh);
        game.settings.apply_enhancements();
    }
    if let Ok(name) = std::env::var("NFSU2_SHOT_LEVEL") {
        let level = nfsu2::qol::LEVELS
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(&name))
            .with_context(|| format!("no level {name}"))?;
        level.apply(&mut game.settings.enh);
        game.settings.apply_enhancements();
    }
    // NFSU2_SHOT_ENH="enh.hud_scale 150;enh.fov 20": Enhancements page
    // values (options.txt lines) for the shot, to check them.
    if let Ok(lines) = std::env::var("NFSU2_SHOT_ENH") {
        for line in lines.split(';') {
            let (k, v) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
            // `custom.N V`: a World Detail custom value (options.txt's
            // line), e.g. `custom.7 0` the road reflection off.
            if let Some(n) = k.strip_prefix("custom.").and_then(|n| n.parse::<usize>().ok())
                && let (Some(slot), Ok(v)) = (game.settings.custom.get_mut(n), v.parse())
            {
                *slot = v;
                continue;
            }
            if game.settings.overlay.parse(k, v) {
                continue;
            }
            game.settings.enh.parse(k, v);
        }
        game.settings.apply_enhancements();
    }
    // `ui2d`: the Enhancements page with HD UI Art on.
    if screen == "ui2d" {
        game.settings.enh.hd_ui = true;
        game.settings.apply_enhancements();
    }
    // `garage_neon` (and NFSU2_SHOT_SAVE=file for any shot): the profile
    // from a native save, by default the finished-career save
    // (tests/fixtures/native_saves/career_complete.sav, cars with
    // underglow in career slots 0 and 4). NFSU2_SHOT_SLOT=i: that career
    // slot as the current car (`garage_neon`: slot 0, the Corolla's WHITE
    // PULSE underglow).
    let shot_save = match std::env::var_os("NFSU2_SHOT_SAVE").and_then(|f| std::fs::read(f).ok()) {
        Some(b) => Some(b),
        None if screen == "garage_neon" => {
            Some(crate::dev::private::require(crate::dev::private::FINISHED_CAREER, "--shot garage_neon (career_complete.sav)")?)
        }
        None => None,
    };
    if let (Some(bytes), Some(ctx)) = (shot_save, career::NativeContext::get()) {
        let s = nfsu2::formats::native_save::NativeSave::parse(&bytes)?;
        // The profile's own options too, as a real load applies them
        // (`Settings::load` on the current profile; units, maps, volumes).
        game.settings.apply_native(&s);
        let mut p = Profile::from_native(&s, ctx);
        // The current profile as a real load makes it (the Career menu's
        // Resume reads it, as `load_finished_career` does).
        profiles::set_current(&p.name, s.clone());
        let slot = std::env::var("NFSU2_SHOT_SLOT")
            .ok()
            .and_then(|v| v.parse().ok())
            .or((screen == "garage_neon").then_some(0));
        if let Some(i) = slot {
            p.select_slot(i);
        }
        game.profile = Some(p);
    }
    // `finished_career_<screen>`: the screen with the finished-career save (the
    // original's own save, dev::private::FINISHED_CAREER) as the profile.
    let screen = match screen.strip_prefix("finished_career_") {
        Some(rest) => {
            load_finished_career(game)?;
            rest
        }
        None => screen,
    };
    if game.profile.is_none() {
        // NFSU2_SHOT_NAME: the profile's name (the help bar shows it).
        let mut p = Profile::new(&std::env::var("NFSU2_SHOT_NAME").unwrap_or_else(|_| "Player".into()));
        // NFSU2_SHOT_CAR: the profile's car (default the 206), e.g. 240SX
        // for a rear-wheel-drive ending burnout.
        p.skip_intro(&game.game_data.career, &game.game_data.root, &std::env::var("NFSU2_SHOT_CAR").unwrap_or_else(|_| "PEUGOT".into()), &mut game.rng);
        game.profile = Some(p);
    }
    // `<screen>_dday`: the profile without a car of its own, so the garage
    // and the drive show Rachel's car, the DDAY_PLAYER_CAR preset (350Z
    // with Racinghart rims, a shared spoiler and exhaust).
    // `<screen>_kit`: that car as the profile's, with the first
    // aftermarket roof scoop and mirrors fitted too.
    let screen = match screen.strip_suffix("_kit") {
        Some(base) => {
            if let (Some(p), Some(x)) =
                (game.profile.as_mut(), game.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR))
            {
                p.car = x.car.clone();
                p.installed = x.installed();
                for slot in [0x07usize, 0x20] {
                    let mut after = None;
                    while let Some(part) = game.game_data.parts.find_after(&x.car, slot, after) {
                        after = Some(part);
                        if part.level > 0 {
                            p.installed.insert(slot as u8, part.hash);
                            break;
                        }
                    }
                }
            }
            base
        }
        None => screen,
    };
    // `<screen>_audio`: Rachel's car with the top trunk audio layout and
    // the first part of each trunk audio component slot (0x23–0x2E).
    let screen = match screen.strip_suffix("_audio").filter(|b| !matches!(*b, "options" | "pause")) {
        Some(base) => {
            if let (Some(p), Some(x)) =
                (game.profile.as_mut(), game.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR))
            {
                p.car = x.car.clone();
                p.installed = x.installed();
                for slot in 0x22usize..=0x2E {
                    let mut after = None;
                    let mut best = None;
                    while let Some(part) = game.game_data.parts.find_after(&x.car, slot, after) {
                        after = Some(part);
                        if part.level > 0 && (slot == 0x22 || best.is_none()) {
                            best = Some(part.hash);
                        }
                    }
                    if let Some(h) = best {
                        p.installed.insert(slot as u8, h);
                    }
                }
            }
            base
        }
        None => screen,
    };
    // `<screen>_spin`: Rachel's car on the first spinner rims (a rim part
    // with a second model group, front and rear).
    let screen = match screen.strip_suffix("_spin") {
        Some(base) => {
            if let (Some(p), Some(x)) =
                (game.profile.as_mut(), game.game_data.presets.iter().find(|x| x.name == career::DDAY_PLAYER_CAR))
            {
                p.car = x.car.clone();
                p.installed = x.installed();
                for slot in [0x1Dusize, 0x1E] {
                    let mut after = None;
                    while let Some(part) = game.game_data.parts.find_after(&x.car, slot, after) {
                        after = Some(part);
                        if game.game_data.parts.solids(part, &x.car).len() > 1 {
                            p.installed.insert(slot as u8, part.hash);
                            break;
                        }
                    }
                }
            }
            base
        }
        None => screen,
    };
    // `<screen>_cust`: the screen as reached from the main menu's
    // Customize (game mode 0x20, no career shop: SHOWROOM).
    let (screen, cust) = match screen.strip_suffix("_cust") {
        Some(base) => (base, true),
        None => (screen, false),
    };
    let screen = match screen.strip_suffix("_dday") {
        Some(base) => {
            if let Some(p) = game.profile.as_mut() {
                p.car.clear();
                p.installed.clear();
            }
            base
        }
        None => screen,
    };
    // The career shops' screens as seen in the shop (0x86344C set): its
    // platform scene and light setup (`Game::frontend_scene`). `<screen>`
    // alone for the body and performance shops' screens is the shop;
    // `crib_parts` is the garage's own parts storage on CRIB01.
    let shop_kind = match screen {
        "carlot" | "carlot_next" | "carlot_exit" | "carlot_stock" => Some(4),
        "bodyshop" | "parts" | "parts_bumper" | "parts_hood" | "rims" | "rimstyles" | "rimsizes" | "mouse_parts" => {
            Some(2)
        }
        "graphics" | "paint" | "paint_base" | "paint_engine" | "vinyls" | "vinyls_sponsor" | "decals"
        | "mouse_paint" => Some(1),
        "perf" | "perfpackage" | "perfparts" | "perfparts_new" | "perf_part_ask" | "brandselect"
        | "brandselect_part" | "brandselect_pkg" | "mouse_perf" => Some(3),
        "spinners" => Some(5),
        _ => None,
    };
    game.shop.visit =
        shop_kind.filter(|_| !cust).and_then(|k| game.game_data.career.shops.iter().find(|s| s.kind == k)).map(|s| s.hash);
    // NFSU2_SHOT_SHOP=<shop name>: the screen as seen in that shop (its
    // platform scene), e.g. the specialties' lists in IND_CAR_SPECIALTY_2.
    if let Ok(name) = std::env::var("NFSU2_SHOT_SHOP") {
        game.shop.visit = Some(
            game.game_data.career.shops.iter().find(|s| s.name == name).with_context(|| format!("NFSU2_SHOT_SHOP {name}"))?.hash,
        );
    }
    // A career shop's screen as entered from free roam: no Visual Rating
    // widget up yet (`rating_up`).
    if game.shop.visit.is_some() {
        game.fe.rating_up.set(false);
    }
    if cust {
        game.save_flow.customize = Some(None);
        if let Some(p) = game.profile.as_mut() {
            p.scratch = true;
        }
    }
    let screen = if screen == "crib_parts" { "parts" } else { screen };
    if std::env::var("NFSU2_FIT").is_ok() {
        install_shot_fit(game);
    }
    // The tuning session's screens: over the car on the dyno track.
    if matches!(
        screen,
        "sliders"
            | "graph"
            | "nos"
            | "tune_tip"
            | "tune_drivetrain"
            | "tune_aero"
            | "tune_tires"
            | "tune_ecu"
            | "tune_turbo"
            | "tune_nos"
            | "tune_reset"
            | "dynochart"
            | "dynochart_zoom"
            | "dynochart2"
            | "dynochart_end"
            | "dynoresults"
            | "dynorun"
            | "dynorun_results"
            | "tuning"
    ) {
        game.dyno.tuning_session = true;
        game.load_dyno_track()?;
    }
    Ok(Setup { gpu, format, ui, screen, cust, width, height })
}

/// [`post_setup`]'s keys: when they were sent and how long after them the still is.
struct Keys {
    keys_at: f32,
    after_keys: Option<f32>,
    /// The help bar's cycle start (the cycle steps on ticks; the
    /// still's time puts it where the continuous cycle stands then).
    help_start: Option<f32>,
}

/// After the target's setup: NFSU2_SHOT_FIT, the game mode, the
/// NFSU2_SHOT_KEYS sent and the help bar's cycle.
fn post_setup(game: &mut Game, screen: &str, cust: bool) -> Result<Keys> {
    // NFSU2_SHOT_FIT="slot:i[,slot:i]": the profile car's i-th part of
    // each slot (database order, hex or decimal slot) fitted, e.g. "9:12"
    // a split hood for the hood animations.
    if let Ok(list) = std::env::var("NFSU2_SHOT_FIT") {
        for item in list.split(',') {
            let Some((slot, i)) = item.split_once(':') else { continue };
            let slot = slot
                .strip_prefix("0x")
                .map_or_else(|| slot.parse().ok(), |h| usize::from_str_radix(h, 16).ok())
                .unwrap_or(0);
            let i: usize = i.parse().unwrap_or(0);
            if let Some(p) = game.profile.as_mut() {
                let mut after = None;
                let mut n = 0;
                while let Some(x) = game.game_data.parts.find_after(&p.car, slot, after) {
                    after = Some(x);
                    if n == i {
                        log::info!("shot fit slot {slot:#x}: {:08X} kind {} level {}", x.hash, x.kind, x.level);
                        p.installed.insert(slot as u8, x.hash);
                        break;
                    }
                    n += 1;
                }
            }
        }
    }
    // g_GameMode for the shot: the main menu's own screens are in game
    // mode 2 (UI_Main's constructor), Customize's 0x20; the rest is the
    // career's (1).
    let base = if screen == "customize_display" { "options" } else { screen.split('_').next().unwrap_or(screen) };
    game.race_flow.game_mode = match base {
        "main" | "career" | "qrmode" | "online" | "lan" | "quickrace" | "qropts" | "qrcars" | "qrsponsor"
        | "options" | "controls" | "controls_capture" | "jukebox" | "credits" | "mc" | "profile" | "splash"
        | "movie" | "wheel" | "dialog" => 2,
        "customize" | "custcars" => 0x20,
        // The online flow (0x4F98B0).
        "ealogin" => 8,
        _ if cust => 0x20,
        _ => 1,
    };
    // NFSU2_SHOT_KEYS="Right,Right,Accept": menu keys sent to the shot's
    // screen before the still (to reach a list entry or a row as the
    // original is driven under Wine).
    // NFSU2_SHOT_KEYS_GAP_MS: the keys that far apart on the front end's
    // clock (pinned from the first key), and NFSU2_SHOT_AFTER_KEYS_MS: the
    // still that long after the first key (frame sequences of a step's
    // scripts, e.g. an arrow's BLINK).
    let key_gap: Option<f64> = std::env::var("NFSU2_SHOT_KEYS_GAP_MS").ok().and_then(|v| v.parse().ok());
    let after_keys: Option<f32> = std::env::var("NFSU2_SHOT_AFTER_KEYS_MS").ok().and_then(|v| v.parse().ok());
    let keys_at = game.now();
    if key_gap.is_some() || after_keys.is_some() {
        game.pacing.clock = Some(f64::from(keys_at) / 1000.0);
    }
    if let Ok(keys) = std::env::var("NFSU2_SHOT_KEYS") {
        for k in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let key = match k {
                "Left" => Key::Left,
                "Right" => Key::Right,
                "Up" => Key::Up,
                "Down" => Key::Down,
                "Accept" | "Return" => Key::Accept,
                "Back" | "Escape" => Key::Back,
                "LTrigger" => Key::LTrigger,
                "RTrigger" => Key::RTrigger,
                "Button1" => Key::Button1,
                other => anyhow::bail!("NFSU2_SHOT_KEYS: unknown key {other}"),
            };
            // A frame's garage first (the front end's scene stays under
            // a dialog as the last frame left it).
            let _ = game.garage_request();
            // The frame's widget state (`rating_up`) as play leaves it.
            let _ = game.shop_rating_quads(game.now());
            game.menu_key(key)?;
            if let (Some(gap), Some(c)) = (key_gap, game.pacing.clock) {
                game.pacing.clock = Some(c + gap / 1000.0);
            }
        }
    }
    // The help bar's cycle (`HelpCycle`) from the shot's moment, or
    // NFSU2_SHOT_HELP_MS earlier (e.g. 1000: the cash shown).
    let mut help_start = None;
    if game.race_flow.game_mode == 1 {
        let back = std::env::var("NFSU2_SHOT_HELP_MS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        help_start = Some(game.now() - back);
        game.fe.help_cycle = Some(screens::HelpCycle::started(game.now() - back, game.now()));
    }
    Ok(Keys { keys_at, after_keys, help_start })
}

/// The track-map widgets (World Map, Quick Race's track select:
/// `MapWidget`, stepped on the tick) as the RACE ticks from the front
/// end's time now to the still's `at` (ms) leave them: a shot draws its
/// still without ticks (as `HelpCycle::started` does for the help bar).
fn settle_map_widgets(game: &mut Game, at: f32) {
    let track = match &game.mode {
        Mode::Menu(Menu::WorldMap(map)) => game.world_map_track(map),
        Mode::Menu(Menu::QuickRace(qr)) => qr.track().map(|t| t.number),
        _ => return,
    };
    let mut real = game.real_now();
    let end = real + (f64::from((at - game.now()).max(0.0)) * 4.0) as u64;
    let widget = match &mut game.mode {
        Mode::Menu(Menu::WorldMap(map)) => &mut map.widget,
        Mode::Menu(Menu::QuickRace(qr)) => &mut qr.widget,
        _ => return,
    };
    for i in 0.. {
        let dt = [30u32, 29, 31][i % 3] as f32 * (1.0f32 / 1800.0);
        real += u64::from(nfsu2::game_clock::clock_4000(dt));
        if real > end {
            break;
        }
        widget.step(real, dt, track);
    }
}

/// [`render_offscreen`]'s frame for [`write_outputs`].
struct Offscreen {
    target: wgpu::Texture,
    view: wgpu::TextureView,
    post: PostFx,
    hud_at: f32,
    overlay: Vec<Quad>,
    rear_image: Option<wgpu::TextureView>,
    cleared: bool,
    garage_drawn: bool,
}

/// The 3D frame: the world scene (`draw_world_scene`) or the garage, then
/// the post passes, into the shot's target.
fn render_offscreen(game: &mut Game, s: &Setup, env: &ShotEnv) -> Result<Offscreen> {
    let &Setup { ref gpu, format, width, height, .. } = s;
    // The renderers of cars the setup dropped (the window's frame sweeps too).
    game.render.sweep();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    // The shot's settings, read once (its world's display and
    // switches from `NFS_DETAIL` below).
    let opts = nfsu2::render::RenderOptions::in_force();
    // The 3D scene goes to the post passes' back buffer first.
    let mut post = PostFx::new(&gpu, format);
    // NFSU2_MSAA=n, or the Graphics page's MSAA: the scene at that many
    // samples (the shots' scenes are otherwise not multisampled).
    let samples = if std::env::var("NFSU2_MSAA").ok().and_then(|v| v.parse::<u32>().ok()).is_some_and(|n| n > 1)
        || opts.msaa_override().is_some()
    {
        nfsu2::render::display::msaa_samples(&game.settings.custom, &gpu, &opts)
    } else {
        1
    };
    // Match the window's allocation limits, including MSAA's pixel budget.
    // SSAA sets scene resolution independently of the output/HUD size.
    let scene_size = opts.scene_size((width, height), samples);
    let scene = post.scene_view(&gpu, scene_size, &opts);
    let mut cleared = false;
    let mut in_game = false;
    let mut game_frame = PostFrame::GAME;
    let mut glow_depth = None;
    let mut ssao = None;
    let mut ssr = None;
    let mut volumetric = None;
    let mut car_air = None;
    nfsu2::render::car_lights::publish(Vec::new(), opts.car_lights_next_gen);
    // The HUD first, as the window's frame: its update decides whether
    // the rear-view mirror (view 3) is drawn.
    let hud_at: f32 = env.ms.unwrap_or(5000.0);
    let overlay = game.overlay_quads(hud_at);
    let mut rear_image = None;
    let over_world = game.tuning_menu_over_world() || world_under(&game.mode, game.sim.drive.is_some());
    let aid_target = game.gps_aid_target();
    let rollers = game.dyno.rollers.as_ref().filter(|_| game.on_dyno_track()).map(|r| r.transforms());
    let dyno_view = game.tuning_menu_over_world() && game.on_dyno_track();
    let shot_mover = game.view_history_key();
    if let (true, Some(d)) = (over_world, game.sim.drive.as_mut()) {
        if dyno_view {
            let (at, turn) = game.dyno.rest.unwrap_or((d.state.position, d.state.orientation));
            d.camera = dyno_camera(at, turn, game.dyno.distance);
        }
        // A track loaded by the screen itself (the tuning session's).
        if game.render.world.is_none() {
            game.render.world = Some(WorldRenderer::new(&gpu, &d.world, format, &opts));
            game.render.cars.set(&d.car_id, CarRenderer::new(&gpu, &d.car, format, &opts));
        }
        // NFSU2_RAIN_CLOCK=<s>: the world clock (0x7FB718) set to that time
        // five seconds before the still, and the weather stepped up to it
        // (the camera and the car held), to compare a shower with the
        // original's at the same clock.
        if let Some(c) = rain_clock() {
            d.time_of_day = TimeOfDay::default().after(c - 1.0);
            d.clock = c - 5.0;
            // The sky cycle as the original's after that long in the world
            // (0x60FAB0 steps it only through `clock % 300 == 60`, so a
            // jumped clock must replay it): night from about 61 s on.
            d.time_of_day = TimeOfDay::default().after(d.clock - 1.0);
            for _ in 0..300 {
                d.time_of_day.update(d.clock, nfsu2::weather::STEP);
                d.clock += nfsu2::weather::STEP;
                nfsu2::weather::set_clock(&d.world, d.clock);
                let frame = d.weather_frame();
                d.weather.step(&d.world, &frame, nfsu2::weather::STEP, &mut game.rng);
            }
        }
        let base_depth = create_depth(&gpu, scene_size.0, scene_size.1);
        let (scene, depth) =
            post.multisampled(&gpu, scene_size, samples, &opts).unwrap_or_else(|| (scene.clone(), base_depth.clone()));
        game.render.world.as_ref().unwrap().set_samples(&gpu, samples, &opts);
        let encoder = gpu.device.create_command_encoder(&Default::default());
        let aspect = width as f32 / height as f32;
        // `NFS_DETAIL` presets the custom values as the Level Of Detail does.
        let detail = std::env::var("NFS_DETAIL").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        let mut shot_display = Display::from_custom(&nfsu2::frontend::options::custom_presets(detail));
        // NFSU2_SHOT_NO_CAR_SHADOW=1 (comparison aid): the cars'
        // shadow texture off (Customize Display's Car Shadow).
        if std::env::var("NFSU2_SHOT_NO_CAR_SHADOW").is_ok_and(|v| v == "1") {
            shot_display.shadows = false;
        }
        // The world's scene as `Game::draw` draws it (`draw_world_scene`),
        // with the shot's differences: no paused race, no
        // traffic in the headlight pools and glows, the flares and glows
        // not gated on Light Glow, the detail from `NFS_DETAIL`, the clock
        // at 0, a still's shutter; one encoder submitted in place.
        let inputs = FrameInputs {
            aspect,
            size: scene_size,
            samples,
            ssao_texel_scale: scene_size.1 as f32 / height.max(1) as f32,
            out_size: (width, height),
            // The shaders' clock at 0, the blur's at 1 s, the world clock
            // as the drive stands (a still is its own tick).
            clocks: crate::render::clocks::DrawClocks::still(0.0, 1.0, Some(d.clock)),
            settings: nfsu2::render::FrameSettings { display: shot_display, switches: Switches::from_detail(detail), opts },
            gate_glow: false,
            paused_fallback: false,
            pool_traffic: TrafficSet::None,
            glow_traffic: TrafficSet::None,
            rear_view: game.hud_state.rear_view_on,
            photo_open: false,
            aid_target,
            rollers,
            mover: shot_mover,
            still: Some(nfsu2::render::blur_dof::SHUTTER as f64),
            blur_after_upscale: false,
            format,
            stepped: None,
        };
        // NFS_REAR_PNG=path: the mirror's image (view 3) as drawn.
        let mut rear_png = |gpu: &Gpu, r: &WorldRenderer| -> Result<()> {
            if let (Ok(path), Some(texture)) = (std::env::var("NFS_REAR_PNG"), r.rear_view_texture()) {
                let (w, h) = nfsu2::render::REAR_VIEW_SIZE;
                image::save_buffer(
                    &path,
                    &nfsu2::render::read_back(gpu, texture, w, h),
                    w,
                    h,
                    image::ColorType::Rgba8,
                )?;
            }
            Ok(())
        };
        // A still's visuals stepped once at its camera and drawn
        // at once, as every frame did before the tick form.
        crate::render::visuals::still_step(d, &inputs.settings, inputs.gate_glow);
        // A still is drawn on its tick (α = 1).
        let poses = crate::render::snapshot::FramePoses::live(Some(&*d));
        let mut camera = d.camera;
        let ctx = SceneCtx {
            gpu: &gpu,
            post: &mut post,
            color: &scene,
            depth: &depth,
            d,
            camera: &mut camera,
            render: &mut game.render,
            game: SceneGame {
                poses: &poses,
                mode: &game.mode,
                paused_race: game.sim.paused_race.as_ref(),
                traffic: game.sim.traffic.as_ref(),
                encounters: game.sim.encounters.as_ref(),
                shop_scene: game.scenes.shop.as_ref(),
                scene_crowd: game.scenes.crowd.as_ref(),
                settings: &game.settings,
            },
            encoder,
            path: ScenePath::Still { on_rear_view: &mut rear_png },
        };
        let out = draw_world_scene(ctx, &inputs)?;
        game_frame = out.game_frame;
        ssao = out.ssao;
        ssr = out.ssr;
        volumetric = out.volumetric;
        car_air = out.car_air;
        rear_image = out.rear_image;
        cleared = true;
        in_game = true;
        glow_depth = Some(depth);
    }
    let depth = create_depth(&gpu, scene_size.0, scene_size.1);
    // The garage as it stands NFSU2_SHOT_MS after the shot's menu asked
    // for its view and parts (the camera blend, the car's turn, the hood
    // or trunk opening).
    let shot_ms: f32 = env.ms.unwrap_or(5000.0);
    let garage = game.garage_request().map(|r| GarageRequest { now: r.asked + shot_ms / 1000.0, ..r });
    // The garage scene at the same samples.
    let (garage_color, garage_depth) =
        post.multisampled(&gpu, scene_size, samples, &opts).unwrap_or_else(|| (scene.clone(), depth.clone()));
    let garage_drawn = draw_garage(
        garage,
        &game.game_data.root,
        &game.game_data.caches,
        &game.game_data.career.packages,
        &mut game.render.garage,
        &gpu,
        format,
        &garage_color,
        &garage_depth,
        scene_size,
        1.0,
        Display::ALL,
        &opts,
        samples,
        Some(&mut post),
    );
    if garage_drawn {
        cleared = true;
        in_game = false;
        // NFS_REFLECTION_PNG=path: the floor reflection image (view 4) too.
        if let (Ok(path), Some((texture, (w, h)))) = (
            std::env::var("NFS_REFLECTION_PNG"),
            game.render.garage.as_ref().and_then(|g| g.renderer.as_ref()).and_then(|r| r.reflection_image()),
        ) {
            image::save_buffer(&path, &nfsu2::render::read_back(&gpu, texture, w, h), w, h, image::ColorType::Rgba8)?;
        }
    }
    if cleared {
        // A still of a running game at the top detail (5): the streak
        // buffer settled over 16 frames.
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let renderer = if in_game { game.sim.drive.as_ref().and(game.render.world.as_ref()) } else { None };
        let glow_view = glow_depth.as_ref();
        let detail = std::env::var("NFS_DETAIL").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        // NFS_NO_POST=1: the frame before the screen passes, for
        // comparison with an apitrace snapshot of the original.
        let switches = if std::env::var("NFS_NO_POST").is_ok_and(|v| v == "1") {
            Switches::from_detail(0)
        } else {
            Switches::from_detail(detail)
        };
        let rain = game
            .sim.drive
            .as_ref()
            .filter(|_| in_game)
            .map(|d| (d.rain_frame(d.fog.rain_color, None, opts.lens_drops), d.world.textures.get(&nfsu2::formats::hash::bin_hash("RAINDROPPC"))));
        match rain {
            Some((frame, tex)) => post.set_rain(&gpu, frame, tex),
            None => post.set_rain(&gpu, Default::default(), None),
        }
        post.set_output_size((width, height));
        post.set_ssao(if in_game { ssao.take() } else { None });
        post.set_ssr(if in_game { ssr.take() } else { None });
        post.set_volumetric(if in_game { volumetric.take() } else { None });
        post.set_car_lights(if in_game { car_air.take() } else { None });
        post.apply(
            &gpu,
            &opts,
            &mut encoder,
            &view,
            switches,
            if in_game { game_frame } else { PostFrame::FRONT_END },
            std::env::var("NFSU2_STREAK_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(16),
            false,
            &mut |e, streak, resolve| {
                if let (Some(r), Some(depth)) = (renderer, glow_view.as_ref()) {
                    r.draw_glows_resolved(e, streak, resolve, depth, true);
                }
            },
        );
        gpu.queue.submit([encoder.finish()]);
    }
    Ok(Offscreen { target, view, post, hud_at, overlay, rear_image, cleared, garage_drawn })
}

/// The front end's quads over the frame, the PNG (and the HDR outputs).
fn write_outputs(game: &mut Game, s: Setup, keys: Keys, drawn: Offscreen, out: &str) -> Result<()> {
    let Setup { gpu, format, mut ui, width, height, .. } = s;
    let Keys { keys_at, after_keys, help_start } = keys;
    let Offscreen { target, view, mut post, hud_at, overlay, rear_image, cleared, garage_drawn } = drawn;
    for t in game.platform.ui_uploads.drain(..) {
        ui.add_texture(&gpu, &t);
    }
    draw_cover(
        &game.mode,
        &mut game.showcase.cover_car,
        &mut game.showcase.cover_picture,
        &gpu,
        format,
        &mut ui,
        &game.game_data.assets,
        &nfsu2::render::FrameSettings::new(&game.settings.custom, nfsu2::render::RenderOptions::in_force()),
    );
    upload_movie(&mut game.mode, &gpu, &mut ui);
    // NFSU2_SHOT_MS: the front end's time for the shot (default 5000 ms),
    // for frame sequences of the FEng scripts.
    let at = after_keys.map_or(hud_at, |ms| keys_at + ms);
    let mut quads = overlay;
    if let Some(image) = rear_image.as_ref() {
        ui.set_texture_clamped(&gpu, nfsu2::rear_view::TEXTURE, image);
        quads.insert(0, nfsu2::rear_view::quad());
    }
    if let (Some(start), Some(_)) = (help_start, game.fe.help_cycle) {
        game.fe.help_cycle = Some(screens::HelpCycle::started(start, at));
    }
    settle_map_widgets(game, at);
    nfsu2::render::native::ui::geom_dump::begin_layout();
    quads.extend(game.menu_quads(at));
    quads.extend(game.loading_quads(at));
    // NFSU2_SHOT_FPS=1 (minimal) / standard / full: the performance
    // overlay with made-up frame figures and this machine's metrics now,
    // to check it (docs/perf-overlay.md); the overlay's options from
    // NFSU2_SHOT_ENH (`overlay.position 1;overlay.scale 150`).
    if let Ok(layout) = std::env::var("NFSU2_SHOT_FPS")
        && let Some(font) = game.game_data.assets.fonts.values().find(|f| f.name.contains("tt21i"))
    {
        let mut o = nfsu2::overlay::Overlay::default();
        o.shown = nfsu2::overlay::Layout::from_index(match layout.as_str() {
            "standard" | "2" => 2,
            "full" | "3" => 3,
            _ => 1,
        });
        let info = gpu.adapter.get_info();
        o.pci = Some((info.vendor, info.device));
        o.fake_frames_for_shot(&game.settings.overlay);
        nfsu2::overlay::record_load(214.0);
        let display = nfsu2::overlay::DisplayInfo {
            present: "Mailbox".into(),
            latency: 2,
            low_latency: nfsu2::extras::low_latency(),
            frame_cap: game.settings.enh.frame_cap.into(),
            size: (width, height),
            adapter: format!("{} ({:?})", info.name, info.backend),
        };
        o.quads(&game.game_data.assets, font, || game.settings.deviations(), || display, &game.settings.overlay, &mut quads);
    }
    // NFS_NO_UI=1: the 3D scene alone, as an apitrace snapshot taken
    // before the front end's quads.
    if std::env::var("NFS_NO_UI").is_ok_and(|v| v == "1") {
        quads.clear();
    }
    nfsu2::render::display::set_scene_capped(fe_scene_capped(&game.mode, garage_drawn));
    ui.render(&gpu, &game.game_data.assets, &view, (width, height), &quads, !cleared);
    nfsu2::render::display::set_scene_capped(false);
    let rgba = if format == wgpu::TextureFormat::Rgba8Unorm {
        nfsu2::render::read_back(&gpu, &target, width, height)
    } else {
        if format == wgpu::TextureFormat::Rgba16Float {
            over_white(&nfsu2::render::read_back(&gpu, &target, width, height));
        }
        nfsu2::render::read_back_rgba8(&gpu, &target)?
    };
    image::save_buffer(out, &rgba, width, height, image::ColorType::Rgba8)?;
    println!("wrote {out}");
    // HDR Output headless (`NFSU2_HDR_FAKE=pq|scrgb` with `enh.hdr_output
    // 1`): the display transform of this frame too, as `<out>.hdr.png`
    // (16-bit PNG of the PQ signal; scRGB is written as PQ) and a
    // tone-mapped preview `<out>.hdr-preview.png`.
    if let Some(signal) = nfsu2::qol::hdr_output_signal().filter(|_| nfsu2::qol::hdr_output_active()) {
        // The Display page is the calibration screen (as in `redraw`).
        let pattern = crate::render::frame::hdr_pattern(
            &game.mode,
            &game.settings,
            &game.game_data.assets,
            (width, height),
            std::env::var("NFSU2_HDR_PATTERN").is_ok_and(|v| v == "1"),
        );
        nfsu2::qol::set_hdr_pattern(pattern);
        // For the resolution matrix's check (scripts/checks/resolutions.py):
        // the pattern's pixel box.
        match pattern {
            Some(r) => println!(
                "hdr pattern: px {:.1} {:.1} {:.1} {:.1}",
                r[0] * width as f32,
                r[1] * height as f32,
                r[2] * width as f32,
                r[3] * height as f32
            ),
            None => println!("hdr pattern: none"),
        }
        let unclipped = post.take_unclipped();
        hdr_shot(&gpu, &view, unclipped.as_ref(), signal, (width, height), out)?;
    }
    Ok(())
}

/// A 16-bit float target's texels over white: the share of pixels with a channel over 1, the
/// brightest channel, and the mean of the light over 1 (what an 8-bit target clamps away).
fn over_white(raw: &[u8]) {
    fn f16(h: u16) -> f32 {
        let (s, e, m) = ((h >> 15) as i32, ((h >> 10) & 0x1F) as i32, (h & 0x3FF) as f32);
        let v = match e {
            0 => m * 2f32.powi(-24),
            31 => f32::INFINITY,
            _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
        };
        if s == 1 { -v } else { v }
    }
    let (mut n, mut over, mut max, mut excess) = (0usize, 0usize, 0f32, 0f64);
    for t in raw.chunks_exact(8) {
        let c = (0..3).map(|i| f16(u16::from_le_bytes([t[2 * i], t[2 * i + 1]]))).fold(0f32, f32::max);
        n += 1;
        if c > 1.0 {
            over += 1;
            excess += f64::from(c - 1.0);
        }
        max = max.max(c);
    }
    println!(
        "shot target: over white {:.3} % of pixels, brightest channel {max:.3}, mean excess {:.5}",
        100.0 * over as f64 / n.max(1) as f64,
        excess / n.max(1) as f64
    );
}
