use super::{EnvInput, EnvSfx, ramp};
use crate::audio::carfx::{Sink, open_bank, run_bank};
use crate::audio::skids::WheelCtl;
use crate::audio::stitch::StitchVoice;

/// Graph_Eval (0x5767A0) through the road-noise points (0x7F09B0).
pub fn road_graph(mph: f32) -> f32 {
    const P: [(f32, f32); 5] = [(0.0, 0.0), (60.0, 28000.0), (100.0, 32500.0), (150.0, 24500.0), (175.0, 18000.0)];
    if mph <= P[0].0 {
        return P[0].1;
    }
    if mph >= P[4].0 {
        return P[4].1;
    }
    for w in P.windows(2) {
        if w[0].0 <= mph && mph < w[1].0 {
            return (mph - w[0].0) / (w[1].0 - w[0].0) * (w[1].1 - w[0].1) + w[0].1;
        }
    }
    P[0].1
}

/// 0x45F940: a surface's road-noise class (the FX_ROADNOISE variant).
pub fn surface_class(s: i32) -> i32 {
    match s {
        1 | 3 => 5,
        2 | 13 => 4,
        7 | 26 => 7,
        8 | 10 | 15 => 0,
        11 | 12 | 17 => 2,
        14 => 3,
        16 => 1,
        25 => 6,
        _ => -1,
    }
}

/// 0x45F9D0: the transition variant entering a surface.
pub fn surface_transition(s: i32) -> i32 {
    match s {
        11 | 16 => 0,
        14 => 2,
        _ => -1,
    }
}

/// 0x7ECF4C: the class volumes.
pub const CLASS_VOLUME: [i32; 8] = [0x3A98, 0x5DC0, 0x7D00, 0x5DC0, 0x5DC0, 0x1388, 0x4A38, 0x7EF4];

/// 0x4735E0's outputs for the player: [left +0x100, right +0xFC] and
/// [left +0x108, right +0x104].
pub fn road_levels(speed: f32, w: &WheelCtl) -> ([i32; 2], [i32; 2]) {
    let mph = speed * 2.237;
    let b = ((road_graph(mph) as i32) * 0x7FFF) >> 15;
    let b = b as f32;
    let r = |x: f32, y: f32, k: f32, cap: f32| ((x * x + y * y).sqrt() * k).min(cap) + 1.0;
    let right = (0.1f32 + 1.0) * r(w.w90, w.w94, 0.01, 0.15) * b;
    let left = b * r(w.w98, w.w9c, 0.01, 0.15) * (((w.wa[3] + w.wa[0]) * 0.5 * 1.0 * 0.1).min(0.1) + 1.0);
    let cap = |v: f32, m: f32| if v > m { m } else { v };
    let o = ramp(1500.0, 4500.0, 0.0, 100.0, mph);
    let lp = o * r(w.w98, w.w9c, 0.01, 0.2) * (((w.wa[3] + w.wa[0]) * 0.5 * 1.0 * 0.15).min(0.15) + 1.0);
    let rp = o * r(w.w90, w.w94, 0.01, 0.2) * (((w.wa[2] + w.wa[1]) * 0.5 * 1.0 * 0.15).min(0.15) + 1.0);
    ([cap(left, 32000.0) as i32, cap(right, 32000.0) as i32], [cap(lp, 6000.0) as i32, cap(rp, 6000.0) as i32])
}

#[derive(Default)]
pub(super) struct Road {
    surf: [i32; 2],
    prev: [i32; 2],
    changed: [bool; 2],
    class: [i32; 2],
    vol: [i32; 2],
    p2: [i32; 2],
    base: [Option<u32>; 2],
    surface: [Option<u32>; 2],
    blocks: [[i32; 7]; 2],
    trans: [Option<u32>; 2],
    pub(super) thump: [Option<StitchVoice>; 2],
    counter: [u32; 2],
}

pub(super) const ROAD: &str = "SOUND/IG_GLOBAL/ROADNOISE_00_MB.abk";

