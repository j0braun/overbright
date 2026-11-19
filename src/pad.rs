//! Gamepads and joysticks: the original's DirectInput 8 model
//! (formats.md, **Gamepads and joysticks**), from the device to the
//! driving and menu inputs. The host side (gilrs, the synthetic script)
//! is [`host`]; this module is the native logic only.
//!
//! The chain in SPEED2.EXE:
//!
//! 1. 0x5D0080 creates DirectInput (0x8709EC) and enumerates attached game
//!    controllers (EnumDevices(DI8DEVCLASS_GAMECTRL, 0x5CEF10,
//!    DIEDFL_ATTACHEDONLY)); 0x5CEF10 skips the Nostromo n40 / n50 and
//!    0x5CD200 opens the first two (channels 0x86FFB4[2]) with
//!    c_dfDIJoystick2 (0x7BFDE0), every axis ranged ±10000 (0x5C8670), and
//!    the device type from 0x5BFB70 ([`DevType`]).
//! 2. Each frame 0x5C0FA0 detaches records whose channel lost its handle,
//!    0x5CA6A0 attaches a channel with no record to the first free one of
//!    the six input records (0x874C40, 0x370 bytes; 0x5C1500 →
//!    [`PadMap::attach`] → 0x5C11B0 [`PadMap::defaults`]), and 0x5CFA20
//!    polls the channels into 0x870760 (GetDeviceState; DIERR_INPUTLOST
//!    releases the channel, 0x5BFA60).
//! 3. 0x5C1850 turns the state into the record's 64-bit active-low action
//!    mask (+8) and four analog bytes (+0x10..+0x13): [`PadMap::poll`].
//! 4. The JOY descriptor rows (0x7FDD10) read those bits and bytes:
//!    [`driving`] and [`fe_actions`].

/// DirectInput channels the game opens (0x86FFB4[2]).
pub const CHANNELS: usize = 2;
/// Input records (0x874C40 + 0x370 × k up to 0x8760E0): the keyboard and
/// up to five controllers.
pub const RECORDS: usize = 6;
/// One record's block in the profile (0x5B9430 / 0x5B9580).
pub const BLOCK_LEN: usize = 0xB5;

/// DIJOYSTATE2 axis offsets (the ids 0x5B8260 stores).
pub mod axis {
    pub const X: u32 = 0x00;
    pub const Y: u32 = 0x04;
    pub const Z: u32 = 0x08;
    pub const RX: u32 = 0x0C;
    pub const RY: u32 = 0x10;
    pub const RZ: u32 = 0x14;
    pub const SLIDER0: u32 = 0x18;
    pub const SLIDER1: u32 = 0x1C;
    /// The order 0x5CA730 tests them in while waiting for an axis.
    pub const CAPTURE_ORDER: [u32; 8] = [X, RX, Y, RY, Z, RZ, SLIDER0, SLIDER1];
}

/// DIACTION ids (the driving map 0x7FCAC0) the records store per button.
pub mod act {
    pub const THROTTLE: u8 = 1;
    pub const BRAKE: u8 = 2;
    pub const STEER_LEFT: u8 = 3;
    pub const STEER_RIGHT: u8 = 4;
    pub const EBRAKE: u8 = 5;
    pub const NITROUS: u8 = 6;
    pub const SHIFT_UP: u8 = 7;
    pub const SHIFT_DOWN: u8 = 8;
    pub const LOOK_BACK: u8 = 9;
    pub const CAMERA: u8 = 10;
    pub const PAUSE: u8 = 12;
    pub const RESET: u8 = 14;
    pub const ENGAGE: u8 = 15;
    pub const WORLD_MAP: u8 = 16;
    pub const READ_SMS: u8 = 17;
    pub const CAREER_STATS: u8 = 18;
    pub const HYD_BOUNCE: u8 = 19;
    pub const HYD_PRESSURE: u8 = 22;
    /// The FE map's ids (0x7FD7E0): Menu Up / Down / Left / Right.
    pub const MENU_UP: u8 = 0x1B;
    pub const MENU_DOWN: u8 = 0x1C;
    pub const MENU_LEFT: u8 = 0x1D;
    pub const MENU_RIGHT: u8 = 0x1E;
    pub const MENU_SELECT: u8 = 0x1F;
    pub const MENU_SELECT_ALT: u8 = 0x20;
    pub const MENU_HELP: u8 = 0x21;
    pub const MENU_BACK: u8 = 0x22;
    pub const MENU_CUSTOMISE: u8 = 0x23;
    pub const MENU_BUTTON2: u8 = 0x24;
    pub const MENU_BUTTON3: u8 = 0x25;
    pub const MENU_L1: u8 = 0x26;
    pub const MENU_R1: u8 = 0x27;
}

/// The device type 0x5BFB70 gives (the DIDEVICEINSTANCE type's low byte,
/// or 0x800588's per-product override); 0x5C1500 attaches only these three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevType {
    /// DI8DEVTYPE_JOYSTICK (0x14), flag 0x86FF88.
    Joystick,
    /// DI8DEVTYPE_GAMEPAD (0x15), flag 0x86F8A8.
    Gamepad,
    /// DI8DEVTYPE_DRIVING (0x16), flag 0x86E5BC: a wheel (0x5C1010), with
    /// its own steering (0x4061C0) and autocenter off (0x5CD200).
    Driving,
}

impl DevType {
    pub fn code(self) -> u32 {
        match self {
            DevType::Joystick => 0x14,
            DevType::Gamepad => 0x15,
            DevType::Driving => 0x16,
        }
    }

    /// The Controls screen's kind (0x5B8730): 2 a wheel, 3 a joypad.
    pub fn kind(self) -> u8 {
        if self == DevType::Driving { 2 } else { 3 }
    }
}

/// The per-product device type overrides (0x800588, 0x14-byte entries:
/// guidProduct, type; read by 0x5BFB70).
pub const TYPE_OVERRIDES: [((u16, u16), DevType); 2] = [((0x044F, 0x04B2), DevType::Driving), ((0x06A3, 0xFF0C), DevType::Gamepad)];

/// The device type for a product (0x5BFB70).
pub fn dev_type(vid: u16, pid: u16, reported: DevType) -> DevType {
    TYPE_OVERRIDES.iter().find(|(p, _)| *p == (vid, pid)).map_or(reported, |&(_, t)| t)
}

/// 0x5CEF10 skips these (guidProduct 0x0802050D / 0x0805050D, or the
/// product names at 0x7A4154 / 0x7A413C).
pub fn skipped(vid: u16, pid: u16, name: &str) -> bool {
    (vid, pid) == (0x050D, 0x0802) || (vid, pid) == (0x050D, 0x0805) || name == "Nostromo n40 Gamepad" || name == "Nostromo n50 Speedpad"
}

