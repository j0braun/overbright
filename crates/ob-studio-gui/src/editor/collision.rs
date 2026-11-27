//! The collision layer over the viewport (View ▸ Collision,
//! docs/modding/map-editor.md "Collision"): the original's static collision
//! from the region's collision packs, as the game loads them, drawn in the
//! 3D view and on the Map. Off, the invisible barriers only (walls no
//! scenery stands at, and the race and career barriers), or all collision
//! (also the walls behind scenery and the floors).
//!
//! The layer is built the first time it is shown, and again whenever the
//! mod changes the static collision (moved scenery, removed and drawn
//! walls and ground), from the collision in play; the cover test looks at
//! the scenery as the original places it (`session.base`), so a wall
//! counts as invisible where the original game shows nothing at it.
//!
//! With the layer shown a click on a wall selects it (a drawn ground: a
//! click inside it), and the Properties panel offers Remove wall (Del),
//! one undo step. Edit ▸ Draw wall arms two clicks on the ground (a wall
//! of the panel's height between them); Edit ▸ Draw ground clicks an
//! outline, ended with Enter or a click on its first point. Esc disarms.
//! The same in Top and on the Map (the point is the mouse's x/y at the
//! floor's height there, a wall is picked by its foot). A drawn point snaps
//! to collision corners and the ends of the mod's drawn walls within
//! [`SNAP_PX`] (the snap mode Free: no snapping), a ring marking the corner.

use egui::{Color32, Pos2, Rect, Shape, Stroke};
use glam::{Vec2, Vec3};

use super::overlay::{Overlay, colour, line2d};
use super::state::EditorState;
use crate::tools::map::CollisionEdit;
use crate::tools::map::collision::{self, Layer, Mine, Wall};
// Named `Target` here: the controls test reads every `Key::` as a key.
use crate::tools::map::ops::{CollisionKey as Target, Op};

/// A drawing tool armed (Edit ▸ Draw wall / Draw ground) and its points so far.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Wall { first: Option<Vec3> },
    Ground { points: Vec<Vec3> },
}

/// The settings a drawn wall or ground gets (the Properties panel's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawSettings {
    /// A wall's height, metres.
    pub height: f32,
    /// The surface id; None: the floor's under the first point.
    pub surface: Option<u8>,
}

impl Default for DrawSettings {
    fn default() -> Self {
        DrawSettings { height: 2.0, surface: None }
    }
}

/// A piece of static collision selected in the layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub key: Target,
    /// What it is, for the panel ("Invisible wall", "Drawn wall wall-1").
    pub what: String,
    /// Its outline on the ground (a wall: its foot), for the highlight.
    pub outline: Vec<Vec3>,
    /// A wall's height range.
    pub z: (f32, f32),
    /// The surface id of its face (grip and sound); None for a drawn ground set by the mod's own outline.
    pub surface: Option<u8>,
    /// What it belongs to ("Barrier group BARRIERS_4126", "Next to the prop LAMP_01"), for the panel and status.
    pub owner: String,
}

/// A click this close (points) to a wall's foot or top picks it.
const PICK_PX: f32 = 8.0;
/// A click this close (points) to an outline's first point closes it.
const CLOSE_PX: f32 = 10.0;
/// A drawn point this close (points) to a collision corner or the end of a
/// drawn wall snaps to it (Free places without snapping).
pub const SNAP_PX: f32 = 10.0;

pub use ob_studio::collision_view::{Class, Colour, Look, OPACITIES, Show, opacity_label};

/// Which barrier groups a workspace's collision layer draws. In the game a barrier group is switched on by what
/// is being played: `BARRIERS_<track>` and `PLAYER_BARRIERS_<track>` by a race on that base track (free roam is
/// track 4000), `BARRIERS_CAREER<n>` by career stage n in free roam; group 0 is always there. World draws the
/// permanent walls and free roam's own groups (the other races' barriers on request); Races draws the selected
/// race's barriers over the permanent walls, dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Scope {
    pub focus: super::state::Focus,
    /// Races: the selected race's base track (None: no race selected, no barriers).
    pub track: Option<u16>,
    /// World: also draw the barriers of races.
    pub race_barriers: bool,
    /// What colours the walls and floors.
    pub colour: Colour,
    /// Which classes are shown, opacity, wireframe, full height.
    pub look: Look,
}

/// How a wall is drawn under a [`Scope`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Full,
    Dim,
    Hidden,
}

/// The free roam track's number (its barrier groups are `BARRIERS_4000`, `PLAYER_BARRIERS_4000`).
pub const FREE_ROAM: u16 = 4000;

/// The base track a barrier group's name belongs to (`BARRIERS_4126`, `PLAYER_BARRIERS_4126`); None for the
/// career stages' (`BARRIERS_CAREER2`) and anything else.
pub fn group_track(name: &str) -> Option<u16> {
    let n = name.strip_prefix("PLAYER_BARRIERS_").or_else(|| name.strip_prefix("BARRIERS_"))?;
    n.parse().ok()
}

impl Scope {
    /// Everything at full strength (the editor's own window).
    pub const ALL: Scope = Scope { focus: super::state::Focus::Both, track: None, race_barriers: true, colour: Colour::Kind, look: Look::DEFAULT };

    pub fn of(s: &EditorState) -> Scope {
        use super::state::Focus;
        let track = s.race.and_then(|i| s.session.map.races.get(i)).map(|r| r.track);
        Scope { focus: s.focus, track, race_barriers: s.race_barriers && s.focus == Focus::World, colour: s.collision_colour, look: s.collision_look }
    }

    /// How wall `w` of `layer` is drawn.
    pub fn weight(&self, layer: &Layer, w: &Wall) -> Weight {
        use super::state::Focus;
        if self.focus == Focus::Both {
            return Weight::Full;
        }
        let track = if w.group == 0 { None } else { group_track(layer.group_name(w.group)) };
        match (self.focus, w.group) {
            (Focus::World, 0) => Weight::Full,
            (Focus::World, _) => match track {
                Some(n) if n != FREE_ROAM && !self.race_barriers => Weight::Hidden,
                _ => Weight::Full,
            },
            (_, 0) => Weight::Dim,
            _ => match (track, self.track) {
                (Some(n), Some(r)) if n == r => Weight::Full,
                _ => Weight::Hidden,
            },
        }
    }
}

/// The layer's colours, the overlays' semantic ones: red for invisible
/// walls (what a player runs into unwarned), amber for the switched race
/// and career barriers, neutral for the rest.
pub mod key {
    use super::*;

    pub const INVISIBLE: Color32 = colour::ERROR;
    pub const BARRIER: Color32 = colour::WARN;
    pub const VISIBLE: Color32 = colour::DIM;
    pub const FLOOR: Color32 = colour::ROAD;
    /// A floor the wheels do not count as ground (flag 0x08).
    pub const FLOOR_OFF: Color32 = colour::SELECT;

