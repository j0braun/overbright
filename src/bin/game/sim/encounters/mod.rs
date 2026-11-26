use crate::*;

pub(crate) mod ai;
pub(crate) mod draw;

/// The class the encounter racers' racing lines are computed for
/// (0x5DDFD0): the best (lowest) performance class of the loaded racers,
/// whose performance is matched to the player's; 3 with none.
pub(crate) fn encounter_class<'a>(cars: impl Iterator<Item = &'a Car>) -> usize {
    cars.map(|c| c.car_type.handling.performance_class).min().unwrap_or(3)
}

/// The hold a racer done with an outrun keeps before it is parked (car
/// +0x978 = 2.0 from 0x7A55C8, stored at 0x607144 in 0x606CC0, 0x607683 in
/// 0x607190 and 0x608257 in 0x6076E0; read by 0x5EBFB0).
pub(crate) const ENCOUNTER_HOLD: f32 = 2.0;

/// An outrun (race state 0x89E7B0 = 7): the player against one encounter
/// car.
pub(crate) struct Outrun {
    pub(crate) opponent: usize,
    /// Seconds since it started (0x607190 starts the racers' clocks).
    pub(crate) clock: f32,
    pub(crate) start_tick: u32,
    /// The racers' distance behind the leader (racer +0x24, 0x5FC520) and
    /// whether the player is first in the race order (race status +0xBD0).
    pub(crate) player: f32,
    pub(crate) rival: f32,
    pub(crate) player_leads: bool,
}

/// Career free roam's random encounters (docs/formats.md, "Outrun"): the
/// race status's encounter manager (+0xC40, 0x42EDF0, run while the race
/// state 0x89E7B0 is 0 or 7) with the career's side (career +0x4230,
/// 0x513DB0), its three AI racers, and the outrun the Enter Race / Shop key
/// starts. The racers drive with the race rivals' AI on RoutesFreeRoam:
/// before the race AIGoalOffsetPursuit on the player (0x42A860 → 0x428630,
/// type 9, with the slot's side offsets 0 / ±5 and action 0x13); in it
/// the challenged car's goals 5 / 6 (0x4285A0 / 0x428510), and every other
/// car out goal 5 on the race's leader (0x42EF04..0x42EF36).
pub(crate) struct Encounters {
    /// 0x42EDF0's state (+0x00): 0 NONE, 1 POLL_CAREER, 3 TRYING_TO_ENGAGE,
    /// 4 RACING, 5 DEACTIVATE_OPPONENT. POLL_CAREER goes straight to 3
    /// (0x42EE2C); LOADING_OPPONENTS (2) is not entered from it.
    pub(crate) state: u8,
    /// The routes' points by place, for `player_node`.
    pub(crate) grid: Option<nfsu2::formats::ai_routes::PointGrid>,
    /// The career's state (+0x1AE8: 0 idle, 1 asking for opponents, 2 a
    /// car in reach, 3 none) and timer (+0x1AE4: 30 s from the career
    /// reset 0x532840, kept in `Profile::outrun_timer`; 0 after an
    /// outrun, 0x500CA0; less each frame's dt).
    pub(crate) career: u8,
    pub(crate) timer: f32,
    pub(crate) routes: nfsu2::ai::route::AiRoutes,
    pub(crate) cars: Vec<EncounterCar>,
    /// The cars' collision state when explore mode has no traffic (the
    /// traffic's field otherwise). Its generator is the world generator
    /// (world +0xF8) for every encounter draw — the spawn searches, the
    /// racers' thinks: with traffic loaded the traffic's field holds it
    /// and [`Encounters::frame`] borrows it for the frame.
    pub(crate) field: nfsu2::drive::CarField,
    /// The car the player may challenge (player +0x250) and whether the
    /// HUD's encounter icon is up (0x4DB890).
    pub(crate) target: Option<usize>,
    pub(crate) icon: bool,
    /// The key was taken (race status +0xC30): the outrun starts the next
    /// frame (0x6076E0 → 0x607190).
    pub(crate) start: bool,
    pub(crate) outrun: Option<Outrun>,
    /// The outrun that just ended, while the manager deactivates (state
    /// 5). The end (0x5F84A0) calls 0x42EFB0, which first runs 0x42CD60:
    /// every AI racer out (0x5ED440) whose goal is not of type 1 gets goal
    /// 1, "Win road race" (0x42AA20(1) → 0x40CA20; 0x402070 drops the old
    /// goal and its action). The cars still out drive on with it until
    /// 0x40BF60 / 0x40BFB0 park them.
    pub(crate) ended: Option<Outrun>,
    /// The manager's clock, s, for the slots' stamps.
    pub(crate) clock: f32,
    /// `clock` at the last challenge (0x607190 restarts race status +0x14,
    /// the belt gate's race clock); 0 from the free-roam load (inferred).
    pub(crate) race_clock_from: f32,
    pub(crate) road_prev: HashMap<usize, Vec3>,
    pub(crate) road_dt: Option<f32>,
    /// The player's road node direction flag (controller +0x10, 1 from the
    /// constructor 0x40F630, flipped by 0x40A390).
    pub(crate) forward: bool,
    /// The opt-in encounter variety's own generator (not the game's:
    /// no native draw reads or moves it).
    pub(crate) rng: nfsu2::opponents::GameRng,
    /// The last outrun's end for the race-over message (0x5FFF60 shows it
    /// 3 s from the finish tick): won, the bank shown, the race time and
    /// the manager's clock at the end.
    pub(crate) result: Option<(bool, Option<i32>, f32, f32)>,
    /// The racers' cars still loading (on threads of their own, so the
    /// free-roam load does not wait for them), with each one's skill row
    /// and seed and the throttle level (0x89E804): joined into `cars` by
    /// [`Encounters::ready`] before the first car is placed. Until then
    /// `cars` is empty, which every reader takes as all parked.
    pub(crate) pending: Option<PendingCars>,
    /// The throttle level the cars were set up with (0x89E804).
    pub(crate) throttle: usize,
    /// Outrun Variety (Enhancements, clone deviation; off = native, the
    /// three cars of the set-up until the next free-roam load): per
    /// racer, whether it has been on the road since it was drawn; once
    /// such a car is parked, a new car is drawn for its slot by the
    /// set-up's own rule ([`EncounterDraw`]) and loaded on a thread
    /// (`variety_load`: slot, skill row, seed, the load, the tick it was
    /// asked at), and swapped in
    /// if still parked when the load is in.
    pub(crate) variety: Vec<bool>,
    pub(crate) variety_load: Option<(usize, usize, u32, std::thread::JoinHandle<Result<Car>>, u64)>,
}

