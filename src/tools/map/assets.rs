//! What the map editor reads of the game's data, behind one seam: the
//! region's race tracks, their road networks and the free roam's lanes.
//! [`GameAssets`] reads them from the game folder; [`GivenAssets`] holds
//! them in memory (tests, tools that bring their own). The editor's panels read only through
//! [`MapAssets`], never the game's files directly.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::formats::roads::RoadNet;
use crate::frontend::career::{CareerData, Track};

/// A base track a race can be on.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackInfo {
    pub number: u16,
    pub name: String,
}

/// The game data a map editor reads.
pub trait MapAssets {
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
}

impl MapAssets for GivenAssets {
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
