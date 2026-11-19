//! A free-flying camera in the game's Z-up world coordinates.

use glam::{Mat4, Vec2, Vec3};

/// 0x5C7ED0's angle factors for the player views (0x7A27E4, and 0x7A27E8
/// over 0x7A27E0).
const GAME_FOV_X: f32 = 1.1;
const GAME_FOV_Y: f32 = 1.1 / 0.869_565_2;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: Vec3,
    /// Heading around +Z, radians; 0 looks down +Y.
    pub yaw: f32,
    /// Up/down, radians; positive looks up.
    pub pitch: f32,
    pub fov_y: f32,
    /// A game camera's field of view (radians, from its binary angle at
    /// camera +0xC4). When set it replaces `fov_y`: 0x5C7ED0 builds the
    /// projection of views 1, 2, 4 and 5 from half-angles, not tangents —
    /// `x = 1 / tan(fov·1.1 / 2)`, `y = 1 / tan(fov·(1.1 / 0.8696)·(h/w) / 2)`
    /// (0x7A27E4 / 0x7A27E0).
    pub game_fov: Option<f32>,
    /// Which way is up on screen; +Z except when a chase camera leans.
    pub up: Vec3,
    pub near: f32,
    pub far: f32,
    /// The temporal pass's sub-pixel offset (docs/temporal.md), in
    /// clip space (NDC units): the projection is moved by it. Zero
    /// (native) everywhere but the main view's draw with TAA.
    pub jitter: Vec2,
}

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Camera {
        Camera { position, yaw, pitch, fov_y: 60f32.to_radians(), game_fov: None, up: Vec3::Z, near: 0.5, far: 20_000.0, jitter: Vec2::ZERO }
    }

    pub fn looking_at(eye: Vec3, target: Vec3) -> Camera {
        let d = (target - eye).normalize();
        Camera::new(eye, d.x.atan2(d.y), d.z.asin())
    }

    /// A camera at `eye` looking at `target`, with `up` leaning the horizon.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3, fov_y: f32) -> Camera {
        let mut camera = Camera::looking_at(eye, target);
        camera.up = up.normalize_or(Vec3::Z);
        camera.fov_y = fov_y;
        camera
    }

    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(sy * cp, cy * cp, sp)
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Z).normalize()
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward(), self.up)
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_between(aspect, self.near, self.far) * self.view()
    }

    /// [`Camera::view_projection`] for a scene drawn with reversed depth
    /// when `reversed` (the Graphics page's Reversed-Z, clone deviation):
    /// the same frustum with the near plane at depth 1 and the far one at
    /// 0, built directly (not as `1 − z` of the matrix above, whose rows
    /// would cancel in f32).
    pub fn view_projection_depth(&self, aspect: f32, reversed: bool) -> Mat4 {
        if !reversed {
            return self.view_projection(aspect);
        }
        self.projection_between(aspect, self.far, self.near) * self.view()
    }

    /// The projection alone (with the jitter), depth reversed when
    /// `reversed`.
    pub fn projection_depth(&self, aspect: f32, reversed: bool) -> Mat4 {
        if reversed { self.projection_between(aspect, self.far, self.near) } else { self.projection_between(aspect, self.near, self.far) }
    }

    /// The projection's x and y scales (m00, m11) at `aspect`.
    pub fn projection_scale(&self, aspect: f32) -> (f32, f32) {
        let p = self.projection_between(aspect, self.near, self.far);
        (p.x_axis.x, p.y_axis.y)
    }

    /// The projection with `near` mapped to depth 0 and `far` to 1 (either
    /// way round).
    fn projection_between(&self, aspect: f32, near: f32, far: f32) -> Mat4 {
        let mut proj = Mat4::perspective_rh(self.fov_y, aspect, near, far);
        if let Some(fov) = self.game_fov {
            // The h / w and m00 factor of the aspect mode (Hor+ by
            // default, `display::view_aspect`; native at 4:3).
            let (hw, across) = super::aspect::view_aspect(aspect);
            proj.x_axis.x = across / (fov * GAME_FOV_X * 0.5).tan();
            proj.y_axis.y = 1.0 / (fov * GAME_FOV_Y * hw * 0.5).tan();
        }
        #[cfg(feature = "dev-harness")]
        projections::record(aspect, self.game_fov, self.fov_y, proj.x_axis.x, proj.y_axis.y, (GAME_FOV_X, GAME_FOV_Y));
        if self.jitter != Vec2::ZERO {
            // x' = x + j·w: the image moved by j in NDC at every depth.
            proj = Mat4::from_translation(self.jitter.extend(0.0)) * proj;
        }
        proj
    }

    /// The camera without the temporal jitter.
    pub fn unjittered(&self) -> Camera {
        Camera { jitter: Vec2::ZERO, ..*self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_depth_keeps_the_frustum() {
        let mut c = Camera::new(Vec3::new(2500.0, -1800.0, 30.0), 0.7, -0.1);
        c.game_fov = Some(1.2);
        let (a, b) = (c.view_projection(16.0 / 9.0), c.view_projection_depth(16.0 / 9.0, true));
        assert_eq!(c.view_projection_depth(16.0 / 9.0, false), a);
        for d in [0.5f32, 1.0, 10.0, 300.0, 5000.0, 19_999.0] {
            let p = c.position + c.forward() * d + c.right() * (d * 0.3);
            let (n, r) = (a * p.extend(1.0), b * p.extend(1.0));
            // Same x, y and w: the same pixel and the same clipping.
            assert!((n.x - r.x).abs() <= 1e-3 * n.w.abs() && (n.y - r.y).abs() <= 1e-3 * n.w.abs() && (n.w - r.w).abs() <= 1e-3 * n.w.abs());
            // Depth reversed: z_r / w = 1 − z_n / w.
            assert!((r.z / r.w - (1.0 - n.z / n.w)).abs() < 1e-3, "{d}: {} {}", r.z / r.w, n.z / n.w);
        }
    }
}

/// Developer harness only (docs/resolutions.md): with `NFSU2_UI_GEOM` set,
/// every game-FOV projection the 3D views use is counted here, and the UI
/// geometry dump (`render::native::ui::geom_dump`) takes them with each
/// dump. Without the variable nothing is collected.
#[cfg(feature = "dev-harness")]
pub mod projections {
    use std::sync::Mutex;

    /// (aspect, fov, hw, across, m00, m11, count).
    pub type Projection = (f32, f32, f32, f32, f32, f32, u32);

    /// The projections since the last [`take`] and the camera's x / y FOV
    /// factors; None before the first one.
    static LOG: Mutex<Option<(Vec<Projection>, (f32, f32))>> = Mutex::new(None);

    /// A camera projection: the aspect it was asked for, the game camera's
    /// angle (`game_fov`; None: a free camera's vertical angle `fov_y`,
    /// photo mode), the matrix's m00, m11; `consts` the camera's x / y FOV
    /// factors. For a game camera `aspect::view_aspect`'s (h / w, m00
    /// factor) are recorded too.
    pub fn record(aspect: f32, game_fov: Option<f32>, fov_y: f32, m00: f32, m11: f32, consts: (f32, f32)) {
        if crate::env_value_os!("NFSU2_UI_GEOM").is_none() {
            return;
        }
        let Ok(mut g) = LOG.lock() else { return };
        let (list, fov_consts) = g.get_or_insert_with(Default::default);
        *fov_consts = consts;
        let (fov, hw, across) = match game_fov {
            Some(f) => {
                let (hw, across) = super::super::aspect::view_aspect(aspect);
                (f, hw, across)
            }
            // A free camera: fov negative, its vertical angle.
            None => (-fov_y, 0.0, 0.0),
        };
        let key = (aspect, fov, hw, across, m00, m11);
        match list.iter_mut().find(|p| (p.0, p.1, p.2, p.3, p.4, p.5) == key) {
            Some(p) => p.6 += 1,
            None => list.push((aspect, fov, hw, across, m00, m11, 1)),
        }
    }

    /// The projections recorded since the last call (the list starts anew)
    /// and the FOV factors; None when none was ever recorded.
    pub fn take() -> Option<(Vec<Projection>, (f32, f32))> {
        let mut g = LOG.lock().ok()?;
        let (list, consts) = g.as_mut()?;
        Some((std::mem::take(list), *consts))
    }
}