    /// The colours of the mod's own collision, set apart from the original's in every mode (an added prop's, a
    /// moved prop's, a drawn wall or ground).
    pub fn mine(m: collision::Mine) -> Option<Color32> {
        let t = crate::launch::theme::THEME_A;
        let rgb = |c: crate::launch::theme::Rgb| Color32::from_rgb(c.0, c.1, c.2);
        match m {
            collision::Mine::Original => None,
            collision::Mine::Added => Some(rgb(t.info)),
            collision::Mine::Moved => Some(rgb(t.sel)),
            collision::Mine::Drawn => Some(rgb(t.ok)),
        }
    }

    /// The colour of surface `id` (the same id always the same hue; neighbouring ids are far apart).
    pub fn surface(id: u8) -> Color32 {
        let hue = (id as f32 * 0.381_966).fract();
        egui::ecolor::Hsva::new(hue, 0.55, 0.95, 1.0).into()
    }

    /// A wall's fill: its colour, see-through.
    pub fn fill(c: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
    }
}

/// The most surface rows the legend lists.
const SURFACE_ROWS: usize = 8;

/// How far from the camera the 3D view draws walls and floors (metres; the
/// walls' reach grows with the camera's height, for the overview).
const WALLS_NEAR: f32 = 300.0;
const WALLS_FAR: f32 = 2500.0;
const FLOORS_NEAR: f32 = 120.0;
/// At most this many walls and floors in one frame (nearest cells first).
const MAX_WALLS: usize = 8000;
const MAX_FLOORS: usize = 4000;

/// The scenery triangles the cover test looks at (the original's placements, [`ob_studio::collision_view`]).
pub fn scenery(s: &EditorState) -> Vec<[Vec3; 3]> {
    ob_studio::collision_view::scenery(&s.session.world, &s.session.base.placements)
}

/// The layer of the open region (read from the loaded collision packs).
pub fn layer_of(s: &EditorState) -> Layer {
    let t = std::time::Instant::now();
    let scenery = scenery(s);
    let world = &s.session.world;
    let mut layer = collision::build(world.ground.listed_faces(), &world.barrier_groups, &scenery);
    ob_studio::collision_view::mark(&mut layer, world);
    let c = layer.counts();
    log::info!(
        "collision layer: {} faces read, {} invisible walls, {} barrier walls, {} walls at scenery, {} floors ({} scenery triangles, {:.0} ms)",
        layer.read,
        c.invisible,
        c.barriers,
        c.visible,
        c.floors,
        scenery.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    layer
}

/// The layer kept for the open region, drawn over the viewport.
#[derive(Default)]
pub struct CollisionOverlay {
    /// The region and collision version it was built for, and the layer.
    layer: Option<(String, u64, Layer)>,
}

impl CollisionOverlay {
    fn layer(&mut self, s: &EditorState) -> &Layer {
        let version = s.session.world.map_overlay.collision_version;
        if self.layer.as_ref().is_none_or(|(r, v, _)| *r != s.session.region || *v != version) {
            self.layer = Some((s.session.region.clone(), version, layer_of(s)));
        }
        &self.layer.as_ref().unwrap().2
    }
}

impl super::app::Overlay for CollisionOverlay {
    fn overlay(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) -> bool {
        if super::menu::take(ctx, super::menu::Command::RemoveWall) {
            remove_picked(s);
        }
        let proj = Proj::of(s, ctx, rect);
        let show = s.collision;
        let floors = s.camera.fov_y >= 5f32.to_radians();
        let lg = (show != Show::Off).then(|| {
            let scope = Scope::of(s);
            legend_layout(self.layer(s), show, &scope, floors, painter, rect)
        });
        // A click on the key switches a class; it is not a pick or a drawn point.
        let on_key = lg.as_ref().is_some_and(|l| {
            ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| l.panel.contains(p))
                && ctx.layer_id_at(ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default()).is_none_or(|l| l.order == egui::Order::Background)
        });
        if on_key
            && let (Some(l), Some(p)) = (&lg, ctx.input(|i| i.pointer.hover_pos()))
            && ctx.input(|i| i.pointer.primary_pressed())
            && let Some(t) = l.hit(p)
        {
            toggle(&mut s.collision_look, t);
            ctx.request_repaint();
        }
        let taken = on_key || proj.as_ref().is_some_and(|proj| draw_input(self, s, ctx, painter, rect, proj));
        if show == Show::Off {
            return taken;
        }
        let taken =
            taken || (s.draw.is_none() && proj.as_ref().is_some_and(|proj| pick_input(self, s, ctx, rect, proj)));
        if let (Some(p), Some(proj)) = (&s.wall, &proj) {
            poly(proj, painter, &p.outline, p.outline.len() > 2, colour::SELECT, 3.0);
            if p.z.1 > p.z.0 && !proj.is_2d() {
                let top: Vec<Vec3> = p.outline.iter().map(|v| v.truncate().extend(p.z.1)).collect();
                poly(proj, painter, &top, false, colour::SELECT, 2.0);
            }
        }
        let scope = Scope::of(s);
        let layer = self.layer(s);
        paint_3d(layer, show, &scope, &Overlay::new(painter, &s.camera, rect));
        if let Some(l) = &lg {
            paint_legend(l, painter);
        }
        taken
    }
}

/// How the view puts a world point on the screen: the 3D camera, the
/// overview (Top). The race tools read the same views.
pub enum Proj {
    Camera(crate::draw::camera::Camera, Rect),
    Top(super::camera::TopView, Rect),
}

impl Proj {
    pub fn of(s: &EditorState, ctx: &egui::Context, rect: Rect) -> Option<Proj> {
        Some(match super::viewport::top_view(ctx) {
            Some((top, shown)) => Proj::Top(top, shown),
            None => Proj::Camera(s.camera, rect),
        })
    }

    pub fn is_2d(&self) -> bool {
        !matches!(self, Proj::Camera(..))
    }

    pub fn at(&self, p: Vec3) -> Option<Pos2> {
        match self {
            Proj::Camera(c, r) => super::camera::project(c, *r, p),
            Proj::Top(top, r) => {
                let (m, c) = (top.metres_per_point(*r), r.center());
                Some(Pos2::new(c.x + (p.x - top.centre.x) / m, c.y - (p.y - top.centre.y) / m))
            }
        }
    }

    /// Metres per screen point (2D views; the 3D view has no one scale).
    pub fn metres_per_point(&self) -> Option<f32> {
        match self {
            Proj::Camera(..) => None,
            Proj::Top(top, r) => Some(top.metres_per_point(*r)),
        }
    }
}

/// A world polyline in the view (closed when `closed`).
fn poly(proj: &Proj, painter: &egui::Painter, points: &[Vec3], closed: bool, c: Color32, width: f32) {
    match proj {
        Proj::Camera(cam, rect) => Overlay::new(painter, cam, *rect).polyline(points, closed, c, width),
        _ => {
            let n = points.len();
            for k in 0..n {
                if k + 1 == n && !closed {
                    break;
                }
                if let (Some(a), Some(b)) = (proj.at(points[k]), proj.at(points[(k + 1) % n])) {
                    line2d(painter, a, b, c, width);
                }
            }
        }
    }
}

