//! `--flow postrace` (docs/notes/front-end/postrace-jitter.md): a race finished on the
//! player's autopilot and its post-race sequence — the 3 s "race over"
//! hold behind the line, the switch to the ending scene (EndingNis*,
//! 0x4309D0 → 0x5A40B0 mode 5) and the scene behind Event Results —
//! driven frame by frame as the window drives it (`Game::advance`, the
//! scene limiter's period while a scene holds the cars) at several display
//! rates, recording what each drawn frame shows: the player's car and the
//! camera as `draw_between` blends them, and the world time each frame
//! draws. It fails when a drawn frame's world time moves off its real
//! length by more than [`MAX_PACING`], or when more than [`MAX_REPEATS`]
//! of the display's vblanks (at the run's rate; 60 and 75 Hz for the
//! uncapped runs) show no newer state than the one before.
use crate::*;

/// The most a drawn frame's world time may move off its real length (the
/// 29/30/31 and 60/61-tick updates alone give ±3 %).
const MAX_PACING: f32 = 0.15;
/// The largest share of vblanks that may repeat the frame before.
const MAX_REPEATS: f32 = 0.02;
/// The display rates the uncapped runs are shown on (Hz; vsync off,
/// mailbox: the newest finished frame at each vblank).
const UNCAPPED_DISPLAYS: [f32; 2] = [59.98, 74.96];
/// A frame's time from its start to being ready to show (s).
const RENDER: f64 = 0.001;
/// Motion slower than this (m/s) is not judged (a standing camera).
const MIN_SPEED: f32 = 1.0;
/// Seconds of the ending scene recorded.
const SCENE_SECS: f32 = 8.0;

/// One drawn frame.
#[derive(Clone, Debug)]
pub(super) struct Row {
    /// Real time since the recording began (s).
    pub t: f64,
    pub interval: f32,
    pub updates: usize,
    pub alpha: f32,
    pub cap: bool,
    /// 0 racing, 1 finished (the hold), 2 the post-race menus.
    pub phase: u8,
    pub scene_frame: f32,
    /// The world time the frame draws (s): the newest state's, less the
    /// part of the last update the blend has not reached.
    pub drawn: f64,
    /// The world time this frame's updates stepped (s).
    pub world: f32,
    /// The camera mover (`Game::camera_mover`; a scene's shots 0x100 +).
    pub mover: u32,
    pub car: Vec3,
    pub camera: Vec3,
    /// The camera was cut (another mover, a jump): no blend this frame.
    pub cut: bool,
}

/// One run's verdict.
#[derive(Debug, Default)]
pub(super) struct PostRaceRun {
    pub label: String,
    /// Frames recorded per phase [race-over hold, ending scene].
    pub frames: [usize; 2],
    /// Worst drawn-time step off the frame's real length, as a fraction
    /// of it, per phase (the world drawn moving on with real time).
    pub pacing: [f32; 2],
    /// Per display refresh (Hz): the share of vblanks per phase that
    /// show no newer state than the vblank before (a frame shown twice
    /// while the scene moves: the 30 fps render's 2-3 / 4-5 vblank holds).
    pub repeats: Vec<(f32, [f32; 2])>,
    /// Informational: the worst velocity change of the drawn car and
    /// camera off their neighbours' per phase (a collision counts too).
    pub car: [f32; 2],
    pub camera: [f32; 2],
    /// World updates per real second in the scene (native 30).
    pub scene_rate: f32,
    /// Scene updates shorter than the limiter's 1/30 s (59 ticks): the
    /// NIS mover (`nis::SCENE_FRAME`) skips them and steps the next one
    /// twice, so a moving ending car stood still for a frame, then leapt.
    pub short_updates: usize,
    /// Drawn frames in the scene where the player's car stood still
    /// between two frames that moved it (over 0.5 m/s).
    pub stalls: usize,
}

