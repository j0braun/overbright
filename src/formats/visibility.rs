//! Which sections the game shows, depending on where the player is.
//!
//! Both tables live in the track's resident file (`TRACKS/L4RA.BUN`), inside
//! container 0x80034150:
//!
//! * 0x00034152 — one polygon per *drivable* section (its outline on the map).
//! * 0x00034153 — for each drivable section, the list of sections to load
//!   and draw while the player is inside it: its neighbours, the region's
//!   low-detail panorama section (`x99`), and shared prop/texture sections
//!   (`X`/`Y`, numbers 2400+).

use anyhow::{Context, Result};
use glam::Vec2;

use super::bundle;
use super::reader::Reader;

pub const DRIVABLE_BOUNDARIES: u32 = 0x0003_4152;
/// First u32: how many detail sections each letter has (0x800DFC; 20 in
/// L4RA). Sections n..2n of a letter are the same places at another level
/// of detail and share their collision (0x5D3C20).
pub const SECTION_INFO: u32 = 0x0003_4151;
pub const VISIBLE_SECTIONS: u32 = 0x0003_4153;

#[derive(Clone, Debug)]
pub struct DrivableBoundary {
    pub section: u32,
    pub bbox_min: Vec2,
    pub bbox_max: Vec2,
    pub points: Vec<Vec2>,
}

impl DrivableBoundary {
    /// Distance from `p` to the outline: 0 inside, otherwise to the nearest
    /// edge, never more than `limit` (0x5D70E0).
    pub fn distance_outside(&self, p: Vec2, limit: f32) -> f32 {
        let in_box = |m: f32| {
            self.bbox_min.x - m <= p.x && p.x <= self.bbox_max.x + m && self.bbox_min.y - m <= p.y && p.y <= self.bbox_max.y + m
        };
        if !in_box(limit) {
            return limit;
        }
        if self.contains(p) {
            return 0.0;
        }
        self.edge_distance(p, limit)
    }

    /// Inside, how far `p` is from the outline (0x5D7280); 0 outside.
    pub fn depth(&self, p: Vec2) -> f32 {
        if !self.contains(p) {
            return 0.0;
        }
        self.edge_distance(p, p.distance(self.points[0]))
    }

    fn edge_distance(&self, p: Vec2, start: f32) -> f32 {
        let n = self.points.len();
        let mut best = start;
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            let rel = p - a;
            let length = (b - a).length();
            let dir = (b - a).normalize_or_zero();
            let along = rel.dot(dir);
            let across = rel.x * dir.y - dir.x * rel.y;
            let d = if along < 0.0 {
                (along * along + across * across).sqrt()
            } else if along <= length {
                across.abs()
            } else {
                let past = along - length;
                (across * across + past * past).sqrt()
            };
            best = best.min(d);
        }
        best
    }

    /// Even-odd point-in-polygon test on the ground plane.
    pub fn contains(&self, p: Vec2) -> bool {
        if p.x < self.bbox_min.x || p.y < self.bbox_min.y || p.x > self.bbox_max.x || p.y > self.bbox_max.y {
            return false;
        }
        let mut inside = false;
        let n = self.points.len();
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + n - 1) % n]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    }
}

