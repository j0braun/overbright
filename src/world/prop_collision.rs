//! Static collision for map mods' scenery (docs/modding/map-editor.md
//! "Collision of edited scenery"). The original bakes the collision of its
//! static scenery into the stream sections' collision packs, with nothing
//! tying a face to the placement it came from. A solid's own collision is
//! found the way the packs hold it: the walls standing inside one of its
//! original placements' boxes, taken into the solid's space. An added
//! placement of a solid gets that shape at its place, a moved original
//! carries its own faces to its new place, and a deleted or moved one
//! leaves none behind. Knock-over props are not touched here: their
//! collision is their kind's volume (`smackable`).
//!
//! New faces go into every pack whose grid covers them, the way the
//! original's packs list a face on a section edge in each section's pack,
//! with cells and sub-cell masks by the rule the shipped packs follow
//! (`cells_of`).

use std::collections::{HashMap, HashSet};

use glam::{DVec2, Mat4, Vec2, Vec3};

use super::*;
use crate::formats::collision::{CollisionFace, FLAG_QUAD, FLAG_TALL, FLAG_WALL};

/// How far (metres, in the solid's space) a face's corner may stand outside
/// the solid's box and still be the solid's own.
pub const MARGIN: f32 = 0.25;
/// Face flag 0x08: not ground for the wheels (docs/modding/asset-format/collision.md).
const FLAG_NOT_GROUND: u8 = 0x08;
/// The most original placements looked at for a solid's shape.
const SAMPLES: usize = 64;
/// A grid cell, and its sub-cells, metres.
const CELL: f64 = 32.0;
const SUB: f64 = 8.0;

/// A face of a solid's collision in the solid's own space.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalFace {
    pub corners: [Vec3; 4],
    pub quad: bool,
    pub surface: u8,
    pub flags: u8,
}

/// What the scenery collision of a world's map mods did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropCollision {
    /// Faces taken out of packs (one per pack that held it).
    pub cut: usize,
    /// Faces written into packs (one per pack that holds it).
    pub added: usize,
    /// Edited placements whose solid has static collision.
    pub solids_with_collision: usize,
    /// Faces outside every pack's grid (not written).
    pub outside: usize,
    /// Removed faces the original's collision does not have.
    pub unmatched: usize,
    /// Where those are (each one's corners' centre), in the mods' order: the map checks point at them.
    pub unmatched_at: Vec<Vec3>,
    /// The drawn faces outside every pack's grid (each one's corners' centre), in the mods' order.
    pub outside_at: Vec<Vec3>,
}

/// A map mod's removed and drawn collision on one region
/// (`format::CollisionEdit`, docs/modding/map-editor.md "Removed and drawn
/// collision"): the corners of the original faces taken out, and the drawn
/// faces, in the mods' order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawnCollision {
    pub remove: Vec<Vec<Vec3>>,
    pub faces: Vec<crate::tools::map::draw::DrawnFace>,
}

impl DrawnCollision {
    /// The collision edits of `region` among `edits`, in order.
    pub fn of<'a>(region: &str, edits: impl IntoIterator<Item = &'a crate::tools::map::CollisionEdit>) -> DrawnCollision {
        use crate::tools::map::CollisionEdit;
        let mut d = DrawnCollision::default();
        for e in edits.into_iter().filter(|e| e.region().eq_ignore_ascii_case(region)) {
            match e {
                CollisionEdit::Remove { corners, .. } => d.remove.push(corners.clone()),
                _ => d.faces.extend(crate::tools::map::draw::faces(e)),
            }
        }
        d
    }

    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.faces.is_empty()
    }
}

/// The centre of a face's corners.
fn centre(corners: &[Vec3]) -> Vec3 {
    corners.iter().copied().sum::<Vec3>() / corners.len().max(1) as f32
}

/// A face's corners as the packs store them (eighths of a metre across,
/// 1/256 m up): how a removal names an original face.
pub fn face_key(corners: &[Vec3]) -> Vec<[i32; 3]> {
    corners.iter().map(|v| [(v.x * 8.0).round() as i32, (v.y * 8.0).round() as i32, (v.z * 256.0).round() as i32]).collect()
}

