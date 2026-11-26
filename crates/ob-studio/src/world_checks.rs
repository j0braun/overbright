//! The World view's checks on the props a mod adds or moves (View-only: nothing here changes the mod):
//!
//! * **pop-in**: the game draws a prop only while the road section under the camera lists the prop's section
//!   (`TRACKS/<region>.BUN`'s visible lists) and the prop is big enough on screen (the scenery size cull,
//!   `docs/notes/graphics/pop-in.md`). A prop added or moved to a place some road section within sight of it does
//!   not draw its section for appears only once the car is in a section that lists it, or not at all from there.
//!   An added prop takes the section of the nearest placement; a moved one keeps its own.
//! * **model**: an added prop whose model no section drawn with the prop's section has a placement of is likely
//!   not loaded there (inferred from the region's placements; the game's own loader is not consulted).
//!
//! The checks are pure functions of [`Prop`]s and the region's visibility lists; [`of`] reads them off a world.

use std::collections::{HashMap, HashSet};

use glam::{Vec2, Vec3};
use nfsu2::studio_api::world::{HIDDEN_FLAG, World, is_panorama};
use ob_formats::formats::visibility::Visibility;

use crate::session::Level;
use crate::validate::Check;

/// The most entries of each kind the Validation tab lists (a mod that moves a hundred props the same wrong way
/// would bury the races' entries); the rest are counted in one last entry.
pub const MAX_LISTED: usize = 25;
/// The cull scale of the game's own camera (`256 / tan(fov / 2)` at 70 degrees), for how far a prop is seen.
const CULL_SCALE: f32 = 365.6;
/// The scenery cull's smallest size (pixels of radius on a 512-wide view, `render::MAIN_MIN_SIZE`).
const MIN_SIZE: f32 = 17.0;
/// No prop is looked for further than this, however large (metres).
const MAX_SIGHT: f32 = 400.0;

/// A prop the mod added or moved.
#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    pub name: String,
    pub added: bool,
    pub at: Vec3,
    /// Where an original stood before it was moved.
    pub was: Option<Vec3>,
    pub section: u32,
    pub radius: f32,
    pub solid: usize,
    /// The model has no triangles.
    pub empty: bool,
}

/// How far (metres) a prop of cull radius `r` is seen from: where the size cull stops dropping it.
pub fn sight(r: f32) -> f32 {
    (r + CULL_SCALE * r / MIN_SIZE).min(MAX_SIGHT)
}

/// The props `world` has added or moved, with the sections they are in.
pub fn props_of(world: &World) -> Vec<Prop> {
    let o = &world.map_overlay;
    let mut out = Vec::new();
    let mut take = |i: usize, added: bool| {
        let Some(p) = world.placements.get(i) else { return };
        if p.flags & HIDDEN_FLAG != 0 || is_panorama(p.section) {
            return;
        }
        let Some(solid) = world.solids.get(p.solid) else { return };
        let name = match o.added.get(&i) {
            Some((_, id)) => format!("{} ({id})", solid.name),
            None => solid.name.clone(),
        };
        let was = if added { None } else { o.original_at.get(&i).copied() };
        out.push(Prop {
            name,
            added,
            at: p.transform.w_axis.truncate(),
            was,
            section: p.section,
            radius: p.cull_radius,
            solid: p.solid,
            empty: solid.indices.is_empty(),
        });
    };
    let mut moved: Vec<usize> = o.original_at.keys().copied().collect();
    moved.sort_unstable();
    for i in moved {
        if o.first_added.is_none_or(|f| i < f) && o.original_at.get(&i).is_some_and(|&a| a != world.placements[i].transform.w_axis.truncate()) {
            take(i, false);
        }
    }
    let mut added: Vec<usize> = o.added.keys().copied().collect();
    added.sort_unstable();
    for i in added {
        take(i, true);
    }
    out
}

/// The road sections (with a visible list) within `range` m of `at`, with their distance, that do not list `section`.
fn blind(vis: &Visibility, lists: &HashMap<u32, &[u32]>, at: Vec2, section: u32, range: f32) -> Vec<(u32, f32)> {
    let mut out = Vec::new();
    for (_, b) in vis.drivable() {
        let Some(list) = lists.get(&b.section) else { continue };
        let d = b.distance_outside(at, range);
        if d < range && b.section != section && !list.contains(&section) {
            out.push((b.section, d));
        }
    }
    out
}

