use super::{RaceTick, Session, TickEvents, TickInput, race_drive};
use crate::{
    Controls, RaceWorldSamples, debug_env, race_traffic_fill, scene_movers, step_race, traffic_body, traffic_coast,
};
use glam::Vec3;
use nfsu2::chase::{ChaseCamera, VIEW_ORDER};

impl Session<'_> {
    /// A race's start scene (with the screen flash and the crowd);
    /// `RaceTick::Stop` while it runs.
    pub(super) fn update_race_scene(&mut self, dt: f32, input: &TickInput, events: &mut TickEvents) -> RaceTick {
        let controls = input.controls;
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        // A SCREEN_FLASH still running past the scene keeps
        // stepping (0x5D7590, once a drawn frame).
        if self.scene.is_none() {
            d.screen_flash.step(dt * nfsu2::shop_scene::FPS);
        }
        if let Some(c) = self.crowd.as_mut() {
            c.frame += dt * nfsu2::shop_scene::FPS;
            if c.frame >= c.length {
                *self.crowd = None;
            }
        }
        if let Some(run) = self.scene.as_mut() {
            // The race's start scene (`race_start_scene`): every
            // car held on the grid; the scene's camera up to its
            // second mark, then the chase view with the pre-green
            // HUD, and the race on at the first mark with no
            // countdown.
            let [green, cut] = run.scene.marks.map(|m| m as f32);
            let was = run.frame;
            run.frame += dt * nfsu2::shop_scene::FPS;
            // SCREEN_FLASH (event type 10, 0x4395F0 → 0x4392E0 →
            // 0x5DBD80(1)): the scene's white flashes.
            d.screen_flash.advance(&run.scene.flashes, was, run.frame);
            if debug_env().flash_probe {
                eprintln!(
                    "flash: scene {:?} frame {:.2} flashes {:?} amount {}",
                    run.scene.kind,
                    run.frame,
                    run.scene.flashes,
                    d.screen_flash.amount()
                );
            }
            // The scene cars are on their NIS movers (mode 5) until
            // the cut: the held physics step below still runs their
            // drivetrain, lights and drivers, but their body and
            // wheels are put back before the mover steps them.
            let scene_kinematics: Vec<_> =
                std::iter::once(&d.state).chain(race.racers.rivals.iter().map(|r| &r.state)).map(|s| s.kinematics()).collect();
            // The cars run under the race start's hold through the
            // scene, as in the countdown below (race status 3: the
            // car update 0x6049D9 runs, so the brake flag 0x5F62C0
            // sees the hold's full brake, 0x5EB900(1.0), and the
            // brake lights, centre glow included, are lit before the
            // green; **observed** in the original's drag start,
            // compare/race112/sbs-drag-pregreen.jpg, the rivals' too
            // from their countdown pedals). Whether the update also
            // runs under the scene's own camera (before mark 2) is
            // **inferred**.
            let held = Controls { steer: if race.drag { 0.0 } else { controls.steer }, ..controls };
            let scene_samples = if race.racers.ai_routes.is_some() && input.ai.is_some() && !race.racers.rivals.is_empty() {
                for r in &mut race.racers.rivals {
                    r.state.held = true;
                }
                race.scene_camera = run.frame < cut;
                let samples = step_race(d, race, held, input.ai, dt, &mut events.contacts);
                race.scene_camera = false;
                d.tick_clock(dt, &mut self.rng);
                samples
            } else {
                let sample = d.state.begin_world_update(dt);
                let rivals = race.racers.rivals.iter().map(|r| Some(r.state.begin_world_update(dt))).collect();
                d.posed.extend(d.state.step_with_props(&d.car, &d.world, &mut d.props, held, dt, &mut events.contacts));
                d.state.finish_world_update(sample);
                d.impact_trigger();
                d.hold(dt, &mut self.rng);
                RaceWorldSamples { player: sample, rivals }
            };
            // The scene's cars (0x439240 → 0x4309D0, until the
            // cut): each racer on its grid slot's ZPM_Car<n>.
            if run.frame < cut {
                scene_movers(d, race, &run.scene, run.origin, run.frame, dt, &scene_kinematics);
                for (state, sample) in
                    std::iter::once(&mut d.state).chain(race.racers.rivals.iter_mut().map(|r| &mut r.state)).zip(
                        std::iter::once(scene_samples.player)
                            .chain(scene_samples.rivals.into_iter().map(Option::unwrap)),
                    )
                {
                    state.finish_world_update(sample);
                }
            }
            race.timing.clock = -(green - run.frame).max(0.0) / nfsu2::shop_scene::FPS;
            // The traffic is placed at the load and paused by the
            // scene (0x4099B0: inactive, so not drawn, its think
            // off) until race state 3, the end of the scene's
            // camera (0x430310 / 0x408CA0; **observed** under Wine,
            // race112). Meanwhile its cars brake on their
            // TrafficMovers to a stop (`traffic_coast`).
            let drag = d.car.car_type.handling.race.drag();
            race_traffic_fill(d, race, race.timing.clock, drag);
            if let Some(t) = race.racers.traffic.as_mut() {
                t.paused = run.frame < cut;
                let racers: Vec<nfsu2::ai::traffic::Other> =
                    race.racers.rivals.iter().map(|r| traffic_body(&r.car, &r.state)).collect();
                traffic_coast(
                    &d.world,
                    t,
                    &d.state,
                    nfsu2::ai::traffic::Other::half_box(&d.car.hull),
                    &racers,
                    if run.frame < cut { 0.0 } else { 1.0 },
                    dt,
                );
            }
            if run.frame < cut {
                let world = &d.world;
                let ground = |p: Vec3| world.road_height_below(p + Vec3::Z);
                let cam = run.camera.as_ref().and_then(|t| {
                    let fraction = run.frame / cut.max(1.0);
                    nfsu2::track_camera::entry_index(t, fraction).filter(|&i| t.entries[i].mover > 1).and_then(|i| {
                        nfsu2::track_camera::camera_in(t, i, fraction, run.origin, Some(run.origin), Some(&ground))
                    })
                });
                if let Some(cam) = cam {
                    d.camera = cam;
                }
            } else if was < cut {
                // The scene lets the cars go (0x437A00): off the NIS
                // movers, their placement counts reset.
                d.state.nis = None;
                for r in &mut race.racers.rivals {
                    r.state.nis = None;
                }
                // As after a shop scene: a fresh chase view (0x439070),
                // 0x5FA830(1), which also clears the shake slots (0x5729F0).
                d.impact.shake = Default::default();
                d.chase = ChaseCamera::new(&d.car, VIEW_ORDER[input.camera.min(4) as usize]);
                d.chase_view(0.0);
            }
            if run.frame >= green {
                *self.scene = None;
                race.timing.clock = 0.0;
            }
            return RaceTick::Stop;
        }
        RaceTick::Go
    }
}
