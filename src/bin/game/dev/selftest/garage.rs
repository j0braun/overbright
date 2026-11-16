use crate::*;

/// A player report ("the car stays in that position when
/// returning to the car preview"), checked against the original under
/// Wine: Back from a body part's list and from the rim sizes
/// returns the camera to the base view (0x55EF90 / 0x55FCF0 →
/// 0x4C2140) while the car keeps the part view's angle and steer, and
/// a step on the rim styles asks the wheel view again (0x55F900). The
/// keys go through `menu_key` and the view through `garage_request`, as
/// in play; 1/60 s frames on the game's clock, given back after.
pub(crate) fn selftest_garage_returns(game: &mut Game) -> Result<()> {
    let saved = (game.pacing.clock, std::mem::replace(&mut game.mode, Mode::Explore));
    let mut t = f64::from(game.now()) / 1000.0;
    game.pacing.clock = Some(t);
    game.garage_fe.view = None;
    game.garage_request();
    let mut run = |game: &mut Game, seconds: f64| {
        for _ in 0..(seconds * 60.0) as usize {
            t += 1.0 / 60.0;
            game.pacing.clock = Some(t);
            game.garage_request();
        }
    };
    let view = |game: &Game| -> Result<nfsu2::garage::View> {
        let (_, _, v) = game.garage_fe.view.as_ref().context("no garage view")?;
        Ok(v.at(game.now() / 1000.0))
    };
    let base = nfsu2::garage::SHOTS[0];
    let check = |game: &Game, what: &str, camera: &nfsu2::garage::Shot, angle: f32, steer: f32| -> Result<()> {
        let v = view(game)?;
        anyhow::ensure!(
            (v.camera.fov - camera.fov).abs() < 0.01
                && v.camera.eye.abs_diff_eq(camera.eye, 0.001)
                && v.camera.target.abs_diff_eq(camera.target, 0.001),
            "{what}: camera {:?}, wanted {camera:?}",
            v.camera
        );
        anyhow::ensure!(
            (v.car_angle.rem_euclid(360.0) - angle).abs() < 0.01 && (v.steer - steer).abs() < 0.01,
            "{what}: car at {} steer {}, wanted {angle} / {steer}",
            v.car_angle,
            v.steer
        );
        Ok(())
    };
    // A body part's list: its view, then Back to the strip.
    game.mode = game.parts_browser(0, None);
    run(game, 0.5);
    game.menu_key(Key::Accept)?;
    let slot = match &game.mode {
        Mode::Menu(Menu::Parts(b)) if b.part.is_some() => u32::from(b.categories[b.category].slot),
        _ => anyhow::bail!("the body shop's first list did not open"),
    };
    run(game, 4.0);
    let shot = nfsu2::garage::slot_shot(slot, false, false, None).context("the list's slot has a view")?;
    check(game, "in the list", &shot, shot.car_angle, shot.steer)?;
    game.menu_key(Key::Back)?;
    anyhow::ensure!(matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.part.is_none()), "Back closes the list");
    run(game, 4.0);
    check(game, "back on the strip", &base, shot.car_angle, shot.steer)?;
    // The rims: the styles ask Front Wheel; Back from the sizes returns
    // the camera; a style step asks the wheel again.
    let wheel = nfsu2::garage::slot_shot(0x1D, false, false, None).context("Front Wheel")?;
    game.mode = game.rims_browser(0, None, None);
    run(game, 4.0);
    check(game, "rim styles", &wheel, wheel.car_angle, wheel.steer)?;
    let styles = match &game.mode {
        Mode::Menu(Menu::Parts(b)) => b.categories.len(),
        _ => 0,
    };
    game.menu_key(Key::Accept)?;
    anyhow::ensure!(
        matches!(&game.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_Rims_Browser" && b.part.is_some()),
        "the rim sizes"
    );
    run(game, 1.0);
    game.menu_key(Key::Back)?;
    run(game, 4.0);
    check(game, "back from the rim sizes", &base, wheel.car_angle, wheel.steer)?;
    let style = |game: &Game| match &game.mode {
        Mode::Menu(Menu::Parts(b)) => b.category,
        _ => usize::MAX,
    };
    if styles > 1 {
        let was = style(game);
        game.menu_key(Key::Right)?;
        if style(game) == was {
            game.menu_key(Key::Left)?;
        }
        anyhow::ensure!(style(game) != was, "the rim styles step");
        run(game, 4.0);
        check(game, "the next rim style", &wheel, wheel.car_angle, wheel.steer)?;
    }
    println!(
        "garage returns: the base camera after a list and the rim sizes, the car kept at {:.1}° (steer {})",
        wheel.car_angle, wheel.steer
    );
    game.garage_fe.view = None;
    (game.pacing.clock, game.mode) = saved;
    Ok(())
}