fn visible_lists(vis: &Visibility) -> HashMap<u32, &[u32]> {
    vis.lists.iter().map(|l| (l.section, l.visible.as_slice())).collect()
}

/// The pop-in entries: a warning per prop that comes in late (or not at all) from a road within sight of it.
pub fn popin(props: &[Prop], vis: &Visibility) -> Vec<Check> {
    let lists = visible_lists(vis);
    let mut out = Vec::new();
    let mut more = 0;
    for p in props {
        let range = sight(p.radius);
        let now = blind(vis, &lists, p.at.truncate(), p.section, range);
        // A moved prop's old place already had the roads it was out of sight from.
        let before: HashSet<u32> = match p.was {
            Some(w) => blind(vis, &lists, w.truncate(), p.section, sight(p.radius)).into_iter().map(|b| b.0).collect(),
            None => HashSet::new(),
        };
        let new: Vec<(u32, f32)> = now.into_iter().filter(|b| !before.contains(&b.0)).collect();
        let Some(&(road, d)) = new.iter().min_by(|a, b| a.1.total_cmp(&b.1)) else { continue };
        if out.len() >= MAX_LISTED {
            more += 1;
            continue;
        }
        let what = if p.added { "added" } else { "moved" };
        out.push(Check::general(
            Level::Warning,
            format!(
                "The {what} prop {} would pop in: road section {road}, {:.0} m from it ({} such section{}), does not draw its section {} (seen up to about {:.0} m)",
                p.name,
                d,
                new.len(),
                if new.len() == 1 { "" } else { "s" },
                p.section,
                range
            ),
            p.at.truncate(),
        ));
    }
    if more > 0 {
        let mut c = Check::general(Level::Warning, format!("{more} more added or moved props would pop in"), Vec2::ZERO);
        c.at = None;
        out.push(c);
    }
    out
}

/// Which sections have a placement of each model: the region's own, before the mod.
pub type Models = HashSet<(usize, u32)>;

/// The (model, section) pairs of the original placements of `world`.
pub fn models_of(world: &World) -> Models {
    let last = world.map_overlay.first_added.unwrap_or(world.placements.len());
    world.placements.iter().take(last).map(|p| (p.solid, p.section)).collect()
}

/// The entries for added props whose model has no triangles, or that no section drawn with their own section has a
/// placement of.
pub fn models(props: &[Prop], vis: &Visibility, have: &Models) -> Vec<Check> {
    let lists = visible_lists(vis);
    let mut out = Vec::new();
    for p in props.iter().filter(|p| p.added) {
        if out.len() >= MAX_LISTED {
            break;
        }
        if p.empty {
            out.push(Check::general(Level::Error, format!("The added prop {} has no model to draw", p.name), p.at.truncate()));
            continue;
        }
        // The road sections that draw the prop's section, and which of them draw a section with this model.
        let drawn: Vec<&[u32]> = lists.values().copied().filter(|l| l.contains(&p.section)).collect();
        if drawn.is_empty() {
            continue;
        }
        let without = drawn.iter().filter(|l| !l.iter().chain(std::iter::once(&p.section)).any(|&s| have.contains(&(p.solid, s)))).count();
        if without == 0 {
            continue;
        }
        let level = if without == drawn.len() { Level::Error } else { Level::Warning };
        out.push(Check::general(
            level,
            format!(
                "The added prop {} likely cannot be loaded where {without} of the {} road sections that draw its section {} are: no section drawn with it has a placement of its model",
                p.name,
                drawn.len(),
                p.section
            ),
            p.at.truncate(),
        ));
    }
    out
}

/// Every entry for the props `world` added or moved.
pub fn of(world: &World) -> Vec<Check> {
    if world.visibility.lists.is_empty() {
        return Vec::new();
    }
    let props = props_of(world);
    if props.is_empty() {
        return Vec::new();
    }
    let mut out = models(&props, &world.visibility, &models_of(world));
    out.extend(popin(&props, &world.visibility));
    out
}

#[cfg(test)]
#[path = "world_checks_tests.rs"]
mod tests;
