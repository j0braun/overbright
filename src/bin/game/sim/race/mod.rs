use crate::*;

mod consts;
mod progress;
mod rules;
mod scene;
pub(crate) mod frame;
pub(crate) mod step;
pub(crate) use consts::*;
pub(crate) use progress::*;
pub(crate) use rules::*;
pub(crate) use scene::*;

/// What the AI racers share, from GLOBALB: the tuning rows (0x34B00), the
/// upgrade presets (0x34A31), the rating bounds (0x34A32) and every car
/// type's scaled rating.
pub(crate) struct AiData {
    pub(crate) tuning: nfsu2::ai::route::AiTuning,
    pub(crate) presets: nfsu2::ai::route::Presets,
    pub(crate) bounds: nfsu2::opponents::Bounds,
    /// The racers' top speed and acceleration by class (0x34A33).
    pub(crate) curves: Option<nfsu2::ai::route::ClassCurves>,
}

/// A race in progress.
pub(crate) struct Race {
    pub(crate) event: usize,
    pub(crate) camera: RaceCamera,
    pub(crate) route: Route,
    pub(crate) laps: u32,
    /// Run backward (variant bit 0; the reverse flag 0x89E7A4, which also
    /// picks the scenery the world views cull, 0x4888A0).
    pub(crate) reversed: bool,
    pub(crate) racers: RaceField,
    pub(crate) timing: RaceClock,
    /// A Quick Race: no career effects.
    pub(crate) quick: bool,
    /// Started from the World Map (0x85BA67): its menus offer going back.
    pub(crate) from_map: bool,
    /// Seconds since the finish: the race-over message shows this long
    /// before the reward screen.
    pub(crate) over: f32,
    /// The player is going the wrong way (racer +0x248, 0x600720).
    pub(crate) wrong_way: bool,
    /// Which of the event's races this is (a tournament has 2–3), and
    /// the times of those before.
    pub(crate) round: usize,
    /// The event's racers carried from race to race.
    pub(crate) field: Field,
    pub(crate) scoring: RaceScoring,
    /// The racers in race order (race status +0xBD0: 0 the player, 1.. the
    /// rivals), re-sorted each frame, and the reference lap the gaps are
    /// read through (+0xC34).
    pub(crate) order: Vec<usize>,
    pub(crate) reference: nfsu2::standings::LapReference,
    /// A drag race (its racers drop out when their engine blows).
    pub(crate) drag: bool,
    /// The track's start gap for the race's direction (track +0xF8 +
    /// 4·0x89E7A4; None when its byte +0xF4 + 0x89E7A4 is set): before the
    /// green the drag placement refuses traffic nearer the start line
    /// (0x418110).
    pub(crate) start_gap: Option<f32>,
    /// The player's DragCarController in a drag race (made on its first
    /// frame).
    pub(crate) dragger: Option<DragSteer>,
    /// The AI racer controller that drives the player's car past the line.
    pub(crate) autopilot: Option<Autopilot>,
    pub(crate) replay: RaceReplay,
    /// A dyno test run (0x526980(0)): its track and race flags; it has no
    /// event (`event` is unused).
    pub(crate) test: Option<(u16, RaceMode)>,
    /// A Quick Race's options (its mode record, 0x53F160): the catch-up
    /// (0x89E7F8) and Lap KO's knockout (race type 0x89E7B0 = 3), with
    /// each racer's laps done last frame for it.
    /// Some(traffic level 2 or 3, its curve choice) with the catch-up on.
    pub(crate) catch_up: Option<bool>,
    /// The human's catch-up scale 0x89E7FC (Quick Race: tuning +0x94C by
    /// difficulty and the human car's class, see `AiTuning::catch_up_scale`;
    /// 1 otherwise) and the Quick Race difficulty it is read with.
    pub(crate) catch_up_difficulty: Option<usize>,
    /// The human's line-speed mean (player +0xA0, 0x579F30(10)) and its
    /// one-second timer (+0xA4), pushed by the player update at 0x602842.
    pub(crate) human_line: nfsu2::ai::driver::RunningMean,
    pub(crate) human_line_timer: f32,
    /// Its start scene started it (`Game::race_start_scene`).
    pub(crate) scene_start: bool,
    /// The race clock at which its start scene stops (its length; −∞
    /// with none). While a scene runs the player's +0x234 holds its id
    /// (0x437910 sets it, the scene stop 0x439070 clears it at 0x439109),
    /// and 0x6000A0's tail adds the countdown element 0x20000 only with
    /// +0x234 = 0 (0x600555…0x600563), so neither the 3-2-1 (race state
    /// 3 from the scene's cut sets +8, 0x5FC910(3)) nor "GO!" (+4 from
    /// the green, for +0xC) shows until the scene has stopped
    /// (**verified**; **observed** under Wine, raceflow: +0x234 = 1 from
    /// the scene's start to 4.1 s past the green in a Quick Race sprint,
    /// no 3-2-1 or "GO!" drawn in circuit and sprint).
    pub(crate) scene_end: f32,
    /// The player's grid slot, 0-based (racer info +0x03 − 1, 0x5264B0).
    pub(crate) grid_slot: usize,
    /// Race status 2: the start scene's camera runs (to its cut). The
    /// rivals' drivers do not rev on the line then (their countdown branch
    /// is race status 3 only, 0x421030; **observed**: the rivals' engines
    /// idle, ≈ 850 rpm, at the countdown's first frame).
    pub(crate) scene_camera: bool,
}

