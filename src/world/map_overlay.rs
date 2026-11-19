//! Map mods' scenery edits on a loaded world (`tools::map`, docs/modding/map-editor.md).
//! Applied after [`World::load`] (so the asset cache keeps the original
//! world) and before the world's renderer is built (so added placements
//! are drawn); the world's loaded state is then taken again, so a kept
//! world's [`World::reset_to_loaded`] comes back to the world with its
//! mods. With no edit for the region nothing is touched.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};

use super::*;
use crate::tools::map::{MapMod, PropEdit, PropRef};

/// How close (metres) an edit's `at` must be to the placement it names.
pub const MATCH_DISTANCE: f32 = 0.05;

/// The flags an added placement takes from a placement of the same solid
/// (its view bits); the ones tied to the original's own role are left
/// out: hidden / group member, no matrix, race-only, drawn-once.
const ADDED_DROPS: u16 = HIDDEN_FLAG | 0x0100 | RACE_ONLY_FLAG | 0x8000;

/// A transform from a position and a rotation (rows: local axes).
pub fn transform_of(position: Vec3, rotation: Mat3) -> Mat4 {
    // Row-vector `p * M` in the file is column-vector `M^T p` here: the
    // rotation's rows become the matrix's columns.
    Mat4::from_cols(
        rotation.row(0).extend(0.0),
        rotation.row(1).extend(0.0),
        rotation.row(2).extend(0.0),
        position.extend(1.0),
    )
}

/// The position and rotation (rows: local axes) of a transform.
pub fn parts_of(t: Mat4) -> (Vec3, Mat3) {
    let r = Mat3::from_cols(t.x_axis.truncate(), t.y_axis.truncate(), t.z_axis.truncate()).transpose();
    (t.w_axis.truncate(), r)
}

impl World {
    /// The solid's index by name.
    pub fn solid_named(&self, name: &str) -> Option<usize> {
        self.solid_by_hash
            .get(&crate::formats::hash::bin_hash(name))
            .copied()
            .filter(|&i| self.solids[i].name == name)
            .or_else(|| self.solids.iter().position(|s| s.name == name))
    }

    /// Where original placement `i` stood before any mod moved it.
    pub fn original_position(&self, i: usize) -> Option<Vec3> {
        self.map_overlay
            .original_at
            .get(&i)
            .copied()
            .or_else(|| self.placements.get(i).map(|p| p.transform.w_axis.truncate()))
    }

    /// The original placement `r` names: the nearest of its section and
    /// solid within [`MATCH_DISTANCE`] of `r.at`, then the `r.nth`-th of
    /// those standing exactly where that one stands (in the original's
    /// order). Positions are the original ones; an added placement is
    /// never named.
    pub fn find_placement(&self, r: &PropRef) -> Option<usize> {
        let solid = self.solid_named(&r.solid)?;
        let Some(f) = self.map_overlay.index.finds.as_deref() else { return self.find_placement_scan(r, solid) };
        let list = f.by_key.get(&(r.section, solid))?;
        let mut near: Option<(f32, Vec3)> = None;
        for &i in list {
            let at = f.at[i as usize];
            let d = at.distance(r.at);
            if d <= MATCH_DISTANCE && near.is_none_or(|(nd, _)| d.total_cmp(&nd).is_lt()) {
                near = Some((d, at));
            }
        }
        let (_, near) = near?;
        list.iter().filter(|&&i| f.at[i as usize] == near).nth(r.nth as usize).map(|&i| i as usize)
    }

