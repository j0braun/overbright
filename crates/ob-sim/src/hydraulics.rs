//! The hydraulics (car specialties part slot 0xA1), traced in SPEED2.EXE
//! (verified unless marked; docs/formats.md, "Hydraulics").
//!
//! * **The part.** Fitting a slot 0xA1 part (0x61BE0E) stores its level
//!   (part byte +5 >> 5: 1–3) in the car record's +0x581 (cleared to 0 by
//!   the record's constructor 0x610280; serialised by 0x631A80). The human
//!   input's event handler reads it through car +0x10 → +0x581, unless the
//!   override 0x7E949C (−1 in the image, no writer found) is 0 or more.
//! * **The input** (the human input object, vtable 0x784FC8): its handler
//!   0x4078A0, registered for the events 0xA5…0xAE (0x410480), returns at
//!   once without a level or while not available (+0x2F9). BOUNCE (0xA5,
//!   Controls row 17, action 19, J / U) and PRESSURIZE (0xAA, row 16,
//!   action 22, H / Y): pressed, the time is taken (+0x300, g_SimTicks) if
//!   none was held, and the event is kept (+0x2FA); let go, a mode
//!   (+0x2FB) ends. The analog events (`(byte − 128) · −1/128`): bounce
//!   up/down +0x308, left/right +0x304, pressure up/down +0x310,
//!   left/right +0x30C (only with level 2 or 3); either pressure event with
//!   both pressure axes within ±0.2 clears the toggle +0x2FC. On the
//!   keyboard the throttle / brake keys send up / down and the steering
//!   keys left / right (live, 2026-10-31: Up 1.0, Down −0.9921875, Left
//!   1.0, Right −0.9921875, both keys of a pair 0).
//! * **Each frame** (0x407400, from the human input's update 0x41E2C0):
//!   available (0x407370) with g_HydraulicsOn 0x7E94A0 (1, no writer
//!   found), under 30 mph (|car +0x430| × 2.237), and not in a drift or
//!   street X race (0x89E7D9, 0x890104, 0x89E7E3, 0x89010C); turning
//!   unavailable clears both input sets, the mode and the axes (+0x304,
//!   +0x308, +0x310; not +0x30C). A key held over 0.22 s (0x784534) under
//!   10 mph enters its mode (pressure 1, bounce 2); the time is cleared
//!   either way. Mode 2 sets the bounce input (+0x1EC + 4i) from the bounce
//!   axes over ±0.8: up the front pair (0, 1), down the rear (2, 3), left
//!   the left side (0, 3), right the right (1, 2). Mode 1 picks the same
//!   pairs from the pressure axes and, with the toggle clear, sets it to
//!   raise if a picked corner is at rest (state 0) or else to lower if one
//!   is up (state 1); then each picked corner's pressure input (+0x1FC +
//!   4i) is 1 (raise) or 0 (lower). While a mode is on the throttle, brake
//!   and reverse events are ignored (0x416DE0, 0x407EB0, 0x4077D0) and the
//!   steering input is 0 (0x41E341).
//! * **The physics** (0x5A51C0, at the end of each sub-step's force pass
//!   0x5B0530), skipped while the car has a live contact (car +0x3C →
//!   +0x18): see [`Suspension::force_pass`]. The heights move the
//!   suspension mounts down (0x5AB9A0 → 0x59E0D0: the mount's z less the
//!   height), once a frame at the mover's update (0x5B72E0), which also
//!   clears the kick flags (car +0x408 + i).

use glam::Vec3;

/// The pressure input's height, m (0x7FC09C).
pub const HEIGHT: f32 = 0.2;
/// The height a bouncing pair's other corners keep, m (0x7FC08C).
pub const BOUNCE_FLOOR: f32 = 0.2;
/// The corners' speed, m/s (0x7FC098).
pub const RATE: f32 = 0.5;
/// A bouncing wheel extended past this aborts the kick (0x7FC090).
pub const EXTENDED: f32 = -0.01;
/// The hold that enters a mode, s (0x784534).
pub const HOLD: f32 = 0.22;
/// Available below this speed (0x407370(0)), mph.
pub const AVAILABLE_MPH: f32 = 30.0;
/// A mode starts below this speed (0x407370(1)), mph.
pub const ENTER_MPH: f32 = 10.0;

