//! Playing [`super::ff::Command`]s on a device: the DirectInput 8 calls
//! the original makes (docs/formats/front-end.md, **Force feedback**)
//! turned into the host's force-feedback API.
//!
//! - **Linux**: the evdev force-feedback interface of the device node
//!   gilrs reads (EVIOCSFF uploads, EV_FF writes to play and stop, FF_GAIN,
//!   FF_AUTOCENTER), the API Wine's DirectInput maps the same effects to.
//! - **Windows**: DirectInput 8 on the game controller with the gamepad's
//!   vendor / product id, the calls and DIEFFECT parameters the original
//!   makes ([`dinput`]).
//! - **Recorder**: every command with its world tick, for the tests and
//!   the scripted pads (`connect … wheel ff`).
//!
//! Units: DirectInput's ±10000 levels and coefficients become evdev's
//! ±0x7FFF, saturations 0..10000 become 0..0xFFFF, the gain 0..10000
//! becomes 0..0xFFFF. A DirectInput direction along +X (DIEFF_CARTESIAN,
//! one axis) is evdev direction 0x4000, as SDL and Wine map it.

use super::ff::{Command, Effect};

pub mod dinput;

/// One recorded call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// DIPROP_FFGAIN (0..10000).
    Gain(i32),
    /// DIPROP_AUTOCENTER on / off.
    Autocenter(bool),
    Command(Command),
}

/// Where the commands go.
#[derive(Debug)]
pub enum Output {
    /// Kept in order with the world tick they were made on.
    Recorder(Vec<(i64, Call)>),
    #[cfg(target_os = "linux")]
    Evdev(evdev::Device),
    #[cfg(windows)]
    DirectInput(dinput::device::Device),
}

impl Output {
    pub fn recorder() -> Output {
        Output::Recorder(Vec::new())
    }

    /// The device node's force-feedback output with its actuator count
    /// (1: evdev does not say which axes take forces, so the steering
    /// axis alone, as a wheel reports it; **inferred**), or None when the
    /// node has no condition, constant or periodic effect (a rumble-only
    /// pad keeps the Rumble extra's path).
    #[cfg(target_os = "linux")]
    pub fn open(path: &std::path::Path) -> Option<(Output, u8)> {
        evdev::Device::open(path).map(|d| (Output::Evdev(d), 1))
    }

    /// The DirectInput game controller with this vendor / product id and
    /// FF actuators (at most 2, 0x5C8670), or None (no such device, or an
    /// XInput pad, which reports none: the Rumble extra's path stays).
    #[cfg(windows)]
    pub fn open_dinput(vid: u16, pid: u16) -> Option<(Output, u8)> {
        dinput::device::Device::open(vid, pid).map(|(d, n)| (Output::DirectInput(d), n))
    }

    pub fn calls(&self) -> &[(i64, Call)] {
        match self {
            Output::Recorder(v) => v,
            #[cfg(target_os = "linux")]
            Output::Evdev(_) => &[],
            #[cfg(windows)]
            Output::DirectInput(_) => &[],
        }
    }

    pub fn call(&mut self, tick: i64, c: Call, actuators: u8) {
        match self {
            Output::Recorder(v) => v.push((tick, c)),
            #[cfg(target_os = "linux")]
            Output::Evdev(d) => d.call(c, actuators),
            #[cfg(windows)]
            Output::DirectInput(d) => d.call(c, actuators),
        }
    }
}

/// The evdev magnitude of a DirectInput level.
pub fn level(v: i32) -> i16 {
    (i64::from(v.clamp(-10_000, 10_000)) * 0x7FFF / 10_000) as i16
}

/// The evdev saturation / gain of a DirectInput 0..10000 value.
pub fn unsigned(v: i32) -> u16 {
    (i64::from(v.clamp(0, 10_000)) * 0xFFFF / 10_000) as u16
}

