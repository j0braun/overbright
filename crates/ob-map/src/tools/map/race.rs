//! A custom race as the race set-up takes it: the same [`Route`] a track's
//! own route is (`career::load_route`), built from the mod's checkpoints
//! and markers on the base track's road network (its Paths<track>.bin,
//! with the base track's race barriers closed, as `load_route` does).

use anyhow::{Context, Result, ensure};
use glam::{Vec2, Vec3};

use super::format::{CustomRace, Spot};
use crate::formats::roads::{RoadNet, RoadRoute};
use crate::frontend::career::routes::{Route, route_path};
use crate::frontend::career::tracks::Track;

/// The base track's road network with its race barriers closed.
pub fn road_net(root: &std::path::Path, track: &Track) -> Result<RoadNet> {
    let path = route_path(root, track);
    let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(net_from(&data, track.number))
}

/// [`road_net`] from the file's bytes.
pub fn net_from(data: &[u8], track: u16) -> RoadNet {
    let mut net = RoadNet::parse(data);
    let n = track as i16;
    net.close_barriers(&[&format!("BARRIERS_{n}"), &format!("PLAYER_BARRIERS_{n}")]);
    net
}

/// Why a race cannot be raced: the first checkpoint leg with no way
/// through the network (`Err((k, why))`: leg k → k + 1), else the route.
pub fn check(net: &RoadNet, race: &CustomRace) -> Result<Vec<(usize, u8)>, (usize, String)> {
    if race.checkpoints.len() < 2 {
        return Err((0, "a race needs at least 2 checkpoints".into()));
    }
    let points = closed(race);
    if let Some(route) = net.route(&points, race.kind.loops()) {
        return Ok(route);
    }
    for (k, pair) in points.windows(2).enumerate() {
        if net.route(pair, false).is_none() {
            return Err((k, format!("no road from checkpoint {} to {}", k + 1, (k + 1) % race.checkpoints.len() + 1)));
        }
    }
    Err((0, "the route runs over a road it already took".into()))
}

/// The checkpoints as the route is built through them: a circuit's
/// first again at the end.
fn closed(race: &CustomRace) -> Vec<Vec2> {
    let mut points = race.checkpoints.clone();
    if race.kind.loops() && points.len() > 2 && points.first() != points.last() {
        points.push(points[0]);
    }
    points
}

/// The default start grid behind `start`, facing its heading: 8 slots in
/// rows of two, 7 m between rows and 4 m across (the game's grids are
/// about that), pole on the left.
pub fn default_grid(start: Spot) -> Vec<Spot> {
    let h = start.heading.to_radians();
    let fwd = Vec2::new(h.cos(), h.sin());
    let left = fwd.perp();
    (0..8)
        .map(|k| {
            let row = (k / 2) as f32;
            let side = if k % 2 == 0 { 2.0 } else { -2.0 };
            let at = start.position.truncate() - fwd * (4.0 + row * 7.0) + left * side;
            Spot { position: at.extend(start.position.z), heading: start.heading }
        })
        .collect()
}

/// The race's [`Route`] on `net`. `ground`: the height at a point (the
/// world's ground; the start's own height without one).
pub fn route(net: &RoadNet, race: &CustomRace, ground: &dyn Fn(Vec2) -> Option<f32>) -> Result<Route> {
    let segments = check(net, race).map_err(|(_, why)| anyhow::anyhow!(why))?;
    let loops = race.kind.loops();
    let road: RoadRoute = net.race_road(&segments, loops);
    ensure!(road.length > 1.0, "the route is empty");
    let a = race.checkpoints[0];
    let b = race.checkpoints[1];
    let start = race.start.unwrap_or_else(|| {
        let z = ground(a).unwrap_or(0.0);
        Spot { position: a.extend(z), heading: (b - a).y.atan2((b - a).x).to_degrees() }
    });
    let grid = if race.grid.is_empty() { default_grid(start) } else { race.grid.clone() };
    let grid: Vec<(Vec3, f32)> = grid
        .iter()
        .map(|g| {
            let z = ground(g.position.truncate()).unwrap_or(g.position.z);
            (g.position.truncate().extend(z), g.heading.to_radians())
        })
        .collect();
    let start_at = road.locate_once(start.position.truncate());
    let finish_at = match race.finish {
        _ if loops => start_at,
        Some(f) => road.locate_once(f.position.truncate()),
        // A sprint without a finish marker ends at its last checkpoint.
        None => road.locate_once(*race.checkpoints.last().unwrap()),
    };
    Ok(Route {
        checkpoints: closed(race),
        loops,
        grid,
        start_line: Some(start.position),
        start_heading: Some(start.heading.to_radians()),
        finish_heading: race
            .finish
            .map(|f| f.heading.to_radians())
            .or(Some(start.heading.to_radians()).filter(|_| loops)),
        finish_line: race.finish.map(|f| f.position).or(Some(start.position).filter(|_| loops)),
        road,
        start_at,
        finish_at,
        splits: [None; 3],
        backward: race.backward,
    })
}

