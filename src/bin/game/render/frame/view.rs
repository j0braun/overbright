//! The frame's draw on explicit inputs. [`Game::draw_frame`]
//! runs the per-frame updates that still write game state (the HUD's and
//! the menus' layout with their `*_update`s, the garage's request, the
//! dyno camera, the display mode: `prepare_frame`, unchanged), then
//! [`draw`] on the game's state read through a shared [`FrameView`] and
//! the render side written through [`RenderMut`], then the updates that
//! take the draw's results ([`DrawOutcome`]).
use crate::*;

/// What a frame's draw reads of the game: shared borrows only.
pub(crate) struct FrameView<'a> {
    pub(crate) mode: &'a Mode,
    pub(crate) settings: &'a options::Settings,
    pub(crate) game_data: &'a crate::state::GameData,
    /// Of the pacing only what the draw takes (no clock: the frame's time
    /// is `poses.clocks`): the window's focus and visibility.
    pub(crate) focused: bool,
    pub(crate) occluded: bool,
    pub(crate) paused_race: Option<&'a Race>,
    pub(crate) traffic: Option<&'a Traffic>,
    pub(crate) encounters: Option<&'a Encounters>,
    pub(crate) shop_scene: Option<&'a ShopRun>,
    pub(crate) scene_crowd: Option<&'a SceneCrowd>,
    /// The HUD asked for the rear-view mirror this frame.
    pub(crate) rear_view_on: bool,
    pub(crate) photo: Option<&'a photo::Photo>,
    /// The frame's own values between the two tick snapshots (the
    /// cars as drawn, the crowd's frame, the carried particles, the
    /// clocks, α).
    pub(crate) poses: &'a render::snapshot::FramePoses,
}

/// What a frame's draw writes: the render state and the window's side of
/// the platform (its GPU, swap chain, post passes and UI; the mirror's
/// bound image; the UI textures queued), plus the game state the draw
/// still writes ([`DrawWrites`]).
pub(crate) struct RenderMut<'a> {
    pub(crate) state: &'a mut RenderState,
    pub(crate) window: &'a mut Option<WindowState>,
    pub(crate) rear_view_bound: &'a mut Option<wgpu::TextureView>,
    pub(crate) ui_uploads: &'a mut Vec<nfsu2::formats::texture::Texture>,
    pub(crate) writes: DrawWrites<'a>,
}

/// Game state the draw still writes (not movable without a logic
/// or library change; each listed here):
/// - `drive`: the scene's per-frame state kept in the drive (its effects'
///   GPU halves, water and smoke preparation, the flare pools' per-frame
///   fill, the camera's TAA jitter; with the visuals on ticks the screen tint, the fog,
///   the horizon fog, the motion blur's level and the flare blink step on
///   the tick, `render::visuals`, and the draw only reads them);
/// - `cover_car` / `cover_picture`: UI_MagazineView's cover shot (its car
///   renderer, target and the read-back pixels the game saves) and the
///   loaded picture the draw binds (and takes).
pub(crate) struct DrawWrites<'a> {
    pub(crate) drive: Option<&'a mut Drive>,
    pub(crate) cover_car: &'a mut Option<CoverShot>,
    pub(crate) cover_picture: &'a mut Option<Vec<u8>>,
}

/// What the draw leaves for the updates after it.
#[derive(Default)]
pub(crate) struct DrawOutcome {
    /// The frame reached its end (the frame counter counts it).
    pub(crate) presented: bool,
    /// The texture packs were reloaded: the maps' calibration follows.
    pub(crate) packs_reloaded: bool,
    /// The movie's fresh frame was uploaded.
    pub(crate) movie_uploaded: bool,
    /// The showcase's capture: its request and the picture taken (only for
    /// a frame of the world, drawn).
    pub(crate) picture: Option<((usize, bool), Option<(Vec<u8>, u32, u32)>)>,
}

impl Game {
    /// The frame's view and render side, borrowed apart.
    pub(crate) fn draw_parts<'a>(&'a mut self, poses: &'a render::snapshot::FramePoses) -> (FrameView<'a>, RenderMut<'a>) {
        let view = FrameView {
            mode: &self.mode,
            settings: &self.settings,
            game_data: &self.game_data,
            focused: self.pacing.focused,
            occluded: self.pacing.occluded,
            paused_race: self.sim.paused_race.as_ref(),
            traffic: self.sim.traffic.as_ref(),
            encounters: self.sim.encounters.as_ref(),
            shop_scene: self.scenes.shop.as_ref(),
            scene_crowd: self.scenes.crowd.as_ref(),
            rear_view_on: self.hud_state.rear_view_on,
            photo: self.photo.as_ref(),
            poses,
        };
        let render = RenderMut {
            state: &mut self.render,
            window: &mut self.platform.window,
            rear_view_bound: &mut self.platform.rear_view_bound,
            ui_uploads: &mut self.platform.ui_uploads,
            writes: DrawWrites {
                drive: self.sim.drive.as_mut(),
                cover_car: &mut self.showcase.cover_car,
                cover_picture: &mut self.showcase.cover_picture,
            },
        };
        (view, render)
    }

    /// The updates that take the draw's results, in the order the draw
    /// made them: the maps' calibration after a pack reload, the movie's
    /// frame marked shown, the showcase's picture saved.
    pub(crate) fn drawn_update(&mut self, out: &DrawOutcome) {
        if out.packs_reloaded
            && let Err(e) =
                nfsu2::frontend::map_pack::reapply(nfsu2::hd_ui_pack::active_root(), &mut self.game_data.career.tracks)
        {
            log::warn!("Ignoring map calibration: {e:#}");
        }
        if out.movie_uploaded {
            movie_shown_update(&mut self.mode);
        }
        if let Some(((milestone, dvd), captured)) = &out.picture {
            self.mode =
                self.save_picture(*milestone, *dvd, captured.as_ref().map(|(rgba, w, h)| (rgba.as_slice(), *w, *h)));
        }
    }
}
