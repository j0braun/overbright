use crate::*;

/// The dyno rollers' placements: their matrices while the dyno manager
/// stands (`Game::dyno_rollers`), else hidden.
pub(crate) fn roller_poses(rollers: Option<&[glam::Mat4; 4]>) -> [(glam::Mat4, u16); 4] {
    std::array::from_fn(|i| rollers.map_or((glam::Mat4::IDENTITY, nfsu2::world::HIDDEN_FLAG), |r| (r[i], 0)))
}

/// MARKER_GPS_AID's placement (0x5EACA0's matrix towards the kept
/// look-ahead point), hidden without one.
pub(crate) fn gps_aid_transform(camera: &nfsu2::render::camera::Camera, target: Option<Vec3>) -> (glam::Mat4, u16) {
    let axes = CameraAxes::look(camera.forward(), camera.up);
    let fov = camera.game_fov.unwrap_or(camera.fov_y);
    match target {
        Some(t) => (dday::aid_matrix(camera.position, axes.right, axes.down, axes.forward, fov, t), 0),
        None => (glam::Mat4::IDENTITY, nfsu2::world::HIDDEN_FLAG),
    }
}
