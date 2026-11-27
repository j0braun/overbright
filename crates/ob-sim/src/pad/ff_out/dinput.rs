//! The DirectInput 8 calls of the original's force feedback
//! (docs/formats/front-end.md, **Force feedback**), for Windows.
//!
//! [`params`] gives the DIEFFECT each [`Effect`] is created or updated
//! with, field for field as 0x5C04F0 (conditions), 0x5C0210 (constant) and
//! 0x5C8D30 (periodic) fill it; [`Plan`] turns the recorder's [`Call`]s into
//! the device and effect methods in the order the original calls them
//! (0x5C9160 / 0x5C9390 / 0x5C9770 / 0x5C9FE0 / 0x5C9DA0 and their
//! helpers). Both are plain data, tested on every platform; the `device`
//! module (Windows only) makes the calls.
//!
//! - **Create** (CreateEffect, vtable +0x48): when the slot has no effect.
//! - **Update** (SetParameters, +0x18, always DIEP_TYPESPECIFICPARAMS |
//!   DIEP_ENVELOPE = 0x140, 0x5C06B0 / 0x5C03A0 / 0x5C8EE0): when it has one
//!   of the same GUID.
//! - **Unload and create**: a periodic effect whose wave (GUID) changes
//!   (0x5C9FE0 → 0x5C0000: Acquire, then Unload, +0x2C).
//! - **Start** (0x5BFEF0): Acquire (+0x1C), and only when that succeeds
//!   Start(1, 0).
//! - **Stop** (0x5BFF70): Acquire (its result unused), Stop (+0x20).
//! - **Gain**: SetProperty(DIPROP_FFGAIN, DIPH_DEVICE) (0x5C00C0).
//! - **Autocenter**: SetProperty(DIPROP_AUTOCENTER, 0) right after the
//!   device is opened (0x5CD3E7).

use super::Call;
use crate::pad::ff::{Command, Effect, JOLT_FADE_MS, JOLT_MS, JOLT_PERIOD_MS, Slot, Wave};

/// DIEFF_OBJECTOFFSETS | DIEFF_CARTESIAN, every effect's flags.
pub const FLAGS: u32 = 0x12;
/// DIEP_TYPESPECIFICPARAMS | DIEP_ENVELOPE, every SetParameters.
pub const SET_FLAGS: u32 = 0x140;
/// INFINITE.
pub const INFINITE: u32 = 0xFFFF_FFFF;
/// DIEB_NOTRIGGER.
pub const NO_TRIGGER: u32 = 0xFFFF_FFFF;
/// DIJOFS_X, DIJOFS_Y (c_dfDIJoystick2, 0x7BFDE0): the actuator axes.
pub const AXES: [u32; 2] = [0, 4];

/// The effect GUID (0x7BFC90..0x7BFCF0; the damper's GUID_Damper).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spring,
    Damper,
    Constant,
    Sine,
    Square,
    Triangle,
    SawUp,
    SawDown,
}

/// DICONDITION.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Condition {
    pub offset: i32,
    pub positive_coefficient: i32,
    pub negative_coefficient: i32,
    pub positive_saturation: u32,
    pub negative_saturation: u32,
    pub dead_band: i32,
}

/// DIENVELOPE (times in µs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Envelope {
    pub attack_level: u32,
    pub attack_time: u32,
    pub fade_level: u32,
    pub fade_time: u32,
}

/// The type-specific block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Specific {
    /// One DICONDITION per axis (cbTypeSpecificParams = axes × 24).
    Condition([Condition; 2]),
    /// DICONSTANTFORCE.
    Constant(i32),
    /// DIPERIODIC (period in µs). The magnitude is the original's int
    /// passed as the DWORD it is (a negative jolt stays its bit pattern).
    Periodic { magnitude: u32, offset: i32, phase: u32, period: u32 },
}

