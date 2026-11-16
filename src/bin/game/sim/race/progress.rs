use crate::*;

/// One racer's run: its place on the route's road, its laps and times,
/// and the stats the race stats screen shows.
#[derive(Clone, Debug, Default)]
pub(crate) struct Progress {
    /// Where on the route's road the car last was, m (the locator's +0x24).
    pub(crate) along: f32,
    /// The car's locator on the road (car +0x8D8, its fraction +0x8F8;
    /// a racer's threshold +0x38 stays 0, 0x6042E7).
    pub(crate) locator: Locator,
    /// The racer's lap count, crossings and progress in laps (racer
    /// +0x04 … +0x30, 0x5F7C40); set by `place_on_road`.
    pub(crate) laps: Option<nfsu2::standings::Laps>,
    /// When the lap under way started (line 0's crossing), s on the race
    /// clock.
    pub(crate) lap_start: f32,
    /// The last lap's time and when it ended; the best lap.
    pub(crate) last_lap: Option<(f32, f32)>,
    pub(crate) best_lap: Option<f32>,
    /// The race clock at the finish (or when the racer dropped out).
    pub(crate) finished: Option<f32>,
    /// Dropped out of a drag race (0x6076E0): racer status 9 "Totaled" or
    /// 8 "Blown Engine".
    pub(crate) dropped: Option<u16>,
    /// Ended by the race's end while still running (0x60AAB0 → 0x608CB0,
    /// racer +0x19C set by 0x5F7ED0): its place in the race order then,
    /// which its finish order (+0x0C) follows; `finished` is the
    /// estimate.
    pub(crate) ended: Option<usize>,
    /// The racer's stats (racer +0x1A0, kept by 0x5F7420): the top speed
    /// (m/s) and the distance driven (car +0x414) for the average; the
    /// lap times (racer +0x98 on).
    pub(crate) top_speed: f32,
    pub(crate) distance: f32,
    /// Car +0x410: dt × |v| while 0 < |v| < 1000 in race status 4
    /// (0x604874), the result's +0xF8 (0x515DB0) that the profile's race
    // prov: inferred game.quads
    /// totals add (0x515990 → +0xA00). **Inferred**: the clone stops it at
    /// the finish (the original reads it at the results).
    pub(crate) odometer: f32,
    pub(crate) lap_times: Vec<f32>,
    /// Up-shifts and perfect ones (racer +0x1B8 / +0x1BC, 0x5F7420), and
    /// the gear last seen (+0x1CC; 1, neutral, at set-up 0x5F7936).
    pub(crate) shifts: u32,
    pub(crate) perfect_shifts: u32,
    pub(crate) gear_seen: Option<usize>,
    /// Where the car was last frame (car +0x510), for the distance.
    pub(crate) last_at: Option<Vec3>,
    /// The racer's four saved places (racer +0xC0, 16 bytes each) with
    /// their headings (+0x100) and the ring's count (+0xBC), and the
    /// frames counted for the every-32nd-frame save (0x7FB724).
    pub(crate) places: [(Vec3, f32); 4],
    pub(crate) place_index: u32,
    pub(crate) frames: u32,
}

/// The race's lines on its road (race status +0xBF8, 0x5EE630).
pub(crate) fn race_lines(route: &Route) -> nfsu2::standings::RaceLines {
    let finish_at = if route.road.loops { route.start_at } else { route.finish_at };
    nfsu2::standings::RaceLines::with_splits(
        &nfsu2::standings::RaceRoad { road: &route.road, start_at: route.start_at, finish_at },
        route.splits,
        route.backward,
    )
}

/// The race's laps (race status +0xBE8, 0x5FC7B4): a point to point
/// race's 1, a circuit's the event's.
pub(crate) fn race_laps(route: &Route, laps: u32) -> u32 {
    if route.loops { laps.max(1) } else { 1 }
}

