//! The 2D Map view (M): the whole region from above — the drivable
//! sections' outlines and the free roam's roads — with the current race's
//! base track network and its closed barriers, the selection and the 3D
//! camera on top (the races are drawn over it by the races' overlay in
//! `app::Panels`). It opens fitted to the region; drag pans, the
//! wheel zooms around the mouse, Home fits the region again, a
//! double-click moves the 3D camera there.
//!
//! World X runs right and world Y up on screen (the game's map is Z-up,
//! so this is the view from above).

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke};
use glam::{Vec2, Vec3};

use super::overlay::{colour, label2d, line2d};
use super::state::EditorState;
use crate::formats::roads::RoadNet;

/// Zoom limits, screen points per metre.
pub const MIN_SCALE: f32 = 0.01;
pub const MAX_SCALE: f32 = 20.0;
/// One wheel notch zooms by this.
pub const ZOOM_STEP: f32 = 1.2;

/// The map's pan and zoom.
#[derive(Clone, Debug)]
pub struct MapView {
    /// The world point at the rectangle's centre.
    pub centre: Vec2,
    /// Screen points per metre.
    pub scale: f32,
    /// Fitted to a network once (the first time one is shown).
    pub fitted: bool,
    /// The rectangle the map took this frame.
    pub rect: Rect,
    /// The roads shown when no race is chosen ([`overview_net`]; read
    /// once, None inside: unreadable).
    overview: Option<Option<(u16, RoadNet)>>,
    /// The drivable sections' outlines (x, y), read from the world once.
    outlines: Option<Vec<Vec<Vec2>>>,
}

impl Default for MapView {
    fn default() -> MapView {
        MapView { centre: Vec2::ZERO, scale: 0.2, fitted: false, rect: Rect::NOTHING, overview: None, outlines: None }
    }
}