/// The race's clocks: the race clock, its origin and countdown step, the
/// drift time-out, the first finish and the player's hold.
pub(crate) struct RaceClock {
    /// Seconds since the green light (negative during the countdown).
    pub(crate) clock: f32,
    /// The race clock at its set-up (the load): −the countdown, or with a
    /// start scene −its green mark (`Input::race_age` counts from here).
    pub(crate) origin: f32,
    /// The countdown's step (race status +0xC): 0.9 s (0x5FC740), or
    /// the picked start scene's third of its two marks' gap (0x437910:
    /// (header +0x2C / 30 − header +0x30 / 30) / 3; **observed** under
    /// Wine, raceflow: 0.889 on Resort Loop, 0.989 on a sprint, the green
    /// three steps after the cut).
    pub(crate) countdown_step: f32,
    /// A drift race's time-out (race status +0x1C / +0x20): the race
    /// clock when the first racer to finish, an AI racer, did (armed by
    /// 0x606CC0 at 0x607004; the length is [`HURRY_TIME`]).
    pub(crate) hurry: Option<f32>,
    /// A racer has finished (place byte +0x0C = 1 taken).
    pub(crate) first_done: bool,
    /// The player's hold after its finish (+0x54, set once by 0x5EA540
    /// at the finish): 6 s (0x7A54A4) when its running event kind +0x58
    /// is 6 or 7 (a crash / jump slow-motion camera), else 3 s
    /// (0x7A54A8, `RACE_OVER`); None before the finish.
    pub(crate) hold: Option<f32>,
    /// The race status's +0x10: the green's world tick rounded half up
    /// to 0.01 s (0x5FC910(4): +0x10 = 0x865188, 0x577410(0.01)); None
    /// before the green.
    pub(crate) green_tick: Option<i32>,
    /// The race status's +0x14, the race clock in ticks: written only in
    /// state 4 (racing) by 0x6076E0 as 0x865188 − +0x10 rounded half up to
    /// 0.01 s (0x607726..0x60773E, 0x577410), 0 through the start scene
    /// and the countdown (the reset 0x5FC7F4), frozen once the race leaves
    /// state 4 (**verified** + **measured**).
    pub(crate) status_clock: i32,
}

/// The cars in the race: the player's run, the opponents, the traffic and
/// the AI's shared state.
pub(crate) struct RaceField {
    /// The player's run.
    pub(crate) me: Progress,
    /// The opponents.
    pub(crate) rivals: Vec<Rival>,
    /// The race's traffic (its level 0x89E7A8 from the event or the Quick
    /// Race mode record; none at level 0, in a test run, whose race-type
    /// default 0x4FEF80 … clears the level, or in a drag race, whose
    /// placement 0x42E620 is not ported).
    pub(crate) traffic: Option<Traffic>,
    /// The track's AI routes (Routes<track><F|B>.bin) and the cars' shared
    /// collision state (pairs, the world's random numbers).
    pub(crate) ai_routes: Option<nfsu2::ai::route::AiRoutes>,
    pub(crate) cars: nfsu2::drive::CarField,
    /// The world update counter the racers' full thinks take turns by
    /// (0x7FB724 mod 8, one per race frame).
    pub(crate) ai_frames: u64,
}

/// The player's scoring: style and drift managers, the drift board and
/// the knockout record.
pub(crate) struct RaceScoring {
    /// The player's style manager and, in a drift race, drift manager
    /// (racer +0x1D4 / +0x1D8, 0x5F7A10), and the last drift tier
    /// announced with when.
    pub(crate) style: Option<nfsu2::drift::StyleManager>,
    pub(crate) drift: Option<nfsu2::drift::DriftManager>,
    pub(crate) announce: Option<(u32, f32)>,
    /// The drift angle the HUD shows (0x4AB390).
    pub(crate) drift_angle: f32,
    /// The Race Breaker flag (0x89E7E7): style points refill the nitrous
    /// (0x601737). 1 from the race set-up's reset (0x5ED600); the Quick
    /// Race drift (0x53F935) and street X (0x53F459) set-ups clear it.
    pub(crate) race_breaker: bool,
    /// The style total already turned into nitrous (racer +0x194).
    pub(crate) style_base: f32,
    /// The player's misc style manager (racer +0x1DC, 0x56CE00).
    pub(crate) misc: nfsu2::style::Misc,
    /// A drift race's standings by points (0x864630; see
    /// [`nfsu2::drift::BoardEntry`]), highest first; empty outside drift.
    pub(crate) board: Vec<nfsu2::drift::BoardEntry>,
    pub(crate) knockout: bool,
    pub(crate) ko_seen: Vec<i32>,
}