/// One digital pair as the analog event's byte: + 0, − 255, neither or
/// both 128; the handler's value is `(byte − 128) · −1/128`.
pub fn axis(plus: bool, minus: bool) -> f32 {
    let byte: i32 = match (plus, minus) {
        (true, false) => 0,
        (false, true) => 255,
        _ => 128,
    };
    (byte - 128) as f32 * -0.0078125
}

/// What the player holds this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keys {
    pub bounce: bool,
    pub pressure: bool,
    /// The up / down and left / right axes ([`axis`]).
    pub ud: f32,
    pub lr: f32,
}

/// The input the mover reads (controls +0x1EC… bounce, +0x1FC… pressure).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub bounce: [f32; 4],
    pub pressure: [f32; 4],
}

impl Input {
    /// 0x5ED2A0: a bounce input on.
    pub fn bouncing(&self) -> bool {
        self.bounce.iter().any(|&b| b > 0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Bounce,
    Pressure,
}

/// Where the race is: 0x407370's race tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    /// Drift (0x89E7D9 / 0x890104) or street X (0x89E7E3 / 0x89010C).
    pub barred: bool,
    /// |car +0x430| × 2.237.
    pub mph: f32,
    /// The corners' states (mover +0x74 + 4i, 0x5ED480).
    pub states: [u8; 4],
}

/// The human input's hydraulics fields (+0x1EC…, +0x2F9…).
#[derive(Clone, Debug, Default)]
pub struct Controller {
    pub input: Input,
    /// +0x2F9.
    available: bool,
    /// +0x2FA.
    held: Option<Event>,
    /// +0x2FB: 0 none, 1 pressure, 2 bounce.
    pub mode: u8,
    /// +0x300, s.
    pressed_at: Option<f32>,
    /// +0x2FC: 1 raise, −1 lower, 0 not picked.
    toggle: i8,
    /// +0x304 bounce LR, +0x308 bounce UD, +0x30C pressure LR, +0x310
    /// pressure UD.
    axes: [f32; 4],
    last: Keys,
    clock: f32,
}

impl Controller {
    fn available(&self, limit: f32, c: &Context) -> bool {
        c.mph < limit && !c.barred
    }

    /// The events since the last frame (0x4078A0), then the frame
    /// (0x407400). `level` is the car's (0 none). Returns the input for the
    /// mover.
    pub fn update(&mut self, level: u8, keys: Keys, c: &Context, dt: f32) -> Input {
        self.clock += dt;
        let was = std::mem::replace(&mut self.last, keys);
        if level > 0 && self.available {
            for (now, before, ev) in [(keys.bounce, was.bounce, Event::Bounce), (keys.pressure, was.pressure, Event::Pressure)] {
                if now == before {
                    continue;
                }
                if now {
                    if self.held.is_none() {
                        self.pressed_at = Some(self.clock);
                    }
                    self.held = Some(ev);
                } else {
                    if self.held.is_some() && self.mode != 0 {
                        self.mode = 0;
                        self.pressed_at = None;
                    }
                    self.held = None;
                }
            }
            // 0x4079C6: after a pressure axis event, both axes under 0.2
            // clear the toggle.
            let mut check = false;
            if keys.ud != was.ud {
                self.axes[1] = keys.ud;
                self.axes[3] = keys.ud;
                check = true;
            }
            if keys.lr != was.lr {
                self.axes[0] = keys.lr;
                if level > 1 {
                    self.axes[2] = keys.lr;
                    check = true;
                }
            }
            if check && self.axes[2].abs() < 0.2 && self.axes[3].abs() < 0.2 {
                self.toggle = 0;
            }
        }
        // 0x407400.
        let now = self.available(AVAILABLE_MPH, c);
        if !now && self.available {
            self.input = Input::default();
            self.mode = 0;
            self.axes[0] = 0.0;
            self.axes[1] = 0.0;
            self.axes[3] = 0.0;
        }
        self.available = now;
        if let Some(t) = self.pressed_at
            && self.clock - t > HOLD
        {
            if self.available(ENTER_MPH, c)
                && let Some(ev) = self.held
            {
                self.mode = if ev == Event::Pressure { 1 } else { 2 };
            }
            self.pressed_at = None;
        }
        if self.mode == 2 {
            let b = &mut self.input.bounce;
            *b = [0.0; 4];
            let (lr, ud) = (self.axes[0], self.axes[1]);
            if ud < -0.8 {
                b[2] = -ud;
                b[3] = -ud;
            } else if ud > 0.8 {
                b[0] = ud;
                b[1] = ud;
            } else if lr < -0.8 {
                b[1] = -lr;
                b[2] = -lr;
            } else if lr > 0.8 {
                b[0] = lr;
                b[3] = lr;
            }
        }
        if self.mode == 1 {
            let (lr, ud) = (self.axes[2], self.axes[3]);
            let mask: u8 = if ud < -0.8 {
                0xC
            } else if ud > 0.8 {
                3
            } else if lr < -0.8 {
                6
            } else if lr > 0.8 {
                9
            } else {
                0
            };
            let mut go = true;
            if self.toggle == 0 {
                for i in 0..4 {
                    if mask & (1 << i) != 0 {
                        if c.states[i] == 0 {
                            self.toggle = 1;
                        } else if c.states[i] == 1 && self.toggle == 0 {
                            self.toggle = -1;
                        }
                    }
                }
                go = self.toggle != 0;
            }
            if go {
                for i in 0..4 {
                    if mask & (1 << i) != 0 {
                        self.input.pressure[i] = if self.toggle >= 1 { 1.0 } else { 0.0 };
                    }
                }
            }
        }
        self.input
    }

