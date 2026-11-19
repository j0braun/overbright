//! Race validation: what keeps a custom race from being raced (errors,
//! they block Test race), what is likely a mistake (warnings) and what is
//! fine (info), each with the spot "Show" takes the camera to and, where
//! one exists, a one-click fix ("Try base track N", "Snap grid to road").
//!
//! The checks are pure functions of the mod and the base tracks' road
//! networks; [`refresh`] runs them on the editor state and fills
//! `EditorState::issues`. Whether a race was tested since its last change
//! is kept in the mod's `.editor/tested.txt` ([`Tested`]); the mod's
//! exports leave it out.

use std::collections::HashMap;
use std::path::Path;

use glam::Vec2;

use super::state::{EditorState, Issue, Level, RacePoint, commit};
use crate::formats::roads::RoadNet;
use crate::tools::map::edit::RacePart;
use crate::tools::map::race::{self, RoadPoint};
use crate::tools::map::{CustomRace, MAX_CHECKPOINTS, MAX_GRID, MAX_LAPS, MapMod, Spot, valid_id};

/// A grid slot or a start / finish line further than this from the base
/// track's road network is reported. The network's line need not run in
/// the middle of the road: the game's own grids lie up to about 19 m from
/// it on wide streets (all but four of its 98 race tracks within 20 m).
pub const OFF_ROAD_M: f32 = 20.0;
/// A checkpoint further than this from the road network: the route is
/// built from the road node nearest it, likely not the road meant.
pub const CHECKPOINT_OFF_ROAD_M: f32 = 25.0;
/// Checkpoints closer together than this are likely a double click.
pub const CLOSE_M: f32 = 30.0;
/// Checkpoints further apart than this leave the route to the network's
/// shortest way, which may not be the road meant.
pub const FAR_M: f32 = 1500.0;
/// A grid slot moved onto the road keeps at most this much of its offset
/// across the road (two cars side by side).
const GRID_SIDE_M: f32 = 2.0;
/// The file in the mod's `.editor/` folder that records the tested races.
pub const TESTED_FILE: &str = crate::tools::map::store::TESTED;

/// A one-click fix of a validation entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Race on this base track instead: its barriers leave the route open.
    TryTrack(u16),
    /// Move the grid slots that are off the road onto it.
    SnapGrid,
    /// Move the start line onto the road, along it.
    SnapStart,
    /// Move the finish line onto the route, along it.
    SnapFinish,
    /// Laps into the range the mode allows.
    Laps(u32),
    /// A free id.
    Rename(String),
}

impl Fix {
    /// The button's label.
    pub fn label(&self) -> String {
        match self {
            Fix::TryTrack(t) => format!("Try base track {t}"),
            Fix::SnapGrid => "Snap grid to road".into(),
            Fix::SnapStart => "Snap start to road".into(),
            Fix::SnapFinish => "Snap finish to route".into(),
            Fix::Laps(n) => format!("Set laps to {n}"),
            Fix::Rename(id) => format!("Rename to {id}"),
        }
    }
}

/// One validation entry with what the panel needs beyond [`Issue`].
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub level: Level,
    /// A passed check (shown with a tick).
    pub ok: bool,
    pub text: String,
    pub race: Option<usize>,
    /// The race part to select on Show.
    pub point: Option<RacePoint>,
    /// The spot to show (x, y; the panel puts it on the ground).
    pub at: Option<Vec2>,
    pub fix: Option<Fix>,
}

impl Check {
    fn new(level: Level, race: usize, text: String) -> Check {
        Check { level, ok: false, text, race: Some(race), point: None, at: None, fix: None }
    }

    fn at(mut self, at: Vec2, point: Option<RacePoint>) -> Check {
        (self.at, self.point) = (Some(at), point);
        self
    }

    fn fix(mut self, fix: Fix) -> Check {
        self.fix = Some(fix);
        self
    }
}

fn point(part: RacePart, index: usize) -> Option<RacePoint> {
    Some(RacePoint { part, index })
}

/// The races tested and what each was when it was: a fingerprint of the
/// parts that change how it races (not its name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tested(pub HashMap<String, u64>);

