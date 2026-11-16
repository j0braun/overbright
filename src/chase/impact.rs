//! The impact shake: a short jolt of the player's view when the car's
//! acceleration spikes (a wall, a prop, a hard landing). Traced in
//! SPEED2.EXE (docs/formats.md, "Impact shake"):
//!
//! * **Trigger** (0x572E60, from the world update's human-car loop at
//!   0x60A154, after every car's 0x604750 has written car +0x80 = (velocity
//!   − last frame's) / the world step): the acceleration weighted
//!   (0.7, 0.5, 2.0) per world axis (0x7FB750), its length m; nothing below
//!   250 m/s² (0x79DCDC); f = (min(m, 600) − 250) / 350 (0x79DCE0); the
//!   amplitude 0.08 + 0.08 f m (0x79DCE4 / 0x79DCE8), the duration
//!   0.3 + 0.4 f s (0x79DCEC / 0x79DCF0). A running shake is replaced only
//!   by a larger amplitude than its curve's present value. Under the
//!   player's event kind 7 (+0x58) the amplitude is × 0.8 (0x7850EC) and
//!   the duration × 1.2 (0x78A0EC). The slot (0x865360 + player × 0x20)
//!   keeps −amplitude · unit(weighted), the curve 0x7FB7A0, time 0, the
//!   duration and the strobe rate 17 (0x79DC70).
//! * **Step** (0x5728D0, once a main-loop frame at 0x5814C2, after the
//!   cameras): time += world step / duration; past the curve's last key
//!   (1.0) the shake ends.
//! * **Offset** (0x572870): the slot's vector × the curve at the time,
//!   only while trunc(17 · time) is even — a strobe, not a wave.
//! * **Apply** (0x572A60, the chase camera 0x453B55 and the rear-view
//!   mirror 0x44BE7B, views 1 and 3 on player 0's slot): only while the
//!   world's time scale (+0x108) is at least 1; the view matrix V becomes
//!   V · T · Rx(a) · Ry(b) (row vectors), T the offset in the view's axes,
//!   a = trunc(−1200 (T.y + T.z)), b = trunc(2000 T.x) binary angles
//!   (0x7FB740 / 0x7FB744; the z turn's factor 0x7FB748 is 0).

use glam::{Mat3, Vec3};

use crate::render::Camera;

/// The curve 0x7FB7A0 (5 keys at 0x7FB36C): (time, scale).
const CURVE: [(f32, f32); 5] = [(0.0, 1.0), (0.1, 0.7), (0.3, 0.5), (0.5, 0.4), (1.0, 0.0)];
/// The weights of the world axes (0x7FB750).
const WEIGHT: Vec3 = Vec3::new(0.7, 0.5, 2.0);
/// The weighted acceleration's range, m/s² (0x79DCDC, 0x79DCE0).
const FROM: f32 = 250.0;
const TO: f32 = 600.0;
/// The amplitude's (m) and duration's (s) ranges (0x79DCE4..0x79DCF0).
const AMPLITUDE: (f32, f32) = (0.08, 0.16);
const DURATION: (f32, f32) = (0.3, 0.7);
/// Event kind 7's factors (0x7850EC, 0x78A0EC).
const KIND7: (f32, f32) = (0.8, 1.2);
/// The strobe's rate per unit of curve time (0x79DC70).
const RATE: f32 = 17.0;
/// The view's turns per metre of offset, binary angle (0x7FB740, 0x7FB744).
const TURN: (f32, f32) = (-1200.0, 2000.0);

/// One player's shake slot (0x865360 + player × 0x20).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactShake {
    /// +0x00: −amplitude · the weighted acceleration's direction (world).
    vector: Vec3,
    /// +0x10: the curve pointer, set while a shake runs.
    running: bool,
    /// +0x14: the time along the curve (0 → 1).
    time: f32,
    /// +0x18: seconds per unit of curve time.
    duration: f32,
    /// +0x1C: the strobe's rate.
    rate: f32,
}

impl Default for ImpactShake {
    /// The reset 0x5729F0 (the world object's constructor, its list
    /// 0x5FF8A0 and a new chase mover 0x5FA830): no shake, duration 1.
    fn default() -> Self {
        ImpactShake { vector: Vec3::ZERO, running: false, time: 0.0, duration: 1.0, rate: RATE }
    }
}

