//! `--flow firstdrag` (docs/notes/parity/first-drag.md): a fresh career's first drag
//! race (the profile `skip_intro` leaves at the stage 1 city start, whose
//! open list holds DRAG_1), its start scene played through and then raced
//! on the player's keys (gas held, Shift Up near the red line, the arrow
//! keys round traffic ahead, as `reported_issues::drag_run`), driven frame by
//! frame as the window drives it (`Game::advance` at the display's
//! interval; a scene's world updates at 1/30 s) at several display rates and grid
//! seeds; then the pause's Restart and the same again. Each rival is
//! watched every world tick: roll, pitch, its up axis, its drag lane,
//! wheels down, contacts (with the player, another rival, traffic, a wall),
//! speed, crash mode, totaled. A rival still racing (not dropped out) that
//! rolls or pitches past [`MAX_TILT`], turns over, or has no wheel down
//! for more than [`MAX_AIR`] ticks fails the flow; what happens to a car
//! after it is totaled (status 9) is reported, not failed.
use crate::*;

/// The most a racing rival may roll or pitch (degrees). A car on its
/// wheels on Bayview's roads stays within a few degrees (body roll and
/// the crowned / banked roads); a rival-on-rival hit can roll a car
/// about 30° and it lands back on its wheels (seen under Wine), so
/// 45° marks a real tip; upside down (up < 0) always counts.
const MAX_TILT: f32 = 45.0;
/// The most world ticks (1/60 s) a racing rival may have no wheel down.
const MAX_AIR: u32 = 30;
/// Seconds past the green a run lasts at most (the drag is about 40 s).
const RUN_SECS: f32 = 75.0;

/// One rival's record over a run.
#[derive(Debug, Default, Clone)]
pub(super) struct RivalWatch {
    pub car: String,
    /// Largest |roll| / |pitch| (degrees) and least up·z while racing.
    pub roll: f32,
    pub pitch: f32,
    pub min_up: f32,
    /// Longest run of world ticks with no wheel down while racing.
    pub air: u32,
    /// Contacts begun (new records) by kind: player, rival, traffic, wall,
    /// other track; and the hardest summed push of each kind.
    pub hits: [u32; 5],
    pub hardest: [f32; 5],
    /// Lane changes made (lane index changes).
    pub lane_changes: u32,
    /// When it was totaled / dropped (race clock) and its status.
    pub dropped: Option<(f32, u16)>,
    /// Least up·z after it dropped out.
    pub min_up_after: f32,
    /// Whether / when crash mode started.
    pub crash_at: Option<f32>,
    /// Its finish time.
    pub finished: Option<f32>,
    /// The first moment it tipped while racing: (clock, roll, pitch, up z).
    pub tipped: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug, Default)]
pub(super) struct FirstDragRun {
    pub label: String,
    pub event: String,
    pub rivals: Vec<RivalWatch>,
    pub player_finished: Option<f32>,
    /// The player was totaled (the race ends for them).
    pub player_totaled: bool,
    pub ticks: u32,
    /// The mode the run left the race for, if it did.
    pub left: Option<String>,
    /// Whether the run was driven to the end (else a short start).
    pub full: bool,
}

const KINDS: [&str; 5] = ["player", "rival", "traffic", "wall", "track"];

impl FirstDragRun {
    pub fn failures(&self) -> Vec<String> {
        let mut f = Vec::new();
        for (i, r) in self.rivals.iter().enumerate() {
            if let Some((t, roll, pitch, up)) = r.tipped {
                f.push(format!(
                    "{}: rival {} ({}) tipped while racing at {t:.2} s: roll {roll:.1}° pitch {pitch:.1}° up {up:.2}",
                    self.label,
                    i + 1,
                    r.car
                ));
            }
            if r.air > MAX_AIR {
                f.push(format!(
                    "{}: rival {} ({}) had no wheel down for {} ticks while racing",
                    self.label,
                    i + 1,
                    r.car,
                    r.air
                ));
            }
        }
        if self.full && self.player_finished.is_none() && !self.player_totaled {
            f.push(format!("{}: the player did not finish", self.label));
        }
        f
    }
}