impl Tested {
    /// The record of the mod folder `dir` (empty when there is none).
    pub fn load(dir: &Path) -> Tested {
        let text = std::fs::read_to_string(dir.join(TESTED_FILE)).unwrap_or_default();
        Tested(
            text.lines()
                .filter_map(|l| {
                    let (id, h) = l.trim().split_once(' ')?;
                    Some((id.to_string(), u64::from_str_radix(h.trim(), 16).ok()?))
                })
                .collect(),
        )
    }

    /// Whether `race` was tested as it is now.
    pub fn is_tested(&self, race: &CustomRace) -> bool {
        self.0.get(&race.id) == Some(&fingerprint(race))
    }

    /// Record `race` as tested and write the record (atomically) to `dir`.
    pub fn mark(&mut self, dir: &Path, race: &CustomRace) -> anyhow::Result<()> {
        self.0.insert(race.id.clone(), fingerprint(race));
        let mut ids: Vec<_> = self.0.iter().collect();
        ids.sort();
        let text: String = ids.iter().map(|(id, h)| format!("{id} {h:016x}\n")).collect();
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{TESTED_FILE}.tmp"));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, dir.join(TESTED_FILE))?;
        Ok(())
    }
}

/// Record `race` of the mod folder `dir` as tested (after a test run that
/// finished it).
pub fn mark_tested(dir: &Path, race: &CustomRace) -> anyhow::Result<()> {
    Tested::load(dir).mark(dir, race)
}

