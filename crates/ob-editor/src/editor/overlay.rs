//! Drawing over the viewport: the look's colours as egui colours and helpers
//! that put world-space lines, boxes, markers and labels on screen with
//! egui's painter (no renderer change; overlays draw over the world, with
//! no depth test).
//!
//! Colours come from the launcher's tokens (`launch::theme::THEME_A`):
//! lime for the selection and focus, ink / dim for neutral marks, the
//! semantic red / amber for errors and warnings.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};
use glam::{Mat4, Vec3};

use super::camera::{project, project_segment};
use crate::draw::camera::Camera;
use crate::launch::theme::{Rgb, Rgba, THEME_A};

/// An opaque token as an egui colour.
pub const fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A token with alpha as an egui colour (egui wants premultiplied).
pub const fn rgba(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied_const(c.0, c.1, c.2, c.3)
}

/// The overlay palette.
pub mod colour {
    use super::*;

    /// The selection, the current race point, focus.
    pub const SELECT: Color32 = rgb(THEME_A.lime);
    /// Neutral marks: checkpoints, grid slots, the camera on the map.
    pub const INK: Color32 = rgb(THEME_A.ink);
    pub const DIM: Color32 = rgb(THEME_A.dim);
    /// Roads on the map, hover outlines.
    pub const ROAD: Color32 = rgb(THEME_A.rail);
    pub const ERROR: Color32 = rgb(THEME_A.err);
    pub const WARN: Color32 = rgb(THEME_A.warn);
    /// The dark rim under every overlay line so it reads on bright ground.
    pub const SHADOW: Color32 = Color32::from_rgba_premultiplied(2, 3, 2, 170);
    /// The page under the map view.
    pub const PAGE: Color32 = rgb(THEME_A.page);
    /// Text on a lime fill.
    pub const ON_SELECT: Color32 = rgb(THEME_A.on_mark);
}

/// A painter clipped to the viewport plus the camera it projects with.
pub struct Overlay<'a> {
    pub painter: &'a Painter,
    pub camera: &'a Camera,
    pub rect: Rect,
}

impl<'a> Overlay<'a> {
    pub fn new(painter: &'a Painter, camera: &'a Camera, rect: Rect) -> Overlay<'a> {
        Overlay { painter, camera, rect }
    }

    /// Screen position of a world point (`None` behind the camera).
    pub fn at(&self, p: Vec3) -> Option<Pos2> {
        project(self.camera, self.rect, p)
    }

    /// A world-space line, `width` logical pixels, with a dark rim.
    pub fn line(&self, a: Vec3, b: Vec3, color: Color32, width: f32) {
        if let Some((a, b)) = project_segment(self.camera, self.rect, a, b) {
            line2d(self.painter, a, b, color, width);
        }
    }

    /// A world-space polyline (closed when `closed`).
    pub fn polyline(&self, points: &[Vec3], closed: bool, color: Color32, width: f32) {
        // Rims first, then the lines, so joins stay clean.
        let segs: Vec<(Pos2, Pos2)> =
            segments(points, closed).filter_map(|(a, b)| project_segment(self.camera, self.rect, a, b)).collect();
        for &(a, b) in &segs {
            self.painter.line_segment([a, b], Stroke::new(width + 2.0, colour::SHADOW));
        }
        for &(a, b) in &segs {
            self.painter.line_segment([a, b], Stroke::new(width, color));
        }
    }

    /// The 12 edges of a box `lo`..`hi` in the local space of `transform`.
    pub fn box_outline(&self, transform: Mat4, lo: Vec3, hi: Vec3, color: Color32, width: f32) {
        let corners = box_corners(transform, lo, hi);
        let segs: Vec<(Pos2, Pos2)> = BOX_EDGES
            .iter()
            .filter_map(|&(i, j)| project_segment(self.camera, self.rect, corners[i], corners[j]))
            .collect();
        for &(a, b) in &segs {
            self.painter.line_segment([a, b], Stroke::new(width + 2.0, colour::SHADOW));
        }
        for &(a, b) in &segs {
            self.painter.line_segment([a, b], Stroke::new(width, color));
        }
    }

    /// The selection outline of placement `i` of `world` (lime box).
    pub fn selection(&self, world: &crate::world::World, i: usize) {
        if let Some((transform, lo, hi)) = placement_box(world, i) {
            self.box_outline(transform, lo, hi, colour::SELECT, 2.0);
        }
    }

    /// A round marker at a world point: filled `fill` with an optional
    /// number or letter in it; the ring takes `ring`.
    pub fn marker(&self, p: Vec3, radius: f32, fill: Color32, ring: Color32, label: Option<&str>) -> Option<Pos2> {
        let s = self.at(p)?;
        marker2d(self.painter, s, radius, fill, ring, label);
        Some(s)
    }

    /// A text label just above a world point, on a dark pill.
    pub fn label(&self, p: Vec3, text: &str, color: Color32) -> Option<Pos2> {
        let s = self.at(p)?;
        label2d(self.painter, s - egui::vec2(0.0, 14.0), text, color);
        Some(s)
    }

    /// An arrow on the ground from `p` along `heading` (radians around +Z
    /// from +X, as a `Spot`'s heading in degrees is), `length` metres:
    /// start lines, grid slots, finish.
    pub fn heading_arrow(&self, p: Vec3, heading: f32, length: f32, color: Color32, width: f32) {
        let d = Vec3::new(heading.cos(), heading.sin(), 0.0) * length;
        let side = Vec3::new(-d.y, d.x, 0.0) * 0.3;
        let tip = p + d;
        self.polyline(&[p, tip], false, color, width);
        self.polyline(&[tip - d * 0.35 + side, tip, tip - d * 0.35 - side], false, color, width);
    }

    /// A line across the road through `p`, perpendicular to `heading`,
    /// `half` metres each side (start / finish lines).
    pub fn cross_line(&self, p: Vec3, heading: f32, half: f32, color: Color32, width: f32) {
        let side = Vec3::new(-heading.sin(), heading.cos(), 0.0) * half;
        self.polyline(&[p - side, p + side], false, color, width);
    }
}

