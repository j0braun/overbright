//! The desktop map editor's simulation (docs/modding/map-editor.md, "Simulate"):
//! `--map-mod <folder> --map-race <id> --simulate` (or `--map-file <path>`
//! for the editor's run copy) races the custom race
//! with the AI driving every car, the player's slot included (the AI racer
//! controller that drives the player's car past the line, `Autopilot`,
//! here from the grid); `--map-at x,y,z,heading --simulate-world` runs
//! free roam from the spot with its traffic and street racers, the
//! player's car parked. No window and no audio device are opened. Each
//! tick's cars and the race's events go to `--stream <file>` (`-` or no
//! flag: standard output) in the editor's stream format
//! (`nfsu2::tools::map::sim::format`). The run is deterministic for its seed
//! (`--sim-seed`, default 1). It ends at the race's results, after
//! `--sim-seconds` (default 900 for a race, 120 in free roam), or when
//! standard input reads `quit`. Nothing is saved.
use crate::*;

use nfsu2::tools::map::sim::detect::{Tick, Watch, checkpoints_passed};
use nfsu2::tools::map::sim::format::{
    CarInfo, CarKind, CarLook, CarSample, Drive, Event, EventKind, Frame, Header, LIGHT_BRAKE, LIGHT_HEAD, Line,
    ResultRow, SCHEMA, SCHEMA_DRIVE, SimMode, StateHash, VERSION, Wheels,
};

/// The stream's ids: the player's slot 0, opponents from 1, traffic from
/// 100, street racers from 200 (by their place in the game's lists).
const TRAFFIC_ID: u32 = 100;
const ENCOUNTER_ID: u32 = 200;
/// One update of the original's 60 fps cadence, ns.
const TICK_NS: u64 = 1_000_000_000 / 60;

/// The opt-in channels of a run file beyond the poses (`--run-channels drive,hash`, runs.md): all off writes the
/// file as before. Reading them changes nothing in the run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Channels {
    /// Each car's gear, rpm and pedals after its wheels (`ob.run/1.1`).
    pub(crate) drive: bool,
    /// The state hash every tick (the header's `hash=1`) instead of every 60.
    pub(crate) hash: bool,
}

impl Channels {
    /// `drive`, `hash` (a `,`-separated list); a usage error for anything else.
    pub(crate) fn parse(s: &str) -> std::result::Result<Channels, String> {
        let mut c = Channels::default();
        for w in s.split(',') {
            match w {
                "drive" => c.drive = true,
                "hash" => c.hash = true,
                _ => return Err(format!("--run-channels: `{w}` is not a channel (drive, hash)")),
            }
        }
        Ok(c)
    }

    /// The ticks between two state hashes.
    fn hash_every(self) -> u64 {
        if self.hash { 1 } else { StateHash::EVERY }
    }

    /// The schema id of a file written with these channels (1.0 when none: the file is as it was).
    fn schema(self) -> &'static str {
        if self.drive || self.hash { SCHEMA_DRIVE } else { SCHEMA }
    }
}

/// A run file being written by the window's test run.
pub(crate) struct Recorder {
    out: Streamer<Box<dyn std::io::Write>>,
    mode: SimMode,
}

/// What the run is asked for (`shell::cli::MapTest`'s simulate part).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SimRun {
    pub(crate) mode: SimMode,
    /// None or `-`: standard output.
    pub(crate) stream: Option<String>,
    pub(crate) seed: u64,
    pub(crate) seconds: Option<f32>,
}

/// One racer's run as last seen, for its events.
#[derive(Default)]
struct Seen {
    laps: usize,
    checkpoints: u32,
    finished: bool,
    watch: Watch,
}

/// The stream writer and what it remembers between ticks.
pub(crate) struct Streamer<W: std::io::Write> {
    out: W,
    tick: u64,
    named: std::collections::HashSet<u32>,
    /// Each car's look as last written.
    looks: HashMap<u32, CarLook>,
    seen: HashMap<u32, Seen>,
    /// The race's checkpoints on its road (m), its start, length, loops.
    road: Option<(Vec<f32>, f32, f32, bool)>,
    green: bool,
    /// The state hash over every tick so far, and the last tick it covers
    /// when that is not written yet.
    hash: StateHash,
    unwritten: Option<u64>,
    /// Events and frames written (the tests read them back).
    pub(crate) lines: u64,
    pub(crate) channels: Channels,
    /// A race tick was seen (a run that left for the results menu then ends as a finished race).
    raced: bool,
    /// Written by the window's test run: a pause menu is not the end of the run (only the results are).
    windowed: bool,
}

impl<W: std::io::Write> Streamer<W> {
    pub(crate) fn new(out: W) -> Streamer<W> {
        Streamer { out, tick: 0, named: Default::default(), looks: HashMap::new(), seen: HashMap::new(), road: None, green: false, hash: StateHash::default(), unwritten: None, lines: 0, channels: Channels::default(), raced: false, windowed: false }
    }

    pub(crate) fn with_channels(out: W, channels: Channels) -> Streamer<W> {
        Streamer { channels, ..Streamer::new(out) }
    }

    fn put(&mut self, l: &Line) -> Result<()> {
        writeln!(self.out, "{}", l.write())?;
        self.lines += 1;
        Ok(())
    }

    pub(crate) fn header(&mut self, h: Header) -> Result<()> {
        self.put(&Line::Header(h))
    }