/// FNV-1a over what makes the race drive the way it does.
pub fn fingerprint(r: &CustomRace) -> u64 {
    let text = format!(
        "{} {} {:?} {} {:?} {:?} {:?} {:?}",
        r.track, r.backward, r.kind, r.laps, r.checkpoints, r.grid, r.start, r.finish
    );
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn km(m: f32) -> String {
    if m >= 1000.0 { format!("{:.1} km", m / 1000.0) } else { format!("{m:.0} m") }
}

/// What a custom race is limited to: the base track's road network covers
/// only the area around that track's own race (docs/tracing/ai-lanes.md),
/// never the whole region.
pub const AREA_NOTE: &str = "a race uses only its base track's roads";

/// The size of the area `net`'s roads cover ("1.2 km × 800 m").
pub fn area(net: &RoadNet) -> Option<String> {
    let mut it = net.nodes.iter().map(|&(p, _)| p);
    let first = it.next()?;
    let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let d = hi - lo;
    Some(format!("{} × {}", km(d.x), km(d.y)))
}

/// A free race id like `base`, `base-2`, ….
pub fn free_id(map: &MapMod, base: &str) -> String {
    let base: String = base
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let base = if base.is_empty() { "race".to_string() } else { base };
    let taken = |id: &str| map.races.iter().filter(|r| r.id == id).count();
    if taken(&base) == 0 {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| taken(id) == 0).unwrap()
}

/// The checks of race `k` of `map`. `net`: its base track's network
/// (None: unreadable); `others`: the region's other race tracks' networks,
/// tried when a leg has no road.
pub fn race_checks(
    map: &MapMod,
    k: usize,
    net: Option<&RoadNet>,
    others: &[(u16, &RoadNet)],
    tested: &Tested,
) -> Vec<Check> {
    let r = &map.races[k];
    let name = if r.name.is_empty() { r.id.as_str() } else { r.name.as_str() };
    let mut out = Vec::new();
    let first = r.checkpoints.first().copied().or(r.start.map(|s| s.position.truncate()));
    let err = |text: String| Check::new(Level::Error, k, format!("{name}: {text}"));
    let warn = |text: String| Check::new(Level::Warning, k, format!("{name}: {text}"));

    if !valid_id(&r.id) {
        out.push(
            err(format!("the id `{}` may only hold a-z, 0-9, - and _", r.id)).fix(Fix::Rename(free_id(map, &r.id))),
        );
    } else if map.races.iter().take(k).any(|o| o.id == r.id) {
        out.push(err(format!("the id `{}` is used by an earlier race", r.id)).fix(Fix::Rename(free_id(map, &r.id))));
    }
    if r.kind.loops() && !(1..=MAX_LAPS).contains(&r.laps) {
        out.push(
            err(format!("{} laps; a circuit runs 1 to {MAX_LAPS}", r.laps)).fix(Fix::Laps(r.laps.clamp(1, MAX_LAPS))),
        );
    } else if !r.kind.loops() && r.laps != 1 {
        out.push(warn(format!("{} laps on a {} (it runs once)", r.laps, r.kind.name())).fix(Fix::Laps(1)));
    }
    if r.checkpoints.len() > MAX_CHECKPOINTS {
        out.push(err(format!("{} checkpoints; a race holds at most {MAX_CHECKPOINTS}", r.checkpoints.len())));
    }
    if r.grid.len() > MAX_GRID {
        out.push(err(format!("{} grid slots; a grid holds at most {MAX_GRID}", r.grid.len())));
    }
    if r.checkpoints.len() < 2 {
        let mut c = err(format!("{} checkpoint(s); a race needs at least 2", r.checkpoints.len()));
        if let Some(at) = first {
            c = c.at(at, point(RacePart::Checkpoint, 0));
        }
        out.push(c);
    }
    for (i, w) in r.checkpoints.windows(2).enumerate() {
        let d = w[0].distance(w[1]);
        if d < CLOSE_M {
            out.push(
                warn(format!("checkpoints {} and {} are {d:.0} m apart", i + 1, i + 2))
                    .at(w[1], point(RacePart::Checkpoint, i + 1)),
            );
        } else if d > FAR_M {
            out.push(
                warn(format!(
                    "checkpoints {} and {} are {} apart; the route between takes the shortest road",
                    i + 1,
                    i + 2,
                    km(d)
                ))
                .at(w[1], point(RacePart::Checkpoint, i + 1)),
            );
        }
    }
    let Some(net) = net else {
        out.push(err(format!("base track {} has no road network", r.track)));
        return out;
    };
    let near = |p: Vec2| race::snap(net, p);
    for (i, &c) in r.checkpoints.iter().enumerate() {
        if let Some(rp) = near(c).filter(|rp| rp.distance > CHECKPOINT_OFF_ROAD_M) {
            out.push(
                warn(format!(
                    "checkpoint {} is {:.0} m from base track {}'s roads; {AREA_NOTE}",
                    i + 1,
                    rp.distance,
                    r.track
                ))
                    .at(c, point(RacePart::Checkpoint, i)),
            );
        }
    }

    // The route.
    let mut road: Option<Vec<Vec2>> = None;
    if r.checkpoints.len() >= 2 {
        match race::check(net, r) {
            Err((leg, why)) => {
                let a = r.checkpoints[leg.min(r.checkpoints.len() - 1)];
                let n = r.checkpoints.len();
                let pair = [a, r.checkpoints[(leg + 1) % n]];
                let mut open = net.clone();
                open.closed.iter_mut().for_each(|c| *c = false);
                let barrier = why.starts_with("no road")
                    && net.route(&pair, false).is_none()
                    && open.route(&pair, false).is_some();
                let text = if barrier {
                    format!("{why}: a race barrier of track {} closes it", r.track)
                } else if why.starts_with("no road") {
                    format!("{why} on base track {}'s roads; {AREA_NOTE}", r.track)
                } else {
                    why
                };
                let mut c = err(text).at(a, point(RacePart::Checkpoint, leg));
                // Only a track whose own roads reach every checkpoint: a
                // route that builds from far-off road nodes is not the race.
                let reaches = |o: &RoadNet| {
                    r.checkpoints.iter().all(|&c| race::snap(o, c).is_some_and(|rp| rp.distance <= CHECKPOINT_OFF_ROAD_M))
                };
                if let Some((t, _)) =
                    others.iter().find(|(t, o)| *t != r.track && reaches(o) && race::check(o, r).is_ok())
                {
                    c = c.fix(Fix::TryTrack(*t));
                }
                out.push(c);
            }
            Ok(_) => match race::route(net, r, &|_| None) {
                Ok(route) => {
                    let mut c = Check::new(
                        Level::Info,
                        k,
                        format!(
                            "{name}: {} on track {}, {}{}, {} checkpoints; it stays on that track's roads{}",
                            r.kind.name(),
                            r.track,
                            km(route.road.length),
                            if r.kind.loops() { " a lap" } else { "" },
                            r.checkpoints.len(),
                            area(net).map_or(String::new(), |a| format!(" (an area of {a})"))
                        ),
                    );
                    c.ok = true;
                    out.push(c);
                    road = Some(route.road.points.clone());
                }
                Err(e) => out.push(err(format!("{e}")).at(r.checkpoints[0], point(RacePart::Checkpoint, 0))),
            },
        }
    }

    // Start line and grid.
    if let Some(s) = r.start {
        let p = s.position.truncate();
        if let Some(rp) = near(p).filter(|rp| rp.distance > OFF_ROAD_M) {
            out.push(
                warn(format!("the start line is {:.0} m from the road network", rp.distance))
                    .at(p, point(RacePart::Start, 0))
                    .fix(Fix::SnapStart),
            );
        } else if let Some(d) = road.as_deref().map(|l| race::polyline_distance(l, p).0).filter(|&d| d > OFF_ROAD_M) {
            out.push(
                warn(format!("the start line is {d:.0} m from the route"))
                    .at(p, point(RacePart::Start, 0))
                    .fix(Fix::SnapStart),
            );
        }
    }
    let grid = race::effective_grid(r);
    let off: Vec<(usize, f32)> = grid
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            near(g.position.truncate()).filter(|rp| rp.distance > OFF_ROAD_M).map(|rp| (i, rp.distance))
        })
        .collect();
    if let Some(&(i, d)) = off.first() {
        let text = if off.len() == 1 {
            format!("grid slot {} is {d:.0} m from the road network", i + 1)
        } else {
            format!(
                "grid slots {} are off the road network",
                off.iter().map(|o| (o.0 + 1).to_string()).collect::<Vec<_>>().join(", ")
            )
        };
        out.push(warn(text).at(grid[i].position.truncate(), point(RacePart::Grid, i)).fix(Fix::SnapGrid));
    }

    // The finish.
    if let Some(f) = r.finish.filter(|_| !r.kind.loops()) {
        let p = f.position.truncate();
        if let Some(line) = road.as_deref() {
            let (d, piece) = race::polyline_distance(line, p);
            let (_, last_piece) = race::polyline_distance(line, *r.checkpoints.last().unwrap());
            if d > OFF_ROAD_M {
                out.push(
                    warn(format!("the finish line is {d:.0} m from the route"))
                        .at(p, point(RacePart::Finish, 0))
                        .fix(Fix::SnapFinish),
                );
            } else if piece < last_piece {
                out.push(
                    warn("the finish line comes before the last checkpoint".into()).at(p, point(RacePart::Finish, 0)),
                );
            }
        }
    }
    if road.is_some() && out.iter().all(|c| c.level == Level::Info) {
        let mut parts = Vec::new();
        if r.start.is_some() {
            parts.push("start line on the route".to_string());
        }
        if r.finish.is_some() && !r.kind.loops() {
            parts.push("finish line after the last checkpoint".to_string());
        }
        parts.push(format!("{} grid slots", grid.len()));
        let mut c = Check::new(Level::Info, k, format!("{name}: {}", parts.join(", ")));
        c.ok = true;
        out.push(c);
    }
    let mut t = Check::new(
        Level::Info,
        k,
        if tested.is_tested(r) {
            format!("{name}: tested")
        } else {
            format!("{name}: not tested since the last change")
        },
    );
    t.ok = tested.is_tested(r);
    out.push(t);
    out
}

