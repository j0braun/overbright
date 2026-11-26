use crate::*;

/// The race start scene's cars (0x439240 → 0x4309D0, **verified**; see
/// `Scene::racer`): every racer whose grid slot (racer info +0x03 =
/// slot + 1, 0x432860) has a `ZPM_Car<n>` in the scene goes to that car's
/// pose through the scene's origin: its x / y and the heading of the
/// transformed forward axis, on the ground there. Used for the Enter skip's
/// seek (NIS_SeekTo 0x43AF80 re-places the scene cars at the cut); the
/// frame-by-frame scene drives the cars on their NIS movers instead
/// (`scene_movers`).
pub(crate) fn scene_cars(
    d: &mut Drive,
    race: &mut Race,
    scene: &nfsu2::shop_scene::Scene,
    origin: glam::Mat4,
    from: f32,
    frame: f32,
    phases: &[[nfsu2::drive::WheelState; 4]],
) {
    let pose = |n: usize, frame: f32| {
        let (at, rot) = scene.racer(n, frame)?;
        let world = origin.transform_point3(at);
        let forward = origin.transform_vector3(rot * Vec3::X);
        Some((world, forward.y.atan2(forward.x)))
    };
    let place = |state: &mut CarState, car: &Car, slot: usize, phases: &[nfsu2::drive::WheelState; 4]| {
        let Some((at, heading)) = pose(slot, frame) else { return false };
        let placed = CarState::placed(&car.car_type, &d.world, Vec3::new(at.x, at.y, d.world.grid_height(at)), heading);
        state.position = placed.position;
        state.orientation = placed.orientation;
        // Keep the existing scene-pose approximation, but do not reset the
        // car/wheel state on every moving key. The velocity comes from two
        // scene samples, avoiding the initial teleport from the start grid.
        let dt = (frame - from) / nfsu2::shop_scene::FPS;
        let old_pose = pose(slot, from).filter(|_| dt > 0.0);
        state.velocity = old_pose.map_or(Vec3::ZERO, |(old, _)| (at - old) / dt);
        let spin = Vec3::Z
            * old_pose.map_or(0.0, |(_, old_heading)| {
                let change = heading - old_heading;
                change.sin().atan2(change.cos()) / dt
            });
        state.set_scripted_spin(&car.car_type.handling, spin);
        for (wheel, old) in state.wheels.iter_mut().zip(phases) {
            wheel.roll = old.roll;
            wheel.spinner = old.spinner;
            wheel.spinner_rate = old.spinner_rate;
        }
        scene_wheels(state, car, scene, slot, from, frame);
        true
    };
    if place(&mut d.state, &d.car, race.grid_slot + 1, &phases[0]) {
        race.racers.me = Progress::default();
        race.racers.me.place_on_road(&race.route, d.state.position.truncate());
    }
    for (index, r) in race.racers.rivals.iter_mut().enumerate() {
        if !place(&mut r.state, &r.car, r.slot + 1, &phases[index + 1]) {
            continue;
        }
        r.progress = Progress::default();
        r.progress.place_on_road(&race.route, r.state.position.truncate());
        // Keep the grid route while this loop approximates continuous NIS
        // movement by replacing pose. Native teleport placement (0x5FBD70)
        // DOES call controller +0x30(1) and +0x34, clearing route/cache state;
        // it is distinct from an ordinary target update. Resetting the route
        // every frame here previously selected wrong roads behind the start.
        // Port the actual teleport lifecycle with the full NIS mover.
    }
}

/// The race start scene's cars on their NIS movers (mode 5, verified chain
/// 0x439240 -> 0x4309D0 -> 0x5A40B0, then 0x5AB3D0 in the world step; see
/// `nfsu2::drive::nis`): each racer whose grid slot has a `ZPM_Car<n>`
/// gets the channel's x / y through the scene origin and the turned forward
/// axis' 16-bit heading as its target, and its mover drives the body there
/// physically (target velocity / impulses, suspension, authored burnout
/// and body torque). `kinematics` are the cars' bodies before this frame's
/// held physics step, which the scene cars do not run.
pub(crate) fn scene_movers(
    d: &mut Drive,
    race: &mut Race,
    scene: &nfsu2::shop_scene::Scene,
    origin: glam::Mat4,
    frame: f32,
    dt: f32,
    kinematics: &[nfsu2::drive::nis::Kinematics],
) {
    let world = &d.world;
    let drive = |state: &mut CarState, car: &Car, slot: usize, k: &nfsu2::drive::nis::Kinematics| {
        if scene.racer(slot, frame).is_none() {
            return false;
        }
        state.restore_kinematics(k);
        scene_mover_step(state, car, world, scene, origin, slot, frame, dt, false)
    };
    if drive(&mut d.state, &d.car, race.grid_slot + 1, &kinematics[0]) {
        race.racers.me = Progress::default();
        race.racers.me.place_on_road(&race.route, d.state.position.truncate());
    }
    for (index, r) in race.racers.rivals.iter_mut().enumerate() {
        if !drive(&mut r.state, &r.car, r.slot + 1, &kinematics[index + 1]) {
            continue;
        }
        r.progress = Progress::default();
        r.progress.place_on_road(&race.route, r.state.position.truncate());
    }
}

