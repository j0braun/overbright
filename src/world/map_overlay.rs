//! Map mods' scenery edits on a loaded world (`tools::map`, docs/map-editor.md).
//! Applied after [`World::load`] (so the asset cache keeps the original
//! world) and before the world's renderer is built (so added placements
//! are drawn); the world's loaded state is then taken again, so a kept
//! world's [`World::reset_to_loaded`] comes back to the world with its
//! mods. With no edit for the region nothing is touched.

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

    /// The original placement `r` names (nearest within
    /// [`MATCH_DISTANCE`]; an added placement never).
    pub fn find_placement(&self, r: &PropRef) -> Option<usize> {
        let solid = self.solid_named(&r.solid)?;
        let first_added = self.map_overlay.first_added.unwrap_or(self.placements.len());
        self.placements[..first_added]
            .iter()
            .enumerate()
            .filter(|(_, p)| p.section == r.section && p.solid == solid)
            .map(|(i, p)| (i, p.transform.w_axis.truncate().distance(r.at)))
            .filter(|&(_, d)| d <= MATCH_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// How an original placement is named in a map mod: as the original
    /// data has it (its place before any mod moved it).
    pub fn placement_ref(&self, region: &str, i: usize) -> Option<PropRef> {
        if let Some(r) = self.map_overlay.originals.get(&i) {
            return Some(r.clone());
        }
        if self.map_overlay.first_added.is_some_and(|f| i >= f) {
            return None;
        }
        let p = self.placements.get(i)?;
        Some(PropRef {
            region: region.to_string(),
            section: p.section,
            solid: self.solids.get(p.solid)?.name.clone(),
            at: p.transform.w_axis.truncate(),
        })
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
        let mut applied = std::mem::take(&mut self.map_overlay);
        for (m, k, e) in edits {
            let folder = &mods[m].0;
            match e {
                PropEdit::Move { target, position, rotation } => match self.find_placement(target) {
                    Some(i) => {
                        applied.originals.entry(i).or_insert_with(|| target.clone());
                        self.set_placement_transform(i, transform_of(*position, *rotation));
                        applied.moved += 1;
                    }
                    None => applied.missing += 1,
                },
                PropEdit::Delete { target } => match self.find_placement(target) {
                    Some(i) => {
                        applied.originals.entry(i).or_insert_with(|| target.clone());
                        self.hide_placement(i);
                        applied.deleted += 1;
                    }
                    None => applied.missing += 1,
                },
                PropEdit::Add { solid, position, rotation, .. } => {
                    match self.add_placement(solid, transform_of(*position, *rotation)) {
                        Some(i) => {
                            applied.first_added.get_or_insert(i);
                            applied.added.insert(i, (folder.clone(), k));
                        }
                        None => applied.missing += 1,
                    }
                }
            }
        }
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
        self.map_overlay = applied;
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
