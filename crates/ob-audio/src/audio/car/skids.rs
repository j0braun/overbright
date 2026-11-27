//! The player car's skids: `SFXCTL_Wheel` (0x46EA50), `SFXCTL_Skid`
//! (0x46DFE0, ticked by 0x47BDF0) and `CARSFX_Skids` (vtable 0x78AE48:
//! creator 0x4608D0, update 0x460A70, per frame 0x460C70), feeding the
//! FX_SKID graph of SOUND/SKIDS (verified unless marked; see "Skids" in
//! docs/formats.md).

// The record type sits with the engine's tables (`exe_tables::records`).
pub use crate::exe_tables::records::SkidRecord;


pub static RECORDS: crate::exe_tables::Table<[SkidRecord; 4]> = crate::exe_tables::Table::new(|t| &t.skid_records);

/// The record (0x45BD10 / table at 0x45BD81): the drift flag, or 2 +
/// drift for sound indices 9, 13, 14, 20 and 21.
pub fn record_index(sound_index: usize, drift: bool) -> usize {
    if matches!(sound_index, 9 | 13 | 14 | 20 | 21) { 2 + drift as usize } else { drift as usize }
}

/// The skid class 0x458F00 of a sound index (1..32).
pub fn class(sound_index: usize) -> u8 {
    static C: crate::exe_tables::Table<[u8; 32]> = crate::exe_tables::Table::new(|t| &t.skid_class);
    if (1..=32).contains(&sound_index) { C[sound_index - 1] } else { 0 }
}

/// The bank (0x465B90): 0x7ED1F8[2·drift + (class = 2)] under
/// sound\Skids\.
pub fn bank_path(sound_index: usize, drift: bool) -> String {
    const B: [&str; 4] = ["SKID_PAV_MB", "SKID_PAV2_MB", "SKIDS_DRIFT_MB", "SKIDS_DRIFT2_MB"];
    format!("SOUND/SKIDS/{}.abk", B[2 * drift as usize + (class(sound_index) == 2) as usize])
}

/// One wheel as the car physics holds it (car +0x390 + i, +0x394 + 4i,
/// +0x3B4 + 8i / +0x3B8 + 8i). Order FL, FR, RR, RL.
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelIn {
    pub on_ground: bool,
    pub surface: u8,
    /// The longitudinal slip the wheel exports to the car (car +0x3B4 +
    /// 8i: r·ω − v while the tyre slides, 0 while it grips) and the
    /// lateral slip (+0x3B8 + 8i), m/s.
    pub slip: f32,
    pub lateral: f32,
    /// The wheel's own slip r·ω − v (the movers' +0x118, whatever the
    /// tyre's state), which car +0x3DC / +0x3E0 take (0x5A5A6A).
    pub spin_slip: f32,
}

/// The car's inputs to the skid controllers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkidInput {
    pub wheels: [WheelIn; 4],
    /// Forward speed, m/s (car +0x430).
    pub speed: f32,
    /// The signed forward speed (car +0x42C): both slip ratios are 0
    /// unless it is above 0 (0x5ED4A0 / 0x5ED540).
    pub forward: f32,
    /// The slip angle, radians (0x59FA20).
    pub slip_angle: f32,
    /// The velocity in the car's frame, normalised (x forward, y left;
    /// (1, 0) when still).
    pub local_dir: (f32, f32),
    /// Steering, degrees (car +0x40E as degrees, positive left).
    pub steer_degrees: f32,
    /// The input block's brake and handbrake (car +0x30 → +0x214 / +0x210).
    pub brake: f32,
    pub handbrake: f32,
}

/// The running mean 0x576980 (window +5, here 5: 0x45BB00 calls
/// 0x579F30(5)); `reset` is 0x576A20.
#[derive(Clone, Debug)]
struct Mean {
    buf: [f32; 5],
    count: usize,
    idx: usize,
    sum: f32,
    mean: f32,
}