    /// [`World::find_placement`] without the index: every placement looked at.
    fn find_placement_scan(&self, r: &PropRef, solid: usize) -> Option<usize> {
        let first_added = self.map_overlay.first_added.unwrap_or(self.placements.len());
        let candidates = || {
            self.placements[..first_added]
                .iter()
                .enumerate()
                .filter(move |(_, p)| p.section == r.section && p.solid == solid)
                .map(move |(i, _)| (i, self.original_position(i).unwrap_or(Vec3::NAN)))
        };
        let (_, near) = candidates()
            .map(|(i, at)| (i, at, at.distance(r.at)))
            .filter(|&(.., d)| d <= MATCH_DISTANCE)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, at, _)| (i, at))?;
        candidates().filter(|&(_, at)| at == near).nth(r.nth as usize).map(|(i, _)| i)
    }

    /// How an original placement is named in a map mod: as the original
    /// data has it (its place before any mod moved it, and which of the
    /// placements of its section and solid at exactly that place it is).
    pub fn placement_ref(&self, region: &str, i: usize) -> Option<PropRef> {
        if let Some(r) = self.map_overlay.originals.get(&i) {
            return Some(r.clone());
        }
        if self.map_overlay.first_added.is_some_and(|f| i >= f) {
            return None;
        }
        let p = self.placements.get(i)?;
        let at = self.original_position(i)?;
        let nth = match self.map_overlay.index.finds.as_deref() {
            Some(f) if i >= f.n => return None,
            Some(f) => f.by_key.get(&(p.section, p.solid)).map_or(0, |l| {
                l.iter().take_while(|&&j| (j as usize) < i).filter(|&&j| f.at[j as usize] == at).count()
            }),
            None => self.placements[..i]
                .iter()
                .enumerate()
                .filter(|(j, q)| q.section == p.section && q.solid == p.solid && self.original_position(*j) == Some(at))
                .count(),
        };
        Some(PropRef {
            region: region.to_string(),
            section: p.section,
            solid: self.solids.get(p.solid)?.name.clone(),
            at,
            nth: nth as u32,
        })
    }

    /// Note that a mod changes original placement `i`, named `target`:
    /// its name and where it stands now are kept (the first time).
    pub fn note_original(&mut self, i: usize, target: &PropRef) {
        let Some(p) = self.placements.get(i) else { return };
        self.map_overlay.original_at.entry(i).or_insert(p.transform.w_axis.truncate());
        self.map_overlay.originals.entry(i).or_insert_with(|| target.clone());
    }

    /// Apply the map mods `mods` (folder name, mod) in order to this world
    /// of `region`. Edits naming a placement or solid the world does not
    /// have are skipped and counted.
    pub fn apply_map_mods(&mut self, region: &str, mods: &[(String, MapMod)]) {
        let edits: Vec<(usize, usize, &PropEdit)> = mods
            .iter()
            .enumerate()
            .flat_map(|(m, (_, md))| md.props.iter().enumerate().map(move |(k, e)| (m, k, e)))
            .filter(|(.., e)| e.region().eq_ignore_ascii_case(region))
            .collect();
        if edits.is_empty() {
            return;
        }
        let _t = crate::prof::span("World: map mods");
        self.index_originals();
        // The originals named first, all on the world as it stands, so an
        // edit never sees another's move (twins at one place keep their
        // order).
        let found: Vec<Option<usize>> =
            edits.iter().map(|(.., e)| e.target().and_then(|t| self.find_placement(t))).collect();
        for ((m, _, e), found) in edits.into_iter().zip(found) {
            let folder = &mods[m].0;
            match (e, found) {
                (PropEdit::Move { target, position, rotation }, Some(i)) => {
                    self.note_original(i, target);
                    self.set_placement_transform(i, transform_of(*position, *rotation));
                    self.map_overlay.moved += 1;
                }
                (PropEdit::Delete { target }, Some(i)) => {
                    self.note_original(i, target);
                    self.hide_placement(i);
                    self.map_overlay.deleted += 1;
                }
                (PropEdit::Add { id, solid, position, rotation, .. }, _) => {
                    match self.add_placement(solid, transform_of(*position, *rotation)) {
                        Some(i) => {
                            self.map_overlay.first_added.get_or_insert(i);
                            self.map_overlay.added.insert(i, (folder.clone(), id.clone()));
                        }
                        None => self.map_overlay.missing += 1,
                    }
                }
                _ => self.map_overlay.missing += 1,
            }
        }
        let applied = &self.map_overlay;
        if applied.missing > 0 {
            log::warn!(
                "map mods: {} edit(s) for {region} name a placement or solid this region does not have",
                applied.missing
            );
        }
        log::info!(
            "map mods on {region}: {} moved, {} deleted, {} added",
            applied.moved,
            applied.deleted,
            applied.added.len()
        );
        self.light_placements = Default::default();
        self.light_by_section = Default::default();
        self.retake_loaded();
    }

    /// The loaded state taken again from the world as it is now (after
    /// its mods), for [`World::reset_to_loaded`].
    pub fn retake_loaded(&mut self) {
        let mut w = crate::cache::Writer::default();
        self.put_rest(&mut w);
        self.loaded = w.buf.into();
    }

    /// Move placement `i` (its knock-over prop's rest pose with it).
    pub fn set_placement_transform(&mut self, i: usize, t: Mat4) {
        let Some(p) = self.placements.get_mut(i) else { return };
        p.transform = t;
        self.changed.push(i);
        for s in self.smackables.iter_mut().filter(|s| s.placement == i) {
            s.rest = t;
        }
    }

    /// Hide placement `i` for good: no scenery group shows it again, and
    /// its knock-over prop goes.
    pub fn hide_placement(&mut self, i: usize) {
        let Some(p) = self.placements.get_mut(i) else { return };
        p.flags |= HIDDEN_FLAG;
        self.changed.push(i);
        for l in self.link_placements.iter_mut().filter(|l| **l == Some(i)) {
            *l = None;
        }
        self.smackables.retain(|s| s.placement != i);
    }

    /// Show a placement hidden by [`World::hide_placement`] again (the
    /// editor's undo of a delete).
    pub fn show_placement(&mut self, i: usize) {
        let Some(p) = self.placements.get_mut(i) else { return };
        p.flags &= !HIDDEN_FLAG;
        self.changed.push(i);
    }

    /// A new placement of the solid named `solid` at `t`: its lower LODs,
    /// view flags and section taken from a placement of the same solid
    /// (the nearest; the section from the nearest placement of any solid
    /// when there is none), a knock-over prop when that one has one.
    /// None when the region has no such solid.
    pub fn add_placement(&mut self, solid: &str, t: Mat4) -> Option<usize> {
        let s = self.solid_named(solid)?;
        let at = t.w_axis.truncate();
        let near = |filter: &dyn Fn(&Placement) -> bool| {
            self.placements
                .iter()
                .enumerate()
                .filter(|(_, p)| filter(p) && p.flags & 0x0100 == 0)
                .min_by(|a, b| {
                    a.1.transform
                        .w_axis
                        .truncate()
                        .distance_squared(at)
                        .total_cmp(&b.1.transform.w_axis.truncate().distance_squared(at))
                })
                .map(|(i, _)| i)
        };
        let template = near(&|p| p.solid == s);
        let section_of = near(&|_| true).map_or(0, |i| self.placements[i].section);
        let (low_solid, mid_solid, flags, cull_radius) = match template {
            Some(i) => {
                let p = &self.placements[i];
                (p.low_solid, p.mid_solid, p.flags & !ADDED_DROPS, p.cull_radius)
            }
            None => {
                let so = &self.solids[s];
                let r = Vec3::from_array(so.bbox_max).distance(Vec3::from_array(so.bbox_min)) * 0.5 + 6.0;
                (None, None, 0, r)
            }
        };
        let i = self.placements.len();
        self.placements.push(Placement {
            solid: s,
            low_solid,
            mid_solid,
            transform: t,
            section: section_of,
            flags,
            cull_radius,
        });
        if let Some(prop) = template.and_then(|ti| self.smackables.iter().find(|p| p.placement == ti)).cloned() {
            self.smackables.push(crate::smackable::Prop { placement: i, rest: t, ..prop });
        }
        Some(i)
    }
}

// ---- The index of the originals and the live re-apply ----------------------

/// Flag 0x0100: a placement an add never takes after ([`World::add_placement`]).
const GROUPED: u16 = 0x0100;
/// No add before an edit.
const NONE: u32 = u32::MAX;
/// The cell (metres, x and y) of the grid the adds' nearest placements are
/// looked up on.
const CELL: f32 = 50.0;
/// A solid with up to this many originals is looked through whole, not
/// on the grid.
const SMALL_SOLID: usize = 2048;

/// A world's original placements indexed for its map mods
/// (`World::map_overlay.index`): by section and solid for naming them
/// ([`World::find_placement`], [`World::placement_ref`]), and what
/// [`World::reapply_map_mod`] keeps between two calls.
#[derive(Clone, Default)]
pub struct MapIndex {
    finds: Option<Arc<Finds>>,
    live: Option<Box<Live>>,
}

impl std::fmt::Debug for MapIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapIndex")
            .field("originals", &self.finds.as_ref().map(|x| x.n))
            .field("live", &self.live.is_some())
            .finish()
    }
}

/// The originals by (section, solid), each list ascending, and where each
/// stood before any mod.
struct Finds {
    n: usize,
    by_key: HashMap<(u32, usize), Vec<u32>>,
    at: Vec<Vec3>,
}

/// The world as it was before any mod (what the live re-apply starts
/// from).
struct Base {
    placements: Vec<Placement>,
    smackables: Vec<crate::smackable::Prop>,
    /// Each placement's first knock-over prop (index into `smackables`).
    prop_of: HashMap<usize, usize>,
    /// The scenery link slots naming each placement.
    links: HashMap<usize, Vec<u32>>,
    /// The originals an add may take after (flag 0x0100 clear), by solid
    /// and on the grid.
    by_solid: HashMap<usize, Vec<u32>>,
    grid: Grid,
}

/// What the last live re-apply did, to tell what the next one changes.
#[derive(Clone)]
struct Live {
    region: String,
    base: Arc<Base>,
    /// Which original each name found (a name always finds the same one).
    found: HashMap<u64, Vec<(PropRef, Option<usize>)>>,
    /// The serial number of each add id's first, second, ... add.
    serials: HashMap<String, Vec<u32>>,
    next_serial: u32,
    /// The moves and deletes of originals, by placement, each placement's
    /// in edit order.
    entries: Vec<Entry>,
    adds: Vec<AddState>,
    /// Each add's place among the adds, by serial (NONE: not in the mod).
    order: Vec<u32>,
}

#[derive(Clone, PartialEq)]
enum Kind {
    Move(Mat4),
    Delete,
}

/// A move or delete of original `j`; `prev` is the serial of the last add
/// before it in the mod (NONE: none), `edit` its index among the region's
/// edits.
#[derive(Clone)]
struct Entry {
    j: u32,
    prev: u32,
    kind: Kind,
    edit: u32,
}

impl Entry {
    fn same(&self, o: &Entry) -> bool {
        self.j == o.j && self.prev == o.prev && self.kind == o.kind
    }
}

/// A placement an add took after: an original or another add (by serial).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pid {
    Orig(u32),
    Add(u32),
}

#[derive(Clone, Copy)]
struct Near {
    pid: Pid,
    d2: f32,
}

