mod clutch;
mod engine;
mod gearbox;
mod shift_grade;
mod turbo;

pub use gearbox::shift_points;
pub use shift_grade::{drag_ai_shift_setup, drag_shift_scale};
pub use turbo::Turbo;

use super::*;

thread_local! {
    /// The world generator ([0x890080] +0xF8) lent to the drivetrains for
    /// one world step ([`with_world_rng`]); None outside one.
    static WORLD_RNG: std::cell::RefCell<Option<crate::opponents::GameRng>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` (a world step: every car's motion and contact re-steps) with
/// the world's generator lent to the drivetrains, so the shift offsets'
/// rerolls (0x594720 → 0x5EA510) draw from it in the cars' step order, as
/// the original's do.
pub fn with_world_rng<R>(rng: &mut crate::opponents::GameRng, f: impl FnOnce() -> R) -> R {
    let lent = WORLD_RNG.with(|w| w.borrow_mut().replace(rng.clone()));
    let r = f();
    if let Some(back) = WORLD_RNG.with(|w| std::mem::replace(&mut *w.borrow_mut(), lent)) {
        *rng = back;
    }
    r
}

/// One draw on the lent world generator (0x5EA510: Rand_Float(scale)),
/// None when none is lent.
fn world_unit(scale: f32) -> Option<f32> {
    WORLD_RNG.with(|w| w.borrow_mut().as_mut().map(|r| r.unit(scale)))
}

/// rad/s → rpm (0x7A1D30).
pub const RPM_PER_RAD_S: f32 = 9.549304;
pub const NEUTRAL: usize = 1;

/// The gearbox state machine of the drivetrain update (0x5A1410).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    /// 0: clutch opening, engine power cut.
    Disengaging,
    /// 1: clutch open, the gear changes.
    Changing,
    /// 2: clutch closing, engine power at 0.7.
    Engaging,
    /// 3: clutch locked, full power.
    Locked,
}

#[derive(Clone, Debug)]
pub struct Drivetrain {
    /// Engine speed, rad/s (engine +0x14).
    pub engine_w: f32,
    /// The rear axle's live torque share (drivetrain +0xEC): the
    /// gearbox's (+0x2D0) from the constructor (0x5AFA69), but 0.65 for a
    /// rear-driven car against a wall under 60 mph (0x5A4D24, see
    /// `CarState::wall_drive`). The torque split (0x594A40) and the
    /// coupling's compliance (0x594E32, 0x594E66) use it; which axles
    /// count as driven stays the gearbox's.
    pub rear_share: f32,
    /// Torque the driveline puts back on the crank this sub-step, kN·m
    /// (engine +0x7C; cleared after each integration, 0x5AA770).
    pub reaction: f32,
    /// The crank torque sent to the wheels in the current sub-step
    /// (+0x70: cleared by 0x594CC0, each 0x594A40 call adds its torque).
    pub sent: f32,
    /// Gearbox input shaft speed carried from the driven wheels, rad/s
    /// (+0x4C, refreshed by the state machine 0x5A1410).
    pub input_w: f32,
    /// Clutch slip in rpm at the last sub-step (+0x58).
    last_slip_rpm: f32,
    pub gear: usize,
    pub target_gear: usize,
    /// A gear change is under way (+0x48).
    pub pending: bool,
    pub shift: Shift,
    /// 0..1 clutch engagement (+0x14).
    pub clutch: f32,
    /// Engine power factor set by the state machine (engine +0x80).
    pub power: f32,
    /// Throttle pedal (engine +0x78).
    pub throttle: f32,
    /// Fuel cut at the limiter (engine +0x88).
    /// The race catch-up (engine +0x90 = car +0x468, written each frame
    /// by 0x599180): the torque curve's value × it (0x594330), the
    /// nitrous term not. 1 outside Quick Races with Catch Up.
    pub catch_up: f32,
    pub limiter: bool,
    /// Per-gear up-shift rpm (drivetrain +0x84, 0x5AC7E0).
    pub(in crate::handling) up_rpm: Vec<f32>,
    /// Per-gear down-shift rpm (drivetrain +0xA8).
    pub(in crate::handling) down_rpm: Vec<f32>,
    /// Random offsets re-drawn at every gear change (+0x78, +0x7C, +0x80;
    /// 0x594720): each in its range ([`Handling::shift_ranges`]; −100..0
    /// unless an AI racer's in a drag race).
    pub(in crate::handling) jitter: [f32; 3],
    /// The car's shown rpm (car +0x440) as the mover hands it to the gear
    /// choice and the shift grading (0x5ABD83..0x5ABDC0: 0x5AA970's second
    /// argument, the override 0x7FBED8 being off at 100); `None` (outside
    /// a car's step: tools and tests) reads the engine's.
    pub shown_rpm: Option<f32>,
    seed: u32,
    /// Shift quality of the last up-shift in a drag race (+0x68, 0x5A1710):
    /// 1 perfect, 0.85 / 0.5 good / early, −1 over-revved; 0 otherwise.
    pub shift_bonus: f32,
    /// When the shift bonus started boosting the engine (+0x64), s.
    bonus_since: Option<f32>,
    /// Engine heat (engine +0x8C): −1 off (free roam), 0..1.2 in a drag
    /// race, 2 blown.
    pub heat: f32,
    /// Seconds simulated, for the bonus timer (the game's clock 0x865188).
    clock: f32,
    /// The clutch held open by the driver (transmission +0x74, cleared by
    /// the constructor 0x5AFB14): set each frame by the human driver's
    /// update (0x41E2C0) in a drift race (0x401950 on 0x89E7A0: not drag,
    /// with a drift flag) to handbrake (+0x210) over 0.6. Every state of
    /// 0x5A1410 then ends in Changing (clutch open, no torque through it)
    /// with full engine power, so the engine revs free; letting go closes
    /// the clutch again from where it was (a clutch kick).
    pub clutch_held: bool,
    /// The running nitrous shot's stage (engine +0x84 = 0x59FA40 each
    /// frame, 0x5ABD75): 0 none.
    pub nitrous: usize,
    /// The turbo (engine +0x98..+0xAC, [`Drivetrain::turbo`]).
    pub turbo: Turbo,
}

impl Drivetrain {
    /// Force-evaluation projection only: restore every engine/clutch field
    /// consumed before integration/post. Timer/RNG histories are not consumed
    /// by this phase; post-state is intentionally not an oracle output.
    #[cfg(test)]
    pub(crate) fn native_force_boundary(h: &Handling, engine: &[u8], clutch: &[u8]) -> Self {
        let f = |b: &[u8], o| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let u = |b: &[u8], o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut d = Self::new(h);
        d.engine_w = f(engine, 0x14);
        d.reaction = f(engine, 0x7c);
        d.throttle = f(engine, 0x78);
        d.power = f(engine, 0x80);
        d.limiter = u(engine, 0x88) != 0;
        d.heat = f(engine, 0x8c);
        d.catch_up = f(engine, 0x90);
        d.nitrous = u(engine, 0x84) as usize;
        d.input_w = f(clutch, 0x4c);
        d.last_slip_rpm = f(clutch, 0x58);
        d.clutch = f(clutch, 0x14);
        d.gear = u(clutch, 0x3c) as usize;
        d.target_gear = u(clutch, 0x38) as usize;
        d.pending = u(clutch, 0x48) != 0;
        d.shift = match u(clutch, 0x44) {
            0 => Shift::Disengaging,
            1 => Shift::Changing,
            2 => Shift::Engaging,
            3 => Shift::Locked,
            x => panic!("native shift {x}"),
        };
        d.rear_share = f(clutch, 0xec);
        d.shift_bonus = f(clutch, 0x68);
        d.clutch_held = u(clutch, 0x74) != 0;
        d.jitter = std::array::from_fn(|i| f(clutch, 0x78 + i * 4));
        d.up_rpm = (0..h.gearbox.ratios.len()).map(|i| f(clutch, 0x84 + i * 4)).collect();
        d.down_rpm = (0..h.gearbox.ratios.len()).map(|i| f(clutch, 0xa8 + i * 4)).collect();
        d.turbo = Turbo {
            max: f(engine, 0x98),
            boost: f(engine, 0x9c),
            state: u(engine, 0xa0) as u8,
            blow_from: f(engine, 0xa4),
            blow_time: f(engine, 0xa8),
            turn: f(engine, 0xac),
        };
        assert_eq!(d.shift_bonus, 0.0, "bonus clock projection unsupported");
        assert_eq!(f(clutch, 0xe8), 1.0, "extra clutch rate factor unsupported");
        assert_ne!(u(clutch, 0x60), 3, "native held clutch derivative unsupported");
        d
    }
    /// The clutch's slip at the last sub-step, rpm (+0x58), which the next
    /// slipping sub-step compares against (0x594CC0). For the developer
    /// harness's transplant of the original's state.
    pub fn set_last_slip_rpm(&mut self, rpm: f32) {
        self.last_slip_rpm = rpm;
    }

    /// The gear change's random offsets (+0x78, +0x7C, +0x80; drawn at each
    /// change by 0x594720). For the developer harness's transplant.
    pub fn set_shift_jitter(&mut self, jitter: [f32; 3]) {
        self.jitter = jitter;
    }

    pub fn new(h: &Handling) -> Drivetrain {
        let idle_w = h.engine.idle_rpm / RPM_PER_RAD_S;
        Drivetrain {
            // The mover reset (0x5A48B0) starts the engine at idle, in the
            // car's stored gear (car +0x3F4: neutral at spawn) with the
            // clutch locked.
            engine_w: idle_w,
            rear_share: h.gearbox.rear_split,
            reaction: 0.0,
            sent: 0.0,
            input_w: 0.0,
            last_slip_rpm: 0.0,
            gear: NEUTRAL,
            target_gear: NEUTRAL,
            pending: false,
            shift: Shift::Locked,
            clutch: 1.0,
            power: 1.0,
            catch_up: 1.0,
            throttle: 0.0,
            limiter: false,
            up_rpm: shift_points(h).0,
            down_rpm: shift_points(h).1,
            jitter: [0.0; 3],
            shown_rpm: None,
            seed: 0x1234_5678,
            shift_bonus: 0.0,
            bonus_since: None,
            heat: if h.race.drag() { 0.0 } else { -1.0 },
            clock: 0.0,
            clutch_held: false,
            nitrous: 0,
            turbo: Turbo { max: h.engine.curve_334.values.iter().fold(0.0f32, |m, &v| m.max(v)), ..Turbo::default() },
        }
    }

    /// A drivetrain whose shift offsets draw from their own sequence
    /// `seed` where no world generator is lent ([`with_world_rng`]: the
    /// world steps of a race and of free roam lend it, as the original's
    /// 0x594720 draws from 0x890080 +0xF8, shared by every car).
    pub fn seeded(h: &Handling, seed: u32) -> Drivetrain {
        Drivetrain { seed, ..Drivetrain::new(h) }
    }
}