impl PostRaceRun {
    pub fn failures(&self) -> Vec<String> {
        let mut f = Vec::new();
        for (i, phase) in ["race-over hold", "ending scene"].iter().enumerate() {
            if self.frames[i] < 10 {
                f.push(format!("{}: only {} frames of the {phase}", self.label, self.frames[i]));
            }
            if self.pacing[i] > MAX_PACING {
                f.push(format!(
                    "{}: {phase}: a drawn frame moved the world {:.0} % off the frame's real length",
                    self.label,
                    self.pacing[i] * 100.0
                ));
            }
            for (hz, r) in &self.repeats {
                if r[i] > MAX_REPEATS {
                    f.push(format!(
                        "{}: {phase}: {:.0} % of a {hz} Hz display's vblanks repeated the last frame",
                        self.label,
                        r[i] * 100.0
                    ));
                }
            }
        }
        if self.short_updates > 0 {
            f.push(format!("{}: {} scene updates shorter than the limiter's 1/30 s", self.label, self.short_updates));
        }
        if self.stalls > 0 {
            f.push(format!(
                "{}: the ending car stood still in {} drawn frames between moving ones",
                self.label, self.stalls
            ));
        }
        if !(29.0..=31.5).contains(&self.scene_rate) {
            f.push(format!(
                "{}: the scene's world ran {:.1} updates a second (native 30)",
                self.label, self.scene_rate
            ));
        }
        f
    }
}

/// The drawn car and camera this frame (as `draw_between` blends them).
fn drawn(game: &Game, alpha: f32) -> (Vec3, Vec3, bool) {
    let Some(d) = game.sim.drive.as_ref() else { return (Vec3::ZERO, Vec3::ZERO, true) };
    let now = nfsu2::interpolate::CarPose::of(&d.state);
    if alpha >= 1.0 || game.interp.prev.poses.is_empty() {
        return (now.position, d.camera.position, false);
    }
    let car = game
        .interp.prev.poses
        .first()
        .and_then(|p| nfsu2::interpolate::CarPose::blend(p, &now, alpha, game.pacing.last_update_dt))
        .map_or(now.position, |b| b.position);
    let cut = game.view_history_key() != game.interp.prev.mover;
    let cam =
        game.interp.prev.camera.as_ref().filter(|_| !cut).and_then(|a| nfsu2::interpolate::blend_camera(a, &d.camera, alpha));
    (car, cam.map_or(d.camera.position, |c| c.position), cam.is_none())
}

/// Worst velocity jitter of `pos` over `rows` (indices of one phase).
fn jitter(rows: &[Row], idx: &[usize], pos: impl Fn(&Row) -> Vec3) -> f32 {
    let v = |k: usize| -> Option<Vec3> {
        let (a, b) = (&rows[k - 1], &rows[k]);
        if b.cut || a.cut || pos(a).distance(pos(b)) > 10.0 {
            return None;
        }
        Some((pos(b) - pos(a)) / b.interval)
    };
    let mut worst = 0.0f32;
    for w in idx.windows(3) {
        if w[1] != w[0] + 1 || w[2] != w[1] + 1 || w[0] == 0 {
            continue;
        }
        let (Some(a), Some(b), Some(c)) = (v(w[0]), v(w[1]), v(w[2])) else { continue };
        let mean = (a + c) * 0.5;
        if mean.length() < MIN_SPEED {
            continue;
        }
        worst = worst.max((b - mean).length() / mean.length());
    }
    worst
}

