use crate::*;

/// The flight prediction (0x5EC3E0) for the player's car over `window`
/// seconds, stopping at the first landing: from the mean of the wheels'
// prov: inferred game.jump_flight
/// contact points (each wheel's centre less its radius; **inferred**: the
/// static wheel places, not the suspension's), the ground the 0x5E72F0
/// query finds (with its normal for a touch), the downforce (record +0x374
/// + +0x378) × 0.3 × speed / mass.
pub(crate) fn jump_flight(d: &Drive, window: f32) -> nfsu2::event_camera::Flight {
    let s = &d.state;
    let h = &d.car.car_type.handling;
    // 0x5EC3E0 starts from the wheel matrices with their suspension
    // offsets (observed under Wine).
    let start = s.flight_start(&d.car);
    let speed = s.speed();
    let downforce = (h.downforce[0] + h.downforce[1]) * 0.3 * speed / h.mass.max(1e-3);
    let ground = |p: Vec3| {
        let z = d.world.grid_height(p);
        let n = d.world.road_below(Vec3::new(p.x, p.y, z + 0.5)).map_or(Vec3::Z, |(_, n)| n);
        (z, n)
    };
    nfsu2::event_camera::predict_flight(start, s.velocity, speed, window, downforce, &ground, true)
}

/// 0x5F1EF0, the crash camera's placement: on the car's route (car +0x2C,
/// vt+0x24's node list) from the node nearest the car (0x4198C0), the
/// first node at least 75 m further on (node +0x54) must be more than four
/// nodes on (counting both), else no camera (and no event). The route is
// prov: inferred game.crash_route-3
/// the player's route from where the car is (**inferred**: the human car's
/// +0x2C as a fresh list from the car's node); its nodes and
/// the far node's index; the list follows the car's heading
/// (`RacerRoute::start_human`, observed). Before, the trigger read the race road's
/// points instead, 30–55 m apart on Quick Race circuits, so a crash on a
/// straight never placed (the original's crash camera did, observed).
/// The node list 0x5F1EF0 walks is the controller's own +0x68 list, which
/// 0x418560 returns (**verified** identity); the clone builds
/// a fresh list from the car's node instead (**inferred** equivalent).
pub(crate) fn crash_route(
    d: &Drive,
    routes: Option<&nfsu2::ai::route::AiRoutes>,
) -> Option<(Vec<nfsu2::event_camera::NodeEdges>, usize)> {
    let routes = routes?;
    let centre = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
    let f = d.state.forward();
    let route = nfsu2::ai::route::RacerRoute::start_human(routes, centre, Vec2::new(f.x, f.y).normalize_or(Vec2::X))?;
    let nodes = route.node_edges(routes, 2.0);
    let far = nfsu2::event_camera::far_node(&nodes)?;
    Some((nodes, far))
}

/// The crash / jump camera's mover (Camera_Factory 0x0F / 0x10 → 0x44BEA0,
/// update 0x44C350), built when its kind starts and run each frame in
/// place of the chase view; the factory keeps the chase camera when the
/// shot cannot be placed or its eye is out of sight of the car. The
/// route's nodes are the player's racer route from where the car is
// prov: inferred game.crash_route
/// (**inferred**: the human car's route +0x2C as a fresh racer route).
// prov: inferred game.crash_route-2
/// The chase camera keeps easing meanwhile (**inferred** deviation: the
/// game parks it, Camera_Factory with "keep" set, +0x23C).
pub(crate) fn crash_camera(d: &mut Drive, race: &mut Race, dt: f32, crash_velocity: Option<Vec3>) {
    let road = &race.route.road;
    let locate = |p: Vec2| {
        let mut loc = nfsu2::formats::roads::Locator::default();
        road.locate(&mut loc, p);
        loc.t
    };
    let on_road = ShotRoad { progress: race.racers.me.locator.fraction, locate: &locate };
    event_shot(d, &mut race.camera, race.racers.ai_routes.as_ref(), Some(on_road), race.timing.clock, dt, crash_velocity);
}