/// What 0x5C11B0 asks the device (GetCapabilities and GetObjectInfo).
#[derive(Clone, Debug, PartialEq)]
pub struct Caps {
    pub dev_type: DevType,
    /// DIDEVCAPS dwAxes / dwButtons / dwPOVs.
    pub axes: u32,
    pub buttons: u32,
    pub povs: u32,
    /// DIDC_FORCEFEEDBACK (dwFlags bit 8, 0x87078C).
    pub ff: bool,
    pub vid: u16,
    pub pid: u16,
    /// Which axis objects exist, by DIJOYSTATE2 offset / 4 (GetObjectInfo
    /// by offset succeeds).
    pub objects: [bool; 8],
}

/// A device state as GetDeviceState fills c_dfDIJoystick2 (the parts the
/// game reads: the eight axes 0x00..0x1C, the POVs, the buttons).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiState {
    /// By offset / 4, −10000..=10000 (0x5C8670's DIPROP_RANGE).
    pub axes: [i32; 8],
    /// Hundredths of a degree clockwise from north; 0xFFFFFFFF centred.
    pub pov: [u32; 4],
    pub buttons: [bool; 32],
}

impl Default for DiState {
    fn default() -> DiState {
        DiState { axes: [0; 8], pov: [u32::MAX; 4], buttons: [false; 32] }
    }
}

impl DiState {
    pub fn axis(&self, offset: u32) -> i32 {
        self.axes.get(offset as usize / 4).copied().unwrap_or(0)
    }
}