/// The current race from the green (scene skipped) to [`SCENE_SECS`] into
/// the ending scene at `hz` (0 uncapped), the player on the autopilot.
pub(super) fn run_race(game: &mut Game, label: &str, hz: f32, csv: Option<&std::path::Path>) -> Result<PostRaceRun> {
    let mut run = PostRaceRun { label: label.to_string(), ..Default::default() };
    let mut rows: Vec<Row> = Vec::new();
    // One tick per pass (the start skip, the autopilot and the
    // reset happen between ticks), a row per drawn frame as it ends.
    let mut frames = super::ticks::Frames::new(hz);
    let mut skipped = false;
    let mut t = 0.0f64;
    let mut recording = false;
    let mut scene_start: Option<f64> = None;
    let mut scene_updates = 0usize;
    let mut world = 0.0f64;
    let log = std::env::var_os("NFSU2_POSTRACE_LOG").is_some();
    let mut dt_last = 0.0f32;
    let mut stuck = 0.0f32;
    let mark = super::rate::ticks();
    let mut scene_tick: Option<u64> = None;
    loop {
        // Skip the start scene (it is the race's other end).
        if !skipped
            && matches!(game.mode, Mode::Race(_))
            && game.scenes.shop.is_some()
            && super::rate::keyed(mark) > 2
        {
            game.tap_next(KeyCode::Enter);
            skipped = true;
        }
        if let (Mode::Race(r), Some(d)) = (&mut game.mode, game.sim.drive.as_ref())
            && r.timing.clock >= 0.0
            && r.autopilot.is_none()
            && let Some(routes) = r.racers.ai_routes.as_ref()
        {
            r.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
        }
        // A car stuck against a wall for 8 s is put back on the road (the
        // pause's Reset Car), so a long career race still ends.
        if let (Mode::Race(r), Some(d)) = (&game.mode, game.sim.drive.as_mut())
            && r.timing.clock > 0.0
            && r.racers.me.finished.is_none()
        {
            stuck = if d.state.speed().abs() < 2.0 { stuck + dt_last } else { 0.0 };
            if stuck > 8.0 {
                reset_to_road(d, r);
                stuck = 0.0;
            }
        }
        let phase = match &game.mode {
            Mode::Race(r) if r.racers.me.finished.is_some() => 1,
            Mode::Race(r) => {
                anyhow::ensure!(r.timing.clock < 900.0, "{label}: no finish in 15 minutes");
                if log
                    && let Some(d) = game.sim.drive.as_ref()
                    && (r.timing.clock / 10.0).floor() != ((r.timing.clock - dt_last) / 10.0).floor()
                {
                    println!(
                        "{label}: t {:.1} along {:.0} at {:?} v {:.1}",
                        r.timing.clock,
                        r.racers.me.along,
                        d.state.position,
                        d.state.speed()
                    );
                }
                0
            }
            Mode::Menu(_) => 2,
            m => anyhow::bail!("{label}: left the race ({})", flow_mode(m)),
        };
        // From 1 s before the line (the race clock's estimate) on.
        if !recording && phase == 1 {
            recording = true;
        }
        // The real time of this tick (the frame interval at 60 Hz: the
        // stuck clock and the log's 10 s marks).
        dt_last = game.tick_rate().update as f32;
        frames.tick_with(game, &mut |game, super::ticks::Drawn { alpha, interval: dt }| {
            world += f64::from(game.pacing.frame_world);
            let drawn_time =
                if alpha >= 1.0 { world } else { world - f64::from((1.0 - alpha) * game.pacing.last_update_dt) };
            if !recording {
                return Ok(());
            }
            let phase = match &game.mode {
                Mode::Race(_) => 1,
                _ => 2,
            };
            t += f64::from(dt);
            let (car, camera, cut) = drawn(game, alpha);
            let scene_frame = game.scenes.shop.as_ref().map_or(-1.0, |s| s.frame);
            if phase == 2 && game.ending_scene() && scene_start.is_none() {
                scene_start = Some(t);
            }
            if scene_start.is_some() {
                scene_updates += game.pacing.frame_updates;
            }
            rows.push(Row {
                t,
                interval: dt,
                updates: game.pacing.frame_updates,
                alpha,
                cap: game.scene_frame_cap(),
                phase,
                scene_frame,
                drawn: drawn_time,
                world: game.pacing.frame_world,
                mover: game.view_history_key(),
                car,
                camera,
                cut,
            });
            Ok(())
        })?;
        anyhow::ensure!(frames.frames() < 2_000_000, "{label}: too many frames");
        // The end: SCENE_SECS into the ending scene. A rate run counts the
        // scene's 1/30 s ticks after the tick that started it (a span of
        // drawn frames' time would end a display whose stalls drop time
        // past the tick cap a few ticks early; at every steady rate it is
        // the same tick as the time span's); the flow's own displays keep
        // the drawn frames' time.
        if scene_tick.is_none() && recording && !matches!(game.mode, Mode::Race(_)) && game.ending_scene() {
            scene_tick = Some(super::rate::ticks());
        }
        let end = if super::rate::active() {
            scene_tick.is_some_and(|s| super::rate::ticks() - s > (SCENE_SECS * 30.0) as u64)
        } else {
            scene_start.is_some_and(|s| t - s > f64::from(SCENE_SECS))
        };
        if end {
            break;
        }
        anyhow::ensure!(t < 60.0, "{label}: no ending scene within 60 s of the finish ({})", flow_mode(&game.mode));
    }
    if let Some(dir) = csv {
        use std::io::Write;
        std::fs::create_dir_all(dir)?;
        let mut f = std::io::BufWriter::new(std::fs::File::create(
            dir.join(format!("{}.csv", label.replace([' ', '/'], "_"))),
        )?);
        writeln!(
            f,
            "t,interval,updates,alpha,cap,phase,scene_frame,cut,car_x,car_y,car_z,cam_x,cam_y,cam_z,drawn,world,mover"
        )?;
        for r in &rows {
            writeln!(
                f,
                "{:.5},{:.5},{},{:.4},{},{},{:.3},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:#x}",
                r.t,
                r.interval,
                r.updates,
                r.alpha,
                u8::from(r.cap),
                r.phase,
                r.scene_frame,
                u8::from(r.cut),
                r.car.x,
                r.car.y,
                r.car.z,
                r.camera.x,
                r.camera.y,
                r.camera.z,
                r.drawn,
                r.world,
                r.mover
            )?;
        }
    }
    let displays: Vec<f32> = if hz > 0.0 { vec![hz] } else { UNCAPPED_DISPLAYS.to_vec() };
    run.repeats = displays.iter().map(|&r| (r, [0.0; 2])).collect();
    for (i, phase) in [1u8, 2].iter().enumerate() {
        // The scene from its first update on (its start is a cut and a load).
        let idx: Vec<usize> = (0..rows.len())
            .filter(|&k| rows[k].phase == *phase && (*phase == 1 || rows[k].scene_frame >= 1.0))
            .collect();
        run.frames[i] = idx.len();
        if *phase == 2 {
            run.short_updates = idx
                .iter()
                .filter(|&&k| {
                    rows[k].cap
                        && rows[k].updates > 0
                        && rows[k].world / (rows[k].updates as f32) < nfsu2::drive::nis::SCENE_FRAME * 0.999
                })
                .count();
            let speed = |k: usize| rows[k].car.distance(rows[k - 1].car) / rows[k].interval;
            run.stalls = idx
                .windows(3)
                .filter(|w| {
                    w[0] > 0
                        && w[2] == w[0] + 2
                        && !rows[w[1]].cut
                        && !rows[w[2]].cut
                        && speed(w[1]) < 0.01
                        && speed(w[0]) > 0.5
                        && speed(w[2]) > 0.5
                })
                .count();
        }
        run.car[i] = jitter(&rows, &idx, |r| r.car);
        run.camera[i] = jitter(&rows, &idx, |r| r.camera);
        for w in idx.windows(2).skip(2) {
            let (a, b) = (&rows[w[0]], &rows[w[1]]);
            if w[1] == w[0] + 1 {
                run.pacing[i] = run.pacing[i].max((((b.drawn - a.drawn) / f64::from(b.interval)) - 1.0).abs() as f32);
            }
        }
        // The display: each vblank shows the newest frame done by then.
        let (Some(&first), Some(&last)) = (idx.get(2), idx.last()) else { continue };
        for (hz, rep) in run.repeats.iter_mut() {
            let period = 1.0 / f64::from(*hz);
            let mut v = (rows[first].t / period).ceil() * period + 0.000_25;
            let (mut k, mut shown, mut n, mut same) = (first, None::<f64>, 0usize, 0usize);
            while v < rows[last].t {
                while k + 1 <= last && rows[k + 1].t + RENDER <= v {
                    k += 1;
                }
                let now = rows[k].drawn;
                if let Some(before) = shown {
                    n += 1;
                    same += usize::from(now <= before);
                }
                shown = Some(now);
                v += period;
            }
            rep[i] = if n > 0 { same as f32 / n as f32 } else { 0.0 };
        }
    }
    if let Some(s) = scene_start {
        run.scene_rate = scene_updates as f32 / (t - s) as f32;
    }
    Ok(run)
}

