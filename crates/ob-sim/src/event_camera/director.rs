use glam::Vec3;

use super::*;

/// 0x8020E0: a running kind that a new event ends first.
pub(super) const ENDS_FIRST: [bool; 8] = [false, true, true, false, false, false, false, false];
/// 0x8020E8: a running kind that refuses a new event.
pub(super) const BLOCKS: [bool; 8] = [false, false, false, true, true, true, true, true];

/// The camera mover type a kind asks Camera_Factory for.
pub fn mover(kind: u8) -> Option<u8> {
    match kind {
        1 => Some(0x0D),
        2 => Some(0x0E),
        6 => Some(0x0F),
        7 => Some(0x10),
        _ => None,
    }
}

/// What the race tells the director this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct RaceState {
    /// Race status 4 (after the green light, the player not finished).
    pub racing: bool,
    /// A drift race (0x89E7D9).
    pub drift: bool,
    /// The player's car in crash mode (+0x38 = 1).
    pub crash_mode: bool,
    /// Its wheels down (+0x424).
    pub wheels_down: usize,
    /// Its speed (+0x430).
    pub speed: f32,
    /// Its vertical velocity (+0x78).
    pub climb: f32,
    /// Its height (+0x68).
    pub height: f32,
}

/// The Jump Camera's graphs over the option's value 0..255 (static
/// initialisers 0x77ECC0..0x77ED2C, points 0x802404..0x802434): the
/// least speed (0x89CF18), the prediction's window (0x890150), the least
/// height (0x89E790) and airtime (0x89E824) of x = trunc(+0x24) & 0xFF. With the option on (127.5, x = 127):
/// 30.04 m/s, 1.80 s, 1.25 m, 0.90 s.
pub(super) const JUMP_SPEED: [f32; 4] = [0.0, 40.0, 255.0, 20.0];
pub(super) const JUMP_WINDOW: [f32; 4] = [0.0, 3.0, 255.0, 0.6];
pub(super) const JUMP_HEIGHT: [f32; 4] = [0.0, 2.0, 255.0, 0.5];
pub(super) const JUMP_AIRTIME: [f32; 4] = [0.0, 1.5, 255.0, 0.3];

/// Graph_Eval (0x5767A0) on a two-point graph, clamped at both ends.
pub(super) fn graph(g: [f32; 4], x: f32) -> f32 {
    let t = ((x - g[0]) / (g[2] - g[0])).clamp(0.0, 1.0);
    g[1] + (g[3] - g[1]) * t
}

/// What the flight prediction (0x5EC3E0) hands back: the greatest height
/// above the ground (its arg 2), the longest time in the air since the
/// last touch (arg 3) and the mean height (its result).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    pub height: f32,
    pub airtime: f32,
    pub mean: f32,
}

/// The flight prediction 0x5EC3E0(window, &height, &airtime, stop): from
/// `start` (the mean of the four wheels' contact points, 0x401340 on car
/// +0x90 / +0x60 and each wheel's matrix +0x130 + 0x40·i at −radius,
/// weighted 0.25) and the velocity (+0x70), n = trunc(speed / 3 ×
/// window) steps (none under 1: no flight). Starting in the air the
/// clock of the last touch is −dt and the airtime dt, the fall 0;
/// on the ground the fall is gravity (0x7FBEF0) + `downforce` (record
/// (+0x374 + +0x378) × 0.3 × speed / mass). Each step k (1..n) puts the
/// car at p0 + (v + a·τ/2)·τ, τ = k·dt − the last touch; on or under the
/// ground (0x5E72F0) it stops when `stop` and it has been in the air,
/// else the ground there becomes p0, the fall gravity + downforce, v the
/// velocity along the ground (n × v × n) at the speed, the last touch
/// k·dt; above it the height adds to the sum, the greatest height and
/// longest τ are kept and the fall is gravity alone.
pub fn predict_flight(
    start: Vec3,
    velocity: Vec3,
    speed: f32,
    window: f32,
    downforce: f32,
    ground: &dyn Fn(Vec3) -> (f32, Vec3),
    stop: bool,
) -> Flight {
    const GRAVITY: f32 = 9.8;
    let n = (speed / 3.0 * window) as i32;
    if n < 1 {
        return Flight::default();
    }
    let mut p0 = start;
    let h0 = (p0.z - ground(p0).0).max(0.0);
    let (mut sum, mut height) = (h0, h0);
    let dt = window / n as f32;
    let (mut air, mut touch) = if h0 > 0.0 { (dt, -dt) } else { (0.0, 0.0) };
    let mut v = velocity;
    let mut fall = if h0 <= 0.0 { GRAVITY + downforce } else { 0.0 };
    let mut k = 1;
    while k < n {
        let t = k as f32 * dt;
        let tau = t - touch;
        let p = p0 + (v + Vec3::new(0.0, 0.0, -fall) * (tau * 0.5)) * tau;
        let (gz, normal) = ground(p);
        let h = p.z - gz;
        if h <= 0.0 {
            if stop && air > 0.0 {
                break;
            }
            p0 = Vec3::new(p.x, p.y, gz);
            fall = GRAVITY + downforce;
            v = normal.cross(v).cross(normal).normalize_or_zero() * speed;
            touch = t;
        } else {
            sum += h;
            height = height.max(h);
            air = air.max(tau);
            fall = GRAVITY;
        }
        k += 1;
    }
    Flight { height, airtime: air, mean: sum / k as f32 }
}