/// Every race's checks, errors first. `nets` holds the base tracks'
/// networks read so far; `region_tracks` the region's race tracks, whose
/// networks (where read) are tried for broken routes.
pub fn checks(
    map: &MapMod,
    nets: &HashMap<u16, Option<RoadNet>>,
    region_tracks: &[u16],
    tested: &Tested,
) -> Vec<Check> {
    let others: Vec<(u16, &RoadNet)> =
        region_tracks.iter().filter_map(|t| nets.get(t).and_then(|n| n.as_ref()).map(|n| (*t, n))).collect();
    let mut out: Vec<Check> = (0..map.races.len())
        .flat_map(|k| race_checks(map, k, nets.get(&map.races[k].track).and_then(|n| n.as_ref()), &others, tested))
        .collect();
    out.sort_by_key(|c| (std::cmp::Reverse(c.level), c.ok));
    out
}

/// Whether any of `checks` blocks Test race for race `k`.
pub fn blocks(checks: &[Check], k: usize) -> bool {
    checks.iter().any(|c| c.race == Some(k) && c.level == Level::Error)
}

/// The ground under `p` in the edited world (0 with none).
pub fn ground(s: &EditorState, p: Vec2) -> f32 {
    s.session.world.grid_ground(p.extend(1.0e5)).unwrap_or(0.0)
}