    /// The state hash after the last tick, when not written yet.
    fn last_hash(&mut self) -> Result<()> {
        if let Some(tick) = self.unwritten.take() {
            let hash = self.hash;
            self.put(&Line::Hash { tick, hash })?;
        }
        Ok(())
    }

    pub(crate) fn end(&mut self, why: &str) -> Result<()> {
        self.last_hash()?;
        self.put(&Line::End(why.to_string()))?;
        self.out.flush()?;
        Ok(())
    }

    /// This tick's cars and events.
    pub(crate) fn frame(&mut self, game: &Game, dt: f32) -> Result<()> {
        let mut cars: Vec<(CarSample, String, CarLook, Exact)> = Vec::new();
        let mut events = Vec::new();
        let drive_on = self.channels.drive;
        let sample = |id: u32, kind: CarKind, car: &Car, st: &CarState| {
            (
                CarSample {
                    id,
                    kind,
                    position: st.position,
                    rotation: st.orientation,
                    speed: st.velocity.length(),
                    lap: 0,
                    checkpoint: 0,
                    place: 0,
                    wheels: Some(wheels(kind, car, st)),
                    drive: drive_on.then(|| drive(st)),
                },
                look(id, car),
                (st.velocity, st.spin),
            )
        };
        let time;
        match &game.mode {
            Mode::Race(r) => {
                time = r.timing.clock;
                let road = self.road.get_or_insert_with(|| {
                    let rd = &r.route.road;
                    let cps = r.route.checkpoints.iter().map(|&c| rd.locate_once(c)).collect();
                    (cps, r.route.start_at, rd.length, rd.loops)
                });
                let road = road.clone();
                if !self.green && time >= 0.0 {
                    self.green = true;
                    events.push(Event {
                        tick: self.tick,
                        time,
                        kind: EventKind::Start,
                        car: 0,
                        checkpoint: 0,
                        value: 0.0,
                    });
                }
                let place = |racer: usize| r.order.iter().position(|&k| k == racer).map_or(0, |p| p as u32 + 1);
                let mut racers: Vec<(u32, CarKind, &Car, &CarState, &Progress, String)> = Vec::new();
                if let Some(d) = game.sim.drive.as_ref() {
                    racers.push((0, CarKind::Player, &d.car, &d.state, &r.racers.me, d.car.car_type.name.clone()));
                }
                for (i, rv) in r.racers.rivals.iter().enumerate() {
                    racers.push((i as u32 + 1, CarKind::Racer, &rv.car, &rv.state, &rv.progress, rv.name.clone()));
                }
                for (id, kind, car, st, prog, name) in racers {
                    let (mut s, look, exact) = sample(id, kind, car, st);
                    let (lap, _) = prog.lap();
                    s.lap = lap.max(0);
                    s.checkpoint = checkpoints_passed(&road.0, road.1, road.2, prog.along, road.3);
                    s.place = place(id as usize);
                    let seen = self.seen.entry(id).or_default();
                    let racing = time > 0.0 && prog.finished.is_none();
                    if racing && s.checkpoint > seen.checkpoints {
                        events.push(Event {
                            tick: self.tick,
                            time,
                            kind: EventKind::Checkpoint,
                            car: id,
                            checkpoint: s.checkpoint,
                            value: 0.0,
                        });
                    }
                    if racing || s.checkpoint < seen.checkpoints {
                        seen.checkpoints = s.checkpoint;
                    }
                    while seen.laps < prog.lap_times.len() {
                        events.push(Event {
                            tick: self.tick,
                            time,
                            kind: EventKind::Lap,
                            car: id,
                            checkpoint: seen.laps as u32 + 1,
                            value: prog.lap_times[seen.laps],
                        });
                        seen.laps += 1;
                    }
                    if let Some(t) = prog.finished
                        && !seen.finished
                    {
                        seen.finished = true;
                        events.push(Event {
                            tick: self.tick,
                            time,
                            kind: EventKind::Finish,
                            car: id,
                            checkpoint: 0,
                            value: t,
                        });
                    }
                    let forward = st.forward().truncate().normalize_or_zero();
                    let along_road = (!road.0.is_empty() || r.route.road.length > 0.0)
                        .then(|| r.route.road.direction(prog.along).dot(forward));
                    let next = (s.checkpoint + 1).min(road.0.len() as u32);
                    events.extend(seen.watch.step(Tick {
                        tick: self.tick,
                        time,
                        dt,
                        car: id,
                        speed: s.speed,
                        along_road,
                        racing,
                        next_checkpoint: next,
                    }));
                    cars.push((s, name, look, exact));
                }
                if let Some(t) = r.racers.traffic.as_ref() {
                    traffic_cars(t, &mut cars, sample);
                }
            }
            _ => {
                time = self.tick as f32 / 60.0;
                if let Some(d) = game.sim.drive.as_ref() {
                    let (s, look, exact) = sample(0, CarKind::Player, &d.car, &d.state);
                    cars.push((s, d.car.car_type.name.clone(), look, exact));
                }
                if let Some(t) = game.sim.traffic.as_ref() {
                    traffic_cars(t, &mut cars, sample);
                }
                if let Some(e) = game.sim.encounters.as_ref() {
                    for (i, c) in e.cars.iter().enumerate() {
                        let (s, look, exact) = sample(ENCOUNTER_ID + i as u32, CarKind::Encounter, &c.car, &c.state);
                        cars.push((s, c.car.car_type.name.clone(), look, exact));
                    }
                }
            }
        }
        for (s, name, look, (velocity, spin)) in &cars {
            self.hash.car(s.id, s.position, s.rotation, *velocity, *spin);
            if self.named.insert(s.id) {
                self.put(&Line::Car(CarInfo { id: s.id, kind: s.kind, name: name.clone() }))?;
            }
            // The look before the car's first tick and whenever it changes
            // (a traffic slot taking another model).
            if self.looks.get(&s.id).is_none_or(|l| !l.same_car(look)) {
                self.put(&Line::Look(look.clone()))?;
                self.looks.insert(s.id, look.clone());
            }
        }
        self.hash.tick(self.tick);
        self.unwritten = Some(self.tick);
        let frame = Frame { tick: self.tick, time, cars: cars.into_iter().map(|c| c.0).collect() };
        self.put(&Line::Frame(frame))?;
        for e in events {
            self.put(&Line::Event(e))?;
        }
        if (self.tick + 1).is_multiple_of(self.channels.hash_every()) {
            self.last_hash()?;
        }
        self.tick += 1;
        Ok(())
    }

