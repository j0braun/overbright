use crate::*;

pub(crate) mod place;
pub(crate) mod think;

/// One traffic car (see docs/formats.md, "Traffic"): its model, where it
/// is, its lane (None while unspawned), the racers' low-level driver and
/// its TrafficMover (None once a hit has handed it to the MeloMvr).
pub(crate) struct TrafficCar {
    pub(crate) car: std::sync::Arc<Car>,
    /// Its renderer is `RenderState.cars[id]`.
    pub(crate) id: CarId,
    pub(crate) state: CarState,
    pub(crate) lane: Option<nfsu2::ai::traffic::TrafficLane>,
    pub(crate) driver: nfsu2::ai::driver::AiDriver,
    pub(crate) controls: nfsu2::drive::AiControls,
    pub(crate) mover: Option<nfsu2::ai::traffic::TrafficMover>,
    /// The path and target speed of its last think (0x422150 hands them to
    /// the driver), kept until its next.
    pub(crate) plan: Option<(Vec<Vec3>, f32)>,
    /// Its model's rating on the skills' 0–10 scale, for the MeloMvr.
    pub(crate) rating: f32,
    /// Stopped for good (controller +0x710: 0x408DE0's stuck test or
    /// 0x417750's dead end; `nfsu2::ai::traffic::STUCK_IMPULSE`), until
    /// it is placed again.
    pub(crate) stopped: bool,
    /// Mode 4, the BeltMover, while the world under the car is not loaded
    /// (see `traffic_stream`), and the car's acceleration curve for its
    /// build (0x4038D0).
    pub(crate) belt: BeltState,
    pub(crate) accel: std::sync::Arc<dyn Fn(f32) -> f32 + Send + Sync>,
    /// Car +0x550, "active" (0x5ED5D0): only an active car is a road
    /// object for the others' road entries (0x6148F0 keeps a node's car
    /// when 0x5EC070 is false and +0x550 is set, or when it is the car
    /// itself). A car the placer cannot place is paused (0x42FAAB →
    /// 0x4020D0: inactive, `held`); placed again, the controller's slot
    /// +0x34 (0x42A050 → 0x402110) makes it active only when the world
    /// under its old place is loaded (body vt+0x54(0), 0x593990: [body
    /// +0x60] +8 = car +0x5CC), and 0x5FBEE0's own 0x5F6CE0 then activates
    /// it when the world under the new place is loaded and was not
    /// (+0x5C0 0 → 1: 0x408C40 → 0x4022D0(1)); so a car placed over
    /// unloaded world from an unloaded parking place runs inactive on the
    /// BeltMover until the world under it loads (**observed** under
    /// Wine: 7–25 % of the belt cars' time in qr_walk_4014 inactive).
    pub(crate) active: bool,
    /// Controller +0x77D, paused (0x4020D0; cleared by 0x402110).
    pub(crate) held: bool,
    /// The walk a paused car keeps (0x4020D0 leaves the
    /// controller's walk, path and driver alone; `lane` is None while
    /// paused, so the car is no road object, no collision body and not
    /// drawn). Its controller update 0x429D90 returns at once (+0x77D: no
    /// think, no stuck test), but the world update 0x609CF0 still runs its
    /// driver (car +0x30 vt+4 = 0x421030) on the last path every frame,
    /// and the world under it loading gives it the TrafficMover
    /// (0x5F6CE0 → 0x408C40 → 0x5F46B0(0x5F4410) = mode 2): it coasts,
    /// inactive, until the world unloads (mode 4, the belt built from
    /// that path) or the placer places it again (see `traffic_stream`).
    pub(crate) coast: Option<nfsu2::ai::traffic::TrafficLane>,
    /// Its road locator in a race (car +0x8D8; its +0x24 is the race
    /// coordinate, car +0x8FC): set at placement, then moved on each frame
    /// once the car is [`TRAFFIC_LOCATOR_STEP`] from where it was last
    /// asked.
    pub(crate) coord: Option<nfsu2::formats::roads::Locator>,
}

impl TrafficCar {
    /// A road object for the other cars' road entries (0x6148F0 keeps a
    /// node's car when its +0x550 is set, `active`): placed and active. A
    /// pooled or paused car keeps its last place and velocity but is no
    /// obstacle, for the traffic and the racers alike.
    pub(crate) fn road_object(&self) -> bool {
        self.lane.is_some() && self.active
    }

