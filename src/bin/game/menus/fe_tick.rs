//! The front end's state (docs/original/uncapped-fps.md "Front end out of the draw"):
//! what the drawn frame used to step, stepped once per tick instead. The garage
//! and the dyno camera run before the tick's sounds (`fe_tick_before_sounds`,
//! the original's order, steps 15 and 17 before 18 of 0x5811D0);
//! photo mode's close and the hit map at the end of `Game::step`. The draw
//! only reads what they leave: drawing a frame twice, or not at all,
//! changes nothing.
use crate::*;

impl Game {
    /// The tick's front-end steps that come before its sounds: in
    /// the original GarageMain's update 0x4D82D0 runs in the front end's
    /// tick (0x5663C0, step 15 of the loop 0x5811D0) and the camera movers
    /// (0x44EFE0, step 17) before the AudioManager (0x483B40, step 18), so
    /// the garage's part / turn / swing sounds and the hydraulics' go out
    /// with the tick that made them, and the tick's sounds hear the dyno's
    /// held camera (after the sounds they would be one tick late).
    pub(crate) fn fe_tick_before_sounds(&mut self) {
        self.garage_tick();
        let tuning_world = self.tuning_menu_over_world();
        self.dyno_camera_update(tuning_world);
    }

    /// The tick's front-end steps after its sounds, in the order the
    /// frame's draw ran them (photo mode's close first in `draw`, then
    /// `prepare_frame`'s layout). Neither posts a sound: photo mode is the
    /// clone's (no native step), the hit map is a layout.
    pub(crate) fn fe_tick(&mut self) {
        self.photo_close_update();
        self.free_camera_close_update();
        self.fe_map_tick();
    }

    /// The mouse's hit map from the tick's layout — the same layout
    /// the draw makes (`replay_quads`' control bar, photo mode's strip or
    /// `menu_quads`), laid out with the tick's state and recorded (only
    /// while a menu or a replay is up, as before). The next tick's mouse
    /// (`fe_mouse`, at its start) tests a click against it: the layout of
    /// the state the click is applied to, at every display rate; the
    /// hovers (`help_hover`, `fe_hover`, the lists' and sliders' rows)
    /// follow the same map. Before, the map was what the last drawn frame
    /// laid out.
    pub(crate) fn fe_map_tick(&self) {
        let recording =
            matches!(&self.mode, Mode::Menu(_)) || matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_some());
        nfsu2::frontend::mouse::begin_frame(recording);
        if recording {
            let now = self.now();
            if matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_some()) {
                let _ = self.replay_quads(Vec::new(), now);
            }
            if self.photo_quads().is_none() && self.free_camera.is_none() {
                let _ = self.menu_quads(now);
            }
        }
        // The map done; layouts until the next tick's record nothing.
        nfsu2::frontend::mouse::begin_frame(false);
    }

    /// F4–F7 `garage_tick`: GarageMain's update (0x4D82D0, run by the front
    /// end's tick): the request with its lot swing, view tick, FE car swing
    /// and their sounds (`garage_update`), then the scene stepped to it
    /// (`garage_scene_tick`: loads, the builds on threads taken when done,
    /// hydraulics, hood / trunk / doors, particles).
    pub(crate) fn garage_tick(&mut self) {
        self.garage_update();
        self.garage_ahead_take();
        let gpu = self.platform.window.as_ref().map(|w| (&*w.gpu, w.config.format));
        garage_scene_tick(
            self.garage_fe.request.as_ref(),
            &self.game_data.root,
            &self.game_data.caches,
            &self.game_data.career.packages,
            &mut self.render.garage,
            gpu,
            Display::from_custom(&self.settings.custom),
        );
        // The FE car's hydraulics sounds in the same update (0x4D8BB6 →
        // 0x4A6030 → PlayFESound 0x58–0x5D; before, the next tick's
        // input phase played them). The input phase still plays what a
        // harness's own scene step left.
        let hydro_sounds = self.render.garage.as_mut().map(|g| std::mem::take(&mut g.hydro_sounds)).unwrap_or_default();
        for id in hydro_sounds {
            self.fe_sample(id);
        }
    }

    /// The dyno camera (0x15) holds while the tuning menus are
    /// up on the dyno track (its rates are zero: nothing moves it). Called
    /// from the tick's `fe_tick` (not the frame's `prepare_frame`;
    /// `tuning_world`: `tuning_menu_over_world`).
    pub(crate) fn dyno_camera_update(&mut self, tuning_world: bool) {
        if tuning_world
            && self.on_dyno_track()
            && let Some(d) = self.sim.drive.as_mut()
        {
            let (at, turn) = self.dyno.rest.unwrap_or((d.state.position, d.state.orientation));
            d.camera = dyno_camera(at, turn, self.dyno.distance);
        }
    }
}
