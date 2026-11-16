use super::{RaceTick, Session, TickEvents, TickInput, race_drive};
use crate::{
    Drive, Playback, Race, Recording, SKIP_WAIT, SkipTo, debug_env, lift_car, race_frame, replay_typed_input,
    traffic_lift_cars,
};
use glam::{Vec2, Vec3};
use nfsu2::chase::ChaseCamera;
use nfsu2::render::Camera;

impl Session<'_> {
    /// A replay's frame; always ends the update (`RaceTick::Stop`, or
    /// `ReplayLeft` when a skip left it).
    pub(super) fn update_replay(&mut self, dt: f32, input: &TickInput, events: &mut TickEvents) -> RaceTick {
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        let Some(mut play) = race.replay.playback.take() else { unreachable!("update_replay without a playback") };
        // A replay (the manager's frame, 0x581060): the recorded
        // frames at their own dt, as many as the real time
        // covers; held while paused, and in slow motion on
        // three render frames of four (0x865148 & 3, manager
        // +0x1C; the frame loop then skips the simulation,
        // 0x5813A3). Over at the last frame.
        let Some(rec) = race.replay.recording.take() else { return RaceTick::Stop };
        play.clock += dt;
        play.frames = play.frames.wrapping_add(1);
        let mut leave = false;
        // A skip (0x4C10B0): restored 0.5 s after it was asked
        // for, then playing (manager +0x14 = 0); after Back
        // (+0xD2: THE_PRESENT, or a skip already waiting) the
        // replay is left (0x4A50E0).
        if let Some((to, since)) = play.skip
            && play.clock - since >= SKIP_WAIT
        {
            play.skip = None;
            play.paused = false;
            play.carry = 0.0;
            match to {
                SkipTo::Lap(i) => {
                    let lap = rec.mark(i);
                    lap.state.clone().restore(d, race);
                    play.at = lap.frame;
                }
                SkipTo::Present => {
                    if let Some(p) = play.present.take() {
                        p.restore(d, race);
                    }
                    play.at = rec.frames.len();
                }
            }
            leave = play.leaving;
            if let Some(view) = play.camera.view {
                d.chase = ChaseCamera::new(&d.car, view);
            }
            // The restore resets the race cameras (0x57C1E0 →
            // 0x5FAF90): the director's time on its view
            // (+0x214) starts again (**observed**: +0x214 = the
            // skip's real tick), +0x224 = 1, and a track shot
            // is dropped for the chase view (+0x220 = 0,
            // 0x5FA830(1); **verified** 0x5FAF90).
            play.camera.reset(play.clock);
            play.track.shot = None;
            play.typed_shot = None;
            play.last_velocity = None;
        }
        let held = play.paused || play.skip.is_some() || (play.slow && play.frames % 4 != 0);
        if !held {
            // Smooth slow motion: the replay's time at ½ / ¼
            // of the real time, drawn between its frames.
            play.carry += if play.smooth > 0.0 { dt * play.smooth } else { dt };
        }
        while let Some(f) = rec.frames.get(play.at).filter(|f| f.dt <= play.carry + 1e-6) {
            play.carry -= f.dt;
            play.at += 1;
            race_frame(d, race, f, input.ai, input.route_triggers, input.lap_info, true, &mut self.rng, &mut events.contacts);
        }
        replay_direct(&mut play, &rec, d, race, &mut self.rng, dt);
        if !play.paused && play.skip.is_none() {
            play.last_velocity = Some(d.state.velocity);
        }
        replay_camlog(&play, d, race);
        // An extra camera (index 6 on, Replay Cameras
        // "Extended") replaces the chase view's.
        if let Some(&extra) = play.camera.index.checked_sub(6).and_then(|i| nfsu2::replay_cams::EXTRAS.get(i))
            && !play.paused
        {
            let target = nfsu2::replay_cams::Target { transform: d.state.transform(), velocity: d.state.velocity };
            // Replay time (still between the slow frames).
            let t = rec.frames[..play.at.min(rec.frames.len())].iter().map(|f| f.dt).sum::<f32>();
            d.camera = play.extra.camera(extra, &target, Some(&race.route.road), race.racers.me.along, t);
        }
        play.last_eye = Some(d.camera.position);
        // The last recorded frame: the replay holds there,
        // paused with the bar up, until Back or a skip
        // (**verified**: the manager's end of frame 0x578E20,
        // after the simulation at 0x5813CA / 0x581426, clears
        // playing (+0) and the held frame (+0x1C) and, when the
        // loop counter is +0x7C − 1 — the last recorded loop
        // just played — sets paused (+0x14 = 1, 0x578E8C) and
        // sends 0x7C8D5450 (0x55DDF0), which UI_ReplayControl's
        // handler takes at 0x4D7C83 → 0x4D7CD1: SHOW, and the
        // bar back if hidden; **observed** under Wine). Play /
        // pause has no frames left then (0x4A4E80).
        if play.at >= rec.frames.len() && play.skip.is_none() {
            play.paused = true;
        }
        race.replay.recording = Some(rec);
        race.replay.playback = Some(play);
        // Leaving the replay is the flow's (`Game::end_replay`, right
        // after the step).
        if leave { RaceTick::ReplayLeft } else { RaceTick::Stop }
    }
}