impl MapView {
    /// A world point (x, y) on screen.
    pub fn to_screen(&self, p: Vec2) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(c.x + (p.x - self.centre.x) * self.scale, c.y - (p.y - self.centre.y) * self.scale)
    }

    /// A screen point in the world (x, y).
    pub fn to_world(&self, s: Pos2) -> Vec2 {
        let c = self.rect.center();
        Vec2::new(self.centre.x + (s.x - c.x) / self.scale, self.centre.y - (s.y - c.y) / self.scale)
    }

    /// Zoom by `factor` keeping the world point under `at` where it is.
    pub fn zoom_at(&mut self, at: Pos2, factor: f32) {
        let before = self.to_world(at);
        self.scale = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
        let after = self.to_world(at);
        self.centre += before - after;
    }

    /// Pan by a screen movement (the map follows the mouse).
    pub fn pan(&mut self, by: egui::Vec2) {
        self.centre += Vec2::new(-by.x, by.y) / self.scale;
    }

    /// Show the box `lo`..`hi` (world x, y) whole, with a margin.
    pub fn fit(&mut self, lo: Vec2, hi: Vec2) {
        let size = (hi - lo).max(Vec2::splat(1.0));
        self.centre = (lo + hi) * 0.5;
        let (w, h) = (self.rect.width().max(1.0), self.rect.height().max(1.0));
        self.scale = ((w / size.x).min(h / size.y) * 0.9).clamp(MIN_SCALE, MAX_SCALE);
        self.fitted = true;
    }

    /// Read the networks the map needs and the state does not hold yet:
    /// the current race's base track's (into `s.nets`, its race barriers
    /// closed as `tools::map::race::road_net` does) and the overview.
    pub fn load_networks(&mut self, s: &mut EditorState) {
        if let Some(t) = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| r.track)
            && !s.nets.contains_key(&t)
        {
            let net = s.assets.road_net(t);
            s.nets.insert(t, net);
        }
        if self.overview.is_none() {
            self.overview = Some(overview_net(s.assets.as_ref(), &s.session.region));
        }
    }

    /// The network the map shows: the current race's base track's, else
    /// the lowest-numbered one loaded, else the region's overview.
    pub fn network<'s>(&'s self, s: &'s EditorState) -> Option<(u16, &'s RoadNet)> {
        network(s, &self.overview)
    }

    fn race_network(s: &EditorState) -> Option<(u16, &RoadNet)> {
        let race_track = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| r.track);
        if let Some(t) = race_track
            && let Some(Some(net)) = s.nets.get(&t)
        {
            return Some((t, net));
        }
        let mut loaded: Vec<(u16, &RoadNet)> = s.nets.iter().filter_map(|(&t, n)| n.as_ref().map(|n| (t, n))).collect();
        loaded.sort_by_key(|&(t, _)| t);
        loaded.into_iter().next()
    }

    /// The panel.
    pub fn show(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = rect;
        crate::editor::sim::panel::publish_map(ui.ctx(), self.centre, self.scale, rect);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, colour::PAGE);

        self.load_networks(s);
        // Held outside `self` while drawing (the view changes as it draws).
        let overview = self.overview.take();
        let outlines = self.outlines.take().unwrap_or_else(|| region_outlines(&s.session.world));
        let net = network(s, &overview);
        let region =
            region_bounds(&outlines, overview.as_ref().and_then(|o| o.as_ref()).map(|(_, n)| n), net.map(|(_, n)| n));
        if !self.fitted {
            match region {
                Some((lo, hi)) => self.fit(lo, hi),
                None => self.centre = s.camera.position.truncate(),
            }
        }

        // Input: drag pans (either button), the wheel zooms, Home fits,
        // a double-click moves the 3D camera there; not while an overlay
        // has the pointer (a race point placed or dragged).
        let ctx = ui.ctx().clone();
        let taken = super::app::pointer_taken(&ctx);
        if response.dragged() && !taken {
            self.pan(response.drag_delta());
        }
        if let Some(at) = response.hover_pos() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom_at(at, ZOOM_STEP.powf(scroll / 50.0));
            }
        }
        if response.hovered()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Home))
            && let Some((lo, hi)) = region
        {
            self.fit(lo, hi);
        }
        let move_to =
            if response.double_clicked() && !taken { response.interact_pointer_pos().map(|at| self.to_world(at)) } else { None };

        // The region: every drivable section's outline.
        let edge = Stroke::new(1.0, colour::DIM.gamma_multiply(0.3));
        let view = rect.expand(20.0);
        for outline in &outlines {
            let pts: Vec<Pos2> = outline.iter().map(|&p| self.to_screen(p)).collect();
            if pts.len() >= 3 && pts.iter().any(|p| view.contains(*p)) {
                painter.add(egui::Shape::closed_line(pts, edge));
            }
        }

        // The road network.
        if let Some((track, net)) = net {
            // The whole region's roads faintly under a race's network.
            if track != 0
                && let Some(Some((_, all))) = &overview
            {
                self.draw_net(&painter, all, colour::ROAD.gamma_multiply(0.5));
            }
            self.draw_net(&painter, net, colour::ROAD);
            painter.text(
                rect.left_top() + egui::vec2(10.0, 8.0),
                Align2::LEFT_TOP,
                if track == 0 { "Free roam roads".to_string() } else { format!("Track {track}") },
                FontId::proportional(13.0),
                colour::DIM,
            );
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No road network loaded",
                FontId::proportional(15.0),
                colour::DIM,
            );
        }

        // The races are drawn by the races' overlay (`app::Panels`).

        // The selection and the 3D camera.
        for &i in &s.selection {
            if let Some(p) = s.session.world.placements.get(i) {
                painter.circle_stroke(
                    self.to_screen(p.transform.w_axis.truncate().truncate()),
                    5.0,
                    Stroke::new(2.0, colour::SELECT),
                );
            }
        }
        self.draw_camera(&painter, s);
        if let Some(at) = response.hover_pos() {
            let p = self.to_world(at);
            label2d(
                &painter,
                rect.left_bottom() + egui::vec2(70.0, -14.0),
                &format!("{:.0}, {:.0}", p.x, p.y),
                colour::DIM,
            );
        }
        self.overview = overview;
        self.outlines = Some(outlines);
        if let Some(p) = move_to {
            s.camera.position.x = p.x;
            s.camera.position.y = p.y;
        }
    }

    fn draw_net(&self, painter: &egui::Painter, net: &RoadNet, color: Color32) {
        let road = Stroke::new(1.5, color);
        let closed = Stroke::new(1.5, colour::ERROR.gamma_multiply(0.6));
        for seg in &net.segments {
            let shut = seg.barriers.iter().any(|&b| net.closed.get(b).copied().unwrap_or(false));
            let stroke = if shut { closed } else { road };
            let pts: Vec<Pos2> = net
                .nodes
                .get(seg.first..seg.end.min(net.nodes.len()))
                .unwrap_or(&[])
                .iter()
                .map(|&(p, _)| self.to_screen(p))
                .collect();
            if pts.len() >= 2 && pts.iter().any(|p| self.rect.expand(50.0).contains(*p)) {
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        for (b, ends) in net.barrier_ends.iter().enumerate() {
            if net.closed.get(b).copied().unwrap_or(false) {
                line2d(painter, self.to_screen(ends[0]), self.to_screen(ends[1]), colour::ERROR, 2.0);
            }
        }
    }

    fn draw_camera(&self, painter: &egui::Painter, s: &EditorState) {
        let p = self.to_screen(s.camera.position.truncate());
        let f = s.camera.forward();
        let dir = Vec2::new(f.x, f.y).normalize_or(Vec2::Y);
        let d = egui::vec2(dir.x, -dir.y);
        let side = egui::vec2(-d.y, d.x);
        let tip = p + d * 14.0;
        let pts = vec![tip, p - d * 6.0 + side * 7.0, p - d * 6.0 - side * 7.0];
        painter.add(egui::Shape::convex_polygon(pts, colour::SELECT, Stroke::new(1.5, Color32::BLACK)));
    }
}

/// The race's network, else the overview.
fn network<'s>(s: &'s EditorState, overview: &'s Option<Option<(u16, RoadNet)>>) -> Option<(u16, &'s RoadNet)> {
    MapView::race_network(s).or_else(|| overview.as_ref()?.as_ref().map(|(t, n)| (*t, n)))
}