pub(crate) type PendingCars = (
    std::thread::JoinHandle<(Vec<Result<Car>>, Result<std::sync::Arc<nfsu2::ai::route::AiRoutes>>)>,
    Vec<(usize, u32)>,
    usize,
);

impl Encounters {
    /// Career free roam's set-up (0x540790: 0x89E7C4 = 3 → 0x53F010(3)):
    /// stock cars drawn by 0x4FEAF0(record, 100 + i, 2 + i, 1, 0, 0, −1) —
    /// the pool for the player's car (the SUVs for an SUV, record +0x88A),
    /// the player's car type allowed, against the player's car's rating
    /// (−1: 0x5B69A0 on the player's) — each with a skill row 0x4FED00 draws
    /// for the difficulty (0x89E7F4: the career difficulty, which the city
    /// starts 0x5404B0 / 0x540790 copy from 0x83AC30); and, the clone's stand-in for their driving,
    /// RoutesFreeRoam as the race rivals' route file, pruned as free roam
    /// loads it (0x5E9E90's free-roam branch on the paths file's barriers
    /// for the career `stage`, the network the GPS and traffic read), with
    /// its racing speeds for their best class (0x5DDFD0 / 0x5E8790; no
    /// junction-type adjustment, free roam builds no race road). The looks
    /// (`nfsu2::ai_looks`): 0x53F010 draws the 8 names first (0x4FE970,
    /// not shown), and each car gets the stock parts (0x6371A0), a random
    /// paint (0x6371F0) and a level-2 neon (0x6371B0(0x9A, 2)) before its
    /// skill row; then 0x532020(0) dresses every AI racer by its
    /// performance: 0x5B7580 matches it to the player's `human`
    /// (`ai::route::match_to_human`: the nearest upgrade preset, the
    /// player's nitrous level, the category-9 refit 0x5A6890), and
    /// 0x6399E0(1, 1) → 0x6374C0 dresses by the levels that leaves
    /// (0x5995F0). The racer set-up 0x6373F0 is not traced.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        root: &std::path::Path,
        caches: &std::sync::Arc<Caches>,
        car_types: &[nfsu2::car::CarType],
        parts: &nfsu2::formats::parts::PartDb,
        perf: (&[nfsu2::performance::Package], &[(String, Vec<u8>)], Option<nfsu2::ai::route::HumanPerf>),
        ai: &AiData,
        track: Option<&career::Track>,
        player: &Car,
        (difficulty, throttle): (usize, usize),
        stage: u32,
        rng: &mut nfsu2::opponents::GameRng,
        ahead: &mut Vec<(RivalKey, std::thread::JoinHandle<Result<Car>>)>,
    ) -> Result<Encounters> {
        let packages = perf.0;
        let (picks, loads) = Self::picks(car_types, parts, perf, ai, player, difficulty, rng);
        // The cars load on threads while free roam starts; `ready` joins
        // them before the first placement (they are all parked till
        // then), each with its matched performance (`load_upgraded`).
        // The racing lines' speeds (0x5DDFD0 → 0x5E8790) take the best
        // performance class of the AI racers' infos (kind 2, 0x5B5340 on
        // their matched performance), so the loaded, upgraded cars' class,
        // not the stock car types' (observed under Wine on a finished-career
        // save: route manager +0x4674 (0x88E994) = 0 with a
        // COROLLA / CORSA / PEUGOT whose stock class is 3; the line bytes
        // of record 1095 matched the clone's class-0 speeds exactly;
        // docs/notes/outrun/ai-strength.md). Picked once the cars are in.
        let speeds = track.map(|t| {
            let suv = picks.iter().any(|p| car_types[p.0 as usize].suv);
            (suv, nfsu2::ai::route::speed_mode(t.flags, t.number))
        });
        let packages_owned: Vec<nfsu2::performance::Package> = packages.to_vec();
        let rows: Vec<(usize, u32)> = picks.iter().map(|p| (p.2, p.3)).collect();
        // A load made ahead for the same arguments ([`EncountersAhead`])
        // is taken as it is.
        let taken: Vec<_> =
            loads.iter().map(|key| ahead.iter().position(|(k, _)| k == key).map(|i| ahead.swap_remove(i).1)).collect();
        if nfsu2::prof::enabled() {
            eprintln!("[time] encounter cars loaded ahead: {} of {}", taken.iter().flatten().count(), loads.len());
        }
        let root_owned = root.to_path_buf();
        let caches_owned = caches.clone();
        let curves = ai.curves.clone();
        let job = std::thread::Builder::new()
            .name("encounter cars".into())
            .spawn(move || {
                std::thread::scope(|s| {
                    let hs: Vec<_> = loads
                        .iter()
                        .zip(taken)
                        .map(|((n, i, f), done)| match done {
                            Some(h) => Err(h),
                            None => Ok(s.spawn(|| {
                                Car::load_upgraded(&root_owned, n, i, &packages_owned, &|id| f.contains(&id))
                            })),
                        })
                        .collect();
                    let cars: Vec<Result<Car>> = hs
                        .into_iter()
                        .map(|h| match h {
                            Ok(h) => h.join().unwrap(),
                            Err(h) => h
                                .join()
                                .unwrap_or_else(|_| Err(anyhow::anyhow!("an encounter car's load ahead panicked"))),
                        })
                        .collect();
                    let speeds = speeds
                        .map(|(suv, mode)| (encounter_class(cars.iter().filter_map(|c| c.as_ref().ok())), suv, mode));
                    let routes = encounter_routes(&caches_owned, &root_owned, stage, curves.as_ref(), speeds);
                    (cars, routes)
                })
            })
            .context("the encounter cars' thread")?;
        let file = (*free_roam_routes(caches, root, stage)?).clone();
        let routes = nfsu2::ai::route::AiRoutes {
            file,
            track: 4000,
            backward: false,
            race_coords: false,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        Ok(Encounters {
            grid: None,
            state: 0,
            career: 0,
            timer: 0.0,
            routes,
            cars: Vec::new(),
            // The world generator comes with the world build (0x5FF557,
            // `Game::world_build`); the set-up draws no seeds (docs/original/rng.md: the
            // clone's two extra draws here are gone).
            field: nfsu2::drive::CarField::new(0),
            target: None,
            icon: false,
            start: false,
            outrun: None,
            ended: None,
            clock: 0.0,
            race_clock_from: 0.0,
            forward: true,
            rng: nfsu2::opponents::GameRng::default(),
            result: None,
            road_prev: HashMap::new(),
            road_dt: None,
            pending: Some((job, rows, throttle)),
            throttle,
            variety: Vec::new(),
            variety_load: None,
        })
    }

    /// The set-up's draws ([`Encounters::new`]): each racer's car type,
    /// looks, skill row and seed, and its car's load (car, parts,
    /// performance).
    #[allow(clippy::type_complexity)]
    pub(crate) fn picks(
        car_types: &[nfsu2::car::CarType],
        parts: &nfsu2::formats::parts::PartDb,
        perf: (&[nfsu2::performance::Package], &[(String, Vec<u8>)], Option<nfsu2::ai::route::HumanPerf>),
        ai: &AiData,
        player: &Car,
        difficulty: usize,
        rng: &mut nfsu2::opponents::GameRng,
    ) -> (Vec<(u8, std::collections::BTreeMap<u8, u32>, usize, u32)>, Vec<RivalKey>) {
        let draw = EncounterDraw::new(car_types, ai, player);
        // The draws first (a car's load draws none), then the cars
        // loaded side by side.
        let _names = nfsu2::frontend::quick_race::draw_names(rng, 8);
        let t_picks = nfsu2::prof::span("Encounters::new: picks");
        let mut picks = Vec::new();
        for _ in 0..nfsu2::outrun::CARS {
            let Some(p) = draw.pick(car_types, parts, ai, difficulty, rng, &[]) else { break };
            picks.push(p);
        }
        // 0x532020(0) after them all, in racer order: the performance
        // matched to the player's (0x5B7580; no draws), then dressed.
        let fits: Vec<_> = picks.iter_mut().map(|p| EncounterDraw::dress(car_types, parts, perf, ai, p, rng)).collect();
        drop(t_picks);
        let loads = picks
            .iter_mut()
            .zip(fits)
            .map(|(p, f)| (car_types[p.0 as usize].name.clone(), std::mem::take(&mut p.1), f))
            .collect();
        (picks, loads)
    }

    /// The racers' cars, once their loads are in (waited for if not):
    /// each parked, with its skill row, driver and seed. A failed load
    /// leaves no racers (the set-up failed as a whole before).
    pub(crate) fn ready(&mut self) {
        let Some((job, rows, throttle)) = self.pending.take() else { return };
        let _t = nfsu2::prof::span("Encounters: cars joined");
        let (loaded, routes) = job.join().expect("the encounter cars");
        match routes {
            Ok(r) => self.routes = (*r).clone(),
            Err(e) => {
                eprintln!("encounters: {e:#}");
                return;
            }
        }
        let mut cars = Vec::new();
        for ((row, seed), car) in rows.into_iter().zip(loaded) {
            let car = match car {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("encounters: {e:#}");
                    return;
                }
            };
            cars.push(encounter_car(car, row, seed, throttle));
        }
        self.variety = vec![false; cars.len()];
        self.throttle = throttle;
        self.cars = cars;
    }

    /// A car as the encounter tests read it.
    pub(crate) fn view(state: &CarState) -> nfsu2::outrun::Car {
        let f = state.forward();
        nfsu2::outrun::Car {
            position: state.position,
            velocity: state.velocity,
            forward: Vec2::new(f.x, f.y),
            speed: state.velocity.length(),
            forward_speed: state.speed(),
        }
    }

    /// The player's road node: the nearest control point (the stand-in the
    /// traffic placer takes for the human controller's +0x0C, inferred)
    /// travelled by the direction flag, which 0x40A390 flips when the
    /// travel (the velocity above 10 mph, else the heading) is more than
    /// 135° off the point's direction (dot < −0.707).
    pub(crate) fn player_node(&mut self, state: &CarState) -> Option<(usize, usize, bool)> {
        // (Called once a frame, as the controller's think, 0x42CAA0.)
        let f = &self.routes.file;
        let (record, cp) =
            self.grid.get_or_insert_with(|| nfsu2::formats::ai_routes::PointGrid::new(f)).nearest(f, state.position)?;
        let dir = f.direction(record, cp, !self.forward);
        let travel = if state.velocity.length() * 2.237 <= 10.0 {
            state.forward().truncate()
        } else {
            state.velocity.truncate().normalize_or_zero()
        };
        if travel.dot(dir) < -0.707 {
            self.forward = !self.forward;
        }
        Some((record, cp, !self.forward))
    }

    /// 0x42A5E0: at most one placement 5 s after the newest; for each
    /// parked car, search 250 m on from the player's road node in its
    /// direction (0x415130 with its racing-line argument,
    /// `racer_search`; free roam, so no coordinate race), then 50 m a try while under 500 m, until a
    /// point 250 m or more from the player (0x5F0FC0, 2D here) with flag bit
    /// 11 and no car near (0x410FC0: 30 m, 300 m for racers); the car goes
    /// on it (0x5FBEE0) facing the walk's way at its racing line's speed
    /// byte plus 0.1 m/s (0x5F52D0), the slot stamped, its mode 1 and held
    /// flag clear.
    pub(crate) fn spawn(&mut self, d: &Drive, ai: &AiData, node: Option<(usize, usize, bool)>, traffic: &[Vec2]) {
        use nfsu2::outrun::{
            SPAWN_CLEAR, SPAWN_END, SPAWN_FLAG, SPAWN_GAP, SPAWN_SPACING, SPAWN_SPEED_ADD, SPAWN_START, SPAWN_STEP,
        };
        if let Some(newest) = self.cars.iter().filter_map(|c| c.placed).reduce(f32::max)
            && self.clock - newest <= SPAWN_GAP
        {
            return;
        }
        let me = d.state.position;
        for i in 0..self.cars.len() {
            if self.cars[i].active {
                continue;
            }
            let Some((record, cp, backward)) = node else { return };
            let f = &self.routes.file;
            let Some(mut at) =
                nfsu2::ai::traffic::racer_search(f, record, cp, backward, SPAWN_START, false, &mut self.field.rng)
            else {
                continue;
            };
            let mut walked = SPAWN_START;
            while walked < SPAWN_END {
                let c = &f.records[at.0].points[at.1];
                let p = c.position.truncate();
                let racers = self
                    .cars
                    .iter()
                    .filter(|o| o.active)
                    .all(|o| o.state.position.truncate().distance(p) >= SPAWN_SPACING.1);
                let cars = traffic.iter().all(|&t| t.distance(p) >= SPAWN_SPACING.0);
                if p.distance(me.truncate()) >= SPAWN_CLEAR && c.flags & SPAWN_FLAG != 0 && racers && cars {
                    let dir = f.direction(at.0, at.1, at.2);
                    let speed = f32::from(c.lane_speed[usize::from(at.2)]) + SPAWN_SPEED_ADD;
                    let position = c.position;
                    let e = &mut self.cars[i];
                    let w = e.car.car_type.wheels[0];
                    let ground = d.world.road_height_below(position + Vec3::Z * 3.0).unwrap_or(position.z);
                    let heading = dir.y.atan2(dir.x);
                    // The tank is the car's, kept across parking and
                    // placements (observed under Wine: a 350Z's tank went
                    // 24000 → 21841 → 18431 → 12357 → 2948 → 915 ticks
                    // over five outruns, never refilled).
                    let tank = e.state.nitrous.take();
                    e.state =
                        CarState::new(Vec3::new(position.x, position.y, ground - (w.position.z - w.radius)), heading);
                    e.state.nitrous = tank;
                    // Car +0x464: the encounter racers read 1 under Wine
                    // with the player's option Off (writer not traced;
                    // **inferred**: the player's value, as 0x403C20 gives
                    // racers at a race start).
                    e.state.stability_off = d.state.stability_off;
                    e.state.start_race(RaceMode::FreeRoam);
                    e.state.velocity = dir.extend(0.0) * speed;
                    // Road placement publishes its unadjusted lane argument;
                    // preserve the existing route-direction velocity approximation.
                    e.state.set_exported_forward_speed(f32::from(c.lane_speed[usize::from(at.2)]));
                    e.route = nfsu2::ai::route::RacerRoute::start(&self.routes, e.state.position, dir);
                    e.driver = nfsu2::ai::driver::AiDriver::new(&e.car.car_type.handling, e.car.car_type.suv, e.cap);
                    e.unstuck = Default::default();
                    let last_tick = e.controller.schedule.last_tick;
                    e.controller = Default::default();
                    e.controller.schedule.last_tick = last_tick;
                    e.controller.reset_placement((i + 1) as i16);
                    self.road_prev.remove(&(i + 1));
                    e.active = true;
                    e.recovery_pending = false;
                    e.belt = Default::default();
                    e.belt_hold = 0;
                    e.belt_block_ticks = 0;
                    // 0x5F6D60 brings the car back in 0x5F4410's mode
                    // (0x5F6DCE → 0x5F46B0): an encounter car (owner kind
                    // +0x24 = 3) with no contact hold (controller vt+0x50,
                    // 0x41E510: +0x734) and no drag / drift takes mode 3,
                    // the MeloMvr (observed under Wine: mode 3 whenever
                    // not on the belt).
                    e.belt_rating = ai.bounds.scale(e.car.car_type.handling.performance_rating);
                    e.state.melo = Some(nfsu2::melo::Melo::new(&e.car, &e.state, e.belt_rating));
                    // The tank is filled at the car's set-up (0x59F9DA),
                    // not by the physics mover: the MeloMvr never runs the
                    // physics step that fills the clone's lazily. The
                    // refill at a racer's reset (0x5F6C8C) needs a size
                    // change, which a placement does not make.
                    let h = &e.car.car_type.handling;
                    if e.state.nitrous.is_none() {
                        e.state.nitrous = Some(nfsu2::nitrous::Tank::new(h.nitrous_capacity, h.nitrous_shots));
                    }
                    e.driver_plan = Default::default();
                    e.done = None;
                    e.placed = Some(self.clock);
                    e.held = false;
                    // 0x42A5E0 → 0x42A840: the slot's side state 1.
                    e.side = Default::default();
                    return;
                }
                let Some(next) =
                    nfsu2::ai::traffic::racer_search(f, at.0, at.1, at.2, SPAWN_STEP, false, &mut self.field.rng)
                else {
                    break;
                };
                at = next;
                walked += SPAWN_STEP;
            }
        }
    }

    /// 0x40BEE0: a car placed over 30 s ago and 250 m or more from the
    /// player loses its held flag and is parked (0x5F6E30).
    pub(crate) fn park_far(&mut self, me: Vec3) {
        for c in self.cars.iter_mut().filter(|c| c.active) {
            if c.placed.is_some_and(|t| self.clock - t > nfsu2::outrun::PARK_AGE)
                && c.state.position.truncate().distance(me.truncate()) >= nfsu2::outrun::PARK_DISTANCE
            {
                c.held = false;
                c.active = false;
            }
        }
    }

    /// 0x40BF60: a car done with an outrun is parked once its 2 s hold has
    /// run (0x5EC070, 0x5EBFB0).
    pub(crate) fn park_done(&mut self) {
        for c in self.cars.iter_mut().filter(|c| c.active) {
            if c.done.is_some_and(|t| self.clock - t >= ENCOUNTER_HOLD) {
                c.active = false;
            }
        }
    }

    /// 0x40BFB0: the cars still out: done or within 250 m count; the rest
    /// are parked.
    pub(crate) fn deactivate(&mut self, me: Vec3) -> usize {
        let mut n = 0;
        for c in self.cars.iter_mut().filter(|c| c.active) {
            if c.done.is_some() || c.state.position.truncate().distance(me.truncate()) < nfsu2::outrun::PARK_DISTANCE {
                n += 1;
            } else {
                c.active = false;
            }
        }
        n
    }

    /// 0x413030 → 0x40C030: the car the player may challenge, then the
    /// career's state (0x500D20: 2 with the icon when the career allows,
    /// 0x513E00, else 3 and no icon); player +0x250 takes the car either way.
    pub(crate) fn scan(&mut self, me: &CarState, stage: u8, closed: bool, challenge: bool) {
        let player = Encounters::view(me);
        let cars: Vec<Option<nfsu2::outrun::Car>> =
            self.cars.iter().map(|c| c.active.then(|| Encounters::view(&c.state))).collect();
        let mut held: Vec<bool> = self.cars.iter().map(|c| c.held).collect();
        let pick = nfsu2::outrun::pick(&player, &cars, &mut held);
        for (c, h) in self.cars.iter_mut().zip(held) {
            c.held = h;
        }
        let ok = pick.is_some() && nfsu2::outrun::allowed(stage, closed, challenge);
        self.career = if ok { 2 } else { 3 };
        self.icon = ok;
        self.target = pick;
    }

    /// The racers' AI for a frame, as the race rivals' in `step_race`:
    /// route, target speed, JustDrive's path, GetUnstuck and the driver.
    pub(crate) fn think(
        &mut self,
        ai: &AiData,
        dt: f32,
        d: &Drive,
        player_node: Option<(usize, usize, bool)>,
        traffic: &[nfsu2::ai::actions::RoadCar],
        mut world_field: Option<&mut nfsu2::drive::CarField>,
    ) {
        // 609CF0 snapshots human full-car+60 before controller dispatch.
        // Drive is immutably borrowed throughout this think pass; no mover or
        // recovery placement executes between this snapshot and the gates.
        let human_car_position = belt_car_position(&d.car, &d.state);
        let player = d.state.position.truncate();
        let sample_dt = self.road_dt.unwrap_or(dt);
        self.road_dt = Some(dt);
        let mut road_cars = vec![road_car(0, &d.car, &d.state, self.road_prev.get(&0).copied(), true, true, sample_dt)];
        human_road_path(&mut road_cars[0], &d.human_road, &self.routes, &d.car, &d.state);
        road_cars.extend(traffic.iter().copied());
        road_cars.extend(self.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(i, c)| {
            road_car(i + 1, &c.car, &c.state, self.road_prev.get(&(i + 1)).copied(), true, false, sample_dt)
        }));
        self.road_prev = road_cars.iter().map(|c| (c.id, c.velocity)).collect();
        let racer_count = self.cars.len() as i32;
        let target_position = player_node
            .map_or(d.state.position, |(record, cp, _)| self.routes.file.records[record].points[cp].position);
        let clock = self.clock;
        let race_clock_from = self.race_clock_from;
        // The manager's racing state (0x42EDF0 state 4, race type 7,
        // 0x42EEB0; **verified**): the leader L = race order 1;
        // for i = 2 … count every racer out (0x5ED440) of car kind 2–4 gets
        // goal 5 on L (0x4285A0: a new goal 5, vtable 0x784B58, when its goal
        // is not type 5, else only retargeted). The clone's order holds the
        // player and the challenged car (`Encounters::race`); the other cars
        // out stand behind them (observed under Wine: GOLF / CELICA goal 5 on
        // the leading MUSTANGGT). L's target node: the player's road node,
        // or the challenged car's walk point (as `race` reads them).
        // From the end (the end's frame, then state 5) every car out has
        // goal 1 (`Encounters::ended`, 0x42CD60).
        let racing = self.outrun.as_ref().filter(|_| self.state == 4);
        let outrun = racing.or(self.ended.as_ref());
        let leader_target = outrun.map(|o| {
            if o.player_leads {
                target_position
            } else {
                let rival = &self.cars[o.opponent];
                rival
                    .route
                    .as_ref()
                    .and_then(|r| r.node())
                    .map_or(rival.state.position, |(rec, cp, _)| self.routes.file.records[rec].points[cp].position)
            }
        });
        let f = EncounterFrame {
            ai,
            d,
            routes: &self.routes,
            outrun: self.outrun.as_ref().or(outrun),
            state: self.state,
            road_cars: &road_cars,
            racer_count,
            target_position,
            player,
            human_car_position,
            clock,
            race_clock_from,
            sample_dt,
            dt,
        };
        use nfsu2::ai::encounter_controller::EncounterGoal;
        for (i, r) in self.cars.iter_mut().enumerate().filter(|(_, c)| c.active) {
            if racing.is_none()
                && let Some(ended) = self.ended.as_ref()
            {
                let field = world_field.as_deref_mut().unwrap_or(&mut self.field);
                outrun_racer_think(r, i, ended, &f, field, EncounterGoal::RoadRace, f.target_position);
                continue;
            }
            if let Some(outrun) = f.outrun.filter(|o| o.opponent == i) {
                let field = world_field.as_deref_mut().unwrap_or(&mut self.field);
                let goal = if outrun.player_leads { EncounterGoal::Chase } else { EncounterGoal::Lead };
                outrun_racer_think(r, i, outrun, &f, field, goal, f.target_position);
                continue;
            }
            if let (Some(outrun), Some(leader)) = (f.outrun, leader_target) {
                let field = world_field.as_deref_mut().unwrap_or(&mut self.field);
                outrun_racer_think(r, i, outrun, &f, field, EncounterGoal::Chase, leader);
                continue;
            }
            goal9_think(r, i, &f, world_field.as_deref_mut().unwrap_or(&mut self.field));
        }
    }

    /// One explore frame before the collision step, in 0x6076E0's order:
    /// the outrun's start (0x607190, race status +0xC30), its race (state
    /// 7: each racer's +0x24, the sort, the limit), the manager (0x42EDF0,
    /// last), then the racers' AI; and the career's side (0x513DB0, outside
    /// an outrun). Returns an outrun's end: whether the player won.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame(
        &mut self,
        d: &Drive,
        traffic: &[nfsu2::ai::actions::RoadCar],
        profile: Option<&Profile>,
        data: &CareerData,
        ai: &AiData,
        metric: bool,
        challenge: bool,
        dt: f32,
        mut world_field: Option<&mut nfsu2::drive::CarField>,
    ) -> Option<bool> {
        self.clock += dt;
        let stage = profile.map_or(0, |p| p.stage);
        // 0x86347E (stage 0's races closed again) for 0x500CB0 / 0x513E00.
        let closed = profile.is_some_and(|p| p.intro_closed);
        let me = d.state.position;
        let player_node = self.player_node(&d.state);
        if self.outrun.is_none() {
            self.timer -= dt;
            if self.career == 0 && profile.is_some_and(|p| p.outruns_offered(data, challenge, self.timer)) {
                self.career = 1;
            }
        }
        // 0x607190, the frame after the key: race state 7 with the
        // challenged car; the other cars out are done with their 2 s hold;
        // the manager races (0x40BE80; +0x04 = 7 only with two humans and no
        // AI, 0x89E7C0 = 2 and 0x89E7C4 = 0). The game's first leader is the
        // race order's first, the first encounter racer (0x607190 puts the
        // AI ahead of the human); the clone starts with the challenged car,
        // which the engage test put ahead of the player.
        if std::mem::take(&mut self.start)
            && let Some(o) = self.target.filter(|&o| self.cars.get(o).is_some_and(|c| c.active))
        {
            for (i, c) in self.cars.iter_mut().enumerate() {
                if i != o && c.active {
                    c.done = Some(self.clock);
                }
            }
            self.race_clock_from = self.clock;
            self.outrun = Some(Outrun {
                opponent: o,
                clock: 0.0,
                start_tick: d.world_clock.tick,
                player: 0.0,
                rival: 0.0,
                player_leads: false,
            });
            self.state = 4;
        }
        let ended = self.race(d, player_node, metric, dt);
        match self.state {
            0 => {
                // 0x412F90: the cars parked, the slots cleared, the career
                // idle (0x500D70).
                for c in &mut self.cars {
                    c.active = false;
                    c.placed = None;
                    c.held = false;
                    c.done = None;
                }
                self.ended = None;
                self.career = 0;
                self.state = 1;
            }
            // 0x41ACE0 with 0x500CB0: the career asking (and stage 0's
            // 0x86347E clear).
            1 if self.career == 1 && nfsu2::outrun::allowed(stage, closed, false) => self.state = 3,
            3 => {
                self.ready();
                // The spawn searches draw on the world generator (0x5EA4F0),
                // which the traffic's field holds while traffic is loaded.
                if let Some(w) = world_field.as_deref_mut() {
                    std::mem::swap(&mut w.rng, &mut self.field.rng);
                }
                self.spawn(d, ai, player_node, &traffic.iter().map(|c| c.position.truncate()).collect::<Vec<_>>());
                if let Some(w) = world_field.as_deref_mut() {
                    std::mem::swap(&mut w.rng, &mut self.field.rng);
                }
                self.scan(&d.state, stage, closed, challenge);
                self.park_far(me);
            }
            4 => self.park_done(),
            5 => {
                self.park_done();
                if self.deactivate(me) == 0 {
                    // 0x500CA0, then NONE.
                    self.timer = 0.0;
                    self.career = 0;
                    self.state = 0;
                }
            }
            _ => {}
        }
        self.think(ai, dt, d, player_node, traffic, world_field);
        let won = ended?;
        // 0x60AAB0 at once (no hold in race state 7, 0x5EA540) → 0x5F84A0:
        // race state 0, the career's result (0x541330, the caller),
        // player +0x250 cleared, the manager to DEACTIVATE_OPPONENT
        // (0x42EFB0: 0x89E7C4 is 3).
        self.target = None;
        self.state = 5;
        Some(won)
    }

    /// 0x6076E0 in race state 7 for the player and the challenged car:
    /// each one's +0x24 against the race order's first (0x5FC520; the
    /// leader's road node the player's, or the racer's walk's current
    /// point), the order (0x5EEC90 / 0x5EEC10: the smaller first), the
    /// leader's taken off both; the chaser more than the limit behind gets
    /// status 7, the leader 6, the AI racer its 2 s hold. Returns the end.
    pub(crate) fn race(
        &mut self,
        d: &Drive,
        player_node: Option<(usize, usize, bool)>,
        metric: bool,
        dt: f32,
    ) -> Option<bool> {
        let o = self.outrun.as_mut()?;
        o.clock += dt;
        let rival = &self.cars[o.opponent];
        let (pv, rv) = (Encounters::view(&d.state), Encounters::view(&rival.state));
        let node = |(r, c, b): (usize, usize, bool)| nfsu2::outrun::Node {
            position: self.routes.file.records[r].points[c].position,
            direction: self.routes.file.direction(r, c, b),
        };
        if o.player_leads {
            let n = player_node.map(node);
            o.player = 0.0;
            o.rival = nfsu2::outrun::behind(&rv, &pv, n.as_ref());
        } else {
            let n = rival.route.as_ref().and_then(|r| r.node()).map(node);
            o.rival = 0.0;
            o.player = nfsu2::outrun::behind(&pv, &rv, n.as_ref());
        }
        let (first, second) = if o.player_leads { (o.player, o.rival) } else { (o.rival, o.player) };
        if nfsu2::outrun::second_leads(first, second) {
            o.player_leads = !o.player_leads;
        }
        let lead = if o.player_leads { o.player } else { o.rival };
        o.player -= lead;
        o.rival -= lead;
        let gap = if o.player_leads { o.rival } else { o.player };
        if !nfsu2::outrun::lost(gap, metric) {
            return None;
        }
        let (won, time, opponent) = (o.player_leads, o.clock, o.opponent);
        self.cars[opponent].done = Some(self.clock);
        self.ended = self.outrun.take();
        self.result = Some((won, None, time, self.clock));
        Some(won)
    }

    /// The Enter Race / Shop key with no event or shop in reach (0x602BE0
    /// case 0x602E19): in career free roam (race state 0) with a car to
    /// challenge (player +0x250) and the career's state 2 (0x500D80), the
    /// icon goes (0x4DB890(0)) and the outrun starts the next frame (race
    /// status +0xC30). The game also needs 0x85EF50 +0x1C clear (0x5012C0,
    /// not traced) and calls 0x838590 → 0x5010E0 (not traced).
    pub(crate) fn engage(&mut self) {
        if self.outrun.is_none() && self.target.is_some() && self.career == 2 {
            self.icon = false;
            self.start = true;
        }
    }

    /// The race-over message's time and result while 0x5FFF60 holds (3 s
    /// from the finish tick, 0x5FFF9B).
    pub(crate) fn race_over(&self) -> Option<(bool, Option<i32>, f32)> {
        self.result.filter(|r| self.clock - r.3 <= 3.0).map(|r| (r.0, r.1, r.2))
    }
}