/// Roll and pitch (degrees) of an orientation: the left axis' and the
/// forward axis' rise.
fn tilt(q: Quat) -> (f32, f32, f32) {
    let (fwd, left, up) = (q * Vec3::X, q * Vec3::Y, q * Vec3::Z);
    (left.z.atan2(up.z).to_degrees(), fwd.z.clamp(-1.0, 1.0).asin().to_degrees(), up.z)
}

/// Which kind of thing a contact point touches: the nearest car body
/// within 3 m of it (player, rival, traffic), else a wall / the track.
fn contact_kind(race: &Race, me: Vec3, own: usize, note: &nfsu2::drive::ContactNote) -> usize {
    match note.touching {
        nfsu2::drive::Touching::Track { .. } => {
            if note.wall {
                3
            } else {
                4
            }
        }
        nfsu2::drive::Touching::Car { .. } => {
            let p = note.point;
            let mut best = (f32::MAX, 2usize);
            best = best.min_by_dist(me.distance(p), 0);
            for (j, r) in race.racers.rivals.iter().enumerate() {
                if j != own {
                    best = best.min_by_dist(r.state.position.distance(p), 1);
                }
            }
            for c in race.racers.traffic.iter().flat_map(|t| t.cars.iter()) {
                best = best.min_by_dist(c.state.position.distance(p), 2);
            }
            best.1
        }
    }
}

trait MinBy {
    fn min_by_dist(self, d: f32, kind: usize) -> Self;
}
impl MinBy for (f32, usize) {
    fn min_by_dist(self, d: f32, kind: usize) -> Self {
        if d < self.0 { (d, kind) } else { self }
    }
}

/// A fresh profile past the intro (stage 1, DRAG_1 open), in free roam;
/// its first open drag started with the grid seed `seed`. Returns the
/// event's name.
fn fresh_first_drag(game: &mut Game, car: &str, seed: u32) -> Result<String> {
    let mut p = Profile::new("FIRSTDRAG");
    p.skip_intro(&game.game_data.career, &game.game_data.root, car, &mut game.rng);
    anyhow::ensure!(p.stage == 1, "the fresh profile is in stage 1 ({})", p.stage);
    let drag = p
        .open
        .iter()
        .filter_map(|h| game.game_data.career.events.iter().find(|e| e.unlock == *h))
        .find(|e| e.kind == career::EventKind::Drag)
        .map(|e| (e.index, e.name.clone()))
        .context("an open drag at the city start")?;
    game.profile = Some(p);
    game.explore(None)?;
    game.rng = nfsu2::opponents::GameRng(seed);
    // Flow aid: the cadence's real-time remainder and the frame
    // clock's 1/4000 s carry (0x865144) start each run from zero, so a run
    // does not depend on the rate of the run before it (else "a flow
    // run depends on the run before it": the carry left by a 30 Hz run
    // shifted the start scene's 60, 60, 60, 60, 61 tick phase, and with it
    // the whole race). Not a game change.
    game.pacing.cadence = Default::default();
    game.start_race(drag.0, false, false)?;
    Ok(drag.1)
}

