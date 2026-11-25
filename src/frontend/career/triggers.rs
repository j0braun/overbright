//! World and route triggers, neighbourhoods, the free-roam start.

use super::*;

/// A trigger the player drives into: its name, position, the box it
/// covers on the ground and, for a route trigger, the polygon inside it.
#[derive(Clone, Copy, Debug)]
pub struct Trigger {
    pub name: u32,
    pub position: Vec3,
    /// +0x34, +0x38, +0x3C (i32 each): the point on the road the GPS
    /// routes to (0x529490 reads them with fild); world triggers: the
    /// position.
    pub gps: Vec3,
    pub min: Vec2,
    pub max: Vec2,
    /// The ground polygon (route triggers; 0 points: the box alone).
    pub polygon: [Vec2; 8],
    pub points: u8,
}

impl Trigger {
    /// 0x5D9C00: inside the box (0x43D410, edges included), then inside
    /// the polygon by the even-odd rule (0x43D890) when there is one.
    pub fn contains(&self, at: Vec2) -> bool {
        let in_box = at.x >= self.min.x && at.x <= self.max.x && at.y >= self.min.y && at.y <= self.max.y;
        if !in_box || self.points == 0 {
            return in_box;
        }
        let pts = &self.polygon[..self.points as usize];
        let mut inside = false;
        let mut j = pts.len() - 1;
        for (i, a) in pts.iter().enumerate() {
            let b = pts[j];
            if (a.y > at.y) != (b.y > at.y) && at.x < (b.x - a.x) * (at.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

/// The region's route triggers (chunk 0x3414A, which every
/// Paths<track>.bin of the region repeats; loaded by 0x5D9B40 into the
/// route data 0x88F298: records +0x43C, bytes +0x438, an index of the
/// first record of each kind 0..0x14 at +0x440, 0x5D5660). Each record:
/// +0x00 kind (sorted by it), +0x04 x, +0x08 y, +0x14 z, +0x20 the ground
/// box (min x, min y, max x, max y), +0x30 its name hash, +0x34 / +0x38 / +0x3C the GPS point on the road
/// (i32 each, 0x529490), +0x40 (i16) the
/// polygon's point count, +0x42 (i16) the record's size, +0x44 the
/// polygon (x, y pairs). Looked up by name among kinds 13–15 (mask
/// 0xE000, 0x4FFC50 → 0x5D9C80): the event markers (`EV_nn`, event
/// +0x28) and the photo-shoot spots (milestone +0x2C, 0x533320) are kind
/// 13; the shops (their +0x3C) are kind 14 as well as world triggers.
/// +0x0C / +0x10 (1, 0 for `EV_01`) are not traced.
pub struct RouteTriggers {
    records: Vec<Trigger>,
    /// The kind-0x12 records: the engage tips (condition-15 messages by
    /// their hash, 0x529C50).
    tips: Vec<Trigger>,
    /// The kind-0x13 records: the hidden cash pickups ([`Bank`] by their
    /// hash, 0x5283D0).
    banks: Vec<Trigger>,
    /// The kind-0xB records: the shortcuts (0x56F970; four, the same in
    /// every Paths file).
    shortcuts: Vec<Trigger>,
    /// The kind-0x11 records: the neighbourhoods (+0x30 the bin hash of a
    /// name in 0x5D3DF0's table, 0x800C38).
    zones: Vec<Trigger>,
}

/// 0x5D3DF0's thirteen neighbourhood names (0x800C38..0x800CA0), with
/// the bit 0x525660 names each `NEIGHBOURHOOD_<name>` by (0x52573C).
pub const NEIGHBOURHOODS: [(&str, u32); 13] = [
    ("JACKSON_HEIGHTS", 0x20_0000),
    ("PIGEON_PARK", 0x10_0000),
    ("BEACON_HILL_EAST", 1),
    ("BEACON_HILL_WEST", 1),
    ("HOTELS", 0x100_0000),
    ("FINANCE", 0x80_0000),
    ("FORT_UNION", 0x8000),
    ("EL_NORTE", 0x10),
    ("SOUTH_MARKET", 0x4_0000),
    ("STADIUM", 0x80),
    ("AIRPORT", 0x40),
    ("COAL_HARBOUR_EAST", 0x800_0000),
    ("COAL_HARBOUR_WEST", 0x1000_0000),
];

/// A zone trigger's +0x30 to the player's +0x9B8 (0x5EB0C1: 0x5D3DF0's
/// name, then `bin_hash("NEIGHBOURHOOD_%s")` by 0x50B9C0); 0 when the
/// name is not in the table.
pub fn neighbourhood_hash(name: u32) -> u32 {
    use crate::formats::hash::bin_hash;
    NEIGHBOURHOODS.iter().find(|n| bin_hash(n.0) == name).map_or(0, |n| bin_hash(&format!("NEIGHBOURHOOD_{}", n.0)))
}

impl RouteTriggers {
    pub fn load(root: &Path) -> RouteTriggers {
        let dir = crate::game_fs::path(root, "TRACKS/ROUTESL4RA");
        let file = std::fs::read_dir(&dir).ok().and_then(|d| {
            d.flatten()
                .map(|e| e.path())
                .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Paths")))
        });
        let data = file.and_then(|f| std::fs::read(f).ok()).unwrap_or_default();
        RouteTriggers {
            records: Self::parse(&data, 13..=15),
            tips: Self::parse(&data, 0x12..=0x12),
            banks: Self::parse(&data, 0x13..=0x13),
            shortcuts: Self::parse(&data, 0xB..=0xB),
            zones: Self::parse(&data, 0x11..=0x11),
        }
    }

    /// The first kind-0x11 trigger holding a point (0x5D9C00(point, 0x11,
    /// 0)), by its place among them.
    pub fn zone_at(&self, at: Vec2) -> Option<usize> {
        self.zones.iter().position(|t| t.contains(at))
    }

    /// A kind-0x11 trigger's +0x30.
    pub fn zone_name(&self, i: usize) -> u32 {
        self.zones.get(i).map_or(0, |t| t.name)
    }

    /// The first kind-0xB trigger holding a point (0x5D9C00(point, 0xB,
    /// 0), 0x56F986), by its place among them.
    pub fn shortcut_at(&self, at: Vec2) -> Option<usize> {
        self.shortcuts.iter().position(|t| t.contains(at))
    }

    /// The first kind-0x13 trigger holding a point (0x5D9C00(point, 0x13,
    /// 0), 0x5EB12F): the cash pickup the car is in.
    pub fn bank_at(&self, at: Vec2) -> Option<&Trigger> {
        self.banks.iter().find(|t| t.contains(at))
    }

    /// The kind-0x13 (cash pickup) triggers in record order.
    pub fn banks(&self) -> impl Iterator<Item = &Trigger> {
        self.banks.iter()
    }

    /// The first kind-0x12 trigger holding a point (0x5D9C00(point, 0x12,
    /// 0)): the engage tip the car is in.
    pub fn tip_at(&self, at: Vec2) -> Option<&Trigger> {
        self.tips.iter().find(|t| t.contains(at))
    }

    /// The kind-0x12 (engage tip) triggers in record order.
    pub fn tips(&self) -> impl Iterator<Item = &Trigger> {
        self.tips.iter()
    }

    /// The records of one `kind`, in record order (the route data's
    /// triggers 0x5D9C80 walks for a mask bit `1 << kind`).
    pub fn load_kind(root: &Path, kind: u32) -> Vec<Trigger> {
        Self::load_kind_in(root, "L4RA", kind)
    }

    /// [`RouteTriggers::load_kind`] for another region's route data
    /// (`TRACKS/ROUTES<region>/PathsFreeRoam.bin`; the dyno's L4RG holds
    /// 15 kind-6 records and the one kind-0x14 zone around the dyno, the
    /// same chunk in every Paths47xx.bin of the region).
    pub fn load_kind_in(root: &Path, region: &str, kind: u32) -> Vec<Trigger> {
        let data = crate::game_fs::read(root, &format!("TRACKS/ROUTES{region}/PathsFreeRoam.bin")).unwrap_or_default();
        Self::parse(&data, kind..=kind)
    }

    /// The route data a track load reads (0x57F46A..0x57F4FD, into the
    /// loader's +0x10): in a race (0x89E7B0 ≠ 0) the track's own
    /// `TRACKS/ROUTES<region>/Paths<track>.bin` when the file is there
    /// (0x57CAD0), else the region's `PathsFreeRoam.bin`; its records of
    /// one `kind` (the cover zones 0x61E710 tests: kinds 5 and 6, so a
    /// race's covered drift lot or speedway garage keeps out the rain).
    pub fn load_kind_for(root: &Path, region: &str, race_track: Option<u32>, kind: u32) -> Vec<Trigger> {
        let own = race_track.and_then(|t| crate::game_fs::read(root, &format!("TRACKS/ROUTES{region}/Paths{t}.bin")).ok());
        match own {
            Some(data) => Self::parse(&data, kind..=kind),
            None => Self::load_kind_in(root, region, kind),
        }
    }

    /// The `kinds` records of the first 0x3414A chunk.
    fn parse(d: &[u8], kinds: std::ops::RangeInclusive<u32>) -> Vec<Trigger> {
        let key = 0x3414Au32.to_le_bytes();
        let Some(at) = d.windows(4).position(|w| w == key) else {
            return Vec::new();
        };
        if at + 8 > d.len() {
            return Vec::new();
        }
        let end = (at + 8 + read_u32(d, at + 4) as usize).min(d.len());
        let mut out = Vec::new();
        let mut p = at + 8;
        while p + 0x44 <= end {
            let (kind, n, size) = (read_u32(d, p), read_u16(d, p + 0x40) as usize, read_u16(d, p + 0x42) as usize);
            if size < 0x44 || p + size > end {
                break;
            }
            if kinds.contains(&kind) {
                let mut polygon = [Vec2::ZERO; 8];
                let fits = n <= 8 && 0x44 + n * 8 <= size;
                if fits {
                    for (k, v) in polygon.iter_mut().enumerate().take(n) {
                        *v = Vec2::new(read_f32(d, p + 0x44 + k * 8), read_f32(d, p + 0x48 + k * 8));
                    }
                }
                out.push(Trigger {
                    name: read_u32(d, p + 0x30),
                    position: Vec3::new(read_f32(d, p + 4), read_f32(d, p + 8), read_f32(d, p + 0x14)),
                    gps: Vec3::new(
                        read_u32(d, p + 0x34) as i32 as f32,
                        read_u32(d, p + 0x38) as i32 as f32,
                        read_u32(d, p + 0x3C) as i32 as f32,
                    ),
                    min: Vec2::new(read_f32(d, p + 0x20), read_f32(d, p + 0x24)),
                    max: Vec2::new(read_f32(d, p + 0x28), read_f32(d, p + 0x2C)),
                    polygon,
                    points: if fits { n as u8 } else { 0 },
                });
            }
            p += size;
        }
        out
    }

    /// The first kind 13–15 trigger with this name (0x4FFC50).
    pub fn find(&self, name: u32) -> Option<Trigger> {
        self.records.iter().find(|t| t.name == name).copied()
    }

    /// The kind 13–15 triggers in record order (0x5D9C80 with mask 0xE000
    /// and no point).
    pub fn iter(&self) -> impl Iterator<Item = &Trigger> {
        self.records.iter()
    }

    /// The kind 13–15 triggers holding a point, in record order (0x5D9C80
    /// with a point: 0x5D9C00's box, then polygon test).
    pub fn containing(&self, at: Vec2) -> impl Iterator<Item = &Trigger> {
        self.records.iter().filter(move |t| t.contains(at))
    }
}

/// Where a route trigger stands (see [`RouteTriggers`]).
pub fn trigger_spot(root: &Path, name: u32) -> Option<Vec3> {
    RouteTriggers::load(root).find(name).map(|t| t.position)
}

/// The world's triggers (the track bundle's 0x37090 chunks, 0xA0 bytes
/// after the 16-byte alignment): +0x20 the name (the shops' +0x3C names,
/// 0x533320), +0x40 a 3×4 rotation, +0x70 the position, +0x80 and +0x90
// prov: inferred frontend.career.world_triggers
/// the corners of the box it covers (**inferred** from the shops').
pub fn world_triggers(root: &Path, region: &str) -> HashMap<u32, Trigger> {
    let Ok(data) = crate::game_fs::read(root, &format!("TRACKS/{region}.BUN")) else {
        return HashMap::new();
    };
    chunks(&data, 0)
        .filter(|c| c.id == 0x37090)
        .filter_map(|c| {
            let b = c.payload();
            (b.len() >= 0xA0).then(|| {
                let lo = Vec2::new(read_f32(b, 0x80), read_f32(b, 0x84));
                let hi = Vec2::new(read_f32(b, 0x90), read_f32(b, 0x94));
                let position = Vec3::new(read_f32(b, 0x70), read_f32(b, 0x74), read_f32(b, 0x78));
                let t = Trigger {
                    name: read_u32(b, 0x20),
                    position,
                    gps: position,
                    min: lo.min(hi),
                    max: lo.max(hi),
                    polygon: [Vec2::ZERO; 8],
                    points: 0,
                };
                (t.name, t)
            })
        })
        .collect()
}

/// Where explore mode starts: the free-roam grid (ROUTESL4RA
/// TrackPosMarkersFreeRoam.bin, ForwardStartingPosition).
pub fn free_roam_start(root: &Path) -> Option<(Vec3, f32)> {
    let markers = load_markers(&crate::game_fs::path(root, "TRACKS/ROUTESL4RA/TrackPosMarkersFreeRoam.bin")).ok()?;
    markers.iter().find(|m| m.name == bin_hash("ForwardStartingPosition")).map(|m| (m.position, m.heading))
}

#[cfg(test)]
mod cover_zone_tests {
    use super::RouteTriggers;

    /// #239: a race reads the cover zones (kinds 5, 6) of its own track's
    /// route data, as the track loader 0x57F46A loads it: the Stadium's
    /// drift lots (L4RC, Paths4312.bin) and the speedways' garages (L4RG)
    /// have zones of their own, none of them free roam's (L4RA); free roam
    /// and the dyno read the region's PathsFreeRoam.bin.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_race_reads_its_own_tracks_cover_zones() {
        let root = crate::testkit::game_root();
        let names = |v: &[super::Trigger]| v.iter().map(|t| (t.name, t.position.to_array())).collect::<Vec<_>>();
        let city = RouteTriggers::load_kind(&root, 5);
        assert!(!city.is_empty(), "free roam's covers");
        assert_eq!(names(&RouteTriggers::load_kind_for(&root, "L4RA", None, 5)), names(&city), "free roam");
        for (region, track, kind) in [("L4RC", 4312, 5), ("L4RC", 4311, 5), ("L4RG", 4714, 6), ("L4RB", 4201, 6), ("L4RF", 4601, 6)] {
            let own = RouteTriggers::load_kind_for(&root, region, Some(track), kind);
            assert!(!own.is_empty(), "{region} {track}: zones of kind {kind}");
            let free = RouteTriggers::load_kind(&root, kind);
            assert!(own.iter().all(|t| !free.iter().any(|f| f.position == t.position)), "{region} {track}: not free roam's");
            assert_eq!(names(&own), names(&RouteTriggers::load_kind_in(&root, region, kind)), "{region} {track}: the region's zones");
        }
        // A track without its own file (none in the game; a missing one):
        // the region's PathsFreeRoam.bin.
        assert_eq!(names(&RouteTriggers::load_kind_for(&root, "L4RC", Some(4399), 5)), names(&RouteTriggers::load_kind_in(&root, "L4RC", 5)));
    }
}
