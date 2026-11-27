use glam::Vec3;

use super::*;
use crate::draw::camera::Camera;

/// The showcase camera (the mover type 0x14 the factory 0x5F1FE0 builds
/// with 0x452520, vtable 0x7880D8; UI_ShowCase_Preview's constructor
/// 0x4CE770 selects it through 0x501290(1) → 0x5F1FE0(0x14), and while its
/// flag 0x85EF50 +0x1D is set 0x5FA830 keeps it). Its update 0x44EF60:
/// 0x446400 places it, the field of view is mover +0x50 degrees as a
/// binary angle (camera +0xC4) and the focus +0xB4 / +0xB8 is +0x54 /
/// +0x58. 0x446400: a ground frame at the car's position — the car's
/// first two rows with z cleared, the ground normal N under the car
/// (0x5E7E80) the third, then forward = left × N, left = N × forward,
/// both normalised — takes the point looked at from +0x40..+0x48 in that
/// frame; the eye is that point plus d·(cos a, sin a, sin p) in the frame,
/// a = +0x2C and p = +0x30 (raised to 3° when under it) in degrees as
/// binary angles, d = +0x34; it looks at the point with up +z
/// (0x5BA9B0) and the world push (0x444530) follows (`keep_out` is the
/// caller's).
pub fn showcase_view(params: &[f32; 9], position: Vec3, orientation: glam::Quat, ground_normal: Vec3) -> Camera {
    // ftol(degrees × 65536) / 360, kept to 16 bits (0x43C390 reads a word).
    let angle = |deg: f32| binary_angle(((deg * 65536.0) as i32 / 360) as u16);
    let flat = |v: Vec3| Vec3::new(v.x, v.y, 0.0);
    let n = ground_normal;
    let left = flat(orientation * Vec3::Y);
    let forward = left.cross(n).normalize_or(Vec3::X);
    let left = n.cross(forward).normalize_or(Vec3::Y);
    let frame = |v: Vec3| forward * v.x + left * v.y + n * v.z;
    let target = position + frame(Vec3::new(params[3], params[4], params[5]));
    let (sa, ca) = angle(params[0]).sin_cos();
    let pitch = if params[1] < 3.0 { 3.0 } else { params[1] };
    let sp = angle(pitch).sin();
    let d = params[2];
    let eye = target + frame(Vec3::new(ca * d, sa * d, sp * d));
    let mut camera = Camera::looking_at(eye, target);
    camera.game_fov = Some(angle(params[6]));
    camera
}