impl Mean {
    fn new() -> Mean {
        Mean { buf: [0.0; 5], count: 0, idx: 0, sum: 0.0, mean: 0.0 }
    }
    fn reset(&mut self, v: f32) {
        self.buf = [v; 5];
        self.count = 5;
        self.mean = v;
        self.sum = 5.0 * v;
    }
    fn push(&mut self, v: f32) -> f32 {
        self.count = (self.count + 1).min(5);
        self.sum += v - self.buf[self.idx];
        self.buf[self.idx] = v;
        self.mean = self.sum / self.count as f32;
        self.idx = (self.idx + 1) % 5;
        self.mean
    }
}

fn t(x: f32) -> i32 {
    x as i32
}

/// SFXCTL_Wheel's sums (0x46EA50): +0x90/+0x94 over wheels 1, 2 on the
/// ground, +0x98/+0x9C over wheels 0, 3; +0xA0 + 4i = |car +0x3D4 + 4i|
/// (traced): +0x3D4 is only zeroed (0x5A93E7) and replay-copied, +0x3D8 is
/// an integer state (0x5A8EA1, read as a float it is a denormal: 0),
/// +0x3DC = min(max(|wheel 0 slip|, |wheel 3 slip|) / 6, 1) and +0x3E0 the
/// same for wheels 1 and 2 (0x5A5A6A–0x5A5B11: the movers' +0x118, the
/// longitudinal slip r·ω − v, over 6.0 at 0x7A1A2C); +0xB0 / +0xB4 the side surfaces
/// (0x45BFB0: wheel 0's when it equals wheel 3's, wheel 1's when it equals
/// wheel 2's, else kept), +0xB8 / +0xBC the previous ones; +0xE8 / +0xE9 the left / right side on the
/// ground (0x46E8B0).
#[derive(Clone, Debug, Default)]
pub struct WheelCtl {
    pub w90: f32,
    pub w94: f32,
    pub w98: f32,
    pub w9c: f32,
    pub wa: [f32; 4],
    pub surface: [u8; 2],
    /// +0xB8 / +0xBC: +0xB0 / +0xB4 as the previous update left them
    /// (0x45BFE3..0x45BFF5, stored before the new ones).
    pub prev_surface: [u8; 2],
    pub down: [bool; 2],
}

impl WheelCtl {
    pub fn update(&mut self, i: &SkidInput) {
        self.prev_surface = self.surface;
        let w = &i.wheels;
        let (mut a, mut b) = ((0.0, 0.0), (0.0, 0.0));
        for (k, wh) in w.iter().enumerate() {
            if !wh.on_ground {
                continue;
            }
            let s = if k == 1 || k == 2 { &mut a } else { &mut b };
            s.0 += wh.slip;
            s.1 += wh.lateral;
        }
        (self.w90, self.w94, self.w98, self.w9c) = (a.0, a.1, b.0, b.1);
        let side = |a: usize, b: usize| (w[a].spin_slip.abs().max(w[b].spin_slip.abs()) * (1.0 / 6.0)).min(1.0);
        self.wa = [0.0, 0.0, side(0, 3), side(1, 2)];
        if w[0].surface == w[3].surface {
            self.surface[0] = w[0].surface;
        }
        if w[1].surface == w[2].surface {
            self.surface[1] = w[1].surface;
        }
        self.down = [w[0].on_ground || w[3].on_ground, w[1].on_ground || w[2].on_ground];
    }
}

/// SFXCTL_Skid (0x46DFE0): three intensities per side, 0..1024: [right
/// (+0x194, +0x198, +0x19C), left (+0x1A0, +0x1A4, +0x1A8)].
#[derive(Clone, Debug)]
pub struct SkidCtl {
    pub rec: SkidRecord,
    mean_a: Mean,
    mean_b: Mean,
    mean_q1: Mean,
    mean_q2: Mean,
    timer: [f32; 2],
    l2: [i32; 2],
    l3: [i32; 2],
    pub out: [[i32; 3]; 2],
}

