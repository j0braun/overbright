//! The free camera in the game (docs/map-editor.md): an opt-in modding
//! aid, reachable only after a start with `--editor`, then F8 in free
//! roam. Like photo mode it stands over the explore pause menu: the world
//! stays paused under it and is drawn from the free camera with the HUD,
//! the minimap and the race UI hidden. Editing maps happens in the desktop
//! editor (`overbright-editor`); in the game the camera only looks around
//! and can put the car where it aims (G).
use crate::*;
use nfsu2::render::Camera;
use nfsu2::render::display::Place;

/// The flying speed's range (m/s, the wheel's steps) and its start.
pub(crate) const SPEED_RANGE: (f32, f32) = (0.5, 200.0);
pub(crate) const SPEED_START: f32 = 20.0;
/// One wheel notch multiplies the speed by this.
const SPEED_NOTCH: f32 = 1.25;
/// Shift and Ctrl while flying.
const FAST: f32 = 4.0;
const SLOW: f32 = 0.25;
/// The vertical field of view's range (degrees), its start and how fast
/// Z / X change it (degrees a second, photo mode's rate).
pub(crate) const FOV_RANGE: (f32, f32) = (20.0, 110.0);
const FOV_START: f32 = 60.0;
const FOV_RATE: f32 = 30.0;
/// Turning: numpad keys (radians a second) and a mouse drag (a pixel).
const TURN_RATE: (f32, f32) = (1.8, 1.4);
const DRAG_RATE: f32 = 0.004;
const MAX_PITCH: f32 = 1.5;

/// The free camera's state (`Game::free_camera`).
pub(crate) struct FreeCamera {
    pub camera: Camera,
    /// Metres a second at no modifier.
    pub speed: f32,
    /// The hint strip and the crosshair shown (H).
    pub hint: bool,
    /// The mouse at the last tick while a button was held (look).
    drag_from: Option<Vec2>,
}

impl FreeCamera {
    /// The camera at `from`'s place and direction (the game's camera when
    /// F8 was pressed), upright, at the start field of view and speed.
    pub(crate) fn new(from: Camera) -> FreeCamera {
        let mut camera = from;
        camera.game_fov = None;
        camera.fov_y = FOV_START.to_radians();
        camera.near = 0.1;
        camera.up = Vec3::Z;
        camera.jitter = Vec2::ZERO;
        FreeCamera { camera, speed: SPEED_START, hint: true, drag_from: None }
    }

    /// The field of view in whole degrees (the strip's).
    pub(crate) fn fov_degrees(&self) -> f32 {
        self.camera.fov_y.to_degrees()
    }
}

/// The held keys' move (`mv`: forward, right, up in −1..1) along the
/// camera's ground directions and up, at `speed` m/s for `dt` seconds;
/// `turn` (yaw right, pitch up in −1..1) at the keys' turn rate.
pub(crate) fn fly(cam: &mut Camera, mv: Vec3, turn: Vec2, dt: f32, speed: f32) {
    cam.yaw += turn.x * TURN_RATE.0 * dt;
    cam.pitch = (cam.pitch + turn.y * TURN_RATE.1 * dt).clamp(-MAX_PITCH, MAX_PITCH);
    let f = cam.forward();
    let flat = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::Y);
    let right = flat.cross(Vec3::Z);
    cam.position += (flat * mv.x + right * mv.y + Vec3::Z * mv.z) * speed * dt;
}

/// The speed after `notches` of the wheel (> 0 away from the player:
/// faster), held in [`SPEED_RANGE`].
pub(crate) fn wheel_speed(speed: f32, notches: f32) -> f32 {
    (speed * SPEED_NOTCH.powf(notches)).clamp(SPEED_RANGE.0, SPEED_RANGE.1)
}

/// The vertical field of view (radians) after `dt` seconds of Z (`dir`
/// −1, narrower) or X (+1, wider), held in [`FOV_RANGE`].
pub(crate) fn step_fov(fov_y: f32, dir: f32, dt: f32) -> f32 {
    (fov_y.to_degrees() + dir * FOV_RATE * dt).clamp(FOV_RANGE.0, FOV_RANGE.1).to_radians()
}

