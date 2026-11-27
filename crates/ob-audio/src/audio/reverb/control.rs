use crate::audio::mixmap::db_to_lin;
use crate::audio::util::interp::Interp;

/// 0x7F3778: per preset {wet A, dry A, wet B, dry B} (mB), the fade-out
/// and the fade-in (ms).
pub const LEVELS: [([i32; 4], i32, i32); 12] = [
    ([-1800, 0, -1800, 0], 140, 60),
    ([-1800, 0, -1800, 0], 140, 60),
    ([-1800, 0, -1800, 0], 140, 60),
    ([-500, 0, -500, 0], 140, 60),
    ([-500, 0, -500, 0], 140, 60),
    ([0, 0, 0, 0], 140, 60),
    ([-300, 0, -300, 0], 120, 60),
    ([-1800, 0, -1800, 0], 140, 60),
    ([-1600, 0, -1600, 0], 140, 60),
    ([-1600, 0, -1600, 0], 140, 60),
    ([-1200, 0, -1200, 0], 140, 120),
    ([-900, 0, -900, 0], 140, 80),
];

/// SFXObj_Reverb's start (0x4632C0): the preset the front end loads.
pub fn front_end_preset(car_lot_or_show: bool) -> usize {
    if car_lot_or_show { 0 } else { 3 }
}

/// SFXCTL_Tunnel's zone (+0x78) → preset (0x483441): kind 5 → 5, 6–7 → 6,
/// else 3.
pub fn zone_preset(kind: u32) -> usize {
    match kind {
        5 => 5,
        6 | 7 => 6,
        _ => 3,
    }
}

/// The reverb controller: SFXCTL_Tunnel's +0xB0..+0xD7 and the globals of
/// 0x45DB90 / 0x4713F0 / 0x4714A0 / 0x47C6D0.
#[derive(Clone, Debug)]
pub struct ReverbCtl {
    /// +0xD7: the tunnel object is the player's (0x4832E3).
    pub enabled: bool,
    /// 0x82B060 (the loaded preset) and 0x82AF2C (the asked one).
    pub current: usize,
    requested: usize,
    /// 0x82A99C: the level offset (mB).
    offset: f32,
    /// +0xB8.
    interp: Interp,
    /// +0xD4 fading out, +0xD5 fading in, +0xAC (the fade not done at the
    /// end of the last frame).
    fading_out: bool,
    fading_in: bool,
    busy: bool,
    /// +0xD6: reload the graph with `current` this frame (0x4633F0).
    pub reload: bool,
    /// {wet A, dry A, wet B, dry B}: the current levels (0x82A5B8,
    /// 0x82B618, 0x82A5CC, 0x82A5D0) and the targets (0x82B064, 0x82AFAC,
    /// 0x82AF30, 0x82B074), 0..1.
    cur: [f32; 4],
    target: [f32; 4],
    /// The outputs 0x82B078, 0x82A998, 0x82B070, 0x82B880 (0..0x7FFF).
    pub out: [i32; 4],
    /// 0x82AF28: the alternate frames' +1.
    nudge: bool,
    /// +0xB4 / +0xB0: the region this frame and the last; +0x70 last frame.
    region: i32,
    last_region: i32,
    inside: bool,
}

impl Default for ReverbCtl {
    /// 0x45D990's start values.
    fn default() -> ReverbCtl {
        ReverbCtl {
            enabled: true,
            current: 5,
            requested: 5,
            offset: 0.0,
            interp: Interp::at(0.0),
            fading_out: false,
            fading_in: false,
            busy: false,
            reload: false,
            cur: [0.0, 1.0, 0.0, 1.0],
            target: [0.0, 1.0, 0.0, 1.0],
            out: [0, 0x7F, 0, 0],
            nudge: false,
            region: -1,
            last_region: 0,
            inside: false,
        }
    }
}

fn level(mb: i32) -> f32 {
    (db_to_lin(mb) as f64 * (1.0f32 / 32767.0) as f64) as f32
}

impl ReverbCtl {
    /// 0x45DB90(preset, offset): fade the wet out over the current preset's
    /// fade-out, then switch.
    pub fn request(&mut self, preset: usize, offset: i32) {
        if !self.enabled {
            return;
        }
        self.offset = offset as f32;
        self.requested = preset.min(11);
        self.fading_out = true;
        let ms = LEVELS[self.current].1;
        self.interp.set(0.0, 1.0, if ms > 0 { ms } else { 10 }, 0);
        self.target = [0.0, 1.0, 0.0, 1.0];
    }

