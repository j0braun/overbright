use crate::*;

/// Which of the car's five settings (UI_PerformanceTuning_Setting's
/// Circuit, Drag, Drift, Street X, URL Race) a career race loads: 0x532401
/// by the event's type (+0x0F, table 0x5325B4): drag 1, drift 2, street X
/// 3, circuit, sprint and the rest 0 — URL races too (the category is not
/// read), so no race takes URL Race's. Free roam takes 0 (0x540691).
pub(crate) fn tuning_mode_for(kind: career::EventKind) -> usize {
    match kind {
        career::EventKind::Drag => 1,
        career::EventKind::Drift => 2,
        career::EventKind::StreetX => 3,
        _ => 0,
    }
}

/// `drive_in`'s tuning index for the dyno's own sliders (the test run).
pub(crate) const DYNO_SLIDERS: usize = usize::MAX;

/// The test run per tuning mode (0x526980's table 0x526C0C): track, race
/// flags and laps (0x89E7A0 +0, the flags, +0x1C).
/// The dyno track (0x4FF160: 4706 "Speedway Dyno Testing").
pub(crate) const DYNO_TRACK: u16 = 4706;

/// The dyno camera 0x15 (constructor 0x452600, vtable 0x788070, update
/// 0x44F300) as the tuning menus show it: an orbit about the car (its
/// +0xD0 orientation and +0x50 position, taken once) at distance +0x2C =
/// 7.2 (zoom factor +0x3C 1, clamped to 5.0…7.2), yaw +0x50 = 0xE270 and
/// pitch +0x40 = 0x70A0 (turned by 0x8000; clamped 0x70A0…0x8150), their
/// rates +0x58 / +0x48 zero (and zeroed while the dyno manager's state is
/// 0), field of view 8000 (+0xC4, 0x1F40). The chain: RotZ(yaw) (0x5BAE70),
/// RotY(pitch + 0x8000) (0x5BADB0), a step of the distance along x
/// (0x5BAEA0), the axis swap 0x4000 / 0xC000: the eye 7.2 behind the car,
/// 41.6° to its right and 21.6° up, looking at its position.
/// **Measured** under Wine (menus112, the mover read through player
/// 0x8900B4 +0x238: vtable 0x788070, camera 0x8760E0): car (−1682.08,
/// −1957.33, 43.39) heading 120°, eye (−1675.75, −1959.47, 46.04) — 7.19
/// away, 21.6° up, 41.3° off the car's back to its right, looking at the
/// car; +0xC4 = 8000.
/// The dyno camera's distance range (0x44F300 keeps +0x3C's product
/// within 5.0 … 7.2; it starts at the far end).
pub(crate) const DYNO_DISTANCE: (f32, f32) = (5.0, 7.2);

pub(crate) fn dyno_camera(position: Vec3, orientation: glam::Quat, distance: f32) -> nfsu2::render::Camera {
    let turn = |b: u16| b as i16 as f32 * std::f32::consts::TAU / 65536.0;
    let yaw = turn(0xE270);
    let pitch = turn(0x70A0u16.wrapping_add(0x8000));
    let target = position;
    let forward = (orientation * Vec3::X).normalize_or(Vec3::X);
    let left = (orientation * Vec3::Y).normalize_or(Vec3::Y);
    let up = (orientation * Vec3::Z).normalize_or(Vec3::Z);
    let back = -forward * yaw.cos() + left * yaw.sin();
    let eye = target + (back * pitch.cos() - up * pitch.sin()) * distance;
    let fov = 8000.0 * std::f32::consts::TAU / 65536.0;
    let mut camera = nfsu2::render::Camera::look_at(eye, target, Vec3::Z, fov);
    camera.game_fov = Some(fov);
    camera
}

pub(crate) const TEST_RUNS: [(u16, RaceMode, u32); 5] = [
    (4705, RaceMode::FreeRoam, 10),
    (4709, RaceMode::Drag, 1),
    (4708, RaceMode::Drift, 10),
    (4708, RaceMode::StreetX, 10),
    (4705, RaceMode::FreeRoam, 10),
];
