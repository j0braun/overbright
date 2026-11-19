use crate::*;

/// The player's impact shake (`nfsu2::chase::impact`) on the tick.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Impact {
    /// Slot 0 (0x865360): triggered after the player's world update
    /// (`Drive::impact_trigger`), stepped once a world frame after the
    /// cameras (`Drive::tick_clock`, as 0x5728D0 at 0x5814C2).
    pub(crate) shake: nfsu2::chase::impact::ImpactShake,
    /// The world's time scale (+0x108, the slow motion's rate) as the
    /// cameras read it; set by the sessions from their event cameras.
    pub(crate) time_scale: f32,
    /// The player's event kind (+0x58) is 7 (the jump camera's second
    /// shot), as the trigger reads it.
    pub(crate) kind7: bool,
    /// The offset this tick's chase view applied: the rear-view mirror's
    /// (0x44BE7B, view 3 on the same slot).
    pub(crate) applied: Vec3,
}

impl Default for Impact {
    fn default() -> Self {
        Impact { shake: Default::default(), time_scale: 1.0, kind7: false, applied: Vec3::ZERO }
    }
}

impl Impact {
    /// The event cameras' state as the shake reads it: the slow motion's
    /// rate and whether kind 7 runs.
    pub(crate) fn follow(&mut self, events: &nfsu2::event_camera::EventCameras) {
        self.time_scale = events.slow.rate;
        self.kind7 = events.kind == 7;
    }
}

impl Drive {
    /// 0x572E60 for the player's car (World_UpdateControllers' loop at
    /// 0x60A154, after the car's 0x604750 wrote car +0x80): a hard enough
    /// acceleration starts a shake.
    pub(crate) fn impact_trigger(&mut self) {
        self.impact.shake.trigger(self.state.world_acceleration, self.impact.kind7);
    }

    /// This frame's chase view: the eased camera (`ChaseCamera::update`),
    /// then for the chase kinds 0, 1 and 5 the world push
    /// (`nfsu2::chase::keep_out`, 0x453570 → 0x444530) against the
    /// polygons gathered around the car-to-eye segment (0x5E6FB0: its
    /// midpoint, half-size (length + 1) / 2 + 0.5) with the car's barrier
    /// groups (0x444530 copies car +0x8B4 / +0x8B8). The centre is the
    /// body's: car +0x50 (the car's position, the centre of mass less the
    /// record's +0x110..+0x118, `CarState::placed`) plus the turned middle
    /// of the body's box relative to the centre of mass (body +0x110,
    /// 0x4445A5; `chase::body_box`), not the bare position: that one sits at about the road's height on a car
    /// with a low centre of mass (the Corolla: wheel z 0.275, radius
    /// 0.265), where the road itself then crosses the segment at the
    /// centre and the sweep's pull-in (crossing − 0.7, negative) put the
    /// eye 0.7 m ahead of the car, under the road, looking back up at it.
    ///
    /// The shake (0x452ED0) runs on the sim clock 0x865188 (the world
    /// clock's tick, `ChaseCamera::set_clock`) and burns harder while the
    /// car's nitrous flame is on (0x59FA40 on car +0x40 set, 0x5ECDC0 zero:
    /// `carfx::purging` false; `ChaseCamera::set_nitrous_flame`). Both are
    /// read per world update, as the camera itself is stepped (drawn frames
    /// between updates blend the camera, docs/original/uncapped-fps.md).
    pub(crate) fn chase_view(&mut self, dt: f32) {
        self.chase_view_lifted(dt, &[]);
    }

    /// [`Drive::chase_view`], then for the chase kinds the lift over the
    /// traffic the eye sits in (0x453570 → 0x44B040 at 0x453BDD with flag
    /// 1, after the world push; `chase::lift_over_traffic`): `traffic` the
    /// world list's traffic cars that are drawn ([`lift_car`]). The flow
    /// state is never 6 on the paths that hand traffic (free roam and the
    /// race: 4, or 7 after the finish), so the moving-car test runs.
    pub(crate) fn chase_view_lifted(&mut self, dt: f32, traffic: &[nfsu2::chase::LiftCar]) {
        self.chase.set_race(self.car.car_type.handling.race);
        self.chase.set_clock(self.world_clock.tick);
        let stage = self.state.drivetrain.as_ref().map_or(0, |t| t.nitrous);
        self.chase.set_nitrous_flame(
            stage != 0 && !nfsu2::audio::carfx::purging(stage, self.state.speed(), self.state.gas()),
        );
        // The impact shake as this update's views read it (0x572A60: only
        // at a time scale of 1 or more); the mirror takes the same.
        self.impact.applied = if self.impact.time_scale >= 1.0 { self.impact.shake.offset() } else { Vec3::ZERO };
        self.chase.set_impact(self.impact.applied);
        self.camera = self.chase.update(&self.state, dt, self.fov_offset);
        if matches!(self.chase.kind(), 0 | 1 | 5) {
            self.push_camera();
            if !traffic.is_empty() {
                let mut camera = self.camera.clone();
                nfsu2::chase::lift_over_traffic(&mut camera, self.state.position, self.state.velocity, traffic, false);
                self.camera = camera;
            }
        }
    }