/// A point of the collision a drawn point snaps to.
pub fn snap_to(corners: impl IntoIterator<Item = Vec3>, proj: &Proj, at: Pos2) -> Option<Vec3> {
    corners
        .into_iter()
        .filter_map(|c| proj.at(c).map(|p| (p.distance(at), c)))
        .filter(|(d, _)| *d <= SNAP_PX)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// The corners near the ground point `g` a drawn point may snap to: the
/// original collision's (walls' feet, floors' corners) and the ends of the
/// mod's own drawn walls and outlines.
fn snap_corners(o: &mut CollisionOverlay, s: &EditorState, proj: &Proj, g: Vec3) -> Vec<Vec3> {
    let reach = proj.metres_per_point().map_or(40.0, |m| (SNAP_PX + 2.0) * m);
    let (lo, hi) = (g.truncate() - Vec2::splat(reach), g.truncate() + Vec2::splat(reach));
    let mut out = Vec::new();
    // The sides of a drawn wall's box are not corners to snap to: its ends are.
    let drawn: Vec<(Vec2, Vec2)> = s
        .session
        .map
        .collision
        .iter()
        .filter_map(|e| match e {
            CollisionEdit::Wall { region, from, to, .. } if region.eq_ignore_ascii_case(&s.session.region) => {
                Some((from.truncate(), to.truncate()))
            }
            _ => None,
        })
        .collect();
    let on_drawn = |p: Vec2| {
        drawn.iter().any(|&(a, b)| {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            (a + ab * t).distance(p) <= crate::tools::map::draw::WALL_THICKNESS
        })
    };
    let layer = o.layer(s);
    let (walls, floors) = layer.near(lo, hi);
    for i in walls {
        let w = &layer.walls[i as usize];
        if on_drawn(w.a.lerp(w.b, 0.5)) {
            continue;
        }
        out.push(w.a.extend(w.z0));
        out.push(w.b.extend(w.z0));
    }
    for i in floors {
        out.extend_from_slice(layer.floors[i as usize].points());
    }
    for e in &s.session.map.collision {
        match e {
            CollisionEdit::Wall { region, from, to, .. } if region.eq_ignore_ascii_case(&s.session.region) => {
                out.extend([*from, *to]);
            }
            CollisionEdit::Ground { region, outline, .. } if region.eq_ignore_ascii_case(&s.session.region) => {
                out.extend_from_slice(outline);
            }
            _ => {}
        }
    }
    out
}

/// The ground under the mouse when the left button went down over the
/// view this frame (not over a window or a handle), snapped to existing
/// corners unless the snap mode is Free.
fn pressed_ground(
    o: &mut CollisionOverlay,
    s: &EditorState,
    ctx: &egui::Context,
    rect: Rect,
    proj: &Proj,
    snap: bool,
) -> Option<(Pos2, Option<Vec3>)> {
    let (pointer, pressed) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_pressed()));
    let p = pointer
        .filter(|p| rect.contains(*p) && ctx.layer_id_at(*p).is_none_or(|l| l.order == egui::Order::Background))?;
    if !pressed || super::app::flying(ctx) || super::app::pointer_taken(ctx) {
        return None;
    }
    Some((p, if snap { ground_snapped(o, s, ctx, rect, proj, p) } else { super::app::race_ground(s, ctx, rect, p) }))
}

/// The world point under screen point `p` for drawing (see
/// [`pressed_ground`]).
fn ground_snapped(
    o: &mut CollisionOverlay,
    s: &EditorState,
    ctx: &egui::Context,
    rect: Rect,
    proj: &Proj,
    p: Pos2,
) -> Option<Vec3> {
    let g = super::app::race_ground(s, ctx, rect, p)?;
    if !s.snap_mode.snaps() {
        return Some(g);
    }
    let cands = snap_corners(o, s, proj, g);
    Some(snap_to(cands, proj, p).unwrap_or(g))
}

/// The snap marker: a ring and a dot on the point a click would take.
fn snap_marker(proj: &Proj, painter: &egui::Painter, snapped: Vec3, plain: Vec3) {
    if snapped == plain {
        return;
    }
    if let Some(at) = proj.at(snapped) {
        painter.circle_stroke(at, SNAP_PX * 0.6, Stroke::new(2.0, colour::SELECT));
        painter.circle_filled(at, 2.5, colour::SELECT);
    }
}

/// The armed drawing tool: its clicks, Enter and Esc, and its preview.
/// True when it took the pointer.
fn draw_input(
    o: &mut CollisionOverlay,
    s: &mut EditorState,
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: Rect,
    proj: &Proj,
) -> bool {
    let Some(draw) = s.draw.clone() else { return false };
    let (enter, esc, backspace) = if ctx.egui_wants_keyboard_input() {
        (false, false, false)
    } else {
        ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace),
            )
        })
    };
    if esc {
        step_back(s);
        return false;
    }
    // Backspace takes back the outline's last corner (the way Esc takes back the whole shape).
    if backspace && let Some(Draw::Ground { points }) = s.draw.as_mut() {
        points.pop();
        return false;
    }
    let hover_at = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
    let plain = hover_at.and_then(|p| super::app::race_ground(s, ctx, rect, p));
    let hover = hover_at.and_then(|p| ground_snapped(o, s, ctx, rect, proj, p));
    if let (Some(h), Some(g)) = (hover, plain) {
        snap_marker(proj, painter, h, g);
    }
    let click = pressed_ground(o, s, ctx, rect, proj, true);
    match draw {
        Draw::Wall { first } => {
            if let (Some(a), Some(b)) = (first, hover) {
                let h = s.draw_settings.height;
                if proj.is_2d() {
                    poly(proj, painter, &[a, b], false, colour::SELECT, 2.0);
                } else {
                    for side in crate::tools::map::draw::wall_box(a, b, h) {
                        poly(proj, painter, &side, true, colour::SELECT, 1.5);
                    }
                }
            }
            match click {
                Some((_, Some(at))) => {
                    match first {
                        None => s.draw = Some(Draw::Wall { first: Some(at) }),
                        Some(a) => {
                            let surface = s.draw_settings.surface.unwrap_or_else(|| surface_under(s, a));
                            let op =
                                Op::DrawWall { from: a, to: at, height: s.draw_settings.height, surface, id: None };
                            let _ = super::undo::run(s, op);
                            s.draw = Some(Draw::Wall { first: None });
                        }
                    }
                    true
                }
                Some((_, None)) => {
                    s.status = "No ground under the mouse: fly closer".into();
                    true
                }
                None => false,
            }
        }
        Draw::Ground { mut points } => {
            let mut line = points.clone();
            line.extend(hover);
            poly(proj, painter, &line, points.len() >= 2, colour::SELECT, 2.0);
            let close = |at: Pos2| points.first().and_then(|p| proj.at(*p)).is_some_and(|f| f.distance(at) < CLOSE_PX);
            let mut finish = enter;
            let took = match click {
                Some((at, _)) if points.len() >= 3 && close(at) => {
                    finish = true;
                    true
                }
                Some((_, Some(at))) => {
                    if points.len() < crate::tools::map::format::MAX_OUTLINE {
                        points.push(at);
                    }
                    true
                }
                Some((_, None)) => {
                    s.status = "No ground under the mouse: fly closer".into();
                    true
                }
                None => false,
            };
            if finish && points.len() >= 3 {
                let surface = s.draw_settings.surface.unwrap_or_else(|| surface_under(s, points[0]));
                let _ = super::undo::run(s, Op::DrawGround { outline: points, surface, id: None });
                s.draw = Some(Draw::Ground { points: Vec::new() });
            } else {
                s.draw = Some(Draw::Ground { points });
            }
            took
        }
    }
}