/// The curve's scale at `t` (0x5722B0): 0 outside [first key, last key),
/// else linear between the keys around `t`.
fn curve(t: f32) -> f32 {
    let (first, last) = (CURVE[0].0, CURVE[CURVE.len() - 1].0);
    if t < first || last <= t {
        return 0.0;
    }
    let mut i = CURVE.iter().take_while(|k| t >= k.0).count();
    i = i.saturating_sub(1);
    let (t0, v0) = CURVE[i];
    if i + 2 <= CURVE.len() {
        let (t1, v1) = CURVE[i + 1];
        (t - t0) * ((v1 - v0) / (t1 - t0)) + v0
    } else {
        v0
    }
}

impl ImpactShake {
    /// A shake runs (the slot's curve pointer is set).
    pub fn running(&self) -> bool {
        self.running
    }

    /// The slot's vector (+0x00), time (+0x14) and running flag (+0x10),
    /// for the developer harness's recordings.
    pub fn fields(&self) -> (Vec3, f32, bool) {
        (self.vector, self.time, self.running)
    }

    /// 0x572E60 for the player's car acceleration `accel` (car +0x80,
    /// m/s², world axes); `kind7` the player's event kind (+0x58) is 7.
    /// Whether a shake started.
    pub fn trigger(&mut self, accel: Vec3, kind7: bool) -> bool {
        let w = accel * WEIGHT;
        let m = (w.z * w.z + w.x * w.x + w.y * w.y).sqrt();
        if !(m >= FROM) {
            return false;
        }
        let f = (m.min(TO) - FROM) / (TO - FROM);
        let mut amplitude = (AMPLITUDE.1 - AMPLITUDE.0) * f + AMPLITUDE.0;
        let now = if self.running { curve(self.time) } else { 0.0 };
        if !(amplitude > now) {
            return false;
        }
        let mut duration = (DURATION.1 - DURATION.0) * f + DURATION.0;
        if kind7 {
            amplitude *= KIND7.0;
            duration *= KIND7.1;
        }
        // 0x43CF90: the vector scaled to the amplitude ((amplitude, 0, 0)
        // for a zero vector).
        let len = (w.z * w.z + w.y * w.y + w.x * w.x).sqrt() / amplitude;
        let dir = if len != 0.0 { w * (1.0 / len) } else { Vec3::new(amplitude, 0.0, 0.0) };
        // 0x572820: stored × −1 (0x7842A0).
        *self = ImpactShake { vector: dir * -1.0, running: true, time: 0.0, duration, rate: RATE };
        true
    }

    /// 0x5728D0 for one main-loop frame of `world_dt` seconds (the world's
    /// step, slow motion included).
    pub fn step(&mut self, world_dt: f32) {
        if self.running {
            self.time += world_dt / self.duration;
            if self.time < CURVE[0].0 || CURVE[CURVE.len() - 1].0 <= self.time {
                self.running = false;
            }
        }
    }

    /// 0x572870: the view's offset this frame (world axes, m); zero
    /// between shakes and on the strobe's odd steps.
    pub fn offset(&self) -> Vec3 {
        // The product of two floats is exact in f64 (as in the x87's
        // extended registers); _ftol truncates.
        let step = (f64::from(self.rate) * f64::from(self.time)) as i64;
        if step & 1 != 0 || !self.running {
            return Vec3::ZERO;
        }
        self.vector * curve(self.time)
    }
}