/// An effect's condition (spring / damper) per axis: (coefficient,
/// saturation, deadband) in DirectInput units; the second axis the
/// observed {8000, 8000, 500}.
pub fn condition(e: &Effect, axis: usize) -> Option<(i32, i32, i32)> {
    let first = match *e {
        Effect::Spring { coef } => (coef, coef, 0),
        Effect::Damper { coef } => (coef, 10_000, 0),
        Effect::Puddle { coef } => (-coef, 10_000, 0),
        _ => return None,
    };
    Some(if axis == 0 { first } else { (8_000, 8_000, 500) })
}

#[cfg(target_os = "linux")]
pub mod evdev {
    use super::*;
    use crate::pad::ff::Wave;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    const EV_FF: u16 = 0x15;
    const FF_PERIODIC: u16 = 0x51;
    const FF_CONSTANT: u16 = 0x52;
    const FF_SPRING: u16 = 0x53;
    const FF_DAMPER: u16 = 0x55;
    const FF_SQUARE: u16 = 0x58;
    const FF_TRIANGLE: u16 = 0x59;
    const FF_SINE: u16 = 0x5A;
    const FF_SAW_UP: u16 = 0x5B;
    const FF_SAW_DOWN: u16 = 0x5C;
    const FF_GAIN: u16 = 0x60;
    const FF_AUTOCENTER: u16 = 0x61;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Envelope {
        attack_length: u16,
        attack_level: u16,
        fade_length: u16,
        fade_level: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Condition {
        right_saturation: u16,
        left_saturation: u16,
        right_coeff: i16,
        left_coeff: i16,
        deadband: u16,
        center: i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Periodic {
        waveform: u16,
        period: u16,
        magnitude: i16,
        offset: i16,
        phase: u16,
        envelope: Envelope,
        custom_len: u32,
        custom_data: *mut i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Constant {
        level: i16,
        envelope: Envelope,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union Params {
        constant: Constant,
        periodic: Periodic,
        condition: [Condition; 2],
    }

    /// `struct ff_effect` (linux/input.h).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FfEffect {
        kind: u16,
        id: i16,
        direction: u16,
        trigger_button: u16,
        trigger_interval: u16,
        replay_length: u16,
        replay_delay: u16,
        u: Params,
    }

    #[repr(C)]
    struct InputEvent {
        time: libc::timeval,
        kind: u16,
        code: u16,
        value: i32,
    }

    const fn ioc(dir: u64, nr: u64, size: usize) -> u64 {
        (dir << 30) | ((size as u64) << 16) | ((b'E' as u64) << 8) | nr
    }
    const EVIOCSFF: u64 = ioc(1, 0x80, std::mem::size_of::<FfEffect>());
    const EVIOCRMFF: u64 = ioc(1, 0x81, std::mem::size_of::<libc::c_int>());

    fn bit(bits: &[u8], n: u16) -> bool {
        bits.get(usize::from(n / 8)).is_some_and(|b| b & (1 << (n % 8)) != 0)
    }

    /// A device node opened for force feedback, its effect ids by slot.
    #[derive(Debug)]
    pub struct Device {
        file: File,
        ids: [i16; 6],
        /// The wave each periodic id was uploaded with.
        waves: [Option<Wave>; 6],
        bits: [u8; 16],
    }

    impl Drop for Device {
        fn drop(&mut self) {
            for id in self.ids {
                if id >= 0 {
                    // SAFETY: an ioctl on our own open fd with an int arg.
                    unsafe { libc::ioctl(self.file.as_raw_fd(), EVIOCRMFF as _, libc::c_int::from(id)) };
                }
            }
        }
    }

    impl Device {
        pub fn open(path: &std::path::Path) -> Option<Device> {
            let file = std::fs::OpenOptions::new().read(true).write(true).open(path).ok()?;
            let mut bits = [0u8; 16];
            // EVIOCGBIT(EV_FF, 16).
            let req = ioc(2, 0x20 + u64::from(EV_FF), bits.len());
            // SAFETY: the buffer is 16 bytes, as the request says.
            let r = unsafe { libc::ioctl(file.as_raw_fd(), req as _, bits.as_mut_ptr()) };
            if r < 0 || ![FF_CONSTANT, FF_SPRING, FF_DAMPER, FF_PERIODIC].iter().any(|&b| bit(&bits, b)) {
                return None;
            }
            Some(Device { file, ids: [-1; 6], waves: [None; 6], bits })
        }

        fn write(&mut self, code: u16, value: i32) {
            use std::io::Write;
            let ev = InputEvent { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, kind: EV_FF, code, value };
            // SAFETY: InputEvent is repr(C) plain data.
            let bytes = unsafe {
                std::slice::from_raw_parts((&ev as *const InputEvent).cast::<u8>(), std::mem::size_of::<InputEvent>())
            };
            let _ = self.file.write_all(bytes);
        }

        fn upload(&mut self, e: &Effect, actuators: u8) {
            let s = e.slot() as usize;
            let zero = Envelope::default();
            let cond = |axis: usize| {
                let (c, sat, dead) = condition(e, axis).unwrap_or_default();
                let used = axis < usize::from(actuators.max(1));
                if !used {
                    return Condition::default();
                }
                Condition {
                    right_saturation: unsigned(sat),
                    left_saturation: unsigned(sat),
                    right_coeff: level(c),
                    left_coeff: level(c),
                    deadband: unsigned(dead),
                    center: 0,
                }
            };
            let (kind, length, u) = match *e {
                Effect::Spring { .. } => (FF_SPRING, 0, Params { condition: [cond(0), cond(1)] }),
                Effect::Damper { .. } | Effect::Puddle { .. } => {
                    (FF_DAMPER, 0, Params { condition: [cond(0), cond(1)] })
                }
                Effect::Constant { level: l } => {
                    (FF_CONSTANT, 0, Params { constant: Constant { level: level(l), envelope: zero } })
                }
                Effect::Terrain { wave, magnitude, period_ms } => {
                    let waveform = match wave {
                        Wave::Sine => FF_SINE,
                        Wave::Square => FF_SQUARE,
                        Wave::Triangle => FF_TRIANGLE,
                        Wave::SawUp => FF_SAW_UP,
                        Wave::SawDown => FF_SAW_DOWN,
                    };
                    (
                        FF_PERIODIC,
                        0,
                        Params {
                            periodic: Periodic {
                                waveform,
                                period: period_ms.max(0) as u16,
                                magnitude: level(magnitude),
                                offset: 0,
                                phase: 0,
                                envelope: zero,
                                custom_len: 0,
                                custom_data: std::ptr::null_mut(),
                            },
                        },
                    )
                }
                Effect::Jolt { magnitude } => (
                    FF_PERIODIC,
                    super::super::ff::JOLT_MS as u16,
                    Params {
                        periodic: Periodic {
                            waveform: FF_SQUARE,
                            period: super::super::ff::JOLT_PERIOD_MS as u16,
                            magnitude: level(magnitude),
                            offset: 0,
                            phase: 0,
                            envelope: Envelope { fade_length: super::super::ff::JOLT_FADE_MS as u16, ..zero },
                            custom_len: 0,
                            custom_data: std::ptr::null_mut(),
                        },
                    },
                ),
            };
            let needed = if kind == FF_PERIODIC { FF_PERIODIC } else { kind };
            if !bit(&self.bits, needed) {
                return;
            }
            // A periodic effect whose wave changes is released and made again (0x5C9FE0).
            let wave = match *e {
                Effect::Terrain { wave, .. } => Some(wave),
                _ => None,
            };
            if wave.is_some() && self.waves[s] != wave && self.ids[s] >= 0 {
                // SAFETY: as in Drop.
                unsafe { libc::ioctl(self.file.as_raw_fd(), EVIOCRMFF as _, libc::c_int::from(self.ids[s])) };
                self.ids[s] = -1;
            }
            let mut fx = FfEffect {
                kind,
                id: self.ids[s],
                direction: 0x4000,
                trigger_button: 0,
                trigger_interval: 0,
                replay_length: length,
                replay_delay: 0,
                u,
            };
            // SAFETY: FfEffect is struct ff_effect's layout; the kernel writes the id back.
            let r = unsafe { libc::ioctl(self.file.as_raw_fd(), EVIOCSFF as _, &mut fx as *mut FfEffect) };
            if r >= 0 {
                self.ids[s] = fx.id;
                self.waves[s] = wave;
            }
        }

        pub fn call(&mut self, c: Call, actuators: u8) {
            match c {
                Call::Gain(g) => {
                    if bit(&self.bits, FF_GAIN) {
                        self.write(FF_GAIN, i32::from(unsigned(g)));
                    }
                }
                Call::Autocenter(on) => {
                    if bit(&self.bits, FF_AUTOCENTER) {
                        self.write(FF_AUTOCENTER, if on { 0xFFFF } else { 0 });
                    }
                }
                Call::Command(Command::Set(e)) => self.upload(&e, actuators),
                Call::Command(Command::Start(s)) => {
                    let id = self.ids[s as usize];
                    if id >= 0 {
                        self.write(id as u16, 1);
                    }
                }
                Call::Command(Command::Stop(s)) => {
                    let id = self.ids[s as usize];
                    if id >= 0 {
                        self.write(id as u16, 0);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Plays one of each effect on the device node `NFSU2_FF_NODE` (a
        /// virtual force-feedback wheel made with uinput, which logs what
        /// it receives; docs/playing/controller-support.md).
        #[test]
        #[ignore = "needs NFSU2_FF_NODE, a force-feedback event node"]
        fn plays_on_a_device_node() {
            use crate::pad::ff::{Command::*, Effect::*, Slot};
            let node = std::env::var("NFSU2_FF_NODE").expect("NFSU2_FF_NODE");
            let mut d = Device::open(std::path::Path::new(&node)).expect("an FF node");
            let calls = [
                Call::Autocenter(false),
                Call::Gain(7_600),
                Call::Command(Set(Spring { coef: 7_000 })),
                Call::Command(Start(Slot::Spring)),
                Call::Command(Set(Damper { coef: 7_058 })),
                Call::Command(Start(Slot::Damper)),
                Call::Command(Set(Constant { level: -4_995 })),
                Call::Command(Start(Slot::Constant)),
                Call::Command(Set(Terrain { wave: Wave::Sine, magnitude: 1_568, period_ms: 50 })),
                Call::Command(Start(Slot::Terrain)),
                Call::Command(Set(Terrain { wave: Wave::Square, magnitude: 1_000, period_ms: 45 })),
                Call::Command(Start(Slot::Terrain)),
                Call::Command(Set(Jolt { magnitude: 10_000 })),
                Call::Command(Start(Slot::Jolt)),
                Call::Command(Set(Puddle { coef: 10_000 })),
                Call::Command(Start(Slot::Puddle)),
                Call::Command(Stop(Slot::Spring)),
            ];
            for c in calls {
                d.call(c, 1);
            }
            assert!(d.ids.iter().all(|&id| id >= 0), "{:?}", d.ids);
        }

        #[test]
        fn ff_effect_is_the_kernels_size() {
            // struct ff_effect on 64-bit Linux: 48 bytes (EVIOCSFF 0x40304580).
            #[cfg(target_pointer_width = "64")]
            assert_eq!(EVIOCSFF, 0x4030_4580);
            assert_eq!(EVIOCRMFF, 0x4004_4581);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units() {
        assert_eq!(level(10_000), 0x7FFF);
        assert_eq!(level(-10_000), -0x7FFF);
        assert_eq!(level(-9_990), -0x7FDE);
        assert_eq!(unsigned(10_000), 0xFFFF);
        assert_eq!(unsigned(0), 0);
        assert_eq!(condition(&Effect::Puddle { coef: 10_000 }, 0), Some((-10_000, 10_000, 0)));
        assert_eq!(condition(&Effect::Spring { coef: 7_000 }, 1), Some((8_000, 8_000, 500)));
    }
}
