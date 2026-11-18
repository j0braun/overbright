//! The command line's dispatch: the headless modes or the window.
use crate::*;
use super::command::Command;

pub(crate) fn run(command: &Command, args: &[String]) -> Result<()> {
    // A window's start side by side (`shell::start_ahead`): its devices
    // made on threads from the moment the options are in force, its event
    // loop on this thread while the game's data loads.
    let window = matches!(command, Command::Window { .. });
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
    // 0x57ECC0: the random numbers seeded from the clock at boot, so each
    // run draws its own rivals, names and grid (headless runs keep the
    // static seed).
    game.rng = nfsu2::opponents::GameRng::boot(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64 >> 8),
    );
    // The start-up's draws come after the seed (0x57ED5C before 0x57ED7C):
    // made again on the clock's state.
    game.boot_draws();
    // `--editor`: the modding tools can be opened (docs/texture-editor.md).
    if args.iter().any(|a| a == "--editor") {
        nfsu2::tools::texed::enable();
        log::info!("modding tools on: F7 opens the texture editor");
    }
    // The boot screens first (0x4D9580), unless asked to skip them.
    if !matches!(command, Command::Window { no_boot: true }) {
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
    nfsu2::fingerprint::start(game.game_data.root.clone());
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
    event_loop.run_app(&mut game)?;
    game.exit();
    Ok(())
}