/// Polylines (route records' points) as a barrier-free road network,
/// one segment each.
fn net_of_lines<'a>(lines: impl Iterator<Item = &'a [Vec3]>) -> RoadNet {
    let mut net = RoadNet::default();
    for points in lines {
        let first = net.nodes.len();
        net.nodes.extend(points.iter().map(|p| (p.truncate(), net.segments.len())));
        let length = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        net.segments.push(crate::formats::roads::Segment {
            first,
            end: net.nodes.len(),
            length,
            spline_length: length,
            exits: [Vec::new(), Vec::new()],
            barriers: Vec::new(),
        });
    }
    net
}

/// The region's roads as a whole, for the map without a race: the
/// free roam's AI routes (every lane the free roam's traffic drives, read
/// through the assets), each a segment of a network with no barriers. Its
/// number is 0 here.
fn overview_net(assets: &dyn crate::tools::map::assets::MapAssets, region: &str) -> Option<(u16, RoadNet)> {
    let lines = assets.free_roam_lines(region);
    Some((0, net_of_lines(lines.iter().map(Vec::as_slice)))).filter(|(_, n)| !n.nodes.is_empty())
}

/// The drivable sections' outlines on the ground: together they draw the
/// region's shape.
pub fn region_outlines(world: &crate::world::World) -> Vec<Vec<Vec2>> {
    world.visibility.drivable().map(|(_, b)| b.points.clone()).collect()
}

/// The region's extent (x, y): the outlines and the free roam's roads,
/// else the network shown.
pub fn region_bounds(outlines: &[Vec<Vec2>], roads: Option<&RoadNet>, shown: Option<&RoadNet>) -> Option<(Vec2, Vec2)> {
    let join = |a: Option<(Vec2, Vec2)>, b: Option<(Vec2, Vec2)>| match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => Some((a0.min(b0), a1.max(b1))),
        (a, b) => a.or(b),
    };
    let from_outlines = outlines.iter().flatten().fold(None, |acc, &p| join(acc, Some((p, p))));
    let region = join(from_outlines, roads.and_then(bounds));
    region.or_else(|| shown.and_then(bounds))
}