/// The per-product presets 0x5B81C0 copies instead of the defaults
/// (0x7FE7F8, nine entries of 0xB0 bytes: guidProduct, 20 driving entries,
/// 20 FE entries; buttons 0–15 then POV 0's four directions).
pub const PRESETS: [((u16, u16), [u8; 20], [u8; 20]); 9] = [
    ((0x046D, 0xCA03), [5, 6, 9, 10, 15, 17, 16, 12, 8, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], [34, 31, 34, 31, 36, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ((0x0B43, 0x0003), [10, 9, 1, 2, 8, 7, 6, 5, 19, 22, 0, 0, 0, 0, 0, 0, 0, 4, 0, 3], [34, 0, 31, 35, 0, 0, 36, 37, 0, 0, 0, 0, 0, 0, 0, 0, 27, 30, 28, 29]),
    ((0x046D, 0xC216), [2, 1, 9, 10, 6, 5, 8, 7, 14, 12, 22, 19, 0, 0, 0, 0, 18, 15, 16, 17], [35, 31, 33, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x046D, 0xC21A), [2, 1, 9, 10, 6, 5, 8, 7, 14, 12, 22, 19, 0, 0, 0, 0, 18, 15, 16, 17], [35, 31, 33, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x046D, 0xC218), [2, 1, 9, 10, 6, 5, 8, 7, 14, 12, 22, 19, 0, 0, 0, 0, 18, 15, 16, 17], [35, 31, 33, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x046D, 0xC20A), [1, 2, 8, 9, 10, 7, 6, 5, 12, 0, 0, 0, 0, 0, 0, 0, 18, 15, 16, 17], [31, 35, 0, 34, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x05FD, 0x3002), [2, 10, 1, 9, 6, 5, 8, 7, 14, 12, 22, 19, 0, 0, 0, 0, 18, 15, 16, 17], [35, 34, 31, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x06A3, 0xFF0C), [2, 10, 1, 9, 7, 8, 6, 5, 19, 22, 0, 0, 0, 0, 0, 0, 18, 15, 16, 17], [35, 34, 31, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
    ((0x045E, 0x0027), [1, 9, 2, 10, 6, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 4], [31, 33, 35, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 27, 29, 30]),
];

/// The EMS USB2 PS2 adapter (guidProduct 0x00030B43 at 0x8005B0): its
/// combined pedal axis is lZ, not lRz (0x5C11B0), and on channel 1 its
/// lX / lY / lZ / lRz all at 10000 read as 0 (0x5C8710: no second pad).
pub const EMS_ADAPTER: (u16, u16) = (0x0B43, 0x0003);

/// The controller part of an input record (0x874C40 + 0x370 × k).
#[derive(Clone, Debug, PartialEq)]
pub struct PadMap {
    /// Four action tables of 32 entries (+0x18, +0x98, +0x118, +0x198):
    /// buttons 0–15, then four POVs × (down, up, left, right). The first is
    /// the driving map (the one the profile keeps), the second the FE map.
    pub tables: [[u8; 32]; 4],
    /// The analog roles' axis offsets (+0x344): 0 steering, 1 throttle,
    /// 2 brake, 3 the combined throttle / brake axis.
    pub axis: [u32; 4],
    /// Their directions (+0x354), ±1.
    pub sign: [i32; 4],
    /// Their flags (+0x364..+0x367).
    pub on: [bool; 4],
    /// +0x368: the pedals share one axis (role 3).
    pub combined: bool,
    /// The action being bound (+0x33C) and its column (+0x340).
    pub capture: u8,
    pub capture_column: u8,
    /// The axis values 0x5CA730 compares against (0x86F8B0..0x86F8CC, by
    /// DIJOYSTATE2 offset / 4): written at the end of each capture frame of
    /// an action 1–4, so a new capture starts from the last one's values
    /// (zero at start-up). Not in the profile.
    pub axis_base: [i32; 8],
    /// Clone extra (Controller Layout Modern, docs/playing/controller-support.md):
    /// the analog bytes made from the axes without the DirectInput chain's
    /// 8-bit quirks (rest steering 1/128 right, full pedal 252/255). Not in
    /// the profile; false is native.
    pub exact: bool,
}

/// A record's report (0x5C1850): the action mask, active low (bit a clear
/// while action a is held), and the analog bytes +0x10..+0x13 (throttle,
/// brake, steering, combined).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub mask: u64,
    pub analog: [u8; 4],
}

impl Default for Report {
    /// A detached record (0x5C0FA0): all released, analog 0, 0, 0x80, 0x80.
    fn default() -> Report {
        Report { mask: u64::MAX, analog: [0, 0, 0x80, 0x80] }
    }
}

impl Report {
    pub fn held(&self, action: u8) -> bool {
        action < 64 && self.mask & (1u64 << action) == 0
    }
}

/// MSVC's _ftol: truncation toward zero, wrapped to 32 bits.
fn ftol(v: f64) -> i32 {
    v.trunc() as i64 as i32
}

/// The axis scale 0x7A3C20 (−0.0127, f32).
const AXIS_SCALE: f32 = -0.0127;

/// One analog byte (0x5C1FF0..): `base − (i8)(ftol(v × −0.0127)) × sign`,
/// in 8-bit arithmetic.
fn analog_byte(v: i32, sign: i32, base: u8) -> u8 {
    let t = ftol(v as f64 * AXIS_SCALE as f64) as i8;
    base.wrapping_sub(t.wrapping_mul(sign as i8) as u8)
}

/// Clone extra (Controller Layout Modern): the byte a JOY row reads as the
/// exact value, so that after the row's XOR 1 a centred stick is 0x80, a
/// full stick 0x00 / 0xFF and a pedal axis −10000..10000 0..0xFF (the
/// combined axis 0..0xFF around 0x7F). `base` 0x80 the steering, 0x7F a
/// pedal (`combined` false) or the combined axis.
fn exact_byte(v: i32, sign: i32, base: u8) -> u8 {
    let f = (v as f32 / 10000.0 * sign as f32).clamp(-1.0, 1.0);
    let w = if base == 0x80 { 128.0 + f * 128.0 } else { (f + 1.0) * 127.5 };
    (w.round().clamp(0.0, 255.0) as u8) ^ 1
}

/// 0x5B83C0: a POV value's direction for a binding — a = v × 0.01 × π ×
/// 1/180 (f32), cos a = sin(a + π/2); the larger of |cos|, |sin| decides:
/// up (1) when cos > 0 else down (0), right (3) when sin > 0 else left (2).
pub fn pov_direction(v: u32) -> usize {
    let a = v as f32 * 0.01 * std::f32::consts::PI * (1.0 / 180.0);
    let (c, s) = ((a + std::f32::consts::FRAC_PI_2).sin(), a.sin());
    if c.abs() > s.abs() {
        if c > 0.0 { 1 } else { 0 }
    } else if s > 0.0 {
        3
    } else {
        2
    }
}

impl PadMap {
    /// 0x5C1500: a record taking a channel, then 0x5C11B0's defaults.
    pub fn attach(caps: &Caps) -> PadMap {
        let mut m = PadMap { tables: [[0; 32]; 4], axis: [axis::X, 0, 0, axis::Y], sign: [1, -1, -1, 1], on: [false; 4], combined: true, capture: 0, capture_column: 0, axis_base: [0; 8], exact: false };
        m.defaults(caps);
        m
    }

    /// Clone extra (Controller Layout Modern): a standard pad presented as
    /// [`host::Layout::Modern`] (buttons South East West North L1 R1 Select
    /// Start L3 R3 Mode; lX / lY the left stick, lRx / lRy the right, lZ the
    /// left trigger, lRz the right, the triggers resting at −10000): the
    /// left stick steers, the right trigger is the throttle and the left the
    /// brake (separate pedals), Start pauses; see
    /// docs/playing/controller-support.md for the whole table. Rebinding on the
    /// Controls page works on it as on a native map.
    pub fn modern(caps: &Caps) -> PadMap {
        use act::*;
        let mut m = PadMap { tables: [[0; 32]; 4], axis: [axis::X, axis::RZ, axis::Z, axis::Y], sign: [1, 1, 1, 1], on: [true, true, true, false], combined: false, capture: 0, capture_column: 0, axis_base: [0; 8], exact: true };
        let _ = caps;
        // South East West North L1 R1 Select Start L3 R3.
        m.tables[0][..10].copy_from_slice(&[NITROUS, LOOK_BACK, EBRAKE, CAMERA, SHIFT_DOWN, SHIFT_UP, WORLD_MAP, PAUSE, HYD_PRESSURE, HYD_BOUNCE]);
        m.tables[1][..10].copy_from_slice(&[MENU_SELECT, MENU_BACK, MENU_CUSTOMISE, MENU_BUTTON2, MENU_L1, MENU_R1, MENU_BUTTON3, MENU_SELECT_ALT, 0, 0]);
        // The d-pad (POV 0: down, up, left, right).
        m.tables[0][16..20].copy_from_slice(&[READ_SMS, ENGAGE, STEER_LEFT, STEER_RIGHT]);
        m.tables[1][16..20].copy_from_slice(&[MENU_DOWN, MENU_UP, MENU_LEFT, MENU_RIGHT]);
        m
    }

    /// 0x5C11B0 (also Restore Defaults, 0x4ACD90).
    pub fn defaults(&mut self, caps: &Caps) {
        if caps.axes != 0 {
            let has = |o: u32| caps.objects[o as usize / 4];
            self.axis[0] = axis::X;
            self.sign[0] = 1;
            self.on[0] = true;
            let combined = if caps.dev_type == DevType::Driving {
                // A wheel's pedals: one axis only when the device lacks
                // the separate ones (0x5C12EF).
                if has(axis::SLIDER0) {
                    (has(axis::Y) && !has(axis::Z)).then_some(axis::SLIDER0)
                } else {
                    (has(axis::Y) && !has(axis::RZ) && !has(axis::Z)).then_some(axis::Y)
                }
            } else if caps.axes < 4 {
                Some(axis::Y)
            } else if (caps.vid, caps.pid) == EMS_ADAPTER {
                Some(axis::Z)
            } else {
                Some(axis::RZ)
            };
            if let Some(a) = combined {
                self.axis[3] = a;
                self.sign[3] = 1;
                self.on[3] = true;
                self.combined = true;
            }
        }
        for t in &mut self.tables[..2] {
            t[..16].fill(0);
        }
        if let Some((_, a, b)) = PRESETS.iter().find(|(p, ..)| *p == (caps.vid, caps.pid)) {
            self.tables[0][..20].copy_from_slice(a);
            self.tables[1][..20].copy_from_slice(b);
            return;
        }
        use act::*;
        let buttons: &[u8] = if caps.buttons < 6 {
            &[NITROUS, EBRAKE, SHIFT_DOWN, SHIFT_UP, LOOK_BACK]
        } else if caps.buttons < 8 {
            &[BRAKE, THROTTLE, NITROUS, EBRAKE, SHIFT_DOWN, SHIFT_UP, LOOK_BACK]
        } else {
            &[BRAKE, THROTTLE, LOOK_BACK, CAMERA, NITROUS, EBRAKE, SHIFT_DOWN, SHIFT_UP, HYD_PRESSURE, HYD_BOUNCE]
        };
        self.tables[0][..buttons.len()].copy_from_slice(buttons);
        self.tables[1][..5].copy_from_slice(&[MENU_SELECT, MENU_BACK, MENU_CUSTOMISE, MENU_BUTTON2, MENU_BUTTON3]);
        // With few buttons the POVs also give the pedals.
        let (down, up) = if caps.buttons < 6 { (BRAKE, THROTTLE) } else { (0, 0) };
        for k in 0..4 {
            let o = 16 + 4 * k;
            for t in [0, 2] {
                self.tables[t][o..o + 4].copy_from_slice(&[down, up, STEER_LEFT, STEER_RIGHT]);
            }
            for t in [1, 3] {
                self.tables[t][o..o + 4].copy_from_slice(&[MENU_DOWN, MENU_UP, MENU_LEFT, MENU_RIGHT]);
            }
        }
    }

    /// 0x5C1850 for a controller record.
    pub fn poll(&self, caps: &Caps, st: &DiState) -> Report {
        let mut mask = u64::MAX;
        let mut clear = |a: u8| {
            if a < 64 {
                mask &= !(1u64 << a);
            }
        };
        for i in 0..16 {
            if st.buttons[i] {
                for t in &self.tables {
                    clear(t[i]);
                }
            }
        }
        for k in 0..(caps.povs as usize).min(4) {
            let p = st.pov[k];
            if p & 0xFFFF == 0xFFFF {
                continue;
            }
            // f32 arithmetic as the game's: × 0.01 × π × 1/180.
            let a = p as f32 * 0.01 * std::f32::consts::PI * (1.0 / 180.0);
            let (c, s) = ((a + std::f32::consts::FRAC_PI_2).sin(), a.sin());
            let o = 16 + 4 * k;
            for t in &self.tables {
                if c < -0.1 {
                    clear(t[o]);
                }
                if c > 0.1 {
                    clear(t[o + 1]);
                }
                if s < -0.1 {
                    clear(t[o + 2]);
                }
                if s > 0.1 {
                    clear(t[o + 3]);
                }
            }
        }
        let v = |r: usize| st.axis(self.axis[r]);
        let byte = |r: usize, base: u8| if self.exact { exact_byte(v(r), self.sign[r], base) } else { analog_byte(v(r), self.sign[r], base) };
        // The bytes of unused roles: the original's (read through the rows'
        // XOR 1 as 1/128 right, a pedal 1/255, the combined axis 2/255),
        // with `exact` ones that read 0.
        let (idle_steer, idle_pedal, idle_combined) = if self.exact { (0x81, 1, 0x7E) } else { (0x80, 0, 0x80) };
        let steer = if self.on[0] { byte(0, 0x80) } else { idle_steer };
        let analog = if self.combined && self.on[3] {
            [idle_pedal, idle_pedal, steer, byte(3, 0x7F)]
        } else {
            let brake = if self.on[2] { byte(2, 0x7F) } else { idle_pedal };
            let gas = if self.on[1] { byte(1, 0x7F) } else { idle_pedal };
            [gas, brake, steer, idle_combined]
        };
        Report { mask, analog }
    }

    /// 0x5C2110: wait for a button or axis for `action`'s `column`.
    pub fn begin_capture(&mut self, action: u8, column: u8) {
        self.capture = action;
        self.capture_column = column;
    }

    /// 0x5B8470: bind `slot` (a button 0–15, or a POV entry 16–31) to the
    /// action being captured.
    pub fn bind_button(&mut self, slot: usize) {
        let a = self.capture;
        let mut keep_one = !((self.on[0] && (a == 3 || a == 4))
            || (self.on[1] && !self.combined && a == 1)
            || (self.on[2] && !self.combined && a == 2)
            || (self.on[3] && self.combined && (a == 1 || a == 2)));
        for e in self.tables[0].iter_mut() {
            if *e == a {
                if self.capture_column == 1 && keep_one && !(1..=4).contains(&a) {
                    keep_one = false;
                } else {
                    *e = 0;
                }
            }
        }
        if let Some(e) = self.tables[0].get_mut(slot) {
            *e = a;
        }
        self.capture = 0;
    }

    /// 0x5B8260: give axis `offset`, moved in direction `dir` (±1), to the
    /// role the captured action asks for.
    pub fn assign_axis(&mut self, offset: u32, dir: i32) {
        for r in 0..4 {
            if self.axis[r] == offset {
                self.on[r] = false;
            }
        }
        match self.capture {
            1 | 2 => {
                let (me, other) = if self.capture == 1 { (1, 2) } else { (2, 1) };
                self.axis[me] = offset;
                self.sign[me] = dir;
                if self.axis[other] == offset && self.sign[other] != dir {
                    // Throttle and brake on the two halves of one axis.
                    self.sign[3] = if self.capture == 1 { -dir } else { dir };
                    self.axis[3] = offset;
                    self.on[3] = true;
                    self.combined = true;
                } else {
                    self.on[me] = true;
                    self.combined = false;
                }
            }
            3 | 4 => {
                self.axis[0] = offset;
                self.sign[0] = if self.capture == 3 { -dir } else { dir };
                self.on[0] = true;
            }
            _ => {}
        }
        self.capture = 0;
    }

    /// 0x5CA730, one frame of a capture (the original's frames are 60 Hz
    /// ticks; the caller runs it once per 1/60 s tick): the first button
    /// 0–15 pressed since the last frame (0x5B8470); then every POV that is
    /// not centred (0x5C1730 — with the capture already taken by a button
    /// that frame, it writes 0 to the POV's direction); then, for an action
    /// 1–4, **every** axis of lX, lRx, lY, lRy, lZ, lRz, slider 0, 1 that
    /// moved more than 1000 (500 on a wheel) from [`Self::axis_base`]
    /// (0x5B8260 each: the first takes the action, each later one only
    /// leaves the roles that used it — traced 2026-11-11, so this is the
    /// original's behaviour, as the audit found) and the base set
    /// to the frame's values. True once the capture is over.
    pub fn capture_step(&mut self, caps: &Caps, prev: &DiState, now: &DiState) -> bool {
        self.capture_step_with(caps, prev, now, false)
    }

    /// [`Self::capture_step`]; `safe` (the Safe Axis Binding extra): only
    /// the axis that moved most (the first in the capture order on a tie)
    /// is given to the action, the others keep their roles.
    pub fn capture_step_with(&mut self, caps: &Caps, prev: &DiState, now: &DiState, safe: bool) -> bool {
        if self.capture == 0 {
            return false;
        }
        if let Some(i) = (0..16).find(|&i| now.buttons[i] && !prev.buttons[i]) {
            self.bind_button(i);
        }
        for k in 0..4 {
            if now.pov[k] & 0xFFFF != 0xFFFF {
                self.bind_pov(k, pov_direction(now.pov[k]));
            }
        }
        if (1..=4).contains(&self.capture) {
            let limit = if caps.dev_type == DevType::Driving { 500 } else { 1000 };
            let base = self.axis_base;
            let moved = axis::CAPTURE_ORDER.into_iter().map(|o| (o, now.axis(o) - base[o as usize / 4])).filter(|&(_, d)| d.abs() > limit);
            if safe {
                if let Some((o, d)) = moved.fold(None, |best: Option<(u32, i32)>, (o, d)| if best.is_none_or(|(_, b)| d.abs() > b.abs()) { Some((o, d)) } else { best }) {
                    self.assign_axis(o, if d >= 0 { 1 } else { -1 });
                }
            } else {
                for (o, d) in moved {
                    self.assign_axis(o, if d >= 0 { 1 } else { -1 });
                }
            }
            self.axis_base = now.axes;
        }
        self.capture == 0
    }

    /// 0x5C1730: POV `k`'s direction `dir` (0 down, 1 up, 2 left, 3
    /// right) takes the action being captured, with 0x5B8470's rule for
    /// the action's other entries.
    pub fn bind_pov(&mut self, k: usize, dir: usize) {
        let slot = 16 + 4 * k + dir;
        self.bind_button(slot);
    }

    /// What a Controls cell names (0x5B8AA0: the action's axis, else its
    /// first button or POV — the second for column 1 when the first is not
    /// an axis).
    pub fn binding(&self, caps: &Caps, action: u8, column: u8) -> Binding {
        let mut skip_first = true;
        let mut role: Option<usize> = None;
        if self.on[0] && (action == 3 || action == 4) {
            if column == 0 {
                role = Some(0);
            } else {
                skip_first = false;
            }
        }
        if self.on[1] && !self.combined && action == 1 {
            if column == 0 {
                role = Some(1);
            } else {
                skip_first = false;
            }
        }
        if self.on[2] && !self.combined && action == 2 {
            if column == 0 {
                role = Some(2);
            } else {
                skip_first = false;
            }
        }
        if self.on[3] && self.combined && (action == 1 || action == 2) {
            if column == 0 {
                role = Some(3);
            } else {
                skip_first = false;
            }
        }
        if let Some(r) = role {
            let o = self.axis[r];
            if caps.objects.get(o as usize / 4).copied().unwrap_or(false) {
                return Binding::Axis(o);
            }
        }
        let t = &self.tables[0];
        for i in 0..16 {
            if t[i] == action {
                if column == 1 && skip_first {
                    skip_first = false;
                } else if (i as u32) < caps.buttons {
                    return Binding::Button(i as u8);
                }
            }
        }
        for k in 0..4 {
            for d in 0..4 {
                if t[16 + 4 * k + d] == action {
                    if column == 1 && skip_first {
                        skip_first = false;
                    } else if (k as u32) < caps.povs {
                        return Binding::Pov(k as u8);
                    }
                }
            }
        }
        Binding::None
    }

    /// The profile's block for this record (0x5B9430): the device GUID,
    /// +0x344..+0x368, then the driving table as 32 dwords.
    pub fn to_block(&self, guid: &[u8; 16]) -> [u8; BLOCK_LEN] {
        let mut b = [0u8; BLOCK_LEN];
        b[..16].copy_from_slice(guid);
        for r in 0..4 {
            b[0x10 + 4 * r..0x14 + 4 * r].copy_from_slice(&self.axis[r].to_le_bytes());
            b[0x20 + 4 * r..0x24 + 4 * r].copy_from_slice(&self.sign[r].to_le_bytes());
            b[0x30 + r] = self.on[r] as u8;
        }
        b[0x34] = self.combined as u8;
        for (i, &a) in self.tables[0].iter().enumerate() {
            b[0x35 + 4 * i..0x39 + 4 * i].copy_from_slice(&(a as u32).to_le_bytes());
        }
        b
    }

    /// 0x5B9580: a block's settings given to the record with its GUID.
    pub fn apply_block(&mut self, b: &[u8]) {
        if b.len() < BLOCK_LEN {
            return;
        }
        let d = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        for r in 0..4 {
            self.axis[r] = d(0x10 + 4 * r);
            self.sign[r] = d(0x20 + 4 * r) as i32;
            self.on[r] = b[0x30 + r] != 0;
        }
        self.combined = b[0x34] != 0;
        for i in 0..32 {
            self.tables[0][i] = d(0x35 + 4 * i).min(255) as u8;
        }
    }
}

/// A Controls cell's binding (0x5B8AA0; the names are the device's
/// object names, DIDEVICEOBJECTINSTANCE tszName).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    None,
    Axis(u32),
    Button(u8),
    Pov(u8),
}

impl Binding {
    /// The cell's text. Buttons: DirectInput's "Button n" renumbered from
    /// 1 in English (0x5B8C4A: "Button " + (n + 1)). The axis and POV
    /// names are the driver's (**inferred**: Windows' HID names); an
    /// unbound cell is "" (drawn as `---`, 0x790B94).
    pub fn name(self) -> String {
        match self {
            Binding::None => String::new(),
            Binding::Button(i) => format!("Button {}", i + 1),
            Binding::Pov(_) => "Hat Switch".into(),
            Binding::Axis(o) => match o {
                axis::X => "X Axis",
                axis::Y => "Y Axis",
                axis::Z => "Z Axis",
                axis::RX => "X Rotation",
                axis::RY => "Y Rotation",
                axis::RZ => "Z Rotation",
                _ => "Slider",
            }
            .into(),
        }
    }
}

/// The JOY row bit read (0x571B20): `bits` holds the mask (0–63) then the
/// analog bytes (64–95); the row's inversion byte is XORed into the
/// value's low bit / byte.
fn row_byte(r: &Report, bit: u8, invert: u8) -> u8 {
    let byte = if bit >= 64 { r.analog[((bit - 64) / 8) as usize] } else { (r.mask >> bit) as u8 };
    byte ^ invert
}

/// The analog row 0x5795D0: the byte (inverted by the row, +0x10 = 1)
/// through the device context's graph of type 3 (0x57C250 builds every
/// type's graph as (0, 0), (128, 128), (128, 128), (255, 255) — the
/// identity — and nothing else writes them), then the half: 0 the whole
/// byte, 1 the lower half (0x7F − v) × 2, 2 the upper (v − 0x80) × 2, with
/// 0xFE read as 0xFF.
fn analog_row(r: &Report, bit: u8, half: u8) -> u8 {
    let v = row_byte(r, bit, 1) as u32;
    let v = match half {
        1 => (0x7Fu32.saturating_sub(v)) * 2,
        2 => v.saturating_sub(0x80) * 2,
        _ => return v as u8,
    };
    if v == 0xFE { 0xFF } else { v as u8 }
}

/// The neutral throttle graph (0x7EA004 → 0x7E94F8, six points, f32).
pub const NEUTRAL_THROTTLE: [(f32, f32); 6] = [
    (0.0, 0.0),
    (f32::from_bits(0x3F13_3333), f32::from_bits(0x3EDF_BE77)),
    (f32::from_bits(0x3F4C_0831), f32::from_bits(0x3F1E_B852)),
    (f32::from_bits(0x3F64_DD2F), f32::from_bits(0x3F3E_F9DB)),
    (f32::from_bits(0x3F73_3333), f32::from_bits(0x3F5D_B22D)),
    (1.0, 1.0),
];

/// 0x40C840: a piecewise-linear graph — the first point's y at or below
/// its x, the last's at or above its x, else the segment's (1 − t) × y0 +
/// t × y1 (0x576750) with t = (x − x0) / (x1 − x0) stored as f32.
pub fn graph(points: &[(f32, f32)], x: f32) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return 0.0 };
    if points.len() < 2 || x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x >= x0 && x <= x1 {
            let t = ((f64::from(x) - f64::from(x0)) / (f64::from(x1) - f64::from(x0))) as f32;
            return ((1.0 - f64::from(t)) * f64::from(y0) + f64::from(t) * f64::from(y1)) as f32;
        }
    }
    last.1
}

