use crate::*;

/// The process's age in ms: now on the boot clock minus the kernel's start
/// time of the process (`/proc/self/stat` field 22, in clock ticks, so to
/// 10 ms). Counts the time before `main` too (the loader, the libraries).
/// None where the kernel does not say.
pub(crate) fn process_age_ms() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        // The fields after the command's closing bracket: state is the first.
        let start: f64 = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()?;
        // SAFETY: plain libc calls with a valid out pointer.
        let (ticks, now) = unsafe {
            let mut t = std::mem::zeroed::<libc::timespec>();
            if libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut t) != 0 {
                return None;
            }
            (libc::sysconf(libc::_SC_CLK_TCK) as f64, t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9)
        };
        (ticks > 0.0).then(|| (now - start / ticks) * 1000.0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// `game --bench boot`: what the game's start costs up to its first frames,
/// without a window (the window, its surface and the audio device are not
/// opened here; the windowed game logs its own `game start: done after`).
///
/// * the process's age when the bench is entered (`entered`: the age and
///   the moment): the loader and `Game::new` (the front end's packages, the
///   career's data);
/// * the GPU device and the offscreen window state (made by the caller);
/// * the first boot screen's first frame (LS_BlankMovie), drawn and done on
///   the GPU: the first frame a player would be shown;
/// * Enter pressed every frame until the title screen (UG_LS_Splash) is up
///   and its first frame is drawn and done: the shortest way to the title;
/// * each boot screen's own first frame after that (its movie opened, the
///   first picture decoded and uploaded).
pub(crate) fn bench_boot(
    game: &mut Game,
    step: impl Fn(&mut Game) -> Result<()>,
    entered: (Option<f64>, Instant),
) -> Result<()> {
    use crate::flow::movies::BootStep;
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    let wait = |game: &Game| {
        let w = game.platform.window.as_ref().unwrap();
        let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    };
    // One frame: its update, its draw, the GPU's wait (ms).
    let frame = |game: &mut Game| -> Result<(f64, f64, f64)> {
        let t = Instant::now();
        step(game)?;
        let update = ms(t);
        let t = Instant::now();
        game.draw()?;
        let draw = ms(t);
        let t = Instant::now();
        wait(game);
        Ok((update, draw, ms(t)))
    };
    let (age, at) = entered;
    let age_of = |t: Instant| {
        age.map_or("unknown".to_string(), |a| format!("{:.0}", a + t.duration_since(at).as_secs_f64() * 1000.0))
    };
    println!(
        "boot: process start to the bench's entry (the loader, Game::new): {} ms",
        age.map_or("unknown".to_string(), |a| format!("{a:.0}"))
    );
    println!("boot: GPU device and window state: {:.0} ms", ms(at));
    let builds = super::Builds::now();
    let pipelines = nfsu2::render::pipelines_built();
    game.mode = game.boot_screen(BootStep::Blank);
    let (update, draw, gpu) = frame(game)?;
    let first = Instant::now();
    println!(
        "boot: the first frame (LS_BlankMovie): update {update:.1} ms, draw {draw:.1} ms, GPU wait {gpu:.1} ms; process start to it drawn: {} ms; builds {}",
        age_of(first),
        super::Builds::now().since(&builds)
    );
    // Enter every frame until the title screen is up.
    let t = Instant::now();
    let mut frames = 0;
    while !matches!(game.mode, Mode::Menu(Menu::Splash(_))) && frames < 600 {
        game.menu_key(Key::Accept)?;
        step(game)?;
        game.draw()?;
        frames += 1;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Splash(_))),
        "the title screen after {frames} frames of Enter"
    );
    let (update, draw, gpu) = frame(game)?;
    let title = Instant::now();
    println!(
        "boot: Enter to the title screen: {frames} frames, {:.0} ms, then its first frame: update {update:.1} ms, draw {draw:.1} ms, GPU wait {gpu:.1} ms; process start to the title drawn: {} ms; pipelines {}",
        ms(t),
        age_of(title),
        nfsu2::render::pipelines_built() - pipelines
    );
    // Enter on until the front end's garage scene shows (the main menu, or
    // the screen a start without a profile opens on): the frame that makes
    // it, and its builds (`NFSU2_BENCH_BOOT_MENU=1`).
    if std::env::var_os("NFSU2_BENCH_BOOT_MENU").is_some() {
        let t = Instant::now();
        let all = super::Builds::now();
        let mut frames = 0;
        let mut last = (0.0, String::new());
        while game.render.garage.is_none() && frames < 600 {
            game.menu_key(Key::Accept)?;
            let f = Instant::now();
            let before = super::Builds::now();
            step(game)?;
            game.draw()?;
            wait(game);
            last = (ms(f), super::Builds::now().since(&before));
            frames += 1;
        }
        let ahead = game.loads.programs.as_ref().map_or("none", |p| if p.job.is_finished() { "done" } else { "running" });
        println!(
            "boot: Enter to the garage scene: {frames} frames, {:.0} ms (process start to it: {} ms); the frame that made it {:.1} ms with the GPU's wait, its builds {}; builds since the title {}; programs ahead {ahead}",
            ms(t),
            age_of(Instant::now()),
            last.0,
            last.1,
            super::Builds::now().since(&all)
        );
    }
    // Each boot screen's own first frame (opened now; the title's and the
    // blank movie's a second time).
    for boot in BootStep::ALL {
        let t = Instant::now();
        game.mode = game.boot_screen(boot);
        let opened = ms(t);
        let (update, draw, gpu) = frame(game)?;
        let name = boot.movie().map_or("UG_LS_Splash", |m| m.0);
        println!(
            "boot: {name}: opened in {opened:.1} ms, first frame update {update:.1} ms, draw {draw:.1} ms, GPU wait {gpu:.1} ms"
        );
    }
    println!(
        "bench boot: first frame {} ms, title {} ms after the process's start (entry {} ms, GPU {:.0} ms)",
        age_of(first),
        age_of(title),
        age.map_or("unknown".to_string(), |a| format!("{a:.0}")),
        first.duration_since(at).as_secs_f64() * 1000.0
    );
    Ok(())
}
