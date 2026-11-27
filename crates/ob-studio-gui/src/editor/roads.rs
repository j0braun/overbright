//! The road network over the top view (View ▸ Roads): the selected race's base track's roads (else the region's free
//! roam roads), the region's roads faint under them, and the closed race barriers in red, drawn over the world image
//! of the top view where the 2D Map view had them. The selection, the 3D camera and the pointer's x, y are drawn by
//! the viewport; the races over it by the races' overlay in `app::Panels`.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

use super::camera::TopView;
use super::overlay::{colour, line2d};
use super::state::EditorState;
use crate::formats::roads::RoadNet;

/// The roads the layer draws.
#[derive(Default)]
pub struct RoadLayer {
    /// The roads shown when no race is chosen ([`overview_net`]; read once, None inside: unreadable).
    overview: Option<Option<(u16, RoadNet)>>,
    /// The drivable sections' outlines (x, y), read from the world once.
    outlines: Option<Vec<Vec<Vec2>>>,
    /// Each base track's own route line (x, y), read once through the assets.
    routes: HashMap<u16, Vec<Vec2>>,
}

/// Where View ▸ Show nearest road looked from, and the road point it found: drawn over the top view until the
/// next one or until Roads is turned off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearestRoad {
    pub from: Vec2,
    pub at: Vec2,
    /// The road segment, and the base track whose roads it is of (0: the region's free roam roads).
    pub segment: usize,
    pub track: u16,
}

/// The spot "Show nearest road" looks from: the selected race point, else the selected prop, else the camera.
pub fn look_from(s: &EditorState) -> Vec2 {
    let point = s
        .point
        .filter(|_| s.mode == super::state::Mode::Races)
        .and_then(|p| super::races::point_xy(s.session.map.races.get(s.race?)?, p));
    point
        .or_else(|| {
            let i = *s.selection.first()?;
            crate::tools::map::edit::placement_parts(&s.session.world, i).map(|p| p.0.truncate())
        })
        .unwrap_or_else(|| s.camera.position.truncate())
}

/// The road nearest [`look_from`] in the roads the layer shows (the selected race's base track's, else the
/// region's free roam roads), over the segments a closed race barrier does not shut.
pub fn nearest(s: &mut EditorState) -> Option<NearestRoad> {
    if let Some(t) = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| r.track)
        && !s.nets.contains_key(&t)
    {
        let net = s.assets.road_net(t);
        s.nets.insert(t, net);
    }
    let from = look_from(s);
    let hit = |track: u16, net: &RoadNet| {
        crate::tools::map::race::snap(net, from).map(|rp| NearestRoad { from, at: rp.at, segment: rp.segment, track })
    };
    if let Some((t, net)) = race_network(s) {
        return hit(t, net);
    }
    let (t, net) = overview_net(s.assets.as_ref(), &s.session.region)?;
    hit(t, &net)
}

/// View ▸ Show nearest road: mark the nearest road and take the camera to it.
pub fn show_nearest(s: &mut EditorState) {
    match nearest(s) {
        Some(n) => {
            s.roads = true;
            s.nearest_road = Some(n);
            s.goal = Some(super::views::Goal::At(n.at));
            s.status = format!("The nearest road is {:.0} m away", n.from.distance(n.at));
        }
        None => {
            s.nearest_road = None;
            s.status = "No road network to show".into();
        }
    }
}

impl RoadLayer {
    /// Read the networks the layer needs and the state does not hold yet: the current race's base track's (into
    /// `s.nets`, its race barriers closed as `tools::map::race::road_net` does) and the overview.
    pub fn load(&mut self, s: &mut EditorState) {
        if let Some(t) = s.race.and_then(|k| s.session.map.races.get(k)).map(|r| r.track)
            && !s.nets.contains_key(&t)
        {
            let net = s.assets.road_net(t);
            s.nets.insert(t, net);
        }
        if self.overview.is_none() {
            self.overview = Some(overview_net(s.assets.as_ref(), &s.session.region));
        }
        if let Some(t) = self.network(s).map(|(t, _)| t).filter(|t| *t != 0) {
            self.routes.entry(t).or_insert_with(|| s.assets.track_route(t));
        }
    }