#[derive(Clone, Debug)]
pub struct VisibleSections {
    pub section: u32,
    pub visible: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Visibility {
    pub boundaries: Vec<DrivableBoundary>,
    pub lists: Vec<VisibleSections>,
    /// Detail sections per letter (chunk `0x34151`).
    pub detail_limit: u32,
}

impl Visibility {
    pub fn parse(track_bun: &[u8]) -> Result<Visibility> {
        let mut boundaries_chunk = None;
        let mut lists_chunk = None;
        let mut detail_limit = 0;
        bundle::walk(bundle::chunks(track_bun, 0), &mut |c| match c.id {
            SECTION_INFO if c.body.len() >= 4 => detail_limit = Reader::new(c.body).u32_at(0).unwrap_or(0),
            DRIVABLE_BOUNDARIES => boundaries_chunk = Some(c.body),
            VISIBLE_SECTIONS => lists_chunk = Some(c.body),
            _ => {}
        });

        // Boundary record: 8 bytes runtime link, u16 section, u16 point
        // count, bbox (min x, min y, max x, max y) at +0xC, a point inside
        // the polygon at +0x1C, then the outline's points from +0x24 (read
        // there by 0x5D7280 and 0x5DB860).
        let mut boundaries = Vec::new();
        let mut body = Reader::new(boundaries_chunk.context("no drivable-section boundaries")?);
        while body.remaining() >= 28 {
            let p = body.position();
            let section = body.u16_at(p + 8)? as u32;
            let count = body.u16_at(p + 10)? as usize;
            let size = 28 + 8 * (count + 1);
            // A cut-off last record ends the table.
            let Ok(r) = body.sub(size) else { break };
            let point = |i: usize| -> Result<Vec2> { Ok(Vec2::new(r.f32_at(36 + i * 8)?, r.f32_at(40 + i * 8)?)) };
            boundaries.push(DrivableBoundary {
                section,
                bbox_min: Vec2::new(r.f32_at(12)?, r.f32_at(16)?),
                bbox_max: Vec2::new(r.f32_at(20)?, r.f32_at(24)?),
                points: (0..count).map(point).collect::<Result<_>>()?,
            });
        }

        // Visible-list record: 8 bytes runtime link, u32 section, u32 unused,
        // u32 count, then a fixed array of 72 u16 section numbers.
        const LIST_RECORD: usize = 0x14 + 72 * 2;
        let lists = Reader::new(lists_chunk.context("no visible-section lists")?)
            .whole_records(LIST_RECORD)
            .map(|r| {
                let count = (r.u32_at(0x10)? as usize).min(72);
                Ok(VisibleSections {
                    section: r.u32_at(0x08)?,
                    visible: (0..count).map(|i| Ok(r.u16_at(0x14 + i * 2)? as u32)).collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Visibility { boundaries, lists, detail_limit })
    }

    /// Whether `section` is one the game lists as drivable (0x5E9C62):
    /// letters A..T, numbers 1 up to the detail limit. Region outlines
    /// (x73, x96, x99 …) are not.
    pub fn is_drivable(&self, section: u32) -> bool {
        let (letter, n) = (section / 100, section % 100);
        (1..=20).contains(&letter) && n >= 1 && n < self.detail_limit
    }

    /// The drivable sections' outlines in the game's list order (file
    /// order; the list at 0x883E70).
    pub fn drivable(&self) -> impl Iterator<Item = (usize, &DrivableBoundary)> {
        self.boundaries.iter().enumerate().filter(|(_, b)| self.is_drivable(b.section))
    }

    /// The drivable section under `p`, or the one whose box centre is
    /// nearest when `p` is off the drivable map (e.g. a flying camera).
    pub fn section_at(&self, p: Vec2) -> Option<u32> {
        let has_list = |s: u32| self.lists.iter().any(|l| l.section == s);
        // The outline test first: few outlines hold `p`, so the list
        // search runs for those alone.
        if let Some(b) = self.drivable().map(|(_, b)| b).find(|b| b.contains(p) && has_list(b.section)) {
            return Some(b.section);
        }
        self.drivable()
            .map(|(_, b)| b)
            .filter(|b| has_list(b.section))
            .min_by(|a, b| {
                let da = ((a.bbox_min + a.bbox_max) * 0.5).distance_squared(p);
                let db = ((b.bbox_min + b.bbox_max) * 0.5).distance_squared(p);
                da.total_cmp(&db)
            })
            .map(|b| b.section)
    }

    pub fn visible_from(&self, section: u32) -> Option<&[u32]> {
        self.lists.iter().find(|l| l.section == section).map(|l| l.visible.as_slice())
    }
}

crate::persist!(Visibility { boundaries, lists, detail_limit });
crate::persist!(DrivableBoundary { section, bbox_min, bbox_max, points });
crate::persist!(VisibleSections { section, visible });
