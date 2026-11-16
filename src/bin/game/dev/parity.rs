//! `--flow parity <plan>` (docs/parity-harness.md): the clone's side of a
//! parity scenario. the maintainer's tool `parity.py` writes the plan from the
//! scenario (tools/parity/scenarios/*.toml) and the original's run: the
//! start state, the native tick counts per frame, the input events by
//! frame and the fields to record. The flow brings the same start state
//! about (a native save resumed, then a career event started, or an outrun
//! challenge from the original's start pose), then steps the world frame
//! by frame (`Game::step` with the native frame's ticks / 1800 s) with the
//! keys held as the plan says, recording each frame's state before its
//! inputs and update, as the original's cave does (the maintainer's tool `cave.s`).
use crate::*;

/// One recorded field: its scope (car or global), TSV name, clone accessor
/// and whether only AI racers have it.
struct PlanField {
    car: bool,
    name: String,
    accessor: String,
    components: usize,
    ai_only: bool,
    /// 0 not for traffic cars, 1 for traffic cars too (fields.toml
    /// `traffic = true`; a traffic car records only these), 2 for traffic
    /// cars only (fields.py `TRAFFIC_ONLY`).
    traffic: u8,
}

#[derive(Default)]
struct Plan {
    save: String,
    profile: String,
    event: Option<u32>,
    /// The outrun start: the original's frame-0 pose file (player and
    /// rival: position, velocity, forward; the rival's car name).
    outrun: Option<String>,
    /// The encounter approach (plan `encounter POSE`): the same pose
    /// file, both cars put there with no challenge.
    encounter: Option<String>,
    /// The original's tick 0x865188 at the outrun's frame 0 (plan `tick
    /// N`): the world load sets it to 4000 in both games (0x576D00),
    /// but the original's challenge came 18–45 s into free roam and the
    /// clone flow's after ~3 s; the controller's nitrous gap counts from
    /// +0x7AC = 0 (the ctor 0x4260B0), so the clone's world tick is put on
    /// the original's before the challenge.
    tick: Option<u32>,
    frames: usize,
    /// The original player's grid place at the green (its frame-0 x, y):
    /// the original draws the player's grid slot (two circuit runs gave
    /// two slots), so the clone's player swaps places with the car there.
    player_slot: Option<Vec2>,
    /// The original's world update counter 0x7FB724 at frame 0 (the
    /// DUMMY_SNAPSHOT's phase; its value at the green varies run to run
    /// with the load): the clone's race takes it at the arm.
    world_n: Option<u64>,
    /// The original's world generator ([0x890080] +0xF8) at frame 0: the
    /// clone's race field generator takes it at the arm (a pin like
    /// `world_n`; the boot-seeded stream itself is not reproduced, PLAN.md
    /// "RNG scope"), so the draw sites can be compared draw by draw.
    world_rng: Option<u32>,
    /// Frame 0 is the countdown's first frame (race status 3), not the
    /// green (scenario `arm = "countdown"`).
    countdown: bool,
    /// Free roam (`[start] kind = "free_roam"`): no race; frame 0 is the
    /// first frame after the resume (and the optional `place`).
    free_roam: bool,
    /// The player's pose for a free-roam start (x, y, z, heading in
    /// degrees, the original's 0 = +x).
    place: Option<(Vec3, f32)>,
    /// A Quick Race (mode name as `NFSU2_QR_MODE`, Lap KO) on the mode's
    /// first track, started from the main menu as the original's.
    quick: Option<(String, bool, String)>,
    /// Every racer's frame-0 grid place in the original: (`player` or the
    /// model name, x, y, forward x, forward y). With the rivals forced to
    /// the original's models (`NFSU2_BENCH_RIVAL_CARS`, set by parity.py)
    /// each clone car is put on its counterpart's place and heading.
    grid: Vec<(String, Vec2, Vec3)>,
    /// The original's tick count of each recorded frame (record f holds
    /// the ticks of the update that led to it, 0x86513C).
    ticks: Vec<u32>,
    /// (frame, down, key name); a negative frame is held from the race's
    /// start (through the scene and the countdown), as the original's
    /// pre-arm events.
    keys: Vec<(i64, bool, String)>,
    /// Key taps by the player's race coordinate (plan `tap COORD
    /// FRAMES KEY`): the key goes down on the first frame whose record
    /// shows the player's race coordinate (the original's car +0x8FC, the
    /// clone's `Progress::along`) at or past COORD and up FRAMES frames
    /// later, in order. The drag's lane taps: after the crest the two
    /// games' timing drifts apart, and a frame-keyed tap changed lanes at
    /// another place than the original's (the lanes end at obstacles).
    taps: Vec<(f32, usize, String)>,
    fields: Vec<PlanField>,
    /// The original player's state transplanted before frame F's update
    /// (scenario `[run] transplant = F`, plan `pose F com vel fwd`): the
    /// centre of mass, velocity and forward axis; spin zeroed. Separates a
    /// response (a wall hit) from the approach that led to it.
    pose: Option<(usize, Vec3, Vec3, Vec3)>,
    /// The original player's exact state at the arm (scenario `[run]
    /// stance = true`, plan `stance` with 30 numbers): centre of mass,
    /// the body's rows, velocity, ω, L, the four suspensions' compression
    /// and rate, the rest class. Put on the clone's player at the arm.
    stance: Option<Vec<f32>>,
    /// The original player's exact state before frame F's update (scenario
    /// `[run] transplant = F` with `transplant_full = true`, plan `stance_at
    /// F` with the stance's 30 numbers, then the gear (car +0x434), the
    /// engine's crank speed, the input shaft's speed, the clutch, the four
    /// wheels' spin, FL FR RR RL, the gearbox's target gear (+0x38), the
    /// shown rpm (car +0x440, which the automatic gear choice reads) and the
    /// shift state (+0x44)):
    /// the pose transplant with the body's
    /// ω, L, suspensions and drivetrain, so a response (a prop hit) runs from
    /// the original's state rather than from a re-launched one.
    stance_at: Option<(usize, Vec<f32>)>,
    /// The tyre and steering state the next frame reads, put on the clone's
    /// player with `stance_at` (plan `tyres_at F` and [`TYRES_AT`] numbers,
    /// see [`put_tyres`]).
    tyres_at: Option<(usize, Vec<f32>)>,
    out: String,
}

/// The numbers of a plan `tyres_at` line after its frame.
const TYRES_AT: usize = 23;

impl Plan {
    fn read(path: &str) -> Result<Plan> {
        let mut p = Plan { profile: "NAME".into(), ..Default::default() };
        for line in std::fs::read_to_string(path).with_context(|| format!("plan {path}"))?.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let w: Vec<&str> = line.split_whitespace().collect();
            let Some(&head) = w.first() else { continue };
            let arg = |i: usize| w.get(i).copied().with_context(|| format!("plan line `{line}`: argument {i}"));
            match head {
                "save" => p.save = arg(1)?.to_string(),
                "profile" => p.profile = arg(1)?.to_string(),
                "event" => p.event = Some(u32::from_str_radix(arg(1)?, 16)?),
                "outrun" => p.outrun = Some(arg(1)?.to_string()),
                "encounter" => p.encounter = Some(arg(1)?.to_string()),
                "tick" => p.tick = Some(arg(1)?.parse()?),
                "frames" => p.frames = arg(1)?.parse()?,
                "world_n" => p.world_n = Some(arg(1)?.parse()?),
                "world_rng" => p.world_rng = Some(arg(1)?.parse::<i64>()? as u32),
                "player_slot" => p.player_slot = Some(Vec2::new(arg(1)?.parse()?, arg(2)?.parse()?)),
                "quick" => {
                    p.quick =
                        Some((arg(1)?.to_string(), arg(2)? == "1", w.get(3).copied().unwrap_or("PEUGOT").to_string()))
                }
                "arm" => p.countdown = arg(1)? == "countdown",
                "free_roam" => p.free_roam = true,
                "place" => {
                    p.place = Some((Vec3::new(arg(1)?.parse()?, arg(2)?.parse()?, arg(3)?.parse()?), arg(4)?.parse()?))
                }
                "grid" => p.grid.push((
                    arg(1)?.to_string(),
                    Vec2::new(arg(2)?.parse()?, arg(3)?.parse()?),
                    Vec3::new(arg(4)?.parse()?, arg(5)?.parse()?, w.get(6).map_or(Ok(0.0), |z| z.parse())?),
                )),
                "ticks" => p.ticks = w[1..].iter().map(|t| t.parse()).collect::<Result<_, _>>()?,
                "key" => p.keys.push((arg(1)?.parse()?, arg(2)? == "down", arg(3)?.to_string())),
                "tap" => p.taps.push((arg(1)?.parse()?, arg(2)?.parse()?, arg(3)?.to_string())),
                "field" => p.fields.push(PlanField {
                    car: arg(1)? == "car",
                    name: arg(2)?.to_string(),
                    accessor: arg(3)?.to_string(),
                    components: arg(4)?.parse()?,
                    ai_only: arg(5)? == "1",
                    traffic: w.get(6).map_or(0, |t| t.parse().unwrap_or(0)),
                }),
                "pose" => {
                    let v = |i: usize| -> Result<f32> { Ok(arg(i)?.parse()?) };
                    p.pose = Some((
                        arg(1)?.parse()?,
                        Vec3::new(v(2)?, v(3)?, v(4)?),
                        Vec3::new(v(5)?, v(6)?, v(7)?),
                        Vec3::new(v(8)?, v(9)?, v(10)?),
                    ));
                }
                "stance" => {
                    let v = w[1..].iter().map(|t| t.parse::<f32>()).collect::<Result<Vec<_>, _>>()?;
                    anyhow::ensure!(v.len() == 30, "plan line `stance`: 30 numbers");
                    p.stance = Some(v);
                }
                "stance_at" => {
                    let v = w[2..].iter().map(|t| t.parse::<f32>()).collect::<Result<Vec<_>, _>>()?;
                    anyhow::ensure!(v.len() == 41, "plan line `stance_at`: a frame and 41 numbers");
                    p.stance_at = Some((arg(1)?.parse()?, v));
                }
                "tyres_at" => {
                    let v = w[2..].iter().map(|t| t.parse::<f32>()).collect::<Result<Vec<_>, _>>()?;
                    anyhow::ensure!(v.len() == TYRES_AT, "plan line `tyres_at`: a frame and {TYRES_AT} numbers");
                    p.tyres_at = Some((arg(1)?.parse()?, v));
                }
                "out" => p.out = arg(1)?.to_string(),
                _ => anyhow::bail!("plan: unknown line `{line}`"),
            }
        }
        anyhow::ensure!(!p.save.is_empty() && !p.out.is_empty() && p.frames > 0, "plan needs save, out and frames");
        Ok(p)
    }
}