/// What an add made: its placement, the placement it took its LODs, flags
/// and knock-over prop from (`template`) and the one it took its section
/// from (`source`), each with its distance squared, and its prop (index
/// into the base's knock-over props).
#[derive(Clone)]
struct AddResult {
    placement: Placement,
    template: Option<Near>,
    source: Option<Near>,
    prop: Option<usize>,
}

#[derive(Clone)]
struct AddState {
    serial: u32,
    solid: Option<usize>,
    position: Vec3,
    rotation: Mat3,
    edit: u32,
    result: Option<AddResult>,
}

/// The nearest placement found so far: by distance squared, then index
/// (as `Iterator::min_by` over the placements in order).
#[derive(Clone, Copy)]
struct Best {
    d2: f32,
    idx: usize,
    pid: Option<Pid>,
}

impl Best {
    const EMPTY: Best = Best { d2: f32::INFINITY, idx: usize::MAX, pid: None };

    fn offer(&mut self, d2: f32, idx: usize, pid: Pid) {
        if self.pid.is_none() || d2.total_cmp(&self.d2).then(idx.cmp(&self.idx)).is_lt() {
            *self = Best { d2, idx, pid: Some(pid) };
        }
    }

    fn near(&self) -> Option<Near> {
        self.pid.map(|pid| Near { pid, d2: self.d2 })
    }
}

/// Placements on an x/y grid of [`CELL`] cells.
#[derive(Default)]
struct Grid {
    cells: HashMap<(i32, i32), Vec<u32>>,
    lo: (i32, i32),
    hi: (i32, i32),
    /// Those not at a finite place (looked at every time).
    odd: Vec<u32>,
}

fn cell_of(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

impl Grid {
    fn of(points: impl Iterator<Item = (u32, Vec3)>) -> Grid {
        let mut g = Grid { lo: (i32::MAX, i32::MAX), hi: (i32::MIN, i32::MIN), ..Grid::default() };
        for (i, p) in points {
            if !p.is_finite() {
                g.odd.push(i);
                continue;
            }
            let c = cell_of(p);
            g.lo = (g.lo.0.min(c.0), g.lo.1.min(c.1));
            g.hi = (g.hi.0.max(c.0), g.hi.1.max(c.1));
            g.cells.entry(c).or_default().push(i);
        }
        g
    }

    /// Hand `f` every placement that could be nearer to `at` than `best`
    /// (rings of cells outwards until the next ring is farther than the
    /// best found).
    fn visit(&self, at: Vec3, best: &mut Best, mut f: impl FnMut(u32, &mut Best)) {
        for &i in &self.odd {
            f(i, best);
        }
        if self.cells.is_empty() {
            return;
        }
        let c = cell_of(at);
        let reach = [c.0.saturating_sub(self.lo.0), self.hi.0.saturating_sub(c.0)]
            .into_iter()
            .chain([c.1.saturating_sub(self.lo.1), self.hi.1.saturating_sub(c.1)])
            .max()
            .unwrap_or(0);
        if !at.is_finite() || reach > 512 {
            for l in self.cells.values() {
                for &i in l {
                    f(i, best);
                }
            }
            return;
        }
        for r in 0..=reach.max(0) {
            // Everything in ring r or beyond is more than (r - 1) cells
            // away; one cell more for rounding.
            if r >= 2 {
                let b = (r - 2) as f32 * CELL;
                if best.pid.is_some() && b * b > best.d2 {
                    break;
                }
            }
            let mut cell = |x: i32, y: i32, best: &mut Best| {
                if let Some(l) = self.cells.get(&(x, y)) {
                    for &i in l {
                        f(i, best);
                    }
                }
            };
            if r == 0 {
                cell(c.0, c.1, best);
                continue;
            }
            for x in c.0 - r..=c.0 + r {
                cell(x, c.1 - r, best);
                cell(x, c.1 + r, best);
            }
            for y in c.1 - r + 1..c.1 + r {
                cell(c.0 - r, y, best);
                cell(c.0 + r, y, best);
            }
        }
    }
}

/// The entries of original `j` (`entries` sorted by placement).
fn run_of(entries: &[Entry], j: u32) -> &[Entry] {
    let s = entries.partition_point(|e| e.j < j);
    let t = s + entries[s..].partition_point(|e| e.j == j);
    &entries[s..t]
}

/// An original's state as the `k`-th add (by `order`) sees it, from its
/// entries: the transform its last move before that add gave it, and
/// whether a delete before it hid it (`k` = NONE: after all edits).
fn as_of<'e>(run: &'e [Entry], k: u32, order: &[u32]) -> (Option<&'e Mat4>, bool) {
    let (mut moved, mut deleted) = (None, false);
    for e in run {
        let adds_before = if e.prev == NONE { 0 } else { order[e.prev as usize] + 1 };
        if adds_before <= k {
            match &e.kind {
                Kind::Move(t) => moved = Some(t),
                Kind::Delete => deleted = true,
            }
        }
    }
    (moved, deleted)
}

fn ref_hash(r: &PropRef) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (r.section, r.solid.as_str(), r.at.to_array().map(f32::to_bits), r.nth).hash(&mut h);
    h.finish()
}

fn same_ref(a: &PropRef, b: &PropRef) -> bool {
    a.section == b.section && a.solid == b.solid && a.at == b.at && a.nth == b.nth
}

fn same_placement(a: &Placement, b: &Placement) -> bool {
    a.solid == b.solid
        && a.low_solid == b.low_solid
        && a.mid_solid == b.mid_solid
        && a.transform == b.transform
        && a.section == b.section
        && a.flags == b.flags
        && a.cull_radius.to_bits() == b.cull_radius.to_bits()
}

/// The same for any add taking after this one (all but the place).
fn same_for_others(a: &Option<AddResult>, b: &Option<AddResult>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let (p, q) = (&a.placement, &b.placement);
            p.solid == q.solid
                && p.low_solid == q.low_solid
                && p.mid_solid == q.mid_solid
                && p.section == q.section
                && p.flags == q.flags
                && p.cull_radius.to_bits() == q.cull_radius.to_bits()
                && a.prop == b.prop
        }
        _ => false,
    }
}

impl Live {
    fn serial(&mut self, id: &str, k: u32) -> u32 {
        let next = &mut self.next_serial;
        let list = match self.serials.get_mut(id) {
            Some(l) => l,
            None => self.serials.entry(id.to_string()).or_default(),
        };
        while list.len() <= k as usize {
            list.push(*next);
            *next += 1;
        }
        list[k as usize]
    }

    fn found(&mut self, w: &World, r: &PropRef) -> Option<usize> {
        let key = ref_hash(r);
        if let Some((_, f)) = self.found.get(&key).and_then(|v| v.iter().find(|(q, _)| same_ref(q, r))) {
            return *f;
        }
        let f = w.find_placement(r);
        self.found.entry(key).or_default().push((r.clone(), f));
        f
    }
}

/// The adds of one re-apply, worked out in order.
struct AddsCtx<'a> {
    base: &'a Base,
    solids: &'a [Solid],
    entries: &'a [Entry],
    order: &'a [u32],
    adds: &'a [AddState],
    results: Vec<Option<AddResult>>,
    /// Each add's placement index (usize::MAX: none).
    index_of: Vec<usize>,
}