/// One DIEFFECT (dwSize 0x38).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub kind: Kind,
    pub flags: u32,
    /// µs.
    pub duration: u32,
    pub sample_period: u32,
    pub gain: u32,
    pub trigger_button: u32,
    pub trigger_repeat_interval: u32,
    /// cAxes: the device's actuators (1 or 2).
    pub axes: u32,
    pub axis_offsets: [u32; 2],
    pub direction: [i32; 2],
    /// None: lpEnvelope NULL (the conditions).
    pub envelope: Option<Envelope>,
    pub specific: Specific,
    pub start_delay: u32,
}

impl Params {
    /// cbTypeSpecificParams.
    pub fn specific_size(&self) -> u32 {
        match self.specific {
            Specific::Condition(_) => 24 * self.axes,
            Specific::Constant(_) => 4,
            Specific::Periodic { .. } => 16,
        }
    }
}

fn wave_kind(w: Wave) -> Kind {
    match w {
        Wave::Sine => Kind::Sine,
        Wave::Square => Kind::Square,
        Wave::Triangle => Kind::Triangle,
        Wave::SawUp => Kind::SawUp,
        Wave::SawDown => Kind::SawDown,
    }
}

/// The GUID `e` is created with.
pub fn kind(e: &Effect) -> Kind {
    match *e {
        Effect::Spring { .. } => Kind::Spring,
        Effect::Damper { .. } | Effect::Puddle { .. } => Kind::Damper,
        Effect::Constant { .. } => Kind::Constant,
        Effect::Terrain { wave, .. } => wave_kind(wave),
        Effect::Jolt { .. } => Kind::Square,
    }
}

/// The DIEFFECT the original creates / updates `e` with on a device with
/// `actuators` FF axes (0x86FF80, 1 or 2).
pub fn params(e: &Effect, actuators: u8) -> Params {
    let axes = u32::from(actuators.clamp(1, 2));
    let cond = |axis: usize| {
        let (c, sat, dead) = super::condition(e, axis).unwrap_or_default();
        Condition {
            offset: 0,
            positive_coefficient: c,
            negative_coefficient: c,
            positive_saturation: sat as u32,
            negative_saturation: sat as u32,
            dead_band: dead,
        }
    };
    let periodic = |magnitude: i32, period_ms: i32| Specific::Periodic {
        magnitude: magnitude as u32,
        offset: 0,
        phase: 0,
        period: (period_ms as u32).wrapping_mul(1000),
    };
    let (duration, direction, envelope, specific) = match *e {
        Effect::Spring { .. } | Effect::Damper { .. } | Effect::Puddle { .. } => {
            (INFINITE, [1, 1], None, Specific::Condition([cond(0), cond(1)]))
        }
        Effect::Constant { level } => (INFINITE, [1, 0], Some(Envelope::default()), Specific::Constant(level)),
        Effect::Terrain { magnitude, period_ms, .. } => {
            (INFINITE, [1, 0], Some(Envelope::default()), periodic(magnitude, period_ms))
        }
        Effect::Jolt { magnitude } => (
            JOLT_MS as u32 * 1000,
            [1, 0],
            Some(Envelope { fade_time: JOLT_FADE_MS as u32 * 1000, ..Envelope::default() }),
            periodic(magnitude, JOLT_PERIOD_MS),
        ),
    };
    Params {
        kind: kind(e),
        flags: FLAGS,
        duration,
        sample_period: 0,
        gain: 10_000,
        trigger_button: NO_TRIGGER,
        trigger_repeat_interval: 0,
        axes,
        axis_offsets: AXES,
        direction,
        envelope,
        specific,
        start_delay: 0,
    }
}

/// One DirectInput call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// SetProperty(DIPROP_FFGAIN).
    Gain(u32),
    /// SetProperty(DIPROP_AUTOCENTER).
    Autocenter(bool),
    /// IDirectInputDevice8::Acquire; `required`: the next call is skipped
    /// when it fails (Start), else its result is not looked at.
    Acquire { required: bool },
    /// CreateEffect(GUID, DIEFFECT) into the slot.
    Create(Slot, Params),
    /// SetParameters(DIEFFECT, [`SET_FLAGS`]).
    Update(Slot, Params),
    /// Unload, and the slot is empty.
    Unload(Slot),
    /// Start(1, 0).
    Start(Slot),
    /// Stop.
    Stop(Slot),
}