/// The scenario keys as the player's keyboard (Controls rows' first keys).
fn key_code(name: &str) -> Result<KeyCode> {
    Ok(match name {
        "throttle" => KeyCode::ArrowUp,
        "brake" => KeyCode::ArrowDown,
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "handbrake" => KeyCode::Space,
        "nitro" => KeyCode::AltLeft,
        "shift_up" => KeyCode::ShiftLeft,
        "shift_down" => KeyCode::ControlLeft,
        // Controls row 9 (Look Back; the original's event 0x7E, 0x605C60).
        "look_back" => KeyCode::KeyB,
        _ => anyhow::bail!("unknown key {name}"),
    })
}

/// What a car row reads: its state and, for the AI, its controls and
/// action; its race progress and place when racing.
struct CarView<'a> {
    name: String,
    state: &'a CarState,
    /// The handling's centre of mass (body frame): the original's car
    /// +0x60 is the body's centre of mass (`CarState::placed`), the
    /// clone's position the body origin less it.
    com: Vec3,
    /// The handling's shift offset ranges and clutch factor (drivetrain
    /// +0xCC..+0xE0, +0xE8), the performance class (+0xF0) and the
    /// shift-window scale (+0xE4).
    shift: ([[f32; 2]; 3], f32, usize, f32),
    ai: Option<(&'a nfsu2::drive::AiControls, Option<&'a nfsu2::ai::actions::Actions>)>,
    /// The AI driver's target speed (+0x230), look-ahead point (+0x224)
    /// and path point count (+0x10).
    target: Option<f32>,
    /// The catch-up multiplier (controller +0x770).
    catch: Option<f32>,
    /// The controller's goal type (+0x6FC → +0x14): an encounter racer's 5
    /// (chase, the player leads) or 6 (lead).
    goal: Option<i32>,
    look: Option<Vec2>,
    path_n: Option<usize>,
    /// A drag race (the AI's lane action).
    drag: bool,
    laps: Option<&'a nfsu2::standings::Laps>,
    /// The race coordinate (car +0x8FC; the clone's `Progress::along`).
    coord: Option<f32>,
    place: Option<usize>,
    human: bool,
    /// A traffic car (the original's world list past +0x18; car +0x24 = 4).
    traffic: Option<&'a TrafficCar>,
    /// A racer's style manager and drift manager (racer +0x1D4 / +0x1D8).
    style: Option<&'a nfsu2::drift::StyleManager>,
    scorer: Option<&'a nfsu2::drift::DriftManager>,
    /// In an outrun: the racer's distance behind the leader (racer +0x24,
    /// 0x5FC520 less the first's, 0x607DBB).
    behind: Option<f32>,
}

