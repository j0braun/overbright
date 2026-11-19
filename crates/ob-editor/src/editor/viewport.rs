//! The 3D viewport: the world drawn by the game's own [`WorldRenderer`] into
//! an off-screen target that egui shows as an image, the fly camera, mouse
//! picking, the selection outline, and the 2D map view (M) in the same
//! place.
//!
//! Two calls a frame, both from the shell:
//! 1. [`Panel::ui`] inside the egui pass, after the shell's overlays (the
//!    Props handles, the browser's drop target): fills the central rect
//!    with the image, reads the fly / pick input (not when an overlay took
//!    the pointer, [`app::pointer_taken`]), takes the View menu's Frame
//!    selection and Reset camera, draws the selection outline, and notes
//!    the pixel size the image wants;
//! 2. [`app::Viewport::render`] after the egui pass, before egui's own
//!    paint ([`Viewport::prepare`]): takes the world's changed placements,
//!    (re)builds the renderer and the target, and records the world draw —
//!    only when something changed (camera, size, world), so an idle editor
//!    draws nothing.
//!
//! Picking and the ground hit are plain functions over the [`World`] so
//! they run in tests without a GPU.
//!
//! Three looks ([`Look`], the buttons at the image's top right):
//! - **Street**, the game's own draw: the camera section's visibility list,
//!   the region's fog;
//! - **Whole map**, every section of the region whatever the visibility
//!   lists say ([`SectionIndex`]), each culled against the view by the
//!   renderer, scenery smaller than [`WHOLE_MIN_PX`] pixels left out by its
//!   size cull; the far plane past the region, fog off (Haze puts a light
//!   one back). The sections' lowest LOD (the renderer's far list) is not
//!   used: it leaves out the ground and most buildings and, measured on
//!   the whole city, it is not faster;
//! - **Top**, the top-down overview (T): a near-orthographic camera
//!   straight down ([`cam::TopView`]), pan with the right or middle button,
//!   zoom with the wheel, Home fits the region, a double-click flies the
//!   3D camera there; the mod's props and the race's points are marked.
//!
//! With Auto on (the default) the 3D view is Street near the ground and
//! Whole map above [`AUTO_UP`] metres over it. None of this changes the
//! renderer: it already draws any section list it is given.

use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Sense, Vec2 as EVec2};
use glam::{Vec2, Vec3};

use super::app::{self, Gfx};
use super::camera::{self as cam, FlyCam, FlyInput, TopView};
use super::mapview::MapView;
use super::menu::{self, Command};
use super::overlay::{Overlay, colour};
use super::state::{EditorState, Mode, Panel, View};
use crate::render::{FrameParams, Gpu, RenderOptions, WorldRenderer, create_depth};
use crate::world::{HIDDEN_FLAG, World};

/// The colour format the world is drawn in. egui samples a user texture as
/// gamma-encoded `Rgba8Unorm`, which is also what the world renderer writes
/// for `world --shot`, so no conversion pass is needed.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Added placements change the renderer's instance tables, which are built
/// once; the rebuild waits this long after the last such change.
pub const REBUILD_DELAY: Duration = Duration::from_millis(300);
/// Placements larger than this (bounding-sphere radius, metres) are not
/// picked: road and terrain pieces, buildings' shells.
pub const MAX_PICK_RADIUS: f32 = 40.0;
/// How far the mouse ray reaches (metres).
pub const PICK_RANGE: f32 = 1500.0;
/// Placement flag: drawn without a matrix (not a movable prop).
const NO_MATRIX_FLAG: u16 = 0x0100;

/// Auto switches to the whole-map look above this height over the ground
/// (metres) and back to the street look below [`AUTO_DOWN`].
pub const AUTO_UP: f32 = 150.0;
pub const AUTO_DOWN: f32 = 110.0;
/// The smallest scenery the whole-map and overview looks draw: a radius of
/// this many output pixels (the game's own size cull is 17 pixels of a
/// 512-wide view, tuned for the street).
pub const WHOLE_MIN_PX: f32 = 2.0;

/// How the 3D viewport draws the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// The game's draw: the camera section's visibility list, fog.
    Street,
    /// Every section, far ones at their lowest LOD, no fog.
    Whole,
    /// The top-down overview.
    Top,
}

/// The look Auto picks at `height` metres over the ground, given the one
/// it picked last (the band between [`AUTO_DOWN`] and [`AUTO_UP`] keeps
/// it, so hovering at the edge does not flicker).
pub fn auto_look(last: Look, height: f32) -> Look {
    if height > AUTO_UP {
        Look::Whole
    } else if height < AUTO_DOWN {
        Look::Street
    } else if last == Look::Top {
        Look::Whole
    } else {
        last
    }
}

/// A section's drawn extent: its placements' boxes, world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionBox {
    pub section: u32,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// The region's sections the whole-map look draws, with their boxes: the
/// sections that have placements, without the panoramas (x90–x99 single
/// backdrop meshes standing in for far parts of the city, which the
/// whole map draws for real) and the global section (always drawn).
#[derive(Clone, Debug, Default)]
pub struct SectionIndex {
    pub boxes: Vec<SectionBox>,
    /// The region's extent on the ground (x, y): the drivable outlines,
    /// else the boxes.
    pub ground: Option<(Vec2, Vec2)>,
    /// The highest point of any box.
    pub top: f32,
}

impl SectionIndex {
    pub fn new(world: &World) -> SectionIndex {
        let mut by: std::collections::BTreeMap<u32, (Vec3, Vec3, usize)> = Default::default();
        for p in &world.placements {
            let Some(solid) = world.solids.get(p.solid) else { continue };
            let section = p.section;
            let e = by.entry(section).or_insert((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN), 0));
            for c in super::overlay::box_corners(p.transform, solid.bbox_min.into(), solid.bbox_max.into()) {
                e.0 = e.0.min(c);
                e.1 = e.1.max(c);
            }
            e.2 += 1;
        }
        let global = crate::render::GLOBAL_SECTION;
        let boxes: Vec<SectionBox> = by
            .into_iter()
            .filter(|&(s, (_, _, n))| s != global && !(crate::world::is_panorama(s) && n <= PANORAMA_MAX_PLACEMENTS))
            .map(|(section, (lo, hi, _))| SectionBox { section, lo, hi })
            .collect();
        let outlines = world.visibility.drivable().flat_map(|(_, b)| [b.bbox_min, b.bbox_max]);
        let ground =
            bounds2(outlines).or_else(|| bounds2(boxes.iter().flat_map(|b| [b.lo.truncate(), b.hi.truncate()])));
        let top = boxes.iter().map(|b| b.hi.z).fold(f32::MIN, f32::max);
        SectionIndex { boxes, ground, top: if top.is_finite() && top > f32::MIN { top } else { 0.0 } }
    }

    /// Half the diagonal of the region on the ground (metres).
    pub fn radius(&self) -> f32 {
        self.ground.map_or(5000.0, |(lo, hi)| (hi - lo).length() * 0.5)
    }
}

/// A panorama section with more placements than this is drawn as an
/// ordinary one (x90–x93 of some letters hold real props).
const PANORAMA_MAX_PLACEMENTS: usize = 12;

/// The box around some ground points.
fn bounds2(points: impl Iterator<Item = Vec2>) -> Option<(Vec2, Vec2)> {
    points.fold(None, |acc, p| Some(acc.map_or((p, p), |(lo, hi): (Vec2, Vec2)| (lo.min(p), hi.max(p)))))
}

