use crate::*;

impl Game {
    /// The dyno camera's zoom keys: the FE keyboard map's "Debug Camera
    /// In" U (0x7FD7E0 entry 22, uAppData 0x35, DIK 0x16) and "Debug Camera
    /// Out" O (entry 23, 0x3D, DIK 0x18) are descriptor rows 65 / 66
    /// (0x7FDD10: JOY 0x51 / 0x52, kind 0xFF), which 0x44F2B0 binds for
    /// both devices (0x5794C0 → 0x579420) to the label 0x446740: with the
    /// dyno manager's state ≠ 0 a press sets +0x3C = 0.98 (In) / 1.02
    /// (Out), 1.0 on release, and the update 0x44F300 multiplies the
    /// distance by it within 5.0 … 7.2. Under Wine (menus12) U / O zoom
    /// the camera on the Dyno Chart and do nothing on Mode Select or the
    // prov: inferred game.dyno_zoom
    /// Dyno Shop menu. **Inferred**: one step per 1/60 s (0x44F300 runs
    /// once a frame; the original's frame rate is not traced), and the
    /// distance kept until the dyno is entered again.
    /// The state the label tests (mover +0x60 → +0x1C → +0x1E0 → +4,
    /// **verified**) is the dyno manager's (vehicle +0x1E0, update
    /// 0x56E6F0; the same chain 0x557B39 starts the pull on): 0 until the
    /// Dyno Chart's new run starts it (0x557B42 → 0x56D350: state 1), 1–4
    /// during the pull, 0 again once it has stopped (state 4 →
    /// 0x56E690(1)). So the keys work only while a pull runs.
    pub(crate) fn dyno_zoom(&mut self, dt: f32) {
        let Mode::Menu(Menu::DynoChart(c)) = &self.mode else { return };
        if !c.pull.as_ref().is_some_and(|p| p.stage() != 0) || !self.on_dyno_track() {
            return;
        }
        let (zin, zout) = (self.platform.keys.contains(&KeyCode::KeyU), self.platform.keys.contains(&KeyCode::KeyO));
        // 0x51 is bound first: with both held the later event (0x52) wins
        // only when it changes (**inferred**: the clone lets In win).
        let rate: f32 = if zin {
            0.98
        } else if zout {
            1.02
        } else {
            return;
        };
        self.dyno.distance = (self.dyno.distance * rate.powf(dt * 60.0)).clamp(DYNO_DISTANCE.0, DYNO_DISTANCE.1);
    }
}
