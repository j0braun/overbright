use crate::*;

/// `game --bench menu [frames]`: the main menu's frames while the loads
/// and the programs made ahead run behind it (`Preload`, `ProgramsAhead`),
/// and after: each frame's update and draw (wall, the GPU not waited for:
/// what the menu's thread spends), `frames` of them (default 600; at
/// least until the work ahead is done). Prints the median, p99 and
/// longest frame of both spans and when the work ahead ended, to see
/// whether the menus stutter under it (`NFSU2_NO_PROGRAMS_AHEAD=1`,
/// `NFSU2_NO_PRELOAD=1` for the other arms; `taskset` for a small machine).
pub(crate) fn bench_menus(game: &mut Game, step: impl Fn(&mut Game) -> Result<()>, frames: usize) -> Result<()> {
    game.mode = game.icon_menu(MenuKind::Main);
    let started = Instant::now();
    let (mut during, mut after) = (Vec::new(), Vec::new());
    let mut ended = None;
    let ahead_done = |game: &Game| {
        game.loads.preload.as_ref().is_none_or(|p| {
            p.world.as_ref().is_none_or(|h| h.is_finished()) && p.car.as_ref().is_none_or(|c| c.1.is_finished())
        }) && game.loads.programs.as_ref().is_none_or(|p| p.job.is_finished())
    };
    let mut n = 0;
    while n < frames || (ended.is_none() && n < 6000) {
        let t = Instant::now();
        step(game)?;
        game.draw()?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        // The first frame makes the menu's own renderers: left out.
        if n > 0 {
            if ended.is_none() { during.push(ms) } else { after.push(ms) }
        }
        if ended.is_none() && n > 0 && ahead_done(game) {
            ended = Some((n + 1, started.elapsed().as_secs_f64() * 1000.0));
        }
        n += 1;
        // A frame every 4 ms at most, as a window's pacing would space them.
        if let Some(left) = std::time::Duration::from_millis(4).checked_sub(t.elapsed()) {
            std::thread::sleep(left);
        }
    }
    let w = game.platform.window.as_ref().unwrap();
    let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    let stats = |v: &mut Vec<f64>| -> String {
        if v.is_empty() {
            return "no frames".into();
        }
        v.sort_by(f64::total_cmp);
        let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        format!(
            "{} frames, median {:.2} ms, p99 {:.2} ms, longest {:.2} ms",
            v.len(),
            at(0.5),
            at(0.99),
            v[v.len() - 1]
        )
    };
    let (frames_ahead, ms_ahead) = ended.unwrap_or((n, started.elapsed().as_secs_f64() * 1000.0));
    println!("menu frames while the work ahead ran ({frames_ahead} frames, {ms_ahead:.0} ms): {}", stats(&mut during));
    println!("menu frames after it: {}", stats(&mut after));
    println!(
        "bench menu: work ahead {ms_ahead:.0} ms; pipelines built {}; programs ahead {}, preload {}",
        nfsu2::render::pipelines_built(),
        if game.loads.programs.is_some() { "on" } else { "off" },
        if game.loads.preload.is_some() { "on" } else { "off" },
    );
    Ok(())
}
