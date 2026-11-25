//! The command line's dispatch: the headless modes or the window.
use crate::*;
use super::command::Command;

pub(crate) fn run(command: &Command, args: &[String]) -> Result<()> {
    // A window's start side by side (`shell::start_ahead`): its devices
    // made on threads from the moment the options are in force, its event
    // loop on this thread while the game's data loads.
    // A map editor's simulation (`--simulate`) is headless: no window, no
    // devices, no audio.
    let simulate = MAP_TEST.get().and_then(|t| t.simulate.clone().map(|r| (t.clone(), r)));
    let window = matches!(command, Command::Window { .. }) && simulate.is_none();
    let (mut game, ahead, event_loop) = Game::new_beside(
        || window.then(super::start_ahead::StartAhead::start).flatten(),
        || {
            (window && !debug_env().headless_only).then(|| {
                let _t = nfsu2::prof::span("start: event loop");
                EventLoop::new()
            })
        },
    )?;
    game.platform.ahead = ahead;
    game.loads.waits.start_part("data loaded");
    // `--prepare`: the caches a first drive needs, made ahead (no window).
    if *command == Command::Prepare {
        return super::prepare::run(&mut game);
    }
    // The deterministic headless modes build the garage's part changes in
    // the frame (`GarageCarJob`); the window and the bench on a thread.
    #[cfg(feature = "dev-harness")]
    if command.headless(true) || debug_env().garage_sync {
        GARAGE_ASYNC.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    // The scripted modes' menu keys act at once on an icon menu's accept
    // (no FORWARD wait, `dev_hooks::instant_forward`): their key scripts
    // tap faster than a player and their state hashes stay comparable.
    // The shots keep the wait (frame sequences of the fade).
    #[cfg(feature = "dev-harness")]
    crate::dev::hooks::INSTANT_FORWARD.store(
        command.headless(true) && !matches!(command, Command::Shot(_)) || matches!(command, Command::Bench(..) | Command::Photo(..)),
        std::sync::atomic::Ordering::Relaxed,
    );
    // Surface Detail's normal maps: every headless mode has the whole
    // set before its first frame (deterministic images); the window makes
    // them in the background.
    #[cfg(feature = "dev-harness")]
    if command.headless(false) {
        nfsu2::surface_maps::set_blocking(true);
    }
    #[cfg(feature = "dev-harness")]
    if *command == Command::SelftestShowcase {
        return selftest_showcase_picture(&mut game);
    }
    #[cfg(feature = "dev-harness")]
    if *command == Command::Selftest {
        // NFSU2_AUDIO_WAV here too: the career run with sounds playing
        // (Rachel's lines then hold the intro script as in a real run).
        let wav = debug_env().audio_wav.clone();
        if wav.is_some() {
            game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
            *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
        }
        let r = selftest(&mut game);
        if let Some(path) = wav {
            write_audio_capture(&path)?;
        }
        return r;
    }
    #[cfg(feature = "dev-harness")]
    if let Command::Flow(name) = command {
        // NFSU2_FLOW_DEVICE=1 (with NFSU2_DEVICE_CHECK_OK=1): the flow on
        // the default output device, a check aid for the device path
        // (docs/formats.md "No in-game sound"). Run it only in a sandbox
        // whose only sound server is a private null sink.
        if debug_env().flow_device {
            anyhow::ensure!(
                debug_env().device_check_ok,
                "NFSU2_FLOW_DEVICE needs NFSU2_DEVICE_CHECK_OK=1 and a private sink"
            );
            let mixer = Mixer::new().context("no output device")?;
            game.audio = Some(game.new_audio(mixer));
            *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
        }
        let wav = debug_env().audio_wav.clone();
        if wav.is_some() {
            game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
            *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
        }
        if wav.is_some()
            && debug_env().audio_realtime
            && let Some(a) = game.audio.as_ref()
        {
            a.mixer.spawn_realtime(1024, |b| {
                if let Some(c) = AUDIO_CAPTURE.lock().unwrap().as_mut() {
                    c.0.extend_from_slice(b);
                }
            });
        }
        let r = flow(&mut game, name.as_deref().unwrap_or("newcareer"));
        if let Some(path) = wav {
            write_audio_capture(&path)?;
        }
        return r;
    }
    #[cfg(feature = "dev-harness")]
    if *command == Command::Playthrough {
        return playthrough::playthrough(&mut game);
    }
    #[cfg(feature = "dev-harness")]
    if let Command::Photo(dir, screen) = command {
        return photo::photo_check(
            &mut game,
            dir.as_deref().unwrap_or("photo-check"),
            screen.as_deref().unwrap_or("explore"),
            None,
        );
    }
    #[cfg(feature = "dev-harness")]
    if let Command::Bench(scene, frames) = command {
        let frames = frames.as_ref().and_then(|n| n.parse().ok()).unwrap_or(600);
        return bench(&mut game, scene.as_deref().unwrap_or("explore"), frames);
    }
    #[cfg(feature = "dev-harness")]
    if let Command::Shot(args) = command {
        let _t = nfsu2::prof::span("shot total");
        // NFSU2_AUDIO_WAV=file: the game's sounds on an offline mixer,
        // rendered along the updates' clock and written as a WAV (a check
        // aid for headless runs; shots are otherwise silent).
        let wav = debug_env().audio_wav.clone();
        if wav.is_some() {
            game.audio = Some(game.new_audio(Mixer::offline(audio_capture_rate())));
            *AUDIO_CAPTURE.lock().unwrap() = Some((Vec::new(), 0.0));
        }
        let r = shot(&mut game, &args[1], &args[2]);
        drop(_t);
        if let Some(path) = wav {
            // The voices' stops (frame, the step a cut would leave), for a
            // check that they do not click.
            if let Some(a) = game.audio.as_ref() {
                let stops: String = a.mixer.take_stops().iter().map(|(f, s)| format!("{f} {s:.5}\n")).collect();
                std::fs::write(format!("{path}.stops"), stops)?;
            }
            write_audio_capture(&path)?;
        }
        return r;
    }
    if let Some((t, run)) = &simulate {
        return crate::enh::sim_stream::run(&mut game, t, run);
    }
    // 0x57ECC0: the random numbers seeded from the clock at boot, so each
    // run draws its own rivals, names and grid (headless runs keep the
    // static seed).
    game.rng = nfsu2::opponents::GameRng::boot(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64 >> 8),
    );
    // The start-up's draws come after the seed (0x57ED5C before 0x57ED7C):
    // made again on the clock's state.
    game.boot_draws();
    // `--editor`: the modding tools can be opened (docs/modding/texture-editor.md).
    if args.iter().any(|a| a == "--editor") {
        nfsu2::tools::texed::enable();
        log::info!("modding tools on: F7 opens the texture editor");
    }
    // A test run of the desktop map editor: straight onto the race's grid
    // (or the spot in free roam) with only its mod, no boot screens.
    let test = MAP_TEST.get().cloned();
    // The map editor's and Studio's test game, and scripted windowed runs
    // (`NFSU2_KEEP_RUNNING`), run in a window nobody focuses: In the
    // background is Keep running for them.
    if test.is_some() || std::env::var_os("NFSU2_KEEP_RUNNING").is_some() {
        nfsu2::extras::force_keep_running(true);
        log::info!("in the background: keep running (test or scripted run)");
    }
    if let Some(t) = &test {
        game.start_map_test(t)?;
    } else if !matches!(command, Command::Window { no_boot: true }) {
        // The boot screens first (0x4D9580), unless asked to skip them.
        let _t = nfsu2::prof::span("start: boot screen");
        game.mode = game.boot_screen(BootStep::Blank);
    }
    // The window reads the real controllers too (hot-plug through gilrs).
    let t = nfsu2::prof::span("start: controllers");
    game.platform.pads = nfsu2::pad::host::Host::new(true);
    drop(t);
    game.apply_pad_maps();
    // Development guard: with NFSU2_HEADLESS_ONLY set (the developers' and the
    // check scripts' runs), never open a window or the audio device, so a
    // mistyped or missing mode can't play on the desktop.
    if debug_env().headless_only {
        anyhow::bail!("NFSU2_HEADLESS_ONLY is set: not opening a window (args {args:?})");
    }
    // The game files' SHA-256 fingerprint on a background thread
    // (logged, a warning for an unknown or modified edition; kept for
    // crash reports and the Mark Spot report).
    nfsu2::fingerprint::start(game.game_data.root.clone(), nfsu2::game_assets::log_report);
    // The game start's PC_Loading (0x57FF20 → 0x57FF95), under the
    // 200 ms rule ([`BootLoading`]).
    game.loads.boot_loading = Some(BootLoading::new(now_ms(game.pacing.started)));
    let event_loop = match event_loop {
        Some(made) => made?,
        None => EventLoop::new()?,
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    // 1 ms timer resolution while the game runs (Windows; the
    // frame limiter's sleeps), released when the loop ends.
    let _timer = nfsu2::platform::timer::TimerResolution::one_ms();
    match &test {
        Some(t) => run_test_loop(event_loop, &mut game, t)?,
        None => event_loop.run_app(&mut game)?,
    }
    game.exit();
    Ok(())
}

/// A test run the desktop map editor starts (docs/modding/map-editor.md, "Test
/// race"): `--map-mod <folder>` or `--map-file <path>` with `--map-race
/// <id>` or `--map-at x,y,z,heading`, and `--map-result <file>`. All opt-in: without them the
/// start is the game's own. Such a run applies only that mod (never
/// `enabled.txt`), skips the boot screens and menus, lets go of the loaded
/// career save (nothing is saved), and ends when the race ends or on Esc.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MapTest {
    /// The mod's folder under `<data>/maps` (with `--map-file`: the name
    /// the run knows the file's mod by, `store::folder_of_file`).
    pub(crate) folder: String,
    /// `--map-file`: the mod is read from this file instead of the folder
    /// (the editor's run copy of unsaved changes).
    pub(crate) file: Option<std::path::PathBuf>,
    /// The custom race to race, straight onto its grid.
    pub(crate) race: Option<String>,
    /// Free roam from this spot (world x, y, z; heading in degrees around
    /// +z from +x, the map mod format's convention).
    pub(crate) at: Option<(Vec3, f32)>,
    /// Where the run's result line is written.
    pub(crate) result: Option<std::path::PathBuf>,
    /// `--simulate` (with `--map-race`) or `--simulate-world` (with
    /// `--map-at`): the run headless, streamed (`enh::sim_stream`).
    pub(crate) simulate: Option<crate::enh::sim_stream::SimRun>,
    /// `--looks-live <folder>`: Studio Looks' live textures, the mod's
    /// `<folder>/world` and `<folder>/car`, before every other pack and
    /// watched whatever Reload Packs When Changed says
    /// (docs/modding/studio-looks.md). A test run's only.
    pub(crate) looks: Option<std::path::PathBuf>,
}