impl Progress {
    /// The racer's car is at `at` at race time `clock`: follow it along
    /// the road (0x5DA340) and count its laps (0x5F7C40, from the green
    /// light, 0x5FC910 state 4); it is done once its most laps reach the
    /// race's (0x608C11 → 0x606CC0 status 1, the finish the finish line's
    /// crossing, the frame's clock).
    pub(crate) fn update(&mut self, route: &Route, laps: u32, clock: f32, at: Vec2) {
        // Before the green light the cars are held (the clone leaves the
        // run as `place_on_road` set it).
        if self.finished.is_some() || clock < 0.0 {
            return;
        }
        let lines = race_lines(route);
        route.road.relocate(&mut self.locator, at);
        self.along = self.locator.along;
        let p = self.locator.fraction * 100.0;
        let run = self.laps.get_or_insert_with(|| nfsu2::standings::Laps::new(&lines, p));
        if run.crossed[0].is_none() {
            run.go(&lines, 0.0);
        }
        if let Some((_, time)) = run.update(&lines, p, clock) {
            self.last_lap = Some((time, clock));
            self.lap_times.push(time);
            self.best_lap = Some(self.best_lap.map_or(time, |b| b.min(time)));
        }
        self.lap_start = run.lap_start();
        if run.done(race_laps(route, laps)) {
            self.finished = Some(clock);
        }
    }

    /// 0x5F7ED0 for a racer the race's end stops short of its `laps`
    /// (racer +0x19C = 1): the finish time, rounded to 0.01 s (0x577410),
    /// is the race clock plus the reference lap's time from the racer's
    /// progress (+0x20) to the leader's laps done (+0x09) when the leader
    /// (race order +0xBD0 [0]) is another racer that finished (status 1)
    /// for real (+0x19C clear); else the clock over the race fraction done
    /// (+0x14 / laps, at least 0.01). The laps not done each take an equal
    /// share of the rest (finish − the running sum at the laps done,
    /// +0xE8 + 4·done), a new best lap kept.
    pub(crate) fn end_short(
        &mut self,
        laps: u32,
        clock: f32,
        leader: Option<i32>,
        reference: &nfsu2::standings::LapReference,
    ) {
        let laps = laps.max(1);
        let progress = self.progress();
        let time = match leader {
            Some(most) => clock + reference.time_at(most as f32) - reference.time_at(progress),
            None => clock / (progress / laps as f32).max(0.01),
        };
        let time = (time * 100.0).round() / 100.0;
        let done = self.lap_times.len().min(laps as usize);
        let rest = laps as usize - done;
        if rest > 0 {
            let share = (time - self.lap_times.iter().take(done).sum::<f32>()) / rest as f32;
            self.lap_times.extend(std::iter::repeat_n(share, rest));
            self.best_lap = Some(self.best_lap.map_or(share, |b| b.min(share)));
        }
        self.finished = Some(time);
    }

    /// The racer's lap count (+0x08) and most laps done (+0x09), 0 / 0
    /// before it is placed.
    pub(crate) fn lap(&self) -> (i32, i32) {
        self.laps.as_ref().map_or((0, 0), |l| (l.lap, l.most))
    }

    /// Progress in laps (+0x20).
    pub(crate) fn progress(&self) -> f32 {
        self.laps.as_ref().map_or(0.0, |l| l.progress)
    }

    /// A drag race's drop-out test (0x6076E0, each frame for each racer
    /// still running): a totaled car (car +0x964) ends the run with racer
    /// status 9 "Totaled", else a blown engine (0x5ECE10 → 0x5942D0) with
    /// status 8 "Blown Engine" (0x606CC0 with the status).
    pub(crate) fn drag_drop_out(&mut self, state: &CarState, clock: f32) {
        if self.finished.is_some() || clock < 0.0 {
            return;
        }
        let status = if state.totaled {
            9
        } else if state.drivetrain.as_ref().is_some_and(|t| t.blown()) {
            8
        } else {
            return;
        };
        self.finished = Some(clock);
        self.dropped = Some(status);
    }

