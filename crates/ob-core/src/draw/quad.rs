//! [`Quad`], the canvas draw unit the menus, the HUD and the renderer share, and the canvas it is laid out on.

use glam::Vec2;

use super::ui_frame::Place;

/// The canvas FEng lays out on; (0, 0) is its centre.
pub const CANVAS: Vec2 = Vec2::new(640.0, 480.0);

/// The texture corners that mark a flat [`Quad`] as a hairline: a line one
/// target pixel wide along the rectangle's middle, whatever the target's
/// size (the dyno graph's Direct3D lines, 0x5C5760).
pub const HAIRLINE_UV: [Vec2; 2] = [Vec2::new(-1.0, -1.0), Vec2::new(-1.0, -1.0)];

/// One textured, tinted rectangle on the canvas.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Texture hash, or None for a flat colour.
    pub texture: Option<u32>,
    /// Corners on the canvas (pixels, origin top left): upper left,
    /// lower right.
    pub rect: [Vec2; 2],
    pub uv: [Vec2; 2],
    pub color: [f32; 4],
    /// Draw order: larger z is farther back (FEng's buckets 0..255).
    pub z: f32,
    /// Rotation about the rectangle's centre, radians (clockwise on
    /// screen; the object's quaternion about z).
    pub angle: f32,
    /// Where it goes on a screen that is not 4:3 (clone deviation; see
    /// [`Place`] and the FEng layout).
    pub place: Place,
}
