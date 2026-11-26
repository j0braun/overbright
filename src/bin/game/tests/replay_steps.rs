//! #240: a replay re-simulates the race step for step. Every recorded
//! frame's state (the player's car, the rivals, the traffic, the world
//! clock the grip's rain reads) in the replay equals the live race's
//! after the same frame, whatever the drawn frame rate, with the
//! keyboard or a controller, with or without rain.

use crate::*;
use nfsu2::pad::host::Btn;
use std::collections::BTreeMap;

/// The state a race frame leaves, as bits: the player's car, the rivals,
/// the traffic and the clocks (race and world).
fn step_hash(game: &Game, race: &Race) -> [u64; 4] {
    use std::hash::{Hash, Hasher};
    let d = game.sim.drive.as_ref().unwrap();
    let mut hs = [(); 4].map(|_| std::collections::hash_map::DefaultHasher::new());
    let mut k = 0;
    let mut put = |v: Vec3, part: usize| {
        k = part;
        v.to_array().map(f32::to_bits).hash(&mut hs[k]);
    };
    put(d.state.position, 0);
    put(d.state.velocity, 0);
    put(d.state.orientation * Vec3::X, 0);
    put(d.state.orientation * Vec3::Z, 0);
    for r in &race.racers.rivals {
        put(r.state.position, 1);
        put(r.state.velocity, 1);
    }
    if let Some(t) = race.racers.traffic.as_ref() {
        for c in &t.cars {
            put(c.state.position, 2);
        }
    }
    put(Vec3::new(race.racers.me.distance, race.timing.clock, nfsu2::weather::clock(&d.world)), 3);
    hs.map(|h| h.finish())
}

#[derive(Clone, Copy, Debug)]
enum Via {
    Keys,
    Pad,
}

const BUTTONS: [Btn; 10] =
    [Btn::South, Btn::East, Btn::West, Btn::North, Btn::L1, Btn::R1, Btn::Up, Btn::Down, Btn::L3, Btn::R3];

/// The input of live tick `i`.
fn input(game: &mut Game, how: Via, i: u32) {
    let steer = match i % 120 {
        0..30 => -1.0,
        60..80 => 1.0,
        _ => 0.0,
    };
    let gas = i % 300 < 250;
    match how {
        Via::Keys => {
            game.platform.keys.clear();
            game.platform.keys.insert(if gas { KeyCode::ArrowUp } else { KeyCode::ArrowDown });
            if steer < 0.0 {
                game.platform.keys.insert(KeyCode::ArrowLeft);
            } else if steer > 0.0 {
                game.platform.keys.insert(KeyCode::ArrowRight);
            }
        }
        Via::Pad => {
            let p = &mut game.platform.pads.devices[0].inputs;
            p.sticks[0] = steer * 0.8;
            p.triggers = if gas { [0.0, 1.0] } else { [0.9, 0.0] };
            // The face and shoulder buttons now and then (nitrous, the
            // e-brake, the shifts, the camera, a look back, Engage).
            for (k, b) in BUTTONS.into_iter().enumerate() {
                p.buttons[b as usize] = k == (i / 37) as usize % BUTTONS.len() && i % 37 < 4;
            }
        }
    }
}