/// What a race hands the crash camera's record (Camera_Factory 0x5F1FE0
/// case 0x0F): the car's progress (+0x8F8) and the road locator's +0x1C
/// at a place (0x5DEDA0). Outside a race the record takes 0 and 1.0
/// (**inferred**: g_RouteCoordRace 0x88F2BE, which Race_BuildRoute sets,
/// is clear in free roam).
pub(crate) struct ShotRoad<'a> {
    pub(crate) progress: f32,
    pub(crate) locate: &'a dyn Fn(Vec2) -> f32,
}

/// The crash / jump camera's mover for a camera director's events `cam`
/// (a race's or free roam's), the car's route list `routes`, the race's
/// road (none in free roam) and the clock the parked chase camera's age
/// runs on. `crash_velocity` is the car's velocity before the world
/// update when Car_Crash started the event in it: 0x59FB80 runs inside the
/// contact before its push (0x5A9630 → 0x59FB80, then the impulse), so the
/// factory's place (0x5F1FE0 case 0x0F, the dot with car +0x70 at
/// 0x5F2710..0x5F275E) reads the velocity from before the hit (observed
/// under Wine: a head-on hit that threw the car backwards placed the eye
/// on the side the forward motion picks; the update's start standing for
/// the push's instant is **inferred**). A jump camera (0x604750, after
/// the motion) reads the velocity now.
pub(crate) fn event_shot(
    d: &mut Drive,
    cam: &mut RaceCamera,
    routes: Option<&nfsu2::ai::route::AiRoutes>,
    road: Option<ShotRoad>,
    clock: f32,
    dt: f32,
    crash_velocity: Option<Vec3>,
) {
    let centre = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
    let progress = road.as_ref().map_or(0.0, |r| r.progress);
    let started = cam.events.started_now.take();
    d.event_mover = 0;
    // The factory's keep flag (0x6022F0 / 0x604CD3 / 0x604FD3 push 1 for
    // 0x0F and 0x10): the chase camera is parked, not updated, and a new
    // mover made — the crash mover, or a fresh chase camera (0x451660)
    // when the sight is blocked. Kind 7 starts only after 0x604D9C ended
    // kind 6 (0x5FFE80: the parked one back) in the same update, so the
    // same chase camera is parked again and the blocked kind 6's fresh one
    // is dropped (observed, Wine log b3: mover 0x04915AB0 → 0x0ED32740, park
    // 0x04915730 throughout).
    if matches!(started, Some(6 | 7)) {
        let fresh = ChaseCamera::new(&d.car, d.chase.kind());
        let parked = std::mem::replace(&mut d.chase, fresh);
        if cam.parked_chase.is_none() {
            cam.parked_chase = Some((parked, clock));
        }
    }
    let ground = |p: Vec3| d.world.grid_height(p);
    let blocked = |a: Vec3, b: Vec3| d.sight_blocked(a, b);
    if started == Some(6) {
        cam.shot = crash_route(d, routes).and_then(|(nodes, far)| {
            let velocity = crash_velocity.unwrap_or(d.state.velocity);
            let mut shot = nfsu2::event_camera::plan_shot(&nodes, far, centre, velocity, &ground, &blocked)?;
            // Record +0x0C / +0x10: the car's progress (+0x8F8, in a
            // race) and the eye's locator's +0x1C (0x5DEDA0); 0 and 1.0
            // outside a race.
            shot.from = progress;
            shot.to = road.as_ref().map_or(1.0, |r| (r.locate)(shot.eye.truncate()));
            Some(shot)
        });
    }
    // 0x604D2F, on the eighth updates only, with kind 6 running at that
    // point of 0x604750 (so also on the update that then changes it to 7,
    // before case 0x10 reads the place): the car falling (+0x78 < 0), the
    // second shot's eye rises to the car's height (+0x298 = +0x68).
    if cam.events.eighth()
        && (cam.events.kind == 6 || started == Some(7))
        && d.state.velocity.z < 0.0
        && let Some(shot) = cam.shot.as_mut()
        && shot.second.z < centre.z
    {
        shot.second.z = centre.z;
    }
    if started == Some(7) {
        cam.shot = cam.shot.as_ref().map(|s| s.second_shot(centre, progress, &ground, &blocked));
    }
    // A fresh chase mover in place of a blocked shot is updated in the
    // frame it is made (Wine log b3: view 1 moved on the event's frame).
    if matches!(started, Some(6 | 7)) && !cam.shot.as_ref().is_some_and(|s| s.visible) {
        d.chase_view(dt);
    }
    if !matches!(cam.events.kind, 6 | 7) {
        cam.shot = None;
        // 0x5FFE80 → 0x5FFDE0: the parked chase camera current again,
        // updated in the same frame with every ease snapped (+0x52;
        // `ChaseCamera::resume`).
        if let Some((mut chase, at)) = cam.parked_chase.take() {
            chase.resume(clock - at);
            chase.set_look_back(d.chase.looking_back());
            d.chase = chase;
            d.chase_view(dt);
        }
        return;
    }
    let Some(shot) = cam.shot.as_ref() else { return };
    if !shot.visible {
        return;
    }
    d.event_mover = if cam.events.kind == 7 { 0x10 } else { 0x0F };
    let mut camera = shot.view(centre, progress);
    camera.near = d.camera.near;
    camera.far = d.camera.far;
    d.camera = camera;
}

