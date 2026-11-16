use super::Session;
use crate::sim::race::ShopRun;
use crate::{debug_env, scene_mover_step};
use glam::Vec3;
use nfsu2::chase::{ChaseCamera, VIEW_ORDER};

/// A shop scene's frame in the tick: it ran on (`Ran`), or it
/// reached its end, which the flow acts on (`Ended`: the drive-in opens
/// the shop, DynoEnter the dyno, DynoExit the test run, a type-4 scene
/// aims the GPS; the run goes on to the audio's tail). The glue's
/// `Game::shop_scene_end` takes it.
pub(crate) enum SceneTick {
    Ran,
    Ended(Box<ShopRun>),
}

impl std::fmt::Debug for SceneTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneTick::Ran => f.write_str("Ran"),
            SceneTick::Ended(run) => write!(f, "Ended({:?})", run.scene.kind),
        }
    }
}

impl Session<'_> {
    /// A frame of the shop scene (the step of `Game::shop_scene_frame`,
    /// moved here verbatim): the scene clock at 30 frames a
    /// second, its door messages as their time comes, the car on its NIS
    /// mover towards its channel (the scene origin times the key; see
    /// below), and the scene's camera track in the scene's frame
    /// (`Scene::camera`; the chase camera when it has none). At the end
    /// the car leaves its NIS mover and the scene comes back as
    /// `SceneTick::Ended` for the flow. `None` when there is no scene or
    /// no drive (a scene without a drive is dropped, as before).
    /// `camera`: the player's camera option (`TickInput::camera`).
    pub(crate) fn scene_frame(&mut self, dt: f32, camera: u8) -> Option<SceneTick> {
        let mut run = self.scene.take()?;
        let d = self.drive.as_mut()?;
        let sample = d.state.begin_world_update(dt);
        let previous_frame = run.frame;
        run.frame += dt * nfsu2::shop_scene::FPS;
        let mut told = false;
        while let Some(e) = run.scene.doors.get(run.events).filter(|e| e.time <= run.frame) {
            d.world.door_message(run.key, e.position, e.duration);
            run.events += 1;
            told = true;
        }
        let moved = d.world.step_doors(dt);
        d.posed.extend(moved);
        if told {
            let all = d.world.all_door_poses();
            d.posed.extend(all);
        }
        // Every scene's bound car is on its NIS mover (mode 5): 0x439240
        // has no scene-kind test, so the shop drive-in / drive-out, the
        // career start and DynoEnter go through 0x4309D0 (the channel's x / y
        // and heading as the target, the car's own height) → 0x5A40B0 →
        // 0x5AB3D0, as the race starts and endings do (**verified**;
        // **observed** under Wine: ShopEnter1 / ShopExit1 at a body
        // shop and DynoEnter on mode 5 with the centre of mass on the
        // channel, the car on the pit floor and up the ramp onto the
        // rollers; docs/evidence/shop-scene-mover/). The clone had put
        // the car's origin on the channel at the channel's height (0.29 m
        // behind the original's on the rollers; DynoEnter's floor drop).
        // The Enter skip's seek (NIS_SeekTo 0x43AF80 re-places the scene
        // cars, as `scene_cars` does for a race start) stands the car at
        // the scene's end.
        let ending = matches!(run.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_));
        if dt == 0.0 && run.frame >= run.scene.length && !ending {
            let (p, q) = run.scene.car_at(run.car_slot, run.scene.length);
            let at = run.origin.transform_point3(p);
            let forward = run.origin.transform_vector3(q * Vec3::X);
            let placed = nfsu2::drive::CarState::placed(
                &d.car.car_type,
                &d.world,
                Vec3::new(at.x, at.y, d.world.grid_height(at)),
                forward.y.atan2(forward.x),
            );
            if placed.position.is_finite() {
                d.state.position = placed.position;
                d.state.orientation = placed.orientation;
            }
            d.state.velocity = Vec3::ZERO;
            d.state.spin = Vec3::ZERO;
            d.state.angular_momentum = Vec3::ZERO;
        } else {
            scene_mover_step(&mut d.state, &d.car, &d.world, &run.scene, run.origin, run.car_slot, run.frame, dt);
        }
        d.state.finish_world_update(sample);
        d.hold(dt, &mut self.rng);
        if let Some(car) = run.companion.as_mut() {
            if ending {
                let sample = car.state.begin_world_update(dt);
                scene_mover_step(&mut car.state, &car.car, &d.world, &run.scene, run.origin, car.slot, run.frame, dt);
                car.state.finish_world_update(sample);
            } else {
                car.frame(&run.scene, run.origin, previous_frame, run.frame);
            }
        }
        // The scene camera's ground step (0x4427C0, `track_camera::to_world`).
        let world = &d.world;
        let ground = |p: Vec3| world.road_height_below(p + Vec3::Z);
        if let Some(cam) = run
            .camera
            .as_ref()
            .and_then(|t| run.scene.camera(t, run.frame.min(run.scene.length), run.origin, Some(&ground)))
        {
            d.camera = cam;
        }
        if debug_env().probe {
            eprintln!(
                "PROBE frame {:.1} origin {:?} car {:?} road {:?} cam {:?} fov {:?}",
                run.frame,
                run.origin.w_axis,
                d.state.position,
                d.world.road_height_below(d.state.position + Vec3::Z * 2.0),
                d.camera.position,
                d.camera.fov_y
            );
        }
        // A race's ending scene holds its last frame behind the post-race
        // screens (**observed** under Wine: a minute on, the player's NIS
        // id +0x234 still names it and the camera stays on the car; what
        // the scene does at its end is **not traced**).
        if run.frame < run.scene.length || matches!(run.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_)) {
            run.frame = run.frame.min(run.scene.length);
            *self.scene = Some(run);
            return Some(SceneTick::Ran);
        }
        // The scene's end (0x43BB10 once its clock +0x2C passes +0x34, the
        // header's +0x28 / 30 s; verified): 0x43B7A0 → 0x43B1F0 →
        // 0x437A00 gives each scene car back (0x5F6D60: the human
        // controller 0x5EBA40(1) and its physics mover 0x5F46B0(0x5F4410))
        // with no placement and its velocity kept — the scene drove it
        // physically (mover 5, 0x4309D0 → 0x5A40B0) — and 0x439070 gives
        // every racer whose camera mover is not type 1 a new chase mover
        // (0x5FA830(1) → 0x5F1FE0(1, 0), then its vtable +0x38, 0x445B20,
        // snaps its eases): no blend from the scene's camera. The chase
        // view then keeps its eye out of the walls (`Drive::chase_view`).
        // The car leaves its NIS mover (0x5F6D60 → 0x5F46B0): the next
        // scene's slot starts at count 0 again.
        d.state.nis = None;
        if run.camera.is_some() {
            d.chase = ChaseCamera::new(&d.car, VIEW_ORDER[camera.min(4) as usize]);
            d.chase_view(0.0);
        }
        if debug_env().probe {
            eprintln!("PROBE end car {:?} cam {:?} door {:?}", d.state.position, d.camera.position, run.origin.w_axis);
        }
        Some(SceneTick::Ended(Box::new(run)))
    }

    /// The race's ending scene behind the post-race screens (the menus'
    /// tick, before the dyno and the menus: `Game::update_menus` ran it
    /// there before), and its characters past the scene's length.
    pub(super) fn ending_scene(&mut self, dt: f32, camera: u8) -> Option<SceneTick> {
        // The race's ending scene plays on behind the post-race
        // screens.
        if self.scene.as_ref().is_some_and(|r| matches!(r.scene.kind, nfsu2::shop_scene::SceneKind::Ending(_))) {
            let tick = self.scene_frame(dt, camera);
            // The characters play on past the scene's length
            // (their loop flags; under Wine the crowd still moves
            // a minute on).
            if let Some(c) = self.crowd.as_mut() {
                c.frame += dt * nfsu2::shop_scene::FPS;
            }
            return tick;
        }
        None
    }
}