    /// The network shown: the current race's base track's, else the lowest-numbered one loaded, else the region's
    /// overview.
    pub fn network<'s>(&'s self, s: &'s EditorState) -> Option<(u16, &'s RoadNet)> {
        network(s, &self.overview)
    }

    /// Draw the layer over the top view `top` shown in `rect`.
    pub fn paint(&mut self, s: &mut EditorState, painter: &egui::Painter, top: &TopView, rect: Rect) {
        if !s.roads {
            s.nearest_road = None;
            return;
        }
        self.load(s);
        let (m, c) = (top.metres_per_point(rect), rect.center());
        let to = |p: Vec2| Pos2::new(c.x + (p.x - top.centre.x) / m, c.y - (p.y - top.centre.y) / m);
        let painter = painter.with_clip_rect(rect);
        // The region: every drivable section's outline.
        let outlines = self.outlines.get_or_insert_with(|| region_outlines(&s.session.world));
        let edge = Stroke::new(1.0, colour::DIM.gamma_multiply(0.3));
        let view = rect.expand(20.0);
        for outline in outlines.iter() {
            let pts: Vec<Pos2> = outline.iter().map(|&p| to(p)).collect();
            if pts.len() >= 3 && pts.iter().any(|p| view.contains(*p)) {
                painter.add(egui::Shape::closed_line(pts, edge));
            }
        }
        if let Some((track, net)) = self.network(s) {
            // The whole region's roads faintly under a race's network; the roads its base track does not allow dim.
            if track != 0
                && let Some(Some((_, all))) = &self.overview
            {
                draw_net(&painter, rect, &to, all, colour::ROAD.gamma_multiply(0.5), None);
            }
            // The base track's own route, faint under its roads.
            if let Some(line) = self.routes.get(&track).filter(|l| l.len() >= 2) {
                let pts: Vec<Pos2> = line.iter().map(|&p| to(p)).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(5.0, colour::SELECT.gamma_multiply(0.22))));
            }
            draw_net(&painter, rect, &to, net, colour::ROAD, (track != 0).then_some(track));
            if let Some(n) = s.nearest_road {
                let (a, b) = (to(n.from), to(n.at));
                line2d(&painter, a, b, colour::SELECT, 1.5);
                painter.circle_stroke(b, 9.0, Stroke::new(2.5, colour::SELECT));
                painter.circle_filled(b, 2.5, colour::SELECT);
            }
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
    }
}

/// The network's roads (a road behind a closed race barrier in red, a road `track` does not allow dim) and its
/// closed barriers, through `to` (world x, y to the screen).
pub fn draw_net(
    painter: &egui::Painter,
    rect: Rect,
    to: &dyn Fn(Vec2) -> Pos2,
    net: &RoadNet,
    color: Color32,
    track: Option<u16>,
) {
    let road = Stroke::new(1.5, color);
    let dim = Stroke::new(1.0, color.gamma_multiply(0.3));
    let closed = Stroke::new(1.5, colour::ERROR.gamma_multiply(0.6));
    for seg in &net.segments {
        let shut = seg.barriers.iter().any(|&b| net.closed.get(b).copied().unwrap_or(false));
        let allowed = track.is_none_or(|t| seg.tracks.allows(t));
        let stroke = if shut { closed } else if allowed { road } else { dim };
        let pts: Vec<Pos2> =
            net.nodes.get(seg.first..seg.end.min(net.nodes.len())).unwrap_or(&[]).iter().map(|&(p, _)| to(p)).collect();
        if pts.len() >= 2 && pts.iter().any(|p| rect.expand(50.0).contains(*p)) {
            painter.add(egui::Shape::line(pts, stroke));
        }
    }
    for (b, ends) in net.barrier_ends.iter().enumerate() {
        if net.closed.get(b).copied().unwrap_or(false) {
            line2d(painter, to(ends[0]), to(ends[1]), colour::ERROR, 2.0);
        }
    }
}

/// The race's network, else the overview.
fn network<'s>(s: &'s EditorState, overview: &'s Option<Option<(u16, RoadNet)>>) -> Option<(u16, &'s RoadNet)> {
    race_network(s).or_else(|| overview.as_ref()?.as_ref().map(|(t, n)| (*t, n)))
}

pub(super) fn race_network(s: &EditorState) -> Option<(u16, &RoadNet)> {
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
            tracks: Default::default(),
        });
    }
    net
}

/// The region's roads as a whole, for the map without a race: the
/// free roam's AI routes (every lane the free roam's traffic drives, read
/// through the assets), each a segment of a network with no barriers. Its
/// number is 0 here.
pub(super) fn overview_net(assets: &dyn crate::tools::map::assets::MapAssets, region: &str) -> Option<(u16, RoadNet)> {
    let lines = assets.free_roam_lines(region);
    Some((0, net_of_lines(lines.iter().map(Vec::as_slice)))).filter(|(_, n)| !n.nodes.is_empty())
}

