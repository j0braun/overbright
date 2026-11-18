use crate::*;

/// The player's car past the finish line (0x606CC0, verified): for the
/// local player (racer car +0x18 set) the finish puts its input in state
/// 0 (0x605C80(0): every driving key handler removed; state 9 online or
/// split-screen) and gives it controller type 3 (0x5EBA40(3) → 0x4260B0,
/// the AI racer controller, with the CarDriver 0x425AB0 — 0x425E40 when
/// 0x890114, 0x89E7D9 or 0x890104 is set and 0x89E7D8 / 0x890108 are
/// not), whose goal is then "Park" (0x42B330): it brakes to a stand on
/// the line as the finished rivals do (`racer_think`'s `parked`). Its
/// skill row is the human's −2 ([`SpeedTarget::human`]); the driver's
/// throttle cap 1 (0x425B44: the controller's +0x14 object is class 1,
/// not 2).
#[derive(Clone)]
pub(crate) struct Autopilot {
    pub(crate) route: Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) speed: nfsu2::ai::route::SpeedTarget,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) controls: nfsu2::drive::AiControls,
    /// The car's random numbers (car +0xF8): unused, the human row draws
    /// no jitter.
    pub(crate) rng: nfsu2::opponents::GameRng,
    pub(crate) unstuck: nfsu2::ai::driver::Unstuck,
    pub(crate) lane: nfsu2::ai::drag::DragLane,
    /// In a drift race the controller's driver is the DriftDriver
    /// (0x4260B0 at 0x42615A–0x4261B3: 0x425E40 when 0x890114, 0x89E7D9 or
    /// 0x890104 is set and neither 0x89E7D8 nor 0x890108).
    pub(crate) drift: Option<nfsu2::ai::driver::DriftDriver>,
    /// Driving the whole race, from the grid (the map editor's simulation,
    /// `enh::sim_stream`; never in play): the racer is not finished until
    /// it is.
    pub(crate) racing: bool,
}

impl Autopilot {
    pub(crate) fn new(d: &Drive, routes: &nfsu2::ai::route::AiRoutes, drift: bool) -> Autopilot {
        let h = &d.car.car_type.handling;
        let f = d.state.forward();
        Autopilot {
            // The player's car (number 0): no start hold (0x5EF000).
            route: nfsu2::ai::route::RacerRoute::start(
                routes,
                d.state.position,
                Vec2::new(f.x, f.y).normalize_or(Vec2::X),
            )
            .map(|mut r| {
                r.branch_hold = nfsu2::ai::route::start_hold(0);
                r
            }),
            speed: nfsu2::ai::route::SpeedTarget::human(),
            driver: nfsu2::ai::driver::AiDriver::new(h, d.car.car_type.suv, 1.0),
            controls: Default::default(),
            rng: nfsu2::opponents::GameRng(0),
            unstuck: Default::default(),
            lane: Default::default(),
            drift: drift.then(Default::default),
            racing: false,
        }
    }
}