    /// A mode is on (+0x2FB): no throttle, brake, reverse or steering.
    pub fn holds_driving(&self) -> bool {
        self.mode != 0
    }
}

/// The mover's side: the corner heights ([mover +0x58] +0x3B8 + 4i = car
/// +0x3F8), their states (mover +0x74 + 4i) and this frame's kicks (car
/// +0x408 + i), and the mounts' heights as the frame began (0x5AB9A0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Suspension {
    pub height: [f32; 4],
    pub state: [u8; 4],
    pub kicked: [bool; 4],
    pub mount: [f32; 4],
    pub input: Input,
}

/// One wheel as the kick reads it.
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    /// The suspension's compression (+0x1C).
    pub compression: f32,
    /// The hub (+0x30) from the centre of mass, world axes.
    pub arm: Vec3,
    /// Its ground normal (+0x150).
    pub normal: Vec3,
}

impl Suspension {
    /// The mover's frame start (0x5B72E0): the kick flags cleared, the
    /// mounts set from the heights (0x5AB9A0).
    pub fn frame(&mut self, input: Input) {
        self.kicked = [false; 4];
        self.mount = self.height;
        self.input = input;
    }

    /// 0x5A51C0, one sub-step: the heights move toward the pressure input
    /// × 0.2 at 0.5 m/s (a bouncing pair's other corners at least 0.2);
    /// then, with the pair's other corners up, no bouncing wheel extended
    /// past −0.01 and the hub not rising along its ground normal at 3 m/s
    /// or more, each bouncing wheel takes the impulse (0.9 − 0.3 · v_n) ·
    /// bounce × mass along the normal (0x59BC70) and is flagged. Returns
    /// the impulses (at their arms); `v_at(arm)` is the body's velocity at
    /// a point before any of them.
    pub fn force_pass(&mut self, wheels: &[Wheel; 4], mass: f32, v_at: &dyn Fn(Vec3) -> Vec3, dt: f32) -> Vec<(Vec3, Vec3)> {
        let b = self.input.bounce;
        let on = |i: usize| b[i] != 0.0;
        let mask: u8 = if on(1) && on(0) {
            0xC
        } else if on(2) && on(3) {
            3
        } else if (on(2) && on(1)) || (on(3) && on(0)) {
            0x20
        } else {
            0
        };
        let mut kick = mask != 0;
        for i in 0..4 {
            let mut target = self.input.pressure[i] * HEIGHT;
            let floor = mask & (1 << i) != 0;
            if floor && target <= BOUNCE_FLOOR {
                target = BOUNCE_FLOOR;
            }
            let h = &mut self.height[i];
            self.state[i] = 0;
            if target > *h {
                *h = (*h + dt * RATE).min(target);
                self.state[i] = 2;
            } else if target < *h {
                *h = (*h - dt * RATE).max(0.0);
                self.state[i] = 3;
            } else if target > 0.0 {
                self.state[i] = 1;
            }
            if floor && self.height[i] < BOUNCE_FLOOR {
                kick = false;
            }
        }
        if !kick {
            return Vec::new();
        }
        if (0..4).any(|i| b[i] > 0.0 && wheels[i].compression < EXTENDED) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..4 {
            if b[i] <= 0.0 {
                continue;
            }
            let w = &wheels[i];
            let along = v_at(w.arm).dot(w.normal);
            if along < 0.0 {
                continue;
            }
            let f = (0.9 - along * 0.3) * b[i];
            if f > 0.0 {
                out.push((w.normal * (f * mass), w.arm));
                self.kicked[i] = true;
            }
        }
        out
    }