/// The effect objects the device holds, by slot: which calls a [`Call`]
/// becomes.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    made: [Option<Kind>; 6],
}

impl Plan {
    /// Whether `slot` holds an effect, and of which GUID.
    pub fn made(&self, slot: Slot) -> Option<Kind> {
        self.made[slot as usize]
    }

    /// The calls for `c`, in order. Feed back each one's result with
    /// [`Plan::done`].
    pub fn ops(&self, c: Call, actuators: u8) -> Vec<Op> {
        match c {
            Call::Gain(g) => vec![Op::Gain(g as u32)],
            Call::Autocenter(on) => vec![Op::Autocenter(on)],
            Call::Command(Command::Set(e)) => {
                let s = e.slot();
                let p = params(&e, actuators);
                match self.made(s) {
                    None => vec![Op::Create(s, p)],
                    Some(k) if k == p.kind => vec![Op::Update(s, p)],
                    Some(_) => vec![Op::Acquire { required: false }, Op::Unload(s), Op::Create(s, p)],
                }
            }
            Call::Command(Command::Start(s)) if self.made(s).is_some() => {
                vec![Op::Acquire { required: true }, Op::Start(s)]
            }
            Call::Command(Command::Stop(s)) if self.made(s).is_some() => {
                vec![Op::Acquire { required: false }, Op::Stop(s)]
            }
            // No effect object: the original logs and does nothing.
            Call::Command(_) => Vec::new(),
        }
    }

    /// `op` returned `ok` (SUCCEEDED).
    pub fn done(&mut self, op: &Op, ok: bool) {
        match *op {
            Op::Create(s, p) if ok => self.made[s as usize] = Some(p.kind),
            // 0x5C0000 clears the slot only when Unload succeeds; a failed
            // Unload is followed by the create all the same (which then
            // replaces the object).
            Op::Unload(s) if ok => self.made[s as usize] = None,
            _ => {}
        }
    }
}

/// Runs `c`'s calls through `run` (the device, or a test's stand-in),
/// skipping a Start whose Acquire failed.
pub fn play(plan: &mut Plan, c: Call, actuators: u8, mut run: impl FnMut(&Op) -> bool) {
    let mut skip = false;
    for op in plan.ops(c, actuators) {
        if std::mem::take(&mut skip) {
            continue;
        }
        let ok = run(&op);
        plan.done(&op, ok);
        if let Op::Acquire { required: true } = op {
            skip = !ok;
        }
    }
}

#[cfg(windows)]
pub mod device {
    //! The calls on a DirectInput 8 device (the `windows` crate's
    //! bindings).

