//! Motion blur (0x5CC470; verified unless marked). See docs/formats.md
//! "Motion blur".
//!
//! The frame function calls it in the main view after the world and before
//! the car bodies (0x5CB31F), when g_MotionBlurEnable (0x870CDC, detail
//! above 2, capped by 0x8764F4) and 0x7FEE38 (static 1) are set.
//!
//! * Level 0x870900 steps towards a target once a frame: +0.2 (0x7FF7C0)
//!   below it, −0.1 (0x7FF7C4) above, while they differ by more than 1/255.
//!   The target is 1 while the main view's effect controller has slot 1 on
//!   (+0x18), which 0x5DBE90 sets (descriptor 0x8768B8: slots 1, 4, 5)
//!   while the player's nitrous stage (0x59FA40) is 1–3 and the car's speed
//!   +0x42C is positive, and clears (0x5DBDA0) when the stage is 0 or
//!   0x5ECDC0 is nonzero (under 5 mph, or the gas, input +0x20C, at most
//!   0.001: `audio::carfx::purging`).
//! * The camera's velocity (+0x1E0 = (position − last) / dt, 0x4474A4) by
//!   its matrix (w = 0) gives `t`; `o = t.xyz / |t|` when the speed is
//!   above 1e-5. Speed factor: 0 up to 25, 1 from 125, linear between
//!   (0x7FF7B0 / 0x7FF7B4). `amount = level·1.1 + factor`, at most 0.65
//!   (0x7FF7B8); nothing is drawn below 1/255.
//! * `k = (level·0.5 + 1)·amount·0.00625·1.2` (0x7FF7BC, 0x78A0EC); the
//!   per-corner UV shifts are `k·(o.x + o.z, o.y + o.z, o.x − o.z,
//!   o.y − o.z)` (left u, top v, right u, bottom v).
//! * The back buffer is copied to screen copy 0 (StretchRect), blurred into
//!   copy 1 by 0x5C5C10 (4 taps, ¼ each, tap i at `uv − i·shift`, the shift
//!   interpolated across the screen), back into copy 0 with 4× the shifts,
//!   and drawn over the back buffer with BLENDFACTOR `ftol(amount·255)`
//!   (BLENDFACTOR / INVBLENDFACTOR; with DOF on, INVDESTALPHA / DESTALPHA).

use glam::Vec3;

/// 0x7FF7C0 / 0x7FF7C4.
const STEP_UP: f32 = 0.2;
const STEP_DOWN: f32 = 0.1;
/// 0x7A4078 (a double).
const SETTLED: f64 = 1.0 / 255.0;
/// 0x7FF7B0 / 0x7FF7B4: the speed ramp.
const SPEED_FROM: f32 = 25.0;
const SPEED_TO: f32 = 125.0;
/// 0x7A4070.
const LEVEL_SCALE: f64 = 1.1;
/// 0x7FF7B8.
const MAX_AMOUNT: f32 = 0.65;
/// 0x784548.
const MIN_AMOUNT: f32 = 1.0 / 255.0;
/// 0x7845A0.
const MIN_SPEED: f32 = 1e-5;
/// 0x7FF7BC and 0x78A0EC.
const SHIFT: f32 = 0.00625;
const SHIFT_SCALE: f32 = 1.2;
/// 0x7A4068: the second pass's shift.
pub const SECOND_PASS: f32 = 4.0;

/// What the passes need for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blur {
    /// UV shift per tap at the left edge (u), top (v), right (u), bottom
    /// (v), for the first pass.
    pub shift: [f32; 4],
    /// BLENDFACTOR, 0–255.
    pub factor: u8,
}

/// The level 0x870900 and the camera's last position.
#[derive(Clone, Copy, Debug, Default)]
pub struct MotionBlur {
    level: f32,
    last: Option<Vec3>,
    velocity: Vec3,
}

impl MotionBlur {
    /// Dev harness (rate gate, `state_hash` `vis`): the level and the camera velocity as bits.
    #[cfg(feature = "dev-harness")]
    pub fn dev_bits(&self) -> [u32; 4] {
        [self.level.to_bits(), self.velocity.x.to_bits(), self.velocity.y.to_bits(), self.velocity.z.to_bits()]
    }