/// The sections of `index` whose boxes reach into the ground rectangle
/// `lo`..`hi` (x, y).
pub fn sections_under(index: &SectionIndex, lo: Vec2, hi: Vec2) -> Vec<u32> {
    index
        .boxes
        .iter()
        .filter(|b| b.lo.x <= hi.x && b.hi.x >= lo.x && b.lo.y <= hi.y && b.hi.y >= lo.y)
        .map(|b| b.section)
        .collect()
}

/// The render options of the whole-map and overview looks: the player's,
/// with a size cull of [`WHOLE_MIN_PX`] output pixels at `width`, and no
/// LOD fade (its fades follow one street camera).
pub fn whole_options(opts: &RenderOptions, width: u32) -> RenderOptions {
    // The cull's sizes are pixels of radius on a 512-wide view.
    let min_size = WHOLE_MIN_PX * 512.0 / width.max(1) as f32;
    let detail = (crate::render::MAIN_MIN_SIZE / min_size).max(opts.detail_distance);
    RenderOptions { detail_distance: detail, pop_free: false, lod_fade: false, ..*opts }
}

/// The light haze the whole-map look draws with Haze on: the region's fog
/// colour, starting a third of the way across the region and never
/// hiding more than 60 % of what is far off.
pub fn haze(fog: Option<crate::fog::Fog>, radius: f32) -> crate::fog::Fog {
    let color = fog.map_or([0.1, 0.11, 0.12], |f| f.color);
    let far = radius * 2.2;
    let start = radius * 0.35;
    crate::fog::Fog { value: [far, 1.0 / (far - start), 0.4], color }
}

/// A pick under the mouse: the placement and the ray's distance to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub placement: usize,
    pub distance: f32,
}

/// A click in the viewport this frame, for the tools of the other panels:
/// where, the ray, what it hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Click {
    pub at: Pos2,
    pub eye: Vec3,
    pub dir: Vec3,
    /// The placement under the mouse (props only).
    pub hit: Option<Hit>,
    /// Where the ray meets the ground (the road surface, kerbs, floors).
    pub ground: Option<Vec3>,
    pub ctrl: bool,
    pub shift: bool,
}

/// The off-screen target the world is drawn into.
struct Target {
    size: (u32, u32),
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    id: egui::TextureId,
}

/// The viewport panel.
pub struct Viewport {
    pub fly: FlyCam,
    /// The 2D map view (shown when the state's view is [`View::Map`]).
    pub map: MapView,
    /// The rectangle the image took this frame (screen points).
    pub rect: Rect,
    /// The placement under the mouse (Props mode, not while flying).
    pub hover: Option<Hit>,
    /// This frame's left click, for the props / races tools.
    pub click: Option<Click>,
    /// Set by a tool while it owns the left button (a gizmo drag), so a
    /// click does not change the selection.
    pub suppress_pick: bool,
    /// Right button held: the camera flies.
    pub flying: bool,
    renderer: Option<WorldRenderer>,
    target: Option<Target>,
    /// Pixel size the last layout asked for.
    want: (u32, u32),
    rebuild_at: Option<Instant>,
    /// The camera last drawn (position, yaw, pitch, fov) and whether a
    /// draw is due for another reason.
    drawn: Option<[f32; 6]>,
    redraw: bool,
    fog: crate::fog::FogState,
    started: Instant,
    /// Auto: Street near the ground, Whole map above it.
    pub auto: bool,
    /// The look chosen with Auto off (Street or Whole).
    pub chosen: Look,
    /// The look in force this frame.
    pub look: Look,
    /// Whole map: a light haze instead of no fog.
    pub haze: bool,
    /// The overview's pan and zoom, and whether it was fitted to the
    /// region yet.
    pub top: TopView,
    top_fitted: bool,
    /// Fit the overview again once the image's rectangle is known.
    fit_on_layout: bool,
    /// While the overview shows: the 3D camera to come back to.
    persp: Option<crate::draw::camera::Camera>,
    /// The region's sections for the whole-map look (made with the
    /// renderer).
    index: Option<SectionIndex>,
    /// The camera's height over the ground, kept for the position it was
    /// measured at.
    height: Option<(Vec3, f32)>,
    /// The look and haze last drawn.
    drawn_look: Option<(Look, bool)>,
}

impl Default for Viewport {
    fn default() -> Viewport {
        Viewport::new()
    }
}

impl Viewport {
    pub fn new() -> Viewport {
        Viewport {
            fly: FlyCam::default(),
            map: MapView::default(),
            rect: Rect::NOTHING,
            hover: None,
            click: None,
            suppress_pick: false,
            flying: false,
            renderer: None,
            target: None,
            want: (0, 0),
            rebuild_at: None,
            drawn: None,
            redraw: true,
            fog: Default::default(),
            started: Instant::now(),
            auto: true,
            chosen: Look::Street,
            look: Look::Street,
            haze: false,
            top: TopView::default(),
            top_fitted: false,
            fit_on_layout: false,
            persp: None,
            index: None,
            height: None,
            drawn_look: None,
        }
    }

    /// The region's section index (made on first use).
    pub fn index(&mut self, world: &World) -> &SectionIndex {
        self.index.get_or_insert_with(|| SectionIndex::new(world))
    }

    /// The camera's height over the ground below it (over z = 0 off the
    /// map), measured again when it moved.
    fn height_over_ground(&mut self, world: &World, p: Vec3) -> f32 {
        if let Some((at, h)) = self.height
            && at.distance_squared(p) < 0.25
        {
            return h;
        }
        let ground = world.collision().ground_at(p).map(|(z, _)| z);
        let h = p.z - ground.unwrap_or(0.0);
        self.height = Some((p, h));
        h
    }

    /// The look for this frame from the toggles and the camera.
    fn update_look(&mut self, s: &EditorState) {
        self.look = if self.persp.is_some() {
            Look::Top
        } else if self.auto {
            let h = self.height_over_ground(&s.world, s.camera.position);
            auto_look(self.look, h)
        } else {
            self.chosen
        };
    }

    /// Show the top-down overview: the 3D camera is kept to come back to,
    /// and the state's camera becomes the overview's (so every tool that
    /// projects with it draws over the overview).
    pub fn enter_top(&mut self, s: &mut EditorState) {
        if self.persp.is_some() {
            return;
        }
        if !self.top_fitted {
            self.fit_top(s);
        }
        self.persp = Some(s.camera);
        s.camera = self.top.camera();
        self.look = Look::Top;
    }

    /// Back to the 3D camera, or to `to` when given (a double-click).
    pub fn leave_top(&mut self, s: &mut EditorState, to: Option<crate::draw::camera::Camera>) {
        let Some(back) = self.persp.take() else { return };
        s.camera = to.unwrap_or(back);
        self.look = if self.auto { Look::Street } else { self.chosen };
        self.height = None;
    }

    /// Fit the overview to the whole region (Home).
    pub fn fit_top(&mut self, s: &mut EditorState) {
        let rect = if self.rect.is_positive() {
            self.rect
        } else {
            Rect::from_min_size(Pos2::ZERO, EVec2::new(1600.0, 900.0))
        };
        let ground = self.index(&s.world).ground;
        match ground {
            Some((lo, hi)) => self.top.fit(rect, lo, hi),
            None => self.top.centre = s.camera.position.truncate(),
        }
        self.top_fitted = true;
        self.fit_on_layout = !self.rect.is_positive();
        if self.persp.is_some() {
            s.camera = self.top.camera();
        }
    }

