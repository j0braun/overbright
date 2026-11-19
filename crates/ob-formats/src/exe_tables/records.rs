//! The record types of the engine's tables that other parts of the game
//! read (`audio::engine_chain`, `audio::car`, `garage`,
//! `render::native::car::lighting` re-export them): plain values, defined
//! here so the tables need nothing above them.

use glam::Vec3;

use crate::draw::camera::Camera;

#[derive(Clone, Copy, Debug)]
pub struct Rec80 {
    pub gin: &'static str,
    pub hybrid: bool,
    pub decel: usize,
    pub rpm_max: f32,
    pub rpm_min: f32,
    pub vol_a: i32,
    pub vol_b: i32,
    pub d0: f32,
    pub a1: f32,
    pub b1: f32,
    pub a0: f32,
    pub b0: f32,
    pub fixed_a: f32,
    pub fixed_b: f32,
    pub cutoff: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Rec84 {
    pub gin: &'static str,
    pub vol_a: i32,
    pub vol_d: i32,
    pub rpm0: i32,
    pub rpm1: i32,
    pub up: f32,
    pub down: f32,
    pub d0: f32,
    pub a1: f32,
    pub b1: f32,
    pub a0: f32,
    pub b0: f32,
}

/// The tuning records 0x7F2F20 + 0x48·k (18 dwords; +0x18, +0x1C,
/// +0x30, +0x34 ints).
#[derive(Clone, Copy, Debug)]
pub struct SkidRecord {
    pub long_scale: f32,
    pub long_hi: f32,
    pub long_lo: f32,
    pub lat_scale: f32,
    pub lat_hi: f32,
    pub lat_lo: f32,
    pub l3_up: i32,
    pub l3_down: i32,
    pub l3_mix: f32,
    pub wheel_threshold: f32,
    pub hold: f32,
    pub l2_up: i32,
    pub l2_down: i32,
    pub l2_mix: f32,
    pub off_side: f32,
    pub angle_in0: f32,
    pub angle_in1: f32,
}

/// A shift record (0x7F2860 + 0x60 · n; the bank names and volumes are
/// `carfx::SHIFT_BANKS`). Times in ms unless marked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShiftRec {
    /// +8 the up-shift clunk delay (s); +0xC / +0x10 the rpm fall of the
    /// up-shift and its time; +0x14 / +0x18 the rpm added on engaging and
    /// the time it decays to the engine's; +0x1C / +0x20 the volume bump
    /// time and size (f32, × 32767 onto Engine +0xE0).
    pub up_delay: f32,
    pub up_drop: i32,
    pub up_drop_ms: i32,
    pub up_add: i32,
    pub up_add_ms: i32,
    pub bump_ms: i32,
    pub bump: f32,
    /// +0x24 / +0x28 / +0x2C the volume wobble's amplitude, period and
    /// decay time; +0x30 / +0x34 / +0x38 the rpm wobble's.
    pub wob_vol: i32,
    pub wob_vol_period: i32,
    pub wob_vol_ms: i32,
    pub wob_rpm: i32,
    pub wob_rpm_period: i32,
    pub wob_rpm_ms: i32,
    /// +0x40 the down-shift clunk delay (s); +0x44 / +0x48 the rpm fall
    /// and its time; +0x4C / +0x50 the throttle blip's rpm rise and time;
    /// +0x54 / +0x58 the fall after it; +0x5C (f32) the settle time per
    /// rpm of difference.
    pub down_delay: f32,
    pub down_drop: i32,
    pub down_drop_ms: i32,
    pub down_add: i32,
    pub down_blip_ms: i32,
    pub down_fall: i32,
    pub down_fall_ms: i32,
    pub settle: f32,
}

/// One of GarageMain's two camera set-ups (0x7F608C + 0x30·n; the
/// constructor takes record +0x55, 0, and 0x4D82D0 switches to record 1
/// for an SUV, car type +0x88A).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    /// +0x00: the turntable angle in degrees (GarageMain +0x274, through
    /// +0x9D·4).
    pub car_angle: f32,
    /// +0x04: the front wheels' steer in degrees (0x7F45A4).
    pub steer: f32,
    /// +0x08: the field of view in degrees (camera +0x80 → +0xC4).
    pub fov: f32,
    /// +0x0C: what the camera looks at (+0x90).
    pub target: Vec3,
    /// +0x18: the eye (+0xA0).
    pub eye: Vec3,
}

impl Shot {
    /// The garage camera: a look-at with z up (0x444DE0: right =
    /// (0, 0, −1) × forward, no roll while +0xB8 is 0) and view 1's
    /// projection of the field of view (`Camera::game_fov`).
    pub fn camera(&self) -> Camera {
        let fov = self.fov.to_radians();
        let mut camera = Camera::look_at(self.eye, self.target, Vec3::Z, fov);
        camera.game_fov = Some(fov);
        camera
    }
}

/// One of a lighting setup's three directional lights (+0x20 + i·0x1C:
/// flag, θ°, φ°, red, green, blue, intensity).
#[derive(Clone, Copy, Debug)]
pub struct RigLight {
    /// Fixed to the camera rather than the world.
    pub camera_relative: bool,
    pub theta: f32,
    pub phi: f32,
    pub colour: Vec3,
    pub intensity: f32,
}
