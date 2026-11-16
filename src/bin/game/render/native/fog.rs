use crate::*;

/// The main view as the horizon fog reads it (0x5DB910, 0x5BC4B0).
pub(crate) fn fog_view(camera: &Camera, size: (u32, u32)) -> nfsu2::horizon_fog::View {
    nfsu2::horizon_fog::View {
        position: camera.position,
        forward: camera.forward(),
        view_projection: camera.view_projection(size.0 as f32 / size.1 as f32),
        size,
    }
}