impl AddsCtx<'_> {
    fn idx_of(&self, pid: Pid) -> usize {
        match pid {
            Pid::Orig(j) => j as usize,
            Pid::Add(s) => self.index_of[self.order[s as usize] as usize],
        }
    }

    /// The placement nearest to `at` (of solid `solid` if given, flag
    /// 0x0100 clear) as the `k`-th add sees the world.
    fn near(&self, at: Vec3, solid: Option<usize>, k: usize) -> Best {
        let mut best = Best::EMPTY;
        let base = self.base;
        let wants = |j: u32| solid.is_none_or(|s| base.placements[j as usize].solid == s);
        // Originals a move before this add put elsewhere.
        for run in self.entries.chunk_by(|a, b| a.j == b.j) {
            let j = run[0].j;
            if base.placements[j as usize].flags & GROUPED != 0 || !wants(j) {
                continue;
            }
            if let (Some(t), _) = as_of(run, k as u32, self.order) {
                best.offer(t.w_axis.truncate().distance_squared(at), j as usize, Pid::Orig(j));
            }
        }
        // The others where they were loaded.
        let loaded = |j: u32, best: &mut Best| {
            if !wants(j) || as_of(run_of(self.entries, j), k as u32, self.order).0.is_some() {
                return;
            }
            let d2 = base.placements[j as usize].transform.w_axis.truncate().distance_squared(at);
            best.offer(d2, j as usize, Pid::Orig(j));
        };
        match solid.map(|s| base.by_solid.get(&s)) {
            Some(None) => {}
            Some(Some(l)) if l.len() <= SMALL_SOLID => {
                for &j in l {
                    loaded(j, &mut best);
                }
            }
            _ => base.grid.visit(at, &mut best, loaded),
        }
        // The adds before it.
        for (m, r) in self.results[..k].iter().enumerate() {
            if let Some(r) = r
                && solid.is_none_or(|s| r.placement.solid == s)
            {
                let d2 = r.placement.transform.w_axis.truncate().distance_squared(at);
                best.offer(d2, self.index_of[m], Pid::Add(self.adds[m].serial));
            }
        }
        best
    }

    /// The `k`-th add worked out afresh, as [`World::add_placement`] makes
    /// it on the world as the edits before it left it.
    fn compute(&self, k: usize) -> Option<AddResult> {
        let a = &self.adds[k];
        let s = a.solid?;
        let t = transform_of(a.position, a.rotation);
        let at = t.w_axis.truncate();
        let template = self.near(at, Some(s), k).near();
        let source = self.near(at, None, k).near();
        let result = |pid: u32| self.results[self.order[pid as usize] as usize].as_ref();
        let section = match source.map(|n| n.pid) {
            Some(Pid::Orig(j)) => self.base.placements[j as usize].section,
            Some(Pid::Add(sr)) => result(sr).map_or(0, |r| r.placement.section),
            None => 0,
        };
        let (low_solid, mid_solid, flags, cull_radius, prop) = match template.map(|n| n.pid) {
            Some(Pid::Orig(j)) => {
                let p = &self.base.placements[j as usize];
                let deleted = as_of(run_of(self.entries, j), k as u32, self.order).1;
                let prop = self.base.prop_of.get(&(j as usize)).copied().filter(|_| !deleted);
                (p.low_solid, p.mid_solid, p.flags & !ADDED_DROPS, p.cull_radius, prop)
            }
            Some(Pid::Add(sr)) => {
                let r = result(sr)?;
                let p = &r.placement;
                (p.low_solid, p.mid_solid, p.flags & !ADDED_DROPS, p.cull_radius, r.prop)
            }
            None => {
                let so = &self.solids[s];
                let r = Vec3::from_array(so.bbox_max).distance(Vec3::from_array(so.bbox_min)) * 0.5 + 6.0;
                (None, None, 0, r, None)
            }
        };
        Some(AddResult {
            placement: Placement { solid: s, low_solid, mid_solid, transform: t, section, flags, cull_radius },
            template,
            source,
            prop,
        })
    }
}

impl World {
    /// The (section, solid) index of the originals, made from the world
    /// as it stands (once: it must not have a mod applied yet but by the
    /// map overlay's bookkeeping).
    pub fn index_originals(&mut self) {
        if self.map_overlay.index.finds.is_some() {
            return;
        }
        let n = self.map_overlay.first_added.unwrap_or(self.placements.len()).min(self.placements.len());
        let mut by_key: HashMap<(u32, usize), Vec<u32>> = HashMap::new();
        for (i, p) in self.placements[..n].iter().enumerate() {
            by_key.entry((p.section, p.solid)).or_default().push(i as u32);
        }
        let at = (0..n).map(|i| self.original_position(i).unwrap_or(Vec3::NAN)).collect();
        self.map_overlay.index.finds = Some(Arc::new(Finds { n, by_key, at }));
    }

    fn base_of(&self, n: usize) -> Base {
        let placements = self.placements[..n].to_vec();
        let smackables = self.smackables.clone();
        let mut prop_of = HashMap::new();
        for (k, p) in smackables.iter().enumerate() {
            prop_of.entry(p.placement).or_insert(k);
        }
        let mut links: HashMap<usize, Vec<u32>> = HashMap::new();
        for (slot, l) in self.link_placements.iter().enumerate() {
            if let Some(i) = l {
                links.entry(*i).or_default().push(slot as u32);
            }
        }
        let free = || placements.iter().enumerate().filter(|(_, p)| p.flags & GROUPED == 0);
        let mut by_solid: HashMap<usize, Vec<u32>> = HashMap::new();
        for (j, p) in free() {
            by_solid.entry(p.solid).or_default().push(j as u32);
        }
        let grid = Grid::of(free().map(|(j, p)| (j as u32, p.transform.w_axis.truncate())));
        Base { placements, smackables, prop_of, links, by_solid, grid }
    }

