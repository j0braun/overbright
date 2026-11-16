use crate::*;

/// `--bench scene [frames]`: the frame limiter's pacing in a race start
/// scene (`display::frame_period`, 1/30 s while the scene holds the cars),
/// on the real clock: each frame waits as the window's does, steps by its
/// measured time and draws, two frames in flight (the swap chain's
/// latency). Prints the frame intervals in the capped part and after it.
pub(crate) fn bench_scene_pacing(game: &mut Game, frames: usize) -> Result<()> {
    if game.scenes.shop.is_none() {
        anyhow::bail!("the race has no start scene");
    }
    game.pacing.clock = Some(0.0);
    game.pacing.last = Instant::now();
    let (mut capped, mut free) = (Vec::new(), Vec::new());
    let mut n = 0u64;
    for _ in 0..frames.max(2) {
        if !matches!(game.mode, Mode::Race(_)) {
            break;
        }
        let cap = game.scene_frame_cap();
        if let Some(period) = nfsu2::render::display::frame_period(cap) {
            nfsu2::render::display::wait_until(game.pacing.last + period);
        }
        let now = Instant::now();
        let interval = (now - game.pacing.last).as_secs_f64();
        game.pacing.last = now;
        if n > 0 {
            if cap { &mut capped } else { &mut free }.push(interval * 1000.0);
        }
        // The window's path: the ticks the measured frame
        // holds on the tick clock.
        game.pacing.clock = game.pacing.clock.map(|c| c + interval);
        game.advance(interval as f32)?;
        game.draw()?;
        n += 1;
        let w = game.platform.window.as_mut().unwrap();
        while let Some((k, index)) = w.in_flight.front().cloned() {
            if k + 2 > w.frames_ended {
                break;
            }
            w.in_flight.pop_front();
            let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
        }
    }
    let report = |name: &str, v: &mut Vec<f64>| {
        if v.is_empty() {
            println!("{name}: no frames");
            return;
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        v.sort_by(f64::total_cmp);
        let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        println!(
            "{name}: {} frames, mean {mean:.3} ms ({:.2} fps), sd {sd:.3} ms, min {:.3}, p50 {:.3}, p99 {:.3}, max {:.3} ms",
            v.len(),
            1000.0 / mean,
            v[0],
            at(0.5),
            at(0.99),
            v[v.len() - 1]
        );
    };
    report("scene (capped)", &mut capped);
    report("after the scene's cut", &mut free);
    Ok(())
}