/// The replay: the recording, the playback, the road entries' last
/// velocities and the world update counter the snapshots key on.
pub(crate) struct RaceReplay {
    /// The replay's recording from the green light, and the replay being
    /// played back (None while racing).
    pub(crate) recording: Option<Recording>,
    pub(crate) playback: Option<Playback>,
    /// Last frame's velocities of the human and the traffic, for the
    /// racers' road entries (`ai::actions::RoadCar::accel`).
    pub(crate) road_prev: std::collections::HashMap<usize, Vec3>,
    /// The world update counter 0x7FB724 as the replay manager reads it:
    /// 0 when the race's world is built (0x576D00 from the world's
    /// constructor 0x5FF54D), +1 per world update (0x57A19D), the start
    /// scene's and the countdown's included. Its multiples of 100 take the
    /// DUMMY_SNAPSHOT.
    pub(crate) world_updates: u64,
}

/// The race's event cameras: the director, the crash / jump shot and the
/// parked chase camera.
pub(crate) struct RaceCamera {
    /// The player's camera director's event cameras and the slow motion
    /// (Jump Camera option copied at the start, 0x5F180B).
    pub(crate) events: nfsu2::event_camera::EventCameras,
    /// The crash / jump camera's record while event kind 6 / 7 runs (its
    /// mover 0x0F / 0x10 only when `visible`; else the chase camera).
    pub(crate) shot: Option<nfsu2::event_camera::CrashShot>,
    /// The player's chase camera parked while event kind 6 / 7 runs
    /// (Camera_Factory 0x5F1FE0 cases 0x0F / 0x10 with the keep flag: player
    /// +0x23C), with the race clock it was parked at; `Drive::chase` is the
    /// fresh chase mover 0x451660 the factory builds when the sight test
    /// fails. Restored by `crash_camera` at the kind's end (0x5FFDE0).
    pub(crate) parked_chase: Option<(ChaseCamera, f32)>,
}

impl RaceCamera {
    /// A camera director's event fields at its construction, with the
    /// Jump Camera option copied (0x5F180B).
    pub(crate) fn new(jump_camera: bool) -> RaceCamera {
        RaceCamera { events: nfsu2::event_camera::EventCameras::new(jump_camera), shot: None, parked_chase: None }
    }
}

impl Race {
    /// A race on `route` at its set-up: every racer, score, replay and
    /// camera field empty, the clock at −the countdown. `quick` with a
    /// drift or street X `mode` clears the Race Breaker flag. The cars'
    /// field's generator (the world generator) is seeded by the world build
    /// after the line-up (`Game::world_build`). The
    /// caller sets the event's own fields (`event`, `reversed`, `from_map`,
    /// `round`, `field`, `order`, `test`, `start_gap`) afterwards.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        route: Route,
        laps: u32,
        mode: RaceMode,
        quick: bool,
        style: Option<nfsu2::drift::StyleManager>,
        drift: Option<nfsu2::drift::DriftManager>,
        jump_camera: bool,
    ) -> Race {
        Race {
            event: 0,
            camera: RaceCamera::new(jump_camera),
            route,
            laps,
            reversed: false,
            racers: RaceField {
                me: Progress::default(),
                rivals: Vec::new(),
                traffic: None,
                ai_routes: None,
                cars: nfsu2::drive::CarField::new(0),
                ai_frames: 0,
            },
            timing: RaceClock {
                clock: -COUNTDOWN,
                origin: -COUNTDOWN,
                countdown_step: COUNTDOWN_STEP,
                hurry: None,
                first_done: false,
                hold: None,
                green_tick: None,
                status_clock: 0,
            },
            quick,
            from_map: false,
            over: 0.0,
            wrong_way: false,
            round: 0,
            field: Field::default(),
            scoring: RaceScoring {
                style,
                drift,
                announce: None,
                drift_angle: 0.0,
                race_breaker: !(quick && (mode.drift() || mode.street_x())),
                style_base: 0.0,
                misc: Default::default(),
                board: Vec::new(),
                knockout: false,
                ko_seen: Vec::new(),
            },
            order: Vec::new(),
            reference: nfsu2::standings::LapReference::new(1.0, []),
            drag: mode == RaceMode::Drag,
            start_gap: None,
            dragger: None,
            autopilot: None,
            replay: RaceReplay {
                recording: None,
                playback: None,
                road_prev: Default::default(),
                world_updates: 0,
            },
            test: None,
            catch_up: None,
            catch_up_difficulty: None,
            human_line: nfsu2::ai::driver::RunningMean::new(10),
            human_line_timer: 0.0,
            scene_start: false,
            scene_end: f32::NEG_INFINITY,
            grid_slot: 0,
            scene_camera: false,
        }
    }
}
