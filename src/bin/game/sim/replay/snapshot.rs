use crate::*;

/// What a race frame changes: the player's car and camera, every racer's
/// run, the rivals' AI (route, speed target, driver, unstuck, random
/// numbers), the shared collision state, the knock-over props and the
/// drift scoring.
#[derive(Clone)]
pub(crate) struct Snapshot {
    /// The race's world-update count (the racers' think schedule).
    pub(crate) ai_frames: u64,
    /// The world update counter (the DUMMY_SNAPSHOT's cadence).
    pub(crate) world_updates: u64,
    /// The world time (the snapshot's "WorldTime" chunk, 0x57BBB0: the
    /// tick 0x865188, the clock 0x7FB718 and the counters beside them):
    /// the cars' grip reads the rain through the clock (0x5ED5E0), so a
    /// replay runs it again from here.
    pub(crate) world_clock: f32,
    pub(crate) world_tick: nfsu2::ai::encounter_controller::WorldClock,
    /// The race's traffic as it was (its cars keep their models).
    pub(crate) traffic: Option<TrafficSnapshot>,
    pub(crate) player: CarState,
    pub(crate) chase: ChaseCamera,
    pub(crate) me: Progress,
    pub(crate) clock: f32,
    pub(crate) over: f32,
    pub(crate) wrong_way: bool,
    pub(crate) cars: nfsu2::drive::CarField,
    pub(crate) props: Props,
    pub(crate) style: Option<nfsu2::drift::StyleManager>,
    pub(crate) drift: Option<nfsu2::drift::DriftManager>,
    pub(crate) announce: Option<(u32, f32)>,
    pub(crate) drift_angle: f32,
    pub(crate) style_base: f32,
    pub(crate) misc: nfsu2::style::Misc,
    pub(crate) order: Vec<usize>,
    pub(crate) rivals: Vec<RivalSnapshot>,
    pub(crate) board: Vec<nfsu2::drift::BoardEntry>,
    pub(crate) dragger: Option<DragSteer>,
    pub(crate) autopilot: Option<Autopilot>,
    /// The human's line-speed mean and timer, and last frame's
    /// velocities (they feed the next frame's catch-up and road entries).
    pub(crate) human_line: nfsu2::ai::driver::RunningMean,
    pub(crate) human_line_timer: f32,
    pub(crate) road_prev: std::collections::HashMap<usize, Vec3>,
    /// The human controller's road (node and walk): the traffic placer
    /// starts from its node (0x42E9F0).
    pub(crate) human_road: nfsu2::ai::route::HumanRoad,
}

#[derive(Clone)]
pub(crate) struct RivalSnapshot {
    pub(crate) think: nfsu2::ai::encounter_controller::ThinkSchedule,
    pub(crate) retained: Option<nfsu2::ai::actions::Steer>,
    pub(crate) state: CarState,
    pub(crate) progress: Progress,
    pub(crate) route: Option<nfsu2::ai::route::RacerRoute>,
    pub(crate) speed: nfsu2::ai::route::SpeedTarget,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) controls: nfsu2::drive::AiControls,
    pub(crate) rng: nfsu2::opponents::GameRng,
    pub(crate) unstuck: nfsu2::ai::driver::Unstuck,
    pub(crate) actions: nfsu2::ai::actions::Actions,
    pub(crate) lane: nfsu2::ai::drag::DragLane,
    pub(crate) drift: Option<nfsu2::ai::driver::DriftDriver>,
    pub(crate) style: Option<nfsu2::drift::StyleManager>,
    pub(crate) scorer: Option<nfsu2::drift::DriftManager>,
    pub(crate) refill: (f32, f32),
    /// The controller's physics hold (+0x734): 5 s from the green.
    pub(crate) hold: i32,
    pub(crate) belt: BeltState,
}

/// A race's traffic in a replay snapshot: each car's state, lane, driver,
/// mover, last plan and stop (+0x710), and the placer's and intersections' state.
#[derive(Clone)]
pub(crate) struct TrafficSnapshot {
    pub(crate) cars: Vec<(
        CarState,
        Option<nfsu2::ai::traffic::TrafficLane>,
        nfsu2::ai::driver::AiDriver,
        nfsu2::drive::AiControls,
        Option<nfsu2::ai::traffic::TrafficMover>,
        Option<(Vec<Vec3>, f32)>,
        bool,
        BeltState,
        // The paused car's walk, +0x550, +0x77D and the race coordinate
        // (+0x8FC) with where it was taken.
        (Option<nfsu2::ai::traffic::TrafficLane>, bool, bool, Option<(f32, Vec2)>),
    )>,
    pub(crate) rng: nfsu2::opponents::GameRng,
    pub(crate) clock: f32,
    pub(crate) timer: f32,
    pub(crate) cursor: usize,
    /// 0x42E9F0's cursor over the walk's starts (placer +0x0C, 0x417D80).
    pub(crate) start: usize,
    pub(crate) crossings: nfsu2::ai::traffic::Crossings,
    pub(crate) updates: u64,
}

impl TrafficSnapshot {
    pub(crate) fn take(t: &Traffic) -> TrafficSnapshot {
        TrafficSnapshot {
            cars: t
                .cars
                .iter()
                .map(|c| {
                    (
                        c.state.clone(),
                        c.lane.clone(),
                        c.driver.clone(),
                        c.controls,
                        c.mover,
                        c.plan.clone(),
                        c.stopped,
                        c.belt.clone(),
                        (c.coast.clone(), c.active, c.held, c.coord),
                    )
                })
                .collect(),
            rng: t.field.rng.clone(),
            clock: t.clock,
            timer: t.timer,
            cursor: t.cursor,
            start: t.start,
            crossings: t.crossings.clone(),
            updates: t.updates,
        }
    }

