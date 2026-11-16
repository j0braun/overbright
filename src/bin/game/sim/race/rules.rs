use crate::*;

/// The wrong-way test (0x600720, run each frame with the last answer):
/// the heading at least 110° off the road's direction (10° less once
/// wrong); above 10 mph the direction of travel must be too, and a new
/// wrong way needs all four wheels down (car +0x424); at 10 mph or less
/// the last answer stands. `road` is the road locator's driving direction
/// (car +0x900, see [`road_heading`]).
pub(crate) fn wrong_way(was: bool, heading: f32, road: Vec2, velocity: Vec2, wheels_down: usize) -> bool {
    let off = |dir: Vec2| {
        let a = road.y.atan2(road.x) - dir.y.atan2(dir.x);
        a.sin().atan2(a.cos()).abs().to_degrees()
    };
    let slack = if was { 10.0 } else { 0.0 };
    if off(Vec2::new(heading.cos(), heading.sin())) + slack < 110.0 {
        return false;
    }
    if velocity.length() * 2.237 <= 10.0 {
        return was;
    }
    if off(velocity) + slack < 110.0 {
        return false;
    }
    was || wheels_down >= 4
}

impl Race {
    /// Run the race clock and the player's progress (the car at `at`);
    /// the clock stops once every racer has finished.
    pub(crate) fn update(&mut self, at: Vec2, dt: f32) {
        if self.racers.me.finished.is_some() && self.racers.rivals.iter().all(|r| r.progress.finished.is_some()) {
            return;
        }
        self.timing.clock += dt;
        self.racers.me.update(&self.route, self.laps, self.timing.clock, at);
        let standings = self.standings();
        nfsu2::standings::rank(&mut self.order, &standings);
        if self.scoring.knockout {
            self.knock_out();
        }
        self.time_out();
    }

    /// The race status's clock +0x14 after this update, `world` the world
    /// clock (0x865188 as seconds, `Drive::clock`). 0x6076E0 (
    /// **verified** + **measured**): at the green 0x5FC910(4) stamps +0x10
    /// = the tick rounded half up to 0.01 s; while racing (state 4) +0x14 =
    /// tick − +0x10, rounded the same way (0x577410); 0 through the scene
    /// and the countdown; not updated once the race clock stops (every
    // prov: inferred game.race_status_clock_end
    /// racer finished: the clone's end of state 4, **inferred**).
    pub(crate) fn status_clock_update(&mut self, world: f32) {
        if self.timing.clock < 0.0 {
            return;
        }
        let tick = (world as f64 * nfsu2::standings::TICKS as f64).round() as i32;
        let green = *self.timing.green_tick.get_or_insert_with(|| nfsu2::standings::round_centi(tick));
        if self.racers.me.finished.is_some() && self.racers.rivals.iter().all(|r| r.progress.finished.is_some()) {
            return;
        }
        self.timing.status_clock = nfsu2::standings::round_centi(tick - green);
    }

    /// The race status (0x890118) as the NIS sound object reads it
    /// (0x475330): its state +0 (2 the start scene's camera to its cut,
    /// 3 the countdown from the cut, 0x5FC910(3), 4 racing) and its clock
    /// +0x14 in seconds (× 0.00025, 0x4753E5).
    pub(crate) fn status(&self, scene_before_cut: bool) -> (i32, f32) {
        let state = if self.timing.clock >= 0.0 {
            4
        } else if scene_before_cut {
            2
        } else {
            3
        };
        (state, self.timing.status_clock as f32 * 0.00025)
    }

