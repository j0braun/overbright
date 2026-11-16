use crate::Game;
use glam::Vec2;
use nfsu2::frontend::mouse as fm;

/// The shots' mouse (`--shot mouse_*`): the screen laid out, the cursor
/// put on an object, one frame's mouse.
impl Game {
    /// Lay the screen out so the mouse tests this frame's rectangles.
    pub(crate) fn mouse_record(&self) {
        fm::begin_frame(true);
        let _ = self.menu_quads(self.now() + 2000.0);
        fm::begin_frame(true);
    }

    /// The window point at the centre of `package`'s object `hash` as last
    /// laid out (drawn or hidden).
    pub(crate) fn mouse_at(&self, package: &str, hash: u32) -> anyhow::Result<Vec2> {
        let r =
            fm::rect_any(package, hash).ok_or_else(|| anyhow::anyhow!("{package}: object {hash:08X} not laid out"))?;
        self.mouse_point(r, Vec2::splat(0.5)).ok_or_else(|| anyhow::anyhow!("no frame"))
    }

    /// One frame with the cursor moved to `p` (from elsewhere), the button
    /// pressed there (held, released the next frame) or the wheel turned.
    pub(crate) fn mouse_frame(&mut self, p: Vec2, press: bool, wheel: f32) -> anyhow::Result<()> {
        self.mouse_record();
        self.platform.mouse.last = if press || wheel != 0.0 { Some(p) } else { Some(p + Vec2::splat(7.0)) };
        self.platform.mouse.at = Some(p);
        self.platform.mouse.pressed = press;
        self.platform.mouse.held = press;
        self.platform.mouse.wheel = wheel;
        self.fe_mouse()?;
        if press {
            self.mouse_record();
            self.platform.mouse.held = false;
            self.platform.mouse.released = true;
            self.fe_mouse()?;
        }
        Ok(())
    }

    /// A drag: press at `from`, move in `steps` frames to `to` with the
    /// button held, release there.
    pub(crate) fn mouse_drag(&mut self, from: Vec2, to: Vec2, steps: usize) -> anyhow::Result<()> {
        self.mouse_record();
        self.platform.mouse.at = Some(from);
        self.platform.mouse.last = Some(from);
        self.platform.mouse.pressed = true;
        self.platform.mouse.held = true;
        self.fe_mouse()?;
        for k in 1..=steps {
            self.mouse_record();
            self.platform.mouse.at = Some(from + (to - from) * (k as f32 / steps as f32));
            self.platform.mouse.held = true;
            self.fe_mouse()?;
        }
        self.mouse_record();
        self.platform.mouse.held = false;
        self.platform.mouse.released = true;
        self.fe_mouse()?;
        Ok(())
    }
}