    /// The car's race coordinate (car +0x8FC) in a race: its locator's
    /// place (+0x24); its place located on the road until it has one.
    pub(crate) fn race_coord(&self, road: &nfsu2::formats::roads::RoadRoute) -> f32 {
        self.coord.map_or_else(|| road.locate_once(self.state.position.truncate()), |c| c.along)
    }

    /// The frame's relocation (0x604750 → 0x5DA340 on car +0x60): the
    /// locator moves on once the car is its threshold (|Δx| + |Δy|) from
    /// where it was last asked.
    pub(crate) fn refresh_coord(&mut self, road: &nfsu2::formats::roads::RoadRoute) {
        let p = self.state.position.truncate();
        let loc = self.coord.get_or_insert_with(|| nfsu2::formats::roads::Locator {
            threshold: TRAFFIC_LOCATOR_STEP,
            ..Default::default()
        });
        road.relocate(loc, p);
    }

    /// The locator set at a placement (0x5FBEE0): for a point measured by
    /// race coordinate (0x5DCC90) from the route node nearest the point
    /// and its coordinate (+0x08; 0x5DA420), else from the place alone
    /// (0x5DA3E0); then relocated at the car's place (0x5DA340).
    pub(crate) fn place_coord(
        &mut self,
        road: &nfsu2::formats::roads::RoadRoute,
        point: Option<(Vec2, f32)>,
        at: Vec2,
    ) {
        let mut loc = nfsu2::formats::roads::Locator { threshold: TRAFFIC_LOCATOR_STEP, ..Default::default() };
        match point {
            Some((p, c)) => road.locate_at(&mut loc, p, c),
            None => road.locate(&mut loc, at),
        }
        road.relocate(&mut loc, at);
        self.coord = Some(loc);
    }
}

/// A traffic car's locator threshold (car +0x910 = locator +0x38): 16 m
/// of |Δx| + |Δy|, set by the car's constructor 0x604270 for a car of
/// kind 3 (traffic; **verified**). Under Wine the race coordinate moved
/// on after 11.1–12.1 m (2D), which a 16 m |Δx| + |Δy| gives on roads
/// running near the diagonal.
pub(crate) const TRAFFIC_LOCATOR_STEP: f32 = 16.0;

/// The traffic of explore mode (the region's RoutesFreeRoam.bin) or of a
/// race (the race's Routes<track><F|B>.bin, which carries the same
/// traffic lanes on its records): the cars and their shared collision
/// state.
pub(crate) struct Traffic {
    /// Placement tries and how many placed a car (the probe's count).
    pub(crate) tries: (u32, u32),
    pub(crate) region: String,
    /// The career stage whose barriers pruned `routes` (free roam's
    /// post-load 0x5E9E90 after 0x578070 closes `BARRIERS_CAREER<stage>`);
    /// None for a race's routes.
    pub(crate) stage: Option<u32>,
    pub(crate) routes: nfsu2::formats::ai_routes::RouteFile,
    pub(crate) cars: Vec<TrafficCar>,
    /// The pool not yet given its models ([`Traffic::populate`]).
    pub(crate) pool: Option<PoolSetup>,
    /// The traffic's car field. Its generator is not a stream of its own:
    /// in free roam it is the world generator (world +0xF8) whenever the
    /// traffic is loaded (the world's field, `Game::world_build`); in a
    /// race the race's field's generator is swapped in around the traffic's
    /// think and fill (`race_traffic`), so every traffic draw is the
    /// world's, as 0x5EA4F0 / 0x5EA510 draw on [0x890080].
    pub(crate) field: nfsu2::drive::CarField,
    pub(crate) clock: f32,
    /// The odds a placed car goes the placer's way (track +0x108 + 4·level).
    pub(crate) with: f32,
    /// The traffic level (0x89E7A8, 1–3; level 0 has no pool).
    pub(crate) level: usize,
    /// The placer's countdown to its next pass and the slot it last
    /// looked at (0x42F910's +0x00 / +0x04).
    pub(crate) timer: f32,
    pub(crate) cursor: usize,
    /// 0x42E9F0's cursor over the walk's starts (placer +0x0C, 0x417D80).
    pub(crate) start: usize,
    /// The region's traffic zones (route triggers of kind 0x10) and the AI
    /// tuning's spacing rows (0x418240).
    pub(crate) zones: Vec<nfsu2::ai::traffic::TrafficZone>,
    pub(crate) spacing: Vec<(f32, f32)>,
    /// The intersections' state (0x34123).
    pub(crate) crossings: nfsu2::ai::traffic::Crossings,
    /// The world update counter (0x7FB724) the cars' thinks take turns by,
    /// and the time not yet counted.
    pub(crate) updates: u64,
    /// The track's drag fill step (+0x100 + 4·direction, 0x42E4A0) and
    /// whether the race set-up's fill (0x6065C0 → 0x42F7A0) has run.
    pub(crate) fill_step: f32,
    pub(crate) filled: bool,
    /// Paused by a scene (0x4099B0 from the scene start 0x43B930 /
    /// 0x43B9A0): every car inactive (car +0x550 = 0, so World_DrawCars
    /// skips it) and its think off (NIS +0x61, 0x430340) until race state
    /// 3 (0x430310 / 0x408CA0). The clone hides the race's traffic during
    /// the start scene's camera.
    pub(crate) paused: bool,
}