    /// Apply the one map mod `map` (kept in `folder`) to this world of
    /// `region` again after an edit of it, changing only what the edit
    /// changes: the world [`World::apply_map_mods`] makes of the original
    /// world with `map` (but for the loaded state
    /// [`World::reset_to_loaded`] restores, which stays the original's).
    /// The first call takes the world as it stands for the original (no
    /// mod applied yet); edits made to the world directly in between
    /// (`tools::map::edit`) are taken back where `map` does not make them.
    /// True when anything changed.
    pub fn reapply_map_mod(&mut self, region: &str, folder: &str, map: &MapMod) -> bool {
        self.index_originals();
        let finds = self.map_overlay.index.finds.clone().unwrap_or_else(|| unreachable!());
        let n = finds.n;
        let mut live = match self.map_overlay.index.live.take() {
            Some(l) if l.region == region => l,
            other => Box::new(Live {
                region: region.to_string(),
                base: other.map_or_else(|| Arc::new(self.base_of(n)), |l| l.base),
                found: HashMap::new(),
                serials: HashMap::new(),
                next_serial: 0,
                entries: Vec::new(),
                adds: Vec::new(),
                order: Vec::new(),
            }),
        };
        let base = live.base.clone();

        // The region's edits: the originals' moves and deletes, the adds.
        let edits: Vec<&PropEdit> = map.props.iter().filter(|e| e.region().eq_ignore_ascii_case(region)).collect();
        let mut entries = Vec::with_capacity(edits.len());
        let mut adds: Vec<AddState> = Vec::new();
        let mut occurrences: HashMap<&str, u32> = HashMap::new();
        let (mut prev, mut moved, mut deleted, mut missing) = (NONE, 0, 0, 0);
        for (k, e) in edits.iter().enumerate() {
            let edit = k as u32;
            match e {
                PropEdit::Add { id, solid, position, rotation, .. } => {
                    let o = occurrences.entry(id).or_insert(0);
                    let serial = live.serial(id, *o);
                    *o += 1;
                    let solid = self.solid_named(solid);
                    adds.push(AddState { serial, solid, position: *position, rotation: *rotation, edit, result: None });
                    prev = serial;
                }
                PropEdit::Move { target, position, rotation } => match live.found(self, target) {
                    Some(j) => {
                        let kind = Kind::Move(transform_of(*position, *rotation));
                        entries.push(Entry { j: j as u32, prev, kind, edit });
                        moved += 1;
                    }
                    None => missing += 1,
                },
                PropEdit::Delete { target } => match live.found(self, target) {
                    Some(j) => {
                        entries.push(Entry { j: j as u32, prev, kind: Kind::Delete, edit });
                        deleted += 1;
                    }
                    None => missing += 1,
                },
            }
        }
        entries.sort_by_key(|e| e.j);
        let mut order = vec![NONE; live.next_serial as usize];
        for (k, a) in adds.iter().enumerate() {
            order[a.serial as usize] = k as u32;
        }

        // The originals whose edits changed (or their place among the adds).
        let mut changed: Vec<u32> = Vec::new();
        {
            let (mut old, mut new) =
                (live.entries.chunk_by(|a, b| a.j == b.j).peekable(), entries.chunk_by(|a, b| a.j == b.j).peekable());
            loop {
                match (old.peek(), new.peek()) {
                    (None, None) => break,
                    (Some(o), Some(w)) if o[0].j == w[0].j => {
                        if o.len() != w.len() || o.iter().zip(w.iter()).any(|(a, b)| !a.same(b)) {
                            changed.push(o[0].j);
                        }
                        old.next();
                        new.next();
                    }
                    (Some(o), w) if w.is_none_or(|w| o[0].j < w[0].j) => {
                        changed.push(o[0].j);
                        old.next();
                    }
                    (_, Some(w)) => {
                        changed.push(w[0].j);
                        new.next();
                    }
                    (Some(_), None) => unreachable!(),
                }
            }
        }

        // The adds: each kept unless what it was made from changed.
        let old_at = |serial: u32| live.order.get(serial as usize).copied().filter(|&o| o != NONE);
        let mut in_order = true;
        let mut last = None;
        for a in &adds {
            if let Some(o) = old_at(a.serial) {
                in_order &= last.is_none_or(|l| o > l);
                last = Some(o);
            }
        }
        let removed: Vec<u32> = live.adds.iter().map(|a| a.serial).filter(|&s| order[s as usize] == NONE).collect();
        let mut cx = AddsCtx {
            base: &base,
            solids: &self.solids,
            entries: &entries,
            order: &order,
            adds: &adds,
            results: Vec::with_capacity(adds.len()),
            index_of: Vec::with_capacity(adds.len()),
        };
        let mut moved_adds: Vec<usize> = Vec::new();
        let mut next_index = n;
        for (k, a) in adds.iter().enumerate() {
            let old = old_at(a.serial).filter(|_| in_order).map(|o| (o, &live.adds[o as usize]));
            let kept = old.filter(|(_, o)| o.solid == a.solid && o.position == a.position).and_then(|(ko, o)| {
                let stale = o
                    .result
                    .as_ref()
                    .is_some_and(|r| stale(r, a.position, ko, k, &changed, &moved_adds, &removed, &live, &cx));
                (!stale).then(|| {
                    let mut r = o.result.clone();
                    if let Some(r) = &mut r {
                        r.placement.transform = transform_of(a.position, a.rotation);
                    }
                    r
                })
            });
            let r = match kept {
                Some(r) => r,
                None => cx.compute(k),
            };
            if old.is_none_or(|(_, o)| o.position != a.position || !same_for_others(&o.result, &r)) {
                moved_adds.push(k);
            }
            cx.index_of.push(if r.is_some() { next_index } else { usize::MAX });
            next_index += r.is_some() as usize;
            cx.results.push(r);
        }
        let AddsCtx { results, index_of, .. } = cx;

        // The world: the originals touched before or now ...
        let mut any = false;
        let mut lights = false;
        let mut touched: Vec<u32> = self.map_overlay.originals.keys().map(|&j| j as u32).collect();
        touched.extend(entries.iter().map(|e| e.j));
        touched.extend(&changed);
        touched.sort_unstable();
        touched.dedup();
        for j in touched {
            let i = j as usize;
            if i >= n || i >= self.placements.len() {
                continue;
            }
            let run = run_of(&entries, j);
            let (t, del) = as_of(run, NONE, &order);
            let b = &base.placements[i];
            let (t, flags) = (t.copied().unwrap_or(b.transform), if del { b.flags | HIDDEN_FLAG } else { b.flags });
            let p = &mut self.placements[i];
            if p.transform != t || p.flags != flags {
                (p.transform, p.flags) = (t, flags);
                self.changed.push(i);
                any = true;
            }
            for &slot in base.links.get(&i).into_iter().flatten() {
                let want = (!del).then_some(i);
                if let Some(l) = self.link_placements.get_mut(slot as usize)
                    && *l != want
                {
                    *l = want;
                    any = true;
                }
            }
            let o = &mut self.map_overlay;
            match run.first().and_then(|e| edits[e.edit as usize].target()) {
                Some(target) => {
                    if o.originals.get(&i) != Some(target) {
                        o.originals.insert(i, target.clone());
                    }
                    o.original_at.insert(i, b.transform.w_axis.truncate());
                }
                None => {
                    o.originals.remove(&i);
                    o.original_at.remove(&i);
                }
            }
        }
        // ... the adds ...
        let mut i = n;
        for r in results.iter().flatten() {
            match self.placements.get_mut(i) {
                Some(p) if same_placement(p, &r.placement) => {}
                Some(p) => {
                    lights |= (p.solid, p.mid_solid, p.section)
                        != (r.placement.solid, r.placement.mid_solid, r.placement.section);
                    *p = r.placement.clone();
                    self.changed.push(i);
                    any = true;
                }
                None => {
                    self.placements.push(r.placement.clone());
                    (any, lights) = (true, true);
                }
            }
            i += 1;
        }
        if self.placements.len() > i {
            self.placements.truncate(i);
            (any, lights) = (true, true);
        }
        let count = i - n;
        let o = &mut self.map_overlay;
        let named = |k: usize| edits[adds[k].edit as usize].add_id().unwrap_or_default();
        let same_added = o.added.len() == count
            && (0..adds.len()).all(|k| {
                index_of[k] == usize::MAX
                    || o.added.get(&index_of[k]).is_some_and(|(f, id)| f == folder && id == named(k))
            });
        if !same_added {
            o.added.clear();
            for k in (0..adds.len()).filter(|&k| index_of[k] != usize::MAX) {
                o.added.insert(index_of[k], (folder.to_string(), named(k).to_string()));
            }
            any = true;
        }
        o.first_added = (count > 0).then_some(n);
        missing += results.iter().filter(|r| r.is_none()).count();
        (o.moved, o.deleted, o.missing) = (moved, deleted, missing);

        // ... and the knock-over props: the originals' not deleted, then
        // the adds'.
        let deleted_now = |i: usize| as_of(run_of(&entries, i as u32), NONE, &order).1;
        let moved_to = |i: usize| as_of(run_of(&entries, i as u32), NONE, &order).0.copied();
        let layout: Vec<usize> = base
            .smackables
            .iter()
            .map(|p| p.placement)
            .filter(|&i| i >= n || !deleted_now(i))
            .chain(
                (0..adds.len()).filter(|&k| results[k].as_ref().is_some_and(|r| r.prop.is_some())).map(|k| index_of[k]),
            )
            .collect();
        let add_prop = |k: usize| {
            let r = results[k].as_ref().unwrap_or_else(|| unreachable!());
            let root = &base.smackables[r.prop.unwrap_or_else(|| unreachable!())];
            crate::smackable::Prop { placement: index_of[k], rest: r.placement.transform, ..root.clone() }
        };
        let with_prop: Vec<usize> =
            (0..adds.len()).filter(|&k| results[k].as_ref().is_some_and(|r| r.prop.is_some())).collect();
        let s = &mut self.smackables;
        if s.len() != layout.len() || s.iter().zip(&layout).any(|(p, &i)| p.placement != i) {
            let mut fresh: Vec<crate::smackable::Prop> = base
                .smackables
                .iter()
                .filter(|p| p.placement >= n || !deleted_now(p.placement))
                .map(|p| crate::smackable::Prop { rest: moved_to(p.placement).unwrap_or(p.rest), ..p.clone() })
                .collect();
            fresh.extend(with_prop.iter().map(|&k| add_prop(k)));
            *s = fresh;
            (any, lights) = (true, true);
        } else {
            let originals = s.len() - with_prop.len();
            let kept_base = base.smackables.iter().filter(|p| p.placement >= n || !deleted_now(p.placement));
            for (p, b) in s[..originals].iter_mut().zip(kept_base) {
                let rest = moved_to(b.placement).unwrap_or(b.rest);
                if p.rest != rest {
                    p.rest = rest;
                    any = true;
                }
            }
            for (p, &k) in s[originals..].iter_mut().zip(&with_prop) {
                let a = &adds[k];
                let same_root = old_at(a.serial).filter(|_| in_order).is_some_and(|o| {
                    live.adds[o as usize].result.as_ref().and_then(|r| r.prop)
                        == results[k].as_ref().and_then(|r| r.prop)
                });
                if !same_root {
                    *p = add_prop(k);
                    any = true;
                } else if let Some(r) = &results[k]
                    && p.rest != r.placement.transform
                {
                    p.rest = r.placement.transform;
                    any = true;
                }
            }
        }
        if lights {
            self.light_placements = Default::default();
            self.light_by_section = Default::default();
        }

        for (a, r) in adds.iter_mut().zip(results) {
            a.result = r;
        }
        (live.entries, live.adds, live.order) = (entries, adds, order);
        self.map_overlay.index.live = Some(live);
        any
    }
}