    /// A drift race's time-out. Arming (0x606CC0 at 0x606FB4, drift race
    /// 0x89E7D9 / 0x890104, no split screen): the racer finishing (status
    /// 1) in place 1 (+0x0C, the first to finish) with race status +0x1C
    /// unset (0x401820) arms it when it is an AI racer (controller type 2;
    /// a human only with two humans, 0x8900BC = 2): +0x1C = its finish
    /// tick (+0x198 + the start +0x10), +0x20 = 30 s. So a player who
    /// finishes first leaves no time-out. Then (Race_UpdateState 0x6076E0
    /// at 0x607BAD) once +0x20 > 0 and the time since +0x1C reaches it,
    /// every racer still running (+0x10 = 0) is ended with status 11
    /// "Timeup" in the next free place (0x606CC0(i, place, 0, 11)).
    // prov: inferred game.time_out
    /// **Inferred**: two finishes in one frame are taken by their times,
    /// the player first on a tie.
    pub(crate) fn time_out(&mut self) {
        if self.scoring.drift.is_none() {
            return;
        }
        if !self.timing.first_done {
            let finish = |p: &Progress| p.finished.filter(|_| p.dropped.is_none() && p.ended.is_none());
            let mine = finish(&self.racers.me);
            let theirs = self.racers.rivals.iter().filter_map(|r| finish(&r.progress)).min_by(f32::total_cmp);
            match (mine, theirs) {
                (Some(m), Some(t)) if t < m => self.timing.hurry = Some(t),
                (None, Some(t)) => self.timing.hurry = Some(t),
                (None, None) => return,
                _ => {}
            }
            self.timing.first_done = true;
        }
        let Some(armed) = self.timing.hurry else { return };
        if self.timing.clock - armed < HURRY_TIME {
            return;
        }
        let clock = self.timing.clock;
        for p in std::iter::once(&mut self.racers.me).chain(self.racers.rivals.iter_mut().map(|r| &mut r.progress)) {
            if p.finished.is_none() {
                p.finished = Some(clock);
                p.dropped = Some(11);
            }
        }
    }

    /// The hurry counter (0x4EBB10's drift branch, race status +0x20 ≥
    /// 0): the time left, +0x20 − (now − +0x1C) / 4000, s.
    pub(crate) fn hurry_left(&self) -> Option<f32> {
        self.timing.hurry.map(|armed| HURRY_TIME - (self.timing.clock - armed))
    }

    /// Lap KO (race type 3, 0x6079BF): the frame a racer completes a lap
    /// (+0x0D) with its place (+0x0A) plus its laps done (+0x09) equal to
    /// the racer count, every racer behind it in the order (+0xBD0) still
    /// running is knocked out (status 3, "Knocked Out") in its place.
    pub(crate) fn knock_out(&mut self) {
        let laps: Vec<i32> =
            std::iter::once(&self.racers.me).chain(self.racers.rivals.iter().map(|r| &r.progress)).map(|p| p.lap().1).collect();
        let count = laps.len();
        if self.scoring.ko_seen.len() != count {
            self.scoring.ko_seen = laps.clone();
        }
        let mut out = Vec::new();
        for (place, &i) in self.order.iter().enumerate() {
            if laps[i] > self.scoring.ko_seen[i] && place as i32 + 1 + laps[i] == count as i32 {
                out.extend(self.order[place + 1..].iter().copied());
            }
        }
        self.scoring.ko_seen = laps;
        let clock = self.timing.clock;
        for i in out {
            let p = if i == 0 { &mut self.racers.me } else { &mut self.racers.rivals[i - 1].progress };
            if p.finished.is_none() {
                p.finished = Some(clock);
                p.dropped = Some(3);
            }
        }
    }

    /// The race's end (0x60AAB0 → 0x608CB0) for the racers still running
    /// (racer status +0x10 clear), each in its place in the race order
    /// (+0xBD0): 0x606CC0(i, 0, 0, 1) finishes it (status 1, finish order
    /// next, 0x5F7ED0's estimate, `Progress::end_short`). In Lap KO (race
    /// type 3, with race status +0xBEC clear — **not traced** what sets
    /// it) the order is walked from the last place: the leader finishes,
    /// every other racer still running is knocked out (status 3) in its
    /// place.
    pub(crate) fn end_unfinished(&mut self) {
        let laps = race_laps(&self.route, self.laps);
        let clock = self.timing.clock;
        let order = self.order.clone();
        // The leader for 0x5F7ED0: order [0], finished for real.
        let leader = order
            .first()
            .map(|&i| if i == 0 { &self.racers.me } else { &self.racers.rivals[i - 1].progress })
            .filter(|p| p.finished.is_some() && p.dropped.is_none() && p.ended.is_none())
            .map(|p| p.lap().1);
        for (place, &i) in order.iter().enumerate() {
            let reference = &self.reference;
            let p = if i == 0 { &mut self.racers.me } else { &mut self.racers.rivals[i - 1].progress };
            if p.finished.is_some() {
                continue;
            }
            if self.scoring.knockout && place > 0 {
                p.finished = Some(clock);
                p.dropped = Some(3);
                continue;
            }
            let lead = if place == 0 { None } else { leader };
            p.end_short(laps, clock, lead, reference);
            p.ended = Some(place);
        }
    }

