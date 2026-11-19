//! The game's state: `struct Game` and its owner structs' roots,
//! `GameData`, and `Game::new`. The owners live with their behaviour
//! (`shell::cadence::Pacing`, `loads::Loads`, `sim::SimSession` …).
use crate::*;

/// Ray Traced reflections (docs/notes/graphics/reflections.md "Ray-traced reflections (experimental)"): the frame's
/// ray-query scene (the world, the cars where they are) for `ssr`.
/// `player`: the player's car and pose, `others`: [`rt_cars`].




pub(crate) struct Game {
    /// The game's data read at the start ([`GameData`]).
    pub(crate) game_data: GameData,
    pub(crate) profile: Option<Profile>,
    pub(crate) mode: Mode,
    /// The simulation's session ([`sim::SimSession`]).
    pub(crate) sim: sim::SimSession,
    /// The render-side state ([`render::state::RenderState`]): the
    /// drive's, cars', kept region's, crowd's and garage's GPU objects.
    pub(crate) render: render::state::RenderState,
    /// The window and input devices ([`shell::window::Platform`]).
    pub(crate) platform: shell::window::Platform,
    /// The frame clock and cadence ([`shell::cadence::Pacing`]).
    pub(crate) pacing: shell::cadence::Pacing,
    /// The loads in progress and ahead ([`loads::Loads`]).
    pub(crate) loads: loads::Loads,
    /// The harness's and the tests' state ([`dev_hooks::DevState`]).
    #[cfg_attr(not(any(test, feature = "dev-harness")), allow(dead_code))] // empty in the shipping build
    pub(crate) dev: dev_hooks::DevState,
    /// The tick snapshots S_{n−1}, S_n ([`shell::cadence::Interp`]).
    pub(crate) interp: shell::cadence::Interp,
    /// The overbright level on the tick ([`render::visuals::Visuals`]).
    pub(crate) visuals: render::visuals::Visuals,
    /// The HUD's state ([`hud_glue::HudState`]).
    pub(crate) hud_state: hud_glue::HudState,
    /// The races' flow ([`flow::RaceFlow`]).
    pub(crate) race_flow: flow::RaceFlow,
    /// The Quick Race set-up ([`menus::car_select::QuickRaceState`]).
    pub(crate) quick: menus::car_select::QuickRaceState,
    /// The front end's state between screens ([`menus::FeState`]).
    pub(crate) fe: menus::FeState,
    /// The save points and customize session ([`menus::save_flow::SaveFlow`]).
    pub(crate) save_flow: menus::save_flow::SaveFlow,
    /// The dyno and tuning ([`dyno::DynoTuning`]).
    pub(crate) dyno: dyno::DynoTuning,
    /// The career's free roam ([`career_glue::CareerWorld`]).
    pub(crate) career_world: career_glue::CareerWorld,
    /// The options (the profile's options block), saved beside the career.
    pub(crate) settings: options::Settings,
    /// The graphics card's PCI vendor and device, for the detail estimate.
    pub(crate) card: (u32, u32),
    /// The shops ([`shops::ShopState`]).
    pub(crate) shop: shops::ShopState,
    pub(crate) rng: nfsu2::opponents::GameRng,
    /// The scenes playing ([`flow::scenes::Scenes`]).
    pub(crate) scenes: flow::scenes::Scenes,
    /// The front end's garage ([`garage::GarageFe`]).
    pub(crate) garage_fe: garage::GarageFe,
    /// Photo mode (docs/playing/photo-mode.md), up over the pause menu.
    pub(crate) photo: Option<photo::Photo>,
    /// The free camera (docs/modding/map-editor.md; `--editor` only), up over the
    /// explore pause menu.
    pub(crate) free_camera: Option<crate::enh::free_camera::FreeCamera>,
    /// Controller extras (Rumble): the model and the gear it last saw.
    pub(crate) rumble: nfsu2::pad::extras::Rumble,
    pub(crate) rumble_gear: Option<usize>,
    /// Rumble: the wheels' suspension compression at the last tick.
    pub(crate) rumble_travel: Option<[f32; 4]>,
    /// Show Off and the cover shots ([`career_glue::covers::Showcase`]).
    pub(crate) showcase: career_glue::covers::Showcase,
    /// The game's sounds: opened with the window, so headless runs
    /// (`--shot`, `--selftest`) stay silent.
    pub(crate) audio: Option<Audio>,
}