/// A car field's value (the accessor names in the maintainer's `fields.toml`).
fn car_value(c: &CarView, accessor: &str) -> Vec<f64> {
    let s = c.state;
    let v3 = |v: Vec3| vec![v.x as f64, v.y as f64, v.z as f64];
    match accessor {
        // Car +0x60: the centre of mass (the clone's origin + the turned
        // offset; see `CarState::placed`).
        "pos" => v3(s.position + s.orientation * c.com),
        "vel" => v3(s.velocity),
        "accel" => v3(s.world_acceleration),
        "fwd" => v3(s.forward()),
        "speed" => vec![s.speed() as f64],
        // On the MeloMvr its own gearbox (+0x124, the same numbering).
        "gear" => vec![s.melo.as_ref().map(|m| m.gear).or(s.drivetrain.as_ref().map(|d| d.gear)).unwrap_or(1) as f64],
        // Car +0x440, the shown rpm (`CarState::shown_rpm`).
        "rpm" => vec![s.shown_rpm() as f64],
        // The AI driver's output block (+0x1E0) for the AI, the car's
        // wheel angle for the player.
        "steer" => vec![c.ai.map_or(s.steer_degrees, |(k, _)| k.steer_degrees) as f64],
        // The input block (+0x20C / +0x214): an AI driver's own block
        // (after the green's release, which the mover reads only in the
        // next update), the player's pedals.
        "gas" => vec![c.ai.map_or(s.last_gas, |(k, _)| if k.mover_held { k.gas } else { s.last_gas }) as f64],
        "brake" => vec![c.ai.map_or(s.brake_input, |(k, _)| if k.mover_held { k.brake } else { s.brake_input }) as f64],
        "handbrake" => vec![c.ai.map_or(0.0, |(k, _)| k.handbrake) as f64],
        "nos_left" => vec![s.nitrous.map_or(0, |t| t.left) as f64],
        "ai_goal" => vec![c.goal.map_or(f64::NAN, f64::from)],
        // No action before the first think (the original's +0x6F8 is
        // null then and records 0; it gets its goal after the green).
        // In a drag race the running action is AIActionDriveTrafficLane
        // (type 7) unless AvoidAvoidable (1) took over.
        "ai_action" => vec![c.ai.and_then(|(_, a)| a).map_or(f64::NAN, |a| {
            if a.last_velocity.is_some() || !a.last_path.is_empty() {
                if c.drag && a.kind == nfsu2::ai::actions::Kind::JustDrive { 7.0 } else { a.kind.type_id() as f64 }
            } else {
                0.0
            }
        })],
        // The lane record (controller +0x360 / +0x364 / +0x368): the plan's
        // offset and rate.
        "ai_plan_off" => vec![c.ai.and_then(|(_, a)| a).map_or(f64::NAN, |a| f64::from(a.plan.offset))],
        // The action's object (+0x218): the clone's car id (0 the human,
        // 1… the rivals, 1000… the traffic; the original's is a pointer).
        "ai_obj" => vec![c.ai.and_then(|(_, a)| a).map_or(f64::NAN, |a| a.target.map_or(0.0, |t| t as f64))],
        "ai_plan_rate" => vec![c.ai.and_then(|(_, a)| a).map_or(f64::NAN, |a| f64::from(a.plan.rate))],
        "lap" => vec![c.laps.map_or(f64::NAN, |l| l.lap as f64)],
        "place" => vec![c.place.map_or(f64::NAN, |p| p as f64)],
        "progress" => vec![c.laps.map_or(f64::NAN, |l| l.progress as f64)],
        "rcoord" => vec![c.coord.map_or(f64::NAN, |a| a as f64)],
        "human" => vec![c.human as u8 as f64],
        // Car +0x28, the mover: 1 the player's, 2 the TrafficMover, 3 the
        // MeloMvr, 4 the BeltMover (0x5F46B0); traffic only.
        "mover" => vec![c.traffic.map_or(f64::NAN, |t| {
            if t.lane.is_none() {
                0.0
            } else if t.belt.belt.is_some() {
                4.0
            } else if t.mover.is_some() {
                2.0
            } else {
                3.0
            }
        })],
        // The traffic controller's running action ([car+0x2C]+0x6F8 →
        // +0x204): 0x11 obey the intersections, 2 avoid cars, 7 drive the
        // lane (formats.md, Traffic, Actions); none while unplaced.
        // The controller's parked byte (+0x77C): a car out of the pool's
        // play (the clone: no lane).
        "parked" => vec![c.traffic.map_or(f64::NAN, |t| f64::from(u8::from(t.lane.is_none())))],
        // The traffic controller's stopped-for-good byte (+0x710, 0x408DE0 /
        // 0x417750 / 0x428720 false).
        "stopped" => vec![c.traffic.map_or(f64::NAN, |t| f64::from(u8::from(t.stopped)))],
        // The car's live contact records (body +0x3C → +0x18, 0x5A0830).
        "recs" => vec![s.contacts.len() as f64],
        "traffic_action" => vec![c.traffic.and_then(|t| t.lane.as_ref()).map_or(f64::NAN, |l| {
            if l.obeying() {
                17.0
            } else if l.avoiding() {
                2.0
            } else {
                7.0
            }
        })],
        "ai_target" => vec![c.target.map_or(f64::NAN, f64::from)],
        "ai_look" => c.look.map_or(vec![f64::NAN; 3], |p| vec![p.x as f64, p.y as f64, f64::NAN]),
        "ai_catch" => vec![c.catch.map_or(f64::NAN, f64::from)],
        "ai_path_n" => vec![c.path_n.map_or(f64::NAN, |n| n as f64)],
        // Car +0x390..+0x393, one byte per wheel (0x592050), read as one
        // little-endian dword.
        "wheels" => {
            vec![s.wheels.iter().enumerate().map(|(i, w)| u32::from(w.on_ground) << (8 * i)).sum::<u32>() as f64]
        }
        // Drivetrain and wheels (mover +0x48 engine, +0x4C drivetrain,
        // +0x38.. wheels FL FR RR RL, the table's order, as the clone's
        // `wheels`; docs/tuning-physics-parity.md).
        "eng_w" | "eng_pedal" | "eng_power" | "eng_cut" | "clutch" | "shift" | "in_w" => {
            vec![s.drivetrain.as_ref().map_or(f64::NAN, |d| match accessor {
                "eng_w" => d.engine_w as f64,
                "eng_pedal" => d.throttle as f64,
                "eng_power" => d.power as f64,
                "eng_cut" => d.limiter as u8 as f64,
                "clutch" => d.clutch as f64,
                "shift" => d.shift as u8 as f64,
                _ => d.input_w as f64,
            })]
        }
        // The drag shift grade (+0x68), the random shift offsets (+0x78,
        // +0x80) and their ranges (+0xCC, +0xD0, +0xDC, +0xE0).
        "dt_68" | "dt_78" | "dt_80" => vec![s.drivetrain.as_ref().map_or(f64::NAN, |d| match accessor {
            "dt_68" => d.shift_bonus as f64,
            "dt_78" => d.offsets()[0] as f64,
            _ => d.offsets()[2] as f64,
        })],
        "dt_cc" => vec![c.shift.0[0][0] as f64],
        "dt_d0" => vec![c.shift.0[0][1] as f64],
        "dt_dc" => vec![c.shift.0[2][0] as f64],
        "dt_e0" => vec![c.shift.0[2][1] as f64],
        "dt_e8" => vec![c.shift.1 as f64],
        // The class (+0xF0), the shift-window scale (+0xE4), the
        // up-shift rpm per gear (+0x84 + 4 · gear) and the engine heat
        // (engine +0x8C, 0x5ECCE0).
        "dt_f0" => vec![c.shift.2 as f64],
        "dt_e4" => vec![c.shift.3 as f64],
        "dt_up2" | "dt_up3" | "dt_up4" | "dt_up5" | "dt_up6" | "dt_up7" => {
            let g = accessor[5..].parse::<usize>().unwrap_or(0);
            vec![s.drivetrain.as_ref().map_or(f64::NAN, |d| d.shift_point_rpm(g) as f64)]
        }
        "heat" => vec![s.drivetrain.as_ref().map_or(f64::NAN, |d| d.heat as f64)],
        "spin_fl" => vec![s.wheels[0].spin as f64],
        "spin_rl" => vec![s.wheels[3].spin as f64],
        "spin_rr" => vec![s.wheels[2].spin as f64],
        "slip_rl" => vec![s.wheels[3].slip as f64],
        "slip_rr" => vec![s.wheels[2].slip as f64],
        "hop" => vec![s.hop as f64],
        "xspeed" => vec![s.exported_forward_speed() as f64],
        "slip_fl" => vec![s.wheels[0].slip as f64],
        "slip_fr" => vec![s.wheels[1].slip as f64],
        "spin_fr" => vec![s.wheels[1].spin as f64],
        "slide_fr" => vec![s.wheels[1].sliding as u8 as f64],
        "slide_rr" => vec![s.wheels[2].sliding as u8 as f64],
        // The car's exported per-wheel slip pair (car +0x3B4 + 8i, +0x3B8 + 8i):
        // what the skid sound, the burnout score and the AI's nitrous test read.
        "xslip_fl" | "xslip_fr" | "xslip_rr" | "xslip_rl" => {
            let i = ["xslip_fl", "xslip_fr", "xslip_rr", "xslip_rl"].iter().position(|&n| n == accessor).unwrap_or(0);
            vec![s.wheels[i].exported_slip as f64]
        }
        "load_fl" => vec![s.wheels[0].load as f64],
        "grip_fl" => vec![s.wheels[0].grip as f64],
        "slide_fl" => vec![s.wheels[0].sliding as u8 as f64],
        "stab" => vec![s.stability_off as u8 as f64],
        "grip_rl" => vec![s.wheels[3].grip as f64],
        "load_rr" => vec![s.wheels[2].load as f64],
        "slide_rl" => vec![s.wheels[3].sliding as u8 as f64],
        "load_rl" => vec![s.wheels[3].load as f64],
        // The hop start test's load term (+0x164 of the end-state force pass).
        "hop_load_fl" => vec![s.hop_loads[0] as f64],
        "hop_load_rr" => vec![s.hop_loads[2] as f64],
        "hop_load_rl" => vec![s.hop_loads[3] as f64],
        "torq_rl" => vec![s.wheels[3].torque as f64],
        // The tyre state the next frame's tyre pass reads, per wheel FL FR RR
        // RL: the lock flag (+0x10C), the net torque's running mean (+0x120)
        // and the drive less brake torque (+0x108).
        "lock_fl" | "lock_fr" | "lock_rr" | "lock_rl" | "ntq_fl" | "ntq_fr" | "ntq_rr" | "ntq_rl" | "torq_fl"
        | "torq_fr" | "torq_rr" => {
            let (kind, wheel) = accessor.split_once('_').unwrap_or((accessor, "fl"));
            let w = &s.wheels[["fl", "fr", "rr", "rl"].iter().position(|&n| n == wheel).unwrap_or(0)];
            vec![match kind {
                "lock" => w.locked as u8 as f32,
                "ntq" => w.net_torque,
                _ => w.torque,
            } as f64]
        }
        // The player steering's state (PlayerSteering +0x10, +0x20, +0x54,
        // +0x80, +0x84), the handbrake ramp (mover +0x70) and the wheel hop's
        // timers (car +0x470, +0x478).
        "steer_drift" => vec![s.drifting as u8 as f64],
        "steer_avg" => vec![s.steer_average.mean as f64],
        "steer_rate" => vec![s.steer_rate_average.mean as f64],
        "steer_lim" => vec![s.steer_limit as f64],
        "steer_last" => vec![s.last_steer_input as f64],
        "hb_ramp" => vec![s.handbrake_ramp as f64],
        "hop_t" => vec![s.hop_timer as f64],
        "hop_calm" => vec![s.hop_calm as f64],
        "rb_damp" => vec![s.rest_damping as f64],
        "since_collision" => vec![s.since_collision as f64],
        "dt_70" => vec![s.drivetrain.as_ref().map_or(f64::NAN, |d| d.sent as f64)],
        "hop_grip" => vec![s.hop_grip as f64],
        // The DRIFT trick (style manager [+0]; state +0x14: 0 running,
        // 1 pending, 2 idle, 3 banked, 4 lost; 0x56EAC0 / 0x56D510), the
        // total (+0x9C) and the drift manager's level (+0x1C).
        "drift_state" | "drift_pts" | "drift_bank" | "drift_mult" => {
            vec![c.style.and_then(|m| m.trick(nfsu2::drift::DRIFT)).map_or(f64::NAN, |t| match accessor {
                "drift_state" => match t.state {
                    nfsu2::drift::TrickState::Running => 0.0,
                    nfsu2::drift::TrickState::Pending => 1.0,
                    nfsu2::drift::TrickState::Idle => 2.0,
                    nfsu2::drift::TrickState::Banked => 3.0,
                    nfsu2::drift::TrickState::Lost => 4.0,
                },
                "drift_pts" => t.points as f64,
                "drift_bank" => t.banked as f64,
                _ => t.multiplier as f64,
            })]
        }
        "drift_total" => vec![c.style.map_or(f64::NAN, |m| m.total as f64)],
        "drift_level" => vec![c.scorer.map_or(f64::NAN, |m| m.level as f64)],
        "drift_surf" => vec![c.scorer.map_or(f64::NAN, |m| m.surface as f64)],
        "drift_near" => vec![c.scorer.map_or(f64::NAN, |m| m.near as f64)],
        "yaw_rate" => vec![s.spin.z as f64],
        // The stance per wheel (FL FR RR RL): the suspensions' compression
        // and rate (+0x1C, +0x20), the tyre forces (+0x124, +0x128), the
        // sideways slip (car +0x3B8 + 8i) and the load (+0x138).
        "comp_fl" | "comp_fr" | "comp_rr" | "comp_rl" | "crate_fl" | "crate_fr" | "crate_rr" | "crate_rl" | "fy_fl"
        | "fy_fr" | "fy_rr" | "fy_rl" | "fx_fl" | "fx_fr" | "fx_rr" | "fx_rl" | "yslip_fl" | "yslip_fr" | "yslip_rr"
        | "yslip_rl" | "load_fr" => {
            let (kind, wheel) = accessor.split_once('_').unwrap_or((accessor, "fl"));
            let w = &s.wheels[["fl", "fr", "rr", "rl"].iter().position(|&n| n == wheel).unwrap_or(0)];
            vec![match kind {
                "comp" => w.compression,
                "crate" => w.compression_rate,
                "fy" => w.fy,
                "fx" => w.fx,
                "yslip" => w.lateral_slip,
                _ => w.load,
            } as f64]
        }
        // The rigid body's rows (forward, left, up in world space; body
        // +0x30 / +0x40 / +0x50), ω (+0xC0), L (+0x80) and the rest class
        // (mover +0x60).
        "rb_row0" => v3(s.orientation * Vec3::X),
        "rb_row1" => v3(s.orientation * Vec3::Y),
        "rb_row2" => v3(s.orientation * Vec3::Z),
        "rb_spin" => v3(s.spin),
        "rb_angmom" => v3(s.angular_momentum),
        "rest" => vec![s.rest as f64],
        "behind" => vec![c.behind.map_or(f64::NAN, f64::from)],
        // Recorded on the original's side only (context fields).
        "context" => vec![f64::NAN],
        _ => vec![f64::NAN],
    }
}

/// A global field's value.
fn global_value(game: &Game, accessor: &str) -> f64 {
    let race = match &game.mode {
        Mode::Race(r) => Some(r),
        _ => None,
    };
    match accessor {
        // 0x890118 +0: 2 the start scene, 3 the countdown, 4 racing (an
        // outrun: 4 while the challenge runs, observed).
        "race_state" => match race {
            Some(r) => race_status(game, r) as f64,
            None if game.sim.encounters.as_ref().is_some_and(|e| e.state == 4) => 4.0,
            // Free roam: the game state is 4 there too (observed, the
            // free-roam parity runs).
            None if matches!(game.mode, Mode::Explore) => 4.0,
            None => 0.0,
        },
        // +0x14: the race clock in 1/4000 s (not kept for an outrun here).
        "race_clock" => race.map_or(f64::NAN, |r| (r.timing.clock.max(0.0) as f64 * 4000.0).round()),
        "race_type" => {
            if game.sim.encounters.as_ref().is_some_and(|e| e.state == 4) {
                7.0
            } else {
                f64::NAN
            }
        }
        // View 1 (0x8760E0): the eye +0x40, the forward axis +0x50 and the
        // field of view +0xC4 (binary angle) of the player's camera.
        "cam_eye.x" | "cam_eye.y" | "cam_eye.z" | "cam_fwd.x" | "cam_fwd.y" | "cam_fwd.z" => {
            game.sim.drive.as_ref().map_or(f64::NAN, |d| {
                let v = if accessor.starts_with("cam_eye") { d.camera.position } else { d.camera.forward() };
                f64::from(v[usize::from(accessor.as_bytes()[accessor.len() - 1] - b'x')])
            })
        }
        "cam_fov" => game.sim.drive.as_ref().map_or(f64::NAN, |d| {
            (f64::from(d.camera.game_fov.unwrap_or(d.camera.fov_y)) * 65536.0 / std::f64::consts::TAU).round()
        }),
        // The impact shake's slot 0 (0x865360): its vector, time and curve
        // pointer (0x7FB7A0 while it runs).
        "shake_vec.x" | "shake_vec.y" | "shake_vec.z" | "shake_t" | "shake_on" => {
            game.sim.drive.as_ref().map_or(f64::NAN, |d| {
                let (v, t, on) = d.impact.shake.fields();
                match accessor {
                    "shake_t" => f64::from(t),
                    "shake_on" => if on { f64::from(0x7F_B7A0) } else { 0.0 },
                    _ => f64::from(v[usize::from(accessor.as_bytes()[accessor.len() - 1] - b'x')]),
                }
            })
        }
        "context" => f64::NAN,
        "cash" => game.profile.as_ref().map_or(f64::NAN, |p| p.cash as f64),
        // [0x890080] +0xF8: the race's world generator (the AI's speed
        // offsets, junction and pass draws), as the original's signed dword.
        // Free roam: the world's car field (the traffic's while it is
        // loaded, else the encounters').
        "world_rng" => race
            .map(|r| r.racers.cars.rng.0)
            .or_else(|| {
                game.sim.traffic.as_ref().map(|t| t.field.rng.0).or(game.sim.encounters.as_ref().map(|e| e.field.rng.0))
            })
            .map_or(f64::NAN, |x| f64::from(x as i32)),
        _ => f64::NAN,
    }
}