/// The driving inputs the human input object makes of the JOY events
/// (0x41E060's handlers).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Driving {
    /// +0x2C8: positive left (−1..1).
    pub steer: f32,
    /// +0x2D4, +0x2DC, +0x2EC: 0..1.
    pub throttle: f32,
    pub brake: f32,
    pub ebrake: f32,
}

/// 0x407750 (JOY_EVENT_STEER 0x8F / _ANALOG 0x90), 0x416DE0 (THROTTLE
/// 0x91 / _ANALOG 0x92 / _ANALOG_ALTERNATE 0x93), 0x407EB0 (BRAKE 0x94–
/// 0x96) and 0x407F40 (EBRAKE 0x99) over the rows of 0x7FDD10: `held` the
/// actions held on any device (bit a set), `pad` the analog report of the
/// pad (None: no analog device, the analog values stay at rest), `neutral`
/// the player's car in neutral (gear +0x434 = 1).
pub fn driving(held: u64, pad: Option<&Report>, neutral: bool) -> Driving {
    let h = |a: u8| held & (1u64 << a) != 0;
    // STEER (row 25, 0x571CD0): left only 0x00, right only 0xFF, else 0x80.
    let digital = match (h(act::STEER_LEFT), h(act::STEER_RIGHT)) {
        (true, false) => 0x00u8,
        (false, true) => 0xFF,
        _ => 0x80,
    };
    let analog = pad.map_or(0x80, |r| analog_row(r, 0x50, 0));
    let v = if digital != 0x80 { digital } else { analog };
    let steer = if v == 0xFF { -1.0 } else { (v as f32 - 128.0) * -0.0078125 };
    let full = 0xFFu32;
    // THROTTLE (row 26, value 0xFF), _ANALOG (row 36: the throttle byte;
    // 0x416E72 scales it by 2 / 4 only when 0x404800 says so, and 0x404800
    // always returns 0: × 1), _ANALOG_ALTERNATE (row 37: the combined
    // byte's lower half × 1.07, 0x783B1C, except × 1 when the car's gear
    // +0x434 is 1, neutral: 0x416E39).
    let throttle = if h(act::THROTTLE) {
        full
    } else if let Some(r) = pad {
        let a = analog_row(r, 0x40, 0) as u32;
        let lower = analog_row(r, 0x58, 1) as f32;
        let alt = ftol((if neutral { lower } else { lower * 1.07f32 }).min(255.0) as f64) as u32;
        a.max(alt)
    } else {
        0
    };
    // BRAKE (row 27), _ANALOG (row 38), _ANALOG_ALTERNATE (row 39: the upper half).
    let brake = if h(act::BRAKE) {
        full
    } else if let Some(r) = pad {
        (analog_row(r, 0x48, 0) as u32).max(analog_row(r, 0x58, 2) as u32)
    } else {
        0
    };
    // EBRAKE (row 28, value 0xFF; _ANALOG 0x9A has no row on the PC).
    let ebrake = if h(act::EBRAKE) { full } else { 0 };
    let k = 0.003921569f32;
    let mut gas = throttle as f32 * k;
    // In neutral the throttle goes through the graph 0x7EA004 (0x416F2E →
    // 0x40C840; 0x7EA00C would need 0x404800, which is always false).
    if neutral {
        gas = graph(&NEUTRAL_THROTTLE, gas);
    }
    Driving { steer, throttle: gas, brake: brake as f32 * k, ebrake: ebrake as f32 * k }
}