/// The view leaves the player's own car out (bumper and driver always, the
/// hood while looking back: `ChaseCamera::hides_car`, the chase mover's
/// camera +0x280 that the car list's draw 0x60D584 skips) only while the
/// chase mover is the view's. Under a scene's own camera (`scene`: a race's
/// start scene up to its cut, an ending) the car is drawn whatever the
/// player's view and the Look Back key (**observed** under Wine, a Quick
/// Race start scene: in the bumper view the player's car is in every shot;
/// in the hood view, Look Back tapped through the scene, it never blinks).
/// The NIS leaves the player no camera mover (type 0, 0x430490), so no
/// chase update runs to change +0x280 there (**inferred** as the reason).
pub(crate) fn player_car_hidden(mode: &Mode, drive: &Drive, scene: Option<&ShopRun>) -> bool {
    native_mover_type(mode, Some(drive)) == 1
        && !scene.is_some_and(ShopRun::camera_holds_view)
        && drive.chase.hides_car()
}

/// The draw's test for the view's own car (view +0x280): its body (the
/// car list, 0x60D584) and its shadow and neon (0x6316E0's per-car block,
/// 0x63179C) are left out together; the photo mode's free camera always
/// shows the car.
pub(crate) fn view_leaves_out_player(mode: &Mode, drive: &Drive, scene: Option<&ShopRun>, photo_open: bool) -> bool {
    player_car_hidden(mode, drive, scene) && !photo_open
}

/// The type of the first player's camera mover (player +0x238 → +0x0C, the
/// type Camera_Factory 0x5F1FE0 built) for the views the clone has: the
/// showcase's 0x14 (0x501290), a replay's track camera 0x12 (0x5FB5B2),
/// else the chase camera 1. (The game's others: 0x0D / 0x0E the look-back
/// and nearby-object keys 0x7F / 0x80 (0x602BE0), 0x0F / 0x10 the jump
/// cameras of racer +0x58 = 6 / 7 (0x604750), 0x11 input mode 5
/// (0x605C80), 0x13 the target camera (0x5FABD0), 0x15 the dyno (0x600600);
/// type 0, e.g. an NIS (0x430490), leaves no mover.)
pub(crate) fn native_mover_type(mode: &Mode, drive: Option<&Drive>) -> u32 {
    match mode {
        // Free roam's crash / jump camera (its director's, `event_shot`).
        Mode::Explore if drive.is_some_and(|d| d.event_mover != 0) => drive.map_or(1, |d| d.event_mover),
        Mode::Menu(Menu::Showcase(_)) => 0x14,
        // The extra replay cameras, each its own mover (a switch is a
        // cut; Cinematic depth of field runs on them, `dof_scene`).
        Mode::Race(r) if r.replay.playback.as_ref().is_some_and(|p| p.camera.index >= 6) => {
            0x200 + r.replay.playback.as_ref().map_or(0, |p| p.camera.index as u32)
        }
        Mode::Race(r) if r.replay.playback.as_ref().is_some_and(|p| p.track.shot.is_some()) => 0x12,
        // The crash / jump camera (Camera_Factory 0x0F / 0x10).
        Mode::Race(r) if r.camera.shot.as_ref().is_some_and(|s| s.visible) && r.replay.playback.is_none() => {
            if r.camera.events.kind == 7 {
                0x10
            } else {
                0x0F
            }
        }
        _ => 1,
    }
}
