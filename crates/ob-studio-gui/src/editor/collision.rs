//! The collision layer over the viewport (View ▸ Collision,
//! docs/modding/map-editor.md "Collision"): the original's static collision
//! from the region's collision packs, as the game loads them, drawn in the
//! 3D view and on the Map. Off, the invisible barriers only (walls no
//! scenery stands at, and the race and career barriers), or all collision
//! (also the walls behind scenery and the floors). Read-only.
//!
//! The layer is built the first time it is shown, from the world as the
//! original places it (`session.base`), so a wall counts as invisible where
//! the original game shows nothing at it.

use egui::{Color32, Pos2, Rect, Shape, Stroke};
use glam::{Vec2, Vec3};

use super::overlay::{Overlay, colour, line2d};
use super::state::{EditorState, View};
use crate::tools::map::collision::{self, Layer, Wall};

/// What the layer shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Show {
    #[default]
    Off,
    /// Invisible walls and the race and career barriers.
    Invisible,
    /// Also the walls behind scenery and the floors.
    All,
}

impl Show {
    pub const ALL: [Show; 3] = [Show::Off, Show::Invisible, Show::All];

    /// Its name in the View menu.
    pub fn label(self) -> &'static str {
        match self {
            Show::Off => "Off",
            Show::Invisible => "Invisible barriers",
            Show::All => "All collision",
        }
    }

    /// `--collision off|invisible|all`.
    pub fn parse(v: &str) -> Option<Show> {
        match v {
            "off" => Some(Show::Off),
            "invisible" => Some(Show::Invisible),
            "all" => Some(Show::All),
            _ => None,
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

    /// A wall's fill: its colour, see-through.
    pub fn fill(c: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
    }
}

/// How far from the camera the 3D view draws walls and floors (metres; the
/// walls' reach grows with the camera's height, for the overview).
const WALLS_NEAR: f32 = 300.0;
const WALLS_FAR: f32 = 2500.0;
const FLOORS_NEAR: f32 = 120.0;
/// At most this many walls and floors in one frame (nearest cells first).
const MAX_WALLS: usize = 8000;
const MAX_FLOORS: usize = 4000;

/// The scenery triangles the cover test looks at: the original's
/// placements as drawn in free roam (no panorama, no race-only or
/// group-switched pieces), see-through ones included (a fence is seen).
pub fn scenery(s: &EditorState) -> Vec<[Vec3; 3]> {
    let world = &s.session.world;
    let mut out = Vec::new();
    for p in &s.session.base.placements {
        let Some(solid) = world.solids.get(p.solid) else { continue };
        if solid.name.starts_with("PAN_") || p.flags & (crate::world::HIDDEN_FLAG | RACE_ONLY) != 0 {
            continue;
        }
        for tri in solid.indices.chunks_exact(3) {
            let Some(c) = (0..3)
                .map(|i| {
                    solid.vertices.get(tri[i] as usize).map(|v| p.transform.transform_point3(Vec3::from(v.position)))
                })
                .collect::<Option<Vec<Vec3>>>()
            else {
                continue;
            };
            out.push([c[0], c[1], c[2]]);
        }
    }
    out
}

/// The race-only scenery flag (`world::scenery`'s 0x200).
const RACE_ONLY: u16 = 0x0200;

/// The layer of the open region (read from the loaded collision packs).
pub fn layer_of(s: &EditorState) -> Layer {
    let t = std::time::Instant::now();
    let scenery = scenery(s);
    let world = &s.session.world;
    let layer = collision::build(world.ground.faces(), &world.barrier_groups, &scenery);
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
    /// The region it was built for, and the layer.
    layer: Option<(String, Layer)>,
}

impl CollisionOverlay {
    fn layer(&mut self, s: &EditorState) -> &Layer {
        if self.layer.as_ref().is_none_or(|(r, _)| *r != s.session.region) {
            self.layer = Some((s.session.region.clone(), layer_of(s)));
        }
        &self.layer.as_ref().unwrap().1
    }
}

impl super::app::Overlay for CollisionOverlay {
    fn overlay(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) -> bool {
        let show = s.collision;
        if show == Show::Off {
            return false;
        }
        let map = super::sim::panel::map_transform(ctx);
        let layer = self.layer(s);
        match s.view {
            View::ThreeD => paint_3d(layer, show, &Overlay::new(painter, &s.camera, rect)),
            View::Map => {
                if let Some((centre, scale, map)) = map {
                    paint_map(layer, show, painter, centre, scale, map);
                }
            }
        }
        let floors = s.view == View::ThreeD && s.camera.fov_y >= 5f32.to_radians();
        legend(layer, show, floors, painter, rect);
        false
    }
}

/// The parts of `w` drawn under `show`: (t0, t1, colour).
fn parts(w: &Wall, show: Show) -> Vec<(f32, f32, Color32)> {
    let mut out = Vec::new();
    if w.group != 0 {
        out.push((0.0, 1.0, key::BARRIER));
        return out;
    }
    out.extend(w.bare.iter().map(|&(a, b)| (a, b, key::INVISIBLE)));
    if show == Show::All {
        out.extend(w.covered().into_iter().map(|(a, b)| (a, b, key::VISIBLE)));
    }
    out
}

/// The walls (and floors) around the camera in the 3D view: each part a
/// see-through quad with its outline.
pub fn paint_3d(layer: &Layer, show: Show, o: &Overlay) {
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
        foot_lines(layer, &walls, show, o.painter, &feet);
        return;
    }
    let reach = (WALLS_NEAR + cam.z.max(0.0) * 1.5).min(WALLS_FAR);
    let (walls, _) = layer.near(cam.truncate() - Vec2::splat(reach), cam.truncate() + Vec2::splat(reach));
    if show == Show::All {
        let (_, floors) =
            layer.near(cam.truncate() - Vec2::splat(FLOORS_NEAR), cam.truncate() + Vec2::splat(FLOORS_NEAR));
        for &i in floors.iter().take(MAX_FLOORS) {
            let f = &layer.floors[i as usize];
            quad(o, f.points(), None, key::fill(key::FLOOR, 150), 1.0);
        }
    }
    let mut drawn = 0;
    for &i in &walls {
        let w = &layer.walls[i as usize];
        if (w.a.distance(cam.truncate())).min(w.b.distance(cam.truncate())) > reach + w.length() {
            continue;
        }
        for (t0, t1, c) in parts(w, show) {
            let alpha = if c == key::VISIBLE { 40 } else { 70 };
            quad(o, &w.quad(t0, t1), Some(key::fill(c, alpha)), c, 1.5);
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
pub fn paint_map(layer: &Layer, show: Show, painter: &egui::Painter, centre: Vec2, scale: f32, map: Rect) {
    let c = map.center();
    let to = |p: Vec2| Pos2::new(c.x + (p.x - centre.x) * scale, c.y - (p.y - centre.y) * scale);
    let half = Vec2::new(map.width(), map.height()) * 0.5 / scale;
    let (walls, _) = layer.near(centre - half, centre + half);
    let feet = |w: &Wall, t0: f32, t1: f32| Some((to(w.a + (w.b - w.a) * t0), to(w.a + (w.b - w.a) * t1)));
    foot_lines(layer, &walls, show, painter, &feet);
}

/// The parts of `walls` as lines along their feet (`feet` puts a part on
/// screen), the walls behind scenery under the others.
fn foot_lines(
    layer: &Layer,
    walls: &[u32],
    show: Show,
    painter: &egui::Painter,
    feet: &dyn Fn(&Wall, f32, f32) -> Option<(Pos2, Pos2)>,
) {
    let mut lines: Vec<(Pos2, Pos2, Color32)> = Vec::new();
    for &i in walls {
        let w = &layer.walls[i as usize];
        for (t0, t1, colour) in parts(w, show) {
            if let Some((a, b)) = feet(w, t0, t1) {
                lines.push((a, b, colour));
            }
        }
    }
    lines.sort_by_key(|l| if l.2 == key::VISIBLE { 0 } else { 1 });
    for (a, b, colour) in lines {
        if colour == key::VISIBLE {
            painter.line_segment([a, b], Stroke::new(1.0, colour));
        } else {
            // A part shorter than a pixel still shows as a dot.
            let b = if a.distance(b) < 1.5 { a + egui::vec2(1.5, 0.0) } else { b };
            line2d(painter, a, b, colour, 2.0);
        }
    }
}

/// The colour key at the view's bottom right (the overview's scale line
/// holds the bottom left), on the overlays' dark fill.
/// `floors`: the view draws the floors (the 3D view does, the Map not).
pub fn legend(layer: &Layer, show: Show, floors: bool, painter: &egui::Painter, rect: Rect) {
    let counts = layer.counts();
    let mut rows = vec![
        (key::INVISIBLE, format!("Invisible walls ({})", counts.invisible)),
        (key::BARRIER, format!("Race and career barriers ({})", counts.barriers)),
    ];
    if show == Show::All {
        rows.push((key::VISIBLE, format!("Walls behind scenery ({})", counts.visible)));
        if floors {
            rows.push((key::FLOOR, format!("Floors ({})", counts.floors)));
        }
    }
    let font = egui::FontId::proportional(13.0);
    let galleys: Vec<_> =
        rows.iter().map(|(_, t)| painter.layout_no_wrap(t.clone(), font.clone(), colour::INK)).collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 12.0 + 6.0 + 16.0;
    let row_h = 18.0;
    let title_h = 18.0;
    let height = title_h + row_h * rows.len() as f32 + 10.0;
    let size = egui::vec2(width.max(160.0), height);
    let r = Rect::from_min_size(Pos2::new(rect.right() - 10.0 - size.x, rect.bottom() - 10.0 - height), size);
    painter.rect_filled(r, 6.0, colour::SHADOW);
    painter.text(
        Pos2::new(r.left() + 8.0, r.top() + 5.0),
        egui::Align2::LEFT_TOP,
        "Collision",
        font.clone(),
        colour::DIM,
    );
    for (k, ((c, _), g)) in rows.iter().zip(galleys).enumerate() {
        let y = r.top() + 5.0 + title_h + row_h * k as f32;
        let sw = Rect::from_min_size(Pos2::new(r.left() + 8.0, y + 2.0), egui::vec2(12.0, 12.0));
        painter.rect_filled(sw, 2.0, *c);
        painter.galley(Pos2::new(sw.right() + 6.0, y), g, colour::INK);
    }
}

#[cfg(test)]
#[path = "collision_tests.rs"]
mod tests;
