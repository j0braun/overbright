//! `--flow firstrace` (docs/notes/parity/first-race-regression.md): a fresh career's
//! first race after the intro, its start scene played through (not
//! skipped) and then raced, driven frame by frame as the window drives it
//! (`Game::advance` at the display's interval; a scene that holds the cars
//! updates the world at 1/30 s, drawn in between), at several display rates; then the
//! pause's Restart and the same again. Each rival's lateral path from the
//! green is measured: a rival whose heading turns more than [`MAX_YAW`]
//! degrees off the race road's direction in the first
//! [`WINDOW`] s while it drives the racing line (JustDrive; an avoidance
//! or a hit ends its check, as the original takes those at the start too), or comes within [`MIN_GAP`] m of the player, fails the
//! flow; so does a drawn frame whose motion of the player's car is off
//! its speed × the frame's length by more than [`MAX_PACING`].
use crate::*;

/// Seconds after the green the rivals' lateral path is checked.
const WINDOW: f32 = 6.0;
/// The most a rival's heading may turn off the race road's direction
/// (degrees) in the window. JustDrive rejoins the racing line at 0.05 m
/// per m (0x7E8E64, about 3°); the zig-zag path (every inserted point on
/// the line itself) turned the off-line racers 19.5–21.6° across the
/// grid; with the traced path the worst is 14.2° (the road's polyline
/// direction and the Avoid action add to the rejoin).
const MAX_YAW: f32 = 16.0;
/// The most a drawn frame's motion of the player's car may differ from
/// its speed × the frame's length past 1 s (the 29/30/31-tick updates
/// alone give ±3 %; the 60 Hz snap at high rates gave 24 % at 144 Hz and
/// 180 % uncapped: a frame standing, then a jump).
const MAX_PACING: f32 = 0.15;
/// The closest a rival's centre may come to the player's in that window (m).
const MIN_GAP: f32 = 2.2;

/// One run's verdict.
#[derive(Debug, Default)]
pub(super) struct FirstRaceRun {
    pub label: String,
    /// Each rival's largest |lateral − its lateral at the green| (m),
    /// off the race road's centre line (informational: the racing line
    /// itself crosses the road).
    pub lateral: Vec<f32>,
    /// Each rival's fastest change of its offset off the racing line
    /// (`RacerRoute::lateral`, per m driven over 5 m; informational: the
    /// line's own offset moves through curves).
    pub line_rate: Vec<f32>,
    /// Each rival's largest heading off the race road's direction
    /// (degrees) in the window.
    pub yaw: Vec<f32>,
    /// The smallest centre distance between any two cars and which.
    pub min_gap: (f32, usize, usize),
    /// The smallest distance between the player's car and a rival (m).
    pub player_gap: f32,
    /// Each rival's distance from its grid slot when the green came (m).
    pub off_grid: Vec<f32>,
    /// The worst drawn-frame step ratio's deviation from 1 (drawn
    /// distance / speed·interval) from 1 s after the green, at speed.
    pub pacing: (f32,),
}

impl FirstRaceRun {
    pub fn failures(&self) -> Vec<String> {
        let mut f = Vec::new();
        for (i, y) in self.yaw.iter().enumerate() {
            if *y > MAX_YAW {
                f.push(format!(
                    "{}: rival {} turned {y:.1}° off the road's direction in the first {WINDOW} s",
                    self.label,
                    i + 1
                ));
            }
        }
        if self.pacing.0 > MAX_PACING {
            f.push(format!(
                "{}: a drawn frame moved the player's car {:.0} % off its speed × the frame's length",
                self.label,
                self.pacing.0 * 100.0
            ));
        }
        // A rival this close to the player fails; two rivals touching is
        // reported only (still seen after the fix, open in the doc).
        if self.player_gap < MIN_GAP {
            f.push(format!("{}: a rival came within {:.2} m of the player", self.label, self.player_gap));
        }
        f
    }
}

/// The player's position as the frame draws it (`Game::draw_between`).
fn drawn_position(game: &Game, alpha: f32) -> Vec3 {
    let Some(d) = game.sim.drive.as_ref() else { return Vec3::ZERO };
    let now = nfsu2::interpolate::CarPose::of(&d.state);
    match game.interp.prev.poses.first() {
        Some(prev) if alpha < 1.0 => nfsu2::interpolate::CarPose::blend(prev, &now, alpha, game.pacing.last_update_dt)
            .map_or(now.position, |b| b.position),
        _ => now.position,
    }
}

