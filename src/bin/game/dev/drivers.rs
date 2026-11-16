use crate::*;

/// Drive a fresh career's intro to the car lot as a player would: the
/// D-Day script through (its SMS read and closed), then the keys only —
/// throttle, brake, steering toward the GPS route 12 m ahead of the car
/// (`Gps` follower's node, as the minimap draws it), reversing out for a
/// second and a half when the car stalls against something. Done when the
/// car lot's trigger opens its engage dialog (0x514FD0); an error after
/// ten minutes. Returns the seconds and metres driven.
pub(crate) fn drive_intro_route(game: &mut Game, verbose: bool) -> Result<(f32, f32)> {
    const DT: f32 = 1.0 / 30.0;
    for _ in 0..600 {
        if matches!(game.mode, Mode::Menu(Menu::SmsViewer(_) | Menu::SmsMailbox(_))) {
            game.sms_close();
        }
        game.advance_for(DT)?;
        if !game.career_world.dday.running() && game.career_world.gps.as_ref().is_some_and(|g| g.follower.node.is_some()) {
            break;
        }
    }
    anyhow::ensure!(!game.career_world.dday.running(), "the D-Day script over");
    // The intro event's goal shop (kind 4): the car lot, and its trigger.
    let intro = game.profile.as_ref().and_then(|p| p.intro);
    let goals = intro
        .and_then(|h| game.game_data.career.events.iter().find(|e| e.unlock == h))
        .map(|e| e.goals.clone())
        .unwrap_or_default();
    let lot_box = game
        .game_data.career
        .shops
        .iter()
        .find(|s| goals.iter().any(|&(k, h)| k == 4 && h == s.hash))
        .and_then(|s| game.sim.route_triggers.find(s.trigger))
        .map(|t| (t.min, t.max, t.position.z));
    anyhow::ensure!(lot_box.is_some(), "the intro's car lot");
    let mut last = game.here().context("the car")?.0;
    let (mut metres, mut stalled, mut reverse) = (0.0f32, 0.0f32, 0.0f32);
    let mut near_end = false;
    for frame in 0..(600.0 / DT) as u32 {
        // The car lot's marker under the car: the Engage key (0x5EB050).
        if near_end && matches!(game.mode, Mode::Explore) && matches!(game.career_world.marker, Some(MarkerTarget::Shop(_))) {
            game.platform.keys.clear();
            game.engage();
        }
        match &game.mode {
            Mode::Menu(Menu::ShopEngage(_)) => return Ok((frame as f32 * DT, metres)),
            Mode::Menu(Menu::SmsViewer(_) | Menu::SmsMailbox(_)) => game.sms_close(),
            Mode::Explore => {}
            _ => anyhow::bail!("left free roam on the way"),
        }
        let d = game.sim.drive.as_ref().context("the car")?;
        let (pos, fwd, speed) = (d.state.position, d.state.forward(), d.state.velocity.truncate().length());
        let points = game
            .career_world.gps
            .as_ref()
            .and_then(|g| Some(g.follower.node.as_ref()?.points(&game.sim.traffic.as_ref()?.routes)))
            .unwrap_or_default();
        // Near the route's end (the trigger's GPS point, on the road) the
        // car lot's trigger itself, off the road.
        let lot = lot_box.map(|(lo, hi, z)| ((lo + hi) * 0.5).extend(z));
        near_end |= points.last().is_some_and(|e| e.truncate().distance(pos.truncate()) < 25.0);
        let target = if near_end { lot } else { route_ahead(&points, pos, 12.0) };
        let mut steer = 0.0;
        if let Some(t) = target {
            let to = (t - pos).truncate().normalize_or_zero();
            let a = to.y.atan2(to.x) - fwd.y.atan2(fwd.x);
            steer = a.sin().atan2(a.cos());
        }
        stalled = if speed < 1.0 && reverse <= 0.0 { stalled + DT } else { 0.0 };
        if stalled > 1.5 {
            reverse = 1.5;
            stalled = 0.0;
            if verbose {
                eprintln!(
                    "{:6.1} s stalled at {pos:?} heading {:.0} wall {:?} contacts {:?}",
                    frame as f32 * DT,
                    fwd.y.atan2(fwd.x).to_degrees(),
                    d.state.wall_contact,
                    d.state.contacts.iter().map(|c| c.touching).collect::<Vec<_>>()
                );
            }
        }
        // Off the route with the search given up (the GPS reads "Unable
        // to contact satellite"): engage it again, as the World Map does.
        if points.is_empty()
            && frame % 60 == 0
            && let Some(g) = game.career_world.gps.as_ref().filter(|g| g.follower.state == 3 || g.follower.node.is_none())
        {
            let t = career::Trigger {
                gps: g.at,
                position: g.map.extend(g.at.z),
                name: 0,
                min: Vec2::ZERO,
                max: Vec2::ZERO,
                polygon: [Vec2::ZERO; 8],
                points: 0,
            };
            game.career_world.gps = Some(dday::Gps::engage(g.target, &t, Some((pos, fwd.truncate()))));
        }
        game.platform.keys.clear();
        if reverse > 0.0 {
            reverse -= DT;
            game.platform.keys.insert(KeyCode::ArrowDown);
            // Backing up turns the nose the other way.
            if steer.abs() > 0.1 {
                game.platform.keys.insert(if steer > 0.0 { KeyCode::ArrowRight } else { KeyCode::ArrowLeft });
            }
        } else {
            // Slow for a corner, crawl round a hairpin.
            let limit = if steer.abs() > 1.0 || near_end {
                4.0
            } else if steer.abs() > 0.5 {
                8.0
            } else {
                16.0
            };
            if speed < limit {
                game.platform.keys.insert(KeyCode::ArrowUp);
            } else if speed > limit + 3.0 {
                game.platform.keys.insert(KeyCode::ArrowDown);
            }
            if steer.abs() > 0.08 {
                game.platform.keys.insert(if steer > 0.0 { KeyCode::ArrowLeft } else { KeyCode::ArrowRight });
            }
        }
        game.advance_for(DT)?;
        let now = game.here().context("the car")?.0;
        metres += now.truncate().distance(last.truncate());
        last = now;
        if verbose && frame % 150 == 0 {
            eprintln!(
                "{:6.1} s at {:.0} {:.0} {:.1} target {target:?} speed {speed:.1} steer {steer:.2} route {} points, {:.0} m to its end",
                frame as f32 * DT,
                pos.x,
                pos.y,
                pos.z,
                points.len(),
                points.last().map_or(0.0, |e| e.truncate().distance(pos.truncate()))
            );
        }
    }
    game.platform.keys.clear();
    anyhow::bail!("not at the car lot after ten minutes ({metres:.0} m driven, at {last:?})")
}

/// The probe driver's aim: the point `ahead` metres along a polyline past
/// the car's closest point on it (any stretch), or its end.
pub(crate) fn route_ahead(points: &[Vec3], car: Vec3, ahead: f32) -> Option<Vec3> {
    let p = car.truncate();
    let (mut best, mut at) = (f32::MAX, 0.0);
    let mut along = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0].truncate(), w[1].truncate());
        let l = a.distance(b);
        let t = if l > 0.0 { ((p - a).dot(b - a) / (l * l)).clamp(0.0, 1.0) } else { 0.0 };
        let d = p.distance(a + (b - a) * t);
        if d < best {
            (best, at) = (d, along + t * l);
        }
        along += l;
    }
    let mut s = at + ahead;
    for w in points.windows(2) {
        let l = w[0].truncate().distance(w[1].truncate());
        if s <= l {
            return Some(if l > 0.0 { w[0] + (w[1] - w[0]) * (s / l) } else { w[0] });
        }
        s -= l;
    }
    points.last().copied()
}