    pub(crate) fn restore(&self, t: &mut Traffic) {
        for (c, s) in t.cars.iter_mut().zip(&self.cars) {
            (c.state, c.lane, c.driver, c.controls, c.mover, c.plan, c.stopped, c.belt) =
                (s.0.clone(), s.1.clone(), s.2.clone(), s.3, s.4, s.5.clone(), s.6, s.7.clone());
            (c.coast, c.active, c.held, c.coord) = s.8.clone();
        }
        t.field.rng = self.rng.clone();
        t.clock = self.clock;
        t.timer = self.timer;
        t.cursor = self.cursor;
        t.start = self.start;
        t.crossings = self.crossings.clone();
        t.updates = self.updates;
    }
}

impl Snapshot {
    pub(crate) fn take(d: &Drive, race: &Race) -> Snapshot {
        Snapshot {
            ai_frames: race.racers.ai_frames,
            world_updates: race.replay.world_updates,
            world_clock: d.clock,
            world_tick: d.world_clock.clone(),
            traffic: race.racers.traffic.as_ref().map(TrafficSnapshot::take),
            player: d.state.clone(),
            chase: d.chase.clone(),
            me: race.racers.me.clone(),
            clock: race.timing.clock,
            over: race.over,
            wrong_way: race.wrong_way,
            cars: race.racers.cars.clone(),
            props: d.props.clone(),
            style: race.scoring.style.clone(),
            drift: race.scoring.drift.clone(),
            announce: race.scoring.announce,
            drift_angle: race.scoring.drift_angle,
            style_base: race.scoring.style_base,
            misc: race.scoring.misc.clone(),
            order: race.order.clone(),
            dragger: race.dragger.clone(),
            autopilot: race.autopilot.clone(),
            rivals: race
                .racers.rivals
                .iter()
                .map(|r| RivalSnapshot {
                    think: r.think.clone(),
                    retained: r.retained.clone(),
                    state: r.state.clone(),
                    progress: r.progress.clone(),
                    route: r.route.clone(),
                    speed: r.speed.clone(),
                    driver: r.driver.clone(),
                    controls: r.controls,
                    rng: r.rng.clone(),
                    unstuck: r.unstuck.clone(),
                    actions: r.actions.clone(),
                    lane: r.lane.clone(),
                    drift: r.drift.clone(),
                    style: r.style.clone(),
                    scorer: r.scorer.clone(),
                    refill: r.refill,
                    hold: r.hold,
                    belt: r.belt.clone(),
                })
                .collect(),
            board: race.scoring.board.clone(),
            human_line: race.human_line.clone(),
            human_line_timer: race.human_line_timer,
            road_prev: race.replay.road_prev.clone(),
            human_road: d.human_road.clone(),
        }
    }

    pub(crate) fn restore(&self, d: &mut Drive, race: &mut Race) {
        if let (Some(t), Some(s)) = (race.racers.traffic.as_mut(), self.traffic.as_ref()) {
            s.restore(t);
        }
        d.state = self.player.clone();
        d.clock = self.world_clock;
        d.world_clock = self.world_tick.clone();
        nfsu2::weather::set_clock(&d.world, d.clock);
        race.racers.ai_frames = self.ai_frames;
        race.replay.world_updates = self.world_updates;
        d.chase = self.chase.clone();
        d.chase_view(0.0);
        race.racers.me = self.me.clone();
        race.timing.clock = self.clock;
        race.over = self.over;
        race.wrong_way = self.wrong_way;
        race.racers.cars = self.cars.clone();
        // The props as they were, and every one drawn so (a prop knocked
        // later goes back to its rest pose).
        d.props = self.props.clone();
        let posed = std::mem::take(&mut d.posed);
        d.posed = d
            .props
            .poses(&d.world)
            .into_iter()
            .filter(|&(p, _, woken)| woken || posed.contains_key(&p))
            .map(|(p, t, _)| (p, t))
            .collect();
        race.scoring.style = self.style.clone();
        race.scoring.drift = self.drift.clone();
        race.scoring.announce = self.announce;
        race.scoring.drift_angle = self.drift_angle;
        race.scoring.style_base = self.style_base;
        race.scoring.misc = self.misc.clone();
        race.order = self.order.clone();
        for (r, s) in race.racers.rivals.iter_mut().zip(&self.rivals) {
            r.think = s.think.clone();
            r.retained = s.retained.clone();
            r.state = s.state.clone();
            r.progress = s.progress.clone();
            r.route = s.route.clone();
            r.speed = s.speed.clone();
            r.driver = s.driver.clone();
            r.controls = s.controls;
            r.rng = s.rng.clone();
            r.unstuck = s.unstuck.clone();
            r.actions = s.actions.clone();
            r.lane = s.lane.clone();
            r.drift = s.drift.clone();
            r.style = s.style.clone();
            r.scorer = s.scorer.clone();
            r.refill = s.refill;
            r.hold = s.hold;
            r.belt = s.belt.clone();
        }
        race.scoring.board = self.board.clone();
        race.human_line = self.human_line.clone();
        race.human_line_timer = self.human_line_timer;
        race.replay.road_prev = self.road_prev.clone();
        d.human_road = self.human_road.clone();
        race.dragger = self.dragger.clone();
        race.autopilot = self.autopilot.clone();
    }
}
