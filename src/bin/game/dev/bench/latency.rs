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
    // NFSU2_LATENCY_GPU=1: only the vsync-off rows on the real GPU.
    if nfsu2::env_value!("NFSU2_LATENCY_GPU").is_some_and(|v| v != "0") {
        return bench_vsync_off(game, frames);
    }
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

/// The vsync-off rows (report #349): the window loop's frames on the real
/// GPU, uncapped, the swap chain's waits as wgpu's Vulkan swap chain makes
/// them (the acquire waits for the GPU work of the frame `images` back)
/// with Low Latency off (two frames in flight, three images), as 0.7.0 had
/// it (one in flight, two images: the acquire waits for the last frame's
/// GPU work and a Mailbox image for the refresh, 60 Hz here; the pacer
/// locked to the refresh) and now (two in flight; the just-in-time wait for
/// the frame before the last one, then the integrator's sleep). Per mode:
/// frames per second, the main thread's work per frame (input to the
/// frame's end), the GPU's share of the time (the queries' GPU time, where
/// the device has them) and input → the frame done on the GPU (seen when a
/// later wait for it returns).
fn bench_vsync_off(game: &mut Game, frames: usize) -> Result<()> {
    use nfsu2::latency::Pacer;
    #[derive(Clone, Copy, PartialEq)]
    enum M {
        Off,
        Old,
        OldMailbox,
        New,
    }
    let refresh = 1.0 / 60.0;
    for (name, mode) in
        [("off", M::Off), ("0.7.0 on, immediate", M::Old), ("0.7.0 on, mailbox", M::OldMailbox), ("now on", M::New)]
    {
        let _ = nfsu2::prof::gpu::take_frames();
        let mut pacer = Pacer::default();
        pacer.set_period(if matches!(mode, M::Old | M::OldMailbox) {
            Some(refresh)
        } else {
            Pacer::for_present(false, Some(refresh))
        });
        let origin = Instant::now();
        let secs = |t: Instant| (t - origin).as_secs_f64();
        game.pacing.last = Instant::now();
        // Per frame: its input; the frames' ends as their waits saw them.
        let mut inputs: std::collections::HashMap<u64, f64> = Default::default();
        let mut done: std::collections::HashMap<u64, f64> = Default::default();
        let (mut work, mut n, mut first, mut last) = (0.0f64, 0usize, None, origin);
        let wait = |w: &WindowState, back: usize, done: &mut std::collections::HashMap<u64, f64>| {
            let len = w.in_flight.len();
            if back < len {
                let (k, index) = w.in_flight[len - 1 - back].clone();
                let t = Instant::now();
                let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
                if t.elapsed().as_secs_f64() > 0.0002 {
                    done.entry(k).or_insert_with(|| secs(Instant::now()));
                }
                return t.elapsed();
            }
            std::time::Duration::ZERO
        };
        for i in 0..frames {
            if !matches!(game.mode, Mode::Race(_)) {
                break;
            }
            if mode != M::Off {
                let due = pacer.due(secs(Instant::now()));
                nfsu2::render::display::wait_until(origin + std::time::Duration::from_secs_f64(due.max(0.0)));
            }
            let input = Instant::now();
            let interval = (input - game.pacing.last).as_secs_f32();
            game.pacing.last = input;
            let alpha = game.advance(interval)?;
            // The last frame's end to the GPU (the frame's own settle), then
            // in_flight's last entry is it: the just-in-time wait is for the
            // one before (back 1), the acquire's for the frame `images` back.
            let w = game.platform.window.as_mut().unwrap();
            w.settle();
            let w = game.platform.window.as_ref().unwrap();
            let gpu_wait = if mode == M::New { wait(w, 1, &mut done) } else { std::time::Duration::ZERO };
            let acquired = match mode {
                M::Off | M::New => wait(w, 2, &mut done),
                M::Old => wait(w, 1, &mut done),
                M::OldMailbox => {
                    let d = wait(w, 1, &mut done);
                    let now = secs(Instant::now());
                    let vblank = (now / refresh).ceil() * refresh;
                    nfsu2::render::display::wait_until(origin + std::time::Duration::from_secs_f64(vblank));
                    d + std::time::Duration::from_secs_f64(vblank - now)
                }
            };
            let a0 = Instant::now() - acquired;
            game.draw_between(alpha)?;
            let end = Instant::now();
            // This frame's end is settled by the next one under this number.
            inputs.insert(game.platform.window.as_ref().unwrap().frames_ended, secs(input));
            if i >= frames / 4 {
                work += (end - input).as_secs_f64() - (acquired + gpu_wait).as_secs_f64();
            }
            if mode != M::Off {
                let blocked = if mode == M::New { gpu_wait + acquired } else { acquired };
                if mode == M::New && nfsu2::env_value!("NFSU2_LATENCY_TRACE").is_some() && i % 20 == 0 {
                    eprintln!(
                        "frame {i}: gpu wait {:.2} ms, acquire {:.2} ms, delay {:.2} ms, work {:.2} ms",
                        gpu_wait.as_secs_f64() * 1e3,
                        acquired.as_secs_f64() * 1e3,
                        pacer.delay() * 1e3,
                        (end - input).as_secs_f64() * 1e3
                    );
                }
                pacer.record(nfsu2::latency::FrameTimes {
                    input: secs(input),
                    ready: secs(a0),
                    got: secs(a0 + blocked),
                    present: secs(end),
                });
            }
            if i >= frames / 4 {
                n += 1;
                first.get_or_insert(end);
                last = end;
            }
        }
        let lat: Vec<f64> =
            done.iter().filter_map(|(k, &t)| inputs.get(k).map(|&inp| t - inp)).filter(|&d| d > 0.0).collect();
        let span = first.map_or(0.0, |f| (last - f).as_secs_f64());
        let fps = if span > 0.0 { (n.saturating_sub(1)) as f64 / span } else { 0.0 };
        let g = nfsu2::prof::gpu::take_frames();
        let gpu = (!g.is_empty()).then(|| g.iter().sum::<f64>() / g.len() as f64);
        println!(
            "latency vsync off {name:20}: {fps:6.1} frames/s, main thread {:5.2} ms per frame, GPU {} ms per frame, input -> done on the GPU {:6.2} ms ({} frames seen)",
            work / n.max(1) as f64 * 1000.0,
            gpu.map_or("n/a".into(), |g| format!("{g:5.2}")),
            lat.iter().sum::<f64>() / lat.len().max(1) as f64 * 1000.0,
            lat.len()
        );
    }
    Ok(())
}