/// What every encounter racer's think reads this frame
/// (`Encounters::think`).
#[derive(Clone, Copy)]
struct EncounterFrame<'a> {
    ai: &'a AiData,
    d: &'a Drive,
    routes: &'a nfsu2::ai::route::AiRoutes,
    outrun: Option<&'a Outrun>,
    /// The manager's state (3: trying to engage).
    state: u8,
    road_cars: &'a [nfsu2::ai::actions::RoadCar],
    racer_count: i32,
    target_position: Vec3,
    player: Vec2,
    human_car_position: Vec3,
    clock: f32,
    race_clock_from: f32,
    sample_dt: f32,
    dt: f32,
}

/// An outrun racer (goal 5 on `target_position`'s car, goal 6, or after
/// the end goal 1): the challenged car, and during the race every other
/// car out with goal 5 on the leader. `encounter_racer_think` on the
/// world's car field, then its reset request queued (car +0x96C).
fn outrun_racer_think(
    r: &mut EncounterCar,
    i: usize,
    outrun: &Outrun,
    f: &EncounterFrame,
    field: &mut nfsu2::drive::CarField,
    goal: nfsu2::ai::encounter_controller::EncounterGoal,
    target_position: Vec3,
) {
    let EncounterFrame { ai, d, routes, road_cars, racer_count, player, clock, dt, .. } = *f;
    let (human_car_position, sample_dt) = (f.human_car_position, f.sample_dt);
    let partners = field.partners(i as u64 + 1);
    let hits: Vec<usize> = partners.iter().filter(|&&id| id < TRAFFIC_ID).map(|&id| id as usize).collect();
    let me = road_cars.iter().find(|c| c.id == i + 1).copied().unwrap();
    encounter_racer_think(
        r,
        ThinkCtx { ai, routes, field_rng: &mut field.rng, clock, dt },
        EncounterThinkIn {
            others: road_cars,
            me,
            hits: &hits,
            touching: !partners.is_empty(),
            slot: i as i32,
            count: racer_count,
            world_clock: &d.world_clock,
            goal,
            target_position,
            player,
            race_clock: outrun.clock,
            race_start_tick: outrun.start_tick,
            sample_dt,
            human_position: human_car_position,
            loaded: streamed_in(
                &d.world,
                d.world.visible_sections_at(d.state.position).map(|(_, l)| l),
                r.state.position,
            ),
        },
    );
    // 5EBF70 queues car+96C; 6012C0 consumes it after motion.
    r.recovery_pending |=
        r.controller.actions.as_mut().is_some_and(|actions| std::mem::take(&mut actions.reset_pending));
}

