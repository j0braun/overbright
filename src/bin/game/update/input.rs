use super::TickControls;
use crate::*;

impl Game {
    /// `update_world`'s first phase: the pads, the mouse (or photo mode),
    /// the FE car's hydraulics sounds, the headless audio capture's hook,
    /// and this frame's driving controls (with the look-back and rumble).
    pub(super) fn tick_input(&mut self, dt: f32) -> Result<TickControls> {
        self.pad_frame();
        static INPUT_LOG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let log = *INPUT_LOG.get_or_init(|| debug_env().input_log.clone().is_some());
        let (m, before) = (self.platform.mouse, if log { self.input_tag() } else { String::new() });
        // Photo mode takes the mouse (the pause menu under it none).
        if self.photo.is_some() {
            self.photo_tick(dt);
        } else if self.free_camera.is_some() {
            self.free_camera_tick(dt);
        } else {
            self.fe_mouse()?;
        }
        if log {
            let shown = self.cursor_shown(self.now());
            self.input_log(&m, &before, shown);
        }
        // The FE car's hydraulics sounds (0x4A6030 → PlayFESound 0x58–0x5D),
        // as the last drawn garage frame asked.
        let hydro_sounds = self.render.garage.as_mut().map(|g| std::mem::take(&mut g.hydro_sounds)).unwrap_or_default();
        for id in hydro_sounds {
            self.fe_sample(id);
        }
        self.track_undim();
        self.dyno_zoom(dt);
        // The script's input mode 0xA (0x5EA6F2): no driving input.
        let controls = if self.career_world.dday.holds_input() && matches!(self.mode, Mode::Explore) {
            Controls::default()
        } else {
            self.controls()
        };
        let controls = if matches!(self.mode, Mode::Explore | Mode::Race(_)) {
            self.hydraulics_controls(controls, dt)
        } else {
            controls
        };
        // Look Back (Controls row 9, DIACTION 9 → event 0x7E in 0x602BE0):
        // held, it sets player +0x249 and the chase mover's look-back
        // (vtable +0x14, 0x445A30). Not in a replay, nor while the script
        // holds the input.
        let look_back = !self.career_world.dday.holds_input()
            && match &self.mode {
                Mode::Explore => true,
                Mode::Race(r) => r.replay.playback.is_none(),
                _ => false,
            }
            && (self.platform.keys.iter().any(|&k| self.is_action(k, LOOK_BACK_ROW))
                || self.platform.pads.held() & (1 << nfsu2::pad::act::LOOK_BACK) != 0);
        if let Some(d) = self.sim.drive.as_mut() {
            d.chase.set_look_back(look_back);
        }
        self.rumble_tick();
        self.ff_tick();
        Ok(TickControls { controls })
    }
}