/// The ground point the camera aims at: the first step of its ray (1 m
/// steps, 200 m) that reaches the world's ground, else the ground under a
/// point 15 m ahead.
pub(crate) fn aim(cam: &Camera, world: &nfsu2::world::World) -> Vec3 {
    let f = cam.forward();
    let mut p = cam.position;
    for _ in 0..200 {
        p += f;
        if let Some(z) = world.grid_ground(p + Vec3::Z * 0.5)
            && z >= p.z - 0.5
            && (z - p.z).abs() < 3.0
        {
            return p.truncate().extend(z);
        }
    }
    let at = cam.position + f * 15.0;
    at.truncate().extend(world.grid_ground(at).unwrap_or(at.z))
}

/// The camera's heading in the game's convention (radians around +z from
/// +x), as the car is placed.
pub(crate) fn heading(cam: &Camera) -> f32 {
    let f = cam.forward();
    f.y.atan2(f.x)
}

/// What a free camera frame changed for its draw, put back after it.
pub(crate) struct FreeCameraSwap {
    camera: Option<Camera>,
    mode: Mode,
}

impl Game {
    fn held(&self, a: KeyCode, b: KeyCode) -> bool {
        self.platform.keys.contains(&a) || self.platform.keys.contains(&b)
    }

    /// F8 in free roam (with `--editor`): the pause menu under the free
    /// camera, which starts at the game's camera.
    pub(crate) fn free_camera_open(&mut self) {
        if !nfsu2::tools::map::editor_enabled()
            || self.free_camera.is_some()
            || !matches!(self.mode, Mode::Explore)
            || self.here().is_none()
        {
            return;
        }
        let Some(d) = self.sim.drive.as_ref() else { return };
        let camera = d.camera;
        self.mode = self.icon_menu(MenuKind::ExplorePause);
        self.free_camera = Some(FreeCamera::new(camera));
        log::info!("free camera opened");
    }