/// The player's camera director's event fields.
#[derive(Clone, Debug, Default)]
pub struct EventCameras {
    /// +0x24 ≠ 0: the Jump Camera option at the race's start.
    pub enabled: bool,
    /// +0x58: the running kind (0 none).
    pub kind: u8,
    /// +0x5C: the last kind that ended.
    pub previous: u8,
    /// +0x68 + 4·kind: the simulation clock each kind last started at.
    started: [Option<f32>; 8],
    /// The simulation clock (g_SimTicks / 4000).
    pub clock: f32,
    pub slow: SlowMotion,
    /// Car +0x548: when the player's crash began.
    crash_start: f32,
    /// g_WorldUpdateCounter.
    updates: u32,
    /// Events started so far (for the self-test).
    pub count: usize,
    /// Car +0x544: when a jump's predicted landing is due.
    landing: f32,
    /// The option's value the graphs read (director +0x24: 127 on).
    pub option: f32,
    /// A kind that started since the caller last looked (its camera is
    /// to be built, Camera_Factory 0x0F / 0x10).
    pub started_now: Option<u8>,
    /// Jump cameras started (for the self-test).
    pub jumps: usize,
}

impl EventCameras {
    pub fn new(enabled: bool) -> EventCameras {
        EventCameras { enabled, option: if enabled { 127.0 } else { 0.0 }, ..Default::default() }
    }

    /// Whether the last `update` was one of the every-eighth world
    /// updates ((g_WorldUpdateCounter + 3) & 7 = 0, 0x604A40..0x604A56)
    /// that run the jump trigger, the second shot's rise (0x604D2F) and
    /// the kind 6 → 7 change.
    pub fn eighth(&self) -> bool {
        self.updates.wrapping_add(3) & 7 == 0
    }

    /// The mover the running event asks for (None: the chase camera).
    pub fn mover(&self) -> Option<u8> {
        mover(self.kind)
    }

    /// 0x5FFE80.
    pub fn end(&mut self) {
        if self.kind == 0 {
            return;
        }
        self.slow = SlowMotion::default();
        self.previous = self.kind;
        self.kind = 0;
    }

    /// The gates 0x6022F0 and 0x604750 share before a kind starts.
    fn may_start(&self, kind: u8, s: &RaceState) -> bool {
        if s.drift || !s.racing || kind > 7 {
            return false;
        }
        if self.started[kind as usize].is_some_and(|t| self.clock - t < 5.0) {
            return false;
        }
        if matches!(kind, 6 | 7) && !self.enabled {
            return false;
        }
        // Kinds 1..5: their second gate, 0x83AA24 (settings +0x20), is
        // open natively (127.5 from 0x5116E0 and in every save; read live
        // under Wine), but nothing in the PC game starts them: kind
        // 1's timer (director +0x9C, 0x602892) is written only by the
        // constructor (0 at 0x5F1842), the key events 0x7F..0x81 (kinds
        // 1, 2, 3 in 0x602BE0) have no row in the input descriptor table
        // 0x7FDD10 (only 0x60617F's subscriptions), and 0x6022F0's one
        // direct caller is Car_Crash (kind 6, 0x5A0105). So the clone
        // starts none of them (the event camera 0x4531A0 they would build
        // stays unported).
        !(1..=5).contains(&kind)
    }