/// Drawn face `f` as a local face at the identity: a wall (flag 0x02) or,
/// for ground, a floor that competes by height with the others (0x01) and
/// so does not take the ground from a road above or below it.
fn drawn_local(f: &crate::tools::map::draw::DrawnFace) -> LocalFace {
    LocalFace { corners: f.corners, quad: f.quad, surface: f.surface, flags: if f.wall { FLAG_WALL } else { FLAG_LEVELS } }
}

/// Face flag 0x01: a floor on one of several levels.
const FLAG_LEVELS: u8 = 0x01;

/// One edited original or added placement whose static collision changes:
/// where its faces stand in the original (`from`, an original placement),
/// and where they stand now (`to`; None for deleted).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneryEdit {
    pub placement: usize,
    pub from: Option<Mat4>,
    pub to: Option<Mat4>,
}

/// What the editor's live re-apply (`World::reapply_map_mod`) needs to take
/// its scenery collision back to the original's: the edits it applied, the
/// packs as they were before (grid, face count) and the collision
/// triangles as they were. Empty while no edit changed the collision.
#[derive(Clone, Default)]
pub(super) struct SavedCollision {
    edits: Vec<SceneryEdit>,
    drawn: DrawnCollision,
    packs: Vec<(usize, (crate::formats::collision::CollisionGrid, usize))>,
    faces: Option<Vec<crate::collision::Source>>,
    /// Shapes found on the original's collision, which no edit changes: an
    /// original placement's own faces (pack, face) and in its space, and a
    /// solid's shape for added placements.
    cache: ShapeCache,
}

#[derive(Clone, Default)]
struct ShapeCache {
    own: HashMap<usize, (Vec<(usize, usize)>, Vec<LocalFace>)>,
    solids: HashMap<usize, Vec<LocalFace>>,
}