/// A point on the road network: the nearest point of an open segment's
/// centre line to some spot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadPoint {
    /// The point on the centre line.
    pub at: Vec2,
    /// The line's direction there, in the segment's node order.
    pub dir: Vec2,
    pub segment: usize,
    /// How far the spot was from `at`.
    pub distance: f32,
}

/// The point of the network's centre lines nearest `p`, over the segments
/// no closed barrier shuts (the ones a race can use).
pub fn snap(net: &RoadNet, p: Vec2) -> Option<RoadPoint> {
    let mut best: Option<RoadPoint> = None;
    for (k, s) in net.segments.iter().enumerate() {
        if s.barriers.iter().any(|&b| net.closed.get(b).copied().unwrap_or(false)) {
            continue;
        }
        let end = s.end.min(net.nodes.len());
        if end <= s.first {
            continue;
        }
        let mut consider = |a: Vec2, b: Vec2| {
            let ab = b - a;
            let t = if ab.length_squared() > 1e-9 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let at = a + ab * t;
            let distance = at.distance(p);
            if best.is_none_or(|b| distance < b.distance) {
                best = Some(RoadPoint { at, dir: ab.normalize_or_zero(), segment: k, distance });
            }
        };
        if end - s.first == 1 {
            let a = net.nodes[s.first].0;
            consider(a, a);
        }
        for n in s.first..end - 1 {
            consider(net.nodes[n].0, net.nodes[n + 1].0);
        }
    }
    best
}

/// The distance from `p` to the nearest point of a polyline (infinite for
/// an empty one), and the index of the piece it is on.
pub fn polyline_distance(line: &[Vec2], p: Vec2) -> (f32, usize) {
    let mut best = (f32::INFINITY, 0);
    if line.len() == 1 {
        return (line[0].distance(p), 0);
    }
    for (k, w) in line.windows(2).enumerate() {
        let ab = w[1] - w[0];
        let t = if ab.length_squared() > 1e-9 { ((p - w[0]).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let d = (w[0] + ab * t).distance(p);
        if d < best.0 {
            best = (d, k);
        }
    }
    best
}

/// Each checkpoint leg's road on its own (k → k + 1, a circuit's last
/// back to the first): the nodes in driving order, or None when no road
/// leads through. The editor's preview of a race that does not build as a
/// whole.
pub fn legs(net: &RoadNet, race: &CustomRace) -> Vec<Option<Vec<Vec2>>> {
    closed(race).windows(2).map(|pair| net.route(pair, false).map(|r| net.polyline(&r))).collect()
}

/// The grid the race is started on: its own slots, else the default
/// grid behind its start (the start being the first checkpoint facing
/// the second when it has none), as [`route`] makes it.
pub fn effective_grid(race: &CustomRace) -> Vec<Spot> {
    if !race.grid.is_empty() {
        return race.grid.clone();
    }
    match race.start.or_else(|| default_start(race)) {
        Some(start) => default_grid(start),
        None => Vec::new(),
    }
}

/// The start a race without a start line gets: the first checkpoint,
/// facing the second.
pub fn default_start(race: &CustomRace) -> Option<Spot> {
    let (a, b) = (*race.checkpoints.first()?, *race.checkpoints.get(1)?);
    Some(Spot { position: a.extend(0.0), heading: (b - a).y.atan2((b - a).x).to_degrees() })
}