/// One scene car's mover for a frame (0x4309D0 -> 0x5A40B0, then 0x5AB3D0):
/// slot `slot`'s channel x / y through the origin and the turned forward
/// axis' 16-bit heading as the target, the slot's authored wheel/torque
/// controls. Every scene's bound cars go through it (0x439240 runs for race
/// starts and endings alike). False when the scene has no channel for it.
/// `paced`: the scene runs at the display's rate (a shop's), so the mover
/// keeps its own 30 fps schedule (`nis::ScenePace`) rather than taking the
/// frames given as the scene's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scene_mover_step(
    state: &mut CarState,
    car: &Car,
    world: &nfsu2::world::World,
    scene: &nfsu2::shop_scene::Scene,
    origin: glam::Mat4,
    slot: usize,
    frame: f32,
    dt: f32,
    paced: bool,
) -> bool {
    let mut mover = state.nis.take().unwrap_or_default();
    if paced {
        // A scene at the display's rate (the shops' drive-in / drive-out):
        // the steps of a 30 fps run on the scene's own schedule
        // (`nis::ScenePace`), however the world's updates slice the time.
        let mut pace = mover.pace.unwrap_or_else(|| nfsu2::drive::nis::ScenePace::new(frame - dt * nfsu2::shop_scene::FPS, state));
        pace.advance(dt);
        while let Some((step, at_frame)) = pace.due() {
            let first = mover.count == 0;
            pace.before = nfsu2::interpolate::CarPose::of(state);
            mover_step(state, car, world, scene, origin, slot, at_frame, step, &mut mover);
            // The first step places the car (a teleport): nothing to draw
            // it from.
            if first {
                pace.before = nfsu2::interpolate::CarPose::of(state);
            }
        }
        mover.pace = Some(pace);
        state.nis = Some(mover);
        return true;
    }
    // The mover runs on scene frames (`nis::SCENE_FRAME`); in game the
    // limiter already makes every frame one.
    mover.elapsed += dt;
    if mover.count > 0 && mover.elapsed < nfsu2::drive::nis::SCENE_FRAME * 0.999 {
        state.nis = Some(mover);
        return true;
    }
    let step = std::mem::take(&mut mover.elapsed);
    mover_step(state, car, world, scene, origin, slot, frame, step, &mut mover);
    state.nis = Some(mover);
    true
}

/// One step of [`scene_mover_step`]'s mover: `step` seconds towards the
/// slot's channel at scene frame `frame`.
#[allow(clippy::too_many_arguments)]
fn mover_step(
    state: &mut CarState,
    car: &Car,
    world: &nfsu2::world::World,
    scene: &nfsu2::shop_scene::Scene,
    origin: glam::Mat4,
    slot: usize,
    frame: f32,
    step: f32,
    mover: &mut nfsu2::drive::nis::NisMover,
) {
    let (at, rot) = scene.car_at(slot, frame.min(scene.length));
    let at = origin.transform_point3(at).truncate();
    let forward = origin.transform_vector3(rot * Vec3::X);
    let heading = nfsu2::ai::driver::angle16(forward.x, forward.y);
    mover.set_target(state, car, world, at, heading, step);
    let controls = nfsu2::shop_scene::wheels::controls(&scene.wheel_events, slot, frame);
    mover.nitro = nfsu2::shop_scene::wheels::effect(&scene.wheel_events, slot, 10, frame);
    state.step_nis(car, world, mover, controls, step);
    if debug_env().nis_probe {
        let w = |i: usize| {
            (state.wheels[i].spin, state.wheels[i].sliding, state.wheels[i].surface, state.wheels[i].on_ground)
        };
        let road = world.road_height_below(state.position + Vec3::Z * 2.0);
        eprintln!(
            "NIS frame {frame:.1} slot {slot} {} count {} pos {:?} road {road:?} v {:.2} wheels {:?} {:?} {:?} {:?} ctl {:?}",
            car.car_type.name,
            mover.count,
            state.position,
            state.speed(),
            w(0),
            w(1),
            w(2),
            w(3),
            controls
        );
    }
}