/// The race status (0x890118 +0): 2 while the start scene's camera runs
/// (to its second mark, the cut), 3 the countdown (or the scene's held
/// tail to the green), 4 racing.
fn race_status(game: &Game, r: &Race) -> i32 {
    if r.timing.clock >= 0.0 {
        4
    } else if game.scenes.shop.as_ref().is_some_and(|run| run.frame < run.scene.marks[1] as f32) {
        2
    } else {
        3
    }
}

/// The cars in the clone's order: the player, a race's rivals and traffic,
/// free roam's encounter cars.
fn cars(game: &Game) -> Vec<CarView<'_>> {
    let mut v = Vec::new();
    let Some(d) = game.sim.drive.as_ref() else { return v };
    let race = match &game.mode {
        Mode::Race(r) => Some(r),
        _ => None,
    };
    let place_of = |i: usize| race.and_then(|r| r.order.iter().position(|&o| o == i)).map(|p| p + 1);
    v.push(CarView {
        name: d.car.car_type.name.clone(),
        state: &d.state,
        com: Vec3::from(d.car.car_type.handling.centre_of_mass),
        shift: (
            d.car.car_type.handling.shift_ranges,
            d.car.car_type.handling.clutch_scale,
            d.car.car_type.handling.performance_class,
            d.car.car_type.handling.shift_window_scale,
        ),
        ai: None,
        target: None,
        catch: None,
        goal: None,
        look: None,
        path_n: None,
        drag: false,
        laps: race.and_then(|r| r.racers.me.laps.as_ref()),
        coord: race.map(|r| r.racers.me.along),
        place: place_of(0),
        human: true,
        traffic: None,
        style: race.and_then(|r| r.scoring.style.as_ref()),
        scorer: race.and_then(|r| r.scoring.drift.as_ref()),
        behind: game.sim.encounters.as_ref().and_then(|e| e.outrun.as_ref()).map(|o| o.player),
    });
    if let Some(r) = race {
        for (i, x) in r.racers.rivals.iter().enumerate() {
            v.push(CarView {
                name: x.car.car_type.name.clone(),
                state: &x.state,
                com: Vec3::from(x.car.car_type.handling.centre_of_mass),
                shift: (
                    x.car.car_type.handling.shift_ranges,
                    x.car.car_type.handling.clutch_scale,
                    x.car.car_type.handling.performance_class,
                    x.car.car_type.handling.shift_window_scale,
                ),
                ai: Some((&x.controls, Some(&x.actions))),
                target: Some(x.driver.target_speed),
                catch: Some(x.speed.catch_up()),
                goal: None,
                look: Some(x.driver.target),
                path_n: Some(x.driver.path_len),
                drag: r.drag,
                laps: x.progress.laps.as_ref(),
                coord: Some(x.progress.along),
                place: place_of(i + 1),
                human: false,
                traffic: None,
                style: x.style.as_ref(),
                scorer: x.scorer.as_ref(),
                behind: None,
            });
        }
    }
    // Traffic next (the original lists it last, +0x18 .. +0x10; the slots
    // are matched by position, and the pool's size is fixed while the
    // active encounter cars come and go): a race's own, else free roam's.
    if let Some(t) = race.map_or(game.sim.traffic.as_ref(), |r| r.racers.traffic.as_ref()) {
        for x in &t.cars {
            v.push(CarView {
                name: x.car.car_type.name.clone(),
                state: &x.state,
                com: Vec3::from(x.car.car_type.handling.centre_of_mass),
                shift: (
                    x.car.car_type.handling.shift_ranges,
                    x.car.car_type.handling.clutch_scale,
                    x.car.car_type.handling.performance_class,
                    x.car.car_type.handling.shift_window_scale,
                ),
                ai: None,
                target: None,
                catch: None,
                goal: None,
                look: None,
                path_n: None,
                drag: false,
                laps: None,
                coord: None,
                place: None,
                human: false,
                traffic: Some(x),
                style: None,
                scorer: None,
                behind: None,
            });
        }
    }
    if let Some(e) = game.sim.encounters.as_ref() {
        for (i, x) in e.cars.iter().enumerate().filter(|(_, c)| c.active) {
            let behind = e.outrun.as_ref().filter(|o| o.opponent == i).map(|o| o.rival);
            v.push(CarView {
                name: x.car.car_type.name.clone(),
                state: &x.state,
                com: Vec3::from(x.car.car_type.handling.centre_of_mass),
                shift: (
                    x.car.car_type.handling.shift_ranges,
                    x.car.car_type.handling.clutch_scale,
                    x.car.car_type.handling.performance_class,
                    x.car.car_type.handling.shift_window_scale,
                ),
                ai: Some((&x.controls, x.controller.actions.as_ref())),
                target: Some(x.controller.target_speed),
                catch: None,
                goal: x.controller.goal.map(|g| g.type_id()),
                look: None,
                path_n: None,
                drag: false,
                laps: None,
                coord: None,
                place: None,
                human: false,
                traffic: None,
                style: None,
                scorer: None,
                behind,
            });
        }
    }
    v
}

/// The fields a car records: a traffic car the `traffic` ones, an AI
/// racer every one, the rest all but the AI-only ones.
fn records(f: &PlanField, is_ai: bool, is_traffic: bool) -> bool {
    f.car && if is_traffic { f.traffic > 0 } else { f.traffic < 2 && (!f.ai_only || is_ai) }
}

fn header(plan: &Plan, n: usize, ai: &[bool], traffic: &[bool]) -> String {
    let comp = |f: &PlanField| -> Vec<String> {
        if f.components == 1 {
            vec![f.name.clone()]
        } else {
            ["x", "y", "z"][..f.components].iter().map(|c| format!("{}.{c}", f.name)).collect()
        }
    };
    let mut h = vec!["frame".to_string(), "ticks".into(), "clock".into()];
    for f in plan.fields.iter().filter(|f| !f.car) {
        h.extend(comp(f));
    }
    for (k, &is_ai) in ai.iter().enumerate().take(n) {
        for f in plan.fields.iter().filter(|f| records(f, is_ai, traffic[k])) {
            h.extend(comp(f).into_iter().map(|c| format!("c{k}.{c}")));
        }
    }
    h.join("\t")
}

fn num(x: f64) -> String {
    if x.is_nan() {
        String::new()
    } else if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        // Every accessor reads an f32: its shortest round-trip text, so
        // the row carries the value's exact bits.
        format!("{}", x as f32)
    }
}

fn row(game: &Game, plan: &Plan, frame: usize, ticks: u32, n: usize) -> String {
    let mut r = vec![frame.to_string(), ticks.to_string(), num((game.sim.clock_ms as f64 * 4.0).round())];
    for f in plan.fields.iter().filter(|f| !f.car) {
        if f.components == 1 {
            r.push(num(global_value(game, &f.accessor)));
        } else {
            r.extend(
                ["x", "y", "z"][..f.components].iter().map(|c| num(global_value(game, &format!("{}.{c}", f.accessor)))),
            );
        }
    }
    let cs = cars(game);
    for k in 0..n {
        let Some(c) = cs.get(k) else {
            for f in plan.fields.iter().filter(|f| records(f, false, false)) {
                r.extend(std::iter::repeat_n(String::new(), f.components));
            }
            continue;
        };
        for f in plan.fields.iter().filter(|f| records(f, c.ai.is_some(), c.traffic.is_some())) {
            let mut v = car_value(c, &f.accessor);
            v.resize(f.components, f64::NAN);
            r.extend(v.into_iter().map(num));
        }
    }
    r.join("\t")
}

/// The outrun start: the original's frame-0 pose (the maintainer's tool `outrun.py`
/// writes it: `player x y z vx vy vz fx fy fz gear` and `rival NAME x y z
/// vx vy vz fx fy fz tank_left gear` lines). An encounter car out and driving is taken over,
/// both cars put there, the challenge engaged (as `flow_outrun`).
fn start_outrun(game: &mut Game, path: &str, tick: Option<u32>) -> Result<()> {
    let (i, player, rival) = place_encounter(game, path)?;
    challenge(game, i, &player, &rival, tick)
}

/// The encounter table's columns (the maintainer's tool `encpoll.py`'s, for
/// encstats.py).
const ENC_HEADER: &str = "wall\tclock\tworld_n\trace_type\tslot\tcar\tname\tmover\tpos\tvel\tfwd\tgoal\tg_tgt\tg_dist\tg_off\tg_arg4\taction\tpri\ta_tgt\ta_dist\ta_off\ta_state\ta_speed\ta_line\tside\tstamp\tp_pos\tp_vel\tp_fwd\n";

/// One frame's rows of the encounter table (`NFSU2_ENC_POLL`): every
/// active encounter car's centre of mass, velocity, forward, goal (9 the
/// pursuit, 5 / 6 the outrun's chase / lead), action, near state, side
/// state and the action's speed; the player's centre of mass, velocity
/// and forward.
fn enc_rows(game: &Game, out: &mut String) {
    use crate::sim::encounters::ai::belt_car_position;
    let (Some(e), Some(d)) = (game.sim.encounters.as_ref(), game.sim.drive.as_ref()) else { return };
    let f3 = |v: Vec3| format!("{:.3},{:.3},{:.3}", v.x, v.y, v.z);
    let player = (f3(belt_car_position(&d.car, &d.state)), f3(d.state.velocity), f3(d.state.forward()));
    for (i, c) in e.cars.iter().enumerate().filter(|(_, c)| c.active) {
        let a = c.controller.actions.as_ref();
        let pursuit = a.and_then(|a| match a.goal {
            nfsu2::ai::actions::Goal::OffsetPursuit { distance, offset, .. } => Some((distance, offset)),
            nfsu2::ai::actions::Goal::Race => None,
        });
        let goal = match (pursuit, c.controller.goal) {
            (Some(_), _) => 9,
            (None, Some(g)) => g.type_id(),
            (None, None) => -1,
        };
        let (g_dist, g_off) = pursuit.unwrap_or((0.0, 0.0));
        let (action, pri, near) = a.map_or((-1, 0, 0), |a| (a.kind.type_id(), a.priority, i32::from(a.near)));
        let cap = c.controller.output.as_ref().and_then(|o| o.cap);
        out.push_str(&format!(
            "0\t{}\t{}\t{}\t{}\t0\t{}\t{}\t{}\t{}\t{}\t{goal}\t0\t{g_dist:.3}\t{g_off:.3}\t0\t{action}\t{pri}\t0\t{g_dist:.3}\t{g_off:.3}\t{near}\t{:.3}\t{}\t{}\t0\t{}\t{}\t{}\n",
            d.world_clock.tick,
            d.world_clock.update,
            if e.outrun.is_some() { 7 } else { 0 },
            i + 1,
            c.car.car_type.name,
            nfsu2_mover(c),
            f3(belt_car_position(&c.car, &c.state)),
            f3(c.state.velocity),
            f3(c.state.forward()),
            cap.unwrap_or(1000.0),
            u8::from(cap.is_none()),
            c.side.state,
            player.0,
            player.1,
            player.2,
        ));
    }
}