/// The replay's cameras on automatic: a track camera (route or typed) or
/// a chase view.
fn replay_direct(
    play: &mut Playback,
    rec: &Recording,
    d: &mut Drive,
    race: &Race,
    rng: &mut nfsu2::opponents::GameRng,
    dt: f32,
) {
    if !play.paused && play.skip.is_none() {
        // On automatic the track cameras come first, and
        // while one shows no chase view is picked (0x5FB060).
        let was = play.track.shot.is_some() || play.typed_shot.is_some();
        // The view showing, as 0x448280 reads it (camera
        // +0x1E0: the eye's velocity over the frame).
        let view = nfsu2::track_camera::View {
            eye: d.camera.position,
            forward: d.camera.forward(),
            fov: d.camera.game_fov.unwrap_or(d.camera.fov_y),
            velocity: play.last_eye.filter(|_| dt > 0.0).map_or(Vec3::ZERO, |p| (d.camera.position - p) / dt),
        };
        // 0x5FB060 on automatic: a chase view drawn holds
        // 1.5–4 s outright and under 1.5 s keeps the track
        // draw off (unless just reset); a held track runs
        // while its entry is a track camera.
        let gate = if play.camera.index == 0 { play.camera.gate(play.clock, was) } else { None };
        // The shot held: a route track, or a typed one (its
        // start tells two draws of one track apart).
        let shot_id = |p: &Playback| {
            p.track
                .shot
                .as_ref()
                .map(|s| (0usize, s.group, s.track, 0.0f32))
                .or(p.typed_shot.map(|t| (1, t.group, t.track, t.start)))
        };
        let held = shot_id(&play);
        // The replay's own time (the sim clock): a typed
        // track's fraction is the time since it began over
        // its duration (0x442B70).
        let sim: f32 = rec.frames[..play.at.min(rec.frames.len())].iter().map(|f| f.dt).sum();
        let track = replay_track(play, d, race, rng, gate, was, &view, sim, dt);
        match track {
            Some(cam) => {
                d.camera = cam;
                // A new shot (0x5FB33B: another track, or
                // the mover not yet 0x12): the ring of the
                // last three, mover 0x12 (0x5F1FE0 stamps
                // +0x214), +0x224 cleared.
                let now_id = shot_id(&play);
                if now_id != held || !was {
                    if let Some((kind, g, t, _)) = now_id
                        && kind == 1
                    {
                        play.recent.push((g, t));
                    } else {
                        play.recent.push((usize::MAX, usize::MAX));
                    }
                    if play.recent.len() > 3 {
                        play.recent.remove(0);
                    }
                    play.camera.track_started(play.clock);
                } else {
                    play.camera.last = play.camera.index;
                }
            }
            None if gate.is_none() && play.camera.index == 0 => {}
            None => {
                if was {
                    d.camera = d.chase.clone().update(&d.state, 0.0, d.fov_offset);
                }
                let view = play.camera.view;
                play.camera.direct(&mut *rng, d.state.speed(), play.clock, was);
                if let Some(v) = play.camera.view
                    && Some(v) != view
                {
                    d.chase = ChaseCamera::new(&d.car, v);
                }
            }
        }
    }
}