/// The FE pad directions from the analog bytes (rows 18–21,
/// FENG_PAD_*_ALTERNATE, 0x571D60: the combined byte (bit 0x58) below
/// 0x40 up / above 0xBF down, the steering byte (0x50) left / right; 0x64
/// / 0x9B with a wheel, 0x5C0F70); [`FeRepeat`] makes the presses.
pub fn fe_stick(r: &Report, wheel: bool) -> [bool; 4] {
    let (lo, hi) = if wheel { (0x64, 0x9B) } else { (0x40, 0xBF) };
    let ud = row_byte(r, 0x58, 1);
    let lr = row_byte(r, 0x50, 1);
    [ud < lo, ud > hi, lr < lo, lr > hi]
}

/// The FE stick rows' presses (0x571D60, rows 18–21; traced 2026-11-11):
/// crossing the threshold sends the press and sets the row's +0x20 to the
/// tick 0x865134 + 25; held past it, a press each time the tick is above
/// +0x20, which becomes tick + 6. But 0x865134 has no writer in the code
/// and reads 0 under Wine in the main menu, the pause menu and free roam
/// (peek 2026-11-11; 0x57A1B3 would turn it into seconds by × 1/60):
/// with tick 0 the deadline 25 is never passed, so the original sends the
/// press only — the native path passes tick 0. The Modern layout's menu
/// repeat (clone extra) passes the 60 Hz ticks of the game clock instead,
/// the same at any frame rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeRepeat {
    held: [bool; 4],
    deadline: [i64; 4],
}