/// Drives the current drag (started already) through its scene to the
/// player's finish and every rival's end (or [`RUN_SECS`]) at `hz`.
pub(super) fn drive_drag(game: &mut Game, label: &str, hz: f32, until: f32, log: bool) -> Result<FirstDragRun> {
    let mut run = FirstDragRun { label: label.to_string(), full: until >= RUN_SECS, ..Default::default() };
    // One tick per pass, the drawn frames on the schedule.
    let mut frames = super::ticks::Frames::new(hz);
    let mut tick = u32::MAX;
    let redline = game.sim.drive.as_ref().context("the player's car")?.car.car_type.handling.engine.redline_rpm;
    // The bot's waits are race-clock deadlines and it decides once per
    // world update (the first frame after one), so the same keys reach the
    // same world updates at every display rate from 40 Hz up (the old
    // waits counted drawn frames' intervals and a 200 Hz run released a
    // steer key a world tick off a 60 Hz run's).
    let mut shift_until = f32::NEG_INFINITY;
    let mut steer_until: Option<f32> = None;
    let mut decided = u32::MAX;
    let mut trace = std::env::var_os("NFSU2_FIRSTDRAG_TRACE")
        .map(|dir| {
            let name = label.replace(' ', "_");
            std::fs::File::create(std::path::Path::new(&dir).join(format!("{name}.tsv"))).map(std::io::BufWriter::new)
        })
        .transpose()?;
    let mut last_gear = 0usize;
    let (mut shown, mut messages) = (None::<u32>, String::new());
    let mut air: Vec<u32> = Vec::new();
    let mut lanes: Vec<Option<usize>> = Vec::new();
    game.hold_next(KeyCode::KeyW, true);
    loop {
        let Mode::Race(race) = &game.mode else {
            // The race is over (the results, or a total's end).
            run.left = Some(flow_mode(&game.mode));
            break;
        };
        if race.timing.clock > until
            || (race.racers.me.finished.is_some() && race.racers.rivals.iter().all(|r| r.progress.finished.is_some()))
        {
            break;
        }
        anyhow::ensure!(frames.frames() < 400_000, "{label}: the race never ended");
        // The player's keys once the green is up and until the finish.
        let now_tick = game.sim.drive.as_ref().map_or(0, |d| d.world_clock.tick);
        if race.timing.clock >= 0.0 && race.racers.me.finished.is_none() && now_tick != decided {
            decided = now_tick;
            let clock = race.timing.clock;
            let d = game.sim.drive.as_ref().context("the player's car")?;
            // The bot's gears: up near the redline, and down again when a
            // hit has dropped the engine under 40 % of it (flow aid; before,
            // the bot crawled away from a crash in top gear and could miss
            // the 75 s, e.g. seed 0 after another run, 2026-11-06).
            let gear = d.state.drivetrain.as_ref().map_or(0, |t| t.gear);
            if clock >= shift_until && d.state.rpm() >= 0.95 * redline {
                game.tap_next(KeyCode::ShiftLeft);
                shift_until = clock + 0.5;
            } else if clock >= shift_until && gear > 2 && d.state.rpm() < 0.4 * redline {
                game.tap_next(KeyCode::ControlLeft);
                shift_until = clock + 0.5;
            }
            if let Some(until) = steer_until {
                if clock >= until {
                    game.hold_next(KeyCode::ArrowLeft, false);
                    game.hold_next(KeyCode::ArrowRight, false);
                    steer_until = None;
                }
            } else if let Mode::Race(r) = &game.mode {
                let d = game.sim.drive.as_ref().context("the player's car")?;
                let (at, fwd) = (d.state.position, d.state.forward());
                let left = Vec3::Z.cross(fwd);
                let blocking = r
                    .racers.traffic
                    .iter()
                    .flat_map(|t| t.cars.iter().filter(|c| c.lane.is_some()))
                    .map(|c| c.state.position - at)
                    .find(|rel| (0.0..80.0).contains(&rel.dot(fwd)) && rel.dot(left).abs() < 2.5);
                if let Some(rel) = blocking {
                    let key = if rel.dot(left) >= 0.0 { KeyCode::ArrowRight } else { KeyCode::ArrowLeft };
                    game.hold_next(key, true);
                    steer_until = Some(clock + 0.25);
                }
            }
        } else if race.racers.me.finished.is_some() {
            game.hold_next(KeyCode::KeyW, false);
        }
        let traced = trace.is_some();
        frames.tick_with(game, &mut |game, super::ticks::Drawn { interval: dt, .. }| {
            if traced {
                // The HUD each drawn frame on the run's own clock, as the window
                // draws it: the drag shift message as it comes up.
                if !super::rate::active() {
                    game.pacing.clock = Some(game.pacing.clock.unwrap_or(0.0) + f64::from(dt));
                }
                let now = game.now();
                game.hud(now);
                let label = game.hud_state.hud.drag_shift_message();
                if label != shown {
                    if let (Some(l), Mode::Race(r)) = (label, &game.mode) {
                        messages.push_str(&format!("\tmsg {:#010x} at {:.6}", l, r.timing.clock));
                    }
                    shown = label;
                }
            }
            Ok(())
        })?;
        let Mode::Race(race) = &game.mode else { continue };
        let Some(d) = game.sim.drive.as_ref() else { continue };
        if run.rivals.is_empty() {
            run.event = game.game_data.career.events[race.event].name.clone();
            run.rivals = race
                .racers.rivals
                .iter()
                .map(|r| RivalWatch {
                    car: r.car.car_type.name.clone(),
                    min_up: 1.0,
                    min_up_after: 1.0,
                    ..Default::default()
                })
                .collect();
            air = vec![0; race.racers.rivals.len()];
            lanes = race.racers.rivals.iter().map(|r| r.lane.lane).collect();
        }
        if d.world_clock.tick == tick {
            continue;
        }
        tick = d.world_clock.tick;
        run.ticks += 1;
        if let Some(w) = trace.as_mut() {
            // Rate check: one row per world update, the race clock, a hash
            // of every racer's body state and the player's drivetrain, and the
            // player's gear changes with the grade (drivetrain +0x68).
            use std::hash::Hasher;
            use std::io::Write;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            let mut put = |s: &nfsu2::drive::CarState| {
                for v in [s.position.to_array(), s.velocity.to_array(), s.angular_momentum.to_array()]
                    .concat()
                    .into_iter()
                    .chain(s.orientation.to_array())
                {
                    h.write_u32(v.to_bits());
                }
            };
            put(&d.state);
            race.racers.rivals.iter().for_each(|r| put(&r.state));
            let t = d.state.drivetrain.as_ref();
            let gear = t.map_or(0, |t| t.gear);
            let grade = t.map_or(0.0, |t| t.shift_bonus);
            h.write_u32(d.state.rpm().to_bits());
            h.write_usize(gear);
            let shift =
                if gear != last_gear { format!("\tshift {last_gear}->{gear} grade {grade}") } else { String::new() };
            last_gear = gear;
            writeln!(
                w,
                "{}\t{:.6}\t{:016x}\t{:.3}\t{:.1}{shift}{}",
                tick,
                race.timing.clock,
                h.finish(),
                d.state.speed(),
                d.state.rpm(),
                std::mem::take(&mut messages)
            )?;
        }
        if run.player_finished.is_none() {
            run.player_finished = race.racers.me.finished;
        }
        run.player_totaled |= d.state.totaled;
        let clock = race.timing.clock;
        let me = d.state.position;
        let mut line = String::new();
        for (i, r) in race.racers.rivals.iter().enumerate() {
            let w = &mut run.rivals[i];
            let (roll, pitch, up) = tilt(r.state.orientation);
            let wheels = r.state.wheels_down();
            let racing = r.progress.dropped.is_none() && r.progress.finished.is_none() && clock >= 0.0;
            // Contacts begun this tick (record life) count as hits; the
            // pushes this tick (record age) are logged.
            let mut kinds = String::new();
            for note in &r.state.contacts {
                let k = contact_kind(race, me, i, note);
                if note.life <= 1.0 / 60.0 + 1e-4 {
                    w.hits[k] += 1;
                }
                if note.age <= 1.0 / 60.0 + 1e-4 {
                    kinds += &format!("{}:{:.0} ", KINDS[k], note.impulse);
                }
                w.hardest[k] = w.hardest[k].max(note.impulse);
            }
            if r.lane.lane != lanes[i] {
                if lanes[i].is_some() {
                    w.lane_changes += 1;
                }
                lanes[i] = r.lane.lane;
            }
            if r.state.crash && w.crash_at.is_none() {
                w.crash_at = Some(clock);
            }
            if let (Some(s), None) = (r.progress.dropped, w.dropped) {
                w.dropped = Some((clock, s));
            }
            w.finished = r.progress.finished;
            if racing {
                w.roll = w.roll.max(roll.abs());
                w.pitch = w.pitch.max(pitch.abs());
                w.min_up = w.min_up.min(up);
                air[i] = if wheels == 0 { air[i] + 1 } else { 0 };
                w.air = w.air.max(air[i]);
                if w.tipped.is_none() && (roll.abs() > MAX_TILT || pitch.abs() > MAX_TILT || up < 0.0) {
                    w.tipped = Some((clock, roll, pitch, up));
                }
            } else if r.progress.dropped.is_some() {
                w.min_up_after = w.min_up_after.min(up);
            }
            if log {
                line += &format!(
                    " | r{} lane {:?} roll {roll:6.1} pitch {pitch:6.1} up {up:5.2} wh {wheels} v {:5.1} crash {} tot {} drop {:?} c[{}]",
                    i + 1,
                    r.lane.lane,
                    r.state.speed(),
                    r.state.crash as u8,
                    r.state.totaled as u8,
                    r.progress.dropped,
                    kinds.trim_end()
                );
            }
        }
        if log {
            println!("{label} t {clock:7.3} me v {:5.1}{line}", d.state.speed());
        }
    }
    game.platform.keys.clear();
    Ok(run)
}