impl EnvSfx {
    /// 0x4731C0 for `side` (0 left, 1 right).
    fn transition(&mut self, variant: i32, side: usize, i: &EnvInput) {
        let mph = i.speed * 2.237;
        let base = match variant {
            0 | 1 => 12000,
            2 | 3 => 0x5FB4,
            _ => 0x7FFF,
        };
        let v = (base * ramp(20000.0, 32767.0, 12.0, 60.0, mph) as i32) >> 15;
        let p2 = ramp(3600.0, 4400.0, 12.0, 60.0, mph) as i32;
        let slot = if side == 0 { 3 } else { 2 };
        let vol = ((i.mix.level(6, slot) * v) >> 15).clamp(0, 0x7FFF);
        if let Some(b) = self.road_bank.as_mut() {
            let ev = &mut b.events;
            if let Some(id) = self.road.trans[side].take() {
                ev.release(id);
            }
            let block = [variant.clamp(0, 15), vol, p2.clamp(0, 0x1FFF), i.mix.az(6, slot).clamp(0, 0xFFFF), 1, 0, 0];
            self.road.trans[side] = ev.module("FX_ROADNOISE_TRANS").and_then(|m| ev.play(m, &block));
        }
        if self.road.thump[side].is_none() {
            let n = self.road.counter[side] & 3;
            self.road.counter[side] = n + 1;
            if let Some(s) = self.stitches.as_mut() {
                self.road.thump[side] = StitchVoice::start(s, 0, 0x4E + n as usize);
                self.thumps += self.road.thump[side].is_some() as usize;
            }
        }
    }

    pub(super) fn road_update(&mut self, sink: &mut dyn Sink, i: &EnvInput, dt: f32) {
        let Some(w) = i.wheel.clone() else { return };
        open_bank(&mut self.road_bank, &mut self.sounds, &mut self.gains, sink, ROAD);
        // Start (0x472E50): the two base events.
        if let Some(b) = self.road_bank.as_mut() {
            let ev = &mut b.events;
            for e in &mut self.road.base {
                if e.is_none_or(|id| !ev.alive(id)) {
                    *e = ev.module("FX_ROADNOISE").and_then(|m| ev.play(m, &[4, 0, 0, 0, 0, 0, 0]));
                }
            }
        }
        // Update (0x47CD90): 0x4735E0, then 0x45FAA0.
        let (vol, p2) = road_levels(i.speed, &w);
        self.road.vol = vol;
        self.road.p2 = p2;
        for side in 0..2 {
            let s = w.surface[side] as i32;
            self.road.prev[side] = self.road.surf[side];
            self.road.changed[side] = s != self.road.surf[side];
            self.road.surf[side] = s;
        }
        // Per frame (0x47CDB0): left first.
        for side in 0..2 {
            if !self.road.changed[side] {
                continue;
            }
            let s = self.road.surf[side];
            let class = surface_class(s);
            self.road.class[side] = class;
            let trans = if matches!(self.road.prev[side], 11 | 16) { 1 } else { -1 };
            let v2 = surface_transition(s);
            if let Some(b) = self.road_bank.as_mut() {
                let ev = &mut b.events;
                if let Some(id) = self.road.surface[side].take() {
                    ev.release(id);
                }
                if class != -1 {
                    let block = [class.clamp(0, 15), 0, 0x1000, 0, 0, 0, 0];
                    self.road.blocks[side] = block;
                    self.road.surface[side] = ev.module("FX_ROADNOISE").and_then(|m| ev.play(m, &block));
                }
            }
            if w.down[side] {
                if trans != -1 {
                    self.transition(trans, side, i);
                } else if v2 != -1 {
                    self.transition(v2, side, i);
                }
            }
        }
        if let Some(b) = self.road_bank.as_mut() {
            let ev = &mut b.events;
            for side in 0..2 {
                let (slot, base_slot) = if side == 0 { (1, 5) } else { (0, 4) };
                let t = CLASS_VOLUME.get(self.road.class[side] as usize).copied().unwrap_or(-1);
                let mut v = (i.mix.level(6, slot) * ((t * self.road.vol[side]) >> 15)) >> 15;
                if !w.down[side] {
                    v = 0;
                }
                let p2 = self.road.p2[side].clamp(0, 0x1FFF);
                if let Some(id) = self.road.surface[side] {
                    let blk = &mut self.road.blocks[side];
                    blk[1] = v.clamp(0, 0x7FFF);
                    blk[2] = p2;
                    blk[3] = i.mix.az(6, slot).clamp(0, 0xFFFF);
                    ev.set(id, blk);
                }
                // The base events: both with the left volume (as coded).
                if let Some(id) = self.road.base[side] {
                    let v = (i.mix.level(6, base_slot) * ((self.road.vol[0] * CLASS_VOLUME[4]) >> 15)) >> 15;
                    ev.set(id, &[4, v.clamp(0, 0x7FFF), p2, i.mix.az(6, base_slot).clamp(0, 0xFFFF), 0, 0, 0]);
                }
                if matches!(self.road.surf[side], 8 | 9) {
                    self.road.blocks[side][5] = 1;
                }
            }
        }
        for side in 0..2 {
            let slot = if side == 0 { 7 } else { 6 };
            if let Some(t) = self.road.thump[side].as_mut() {
                if !t.update(sink, i.mix.level(6, slot), i.mix.az(6, slot), dt) {
                    self.road.thump[side] = None;
                }
            }
        }
        self.splash_frame();
        run_bank(&mut self.road_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}