    /// Put the 3D camera high over the region looking across all of it
    /// (the whole-map look's start view, View ▸ Whole map from the shot).
    pub fn aerial(&mut self, s: &mut EditorState) {
        let index = self.index(&s.world);
        let (lo, hi) = index.ground.unwrap_or((Vec2::splat(-1000.0), Vec2::splat(1000.0)));
        let centre = ((lo + hi) * 0.5).extend(0.0);
        let r = (hi - lo).length() * 0.5;
        let eye = centre + Vec3::new(-0.1 * r, -0.72 * r, 0.6 * r);
        s.camera = crate::draw::camera::Camera::looking_at(eye, centre - Vec3::new(0.0, 0.04 * r, 0.0));
    }

    /// Set the look from a name (`--view` of the shot): `street`, `whole`
    /// (Auto off, the aerial camera) or `top` (the overview, fitted).
    pub fn set_look(&mut self, s: &mut EditorState, name: &str) {
        match name {
            "street" => {
                self.leave_top(s, None);
                self.auto = false;
                self.chosen = Look::Street;
            }
            "whole" => {
                self.leave_top(s, None);
                self.auto = false;
                self.chosen = Look::Whole;
                self.aerial(s);
            }
            "top" => {
                self.enter_top(s);
                self.fit_top(s);
            }
            _ => log::warn!("editor: no look `{name}`"),
        }
        self.redraw = true;
    }

    /// Ask for one more world draw (a setting the renderer reads changed).
    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    /// Whether [`Viewport::prepare`] would draw: the headless shot runs the
    /// frame again until this is false.
    pub fn needs_frame(&self, s: &EditorState) -> bool {
        self.renderer.is_none()
            || self.redraw
            || s.world_changed
            || s.adds_changed
            || self.rebuild_at.is_some()
            || self.target.as_ref().is_none_or(|t| t.size != self.want)
            || self.drawn != Some(camera_key(&s.camera))
            || self.drawn_look != Some((self.look, self.haze))
    }

    /// An overlay painter on the viewport for this frame (clipped to it).
    pub fn overlay<'a>(&self, painter: &'a egui::Painter, s: &'a EditorState) -> Overlay<'a> {
        Overlay::new(painter, &s.camera, self.rect)
    }

    /// The mouse ray through `at` (screen points).
    pub fn ray(&self, s: &EditorState, at: Pos2) -> (Vec3, Vec3) {
        cam::ray(&s.camera, self.rect, at)
    }

    /// Point the camera at the selection (F): the union of the selected
    /// placements' bounding spheres; nothing when nothing is selected.
    pub fn frame_selection(&mut self, s: &mut EditorState) {
        let spheres: Vec<(Vec3, f32)> = s.selection.iter().filter_map(|&i| placement_sphere(&s.world, i)).collect();
        if let Some((centre, radius)) = union_sphere(&spheres) {
            if self.persp.is_some() {
                self.top.centre = centre.truncate();
                self.top.half_height = (radius * 3.0).clamp(cam::TOP_MIN_HALF, self.top.half_height);
                s.camera = self.top.camera();
            } else {
                cam::frame(&mut s.camera, centre, radius, cam::aspect(self.rect));
            }
        }
    }

    /// The panel: the map view or the 3D image, with `draw` called to add
    /// the other panels' overlays (route, checkpoints, gizmo) on top.
    pub fn show(&mut self, s: &mut EditorState, ui: &mut egui::Ui, draw: impl FnOnce(&mut EditorState, &Overlay)) {
        self.click = None;
        if s.view == View::Map {
            self.flying = false;
            self.map.show(s, ui);
            return;
        }
        // Someone else moved the camera while the overview showed (a
        // "Show" in a list, Reset camera): back to 3D at that place.
        if let Some(back) = self.persp
            && camera_key(&s.camera) != camera_key(&self.top.camera())
        {
            self.persp = None;
            let moved = s.camera;
            s.camera = back;
            s.camera.position = moved.position;
            if moved.pitch > -1.5 {
                s.camera.yaw = moved.yaw;
                s.camera.pitch = moved.pitch;
            }
        }
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = rect;
        if self.fit_on_layout && rect.is_positive() {
            self.fit_top(s);
        }
        let ppp = ui.ctx().pixels_per_point();
        self.want = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);

        let painter = ui.painter_at(rect);
        match &self.target {
            Some(t) => {
                painter.image(t.id, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                painter.rect_filled(rect, 0.0, colour::PAGE);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Loading the world…",
                    egui::FontId::proportional(15.0),
                    colour::DIM,
                );
            }
        }

        self.update_look(s);
        let camera = s.camera;
        let overlay = Overlay::new(&painter, &camera, rect);
        if self.look == Look::Top {
            self.top_marks(s, &overlay);
        }
        if let Some(h) = self.hover.filter(|h| !s.selection.contains(&h.placement))
            && let Some((t, lo, hi)) = super::overlay::placement_box(&s.world, h.placement)
        {
            overlay.box_outline(t, lo, hi, colour::DIM, 1.0);
        }
        for &i in &s.selection {
            overlay.selection(&s.world, i);
        }
        draw(s, &overlay);

