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
    CarInfo, CarKind, CarLook, CarSample, Event, EventKind, Frame, Header, LIGHT_BRAKE, LIGHT_HEAD, Line, ResultRow,
    SimMode, VERSION, Wheels,
};

/// The stream's ids: the player's slot 0, opponents from 1, traffic from
/// 100, street racers from 200 (by their place in the game's lists).
const TRAFFIC_ID: u32 = 100;
const ENCOUNTER_ID: u32 = 200;
/// One update of the original's 60 fps cadence, ns.
const TICK_NS: u64 = 1_000_000_000 / 60;

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
    /// Events and frames written (the tests read them back).
    pub(crate) lines: u64,
}

impl<W: std::io::Write> Streamer<W> {
    pub(crate) fn new(out: W) -> Streamer<W> {
        Streamer { out, tick: 0, named: Default::default(), looks: HashMap::new(), seen: HashMap::new(), road: None, green: false, lines: 0 }
    }

    fn put(&mut self, l: &Line) -> Result<()> {
        writeln!(self.out, "{}", l.write())?;
        self.lines += 1;
        Ok(())
    }

    pub(crate) fn header(&mut self, h: Header) -> Result<()> {
        self.put(&Line::Header(h))
    }

    pub(crate) fn end(&mut self, why: &str) -> Result<()> {
        self.put(&Line::End(why.to_string()))?;
        self.out.flush()?;
        Ok(())
    }

    /// This tick's cars and events.
    pub(crate) fn frame(&mut self, game: &Game, dt: f32) -> Result<()> {
        let mut cars: Vec<(CarSample, String, CarLook)> = Vec::new();
        let mut events = Vec::new();
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
                },
                look(id, car),
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
                    let (mut s, look) = sample(id, kind, car, st);
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
                    cars.push((s, name, look));
                }
                if let Some(t) = r.racers.traffic.as_ref() {
                    traffic_cars(t, &mut cars, sample);
                }
            }
            _ => {
                time = self.tick as f32 / 60.0;
                if let Some(d) = game.sim.drive.as_ref() {
                    let (s, look) = sample(0, CarKind::Player, &d.car, &d.state);
                    cars.push((s, d.car.car_type.name.clone(), look));
                }
                if let Some(t) = game.sim.traffic.as_ref() {
                    traffic_cars(t, &mut cars, sample);
                }
                if let Some(e) = game.sim.encounters.as_ref() {
                    for (i, c) in e.cars.iter().enumerate() {
                        let (s, look) = sample(ENCOUNTER_ID + i as u32, CarKind::Encounter, &c.car, &c.state);
                        cars.push((s, c.car.car_type.name.clone(), look));
                    }
                }
            }
        }
        for (s, name, look) in &cars {
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
        let frame = Frame { tick: self.tick, time, cars: cars.into_iter().map(|c| c.0).collect() };
        self.put(&Line::Frame(frame))?;
        for e in events {
            self.put(&Line::Event(e))?;
        }
        self.tick += 1;
        Ok(())
    }

    /// The racers' results from the race as it ended.
    pub(crate) fn results(&mut self, r: &Race) -> Result<()> {
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

/// The traffic cars on the road (placed ones only).
fn traffic_cars(
    t: &Traffic,
    cars: &mut Vec<(CarSample, String, CarLook)>,
    sample: impl Fn(u32, CarKind, &Car, &CarState) -> (CarSample, CarLook),
) {
    for (i, c) in t.cars.iter().enumerate() {
        if c.lane.is_some() {
            let (s, look) = sample(TRAFFIC_ID + i as u32, CarKind::Traffic, &c.car, &c.state);
            cars.push((s, c.car.car_type.name.clone(), look));
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
        let mut header = Header { version: VERSION, mode: run.mode, seed: run.seed, ..Default::default() };
        if let Some(id) = &t.race
            && let Mode::Race(r) = &self.mode
        {
            header.race = id.clone();
            header.track = self.sim.map_race.as_ref().map_or(0, |m| m.track);
            header.laps = r.laps;
            header.checkpoints = r.route.checkpoints.len() as u32;
        }
        out.header(header)?;
        let limit = run.seconds.unwrap_or(if run.mode == SimMode::Race { 900.0 } else { 120.0 });
        let (mut world, mut raced) = (0.0f32, false);
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
            match &self.mode {
                Mode::Race(r) => {
                    raced = true;
                    out.frame(self, dt)?;
                    // Every racer through (or the race over): the results.
                    let all =
                        r.racers.me.finished.is_some() && r.racers.rivals.iter().all(|x| x.progress.finished.is_some());
                    if all && r.over > 1.0 {
                        out.results(r)?;
                        return Ok("finish");
                    }
                }
                Mode::Explore if run.mode == SimMode::World => out.frame(self, dt)?,
                Mode::Explore => {}
                Mode::Menu(_) if raced => {
                    // The race left for its results before every rival
                    // was through: the race as the results took it.
                    if let Some(r) = self.sim.paused_race.as_ref().or(self.sim.replay.as_ref()) {
                        out.results(r)?;
                    }
                    return Ok("finish");
                }
                Mode::Menu(_) => {}
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
    let mut s = Streamer::new(sink);
    let why = match game.simulate(t, run, &mut s, &|| quit.load(std::sync::atomic::Ordering::Relaxed)) {
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
        let m = MapMod { name: "Sim".into(), races: vec![race], ..Default::default() };
        let dir = store::maps_dir();
        store::save(&dir, folder, &m).unwrap();
        store::set_enabled(&dir, folder, true).unwrap();
        nfsu2::tools::map::reload_active();
    }

    fn simulate(seed: u64, seconds: f32) -> (String, &'static str) {
        let mut game = Game::new().unwrap();
        sprint_mod(&game, "sim-run");
        game.profile = None;
        let t =
            crate::shell::cli::MapTest { folder: "sim-run".into(), race: Some("short".into()), ..Default::default() };
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