/// Any other active encounter car (goal 9, AIGoalOffsetPursuit while the
/// manager engages): its mover timers, the route and target speed, the
/// line path, GetUnstuck and the driver, then the belt gate
/// (`goal9_belt_gate`).
fn goal9_think(r: &mut EncounterCar, i: usize, f: &EncounterFrame, field: &mut nfsu2::drive::CarField) {
    use nfsu2::ai::driver::DriverInputs;
    let EncounterFrame { ai, d, routes, player, clock, dt, .. } = *f;
    let loaded =
        streamed_in(&d.world, d.world.visible_sections_at(d.state.position).map(|(_, l)| l), r.state.position);
    encounter_mover_timers(r, loaded, dt);
    r.driver.mover = encounter_mover_kind(r);
    if r.controller.goal.take().is_some() {
        r.controller.actions = None;
        r.controller.output = None;
        if let Some(route) = r.route.as_mut() {
            route.set_outrun_chaser(false);
        }
        r.unstuck.since = None;
    }
    // The controller's full think (moved ahead of the action; the
    // belt gate below reads the same answer).
    let full = goal9_full_think(r, i, f);
    let h = &r.car.car_type.handling;
    let suv = r.car.car_type.suv;
    let (mut path, mut target) = (Vec::new(), 0.0);
    let travel = nfsu2::ai::route::travel(r.state.speed(), r.state.velocity, r.state.forward());
    if let Some(route) = &mut r.route {
        let leader = f.outrun.is_some_and(|o| o.opponent == i && !o.player_leads);
        route.outrun_leader_positions = leader.then_some((r.state.position.truncate(), player));
        // Goal 6 draws from the collision world's generator, as
        // ordinary race routes do. The player is second when this
        // challenged opponent leads (during the race the other cars
        // out think as goal-5 racers, `Encounters::think`).
        if leader {
            let rng = &mut field.rng;
            std::mem::swap(&mut route.rng, rng);
            route.update(routes, r.state.position, r.state.speed(), travel, h.performance_class, suv, &*r.accel);
            std::mem::swap(&mut route.rng, rng);
        } else {
            route.update(routes, r.state.position, r.state.speed(), travel, h.performance_class, suv, &*r.accel);
        }
        target = if let Some(outrun) = f.outrun.filter(|o| o.opponent == i) {
            r.speed.update_outrun(&ai.tuning, route, &mut field.rng, f64::from(clock), !outrun.player_leads, dt)
        } else {
            r.speed.update(&ai.tuning, route, &mut field.rng, f64::from(clock), None, dt)
        };
        // The catch-up (racer controller slot +0x80 = 0x4172D0, read
        // by 0x41C5E0 at 0x41C8A8): with no race on (race type
        // 0x89E7B0 = 0) it is 0.75 (0x783B74, 0x417337) every
        // frame; during an outrun (type 7) the gap bonus needs the
        // option, which a career clears, so 1.
        if f.outrun.is_none() {
            target *= 0.75;
        }
        // While the manager tries to engage (state 3) each car runs
        // AIGoalOffsetPursuit on the player (0x42A860 → 0x428630,
        // goal type 9), whose slot +0x4C caps the driver's speed
        // (0x40DC00, `outrun::pursuit_cap`; observed under Wine:
        // 40 mph far from the player, 60 mph within 100 m).
        if f.state == 3 {
            let distance = r.state.position.truncate().distance(player);
            target = target.min(nfsu2::outrun::pursuit_cap(distance, d.state.velocity.length()));
        }
        // GetUnstuck's own lane path while it runs (see `racer_think`).
        let (rate, fresh) = if r.unstuck.since.is_some() {
            (nfsu2::ai::driver::UNSTUCK_LANE_RATE, r.unstuck.first_path())
        } else {
            (0.05, false)
        };
        if f.state == 3 && r.unstuck.since.is_none() {
            // AIGoalOffsetPursuit on the player: 0x42A860 every
            // update sets the goal (0x428630: the side machine's offset,
            // distance 10); its chooser 0x42DA70 and action 0x13 ("Drive at
            // offset", 0x42BC50) run on the controller's full think, the
            // action's output kept in between (vt+0x10 false).
            use nfsu2::ai::actions::{Actions, PURSUIT_DISTANCE, Scene};
            let me = f.road_cars.iter().find(|c| c.id == i + 1).copied();
            let human = f.road_cars.iter().find(|c| c.id == 0).copied();
            if let (Some(me), Some(human)) = (me, human) {
                let listed = r.controller.actions.as_ref().is_some_and(|a| a.listed.contains(&0));
                let (along, across) = nfsu2::ai::actions::pursuit_offsets(route, routes, &me, &human, listed);
                let offset = r.side.step(along, across);
                r.controller.actions.get_or_insert_with(Actions::default).set_pursuit(0, PURSUIT_DISTANCE, offset);
                if full {
                    let partners = field.partners(i as u64 + 1);
                    let hits: Vec<usize> = partners.iter().filter(|&&id| id < TRAFFIC_ID).map(|&id| id as usize).collect();
                    let skill = r.speed.skill();
                    let scene = Scene {
                        routes,
                        me,
                        others: f.road_cars,
                        clock: clock - f.race_clock_from,
                        rate: 0.05,
                        street_x: false,
                        drift: false,
                        drag: false,
                        held: false,
                        contact_held: false,
                        block_row: skill,
                        hits: &hits,
                        touching: !partners.is_empty(),
                        wheels_down: r.state.wheels_down(),
                        up_z: (glam::Mat3::from_quat(r.state.orientation) * Vec3::Z).z,
                        forward_speed: r.state.speed(),
                        odds: ai.tuning.pass_odds(skill),
                    };
                    let actions = r.controller.actions.as_mut().unwrap();
                    r.controller.output = Some(actions.think(route, &scene, &ai.tuning, &mut field.rng, false, f.sample_dt));
                }
            }
            match r.controller.output.as_ref() {
                Some(out) => {
                    path = out.path.clone();
                    if let Some(cap) = out.cap {
                        target = target.min(cap);
                    }
                }
                None => path = r.driver_plan.0.clone(),
            }
        } else {
            path = route.line_path(
                routes,
                rate,
                r.state.position,
                r.state.forward(),
                r.state.velocity.length(),
                false,
                fresh,
                0.0,
            );
        }
    }
    let edge = r.route.as_ref().and_then(|x| x.edge(routes));
    let facing = r.route.as_ref().and_then(|x| x.facing_point(routes, false));
    let since = r.placed.map_or(0.0, |t| clock - t);
    if let Some(reverse) = r.unstuck.update(
        &r.state,
        h,
        facing,
        Some(r.driver.target),
        edge.as_ref(),
        since,
        None,
        f64::from(clock),
        -1,
        r.state.wheels_down() <= 2,
    ) {
        target = reverse;
    }
    let input = DriverInputs {
        path: &path,
        target_speed: target,
        enabled: true,
        green: true,
        countdown: false,
        since_green: since,
        clock: f64::from(clock),
    };
    // 0x420810 posts driver event 4 when the think planned the
    // path afresh (`RacerRoute::take_replanned`).
    if r.route.as_mut().is_some_and(|route| route.take_replanned()) {
        r.driver.event = 4;
    }
    r.controls = r.driver.update(&r.state, h, suv, &input, dt);
    r.driver_plan = (path, target);
    goal9_belt_gate(r, f, loaded, target, full);
    encounter_world_notice(r, loaded, target);
    r.driver.mover = encounter_mover_kind(r);
}