/// A car's offset to the left of the race road's centre line (m).
fn road_lateral(race: &Race, p: Vec3) -> f32 {
    let road = &race.route.road;
    let along = road.locate_once(p.truncate());
    let dir = road.direction(along);
    (p.truncate() - road.point_at(along)).dot(Vec2::new(-dir.y, dir.x))
}

/// The grid's heading (the player's slot, as the cars are placed).
fn grid_heading(race: &Race) -> f32 {
    race.route.grid.get(race.grid_slot).map_or(0.0, |g| g.1)
}

/// The grid's frame: forward and left unit vectors.
fn axes(race: &Race) -> (Vec2, Vec2) {
    let h = grid_heading(race);
    let f = Vec2::new(h.cos(), h.sin());
    (f, Vec2::new(-f.y, f.x))
}

/// Drives the current race (started already) through its scene and
/// `secs` past the green at `hz`, measuring. `skip` presses Enter in the
/// scene as a player skipping it.
pub(super) fn drive_race(
    game: &mut Game,
    label: &str,
    hz: f32,
    secs: f32,
    skip: bool,
    log: bool,
) -> Result<FirstRaceRun> {
    let mut run = FirstRaceRun {
        label: label.to_string(),
        min_gap: (f32::MAX, 0, 0),
        player_gap: f32::MAX,
        ..Default::default()
    };
    // One tick per pass, the drawn frames on the schedule; the
    // checks and inputs below happen between ticks.
    let mut frames = super::ticks::Frames::new(hz);
    let mut green: Option<Vec<f32>> = None;
    let mut anchors: Vec<(f32, Vec3)> = Vec::new();
    let mut driven: Vec<f32> = Vec::new();
    let mut avoided: Vec<bool> = Vec::new();
    let mut last_drawn: Option<Vec3> = None;
    let mut throttle = false;
    let mut skipped = false;
    let tick = std::cell::Cell::new(0u32);
    let mark = super::rate::ticks();
    if log && let Mode::Race(race) = &game.mode {
        let rivals: Vec<String> = race
            .racers.rivals
            .iter()
            .map(|r| format!("s{} {} {:?} rng {:08x}", r.slot, r.car.car_type.name, r.speed, r.rng.0))
            .collect();
        println!("{label} lineup {rivals:?} field {:08x}", race.racers.cars.rng.0);
    }
    loop {
        let Mode::Race(race) = &game.mode else { anyhow::bail!("{label}: left the race ({})", flow_mode(&game.mode)) };
        let clock = race.timing.clock;
        if clock > secs {
            break;
        }
        if skip && !skipped && game.scenes.shop.is_some() && super::rate::keyed(mark) > 30 {
            game.tap_next(KeyCode::Enter);
            skipped = true;
        }
        if clock >= 0.0 && !throttle {
            game.hold_next(KeyCode::ArrowUp, true);
            throttle = true;
        }
        // Drawn pacing: the player's drawn position in each frame as it
        // is drawn.
        let mut pacing = run.pacing.0;
        frames.tick_with(game, &mut |game, super::ticks::Drawn { alpha, interval: dt }| {
            let (Mode::Race(race), Some(d)) = (&game.mode, game.sim.drive.as_ref()) else { return Ok(()) };
            let drawn = drawn_position(game, alpha);
            if race.timing.clock > 1.0 {
                let speed = d.state.speed();
                if let Some(prev) = last_drawn
                    && speed > 5.0
                {
                    let ratio = prev.distance(drawn) / (speed * dt);
                    if std::env::var_os("NFSU2_FIRSTRACE_PACING").is_some() {
                        println!(
                            "pacing t {:.4} dt {:.5} alpha {alpha:.3} acc {:.5} ratio {ratio:.3}",
                            race.timing.clock, dt, game.pacing.cadence.acc
                        );
                    }
                    pacing = pacing.max((ratio - 1.0).abs());
                }
            }
            last_drawn = Some(drawn);
            Ok(())
        })?;
        run.pacing.0 = pacing;
        anyhow::ensure!(frames.frames() < 200_000, "{label}: the race never reached {secs} s");
        let Mode::Race(race) = &game.mode else { continue };
        let Some(d) = game.sim.drive.as_ref() else { continue };
        let (f, l) = axes(race);
        let at = |p: Vec3| Vec2::new(p.truncate().dot(f), p.truncate().dot(l));
        if race.timing.clock >= 0.0 && green.is_none() {
            green = Some(race.racers.rivals.iter().map(|r| road_lateral(race, r.state.position)).collect());
            run.off_grid = race
                .racers.rivals
                .iter()
                .map(|r| {
                    race.route.grid.get(r.slot).map_or(-1.0, |g| g.0.truncate().distance(r.state.position.truncate()))
                })
                .collect();
            run.lateral = vec![0.0; race.racers.rivals.len()];
            run.line_rate = vec![0.0; race.racers.rivals.len()];
            run.yaw = vec![0.0; race.racers.rivals.len()];
            avoided = vec![false; race.racers.rivals.len()];
            anchors =
                race.racers.rivals.iter().map(|r| (r.route.as_ref().map_or(0.0, |x| x.lateral()), r.state.position)).collect();
            driven = vec![0.0; race.racers.rivals.len()];
        }
        if race.timing.clock >= 0.0
            && race.timing.clock <= WINDOW
            && let Some(g) = green.as_ref()
        {
            for (i, r) in race.racers.rivals.iter().enumerate() {
                run.lateral[i] = run.lateral[i].max((road_lateral(race, r.state.position) - g[i]).abs());
                let road = &race.route.road;
                let dir = road.direction(road.locate_once(r.state.position.truncate()));
                let fwd = r.state.forward().truncate().normalize_or(Vec2::X);
                // The heading check is the racing line's (JustDrive): once
                // a rival has taken another action (an AvoidAvoidable at
                // the player or a racer, a hit), its path is that action's
                // and no longer checked. The original proposes such
                // actions at the human from the start too (the sequence
                // log of STAGE_1_CIRCUIT_1: 0x40D5C0's draws, made right
                // before those actions' constructors, 8 and 71 frames past
                // the green with the traffic hundreds of metres off;
                // docs/original/rng.md).
                avoided[i] |= r.actions.kind != nfsu2::ai::actions::Kind::JustDrive;
                if !avoided[i] {
                    run.yaw[i] = run.yaw[i].max(dir.perp_dot(fwd).atan2(dir.dot(fwd)).abs().to_degrees());
                }
                let lat = r.route.as_ref().map_or(0.0, |x| x.lateral());
                driven[i] = r.state.position.truncate().distance(anchors[i].1.truncate());
                if driven[i] >= 5.0 {
                    run.line_rate[i] = run.line_rate[i].max((lat - anchors[i].0).abs() / driven[i]);
                    anchors[i] = (lat, r.state.position);
                }
            }
            let cars: Vec<Vec2> = std::iter::once(d.state.position.truncate())
                .chain(race.racers.rivals.iter().map(|r| r.state.position.truncate()))
                .collect();
            for a in 0..cars.len() {
                for b in a + 1..cars.len() {
                    let gap = cars[a].distance(cars[b]);
                    if a == 0 {
                        run.player_gap = run.player_gap.min(gap);
                    }
                    if gap < run.min_gap.0 {
                        run.min_gap = (gap, a, b);
                    }
                }
            }
        }
        if log && tick.get() != d.world_clock.tick {
            tick.set(d.world_clock.tick);
            let p = at(d.state.position);
            let mut line = format!(
                "{label} t {:7.3} scene {} me lat {:6.2} ({:7.2},{:6.2}) v {:5.2}",
                race.timing.clock,
                game.scenes.shop.as_ref().map_or(-1.0, |s| s.frame),
                road_lateral(race, d.state.position),
                p.x,
                p.y,
                d.state.speed()
            );
            for r in &race.racers.rivals {
                let p = at(r.state.position);
                let h = (r.state.heading() - grid_heading(race) + PI).rem_euclid(TAU) - PI;
                line += &format!(
                    " | s{} line {:6.2} lat {:6.2} ({:7.2},{:6.2}) h {:6.2} st {:6.2} g {:.2} b {:.2} v {:5.2} nis {} a {:?}",
                    r.slot,
                    r.route.as_ref().map_or(0.0, |x| x.lateral()),
                    road_lateral(race, r.state.position),
                    p.x,
                    p.y,
                    h.to_degrees(),
                    r.controls.steer_degrees,
                    r.controls.gas,
                    r.controls.brake,
                    r.state.speed(),
                    r.state.nis.as_ref().map_or(-1, |m| m.count as i32),
                    r.actions.kind
                );
            }
            println!("{line}");
        }
    }
    game.platform.keys.remove(&KeyCode::ArrowUp);
    Ok(run)
}

