use crate::*;

/// One frame of motion blur (0x5CC470) for the player's view. Slot 1 of
/// the view's controller is on (0x5DBE90) while the nitrous stage is 1–3,
/// the speed +0x42C is positive and 0x5ECDC0 gives 0 (at least 5 mph
/// with the gas on, `carfx::purging`).
pub(crate) fn motion_blur_frame(
    blur: &nfsu2::motion_blur::BlurSnapshot,
    camera: &Camera,
) -> Option<nfsu2::motion_blur::Blur> {
    // The level and the camera's velocity as the ticks left them
    // (`render::visuals`: the level stepped on the tick, the nitrous test
    // `nitrous_blur` there), with the frame's camera axes.
    let axes = CameraAxes::look(camera.forward(), camera.up);
    nfsu2::motion_blur::BlurSnapshot::drawn(blur, blur, 0.0, (axes.right, axes.down, axes.forward))
}