    /// The camera moved to `position` over `dt` seconds (camera +0x1E0).
    pub fn camera(&mut self, position: Vec3, dt: f32) {
        if let Some(last) = self.last
            && dt > 0.0
        {
            self.velocity = (position - last) / dt;
        }
        self.last = Some(position);
    }

    /// The camera's velocity, world units a second.
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// One frame of 0x5CC470: `nitrous` is the controller's slot 1,
    /// `axes` the camera matrix's columns (right, down, forward). The
    /// per-frame path over the tick form: one [`MotionBlur::step`], then
    /// the draw at α = 0 of the snapshot it leaves.
    pub fn frame(&mut self, nitrous: bool, axes: (Vec3, Vec3, Vec3)) -> Option<Blur> {
        self.step(nitrous);
        let s = self.snapshot();
        BlurSnapshot::drawn(&s, &s, 0.0, axes)
    }

    /// Tick form (frame-rate independence): the level's step of one tick (one 60 fps
    /// frame of 0x5CC470's 0x5CC4A0…: +0.2 below the target, −0.1 above,
    /// while they differ by more than 1/255). The velocity is stepped per
    /// tick by [`MotionBlur::camera`] already.
    pub fn step(&mut self, nitrous: bool) {
        let target = if nitrous { 1.0 } else { 0.0 };
        if f64::from((self.level - target).abs()) > SETTLED {
            if self.level < target {
                self.level += STEP_UP;
            } else {
                self.level -= STEP_DOWN;
            }
        }
    }

    /// S_n: what tick n's frame draws with (after its step: 0x5CC470 steps
    /// the level, then uses it).
    pub fn snapshot(&self) -> BlurSnapshot {
        BlurSnapshot { level: self.level, velocity: self.velocity }
    }
}

/// The blur's part of a tick snapshot (level 0x870900, camera +0x1E0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlurSnapshot {
    pub level: f32,
    pub velocity: Vec3,
}

impl BlurSnapshot {
    /// The level and velocity a frame at α between ticks `prev` and `cur`
    /// uses: interpolated, `prev` exactly at α = 0.
    pub fn lerp(prev: &BlurSnapshot, cur: &BlurSnapshot, alpha: f32) -> BlurSnapshot {
        use super::tick::lerp;
        BlurSnapshot {
            level: lerp(prev.level, cur.level, alpha),
            velocity: Vec3::new(
                lerp(prev.velocity.x, cur.velocity.x, alpha),
                lerp(prev.velocity.y, cur.velocity.y, alpha),
                lerp(prev.velocity.z, cur.velocity.z, alpha),
            ),
        }
    }

    /// The drawn frame's blur: [`blur`] of the interpolated snapshot with
    /// the drawn (interpolated) camera's axes.
    pub fn drawn(prev: &BlurSnapshot, cur: &BlurSnapshot, alpha: f32, axes: (Vec3, Vec3, Vec3)) -> Option<Blur> {
        let s = BlurSnapshot::lerp(prev, cur, alpha);
        blur(s.level, s.velocity, axes)
    }
}

