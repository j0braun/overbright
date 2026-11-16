use crate::*;

/// The player's DragCarController (0x4299C0, controller type 2 while the
/// player's input mode +0x22C is 3, 0x602A68): an AI racer controller with
/// its route, the DragDriver (0x427F20: the AI driver 0x421030 for the
/// steering, the player's pedals over its own, 0x4220A0) and the lane
/// change state.
#[derive(Clone)]
pub(crate) struct DragSteer {
    pub(crate) route: Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) lane: nfsu2::ai::drag::DragLane,
    pub(crate) accel: std::rc::Rc<dyn Fn(f32) -> f32>,
}

impl DragSteer {
    pub(crate) fn new(car: &Car, state: &CarState, routes: &nfsu2::ai::route::AiRoutes) -> DragSteer {
        let h = &car.car_type.handling;
        let heading = state.heading();
        DragSteer {
            // The player's car (number 0): no start hold (0x5EF000); the
            // route's clock is never set here, so a 12 s hold blocked every
            // branch for good.
            route: nfsu2::ai::route::RacerRoute::start(routes, state.position, Vec2::new(heading.cos(), heading.sin()))
                .map(|mut r| {
                    r.branch_hold = nfsu2::ai::route::start_hold(0);
                    r
                }),
            driver: nfsu2::ai::driver::AiDriver::new(h, car.car_type.suv, 1.0),
            lane: Default::default(),
            accel: std::rc::Rc::new(nfsu2::handling::acceleration(h)),
        }
    }

    /// One frame: the lane requests from the steering input (inferred
    /// binding of the lane events 0xB8–0xBB), the route, the lane path
    /// (0x42DF10) and the AI driver's front-wheel angle on it, degrees.
    pub(crate) fn steer(
        &mut self,
        car: &Car,
        state: &CarState,
        routes: &nfsu2::ai::route::AiRoutes,
        player: Controls,
        clock: f32,
        dt: f32,
    ) -> Option<f32> {
        use nfsu2::ai::driver::DriverInputs;
        let h = &car.car_type.handling;
        self.lane.input(player.steer < -0.5, player.steer > 0.5);
        let route = self.route.as_mut()?;
        route.update(
            routes,
            state.position,
            state.speed(),
            nfsu2::ai::route::travel(state.speed(), state.velocity, state.forward()),
            h.performance_class,
            car.car_type.suv,
            &*self.accel,
        );
        let path = self.lane.player_path(route, routes, state.position, state.velocity.length());
        // `NFSU2_DRAG_LANE_LOG=1` (a harness knob, not the game's, the
        // parity harness's lane plans): the player's lane, the lanes its
        // point's record has and where each ends along the list, per frame.
        if debug_env().drag_lane_log.is_some() {
            let has: Vec<usize> = (2..=5).filter(|&l| route.has_lane(routes, l)).collect();
            let ends: Vec<i32> = (2..=5).map(|l| route.lane_end(routes, l) as i32).collect();
            let side: Vec<String> =
                has.iter().map(|&l| format!("{l}:{:.2}", route.lane_lateral(routes, state.position, l))).collect();
            let f = state.forward();
            eprintln!(
                "LANE t {clock:.3} d {:.1} coord {:.1} pos {:.1} {:.1} fwd {:.3} {:.3} steer {} lane {:?} has {has:?} ends {ends:?} lat {}",
                route.distance(),
                route.race_coord.unwrap_or(f32::NAN),
                state.position.x,
                state.position.y,
                f.x,
                f.y,
                player.steer,
                self.lane,
                side.join(",")
            );
        }
        let target = route.speed_at(route.distance());
        let input = DriverInputs {
            path: &path,
            target_speed: target,
            enabled: true,
            green: clock >= 0.0,
            countdown: clock < 0.0,
            since_green: clock,
            clock: f64::from(clock),
        };
        Some(self.driver.update(state, h, car.car_type.suv, &input, dt).steer_degrees)
    }
}
