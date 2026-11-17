use crate::*;

/// `game --bench rims [steps]` (the frame rate dropped sharply while
/// picking different rims): UI_Rims_Browser's style strip from the main
/// menu's Customize (SHOWROOM), stepped `steps` times (default 30, Right
/// then Left at the strip's ends), each step's frame and the 9 after it
/// drawn and the GPU waited for. Prints each step's frame ms and the
/// average / worst of the steps' frames and of the frames in between;
/// with NFSU2_TIMING the `garage:` spans too.
pub(crate) fn bench_rims(game: &mut Game, step: impl Fn(&mut Game) -> Result<()>, steps: usize) -> Result<()> {
    let steps = if steps == 600 { 30 } else { steps };
    let wait = |game: &Game| {
        let w = game.platform.window.as_ref().unwrap();
        let _ = w.gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    };
    let frame = |game: &mut Game| -> Result<f64> {
        let t = Instant::now();
        step(game)?;
        game.draw()?;
        wait(game);
        Ok(t.elapsed().as_secs_f64() * 1000.0)
    };
    game.shop.visit = None;
    game.save_flow.customize = Some(None);
    if let Some(p) = game.profile.as_mut() {
        p.scratch = true;
    }
    let brand = (0..game.rim_brands().len())
        .max_by_key(|&i| match game.rims_browser(i, None, None) {
            Mode::Menu(Menu::Parts(b)) => b.categories.len(),
            _ => 0,
        })
        .context("a rim brand")?;
    game.mode = game.rims_browser(brand, None, None);
    for _ in 0..120 {
        frame(game)?;
    }
    let ride = |game: &mut Game| game.garage_request().and_then(|r| r.installed.get(&0x1D).copied());
    let (mut picks, mut between, mut lags) = (Vec::new(), Vec::new(), Vec::new());
    let mut key = Key::Right;
    for i in 0..steps {
        let before = ride(game);
        let t = Instant::now();
        game.menu_key(key)?;
        if ride(game) == before {
            key = if key == Key::Right { Key::Left } else { Key::Right };
            game.menu_key(key)?;
        }
        let input = t.elapsed().as_secs_f64() * 1000.0;
        let ms = frame(game)? + input;
        let after = ride(game);
        // The frames drawn before the garage shows the step's rim (0: the
        // step's own frame).
        let shown = |game: &Game| {
            game.render.garage.as_ref().and_then(|g| g.car.as_ref()).and_then(|c| c.1.get(&0x1D).copied()) == after
        };
        let mut late = (!shown(game)).then_some(0);
        for n in 1..=9 {
            between.push(frame(game)?);
            if late.is_some_and(|l| l + 1 == n) && !shown(game) {
                late = Some(n);
            }
        }
        let late = late.map_or(0, |l| l + 1);
        println!(
            "rim step {i:2}: {ms:6.1} ms (key {input:.1} ms) front rim {after:08x?}, shown after {late} more frames"
        );
        lags.push(late);
        picks.push(ms);
    }
    let stats = |v: &[f64]| (v.iter().sum::<f64>() / v.len().max(1) as f64, v.iter().cloned().fold(0.0, f64::max));
    let (pa, pw) = stats(&picks);
    let (ba, bw) = stats(&between);
    println!(
        "rims: {steps} steps (brand {brand}): step frame avg {pa:.1} ms, worst {pw:.1} ms; other frames avg {ba:.2} ms, worst {bw:.2} ms; the rim shown after at most {} more frames",
        lags.iter().max().unwrap_or(&0)
    );
    Ok(())
}