/// Run the checks on `s` and fill `s.session.issues` with them.
pub fn refresh(s: &mut EditorState, region_tracks: &[u16], tested: &Tested) -> Vec<Check> {
    let out = checks(&s.session.map, &s.nets, region_tracks, tested);
    s.session.issues = out
        .iter()
        .map(|c| Issue { level: c.level, text: c.text.clone(), race: c.race, at: c.at.map(|p| p.extend(ground(s, p))) })
        .collect();
    out
}

/// A spot moved onto the road at `rp`, its heading along the road in the
/// way nearer its old one.
pub fn along_road(spot: Spot, rp: RoadPoint, z: f32) -> Spot {
    let h = spot.heading.to_radians();
    let dir = if rp.dir.dot(Vec2::new(h.cos(), h.sin())) >= 0.0 { rp.dir } else { -rp.dir };
    let heading = if dir == Vec2::ZERO { spot.heading } else { dir.y.atan2(dir.x).to_degrees() };
    Spot { position: rp.at.extend(z), heading }
}

/// Apply `fix` to race `k` of `map` (the pure part of [`apply`]).
/// `net`: the race's road network; `ground`: heights for moved spots.
pub fn fix_race(map: &mut MapMod, k: usize, fix: &Fix, net: Option<&RoadNet>, ground: &dyn Fn(Vec2) -> f32) -> bool {
    let Some(r) = map.races.get_mut(k) else { return false };
    match fix {
        Fix::TryTrack(t) => r.track = *t,
        Fix::Laps(n) => r.laps = *n,
        Fix::Rename(id) => r.id = id.clone(),
        Fix::SnapStart => {
            let (Some(net), Some(s)) = (net, r.start) else { return false };
            let Some(rp) = race::snap(net, s.position.truncate()) else { return false };
            r.start = Some(along_road(s, rp, ground(rp.at)));
        }
        Fix::SnapFinish => {
            let (Some(net), Some(f)) = (net, r.finish) else { return false };
            let line = race::route(net, r, &|_| None).ok().map(|route| route.road.points);
            let p = f.position.truncate();
            let rp = match line.as_deref() {
                Some(l) if l.len() >= 2 => {
                    let (_, i) = race::polyline_distance(l, p);
                    let (a, b) = (l[i], l[i + 1]);
                    let ab = b - a;
                    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
                    let at = a + ab * t;
                    RoadPoint { at, dir: ab.normalize_or_zero(), segment: 0, distance: at.distance(p) }
                }
                _ => match race::snap(net, p) {
                    Some(rp) => rp,
                    None => return false,
                },
            };
            // Along the route's driving direction.
            let heading = if rp.dir == Vec2::ZERO { f.heading } else { rp.dir.y.atan2(rp.dir.x).to_degrees() };
            r.finish = Some(Spot { position: rp.at.extend(ground(rp.at)), heading });
        }
        Fix::SnapGrid => {
            let Some(net) = net else { return false };
            if r.grid.is_empty() {
                // The default grid follows the start: put the start on the road.
                let Some(s) = r.start.or_else(|| race::default_start(r)) else { return false };
                let Some(rp) = race::snap(net, s.position.truncate()) else { return false };
                r.start = Some(along_road(s, rp, ground(rp.at)));
                let still_off = race::effective_grid(r)
                    .iter()
                    .any(|g| race::snap(net, g.position.truncate()).is_some_and(|rp| rp.distance > OFF_ROAD_M));
                if !still_off {
                    return true;
                }
                r.grid = race::effective_grid(r);
            }
            for g in &mut r.grid {
                let p = g.position.truncate();
                let Some(rp) = race::snap(net, p).filter(|rp| rp.distance > OFF_ROAD_M) else { continue };
                let across = rp.dir.perp();
                let side = (p - rp.at).dot(across).clamp(-GRID_SIDE_M, GRID_SIDE_M);
                let at = rp.at + across * side;
                g.position = at.extend(ground(at));
            }
        }
    }
    true
}