    /// One finished update of `game`: its frame written, and `Some(why)` when the run is over (every racer
    /// through, or the race left for its results). The simulation's loop and the window's recorder both use it.
    pub(crate) fn step(&mut self, game: &Game, mode: SimMode, dt: f32) -> Result<Option<&'static str>> {
        match &game.mode {
            Mode::Race(r) => {
                self.raced = true;
                self.frame(game, dt)?;
                // Every racer through (or the race over): the results.
                let all =
                    r.racers.me.finished.is_some() && r.racers.rivals.iter().all(|x| x.progress.finished.is_some());
                if all && r.over > 1.0 {
                    self.results(r)?;
                    return Ok(Some("finish"));
                }
            }
            Mode::Explore if mode == SimMode::World => self.frame(game, dt)?,
            Mode::Explore => {}
            Mode::Menu(m) if self.raced && (!self.windowed || matches!(m, Menu::Results(..))) => {
                // The race left for its results before every rival was through: the race as the results took it.
                if let Some(r) = game.sim.paused_race.as_ref().or(game.sim.replay.as_ref()) {
                    self.results(r)?;
                }
                return Ok(Some("finish"));
            }
            Mode::Menu(_) => {}
        }
        Ok(None)
    }

    /// The racers' results from the race as it ended.
    pub(crate) fn results(&mut self, r: &Race) -> Result<()> {
        self.last_hash()?;
        let runs: Vec<(u32, &Progress)> = std::iter::once((0, &r.racers.me))
            .chain(r.racers.rivals.iter().enumerate().map(|(i, rv)| (i as u32 + 1, &rv.progress)))
            .collect();
        let mut finished: Vec<(u32, f32)> = runs.iter().filter_map(|(id, p)| p.finished.map(|t| (*id, t))).collect();
        finished.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        for (id, p) in runs {
            let place = finished.iter().position(|f| f.0 == id).map_or(0, |k| k as u32 + 1);
            self.put(&Line::Result(ResultRow {
                car: id,
                place,
                time: p.finished,
                best: p.best_lap,
                laps: p.lap_times.clone(),
            }))?;
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<()> {
        Ok(self.out.flush()?)
    }
}

/// A car's velocity and spin, exact (the state hash's; the stream rounds
/// the pose).
type Exact = (Vec3, Vec3);

/// The traffic cars on the road (placed ones only).
fn traffic_cars(
    t: &Traffic,
    cars: &mut Vec<(CarSample, String, CarLook, Exact)>,
    sample: impl Fn(u32, CarKind, &Car, &CarState) -> (CarSample, CarLook, Exact),
) {
    for (i, c) in t.cars.iter().enumerate() {
        if c.lane.is_some() {
            let (s, look, exact) = sample(TRAFFIC_ID + i as u32, CarKind::Traffic, &c.car, &c.state);
            cars.push((s, c.car.car_type.name.clone(), look, exact));
        }
    }
}

/// What builds the car again: its model and fitted visual parts.
fn look(id: u32, car: &Car) -> CarLook {
    CarLook {
        id,
        model: car.name.clone(),
        camber_bits: car.camber.to_bits(),
        parts: car.look_parts.iter().map(|(&slot, &hash)| (slot, hash)).collect(),
    }
}

/// The car's wheels as drawn and its lights: every car's headlights are on
/// (+0x93D, `render::native::cars::headlight_pools`); the brake lights'
/// flag holds 0.5 s for the racers (owner kinds 2 and 4), not for the
/// player's slot or the traffic (`CarState::brake_lights`).
fn wheels(kind: CarKind, car: &Car, st: &CarState) -> Wheels {
    let p = st.wheel_poses(car);
    let holds = matches!(kind, CarKind::Racer | CarKind::Encounter);
    Wheels {
        steer: [p[0].steer, p[1].steer],
        roll: std::array::from_fn(|i| p[i].roll.rem_euclid(std::f32::consts::TAU)),
        lift: std::array::from_fn(|i| p[i].lift),
        lights: LIGHT_HEAD | if st.brake_lights(holds) { LIGHT_BRAKE } else { 0 },
    }
}

/// The car's gearbox, engine and pedals as its last update took them (`ob.run/1.1`'s drive channels; read only).
fn drive(st: &CarState) -> Drive {
    let c = st.last_controls();
    Drive {
        gear: st.gear_number(),
        rpm: st.shown_rpm(),
        throttle: c.throttle,
        brake: c.brake,
        handbrake: c.handbrake,
        steer: c.steer,
        nitrous: c.nitrous,
    }
}

/// `quit` on standard input (a thread reads it; nothing else is read).
fn quit_watch() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let f = flag.clone();
    let _ = std::thread::Builder::new().name("sim stdin".into()).spawn(move || {
        let mut line = String::new();
        while std::io::stdin().read_line(&mut line).is_ok_and(|n| n > 0) {
            if line.trim() == "quit" {
                f.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            line.clear();
        }
    });
    flag
}

impl Game {
    /// The player's slot driven by the AI racer controller from the grid
    /// (once the race and its routes are there).
    fn sim_autopilot(&mut self) {
        if let (Mode::Race(r), Some(d)) = (&mut self.mode, self.sim.drive.as_ref())
            && r.autopilot.is_none()
            && let Some(routes) = r.racers.ai_routes.as_ref()
        {
            let drift = d.car.car_type.handling.race.drift();
            let mut a = Autopilot::new(d, routes, drift);
            a.racing = true;
            r.autopilot = Some(a);
        }
    }

    /// The run file's first line for the run `t` has just started.
    fn run_header(&self, t: &crate::shell::cli::MapTest, mode: SimMode, seed: u64, channels: Channels) -> Header {
        let mut header = Header {
            version: VERSION,
            schema: channels.schema().into(),
            mode,
            seed,
            hash_every: if channels.hash { 1 } else { 0 },
            ..Default::default()
        };
        if let Some(id) = &t.race
            && let Mode::Race(r) = &self.mode
        {
            header.race = id.clone();
            header.track = self.sim.map_race.as_ref().map_or(0, |m| m.track);
            header.laps = r.laps;
            header.checkpoints = r.route.checkpoints.len() as u32;
        }
        header
    }

    /// `--record <file>`: the window's test run writes its run file as it is driven, a frame per update
    /// (`record_tick`; the same file as a simulation's, the player's slot driven by its player).
    pub(crate) fn start_recording(&mut self, t: &crate::shell::cli::MapTest, path: &std::path::Path, seed: u64) -> Result<()> {
        let file = std::fs::File::create(path).with_context(|| format!("--record {}", path.display()))?;
        let mut s = Streamer::with_channels(Box::new(std::io::LineWriter::new(file)) as Box<dyn std::io::Write>, t.channels);
        s.windowed = true;
        let mode = if t.race.is_some() { SimMode::Race } else { SimMode::World };
        s.header(self.run_header(t, mode, seed, t.channels))?;
        self.recorder = Some(Recorder { out: s, mode });
        Ok(())
    }

    /// After each update (`advance_tick`): the recording's frame. Reads the game, changes nothing in it; a
    /// recording that cannot be written stops with a log line, the run goes on.
    pub(crate) fn record_tick(&mut self, dt: f32) {
        let Some(mut rec) = self.recorder.take() else { return };
        match rec.out.step(self, rec.mode, dt) {
            Ok(None) => self.recorder = Some(rec),
            Ok(Some(why)) => {
                let _ = rec.out.end(why);
            }
            Err(e) => log::warn!("record: stopped: {e:#}"),
        }
    }

    /// Close the recording (the run was left, restarted or the window closed); nothing when none is open.
    pub(crate) fn end_recording(&mut self, why: &str) {
        if let Some(mut rec) = self.recorder.take() {
            let _ = rec.out.end(why);
        }
    }

    /// Run the simulation `t` asks for into `out`; `quit` stops it early.
    /// Returns why it ended.
    pub(crate) fn simulate<W: std::io::Write>(
        &mut self,
        t: &crate::shell::cli::MapTest,
        run: &SimRun,
        out: &mut Streamer<W>,
        quit: &dyn Fn() -> bool,
    ) -> Result<&'static str> {
        // The run's random numbers from its seed (0x57ECC0's boot seed),
        // and the start-up's draws after it, as a game start.
        self.rng = nfsu2::opponents::GameRng::boot(run.seed);
        self.boot_draws();
        self.start_map_test(t)?;
        out.header(self.run_header(t, run.mode, run.seed, out.channels))?;
        let limit = run.seconds.unwrap_or(if run.mode == SimMode::Race { 900.0 } else { 120.0 });
        let mut world = 0.0f32;
        loop {
            if quit() {
                return Ok("quit");
            }
            if world >= limit {
                return Ok("limit");
            }
            if run.mode == SimMode::Race {
                self.sim_autopilot();
            }
            self.advance_ns(TICK_NS)?;
            let dt = self.pacing.frame_world;
            world += dt.max(1.0 / 120.0);
            if let Some(why) = out.step(self, run.mode, dt)? {
                return Ok(why);
            }
        }
    }
}

