//! Race routes and placed markers (TrackPosMarkers*.bin, Paths<track>.bin).

use super::*;

/// A placed marker (TrackPosMarkers*.bin, chunk 0x34146 after 16 bytes:
/// 0x30-byte records `u32 11, u32 11, u32 name hash, u32 0, f32 x, y, z,
/// u32 0, u16 heading, u16 —, u16 track, …`). Names (`bin_hash`):
/// ForwardStartingPosition (×8, the grid), ForwardStartLine,
/// ForwardFinishLine and the Backward set; in free roam also the shops.
#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub name: u32,
    pub position: Vec3,
    /// Radians around +z from +x (the game's 16-bit angle, 0x43C570).
    pub heading: f32,
    pub track: u16,
    /// +0x0C: a starting position's grid slot, 0..7 (0x5EB5E0 looks the
    /// markers up by it; two rows of four).
    pub slot: u32,
}

pub fn load_markers(path: &Path) -> Result<Vec<Marker>> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let body = data.get(16..).unwrap_or(&[]);
    Ok(body
        .chunks_exact(0x30)
        .map(|r| Marker {
            name: read_u32(r, 8),
            position: Vec3::new(read_f32(r, 16), read_f32(r, 20), read_f32(r, 24)),
            heading: read_u16(r, 32) as f32 * std::f32::consts::TAU / 65536.0,
            track: read_u16(r, 36),
            slot: read_u32(r, 12),
        })
        .collect())
}

/// A race route: the checkpoints in driving order (world x/y), the start
/// grid, and the start and finish lines.
#[derive(Clone, Debug)]
pub struct Route {
    pub checkpoints: Vec<Vec2>,
    /// The route closes on itself (circuits).
    pub loops: bool,
    /// Grid slots, pole first: position and heading.
    pub grid: Vec<(Vec3, f32)>,
    pub start_line: Option<Vec3>,
    /// The start line marker's heading (radians), the origin of a race's
    /// start scene (0x431990).
    pub start_heading: Option<f32>,
    /// The finish line marker's heading (`FinishLine`, radians).
    pub finish_heading: Option<f32>,
    pub finish_line: Option<Vec3>,
    /// The road the race runs (0x5E5BC0 through the paths file's road
    /// network), and where on it the start line and the finish line are
    /// (0x5EE630): a circuit's finish line is its start line, a sprint's
    /// its own.
    pub road: RoadRoute,
    pub start_at: f32,
    pub finish_at: f32,
    /// SplitTimeBoundary1–3 (the same names both ways), where placed:
    /// their distances along the road (0x5EE500).
    pub splits: [Option<f32>; 3],
    /// Run the other way (0x89E7A4).
    pub backward: bool,
}

impl Route {
    /// Straight-line length through the checkpoints, metres.
    pub fn length(&self) -> f32 {
        self.checkpoints.windows(2).map(|w| w[0].distance(w[1])).sum()
    }
}

pub fn route_path(root: &Path, track: &Track) -> PathBuf {
    crate::game_fs::path(root, &format!("TRACKS/ROUTES{}/Paths{}.bin", track.region, track.number))
}

/// A track's route. The checkpoints are the track's record in the paths
/// file's 0x3414C table (0x110-byte records: u16 track, u8 count, u8 loop
/// flag, u8 —, u8 width?, u16 —, then up to 33 × (f32 x, f32 y); read at
/// 0x5D9AA0); markers come from the region's TrackPosMarkersAll.bin.
/// `backward` runs the route the other way with the Backward markers.
pub fn load_route(root: &Path, track: &Track, backward: bool) -> Result<Route> {
    let path = route_path(root, track);
    let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut checkpoints = Vec::new();
    let mut loops = false;
    crate::formats::bundle::walk(chunks(&data, 0), &mut |c| {
        if c.id == 0x3414C && checkpoints.is_empty() {
            for r in c.body.chunks_exact(0x110) {
                if read_u16(r, 0) == track.number {
                    let count = (r[2] as usize).min(33);
                    loops = r[3] != 0;
                    checkpoints =
                        (0..count).map(|k| Vec2::new(read_f32(r, 8 + k * 8), read_f32(r, 12 + k * 8))).collect();
                }
            }
        }
    });
    anyhow::ensure!(checkpoints.len() >= 2, "no checkpoints for track {} in {}", track.number, path.display());
    if backward {
        checkpoints.reverse();
    }
    let markers = load_markers(&crate::game_fs::path(root, &format!("TRACKS/ROUTES{}/TrackPosMarkersAll.bin", track.region)))?;
    let dir = if backward { "Backward" } else { "Forward" };
    let named = |what: &str| bin_hash(&format!("{dir}{what}"));
    let mine = || markers.iter().filter(|m| m.track == track.number);
    let mut slots: Vec<&Marker> = mine().filter(|m| m.name == named("StartingPosition")).collect();
    // By grid slot (marker +0x0C; 0x6065C0 places a racer at its info +3 − 1).
    slots.sort_by_key(|m| m.slot);
    let grid: Vec<(Vec3, f32)> = slots.iter().map(|m| (m.position, m.heading)).collect();
    let start_line = mine().find(|m| m.name == named("StartLine")).map(|m| m.position);
    let start_heading = mine().find(|m| m.name == named("StartLine")).map(|m| m.heading);
    let finish_line = mine().find(|m| m.name == named("FinishLine")).map(|m| m.position);
    let finish_heading = mine().find(|m| m.name == named("FinishLine")).map(|m| m.heading);
    // The road through the checkpoints; the checkpoints themselves stand
    // in if the network has no way through (none of the game's tracks).
    let mut net = RoadNet::parse(&data);
    // The race's barriers close their road records first (0x578070).
    let n = track.number as i16;
    net.close_barriers(&[&format!("BARRIERS_{n}"), &format!("PLAYER_BARRIERS_{n}")]);
    let road = match net.route(&checkpoints, loops) {
        Some(r) => net.race_road(&r, loops),
        None => RoadRoute::new(checkpoints.clone(), loops),
    };
    // 0x5EE500: a marker by a fresh locator, else a default percent: the
    // start line 5, a sprint's finish line the start's + 95 (past 100,
    // less 100).
    let start_at = start_line.map_or(road.length * 0.05, |p| road.locate_once(p.truncate()));
    let finish_at = match finish_line {
        _ if loops => start_at,
        Some(p) => road.locate_once(p.truncate()),
        None => {
            let pct = start_at / road.length.max(1.0) * 100.0 + 95.0;
            (if pct > 100.0 { pct - 100.0 } else { pct }) * road.length / 100.0
        }
    };
    let splits = [1, 2, 3].map(|k| {
        mine()
            .find(|m| m.name == bin_hash(&format!("SplitTimeBoundary{k}")))
            .map(|m| road.locate_once(m.position.truncate()))
    });
    Ok(Route {
        checkpoints,
        loops,
        grid,
        start_line,
        start_heading,
        finish_line,
        finish_heading,
        road,
        start_at,
        finish_at,
        splits,
        backward,
    })
}