impl SkidCtl {
    /// Slot 8 (0x45BDF0): the means at +0xA4, +0x144, +0x16C reset to 0;
    /// +0xCC is never reset (the game's).
    pub fn new(rec: SkidRecord) -> SkidCtl {
        let mut s = SkidCtl {
            rec,
            mean_a: Mean::new(),
            mean_b: Mean::new(),
            mean_q1: Mean::new(),
            mean_q2: Mean::new(),
            timer: [0.0; 2],
            l2: [0; 2],
            l3: [0; 2],
            out: [[0; 3]; 2],
        };
        s.mean_a.reset(0.0);
        s.mean_q1.reset(0.0);
        s.mean_q2.reset(0.0);
        s
    }

    pub fn update(&mut self, w: &WheelCtl, i: &SkidInput, dt: f32) {
        let r = self.rec;
        // Layer 1: wheelspin.
        let side = |x: f32| if x > 0.0 { r.long_scale * x } else { 0.0 }.clamp(r.long_lo, r.long_hi);
        let ma = self.mean_a.push(side(w.w90));
        let mb = self.mean_b.push(side(w.w98));
        let l1 = |m: f32| t((m - r.long_lo) * 1024.0 / (r.long_hi - r.long_lo));
        let (o194, o1a0) = (l1(ma), l1(mb));
        // The wheel gates (+0x1AC / +0x1B0 timers, I1 / I2).
        let mut gate = |k: usize, a: f32, b: f32| -> i32 {
            if i.speed.abs() <= 0.5 {
                return 0;
            }
            if a > r.wheel_threshold || b > r.wheel_threshold {
                self.timer[k] += dt;
                if self.timer[k] <= r.hold {
                    0
                } else {
                    t((a.max(b) - r.wheel_threshold) * 1024.0 / (1.0 - r.wheel_threshold))
                }
            } else {
                self.timer[k] = 0.0;
                0
            }
        };
        let i1 = gate(0, w.wa[1], w.wa[2]);
        let i2 = gate(1, w.wa[0], w.wa[3]);
        let (nx, ny) = i.local_dir;
        let lat = ny.abs();
        let back = if nx <= 0.0 { nx.abs() } else { 0.0 };
        let ang = i.slip_angle;
        let dir = ang > 0.0;
        let span = if (r.angle_in1 - r.angle_in0).abs() < 1e-6 { 1e-6 } else { r.angle_in1 - r.angle_in0 };
        let g_ang = ((ang.abs() - r.angle_in0) / span).clamp(0.0, 1.0);
        // Layer 2: lateral.
        let slew = |old: i32, v: i32, up: i32, down: i32| (old + up).min((old - down).max(v));
        self.l2[0] = slew(self.l2[0], t(i1 as f32 * lat * r.l2_mix), r.l2_up, r.l2_down);
        self.l2[1] = slew(self.l2[1], t(i2 as f32 * lat * r.l2_mix), r.l2_up, r.l2_down);
        let g = (1.0 - r.l2_mix) * g_ang * 1024.0;
        let (k1, k2) = if dir { (1.0, r.off_side) } else { (r.off_side, 1.0) };
        let o198 = t((self.l2[0] as f32 + g) * k1);
        let o1a4 = t((self.l2[1] as f32 + g) * k2);
        // Layer 3: braking lateral and reverse.
        let lat_side = |s: f32, l: f32| if s < 0.0 { (l * r.lat_scale).abs() } else { 0.0 }.clamp(r.lat_lo, r.lat_hi);
        let q = |c: f32| t((c - r.lat_lo) * 1024.0 / (r.lat_hi - r.lat_lo));
        let mut m1 = t(self.mean_q1.push(q(lat_side(w.w90, w.w94)) as f32));
        let mut m2 = t(self.mean_q2.push(q(lat_side(w.w98, w.w9c)) as f32));
        let (v1, v2) = if i.handbrake.max(i.brake) > 0.15 {
            (t(i1 as f32 * back * r.l3_mix), t(i2 as f32 * back * r.l3_mix))
        } else {
            m1 = 0;
            m2 = 0;
            (0, 0)
        };
        self.l3[0] = slew(self.l3[0], v1, r.l3_up, r.l3_down);
        self.l3[1] = slew(self.l3[1], v2, r.l3_up, r.l3_down);
        let o19c = t((1.0 - r.l3_mix) * m1 as f32) + t(self.l3[0] as f32 * r.l3_mix);
        let o1a8 = t((1.0 - r.l3_mix) * m2 as f32) + t(self.l3[1] as f32 * r.l3_mix);
        let c = |x: i32| x.clamp(0, 1024);
        self.out = [[c(o194), c(o198), c(o19c)], [c(o1a0), c(o1a4), c(o1a8)]];
    }
}

