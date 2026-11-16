use crate::*;

/// A drift event with three rivals: every rival drives with the
/// DriftDriver and scores its own drifts; the player, moved along the road
/// 90 s in (no drifting, so few points) and over the line first, is placed
/// by points, not by the finish (0x56DDA0 on the list 0x864630).
pub(crate) fn selftest_drift(game: &mut Game) -> Result<()> {
    let e = game
        .game_data.career
        .events
        .iter()
        .position(|e| e.kind == career::EventKind::Drift && e.opponents.len() == 3 && e.races.len() == 1)
        .context("a drift event with rivals")?;
    game.start_race(e, false, false)?;
    {
        let Mode::Race(race) = &game.mode else { anyhow::bail!("drift race") };
        anyhow::ensure!(
            race.racers.rivals.len() == 3
                && race.racers.rivals.iter().all(|r| r.drift.is_some() && r.style.is_some() && r.scorer.is_some()),
            "rivals drive with the DriftDriver and score"
        );
        anyhow::ensure!(race.scoring.board.len() == 4, "a points list node per racer: {}", race.scoring.board.len());
        // 0x5264B0(2): the rivals on slots 1–3, the player behind them.
        anyhow::ensure!(race.grid_slot == 3, "the player starts behind the rivals: slot {}", race.grid_slot);
        let at = game.sim.drive.as_ref().map(|d| d.state.position.truncate()).context("the player's car")?;
        let slot = race.route.grid.get(3).map(|g| g.0.truncate()).context("four starting positions")?;
        anyhow::ensure!(at.distance(slot) < 0.5, "the player's car on starting position 4: {at:?} vs {slot:?}");
        anyhow::ensure!(
            race.racers.rivals
                .iter()
                .zip(&race.route.grid)
                .all(|(r, g)| r.state.position.truncate().distance(g.0.truncate()) < 0.5),
            "the rivals on positions 1–3"
        );
    }
    let mut along = None;
    let mut seen = 0.0f32;
    for _ in 0..60000 {
        let Mode::Race(race) = &game.mode else { break };
        if let Some(b) = race.scoring.board.iter().filter(|e| e.racer != Some(0)).map(|e| e.score).reduce(f32::max) {
            seen = seen.max(b);
        }
        // The race ends 3 s after the player's finish (0x60B3B0 case 3,
        // the rivals still running ended where they are, 0x608CB0), so the
        // player waits 90 s at the start while the rivals drift.
        if race.racers.me.finished.is_none() && race.timing.clock >= 90.0 {
            let s = along.unwrap_or(race.racers.me.along) + 50.0;
            along = Some(s);
            let target = race.route.road.point_at(s);
            let forward = race.route.road.point_at(s + 1.0) - target;
            let d = game.sim.drive.as_mut().unwrap();
            // This is a ranking test, not a driving trajectory. An XY-only
            // jump retains height, momentum and wheel/contact histories;
            // advancing over changing road heights then creates real drift.
            let ground = d.world.grid_height(target.extend(d.state.position.z));
            d.place(target.extend(ground), forward.y.atan2(forward.x));
            anyhow::ensure!(
                d.state.velocity == Vec3::ZERO && d.state.spin == Vec3::ZERO && d.state.angular_momentum == Vec3::ZERO,
                "the scripted drift-test relocation starts without motion"
            );
            game.advance_for(0.25)?;
        } else {
            game.advance_for(1.0 / 30.0)?;
        }
    }
    if let Mode::Race(race) = &game.mode {
        let rivals: Vec<_> = race
            .racers.rivals
            .iter()
            .map(|r| (r.name.clone(), r.progress.progress(), r.progress.finished, r.state.speed() as i32))
            .collect();
        anyhow::bail!("drift race not over at {:.0} s: {rivals:?}", race.timing.clock);
    }
    let Mode::Menu(Menu::Results(results, _)) = &game.mode else { anyhow::bail!("drift results") };
    println!("drift results: {:?}", results.rows.iter().map(|r| (r.0.clone(), r.2 as i32, r.3)).collect::<Vec<_>>());
    anyhow::ensure!(results.scores && results.rows.len() == 4, "a scored row per racer");
    anyhow::ensure!(results.rows.windows(2).all(|w| w[0].2 >= w[1].2), "rows by points, highest first");
    anyhow::ensure!(seen > 0.0, "the rivals scored drift points");
    let name = game.profile.as_ref().map_or_else(|| "Player".to_string(), |p| p.name.clone());
    let me = results.rows.iter().position(|r| r.0 == name).context("the player's row")?;
    anyhow::ensure!(
        results.rows[me].2 == 0.0,
        "the stationary relocated player must not score drift points: {}",
        results.rows[me].2
    );
    anyhow::ensure!(
        me > 0 && results.rows[me].2 < results.rows[0].2,
        "first over the line with fewer points is not first"
    );
    game.mode = game.icon_menu(MenuKind::Main);
    Ok(())
}