    /// 0x5ED310: a corner up or a pressure input on.
    pub fn raised(&self) -> bool {
        (0..4).any(|i| self.height[i] > 0.0 || self.input.pressure[i] > 0.0)
    }

    /// The moving corners (states 2 and 3), as 0x56F9C0 counts them.
    pub fn moving(&self) -> usize {
        self.state.iter().filter(|&&s| s == 2 || s == 3).count()
    }
}

/// A mode's record in 0x7F6864 (0x114 bytes, modes 1–3 at +0x114 · m).
#[derive(Clone, Copy, Debug)]
pub struct GarageMode {
    /// +0x00: the phase's rate.
    pub rate: f32,
    /// +0x04: the corners' speed factor.
    pub speed: f32,
    /// +0x08: the swing.
    pub swing: f32,
    /// +0x0C: the wheels follow a corner up to this (car +0x178 base).
    pub follow: f32,
    /// +0x10: every corner's rest height.
    pub rest: f32,
    /// +0x14: the pitch factor (+0x17C).
    pub pitch: f32,
    /// +0x18 / +0x1C: each channel's clamp.
    pub lo: f32,
    pub hi: f32,
    /// +0x20: 0 squares each channel to 0 / 1.
    pub smooth: bool,
    /// +0x24: four channels (front, rear, left, right) × three terms
    /// {on, amplitude, frequency, phase, offset}.
    pub terms: [[(bool, f32, f32, f32, f32); 3]; 4],
}

const OFF: (bool, f32, f32, f32, f32) = (false, 0.0, 0.0, 0.0, 0.0);

/// 0x7F6864's modes 1–3 (every value read from the EXE; mode 3's from its
/// static initializer 0x77D010).
pub const GARAGE_MODES: [GarageMode; 3] = [
    GarageMode {
        rate: 4.0,
        speed: 4.0,
        swing: 0.1,
        follow: 1.0,
        rest: 0.01,
        pitch: 0.0,
        lo: 0.0,
        hi: 1.0,
        smooth: false,
        terms: [
            [(true, 1.0, 0.5, 0.0, -0.25), (true, 1.0, 2.123, 40.0, 0.0), OFF],
            [(true, 1.0, 0.678, 40.0, -0.25), (true, 1.0, 2.0, 0.0, 0.0), OFF],
            [OFF; 3],
            [OFF; 3],
        ],
    },
    GarageMode {
        rate: 5.0,
        speed: 4.0,
        swing: 0.1,
        follow: 1.0,
        rest: 0.01,
        pitch: 0.0,
        lo: 0.0,
        hi: 1.0,
        smooth: false,
        terms: [
            [(true, 1.0, 0.9, 0.0, -0.15), (true, 1.0, 0.5, 36.0, 0.0), (true, -1.0, 0.4, 0.0, -1.0)],
            [(true, 1.0, 0.9, 40.0, -0.15), (true, 1.0, 0.5, 10.0, 0.0), (true, -1.0, 0.4, 0.0, -1.0)],
            [(true, 1.0, 0.8, 40.0, 0.0), (false, 1.0, 0.5, 36.0, 0.0), (true, 1.0, 0.4, 0.0, -1.0)],
            [(true, 1.0, 0.8, 0.0, 0.0), (false, 1.0, 0.5, 10.0, 0.0), (true, 1.0, 0.4, 0.0, -1.0)],
        ],
    },
    // Mode 3: +0x00…+0x0C from the image; +0x10…+0x110 written at start-up
    // by the static initializer 0x77D010 (the image holds zeros there):
    // rest 0.15 (0x78E024), pitch 0.25, clamp −0.1…10, smooth, the front
    // and rear each 0.5 · sin(2.25 φ (+ 17 for the rear)) + 0.4 (the other
    // terms off).
    GarageMode {
        rate: 4.0,
        speed: 4.0,
        swing: 0.5,
        follow: 0.15,
        rest: 0.15,
        pitch: 0.25,
        lo: -0.1,
        hi: 10.0,
        smooth: true,
        terms: [
            [(true, 0.5, 2.25, 0.0, 0.4), (false, 1.0, 3.3, 40.0, 0.0), (false, 1.0, 0.4, 0.0, -1.0)],
            [(true, 0.5, 2.25, 17.0, 0.4), (false, 1.0, 2.0, 0.0, 0.0), (false, 1.0, 0.4, 0.0, -1.0)],
            [(false, 0.5, 2.25, 0.0, 0.4), (false, 1.0, 2.123, 40.0, 0.0), (false, -1.0, 0.4, 0.0, -1.0)],
            [(false, 0.5, 2.25, 17.0, 0.4), (false, 1.0, 2.0, 0.0, 0.0), (false, -1.0, 0.4, 0.0, -1.0)],
        ],
    },
];