impl World {
    /// The faces `(pack, face)` of the original collision that are placement
    /// `solid` at `t`'s own: walls and faces that are not ground, in no
    /// barrier group and not reaching ±2048 m, with every corner inside the
    /// solid's box (± [`MARGIN`]). A face held by several packs comes once
    /// per pack.
    pub fn collision_of_placement(&self, solid: usize, t: Mat4) -> Vec<(usize, usize)> {
        let Some(so) = self.solids.get(solid) else { return Vec::new() };
        let (bmin, bmax) = (Vec3::from_array(so.bbox_min) - MARGIN, Vec3::from_array(so.bbox_max) + MARGIN);
        if !(bmin.cmple(bmax).all()) {
            return Vec::new();
        }
        let inv = t.inverse();
        // The box's footprint in the world.
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { bmin.x } else { bmax.x },
                if k & 2 == 0 { bmin.y } else { bmax.y },
                if k & 4 == 0 { bmin.z } else { bmax.z },
            );
            let w = t.transform_point3(c).truncate();
            lo = lo.min(w);
            hi = hi.max(w);
        }
        let mut out = Vec::new();
        for (pi, g) in self.ground.grids().enumerate() {
            let (ox, oy) = (g.origin[0], g.origin[1]);
            let (w, h) = (g.width as i32, g.height as i32);
            let cx0 = (((lo.x - ox) / 32.0).floor() as i32).max(0);
            let cy0 = (((lo.y - oy) / 32.0).floor() as i32).max(0);
            let cx1 = (((hi.x - ox) / 32.0).floor() as i32).min(w - 1);
            let cy1 = (((hi.y - oy) / 32.0).floor() as i32).min(h - 1);
            let mut seen = HashSet::new();
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let Some(&(first, count)) = g.cells.get((cy * w + cx) as usize) else { continue };
                    for &(face, _) in g.refs.get(first as usize..first as usize + count as usize).unwrap_or(&[]) {
                        if !seen.insert(face) {
                            continue;
                        }
                        let Some(f) = self.ground.face_at(pi, face as usize) else { continue };
                        if f.group != 0 || f.flags & FLAG_TALL != 0 || f.flags & (FLAG_WALL | FLAG_NOT_GROUND) == 0 {
                            continue;
                        }
                        let inside = f.vertices[..f.corners].iter().all(|&v| {
                            let l = inv.transform_point3(v);
                            l.cmpge(bmin).all() && l.cmple(bmax).all()
                        });
                        if inside {
                            out.push((pi, face as usize));
                        }
                    }
                }
            }
        }
        out
    }

    /// The faces of placement `solid` at `t` in the solid's space, each once
    /// (a face several packs hold is one face).
    fn local_faces(&self, solid: usize, t: Mat4) -> Vec<LocalFace> {
        let inv = t.inverse();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (pi, fi) in self.collision_of_placement(solid, t) {
            let Some(f) = self.ground.face_at(pi, fi) else { continue };
            let key = (f.x, f.y, f.vertices.map(|v| v.z.to_bits()), f.flags, f.surface);
            if !seen.insert(key) {
                continue;
            }
            out.push(LocalFace {
                corners: f.vertices.map(|v| inv.transform_point3(v)),
                quad: f.corners == 4,
                surface: f.surface,
                flags: f.flags,
            });
        }
        out
    }

    /// Solid `solid`'s own static collision, from its original placements:
    /// the shape most of those that have one share (by face count; the
    /// larger count on a tie, then the first in the original's order).
    /// Empty for a solid no original placement has collision for.
    pub fn solid_collision(&self, solid: usize) -> Vec<LocalFace> {
        let last = self.map_overlay.first_added.unwrap_or(self.placements.len());
        let mut by_count: HashMap<usize, (usize, Vec<LocalFace>)> = HashMap::new();
        let originals = (0..last)
            .filter(|&i| self.placements[i].solid == solid && !self.smackables.iter().any(|s| s.placement == i))
            .take(SAMPLES);
        for i in originals {
            let t = self.original_transform(i);
            let faces = self.local_faces(solid, t);
            if faces.is_empty() {
                continue;
            }
            let e = by_count.entry(faces.len()).or_insert((0, faces));
            e.0 += 1;
        }
        by_count.into_iter().max_by(|a, b| (a.1.0, a.0).cmp(&(b.1.0, b.0))).map(|(_, (_, f))| f).unwrap_or_default()
    }

    /// Where original placement `i` stood in the original data.
    fn original_transform(&self, i: usize) -> Mat4 {
        self.map_overlay.original_transform.get(&i).copied().unwrap_or(self.placements[i].transform)
    }

    /// Change the static collision for `edits`: the faces of each edited
    /// original at its original place leave every pack, and each edited
    /// placement's faces (an original's own, an added one's its solid's)
    /// are written at its place now. Nothing is touched for an empty list.
    pub fn apply_scenery_collision(&mut self, edits: &[SceneryEdit]) -> PropCollision {
        self.scenery_collision(edits, &DrawnCollision::default(), None, &mut ShapeCache::default())
    }

    /// [`World::apply_scenery_collision`] with a mod's removed and drawn
    /// collision: the removed original faces leave every pack with the
    /// edited scenery's, and the drawn faces are written after its new
    /// ones.
    pub fn apply_map_collision(&mut self, edits: &[SceneryEdit], drawn: &DrawnCollision) -> PropCollision {
        self.scenery_collision(edits, drawn, None, &mut ShapeCache::default())
    }

    /// The live re-apply's [`World::apply_scenery_collision`]: when `edits`
    /// differ from those `saved` holds, the original's collision is put back
    /// and `edits` are applied to it, so the world's collision is the one a
    /// load with the mod makes (the same faces in the same order). Nothing
    /// is touched while the edits stay the same, nor for no edits on the
    /// original's collision. True when the collision changed.
    pub(super) fn redo_scenery_collision(
        &mut self,
        saved: &mut SavedCollision,
        edits: &[SceneryEdit],
        drawn: &DrawnCollision,
    ) -> bool {
        if saved.edits == edits && saved.drawn == *drawn {
            return false;
        }
        for (pi, state) in saved.packs.drain(..).rev() {
            self.ground.restore_pack(pi, state);
        }
        if let Some(faces) = saved.faces.take() {
            self.collision_faces = faces;
            self.collision = Default::default();
        }
        saved.edits = edits.to_vec();
        saved.drawn = drawn.clone();
        self.map_overlay.collision_version += 1;
        if !(edits.is_empty() && drawn.is_empty()) && !self.ground.is_empty() {
            saved.faces = Some(self.collision_faces.clone());
            self.map_overlay.collision = self.scenery_collision(edits, drawn, Some(&mut saved.packs), &mut saved.cache);
        } else {
            self.map_overlay.collision = PropCollision::default();
        }
        true
    }

    fn scenery_collision(
        &mut self,
        edits: &[SceneryEdit],
        drawn: &DrawnCollision,
        mut save: Option<&mut Vec<(usize, (crate::formats::collision::CollisionGrid, usize))>>,
        cache: &mut ShapeCache,
    ) -> PropCollision {
        let mut done = PropCollision::default();
        if (edits.is_empty() && drawn.is_empty()) || self.ground.is_empty() {
            return done;
        }
        let _t = crate::prof::span("World: map mods' collision");
        let mut cut: HashMap<usize, HashSet<u16>> = HashMap::new();
        let mut add: Vec<CollisionFace> = Vec::new();
        for e in edits {
            let solid = self.placements[e.placement].solid;
            let shape = match e.from {
                Some(from) => {
                    let (own, shape) = cache
                        .own
                        .entry(e.placement)
                        .or_insert_with(|| (self.collision_of_placement(solid, from), self.local_faces(solid, from)));
                    for &(pi, fi) in own.iter() {
                        cut.entry(pi).or_default().insert(fi as u16);
                    }
                    &*shape
                }
                None => &*cache.solids.entry(solid).or_insert_with(|| self.solid_collision(solid)),
            };
            if !shape.is_empty() {
                done.solids_with_collision += 1;
            }
            if let Some(to) = e.to {
                add.extend(shape.iter().map(|f| world_face(f, to)));
            }
        }
        // The removed faces: every pack's static face with those corners.
        if !drawn.remove.is_empty() {
            let mut wanted: HashMap<Vec<[i32; 3]>, bool> = drawn.remove.iter().map(|c| (face_key(c), false)).collect();
            for (pi, g) in self.ground.grids().enumerate() {
                let _ = g;
                let mut fi = 0;
                while let Some(f) = self.ground.face_at(pi, fi) {
                    if f.group == 0
                        && let Some(hit) = wanted.get_mut(&face_key(&f.vertices[..f.corners.clamp(3, 4)]))
                    {
                        *hit = true;
                        cut.entry(pi).or_default().insert(fi as u16);
                    }
                    fi += 1;
                }
            }
            done.unmatched = wanted.values().filter(|h| !**h).count();
            let mut told = HashSet::new();
            for c in &drawn.remove {
                let k = face_key(c);
                if !wanted[&k] && told.insert(k) {
                    done.unmatched_at.push(centre(c));
                }
            }
        }
        let first_drawn = add.len();
        add.extend(drawn.faces.iter().map(|f| world_face(&drawn_local(f), Mat4::IDENTITY)));
        // Each new face into every pack whose grid covers it.
        let mut per_pack: HashMap<usize, Vec<(CollisionFace, Vec<(usize, u16)>)>> = HashMap::new();
        for (k, f) in add.iter().enumerate() {
            let mut any = false;
            let n = f.corners.clamp(3, 4);
            let (xl, xh) = (f.x[..n].iter().min().map_or(0, |&v| v), f.x[..n].iter().max().map_or(0, |&v| v));
            let (yl, yh) = (f.y[..n].iter().min().map_or(0, |&v| v), f.y[..n].iter().max().map_or(0, |&v| v));
            let (xl, xh, yl, yh) = (xl as f64 / 8.0, xh as f64 / 8.0, yl as f64 / 8.0, yh as f64 / 8.0);
            for (pi, g) in self.ground.grids().enumerate() {
                // A grid the face's bounds miss lists it in no cell.
                let (ox, oy) = (g.origin[0] as f64, g.origin[1] as f64);
                if xh < ox || yh < oy || xl > ox + g.width as f64 * CELL || yl > oy + g.height as f64 * CELL {
                    continue;
                }
                let cells = cells_of(f, g.origin, g.width, g.height);
                if !cells.is_empty() {
                    per_pack.entry(pi).or_default().push((*f, cells));
                    any = true;
                }
            }
            if !any {
                done.outside += 1;
                if let Some(d) = k.checked_sub(first_drawn).and_then(|d| drawn.faces.get(d)) {
                    done.outside_at.push(centre(&d.corners[..if d.quad { 4 } else { 3 }]));
                }
            }
        }
        let packs: HashSet<usize> = cut.keys().chain(per_pack.keys()).copied().collect();
        let mut packs: Vec<usize> = packs.into_iter().collect();
        packs.sort_unstable();
        let none = HashSet::new();
        // The triangles of the collision the rest of the game asks (walls
        // for the camera, the props, the AI): the same change.
        let mut cut_triangles: crate::fxhash::FxHashSet<(u32, [[u32; 3]; 3])> = Default::default();
        let mut new_sources = Vec::new();
        for pi in packs {
            let section = self.ground.grids().nth(pi).map_or(0, |g| g.section as u32);
            let c = cut.get(&pi).unwrap_or(&none);
            let a = per_pack.remove(&pi).unwrap_or_default();
            for &fi in c {
                if let Some(f) = self.ground.face_at(pi, fi as usize) {
                    for t in fans(f) {
                        cut_triangles.insert((section, t.map(|v| v.to_array().map(f32::to_bits))));
                    }
                }
            }
            if let (Some(save), Some(state)) = (save.as_deref_mut(), self.ground.pack_state(pi)) {
                save.push((pi, state));
            }
            if !self.ground.edit_pack(pi, c, &a) {
                log::warn!("map mods: collision pack {section} is full; its scenery collision is left as it was");
                continue;
            }
            done.cut += c.len();
            done.added += a.len();
            for (f, _) in &a {
                for corners in fans(f) {
                    new_sources.push(crate::collision::Source {
                        corners,
                        tag: section,
                        wall: f.is_wall(),
                        flags: f.flags,
                        group: f.group,
                    });
                }
            }
        }
        if !cut_triangles.is_empty() {
            self.collision_faces
                .retain(|s| !cut_triangles.contains(&(s.tag, s.corners.map(|v| v.to_array().map(f32::to_bits)))));
        }
        self.collision_faces.extend(new_sources);
        self.collision = Default::default();
        done
    }
}

