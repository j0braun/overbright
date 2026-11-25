use crate::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The Rumble option (High) in a race, driven at full throttle for 20 s
/// of game time with one pad attached: the motors run (the gear changes,
/// the bumps and the road texture), and the same at 30, 60, 120 and 200
/// frames per second (the model steps on the 1/60 s world tick).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rumble_runs_in_a_race_at_any_frame_rate() {
    testkit::game();
    let mut seen = Vec::new();
    for fps in [30.0f32, 60.0, 120.0, 200.0] {
        let mut game = Game::new().unwrap();
        game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
        let event = game.quick_event(screens::QrMode::Circuit);
        game.quick.car = Some("240SX".into());
        game.start_race(event, true, false).unwrap();
        game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 028e\n");
        game.platform.pads.poll(0.0);
        game.settings.enh.pad.rumble = 100;
        game.platform.pads.set_options(game.settings.enh.pad);
        // Past the start scene and the countdown.
        for _ in 0..(40.0 * fps) as usize {
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.5) {
                break;
            }
            game.step_for(1.0 / fps).unwrap();
        }
        assert!(matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0), "{fps}: racing");
        game.platform.keys.insert(KeyCode::ArrowUp);
        let (mut strong, mut weak, mut peak) = (0.0f32, 0.0f32, (0.0f32, 0.0f32));
        for _ in 0..(20.0 * fps) as usize {
            game.step_for(1.0 / fps).unwrap();
            let (s, w) = game.platform.pads.rumble_out.first().copied().unwrap_or_default();
            strong += s / fps;
            weak += w / fps;
            peak = (peak.0.max(s), peak.1.max(w));
        }
        eprintln!("{fps} fps: strong {strong:.3} s·level, weak {weak:.3} s·level, peak {peak:?}");
        assert!(weak > 0.0 || strong > 0.0, "{fps}: the motors ran");
        seen.push((strong, weak));
    }
    for &(s, w) in &seen[1..] {
        assert!((s - seen[0].0).abs() <= 0.05 + 0.1 * seen[0].0 && (w - seen[0].1).abs() <= 0.05 + 0.1 * seen[0].1, "{seen:?}");
    }
}

/// What a staged drive gave: the strong motor's peak, the hardest new hit
/// the collision step recorded (push |J|) and the car's largest speed
/// change within the jolt window (0.15 s: what the speed jolt reads).
#[derive(Debug, Clone, Copy)]
struct Staged {
    peak: f32,
    hit: f32,
    speed_change: f32,
    /// Where it started and ended.
    moved: (Vec3, Vec3),
}

/// The car placed at `at` heading `heading` at `speed` m/s (and `lateral`
/// m/s to its right), the rumble model fresh, then `seconds` of coasting
/// at 60 frames a second with the scripted pad's output read every frame.
fn staged(game: &mut Game, at: Vec3, heading: f32, speed: f32, lateral: f32, seconds: f32) -> Staged {
    let d = game.sim.drive.as_mut().unwrap();
    d.place(at, heading);
    let right = d.state.forward().cross(Vec3::Z);
    d.state.velocity = d.state.forward() * speed + right * lateral;
    d.state.publish_forward_speed();
    d.state.take_hit();
    game.rumble.reset();
    game.rumble_travel = None;
    let start = d.state.position;
    let mut out = Staged { peak: 0.0, hit: 0.0, speed_change: 0.0, moved: (start, start) };
    let mut speeds = std::collections::VecDeque::new();
    for _ in 0..(seconds * 60.0) as usize {
        game.step_for(1.0 / 60.0).unwrap();
        let (s, _) = game.platform.pads.rumble_out.first().copied().unwrap_or_default();
        let d = game.sim.drive.as_ref().unwrap();
        out.peak = out.peak.max(s);
        out.hit = out.hit.max(d.state.new_hit);
        speeds.push_back(d.state.velocity.length());
        if speeds.len() > 9 {
            speeds.pop_front();
        }
        let (lo, hi) = speeds.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        out.speed_change = out.speed_change.max(hi - lo);
        out.moved.1 = d.state.position;
    }
    out
}