    /// The world push (0x444530) of this frame's camera from the car.
    pub(crate) fn push_camera(&mut self) {
        let com = Vec3::from(self.car.car_type.handling.centre_of_mass);
        // Without a hull (no body) the centre is the car's +0x50, 0x4445DE.
        let (middle, lift) =
            nfsu2::chase::body_box(&self.car.hull, com).map_or((Vec3::ZERO, None), |(m, r)| (m, Some(r)));
        let centre = self.state.position + self.state.orientation * middle;
        let mut camera = self.camera.clone();
        let polygons = self.polygons_around(centre, camera.position);
        nfsu2::chase::keep_out_lifted(&mut camera, centre, &polygons, self.chase.wall_radius(), lift);
        self.camera = camera;
    }

    /// The world polygons 0x5E6FB0 gathers around a segment (its midpoint,
    /// half-size (length + 1) / 2 + 0.5), with the car's barrier groups.
    pub(crate) fn polygons_around(&self, a: Vec3, b: Vec3) -> Vec<nfsu2::ground::TrackPolygon> {
        let mid = (a + b) * 0.5;
        let r = ((b - a).length() + 1.0) * 0.5 + 0.5;
        let mut q = nfsu2::ground::GroundQuery::default();
        q.barriers = self.state.ground.barriers.clone();
        self.world.ground.nearby(&self.world.visibility, &mut q, mid.truncate() - r, mid.truncate() + r, Vec2::ZERO)
    }

    /// 0x444530's push of a camera from a point (`chase::keep_out_with`);
    /// whether the sweep hit.
    pub(crate) fn push_from(&self, camera: &mut Camera, centre: Vec3, wall: f32) -> bool {
        let polygons = self.polygons_around(centre, camera.position);
        nfsu2::chase::keep_out_with(camera, centre, &polygons, wall)
    }

    /// The sight test 0x5E7EE0 (**verified** in the disassembly): under 30
    /// m one piece, else the segment cut into pieces (0x75BED4 of the
    // prov: inferred game.sight_blocked
    /// length over 30, **inferred**: the rounding) from `from` (the car)
    /// on, each with the polygons gathered around it (0x5E6FB0) and
    /// 0x5D3930's crossing test (`chase::crosses`, the box grown by 0.5);
    /// any crossing blocks. One cache serves every piece: 0x5E68E0 keeps
    /// its section's pack while the piece's middle stays within the
    /// section's margin of the point it was chosen at, so a wall of the
    /// next section at the camera's place is not seen (the clone once
    /// chose a pack per piece and was blocked by such walls where the
    /// original's crash camera ran, Wine logs b1 and j2).
    pub(crate) fn sight_blocked(&self, from: Vec3, to: Vec3) -> bool {
        let len = (to - from).length();
        let n = if len < 30.0 { 1 } else { (len / 30.0) as usize + 1 };
        // The factory's own cache (0x5E0B50 at 0x5F2945), with no barrier
        // groups of its own (+0x2F0 = 0): the race's barrier walls, which
        // only the racers' caches switch on for their gathers, are not
        // seen (the walls that blocked the clone where the
        // original's crash camera ran, Wine logs b1 / j2, were such).
        let mut q = nfsu2::ground::GroundQuery::default();
        // The factory's cache first answers the height at the camera's
        // place (0x5F2961: 0x5E7E80 → 0x5E72F0 on the same object), so its
        // section and pack are the place's when the pieces start.
        let _ = self.world.ground.query(&self.world.visibility, &mut q, to);
        (0..n).any(|i| {
            let (a, b) = (from.lerp(to, i as f32 / n as f32), from.lerp(to, (i + 1) as f32 / n as f32));
            let mid = (a + b) * 0.5;
            let r = (b - a).length() * 0.5 + 0.5;
            let polys = self.world.ground.nearby(
                &self.world.visibility,
                &mut q,
                mid.truncate() - r,
                mid.truncate() + r,
                Vec2::ZERO,
            );
            let hit = nfsu2::chase::crosses(a, b, &polys, 0.5);
            if debug_env().sight_debug {
                eprintln!("sight: piece {i}/{n} mid {mid:?} pack {:?} polys {}", q.pack(), polys.len());
            }
            if hit && debug_env().sight_debug {
                for p in &polys {
                    if nfsu2::chase::crosses(a, b, std::slice::from_ref(p), 0.5) {
                        eprintln!(
                            "sight: piece {i}/{n} blocked by {:?} flags {:#x} group {} normal {:?}",
                            p.id, p.flags, p.turn, p.normal
                        );
                    }
                }
            }
            hit
        })
    }
}

/// A car as the chase lift (0x44AC10) and the track cameras' car test
/// (0x5F1500) read it: its origin (car +0x50, `CarState::position`),
/// velocity (+0x70), rows (+0x90) and its body's box about the centre of
/// mass (`chase::LiftCar::new` from the hull and the record's centre of
/// mass); None without a hull.
pub(crate) fn lift_car(car: &Car, state: &CarState) -> Option<nfsu2::chase::LiftCar> {
    nfsu2::chase::LiftCar::new(
        state.position,
        state.velocity,
        glam::Mat3::from_quat(state.orientation),
        &car.hull,
        Vec3::from(car.car_type.handling.centre_of_mass),
    )
}

/// The drawn traffic cars (placed in a lane, as the frame draws them) for
/// the chase lift.
pub(crate) fn traffic_lift_cars(traffic: Option<&Traffic>) -> Vec<nfsu2::chase::LiftCar> {
    traffic
        .into_iter()
        .flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()))
        .filter_map(|c| lift_car(&c.car, &c.state))
        .collect()
}