/// Consecutive pairs of `points`, plus last→first when `closed`.
fn segments(points: &[Vec3], closed: bool) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    let wrap = (closed && points.len() > 2).then(|| (points[points.len() - 1], points[0]));
    points.windows(2).map(|w| (w[0], w[1])).chain(wrap)
}

/// The box's corners in world space; bit 0 → x, bit 1 → y, bit 2 → z.
pub fn box_corners(transform: Mat4, lo: Vec3, hi: Vec3) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        let c = Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        transform.transform_point3(c)
    })
}

/// The box's edges as corner pairs (corners differing in one bit).
pub const BOX_EDGES: [(usize, usize); 12] =
    [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

/// Placement `i`'s transform and its solid's local box.
pub fn placement_box(world: &crate::world::World, i: usize) -> Option<(Mat4, Vec3, Vec3)> {
    let p = world.placements.get(i)?;
    let s = world.solids.get(p.solid)?;
    Some((p.transform, Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max)))
}

/// A screen-space line with a dark rim.
pub fn line2d(painter: &Painter, a: Pos2, b: Pos2, color: Color32, width: f32) {
    painter.line_segment([a, b], Stroke::new(width + 2.0, colour::SHADOW));
    painter.line_segment([a, b], Stroke::new(width, color));
}

/// A screen-space round marker with an optional short label inside.
pub fn marker2d(painter: &Painter, at: Pos2, radius: f32, fill: Color32, ring: Color32, label: Option<&str>) {
    painter.add(Shape::circle_filled(at, radius + 1.5, colour::SHADOW));
    painter.circle(at, radius, fill, Stroke::new(1.5, ring));
    if let Some(text) = label {
        let ink =
            if fill.r() as u32 + fill.g() as u32 + fill.b() as u32 > 384 { colour::ON_SELECT } else { colour::INK };
        painter.text(at, Align2::CENTER_CENTER, text, FontId::proportional(radius * 1.2), ink);
    }
}

/// A text label centred above `at` on a dark pill.
pub fn label2d(painter: &Painter, at: Pos2, text: &str, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(13.0), color);
    let rect = Rect::from_center_size(at, galley.size()).expand2(egui::vec2(6.0, 2.0));
    painter.rect_filled(rect, rect.height() * 0.5, colour::SHADOW);
    painter.galley(rect.min + egui::vec2(6.0, 2.0), galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::draw::camera::Camera;

    #[test]
    fn box_edges_join_corners_one_axis_apart() {
        for (i, j) in BOX_EDGES {
            assert_eq!((i ^ j).count_ones(), 1);
        }
        let c = box_corners(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)), Vec3::splat(-1.0), Vec3::ONE);
        assert_eq!(c[0], Vec3::new(9.0, -1.0, -1.0));
        assert_eq!(c[7], Vec3::new(11.0, 1.0, 1.0));
    }

    #[test]
    fn closed_polylines_wrap() {
        let p = [Vec3::ZERO, Vec3::X, Vec3::Y];
        assert_eq!(segments(&p, false).count(), 2);
        assert_eq!(segments(&p, true).count(), 3);
        assert_eq!(segments(&p[..2], true).count(), 1);
    }

    #[test]
    fn palette_is_the_launchers() {
        assert_eq!(colour::SELECT, Color32::from_rgb(0xc6, 0xf0, 0x87));
        assert_eq!(colour::ERROR, Color32::from_rgb(0xff, 0x7a, 0x6b));
    }

    /// The painter output of `draw` over a 800×600 viewport.
    fn shapes(draw: impl FnOnce(&Overlay)) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        let mut draw = Some(draw);
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
            let painter = ui.painter_at(rect);
            let camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
            (draw.take().unwrap())(&Overlay::new(&painter, &camera, rect));
        });
        out.shapes
    }

    fn segments_drawn(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Pos2, Pos2, Color32)> {
        shapes
            .iter()
            .filter_map(|s| match &s.shape {
                Shape::LineSegment { points, stroke } => Some((points[0], points[1], stroke.color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_box_in_front_draws_twelve_edges_over_their_rims() {
        let t = Mat4::from_translation(Vec3::new(0.0, 20.0, 0.0));
        let lines = segments_drawn(&shapes(|o| o.box_outline(t, Vec3::splat(-1.0), Vec3::ONE, colour::SELECT, 2.0)));
        assert_eq!(lines.len(), 24);
        assert!(lines[..12].iter().all(|l| l.2 == colour::SHADOW));
        assert!(lines[12..].iter().all(|l| l.2 == colour::SELECT));
        // Around the centre of the view.
        let centre = Pos2::new(400.0, 300.0);
        assert!(lines.iter().all(|l| (l.0 - centre).length() < 100.0 && (l.1 - centre).length() < 100.0));
    }

    #[test]
    fn a_box_behind_draws_nothing_and_a_crossing_line_is_cut() {
        let t = Mat4::from_translation(Vec3::new(0.0, -20.0, 0.0));
        assert!(
            segments_drawn(&shapes(|o| o.box_outline(t, Vec3::splat(-1.0), Vec3::ONE, colour::SELECT, 2.0))).is_empty()
        );
        let lines = segments_drawn(&shapes(|o| {
            o.line(Vec3::new(1.0, 10.0, 0.0), Vec3::new(1.0, -10.0, 0.0), colour::INK, 1.0)
        }));
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.0.x.is_finite() && l.1.x.is_finite()));
    }
}