    /// Every racer's standing, the player first.
    pub(crate) fn standings(&self) -> Vec<nfsu2::standings::Standing> {
        std::iter::once(&self.racers.me).chain(self.racers.rivals.iter().map(|r| &r.progress)).map(|p| self.standing(p)).collect()
    }

    /// A drift race's scoring for the frame (0x5F7A10 → 0x56ED50, then the
    /// style manager's update 0x56FD20); the finish banks a running drift
    /// (0x56D720 once the racer's laps reach the race's).
    pub(crate) fn drift_frame(&mut self, state: &nfsu2::drive::CarState, car: &nfsu2::car::Car, dt: f32) {
        let closeness = self.traffic_closeness(state.position);
        let (Some(style), Some(drift)) = (self.scoring.style.as_mut(), self.scoring.drift.as_mut()) else { return };
        // The race done (inferred: progress in laps, +0x20, over the
        // race's laps).
        let progress = (self.racers.me.progress() / race_laps(&self.route, self.laps) as f32).clamp(0.0, 1.0);
        let input = nfsu2::drift::DriftInput {
            speed: state.velocity.length(),
            forward_speed: state.speed(),
            slip: state.slip_degrees().to_radians(),
            yaw_rate: state.spin.z,
            progress,
            touching: state.since_collision <= 0.0,
            off_road_wheels: state.wheels.iter().filter(|w| w.surface != 1).count() as u32,
            top_speed: car.car_type.handling.computed_top_speed,
            wrong_way: self.wrong_way,
            hit: state.last_hit,
            human: true,
            closeness,
        };
        // 0x4AB390: |slip|, at most 90°, none in reverse (car +0x434 = 0)
        // or rolling backwards.
        let reverse = state.drivetrain.as_ref().is_some_and(|t| t.gear == 0);
        self.scoring.drift_angle =
            if reverse || input.forward_speed < 0.0 { 0.0 } else { state.slip_degrees().abs().min(90.0) };
        if let Some(name) = drift.update(style, &input, self.timing.clock) {
            self.scoring.announce = Some((name, self.timing.clock));
        }
        style.update(dt, self.timing.clock);
        if self.racers.me.finished.is_some() {
            style.bank_running(self.timing.clock);
        }
    }