/// Esc, one step back at a time (as with the race tools): the shape being drawn is dropped and the tool stays
/// armed; with no shape begun the tool is put down; with no tool the selected wall is let go.
pub fn step_back(s: &mut EditorState) {
    match s.draw.as_mut() {
        Some(Draw::Wall { first }) if first.is_some() => *first = None,
        Some(Draw::Ground { points }) if !points.is_empty() => points.clear(),
        Some(_) => s.draw = None,
        None => s.wall = None,
    }
}

/// What the hint line and the Properties panel say for `d`, so a drawing tool reads the same in both: the
/// next click, how to finish, how to step back.
pub fn draw_words(d: &Draw) -> &'static str {
    match d {
        Draw::Wall { first: None } => "Click the wall's first end on the ground · K: snap mode",
        Draw::Wall { first: Some(_) } => "Click the wall's other end · K: snap mode",
        Draw::Ground { points } if points.len() < 3 => {
            "Click the outline's corners on the ground · Backspace: last corner · K: snap mode"
        }
        Draw::Ground { .. } => {
            "Click more corners · Enter or a click on the first corner finishes · Backspace: last corner"
        }
    }
}

/// What Esc does under the armed tool `d`, for the hint line.
pub fn esc_words(d: &Draw) -> &'static str {
    let begun = match d {
        Draw::Wall { first } => first.is_some(),
        Draw::Ground { points } => !points.is_empty(),
    };
    if begun { "cancel this shape" } else { "stop drawing" }
}

/// The surface id of the floor under `p` (the most common floor surface of
/// the region where there is none).
pub fn surface_under(s: &EditorState, p: Vec3) -> u8 {
    let world = &s.session.world;
    let near = world
        .ground
        .listed_faces()
        .filter(|f| !f.is_wall() && f.flags & 0x08 == 0)
        .find(|f| {
            let n = f.corners.clamp(3, 4);
            let v: Vec<Vec2> = f.vertices[..n].iter().map(|v| v.truncate()).collect();
            let at = p.truncate();
            let side = |a: Vec2, b: Vec2| (b - a).perp_dot(at - a);
            let signs: Vec<f32> = (0..n).map(|k| side(v[k], v[(k + 1) % n])).collect();
            signs.iter().all(|&x| x >= 0.0) || signs.iter().all(|&x| x <= 0.0)
        })
        .map(|f| f.surface);
    near.unwrap_or_else(|| {
        let mut count = [0usize; 256];
        for f in world.ground.listed_faces().filter(|f| !f.is_wall()) {
            count[f.surface as usize] += 1;
        }
        (0..=255u8).max_by_key(|&k| count[k as usize]).unwrap_or(0)
    })
}

/// A click on a wall (or inside drawn ground) selects it. True when it
/// picked one.
fn pick_input(o: &mut CollisionOverlay, s: &mut EditorState, ctx: &egui::Context, rect: Rect, proj: &Proj) -> bool {
    let Some((at, ground)) = pressed_ground(o, s, ctx, rect, proj, false) else { return false };
    let show = s.collision;
    let scope = Scope::of(s);
    let camera = s.camera;
    let layer = o.layer(s);
    let ov = |p: Vec3| super::camera::project(&camera, rect, p);
    let near = |a: Pos2, b: Pos2| {
        let ab = b - a;
        let t = ((at - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
        (a + ab * t).distance(at)
    };
    let (centre, reach) = match (proj.metres_per_point(), ground) {
        (Some(m), Some(g)) => (g.truncate(), (PICK_PX + 2.0) * m),
        (Some(_), None) => (Vec2::ZERO, -1.0),
        (None, _) => (camera.position.truncate(), WALLS_NEAR),
    };
    let walls =
        if reach < 0.0 { Vec::new() } else { layer.near(centre - Vec2::splat(reach), centre + Vec2::splat(reach)).0 };
    let mut best: Option<(f32, u32)> = None;
    for &i in &walls {
        let w = &layer.walls[i as usize];
        if parts(layer, w, show, &scope).is_empty() {
            continue;
        }
        if proj.is_2d() {
            // Seen from above a wall is its foot.
            let (Some(f0), Some(f1)) = (proj.at(w.a.extend(w.z0)), proj.at(w.b.extend(w.z0))) else { continue };
            let d = near(f0, f1);
            if d <= PICK_PX && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
            continue;
        }
        let q = wall_quad(w, 0.0, 1.0, &scope.look);
        let (Some(f0), Some(f1), Some(t0), Some(t1)) = (ov(q[0]), ov(q[1]), ov(q[3]), ov(q[2])) else { continue };
        let d = near(f0, f1).min(near(t0, t1)).min(near(f0, t0)).min(near(f1, t1));
        let inside = Shape::convex_polygon(vec![f0, f1, t1, t0], Color32::WHITE, Stroke::NONE)
            .visual_bounding_rect()
            .contains(at)
            && {
                let poly = [f0, f1, t1, t0];
                let s: Vec<f32> = (0..4)
                    .map(|k| {
                        (poly[(k + 1) % 4] - poly[k]).x * (at - poly[k]).y
                            - (poly[(k + 1) % 4] - poly[k]).y * (at - poly[k]).x
                    })
                    .collect();
                s.iter().all(|&x| x >= 0.0) || s.iter().all(|&x| x <= 0.0)
            };
        let d = if inside { 0.0 } else { d };
        let depth = w.a.lerp(w.b, 0.5).extend(w.z0).distance(camera.position);
        if d <= PICK_PX
            && best.is_none_or(|(bd, bi)| {
                let bw = &layer.walls[bi as usize];
                (d, depth) < (bd, bw.a.lerp(bw.b, 0.5).extend(bw.z0).distance(camera.position))
            })
        {
            best = Some((d, i));
        }
    }
    let picked = match best {
        Some((_, i)) => {
            let w = layer.walls[i as usize].clone();
            Some(picked_wall(s, layer, &w))
        }
        None => ground.and_then(|g| picked_ground(s, g)),
    };
    // Nothing else to pick: a floor under the click is named in the status line (the click stays the view's).
    let floor = if picked.is_none() && show == Show::All {
        ground.and_then(|g| picked_floor(layer, &scope, g)).map(|f| (f.surface, floor_owner(f)))
    } else {
        None
    };
    match picked {
        Some(p) => {
            s.selection.clear();
            s.status = pick_words(p.surface, &p.owner);
            s.wall = Some(p);
            true
        }
        None => {
            s.wall = None;
            if let Some((surface, owner)) = floor {
                s.status = pick_words(Some(surface), &owner);
            }
            false
        }
    }
}

/// The status line for a picked face: its surface and what it belongs to.
fn pick_words(surface: Option<u8>, owner: &str) -> String {
    use crate::studio::words;
    let mut out = String::new();
    if let Some(id) = surface {
        out = format!("{} {id}", words::COLLISION_SURFACE);
    }
    if !owner.is_empty() {
        if !out.is_empty() {
            out.push_str(" · ");
        }
        out.push_str(owner);
    }
    out
}

/// What a floor belongs to.
fn floor_owner(f: &collision::Floor) -> String {
    use crate::studio::words;
    match f.mine {
        Mine::Added => words::COLLISION_ADDED_PROP.into(),
        Mine::Moved => words::COLLISION_MOVED_PROP.into(),
        Mine::Drawn => words::COLLISION_DRAWN_BY_MOD.into(),
        Mine::Original if f.not_ground => words::COLLISION_FLOOR_OFF.into(),
        Mine::Original => words::COLLISION_ROAD_FLOOR.into(),
    }
}

/// The floor shown under world point `at` (the one nearest in height).
fn picked_floor<'a>(layer: &'a Layer, scope: &Scope, at: Vec3) -> Option<&'a collision::Floor> {
    let p = at.truncate();
    let (_, floors) = layer.near(p - Vec2::splat(1.0), p + Vec2::splat(1.0));
    floors
        .iter()
        .map(|&i| &layer.floors[i as usize])
        .filter(|f| floor_shown_in(f, scope) && inside(f.points(), p))
        .min_by(|a, b| {
            let d = |f: &collision::Floor| (f.points().iter().map(|v| v.z).sum::<f32>() / f.n as f32 - at.z).abs();
            d(a).total_cmp(&d(b))
        })
}