    fn wet_targets(&mut self, off: i32) {
        let l = LEVELS[self.current].0;
        let clamp = |v: i32| if v > -10000 { v.min(0) } else { -10000 };
        self.target[0] = level(clamp(l[0] + off));
        self.target[2] = level(clamp(l[2] + off));
    }

    /// 0x4713F0(offset): a new offset outside a fade.
    pub fn set_offset(&mut self, offset: i32) {
        if self.fading_out || self.fading_in {
            return;
        }
        self.offset = offset as f32;
        self.wet_targets(offset);
    }

    /// 0x4714A0: one frame of the fades (`dt` s).
    pub fn update(&mut self, dt: f32) {
        self.reload = false;
        self.interp.step(dt * 1000.0);
        if self.fading_out {
            if self.interp.done && !self.busy {
                self.reload = true;
                self.fading_in = true;
                self.fading_out = false;
                self.current = self.requested;
                self.interp.set(0.0, 1.0, LEVELS[self.current].2, 0);
                self.cur = self.target;
                let l = LEVELS[self.current].0;
                self.wet_targets(self.offset as i32);
                self.target[1] = level(l[1]);
                self.target[3] = level(l[3]);
            }
        } else if self.fading_in && self.interp.done {
            self.fading_in = false;
            self.cur = self.target;
        }
        let to = |v: f32| (v as f64 * 32767.0) as i32;
        if !self.fading_out && !self.fading_in {
            for k in 0..4 {
                let (c, t) = (self.cur[k], self.target[k]);
                self.cur[k] = if c + 0.25 < t {
                    c + 0.25
                } else if t < c - 0.25 {
                    c - 0.25
                } else {
                    t
                };
                self.out[k] = to(self.cur[k]);
            }
        } else {
            let v = self.interp.value;
            for k in 0..4 {
                self.out[k] = to((self.target[k] - self.cur[k]) * v + self.cur[k]);
            }
        }
        // 0x47187D: every other frame the wet outputs gain 1.
        if self.nudge {
            for k in [0, 2] {
                self.out[k] = (self.out[k] + 1).clamp(0, 0x7FFF);
            }
            self.nudge = false;
        } else {
            self.nudge = true;
        }
        self.busy = !self.interp.done;
    }

    /// One in-game frame of SFXCTL_Tunnel_Update's reverb part (0x483437..
    /// 0x4834A3): `zone` the car's zone kind when inside one, `region` the
    /// "Reverb" parameter map's {field 0, field 2} at the car (0x47C6D0;
    /// `None` without the map).
    pub fn frame(&mut self, zone: Option<u32>, region: Option<(i32, i32)>, dt: f32) {
        match zone {
            Some(kind) if !self.inside => {
                self.inside = true;
                self.request(zone_preset(kind), 0);
            }
            Some(_) => {}
            None if self.inside => {
                self.inside = false;
                if self.enabled {
                    self.request(region_preset(self.region), 0);
                }
            }
            None => {}
        }
        // 0x47C6D0.
        self.last_region = self.region;
        if self.enabled {
            let mut off = 0;
            if let Some((r, o)) = region {
                self.region = r;
                off = o;
            }
            if !(0..12).contains(&self.region) {
                self.region = 9;
            }
            if self.region != self.last_region && !self.inside {
                self.request(region_preset(self.region), off);
            } else {
                self.set_offset(off);
            }
        }
        self.update(dt);
    }

    /// The engine voices' send and dry bytes (0x473F0E / 0x473ECF) at the
    /// object's level (0..0x7FFF).
    pub fn engine_bytes(&self, level: i32) -> (u8, u8) {
        let send = ((self.out[0] as i64 * level as i64) >> 23).clamp(0, 127) as u8;
        let dry = (self.out[1] >> 8).clamp(0, 127) as u8;
        (send, dry)
    }

    /// Whether a fade runs (+0xD4 / +0xD5).
    pub fn fading(&self) -> bool {
        self.fading_out || self.fading_in
    }
}

/// 0x7ED5D8 (the identity) with 0x47C766's clamp: ≥ 12 → 9.
fn region_preset(region: i32) -> usize {
    if (0..12).contains(&region) { region as usize } else { 9 }
}