        let ctx = ui.ctx().clone();
        if menu::take(&ctx, Command::FrameSelection) {
            self.frame_selection(s);
        }
        if menu::take(&ctx, Command::ResetCamera) {
            self.leave_top(s, None);
            s.camera = crate::render::overview_camera(&s.world);
        }
        self.look_bar(s, ui, rect);
        // A press an overlay (a handle, a drop) took is not a pick and
        // does not start the camera.
        let suppress = self.suppress_pick;
        self.suppress_pick |= app::pointer_taken(&ctx);
        self.input(s, ui, &response);
        self.suppress_pick = suppress;
        if self.look == Look::Top {
            let m = self.top.metres_per_point(rect);
            painter.text(
                rect.left_bottom() + EVec2::new(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Overview · {:.0} m across · {:.2} m per point", m * rect.width(), m),
                egui::FontId::proportional(13.0),
                colour::DIM,
            );
        }
        if self.flying {
            painter.text(
                rect.left_bottom() + EVec2::new(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{:.1} m/s", self.fly.speed),
                egui::FontId::proportional(13.0),
                colour::INK,
            );
        }
        if let Some(at) = self.rebuild_at {
            ui.ctx().request_repaint_after(at.saturating_duration_since(Instant::now()));
        }
    }

    fn input(&mut self, s: &mut EditorState, ui: &egui::Ui, response: &egui::Response) {
        let ctx = ui.ctx();
        if !ctx.egui_wants_keyboard_input()
            && !self.flying
            && response.hovered()
            && ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.is_none())
        {
            if self.persp.is_some() {
                self.leave_top(s, None);
            } else {
                self.enter_top(s);
            }
            self.redraw = true;
        }
        if self.persp.is_some() {
            self.top_input(s, ui, response);
            return;
        }
        // Fly while the right button is held (started over the viewport).
        let rmb_down = ctx.input(|i| i.pointer.secondary_down());
        if !self.suppress_pick
            && (response.drag_started_by(egui::PointerButton::Secondary)
                || (response.hovered() && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))))
        {
            self.flying = true;
        }
        if !rmb_down {
            self.flying = false;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let scroll = if response.hovered() { ctx.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        let notches = scroll / 50.0;
        if self.flying {
            let input = ctx.input(|i| {
                let k = |key| if i.key_down(key) { 1.0 } else { 0.0 };
                FlyInput {
                    forward: k(egui::Key::W) - k(egui::Key::S),
                    right: k(egui::Key::D) - k(egui::Key::A),
                    up: (k(egui::Key::E) + k(egui::Key::R)).min(1.0) - (k(egui::Key::Q) + k(egui::Key::F)).min(1.0),
                    look: Vec2::new(i.pointer.delta().x, i.pointer.delta().y),
                    fast: i.modifiers.shift,
                    slow: i.modifiers.ctrl || i.modifiers.command,
                }
            });
            if notches != 0.0 {
                self.fly.wheel(notches);
            }
            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            self.fly.apply(&mut s.camera, &input, dt);
            ctx.request_repaint();
            self.hover = None;
            return;
        }
        if notches != 0.0 {
            // Dolly along the view.
            let step = (self.fly.speed * 0.25).max(1.0) * notches;
            s.camera.position += s.camera.forward() * step;
        }
        if !typing && response.hovered() && ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.is_none()) {
            self.frame_selection(s);
        }

        self.pick_input(s, ctx, response);
    }

    /// Hover and click picking (both cameras).
    fn pick_input(&mut self, s: &mut EditorState, ctx: &egui::Context, response: &egui::Response) {
        let pointer = response.hover_pos();
        let moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        if s.mode == Mode::Props {
            if let Some(at) = pointer {
                if moved || self.hover.is_none() {
                    let (eye, dir) = self.pick_ray(s, at);
                    self.hover = pick(&s.world, eye, dir);
                }
            } else {
                self.hover = None;
            }
        } else {
            self.hover = None;
        }

        if response.clicked_by(egui::PointerButton::Primary) {
            let Some(at) = response.interact_pointer_pos() else { return };
            let (eye, dir) = self.pick_ray(s, at);
            let hit = (s.mode == Mode::Props).then(|| pick(&s.world, eye, dir)).flatten();
            let modifiers = ctx.input(|i| i.modifiers);
            self.click = Some(Click {
                at,
                eye,
                dir,
                hit,
                ground: ground_hit(&s.world, eye, dir, PICK_RANGE),
                ctrl: modifiers.ctrl || modifiers.command,
                shift: modifiers.shift,
            });
            if s.mode == Mode::Props && !self.suppress_pick && !app::pointer_taken(ctx) {
                select(&mut s.selection, hit.map(|h| h.placement), modifiers.ctrl || modifiers.command);
            }
        }
    }

    /// The ray picking and the ground hit use through `at`: the mouse ray,
    /// in the overview started just above the region's highest point (its
    /// camera is far up, beyond [`PICK_RANGE`]).
    pub fn pick_ray(&mut self, s: &EditorState, at: Pos2) -> (Vec3, Vec3) {
        let (eye, dir) = cam::ray(&s.camera, self.rect, at);
        if self.persp.is_none() || dir.z > -1e-3 {
            return (eye, dir);
        }
        let top = self.index(&s.world).top.max(0.0) + 5.0;
        (eye + dir * ((eye.z - top) / -dir.z).max(0.0), dir)
    }

    /// The overview's input: the right or middle button drags the ground,
    /// the wheel zooms around the mouse, Home fits the region, a
    /// double-click flies the 3D camera there; the left button picks and
    /// clicks as in 3D.
    fn top_input(&mut self, s: &mut EditorState, ui: &egui::Ui, response: &egui::Response) {
        let ctx = ui.ctx().clone();
        let rect = self.rect;
        self.flying = false;
        if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
            self.top.pan(rect, response.drag_delta());
        }
        if let Some(at) = response.hover_pos() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.top.zoom_at(rect, at, mapview_zoom(scroll));
            }
        }
        if response.hovered()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Home) && i.modifiers.is_none())
        {
            self.fit_top(s);
        }
        s.camera = self.top.camera();
        if response.double_clicked_by(egui::PointerButton::Primary)
            && let Some(at) = response.interact_pointer_pos()
        {
            let (eye, dir) = self.pick_ray(s, at);
            let ground =
                ground_hit(&s.world, eye, dir, PICK_RANGE).unwrap_or_else(|| self.top.to_world(rect, at).extend(0.0));
            let from = self.persp.unwrap_or(s.camera);
            self.leave_top(s, Some(cam::fly_to(&from, ground)));
            return;
        }
        self.pick_input(s, &ctx, response);
    }

    /// The overview's marks: the mod's moved and added props (lime) and,
    /// outside Races mode (whose panel draws its own), the current race's
    /// checkpoints, start and finish.
    fn top_marks(&self, s: &EditorState, o: &Overlay) {
        let base = &s.base.placements;
        for (i, p) in s.world.placements.iter().enumerate() {
            let changed = match base.get(i) {
                Some(b) => b.transform != p.transform || b.flags != p.flags,
                None => true,
            };
            if !changed {
                continue;
            }
            let at = p.transform.w_axis.truncate();
            if let Some(q) = o.at(at) {
                let hidden = p.flags & HIDDEN_FLAG != 0;
                let ring = if hidden { colour::ERROR } else { colour::SELECT };
                o.painter.circle(q, 4.0, colour::SHADOW, egui::Stroke::new(2.0, ring));
            }
        }
        if s.mode == Mode::Races {
            return;
        }
        let Some(race) = s.race.and_then(|k| s.map.races.get(k)) else { return };
        let z = race.start.or(race.finish).map_or(0.0, |sp| sp.position.z);
        let pts: Vec<Vec3> = race.checkpoints.iter().map(|c| c.extend(z)).collect();
        o.polyline(&pts, false, colour::INK, 2.0);
        for (k, &p) in pts.iter().enumerate() {
            o.marker(p, 8.0, colour::PAGE, colour::INK, Some(&(k + 1).to_string()));
        }
        for (spot, name) in [(race.start, "S"), (race.finish, "F")] {
            if let Some(spot) = spot {
                o.marker(spot.position, 9.0, colour::PAGE, colour::SELECT, Some(name));
            }
        }
    }

    /// The look buttons at the image's top right: Auto, Street, Whole map,
    /// Top (T), and Haze for the whole map.
    fn look_bar(&mut self, s: &mut EditorState, ui: &mut egui::Ui, rect: Rect) {
        let size = EVec2::new(330.0, 30.0);
        let bar = Rect::from_min_size(rect.right_top() + EVec2::new(-size.x - 10.0, 10.0), size);
        if !rect.contains_rect(bar) {
            return;
        }
        ui.painter().rect_filled(bar.expand(2.0), 6.0, colour::PAGE.gamma_multiply(0.85));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar.shrink2(EVec2::new(6.0, 2.0))));
        let top = self.persp.is_some();
        let look = self.look;
        let mut pressed = None;
        child.horizontal_centered(|ui| {
            let seg = |ui: &mut egui::Ui, on: bool, text: &str, tip: &str| {
                let rt = egui::RichText::new(text).font(super::theme::font(super::theme::LABEL, 14.0));
                let rt = if on { rt.color(super::theme::lime()) } else { rt };
                ui.add(egui::Button::selectable(on, rt)).on_hover_text(tip).clicked()
            };
            if seg(ui, self.auto && !top, "Auto", "Street near the ground, whole map from above") {
                pressed = Some("auto");
            }
            if seg(ui, !top && look == Look::Street, "Street", "The game's own draw: what the street shows, fog") {
                pressed = Some("street");
            }
            if seg(ui, !top && look == Look::Whole, "Whole map", "Every section of the region, no fog") {
                pressed = Some("whole");
            }
            if seg(ui, top, "Top", "Top-down overview (T): drag with the right button, wheel zooms, Home fits") {
                pressed = Some("top");
            }
            ui.separator();
            if seg(ui, self.haze, "Haze", "A light haze over the whole map and the overview") {
                pressed = Some("haze");
            }
        });
        match pressed {
            Some("auto") => {
                self.leave_top(s, None);
                self.auto = true;
            }
            Some(k @ ("street" | "whole")) => {
                self.leave_top(s, None);
                self.auto = false;
                self.chosen = if k == "street" { Look::Street } else { Look::Whole };
            }
            Some("top") => {
                if top {
                    self.leave_top(s, None)
                } else {
                    self.enter_top(s)
                }
            }
            Some("haze") => self.haze = !self.haze,
            _ => {}
        }
        if pressed.is_some() {
            self.redraw = true;
            self.update_look(s);
        }
    }

    /// Bring the GPU side up to date and draw the world when something
    /// changed. Call after the egui pass and before egui paints; submit
    /// the returned buffer before egui's.
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        egui: &mut egui_wgpu::Renderer,
        s: &mut EditorState,
        encoder: &mut wgpu::CommandEncoder,
    ) -> bool {
        let now = Instant::now();
        if s.adds_changed {
            s.adds_changed = false;
            if self.renderer.is_some() {
                self.rebuild_at = Some(now + REBUILD_DELAY);
            }
        }
        if self.renderer.is_none() || self.rebuild_at.is_some_and(|t| now >= t) {
            self.renderer = Some(WorldRenderer::new(gpu, &s.world, FORMAT, &crate::render::RenderOptions::in_force()));
            self.rebuild_at = None;
            self.index = Some(SectionIndex::new(&s.world));
            // A new renderer starts from the world as it is now.
            s.world.take_changed();
            s.world_changed = false;
            self.redraw = true;
        }
        let Some(renderer) = self.renderer.as_mut() else { return false };
        if s.world_changed {
            s.world_changed = false;
            for i in s.world.take_changed() {
                if let Some(p) = s.world.placements.get(i) {
                    renderer.update_placement(gpu, i, p.transform, p.flags);
                }
            }
            self.redraw = true;
        }
        if self.want.0 == 0 || self.want.1 == 0 {
            return false;
        }
        if self.target.as_ref().is_none_or(|t| t.size != self.want) {
            let size = self.want;
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("editor viewport"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let color = texture.create_view(&Default::default());
            let depth = create_depth(gpu, size.0, size.1);
            let id = match self.target.take() {
                Some(old) => {
                    egui.update_egui_texture_from_wgpu_texture(&gpu.device, &color, wgpu::FilterMode::Linear, old.id);
                    old.id
                }
                None => egui.register_native_texture(&gpu.device, &color, wgpu::FilterMode::Linear),
            };
            self.target = Some(Target { size, color, depth, id });
            self.redraw = true;
        }
        let key = camera_key(&s.camera);
        if !self.redraw && self.drawn == Some(key) && self.drawn_look == Some((self.look, self.haze)) {
            return false;
        }
        self.redraw = false;
        self.drawn = Some(key);
        self.drawn_look = Some((self.look, self.haze));
        let Some(target) = self.target.as_ref() else { return false };
        let size = target.size;
        let aspect = size.0 as f32 / size.1 as f32;
        let time_of_day = crate::sky::TimeOfDay::default();
        let game_fog = self.fog.update_weighted(&s.world, s.camera.position.truncate(), time_of_day.fog_weight());
        let look = self.look;
        let height = if look == Look::Whole { self.height_over_ground(&s.world, s.camera.position) } else { 0.0 };
        let index = self.index.get_or_insert_with(|| SectionIndex::new(&s.world));
        let lists = whole_frame(index, &s.camera, look, aspect, height, &self.top, self.haze, game_fog);
        let Some(target) = self.target.as_ref() else { return false };
        let Some(renderer) = self.renderer.as_mut() else { return false };
        let opts = match look {
            Look::Street => crate::render::RenderOptions::in_force(),
            Look::Whole | Look::Top => whole_options(&crate::render::RenderOptions::in_force(), size.0),
        };
        let params = FrameParams {
            camera: &lists.camera,
            aspect,
            visible_sections: match &lists.near {
                Some(near) => Some(near.as_slice()),
                None => s.world.visible_sections_at(s.camera.position).map(|(_, list)| list),
            },
            far_sections: None,
            fog: lists.fog,
            time: self.started.elapsed().as_secs_f32(),
            road_reflections: look == Look::Street,
            road_reflection_plane: None,
            sky: time_of_day.sky(),
            reversed: false,
            headlight_pools: &[],
            reflection_glows: &[],
            display: Default::default(),
            opts: &opts,
            rear_view: None,
            occluders: &[],
        };
        renderer.render(gpu, encoder, &target.color, &target.depth, size, &params);
        true
    }

    /// Free the GPU side (the egui texture too), e.g. while a test race
    /// runs on a small GPU. The next [`Viewport::prepare`] builds it again.
    pub fn release(&mut self, egui: &mut egui_wgpu::Renderer) {
        if let Some(t) = self.target.take() {
            egui.free_texture(&t.id);
        }
        self.renderer = None;
        self.drawn = None;
    }
}