    /// 0x6022F0 without the key flag; whether the event started.
    pub fn start(&mut self, kind: u8, s: &RaceState) -> bool {
        if !self.may_start(kind, s) {
            return false;
        }
        if ENDS_FIRST[self.kind as usize] {
            self.end();
        }
        if BLOCKS[self.kind as usize] {
            return false;
        }
        self.started[kind as usize] = Some(self.clock);
        if let Some((rate, seconds)) = slow_motion(kind) {
            self.slow.start(rate, seconds);
        }
        self.kind = kind;
        self.count += 1;
        self.started_now = Some(kind);
        true
    }

    /// Car_Crash's camera test (0x5A0095..0x5A0105) for the player's car
    /// entering crash mode against another car; `placed` is 0x5F1EF0.
    pub fn crash(&mut self, s: &RaceState, placed: bool) -> bool {
        self.crash_start = self.clock;
        if s.drift || matches!(self.kind, 6 | 7) || !placed {
            return false;
        }
        self.start(6, s)
    }

    /// One world update's step of the simulation clock and the rules of
    /// 0x604750 for kinds 6 / 7, without the jump camera.
    pub fn update(&mut self, sim_dt: f32, s: &RaceState) {
        self.update_with(sim_dt, s, &mut |_| Flight::default(), &mut || false);
    }

    /// The same with the jump camera: `predict(window)` is the flight
    /// prediction 0x5EC3E0 with stop set, `placed` 0x5F1EF0.
    pub fn update_with(
        &mut self,
        sim_dt: f32,
        s: &RaceState,
        predict: &mut dyn FnMut(f32) -> Flight,
        placed: &mut dyn FnMut() -> bool,
    ) {
        self.clock += sim_dt;
        self.updates = self.updates.wrapping_add(1);
        if matches!(self.kind, 6 | 7) {
            let since = self.clock - self.crash_start;
            if s.crash_mode {
                if s.wheels_down == 4 && since > 0.5 {
                    self.end();
                }
            } else if s.wheels_down > 0 {
                self.end();
            }
        }
        if self.updates.wrapping_add(3) & 7 != 0 {
            return;
        }
        // The jump trigger (0x604ACC..0x604D14): no event camera running,
        // prov: inferred event_camera.update_with
        // all four wheels up, the director's +0x248 clear (**inferred**:
        // always), faster than the speed graph; the prediction over the
        // window graph: its landing is due at airtime × 0.5 when the
        // airtime is at least 1 s, else in 5 s; higher than the height
        // graph, longer than the airtime graph, and 0x5F1EF0 placed — then
        // kind 6 by the same gates as Player_StartEventCamera (the 5 s
        // since +0x80, racing, one player, the option, the kind tables),
        // 0.4 for 8 s (0x7A552C / 0x7A5500) and mover 0x0F. A refusal
        // clears +0x644 / +0x648 (0x604D14).
        if !matches!(self.kind, 6 | 7) && s.wheels_down == 0 && graph(JUMP_SPEED, self.option) < s.speed {
            let f = predict(graph(JUMP_WINDOW, self.option));
            self.landing = self.clock + if f.airtime >= 1.0 { f.airtime * 0.5 } else { 5.0 };
            if graph(JUMP_HEIGHT, self.option) < f.height
                && graph(JUMP_AIRTIME, self.option) < f.airtime
                && placed()
                && self.start(6, s)
            {
                self.jumps += 1;
            }
        }
        if self.kind == 6 {
            if s.crash_mode {
                if self.clock - self.crash_start > 3.0 {
                    self.end();
                    self.start(7, s);
                }
            } else if self.clock > self.landing {
                // 0x604E72: past the predicted landing and still up, the
                // prediction again over 1 s; above 0.5 m the second shot.
                self.end();
                if predict(1.0).height > 0.5 {
                    self.start(7, s);
                }
            }
        }
    }
}