/// The drivable sections' outlines on the ground: together they draw the
/// region's shape.
pub fn region_outlines(world: &crate::world::World) -> Vec<Vec<Vec2>> {
    world.visibility.drivable().map(|(_, b)| b.points.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A road the base track does not allow is drawn dim, the ones it allows bright.
    #[test]
    fn roads_the_base_track_does_not_allow_are_dim() {
        let mut net = crate::editor::validate::tests::straight(false);
        net.segments[0].tracks = crate::formats::roads::TrackList { only: 1, not: 0, ids: vec![4001] };
        let strokes = |track: u16| {
            let ctx = egui::Context::default();
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 450.0));
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ui| {
                let to = |p: Vec2| Pos2::new(p.x, 200.0 - p.y);
                draw_net(ui.painter(), rect, &to, &net, colour::ROAD, Some(track));
            });
            out.textures_delta.clear();
            out.shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Path(p) => Some(format!("{:?}", p.stroke.color)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let ok = strokes(4001);
        let other = strokes(4002);
        assert!(!ok.is_empty() && !other.is_empty());
        assert_ne!(ok[0], other[0], "the same road, allowed and not");
    }

    /// The base track's own route is drawn faint under its roads, and only with its track chosen.
    #[test]
    fn the_base_tracks_own_route_is_drawn_faint() {
        let mut s = crate::editor::props::tests::state();
        let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (380.0, 0.0)]);
        race.track = 4001;
        s.session.map.races = vec![race];
        s.race = Some(0);
        s.nets.insert(4001, Some(crate::editor::validate::tests::straight(true)));
        let top = TopView { centre: Vec2::new(200.0, 0.0), half_height: 150.0 };
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 450.0));
        let wide = |s: &mut EditorState| {
            let mut layer = RoadLayer::default();
            let ctx = egui::Context::default();
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ui| {
                layer.paint(s, ui.painter(), &top, rect)
            });
            out.shapes
                .iter()
                .filter(|c| matches!(&c.shape, egui::Shape::Path(p) if p.stroke.width == 5.0))
                .count()
        };
        assert_eq!(wide(&mut s), 0, "the assets give no route");
        s.assets = Box::new(crate::tools::map::assets::GivenAssets {
            routes: std::collections::HashMap::from([(4001, vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)])]),
            ..Default::default()
        });
        assert_eq!(wide(&mut s), 1, "the route line under the roads");
    }

    /// Show nearest road marks the road point nearest the selected race point (else the prop, else the camera),
    /// takes the camera there and says how far it is; Roads off takes the mark away.
    #[test]
    fn show_nearest_road_marks_the_road_nearest_the_selection() {
        let mut s = crate::editor::props::tests::state();
        let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (380.0, 0.0), (150.0, 40.0)]);
        race.track = 4001;
        s.session.map.races = vec![race];
        s.race = Some(0);
        s.nets.insert(4001, Some(crate::editor::validate::tests::straight(false)));
        // Nothing selected: from the camera over (0, -20).
        show_nearest(&mut s);
        let n = s.nearest_road.expect("a road was found");
        assert_eq!(n.from, s.camera.position.truncate());
        assert!((n.at.y).abs() < 1e-3 && n.track == 4001, "{n:?}");
        assert_eq!(s.goal, Some(super::super::views::Goal::At(n.at)));
        // A race point: the third checkpoint is 40 m off the road.
        s.mode = super::super::state::Mode::Races;
        s.point = Some(crate::editor::state::RacePoint { part: crate::tools::map::edit::RacePart::Checkpoint, index: 2 });
        show_nearest(&mut s);
        let n = s.nearest_road.unwrap();
        assert_eq!((n.from, n.at), (Vec2::new(150.0, 40.0), Vec2::new(150.0, 0.0)));
        assert_eq!(s.status, "The nearest road is 40 m away");
        // The mark is drawn with the roads and goes with them.
        let top = TopView { centre: Vec2::new(200.0, 0.0), half_height: 150.0 };
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 450.0));
        let paint = |s: &mut EditorState| {
            let mut layer = RoadLayer::default();
            let ctx = egui::Context::default();
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ui| {
                layer.paint(s, ui.painter(), &top, rect)
            });
            out.shapes.len()
        };
        let with = paint(&mut s);
        s.nearest_road = None;
        assert!(with > paint(&mut s), "the mark is a drawing of its own");
        s.nearest_road = Some(n);
        s.roads = false;
        paint(&mut s);
        assert!(s.nearest_road.is_none());
    }

    /// The top view draws the roads and shuts the closed barriers in red; View ▸ Roads takes them away.
    #[test]
    fn the_layer_draws_the_race_roads_and_follows_the_toggle() {
        let mut s = crate::editor::props::tests::state();
        let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (380.0, 0.0)]);
        race.track = 4001;
        s.session.map.races = vec![race];
        s.race = Some(0);
        s.nets.insert(4001, Some(crate::editor::validate::tests::straight(true)));
        let top = TopView { centre: Vec2::new(200.0, 0.0), half_height: 150.0 };
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 450.0));
        let count = |s: &mut EditorState| {
            let mut layer = RoadLayer::default();
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ui| {
                layer.paint(s, ui.painter(), &top, rect)
            });
            out.textures_delta.clear();
            out.shapes.len()
        };
        let on = count(&mut s);
        s.roads = false;
        let off = count(&mut s);
        assert!(on > off, "{on} shapes with the roads, {off} without");
    }
}