/// Left / Right hammered on the car lot up now (steps 0 to 0.6 s apart,
/// many under the lot's 0.3 s wait and the platform's 0.5 s turn), then
/// back to its first car: at every frame with no turn under way the shown
/// car stands on the front disc, and once the keys stop the garage shows
/// the selected car (`garage_request`'s car is the lot's), its turn over.
/// The game's clock is stepped here (1/60 s frames) and given back after.
pub(crate) fn selftest_lot_hammer(game: &mut Game, what: &str) -> Result<()> {
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { anyhow::bail!("{what}: not a car lot") };
    let (start, n) = (lot.selected, lot.cars.len());
    if n < 2 {
        println!("{what}: {n} car, no hammering");
        return Ok(());
    }
    let saved = game.pacing.clock;
    let mut t = f64::from(game.now()) / 1000.0;
    game.pacing.clock = Some(t);
    game.garage_request();
    let check_front = |game: &Game, at: &str| -> Result<()> {
        let Some((_, _, view)) = game.garage_fe.view.as_ref() else { anyhow::bail!("{what}: no garage view {at}") };
        let now = game.now() / 1000.0;
        if !view.changing(now) {
            anyhow::ensure!(
                view.at(now).turntable().car_in_front(),
                "{what}: the car off the front disc at rest {at}: {:?}",
                view.at(now).turntable()
            );
        }
        Ok(())
    };
    let mut rng = 0x9E37_79B9_u32;
    let (mut steps, mut frames) = (0, 0);
    for i in 0..160 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        // Wander, mostly onwards, back at the ends.
        let selected = match &game.mode {
            Mode::Menu(Menu::CarLot(l)) => l.selected,
            _ => anyhow::bail!("{what}: left the lot at key {i}"),
        };
        let right = if selected == 0 {
            true
        } else if selected + 1 == n {
            false
        } else {
            rng % 5 != 0
        };
        game.menu_key(if right { Key::Right } else { Key::Left })?;
        let after = match &game.mode {
            Mode::Menu(Menu::CarLot(l)) => l.selected,
            _ => anyhow::bail!("{what}: left the lot after key {i}"),
        };
        steps += usize::from(after != selected);
        let gap = 1 + (rng >> 8) % 36;
        for _ in 0..gap {
            t += 1.0 / 60.0;
            game.pacing.clock = Some(t);
            game.garage_request().with_context(|| format!("{what}: no garage"))?;
            check_front(game, &format!("after key {i}"))?;
            frames += 1;
        }
    }
    // Back to the first car, then the keys stop.
    loop {
        let selected = match &game.mode {
            Mode::Menu(Menu::CarLot(l)) => l.selected,
            _ => anyhow::bail!("{what}: left the lot"),
        };
        if selected == start {
            break;
        }
        game.menu_key(if selected > start { Key::Left } else { Key::Right })?;
        t += 0.05;
        game.pacing.clock = Some(t);
        game.garage_request();
    }
    for _ in 0..180 {
        t += 1.0 / 60.0;
        game.pacing.clock = Some(t);
        game.garage_request();
        check_front(game, "after the keys")?;
    }
    let request = game.garage_request().with_context(|| format!("{what}: no garage"))?;
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { anyhow::bail!("{what}: the lot") };
    let want = lot.cars.get(lot.selected).filter(|c| c.placeholder.is_none()).map(|c| c.name.clone());
    anyhow::ensure!(request.car == want, "{what}: the garage shows {:?}, the lot has {:?} selected", request.car, want);
    anyhow::ensure!(
        game.garage_fe.crib_shown.as_ref() == lot.cars.get(lot.selected),
        "{what}: the shown car is not the selected one"
    );
    let now = game.now() / 1000.0;
    let view = game.garage_fe.view.as_ref().map(|v| v.2).context("the garage view")?;
    anyhow::ensure!(
        !view.changing(now) && view.at(now).turntable().car_in_front(),
        "{what}: the platform still turning or off the front"
    );
    println!("{what}: {steps} steps over {frames} frames, the garage ends on {:?}", request.car);
    // The wall clock carries on from the stepped one (the front end's
    // times stay in order).
    game.pacing.clock = saved;
    if saved.is_none() {
        game.pacing.started =
            std::time::Instant::now().checked_sub(std::time::Duration::from_secs_f64(t)).unwrap_or(game.pacing.started);
    }
    Ok(())
}