/// Whether `p` is inside the polygon `points` seen from above.
fn inside(points: &[Vec3], p: Vec2) -> bool {
    let n = points.len();
    let mut inside = false;
    for k in 0..n {
        let (a, b) = (points[k].truncate(), points[(k + 1) % n].truncate());
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// What wall `w` belongs to: its barrier group, or the nearest prop of the original's scenery (the wall stands
/// there as the collision of that prop or the road beside it).
fn wall_owner(s: &EditorState, layer: &Layer, w: &Wall) -> String {
    use crate::studio::words;
    match w.mine {
        Mine::Added => return words::COLLISION_ADDED_PROP.into(),
        Mine::Moved => return words::COLLISION_MOVED_PROP.into(),
        Mine::Drawn => return words::COLLISION_DRAWN_BY_MOD.into(),
        Mine::Original => {}
    }
    if w.group != 0 {
        return format!("{} {}", words::COLLISION_BARRIER_GROUP, layer.group_name(w.group));
    }
    let mid = w.a.lerp(w.b, 0.5);
    let nearest = s
        .session
        .base
        .placements
        .iter()
        .map(|p| (p.transform.w_axis.truncate().truncate().distance(mid), p))
        .filter(|(d, _)| *d <= NEAR_PROP)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match nearest.and_then(|(_, p)| s.session.world.solids.get(p.solid)) {
        Some(solid) => format!("{} {}", words::COLLISION_NEXT_TO, solid.name),
        None => words::COLLISION_NO_PROP.into(),
    }
}

/// A wall counts as next to a prop when the prop's origin is this near (metres).
const NEAR_PROP: f32 = 25.0;

/// What a click on wall `w` selects: a drawn wall (its whole box) when it
/// is one of the mod's, else the original face.
fn picked_wall(s: &EditorState, layer: &Layer, w: &Wall) -> Picked {
    let mid = w.a.lerp(w.b, 0.5);
    let drawn = s.session.map.collision.iter().find_map(|e| match e {
        CollisionEdit::Wall { id, region, from, to, .. } if region.eq_ignore_ascii_case(&s.session.region) => {
            let (a, b) = (from.truncate(), to.truncate());
            let ab = b - a;
            let t = ((mid - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            ((a + ab * t).distance(mid) <= crate::tools::map::draw::WALL_THICKNESS).then(|| (id.clone(), *from, *to))
        }
        _ => None,
    });
    match drawn {
        Some((id, from, to)) => Picked {
            what: format!("Drawn wall {id}"),
            key: Target::Drawn(id),
            outline: vec![from, to],
            z: (w.z0, w.z1),
            surface: Some(w.surface),
            owner: crate::studio::words::COLLISION_DRAWN_BY_MOD.into(),
        },
        None => Picked {
            what: match w.kind() {
                collision::WallKind::Invisible => "Invisible wall".into(),
                collision::WallKind::Barrier => "Race or career barrier".into(),
                collision::WallKind::Visible => "Wall behind scenery".into(),
            },
            key: Target::Original(w.corners.clone()),
            outline: vec![w.a.extend(w.z0), w.b.extend(w.z0)],
            z: (w.z0, w.z1),
            surface: Some(w.surface),
            owner: wall_owner(s, layer, w),
        },
    }
}

/// The mod's drawn wall or ground named `id` as the selection (None: the mod has none of that name here).
pub fn pick_drawn(s: &EditorState, id: &str) -> Option<Picked> {
    s.session.map.collision.iter().find_map(|e| match e {
        CollisionEdit::Wall { id: i, region, from, to, height, surface, .. }
            if i == id && region.eq_ignore_ascii_case(&s.session.region) =>
        {
            let base = from.z.min(to.z);
            Some(Picked {
                what: format!("Drawn wall {id}"),
                key: Target::Drawn(id.to_string()),
                outline: vec![*from, *to],
                z: (base, base + height),
                surface: Some(*surface),
                owner: crate::studio::words::COLLISION_DRAWN_BY_MOD.into(),
            })
        }
        CollisionEdit::Ground { id: i, region, outline, .. }
            if i == id && region.eq_ignore_ascii_case(&s.session.region) =>
        {
            Some(Picked {
                what: format!("Drawn ground {id}"),
                key: Target::Drawn(id.to_string()),
                outline: outline.clone(),
                z: (0.0, 0.0),
                surface: None,
                owner: crate::studio::words::COLLISION_DRAWN_BY_MOD.into(),
            })
        }
        _ => None,
    })
}

/// The mod's drawn ground under `at`, if any.
fn picked_ground(s: &EditorState, at: Vec3) -> Option<Picked> {
    s.session.map.collision.iter().find_map(|e| match e {
        CollisionEdit::Ground { id, region, outline, .. } if region.eq_ignore_ascii_case(&s.session.region) => {
            let p = at.truncate();
            let n = outline.len();
            let mut inside = false;
            for k in 0..n {
                let (a, b) = (outline[k].truncate(), outline[(k + 1) % n].truncate());
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                    inside = !inside;
                }
            }
            inside.then(|| Picked {
                what: format!("Drawn ground {id}"),
                key: Target::Drawn(id.clone()),
                outline: outline.clone(),
                z: (0.0, 0.0),
                surface: None,
                owner: crate::studio::words::COLLISION_DRAWN_BY_MOD.into(),
            })
        }
        _ => None,
    })
}

/// Whether `p` is one of a race's or the career's barriers (the game switches them per race, so they stay).
pub fn is_barrier(p: &Picked) -> bool {
    matches!(p.key, Target::Original(_)) && p.what.starts_with("Race")
}

/// Remove the selected wall or drawn piece (Del, the panel's button): one
/// undo step.
pub fn remove_picked(s: &mut EditorState) -> bool {
    let Some(p) = s.wall.take() else { return false };
    if is_barrier(&p) {
        s.status = "A race's barriers are switched per race: they cannot be removed".into();
        s.wall = Some(p);
        return true;
    }
    let _ = super::undo::run(s, Op::RemoveCollision { target: p.key });
    true
}

/// The Properties panel for the selected wall or ground, or the armed
/// drawing tool.
pub fn panel(s: &mut EditorState, ui: &mut egui::Ui) -> bool {
    use crate::launch::theme::THEME_A;
    use egui::RichText;
    let dim = |t: &str| RichText::new(t).small().color(super::theme::rgb(THEME_A.dim));
    if let Some(d) = s.draw.clone() {
        let title = if matches!(d, Draw::Wall { .. }) { "Draw wall" } else { "Draw ground" };
        ui.label(RichText::new(title).strong().color(super::theme::rgb(THEME_A.ink)));
        ui.label(dim(draw_words(&d)));
        ui.label(dim(&format!("Esc: {}", esc_words(&d))));
        egui::Grid::new("draw-settings").num_columns(2).show(ui, |ui| {
            if matches!(d, Draw::Wall { .. }) {
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut s.draw_settings.height)
                        .range(crate::tools::map::format::MIN_WALL_HEIGHT..=crate::tools::map::format::MAX_WALL_HEIGHT)
                        .speed(0.05)
                        .suffix(" m"),
                );
                ui.end_row();
            }
            ui.label("Surface");
            ui.horizontal(|ui| {
                let mut from_floor = s.draw_settings.surface.is_none();
                if ui.checkbox(&mut from_floor, "the floor's").changed() {
                    s.draw_settings.surface = if from_floor { None } else { Some(16) };
                }
                if let Some(v) = s.draw_settings.surface.as_mut() {
                    ui.add(egui::DragValue::new(v).range(0..=255));
                }
            });
            ui.end_row();
        });
        ui.label(dim("A drawn wall is a closed box 0.5 m thick, so it blocks from both sides at any frame rate."));
        if ui.button("Stop drawing").on_hover_text("Esc, twice while a shape is begun").clicked() {
            s.draw = None;
        }
        return true;
    }
    let Some(p) = s.wall.clone() else { return false };
    ui.label(RichText::new(&p.what).strong().color(super::theme::rgb(THEME_A.ink)));
    if p.outline.len() == 2 {
        let l = p.outline[0].truncate().distance(p.outline[1].truncate());
        ui.label(dim(&format!("{l:.1} m long · {:.1} m high · static collision", p.z.1 - p.z.0)));
    } else {
        ui.label(dim(&format!("{} corners · static collision", p.outline.len())));
    }
    ui.label(dim(&pick_words(p.surface, &p.owner)));
    let barrier = is_barrier(&p);
    let label = if matches!(p.key, Target::Drawn(_)) { "Delete" } else { "Remove wall" };
    if ui.add_enabled(!barrier, egui::Button::new(label)).on_hover_text("Del").clicked() {
        remove_picked(s);
    }
    if barrier {
        ui.label(dim("A race's barriers are switched per race; this map mod cannot remove them."));
    } else {
        ui.label(dim("Removing it takes it out of the game's collision with this map mod; Undo brings it back."));
    }
    true
}

/// The parts of `w` drawn under `show`: (t0, t1, colour). The mod's own walls are drawn whole in their own colour
/// in every mode; with the surface colouring each part has its face's surface colour.
fn parts(layer: &Layer, w: &Wall, show: Show, scope: &Scope) -> Vec<(f32, f32, Color32)> {
    let mut out = Vec::new();
    let weight = scope.weight(layer, w);
    if weight == Weight::Hidden {
        return out;
    }
    let dim = |c: Color32| if weight == Weight::Dim { c.gamma_multiply(0.3) } else { c };
    let by_surface = scope.colour == Colour::Surface;
    let look = &scope.look;
    if by_surface && !look.shows_surface(w.surface) {
        return out;
    }
    if let Some(c) = key::mine(w.mine) {
        if look.shows(mine_class(w.mine)) {
            out.push((0.0, 1.0, dim(c)));
        }
        return out;
    }
    if w.group != 0 {
        if !look.shows(Class::Barrier) {
            return out;
        }
        out.push((0.0, 1.0, if by_surface { key::surface(w.surface) } else { key::BARRIER }));
        return out;
    }
    let bare = if by_surface { key::surface(w.surface) } else { key::INVISIBLE };
    if look.shows(Class::Invisible) {
        out.extend(w.bare.iter().map(|&(a, b)| (a, b, dim(bare))));
    }
    if show == Show::All && look.shows(Class::Behind) {
        let covered = if by_surface { key::surface(w.surface).gamma_multiply(0.5) } else { key::VISIBLE };
        out.extend(w.covered().into_iter().map(|(a, b)| (a, b, dim(covered))));
    }
    out
}

/// The class of a mod's own collision.
fn mine_class(m: Mine) -> Class {
    match m {
        Mine::Added => Class::Added,
        Mine::Moved => Class::Moved,
        _ => Class::Drawn,
    }
}

/// The class a floor belongs to.
fn floor_class(f: &collision::Floor) -> Class {
    match f.mine {
        Mine::Original if f.not_ground => Class::NotGround,
        Mine::Original => Class::Road,
        m => mine_class(m),
    }
}

/// Whether the look shows floor `f`.
fn floor_shown_in(f: &collision::Floor, scope: &Scope) -> bool {
    scope.look.shows(floor_class(f)) && (scope.colour != Colour::Surface || scope.look.shows_surface(f.surface))
}

/// The quad of the part `t0`..`t1` of `w` as the look draws it: a wall the game lets reach from -2048 m to 2048 m
/// full height when asked, else beside the floors.
pub fn wall_quad(w: &Wall, t0: f32, t1: f32, look: &Look) -> [Vec3; 4] {
    let mut q = w.quad(t0, t1);
    if look.full_height && w.tall {
        q[0].z = -2048.0;
        q[1].z = -2048.0;
        q[2].z = 2048.0;
        q[3].z = 2048.0;
    }
    q
}

/// A floor's colour by kind: drivable ground, or a face the wheels pass through.
pub fn floor_colour(f: &collision::Floor) -> Color32 {
    if f.not_ground { key::FLOOR_OFF } else { key::FLOOR }
}

/// A floor's colour under `scope`: the mod's own set apart, else by kind or by surface.
fn floor_shown(f: &collision::Floor, scope: &Scope) -> Color32 {
    match (key::mine(f.mine), scope.colour) {
        (Some(c), _) => c,
        (None, Colour::Surface) => key::surface(f.surface),
        (None, Colour::Kind) => floor_colour(f),
    }
}

/// The walls (and floors) around the camera in the 3D view: each part a
/// see-through quad with its outline.
pub fn paint_3d(layer: &Layer, show: Show, scope: &Scope, o: &Overlay) {
    let cam = o.camera.position;
    if o.camera.fov_y < 5f32.to_radians() {
        // The top-down overview (T): seen from far above, a wall is its
        // foot, drawn as on the Map over the whole picture.
        let half = cam.z.max(1.0) * (o.camera.fov_y * 0.5).tan() * super::camera::aspect(o.rect).max(1.0) * 1.2;
        let (walls, _) = layer.near(cam.truncate() - Vec2::splat(half), cam.truncate() + Vec2::splat(half));
        let feet = |w: &Wall, t0: f32, t1: f32| {
            let q = w.quad(t0, t1);
            Some((o.at(q[0])?, o.at(q[1])?))
        };
        foot_lines(layer, &walls, show, scope, o.painter, &feet);
        return;
    }
    let reach = (WALLS_NEAR + cam.z.max(0.0) * 1.5).min(WALLS_FAR);
    let (walls, _) = layer.near(cam.truncate() - Vec2::splat(reach), cam.truncate() + Vec2::splat(reach));
    {
        let (_, floors) =
            layer.near(cam.truncate() - Vec2::splat(FLOORS_NEAR), cam.truncate() + Vec2::splat(FLOORS_NEAR));
        // The mod's own floors (drawn ground, an added prop's) show in every mode, the original's in All.
        let shown = floors
            .iter()
            .map(|&i| &layer.floors[i as usize])
            .filter(|f| (show == Show::All || f.mine != Mine::Original) && floor_shown_in(f, scope));
        for f in shown.take(MAX_FLOORS) {
            let width = if f.mine == Mine::Original { 1.0 } else { 3.0 };
            quad(o, f.points(), None, key::fill(floor_shown(f, scope), scope.look.alpha(150)), width);
        }
    }
    let mut drawn = 0;
    for &i in &walls {
        let w = &layer.walls[i as usize];
        if (w.a.distance(cam.truncate())).min(w.b.distance(cam.truncate())) > reach + w.length() {
            continue;
        }
        for (t0, t1, c) in parts(layer, w, show, scope) {
            let alpha = if c.a() < 255 {
                20
            } else if c == key::VISIBLE {
                40
            } else {
                70
            };
            let look = &scope.look;
            let fill = (!look.wireframe).then(|| key::fill(c, look.alpha(alpha)));
            let line = c.gamma_multiply(look.opacity as f32 / 100.0);
            quad(o, &wall_quad(w, t0, t1, look), fill, line, if w.mine == Mine::Original { 1.5 } else { 3.0 });
        }
        drawn += 1;
        if drawn >= MAX_WALLS {
            break;
        }
    }
}

/// A world polygon: filled when every corner is in front of the camera,
/// its outline cut at the near plane otherwise.
fn quad(o: &Overlay, points: &[Vec3], fill: Option<Color32>, line: Color32, width: f32) {
    if let Some(fill) = fill {
        let screen: Option<Vec<Pos2>> = points.iter().map(|&p| o.at(p)).collect();
        if let Some(screen) = screen
            && screen.iter().any(|p| o.rect.expand(200.0).contains(*p))
            && thick(&screen)
        {
            o.painter.add(Shape::convex_polygon(screen, fill, Stroke::NONE));
        }
    }
    for k in 0..points.len() {
        let (a, b) = (points[k], points[(k + 1) % points.len()]);
        if let Some((sa, sb)) = super::camera::project_segment(o.camera, o.rect, a, b) {
            let (a, b) = (sa, sb);
            o.painter.line_segment([a, b], Stroke::new(width, line));
        }
    }
}

/// Whether a screen polygon is at least a pixel thick: a wall seen edge-on
/// (from above) is a sliver, and egui's feathered fill of a sliver spikes.
fn thick(screen: &[Pos2]) -> bool {
    let mut area = 0.0;
    let mut longest: f32 = 0.0;
    for k in 0..screen.len() {
        let (a, b) = (screen[k], screen[(k + 1) % screen.len()]);
        area += a.x * b.y - b.x * a.y;
        longest = longest.max(a.distance(b));
    }
    (area * 0.5).abs() >= longest.max(1.0)
}

/// The walls on the Map: each part a line along its foot (`centre` and
/// `scale` the map's, `map` its rectangle).
pub fn paint_map(
    layer: &Layer,
    show: Show,
    scope: &Scope,
    painter: &egui::Painter,
    centre: Vec2,
    scale: f32,
    map: Rect,
) {
    let c = map.center();
    let to = |p: Vec2| Pos2::new(c.x + (p.x - centre.x) * scale, c.y - (p.y - centre.y) * scale);
    let half = Vec2::new(map.width(), map.height()) * 0.5 / scale;
    let (walls, _) = layer.near(centre - half, centre + half);
    let feet = |w: &Wall, t0: f32, t1: f32| Some((to(w.a + (w.b - w.a) * t0), to(w.a + (w.b - w.a) * t1)));
    foot_lines(layer, &walls, show, scope, painter, &feet);
}

/// The parts of `walls` as lines along their feet (`feet` puts a part on
/// screen), the walls behind scenery under the others.
fn foot_lines(
    layer: &Layer,
    walls: &[u32],
    show: Show,
    scope: &Scope,
    painter: &egui::Painter,
    feet: &dyn Fn(&Wall, f32, f32) -> Option<(Pos2, Pos2)>,
) {
    let mut lines: Vec<(Pos2, Pos2, Color32)> = Vec::new();
    for &i in walls {
        let w = &layer.walls[i as usize];
        for (t0, t1, colour) in parts(layer, w, show, scope) {
            if let Some((a, b)) = feet(w, t0, t1) {
                lines.push((a, b, colour));
            }
        }
    }
    lines.sort_by_key(|l| if l.2.a() < 255 || l.2 == key::VISIBLE { 0 } else { 1 });
    let strength = scope.look.opacity as f32 / 100.0;
    for (a, b, colour) in lines {
        let faint = colour.a() < 255 || colour == key::VISIBLE;
        let colour = colour.gamma_multiply(strength);
        if faint {
            painter.line_segment([a, b], Stroke::new(1.0, colour));
        } else {
            // A part shorter than a pixel still shows as a dot.
            let b = if a.distance(b) < 1.5 { a + egui::vec2(1.5, 0.0) } else { b };
            line2d(painter, a, b, colour, 2.0);
        }
    }
}

/// What a legend row switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Class(Class),
    Surface(u8),
}