impl Traffic {
    /// A region's traffic lanes from `files` (routes, paths: free roam's
    /// RoutesFreeRoam / PathsFreeRoam, a race's Routes<track><F|B> /
    /// Paths<track>) and the pool for the track and level (0x4FE9F0:
    /// track record +0xEC + 2·level + reversed; track 4000 at level 3:
    /// 16), each car given a traffic model by 0x412BE0 from the car types
    /// with record +0x844 = 2 and their caps for the traffic zone holding
    /// the player (zone +0x3C; the point is 0x430240's: in free roam the
    /// first car's +0x60/+0x64). The models stay for the whole load: the
    /// 5 s streamer 0x412950 loads and frees model data but no code moves a
    /// car to another model (only 0x420030, state 7, binds them).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn load(
        root: &std::path::Path,
        region: &str,
        files: (&str, &str),
        car_types: &[nfsu2::car::CarType],
        track: Option<&career::Track>,
        tuning: Option<&nfsu2::ai::route::AiTuning>,
        bounds: Option<&nfsu2::opponents::Bounds>,
        player: Vec3,
        level: usize,
        reversed: bool,
        density: u8,
    ) -> Result<Traffic> {
        use nfsu2::ai::traffic::{Crossings, TrafficZone, dense_pool, pool_size};
        let path = nfsu2::game_fs::path(&root, &format!("TRACKS/ROUTES{region}/{}", files.0));
        let routes = nfsu2::formats::ai_routes::RouteFile::parse(
            &nfsu2::game_fs::read_file(&path).with_context(|| format!("reading {}", path.display()))?,
        );
        let paths = nfsu2::game_fs::path(&root, &format!("TRACKS/ROUTES{region}/{}", files.1));
        let zones = TrafficZone::parse(&nfsu2::game_fs::read_file(&paths).with_context(|| format!("reading {}", paths.display()))?);
        let tuning = tuning.context("no AI tuning")?;
        let spacing: Vec<(f32, f32)> = (0..9).filter_map(|r| tuning.traffic_spacing(r)).collect();
        let track = track.context("no track record")?;
        let models: Vec<usize> = car_types
            .iter()
            .enumerate()
            .filter(|(_, t)| t.traffic && nfsu2::game_fs::path(&root, &format!("CARS/{}", t.name)).is_dir())
            .map(|(i, _)| i)
            .collect();
        anyhow::ensure!(!models.is_empty(), "no traffic car models");
        let zone = TrafficZone::at(&zones, player.truncate()).map_or(0, |z| z.caps);
        let caps: Vec<i8> = models.iter().map(|&m| car_types[m].traffic_caps.get(zone).copied().unwrap_or(0)).collect();
        // Traffic Density (opt-in; 0 = native): a larger free-roam pool.
        let size = dense_pool(pool_size(&track.traffic, level, reversed), density);
        let pool = PoolSetup { models, caps, size, bounds: bounds.copied() };
        let with = track.traffic_with.get(level).copied().unwrap_or(0.5);
        let crossings = Crossings::new(&routes, size);
        let fill_step = track.drag_fill[usize::from(reversed)];
        Ok(Traffic {
            region: region.into(),
            stage: None,
            routes,
            cars: Vec::new(),
            pool: Some(pool),
            field: nfsu2::drive::CarField::new(traffic_seed()),
            clock: 0.0,
            with,
            level,
            timer: 0.0,
            cursor: 0,
            start: 0,
            tries: (0, 0),
            zones,
            spacing,
            crossings,
            updates: 0,
            fill_step,
            filled: false,
            paused: false,
        })
    }
    /// The pool's cars (0x412BE0, Traffic_AssignModels): each given a
    /// traffic model from the car types with record +0x844 = 2 and their
    /// caps for the zone, drawing on the shared generator (0x412C9A), then
    /// the models loaded side by side. The world build makes it after the
    /// traffic fill and, in free roam, the career's slot events
    /// (**measured**, the sequence log, docs/original/rng.md). No-op once made.
    pub(crate) fn populate(
        &mut self,
        root: &std::path::Path,
        car_types: &[nfsu2::car::CarType],
        rng: &mut nfsu2::opponents::GameRng,
    ) -> Result<()> {
        use nfsu2::ai::traffic::{TRAFFIC_THROTTLE_CAP, assign_models};
        let Some(pool) = self.pool.take() else { return Ok(()) };
        let mut picks = assign_models(&pool.caps, pool.size, rng);
        // `TRAFFIC_MODEL=<name>` (a debugging knob for the shots, not the
        // game's): every pool car takes that model.
        if let Some(m) =
            debug_env().traffic_model.clone().and_then(|w| pool.models.iter().position(|&t| car_types[t].name == w))
        {
            picks.iter_mut().for_each(|p| *p = m);
        }
        // Each model once, loaded side by side.
        let mut distinct: Vec<usize> = Vec::new();
        for &m in &picks {
            if !distinct.contains(&m) {
                distinct.push(m);
            }
        }
        let names: Vec<&str> = distinct.iter().map(|&m| car_types[pool.models[m]].name.as_str()).collect();
        let mut loaded: HashMap<usize, std::sync::Arc<Car>> = HashMap::new();
        for (m, car) in distinct.into_iter().zip(load_cars(root, &names)) {
            loaded.insert(m, std::sync::Arc::new(car?));
        }
        for m in picks {
            let car = loaded[&m].clone();
            let h = &car.car_type.handling;
            // The CarDriver's throttle cap +0x22C (0x425B44): the
            // difficulty's 0x783A28 entry for a racer (controller class 2),
            // 1 for any other, so 1 for traffic (class 3).
            let driver = nfsu2::ai::driver::AiDriver::new(h, car.car_type.suv, TRAFFIC_THROTTLE_CAP);
            // The MeloMvr's skill grip reads the car's rating on the
            // skills' 0–10 scale (0x5B69A0(1, −1) on car +0x10).
            let rating = pool.bounds.map_or(0.0, |b| b.scale(h.performance_rating));
            let accel = std::sync::Arc::new(nfsu2::handling::acceleration(h));
            self.cars.push(TrafficCar {
                car,
                id: CarId::new(),
                state: CarState::new(Vec3::splat(1e6), 0.0),
                lane: None,
                driver,
                controls: Default::default(),
                mover: None,
                plan: None,
                rating,
                stopped: false,
                belt: Default::default(),
                accel,
                // Observed under Wine: every pool car active at free
                // roam's start; a race's fill cars are made active by the
                // race-state notice (0x408CA0, states 3 and 4).
                active: true,
                held: false,
                coast: None,
                coord: None,
            });
        }
        Ok(())
    }

    /// The pool's size before [`Traffic::populate`] (its cars after).
    pub(crate) fn pool_len(&self) -> usize {
        self.pool.as_ref().map_or(self.cars.len(), |p| p.size)
    }
}

/// What [`Traffic::populate`] needs: the traffic model types (indices into
/// the car types), their caps for the zone, the pool's size and the
/// performance bounds.
pub(crate) struct PoolSetup {
    models: Vec<usize>,
    caps: Vec<i8>,
    size: usize,
    bounds: Option<nfsu2::opponents::Bounds>,
}

/// `NFSU2_TRAFFIC_SEED=<n>` (a harness knob, not the game's: the parity
/// runs average traffic statistics over seeds): xored into the world
/// generator when the traffic takes it over (0 without it).
pub(crate) fn traffic_seed() -> u32 {
    debug_env().traffic_seed.clone().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

/// Stock cars loaded side by side, in `names`' order.
pub(crate) fn load_cars(root: &std::path::Path, names: &[&str]) -> Vec<Result<Car>> {
    std::thread::scope(|s| {
        let loads: Vec<_> = names.iter().map(|n| s.spawn(move || Car::load(root, n))).collect();
        loads.into_iter().map(|h| h.join().unwrap()).collect()
    })
}