/// Apply `fix` to race `k` as one undo step.
pub fn apply(s: &mut EditorState, k: usize, fix: &Fix) {
    let Some(track) = s.session.map.races.get(k).map(|r| r.track) else { return };
    let net = s.nets.get(&track).and_then(|n| n.clone());
    let heights = |p: Vec2| s.session.world.grid_ground(p.extend(1.0e5)).unwrap_or(0.0);
    let mut map = s.session.map.clone();
    if fix_race(&mut map, k, fix, net.as_ref(), &heights) {
        s.session.map = map;
        commit(s, &fix.label());
    }
}

/// Take the camera to `at` (from above and behind, looking down at it)
/// and select the race part.
pub fn show(s: &mut EditorState, c: &Check) {
    if let Some(k) = c.race {
        s.race = Some(k);
        s.mode = super::state::Mode::Races;
    }
    s.point = c.point;
    if let Some(at) = c.at {
        let target = at.extend(ground(s, at));
        let f = s.camera.forward();
        let back = Vec2::new(f.x, f.y).normalize_or(Vec2::Y);
        let eye = target + (-back * 45.0).extend(40.0);
        let look = crate::draw::camera::Camera::looking_at(eye, target);
        (s.camera.position, s.camera.yaw, s.camera.pitch) = (look.position, look.yaw, look.pitch);
    }
}

/// The colour of a level (the launcher's semantic colours).
pub fn colour(c: &Check) -> egui::Color32 {
    let t = crate::launch::theme::THEME_A;
    let rgb = |c: crate::launch::theme::Rgb| egui::Color32::from_rgb(c.0, c.1, c.2);
    match c.level {
        Level::Error => rgb(t.err),
        Level::Warning => rgb(t.warn),
        Level::Info if c.ok => rgb(t.ok),
        Level::Info => rgb(t.info),
    }
}

/// A level's mark, drawn (the editor's fonts have no symbol glyphs): a
/// cross in a disc for an error, a triangle for a warning, a tick for a
/// passed check, a dot for information.
pub fn mark(ui: &mut egui::Ui, c: &Check) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let p = ui.painter();
    let col = colour(c);
    let o = rect.center();
    let stroke = egui::Stroke::new(1.8, col);
    match (c.level, c.ok) {
        (Level::Error, _) => {
            p.circle_stroke(o, 6.0, egui::Stroke::new(1.4, col));
            p.line_segment([o + egui::vec2(-3.0, -3.0), o + egui::vec2(3.0, 3.0)], stroke);
            p.line_segment([o + egui::vec2(-3.0, 3.0), o + egui::vec2(3.0, -3.0)], stroke);
        }
        (Level::Warning, _) => {
            let tri = vec![o + egui::vec2(0.0, -6.5), o + egui::vec2(6.5, 5.5), o + egui::vec2(-6.5, 5.5)];
            p.add(egui::Shape::closed_line(tri, egui::Stroke::new(1.4, col)));
            p.line_segment([o + egui::vec2(0.0, -2.5), o + egui::vec2(0.0, 1.5)], stroke);
            p.circle_filled(o + egui::vec2(0.0, 3.5), 0.9, col);
        }
        (_, true) => {
            p.line(vec![o + egui::vec2(-5.0, 0.5), o + egui::vec2(-1.5, 4.0), o + egui::vec2(5.0, -4.0)], stroke);
        }
        _ => {
            p.circle_filled(o, 2.5, col);
        }
    }
}

/// The Validation tab: one row per entry with Show and its fix.
pub fn panel(s: &mut EditorState, checks: &[Check], ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    if checks.is_empty() {
        ui.label("No races yet: Race > New race.");
    }
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        for (i, c) in checks.iter().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal(|ui| {
                    mark(ui, c);
                    ui.label(&c.text);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let (Some(fix), Some(k)) = (&c.fix, c.race)
                            && ui.button(fix.label()).clicked()
                        {
                            apply(s, k, fix);
                            changed = true;
                        }
                        if (c.at.is_some() || c.point.is_some()) && ui.button("Show").clicked() {
                            show(s, c);
                        }
                    });
                });
            });
        }
        ui.label(
            egui::RichText::new(
                "Test race starts the game with this mod only, straight on the grid; the career save is not changed.",
            )
            .color(egui::Color32::from_rgb(0xc8, 0xcc, 0xc0)),
        );
    });
    changed
}

#[cfg(test)]
#[path = "validate_tests.rs"]
pub(crate) mod tests;