/// Rumble (High) on crashes, through a scripted pad in a race: a stop
/// sign knocked over, a light touch of an opponent's car, and a wall at
/// low and at high speed each give a strong-motor pulse from the game's
/// own contact events (not only from the speed loss, which the light hits
/// barely have), harder for the harder hits; a smooth road gives no hit.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rumble_pulses_on_every_crash_by_its_strength() {
    testkit::game();
    let mut game = Game::new().unwrap();
    game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
    let event = game.quick_event(screens::QrMode::Circuit);
    game.quick.car = Some("240SX".into());
    game.start_race(event, true, false).unwrap();
    game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 028e\n");
    game.platform.pads.poll(0.0);
    game.settings.enh.pad.rumble = 100;
    game.platform.pads.set_options(game.settings.enh.pad);
    for _ in 0..(40.0 * 60.0) as usize {
        if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.5) {
            break;
        }
        game.step_for(1.0 / 60.0).unwrap();
    }
    assert!(matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0), "racing");

    // A smooth street (the Chinatown street the drive tests use): no hit.
    let road = staged(&mut game, Vec3::new(361.0, -462.0, 30.0), (-113.0f32).to_radians(), 12.0, 0.0, 1.0);
    eprintln!("smooth road: {road:?}");
    assert_eq!(road.hit, 0.0, "no contact on a smooth road");

    // A breakable prop (a sign) on open road, met at 5 m/s along x.
    let world = &game.sim.drive.as_ref().unwrap().world;
    let (name, sign) = world
        .smackables
        .iter()
        .find(|p| {
            let at = p.rest.w_axis.truncate();
            p.info.break_impulse > 0.0
                && (0..=10).all(|i| {
                    let q = at - Vec3::X * i as f32 + Vec3::Z * 2.0;
                    world.road_height_below(q).is_some_and(|z| (z - at.z).abs() < 0.3)
                })
        })
        .map(|p| (p.info.name.clone(), p.rest.w_axis.truncate()))
        .expect("a breakable prop on open road");
    let prop = staged(&mut game, sign - Vec3::X * 4.0, 0.0, 5.0, 0.0, 1.5);
    eprintln!("{name}: {prop:?}");

    // A light touch: the first opponent, pulling away from the grid,
    // nudged from behind at 4 m/s from a few decimetres back.
    let (at, heading) = match &game.mode {
        Mode::Race(r) => {
            let s = &r.racers.rivals[0].state;
            (s.position, s.heading())
        }
        _ => unreachable!(),
    };
    let ahead = Vec3::new(heading.cos(), heading.sin(), 0.0);
    let touch = staged(&mut game, at - ahead * 4.8, heading, 4.0, 0.0, 1.0);
    eprintln!("light touch: {touch:?}");

    // A wall at 35° to the street ahead (the glancing wall of the drive
    // tests), met from 3.5 m before it at 6 and at 22 m/s.
    let world = &game.sim.drive.as_ref().unwrap().world;
    let ground = world.road_height_below(Vec3::new(365.0, -470.0, 30.0)).expect("the street");
    let from = Vec3::new(365.0, -465.0, ground + 0.5);
    let (f, _) = world.wall_hit(from, from - Vec3::Y * 40.0).expect("the wall ahead");
    let wall_at = from - Vec3::Y * (40.0 * f - 3.5);
    let slow = staged(&mut game, wall_at, -std::f32::consts::FRAC_PI_2, 6.0, 0.0, 1.5);
    eprintln!("wall at 6 m/s: {slow:?}");
    let fast = staged(&mut game, wall_at, -std::f32::consts::FRAC_PI_2, 22.0, 0.0, 1.0);
    eprintln!("wall at 22 m/s: {fast:?}");

    for (what, s) in [("prop", prop), ("touch", touch), ("slow wall", slow), ("fast wall", fast)] {
        assert!(s.hit > 0.0, "{what}: a contact event");
        assert!(s.peak >= nfsu2::pad::extras::HIT_FLOOR, "{what}: a clear pulse {s:?}");
        assert!(s.peak >= nfsu2::pad::extras::hit_strength(s.hit) - 1e-6, "{what}: the pulse follows the hit {s:?}");
    }
    assert!(slow.hit < fast.hit && slow.peak < fast.peak, "harder wall, harder pulse: {slow:?} {fast:?}");
    // The light hits change the speed by less than the jolt's 4 m/s: the
    // speed model alone gave them nothing.
    for (what, s) in [("prop", prop), ("touch", touch), ("slow wall", slow)] {
        assert!(s.speed_change < 4.0, "{what}: {s:?}");
    }
    // Harder hit, harder pulse, across the kinds.
    let mut all = [prop, touch, slow, fast];
    all.sort_by(|a, b| a.hit.total_cmp(&b.hit));
    assert!(all.windows(2).all(|w| w[0].peak <= w[1].peak), "{all:?}");
}