/// 0x43C350: the sine through the 16-bit binary angle.
fn sin16(x: f32) -> f32 {
    let a = (x * (65536.0 / std::f32::consts::TAU)) as i32 as u16;
    (a as f32 * (std::f32::consts::TAU / 65536.0)).sin()
}

/// The garage car's hydraulics (0x4A6030, each frame from 0x4D82D0;
/// the car object's +0x174 mode, +0x178 follow height, +0x17C pitch,
/// +0x180 phase, +0x184 + 4i corners, +0x194 + 4i lifted flags, +0x1A4 +
/// 4i states).
#[derive(Clone, Debug)]
pub struct Garage {
    pub mode: u32,
    pub follow: f32,
    pub pitch: f32,
    pub phase: f32,
    pub corner: [f32; 4],
    lifted: [bool; 4],
    state: [u8; 4],
}

impl Default for Garage {
    fn default() -> Garage {
        Garage { mode: 0, follow: 10.0, pitch: 0.0, phase: 0.0, corner: [0.0; 4], lifted: [false; 4], state: [0; 4] }
    }
}

impl Garage {
    /// One frame. `level` is the fitted slot 0xA1 part's (0 none);
    /// `lowering` the suspension's lowering of the front and rear corners
    /// (0x5A6590(5, 3) on the car's upgrade state: its first gain × the
    /// car type's +0x714 / +0x718, when the category's level is not 0).
    /// Returns the FE sound ids to play (PlayFESound 0x458520).
    pub fn update(&mut self, level: u32, lowering: [f32; 2], dt: f32) -> Vec<u8> {
        self.follow = 10.0;
        let mut speed = 2.5;
        let mut swing = 0.1;
        let mut pitch = 0.0;
        let mut base = [0.0f32; 4];
        let mut ch = [0.0f32; 4];
        let mut used = level;
        let mut through = false;
        if self.mode != level {
            used = 0;
            if self.mode == 3 || level == 3 {
                used = 3;
                through = true;
            }
            self.phase = 0.0;
        }
        if (1..4).contains(&used) {
            let m = &GARAGE_MODES[used as usize - 1];
            base = [m.rest; 4];
            base[0] += lowering[0];
            base[1] += lowering[0];
            base[2] += lowering[1];
            base[3] += lowering[1];
        }
        let mut settling = false;
        if through && (0..4).any(|i| self.corner[i] != base[i]) {
            speed = 20.0;
            if level != 3 {
                self.follow = 0.15;
            }
            settling = true;
        }
        if !settling {
            if (1..4).contains(&level) {
                let m = &GARAGE_MODES[level as usize - 1];
                for (c, terms) in m.terms.iter().enumerate() {
                    let mut v = 0.0;
                    for &(on, amp, freq, phase, offset) in terms {
                        if on {
                            v += sin16(self.phase * freq + phase) * amp + offset;
                        }
                    }
                    if !m.smooth {
                        v = if v <= 0.0 { 0.0 } else { 1.0 };
                    }
                    ch[c] = v.max(m.lo).min(m.hi);
                }
                speed = m.speed;
                swing = m.swing;
                self.follow = m.follow;
                pitch = m.pitch;
                self.phase += dt * m.rate;
                if self.phase > 1e13 {
                    self.phase = 0.0;
                }
                self.mode = level;
            } else {
                self.mode = 0;
            }
        }
        let [front, rear, left, right] = ch;
        let target = [
            ((left + front) - (right + rear) * 0.1).min(1.0) * swing + base[0],
            ((right + front) - (left + rear) * 0.1).min(1.0) * swing + base[1],
            ((right + rear) - (left + front) * 0.1).min(1.0) * swing + base[2],
            ((left + rear) - (right + front) * 0.1).min(1.0) * swing + base[3],
        ];
        self.follow += base.iter().copied().fold(f32::MIN, f32::max);
        let step = dt * swing * speed;
        let mut sounds = Vec::new();
        for i in 0..4 {
            let old = self.corner[i];
            let h = &mut self.corner[i];
            if (*h - target[i]).abs() < step {
                *h = target[i];
            } else if target[i] < *h {
                *h -= step;
            } else if target[i] > *h {
                *h += step;
            }
            let h = *h;
            let even = i % 2 == 0;
            let state;
            if h > base[i] {
                if h <= self.follow {
                    if h < base[i] + swing {
                        if h <= old {
                            state = 3;
                            if self.state[i] != 3 && ((level == 1 && even) || (level == 2 && !even)) {
                                sounds.push(if level == 1 { 0x5A } else { 0x5D });
                            }
                        } else {
                            state = 2;
                            if self.state[i] != 2 && ((level == 1 && even) || (level == 2 && !even)) {
                                sounds.push(if level == 1 { 0x59 } else { 0x5B });
                            }
                        }
                    } else {
                        state = 1;
                    }
                } else {
                    state = 0;
                    self.lifted[i] = true;
                }
            } else {
                state = 0;
                if self.lifted[i] && even {
                    sounds.push(if i / 2 != 0 { 0x58 } else { 0x5C });
                }
                self.lifted[i] = false;
            }
            self.state[i] = state;
        }
        self.pitch = ((self.corner[1] + self.corner[0]) * 0.5 - (self.corner[3] + self.corner[2]) * 0.5) * pitch;
        sounds
    }