impl Panel for Viewport {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        self.show(s, ui, |_, _| {});
    }
}

impl app::Viewport for Viewport {
    fn render(&mut self, s: &mut EditorState, gfx: &mut Gfx<'_>) {
        self.prepare(gfx.gpu, gfx.egui, s, gfx.encoder);
    }

    fn set_look(&mut self, s: &mut EditorState, look: &str) {
        Viewport::set_look(self, s, look);
    }

    fn kept_camera(&self, s: &EditorState) -> crate::draw::camera::Camera {
        self.persp.unwrap_or(s.camera)
    }
}

/// One frame's camera, section lists and fog for a look.
pub struct LookFrame {
    pub camera: crate::draw::camera::Camera,
    /// The full-detail sections (None: the game's visibility list).
    pub near: Option<Vec<u32>>,
    pub fog: Option<crate::fog::Fog>,
}

/// What a frame of `look` draws: the street look the game's lists and fog
/// (`game_fog`); the whole map every section, with the far plane past the
/// region; the overview the sections under its image. No fog in either
/// but Haze.
#[allow(clippy::too_many_arguments)]
pub fn whole_frame(
    index: &SectionIndex,
    camera: &crate::draw::camera::Camera,
    look: Look,
    aspect: f32,
    height: f32,
    top: &TopView,
    haze_on: bool,
    game_fog: Option<crate::fog::Fog>,
) -> LookFrame {
    let radius = index.radius();
    let fog = haze_on.then(|| haze(game_fog, radius));
    match look {
        Look::Street => LookFrame { camera: *camera, near: None, fog: game_fog },
        Look::Whole => {
            let centre = index.ground.map_or(Vec2::ZERO, |(lo, hi)| (lo + hi) * 0.5);
            let reach = radius + camera.position.truncate().distance(centre);
            let all = index.boxes.iter().map(|b| b.section).collect();
            LookFrame { camera: cam::whole_camera(camera, height, reach), near: Some(all), fog }
        }
        Look::Top => {
            let half = Vec2::new(top.half_height * aspect, top.half_height);
            LookFrame { camera: *camera, near: Some(sections_under(index, top.centre - half, top.centre + half)), fog }
        }
    }
}

/// The map view's wheel zoom factor for a scroll of `scroll` points.
fn mapview_zoom(scroll: f32) -> f32 {
    super::mapview::ZOOM_STEP.powf(scroll / 50.0)
}