impl FeRepeat {
    /// The presses for the directions held `now` at tick `tick`.
    pub fn step(&mut self, now: [bool; 4], tick: i64) -> [bool; 4] {
        let mut out = [false; 4];
        for i in 0..4 {
            if now[i] && !self.held[i] {
                out[i] = true;
                self.deadline[i] = tick + 25;
            } else if now[i] && tick > self.deadline[i] {
                out[i] = true;
                self.deadline[i] = tick + 6;
            }
            self.held[i] = now[i];
        }
        out
    }
}

/// The tick 0x865134 for a game-clock time in seconds (60 Hz).
pub fn tick_at(seconds: f64) -> i64 {
    (seconds * 60.0).floor() as i64
}

/// The FE actions held in a report (bits 0x1B..0x2F).
pub fn fe_actions(r: &Report) -> u64 {
    !r.mask & 0x0000_FFFF_F800_0000
}

pub mod extras;
pub mod host;

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(axes: u32, buttons: u32) -> Caps {
        Caps { dev_type: DevType::Gamepad, axes, buttons, povs: 1, ff: false, vid: 0x1234, pid: 0x5678, objects: [true, true, true, false, false, true, false, false] }
    }

    #[test]
    fn defaults_follow_the_button_count() {
        let m = PadMap::attach(&pad(4, 12));
        assert_eq!(&m.tables[0][..10], &[2, 1, 9, 10, 6, 5, 8, 7, 22, 19]);
        assert_eq!(&m.tables[0][16..20], &[0, 0, 3, 4]);
        assert_eq!(&m.tables[1][..5], &[0x1F, 0x22, 0x23, 0x24, 0x25]);
        assert_eq!(&m.tables[1][16..20], &[0x1C, 0x1B, 0x1D, 0x1E]);
        assert_eq!((m.axis[0], m.axis[3], m.on, m.combined), (axis::X, axis::RZ, [true, false, false, true], true));
        let m = PadMap::attach(&pad(2, 4));
        assert_eq!(&m.tables[0][..5], &[6, 5, 8, 7, 9]);
        assert_eq!(&m.tables[0][16..20], &[2, 1, 3, 4]);
        assert_eq!(m.axis[3], axis::Y);
        let m = PadMap::attach(&pad(4, 7));
        assert_eq!(&m.tables[0][..7], &[2, 1, 6, 5, 8, 7, 9]);
    }

    #[test]
    fn presets_replace_the_defaults() {
        let mut c = pad(4, 12);
        (c.vid, c.pid) = (0x046D, 0xC216);
        let m = PadMap::attach(&c);
        assert_eq!(&m.tables[0][..20], &PRESETS[2].1);
        assert_eq!(&m.tables[1][..20], &PRESETS[2].2);
        assert_eq!(m.tables[2], [0; 32]);
    }

    #[test]
    fn poll_and_driving() {
        let c = pad(4, 12);
        let m = PadMap::attach(&c);
        let mut st = DiState::default();
        let r = m.poll(&c, &st);
        assert_eq!(r.analog, [0, 0, 0x80, 0x7F]);
        // At rest: the rows' inversion makes the steering 0x81 (1/128
        // right), the combined byte 0x7E (throttle 2 × 1.07 → 2) and the
        // unused brake byte 1.
        let d = driving(0, Some(&r), false);
        assert_eq!(d.steer, -0.0078125);
        assert!((d.throttle - 2.0 / 255.0).abs() < 1e-6 && (d.brake - 1.0 / 255.0).abs() < 1e-6);
        // Full left, right stick up: _ftol truncates −10000 × −0.0127f
        // (126.99999…) to 126, so the steering byte is 0x80 − 126 = 2 (^1 =
        // 3, 125/128 left) and the combined byte 1 (^1 = 0): throttle
        // (0x7F − 0) × 2 = 0xFE, read as 0xFF.
        st.axes[0] = -10000;
        st.axes[5] = -10000;
        let r = m.poll(&c, &st);
        assert_eq!(r.analog, [0, 0, 2, 1]);
        let d = driving(0, Some(&r), false);
        assert_eq!((d.steer, d.throttle), (0.9765625, 1.0));
        // Full right, stick down: 0xFE (^1 = 0xFF: −1) and the combined
        // byte 0xFD (^1 = 0xFC): brake (0xFC − 0x80) × 2 = 248.
        st.axes[0] = 10000;
        st.axes[5] = 10000;
        let r = m.poll(&c, &st);
        assert_eq!(r.analog, [0, 0, 0xFE, 0xFD]);
        let d = driving(0, Some(&r), false);
        assert_eq!(d.steer, -1.0);
        assert!((d.brake - 248.0 / 255.0).abs() < 1e-6);
        // A digital steer key wins; both keys fall back to the stick.
        assert_eq!(driving(1 << 3, Some(&r), false).steer, 1.0);
        assert_eq!(driving((1 << 3) | (1 << 4), Some(&r), false).steer, -1.0);
        // Buttons: 0 brake, 1 throttle; POV right steers right.
        st = DiState::default();
        st.buttons[1] = true;
        st.pov[0] = 9000;
        let r = m.poll(&c, &st);
        assert!(r.held(act::THROTTLE) && r.held(act::STEER_RIGHT) && r.held(act::MENU_RIGHT) && !r.held(act::BRAKE));
        assert!(r.held(act::MENU_BACK));
        let held = !r.mask;
        let d = driving(held, Some(&r), false);
        assert_eq!((d.steer, d.throttle), (-1.0, 1.0));
    }

    #[test]
    fn separate_pedals_and_combined_capture() {
        let c = pad(6, 12);
        let mut m = PadMap::attach(&c);
        // Throttle on lRx pushed up, brake on lRx pushed down: combined.
        m.begin_capture(1, 0);
        m.assign_axis(axis::RX, -1);
        assert!(!m.combined && m.on[1]);
        m.begin_capture(2, 0);
        m.assign_axis(axis::RX, 1);
        assert!(m.combined && m.on[3] && m.axis[3] == axis::RX && m.sign[3] == 1);
        // Throttle on lRy alone: separate pedals.
        m.begin_capture(1, 0);
        m.assign_axis(axis::RY, 1);
        assert!(!m.combined && m.on[1] && m.axis[1] == axis::RY);
        let mut st = DiState::default();
        st.axes[4] = 10000;
        let r = m.poll(&c, &st);
        assert_eq!(r.analog[0], 0x7F + 126);
        assert_eq!(r.analog[3], 0x80);
    }

    #[test]
    fn binding_a_button_moves_it() {
        let c = pad(4, 12);
        let mut m = PadMap::attach(&c);
        assert_eq!(m.binding(&c, act::LOOK_BACK, 0), Binding::Button(2));
        m.begin_capture(act::LOOK_BACK, 0);
        m.bind_button(11);
        assert_eq!(m.tables[0][2], 0);
        assert_eq!(m.binding(&c, act::LOOK_BACK, 0), Binding::Button(11));
        assert_eq!(m.binding(&c, act::STEER_LEFT, 0), Binding::Axis(axis::X));
        assert_eq!(m.binding(&c, act::STEER_LEFT, 1), Binding::Pov(0));
        assert_eq!(m.binding(&c, act::THROTTLE, 0), Binding::Axis(axis::RZ));
        assert_eq!(m.binding(&c, act::THROTTLE, 1), Binding::Button(1));
        assert_eq!(m.binding(&c, act::PAUSE, 0), Binding::None);
        assert_eq!(Binding::Button(1).name(), "Button 2");
    }

    #[test]
    fn capture_takes_a_new_button_or_a_moved_axis() {
        let c = pad(4, 12);
        let mut m = PadMap::attach(&c);
        let prev = DiState::default();
        let mut now = prev;
        now.axes[1] = 900;
        m.begin_capture(act::STEER_LEFT, 0);
        assert!(!m.capture_step(&c, &prev, &now));
        now.axes[1] = -5000;
        assert!(m.capture_step(&c, &prev, &now));
        assert_eq!((m.axis[0], m.sign[0]), (axis::Y, 1));
        m.begin_capture(act::PAUSE, 0);
        now.buttons[9] = true;
        assert!(m.capture_step(&c, &prev, &now));
        assert_eq!(m.tables[0][9], act::PAUSE);
    }

    #[test]
    fn block_round_trip() {
        let c = pad(4, 12);
        let mut m = PadMap::attach(&c);
        m.begin_capture(act::PAUSE, 0);
        m.bind_button(9);
        let g = [7u8; 16];
        let b = m.to_block(&g);
        assert_eq!(&b[..16], &g);
        assert_eq!(b.len(), 0xB5);
        let mut n = PadMap::attach(&c);
        n.apply_block(&b);
        assert_eq!(n.tables[0], m.tables[0]);
        assert_eq!((n.axis, n.sign, n.on, n.combined), (m.axis, m.sign, m.on, m.combined));
    }

    #[test]
    fn neutral_throttle_takes_the_graph_and_no_1_07() {
        assert_eq!(graph(&NEUTRAL_THROTTLE, 0.0), 0.0);
        assert_eq!(graph(&NEUTRAL_THROTTLE, 1.0), 1.0);
        assert!((graph(&NEUTRAL_THROTTLE, 0.575) - 0.437).abs() < 1e-6);
        // Half-way along the first segment.
        assert!((graph(&NEUTRAL_THROTTLE, 0.2875) - 0.2185).abs() < 1e-4);
        let c = pad(4, 12);
        let m = PadMap::attach(&c);
        let mut st = DiState::default();
        // The right stick half up: the combined byte's lower half.
        st.axes[5] = -5000;
        let r = m.poll(&c, &st);
        let (geared, neutral) = (driving(0, Some(&r), false), driving(0, Some(&r), true));
        let lower = analog_row(&r, 0x58, 1) as f32;
        let analog = analog_row(&r, 0x40, 0) as f32;
        assert_eq!(geared.throttle, (lower * 1.07).trunc().max(analog) * 0.003921569);
        assert_eq!(neutral.throttle, graph(&NEUTRAL_THROTTLE, lower.max(analog) * 0.003921569));
        assert!(neutral.throttle < geared.throttle);
        // The keyboard's full throttle stays full.
        assert_eq!(driving(1 << act::THROTTLE, Some(&r), true).throttle, 1.0);
    }

    #[test]
    fn pov_capture_and_direction() {
        assert_eq!([0, 9000, 18000, 27000].map(pov_direction), [1, 3, 0, 2]);
        let c = pad(4, 12);
        let mut m = PadMap::attach(&c);
        let prev = DiState::default();
        let mut now = prev;
        now.pov[0] = 18000;
        m.begin_capture(act::PAUSE, 0);
        assert!(m.capture_step(&c, &prev, &now));
        assert_eq!(m.tables[0][16], act::PAUSE);
        // A button and a held POV in the same frame: the button takes the
        // action, then 0x5C1730 writes the capture (now 0) to the POV's
        // direction.
        let mut now2 = now;
        now2.buttons[11] = true;
        now2.pov[0] = 27000;
        m.begin_capture(act::CAMERA, 0);
        assert!(m.capture_step(&c, &prev, &now2));
        assert_eq!((m.tables[0][11], m.tables[0][18]), (act::CAMERA, 0));
    }

    #[test]
    fn native_capture_drops_other_moved_axes_and_safe_does_not() {
        let c = pad(6, 12);
        let mut native = PadMap::attach(&c);
        // The combined pedals on lY (as a wheel or a small pad has them).
        native.axis[3] = axis::Y;
        let mut safe = native.clone();
        let prev = DiState::default();
        let mut now = prev;
        now.axes[0] = 9000;
        now.axes[1] = 3000;
        native.begin_capture(act::STEER_RIGHT, 0);
        assert!(native.capture_step(&c, &prev, &now));
        assert_eq!((native.axis[0], native.on[0], native.on[3]), (axis::X, true, false));
        safe.begin_capture(act::STEER_RIGHT, 0);
        assert!(safe.capture_step_with(&c, &prev, &now, true));
        assert_eq!((safe.axis[0], safe.on[0], safe.on[3]), (axis::X, true, true));
        // The base: a second capture compares with the first one's frame.
        native.begin_capture(act::STEER_LEFT, 0);
        assert!(!native.capture_step(&c, &prev, &now));
    }

    #[test]
    fn modern_map_reads_triggers_as_exact_pedals() {
        let caps = host::Layout::Modern.caps(0x045E, 0x0B12);
        let m = PadMap::modern(&caps);
        let mut p = host::PadInputs::default();
        let rest = m.poll(&caps, &host::Layout::Modern.state(&p));
        let d = driving(0, Some(&rest), false);
        assert_eq!((d.steer.abs(), d.throttle, d.brake), (0.0, 0.0, 0.0));
        p.triggers = [0.0, 1.0];
        p.sticks[0] = -1.0;
        let r = m.poll(&caps, &host::Layout::Modern.state(&p));
        let d = driving(0, Some(&r), false);
        assert_eq!((d.steer, d.throttle, d.brake), (1.0, 1.0, 0.0));
        p.triggers = [0.5, 0.0];
        p.sticks[0] = 1.0;
        let r = m.poll(&caps, &host::Layout::Modern.state(&p));
        let d = driving(0, Some(&r), false);
        assert_eq!(d.steer, -1.0);
        assert!((d.brake - 0.5).abs() < 0.01);
        // Start pauses, South is nitrous, the d-pad up engages.
        p = host::PadInputs::default();
        p.buttons[host::Btn::Start as usize] = true;
        p.buttons[host::Btn::South as usize] = true;
        p.buttons[host::Btn::Up as usize] = true;
        let r = m.poll(&caps, &host::Layout::Modern.state(&p));
        assert!(r.held(act::PAUSE) && r.held(act::NITROUS) && r.held(act::ENGAGE) && r.held(act::MENU_SELECT) && r.held(act::MENU_UP));
    }

    #[test]
    fn fe_repeat_is_the_same_at_any_rate() {
        // Held for 1 s from t = 0.1: the press, a repeat at tick 25 past
        // it, then every 6 ticks: the same count at 30, 60, 120 and 200 Hz.
        let mut counts = Vec::new();
        for hz in [30.0f64, 60.0, 120.0, 200.0] {
            let mut rep = FeRepeat::default();
            let mut n = 0;
            let mut t = 0.0;
            while t < 1.5 {
                let held = (0.1..1.1).contains(&t);
                n += rep.step([held, false, false, false], tick_at(t))[0] as u32;
                t += 1.0 / hz;
            }
            counts.push(n);
        }
        // Ticks 6..66: press at 6, repeats at 32, 39, … ≤ 65: 1 + 1 + 5.
        assert!(counts.iter().all(|&n| (6..=7).contains(&n)), "{counts:?}");
        assert!(counts[1..].iter().all(|&n| n == counts[1]), "{counts:?}");
    }

    #[test]
    fn fe_bits_and_stick() {
        let c = pad(4, 12);
        let m = PadMap::attach(&c);
        let mut st = DiState::default();
        st.buttons[0] = true;
        let r = m.poll(&c, &st);
        assert_eq!(fe_actions(&r), 1 << act::MENU_SELECT);
        st.axes[0] = -10000;
        let r = m.poll(&c, &st);
        assert_eq!(fe_stick(&r, false), [false, false, true, false]);
    }
}