    /// How far each wheel drops below its rest point in the car's frame
    /// (0x4A6670: the wheel matrices' z less min(corner, +0x178)).
    pub fn wheel_drop(&self, i: usize) -> f32 {
        self.corner[i].min(self.follow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_make_the_handler_values() {
        assert_eq!(axis(true, false), 1.0);
        assert_eq!(axis(false, true), -0.9921875);
        assert_eq!(axis(true, true), 0.0);
    }

    fn frames(c: &mut Controller, keys: Keys, n: usize, ctx: &Context) -> Input {
        let mut out = Input::default();
        for _ in 0..n {
            out = c.update(2, keys, ctx, 1.0 / 120.0);
        }
        out
    }

    #[test]
    fn garage_level_three_pumps_front_and_rear() {
        // Mode 3's record from 0x77D010 (rest 0.15, swing 0.5, the front
        // and rear 0.5·sin(2.25φ (+17)) + 0.4, smooth): after settling,
        // the nose and the tail each swing well over 0.3.
        let mut g = Garage::default();
        let (mut lo, mut hi) = ([f32::MAX; 4], [f32::MIN; 4]);
        for i in 0..480 {
            g.update(3, [0.0; 2], 1.0 / 60.0);
            if i > 120 {
                for c in 0..4 {
                    lo[c] = lo[c].min(g.corner[c]);
                    hi[c] = hi[c].max(g.corner[c]);
                }
            }
        }
        assert_eq!(g.mode, 3);
        assert!((0..4).all(|c| hi[c] - lo[c] > 0.3 && lo[c] >= 0.0), "{lo:?} {hi:?}");
        // Left and right move together (no side channels).
        assert!((hi[0] - hi[1]).abs() < 1e-3 && (hi[2] - hi[3]).abs() < 1e-3);
    }

    #[test]
    fn pressure_mode_raises_then_lowers_a_pair() {
        let mut c = Controller::default();
        let mut ctx = Context::default();
        frames(&mut c, Keys::default(), 2, &ctx);
        let h = Keys { pressure: true, ..Default::default() };
        frames(&mut c, h, 20, &ctx);
        assert_eq!(c.mode, 0, "not before 0.22 s");
        frames(&mut c, h, 10, &ctx);
        assert_eq!(c.mode, 1);
        let up = Keys { ud: 1.0, ..h };
        assert_eq!(frames(&mut c, up, 2, &ctx).pressure, [1.0, 1.0, 0.0, 0.0]);
        // Let go of Up, the front at its height: the next Up lowers it.
        frames(&mut c, h, 2, &ctx);
        ctx.states = [1, 1, 0, 0];
        assert_eq!(frames(&mut c, up, 2, &ctx).pressure, [0.0, 0.0, 0.0, 0.0]);
        // Left now: corner 0 at rest raises both left corners.
        frames(&mut c, h, 2, &ctx);
        ctx.states = [0, 0, 0, 0];
        assert_eq!(frames(&mut c, Keys { lr: 1.0, ..h }, 2, &ctx).pressure, [1.0, 0.0, 0.0, 1.0]);
        // Let go of the key: the mode ends, the corners keep their input.
        let i = frames(&mut c, Keys::default(), 2, &ctx);
        assert_eq!((c.mode, i.pressure), (0, [1.0, 0.0, 0.0, 1.0]));
        // Over 30 mph everything clears.
        ctx.mph = 31.0;
        assert_eq!(frames(&mut c, Keys::default(), 1, &ctx), Input::default());
    }

    #[test]
    fn bounce_mode_follows_the_axes() {
        let mut c = Controller::default();
        let ctx = Context::default();
        frames(&mut c, Keys::default(), 1, &ctx);
        let j = Keys { bounce: true, ..Default::default() };
        frames(&mut c, j, 30, &ctx);
        assert_eq!(c.mode, 2);
        assert_eq!(frames(&mut c, Keys { ud: -0.9921875, ..j }, 1, &ctx).bounce, [0.0, 0.0, 0.9921875, 0.9921875]);
        assert_eq!(frames(&mut c, Keys { lr: -0.9921875, ..j }, 1, &ctx).bounce, [0.0, 0.9921875, 0.9921875, 0.0]);
        assert_eq!(frames(&mut c, j, 1, &ctx).bounce, [0.0; 4]);
        // Not entered at 10 mph or more.
        let mut c = Controller::default();
        let fast = Context { mph: 12.0, ..Default::default() };
        frames(&mut c, Keys::default(), 1, &fast);
        frames(&mut c, j, 40, &fast);
        assert_eq!(c.mode, 0);
    }

    #[test]
    fn corners_rise_at_half_a_metre_a_second() {
        let mut s = Suspension::default();
        s.frame(Input { pressure: [1.0, 1.0, 0.0, 0.0], ..Default::default() });
        let w = Wheel { compression: 0.05, arm: Vec3::ZERO, normal: Vec3::Z };
        for _ in 0..40 {
            assert!(s.force_pass(&[w; 4], 1.2, &|_| Vec3::ZERO, 0.005).is_empty());
        }
        assert!((s.height[0] - 0.1).abs() < 1e-5 && s.state[..2] == [2, 2] && s.state[2] == 0);
        // The step that reaches the target is still rising (state 2).
        for _ in 0..45 {
            s.force_pass(&[w; 4], 1.2, &|_| Vec3::ZERO, 0.005);
        }
        assert_eq!((s.height[0], s.state[0]), (0.2, 1));
        // Bounce the front: the rear rises to 0.2 first, then the kicks.
        s.frame(Input { bounce: [1.0, 1.0, 0.0, 0.0], pressure: [1.0, 1.0, 0.0, 0.0] });
        assert!(s.force_pass(&[w; 4], 1.2, &|_| Vec3::ZERO, 0.005).is_empty());
        s.height[2] = 0.2;
        s.height[3] = 0.2;
        let j = s.force_pass(&[w; 4], 1.2, &|_| Vec3::ZERO, 0.005);
        assert_eq!(j.len(), 2);
        assert!((j[0].0.z - 0.9 * 1.2).abs() < 1e-5 && s.kicked == [true, true, false, false]);
        // A wheel extended past 1 cm: none.
        let mut ws = [w; 4];
        ws[1].compression = -0.02;
        assert!(s.force_pass(&ws, 1.2, &|_| Vec3::ZERO, 0.005).is_empty());
    }

    #[test]
    fn garage_mode_one_swings_and_plays_its_sounds() {
        let mut g = Garage::default();
        let mut sounds = Vec::new();
        for _ in 0..600 {
            sounds.extend(g.update(1, [0.0; 2], 1.0 / 60.0));
        }
        assert_eq!(g.mode, 1);
        assert!(g.corner.iter().all(|&c| (0.01..=0.111).contains(&c)), "{:?}", g.corner);
        assert!(sounds.contains(&0x59) && sounds.contains(&0x5A));
        assert!(sounds.iter().all(|s| [0x59, 0x5A].contains(s)));
    }
}