/// The game's data read at the start (read-only after boot; were
/// `Game` fields; `career` was `data`).
pub(crate) struct GameData {
    pub(crate) root: PathBuf,
    pub(crate) assets: Assets,
    pub(crate) career: CareerData,
    /// The part database (GLOBALB 0x80034602).
    pub(crate) parts: nfsu2::formats::parts::PartDb,
    /// The car types (GLOBALB).
    pub(crate) car_types: Vec<nfsu2::car::CarType>,
    /// The car-type records as stored, for the shops' ratings.
    pub(crate) car_records: Vec<(String, Vec<u8>)>,
    /// The style points' trick table (GLOBALB chunk 0x34A07).
    pub(crate) tricks: Vec<nfsu2::drift::TrickDef>,
    /// The preset rides (GLOBALB 0x30220).
    pub(crate) presets: Vec<nfsu2::car::Preset>,
    /// MAGAZINES_FRONTEND.BIN's photo-shoot cameras and setups.
    pub(crate) magazines: Option<nfsu2::formats::magazines::Magazines>,
    /// G35: the run's caches of what the game files give.
    pub(crate) caches: std::sync::Arc<Caches>,
}

impl Game {
    /// The game's data and state (the headless modes' and the tests'; the
    /// window's start is [`Game::new_beside`]).
    #[cfg_attr(not(any(test, feature = "dev-harness")), allow(dead_code))]
    pub(crate) fn new() -> Result<Game> {
        Ok(Self::new_beside(|| (), || ())?.0)
    }