/// Native NIS wheel speeds (0x5AB890) with phase preserved across scene
/// placements. The pose velocity remains the clone's channel derivative,
/// not the original mode-5 rigid-body solution. The NIS tyres (0x5AB550)
/// expose hub slip and the >6 m/s skid flag through 0x5A2B70, which the
/// ground effects read as the car's exported pair (`particles::GroundFx`).
/// Torque/audio remain separate.
pub(crate) fn scene_wheels(
    state: &mut CarState,
    car: &Car,
    scene: &nfsu2::shop_scene::Scene,
    slot: usize,
    from: f32,
    frame: f32,
) {
    let speed = state.speed();
    let dt = ((frame - from) / nfsu2::shop_scene::FPS).max(0.0);
    let heading = state.heading();
    let steer = state.steer_angle();
    for (i, (wheel, spec)) in state.wheels.iter_mut().zip(&car.car_type.wheels).enumerate() {
        let (spin, roll) = nfsu2::shop_scene::wheels::motion(
            &scene.wheel_events,
            slot,
            from,
            frame,
            speed,
            spec.radius,
            car.car_type.handling.gearbox.rear_split,
            i,
        );
        wheel.spin = spin;
        wheel.roll = (wheel.roll + roll) % std::f32::consts::TAU;
        wheel.spinner_rate += (spin - wheel.spinner_rate) * (0.0035 / 0.0225) * dt;
        wheel.spinner = (wheel.spinner + wheel.spinner_rate * dt) % std::f32::consts::TAU;
        // NIS tyre slip, 0x5AB614..0x5AB758. Pose/yaw/steer inputs still
        // come from the existing placement approximation, not NISMover's
        // force solver. The physics contact supplies on_ground/surface.
        let arm = state.orientation * (spec.position - Vec3::from(car.car_type.handling.centre_of_mass));
        let hub = state.velocity + state.spin.z * Vec3::Z.cross(arm);
        let (sin, cos) = (heading + if i < 2 { steer } else { 0.0 }).sin_cos();
        wheel.slip = spin * spec.radius - (hub.x * cos + hub.y * sin);
        wheel.lateral_slip = hub.y * cos - hub.x * sin;
        // 0x5A2B70 copies the NIS pair to the car as it is (`nis.rs`).
        wheel.exported_slip = wheel.slip;
        wheel.sliding = wheel.on_ground && wheel.slip.hypot(wheel.lateral_slip) > 6.0;
    }
    // 5AB53F -> 5A2B70 publishes after NIS motion. This uses the existing
    // channel-derived pose/velocity approximation, not a native NIS trajectory.
    state.publish_forward_speed();
}

/// A shop scene playing (`Game::shop_scene_frame`).
pub(crate) struct ShopRun {
    pub(crate) audio: nfsu2::shop_scene::audio::Run,
    /// Player channel slot (ending entry +6 on a win, +7 on a loss).
    pub(crate) car_slot: usize,
    /// AI winner bound beside the player; independent of replay ownership.
    pub(crate) companion: Option<SceneCar>,
    pub(crate) scene: nfsu2::shop_scene::Scene,
    /// The shop (none for the career's start scene).
    pub(crate) shop: Option<usize>,
    /// The scene's space in the world.
    pub(crate) origin: glam::Mat4,
    /// Scene frames so far.
    pub(crate) frame: f32,
    /// Door messages sent.
    pub(crate) events: usize,
    /// The shop's world trigger (0x827ED4).
    pub(crate) key: u32,
    /// The scene's camera track (`Scene::camera_track`).
    pub(crate) camera: Option<nfsu2::formats::camera_tracks::CameraTrack>,
}

/// A scene-owned snapshot keeps the exact race car's fitted art/handling,
/// without changing the original car or physics state retained for replay.
pub(crate) struct SceneCar {
    pub(crate) car: Car,
    pub(crate) state: CarState,
    pub(crate) slot: usize,
    /// Its renderer is `RenderState.cars[id]`.
    pub(crate) id: CarId,
    /// The race's rival index it was taken from (its engine voice in the
    /// ending, `Game::ending_racers`).
    pub(crate) rival: usize,
}

impl SceneCar {
    pub(crate) fn from_rival(rival: &Rival) -> Self {
        let mut state = rival.state.clone();
        state.nis = None;
        Self { car: rival.car.clone(), state, slot: 0, id: CarId::new(), rival: 0 }
    }