    /// Esc or F8: back to driving.
    pub(crate) fn free_camera_close(&mut self) -> Result<()> {
        if self.free_camera.take().is_none() {
            return Ok(());
        }
        log::info!("free camera closed");
        if matches!(self.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))) {
            self.resume_from_pause(false)?;
        }
        Ok(())
    }

    /// The free camera closes once its pause menu is gone (called with
    /// `photo_close_update`).
    pub(crate) fn free_camera_close_update(&mut self) {
        if self.free_camera.is_some() && !matches!(&self.mode, Mode::Menu(Menu::Icons(_, MenuKind::ExplorePause))) {
            self.free_camera = None;
        }
    }

    /// A key while the free camera is up (the held ones are read by
    /// [`Game::free_camera_tick`]); every other key does nothing.
    pub(crate) fn free_camera_key(&mut self, code: KeyCode) -> Result<()> {
        let Some(fc) = self.free_camera.as_mut() else { return Ok(()) };
        match code {
            KeyCode::Escape | KeyCode::F8 => return self.free_camera_close(),
            KeyCode::KeyH => fc.hint = !fc.hint,
            KeyCode::KeyG => {
                // The car to the aim point, facing the camera's way.
                let cam = fc.camera;
                let Some(d) = self.sim.drive.as_mut() else { return Ok(()) };
                let at = aim(&cam, &d.world);
                d.place(at + Vec3::Z * 0.5, heading(&cam));
                log::info!("free camera: car put at {:.1} {:.1} {:.1}", at.x, at.y, at.z);
                return self.free_camera_close();
            }
            _ => {}
        }
        Ok(())
    }

    /// A simulation step's share of the free camera: the held keys fly
    /// and turn it and change its field of view, a mouse drag turns it,
    /// the wheel sets its speed.
    pub(crate) fn free_camera_tick(&mut self, dt: f32) {
        let fast = self.held(KeyCode::ShiftLeft, KeyCode::ShiftRight);
        let slow = self.held(KeyCode::ControlLeft, KeyCode::ControlRight);
        let k = |c: KeyCode| if self.platform.keys.contains(&c) { 1.0 } else { 0.0 };
        let mv = Vec3::new(
            k(KeyCode::KeyW) - k(KeyCode::KeyS),
            k(KeyCode::KeyD) - k(KeyCode::KeyA),
            k(KeyCode::KeyR) - k(KeyCode::KeyF),
        );
        let turn = Vec2::new(k(KeyCode::Numpad6) - k(KeyCode::Numpad4), k(KeyCode::Numpad8) - k(KeyCode::Numpad2));
        let zoom = k(KeyCode::KeyX) - k(KeyCode::KeyZ);
        let m = self.platform.mouse;
        self.platform.mouse.end_frame();
        let Some(fc) = self.free_camera.as_mut() else { return };
        let drag = match (m.held || m.pressed, m.at, fc.drag_from) {
            (true, Some(at), Some(from)) => at - from,
            _ => Vec2::ZERO,
        };
        fc.drag_from = if m.held || m.pressed { m.at } else { None };
        fc.camera.yaw += drag.x * DRAG_RATE;
        fc.camera.pitch = (fc.camera.pitch - drag.y * DRAG_RATE).clamp(-MAX_PITCH, MAX_PITCH);
        fc.speed = wheel_speed(fc.speed, m.wheel);
        let speed = fc.speed * if fast { FAST } else { 1.0 } * if slow { SLOW } else { 1.0 };
        fly(&mut fc.camera, mv, turn, dt, speed);
        fc.camera.fov_y = step_fov(fc.camera.fov_y, zoom, dt);
    }

    /// Before a free camera frame's draw: its camera in, the pause menu's
    /// mode out (the world drawn as when driving). None without it.
    pub(crate) fn free_camera_begin(&mut self) -> Option<FreeCameraSwap> {
        let cam = self.free_camera.as_ref()?.camera;
        let camera = self.sim.drive.as_mut().map(|d| std::mem::replace(&mut d.camera, cam));
        let mode = std::mem::replace(&mut self.mode, Mode::Explore);
        Some(FreeCameraSwap { camera, mode })
    }

    /// After it: everything put back.
    pub(crate) fn free_camera_end(&mut self, s: FreeCameraSwap) {
        self.mode = s.mode;
        if let (Some(c), Some(d)) = (s.camera, self.sim.drive.as_mut()) {
            d.camera = c;
        }
    }

    /// The free camera's quads: the crosshair and the hint strip at the
    /// bottom (none after H). In the launcher's look A colours over a dark
    /// band, centred and scaled with the screen as the HUD is.
    pub(crate) fn free_camera_quads(&self) -> Option<Vec<Quad>> {
        let fc = self.free_camera.as_ref()?;
        let mut out = Vec::new();
        if !fc.hint {
            return Some(out);
        }
        let assets = &self.game_data.assets;
        let Some(font) = assets
            .fonts
            .get(&nfsu2::formats::hash::bin_hash("CONDUITMDITC_TT21I"))
            .or_else(|| assets.fonts.values().next())
        else {
            return Some(out);
        };
        let t = &nfsu2::launcher::draw::THEME_A;
        let rgb =
            |c: nfsu2::launcher::draw::Rgb, a: f32| [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0, a];
        let (title, ink, dim) = (rgb(t.lime, 1.0), rgb(t.ink, 1.0), rgb(t.dim, 0.8));
        let flat = |lo: Vec2, hi: Vec2, color: [f32; 4], z: f32, place| Quad {
            texture: None,
            rect: [lo, hi],
            uv: [Vec2::ZERO, Vec2::ONE],
            color,
            z,
            angle: 0.0,
            place,
        };
        // The crosshair: where G puts the car (a light dot in a dark ring).
        let c = Vec2::new(320.0, 240.0);
        out.push(flat(c - Vec2::splat(2.5), c + Vec2::splat(2.5), [0.0, 0.0, 0.0, 0.55], 0.6, Place::CENTRE));
        out.push(flat(c - Vec2::splat(1.2), c + Vec2::splat(1.2), rgb(t.ink, 0.95), 0.5, Place::CENTRE));
        // The strip: title, the two values, the keys.
        let speed = if fc.speed < 10.0 { format!("{:.1} m/s", fc.speed) } else { format!("{:.0} m/s", fc.speed) };
        let fov = format!("FOV {:.0}\u{b0}", fc.fov_degrees());
        let parts: [(&str, [f32; 4]); 13] = [
            ("Free camera", title),
            (&speed, ink),
            ("wheel", dim),
            (&fov, ink),
            ("Z / X", dim),
            ("WASD  R/F  mouse look  Shift fast  Ctrl slow", dim),
            ("G", ink),
            ("put car here", dim),
            ("H", ink),
            ("hide", dim),
            ("Esc", ink),
            ("back", dim),
            ("", dim),
        ];
        // Gaps: a word's space inside a group, a wide one between groups.
        let gap_after = [3.0, 1.0, 3.0, 1.0, 3.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 0.0, 0.0];
        let space = font.measure(" ");
        let widths: Vec<f32> = parts.iter().map(|(s, _)| font.measure(s)).collect();
        let total: f32 = widths.iter().zip(gap_after).map(|(w, g)| w + g * space).sum();
        // 0.42 × font pixels (photo mode's help), smaller when it would
        // not fit the canvas.
        let scale = 0.42f32.min(600.0 / total.max(1.0));
        let (y, pad) = (452.0, 6.0);
        let width = total * scale;
        let height = font.line_height * scale;
        let lo = Vec2::new(320.0 - width * 0.5 - pad * 2.0, y - pad);
        let hi = Vec2::new(320.0 + width * 0.5 + pad * 2.0, y + height + pad);
        let bottom = Place::new(0, 1);
        out.push(flat(lo, hi, [0.0, 0.0, 0.0, 0.62], 2.0, bottom));
        let start = out.len();
        let mut x = 320.0 - width * 0.5;
        for (((text, color), w), g) in parts.iter().zip(&widths).zip(gap_after) {
            if !text.is_empty() {
                nfsu2::frontend::text_quads(
                    assets,
                    font,
                    text,
                    0,
                    Vec2::splat(scale),
                    Vec2::new(x, y),
                    *color,
                    1.0,
                    &mut out,
                );
            }
            x += (w + g * space) * scale;
        }
        for q in &mut out[start..] {
            q.place = bottom;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flying_is_frame_rate_independent() {
        // One second of W at 30, 60, 120 and 200 steps lands in one place.
        let start = Camera::new(Vec3::new(10.0, 20.0, 5.0), 0.7, -0.2);
        let mut ends = Vec::new();
        for rate in [30, 60, 120, 200] {
            let mut cam = start;
            for _ in 0..rate {
                fly(&mut cam, Vec3::new(1.0, 0.5, -0.25), Vec2::new(0.0, 0.0), 1.0 / rate as f32, 12.0);
            }
            ends.push(cam.position);
        }
        for e in &ends {
            assert!(e.distance(ends[0]) < 1e-3, "{e} vs {}", ends[0]);
        }
        // Forward along the ground: the height changes only by R/F.
        let mut cam = start;
        fly(&mut cam, Vec3::X, Vec2::ZERO, 1.0, 12.0);
        assert!((cam.position.z - start.position.z).abs() < 1e-5);
        assert!((cam.position.distance(start.position) - 12.0).abs() < 1e-3);
    }

    #[test]
    fn the_wheel_and_z_x_stay_in_range() {
        assert!((wheel_speed(20.0, 1.0) - 25.0).abs() < 1e-4);
        assert!((wheel_speed(20.0, -1.0) - 16.0).abs() < 1e-4);
        assert_eq!(wheel_speed(20.0, 100.0), SPEED_RANGE.1);
        assert_eq!(wheel_speed(20.0, -100.0), SPEED_RANGE.0);
        let fov = 60f32.to_radians();
        assert!((step_fov(fov, 1.0, 0.5).to_degrees() - 75.0).abs() < 1e-3);
        assert!((step_fov(fov, -1.0, 100.0).to_degrees() - FOV_RANGE.0).abs() < 1e-3);
        assert!((step_fov(fov, 1.0, 100.0).to_degrees() - FOV_RANGE.1).abs() < 1e-3);
    }

    #[test]
    fn the_heading_follows_the_view() {
        // Yaw 0 looks down +y: the game's heading is 90°.
        let cam = Camera::new(Vec3::ZERO, 0.0, -0.3);
        assert!((heading(&cam) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        let cam = Camera::new(Vec3::ZERO, std::f32::consts::FRAC_PI_2, 0.0);
        assert!(heading(&cam).abs() < 1e-5);
    }
}