/// The events raced: `NFSU2_POSTRACE_EVENTS` (event indices), else the
/// profile's first open circuit or sprint and its first open drag.
fn events(game: &Game) -> Result<Vec<usize>> {
    if let Ok(v) = std::env::var("NFSU2_POSTRACE_EVENTS") {
        return Ok(v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    }
    let p = game.profile.as_ref().context("a profile")?;
    let open = p.open_events(&game.game_data.career);
    let mut picked = Vec::new();
    use nfsu2::frontend::career::EventKind;
    if let Some(e) = open.iter().find(|e| matches!(e.kind, EventKind::Circuit | EventKind::Sprint)) {
        picked.push(e.index);
    }
    if let Some(e) = open.iter().find(|e| matches!(e.kind, EventKind::Drag)) {
        picked.push(e.index);
    }
    anyhow::ensure!(!picked.is_empty(), "no open race event");
    Ok(picked)
}

/// `--flow postrace`. `NFSU2_FLOW_LOAD=<profile>` (with NFSU2_FLOW_DATA
/// holding it) races that career's next events, else a fresh career's
/// first race; `NFSU2_POSTRACE_HZ` (comma list, 0 = uncapped; default
/// 60,75,144,0); `NFSU2_POSTRACE_CSV=<dir>` writes each run's frames.
pub(crate) fn flow_post_race(game: &mut Game) -> Result<()> {
    let rates: Vec<f32> = std::env::var("NFSU2_POSTRACE_HZ")
        .ok()
        .map_or_else(|| vec![60.0, 75.0, 144.0, 0.0], |v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let csv = std::env::var_os("NFSU2_POSTRACE_CSV").map(std::path::PathBuf::from);
    match std::env::var("NFSU2_FLOW_LOAD") {
        Ok(name) => flow_resume_to_explore(game, name)?,
        Err(_) => {
            let mut p = Profile::new("POSTR");
            p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
            game.profile = Some(p);
            game.explore(None)?;
        }
    }
    let saved = game.profile.clone();
    let list = if std::env::var_os("NFSU2_FLOW_LOAD").is_some() {
        events(game)?
    } else {
        vec![
            game.game_data.career
                .events
                .iter()
                .find(|e| e.unlock == dday::STAGE0_RACES[0])
                .map(|e| e.index)
                .context("stage 0's first race")?,
        ]
    };
    let mut failures = Vec::new();
    for e in list {
        for &hz in &rates {
            let tag = if hz > 0.0 { format!("{hz} Hz") } else { "uncapped".to_string() };
            game.profile = saved.clone();
            game.platform.keys.clear();
            game.explore(None)?;
            game.start_race(e, false, false)?;
            let label = format!("event {e} {:?} {tag}", game.game_data.career.events[e].kind);
            let run = run_race(game, &label, hz, csv.as_deref())?;
            println!("{run:?}");
            failures.extend(run.failures());
        }
    }
    for f in &failures {
        println!("FAIL {f}");
    }
    // A rate run compares state across schedules; the flow's own
    // checks (made for steady rates) are reported there, not failed.
    if super::rate::report_only(failures.len()) {
        return Ok(());
    }
    anyhow::ensure!(failures.is_empty(), "{} post-race failures", failures.len());
    println!("post race: the finish and the ending scene drew steadily");
    Ok(())
}