/// 0x572A60's change of a view looking from `eye` along `forward` with
/// `up` (the look-at's own axes, 0x5BA9B0: x = unit(forward × up), y =
/// unit(forward × x) — down —, z = forward): the shaken eye, forward and
/// up. None for a zero offset (the view is left as it is).
pub fn shake_view(eye: Vec3, forward: Vec3, up: Vec3, offset: Vec3) -> Option<(Vec3, Vec3, Vec3)> {
    if offset == Vec3::ZERO {
        return None;
    }
    let f = forward.normalize_or(Vec3::X);
    let r = (-up).cross(f).normalize_or(Vec3::Y);
    let d = f.cross(r).normalize_or(Vec3::NEG_Z);
    // T = offset · V's rotation (row vector): the offset in the view's axes.
    let t = Vec3::new(
        offset.z * r.z + offset.y * r.y + offset.x * r.x,
        offset.z * d.z + offset.y * d.y + offset.x * d.x,
        offset.z * f.z + offset.y * f.y + offset.x * f.x,
    );
    let angle = |v: f32| f32::from(v as i32 as u16) * std::f32::consts::TAU / 65536.0;
    let a = angle((t.y + t.z) * TURN.0);
    let b = angle(t.x * TURN.1);
    // V' = V · T · Rx(a) · Ry(b) with D3DX's row-vector rotations: in
    // column form c' = Ry(b) · Rx(a) · (c + T). The new eye is where c' is
    // 0 (eye − offset); its axes are B · (Ry Rx)ᵀ · eᵢ, B = [x y z].
    let back = (Mat3::from_rotation_y(b) * Mat3::from_rotation_x(a)).transpose();
    let basis = Mat3::from_cols(r, d, f);
    let axes = basis * back;
    Some((eye - offset, axes.z_axis, -axes.y_axis))
}