    use super::*;
    use windows::Win32::Devices::HumanInterfaceDevice as di;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM};
    use windows::core::{BOOL, GUID, Interface};

    /// DIDFT_OPTIONAL (dinput.h; not in the bindings).
    const DIDFT_OPTIONAL: u32 = 0x8000_0000;

    fn guid(k: Kind) -> GUID {
        match k {
            Kind::Spring => di::GUID_Spring,
            Kind::Damper => di::GUID_Damper,
            Kind::Constant => di::GUID_ConstantForce,
            Kind::Sine => di::GUID_Sine,
            Kind::Square => di::GUID_Square,
            Kind::Triangle => di::GUID_Triangle,
            Kind::SawUp => di::GUID_SawtoothUp,
            Kind::SawDown => di::GUID_SawtoothDown,
        }
    }

    /// A wheel or joystick with FF actuators, opened as 0x5CD200 does.
    pub struct Device {
        _di: di::IDirectInput8W,
        dev: di::IDirectInputDevice8W,
        effects: [Option<di::IDirectInputEffect>; 6],
        plan: Plan,
        /// The cooperative level is set (the game's window was found).
        coop: bool,
    }

    impl std::fmt::Debug for Device {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("dinput::Device").field("plan", &self.plan).field("coop", &self.coop).finish()
        }
    }

    impl Drop for Device {
        fn drop(&mut self) {
            for e in self.effects.iter().flatten() {
                // SAFETY: COM calls on objects we hold.
                unsafe {
                    let _ = e.Stop();
                    let _ = e.Unload();
                }
            }
            // SAFETY: as above.
            unsafe {
                let _ = self.dev.Unacquire();
            }
        }
    }

    struct Find {
        vid_pid: u32,
        found: Option<GUID>,
    }

    unsafe extern "system" fn each_device(d: *mut di::DIDEVICEINSTANCEW, ctx: *mut core::ffi::c_void) -> BOOL {
        // SAFETY: DirectInput passes a valid instance and our context.
        let (d, f) = unsafe { (&*d, &mut *(ctx as *mut Find)) };
        // guidProduct's Data1 is MAKELONG(vid, pid) for HID devices.
        if d.guidProduct.data1 == f.vid_pid {
            f.found = Some(d.guidInstance);
            return BOOL(0);
        }
        BOOL(di::DIENUM_CONTINUE as i32)
    }

    unsafe extern "system" fn each_actuator(_: *mut di::DIDEVICEOBJECTINSTANCEW, ctx: *mut core::ffi::c_void) -> BOOL {
        // SAFETY: our counter.
        let n = unsafe { &mut *(ctx as *mut u8) };
        *n = n.saturating_add(1);
        BOOL(di::DIENUM_CONTINUE as i32)
    }

    unsafe extern "system" fn each_window(w: HWND, ctx: LPARAM) -> BOOL {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};
        let mut pid = 0u32;
        // SAFETY: a window handle EnumWindows gave us; our out pointer.
        unsafe { GetWindowThreadProcessId(w, Some(&mut pid)) };
        // SAFETY: as above.
        if pid == std::process::id() && unsafe { IsWindowVisible(w) }.as_bool() {
            // SAFETY: ctx is our HWND slot.
            unsafe { *(ctx.0 as *mut HWND) = w };
            return BOOL(0);
        }
        BOOL(1)
    }

    /// The game's top-level window (the original passes its window,
    /// 0x870990, to SetCooperativeLevel).
    fn game_window() -> Option<HWND> {
        use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
        let mut w = HWND::default();
        // SAFETY: the callback only writes our local.
        let _ = unsafe { EnumWindows(Some(each_window), LPARAM(&mut w as *mut HWND as isize)) };
        (!w.is_invalid()).then_some(w)
    }

    impl Device {
        /// The attached FF game controller with this vendor / product id,
        /// with its actuator count (at most 2, as 0x5C8670 counts them);
        /// None when DirectInput has none (an XInput pad reports no
        /// actuators).
        pub fn open(vid: u16, pid: u16) -> Option<(Device, u8)> {
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            // SAFETY: plain Win32 / COM calls with valid arguments; every
            // pointer handed over lives across the call.
            unsafe {
                let hinst: HINSTANCE = GetModuleHandleW(None).ok()?.into();
                let mut raw = std::ptr::null_mut();
                di::DirectInput8Create(hinst, di::DIRECTINPUT_VERSION, &di::IDirectInput8W::IID, &mut raw, None)
                    .ok()?;
                let dinput = di::IDirectInput8W::from_raw(raw);
                let mut find = Find { vid_pid: u32::from(vid) | (u32::from(pid) << 16), found: None };
                dinput
                    .EnumDevices(
                        di::DI8DEVCLASS_GAMECTRL,
                        Some(each_device),
                        &mut find as *mut Find as *mut _,
                        di::DIEDFL_ATTACHEDONLY | di::DIEDFL_FORCEFEEDBACK,
                    )
                    .ok()?;
                let inst = find.found?;
                let mut dev = None;
                dinput.CreateDevice(&inst, &mut dev, None).ok()?;
                let dev = dev?;
                // The actuator axes at c_dfDIJoystick2's offsets: X at 0, Y at 4.
                let mut objs = [
                    di::DIOBJECTDATAFORMAT {
                        pguid: &di::GUID_XAxis,
                        dwOfs: AXES[0],
                        dwType: di::DIDFT_AXIS | di::DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                        dwFlags: 0,
                    },
                    di::DIOBJECTDATAFORMAT {
                        pguid: &di::GUID_YAxis,
                        dwOfs: AXES[1],
                        dwType: di::DIDFT_AXIS | di::DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                        dwFlags: 0,
                    },
                ];
                let mut fmt = di::DIDATAFORMAT {
                    dwSize: std::mem::size_of::<di::DIDATAFORMAT>() as u32,
                    dwObjSize: std::mem::size_of::<di::DIOBJECTDATAFORMAT>() as u32,
                    dwFlags: di::DIDF_ABSAXIS,
                    dwDataSize: 8,
                    dwNumObjs: objs.len() as u32,
                    rgodf: objs.as_mut_ptr(),
                };
                dev.SetDataFormat(&mut fmt).ok()?;
                let mut actuators = 0u8;
                let _ = dev.EnumObjects(
                    Some(each_actuator),
                    &mut actuators as *mut u8 as *mut _,
                    di::DIDFT_AXIS | di::DIDFT_FFACTUATOR,
                );
                let actuators = actuators.min(2);
                if actuators == 0 {
                    return None;
                }
                let mut d =
                    Device { _di: dinput, dev, effects: Default::default(), plan: Plan::default(), coop: false };
                d.cooperate();
                Some((d, actuators))
            }
        }

        /// SetCooperativeLevel(window, DISCL_EXCLUSIVE | DISCL_FOREGROUND)
        /// (5, observed), once the game's window exists.
        fn cooperate(&mut self) -> bool {
            if !self.coop
                && let Some(w) = game_window()
            {
                // SAFETY: a COM call with our own window.
                self.coop =
                    unsafe { self.dev.SetCooperativeLevel(w, di::DISCL_EXCLUSIVE | di::DISCL_FOREGROUND) }.is_ok();
            }
            self.coop
        }

        fn set_dword(&self, prop: &GUID, value: u32) -> bool {
            let mut p = di::DIPROPDWORD {
                diph: di::DIPROPHEADER {
                    dwSize: std::mem::size_of::<di::DIPROPDWORD>() as u32,
                    dwHeaderSize: std::mem::size_of::<di::DIPROPHEADER>() as u32,
                    dwObj: 0,
                    dwHow: di::DIPH_DEVICE,
                },
                dwData: value,
            };
            // SAFETY: a COM call with a DIPROPDWORD that lives across it.
            unsafe { self.dev.SetProperty(prop, &mut p.diph) }.is_ok()
        }

        fn run(&mut self, op: &Op) -> bool {
            // SAFETY (all arms): COM calls on our device and effects with
            // structures that live across each call.
            unsafe {
                match *op {
                    Op::Gain(g) => self.set_dword(&di::DIPROP_FFGAIN, g),
                    Op::Autocenter(on) => self.set_dword(&di::DIPROP_AUTOCENTER, u32::from(on)),
                    Op::Acquire { .. } => self.cooperate() && self.dev.Acquire().is_ok(),
                    Op::Create(s, p) => {
                        let mut buf = Buffers::new(&p);
                        let mut fx = buf.effect(&p);
                        let mut e = None;
                        let ok = self.dev.CreateEffect(&guid(p.kind), &mut fx, &mut e, None).is_ok();
                        if ok {
                            self.effects[s as usize] = e;
                        }
                        ok && self.effects[s as usize].is_some()
                    }
                    Op::Update(s, p) => {
                        let Some(e) = &self.effects[s as usize] else { return false };
                        let mut buf = Buffers::new(&p);
                        let mut fx = buf.effect(&p);
                        e.SetParameters(&mut fx, SET_FLAGS).is_ok()
                    }
                    Op::Unload(s) => {
                        let ok = self.effects[s as usize].as_ref().is_some_and(|e| e.Unload().is_ok());
                        if ok {
                            self.effects[s as usize] = None;
                        }
                        ok
                    }
                    Op::Start(s) => self.effects[s as usize].as_ref().is_some_and(|e| e.Start(1, 0).is_ok()),
                    Op::Stop(s) => self.effects[s as usize].as_ref().is_some_and(|e| e.Stop().is_ok()),
                }
            }
        }

        pub fn call(&mut self, c: Call, actuators: u8) {
            let mut plan = std::mem::take(&mut self.plan);
            play(&mut plan, c, actuators, |op| self.run(op));
            self.plan = plan;
        }
    }

    /// The blocks a DIEFFECT points to.
    struct Buffers {
        axes: [u32; 2],
        direction: [i32; 2],
        envelope: di::DIENVELOPE,
        conditions: [di::DICONDITION; 2],
        constant: di::DICONSTANTFORCE,
        periodic: di::DIPERIODIC,
    }

    impl Buffers {
        fn new(p: &Params) -> Buffers {
            let env = p.envelope.unwrap_or_default();
            let mut b = Buffers {
                axes: p.axis_offsets,
                direction: p.direction,
                envelope: di::DIENVELOPE {
                    dwSize: std::mem::size_of::<di::DIENVELOPE>() as u32,
                    dwAttackLevel: env.attack_level,
                    dwAttackTime: env.attack_time,
                    dwFadeLevel: env.fade_level,
                    dwFadeTime: env.fade_time,
                },
                conditions: Default::default(),
                constant: di::DICONSTANTFORCE::default(),
                periodic: di::DIPERIODIC::default(),
            };
            match p.specific {
                Specific::Condition(c) => {
                    for (d, c) in b.conditions.iter_mut().zip(c) {
                        *d = di::DICONDITION {
                            lOffset: c.offset,
                            lPositiveCoefficient: c.positive_coefficient,
                            lNegativeCoefficient: c.negative_coefficient,
                            dwPositiveSaturation: c.positive_saturation,
                            dwNegativeSaturation: c.negative_saturation,
                            lDeadBand: c.dead_band,
                        };
                    }
                }
                Specific::Constant(m) => b.constant.lMagnitude = m,
                Specific::Periodic { magnitude, offset, phase, period } => {
                    b.periodic =
                        di::DIPERIODIC { dwMagnitude: magnitude, lOffset: offset, dwPhase: phase, dwPeriod: period };
                }
            }
            b
        }

        /// The DIEFFECT, pointing into `self` (which must outlive its use).
        fn effect(&mut self, p: &Params) -> di::DIEFFECT {
            let specific: *mut core::ffi::c_void = match p.specific {
                Specific::Condition(_) => self.conditions.as_mut_ptr().cast(),
                Specific::Constant(_) => (&mut self.constant as *mut di::DICONSTANTFORCE).cast(),
                Specific::Periodic { .. } => (&mut self.periodic as *mut di::DIPERIODIC).cast(),
            };
            di::DIEFFECT {
                dwSize: std::mem::size_of::<di::DIEFFECT>() as u32,
                dwFlags: p.flags,
                dwDuration: p.duration,
                dwSamplePeriod: p.sample_period,
                dwGain: p.gain,
                dwTriggerButton: p.trigger_button,
                dwTriggerRepeatInterval: p.trigger_repeat_interval,
                cAxes: p.axes,
                rgdwAxes: self.axes.as_mut_ptr(),
                rglDirection: self.direction.as_mut_ptr(),
                lpEnvelope: if p.envelope.is_some() { &mut self.envelope } else { std::ptr::null_mut() },
                cbTypeSpecificParams: p.specific_size(),
                lpvTypeSpecificParams: specific,
                dwStartDelay: p.start_delay,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::DevType;
    use crate::pad::ff::{Clock, Device, Feed, Options, Original};

    #[test]
    fn conditions_as_0x5c04f0() {
        let p = params(&Effect::Spring { coef: 7_000 }, 2);
        assert_eq!(
            (p.kind, p.flags, p.duration, p.gain, p.trigger_button),
            (Kind::Spring, 0x12, INFINITE, 10_000, NO_TRIGGER)
        );
        assert_eq!((p.axes, p.axis_offsets, p.direction, p.envelope, p.start_delay), (2, [0, 4], [1, 1], None, 0));
        let Specific::Condition([x, y]) = p.specific else { panic!() };
        assert_eq!(
            x,
            Condition {
                offset: 0,
                positive_coefficient: 7_000,
                negative_coefficient: 7_000,
                positive_saturation: 7_000,
                negative_saturation: 7_000,
                dead_band: 0
            }
        );
        assert_eq!(
            y,
            Condition {
                offset: 0,
                positive_coefficient: 8_000,
                negative_coefficient: 8_000,
                positive_saturation: 8_000,
                negative_saturation: 8_000,
                dead_band: 500
            }
        );
        assert_eq!(p.specific_size(), 48);
        let d = params(&Effect::Damper { coef: 7_058 }, 1);
        assert_eq!((d.kind, d.axes, d.specific_size()), (Kind::Damper, 1, 24));
        let Specific::Condition([x, _]) = d.specific else { panic!() };
        assert_eq!((x.positive_coefficient, x.positive_saturation), (7_058, 10_000));
        let w = params(&Effect::Puddle { coef: 10_000 }, 1);
        let Specific::Condition([x, _]) = w.specific else { panic!() };
        assert_eq!((w.kind, x.negative_coefficient, x.negative_saturation), (Kind::Damper, -10_000, 10_000));
    }

    #[test]
    fn constant_and_periodic_as_0x5c0210_0x5c8d30() {
        let c = params(&Effect::Constant { level: -4_995 }, 1);
        assert_eq!(
            (c.kind, c.duration, c.direction, c.envelope),
            (Kind::Constant, INFINITE, [1, 0], Some(Envelope::default()))
        );
        assert_eq!((c.specific, c.specific_size()), (Specific::Constant(-4_995), 4));
        let t = params(&Effect::Terrain { wave: Wave::Triangle, magnitude: 1_568, period_ms: 50 }, 1);
        assert_eq!((t.kind, t.duration, t.envelope), (Kind::Triangle, INFINITE, Some(Envelope::default())));
        assert_eq!(t.specific, Specific::Periodic { magnitude: 1_568, offset: 0, phase: 0, period: 50_000 });
        assert_eq!(t.specific_size(), 16);
        let j = params(&Effect::Jolt { magnitude: 10_000 }, 2);
        assert_eq!((j.kind, j.duration, j.direction), (Kind::Square, 150_000, [1, 0]));
        assert_eq!(j.envelope, Some(Envelope { fade_time: 20_000, ..Envelope::default() }));
        assert_eq!(j.specific, Specific::Periodic { magnitude: 10_000, offset: 0, phase: 0, period: 75_000 });
        // A rise in speed gives the original a negative DWORD magnitude.
        let r = params(&Effect::Jolt { magnitude: -12_000 }, 1);
        assert_eq!(
            r.specific,
            Specific::Periodic { magnitude: (-12_000i32) as u32, offset: 0, phase: 0, period: 75_000 }
        );
    }

    #[test]
    fn create_update_recreate_start_stop() {
        use crate::pad::ff::Command::*;
        let mut plan = Plan::default();
        let log = std::cell::RefCell::new(Vec::new());
        let go = |plan: &mut Plan, c: Call, acquire_ok: bool| {
            play(plan, c, 1, |op| {
                log.borrow_mut().push(*op);
                !matches!(op, Op::Acquire { .. }) || acquire_ok
            })
        };
        // No effect yet: Start and Stop do nothing.
        go(&mut plan, Call::Command(Start(Slot::Terrain)), true);
        assert!(log.borrow().is_empty());
        let sine = Effect::Terrain { wave: Wave::Sine, magnitude: 100, period_ms: 40 };
        let square = Effect::Terrain { wave: Wave::Square, magnitude: 100, period_ms: 40 };
        let sine90 = Effect::Terrain { wave: Wave::Sine, magnitude: 90, period_ms: 40 };
        go(&mut plan, Call::Command(Set(sine)), true);
        go(&mut plan, Call::Command(Set(sine90)), true);
        go(&mut plan, Call::Command(Set(square)), true);
        go(&mut plan, Call::Command(Start(Slot::Terrain)), true);
        go(&mut plan, Call::Command(Start(Slot::Terrain)), false);
        go(&mut plan, Call::Command(Stop(Slot::Terrain)), false);
        go(&mut plan, Call::Gain(7_600), true);
        let p = |e: Effect| params(&e, 1);
        assert_eq!(
            *log.borrow(),
            vec![
                Op::Create(Slot::Terrain, p(sine)),
                Op::Update(Slot::Terrain, p(sine90)),
                Op::Acquire { required: false },
                Op::Unload(Slot::Terrain),
                Op::Create(Slot::Terrain, p(square)),
                Op::Acquire { required: true },
                Op::Start(Slot::Terrain),
                // Acquire failed: no Start (0x5BFEF0).
                Op::Acquire { required: true },
                // Stop goes on regardless (0x5BFF70).
                Op::Acquire { required: false },
                Op::Stop(Slot::Terrain),
                Op::Gain(7_600),
            ]
        );
        assert_eq!(plan.made(Slot::Terrain), Some(Kind::Square));
        // A failed create leaves the slot empty: the next Set creates again.
        let mut plan = Plan::default();
        play(&mut plan, Call::Command(Set(sine)), 1, |_| false);
        assert_eq!(plan.ops(Call::Command(Set(sine)), 1), vec![Op::Create(Slot::Terrain, p(sine))]);
    }

    /// The model's commands over a drive turn into DirectInput calls: every
    /// effect is created once, then updated, every Start has its Acquire.
    #[test]
    fn a_drive_through_the_model() {
        let mut m = Original::new(Device { class: DevType::Driving, actuators: 2 }).unwrap();
        let mut plan = Plan::default();
        let mut ops = Vec::new();
        let mut calls = vec![Call::Autocenter(false)];
        calls.extend(m.front_end().into_iter().map(Call::Command));
        calls.push(Call::Gain(10_000));
        for t in 0..240u32 {
            let feed = Feed {
                steer: if t == 100 { 1.0 } else { 0.3 },
                speed: 5.0 + t as f32 * 0.2,
                front_sideways: 2.0,
                bump: if t == 50 { [5.0, 0.0, 0.0, 0.0] } else { [0.0; 4] },
                front_down: [t != 120, t != 120],
                front_surface: if t < 150 { [1, 16] } else { [8, 8] },
                drag: false,
            };
            let clock = Clock { sim: t * 67, real_ms: u64::from(t) * 16 };
            calls.extend(m.driving(&feed, &Options::default(), clock).into_iter().map(Call::Command));
        }
        for c in calls {
            play(&mut plan, c, 2, |op| {
                ops.push(*op);
                true
            });
        }
        assert_eq!(ops[0], Op::Autocenter(false));
        let creates: Vec<Slot> =
            ops.iter().filter_map(|o| if let Op::Create(s, _) = o { Some(*s) } else { None }).collect();
        for s in [Slot::Spring, Slot::Damper, Slot::Constant, Slot::Terrain] {
            assert!(creates.contains(&s), "{s:?} created: {creates:?}");
        }
        // The surface change from SIDEWALK (square) to DIRT (sine) re-creates the terrain effect.
        assert_eq!(creates.iter().filter(|&&s| s == Slot::Terrain).count(), 2);
        assert!(ops.contains(&Op::Unload(Slot::Terrain)));
        for (i, o) in ops.iter().enumerate() {
            if let Op::Start(_) = o {
                assert_eq!(ops[i - 1], Op::Acquire { required: true });
            }
            if let Op::Create(_, p) | Op::Update(_, p) = o {
                assert_eq!((p.flags, p.axes, p.gain), (FLAGS, 2, 10_000));
            }
        }
        assert!(ops.contains(&Op::Gain(10_000)));
    }
}