use std::f32::consts::{PI, TAU};

/// A fresh profile past the intro in the Peugeot, in free roam; the first
/// stage-0 race started from it.
fn fresh_first_race(game: &mut Game) -> Result<()> {
    let mut p = Profile::new("FIRST");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.explore(None)?;
    let e = game
        .game_data.career
        .events
        .iter()
        .find(|e| e.unlock == dday::STAGE0_RACES[0])
        .map(|e| e.index)
        .context("stage 0's first race")?;
    game.start_race(e, false, false)
}

/// The pause's Restart (as the player picks it).
fn restart(game: &mut Game) -> Result<()> {
    game.key(KeyCode::Escape, &mut || {})?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))),
        "the race's pause ({})",
        flow_mode(&game.mode)
    );
    game.act(Action::Restart, MenuKind::RacePause)?;
    for _ in 0..4 {
        if !matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            break;
        }
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Race(_)), "Restart: in {}", flow_mode(&game.mode));
    Ok(())
}

/// `--flow firstrace`. `NFSU2_FIRSTRACE_HZ` (comma list, 0 = uncapped;
/// default 60,144,0), `NFSU2_FIRSTRACE_LOG=1` per-tick rows,
/// `NFSU2_FIRSTRACE_SKIP=1` skips the scene with Enter.
pub(crate) fn flow_first_race(game: &mut Game) -> Result<()> {
    let rates: Vec<f32> = std::env::var("NFSU2_FIRSTRACE_HZ")
        .ok()
        .map_or_else(|| vec![60.0, 144.0, 0.0], |v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let log = std::env::var_os("NFSU2_FIRSTRACE_LOG").is_some();
    let skip = std::env::var_os("NFSU2_FIRSTRACE_SKIP").is_some();
    let mut failures = Vec::new();
    for hz in rates {
        let tag = if hz > 0.0 { format!("{hz} Hz") } else { "uncapped".to_string() };
        fresh_first_race(game)?;
        let first = drive_race(game, &format!("first race {tag}"), hz, WINDOW + 1.0, skip, log)?;
        println!("{first:?}");
        failures.extend(first.failures());
        restart(game)?;
        let again = drive_race(game, &format!("restart {tag}"), hz, WINDOW + 1.0, skip, log)?;
        println!("{again:?}");
        failures.extend(again.failures());
        game.platform.keys.clear();
    }
    for f in &failures {
        println!("FAIL {f}");
    }
    // A rate run compares state across schedules; the flow's own
    // checks (made for steady rates) are reported there, not failed.
    if super::rate::report_only(failures.len()) {
        return Ok(());
    }
    anyhow::ensure!(failures.is_empty(), "{} first-race failures", failures.len());
    println!("first race: rivals kept their lanes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rivals on the line and at the green, as the original's
    /// (docs/notes/parity/race-ai-parity.md, parity runs circuit_cd / circuit_s1 under
    /// Wine): through the countdown no goal, no path, the wheels straight
    /// (steer 0), gas 1 and brake 1 in neutral; the update that turns the
    /// race green clears the brake and the gear request and keeps the gas;
    /// the next one starts the speed control from gas − brake = 1, so the
    /// first green frames are full throttle (the clone had brake 1 at the
    /// green, then gas 0.5, and steered 3° on the line). Through the start
    /// scene's camera (race status 2) the engines idle.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn rivals_hold_the_line_and_launch_at_full_throttle() {
        crate::tests::testkit::game();
        // `Game::new` gives the test a data folder of its own
        // (`test_isolation`): no process-wide XDG_DATA_HOME change here.
        let mut game = Game::new().unwrap();
        fresh_first_race(&mut game).unwrap();
        // A career race runs the catch-up (the boot's option 0x89E7F8 = 1,
        // which the career set-up leaves; observed 0.78–0.80 for leading
        // rivals under Wine).
        assert!(
            matches!(&game.mode, Mode::Race(r) if r.catch_up.is_some() && r.catch_up_difficulty.is_none()),
            "career catch-up on, scale 1"
        );
        let (mut countdown, mut scene_rpm, mut green) = (0, Vec::new(), Vec::new());
        let mut last_clock = f32::NEG_INFINITY;
        for _ in 0..60 * 120 {
            game.advance_for(1.0 / 60.0).unwrap();
            let Mode::Race(r) = &game.mode else { panic!("left the race") };
            if r.timing.clock < 0.0 {
                if game.scenes.shop.as_ref().is_some_and(|run| run.frame < run.scene.marks[1] as f32 - 2.0) {
                    scene_rpm.extend(r.racers.rivals.iter().map(|x| x.state.rpm()));
                } else if r.timing.clock > -1.0 {
                    for x in &r.racers.rivals {
                        let c = &x.controls;
                        assert_eq!(
                            (c.steer_degrees, c.gas, c.brake, c.gear),
                            (0.0, 1.0, 1.0, 1),
                            "{}: countdown pedals",
                            x.name
                        );
                        assert_eq!(x.actions.last_velocity, None, "{}: no action before the green", x.name);
                    }
                    countdown += 1;
                }
            } else {
                green.push(r.racers.rivals.iter().map(|x| x.controls).collect::<Vec<_>>());
                // The mover ran before the green's release (
                // docs/notes/parity/tuning-physics-parity.md): after the green's update
                // the rivals are still in neutral with the clutch locked;
                // the shift to 1st starts one update later (observed under
                // Wine: +0x44 = 3 at the first status-4 frame, 0 after).
                for x in &r.racers.rivals {
                    let t = x.state.drivetrain.as_ref().unwrap();
                    if green.len() == 1 {
                        assert_eq!(
                            (t.gear, t.shift, t.clutch),
                            (nfsu2::handling::NEUTRAL, nfsu2::handling::Shift::Locked, 1.0),
                            "{}: the green's update still in neutral",
                            x.name
                        );
                    } else {
                        assert_eq!(
                            (t.target_gear, t.shift),
                            (nfsu2::handling::NEUTRAL + 1, nfsu2::handling::Shift::Disengaging),
                            "{}: the shift to 1st under way",
                            x.name
                        );
                    }
                }
                if green.len() == 3 {
                    break;
                }
            }
            last_clock = r.timing.clock;
        }
        assert!(last_clock >= 0.0 && countdown > 30, "the countdown ran ({countdown} frames) to the green");
        assert!(
            !scene_rpm.is_empty() && scene_rpm.iter().all(|&rpm| rpm < 1500.0),
            "the engines idle through the scene: {scene_rpm:?}"
        );
        for c in &green[0] {
            assert_eq!(
                (c.steer_degrees, c.gas, c.brake, c.gear, c.mover_held),
                (0.0, 1.0, 0.0, -1, true),
                "the green's update"
            );
        }
        for c in green[1].iter().chain(&green[2]) {
            assert_eq!((c.gas, c.brake), (1.0, 0.0), "full throttle after the green");
            assert!(c.steer_degrees != 0.0, "steering on the path");
        }
        // The controller's full think (0x42ABD0, owner class 2): a rival
        // chooses its action only on world updates ≡ its place among the
        // AI racers mod 8 (observed in the original: rival k's changes all
        // on one residue), the output kept in between.
        let mut kinds: Vec<_> = match &game.mode {
            Mode::Race(r) => r.racers.rivals.iter().map(|x| x.actions.kind).collect(),
            _ => unreachable!(),
        };
        let mut changes = 0;
        for _ in 0..60 * 25 {
            game.advance_for(1.0 / 60.0).unwrap();
            let Mode::Race(r) = &game.mode else { break };
            // The world update counter 0x7FB724 the think read (counted at
            // the update's end, so one less now).
            let update = r.replay.world_updates as i32 - 1;
            for (i, x) in r.racers.rivals.iter().enumerate() {
                if x.actions.kind != kinds[i] && x.unstuck.since.is_none() {
                    assert_eq!(update.rem_euclid(8), i as i32, "rival {i} changed action off its think");
                    changes += 1;
                }
                kinds[i] = x.actions.kind;
            }
        }
        assert!(changes > 0, "the rivals changed actions");
    }

    /// The grid stance (docs/formats.md, "The race start's grid stance";
    /// measured in the original: the car settles in the start scene's
    /// drive-in, and from the countdown's first frame its velocity is
    /// exactly 0 and the suspension bit-constant until the green): the
    /// player's car through the first race's countdown (the scene's frames
    /// from its cut, race status 3).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_player_stands_still_through_the_countdown() {
        let mut game = Game::new().unwrap();
        fresh_first_race(&mut game).unwrap();
        let mut stance: Option<[u32; 4]> = None;
        let mut frames = 0;
        for _ in 0..60 * 120 {
            game.advance_for(1.0 / 60.0).unwrap();
            let Mode::Race(r) = &game.mode else { panic!("left the race") };
            if r.timing.clock >= 0.0 {
                break;
            }
            if !game.scenes.shop.as_ref().is_some_and(|run| run.frame >= run.scene.marks[1] as f32) {
                continue;
            }
            let s = &game.sim.drive.as_ref().unwrap().state;
            assert_eq!(s.velocity, Vec3::ZERO, "countdown frame {frames}: the car moves");
            let now = s.wheels.map(|w| w.compression.to_bits());
            assert_eq!(*stance.get_or_insert(now), now, "countdown frame {frames}: the suspension moved");
            frames += 1;
        }
        assert!(frames > 30, "the countdown ran ({frames} frames)");
    }
}