/// What a draw depends on in the camera.
fn camera_key(c: &crate::draw::camera::Camera) -> [f32; 6] {
    [c.position.x, c.position.y, c.position.z, c.yaw, c.pitch, c.fov_y]
}

/// A click's effect on the selection: replace it (or clear it on empty
/// ground), or with Ctrl toggle the one placement.
pub fn select(selection: &mut Vec<usize>, hit: Option<usize>, toggle: bool) {
    match (hit, toggle) {
        (Some(i), true) => {
            if let Some(at) = selection.iter().position(|&s| s == i) {
                selection.remove(at);
            } else {
                selection.push(i);
            }
        }
        (Some(i), false) => {
            selection.clear();
            selection.push(i);
        }
        (None, true) => {}
        (None, false) => selection.clear(),
    }
}

/// Placement `i`'s bounding sphere (centre, radius), world space.
pub fn placement_sphere(world: &World, i: usize) -> Option<(Vec3, f32)> {
    let p = world.placements.get(i)?;
    let s = world.solids.get(p.solid)?;
    let (lo, hi) = (Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max));
    Some((p.transform.transform_point3((lo + hi) * 0.5), p.transform.transform_vector3(hi - lo).length() * 0.5))
}

/// A sphere around all of `spheres` (not the smallest, but close).
pub fn union_sphere(spheres: &[(Vec3, f32)]) -> Option<(Vec3, f32)> {
    let (lo, hi) = spheres
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &(c, r)| (lo.min(c - r), hi.max(c + r)));
    if spheres.is_empty() {
        return None;
    }
    let centre = (lo + hi) * 0.5;
    let radius = spheres.iter().map(|&(c, r)| c.distance(centre) + r).fold(0.0, f32::max);
    Some((centre, radius))
}

/// Where the ray `o + t·d` enters the box `lo`..`hi` (slab test): the
/// entry distance, `None` when it misses or the box is behind the ray.
/// A ray starting inside the box returns `None` (the camera inside a
/// prop does not pick it).
pub fn ray_box(o: Vec3, d: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let mut enter = f32::MIN;
    let mut leave = f32::MAX;
    for axis in 0..3 {
        let (o, d, lo, hi) = (o[axis], d[axis], lo[axis], hi[axis]);
        if d.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o) / d, (hi - o) / d);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter <= leave && enter > 0.0).then_some(enter)
}

/// The placement the mouse ray from `eye` along `dir` meets first, tested
/// against each solid's box in its own space: shown, drawn with a matrix,
/// a bounding sphere of at most [`MAX_PICK_RADIUS`], within
/// [`PICK_RANGE`].
pub fn pick(world: &World, eye: Vec3, dir: Vec3) -> Option<Hit> {
    let dir = dir.normalize_or_zero();
    let mut best: Option<Hit> = None;
    for (i, p) in world.placements.iter().enumerate() {
        if p.flags & (HIDDEN_FLAG | NO_MATRIX_FLAG) != 0 {
            continue;
        }
        let Some((c, r)) = placement_sphere(world, i) else { continue };
        if !(0.05..=MAX_PICK_RADIUS).contains(&r) {
            continue;
        }
        // The sphere first: cheap, and most placements are far off the ray.
        let oc = c - eye;
        let t = oc.dot(dir);
        if t + r < 0.0 || t - r > PICK_RANGE || oc.length_squared() - t * t > r * r {
            continue;
        }
        let s = &world.solids[p.solid];
        let inv = p.transform.inverse();
        let (o, d) = (inv.transform_point3(eye), inv.transform_vector3(dir));
        let Some(local) = ray_box(o, d, Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max)) else { continue };
        // The local parameter is in world units of `dir` (the inverse maps
        // `eye + t·dir` to `o + t·d`).
        if local <= PICK_RANGE && best.is_none_or(|b| local < b.distance) {
            best = Some(Hit { placement: i, distance: local });
        }
    }
    best
}

/// Where the ray from `eye` along `dir` first meets the ground (the
/// collision floors) within `range` metres: marched in steps that grow with
/// the distance, then halved down to 2 cm.
pub fn ground_hit(world: &World, eye: Vec3, dir: Vec3, range: f32) -> Option<Vec3> {
    ground_hit_with(&|p| world.collision().ground_at(p).map(|(z, _)| z), eye, dir, range)
}

