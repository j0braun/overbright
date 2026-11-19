//! The aspect mode of a screen that is not 4:3 and the player views'
//! projection factors it gives (`render::display` re-exports them; the
//! camera, `draw::camera`, computes with them).

use std::sync::atomic::{AtomicU8, Ordering};

/// How a screen wider (or narrower) than 4:3 is filled. The game is 4:3
/// only: its front end and HUD canvas (640 × 480) stretch over the whole
/// target (0x5C5030, 0x5BF760) and the player views keep their horizontal
/// angle (0x5C7ED0: m00 fixed, m11 from h/w), so a wide screen loses
/// height ("Vert−").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AspectMode {
    /// The default (agreed deviation): the 3D views keep the vertical
    /// angle they have at 4:3 and see more at the sides ("Hor+"; at 4:3
    /// exactly the native projection), and every front-end / HUD element
    /// keeps its 4:3 size and shape, placed by its `Place` (`render::display`): centred, or
    /// moved out to the screen edge it belongs to; flat full-screen quads
    /// (fades, bars) fill the screen. Narrower than 4:3 (5:4) the views
    /// keep the native horizontal angle and the elements move up / down
    /// instead.
    Wide = 0,
    /// The native behaviour (the original's look): the canvas stretched,
    /// the native projection.
    Stretched = 1,
}

static ASPECT: AtomicU8 = AtomicU8::new(0);

pub fn set_aspect_mode(mode: AspectMode) {
    ASPECT.store(mode as u8, Ordering::Relaxed);
}

pub fn aspect_mode() -> AspectMode {
    if ASPECT.load(Ordering::Relaxed) == 1 { AspectMode::Stretched } else { AspectMode::Wide }
}

/// The player views' projection for a target of `aspect` (w / h): the
/// h / w 0x5C7ED0 is given, and the factor its x scale (m00) is then
/// multiplied by. Native ([`AspectMode::Stretched`]): (1 / aspect, 1).
/// Wide: the 4:3 projection (h / w = 0.75) with m00 times (4/3) / aspect
/// at every aspect, i.e. the vertical angle is always the 4:3 one. Wider
/// than 4:3 that is Hor+ (the centre 4:3 of the picture is the native
/// one); narrower (5:4, portrait) the sides of the 4:3 view are cropped
/// (docs/resolutions.md). The native Vert− there grew the vertical angle by (4/3) /
/// aspect (≈ 161° at 9:16), and the road under the chase camera then
/// came nearer than the 0.5 m near plane: a band of clear colour at the
/// bottom (docs/resolutions.md, F1).
pub fn view_aspect(aspect: f32) -> (f32, f32) {
    if aspect_mode() == AspectMode::Wide { (0.75, 4.0 / 3.0 / aspect) } else { (1.0 / aspect, 1.0) }
}