/// 0x5CC4F5…0x5CC76C for level `level` and camera velocity `v`.
pub fn blur(level: f32, v: Vec3, (right, down, forward): (Vec3, Vec3, Vec3)) -> Option<Blur> {
    let speed = v.length();
    let o = if speed > MIN_SPEED {
        Vec3::new(v.dot(right), v.dot(down), v.dot(forward)).normalize_or_zero()
    } else {
        Vec3::ZERO
    };
    let ramp = if speed <= SPEED_FROM {
        0.0
    } else if speed < SPEED_TO {
        (speed - SPEED_FROM) / (SPEED_TO - SPEED_FROM)
    } else {
        1.0
    };
    let amount = ((f64::from(level) * LEVEL_SCALE) as f32 + ramp).min(MAX_AMOUNT);
    if amount < MIN_AMOUNT {
        return None;
    }
    let k = (level * 0.5 + 1.0) * amount * SHIFT * SHIFT_SCALE;
    let shift = [(o.x + o.z) * k, (o.y + o.z) * k, (o.x - o.z) * k, (o.y - o.z) * k];
    Some(Blur { shift, factor: (amount.min(1.0) * 255.0) as i32 as u8 })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXES: (Vec3, Vec3, Vec3) = (Vec3::X, Vec3::NEG_Z, Vec3::Y);

    #[test]
    fn slow_cameras_draw_nothing() {
        assert_eq!(blur(0.0, Vec3::new(0.0, 20.0, 0.0), AXES), None);
    }

    #[test]
    fn speed_ramps_the_amount() {
        let b = blur(0.0, Vec3::new(0.0, 75.0, 0.0), AXES).unwrap();
        // Half way: 0.5 → factor 127; straight ahead the shift is radial.
        assert_eq!(b.factor, 127);
        let k = 0.5 * 0.00625 * 1.2;
        assert!((b.shift[0] - k).abs() < 1e-7 && (b.shift[2] + k).abs() < 1e-7);
        assert!((b.shift[1] - k).abs() < 1e-7 && (b.shift[3] + k).abs() < 1e-7);
        // Capped at 0.65.
        assert_eq!(blur(0.0, Vec3::new(0.0, 500.0, 0.0), AXES).unwrap().factor, 165);
    }

    /// The per-frame code before the tick form (verbatim), the oracle.
    fn legacy_frame(level: &mut f32, velocity: Vec3, nitrous: bool, axes: (Vec3, Vec3, Vec3)) -> Option<Blur> {
        let target = if nitrous { 1.0 } else { 0.0 };
        if f64::from((*level - target).abs()) > SETTLED {
            if *level < target {
                *level += STEP_UP;
            } else {
                *level -= STEP_DOWN;
            }
        }
        blur(*level, velocity, axes)
    }

    fn nitrous_at(i: usize) -> bool {
        (i / 37) % 3 == 1 || i % 53 == 0
    }

    fn camera_at(i: usize) -> Vec3 {
        let t = i as f32 * 0.05;
        Vec3::new(t.sin() * 40.0 * t, t * 60.0 + (i % 7) as f32, (t * 0.3).cos())
    }

    /// 600 ticks stepped = 600 frames of the per-frame code, bit for bit;
    /// the draw at α = 0 is the old output; between ticks the level and
    /// velocity lie within the two snapshots.
    #[test]
    fn tick_form_is_the_per_frame_path() {
        let mut m = MotionBlur::default();
        let mut legacy = 0.0f32;
        let mut prev = m.snapshot();
        for i in 0..600 {
            m.camera(camera_at(i), 1.0 / 60.0);
            let want = legacy_frame(&mut legacy, m.velocity(), nitrous_at(i), AXES);
            let mut f = m.clone();
            assert_eq!(f.frame(nitrous_at(i), AXES), want, "frame {i}");
            m.step(nitrous_at(i));
            let cur = m.snapshot();
            assert_eq!(m.level.to_bits(), legacy.to_bits(), "tick {i}");
            assert_eq!(BlurSnapshot::drawn(&cur, &cur, 0.0, AXES), want);
            assert_eq!(BlurSnapshot::lerp(&prev, &cur, 0.0), prev);
            for a in crate::render::native::fx::tick::testkit::ALPHAS {
                let s = BlurSnapshot::lerp(&prev, &cur, a);
                use crate::render::native::fx::tick::within;
                assert!(within(s.level, prev.level, cur.level));
                assert!(within(s.velocity.y, prev.velocity.y, cur.velocity.y));
            }
            prev = cur;
        }
    }

    #[test]
    fn nitrous_steps_the_level() {
        let mut m = MotionBlur::default();
        for _ in 0..5 {
            m.frame(true, AXES);
        }
        assert!((m.level - 1.0).abs() < 1e-5);
        // Held still: 1.1, capped at 0.65.
        assert_eq!(m.frame(true, AXES).unwrap().factor, 165);
        m.frame(false, AXES);
        assert!((m.level - 0.9).abs() < 1e-5);
    }
}