/// The colour key laid out: its panel and its rows with where each sits.
pub struct Legend {
    pub panel: Rect,
    rows: Vec<LegendRow>,
}

struct LegendRow {
    rect: Rect,
    colour: Color32,
    galley: std::sync::Arc<egui::Galley>,
    toggle: Option<Toggle>,
    shown: bool,
}

impl Legend {
    /// The switch of the row at `at`, if any.
    pub fn hit(&self, at: Pos2) -> Option<Toggle> {
        self.rows.iter().find(|r| r.rect.contains(at)).and_then(|r| r.toggle)
    }
}

/// Flip what a legend row switches (a click on it).
pub fn toggle(look: &mut Look, t: Toggle) {
    match t {
        Toggle::Class(c) => look.toggle(c),
        Toggle::Surface(id) => look.toggle_surface(id),
    }
}

/// The colour key at the view's bottom right (the overview's scale line
/// holds the bottom left), on the overlays' dark fill. A click on a row
/// shows or hides that class.
/// `floors`: the view draws the floors (the 3D view does, the Map not).
pub fn legend(layer: &Layer, show: Show, scope: &Scope, floors: bool, painter: &egui::Painter, rect: Rect) -> Legend {
    let l = legend_layout(layer, show, scope, floors, painter, rect);
    paint_legend(&l, painter);
    l
}