/// Races `ticks` frames after the green (drawn at `fps`), then plays the
/// replay from the post-race menu (drawn at `replay_fps`); the first
/// frame whose replayed state differs, if any.
fn race_and_replay(seed: u32, how: Via, rain: bool, fps: f32, replay_fps: f32, ticks: usize) -> Option<String> {
    crate::tests::testkit::game();
    // The race to its green light, with the world clock set to `clock`
    // at the start (the start scene or countdown runs it on).
    let start = |clock: Option<f32>| {
        let mut game = Game::new().unwrap();
        if matches!(how, Via::Pad) {
            game.settings.custom[5] = 1;
            game.platform.pads = nfsu2::pad::host::Host::scripted("0 connect 1 045e 0b12\n");
            game.advance_for(1.0 / 60.0).unwrap();
            assert_eq!(game.platform.pads.devices.len(), 1);
        }
        game.rng = nfsu2::opponents::GameRng(seed);
        let event = game.game_data.career.events.iter().position(|e| e.kind == career::EventKind::Circuit).unwrap();
        game.start_race(event, true, false).unwrap();
        let d = game.sim.drive.as_mut().unwrap();
        if let Some(c) = clock {
            d.clock = c;
            nfsu2::weather::set_clock(&d.world, c);
        }
        let set = d.clock;
        let green = |game: &Game| matches!(&game.mode, Mode::Race(r) if r.replay.recording.is_some());
        for _ in 0..3000 {
            if green(&game) {
                break;
            }
            game.advance_for(1.0 / 60.0).unwrap();
        }
        assert!(green(&game), "the green light");
        let lead = game.sim.drive.as_ref().unwrap().clock - set;
        (game, lead)
    };
    let (mut game, lead) = start(None);
    if rain {
        // A world clock at which the rain at the car begins early in the
        // race (the grip reads it: 0x5ED5E0).
        let d = game.sim.drive.as_ref().unwrap();
        let at = d.state.position.truncate();
        let span = ticks as f32 / 60.0;
        let c = (0..3600)
            .map(|k| k as f32 * 0.1)
            .find(|&c| {
                nfsu2::weather::target_at(&d.world, at, c) == 0.0
                    && nfsu2::weather::target_at(&d.world, at, c + span * 0.3) > 0.0
            })
            .expect("a rain cycle at the start");
        game = start(Some(c - lead)).0;
    }
    let mut live = BTreeMap::new();
    let mut i = 0u32;
    let mut wet = 0.0f32;
    while live.len() < ticks {
        game.advance_for_each(1.0 / fps, |game, _| {
            let Mode::Race(r) = &game.mode else { return Ok(()) };
            let n = r.replay.recording.as_ref().map_or(0, |rec| rec.frames.len());
            if n > 0 && !live.contains_key(&n) {
                live.insert(n, step_hash(game, r));
                let d = game.sim.drive.as_ref().unwrap();
                wet = wet.max(nfsu2::weather::wetness_at(&d.world, d.state.position.truncate()));
            }
            input(game, how, i);
            i += 1;
            Ok(())
        })
        .unwrap();
    }
    if rain {
        assert!(wet > 0.0, "it rained on the car");
    }
    game.platform.keys.clear();
    let menu = game.icon_menu(MenuKind::PostRace);
    game.keep_replay(menu);
    game.act(Action::Replay, MenuKind::PostRace).unwrap();
    let last = live.keys().copied().max().unwrap();
    let mut seen = 0;
    // The rain in the replay: (frame, drops) when it first rains, and
    // whether the drops moved 30 frames on.
    let mut rain_at: Option<(usize, String)> = None;
    let mut rain_moved = None;
    for _ in 0..(last as f32 * replay_fps / 60.0 * 2.0 + 200.0) as usize {
        let mut bad = None;
        game.advance_for_each(1.0 / replay_fps, |game, _| {
            let Mode::Race(r) = &game.mode else { return Ok(()) };
            let Some(p) = r.replay.playback.as_ref().filter(|p| p.skip.is_none()) else { return Ok(()) };
            if p.at > 0 && p.at != seen {
                seen = p.at;
                let d = game.sim.drive.as_ref().unwrap();
                if d.weather.raining() {
                    let mut drops = nfsu2::weather::RainSnapshot::default();
                    d.weather.capture(&mut drops);
                    let drops = format!("{drops:?}");
                    match &rain_at {
                        None => rain_at = Some((p.at, drops)),
                        Some((f, first)) if rain_moved.is_none() && p.at >= f + 30 => {
                            rain_moved = Some(*first != drops)
                        }
                        _ => {}
                    }
                }
                if let Some(&h) = live.get(&p.at)
                    && h != step_hash(game, r)
                    && bad.is_none()
                {
                    let d = game.sim.drive.as_ref().unwrap();
                    let now = step_hash(game, r);
                    let parts: Vec<_> = ["car", "rivals", "traffic", "clocks"]
                        .iter()
                        .zip(h.iter().zip(now))
                        .filter(|(_, (a, b))| *a != b)
                        .map(|(n, _)| *n)
                        .collect();
                    bad = Some(format!(
                        "frame {} of {last}: {parts:?} differ; world clock {} car {:?}",
                        p.at,
                        nfsu2::weather::clock(&d.world),
                        d.state.position
                    ));
                }
            }
            Ok(())
        })
        .unwrap();
        if bad.is_some() {
            return bad;
        }
        if seen >= last {
            return match (rain, rain_moved) {
                (true, None) => Some("no rain in the replay".to_string()),
                (true, Some(false)) => Some("the replay's raindrops stand still".to_string()),
                _ => None,
            };
        }
    }
    Some(format!("the replay stopped at frame {seen} of {last}"))
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn replay_matches_the_race_step_for_step() {
    let mut fails = Vec::new();
    for (seed, how, rain, fps, rfps) in [
        (1, Via::Keys, false, 60.0, 60.0),
        (2, Via::Keys, true, 60.0, 144.0),
        (3, Via::Pad, false, 144.0, 200.0),
        (4, Via::Pad, true, 200.0, 60.0),
        (5, Via::Keys, true, 144.0, 30.0),
    ] {
        if let Some(e) = race_and_replay(seed, how, rain, fps, rfps, 1500) {
            fails.push(format!("seed {seed} {how:?} rain {rain} {fps}/{rfps} fps: {e}"));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
