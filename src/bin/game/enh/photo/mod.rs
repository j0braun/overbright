//! Photo mode in the game (docs/playing/photo-mode.md; clone deviation, opt-in
//! through the Enhancements row "Photo Mode in Pause Menu"). The pause menu
//! stays the mode underneath (the world paused exactly as under it: its
//! updates run as before, only the menu's mouse is not fed); photo mode
//! draws the world from its own camera with the HUD and the menu hidden,
//! its help bar instead, and puts the game's camera and settings back after
//! every frame. Logic without the GPU: `nfsu2::photo`.
use crate::*;
use nfsu2::photo::{self as ph, Look, Rig, Row};

mod capture;
mod ui;
#[cfg(feature = "dev-harness")]
pub(crate) use crate::dev::photo_check::photo_check;

/// Photo mode's state (`Game::photo`).
pub(crate) struct Photo {
    pub rig: Rig,
    pub look: Look,
    /// The settings strip's selected row.
    pub selected: usize,
    /// The help bar shown.
    pub help: bool,
    /// A capture drawing: no help bar, no cursor.
    pub capturing: bool,
    /// Its grade is `RenderState.photo_grades[id]`.
    pub id: DrawId,
    /// The rig as photo mode opened (C resets to it).
    start: Rig,
    /// The mouse at the last frame while the button was held.
    drag_from: Option<Vec2>,
    /// A line under the strip (the last picture saved) and until when (s).
    pub status: Option<(String, f32)>,
    /// Pictures being written.
    pub saving: Vec<std::thread::JoinHandle<Result<std::path::PathBuf>>>,
    /// Where the pictures go (the player's data dir's Screenshots).
    pub dir: std::path::PathBuf,
}

/// What a photo frame changed for its draw, put back after it.
pub(crate) struct Swapped {
    camera: Option<nfsu2::render::Camera>,
    mode: Option<Mode>,
    custom: [u8; 22],
    rear_view_on: bool,
}

impl Swapped {
    /// Photo mode's camera is in for this frame (the frame draws
    /// it, not the blend between the ticks).
    pub(crate) fn camera_swapped(&self) -> bool {
        self.camera.is_some()
    }
}

/// The car's centre the rig turns about.
pub(crate) fn car_centre(d: &Drive) -> Vec3 {
    d.state.position + d.state.orientation * Vec3::new(0.0, 0.0, 0.5)
}

impl Game {
    /// The pause menu's "Photo Mode": the rig from the game's camera.
    pub(crate) fn photo_open(&mut self) {
        let Some(d) = self.sim.drive.as_ref() else { return };
        let aspect = self.platform.window.as_ref().map_or(4.0 / 3.0, |w| w.config.width as f32 / w.config.height.max(1) as f32);
        let rig = Rig::from_camera(&d.camera, car_centre(d), aspect);
        let dir = nfsu2::platform::dirs::screenshots();
        self.photo = Some(Photo {
            rig,
            look: Look::default(),
            selected: ph::Row::ALL.iter().position(|r| *r == Row::Aperture).unwrap_or(0),
            help: true,
            capturing: false,
            id: DrawId::new(),
            start: rig,
            drag_from: None,
            status: None,
            saving: Vec::new(),
            dir,
        });
        log::info!("photo mode opened");
    }

    /// Back to the pause menu (Esc / B), the pictures written first.
    pub(crate) fn photo_close(&mut self) {
        if let Some(p) = self.photo.take() {
            for h in p.saving {
                let _ = written(h);
            }
        }
        nfsu2::render::blur_dof::set_photo_mode(false);
        log::info!("photo mode closed");
    }