/// [`ground_hit`] over any ground height function (`ground(p)`: the floor
/// height at or below `p`).
pub fn ground_hit_with(ground: &dyn Fn(Vec3) -> Option<f32>, eye: Vec3, dir: Vec3, range: f32) -> Option<Vec3> {
    let dir = dir.normalize_or_zero();
    let below = |t: f32| {
        let p = eye + dir * t;
        ground(p).is_some_and(|z| p.z <= z)
    };
    let mut prev = 0.0;
    let mut t = 0.25;
    while t <= range {
        if below(t) {
            let (mut a, mut b) = (prev, t);
            while b - a > 0.02 {
                let m = (a + b) * 0.5;
                if below(m) { b = m } else { a = m }
            }
            return Some(eye + dir * b);
        }
        prev = t;
        t += (t * 0.01).clamp(0.25, 4.0);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    #[test]
    fn the_ray_enters_the_nearer_face() {
        let lo = Vec3::new(-1.0, -1.0, -1.0);
        let hi = Vec3::ONE;
        assert_eq!(ray_box(Vec3::new(0.0, -10.0, 0.0), Vec3::Y, lo, hi), Some(9.0));
        assert_eq!(ray_box(Vec3::new(5.0, -10.0, 0.0), Vec3::Y, lo, hi), None);
        // Behind, and from inside.
        assert_eq!(ray_box(Vec3::new(0.0, 10.0, 0.0), Vec3::Y, lo, hi), None);
        assert_eq!(ray_box(Vec3::ZERO, Vec3::Y, lo, hi), None);
        // Diagonal.
        let t = ray_box(Vec3::splat(-5.0), Vec3::ONE.normalize(), lo, hi).unwrap();
        assert!((t - 4.0 * 3f32.sqrt()).abs() < 1e-4);
    }

    #[test]
    fn selection_clicks() {
        let mut sel = vec![3, 4];
        select(&mut sel, Some(7), false);
        assert_eq!(sel, [7]);
        select(&mut sel, Some(2), true);
        assert_eq!(sel, [7, 2]);
        select(&mut sel, Some(7), true);
        assert_eq!(sel, [2]);
        select(&mut sel, None, true);
        assert_eq!(sel, [2]);
        select(&mut sel, None, false);
        assert!(sel.is_empty());
    }

    #[test]
    fn the_union_holds_every_sphere() {
        let s = [(Vec3::ZERO, 1.0), (Vec3::new(10.0, 0.0, 0.0), 2.0), (Vec3::new(0.0, 5.0, 3.0), 0.5)];
        let (c, r) = union_sphere(&s).unwrap();
        for (p, pr) in s {
            assert!(p.distance(c) + pr <= r + 1e-4);
        }
        assert!(union_sphere(&[]).is_none());
    }

    #[test]
    fn the_ground_is_found_where_the_ray_crosses_it() {
        // A floor at z = 2 everywhere, and a step up to z = 6 beyond y = 50.
        let ground = |p: Vec3| Some(if p.y > 50.0 { 6.0 } else { 2.0 });
        let eye = Vec3::new(0.0, 0.0, 12.0);
        let hit = ground_hit_with(&ground, eye, Vec3::new(0.0, 1.0, -0.2), 500.0).unwrap();
        assert!((hit.z - 2.0).abs() < 0.03 && (hit.y - 50.0).abs() < 0.2, "{hit:?}");
        let far = ground_hit_with(&ground, eye, Vec3::new(0.0, 1.0, -0.1), 500.0).unwrap();
        assert!(far.y > 50.0 && (far.z - 6.0).abs() < 0.2, "{far:?}");
        assert!(ground_hit_with(&ground, eye, Vec3::Z, 500.0).is_none());
        assert!(ground_hit_with(&|_| None, eye, -Vec3::Z, 500.0).is_none());
    }

    /// A solid that is only a box (no mesh).
    fn solid(bbox_min: [f32; 3], bbox_max: [f32; 3]) -> crate::formats::solid::Solid {
        crate::formats::solid::Solid {
            name: "box".into(),
            hash: 1,
            bbox_min,
            bbox_max,
            textures: vec![],
            vertices: vec![],
            normals: vec![],
            light_materials: vec![],
            indices: vec![],
            materials: vec![],
            mesh_flags: 0,
            markers: vec![],
            light_markers: vec![],
        }
    }

    /// A world of three boxes: two 1 m props on the +Y axis, a hidden one
    /// in front of them, and a 100 m block (too large to pick).
    fn boxes() -> World {
        let mut world = World::default();
        world.solids.push(solid([-0.5; 3], [0.5; 3]));
        world.solids.push(solid([-50.0; 3], [50.0; 3]));
        let place = |solid: usize, at: Vec3, flags: u16| crate::world::Placement {
            solid,
            low_solid: None,
            mid_solid: Some(solid),
            transform: Mat4::from_translation(at),
            section: 1,
            flags,
            cull_radius: 1.0,
        };
        world.placements.push(place(0, Vec3::new(0.0, 20.0, 0.0), 0));
        world.placements.push(place(0, Vec3::new(0.0, 10.0, 0.0), 0));
        world.placements.push(place(0, Vec3::new(0.0, 5.0, 0.0), HIDDEN_FLAG));
        world.placements.push(place(1, Vec3::new(0.0, 300.0, 0.0), 0));
        world.placements.push(place(0, Vec3::new(3.0, 10.0, 0.0), 0));
        world
    }

    #[test]
    fn picking_takes_the_nearest_shown_prop() {
        let world = boxes();
        let hit = pick(&world, Vec3::ZERO, Vec3::Y).unwrap();
        assert_eq!(hit.placement, 1);
        assert!((hit.distance - 9.5).abs() < 1e-4);
        // Off to the side: the one at x = 3.
        let dir = Vec3::new(3.0, 10.0, 0.0).normalize();
        assert_eq!(pick(&world, Vec3::ZERO, dir).unwrap().placement, 4);
        // The big block is never picked.
        assert!(pick(&world, Vec3::new(0.0, 200.0, 0.0), Vec3::Y).is_none());
    }

    #[test]
    fn picking_uses_the_rotated_box() {
        let mut world = boxes();
        world.placements.truncate(1);
        // A long thin prop (4 m along x) turned 90°: it now lies along y.
        world.solids[0].bbox_min = [-2.0, -0.1, -0.1];
        world.solids[0].bbox_max = [2.0, 0.1, 0.1];
        world.placements[0].transform =
            Mat4::from_translation(Vec3::new(0.0, 20.0, 0.0)) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        // Its sphere reaches x = 1.5 but the box does not.
        assert!(pick(&world, Vec3::new(1.5, 0.0, 0.0), Vec3::Y).is_none());
        let hit = pick(&world, Vec3::ZERO, Vec3::Y).unwrap();
        assert!((hit.distance - 18.0).abs() < 1e-3);
    }

    #[test]
    fn a_mouse_ray_picks_what_is_drawn_under_it() {
        use crate::draw::camera::Camera;
        let world = boxes();
        let camera = Camera::new(Vec3::new(0.0, -5.0, 2.0), 0.0, -0.1);
        let rect = Rect::from_min_size(Pos2::new(40.0, 30.0), egui::vec2(1280.0, 720.0));
        for i in [0usize, 1, 4] {
            let centre = world.placements[i].transform.w_axis.truncate();
            let at = cam::project(&camera, rect, centre).unwrap();
            let (eye, dir) = cam::ray(&camera, rect, at);
            assert_eq!(pick(&world, eye, dir).map(|h| h.placement), Some(i));
        }
    }

    #[test]
    fn auto_switches_with_height_and_does_not_flicker() {
        assert_eq!(auto_look(Look::Street, 20.0), Look::Street);
        assert_eq!(auto_look(Look::Street, AUTO_UP + 1.0), Look::Whole);
        // Between the two heights the last look stays.
        let mid = (AUTO_UP + AUTO_DOWN) * 0.5;
        assert_eq!(auto_look(Look::Whole, mid), Look::Whole);
        assert_eq!(auto_look(Look::Street, mid), Look::Street);
        assert_eq!(auto_look(Look::Whole, AUTO_DOWN - 1.0), Look::Street);
        assert_eq!(auto_look(Look::Top, mid), Look::Whole);
    }

    /// A world of sections: two ordinary ones, a panorama (one backdrop
    /// mesh), a panorama-numbered section with many props, the global one.
    fn sections() -> World {
        let mut world = boxes();
        world.placements.clear();
        let mut place = |section: u32, at: Vec3, solid: usize| {
            world.placements.push(crate::world::Placement {
                solid,
                low_solid: None,
                mid_solid: Some(solid),
                transform: Mat4::from_translation(at),
                section,
                flags: 0,
                cull_radius: 1.0,
            })
        };
        place(121, Vec3::new(0.0, 0.0, 0.0), 0);
        place(121, Vec3::new(10.0, 4.0, 2.0), 0);
        place(322, Vec3::new(500.0, 300.0, 0.0), 0);
        place(199, Vec3::new(0.0, 0.0, 0.0), 1);
        for k in 0..20 {
            place(391, Vec3::new(-200.0 + k as f32, 0.0, 0.0), 0);
        }
        place(crate::render::GLOBAL_SECTION, Vec3::ZERO, 1);
        world
    }

    #[test]
    fn the_index_holds_every_drawn_section_but_panoramas_and_the_global_one() {
        let index = SectionIndex::new(&sections());
        let numbers: Vec<u32> = index.boxes.iter().map(|b| b.section).collect();
        assert_eq!(numbers, [121, 322, 391]);
        let b121 = index.boxes[0];
        assert_eq!(b121.lo, Vec3::splat(-0.5));
        assert_eq!(b121.hi, Vec3::new(10.5, 4.5, 2.5));
        // No outlines in this world: the ground is the boxes'.
        let (lo, hi) = index.ground.unwrap();
        assert_eq!((lo, hi), (Vec2::new(-200.5, -0.5), Vec2::new(500.5, 300.5)));
        assert_eq!(index.top, 2.5);
        assert!((index.radius() - (hi - lo).length() * 0.5).abs() < 1e-3);
    }

    #[test]
    fn each_look_draws_its_sections() {
        let index = SectionIndex::new(&sections());
        let camera = crate::draw::camera::Camera::new(Vec3::new(0.0, -50.0, 400.0), 0.0, -0.8);
        let top = TopView { centre: Vec2::new(0.0, 0.0), half_height: 100.0 };
        let fog = crate::fog::Fog { value: [300.0, 0.01, 0.0], color: [0.2, 0.3, 0.4] };
        // Street: the game's lists and fog, the camera untouched.
        let f = whole_frame(&index, &camera, Look::Street, 1.5, 400.0, &top, true, Some(fog));
        assert!(f.near.is_none() && f.fog.is_some_and(|g| g.value == fog.value));
        assert_eq!(f.camera.far, camera.far);
        // Whole: every section, no fog, the far plane past the region.
        let f = whole_frame(&index, &camera, Look::Whole, 1.5, 400.0, &top, false, Some(fog));
        assert_eq!(f.near.as_deref(), Some(&[121, 322, 391][..]));
        assert!(f.fog.is_none());
        assert!(f.camera.far >= index.radius() && f.camera.near > camera.near);
        // Haze keeps the region's colour, far off.
        let f = whole_frame(&index, &camera, Look::Whole, 1.5, 400.0, &top, true, Some(fog));
        let h = f.fog.unwrap();
        assert_eq!(h.color, fog.color);
        assert!(h.value[0] > index.radius() && h.value[2] > 0.0);
        // Top: the sections under the image only.
        let f = whole_frame(&index, &top.camera(), Look::Top, 1.5, 0.0, &top, false, Some(fog));
        assert_eq!(f.near.as_deref(), Some(&[121][..]));
        let wide = TopView { centre: Vec2::new(0.0, 0.0), half_height: 150.0 };
        let f = whole_frame(&index, &wide.camera(), Look::Top, 1.5, 0.0, &wide, false, None);
        assert_eq!(f.near.as_deref(), Some(&[121, 391][..]));
        assert!(f.fog.is_none());
    }

    #[test]
    fn the_whole_map_draws_small_scenery_down_to_a_pixel_or_two() {
        let opts = RenderOptions::default();
        let w = whole_options(&opts, 1600);
        assert!(!w.pop_free && !w.lod_fade);
        // The size need on the 512-wide scale is WHOLE_MIN_PX output pixels.
        let need = crate::render::MAIN_MIN_SIZE / w.detail_distance;
        assert!((need * 1600.0 / 512.0 - WHOLE_MIN_PX).abs() < 1e-3, "{need}");
        // Never less detail than the player chose.
        let far = RenderOptions { detail_distance: 1000.0, ..opts };
        assert_eq!(whole_options(&far, 1600).detail_distance, 1000.0);
    }

    #[test]
    fn the_overview_round_trip_keeps_the_3d_camera() {
        let dir = std::env::temp_dir();
        let camera = crate::draw::camera::Camera::new(Vec3::new(5.0, 6.0, 7.0), 0.4, -0.2);
        let mut s = EditorState::new(dir.clone(), "L4RA", dir, "fixture", Default::default(), sections(), camera);
        let mut vp = Viewport::new();
        vp.enter_top(&mut s);
        assert_eq!(vp.look, Look::Top);
        assert_eq!(camera_key(&s.camera), camera_key(&vp.top.camera()));
        // Fitted to the region's ground.
        let (lo, hi) = vp.index(&s.world).ground.unwrap();
        assert!(vp.top.half_height >= (hi.y - lo.y) * 0.5);
        vp.leave_top(&mut s, None);
        assert_eq!(camera_key(&s.camera), camera_key(&camera));
        // A double-click's camera replaces it.
        vp.enter_top(&mut s);
        let to = cam::fly_to(&camera, Vec3::new(100.0, 50.0, 0.0));
        vp.leave_top(&mut s, Some(to));
        assert_eq!(camera_key(&s.camera), camera_key(&to));
        assert_ne!(vp.look, Look::Top);
    }

    #[test]
    fn overview_picking_starts_above_the_region() {
        let dir = std::env::temp_dir();
        let camera = crate::draw::camera::Camera::new(Vec3::new(0.0, -50.0, 20.0), 0.0, 0.0);
        let mut s = EditorState::new(dir.clone(), "L4RA", dir, "fixture", Default::default(), boxes(), camera);
        let mut vp = Viewport::new();
        vp.rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        vp.enter_top(&mut s);
        vp.top = TopView { centre: Vec2::new(0.0, 10.0), half_height: 30.0 };
        s.camera = vp.top.camera();
        // The prop at (0, 10) sits under the middle of the image.
        let (eye, dir) = vp.pick_ray(&s, vp.rect.center());
        // Just above the highest box (the 100 m block's top, 50 m).
        assert!((eye.z - 55.0).abs() < 0.01, "{eye:?}");
        assert_eq!(pick(&s.world, eye, dir).map(|h| h.placement), Some(1));
    }

    /// Redraw on demand: the world is drawn on the first frame, after a
    /// camera move, a placement change or a resize, and not otherwise.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn the_world_is_drawn_only_when_something_changed() {
        use crate::draw::camera::Camera;
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let mut world = World::default();
        let mut tri = solid([-1.0; 3], [1.0; 3]);
        tri.vertices = [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]]
            .into_iter()
            .map(|position| crate::formats::solid::Vertex { position, color: [255; 4], uv: [0.0; 2] })
            .collect();
        tri.indices = vec![0, 1, 2, 2, 1, 0];
        tri.materials = vec![crate::formats::solid::Material {
            texture_slot: 0,
            light_material: 255,
            first_index: 0,
            index_count: 6,
            flags: 0,
        }];
        world.solids.push(tri);
        world.placements.push(crate::world::Placement {
            solid: 0,
            mid_solid: Some(0),
            low_solid: None,
            transform: Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0)),
            section: 1,
            flags: 0x60,
            cull_radius: 1.0,
        });
        let dir = std::env::temp_dir();
        let camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let mut s = EditorState::new(dir.clone(), "L4RA", dir, "fixture", Default::default(), world, camera);
        let mut egui = egui_wgpu::Renderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, Default::default());
        let mut vp = Viewport::new();
        vp.want = (64, 48);
        let mut frame = |vp: &mut Viewport, s: &mut EditorState| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let drawn = vp.prepare(&gpu, &mut egui, s, &mut encoder);
            gpu.queue.submit([encoder.finish()]);
            drawn
        };
        assert!(frame(&mut vp, &mut s), "first frame");
        assert!(!vp.needs_frame(&s));
        assert!(!frame(&mut vp, &mut s), "idle");
        s.camera.position.x += 1.0;
        assert!(vp.needs_frame(&s));
        assert!(frame(&mut vp, &mut s), "camera moved");
        assert!(!frame(&mut vp, &mut s), "idle again");
        s.world.set_placement_transform(0, Mat4::from_translation(Vec3::new(1.0, 10.0, 0.0)));
        s.world_changed = true;
        assert!(frame(&mut vp, &mut s), "placement moved");
        vp.want = (80, 48);
        assert!(frame(&mut vp, &mut s), "resized");
        assert!(!frame(&mut vp, &mut s), "idle after resize");
        // A look or haze change draws again, once.
        vp.look = Look::Whole;
        assert!(vp.needs_frame(&s));
        assert!(frame(&mut vp, &mut s), "whole map");
        assert!(!frame(&mut vp, &mut s), "idle in the whole map");
        vp.haze = true;
        assert!(frame(&mut vp, &mut s), "haze");
        vp.look = Look::Street;
        vp.haze = false;
        assert!(frame(&mut vp, &mut s), "street again");
        // Added placements rebuild the renderer after the delay.
        s.adds_changed = true;
        assert!(!frame(&mut vp, &mut s), "rebuild waits");
        vp.rebuild_at = Some(Instant::now());
        assert!(frame(&mut vp, &mut s), "rebuilt");
    }
}