    /// [`Game::new`], with `early` run on this thread once the options are
    /// in force (the window's start puts its device's opening on a thread
    /// there) and `beside` run on it while the front end's packages and the
    /// career's data load on their threads (the window's event loop is made
    /// meanwhile). What the game loads and how is unchanged.
    pub(crate) fn new_beside<A, B>(early: impl FnOnce() -> A, beside: impl FnOnce() -> B) -> Result<(Game, A, B)> {
        // Unit tests build games that may save: they keep their profile and
        // options in a directory of their own, never the player's — the
        // process's (for any thread a test spawns), and each test thread
        // its own data folder and current profile, and a hold on the
        // process-wide settings (`test_isolation`, docs/developing/test-determinism.md).
        #[cfg(all(test, feature = "dev-harness"))]
        {
            static ISOLATE: std::sync::Once = std::sync::Once::new();
            ISOLATE.call_once(|| {
                let dir = std::env::temp_dir().join(format!("nfsu2-unittest-{}", std::process::id()));
                // SAFETY: done once, before any test reads the variable.
                unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
            });
            test_isolation::enter();
        }
        // The debug knobs as the environment holds them now.
        reload_debug_env();
        let settings = options::Settings::load();
        nfsu2::hd_ui_pack::initialize(settings.enh.hd_ui);
        let perf_overlay = nfsu2::overlay::Overlay::new(&settings.overlay);
        let early = early();
        let root = nfsu2::game_root();
        let _t = nfsu2::prof::span("Game::new");
        // The front end's packages and the career's data load side by side.
        let (assets, data, beside) = std::thread::scope(|s| {
            let assets = s.spawn(|| Assets::load(&root));
            let data = s.spawn(|| CareerData::load(&root));
            let beside = beside();
            (assets.join().unwrap(), data.join().unwrap(), beside)
        });
        let (assets, data) = (assets?, data?);
        let route_triggers = career::RouteTriggers::load(&root);
        let globalb = nfsu2::car::shared_file(&nfsu2::game_fs::path(&root, "GLOBAL/GLOBALB.BUN"))?;
        let parts = nfsu2::formats::parts::PartDb::load(&globalb)?;
        let car_types = nfsu2::car::parse_car_types(&globalb)?;
        let car_records = nfsu2::car::car_type_records(&globalb)?;
        let tricks = nfsu2::drift::parse_tricks(&globalb);
        let ai = match (nfsu2::ai::route::AiTuning::parse(&globalb), nfsu2::opponents::Bounds::parse(&globalb)) {
            (Some(tuning), Some(bounds)) => Some(AiData { tuning, presets: nfsu2::ai::route::Presets::parse(&globalb), bounds, curves: nfsu2::ai::route::ClassCurves::parse(&globalb) }),
            _ => None,
        };
        let presets = nfsu2::car::parse_presets(&globalb);
        let started = Instant::now();
        let menu = IconMenu::new("UI_Main", None, screens::MAIN_MENU.to_vec(), 0.0);
        // The native profiles (`profiles`): one profile on disk is loaded at
        // once, several wait for the boot's list.
        career::NativeContext::init_with(&data, &parts, &car_types);
        let boot_profiles = profiles::boot();
        if let profiles::Boot::One(name) = &boot_profiles
            && let Err(e) = profiles::load(name)
        {
            eprintln!("profile {name}: {e:#}");
        }
        let mut game = Game {
            game_data: GameData {
                root,
                assets,
                career: data,
                parts,
                car_types,
                car_records,
                tricks,
                presets,
                magazines: None,
                caches: Default::default(),
            },
            profile: Profile::load(),
            mode: Mode::Menu(Menu::Icons(menu, MenuKind::Main)),
            sim: sim::SimSession::new(route_triggers, ai),
            render: Default::default(),
            platform: shell::window::Platform::new(perf_overlay),
            pacing: shell::cadence::Pacing::new(started),
            loads: Default::default(),
            dev: Default::default(),
            interp: Default::default(),
            visuals: Default::default(),
            hud_state: Default::default(),
            race_flow: Default::default(),
            quick: Default::default(),
            fe: menus::FeState::new(boot_profiles),
            save_flow: Default::default(),
            dyno: Default::default(),
            career_world: Default::default(),
            settings,
            card: (0, 0),
            shop: Default::default(),
            rng: nfsu2::opponents::GameRng::default(),
            scenes: Default::default(),
            garage_fe: Default::default(),
            photo: None,
            free_camera: None,
            rumble: Default::default(),
            rumble_gear: None,
            rumble_travel: None,
            showcase: Default::default(),
            audio: None,
        };
        game.boot_draws();
        Ok((game, early, beside))
    }

    /// The front end's milliseconds on ticks: the package
    /// clock (`nfsu2::game_clock`, 20, 19, 20 package ms per 1/60 s tick,
    /// 40 per 1/30 s scene tick, 0x5662D0) over the FEng rate 1.2, so a
    /// screen's script time 1.2 × (now − its start) is the package clock's.
    /// Every screen, menu timer and HUD package reads this; it moves only
    /// in `Game::update`, never with the wall clock or a drawn frame, so it
    /// is the same at every display rate. The harness may pin it
    /// (`Pacing::clock`). Wall time is read for presentation only
    /// (`now_ms(started)` / `Instant`: docs/original/uncapped-fps.md, "Game time").
    pub(crate) fn now(&self) -> f32 {
        match self.pacing.clock {
            Some(c) => (c * 1000.0) as f32,
            None => self.pacing.clocks.fe_ms(),
        }
    }

    /// The real-time clock 0x8651A8 (1/4000 s) as this tick and the draw
    /// after it read it (`GameClock::real_tick`): what the original's C++
    /// menu timers stamp and test. A clock the harness pinned
    /// (`Pacing::clock`) gives its own time, as `now` does.
    pub(crate) fn real_now(&self) -> u64 {
        match self.pacing.clock {
            Some(c) => (c * 4000.0).max(0.0).round() as u64,
            None => self.pacing.clocks.real_tick,
        }
    }

    /// [`Game::now`] in seconds.
    pub(crate) fn seconds(&self) -> f32 {
        match self.pacing.clock {
            Some(c) => c as f32,
            None => self.pacing.clocks.fe_ms() / 1000.0,
        }
    }

}