/// A face's triangles as the load makes them (corners 0 1 2, then 0 2 3).
fn fans(f: &CollisionFace) -> Vec<[Vec3; 3]> {
    let v = f.vertices;
    if f.corners == 4 { vec![[v[0], v[1], v[2]], [v[0], v[2], v[3]]] } else { vec![[v[0], v[1], v[2]]] }
}

/// Local face `f` at `t`, stored as a pack stores it: x and y in eighths of
/// a metre, heights in 1/256 m from a whole-metre base, a triangle's corner
/// 3 repeating corner 2, and the bounds byte naming the last corner that
/// gives each of x-min, y-min, x-max, y-max (as in every shipped face).
pub fn world_face(f: &LocalFace, t: Mat4) -> CollisionFace {
    let n = if f.quad { 4 } else { 3 };
    let mut w = f.corners.map(|c| t.transform_point3(c));
    if n == 3 {
        w[3] = w[2];
    }
    let q = |v: f32, s: f32| (v * s).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    let x = w.map(|v| q(v.x, 8.0));
    let y = w.map(|v| q(v.y, 8.0));
    // Heights are i16 in 1/256 m: up to 128 m either side of the base,
    // so a face taller than that is based at its middle.
    let (zlo, zhi) = w.iter().fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v.z), h.max(v.z)));
    let base = if (zhi - zlo.floor()) * 256.0 <= i16::MAX as f32 { zlo.floor() } else { ((zlo + zhi) * 0.5).floor() };
    let base = base.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    let z = w.map(|v| q(v.z - base as f32, 256.0));
    let vertices =
        std::array::from_fn(|i| Vec3::new(x[i] as f32 / 8.0, y[i] as f32 / 8.0, base as f32 + z[i] as f32 / 256.0));
    let pick = |v: &[i16; 4], better: fn(i16, i16) -> bool| {
        (0..4).fold(0usize, |b, i| if better(v[i], v[b]) || v[i] == v[b] { i } else { b })
    };
    let bounds = pick(&x, |a, b| a < b) as u8
        | (pick(&y, |a, b| a < b) as u8) << 2
        | (pick(&x, |a, b| a > b) as u8) << 4
        | (pick(&y, |a, b| a > b) as u8) << 6;
    let flags = (f.flags & !FLAG_QUAD) | if f.quad { FLAG_QUAD } else { 0 };
    CollisionFace { vertices, corners: n, surface: f.surface, flags, group: 0, bounds, x, y }
}