/// The pause's Restart (as the player picks it).
fn restart(game: &mut Game) -> Result<()> {
    game.key(KeyCode::Escape, &mut || {})?;
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))),
        "the race's pause ({})",
        flow_mode(&game.mode)
    );
    game.act(Action::Restart, MenuKind::RacePause)?;
    for _ in 0..4 {
        if !matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
            break;
        }
        game.menu_key(Key::Left)?;
        game.menu_key(Key::Accept)?;
    }
    anyhow::ensure!(matches!(game.mode, Mode::Race(_)), "Restart: in {}", flow_mode(&game.mode));
    Ok(())
}

fn list<T: std::str::FromStr>(var: &str, default: &str) -> Vec<T> {
    std::env::var(var)
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect()
}

fn summary(run: &FirstDragRun) -> String {
    let mut s = format!(
        "{} [{}] player {:?} totaled {} ticks {} left {:?}",
        run.label,
        run.event,
        run.player_finished.map(|t| (t * 100.0).round() / 100.0),
        run.player_totaled,
        run.ticks,
        run.left
    );
    for (i, r) in run.rivals.iter().enumerate() {
        s += &format!(
            "\n  r{} {:<10} roll {:5.1} pitch {:5.1} up {:5.2} air {:3} lanes {} hits p{}/r{}/t{}/w{} hardest p{:.0}/r{:.0}/t{:.0}/w{:.0} crash {:?} drop {:?} up_after {:5.2} fin {:?} tipped {:?}",
            i + 1,
            r.car,
            r.roll,
            r.pitch,
            r.min_up,
            r.air,
            r.lane_changes,
            r.hits[0],
            r.hits[1],
            r.hits[2],
            r.hits[3],
            r.hardest[0],
            r.hardest[1],
            r.hardest[2],
            r.hardest[3],
            r.crash_at,
            r.dropped,
            r.min_up_after,
            r.finished,
            r.tipped
        );
    }
    s
}