    /// The player's style points for the frame (0x6016B0): the detectors
    /// (0x56FF40, [`nfsu2::style::detect`]), the style manager's update
    /// (0x56FD20; in a drift race `drift_frame` runs it) and, with the
    /// Race Breaker flag, the nitrous refill (0x601737).
    pub(crate) fn style_frame(
        &mut self,
        d: &mut Drive,
        shortcut: Option<usize>,
        lap_info: bool,
        tank_before: Option<bool>,
        dt: f32,
    ) {
        let mut others: Vec<(Vec3, Vec3)> = self.racers.rivals.iter().map(|r| (r.state.position, r.state.velocity)).collect();
        if let Some(t) = self.racers.traffic.as_ref() {
            others.extend(t.cars.iter().filter(|c| c.lane.is_some()).map(|c| (c.state.position, c.state.velocity)));
        }
        let laps = self.racers.me.laps.as_ref();
        let detect = nfsu2::style::Detect {
            // Race status 4 from the green light (**inferred**: the
            // clone's clock at or past 0).
            racing: self.timing.clock >= 0.0,
            race: true,
            // The race time in whole game ticks (0x56F730 reads the race
            // status +0x14 × 0.00025): HEAD START's "more than 1 s" is not met
            // at exactly 4000 ticks (observed: the stage-4 drag's refill
            // at frame 61 natively; the clone's summed f32 clock read
            // 1.0000001 at frame 60 and refilled a frame early).
            race_time: whole_ticks(self.timing.clock),
            racers: 1 + self.racers.rivals.len() as u32,
            place: self.order.iter().position(|&i| i == 0).map_or(1, |p| p as u32 + 1),
            done: self.racers.me.finished.is_some(),
            new_section: laps.is_some_and(|l| l.new_best),
            new_lap: laps.is_some_and(|l| l.new_lap),
            shortcut,
            // The lap element runs with its mask on (racing, lap info
            // on); on the finishing frame the HUD has gone to the race-over
            // features (0x5FFF60), so no BEST LAP then (**inferred**
            // order: the status is set before the HUD's update).
            lap_done: (lap_info && laps.is_some_and(|l| l.new_lap) && self.racers.me.finished.is_none())
                .then(|| self.racers.me.last_lap.map(|l| l.0).zip(self.racers.me.best_lap))
                .flatten(),
            ..style_detect(d, &others, tank_before, dt)
        };
        let Some(style) = self.scoring.style.as_mut() else { return };
        nfsu2::style::detect(&mut self.scoring.misc, style, &detect, self.timing.clock);
        if self.scoring.drift.is_none() {
            style.update(dt, self.timing.clock);
        }
        if self.scoring.race_breaker {
            nfsu2::style::refill(style, &mut self.scoring.style_base, d.state.nitrous.as_mut());
        }
    }

    /// The AI racers' timed refill (0x6016B0 at 0x601795, Race Breaker on,
    /// for an owner-kind-2 racer still racing without a style manager):
    /// more than 1 s (0x7A56A4) after the last (the green light, 0x5FC9FD),
    /// the tank takes max(0, Δp) × 0.01 × 0.5 (0x7A56A8; 0 in a drag race,
    /// 0x4018F0 on 0x89E7D8 / 0x890108), Δp = the road percent (car +0x8F8 ×
    /// 100) less the last one, wrapped by ±100 past ±50 on a looping road
    /// (0x5D5D20): half a tank for each lap of the road.
    pub(crate) fn ai_refill(&mut self) {
        if !self.scoring.race_breaker || self.timing.clock < 0.0 {
            return;
        }
        let (clock, loops, drag) = (self.timing.clock, self.route.road.loops, self.drag);
        for r in self.racers.rivals.iter_mut().filter(|r| r.style.is_none() && r.progress.finished.is_none()) {
            if clock - r.refill.0 <= 1.0 {
                continue;
            }
            let p = r.progress.locator.fraction * 100.0;
            let mut delta = p - r.refill.1;
            if loops {
                if delta < -50.0 {
                    delta += 100.0;
                } else if delta > 50.0 {
                    delta -= 100.0;
                }
            }
            let f = if drag { 0.0 } else { 0.5 };
            if let Some(t) = r.state.nitrous.as_mut() {
                t.refill(delta.max(0.0) * 0.01 * f);
            }
            r.refill = (clock, p);
        }
    }

    /// The AI racers' drift scoring for the frame: 0x6016B0 runs 0x5F7A10
    /// for every racer still racing (status +0x10 = 0, 0x607772), owner
    /// kind 2 included (0x802228 = 1), so each rival's drift manager feeds
    /// its own style manager as the player's does; the finish (0x606CC0 →
    /// 0x5F7A10 → 0x56D720) banks a running drift, and a done racer is no
    /// longer updated. The AI has no wrong-way input (0x56DEF0 reads racer
    /// +0x248 for the human only) and hears no tiers.
    pub(crate) fn drift_rivals(&mut self, dt: f32) {
        let laps = race_laps(&self.route, self.laps) as f32;
        let clock = self.timing.clock;
        let near: Vec<f32> = self.racers.rivals.iter().map(|r| self.traffic_closeness(r.state.position)).collect();
        for (r, closeness) in self.racers.rivals.iter_mut().zip(near) {
            let (Some(style), Some(scorer)) = (r.style.as_mut(), r.scorer.as_mut()) else { continue };
            if r.progress.finished.is_some() {
                style.bank_running(clock);
                continue;
            }
            if clock < 0.0 {
                continue;
            }
            let state = &r.state;
            let input = nfsu2::drift::DriftInput {
                speed: state.velocity.length(),
                forward_speed: state.speed(),
                slip: state.slip_degrees().to_radians(),
                yaw_rate: state.spin.z,
                progress: (r.progress.progress() / laps).clamp(0.0, 1.0),
                touching: state.since_collision <= 0.0,
                off_road_wheels: state.wheels.iter().filter(|w| w.surface != 1).count() as u32,
                top_speed: r.car.car_type.handling.computed_top_speed,
                wrong_way: false,
                hit: state.last_hit,
                human: false,
                closeness,
            };
            scorer.update(style, &input, clock);
            style.update(dt, clock);
        }
        self.refresh_board();
    }