pub fn legend_layout(
    layer: &Layer,
    show: Show,
    scope: &Scope,
    floors: bool,
    painter: &egui::Painter,
    rect: Rect,
) -> Legend {
    let counts = layer.counts();
    let barriers = layer.walls.iter().filter(|w| w.group != 0 && scope.weight(layer, w) == Weight::Full).count();
    let barrier_title = match scope.focus {
        super::state::Focus::Races => "This race's barriers",
        super::state::Focus::World if !scope.race_barriers => "Free roam and career barriers",
        _ => "Race and career barriers",
    };
    let look = &scope.look;
    let mut rows: Vec<(Color32, String, Option<Toggle>)> = Vec::new();
    if scope.colour == Colour::Surface {
        // By surface: one row per surface id the region has (the commonest first, a few), then the rest.
        let mut all = layer.surfaces();
        all.sort_by_key(|&(id, n)| (std::cmp::Reverse(n), id));
        for &(id, n) in all.iter().take(SURFACE_ROWS) {
            rows.push((key::surface(id), format!("Surface {id} ({n})"), Some(Toggle::Surface(id))));
        }
        if all.len() > SURFACE_ROWS {
            let rest: usize = all[SURFACE_ROWS..].iter().map(|r| r.1).sum();
            rows.push((colour::DIM, format!("{} more surfaces ({rest})", all.len() - SURFACE_ROWS), None));
        }
    } else {
        rows.push((key::INVISIBLE, format!("Invisible walls ({})", counts.invisible), Some(Toggle::Class(Class::Invisible))));
        rows.push((key::BARRIER, format!("{barrier_title} ({barriers})"), Some(Toggle::Class(Class::Barrier))));
        if show == Show::All {
            rows.push((key::VISIBLE, format!("Walls behind scenery ({})", counts.visible), Some(Toggle::Class(Class::Behind))));
            if floors {
                let off = layer.floors.iter().filter(|f| f.not_ground).count();
                rows.push((key::FLOOR, format!("Road floors ({})", counts.floors - off), Some(Toggle::Class(Class::Road))));
                if off > 0 {
                    rows.push((key::FLOOR_OFF, format!("Floors that are not ground ({off})"), Some(Toggle::Class(Class::NotGround))));
                }
            }
        }
    }
    let (added, moved, drawn) = layer.mine();
    for (m, n, what) in [(Mine::Added, added, "Added props'"), (Mine::Moved, moved, "Moved props'"), (Mine::Drawn, drawn, "Drawn by this mod")] {
        if let (Some(c), true) = (key::mine(m), n > 0) {
            rows.push((c, format!("{what} ({n})"), Some(Toggle::Class(mine_class(m)))));
        }
    }
    let font = egui::FontId::proportional(13.0);
    let galleys: Vec<_> = rows
        .iter()
        .map(|(_, t, _)| painter.layout_no_wrap(t.clone(), font.clone(), colour::INK))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 12.0 + 6.0 + 16.0;
    let row_h = 18.0;
    let title_h = 18.0;
    let height = title_h + row_h * rows.len() as f32 + 10.0;
    let size = egui::vec2(width.max(160.0), height);
    let r = Rect::from_min_size(Pos2::new(rect.right() - 10.0 - size.x, rect.bottom() - 10.0 - height), size);
    let rows = rows
        .into_iter()
        .zip(galleys)
        .enumerate()
        .map(|(k, ((colour, _, toggle), galley))| {
            let y = r.top() + 5.0 + title_h + row_h * k as f32;
            let shown = match toggle {
                Some(Toggle::Class(c)) => look.shows(c),
                Some(Toggle::Surface(id)) => look.shows_surface(id),
                None => true,
            };
            LegendRow { rect: Rect::from_min_size(Pos2::new(r.left(), y), egui::vec2(r.width(), row_h)), colour, galley, toggle, shown }
        })
        .collect();
    Legend { panel: r, rows }
}

