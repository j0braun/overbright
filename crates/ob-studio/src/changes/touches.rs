//! "What this change touches" (Studio's Changes view, docs/modding/studio.md "Changes"): for one item, the map regions
//! and stream sections its values are in, the neighbouring sections that draw those sections (the game's visible-
//! section lists, so the sections a player sees the change from), and whether it affects play (the tag a save records,
//! docs/modding/asset-format/layering.md §8): placements and races do, textures alone do not.

use std::collections::BTreeSet;

use glam::Vec2;
use ob_formats::formats::visibility::Visibility;

use super::{Row, looks::TEXTURE, map::{PLACEMENT, RACE}};

/// What one item touches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Touches {
    /// The map regions (`L4RA`); a race names none.
    pub regions: Vec<String>,
    /// The stream sections the item is in: where it is now and, for a moved prop, where it was.
    pub sections: Vec<u32>,
    /// The other sections that draw those sections (empty when the city's section lists are not loaded).
    pub seen_from: Vec<u32>,
    /// A saved game depends on it.
    pub affects_play: bool,
}

impl Touches {
    /// The words for the view's line.
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        if !self.regions.is_empty() {
            parts.push(format!("region {}", self.regions.join(", ")));
        }
        if !self.sections.is_empty() {
            let s: Vec<String> = self.sections.iter().map(u32::to_string).collect();
            parts.push(format!("{} {}", if s.len() == 1 { "section" } else { "sections" }, s.join(", ")));
        }
        if !self.seen_from.is_empty() {
            let s: Vec<String> = self.seen_from.iter().map(u32::to_string).collect();
            parts.push(format!("drawn from {} neighbouring {}: {}", s.len(), if s.len() == 1 { "section" } else { "sections" }, s.join(", ")));
        }
        parts.push(if self.affects_play { "affects play".into() } else { "looks only".into() });
        parts.join(" · ")
    }
}

/// The position a row's `yours` (else game) `position` value holds.
fn position(row: &Row) -> Option<Vec2> {
    let v = row.values.iter().find(|v| v.field == "position")?;
    let a = v.yours.as_ref().or(v.game.as_ref())?.as_array()?;
    let f = |i: usize| a.get(i).and_then(|x| x.as_float().or_else(|| x.as_integer().map(|n| n as f64)));
    Some(Vec2::new(f(0)? as f32, f(1)? as f32))
}

/// What `row` touches; `vis` is the city's section lists when they are loaded.
pub fn touches(row: &Row, vis: Option<&Visibility>) -> Touches {
    let mut t = Touches { affects_play: row.kind == PLACEMENT || row.kind == RACE, ..Default::default() };
    if row.kind == TEXTURE {
        return t;
    }
    let mut sections = BTreeSet::new();
    if row.kind == PLACEMENT {
        // `<region>/<section>/<solid>/<x>:<y>:<z>/<nth>` (an original's) or `<region>/<pack>:<id>` (an addition's).
        let mut p = row.key.split('/');
        if let Some(r) = p.next().filter(|r| !r.is_empty()) {
            t.regions.push(r.to_string());
        }
        if let Some(s) = p.next().and_then(|s| s.parse::<u32>().ok()) {
            sections.insert(s);
        }
        // Where a moved or added prop is now: its section too.
        if let (Some(v), Some(pos)) = (vis, position(row))
            && let Some(s) = v.section_at(pos)
        {
            sections.insert(s);
        }
    }
    if let Some(v) = vis {
        let seen: BTreeSet<u32> = v
            .lists
            .iter()
            .filter(|l| l.visible.iter().any(|s| sections.contains(s)))
            .map(|l| l.section)
            .filter(|s| !sections.contains(s))
            .collect();
        t.seen_from = seen.into_iter().collect();
    }
    t.sections = sections.into_iter().collect();
    t
}