/// 0x5ED4A0 / 0x5ED540: front / rear lateral slip over speed (0 under
/// 1 m/s or when not moving forward; the front one only against the
/// steering). The caller truncates them with no scale (0x460F6E), as
/// the original does: 0 for any slip below the speed.
fn front_ratio(i: &SkidInput) -> f32 {
    let avg = 0.5 * (i.wheels[0].lateral + i.wheels[1].lateral);
    if i.forward <= 0.0 || i.speed <= 1.0 || avg * i.steer_degrees >= 0.0 { 0.0 } else { avg.abs() / i.speed }
}

fn rear_ratio(i: &SkidInput) -> f32 {
    if i.forward <= 0.0 || i.speed <= 1.0 {
        0.0
    } else {
        (0.5 * (i.wheels[2].lateral + i.wheels[3].lateral)).abs() / i.speed
    }
}

/// CARSFX_Skids' two events (count 2, one per side: **inferred** for
/// the player from the context's vfunc +0x2C, which is open) and their
/// parameter blocks (0x460C70) with the mix map's levels (event 0 slots
/// 3, 5, 4 and the azimuth of slot 3; event 1 slots 0, 2, 1 and the
/// azimuth of slot 1).
pub fn event_blocks(
    w: &WheelCtl,
    s: &SkidCtl,
    i: &SkidInput,
    drift: bool,
    mix: &crate::audio::mixmap::Levels,
) -> [[i32; 17]; 2] {
    let mut out = [[0i32; 17]; 2];
    for (k, b) in out.iter_mut().enumerate() {
        // 0x460A70: the side's surface and layer volumes.
        let surf = w.surface[k];
        let vol = if !w.down[k] {
            0
        } else if matches!(surf, 14 | 15 | 22) {
            11579
        } else {
            0x7FFF
        };
        let (slots, az) = if k == 0 { ([3, 5, 4], 3) } else { ([0, 2, 1], 1) };
        let lv = |j: usize| ((mix.level(5, slots[j]) * vol) >> 15).clamp(0, 0x7FFF);
        // Event 0 takes the left intensities (+0x1A0..), event 1 the right.
        let int = if k == 0 { s.out[1] } else { s.out[0] };
        b[0] = lv(0);
        b[1] = lv(1);
        b[2] = lv(2);
        b[3] = int[0].clamp(0, 0x400);
        b[4] = int[1].clamp(0, 0x400);
        b[5] = int[2].clamp(0, 0x3FF);
        b[6] = mix.az(5, az);
        b[7] = drift as i32;
        b[8] = 0x1000;
        b[9] = 0x1000;
        b[10] = 0x1000;
        b[11] = t(i.slip_angle.abs() * 57.295_776).clamp(0, 180);
        b[12] = t(i.speed * 2.237).clamp(0, 1000);
        b[13] = 0;
        b[14] = t(front_ratio(i)).clamp(0, 0x400);
        b[15] = t(rear_ratio(i)).clamp(0, 0x400);
        b[16] = matches!(surf, 8 | 9) as i32;
    }
    out
}