    /// A key while photo mode is up (pressed; the held ones are read by
    /// [`Game::photo_tick`]).
    pub(crate) fn photo_key(&mut self, code: KeyCode) -> Result<()> {
        let Some(p) = self.photo.as_mut() else { return Ok(()) };
        let row = Row::ALL[p.selected];
        match code {
            KeyCode::Escape => self.photo_close(),
            KeyCode::ArrowUp => p.selected = (p.selected + Row::ALL.len() - 1) % Row::ALL.len(),
            KeyCode::ArrowDown => p.selected = (p.selected + 1) % Row::ALL.len(),
            KeyCode::ArrowLeft => ph::step(&mut p.rig, &mut p.look, row, -1),
            KeyCode::ArrowRight => ph::step(&mut p.rig, &mut p.look, row, 1),
            KeyCode::KeyH | KeyCode::Period => p.help = !p.help,
            KeyCode::KeyC => p.rig = p.start,
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::F12 => {
                // A picture that cannot be made is a line under the strip;
                // photo mode and the game go on.
                if let Err(e) = self.photo_capture() {
                    log::warn!("photo mode: picture not taken: {e:#}");
                    let until = now_ms(self.pacing.started) / 1000.0 + 6.0;
                    if let Some(p) = self.photo.as_mut() {
                        p.status = Some((format!("Not saved: {e}"), until));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// A simulation step's share of photo mode (60 a second, the pause's
    /// own steps): the held keys, the mouse drag and wheel, the sticks.
    pub(crate) fn photo_tick(&mut self, dt: f32) {
        let Some(p) = self.photo.as_mut() else { return };
        let k = |c: KeyCode| if self.platform.keys.contains(&c) { 1.0 } else { 0.0 };
        let mut mv = Vec3::new(
            k(KeyCode::KeyW) - k(KeyCode::KeyS),
            k(KeyCode::KeyD) - k(KeyCode::KeyA),
            k(KeyCode::KeyR) - k(KeyCode::KeyF),
        );
        let mut turn = (k(KeyCode::KeyL) - k(KeyCode::KeyJ), k(KeyCode::KeyI) - k(KeyCode::KeyK));
        let mut dolly = k(KeyCode::BracketLeft) + k(KeyCode::PageUp) + k(KeyCode::Equal)
            - k(KeyCode::BracketRight)
            - k(KeyCode::PageDown)
            - k(KeyCode::Minus);
        let roll = k(KeyCode::KeyE) - k(KeyCode::KeyQ);
        let fov = k(KeyCode::KeyX) - k(KeyCode::KeyZ);
        // The controllers: left stick moves, right stick orbits, the
        // triggers move up and down (dead zone 0.15).
        let dz = |v: f32| if v.abs() < 0.15 { 0.0 } else { v };
        for d in &self.platform.pads.devices {
            let [lx, ly, rx, ry] = d.inputs.sticks;
            mv += Vec3::new(dz(ly), dz(lx), d.inputs.triggers[1] - d.inputs.triggers[0]);
            turn.0 += dz(rx);
            turn.1 += dz(ry);
            // L1 / R1 (the FE map's [ / ]) as held buttons
            dolly += f32::from(u8::from(d.inputs.buttons[nfsu2::pad::host::Btn::L1 as usize]))
                - f32::from(u8::from(d.inputs.buttons[nfsu2::pad::host::Btn::R1 as usize]));
        }
        // The mouse: a drag orbits (0.005 rad a pixel), the wheel dollies.
        let m = self.platform.mouse;
        self.platform.mouse.end_frame();
        let drag = match (m.held || m.pressed, m.at, p.drag_from) {
            (true, Some(at), Some(from)) => at - from,
            _ => Vec2::ZERO,
        };
        p.drag_from = if m.held || m.pressed { m.at } else { None };
        p.rig.yaw -= drag.x * 0.005;
        p.rig.pitch = (p.rig.pitch + drag.y * 0.005).clamp(-ph::MAX_PITCH, ph::MAX_PITCH);
        p.rig.distance = (p.rig.distance * (-m.wheel * 0.12).exp()).clamp(ph::MIN_DISTANCE, ph::MAX_DISTANCE);
        p.rig.drive(
            mv.clamp(Vec3::splat(-1.0), Vec3::splat(1.0)),
            (turn.0.clamp(-1.0, 1.0), turn.1.clamp(-1.0, 1.0)),
            dolly.clamp(-1.0, 1.0),
            dt,
        );
        p.rig.roll = (p.rig.roll + roll * 30.0 * dt).clamp(-ph::MAX_ROLL, ph::MAX_ROLL);
        p.rig.fov = (p.rig.fov + fov * 30.0 * dt).clamp(ph::FOV_RANGE.0, ph::FOV_RANGE.1);
        // The pictures written: the last one's name under the strip.
        // Wall time: the status line follows the save threads (presentation).
        let now = now_ms(self.pacing.started) / 1000.0;
        let mut i = 0;
        while i < p.saving.len() {
            if p.saving[i].is_finished() {
                let text = match p.saving.remove(i).join() {
                    Ok(Ok(path)) => format!(
                        "Saved {}",
                        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                    ),
                    Ok(Err(e)) => format!("Not saved: {e}"),
                    Err(_) => "Not saved".into(),
                };
                p.status = Some((text, now + 4.0));
            } else {
                i += 1;
            }
        }
        if p.status.as_ref().is_some_and(|(_, until)| now > *until) {
            p.status = None;
        }
    }

    /// The camera photo mode draws from: the rig held by the radius and
    /// the world's collision.
    #[cfg(any(test, feature = "dev-harness"))]
    pub(crate) fn photo_camera(&self) -> Option<nfsu2::render::Camera> {
        self.photo_camera_with(None)
    }

    /// [`Game::photo_camera`] from `rig` (the frame's, between
    /// the ticks), else photo mode's own.
    pub(crate) fn photo_camera_with(&self, rig: Option<Rig>) -> Option<nfsu2::render::Camera> {
        let (p, d) = (self.photo.as_ref()?, self.sim.drive.as_ref()?);
        let rig = rig.unwrap_or(p.rig);
        let centre = car_centre(d);
        let reach = Vec2::splat(ph::MAX_RADIUS + 1.0);
        let faces: Vec<[Vec3; 3]> = d
            .world
            .collision()
            .triangles_near(centre.truncate() - reach, centre.truncate() + reach)
            .into_iter()
            .map(|t| t.1)
            .collect();
        let (pivot, eye) = rig.place(centre, &faces);
        Some(rig.camera(pivot, eye))
    }

    /// Before a photo frame's draw: its camera in, the pause menu's mode
    /// out (the world drawn as when driving: its traffic and racers), the
    /// car shadow and light glow as the strip says, the depth of field's
    /// values. None outside photo mode. `rig`: the frame's rig between
    /// the ticks (None: photo mode's own).
    pub(crate) fn photo_begin(&mut self, rig: Option<Rig>) -> Option<Swapped> {
        let camera = self.photo_camera_with(rig);
        let p = self.photo.as_ref()?;
        let look = p.look;
        nfsu2::render::blur_dof::set_photo_mode(true);
        nfsu2::render::blur_dof::set_photo_dof(look.focus, look.aperture);
        let custom = self.settings.custom;
        if !look.car_shadow {
            self.settings.custom[2] = 0;
        }
        if !look.light_glow {
            self.settings.custom[9] = 0;
        }
        if !look.speed_blur {
            self.settings.custom[11] = 0;
        }
        let saved_camera = match (camera, self.sim.drive.as_mut()) {
            (Some(c), Some(d)) => Some(std::mem::replace(&mut d.camera, c)),
            _ => None,
        };
        let world = if let Some(r) = self.sim.paused_race.take() { Mode::Race(r) } else { Mode::Explore };
        let mode = Some(std::mem::replace(&mut self.mode, world));
        Some(Swapped { camera: saved_camera, mode, custom, rear_view_on: std::mem::take(&mut self.hud_state.rear_view_on) })
    }

    /// After it: everything put back.
    pub(crate) fn photo_end(&mut self, s: Swapped) {
        if let Some(m) = s.mode {
            match std::mem::replace(&mut self.mode, m) {
                Mode::Race(r) => self.sim.paused_race = Some(r),
                _ => {}
            }
        }
        if let (Some(c), Some(d)) = (s.camera, self.sim.drive.as_mut()) {
            d.camera = c;
        }
        self.settings.custom = s.custom;
        self.hud_state.rear_view_on = s.rear_view_on;
    }
}

/// Write a capture as `<dir>/<stamp>[-n].png` (the first free name; the
/// folder made first) on the writer thread. A failure (a full disk, a
/// read-only or missing folder) is the thread's error, logged when it is
/// joined ([`written`]); the game goes on either way.
fn write_picture(dir: &std::path::Path, stamp: &str, pixels: &[u8], size: (u32, u32)) -> Result<std::path::PathBuf> {
    // The encoder panics on a buffer of another size (image 0.25's
    // `write_image`), after making the file: checked first. The read-back
    // gives exactly this size, so this is a guard only.
    anyhow::ensure!(
        pixels.len() as u64 == u64::from(size.0) * u64::from(size.1) * 4,
        "{} bytes for a {}x{} picture",
        pixels.len(),
        size.0,
        size.1
    );
    std::fs::create_dir_all(dir)?;
    let mut n = 0;
    let path = loop {
        let path = dir.join(ph::file_name(stamp, n));
        if !path.exists() {
            break path;
        }
        n += 1;
    };
    image::save_buffer(&path, pixels, size.0, size.1, image::ColorType::Rgba8)?;
    log::info!("photo mode: wrote {} ({}x{})", path.display(), size.0, size.1);
    Ok(path)
}

/// A writer thread's picture, or None with the reason logged (the write
/// failed or the thread died).
fn written(h: std::thread::JoinHandle<Result<std::path::PathBuf>>) -> Option<std::path::PathBuf> {
    match h.join() {
        Ok(Ok(path)) => Some(path),
        Ok(Err(e)) => {
            log::warn!("photo mode: picture not written: {e:#}");
            None
        }
        Err(_) => {
            log::warn!("photo mode: the picture's writer stopped");
            None
        }
    }
}

/// A capture's write into a folder that cannot be made
/// or written is an error the joining side logs, never a panic.
#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn a_picture_goes_to_the_first_free_name_or_fails_cleanly() {
        let d = std::env::temp_dir().join(format!("nfsu2-photo-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let px = [10u8, 20, 30, 255].repeat(4);
        let a = write_picture(&d.join("Screenshots"), "20261105-120000", &px, (2, 2)).unwrap();
        let b = write_picture(&d.join("Screenshots"), "20261105-120000", &px, (2, 2)).unwrap();
        assert_ne!(a, b, "the second takes the next name");
        assert_eq!(image::open(&b).unwrap().to_rgba8().into_raw(), px);
        // A file where the folder goes.
        std::fs::write(d.join("blocked"), b"in the way").unwrap();
        let h = std::thread::spawn({
            let dir = d.join("blocked");
            let px = px.clone();
            move || write_picture(&dir, "20261105-120000", &px, (2, 2))
        });
        assert!(written(h).is_none());
        // Too few pixels for the size: the encoder's error, not a panic.
        assert!(write_picture(&d.join("Screenshots"), "x", &px, (64, 64)).is_err());
        // A writer thread that died.
        let dead =
            std::thread::spawn(|| -> Result<std::path::PathBuf> { panic!("a writer panic (expected by this test)") });
        assert!(written(dead).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