/// The test-run flags; each takes one value.
pub(crate) const MAP_FLAGS: [&str; 9] = [
    "--map-mod",
    "--map-file",
    "--map-race",
    "--map-at",
    "--map-result",
    "--stream",
    "--sim-seed",
    "--sim-seconds",
    "--looks-live",
];

/// `--language <name>`: the text language for this run (the file
/// `LANGUAGES/<name>.bin`; `qol::setup::choose_language`).
pub(crate) const LANGUAGE_FLAG: &str = "--language";

/// The language `--language` names, if given: a usage error for a missing
/// or malformed name or a flag given twice.
pub(crate) fn language_arg(args: &[String]) -> std::result::Result<Option<String>, String> {
    let mut found = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == LANGUAGE_FLAG {
            let v = args.get(i + 1).ok_or_else(|| format!("{LANGUAGE_FLAG} needs a language name"))?;
            if !nfsu2::game_assets::valid_language_name(v) {
                return Err(format!("{LANGUAGE_FLAG}: `{v}` is not a language name (letters and digits, like German)"));
            }
            if found.replace(v.clone()).is_some() {
                return Err(format!("{LANGUAGE_FLAG} given twice"));
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(found)
}

/// The language this run was asked for (`startup` sets it from
/// `--language`).
pub(crate) static LANGUAGE_ARG: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The simulation's switches (no value).
pub(crate) const SIM_SWITCHES: [&str; 2] = ["--simulate", "--simulate-world"];

/// The test run this process was started for (`startup` sets it).
pub(crate) static MAP_TEST: std::sync::OnceLock<MapTest> = std::sync::OnceLock::new();

impl MapTest {
    /// The test run the command line asks for: None without any of the
    /// flags; a usage error for a flag without its value, a malformed
    /// spot, a folder or race id the map format would refuse, or a
    /// combination that names no run.
    pub(crate) fn parse(args: &[String]) -> std::result::Result<Option<MapTest>, String> {
        let mut t = MapTest::default();
        let (mut folder, mut file, mut seen) = (None, None::<std::path::PathBuf>, false);
        let (mut mode, mut stream, mut seed, mut seconds) = (None, None, None, None);
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if SIM_SWITCHES.contains(&flag) {
                seen = true;
                let m = if flag == "--simulate" {
                    nfsu2::tools::map::sim::format::SimMode::Race
                } else {
                    nfsu2::tools::map::sim::format::SimMode::World
                };
                if mode.replace(m).is_some() {
                    return Err("--simulate / --simulate-world given twice".into());
                }
                i += 1;
            } else if MAP_FLAGS.contains(&flag) {
                seen = true;
                let v = args.get(i + 1).ok_or_else(|| format!("{flag} needs a value"))?.clone();
                let twice = |set: bool| if set { Err(format!("{flag} given twice")) } else { Ok(()) };
                match flag {
                    "--map-mod" => {
                        twice(folder.is_some())?;
                        if !nfsu2::tools::map::valid_id(&v) {
                            return Err(format!("--map-mod: `{v}` is not a map mod folder name (a-z, 0-9, - and _)"));
                        }
                        folder = Some(v);
                    }
                    "--map-file" => {
                        twice(file.is_some())?;
                        if v.is_empty() {
                            return Err("--map-file needs a path".into());
                        }
                        file = Some(v.into());
                    }
                    "--map-race" => {
                        twice(t.race.is_some())?;
                        if !nfsu2::tools::map::valid_id(&v) {
                            return Err(format!("--map-race: `{v}` is not a race id"));
                        }
                        t.race = Some(v);
                    }
                    "--map-at" => {
                        twice(t.at.is_some())?;
                        t.at = Some(parse_spot(&v).ok_or_else(|| format!("--map-at: `{v}` is not x,y,z,heading"))?);
                    }
                    "--stream" => {
                        twice(stream.is_some())?;
                        stream = Some(v);
                    }
                    "--sim-seed" => {
                        twice(seed.is_some())?;
                        seed = Some(v.parse::<u64>().map_err(|_| format!("--sim-seed: `{v}` is not a number"))?);
                    }
                    "--looks-live" => {
                        twice(t.looks.is_some())?;
                        if v.is_empty() {
                            return Err("--looks-live needs a folder".into());
                        }
                        t.looks = Some(v.into());
                    }
                    "--sim-seconds" => {
                        twice(seconds.is_some())?;
                        let n = v.parse::<f32>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 86_400.0);
                        seconds = Some(n.ok_or_else(|| format!("--sim-seconds: `{v}` is not 0..86400 seconds"))?);
                    }
                    _ => {
                        twice(t.result.is_some())?;
                        t.result = Some(v.into());
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        if !seen {
            return Ok(None);
        }
        match (folder, file) {
            (Some(_), Some(_)) => return Err("--map-mod and --map-file cannot be used together".into()),
            (Some(f), None) => t.folder = f,
            (None, Some(p)) => {
                t.folder = nfsu2::tools::map::store::folder_of_file(&p);
                t.file = Some(p);
            }
            (None, None) => {
                return Err(
                    "--map-race, --map-at, --map-result, --simulate and --looks-live need --map-mod <folder> or \
                     --map-file <path>"
                        .into(),
                );
            }
        }
        match (&t.race, &t.at) {
            (None, None) => {
                return Err("--map-mod / --map-file need --map-race <id> or --map-at x,y,z,heading".into());
            }
            (Some(_), Some(_)) => return Err("--map-race and --map-at cannot be used together".into()),
            _ => {}
        }
        use nfsu2::tools::map::sim::format::SimMode;
        match mode {
            None if stream.is_some() || seed.is_some() || seconds.is_some() => {
                Err("--stream, --sim-seed and --sim-seconds need --simulate or --simulate-world".into())
            }
            None => Ok(Some(t)),
            Some(SimMode::Race) if t.race.is_none() => Err("--simulate needs --map-race <id>".into()),
            Some(SimMode::World) if t.at.is_none() => Err("--simulate-world needs --map-at x,y,z,heading".into()),
            Some(mode) => {
                t.simulate = Some(crate::enh::sim_stream::SimRun { mode, stream, seed: seed.unwrap_or(1), seconds });
                Ok(Some(t))
            }
        }
    }

    /// The command line without the test-run flags and their values (the
    /// unknown-flag check sees the rest; a spot's value may start with `-`).
    pub(crate) fn strip(args: &[String]) -> Vec<String> {
        let mut out = Vec::with_capacity(args.len());
        let mut i = 0;
        while i < args.len() {
            if MAP_FLAGS.contains(&args[i].as_str()) || args[i] == LANGUAGE_FLAG {
                i += 2;
            } else if SIM_SWITCHES.contains(&args[i].as_str()) {
                i += 1;
            } else {
                out.push(args[i].clone());
                i += 1;
            }
        }
        out
    }
}

/// `x,y,z,heading`, all finite and within the map format's range.
fn parse_spot(s: &str) -> Option<(Vec3, f32)> {
    let v: Vec<f32> = s.split(',').map(|p| p.trim().parse::<f32>().ok()).collect::<Option<_>>()?;
    let ok = |x: f32| x.is_finite() && x.abs() <= nfsu2::tools::map::MAX_COORD;
    match v[..] {
        [x, y, z, h] if ok(x) && ok(y) && ok(z) && h.is_finite() => Some((Vec3::new(x, y, z), h)),
        _ => None,
    }
}

/// What the test run's window shows, as the end watch reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TestView {
    /// Racing (the start scene and countdown included), with the laps.
    Racing { laps: u32 },
    /// The race's results: the player's place among `of`, and time.
    Results { place: usize, of: usize, time: f32 },
    /// The pause menu (Esc), in a race or in free roam.
    Paused,
    /// Free roam.
    Driving,
    /// Anything else (a menu the run has no use for).
    Other,
}

/// How long the results stay up before the run ends by itself.
pub(crate) const RESULTS_HOLD: f32 = 8.0;

/// The test run's end: when to stop and what the result line says.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EndWatch {
    pub(crate) race: bool,
    pub(crate) laps: u32,
    pub(crate) raced: bool,
    /// The results: (place, of, time) and the clock they opened at.
    pub(crate) results: Option<((usize, usize, f32), f32)>,
    /// The line written (once).
    pub(crate) line: Option<String>,
}

impl EndWatch {
    pub(crate) fn new(t: &MapTest) -> EndWatch {
        EndWatch { race: t.race.is_some(), ..Default::default() }
    }

    /// One look at the window at `clock` seconds; true: end the run.
    pub(crate) fn observe(&mut self, view: TestView, clock: f32) -> bool {
        match view {
            TestView::Racing { laps } => {
                (self.raced, self.laps) = (true, laps);
                false
            }
            TestView::Results { place, of, time } => {
                let opened = *self.results.get_or_insert(((place, of, time), clock));
                clock - opened.1 >= RESULTS_HOLD
            }
            TestView::Paused => true,
            TestView::Driving => false,
            // The results left (Enter) or a menu after them: done.
            TestView::Other => self.results.is_some() || (self.race && self.raced),
        }
    }

    /// The result line: `finished=yes place=1 of=4 time=83.412 laps=2`,
    /// `finished=no laps=2` for a race left early, `drove=yes` after a
    /// free-roam run.
    pub(crate) fn result_line(&self) -> String {
        match (self.race, self.results) {
            (false, _) => "drove=yes".into(),
            (true, Some(((place, of, time), _))) => {
                format!("finished=yes place={place} of={of} time={time:.3} laps={}", self.laps)
            }
            (true, None) => format!("finished=no laps={}", self.laps),
        }
    }

    /// Write the result line once (through a temporary file, so the
    /// editor never reads half a line).
    pub(crate) fn write(&mut self, path: Option<&std::path::Path>) {
        if self.line.is_some() {
            return;
        }
        let line = self.result_line();
        log::info!("map test: {line}");
        if let Some(p) = path {
            let tmp = p.with_extension("tmp");
            let r = std::fs::write(&tmp, format!("{line}\n")).and_then(|()| std::fs::rename(&tmp, p));
            if let Err(e) = r {
                log::warn!("map test: result not written to {}: {e}", p.display());
            }
        }
        self.line = Some(line);
    }
}

impl Game {
    /// The test run's view of the game now.
    pub(crate) fn test_view(&self) -> TestView {
        match &self.mode {
            Mode::Race(r) => TestView::Racing { laps: r.laps },
            Mode::Explore => TestView::Driving,
            Mode::Menu(Menu::Results(r, _)) => {
                let p = r.player.unwrap_or(0);
                TestView::Results { place: p + 1, of: r.rows.len(), time: r.rows.get(p).map_or(0.0, |row| row.2) }
            }
            Mode::Menu(Menu::Icons(_, MenuKind::RacePause | MenuKind::ExplorePause)) => TestView::Paused,
            Mode::Menu(_) => TestView::Other,
        }
    }
}

/// The window's handler for a test run: the game's own, and after each
/// turn of the loop a look whether the run is over.
struct TestLoop<'a> {
    game: &'a mut Game,
    watch: EndWatch,
    result: Option<std::path::PathBuf>,
    clock: std::time::Instant,
}

impl ApplicationHandler for TestLoop<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.game.resumed(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.game.window_event(event_loop, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.watch.observe(self.game.test_view(), self.clock.elapsed().as_secs_f32()) {
            self.watch.write(self.result.as_deref());
            event_loop.exit();
            return;
        }
        self.game.about_to_wait(event_loop);
    }
}

/// Run the event loop for a test run; the result line written at its end
/// (also when the window was closed).
fn run_test_loop(event_loop: EventLoop<()>, game: &mut Game, t: &MapTest) -> Result<()> {
    let mut h = TestLoop { game, watch: EndWatch::new(t), result: t.result.clone(), clock: std::time::Instant::now() };
    let r = event_loop.run_app(&mut h);
    h.watch.write(t.result.as_deref());
    Ok(r?)
}

#[cfg(test)]
mod map_test_tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_language_flag_takes_one_name() {
        assert_eq!(language_arg(&args(&["--no-boot"])), Ok(None));
        assert_eq!(language_arg(&args(&["--language", "German", "--no-boot"])), Ok(Some("German".into())));
        assert!(language_arg(&args(&["--language"])).is_err(), "no name");
        assert!(language_arg(&args(&["--language", "../x"])).is_err(), "not a name that can sit in a path");
        assert!(language_arg(&args(&["--language", "A", "--language", "B"])).is_err(), "twice");
        assert_eq!(MapTest::strip(&args(&["--language", "German", "--no-boot"])), args(&["--no-boot"]));
    }

    #[test]
    fn the_test_run_flags_parse_and_refuse() {
        assert_eq!(MapTest::parse(&args(&["--no-boot"])), Ok(None), "no flags: the game's own start");
        let t = MapTest::parse(&args(&["--map-mod", "my-map", "--map-race", "dock_sprint", "--map-result", "r.txt"]))
            .unwrap()
            .unwrap();
        assert_eq!((t.folder.as_str(), t.race.as_deref(), t.at), ("my-map", Some("dock_sprint"), None));
        assert_eq!(t.result.as_deref(), Some(std::path::Path::new("r.txt")));
        let t = MapTest::parse(&args(&["--map-at", "-120.5,33,4.25,-90", "--map-mod", "m"])).unwrap().unwrap();
        assert_eq!(t.at, Some((Vec3::new(-120.5, 33.0, 4.25), -90.0)));
        for bad in [
            &["--map-mod"][..],
            &["--map-mod", "m"],
            &["--map-race", "r"],
            &["--map-mod", "M Y", "--map-race", "r"],
            &["--map-mod", "m", "--map-race", "r", "--map-at", "1,2,3,4"],
            &["--map-mod", "m", "--map-at", "1,2,3"],
            &["--map-mod", "m", "--map-at", "1,2,nan,4"],
            &["--map-mod", "m", "--map-at", "1e9,2,3,4"],
            &["--map-mod", "m", "--map-mod", "n", "--map-race", "r"],
        ] {
            assert!(MapTest::parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_map_file_stands_in_for_the_folder() {
        let run = "/d/maps/my-map/.editor/run/map.obmap";
        let t = MapTest::parse(&args(&["--map-file", run, "--map-race", "dock", "--map-result", "r.txt"]))
            .unwrap()
            .unwrap();
        assert_eq!((t.folder.as_str(), t.race.as_deref()), ("my-map", Some("dock")), "the run copy plays as its mod");
        assert_eq!(t.file.as_deref(), Some(std::path::Path::new(run)));
        let t = MapTest::parse(&args(&["--map-file", "/x/yard/map.obmap", "--map-at", "1,2,3,4", "--simulate-world"]))
            .unwrap()
            .unwrap();
        assert_eq!((t.folder.as_str(), t.simulate.map(|s| s.mode)), ("yard", Some(nfsu2::tools::map::sim::format::SimMode::World)));
        assert_eq!(MapTest::parse(&args(&["--map-mod", "m", "--map-race", "r"])).unwrap().unwrap().file, None);
        for bad in [
            &["--map-file"][..],
            &["--map-file", ""],
            &["--map-file", "a/map.obmap"],
            &["--map-file", "a/map.obmap", "--map-mod", "m", "--map-race", "r"],
            &["--map-file", "a/map.obmap", "--map-file", "b/map.obmap", "--map-race", "r"],
        ] {
            assert!(MapTest::parse(&args(bad)).is_err(), "{bad:?}");
        }
        // Studio Looks' live textures: a test run's only.
        let t = MapTest::parse(&args(&["--map-mod", "m", "--map-race", "r", "--looks-live", "/d/m/textures"]))
            .unwrap()
            .unwrap();
        assert_eq!(t.looks.as_deref(), Some(std::path::Path::new("/d/m/textures")));
        for bad in [
            &["--looks-live", "/d/m/textures"][..],
            &["--map-mod", "m", "--map-race", "r", "--looks-live", ""],
            &["--map-mod", "m", "--map-race", "r", "--looks-live", "a", "--looks-live", "b"],
        ] {
            assert!(MapTest::parse(&args(bad)).is_err(), "{bad:?}");
        }
        let a = args(&["--map-file", "-odd/map.obmap", "--map-race", "r", "--no-boot"]);
        assert_eq!(MapTest::strip(&a), args(&["--no-boot"]), "a path may start with -");
    }

    #[test]
    fn the_simulation_flags_parse_and_refuse() {
        use nfsu2::tools::map::sim::format::SimMode;
        let t = MapTest::parse(&args(&["--map-mod", "m", "--map-race", "r", "--simulate", "--stream", "-", "--sim-seed", "9"]))
            .unwrap()
            .unwrap();
        let run = t.simulate.unwrap();
        assert_eq!((run.mode, run.stream.as_deref(), run.seed, run.seconds), (SimMode::Race, Some("-"), 9, None));
        let t = MapTest::parse(&args(&["--simulate-world", "--map-mod", "m", "--map-at", "1,2,3,4", "--sim-seconds", "30"]))
            .unwrap()
            .unwrap();
        let run = t.simulate.unwrap();
        assert_eq!((run.mode, run.stream, run.seed, run.seconds), (SimMode::World, None, 1, Some(30.0)));
        assert_eq!(MapTest::parse(&args(&["--map-mod", "m", "--map-race", "r"])).unwrap().unwrap().simulate, None);
        for bad in [
            &["--simulate", "--map-mod", "m", "--map-at", "1,2,3,4"][..],
            &["--simulate-world", "--map-mod", "m", "--map-race", "r"],
            &["--simulate", "--simulate-world", "--map-mod", "m", "--map-race", "r"],
            &["--map-mod", "m", "--map-race", "r", "--stream", "x"],
            &["--simulate", "--map-mod", "m", "--map-race", "r", "--sim-seed", "-1"],
            &["--simulate", "--map-mod", "m", "--map-race", "r", "--sim-seconds", "0"],
            &["--simulate"],
        ] {
            assert!(MapTest::parse(&args(bad)).is_err(), "{bad:?}");
        }
        let a = args(&["--simulate", "--map-mod", "m", "--map-race", "r", "--stream", "-", "--sim-seed", "3"]);
        assert!(MapTest::strip(&a).is_empty());
    }

    #[test]
    fn the_test_run_flags_are_known_to_every_build() {
        let a = args(&["--map-mod", "m", "--map-at", "-1,-2,-3,-4", "--map-result", "-r", "--no-boot"]);
        assert_eq!(MapTest::strip(&a), args(&["--no-boot"]));
        assert_eq!(super::super::command::Command::parse(&a), super::super::command::Command::Window { no_boot: true });
    }

    #[test]
    fn a_test_race_ends_after_its_results_or_on_esc() {
        let race = MapTest { folder: "m".into(), race: Some("r".into()), ..Default::default() };
        let mut w = EndWatch::new(&race);
        assert!(!w.observe(TestView::Other, 0.0), "the start before the race");
        assert!(!w.observe(TestView::Racing { laps: 2 }, 1.0));
        let res = TestView::Results { place: 2, of: 4, time: 83.4125 };
        assert!(!w.observe(res, 90.0));
        assert!(!w.observe(res, 90.0 + RESULTS_HOLD - 0.1));
        assert!(w.observe(res, 90.0 + RESULTS_HOLD), "the results held, then the end");
        assert_eq!(w.result_line(), "finished=yes place=2 of=4 time=83.412 laps=2");
        // Enter on the results ends at once.
        let mut w = EndWatch::new(&race);
        w.observe(TestView::Racing { laps: 1 }, 0.0);
        w.observe(res, 5.0);
        assert!(w.observe(TestView::Other, 6.0));
        // Esc in the race: not finished.
        let mut w = EndWatch::new(&race);
        w.observe(TestView::Racing { laps: 3 }, 0.0);
        assert!(w.observe(TestView::Paused, 2.0));
        assert_eq!(w.result_line(), "finished=no laps=3");
        // Free roam ends on Esc only.
        let roam = MapTest { folder: "m".into(), at: Some((Vec3::ZERO, 0.0)), ..Default::default() };
        let mut w = EndWatch::new(&roam);
        assert!(!w.observe(TestView::Driving, 100.0));
        assert!(w.observe(TestView::Paused, 101.0));
        assert_eq!(w.result_line(), "drove=yes");
    }

    #[test]
    fn the_result_line_is_written_once_whole() {
        let dir = std::env::temp_dir().join(format!("ob-map-result-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("result.txt");
        let mut w = EndWatch::new(&MapTest { race: Some("r".into()), ..Default::default() });
        w.observe(TestView::Racing { laps: 1 }, 0.0);
        w.write(Some(&path));
        w.observe(TestView::Results { place: 1, of: 1, time: 1.0 }, 1.0);
        w.write(Some(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "finished=no laps=1\n");
        assert!(!dir.join("result.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