/// A force-feedback wheel (a scripted G29 with the effect recorder) in a
/// race, full throttle and a little steering for 12 s: the original's
/// effects play (the damper, the steering force) and the recorded calls are the same at 30, 60, 120 and 200 frames
/// per second (0x406460 steps on the 1/60 s world tick).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn wheel_force_feedback_at_any_frame_rate() {
    use nfsu2::pad::ff::{Command, Effect};
    use nfsu2::pad::ff_out::Call;
    testkit::game();
    let mut seen: Vec<Vec<(i64, Call)>> = Vec::new();
    for fps in [30.0f32, 60.0, 120.0, 200.0] {
        let mut game = Game::new().unwrap();
        game.quick.tracks = game.quick_track_select(screens::QrMode::Circuit, 0.0);
        let event = game.quick_event(screens::QrMode::Circuit);
        game.quick.car = Some("240SX".into());
        game.start_race(event, true, false).unwrap();
        game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 046d c24f ff\n0 axis 1 lx 0.1\n");
        game.platform.pads.poll(0.0);
        assert!(game.platform.pads.force_feedback() && game.platform.pads.wheel_drives());
        // Past the start scene and the countdown, tick by tick, so the
        // throttle goes down on the same world tick at every rate.
        for _ in 0..40 * 60 {
            if matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.5) {
                break;
            }
            game.step_for(1.0 / 60.0).unwrap();
        }
        assert!(matches!(&game.mode, Mode::Race(r) if r.timing.clock > 0.0), "{fps}: racing");
        assert!(game.controls().wheel.is_some(), "the wheel steers through 0x4061C0");
        game.platform.keys.insert(KeyCode::ArrowUp);
        for _ in 0..(12.0 * fps) as usize {
            game.step_for(1.0 / fps).unwrap();
        }
        let calls: Vec<Call> = game.platform.pads.ff().unwrap().out.calls().iter().map(|c| c.1).collect();
        let has = |f: &dyn Fn(&Call) -> bool| calls.iter().any(f);
        assert!(has(&|c| matches!(c, Call::Command(Command::Set(Effect::Damper { .. })))), "{fps}: the damper");
        assert!(has(&|c| matches!(c, Call::Command(Command::Set(Effect::Constant { level })) if *level != 0)), "{fps}: the steering force");
        assert!(has(&|c| *c == Call::Gain(10_000)), "{fps}: the gain");
        eprintln!("{fps} fps: {} calls", calls.len());
        seen.push(game.platform.pads.ff().unwrap().out.calls().to_vec());
    }
    // Compare up to the earliest last tick (12 s of frames end on the
    // same tick, but a run's last tick may make no call).
    let end = seen.iter().map(|s| s.last().unwrap().0).min().unwrap();
    let upto = |s: &Vec<(i64, Call)>| s.iter().copied().filter(|c| c.0 < end).collect::<Vec<_>>();
    for (i, s) in seen.iter().enumerate().skip(1) {
        let (a, b) = (upto(s), upto(&seen[0]));
        if let Some(k) = (0..a.len().min(b.len())).find(|&k| a[k] != b[k]) {
            panic!("rate {i} differs from 30 fps at call {k}: {:?} vs {:?}", &a[k.saturating_sub(2)..(k + 3).min(a.len())], &b[k.saturating_sub(2)..(k + 3).min(b.len())]);
        }
        assert_eq!(a.len(), b.len(), "rate {i}");
    }
}