/// 0x460A70 for a CARSFX_Skids with one event (count +0xC4 = 1: the
/// AI racers', 0x473BA0): the surface from both sides and the previous
/// update (+0xB0, +0xB4, +0xB8, +0xBC: both sides when they agree, else
/// the left side unless the previous left matches it and the previous
/// right does not, or the previous left matches the right).
pub fn single_surface(w: &WheelCtl) -> u8 {
    let (a, b) = (w.surface[0], w.surface[1]);
    let (c, d) = (w.prev_surface[0], w.prev_surface[1]);
    if a == b {
        a
    } else if c == a {
        if d == a { a } else { b }
    } else if c == b {
        b
    } else {
        a
    }
}

/// CARSFX_Skids' one event on an AI racer (count 1: 0x460A70 /
/// 0x460C70 with event index 0 only): the layer volumes from the one
/// surface ([`single_surface`]: 14, 15, 22 → 0x7ED240, else 0x7FFF), off
/// when the left side is not down (the context kind 2 gate on +0xE8,
/// 0x460B6E; **inferred**: an AI racer's context kind is 2); the
/// intensities the mean of the two sides' ((left + right) >> 1, 0x460BBE
/// / 0x460C01 / 0x460C45); the mix-map levels of event 0 (class 5 slots
/// 3, 5, 4, azimuth slot 3, in the racer's copy); the rest as the
/// player's event 0 (p16 from the left side's surface, +0xB0).
pub fn single_event_block(w: &WheelCtl, s: &SkidCtl, i: &SkidInput, drift: bool, mix: &crate::audio::mixmap::Levels) -> [i32; 17] {
    let mut b = [0i32; 17];
    let surf = single_surface(w);
    let vol = if !w.down[0] {
        0
    } else if matches!(surf, 14 | 15 | 22) {
        11579
    } else {
        0x7FFF
    };
    let lv = |slot: usize| ((mix.level(5, slot) * vol) >> 15).clamp(0, 0x7FFF);
    let mean = |j: usize| (s.out[0][j] + s.out[1][j]) >> 1;
    b[0] = lv(3);
    b[1] = lv(5);
    b[2] = lv(4);
    b[3] = mean(0).clamp(0, 0x400);
    b[4] = mean(1).clamp(0, 0x400);
    b[5] = mean(2).clamp(0, 0x3FF);
    b[6] = mix.az(5, 3);
    b[7] = drift as i32;
    b[8] = 0x1000;
    b[9] = 0x1000;
    b[10] = 0x1000;
    b[11] = t(i.slip_angle.abs() * 57.295_776).clamp(0, 180);
    b[12] = t(i.speed * 2.237).clamp(0, 1000);
    b[14] = t(front_ratio(i)).clamp(0, 0x400);
    b[15] = t(rear_ratio(i)).clamp(0, 0x400);
    b[16] = matches!(w.surface[0], 8 | 9) as i32;
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn one_event_takes_the_mean_of_both_sides() {
        let mut w = WheelCtl::default();
        let mut i = SkidInput { speed: 10.0, forward: 10.0, local_dir: (1.0, 0.0), ..Default::default() };
        for k in 0..4 {
            i.wheels[k] = WheelIn { on_ground: true, surface: 3, slip: 4.0, lateral: 0.0, spin_slip: 4.0 };
        }
        w.update(&i);
        let mut s = SkidCtl::new(RECORDS[0]);
        s.update(&w, &i, 1.0 / 60.0);
        let b = single_event_block(&w, &s, &i, false, &Default::default());
        assert_eq!(b[3], (s.out[0][0] + s.out[1][0]) >> 1);
        assert_eq!((b[0], b[1], b[2]), (0x7FFE, 0x7FFE, 0x7FFE));
        // The left side off the ground silences it (+0xE8).
        w.down[0] = false;
        assert_eq!(single_event_block(&w, &s, &i, false, &Default::default())[0], 0);
    }

    /// SFXCTL_Wheel's sums take the car's exported slip (+0x3B4 + 8i), its
    /// wheel gates the wheels' own slip (+0x3DC / +0x3E0 from the movers'
    /// +0x118): a gripping wheel turning 0.3 m/s fast adds nothing to the
    /// first but counts in the second.
    #[test]
    fn sums_read_the_export_gates_the_wheel_slip() {
        let mut w = WheelCtl::default();
        let mut i = SkidInput::default();
        for k in 0..4 {
            i.wheels[k] = WheelIn { on_ground: true, surface: 3, slip: 0.0, lateral: 0.0, spin_slip: 0.3 };
        }
        i.wheels[1].slip = 9.0;
        i.wheels[1].spin_slip = 9.0;
        w.update(&i);
        assert_eq!((w.w90, w.w98), (9.0, 0.0));
        assert!((w.wa[2] - 0.05).abs() < 1e-6, "{}", w.wa[2]);
        assert_eq!(w.wa[3], 1.0);
    }

    #[test]
    fn one_event_surface_rule() {
        let w = |a, b, c, d| WheelCtl { surface: [a, b], prev_surface: [c, d], ..Default::default() };
        assert_eq!(single_surface(&w(3, 3, 9, 9)), 3);
        assert_eq!(single_surface(&w(3, 5, 3, 3)), 3);
        assert_eq!(single_surface(&w(3, 5, 3, 5)), 5);
        assert_eq!(single_surface(&w(3, 5, 5, 0)), 5);
        assert_eq!(single_surface(&w(3, 5, 7, 7)), 3);
    }

    #[test]
    fn slip_ratios_need_forward_motion() {
        // 0x5ED4A0 / 0x5ED540: 0 unless car +0x42C > 0 (reversing too).
        let mut i = SkidInput { speed: 10.0, forward: 10.0, steer_degrees: 10.0, ..Default::default() };
        for w in &mut i.wheels {
            w.lateral = -15.0;
        }
        assert_eq!(front_ratio(&i), 1.5);
        assert_eq!(rear_ratio(&i), 1.5);
        i.forward = -10.0;
        assert_eq!(front_ratio(&i), 0.0);
        assert_eq!(rear_ratio(&i), 0.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn wheelspin_feeds_layer_one() {
        let mut w = WheelCtl::default();
        let mut i = SkidInput { speed: 10.0, forward: 10.0, local_dir: (1.0, 0.0), ..Default::default() };
        for k in 0..4 {
            i.wheels[k] = WheelIn { on_ground: true, surface: 3, slip: 4.0, lateral: 0.0, spin_slip: 4.0 };
        }
        w.update(&i);
        assert_eq!((w.w90, w.w98), (8.0, 8.0));
        let mut s = SkidCtl::new(RECORDS[0]);
        s.update(&w, &i, 1.0 / 60.0);
        // Right: clamp(1.3 · 8) = 10.4 into a reset (zero-filled) window
        // of 5: 2.08; left: the never-reset window holds just 10.4.
        assert_eq!(s.out[0][0], ((2.08f32 - 0.4) * 1024.0 / 10.6) as i32);
        assert_eq!(s.out[1][0], ((10.4f32 - 0.4) * 1024.0 / 10.6) as i32);
        let b = event_blocks(&w, &s, &i, false, &Default::default());
        assert_eq!(b[0][3], s.out[1][0]);
        assert_eq!(b[1][12], 22);
        assert_eq!(bank_path(9, false), "SOUND/SKIDS/SKID_PAV2_MB.abk");
        assert_eq!(record_index(9, true), 3);
    }
}