/// A goal-9 car's full think (0x42ABD0's schedule; its action type 19,
/// observed, so the world-update slot rule 0x42AC53).
fn goal9_full_think(r: &mut EncounterCar, i: usize, f: &EncounterFrame) -> bool {
    let EncounterFrame { d, racer_count, dt, .. } = *f;
    r.controller.schedule.full_think(nfsu2::ai::encounter_controller::ThinkInput {
        dt,
        tick: d.world_clock.tick,
        world_update: d.world_clock.update,
        owner_class: 2,
        owner_kind: 3,
        owner_id: (100 + i) as i16,
        racer_slot: i as i32,
        racer_count,
        drag: false,
        mover_four: r.belt.belt.is_some(),
        action_present: true,
    })
}

/// A goal-9 car's full think's belt gate (0x42B26C → 0x402180) on a
/// loaded world, or the unloaded mode-4 car's belt rebuilt.
fn goal9_belt_gate(r: &mut EncounterCar, f: &EncounterFrame, loaded: bool, target: f32, full: bool) {
    let EncounterFrame { human_car_position, clock, race_clock_from, .. } = *f;
    // The full think's belt gate (0x42B26C → 0x402180) runs for
    // every AIRacer controller, the goal-9 ones too (observed:
    // mode 3 ↔ 4 at ~120 m from the standing player before any
    // challenge). Their action is type 19 (observed), so the
    // cadence is the world-update slot rule (0x42AC53). The race
    // clock is race status +0x14, which the challenge restarts
    // (observed 328.5 → 0) and which otherwise runs on; its start
    // prov: inferred game.think
    // with the free-roam load is **inferred**.
    if full && loaded {
        let gate = nfsu2::ai::encounter_controller::BeltGate {
            special_track: false,
            drag: false,
            drift: false,
            path_points: r.driver_plan.0.len() as i32,
            timer730: r.belt_block_ticks as u32,
            timer734: r.belt_hold as u32,
            forward_speed: r.state.exported_forward_speed(),
            wheels_down: r.state.wheels_down() as i32,
            action: Some(if r.unstuck.since.is_some() { 9 } else { 19 }),
            skill: r.speed.skill() as i32,
            human_distance: nfsu2::ai::encounter_controller::belt_human_distance(
                belt_car_position(&r.car, &r.state),
                human_car_position,
            ),
            // No racer record outside the race (car +0x1C; 0x5F84A0 drops them,
            // 0x5F7960), so 0x5EE260's finished test never holds here.
            race_clock: clock - race_clock_from,
            finished: false,
        };
        if gate.wanted() {
            let speed = r.belt.belt.as_ref().map_or(r.state.exported_forward_speed(), |b| b.speed());
            if r.belt.belt.is_none() {
                r.state.leave_melo();
            }
            build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, &r.driver_plan.0, target);
        } else {
            encounter_leave_belt(r);
        }
    } else if full && r.belt.belt.is_some() {
        let speed = r.belt.belt.as_ref().map_or(0.0, |b| b.speed());
        build_belt_state(&r.car, &r.state, &mut r.belt, &*r.accel, speed, &r.driver_plan.0, target);
    }
}
