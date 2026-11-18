//! A custom race as the race set-up takes it: the same [`Route`] a track's
//! own route is (`career::load_route`), built from the mod's checkpoints
//! and markers on the base track's road network (its Paths<track>.bin,
//! with the base track's race barriers closed, as `load_route` does).

use anyhow::{Context, Result, ensure};
use glam::{Vec2, Vec3};

use super::format::{CustomRace, Spot};
use crate::formats::roads::{RoadNet, RoadRoute};
use crate::frontend::career::{Route, Track, route_path};

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