/// The network's nodes' bounding box (world x, y).
pub fn bounds(net: &RoadNet) -> Option<(Vec2, Vec2)> {
    let mut it = net.nodes.iter().map(|&(p, _)| p);
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> MapView {
        MapView {
            centre: Vec2::new(100.0, -50.0),
            scale: 0.5,
            fitted: true,
            rect: Rect::from_min_size(Pos2::new(10.0, 20.0), egui::vec2(400.0, 300.0)),
            overview: None,
            outlines: None,
        }
    }

    #[test]
    fn screen_and_world_round_trip() {
        let v = view();
        assert_eq!(v.to_screen(v.centre), v.rect.center());
        for p in [Vec2::new(0.0, 0.0), Vec2::new(250.0, 13.0), Vec2::new(-80.0, -400.0)] {
            assert!(v.to_world(v.to_screen(p)).distance(p) < 1e-3);
        }
        // North (+Y) is up, east (+X) right.
        let n = v.to_screen(v.centre + Vec2::Y);
        let e = v.to_screen(v.centre + Vec2::X);
        assert!(n.y < v.rect.center().y && e.x > v.rect.center().x);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_mouse() {
        let mut v = view();
        let at = Pos2::new(50.0, 70.0);
        let before = v.to_world(at);
        v.zoom_at(at, 3.0);
        assert!((v.scale - 1.5).abs() < 1e-6);
        assert!(v.to_world(at).distance(before) < 1e-3);
        v.zoom_at(at, 1e9);
        assert_eq!(v.scale, MAX_SCALE);
    }

    #[test]
    fn panning_follows_the_mouse() {
        let mut v = view();
        let p = Vec2::new(120.0, -40.0);
        let s = v.to_screen(p);
        v.pan(egui::vec2(30.0, -12.0));
        assert!((v.to_screen(p) - (s + egui::vec2(30.0, -12.0))).length() < 1e-3);
    }

    #[test]
    fn fit_shows_the_whole_box() {
        let mut v = view();
        let (lo, hi) = (Vec2::new(-1000.0, 200.0), Vec2::new(3000.0, 900.0));
        v.fit(lo, hi);
        assert!(v.rect.contains(v.to_screen(lo)) && v.rect.contains(v.to_screen(hi)));
    }

    #[test]
    fn routes_become_one_segment_each() {
        let lines = [
            vec![Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0)],
            vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 1.0, 0.0), Vec3::new(10.0, 3.0, 0.0)],
        ];
        let net = net_of_lines(lines.iter().map(Vec::as_slice));
        assert_eq!(net.nodes.len(), 5);
        assert_eq!((net.segments[0].first, net.segments[0].end, net.segments[0].length), (0, 2, 5.0));
        assert_eq!((net.segments[1].first, net.segments[1].end, net.segments[1].length), (2, 5, 3.0));
    }

    #[test]
    fn the_region_covers_the_outlines_and_the_roads() {
        let outlines = vec![
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), Vec2::new(100.0, 50.0)],
            vec![Vec2::new(-300.0, 20.0), Vec2::new(-250.0, 400.0), Vec2::new(-200.0, 20.0)],
        ];
        let roads = RoadNet { nodes: vec![(Vec2::new(500.0, -60.0), 0)], ..Default::default() };
        let race = RoadNet { nodes: vec![(Vec2::new(9000.0, 9000.0), 0)], ..Default::default() };
        assert_eq!(
            region_bounds(&outlines, None, Some(&race)),
            Some((Vec2::new(-300.0, 0.0), Vec2::new(100.0, 400.0)))
        );
        assert_eq!(
            region_bounds(&outlines, Some(&roads), Some(&race)),
            Some((Vec2::new(-300.0, -60.0), Vec2::new(500.0, 400.0)))
        );
        // No outlines: the network shown.
        assert_eq!(region_bounds(&[], None, Some(&race)), Some((Vec2::splat(9000.0), Vec2::splat(9000.0))));
        assert_eq!(region_bounds(&[], None, None), None);
    }

    #[test]
    fn network_bounds() {
        let net = RoadNet {
            nodes: vec![(Vec2::new(1.0, 5.0), 0), (Vec2::new(-3.0, 2.0), 0), (Vec2::new(4.0, -1.0), 0)],
            ..Default::default()
        };
        assert_eq!(bounds(&net), Some((Vec2::new(-3.0, -1.0), Vec2::new(4.0, 5.0))));
        assert_eq!(bounds(&RoadNet::default()), None);
    }
}
