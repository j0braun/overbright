//! What the map editor reads of the game's data, behind one seam: the
//! region's race tracks, their road networks and the free roam's lanes.
//! [`GameAssets`] reads them from the game folder; [`GivenAssets`] holds
//! them in memory (tests, tools that bring their own). The editor's panels read only through
//! [`MapAssets`], never the game's files directly.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::formats::roads::RoadNet;
use crate::frontend::career::CareerData;
use crate::frontend::career::tracks::Track;

/// A base track a race can be on.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackInfo {
    pub number: u16,
    pub name: String,
}

/// A named neighbourhood: the world rectangle (x, y) the game's own world map shows under its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub lo: glam::Vec2,
    pub hi: glam::Vec2,
}

impl Place {
    pub fn centre(&self) -> glam::Vec2 {
        (self.lo + self.hi) * 0.5
    }
}

/// The neighbourhood a world point (x, y) is in: of the places whose rectangle holds it, the one whose centre is
/// nearest (the game's zoom rectangles overlap), None outside all of them.
pub fn place_at(places: &[Place], p: glam::Vec2) -> Option<&Place> {
    places
        .iter()
        .filter(|q| p.cmpge(q.lo).all() && p.cmple(q.hi).all())
        .min_by(|a, b| a.centre().distance(p).total_cmp(&b.centre().distance(p)))
}

/// The world map's names for its zoom records 1..: after the full map, the neighbourhoods in the game's order.
pub const PLACE_NAMES: [&str; 5] = ["City Core", "Beacon Hill", "Jackson Heights", "Coal Harbor East", "Coal Harbor West"];

/// The neighbourhoods of a free roam whose minimap frame is `frame` (world x at the left edge, world y at the top,
/// the world size it covers) from the world map's zoom records (centre and size in texels of the 512 texel map).
pub fn places_from(frame: [f32; 3], zooms: &[(f32, f32, f32, u32)]) -> Vec<Place> {
    let [left, top, size] = frame;
    let to = |u: f32, v: f32| glam::Vec2::new(left + u / 512.0 * size, top - v / 512.0 * size);
    zooms
        .iter()
        .skip(1)
        .zip(PLACE_NAMES)
        .map(|(&(x, y, s, _), name)| {
            let (a, b) = (to(x, y), to(x + s, y + s));
            Place { name: name.to_string(), lo: a.min(b), hi: a.max(b) }
        })
        .collect()
}

/// The game data a map editor reads.
pub trait MapAssets {
    /// The region's neighbourhoods by name (empty: the game names none for it).
    fn places(&self, _region: &str) -> Vec<Place> {
        Vec::new()
    }
    /// The race tracks of `region` a custom race can borrow the set-up
    /// of (those with a career event), by number.
    fn tracks(&self, region: &str) -> Vec<TrackInfo>;
    /// The road network of base track `track`, its barriers closed (None:
    /// no such track, or its routes are unreadable).
    fn road_net(&self, track: u16) -> Option<RoadNet>;
    /// The region's roads as a whole: the free roam's AI lanes, each a
    /// line of points (empty when unreadable).
    fn free_roam_lines(&self, region: &str) -> Vec<Vec<glam::Vec3>>;
}

/// [`MapAssets`] from the game folder; the career data is read once, on
/// first use.
pub struct GameAssets {
    root: PathBuf,
    career: OnceCell<Option<CareerData>>,
}

impl GameAssets {
    pub fn new(root: PathBuf) -> GameAssets {
        GameAssets { root, career: OnceCell::new() }
    }

    fn career(&self) -> Option<&CareerData> {
        self.career
            .get_or_init(|| {
                CareerData::load(&self.root).map_err(|e| log::warn!("map editor: no career data: {e:#}")).ok()
            })
            .as_ref()
    }

    fn track(&self, number: u16) -> Option<&Track> {
        self.career()?.tracks.get(&number)
    }
}

impl MapAssets for GameAssets {
    fn places(&self, region: &str) -> Vec<Place> {
        // The world map is the free roam's (track 4000).
        match self.track(4000).filter(|t| t.region == region) {
            Some(t) => places_from([t.map_left, t.map_top, t.map_size], &*crate::frontend::screens::WORLD_MAP_ZOOMS),
            None => Vec::new(),
        }
    }

    fn tracks(&self, region: &str) -> Vec<TrackInfo> {
        let Some(career) = self.career() else { return Vec::new() };
        let mut list: Vec<TrackInfo> = career
            .tracks
            .values()
            .filter(|t| t.region == region)
            .filter(|t| career.events.iter().any(|e| e.races.first().map_or(e.track, |r| r.0) == t.number))
            .map(|t| TrackInfo { number: t.number, name: t.name.clone() })
            .collect();
        list.sort_by_key(|t| t.number);
        list
    }

    fn road_net(&self, track: u16) -> Option<RoadNet> {
        let t = self.track(track)?;
        super::race::road_net(&self.root, t).map_err(|e| log::warn!("map editor: {e:#}")).ok()
    }

    fn free_roam_lines(&self, region: &str) -> Vec<Vec<glam::Vec3>> {
        let path = crate::game_fs::path(&self.root, &format!("TRACKS/ROUTES{region}/RoutesFreeRoam.bin"));
        let Ok(data) = crate::game_fs::read_file(&path) else { return Vec::new() };
        let routes = crate::formats::ai_routes::RouteFile::parse(&data);
        routes.records.iter().map(|r| r.points.iter().map(|p| p.position).collect()).collect()
    }
}

/// [`MapAssets`] given in memory: the same tracks for every region.
#[derive(Clone, Default)]
pub struct GivenAssets {
    pub tracks: Vec<TrackInfo>,
    pub nets: HashMap<u16, RoadNet>,
    /// [`MapAssets::free_roam_lines`] of every region.
    pub free_roam: Vec<Vec<glam::Vec3>>,
    /// [`MapAssets::places`] of every region.
    pub places: Vec<Place>,
}

impl MapAssets for GivenAssets {
    fn places(&self, _region: &str) -> Vec<Place> {
        self.places.clone()
    }

    fn tracks(&self, _region: &str) -> Vec<TrackInfo> {
        self.tracks.clone()
    }

    fn road_net(&self, track: u16) -> Option<RoadNet> {
        self.nets.get(&track).cloned()
    }

    fn free_roam_lines(&self, _region: &str) -> Vec<Vec<glam::Vec3>> {
        self.free_roam.clone()
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn the_world_maps_zoom_rectangles_become_named_places() {
        // A 1000 m map with its top left at (-500, 500): texel (256, 256) is the world's origin.
        let zooms = [(0.0, 0.0, 512.0, 0), (128.0, 128.0, 256.0, 0), (0.0, 128.0, 256.0, 0)];
        let p = places_from([-500.0, 500.0, 1000.0], &zooms);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "City Core");
        assert_eq!((p[0].lo, p[0].hi), (Vec2::new(-250.0, -250.0), Vec2::new(250.0, 250.0)));
        assert_eq!(p[1].name, "Beacon Hill");
        // Overlapping rectangles: the nearest centre wins; outside all, none.
        assert_eq!(place_at(&p, Vec2::new(-190.0, 200.0)).map(|q| q.name.as_str()), Some("Beacon Hill"));
        assert_eq!(place_at(&p, Vec2::new(0.0, 0.0)).map(|q| q.name.as_str()), Some("City Core"));
        assert_eq!(place_at(&p, Vec2::new(900.0, 0.0)), None);
    }
}