fn nfsu2_mover(c: &crate::sim::encounters::ai::EncounterCar) -> u32 {
    crate::sim::encounters::ai::encounter_mover_kind(c)
}

/// The encounter approach: an encounter car out, both cars put on
/// the original's frame-0 pose, the world tick on the original's; no
/// challenge.
fn start_encounter(game: &mut Game, path: &str, tick: Option<u32>) -> Result<()> {
    let (i, _, _) = place_encounter(game, path)?;
    if let (Some(t), Some(d)) = (tick, game.sim.drive.as_mut()) {
        d.world_clock.tick = t;
    }
    // The original's controller state at frame 0 (`pursuit ACTION NEAR
    // SIDE`): a running action 0x13 with its near state, and the slot's
    // side state.
    let text = std::fs::read_to_string(path)?;
    if let Some(w) = text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>()).find(|w| w.first() == Some(&"pursuit"))
        && w.len() >= 4
        && let Some(e) = game.sim.encounters.as_mut()
    {
        let (action, near, side): (i32, i32, u8) = (w[1].parse()?, w[2].parse()?, w[3].parse()?);
        let r = &mut e.cars[i];
        r.side.state = side;
        if action == 0x13 {
            let a = r.controller.actions.get_or_insert_with(nfsu2::ai::actions::Actions::default);
            a.set_pursuit(0, nfsu2::ai::actions::PURSUIT_DISTANCE, 0.0);
            a.kind = nfsu2::ai::actions::Kind::OffsetPursuit;
            a.priority = 0;
            a.near = near != 0;
        }
    }
    println!("parity: encounter car {i} placed on the original's pose");
    Ok(())
}

/// An encounter car out and driving, both cars put on the pose file's
/// poses: the car's slot and the two poses.
fn place_encounter(game: &mut Game, path: &str) -> Result<(usize, Vec<f32>, Vec<f32>)> {
    let text = std::fs::read_to_string(path)?;
    let mut player = None;
    let mut rival = None;
    for l in text.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        let nums = |from: usize| -> Result<Vec<f32>> {
            Ok(w[from..].iter().map(|x| x.parse::<f32>()).collect::<Result<_, _>>()?)
        };
        match w.first() {
            Some(&"player") => player = Some(nums(1)?),
            Some(&"rival") => rival = Some((w[1].to_string(), nums(2)?)),
            _ => {}
        }
    }
    let (player, (name, rival)) = (player.context("pose: player")?, rival.context("pose: rival")?);
    anyhow::ensure!(player.len() >= 9 && rival.len() >= 9, "pose: 9 numbers per car");
    // An encounter car of the original's model out and driving (the
    // clone's draws differ: NFSU2_ENCOUNTER_CAR forces the model).
    let mut slot = None;
    for _ in 0..60 * 180 {
        game.advance_ticks(1)?;
        if let Some(i) = game
            .sim.encounters
            .as_ref()
            .filter(|e| e.state == 3)
            .and_then(|e| e.cars.iter().position(|c| c.active && c.placed.is_some_and(|t| e.clock - t > 3.0)))
        {
            slot = Some(i);
            break;
        }
    }
    let i = slot.context("no encounter car out")?;
    let put = |world: &nfsu2::world::World, car: &Car, p: &[f32]| {
        let at = Vec3::new(p[0], p[1], p[2]);
        let fwd = Vec2::new(p[6], p[7]);
        let ground = world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
        let mut s = CarState::placed(&car.car_type, world, Vec3::new(at.x, at.y, ground), fwd.y.atan2(fwd.x));
        s.velocity = Vec3::new(p[3], p[4], p[5]);
        // Explicit replay motion initialization (the native pose).
        s.publish_forward_speed();
        let speed = s.velocity.length();
        for (w, t) in s.wheels.iter_mut().zip(&car.car_type.wheels) {
            w.spin = speed / t.radius;
        }
        s
    };
    {
        let d = game.sim.drive.as_mut().unwrap();
        d.state = put(&d.world, &d.car, &player);
        let e = game.sim.encounters.as_mut().unwrap();
        let r = &mut e.cars[i];
        if !r.car.car_type.name.eq_ignore_ascii_case(&name) {
            println!(
                "parity: the rival is a {} here, a {name} in the original (NFSU2_ENCOUNTER_CAR={name} forces it)",
                r.car.car_type.name
            );
        }
        let tank = r.state.nitrous;
        r.state = put(&d.world, &r.car, &rival);
        r.state.nitrous = tank;
        if let (Some(&left), Some(t)) = (rival.get(9), r.state.nitrous.as_mut()) {
            t.left = left as i32;
        }
        r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, r.belt_rating));
        r.belt = Default::default();
        r.route = nfsu2::ai::route::RacerRoute::start(
            &e.routes,
            r.state.position,
            r.state.velocity.truncate().normalize_or(Vec2::X),
        );
        r.driver_plan = Default::default();
        e.road_prev.clear();
    }
    Ok((i, player, rival))
}

/// The challenge from the placed poses (the outrun start's second half).
fn challenge(game: &mut Game, i: usize, player: &[f32], rival: &[f32], tick: Option<u32>) -> Result<()> {
    let (player, rival) = (player.to_vec(), rival.to_vec());
    // The throttle held while following, as the original's start
    // (the maintainer's tool `outrun.py` holds it through a cave event).
    game.platform.keys.insert(KeyCode::ArrowUp);
    for _ in 0..36 {
        game.advance_ticks(1)?;
        if game.sim.encounters.as_ref().is_some_and(|e| e.career == 2) {
            if let (Some(t), Some(d)) = (tick, game.sim.drive.as_mut()) {
                // Less the challenge update's own ticks (1/60 s → 67).
                d.world_clock.tick = t.saturating_sub(67);
            }
            game.sim.encounters.as_mut().unwrap().engage();
            // Frame 0 is the state after the update that took the
            // challenge, as the original's first frame with race type 7,
            // whose pose the file holds: both cars are put back on it.
            game.advance_ticks(1)?;
            let repose = |s: &mut CarState, p: &[f32]| {
                s.position = Vec3::new(p[0], p[1], p[2]);
                s.velocity = Vec3::new(p[3], p[4], p[5]);
                s.orientation = glam::Quat::from_rotation_z(p[7].atan2(p[6]));
            };
            repose(&mut game.sim.drive.as_mut().unwrap().state, &player);
            repose(&mut game.sim.encounters.as_mut().unwrap().cars[i].state, &rival);
            // The gears too (car +0x434: 0 R, 1 N, 2 first; the clone's
            // drivetrain counts the same way).
            for (s, g) in [
                (&mut game.sim.drive.as_mut().unwrap().state, player.get(9)),
                (&mut game.sim.encounters.as_mut().unwrap().cars[i].state, rival.get(10)),
            ] {
                if let Some(&g) = g {
                    if let Some(d) = s.drivetrain.as_mut() {
                        d.gear = g as usize;
                    }
                    if let Some(m) = s.melo.as_mut() {
                        m.gear = g as usize;
                    }
                }
            }
            return Ok(());
        }
    }
    anyhow::bail!("the career never offered the challenge")
}

/// The player takes the grid place of the car nearest `at` (the
/// original's player place at the green), that car the player's: their
/// positions, orientations and velocities are exchanged (the cars stand).
fn swap_to_slot(game: &mut Game, at: Vec2) {
    let Some(d) = game.sim.drive.as_mut() else { return };
    let Mode::Race(race) = &mut game.mode else { return };
    let mine = d.state.position.truncate().distance(at);
    let Some((i, other)) = race
        .racers.rivals
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.state.position.truncate().distance(at)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return;
    };
    if other + 0.5 >= mine {
        return;
    }
    let r = &mut race.racers.rivals[i].state;
    std::mem::swap(&mut d.state.position, &mut r.position);
    std::mem::swap(&mut d.state.orientation, &mut r.orientation);
    std::mem::swap(&mut d.state.velocity, &mut r.velocity);
    println!("parity: the player takes rival {}'s grid place ({:.2} m from the original's)", i + 1, other);
}