/// `game … --simulate` / `--simulate-world`: the run, headless, its
/// stream on standard output or into the file.
pub(crate) fn run(game: &mut Game, t: &crate::shell::cli::MapTest, run: &SimRun) -> Result<()> {
    let quit = quit_watch();
    let sink: Box<dyn std::io::Write> = match run.stream.as_deref() {
        None | Some("-") => Box::new(std::io::LineWriter::new(std::io::stdout())),
        Some(path) => {
            Box::new(std::io::BufWriter::new(std::fs::File::create(path).with_context(|| format!("--stream {path}"))?))
        }
    };
    let mut s = Streamer::with_channels(sink, t.channels);
    // Studio's link (`--studio-link`): the run answers a probe and ends on a
    // stop command, besides `quit` on standard input.
    let link = std::cell::RefCell::new(t.link.clone().map(crate::shell::studio_link::Link::new));
    let stop = || quit.load(std::sync::atomic::Ordering::Relaxed) || link.borrow_mut().as_mut().is_some_and(|l| l.poll_headless());
    let why = match game.simulate(t, run, &mut s, &stop) {
        Ok(why) => why.to_string(),
        Err(e) => format!("error {e:#}"),
    };
    log::info!("simulate: ended ({why}), {} lines", s.lines);
    s.end(&why)?;
    s.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfsu2::tools::map::{CustomRace, MapMod, RaceKind, store};

    /// A mod in this test's data folder with a short sprint over the first
    /// three checkpoints of a career sprint's track.
    fn sprint_mod(game: &Game, folder: &str) {
        sprint_mod_with(game, folder, Vec::new(), Vec::new());
    }

    /// [`sprint_mod`] with the scenery edits `props`.
    fn sprint_mod_with(
        game: &Game,
        folder: &str,
        props: Vec<nfsu2::tools::map::PropEdit>,
        collision: Vec<nfsu2::tools::map::CollisionEdit>,
    ) {
        let event = game
            .game_data
            .career
            .events
            .iter()
            .find(|e| e.kind == career::EventKind::Sprint && e.races.first().is_none_or(|r| r.1 & 1 == 0))
            .expect("a sprint")
            .clone();
        let number = event.races.first().map_or(event.track, |r| r.0);
        let track = game.game_data.career.tracks[&number].clone();
        let native = career::load_route(&game.game_data.root, &track, false).unwrap();
        let mut race = CustomRace::new("short", number, RaceKind::Sprint);
        race.checkpoints = native.checkpoints[..3].to_vec();
        let m = MapMod { name: "Sim".into(), races: vec![race], props, collision, ..Default::default() };
        let dir = store::maps_dir();
        store::save(&dir, folder, &m).unwrap();
        store::set_enabled(&dir, folder, true).unwrap();
        nfsu2::tools::map::reload_active();
    }

    fn simulate(seed: u64, seconds: f32) -> (String, &'static str) {
        simulate_with(seed, seconds, "sim-run", Vec::new(), Vec::new())
    }

    /// [`simulate`] of the sprint with the scenery edits `props`, in `folder`.
    fn simulate_with(
        seed: u64,
        seconds: f32,
        folder: &str,
        props: Vec<nfsu2::tools::map::PropEdit>,
        collision: Vec<nfsu2::tools::map::CollisionEdit>,
    ) -> (String, &'static str) {
        // The mod is written before the game that runs it starts (its
        // start preloads the region with the mods as they are then).
        sprint_mod_with(&Game::new().unwrap(), folder, props, collision);
        let mut game = Game::new().unwrap();
        game.profile = None;
        let t = crate::shell::cli::MapTest { folder: folder.into(), race: Some("short".into()), ..Default::default() };
        let run = SimRun { mode: SimMode::Race, stream: None, seed, seconds: Some(seconds) };
        let mut s = Streamer::new(Vec::new());
        let why = game.simulate(&t, &run, &mut s, &|| false).unwrap();
        s.end(why).unwrap();
        (String::from_utf8(s.out).unwrap(), why)
    }

    /// A free-roam simulation from `at` (heading in degrees) for `seconds`:
    /// the player's parked car's position each tick, or why it could not
    /// start.
    fn parked(at: Vec3, heading: f32, seconds: f32) -> std::result::Result<Vec<Vec3>, String> {
        let mut game = Game::new().unwrap();
        sprint_mod(&game, "sim-roam");
        game.profile = None;
        let t = crate::shell::cli::MapTest { folder: "sim-roam".into(), at: Some((at, heading)), ..Default::default() };
        let run = SimRun { mode: SimMode::World, stream: None, seed: 1, seconds: Some(seconds) };
        let mut s = Streamer::new(Vec::new());
        game.simulate(&t, &run, &mut s, &|| false).map_err(|e| format!("{e:#}"))?;
        let text = String::from_utf8(s.out).unwrap();
        Ok(text
            .lines()
            .filter_map(|l| Line::parse(l).unwrap())
            .filter_map(|l| {
                if let Line::Frame(f) = l { f.cars.iter().find(|c| c.id == 0).map(|c| c.position) } else { None }
            })
            .collect())
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_world_simulation_parks_the_car_on_the_ground_or_refuses_the_spot() {
        // Bayview just past a road's edge, where the region's collision has
        // no floor at any height: the parked car fell through the world
        // (z -189 after 30 s). Refused now.
        let off = parked(Vec3::new(-664.0, 769.0, 32.5), 200.0, 30.0).unwrap_err();
        assert!(off.contains("no ground"), "{off}");
        // 10 m south, on the road: parked, and it stays there.
        let at = Vec3::new(-664.0, 759.0, 32.5);
        let track = parked(at, 200.0, 10.0).unwrap();
        let last = *track.last().unwrap();
        let first = track[0];
        assert!(
            track.len() > 500 && (last.z - first.z).abs() < 1.0 && last.truncate().distance(at.truncate()) < 3.0,
            "{first} → {last}"
        );
    }

    /// The racers' cars (the player's slot and the opponents) each tick.
    fn racers(stream: &str) -> Vec<Vec<CarSample>> {
        stream
            .lines()
            .filter_map(|l| Line::parse(l).unwrap())
            .filter_map(|l| if let Line::Frame(f) = l { Some(f) } else { None })
            .filter(|f| f.time >= 0.0)
            .map(|f| f.cars.into_iter().filter(|c| matches!(c.kind, CarKind::Player | CarKind::Racer)).collect())
            .collect()
    }

    /// The editor's simulation with real cars: original street lights the
    /// mod moves into a row across the sprint's road stop every racer there
    /// (the AI drives them all), where the same race without the mod passes.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn moved_street_lights_stop_the_simulated_racers_where_they_now_stand() {
        let (plain, _) = simulate_with(3, 25.0, "sim-lights", Vec::new(), Vec::new());
        let ticks = racers(&plain);
        let player = |t: &Vec<CarSample>| t.iter().find(|c| c.id == 0).map(|c| c.position);
        let start = player(&ticks[0]).unwrap();
        let at = |d: f32| ticks.iter().filter_map(player).find(|p| p.truncate().distance(start.truncate()) > d);
        let (line, ahead) = (at(80.0).expect("the player's slot drives 80 m"), at(90.0).unwrap());
        let dir = (ahead - line).truncate().normalize().extend(0.0);
        let across = Vec3::new(-dir.y, dir.x, 0.0);
        let past = |ticks: &[Vec<CarSample>]| {
            ticks.iter().flatten().map(|c| (c.position - line).dot(dir)).fold(f32::MIN, f32::max)
        };
        assert!(past(&ticks) > 20.0, "the racers pass the spot without the mod");

        // Street lights with collision of their own, moved 1 m apart across the road.
        let world = nfsu2::world::World::load(&nfsu2::game_root(), "L4RA").unwrap();
        let light = world.solid_named("XO_STREETLIGHTD_1A_00").unwrap();
        let props: Vec<nfsu2::tools::map::PropEdit> = (0..world.placements.len())
            .filter(|&i| {
                let p = &world.placements[i];
                p.solid == light && !world.collision_of_placement(light, p.transform).is_empty()
            })
            .take(41)
            .enumerate()
            .map(|(k, i)| nfsu2::tools::map::PropEdit::Move {
                target: world.placement_ref("L4RA", i).unwrap(),
                position: line + across * (k as f32 - 20.0) - Vec3::Z * 0.6,
                rotation: glam::Mat3::IDENTITY,
            })
            .collect();
        assert_eq!(props.len(), 41);
        let (walled, _) = simulate_with(3, 25.0, "sim-lights", props, Vec::new());
        let ticks = racers(&walled);
        assert!(ticks.len() > 600 && ticks[0].len() >= 2, "{} ticks", ticks.len());
        let got = past(&ticks);
        eprintln!("furthest past the row: without the lights {:.1} m, with them {got:.2} m", past(&racers(&plain)));
        assert!(got < 0.0, "a racer's centre passed the moved lights ({got} m)");
    }

    /// The editor's simulation with real cars: a wall the mod draws across
    /// the sprint's road (two points, 2 m high) stops every racer there
    /// (the AI drives them all); the same race without it passes.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_drawn_wall_stops_the_simulated_racers() {
        let (plain, _) = simulate_with(3, 25.0, "sim-wall", Vec::new(), Vec::new());
        let ticks = racers(&plain);
        let player = |t: &Vec<CarSample>| t.iter().find(|c| c.id == 0).map(|c| c.position);
        let start = player(&ticks[0]).unwrap();
        let at = |d: f32| ticks.iter().filter_map(player).find(|p| p.truncate().distance(start.truncate()) > d);
        let (line, ahead) = (at(80.0).expect("the player's slot drives 80 m"), at(90.0).unwrap());
        let dir = (ahead - line).truncate().normalize().extend(0.0);
        let across = Vec3::new(-dir.y, dir.x, 0.0);
        let past = |ticks: &[Vec<CarSample>]| {
            ticks.iter().flatten().map(|c| (c.position - line).dot(dir)).fold(f32::MIN, f32::max)
        };
        assert!(past(&ticks) > 20.0, "the racers pass the spot without the wall");
        let foot = line - Vec3::Z * 0.6;
        let wall = nfsu2::tools::map::CollisionEdit::Wall {
            id: "wall-1".into(),
            region: "L4RA".into(),
            from: foot - across * 25.0,
            to: foot + across * 25.0,
            height: 2.0,
            surface: 16,
        };
        let (walled, _) = simulate_with(3, 25.0, "sim-wall", Vec::new(), vec![wall]);
        let ticks = racers(&walled);
        assert!(ticks.len() > 600 && ticks[0].len() >= 2, "{} ticks", ticks.len());
        let got = past(&ticks);
        eprintln!("furthest past the drawn wall: without it {:.1} m, with it {got:.2} m", past(&racers(&plain)));
        assert!(got < 0.0, "a racer's centre passed the drawn wall ({got} m)");
    }

    /// `simulate` with the channels `c` on.
    fn simulate_channels(seed: u64, seconds: f32, c: Channels) -> String {
        sprint_mod_with(&Game::new().unwrap(), "sim-run", Vec::new(), Vec::new());
        let mut game = Game::new().unwrap();
        game.profile = None;
        let t = crate::shell::cli::MapTest { folder: "sim-run".into(), race: Some("short".into()), channels: c, ..Default::default() };
        let run = SimRun { mode: SimMode::Race, stream: None, seed, seconds: Some(seconds) };
        let mut s = Streamer::with_channels(Vec::new(), c);
        let why = game.simulate(&t, &run, &mut s, &|| false).unwrap();
        s.end(why).unwrap();
        String::from_utf8(s.out).unwrap()
    }

    /// A car of an `F` line without its drive fields (the first 24 words: the pose, the wheels and the lights).
    fn without_drive(line: &str) -> String {
        match line.strip_prefix("F ") {
            None => line.to_string(),
            Some(f) => {
                let mut w = f.splitn(3, ' ');
                let (tick, time, cars) = (w.next().unwrap(), w.next().unwrap(), w.next().unwrap_or(""));
                let cars: Vec<String> =
                    cars.split('|').map(|c| c.split(' ').take(24).collect::<Vec<_>>().join(" ")).collect();
                format!("F {tick} {time} {}", cars.join("|"))
            }
        }
    }

    /// The drive channels and the hash of every tick are read from the game and change nothing in it: the same
    /// seed writes the same poses, events and hashes (at the ticks both write) with them on and off, and the file
    /// with them says what it is (runs.md 3.3).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_channels_change_nothing_in_the_run() {
        let off = simulate_channels(7, 30.0, Channels::default());
        let on = simulate_channels(7, 30.0, Channels { drive: true, hash: true });
        assert!(off.starts_with("OBSIM 1 schema=ob.run/1.0 ") && !off.lines().next().unwrap().contains("hash="), "{}", off.lines().next().unwrap());
        assert!(on.lines().next().unwrap().starts_with("OBSIM 1 schema=ob.run/1.1 ") && on.lines().next().unwrap().ends_with(" hash=1"));
        let sum = ob_asset_format::kinds::run::check_run(&on).unwrap();
        assert_eq!((sum.schema.as_str(), sum.ended), ("ob.run/1.1", true));
        // Every line but the header, the drive fields and the extra hashes is the same.
        let keep = |s: &str| -> Vec<String> {
            s.lines().skip(1).filter(|l| !l.starts_with("H ")).map(without_drive).collect()
        };
        assert!(keep(&off) == keep(&on), "poses, events and results are the same with the channels on");
        let hashes = |s: &str| -> Vec<(u64, String)> {
            s.lines().filter_map(|l| l.strip_prefix("H ")).map(|h| h.split_once(' ').map(|(t, x)| (t.parse().unwrap(), x.to_string())).unwrap()).collect()
        };
        let (h_off, h_on) = (hashes(&off), hashes(&on));
        assert!(h_on.len() > 1000 && h_off.len() < h_on.len() / 30, "{} / {} hashes", h_off.len(), h_on.len());
        for h in &h_off {
            assert!(h_on.contains(h), "the hash at tick {} is the same with the hash every tick", h.0);
        }
        // The player's slot, driven by the AI: its gear and pedals are in the file.
        let drive: Vec<CarSample> = on
            .lines()
            .filter_map(|l| Line::parse(l).unwrap())
            .filter_map(|l| if let Line::Frame(f) = l { f.cars.into_iter().find(|c| c.id == 0) } else { None })
            .collect();
        let d = |c: &CarSample| c.drive.expect("the player's slot has the drive channels");
        assert!(drive.iter().any(|c| d(c).gear >= 1 && d(c).throttle > 0.5), "it drove in gear with the throttle down");
        let bad = drive.iter().find(|c| !((0.0..=1.0).contains(&d(c).throttle) && (d(c).gear < 1 || d(c).rpm > 0.0)));
        assert!(bad.is_none(), "{:?}", bad.map(d));
        assert!(drive.iter().map(|c| d(c).gear).collect::<std::collections::BTreeSet<_>>().len() >= 2, "it shifted");
    }

    /// The window's recording (`--record`): a run of `seconds` at `fps` frames a second, with the recorder on
    /// (its file) or off; and the player's car at the end (position, velocity: exact bits).
    fn recorded(fps: f32, seconds: f32, record: bool) -> (String, [u32; 6]) {
        sprint_mod_with(&Game::new().unwrap(), "rec-run", Vec::new(), Vec::new());
        let mut game = Game::new().unwrap();
        game.profile = None;
        let c = Channels { drive: true, hash: true };
        let t = crate::shell::cli::MapTest { folder: "rec-run".into(), race: Some("short".into()), channels: c, ..Default::default() };
        game.rng = nfsu2::opponents::GameRng::boot(5);
        game.boot_draws();
        game.start_map_test(&t).unwrap();
        let path = std::env::temp_dir().join(format!("ob-record-{}-{fps}-{record}.obrun", std::process::id()));
        if record {
            game.start_recording(&t, &path, 5).unwrap();
        }
        for _ in 0..(seconds * fps) as usize {
            game.advance(1.0 / fps).unwrap();
        }
        game.end_recording("quit");
        let st = &game.sim.drive.as_ref().expect("the player's car").state;
        let bits = [st.position.x, st.position.y, st.position.z, st.velocity.x, st.velocity.y, st.velocity.z].map(f32::to_bits);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        (text, bits)
    }

    /// Drive to record: the window's run written as it is played is one file whatever the frame rate, is a valid
    /// run file with the channels, and recording leaves the simulation exactly as it is without it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_recorded_window_run_is_the_same_at_any_frame_rate_and_leaves_the_run_alone() {
        let (base, base_bits) = recorded(60.0, 12.0, true);
        let sum = ob_asset_format::kinds::run::check_run(&base).unwrap();
        assert!(sum.records.iter().find(|r| r.0 == "F").unwrap().1 > 300, "{:?}", sum.records);
        assert_eq!(sum.ended, true);
        assert!(base.lines().next().unwrap().contains("schema=ob.run/1.1") && base.contains(" hash=1"));
        let (_, off_bits) = recorded(60.0, 12.0, false);
        assert_eq!(base_bits, off_bits, "the player's car is the same with the recording on and off");
        let frames = |t: &str| -> Vec<String> { t.lines().filter(|l| l.starts_with("F ") || l.starts_with("H ")).map(String::from).collect() };
        let a = frames(&base);
        for fps in [30.0, 120.0, 200.0] {
            let (text, _) = recorded(fps, 12.0, true);
            let b = frames(&text);
            let n = a.len().min(b.len());
            assert!(n > 300, "{fps} fps: {n} lines");
            assert!(a[..n] == b[..n], "{fps} fps: the same frames and hashes as at 60");
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_simulated_race_streams_every_car_and_repeats_for_its_seed() {
        let (a, why) = simulate(7, 40.0);
        let lines: Vec<Line> = a.lines().filter_map(|l| Line::parse(l).unwrap()).collect();
        let Some(Line::Header(h)) = lines.first() else { panic!("the header first: {}", &a[..200.min(a.len())]) };
        assert_eq!((h.race.as_str(), h.checkpoints, h.seed), ("short", 3, 7));
        assert!(matches!(lines.last(), Some(Line::End(_))));
        let frames: Vec<&Frame> =
            lines.iter().filter_map(|l| if let Line::Frame(f) = l { Some(f) } else { None }).collect();
        assert!(frames.len() > 600, "{} frames", frames.len());
        let last = frames.last().unwrap();
        assert!(last.cars.iter().any(|c| c.kind == CarKind::Player));
        assert!(last.cars.iter().filter(|c| c.kind == CarKind::Racer).count() >= 1);
        // The AI drove the player's slot off the grid.
        let first = frames.iter().find(|f| f.time >= 0.0).unwrap().cars.iter().find(|c| c.id == 0).unwrap().position;
        let now = last.cars.iter().find(|c| c.id == 0).unwrap().position;
        assert!(first.distance(now) > 100.0, "the player's slot moved {} m", first.distance(now));
        assert!(lines.iter().any(|l| matches!(l, Line::Event(e) if e.kind == EventKind::Start)));
        assert!(
            lines.iter().any(|l| matches!(l, Line::Event(e) if e.kind == EventKind::Checkpoint)),
            "a checkpoint passed ({why})"
        );
        // Every car's look before its first tick, with its model and its
        // wheels each tick.
        let looks: Vec<&CarLook> =
            lines.iter().filter_map(|l| if let Line::Look(k) = l { Some(k) } else { None }).collect();
        for c in &last.cars {
            let k = looks.iter().rev().find(|k| k.id == c.id).expect("a look for every car");
            assert!(!k.model.is_empty());
            assert!(c.wheels.is_some_and(|w| w.lights & LIGHT_HEAD != 0), "car {} has its wheels and lights", c.id);
        }
        let player = looks.iter().find(|k| k.id == 0).unwrap();
        assert!(!player.parts.is_empty(), "the player's car's parts: {player:?}");
        // The asset format's run file, with its state hashes.
        let sum = ob_asset_format::kinds::run::check_run(&a).unwrap();
        assert_eq!((sum.schema.as_str(), sum.ended), (SCHEMA, true));
        let hashes: Vec<u64> =
            lines.iter().filter_map(|l| if let Line::Hash { tick, .. } = l { Some(*tick) } else { None }).collect();
        assert!(hashes.len() as u64 >= frames.len() as u64 / StateHash::EVERY, "{} hashes", hashes.len());
        assert_eq!(hashes.last(), Some(&last.tick), "the hash after the last tick");
        // The same seed: the same stream, byte for byte.
        let (b, _) = simulate(7, 40.0);
        assert!(a == b, "the stream repeats for its seed");
        // OB_SIM_DUMP=<file>: the whole race streamed there (a sample for
        // the editor's `--sim`).
        if let Some(path) = std::env::var_os("OB_SIM_DUMP") {
            let (full, why) = simulate(7, 900.0);
            std::fs::write(&path, &full).unwrap();
            let mod_file = store::maps_dir().join("sim-run").join(store::FILE);
            std::fs::copy(mod_file, std::path::Path::new(&path).with_extension("obmap")).unwrap();
            assert_eq!(why, "finish", "the race runs to its end");
        }
    }
}
