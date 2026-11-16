use super::{EnvInput, EnvSfx, ramp};
use crate::audio::carfx::Sink;
use crate::audio::stitch::{StitchBanks, StitchVoice};

#[derive(Default)]
pub(crate) struct Bottom {
    was: [bool; 4],
    timer: [f32; 4],
    tipped: bool,
    pub(super) knock: Option<StitchVoice>,
    /// The second knock (+0xB8) and its counter (0x82CBF0).
    pub(super) knock2: Option<StitchVoice>,
    counter2: u32,
    pub(super) counter: [u32; 2],
    pub(super) started: bool,
}

impl Bottom {
    /// The release (vtable slot 12, 0x462700): the knock stitches go
    /// (+0xB4, +0xB0, +0xB8).
    pub(crate) fn stop(&mut self, sink: &mut dyn Sink) {
        for k in [&mut self.knock, &mut self.knock2] {
            if let Some(mut v) = k.take() {
                v.stop(sink);
            }
        }
    }
}

impl EnvSfx {
    pub(super) fn bottom_update(&mut self, sink: &mut dyn Sink, i: &EnvInput, dt: f32) {
        // The knock's volume: the four wheels' compressions through the ramp
        // 0x7F0B5C × 31.75 for the physics mover (car +0x28 = 1).
        let v: f32 = i.compression.iter().map(|c| ramp(0.4, 1.0, 0.4, 1.0, c.abs())).sum::<f32>() * 31.75;
        let levels = [(i.mix.level(11, 0), i.mix.az(11, 0)), (i.mix.level(11, 2), i.mix.az(11, 2))];
        self.knocks += bottom_out(
            &mut self.bottom,
            self.stitches.as_mut(),
            sink,
            BottomIn { down: i.down, up_z: i.up_z, volume: v, event_kind: i.event_kind, levels },
            self.clock,
            dt,
        );
    }
}

/// One car's bottom-out inputs for [`bottom_out`].
pub struct BottomIn {
    /// The wheels on the ground (car +0x390..+0x393).
    pub down: [bool; 4],
    /// The up vector's z (car +0xB8).
    pub up_z: f32,
    /// The knock volume before the 0..127 clamp: the compression sum ×
    /// 31.75 when car +0x28 (the mover) is 1, else 127 (0x47EFB3).
    pub volume: f32,
    /// The nearest camera director's event kind (0x459AC0 +0x58).
    pub event_kind: u8,
    /// Mix-map (level, azimuth) of the object's slots 0 and 2.
    pub levels: [(i32, i32); 2],
}

/// CARSFX_BottomOut's update 0x47ED80 and per-frame 0x47F100 (also
/// CARSFX_TrafficBottomOut's: 0x47E980 builds it on BottomOut's
/// constructor 0x475F90 with vtable 0x78BC30, keeping both): returns the
/// knocks started.
pub(crate) fn bottom_out(
    bo: &mut Bottom,
    mut stitches: Option<&mut StitchBanks>,
    sink: &mut dyn Sink,
    i: BottomIn,
    clock: f32,
    dt: f32,
) -> usize {
    let mut knocks = 0;
    let d = i.down;
    let pairs = [d[0] && d[1], d[2] && d[3], d[1] && d[2], d[0] && d[3]];
    let all = d.iter().all(|&x| x);
    bo.started = false;
    if !bo.tipped && i.up_z < 0.9 && !all {
        bo.tipped = true;
    }
    let land = (0..2).any(|k| pairs[k] && bo.timer[k] > 0.05 && !bo.was[k]);
    let flag = if land && !bo.tipped {
        Some(false)
    } else if bo.tipped && (0..4).any(|k| pairs[k] && !bo.was[k]) {
        Some(true)
    } else {
        None
    };
    if let Some(flag) = flag {
        let v = i.volume.clamp(0.0, 127.0);
        let k = if flag { 1 } else { 0 };
        let n = bo.counter[k] & 3;
        bo.counter[k] = n + 1;
        let index = if flag {
            0x56 + n as i32 - 4 * (v * -0.0078125) as i32
        } else {
            0x4E + n as i32 - 4 * (v * -0.015625) as i32
        };
        if let Some(mut old) = bo.knock.take() {
            old.stop(sink);
        }
        if let Some(s) = stitches.as_deref_mut() {
            bo.knock = StitchVoice::start(s, 0, index as usize);
            knocks += bo.knock.is_some() as usize;
        }
        // Under the crash / jump camera (the nearest director's kind 6,
        // 0x47EB67) a second knock (+0xB8): record 0x2E + (counter &
        // 3) − 4 · trunc(Rand_Float(127) · −1/64).
        if i.event_kind == 6 {
            if let Some(mut old) = bo.knock2.take() {
                old.stop(sink);
            }
            let r = crate::audio::collision::rand_float(127.0);
            let n = bo.counter2 & 3;
            bo.counter2 = n + 1;
            let index = 0x2E + n as i32 - 4 * (r * -0.015625) as i32;
            if let Some(s) = stitches.as_deref_mut() {
                bo.knock2 = StitchVoice::start(s, 0, index as usize);
                knocks += bo.knock2.is_some() as usize;
            }
        }
        bo.started = true;
    }
    if all && bo.tipped {
        bo.tipped = false;
    }
    for k in 0..4 {
        if pairs[k] {
            bo.was[k] = true;
        } else {
            if bo.was[k] {
                bo.was[k] = false;
                bo.timer[k] = clock;
            }
            bo.timer[k] += dt;
        }
    }
    if let Some(knock) = bo.knock.as_mut() {
        if !knock.update(sink, i.levels[0].0, i.levels[0].1, dt) {
            bo.knock = None;
        }
    }
    if let Some(knock) = bo.knock2.as_mut() {
        if !knock.update(sink, i.levels[1].0, i.levels[1].1, dt) {
            bo.knock2 = None;
        }
    }
    knocks
}