/// `--flow firstdrag`. `NFSU2_FIRSTDRAG_HZ` (comma list, 0 = uncapped;
/// default 60,144,0), `NFSU2_FIRSTDRAG_SEEDS` (grid seeds, default 4),
/// `NFSU2_FIRSTDRAG_CAR` (default PEUGOT), `NFSU2_FIRSTDRAG_LOG=1` a row
/// per world tick.
pub(crate) fn flow_first_drag(game: &mut Game) -> Result<()> {
    let rates: Vec<f32> = list("NFSU2_FIRSTDRAG_HZ", "60,144,0");
    let seeds: Vec<u32> = list("NFSU2_FIRSTDRAG_SEEDS", "4");
    let car = std::env::var("NFSU2_FIRSTDRAG_CAR").unwrap_or_else(|_| "PEUGOT".into());
    let log = std::env::var_os("NFSU2_FIRSTDRAG_LOG").is_some();
    let mut failures = Vec::new();
    let mut runs = 0;
    let mut dropouts = 0;
    for &seed in &seeds {
        for &hz in &rates {
            let tag = if hz > 0.0 { format!("{hz} Hz") } else { "uncapped".to_string() };
            // The whole race, then a fresh start left 5 s past the green
            // for the pause's Restart and that whole race again.
            fresh_first_drag(game, &car, seed)?;
            let first = drive_drag(game, &format!("first drag {tag} seed {seed}"), hz, RUN_SECS, log)?;
            println!("{}", summary(&first));
            failures.extend(first.failures());
            dropouts += first.rivals.iter().filter(|r| r.dropped.is_some()).count();
            fresh_first_drag(game, &car, seed)?;
            let start = drive_drag(game, &format!("start {tag} seed {seed}"), hz, 5.0, log)?;
            failures.extend(start.failures());
            anyhow::ensure!(start.left.is_none(), "the start run left the race ({:?})", start.left);
            restart(game)?;
            let again = drive_drag(game, &format!("restart {tag} seed {seed}"), hz, RUN_SECS, log)?;
            println!("{}", summary(&again));
            failures.extend(again.failures());
            dropouts += again.rivals.iter().filter(|r| r.dropped.is_some()).count();
            runs += 2;
        }
    }
    for f in &failures {
        println!("FAIL {f}");
    }
    println!("first drag: {runs} runs, {dropouts} rival drop-outs, {} failures", failures.len());
    // A rate run compares state across schedules; the flow's own
    // checks (made for steady rates) are reported there, not failed.
    if super::rate::report_only(failures.len()) {
        return Ok(());
    }
    anyhow::ensure!(failures.is_empty(), "{} first-drag failures", failures.len());
    println!("first drag: rivals stayed on their wheels");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first career drag at 60 Hz, two grid seeds, each raced whole and
    /// again after the pause's Restart (runs with NFSU2_ROOT): no racing
    /// rival tips, and with AvoidAvoidable's lane search (0x42BF00) the
    /// rivals mostly change lanes round the traffic instead of ramming it
    /// (before the port these four runs had 5 rivals totaled and 13 lane
    /// changes, after 2 and 92).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn first_drag_rivals_stay_upright_and_steer_round_traffic() {
        crate::tests::testkit::game();
        let mut game = Game::new().unwrap();
        let (mut dropouts, mut changes) = (0, 0);
        for seed in [4, 7] {
            fresh_first_drag(&mut game, "PEUGOT", seed).unwrap();
            let first = drive_drag(&mut game, "first", 60.0, RUN_SECS, false).unwrap();
            fresh_first_drag(&mut game, "PEUGOT", seed).unwrap();
            drive_drag(&mut game, "start", 60.0, 5.0, false).unwrap();
            restart(&mut game).unwrap();
            let again = drive_drag(&mut game, "restart", 60.0, RUN_SECS, false).unwrap();
            for run in [&first, &again] {
                assert!(run.failures().is_empty(), "{}", summary(run));
                dropouts += run.rivals.iter().filter(|r| r.dropped.is_some()).count();
                changes += run.rivals.iter().map(|r| r.lane_changes).sum::<u32>();
            }
        }
        assert!(dropouts <= 3, "{dropouts} rivals dropped out in 4 runs");
        assert!(changes >= 30, "{changes} lane changes in 4 runs");
    }
}