/// Whether add `r` (at `at`, the `ko`-th add before and the `k`-th now)
/// may come out differently now: what it took after changed or went, or
/// a placement that changed (an original in `changed`, an add in `moved`)
/// is now nearer.
#[allow(clippy::too_many_arguments)]
fn stale(
    r: &AddResult,
    at: Vec3,
    ko: u32,
    k: usize,
    changed: &[u32],
    moved: &[usize],
    removed: &[u32],
    live: &Live,
    cx: &AddsCtx,
) -> bool {
    let is = |n: &Option<Near>, pid: Pid| n.is_some_and(|x| x.pid == pid);
    let took = |pid: Pid| is(&r.template, pid) || is(&r.source, pid);
    if removed.iter().any(|&s| took(Pid::Add(s))) {
        return true;
    }
    let beats = |d2: f32, idx: usize, n: &Option<Near>| match n {
        None => true,
        Some(x) => d2.total_cmp(&x.d2).then(idx.cmp(&cx.idx_of(x.pid))).is_lt(),
    };
    let solid = r.placement.solid;
    for &j in changed {
        let before = as_of(run_of(&live.entries, j), ko, &live.order);
        let now = as_of(run_of(cx.entries, j), k as u32, cx.order);
        if before == now {
            continue;
        }
        if took(Pid::Orig(j)) {
            return true;
        }
        let b = &cx.base.placements[j as usize];
        if b.flags & GROUPED != 0 {
            continue;
        }
        let d2 = now.0.map_or(b.transform.w_axis.truncate(), |t| t.w_axis.truncate()).distance_squared(at);
        if (b.solid == solid && beats(d2, j as usize, &r.template)) || beats(d2, j as usize, &r.source) {
            return true;
        }
    }
    for &m in moved {
        if took(Pid::Add(cx.adds[m].serial)) {
            return true;
        }
        if let Some(q) = &cx.results[m] {
            let d2 = q.placement.transform.w_axis.truncate().distance_squared(at);
            let idx = cx.index_of[m];
            if (q.placement.solid == solid && beats(d2, idx, &r.template)) || beats(d2, idx, &r.source) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod bench {
    use glam::{Mat3, Vec3};

    use crate::tools::map::ops::{Op, PropKey};
    use crate::tools::map::session::MapSession;
    use crate::tools::map::{MapMod, PropEdit};
    use crate::world::World;

    /// A mod of `edits` edits on `w` (70 % moves, 10 % deletes, 20 % adds),
    /// spread over the region.
    pub(super) fn mod_of(w: &World, region: &str, edits: usize) -> MapMod {
        let mut m = MapMod::default();
        let n = w.placements.len();
        let stride = (n / (edits + 1)).max(1);
        for k in 0..edits {
            let i = (k * stride + 7) % n;
            let p = &w.placements[i];
            let (at, rot) = super::parts_of(p.transform);
            match k % 10 {
                0..=6 => {
                    let target = w.placement_ref(region, i).unwrap();
                    m.props.push(PropEdit::Move { target, position: at + Vec3::new(1.0, 0.5, 0.0), rotation: rot });
                }
                7 => m.props.push(PropEdit::Delete { target: w.placement_ref(region, i).unwrap() }),
                _ => m.props.push(PropEdit::Add {
                    id: m.fresh_add_id(),
                    region: region.into(),
                    solid: w.solids[p.solid].name.clone(),
                    position: at + Vec3::new(3.0, 0.0, 0.0),
                    rotation: rot,
                }),
            }
        }
        m
    }

    fn median(mut v: Vec<f64>) -> (f64, f64) {
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() * 9 / 10])
    }

    /// Times `f` `reps` times (ms): median and 90th percentile.
    fn time(reps: usize, mut f: impl FnMut(usize)) -> (f64, f64) {
        f(0);
        median(
            (1..=reps)
                .map(|k| {
                    let t = std::time::Instant::now();
                    f(k);
                    t.elapsed().as_secs_f64() * 1e3
                })
                .collect(),
        )
    }

    /// One drag step (one `Op::Place`) on a mod of 10 and of 2,000 edits on
    /// Bayview: run with `--ignored --nocapture` (`NFSU2_ROOT` set).
    #[test]
    #[ignore = "bench: needs NFSU2_ROOT; run with --ignored --nocapture"]
    fn bench_one_gesture() {
        let root = crate::testkit::game_root();
        let load = || World::load(std::path::Path::new(&root), "L4RA").unwrap();
        let world = load();
        let reps: usize = std::env::var("BENCH_REPS").ok().and_then(|r| r.parse().ok()).unwrap_or(101);
        println!("L4RA: {} placements, {} knock-over props", world.placements.len(), world.smackables.len());
        for edits in [10, 2000] {
            let map = mod_of(&world, "L4RA", edits);
            let mut s = MapSession::new("L4RA", "bench", map, load());
            // An original already moved (its edit in the middle of the list).
            let moved =
                s.map.props.iter().filter_map(PropEdit::target).nth(edits / 3).cloned().map(PropKey::Original).unwrap();
            let i = s.placement_of(&moved).unwrap();
            let (at, rot) = super::parts_of(s.world.placements[i].transform);
            let (m1, p1) = time(reps, |k| {
                let position = at + Vec3::new(0.01 * k as f32, 0.0, 0.0);
                s.run(Op::Place { key: moved.clone(), position, rotation: rot }).unwrap();
            });
            // An untouched original (a fresh drag's first step).
            let fresh_from = (0..s.world.placements.len())
                .find(|&j| j % 13 == 5 && !s.world.map_overlay.originals.contains_key(&j))
                .unwrap();
            let mut next = fresh_from;
            let (m2, p2) = time(reps.min(31), |_| {
                while s.world.map_overlay.originals.contains_key(&next) || s.world.map_overlay.added.contains_key(&next)
                {
                    next += 1;
                }
                let key = s.key_of(next).unwrap();
                let (at, rot) = super::parts_of(s.world.placements[next].transform);
                s.run(Op::Place { key, position: at + Vec3::Z, rotation: rot }).unwrap();
            });
            // An added prop (its add in the middle of the list).
            let id = s.map.props.iter().filter_map(PropEdit::add_id).nth(edits / 20).unwrap().to_string();
            let key = PropKey::Added(id);
            let j = s.placement_of(&key).unwrap();
            let (at, _) = super::parts_of(s.world.placements[j].transform);
            let (m3, p3) = time(reps, |k| {
                let position = at + Vec3::new(0.0, 0.01 * k as f32, 0.0);
                s.run(Op::Place { key: key.clone(), position, rotation: Mat3::IDENTITY }).unwrap();
            });
            println!(
                "bench {edits} edits: move a moved original {m1:.3} ms (p90 {p1:.3}); \
                 first move of an original {m2:.3} ms (p90 {p2:.3}); move an added prop {m3:.3} ms (p90 {p3:.3})"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat3, Mat4, Vec3};

    use super::*;
    use crate::tools::map::ops::{Op, PropKey};
    use crate::tools::map::session::MapSession;

    const REGION: &str = "L4RA";
    const FOLDER: &str = "my-map";
    const SOLIDS: [&str; 5] = ["XC_CONE_01", "XC_BARRIER_03", "XC_LAMP_02", "XC_TREE_07", "XC_NOWHERE_01"];

    pub(super) struct Rng(pub u64);

    impl Rng {
        pub fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        pub fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
        pub fn f(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
        }
        pub fn one_in(&mut self, n: usize) -> bool {
            self.below(n) == 0
        }
    }

    fn solid(name: &str) -> Solid {
        Solid {
            name: name.into(),
            hash: crate::formats::hash::bin_hash(name),
            bbox_min: [-0.5, -0.5, 0.0],
            bbox_max: [0.5, 0.5, 1.0],
            textures: Vec::new(),
            vertices: Vec::new(),
            normals: Vec::new(),
            light_materials: Vec::new(),
            indices: Vec::new(),
            materials: Vec::new(),
            mesh_flags: 0,
            markers: Vec::new(),
            light_markers: Vec::new(),
        }
    }

    fn prop(placement: usize, rest: Mat4, key: u32) -> crate::smackable::Prop {
        crate::smackable::Prop {
            placement,
            scenery: key as u16,
            info: crate::formats::smackable::ObjectInfo {
                name: format!("ZCS_Prop{key}"),
                key,
                mass: 0.1 * key as f32,
                inertia: Mat3::IDENTITY,
                centre_of_mass: Vec3::Z * 0.5,
                break_impulse: 1.0,
                restitution: 0.0,
                friction: 0.5,
                material: key as usize % 19,
            },
            shape: vec![Vec3::X, Vec3::Y * key as f32],
            offset: Vec3::ZERO,
            raw_shape: vec![Vec3::Z],
            links: Default::default(),
            reach: 1.0,
            above: 1.0,
            below: 0.5,
            rest,
            world_min: Vec3::splat(-1.0),
            world_max: Vec3::splat(1.0),
            radius: 1.0,
        }
    }

    /// A region of 300 placements of four solids over 300 m, in three
    /// sections: twins (several of one solid exactly at one place), some
    /// hidden as loaded, some with flag 0x0100, knock-over props (one
    /// placement with two) and scenery links.
    fn world() -> World {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15);
        let mut w = World::flat(400.0);
        w.solids = SOLIDS.iter().map(|s| solid(s)).collect();
        let mut placements = Vec::new();
        while placements.len() < 300 {
            let at = Vec3::new(r.f(-150.0, 150.0), r.f(-150.0, 150.0), r.f(0.0, 3.0));
            let mut flags = 0u16;
            if r.one_in(8) {
                flags |= GROUPED;
            }
            if r.one_in(10) {
                flags |= HIDDEN_FLAG;
            }
            if r.one_in(5) {
                flags |= 0x0004;
            }
            let p = Placement {
                solid: r.below(4),
                low_solid: r.one_in(2).then(|| r.below(4)),
                mid_solid: r.one_in(3).then(|| r.below(4)),
                transform: Mat4::from_rotation_z(r.f(0.0, 6.0))
                    * Mat4::from_scale(Vec3::splat(r.f(0.8, 1.2)))
                    * Mat4::IDENTITY,
                section: [618, 619, 702][r.below(3)],
                flags,
                cull_radius: r.f(4.0, 40.0),
            };
            let p = Placement { transform: Mat4::from_translation(at) * p.transform, ..p };
            // Twins: two to four of one solid at exactly one place.
            let twins = if r.one_in(12) { 2 + r.below(3) } else { 1 };
            for _ in 0..twins {
                placements.push(p.clone());
            }
        }
        w.placements = placements;
        let n = w.placements.len();
        let mut props = Vec::new();
        for i in 0..n {
            if r.one_in(5) {
                props.push(prop(i, w.placements[i].transform, i as u32));
            }
        }
        props.insert(3, prop(props[2].placement, Mat4::IDENTITY, 999));
        w.smackables = props;
        w.link_placements = (0..80).map(|_| (!r.one_in(4)).then(|| r.below(n))).collect();
        w
    }

    /// The world a full apply of `map` makes.
    fn full(map: &MapMod) -> World {
        let mut w = world();
        w.apply_map_mods(REGION, &[(FOLDER.to_string(), map.clone())]);
        w
    }

    fn assert_same(a: &World, b: &World, what: &str) {
        assert_eq!(a.placements.len(), b.placements.len(), "{what}: placements");
        for (i, (p, q)) in a.placements.iter().zip(&b.placements).enumerate() {
            assert!(same_placement(p, q), "{what}: placement {i}: {p:?} / {q:?}");
        }
        assert_eq!(format!("{:?}", a.smackables), format!("{:?}", b.smackables), "{what}: knock-over props");
        assert_eq!(a.link_placements, b.link_placements, "{what}: links");
        let (o, p) = (&a.map_overlay, &b.map_overlay);
        assert_eq!(o.originals, p.originals, "{what}: originals");
        assert_eq!(o.original_at, p.original_at, "{what}: original_at");
        assert_eq!(o.added, p.added, "{what}: added");
        assert_eq!(o.first_added, p.first_added, "{what}: first_added");
        assert_eq!((o.moved, o.deleted, o.missing), (p.moved, p.deleted, p.missing), "{what}: counts");
    }

    fn rotation(r: &mut Rng) -> Mat3 {
        match r.below(3) {
            0 => Mat3::IDENTITY,
            _ => Mat3::from_rotation_z(r.f(-3.0, 3.0)),
        }
    }

    /// Somewhere: near an existing placement (often the same solid's, to
    /// change which one an add takes after), or anywhere.
    fn spot(s: &MapSession, r: &mut Rng) -> Vec3 {
        if r.one_in(4) {
            return Vec3::new(r.f(-170.0, 170.0), r.f(-170.0, 170.0), r.f(0.0, 2.0));
        }
        let p = &s.world.placements[r.below(s.world.placements.len())];
        p.transform.w_axis.truncate() + Vec3::new(r.f(-3.0, 3.0), r.f(-3.0, 3.0), 0.0)
    }

    fn some_key(s: &MapSession, r: &mut Rng) -> Option<PropKey> {
        let adds: Vec<&str> = s.map.props.iter().filter_map(PropEdit::add_id).collect();
        if !adds.is_empty() && r.one_in(3) {
            return Some(PropKey::Added(adds[r.below(adds.len())].to_string()));
        }
        // Mostly an original the mod already edits (twins among them),
        // sorted: the map's own order differs from run to run, and the
        // ops must not.
        let mut edited: Vec<usize> = s.world.map_overlay.originals.keys().copied().collect();
        edited.sort_unstable();
        let i = if !edited.is_empty() && r.one_in(2) {
            edited[r.below(edited.len())]
        } else {
            r.below(s.base.placements.len())
        };
        s.key_of(i)
    }

    fn random_op(s: &MapSession, r: &mut Rng) -> Option<Op> {
        Some(match r.below(12) {
            0..=3 => {
                let key = some_key(s, r)?;
                Op::Place { key, position: spot(s, r), rotation: rotation(r) }
            }
            4 => Op::Delete { key: some_key(s, r)? },
            5 => Op::Reset { key: some_key(s, r)? },
            6 => {
                let id = match r.below(4) {
                    0 => Some(format!("x{}", r.below(40))),
                    1 => s.map.props.iter().filter_map(PropEdit::add_id).next().map(str::to_string),
                    _ => None,
                };
                Op::Add {
                    solid: SOLIDS[r.below(SOLIDS.len())].to_string(),
                    position: spot(s, r),
                    rotation: rotation(r),
                    id,
                    ground: r.one_in(4),
                }
            }
            7 => Op::Duplicate { key: some_key(s, r)?, position: spot(s, r) },
            8 => Op::DropToGround { key: some_key(s, r)? },
            9 => {
                // A group, sometimes with an op that is refused.
                let mut ops = Vec::new();
                for _ in 0..2 + r.below(2) {
                    ops.push(random_op(s, r)?);
                }
                if r.one_in(3) {
                    ops.push(Op::Delete { key: PropKey::Added("no-such-add".into()) });
                }
                Op::Group { label: "Group".into(), ops }
            }
            _ => {
                // The mod put in place whole: its edits in another order,
                // an add twice under one id now and then.
                let mut map = s.map.clone();
                let n = map.props.len();
                for k in (1..n).rev() {
                    if r.one_in(2) {
                        map.props.swap(k, r.below(k + 1));
                    }
                }
                if r.one_in(3)
                    && let Some(a) = map.props.iter().find(|e| matches!(e, PropEdit::Add { .. })).cloned()
                {
                    map.props.push(a);
                }
                Op::PutMod { map }
            }
        })
    }

    /// Random op sequences (moves, deletes, resets, adds keyed by id,
    /// duplicates, groups, undo and redo, whole mods in another order) on a
    /// region with twins: after every step the live re-apply's world is the
    /// one a full apply of the mod makes.
    #[test]
    fn the_live_apply_is_a_full_apply() {
        for seed in 1..=12u64 {
            let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
            let mut s = MapSession::new(REGION, FOLDER, MapMod::default(), world());
            for step in 0..150 {
                let what = match r.below(10) {
                    0 => format!("undo {:?}", s.undo()),
                    1 => format!("redo {:?}", s.redo()),
                    _ => match random_op(&s, &mut r) {
                        Some(op) => {
                            let label = op.label(&s.map);
                            format!("{label}: {:?}", s.run(op).map(|c| c.changed))
                        }
                        None => continue,
                    },
                };
                assert_same(&s.world, &full(&s.map), &format!("seed {seed} step {step} ({what})"));
            }
            assert!(s.map.props.len() > 10, "seed {seed}: {} edits", s.map.props.len());
        }
    }

    /// An original deleted and put back at the place its move already
    /// gives it, in one group (the mod the same as before): its knock-over
    /// prop comes back too, whether the group is made or refused.
    #[test]
    fn a_delete_taken_back_in_one_group_keeps_the_knock_over_prop() {
        for refused in [false, true] {
            let mut s = MapSession::new(REGION, FOLDER, MapMod::default(), world());
            let i = s.world.smackables[0].placement;
            let key = s.key_of(i).unwrap();
            let (position, rotation) = (Vec3::new(24.5, 29.5, 0.0), Mat3::IDENTITY);
            s.run(Op::Place { key: key.clone(), position, rotation }).unwrap();
            let props = s.world.smackables.len();
            let mut ops = vec![Op::Delete { key: key.clone() }, Op::Place { key, position, rotation }];
            if refused {
                ops.push(Op::Delete { key: PropKey::Added("no-such-add".into()) });
            }
            let r = s.run(Op::Group { label: "Group".into(), ops });
            assert_eq!(r.is_err(), refused);
            assert_eq!(s.world.smackables.len(), props, "refused {refused}");
            assert_same(&s.world, &full(&s.map), &format!("refused {refused}"));
        }
    }

    /// On Bayview with a mod of 2,000 edits: after random gestures the
    /// live re-apply's world is the full apply's.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_live_apply_is_a_full_apply_on_bayview() {
        let root = crate::testkit::game_root();
        let load = || World::load(std::path::Path::new(&root), REGION).unwrap();
        let w = load();
        let map = super::bench::mod_of(&w, REGION, 2000);
        let mut s = MapSession::new(REGION, FOLDER, map, w);
        let mut r = Rng(4242);
        for _ in 0..60 {
            if let Some(op) = random_op(&s, &mut r).filter(|o| !matches!(o, Op::PutMod { .. })) {
                let _ = s.run(op);
            }
            if r.one_in(8) {
                s.undo();
            }
        }
        let mut again = load();
        again.apply_map_mods(REGION, &[(FOLDER.to_string(), s.map.clone())]);
        assert_same(&s.world, &again, "Bayview");
    }

    /// The index names and finds every original as looking through all
    /// placements does, twins and names a little off included.
    #[test]
    fn the_index_finds_what_the_scan_finds() {
        let scan = world();
        let mut indexed = world();
        indexed.index_originals();
        let mut r = Rng(77);
        for i in 0..scan.placements.len() {
            let a = scan.placement_ref(REGION, i);
            assert_eq!(indexed.placement_ref(REGION, i), a, "placement {i}");
            let mut q = a.unwrap();
            assert_eq!(indexed.find_placement(&q), Some(i));
            q.at += Vec3::new(r.f(-0.06, 0.06), r.f(-0.06, 0.06), 0.0);
            q.nth = r.below(3) as u32;
            let solid = scan.solid_named(&q.solid).unwrap();
            assert_eq!(indexed.find_placement(&q), scan.find_placement_scan(&q, solid), "{q:?}");
        }
    }
}