/// Every car on the original's frame-0 grid place and heading, matched by
/// model (the player by `player`); false (nothing moved) unless every
/// racer has its counterpart. The cars stand at the green; each keeps how
/// it has settled on its own place.
fn place_on_grid(game: &mut Game, grid: &[(String, Vec2, Vec3)]) -> bool {
    let Some(d) = game.sim.drive.as_mut() else { return false };
    let Mode::Race(race) = &mut game.mode else { return false };
    if grid.is_empty() || grid.len() != race.racers.rivals.len() + 1 {
        return false;
    }
    let mut used = vec![false; race.racers.rivals.len()];
    let mut moves = Vec::new();
    for (who, at, fwd) in grid {
        if who == "player" {
            moves.push((None, *at, *fwd));
            continue;
        }
        let Some(i) =
            race.racers.rivals.iter().enumerate().position(|(i, r)| !used[i] && r.car.car_type.name.eq_ignore_ascii_case(who))
        else {
            println!("parity: no clone rival is a {who}: the grid is not copied");
            return false;
        };
        used[i] = true;
        moves.push((Some(i), *at, *fwd));
    }
    for (i, at, fwd) in moves {
        let world = &d.world;
        let (s, car_type) = match i {
            None => (&mut d.state, &d.car.car_type),
            Some(i) => {
                let r = &mut race.racers.rivals[i];
                (&mut r.state, &r.car.car_type)
            }
        };
        // The original's place is its centre of mass (car +0x60), with the
        // original's heading. The car is moved as the race set-up places
        // it (`CarState::placed`, 0x5F4A00: on its four wheels' ground)
        // and keeps how it has settled there: the settled pose relative to
        // a fresh placement at its own place is carried to a fresh
        // placement at the original's (turning a settled car flat onto the
        // new heading lifted it off its springs: loads fell from 1.9 to
        // 0.3 kN and the rest test froze it so).
        let com = Vec3::from(car_type.handling.centre_of_mass);
        let (fx, fy) = {
            let f = s.orientation * Vec3::X;
            (f.x, f.y)
        };
        let here = (s.position + s.orientation * com).truncate();
        let old = CarState::placed(car_type, world, here.extend(s.position.z + com.z), fy.atan2(fx));
        let new = CarState::placed(car_type, world, at.extend(s.position.z + com.z), fwd.y.atan2(fwd.x));
        let relative = old.orientation.inverse() * s.orientation;
        let offset = old.orientation.inverse() * (s.position - old.position);
        let moved = at.distance(here);
        s.orientation = (new.orientation * relative).normalize();
        s.position = new.position + new.orientation * offset;
        s.velocity = Vec3::ZERO;
        println!(
            "parity: {} put on the original's grid place ({moved:.2} m)",
            i.map_or("the player".to_string(), |i| format!("rival {}", i + 1))
        );
        println!(
            "parity: {} {} rating {:.1} class {}",
            i.map_or("the player".to_string(), |i| format!("rival {}", i + 1)),
            car_type.name,
            car_type.handling.performance_rating,
            car_type.handling.performance_class
        );
    }
    true
}

/// The original player's state from a plan `stance` line (see
/// `Plan::stance`): the body's rows become the orientation, the centre of
/// mass the position less the turned offset.
fn put_stance(d: &mut Drive, v: &[f32]) {
    let v3 = |i: usize| Vec3::new(v[i], v[i + 1], v[i + 2]);
    let rot = glam::Mat3::from_cols(v3(3), v3(6), v3(9));
    let s = &mut d.state;
    s.orientation = glam::Quat::from_mat3(&rot).normalize();
    s.position = v3(0) - s.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
    // The original's centre of mass exactly (body +0x20).
    s.set_com_world(v3(0), Vec3::from(d.car.car_type.handling.centre_of_mass));
    s.velocity = v3(12);
    s.publish_forward_speed();
    s.spin = v3(15);
    s.angular_momentum = v3(18);
    put_suspensions(s, &v[21..29]);
    s.rest = v[29] as u8;
}

/// The original's suspension compressions and rates (+0x1C, +0x20, FL FR RR
/// RL) on the clone's wheels. The original's suspensions measure at the end of
/// each sub-step (0x5A87A0 from 0x5B04E0), so its +0x1C / +0x20 are the pose as
/// it stands; the clone measures at the start of the next and keeps the
/// measurement before (`WheelState::compression` is one sub-step behind, the
/// rate comes from the difference over the last sub-step). So the clone takes
/// the compression the rate came from: with the original's own (+0x1C) the
/// first sub-step read a rate of about 0 and the body's vertical motion parted
/// in the first frame (vertical speed 0.031 against 0.034 m/s).
fn put_suspensions(s: &mut CarState, v: &[f32]) {
    for i in 0..4 {
        s.wheels[i].compression = v[i] - v[4 + i] * s.last_sub;
        s.wheels[i].compression_rate = v[4 + i];
    }
}

/// The drivetrain part of a plan `stance_at` line: the gear (0 R, 1 N, 2
/// first, as car +0x434 and the clone's drivetrain count), the engine's and
/// the input shaft's speed (rad/s), the clutch, the wheels' spin and the
/// target gear (drivetrain +0x38, the gear a shift under way goes to), the
/// shown rpm (car +0x440) and the shift state (+0x44: 0 disengaging .. 3
/// locked).
fn put_drivetrain(s: &mut CarState, v: &[f32]) {
    s.shown_rpm = v[9];
    if let Some(d) = s.drivetrain.as_mut() {
        d.shown_rpm = Some(v[9]);
        d.shift = match v[10] as u8 {
            0 => nfsu2::handling::Shift::Disengaging,
            1 => nfsu2::handling::Shift::Changing,
            2 => nfsu2::handling::Shift::Engaging,
            _ => nfsu2::handling::Shift::Locked,
        };
        d.gear = v[0] as usize;
        d.engine_w = v[1];
        d.input_w = v[2];
        d.clutch = v[3];
        d.target_gear = v[8] as usize;
        d.pending = d.gear != d.target_gear;
    }
    for i in 0..4 {
        s.wheels[i].spin = v[4 + i];
    }
}

/// The tyre and steering state of a plan `tyres_at` line: per wheel (FL FR
/// RR RL) the lock flag (wheel +0x10C: a locked wheel takes no drive or
/// brake torque in the tyre pass 0x5A8290 and the split 0x594A40, and its
/// spin rate is zeroed, 0x59D3CD) and the net torque's running mean
/// (+0x120, which 0x59D360 averages with the step's), then the steering's
/// wheel angle (input +0x1E0, degrees, 0..360), the lock last allowed
/// (PlayerSteering +0x80), the counter-steer latch (+0x10) and last frame's
/// shaped input (+0x84), the handbrake ramp (mover +0x70) and the wheel
/// hop (car +0x46C count, +0x470 step timer, +0x474 grip, +0x478 calm), and
/// the rigid body's rest damping (+0x90, which the integration takes off
/// the momenta while the car is held: a clone that stood still before the
/// transplant had built its own up), the engine's power factor (+0x80, set
/// by the clutch's tidy-up 0x5A1410 before the force pass), the clutch's last
/// slip (+0x58) and the gear change's three random offsets (+0x78, +0x7C,
/// +0x80, drawn at each change: the next shift point).
/// The driveline's reaction on the crank (engine +0x7C) is not carried: the
/// original's state at a frame's end is the one after the force pass that
/// its mover's post-step (0x5B04E0 → 0x5B0530) runs for the next sub-step,
/// so its +0x7C already holds that pass's reaction (0x594A40, 0x5AA840); the
/// clone runs the same pass at the start of its next frame and builds the
/// same reaction from the 0 its tidy-up left (0x5AA770). Carrying it counted
/// it twice: in a disengaging shift the engine then slowed by 1.17 instead of
/// 1.35 rad/s in the first frame.
/// The steering's two running means (+0x14, +0x48) keep their samples and
/// are not carried: a scenario holds its keys long enough before the
/// transplant that both sides' means are the same (the fields `steer_avg`,
/// `steer_rate` show it).
fn put_tyres(s: &mut CarState, v: &[f32]) {
    for i in 0..4 {
        s.wheels[i].locked = v[i] != 0.0;
        s.wheels[i].net_torque = v[4 + i];
    }
    s.steer_degrees = if v[8] >= 180.0 { v[8] - 360.0 } else { v[8] };
    s.steer_limit = v[9];
    s.drifting = v[10] != 0.0;
    s.last_steer_input = v[11];
    s.handbrake_ramp = v[12];
    s.hop = v[13] as i32;
    s.hop_timer = v[14];
    s.hop_grip = v[15];
    s.hop_calm = v[16];
    s.rest_damping = v[17];
    if let Some(d) = s.drivetrain.as_mut() {
        d.reaction = 0.0;
        d.power = v[18];
        d.set_last_slip_rpm(v[19]);
        d.set_shift_jitter([v[20], v[21], v[22]]);
    }
}

/// A Quick Race mode by its `NFSU2_QR_MODE` name.
fn qr_mode_named(name: &str) -> Result<screens::QrMode> {
    Ok(match name {
        "circuit" => screens::QrMode::Circuit,
        "sprint" => screens::QrMode::Sprint,
        "drag" => screens::QrMode::Drag,
        "drift" => screens::QrMode::Drift,
        "streetx" => screens::QrMode::StreetX,
        "url" => screens::QrMode::Url,
        _ => anyhow::bail!("quick race mode {name}?"),
    })
}

/// The plan's key events whose frame passes `when`.
fn press(game: &mut Game, plan: &Plan, when: impl Fn(i64) -> bool) -> Result<()> {
    for (_, down, k) in plan.keys.iter().filter(|(at, ..)| when(*at)) {
        let code = key_code(k)?;
        if *down {
            // A shift is an edge (the original's 0x407FC0 / 0x4080B0 on the
            // press): the press goes through the game's key handler as the
            // window's would, which shifts the manual gearbox at once (before, a
            // held key in `keys` alone never shifted the clone's player).
            if matches!(k.as_str(), "shift_up" | "shift_down") && !game.platform.keys.contains(&code) {
                game.key(code, &mut || {})?;
            }
            game.platform.keys.insert(code);
        } else {
            game.platform.keys.remove(&code);
        }
    }
    Ok(())
}

/// The plan's coordinate taps as they run: the next tap's index and the
/// keys held with the frame each goes up.
#[derive(Default)]
struct Taps {
    next: usize,
    held: Vec<(usize, KeyCode)>,
}

impl Taps {
    /// Frame `f`'s taps (after its record, before its update, as the
    /// plan's keys): the keys whose hold ends, then the next tap's key once
    /// the player's race coordinate `coord` reaches it (one a frame, in
    /// order).
    fn step(
        &mut self,
        taps: &[(f32, usize, String)],
        f: usize,
        coord: Option<f32>,
    ) -> Result<(Vec<KeyCode>, Option<KeyCode>)> {
        let mut up = Vec::new();
        self.held.retain(|&(at, code)| {
            if at <= f {
                up.push(code);
                false
            } else {
                true
            }
        });
        let mut down = None;
        if let (Some(coord), Some((at, hold, key))) = (coord, taps.get(self.next))
            && coord >= *at
        {
            let code = key_code(key)?;
            self.held.push((f + (*hold).max(1), code));
            self.next += 1;
            down = Some(code);
            println!("parity: tap {key} at frame {f}, race coordinate {coord:.1} (plan {at:.1})");
        }
        Ok((up, down))
    }

    fn frame(&mut self, game: &mut Game, taps: &[(f32, usize, String)], f: usize) -> Result<()> {
        let coord = match &game.mode {
            Mode::Race(r) => Some(r.racers.me.along),
            _ => None,
        };
        let (up, down) = self.step(taps, f, coord)?;
        for code in up {
            game.platform.keys.remove(&code);
        }
        if let Some(code) = down {
            game.platform.keys.insert(code);
        }
        Ok(())
    }
}