    /// HUD finish ordinal. Dropouts also have timestamps, but occupy
    /// losing slots rather than successful finishes (0x6076E0 / 0x606CC0).
    pub(crate) fn finish_order<'a>(&self, rivals: impl Iterator<Item = &'a Progress>) -> u32 {
        self.finished.map_or(0, |t| {
            1 + rivals.filter(|p| p.dropped.is_none() && p.ended.is_none() && p.finished.is_some_and(|f| f < t)).count()
                as u32
        })
    }

    /// Once done (0x606CC0), a racer in a drift or Street X race, or one
    /// that dropped out totaled or with a blown engine, stops colliding
    /// with cars (car +0x3C → body +0x190 = 0).
    pub(crate) fn leaves_traffic(&self, mode: RaceMode) -> bool {
        self.finished.is_some() && (mode.drift() || mode.street_x() || self.dropped.is_some())
    }

    /// Where the car starts on the road (a fresh locator) and its laps
    /// set up there (0x5FC2B0: lap −1, +0x1C the place).
    pub(crate) fn place_on_road(&mut self, route: &Route, at: Vec2) {
        self.locator = Default::default();
        route.road.locate(&mut self.locator, at);
        self.along = self.locator.along;
        self.laps = Some(nfsu2::standings::Laps::new(&race_lines(route), self.locator.fraction * 100.0));
    }

    /// 0x5F7420's per-frame stats: the top speed with more than two
    /// wheels down; and the distance driven (the car update 0x604840 adds
    /// |position − last frame's| to car +0x414 while racing).
    pub(crate) fn track(&mut self, state: &CarState) {
        if self.finished.is_some() {
            return;
        }
        if state.wheels_down() > 2 {
            self.top_speed = self.top_speed.max(state.speed());
        }
        // 0x5F7420: the gear (car +0x434) above the last seen and not 0
        // counts a shift; a perfect one when the car runs on its physics
        // mover (car +0x28 = 1, every racer here) and the transmission's
        // last up-shift rating (+0x68) is over 0.5.
        if let Some(t) = state.drivetrain.as_ref() {
            let last = self.gear_seen.unwrap_or(nfsu2::handling::NEUTRAL);
            if t.gear > last && t.gear != 0 {
                self.shifts += 1;
                if t.shift_bonus > 0.5 {
                    self.perfect_shifts += 1;
                }
            }
            self.gear_seen = Some(t.gear);
        }
        if let Some(last) = self.last_at {
            self.distance += (state.position - last).length();
        }
        self.last_at = Some(state.position);
    }

    /// The saved places (0x602AEE–0x602BD6, each racer frame): on every
    /// 32nd frame (0x7FB724 & 31) or while the ring is empty (+0xBC = 0),
    /// when the car is more than 16 m (0x79C68C, 2D) from the place at the
    /// ring's count and there is ground under it (0x5E7E90: the ground
    /// probe from z 99999.1, 0x5E72F0), the count steps on and the place
    /// there takes the car's position (car +0x60) and heading (+0x40C) —
    /// all four the first time.
    pub(crate) fn save_place(&mut self, state: &CarState, world: &nfsu2::world::World) {
        let frame = self.frames;
        self.frames = self.frames.wrapping_add(1);
        if frame % 32 != 0 && self.place_index != 0 {
            return;
        }
        let at = state.position;
        if self.places[(self.place_index & 3) as usize].0.truncate().distance(at.truncate()) <= 16.0 {
            return;
        }
        if world.road_height_below(Vec3::new(at.x, at.y, 99999.1)).is_none() {
            return;
        }
        let n = if self.place_index == 0 { 4 } else { 1 };
        for _ in 0..n {
            self.place_index = self.place_index.wrapping_add(1);
            self.places[(self.place_index & 3) as usize] = (at, state.heading());
        }
    }
}

/// Each racer's place at the end of each lap (0x541B50, after the race):
/// for laps 1 to 21 the racers are sorted (0x503360) by their time at the
/// end of that lap — the lap times summed — those without the lap last;
/// a racer's lap in first counts as led (+0xF4).
pub(crate) fn lap_places(runs: &[&Progress], laps: u32) -> Vec<Vec<u32>> {
    let mut places = vec![Vec::new(); runs.len()];
    for lap in 0..laps.min(21) as usize {
        let at: Vec<Option<f32>> =
            runs.iter().map(|r| r.lap_times.get(lap).map(|_| r.lap_times[..=lap].iter().sum())).collect();
        let mut order: Vec<usize> = (0..runs.len()).collect();
        order.sort_by(|&a, &b| match (at[a], at[b]) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        for (place, &i) in order.iter().enumerate() {
            places[i].push(place as u32 + 1);
        }
    }
    places
}