    pub(crate) fn frame(&mut self, scene: &nfsu2::shop_scene::Scene, origin: glam::Mat4, from: f32, frame: f32) {
        let dt = (frame - from) / nfsu2::shop_scene::FPS;
        let sample = self.state.begin_world_update(dt);
        let (p, q) = scene.car_at(self.slot, frame.min(scene.length));
        let (_, turn, _) = origin.to_scale_rotation_translation();
        let w = self.car.car_type.wheels[0];
        let at = origin.transform_point3(p) - Vec3::Z * (w.position.z - w.radius);
        let before = self.state.position;
        self.state.position = at;
        self.state.orientation = (turn * q).normalize();
        self.state.velocity = if dt > 0.0 { (at - before) / dt } else { Vec3::ZERO };
        // Same bounded channel-placement approximation as the player.
        self.state.spin = Vec3::ZERO;
        self.state.angular_momentum = Vec3::ZERO;
        scene_wheels(&mut self.state, &self.car, scene, self.slot, from, frame);
        self.state.finish_world_update(sample);
    }
}

impl ShopRun {
    /// Whether the scene still holds the cars, so the frame limiter runs at
    /// 1/30 s (`display::SCENE_FRAME`). Every scene takes them at its start
    /// (0x43B000 → 0x43ACF0 sets property 1: header +0x20 is 1 in every
    /// scene and the ROOT skeleton, 0x434DC0, is GLOBALB's), and 0x437910 →
    /// 0x435F40 sets the 30. They go back at the scene's stop (0x439070 →
    /// 0x437A00), or, for the types 0 and 2 (race starts, DynoEnter /
    /// DynoExit; property 2, 0x43B000), from the update 0x43ABC0 → 0x439240
    /// once the scene clock (+0x2C) is past header +0x30 / 30 (+0x30, the
    /// start scenes' camera cut).
    pub(crate) fn holds_cars(&self) -> bool {
        match self.scene.scene_type {
            0 | 2 => self.frame <= self.scene.marks[1] as f32,
            _ => true,
        }
    }

    /// Whether the scene's own camera is the view (a race's start scene up
    /// to its cut, an ending, the career's start, the dyno): the scene's
    /// shots, not the player's chase mover, are drawn.
    pub(crate) fn camera_holds_view(&self) -> bool {
        self.holds_cars() && self.camera.is_some()
    }
}

/// A race start scene's characters playing (`Game::scene_crowd`).
pub(crate) struct SceneCrowd {
    /// Shared with the skinning job on a render thread.
    pub(crate) set: std::sync::Arc<nfsu2::character::SceneCharacters>,
    pub(crate) origin: glam::Mat4,
    /// Scene frames so far (30 a second).
    pub(crate) frame: f32,
    /// The scene's length (header +0x28).
    pub(crate) length: f32,
    /// Its renderer is `RenderState.crowds[id]`.
    pub(crate) id: DrawId,
    /// Per character the filtered ground it stands on (0x436050 instance
    /// +0x20, `SceneCrowd::settle`); `None` before one is found.
    pub(crate) lifts: Vec<Option<f32>>,
}

impl SceneCrowd {
    /// A scene's characters from their first frame, each put on the
    /// ground at once (0x437BE0 runs the ground step on its set-up).
    pub(crate) fn new(
        set: std::sync::Arc<nfsu2::character::SceneCharacters>,
        origin: glam::Mat4,
        length: f32,
        world: &nfsu2::world::World,
    ) -> SceneCrowd {
        let mut c = SceneCrowd { set, origin, frame: 0.0, length, id: DrawId::new(), lifts: Vec::new() };
        c.settle(world, 1.0);
        c
    }

    /// Advance `dt` seconds and put each character on the ground again.
    pub(crate) fn advance(&mut self, dt: f32, world: &nfsu2::world::World) {
        self.frame += dt * nfsu2::shop_scene::FPS;
        self.settle(world, dt * nfsu2::shop_scene::FPS);
    }

    /// The character update's ground step (0x436050, a skinned character
    /// with +0x18 set, which its constructor 0x431220 sets and nothing
    /// clears): the ground (0x5E72F0, `World::grid_ground`) under its bone
    /// 1 (0x431280(1)) through its current world matrix; when found, the
    /// first is kept as is and later ones are blended in
    /// (`character::filter_ground`, `frames` scene frames since the last),
    /// and the world matrix's height becomes that ground + 0.05.
    pub(crate) fn settle(&mut self, world: &nfsu2::world::World, frames: f32) {
        let t = self.frame / nfsu2::shop_scene::FPS;
        self.lifts.resize(self.set.instances.len(), None);
        for (i, p) in self.set.ground_probe(t, self.origin, &self.lifts).into_iter().enumerate() {
            if let Some(g) = p.and_then(|p| world.grid_ground(p)) {
                self.lifts[i] = Some(nfsu2::character::filter_ground(self.lifts[i], g, frames));
            }
        }
    }
}