pub(crate) fn flow_parity(game: &mut Game, plan_path: &str) -> Result<()> {
    let plan = Plan::read(plan_path)?;
    let bytes = std::fs::read(&plan.save).with_context(|| format!("save {}", plan.save))?;
    let save = nfsu2::formats::native_save::NativeSave::parse(&bytes)?;
    profiles::write(&plan.profile, &save)?;
    flow_resume_to_explore(game, plan.profile.clone())?;
    let quick = match plan.quick.as_ref() {
        Some((mode, ko, car)) => {
            let mode = qr_mode_named(mode)?;
            game.quick.tracks = game.quick_track_select(mode, 0.0);
            if *ko {
                game.settings.qr[0].lap_ko = true;
            }
            let e = game.quick_event(mode);
            game.quick.car = Some(car.clone());
            Some(e)
        }
        None => None,
    };
    if let Some(hash) = plan.event {
        anyhow::ensure!(game.game_data.career.events.iter().any(|e| e.unlock == hash), "event {hash:08X}");
    }
    if let Some(e) = quick
        .or_else(|| plan.event.and_then(|hash| game.game_data.career.events.iter().find(|e| e.unlock == hash).map(|e| e.index)))
    {
        game.dev.parity_grid_at = plan.player_slot;
        game.start_race(e, quick.is_some(), false)?;
        press(game, &plan, |f| f < 0)?;
        // The start scene and the countdown as the window runs them, to
        // the green (frame 0 = the first frame with the clock running, as
        // the original's race status 4).
        let mut n = 0;
        loop {
            let Mode::Race(r) = &game.mode else {
                anyhow::bail!("left the race before the green ({})", flow_mode(&game.mode))
            };
            if r.timing.clock >= 0.0 || (plan.countdown && race_status(game, r) == 3) {
                break;
            }
            game.advance(1.0 / 60.0)?;
            n += 1;
            anyhow::ensure!(n < 60 * 120, "no green within 120 s");
        }
        if !place_on_grid(game, &plan.grid)
            && let Some(at) = plan.player_slot
        {
            swap_to_slot(game, at);
        }
        if let (Some(v), Some(d)) = (plan.stance.as_ref(), game.sim.drive.as_mut()) {
            put_stance(d, v);
            println!("parity: the player put on the original's stance");
        }
        if let (Some(n), Mode::Race(r)) = (plan.world_n, &mut game.mode) {
            r.replay.world_updates = n;
        }
        if let (Some(x), Mode::Race(r)) = (plan.world_rng, &mut game.mode) {
            r.racers.cars.rng = nfsu2::opponents::GameRng(x);
        }
    } else if let Some(pose) = plan.outrun.as_ref() {
        start_outrun(game, pose, plan.tick)?;
    } else if let Some(pose) = plan.encounter.as_ref() {
        start_encounter(game, pose, plan.tick)?;
    } else if plan.free_roam
        && let Some((at, heading)) = plan.place
        && let Some(d) = game.sim.drive.as_mut()
    {
        // The resume's own pose when it is the original's (the state's
        // engine and wheels kept), else the original's pose.
        let ground = d.world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
        let fwd = d.state.forward().truncate().normalize_or_zero();
        if (d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass)).distance(at)
            > 1.0
            || fwd.dot(Vec2::from_angle(heading.to_radians())) < 0.999
        {
            // The driver's flags stay (car +0x464, the Stability Control option: a fresh
            // placed state has it clear, so the rear wheels lost the original's launch
            // factor 0x59D4A0 under 50 mph).
            let (manual, stability_off) = (d.state.manual, d.state.stability_off);
            d.state = CarState::placed(&d.car.car_type, &d.world, Vec3::new(at.x, at.y, ground), heading.to_radians());
            d.state.manual = manual;
            d.state.stability_off = stability_off;
        }
    }
    let ai: Vec<bool> = cars(game).iter().map(|c| c.ai.is_some()).collect();
    let traffic: Vec<bool> = cars(game).iter().map(|c| c.traffic.is_some()).collect();
    let n = ai.len();
    let names: Vec<String> = cars(game).iter().map(|c| c.name.clone()).collect();
    let mut out = String::new();
    out += &header(&plan, n, &ai, &traffic);
    out.push('\n');
    let mut clock = nfsu2::frame_clock::FrameClock::default();
    let mut last_ticks = 0;
    let mut taps = Taps::default();
    // NFSU2_PARITY_PCM=<out.f32>: the run's mix through the offline mixer
    // (48 kHz stereo f32, pumped by each frame's dt) for a level
    // comparison with the original's `--audio-file` capture; with
    // NFSU2_EVENT_LOG the AEMS rows carry the frame time (frame / 60).
    let pcm_out = std::env::var_os("NFSU2_PARITY_PCM");
    let (mut pcm, mut owed) = (Vec::new(), 0.0f64);
    if pcm_out.is_some() {
        game.audio = Some(game.new_audio(Mixer::offline(48000)));
    }
    let enc_out = std::env::var_os("NFSU2_ENC_POLL");
    let node_out = std::env::var_os("NFSU2_NODE_LOG");
    let mut nodes = String::from("frame\trecord\tcp\tn_node_update\tn_node_search\n");
    let mut enc = String::from(ENC_HEADER);
    for f in 0..=plan.frames {
        out += &row(game, &plan, f, last_ticks, n);
        out.push('\n');
        if enc_out.is_some() {
            enc_rows(game, &mut enc);
        }
        // NFSU2_NODE_LOG: the player's road node (record id, point)
        // and the 0x411490 / 0x40F180 counts per frame.
        if node_out.is_some()
            && let Some(d) = game.sim.drive.as_ref()
        {
            let routes = match &game.mode {
                Mode::Race(r) => r.racers.ai_routes.as_ref().map(|a| &a.file),
                _ => game.sim.encounters.as_ref().map(|e| &e.routes.file),
            };
            let (id, cp) = d.human_road.node.map_or((-1, -1), |(r, c)| (routes.map_or(-1, |f| i32::from(f.records[r].id)), c as i32));
            nodes.push_str(&format!("{f}\t{id}\t{cp}\t{}\t{}\n", d.node_counts.0, d.node_counts.1));
        }
        if f == plan.frames {
            break;
        }
        press(game, &plan, |at| at == f as i64)?;
        taps.frame(game, &plan.taps, f)?;
        if let Some((_, com, vel, fwd)) = plan.pose.filter(|p| p.0 == f)
            && let Some(d) = game.sim.drive.as_mut()
        {
            let heading = fwd.y.atan2(fwd.x);
            let pitch = fwd.z.clamp(-1.0, 1.0).asin();
            let q = glam::Quat::from_rotation_z(heading) * glam::Quat::from_rotation_y(-pitch);
            let s = &mut d.state;
            s.orientation = q;
            s.position = com - q * Vec3::from(d.car.car_type.handling.centre_of_mass);
            s.velocity = vel;
            s.spin = Vec3::ZERO;
            s.angular_momentum = Vec3::ZERO;
        }
        if let Some((_, v)) = plan.stance_at.as_ref().filter(|p| p.0 == f)
            && let Some(d) = game.sim.drive.as_mut()
        {
            put_stance(d, &v[..30]);
            put_drivetrain(&mut d.state, &v[30..]);
        }
        if let Some((_, v)) = plan.tyres_at.as_ref().filter(|p| p.0 == f)
            && let Some(d) = game.sim.drive.as_mut()
        {
            put_tyres(&mut d.state, v);
        }
        nfsu2::audio::aems::log::set_log_time(f as f64 / 60.0);
        let (dt, ticks) = match plan.ticks.get(f + 1) {
            Some(&t) if t > 0 => (t as f32 / 1800.0, t),
            _ => clock.frame(1.0 / 60.0),
        };
        // The original's own frames' ticks (its capture), off the clone's
        // cadence by design: a parity replay (the one run off the tick cadence).
        super::ticks::off_cadence(|| game.step(dt))?;
        last_ticks = ticks;
        if let Some(a) = game.audio.as_ref().filter(|_| pcm_out.is_some()) {
            owed += f64::from(dt) * 48000.0;
            let k = owed.floor();
            owed -= k;
            a.mixer.pump(k as usize, &mut pcm);
        }
    }
    if let Some(p) = pcm_out {
        std::fs::write(p, pcm.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())?;
    }
    if let Some(p) = enc_out {
        std::fs::write(p, enc)?;
    }
    if let Some(p) = node_out {
        std::fs::write(p, nodes)?;
    }
    std::fs::write(&plan.out, out)?;
    std::fs::write(format!("{}.cars", plan.out), names.join("\n") + "\n")?;
    // The traffic slots (diff.py leaves traffic out of the car match;
    // the maintainer's tool `trafficstats.py` reads them).
    std::fs::write(
        format!("{}.traffic", plan.out),
        traffic.iter().enumerate().filter(|(_, t)| **t).map(|(k, _)| k.to_string()).collect::<Vec<_>>().join(" ")
            + "\n",
    )?;
    game.platform.keys.clear();
    // The race's outcome when it ended within the run: the results
    // board, the reward screen's points and bank, and the settled result.
    if let Mode::Menu(Menu::Results(results, reward)) = &game.mode {
        for (k, (name, car, time, points)) in results.rows.iter().enumerate() {
            println!("parity: result {} {name} {car} {time:.2} points {points}", k + 1);
        }
        if let Some(r) = reward {
            println!("parity: reward reputation {} bank {}", r.reputation, r.bank);
        }
    }
    if let Some(r) = game.race_flow.result.as_ref() {
        println!(
            "parity: race result won {} time {:.3} reputation {} bank {} pays {}",
            r.won, r.time, r.reward.0, r.reward.1, r.pays
        );
    }
    // What the reward screen's Continue leads to (0x4F3EA0 / 0x4A4610): the
    // sponsor popup while a sponsor is on offer, with the offers.
    if let (Some(p), Mode::Menu(Menu::Results(_, Some(_)))) = (game.profile.as_ref(), &game.mode) {
        let offers: Vec<String> = p.sponsor_offers(&game.game_data.career).iter().map(|s| s.name.clone()).collect();
        println!("parity: after the reward: sponsor popup {} offers {}", p.needs_sponsor(&game.game_data.career), offers.join(","));
    }
    println!("parity: {} frames, {} cars ({}) -> {}", plan.frames + 1, n, names.join(", "), plan.out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // The test kit (tests/common/mod.rs, docs/testing.md).
    use crate::tests::testkit;

    /// Every clone accessor the maintainer's `fields.toml` names is implemented
    /// (an unknown one would record NaN and never be compared).
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn every_field_accessor_is_known() {
        let text = testkit::private_str("tools/parity/fields.toml");
        let mut n = 0;
        for line in text.lines().filter(|l| !l.trim_start().starts_with('#') && l.contains("clone = \"")) {
            let acc = line.split("clone = \"").nth(1).and_then(|r| r.split('"').next()).unwrap();
            let car = line.contains("scope = \"car\"");
            let known = if car {
                const CAR: [&str; 145] = [
                    "lock_fl",
                    "lock_fr",
                    "lock_rr",
                    "lock_rl",
                    "ntq_fl",
                    "ntq_fr",
                    "ntq_rr",
                    "ntq_rl",
                    "torq_fl",
                    "torq_fr",
                    "torq_rr",
                    "steer_drift",
                    "steer_avg",
                    "steer_rate",
                    "steer_lim",
                    "steer_last",
                    "hb_ramp",
                    "hop_t",
                    "hop_calm",
                    "rb_damp",
                    "since_collision",
                    "comp_fl",
                    "comp_fr",
                    "comp_rr",
                    "comp_rl",
                    "crate_fl",
                    "crate_fr",
                    "crate_rr",
                    "crate_rl",
                    "fy_fl",
                    "fy_fr",
                    "fy_rr",
                    "fy_rl",
                    "fx_fl",
                    "fx_fr",
                    "fx_rr",
                    "fx_rl",
                    "yslip_fl",
                    "yslip_fr",
                    "yslip_rr",
                    "yslip_rl",
                    "load_fr",
                    "rb_row0",
                    "rb_row1",
                    "rb_row2",
                    "rb_spin",
                    "rb_angmom",
                    "rest",
                    "rcoord",
                    "ai_obj",
                    "ai_plan_off",
                    "ai_plan_rate",
                    "hop_load_fl",
                    "hop_load_rr",
                    "hop_load_rl",
                    "dt_70",
                    "hop_grip",
                    "dt_f0",
                    "dt_e4",
                    "dt_up2",
                    "dt_up3",
                    "dt_up4",
                    "dt_up5",
                    "dt_up6",
                    "dt_up7",
                    "heat",
                    "stopped",
                    "recs",
                    "dt_68",
                    "dt_78",
                    "dt_80",
                    "dt_cc",
                    "dt_d0",
                    "dt_dc",
                    "dt_e0",
                    "dt_e8",
                    "drift_surf",
                    "drift_near",
                    "behind",
                    "drift_state",
                    "drift_pts",
                    "drift_bank",
                    "drift_mult",
                    "drift_total",
                    "drift_level",
                    "yaw_rate",
                    "xspeed",
                    "hop",
                    "slip_fl",
                    "slip_fr",
                    "spin_fr",
                    "slide_fr",
                    "slide_rr",
                    "xslip_fl",
                    "xslip_fr",
                    "xslip_rr",
                    "xslip_rl",
                    "load_fl",
                    "grip_fl",
                    "slide_fl",
                    "stab",
                    "grip_rl",
                    "load_rr",
                    "slip_rr",
                    "slide_rl",
                    "load_rl",
                    "torq_rl",
                    "eng_w",
                    "eng_pedal",
                    "eng_power",
                    "eng_cut",
                    "clutch",
                    "shift",
                    "in_w",
                    "spin_fl",
                    "spin_rl",
                    "spin_rr",
                    "slip_rl",
                    "ai_catch",
                    "ai_look",
                    "ai_path_n",
                    "ai_target",
                    "wheels",
                    "context",
                    "pos",
                    "vel",
                    "accel",
                    "fwd",
                    "speed",
                    "gear",
                    "rpm",
                    "steer",
                    "gas",
                    "brake",
                    "handbrake",
                    "nos_left",
                    "ai_goal",
                    "ai_action",
                    "lap",
                    "place",
                    "progress",
                    "human",
                    "mover",
                    "traffic_action",
                    "parked",
                ];
                CAR.contains(&acc)
            } else {
                ["race_state", "race_clock", "race_type", "cash", "context", "cam_eye", "cam_fwd", "cam_fov", "world_rng", "shake_vec", "shake_t", "shake_on"]
                    .contains(&acc)
            };
            assert!(known, "fields.toml: clone accessor `{acc}` is not implemented in parity.rs");
            n += 1;
        }
        assert!(n >= 10, "fields.toml read ({n} fields)");
    }

    #[test]
    fn plans_parse() {
        let dir = std::env::temp_dir().join(format!("parity-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.plan");
        std::fs::write(&p, "# c\nsave /a.sav\nevent 4F7D2AC5\nframes 10\nticks 0 30 29 31\nkey -1 down throttle\nkey 3 up throttle\nfield car pos pos 3 0\nfield global race_clock race_clock 1 0\nout /x.tsv\n").unwrap();
        let plan = Plan::read(p.to_str().unwrap()).unwrap();
        assert_eq!(plan.event, Some(0x4F7D_2AC5));
        assert_eq!(plan.ticks, vec![0, 30, 29, 31]);
        assert_eq!(plan.keys, vec![(-1, true, "throttle".to_string()), (3, false, "throttle".to_string())]);
        assert_eq!(
            header(&plan, 2, &[false, true], &[false, false]),
            "frame\tticks\tclock\trace_clock\tc0.pos.x\tc0.pos.y\tc0.pos.z\tc1.pos.x\tc1.pos.y\tc1.pos.z"
        );
        assert!(key_code("nitro").is_ok() && key_code("warp").is_err());
        std::fs::write(&p, "save /a.sav\nframes 10\ntap 812.5 6 left\ntap 900 6 right\nout /x.tsv\n").unwrap();
        let plan = Plan::read(p.to_str().unwrap()).unwrap();
        assert_eq!(plan.taps, vec![(812.5, 6, "left".to_string()), (900.0, 6, "right".to_string())]);
        // The full transplant: the stance's 30 numbers, then gear, engine, input shaft, clutch,
        // four wheel spins, target gear, shown rpm, shift state.
        let tail = "6 597.3 597.1 1 186 186 187 187 6 5712 3";
        let line = format!("stance_at 50 {} {tail}", vec!["0"; 30].join(" "));
        std::fs::write(&p, format!("save /a.sav\nframes 60\n{line}\nout /x.tsv\n")).unwrap();
        let plan = Plan::read(p.to_str().unwrap()).unwrap();
        let (f, v) = plan.stance_at.clone().unwrap();
        assert_eq!((f, v.len(), v[30], v[38], v[40]), (50, 41, 6.0, 6.0, 3.0));
        std::fs::write(&p, format!("save /a.sav\nframes 60\nstance_at 50 {}\nout /x.tsv\n", vec!["0"; 38].join(" "))).unwrap();
        assert!(Plan::read(p.to_str().unwrap()).is_err(), "a stance_at line without the drivetrain part is refused");
        // The tyre state: four lock flags, four net torque means, the steering's angle, lock,
        // latch and last input, the handbrake ramp, the wheel hop's four values, the rest damping, the
        // engine's power factor, the clutch's last slip and the shift offsets.
        let tyres = "1 0 0 1 -0.5 0.25 3 4 359.5 12.5 0 -1 0.5 2 0.25 0.9 0.1 0.02 0.1 -2500 -10 -20 -30";
        std::fs::write(&p, format!("save /a.sav\nframes 60\ntyres_at 50 {tyres}\nout /x.tsv\n")).unwrap();
        let plan = Plan::read(p.to_str().unwrap()).unwrap();
        let (f, v) = plan.tyres_at.clone().unwrap();
        assert_eq!((f, v.len(), v[0], v[8], v[16]), (50, TYRES_AT, 1.0, 359.5, 0.1));
        let mut s = CarState::new(Vec3::ZERO, 0.0);
        put_tyres(&mut s, &v);
        assert!(s.wheels[0].locked && !s.wheels[1].locked && s.wheels[3].locked);
        assert_eq!((s.wheels[2].net_torque, s.steer_degrees, s.hop), (3.0, -0.5, 2));
        // The suspensions go in one sub-step behind: the rate the clone's first sub-step
        // works out from the pose is the original's.
        s.last_sub = 1.0 / 120.0;
        put_suspensions(&mut s, &[0.1, 0.1, 0.05, 0.05, -0.6, 0.0, 1.2, 0.0]);
        let x = s.wheels[0].compression;
        assert!(((0.1 - x) / s.last_sub - -0.6).abs() < 1e-4 && s.wheels[1].compression == 0.1);
        assert!(((0.05 - s.wheels[2].compression) / s.last_sub - 1.2).abs() < 1e-4);
        std::fs::write(&p, "save /a.sav\nframes 60\ntyres_at 50 1 0 0\nout /x.tsv\n").unwrap();
        assert!(Plan::read(p.to_str().unwrap()).is_err(), "a short tyres_at line is refused");
        std::fs::write(&p, "frames 3\n").unwrap();
        assert!(Plan::read(p.to_str().unwrap()).is_err(), "a plan without save/out is refused");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A tap goes down on the first frame the race coordinate reaches
    /// it and up its hold later; the next waits for its own coordinate.
    #[test]
    fn taps_follow_the_race_coordinate() {
        let taps = vec![(100.0, 6, "left".to_string()), (130.0, 6, "right".to_string())];
        let mut t = Taps::default();
        let mut log = Vec::new();
        for f in 0..40usize {
            let coord = 90.0 + f as f32; // 129 at frame 39: the second tap waits
            let (up, down) = t.step(&taps, f, Some(coord)).unwrap();
            log.extend(up.into_iter().map(|k| (f, false, k)));
            log.extend(down.into_iter().map(|k| (f, true, k)));
        }
        assert_eq!(log, vec![(10, true, KeyCode::ArrowLeft), (16, false, KeyCode::ArrowLeft)]);
        assert!(t.step(&taps, 41, None).unwrap().1.is_none(), "no coordinate outside a race");
        assert_eq!(t.step(&taps, 42, Some(130.0)).unwrap().1, Some(KeyCode::ArrowRight));
    }

    #[test]
    fn numbers_print_as_the_tsv_expects() {
        assert_eq!(num(f64::NAN), "");
        assert_eq!(num(4.0), "4");
        assert_eq!(num(-0.5), "-0.5");
        // The shortest text that reads back as the same f32.
        let x = 0.000_590_980_05_f32;
        assert_eq!(num(x as f64).parse::<f32>().unwrap().to_bits(), x.to_bits());
        assert_eq!(num(3_000_000_000.0), "3000000000");
    }
}
