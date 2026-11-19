use crate::*;

/// `--bench latency [frames]` (docs/playing/modern-extras.md): the window
/// loop's own frames (`Game::advance` on the real clock, `draw_between`,
/// the autopilot driving the racing bench's race) through a model of the
/// swap chain and panel (`latency::Display`: FIFO 60 Hz, FIFO 144 Hz, VRR
/// 144 Hz; two frames queued, one with Low Latency), with Low Latency off
/// and on (its pacer, its VRR cap). Per frame that took a world update
/// (the input is read at updates): input sample → the frame on the GPU
/// and presented ("to present"), and → the refresh that shows it in the
/// model ("to photon"). Headless: no real swap chain, so the display's
/// wait is the model's (slept for real, so the pacer sees it as it would
/// the acquire's).
pub(crate) fn bench_latency(game: &mut Game, frames: usize) -> Result<()> {
    use nfsu2::latency::{Display, Pacer, Sample};
    let frames = frames.max(120);
    game.pacing.clock = None;
    for (name, hz, vrr) in [("fifo 60 Hz", 60.0, false), ("fifo 144 Hz", 144.0, false), ("vrr 144 Hz", 144.0, true)] {
        for low in [false, true] {
            let mut display = Display::new(hz, vrr, if low { 1 } else { 2 });
            let mut pacer = Pacer::default();
            pacer.set_period(Some(1.0 / hz));
            let cap = if low && vrr { nfsu2::latency::vrr_period(Some(hz)) } else { None };
            let origin = Instant::now();
            let secs = |t: Instant| (t - origin).as_secs_f64();
            game.pacing.last = Instant::now();
            let mut samples: Vec<Sample> = Vec::new();
            let mut photons = Vec::new();
            let mut parts = [0.0f64; 4];
            for _ in 0..frames {
                if !matches!(game.mode, Mode::Race(_)) {
                    break;
                }
                if let Some(c) = cap {
                    nfsu2::render::display::wait_until(game.pacing.last + c);
                }
                if low {
                    let due = pacer.due(secs(Instant::now()));
                    nfsu2::render::display::wait_until(origin + std::time::Duration::from_secs_f64(due.max(0.0)));
                }
                let input = Instant::now();
                let interval = (input - game.pacing.last).as_secs_f32();
                game.pacing.last = input;
                let alpha = game.advance(interval)?;
                let advanced = Instant::now();
                game.draw_between(alpha)?;
                // The acquire (the model's display; slept for real).
                let ready = Instant::now();
                parts[0] += (advanced - input).as_secs_f64();
                parts[1] += (ready - advanced).as_secs_f64();
                let got = display.acquire(secs(ready));
                let got_at = origin + std::time::Duration::from_secs_f64(got.max(0.0));
                nfsu2::render::display::wait_until(got_at);
                // The frame submitted, presented and done on the GPU.
                let w = game.platform.window.as_mut().unwrap();
                w.settle();
                let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                let present = Instant::now();
                parts[2] += (present - got_at.max(ready)).as_secs_f64();
                parts[3] += 1.0;
                let photon = display.present(secs(present));
                photons.push(photon);
                let blocked = (got - secs(ready)).max(0.0) as f32;
                if low {
                    pacer.record(nfsu2::latency::FrameTimes {
                        input: secs(input),
                        ready: secs(ready),
                        got: got.max(secs(ready)),
                        present: secs(present),
                    });
                }
                if game.pacing.frame_updates > 0 {
                    samples.push(Sample {
                        to_present: (present - input).as_secs_f32(),
                        to_photon: (photon - secs(input)) as f32,
                        blocked,
                    });
                }
            }
            let skip = samples.len() / 5;
            let mut p: Vec<f32> = samples[skip..].iter().map(|s| s.to_photon).collect();
            p.sort_by(f32::total_cmp);
            let p95 = p.get((p.len() as f32 * 0.95) as usize).copied().unwrap_or(0.0);
            let shown = photons.windows(2).filter(|w| w[1] > w[0]).count() as f64;
            let span = photons.last().copied().unwrap_or(0.0) - photons.first().copied().unwrap_or(0.0);
            let missed = photons.windows(2).filter(|w| w[1] - w[0] > 1.5 / hz).count();
            println!(
                "latency {name:12} low latency {:3}: input->present {:6.2} ms, input->photon {:6.2} ms (p95 {:6.2}), blocked {:5.2} ms, {:6.1} frames/s shown, {missed} gaps > 1.5 refresh, {} updates sampled",
                if low { "on" } else { "off" },
                nfsu2::latency::mean(&samples, skip, |s| s.to_present) * 1000.0,
                nfsu2::latency::mean(&samples, skip, |s| s.to_photon) * 1000.0,
                p95 * 1000.0,
                nfsu2::latency::mean(&samples, skip, |s| s.blocked) * 1000.0,
                if span > 0.0 { shown / span } else { 0.0 },
                samples.len()
            );
            let n = parts[3].max(1.0);
            println!(
                "  cpu: advance {:.2} ms, draw {:.2} ms, end + gpu {:.2} ms per frame",
                parts[0] / n * 1000.0,
                parts[1] / n * 1000.0,
                parts[2] / n * 1000.0
            );
        }
    }
    Ok(())
}