/// [`shake_view`] on a built camera (the rear-view mirror's).
pub fn shake_camera(camera: &mut Camera, offset: Vec3) {
    if let Some((eye, forward, up)) = shake_view(camera.position, camera.forward(), camera.up, offset) {
        let shaken = Camera::look_at(eye, eye + forward, up, camera.fov_y);
        camera.position = shaken.position;
        camera.yaw = shaken.yaw;
        camera.pitch = shaken.pitch;
        camera.up = shaken.up;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_is_the_tables() {
        assert_eq!(curve(0.0), 1.0);
        assert!((curve(0.05) - 0.85).abs() < 1e-6);
        assert!((curve(0.2) - 0.6).abs() < 1e-6);
        assert!((curve(0.75) - 0.2).abs() < 1e-6);
        assert_eq!(curve(1.0), 0.0);
        assert_eq!(curve(-0.1), 0.0);
    }

    #[test]
    fn below_250_nothing_starts() {
        let mut s = ImpactShake::default();
        // 0.7 · 357 = 249.9
        assert!(!s.trigger(Vec3::new(357.0, 0.0, 0.0), false));
        assert!(!s.running());
        assert_eq!(s.offset(), Vec3::ZERO);
    }

    #[test]
    fn amplitude_and_duration_follow_the_weighted_length() {
        // Vertical 150 m/s² weighs 300: f = 50 / 350.
        let mut s = ImpactShake::default();
        assert!(s.trigger(Vec3::new(0.0, 0.0, -150.0), false));
        let f = 50.0 / 350.0;
        let amp = 0.08 * f + 0.08;
        assert!((s.offset() - Vec3::new(0.0, 0.0, amp)).length() < 1e-6, "{}", s.offset());
        assert!((s.duration - (0.4 * f + 0.3)).abs() < 1e-6);
        // Over 600 the top: 0.16 m for 0.7 s.
        let mut s = ImpactShake::default();
        assert!(s.trigger(Vec3::new(-2000.0, 0.0, 0.0), false));
        assert!((s.offset() - Vec3::new(0.16, 0.0, 0.0)).length() < 1e-6);
        assert!((s.duration - 0.7).abs() < 1e-6);
    }

    #[test]
    fn kind7_scales_amplitude_and_duration() {
        let mut s = ImpactShake::default();
        assert!(s.trigger(Vec3::new(0.0, 2000.0, 0.0), true));
        assert!((s.offset() - Vec3::new(0.0, -0.128, 0.0)).length() < 1e-6);
        assert!((s.duration - 0.84).abs() < 1e-6);
    }

    #[test]
    fn the_strobe_and_the_end_at_60_fps() {
        let mut s = ImpactShake::default();
        s.trigger(Vec3::new(1000.0, 0.0, 0.0), false);
        // 0.7 s: time grows by (1/60) / 0.7 a frame; trunc(17 t) odd → 0:
        // three frames on, two off.
        let mut on = Vec::new();
        for _ in 0..60 {
            on.push(s.offset() != Vec3::ZERO);
            s.step(1.0 / 60.0);
        }
        let frames = on.iter().rposition(|&b| b).unwrap() + 1;
        assert_eq!(&on[..6], &[true, true, true, false, false, true]);
        assert!((41..=42).contains(&frames), "{frames}");
        assert!(!s.running());
    }

    /// The original's shake through crash_wall's hit (free roam, ~15 m/s
    /// head-on, the parity cave's 60 fps tick pattern; native.tsv rows
    /// 190..223): car +0x80 = (398.185, 784.247, 2.02513) on the hit's
    /// frame, then slot 0's vector and time per frame as read under Wine.
    #[test]
    fn the_originals_crash_wall_shake() {
        // Each frame's ticks of 1/1800 s and the slot's time after it.
        const ROWS: [(u32, f32); 33] = [
            (30, 0.0295442), (29, 0.0581037), (31, 0.0886327), (30, 0.118177), (29, 0.146736),
            (31, 0.177265), (30, 0.20681), (29, 0.235369), (31, 0.265898), (30, 0.295442),
            (29, 0.324002), (31, 0.354531), (30, 0.384075), (29, 0.412634), (31, 0.443163),
            (30, 0.472708), (29, 0.501267), (31, 0.531796), (30, 0.56134), (29, 0.5899),
            (31, 0.620429), (30, 0.649973), (29, 0.678533), (31, 0.709062), (30, 0.738606),
            (29, 0.767165), (31, 0.797694), (30, 0.827239), (29, 0.855798), (31, 0.886327),
            (30, 0.915871), (29, 0.944431), (31, 0.97496),
        ];
        let mut s = ImpactShake::default();
        assert!(s.trigger(Vec3::new(398.185, 784.247, 2.02513), false));
        let native = Vec3::new(-0.0769517, -0.108258, -0.0011182);
        assert!((s.vector - native).abs().max_element() < 2e-6, "{}", s.vector);
        // The views of the hit's frame take the full vector (curve 1).
        assert_eq!(s.offset(), s.vector);
        for (i, &(ticks, time)) in ROWS.iter().enumerate() {
            // The other frames' accelerations (≤ 35 m/s² weighted) start nothing.
            s.step(ticks as f32 / 1800.0);
            assert!(s.running(), "row {i}");
            assert!((s.time - time).abs() < 2e-5, "row {i}: {} vs {time}", s.time);
        }
        // Row 223 (30 ticks): 1.0045, the slot's curve pointer cleared.
        s.step(30.0 / 1800.0);
        assert!(!s.running());
        assert_eq!(s.offset(), Vec3::ZERO);
    }

    #[test]
    fn a_smaller_hit_does_not_replace_a_running_shake() {
        let mut s = ImpactShake::default();
        s.trigger(Vec3::new(1000.0, 0.0, 0.0), false);
        assert!(!s.trigger(Vec3::new(0.0, 0.0, -130.0), false));
        // Late in the curve (scale < 0.08) the smallest hit replaces it.
        for _ in 0..40 {
            s.step(1.0 / 60.0);
        }
        assert!(s.trigger(Vec3::new(0.0, 0.0, -130.0), false));
    }

    #[test]
    fn the_view_moves_by_the_offset_and_turns() {
        let eye = Vec3::new(1.0, 2.0, 3.0);
        let (e, f, u) = shake_view(eye, Vec3::X, Vec3::Z, Vec3::new(0.0, 0.1, 0.0)).unwrap();
        assert!((e - Vec3::new(1.0, 1.9, 3.0)).length() < 1e-6);
        // Offset along +Y (the view's left): T.x = −0.1 → b = trunc(−200),
        // the view turns about its down axis.
        let b = f32::from(-200i32 as u16) * std::f32::consts::TAU / 65536.0;
        assert!((f.dot(Vec3::X) - b.cos()).abs() < 1e-5, "{f}");
        assert!((u - Vec3::Z).length() < 1e-5);
        assert!(shake_view(eye, Vec3::X, Vec3::Z, Vec3::ZERO).is_none());
    }
}