/// The track camera on automatic (`gate` the chase view's gate): a held
/// typed shot, else a route track, else a new typed one; `None` for the
/// chase view.
#[allow(clippy::too_many_arguments)]
fn replay_track(
    play: &mut Playback,
    d: &Drive,
    race: &Race,
    rng: &mut nfsu2::opponents::GameRng,
    gate: Option<bool>,
    was: bool,
    view: &nfsu2::track_camera::View,
    sim: f32,
    dt: f32,
) -> Option<Camera> {
    if let Some(may_draw) = gate {
        let dr: &Drive = d;
        // The world list's other cars the car test sees (active, +0x550):
        // the rivals and the race's drawn traffic.
        let others: Vec<nfsu2::chase::LiftCar> = race
            .racers.rivals
            .iter()
            .filter_map(|r| lift_car(&r.car, &r.state))
            .chain(traffic_lift_cars(race.racers.traffic.as_ref().filter(|t| !t.paused)))
            .collect();
        let car = d.state.transform();
        // A held track is drawn again with bVar5 when
        // the Jump weight 0x441B40 is over 0 or car
        // +0xB8 < 0 (0x5FB060; the typed mover's +0x84
        // request is not ported): the car upside down or
        // a long flight predicted (0x5EC3E0).
        if may_draw {
            let i = replay_typed_input(d, play.typed_top, play.last_velocity, dt);
            if i.upside_down || i.long_flight {
                play.typed_shot = None;
                play.track.shot = None;
            }
        }
        // A typed shot runs while its entry is a camera.
        let mut cam = play.typed_shot.and_then(|t| nfsu2::track_camera::follow_typed(&play.typed, &t, sim, car));
        if cam.is_none() {
            play.typed_shot = None;
            // 0x44FE80 → 0x44F9A0: the route groups
            // first, then the typed ones; a held shot that
            // ended is drawn again whatever the gate says.
            let may = may_draw || was;
            // 0x5FB060 → 0x44FE80 → 0x44F9A0: the
            // group of the car's own road record, with the race
            // coordinates built (0x88F2BE); one shared-generator unit(1.0)
            // before a draw while +0x224 is clear (`direct_on`). The
            // visibility test also fails inside another car (0x448280 →
            // 0x5F1500, `eye_in_car`).
            let routes = race.racers.ai_routes.as_ref();
            let record = routes.and_then(|r| replay_road_record(d, race, r));
            let built = routes.is_some_and(|r| r.race_coords);
            cam = play.track.direct_on(
                &play.tracks,
                record,
                built,
                race.route.road.length,
                race.racers.me.along,
                car,
                d.state.speed(),
                &mut *rng,
                &view,
                may,
                play.camera.fresh,
                &mut |a, b| dr.sight_blocked(a, b) || nfsu2::track_camera::eye_in_car(b, &others),
            );
            if cam.is_none() && may {
                let input = replay_typed_input(d, play.typed_top, play.last_velocity, dt);
                if let Some(t) = nfsu2::track_camera::draw_typed(
                    &play.typed,
                    &input,
                    &play.recent,
                    car,
                    &view,
                    &mut *rng,
                    &mut |a, b| dr.sight_blocked(a, b) || nfsu2::track_camera::eye_in_car(b, &others),
                    sim,
                ) {
                    play.typed_shot = Some(t);
                    cam = nfsu2::track_camera::follow_typed(&play.typed, &t, sim, car);
                }
            }
        }
        // A track camera only with the car's forward
        // speed (+0x430) at 0 or more (0x5FB30F).
        let cam = cam.filter(|_| d.state.speed() >= 0.0);
        if cam.is_none() {
            play.track.shot = None;
            play.typed_shot = None;
        }
        // An entry with +7: the push from its look
        // point (0x453F50 → 0x444530(view, 0, &look);
        // the mover's wall radius 0x444180: 0.7).
        cam.map(|mut c| {
            if let Some(look) = play.track.push_from.filter(|_| play.track.shot.is_some()) {
                dr.push_from(&mut c, look, 0.7);
            }
            c
        })
    } else {
        if play.camera.index != 0 {
            play.track.shot = None;
            play.typed_shot = None;
        }
        None
    }
}

/// The car's own road record (car +0x2C → +0x0C → 0x40F690): the clone's
/// stand-in for the human controller's road node, as the reset takes it
/// (`reset_to_road`: the autopilot's node, else the nearest human-flagged
/// control point; **inferred** equivalent).
fn replay_road_record<'a>(
    d: &Drive,
    race: &Race,
    routes: &'a nfsu2::ai::route::AiRoutes,
) -> Option<&'a nfsu2::formats::ai_routes::RouteRecord> {
    let h = d.state.heading();
    let (record, _, _) = race.autopilot.as_ref().and_then(|a| a.route.as_ref()).and_then(|r| r.node()).or_else(|| {
        nfsu2::ai::route::RacerRoute::find_flagged(
            routes,
            d.state.position,
            Vec2::new(h.cos(), h.sin()),
            Some(race.racers.me.along),
            true,
            0x0800_0000,
        )
    })?;
    routes.file.records.get(record)
}

/// `NFSU2_REPLAY_CAMLOG`'s line for this playback frame.
fn replay_camlog(play: &Playback, d: &Drive, race: &Race) {
    // Check aid (docs/replay-cameras.md): NFSU2_REPLAY_CAMLOG=file
    // appends one line per playback frame — clock, index,
    // chase view, +0x214 age, +0x224, the track shot.
    if let Some(path) = debug_env().replay_camlog.clone() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let shot = play
                .track
                .shot
                .as_ref()
                .map(|s| format!("r{}:{}", s.group, s.track))
                .or(play.typed_shot.map(|t| format!("t{}:{}{}", t.group, t.track, if t.mirror { "m" } else { "" })))
                .unwrap_or_else(|| "-".to_string());
            let routes = race.racers.ai_routes.as_ref();
            let group = nfsu2::track_camera::route_group_of(
                &play.tracks,
                routes.and_then(|r| replay_road_record(d, race, r)),
                routes.is_some_and(|r| r.race_coords),
                race.route.road.length,
                race.racers.me.along,
            )
            .map_or("-".to_string(), |(g, f)| format!("{g}@{f:.3}"));
            let _ = writeln!(
                f,
                "{:.4}\t{}\t{}\t{:?}\t{:.3}\t{}\t{}\t{:.2}\t{}\t{}",
                play.clock,
                play.at,
                play.camera.index,
                play.camera.view,
                play.camera.since.map_or(-1.0, |s| play.clock - s),
                play.camera.fresh as u8,
                shot,
                d.state.speed(),
                (play.paused || play.skip.is_some()) as u8,
                group
            );
        }
    }
}