pub fn paint_legend(l: &Legend, painter: &egui::Painter) {
    let font = egui::FontId::proportional(13.0);
    painter.rect_filled(l.panel, 6.0, colour::SHADOW);
    painter.text(
        Pos2::new(l.panel.left() + 8.0, l.panel.top() + 5.0),
        egui::Align2::LEFT_TOP,
        "Collision",
        font,
        colour::DIM,
    );
    for row in &l.rows {
        let sw = Rect::from_min_size(Pos2::new(row.rect.left() + 8.0, row.rect.top() + 2.0), egui::vec2(12.0, 12.0));
        let text_at = Pos2::new(sw.right() + 6.0, row.rect.top());
        if row.shown {
            painter.rect_filled(sw, 2.0, row.colour);
            painter.galley(text_at, row.galley.clone(), colour::INK);
        } else {
            // Switched off: an empty swatch and a struck-through name.
            painter.rect_stroke(sw, 2.0, Stroke::new(1.0, row.colour), egui::StrokeKind::Inside);
            painter.galley(text_at, row.galley.clone(), colour::DIM);
            let y = row.rect.center().y;
            painter.line_segment(
                [Pos2::new(text_at.x, y), Pos2::new(text_at.x + row.galley.size().x, y)],
                Stroke::new(1.0, colour::DIM),
            );
        }
    }
}

#[cfg(test)]
#[path = "collision_tests.rs"]
mod tests;