/// The cells (row-major index) of a pack's grid that list face `f`, each with
/// its sub-cell mask (bit `sy·4 + sx`): a cell lists the face when the face's
/// outline meets the cell with its far edges left out; a sub-cell's bit is
/// set when the outline meets it edges included. This is the rule the
/// shipped packs follow (L4RA: the same face lists in 14,547 of its 14,550
/// cells, the same masks in 14,533; the rest differ by a face that only
/// touches a cell's edge).
pub fn cells_of(f: &CollisionFace, origin: [f32; 2], width: u16, height: u16) -> Vec<(usize, u16)> {
    let n = f.corners.clamp(3, 4);
    let poly: Vec<DVec2> = (0..n).map(|i| DVec2::new(f.x[i] as f64 / 8.0, f.y[i] as f64 / 8.0)).collect();
    let (lo, hi) =
        poly.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
    let (ox, oy) = (origin[0] as f64, origin[1] as f64);
    let (w, h) = (width as i64, height as i64);
    let cx0 = (((lo.x - ox) / CELL).floor() as i64 - 1).max(0);
    let cy0 = (((lo.y - oy) / CELL).floor() as i64 - 1).max(0);
    let cx1 = (((hi.x - ox) / CELL).floor() as i64 + 1).min(w - 1);
    let cy1 = (((hi.y - oy) / CELL).floor() as i64 + 1).min(h - 1);
    let mut out = Vec::new();
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            let a = DVec2::new(ox + cx as f64 * CELL, oy + cy as f64 * CELL);
            if !meets(&poly, lo, hi, a, a + CELL, false) {
                continue;
            }
            let mut mask = 0u16;
            for sy in 0..4 {
                for sx in 0..4 {
                    let s = a + DVec2::new(sx as f64 * SUB, sy as f64 * SUB);
                    if meets(&poly, lo, hi, s, s + SUB, true) {
                        mask |= 1 << (sy * 4 + sx);
                    }
                }
            }
            if mask != 0 {
                out.push(((cy * w + cx) as usize, mask));
            }
        }
    }
    out
}

/// Whether the outline `poly` (bounds `lo..hi`) meets the square `a..b`:
/// separating axes (the square's, then the outline's edge normals), the
/// square's far edges included only when `closed`.
fn meets(poly: &[DVec2], lo: DVec2, hi: DVec2, a: DVec2, b: DVec2, closed: bool) -> bool {
    let apart = if closed {
        hi.x < a.x || lo.x > b.x || hi.y < a.y || lo.y > b.y
    } else {
        hi.x < a.x || lo.x >= b.x || hi.y < a.y || lo.y >= b.y
    };
    if apart {
        return false;
    }
    let corners = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)];
    for i in 0..poly.len() {
        let e = poly[(i + 1) % poly.len()] - poly[i];
        let nrm = DVec2::new(-e.y, e.x);
        let len = nrm.length();
        if len == 0.0 {
            continue;
        }
        let nrm = nrm / len;
        let (pl, ph) = poly.iter().fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(nrm.dot(*p)), h.max(nrm.dot(*p))));
        let (sl, sh) = corners.iter().fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(nrm.dot(*p)), h.max(nrm.dot(*p))));
        if ph < sl || pl > sh {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