    /// The drift manager's closeness sum at `at` (0x56E0B0): every car
    /// of the world list past the racers (0x890080 +0x18 .. +0x10, the
    /// traffic; the rivals are not counted) with +0x28 set, by its 3D
    /// distance (0x43CE10 on the cars' +0x60). The clone takes the
    /// traffic cars on a lane (+0x28 is **not traced**; a parked car
    /// sits at the parking height z = −123456, 0x8025B8, past 20 m, and
    /// adds 0 either way).
    pub(crate) fn traffic_closeness(&self, at: Vec3) -> f32 {
        self.racers.traffic.as_ref().map_or(0.0, |t| {
            t.cars
                .iter()
                .filter(|c| c.lane.is_some())
                .map(|c| nfsu2::drift::closeness(c.state.position.distance(at)))
                .sum()
        })
    }

    /// 0x56DDE0 (each frame from 0x6076E0 at 0x608C7A in a drift race): every
    /// racer's node takes its style manager's banked DRIFT points (trick 0,
    /// +0x2C), then the list is sorted, highest first.
    pub(crate) fn refresh_board(&mut self) {
        if self.scoring.board.is_empty() {
            return;
        }
        let banked = |s: &Option<nfsu2::drift::StyleManager>| {
            s.as_ref().and_then(|s| s.trick(nfsu2::drift::DRIFT)).map_or(0.0, |t| t.banked)
        };
        for e in &mut self.scoring.board {
            match e.racer {
                Some(0) => e.score = banked(&self.scoring.style),
                Some(i) => e.score = self.racers.rivals.get(i - 1).map_or(0.0, |r| banked(&r.style)),
                None => {}
            }
        }
        nfsu2::drift::sort_board(&mut self.scoring.board);
    }

    /// The player's drift score: the style manager's total.
    pub(crate) fn score(&self) -> Option<i32> {
        self.scoring.drift.as_ref()?;
        self.scoring.style.as_ref().map(|s| s.total as i32)
    }

    /// A racer's progress in laps (racer +0x20) and finish, for the
    /// standings.
    pub(crate) fn standing(&self, p: &Progress) -> nfsu2::standings::Standing {
        nfsu2::standings::Standing { laps: p.progress(), finished: p.finished }
    }

    /// The reference lap (0x605260 → 0x5F6E80, [`nfsu2::standings::walk`])
    /// from the start line marker (+0xC10) through the race's AI routes.
    pub(crate) fn lap_reference(&self) -> nfsu2::standings::LapReference {
        use nfsu2::standings::{LapReference, RaceRoad, walk};
        let road = &self.route.road;
        let Some(routes) = &self.racers.ai_routes else { return LapReference::new(road.length, []) };
        let finish_at = if road.loops { self.route.start_at } else { self.route.finish_at };
        let race = RaceRoad { road, start_at: self.route.start_at, finish_at };
        let start = self.route.start_line.or(self.route.grid.first().map(|g| g.0)).unwrap_or_default();
        LapReference::new(road.length, walk(&routes.file, &race, start))
    }

    /// A point to point race's `%3d%%` (0x4AA1B0): the player's progress
    /// in laps (+0x14), at least 0.
    pub(crate) fn progress(&self) -> f32 {
        self.racers.me.progress().max(0.0)
    }
}
