//! The collision layer the map editor draws (docs/modding/map-editor.md
//! "Collision"): the original's static collision faces as the game loads
//! them (the stream file's `0x80034130` packs, `formats::collision`), sorted
//! into what a player cannot see:
//!
//! * **invisible walls**: the pieces of a wall of barrier group 0 (always
//!   there) with no drawn scenery standing at them;
//! * **race and career barriers**: walls of a barrier group (`0x3412F`),
//!   switched on per race or per career stage (`BARRIERS_<n>`,
//!   `PLAYER_BARRIERS_<n>`, `BARRIERS_CAREER<n>`); none of them has a model
//!   of its own (the race's `XO_TrackBarrier` props stand at some);
//! * the other walls, with scenery at them, and the floors.
//!
//! Read-only: nothing here changes the collision, and the game does not use
//! this module. A face listed in several packs (a third of a region's are)
//! is taken once.

use std::collections::{HashMap, HashSet};

use glam::{Vec2, Vec3};

use crate::formats::collision::{BarrierGroup, CollisionFace, FLAG_TALL};

/// How far drawn scenery may stand from a wall and still count as the
/// wall's look (a facade's skin sits up to about a metre off its wall).
pub const COVER_DISTANCE: f32 = 1.0;
/// How high over a wall's foot the cover test looks for scenery.
pub const COVER_HEIGHT: f32 = 2.0;
/// The length of one piece of a wall the cover test samples.
pub const PIECE: f32 = 2.0;
/// How high a wall that reaches ±2048 m (flag 0x04) is drawn above the
/// floors beside it.
pub const TALL_SHOWN: f32 = 4.0;
/// A scenery triangle steeper than this (|normal.z| below it) can hide a
/// wall; flatter ones are roads, pavements and roofs.
const STEEP_NZ: f32 = 0.7;
/// The scenery grid's cell (metres).
const SCENERY_CELL: f32 = 4.0;
/// The floors' grid cell, for the floors beside a tall wall (metres).
const FLOOR_CELL: f32 = 16.0;
/// The drawing index's cell (metres).
pub const INDEX_CELL: f32 = 64.0;

/// What a wall is to the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WallKind {
    /// Group 0 with no scenery at it: an invisible wall.
    Invisible,
    /// A race or career barrier (a barrier group other than 0).
    Barrier,
    /// Group 0 with scenery at it.
    Visible,
}

/// Whether a face is the original's or written by the mod (the collision view marks the mod's apart).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mine {
    #[default]
    Original,
    /// An added prop's own collision.
    Added,
    /// A moved prop's collision, at its new place.
    Moved,
    /// A wall or ground the mod drew.
    Drawn,
}

/// One wall face, as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Wall {
    /// Its foot on the map: the two corners farthest apart in x, y.
    pub a: Vec2,
    pub b: Vec2,
    /// The height it is drawn from and to: its corners' for an ordinary
    /// wall, the floors beside it (+ [`TALL_SHOWN`]) for one reaching
    /// ±2048 m.
    pub z0: f32,
    pub z1: f32,
    /// Flag 0x04: the wall reaches ±2048 m in the game.
    pub tall: bool,
    /// The barrier group (0: always there).
    pub group: u8,
    /// The surface id (grip and sound; the pack's polygon byte 0x0B).
    pub surface: u8,
    pub mine: Mine,
    /// The face's corners as the pack stores them (3 or 4): how a map mod
    /// names it to remove it (`format::CollisionEdit::Remove`).
    pub corners: Vec<Vec3>,
    /// The parts along `a` → `b` (fractions 0..1) with no scenery at them,
    /// in order; empty for a fully covered wall. A barrier group's walls are
    /// not tested (all of them bare).
    pub bare: Vec<(f32, f32)>,
}

impl Wall {
    /// Its kind as a whole: a barrier, else invisible when any part is bare.
    pub fn kind(&self) -> WallKind {
        if self.group != 0 {
            WallKind::Barrier
        } else if self.bare.is_empty() {
            WallKind::Visible
        } else {
            WallKind::Invisible
        }
    }

    /// The quad of the part `t0`..`t1` along it: foot then top.
    pub fn quad(&self, t0: f32, t1: f32) -> [Vec3; 4] {
        let at = |t: f32, z: f32| (self.a + (self.b - self.a) * t).extend(z);
        [at(t0, self.z0), at(t1, self.z0), at(t1, self.z1), at(t0, self.z1)]
    }

    /// The parts with scenery at them (the gaps between [`Wall::bare`]).
    pub fn covered(&self) -> Vec<(f32, f32)> {
        if self.group != 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut t = 0.0;
        for &(b0, b1) in &self.bare {
            if b0 > t {
                out.push((t, b0));
            }
            t = b1;
        }
        if t < 1.0 {
            out.push((t, 1.0));
        }
        out
    }

    pub fn length(&self) -> f32 {
        self.a.distance(self.b)
    }
}

/// One floor face (triangles repeat no corner: `n` is 3 or 4).
#[derive(Clone, Debug, PartialEq)]
pub struct Floor {
    pub corners: [Vec3; 4],
    pub n: usize,
    /// Flag 0x08: not ground for the wheels.
    pub not_ground: bool,
    /// The surface id (grip and sound).
    pub surface: u8,
    pub mine: Mine,
}

impl Floor {
    pub fn points(&self) -> &[Vec3] {
        &self.corners[..self.n]
    }
}

/// The region's collision as the editor draws it.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    pub walls: Vec<Wall>,
    pub floors: Vec<Floor>,
    /// The barrier groups' names by group byte.
    pub groups: Vec<String>,
    /// [`INDEX_CELL`] cells → wall and floor indices touching them.
    index: HashMap<(i32, i32), (Vec<u32>, Vec<u32>)>,
    /// Faces read (with repeats) and kept.
    pub read: usize,
}

/// How many of each, for the status line and the docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub invisible: usize,
    pub barriers: usize,
    pub visible: usize,
    pub floors: usize,
}

impl Layer {
    /// Mark the faces `of` (given a face's corners) says the mod wrote.
    pub fn mark(&mut self, of: &dyn Fn(&[Vec3]) -> Mine) {
        for w in &mut self.walls {
            w.mine = of(&w.corners);
        }
        for f in &mut self.floors {
            f.mine = of(f.points());
        }
    }

    /// How many walls and floors each kind of mod face has: (added, moved, drawn).
    pub fn mine(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for m in self.walls.iter().map(|w| w.mine).chain(self.floors.iter().map(|f| f.mine)) {
            match m {
                Mine::Added => c.0 += 1,
                Mine::Moved => c.1 += 1,
                Mine::Drawn => c.2 += 1,
                Mine::Original => {}
            }
        }
        c
    }

    /// The surfaces the faces have, with how many faces each (by id).
    pub fn surfaces(&self) -> Vec<(u8, usize)> {
        let mut n = [0usize; 256];
        for s in self.walls.iter().map(|w| w.surface).chain(self.floors.iter().map(|f| f.surface)) {
            n[s as usize] += 1;
        }
        (0..=255u8).filter(|&s| n[s as usize] > 0).map(|s| (s, n[s as usize])).collect()
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts { floors: self.floors.len(), ..Counts::default() };
        for w in &self.walls {
            match w.kind() {
                WallKind::Invisible => c.invisible += 1,
                WallKind::Barrier => c.barriers += 1,
                WallKind::Visible => c.visible += 1,
            }
        }
        c
    }

    /// The name of barrier group `g` (`NONE` for 0).
    pub fn group_name(&self, g: u8) -> &str {
        self.groups.get(g as usize).map_or("", |s| s.as_str())
    }

    /// The walls and floors whose index cells meet the box `lo`..`hi`
    /// (x, y), each once, in index order.
    pub fn near(&self, lo: Vec2, hi: Vec2) -> (Vec<u32>, Vec<u32>) {
        let (c0, c1) = (cell(lo, INDEX_CELL), cell(hi, INDEX_CELL));
        let (mut walls, mut floors) = (Vec::new(), Vec::new());
        // A box larger than the region asks for every cell the index has.
        let cells = (c1.0 - c0.0 + 1) as i64 * (c1.1 - c0.1 + 1) as i64;
        if cells > self.index.len() as i64 {
            for (k, (w, f)) in &self.index {
                if (c0.0..=c1.0).contains(&k.0) && (c0.1..=c1.1).contains(&k.1) {
                    walls.extend_from_slice(w);
                    floors.extend_from_slice(f);
                }
            }
        } else {
            for y in c0.1..=c1.1 {
                for x in c0.0..=c1.0 {
                    if let Some((w, f)) = self.index.get(&(x, y)) {
                        walls.extend_from_slice(w);
                        floors.extend_from_slice(f);
                    }
                }
            }
        }
        for v in [&mut walls, &mut floors] {
            v.sort_unstable();
            v.dedup();
        }
        (walls, floors)
    }

    fn build_index(&mut self) {
        let mut index: HashMap<(i32, i32), (Vec<u32>, Vec<u32>)> = HashMap::new();
        let mut put = |lo: Vec2, hi: Vec2, i: u32, wall: bool| {
            let (c0, c1) = (cell(lo, INDEX_CELL), cell(hi, INDEX_CELL));
            for y in c0.1..=c1.1 {
                for x in c0.0..=c1.0 {
                    let e = index.entry((x, y)).or_default();
                    if wall { e.0.push(i) } else { e.1.push(i) }
                }
            }
        };
        for (i, w) in self.walls.iter().enumerate() {
            put(w.a.min(w.b), w.a.max(w.b), i as u32, true);
        }
        for (i, f) in self.floors.iter().enumerate() {
            let (lo, hi) = bounds_xy(f.points());
            put(lo, hi, i as u32, false);
        }
        self.index = index;
    }
}

fn cell(p: Vec2, size: f32) -> (i32, i32) {
    ((p.x / size).floor() as i32, (p.y / size).floor() as i32)
}

fn bounds_xy(points: &[Vec3]) -> (Vec2, Vec2) {
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for p in points {
        lo = lo.min(p.truncate());
        hi = hi.max(p.truncate());
    }
    (lo, hi)
}

/// The layer of `faces` (every pack's, repeats allowed), with the barrier
/// groups' names from `groups` and `scenery` the drawn triangles of the
/// region as the original places them (world space; only the steep ones
/// count).
pub fn build<'a>(
    faces: impl IntoIterator<Item = &'a CollisionFace>,
    groups: &[BarrierGroup],
    scenery: &[[Vec3; 3]],
) -> Layer {
    let mut seen = HashSet::new();
    let mut walls_raw = Vec::new();
    let mut layer = Layer { groups: groups.iter().map(|g| g.name.clone()).collect(), ..Layer::default() };
    for f in faces {
        layer.read += 1;
        let key = (f.group, f.flags, f.surface, f.x, f.y, f.vertices.map(|v| v.z.to_bits()));
        if !seen.insert(key) {
            continue;
        }
        if f.is_wall() {
            walls_raw.push(*f);
        } else {
            layer.floors.push(Floor { corners: f.vertices, n: f.corners, not_ground: f.flags & 0x08 != 0, surface: f.surface, mine: Mine::Original });
        }
    }
    let floors = FloorGrid::new(&layer.floors);
    let scenery = SceneryGrid::new(scenery);
    let mut walls: Vec<Wall> = walls_raw.iter().map(|f| wall_of(f, &floors)).collect();
    // The cover test, side by side over the walls.
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(16);
    let chunk = walls.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for part in walls.chunks_mut(chunk) {
            let scenery = &scenery;
            s.spawn(move || {
                for w in part {
                    w.bare = if w.group != 0 { vec![(0.0, 1.0)] } else { bare_parts(w, scenery) };
                }
            });
        }
    });
    layer.walls = walls;
    layer.build_index();
    layer
}

/// A wall face as drawn: its foot and heights.
fn wall_of(f: &CollisionFace, floors: &FloorGrid) -> Wall {
    let pts = &f.vertices[..f.corners];
    let (mut a, mut b, mut best) = (pts[0].truncate(), pts[0].truncate(), -1.0);
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            let d = pts[i].truncate().distance_squared(pts[j].truncate());
            if d > best {
                (a, b, best) = (pts[i].truncate(), pts[j].truncate(), d);
            }
        }
    }
    let tall = f.flags & FLAG_TALL != 0;
    let (z0, z1) = if tall {
        let (lo, hi) = floors.beside(a, b).unwrap_or((0.0, 0.0));
        (lo, hi + TALL_SHOWN)
    } else {
        let lo = pts.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let hi = pts.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        (lo, hi)
    };
    Wall { a, b, z0, z1, tall, group: f.group, surface: f.surface, mine: Mine::Original, bare: Vec::new(), corners: f.vertices[..f.corners.clamp(3, 4)].to_vec() }
}

/// The pieces of `w` no steep scenery triangle comes within
/// [`COVER_DISTANCE`] of, merged.
fn bare_parts(w: &Wall, scenery: &SceneryGrid) -> Vec<(f32, f32)> {
    let pieces = (w.length() / PIECE).ceil().max(1.0) as usize;
    let h = w.z1 - w.z0;
    // Tested [`COVER_HEIGHT`] over the foot, above kerbs and planter rims
    // (a kerb at a wall's foot does not show the wall); a lower wall at 60 %
    // of its height.
    let heights: Vec<f32> = if h >= COVER_HEIGHT + 0.4 { vec![w.z0 + COVER_HEIGHT] } else { vec![w.z0 + h * 0.6] };
    let mut bare: Vec<(f32, f32)> = Vec::new();
    for k in 0..pieces {
        let (t0, t1) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
        let mid = w.a + (w.b - w.a) * ((t0 + t1) * 0.5);
        let covered = heights.iter().any(|&z| scenery.near(mid.extend(z), COVER_DISTANCE));
        if covered {
            continue;
        }
        match bare.last_mut() {
            Some(last) if last.1 == t0 => last.1 = t1,
            _ => bare.push((t0, t1)),
        }
    }
    bare
}

/// The floors in a coarse grid, for the floors beside a tall wall.
struct FloorGrid<'a> {
    floors: &'a [Floor],
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl<'a> FloorGrid<'a> {
    fn new(floors: &'a [Floor]) -> FloorGrid<'a> {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, f) in floors.iter().enumerate() {
            if f.not_ground {
                continue;
            }
            let (lo, hi) = bounds_xy(f.points());
            let (c0, c1) = (cell(lo, FLOOR_CELL), cell(hi, FLOOR_CELL));
            for y in c0.1..=c1.1 {
                for x in c0.0..=c1.0 {
                    cells.entry((x, y)).or_default().push(i as u32);
                }
            }
        }
        FloorGrid { floors, cells }
    }

    /// The lowest and highest floor under points 1.5 m either side of the
    /// wall's middle; else the floors' corners nearest it.
    fn beside(&self, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        let mid = (a + b) * 0.5;
        let along = (b - a).normalize_or_zero();
        let side = Vec2::new(-along.y, along.x) * 1.5;
        let mut range: Option<(f32, f32)> = None;
        let mut take = |z: f32| range = Some(range.map_or((z, z), |(lo, hi)| (lo.min(z), hi.max(z))));
        for p in [mid + side, mid - side] {
            for &i in self.cells.get(&cell(p, FLOOR_CELL)).into_iter().flatten() {
                if let Some(z) = height_in(&self.floors[i as usize], p) {
                    take(z);
                }
            }
        }
        if range.is_some() {
            return range;
        }
        let (cx, cy) = cell(mid, FLOOR_CELL);
        let mut nearest: Option<(f32, f32)> = None;
        for y in cy - 1..=cy + 1 {
            for x in cx - 1..=cx + 1 {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    for p in self.floors[i as usize].points() {
                        let d = p.truncate().distance_squared(mid);
                        if nearest.is_none_or(|(nd, _)| d < nd) {
                            nearest = Some((d, p.z));
                        }
                    }
                }
            }
        }
        nearest.map(|(_, z)| (z, z))
    }
}

/// The height of floor `f` at `p` when `p` is inside it (x, y).
fn height_in(f: &Floor, p: Vec2) -> Option<f32> {
    let pts = f.points();
    for t in 1..pts.len() - 1 {
        let (a, b, c) = (pts[0], pts[t], pts[t + 1]);
        let d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
        if d.abs() < 1e-6 {
            continue;
        }
        let u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d;
        let v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
        let w = 1.0 - u - v;
        if u >= -1e-4 && v >= -1e-4 && w >= -1e-4 {
            return Some(u * a.z + v * b.z + w * c.z);
        }
    }
    None
}

/// The steep scenery triangles by [`SCENERY_CELL`] cells.
struct SceneryGrid {
    tris: Vec<[Vec3; 3]>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl SceneryGrid {
    fn new(scenery: &[[Vec3; 3]]) -> SceneryGrid {
        let mut tris = Vec::new();
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for t in scenery {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let len = n.length();
            if len < 1e-6 || (n.z / len).abs() >= STEEP_NZ {
                continue;
            }
            let i = tris.len() as u32;
            tris.push(*t);
            let (lo, hi) = bounds_xy(t);
            let (c0, c1) = (cell(lo, SCENERY_CELL), cell(hi, SCENERY_CELL));
            // A huge triangle (a backdrop) would fill thousands of cells:
            // those are never a wall's skin.
            if (c1.0 - c0.0 + 1) * (c1.1 - c0.1 + 1) > 4096 {
                continue;
            }
            for y in c0.1..=c1.1 {
                for x in c0.0..=c1.0 {
                    cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        SceneryGrid { tris, cells }
    }

    /// Whether a steep triangle comes within `r` of `p`.
    fn near(&self, p: Vec3, r: f32) -> bool {
        let (c0, c1) =
            (cell(p.truncate() - Vec2::splat(r), SCENERY_CELL), cell(p.truncate() + Vec2::splat(r), SCENERY_CELL));
        for y in c0.1..=c1.1 {
            for x in c0.0..=c1.0 {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    let t = &self.tris[i as usize];
                    if closest_on_triangle(p, t).distance_squared(p) <= r * r {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// The point of triangle `t` nearest `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5).
fn closest_on_triangle(p: Vec3, t: &[Vec3; 3]) -> Vec3 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::collision::{FLAG_QUAD, FLAG_WALL};

    /// A face with corners `v` (z as given; a tall wall's as the game
    /// replaces them).
    fn face(flags: u8, group: u8, v: &[Vec3]) -> CollisionFace {
        let mut vertices = [Vec3::ZERO; 4];
        vertices[..v.len()].copy_from_slice(v);
        if v.len() == 3 {
            vertices[3] = v[2];
        }
        let q = |f: f32| (f * 8.0) as i16;
        CollisionFace {
            vertices,
            corners: if flags & FLAG_QUAD != 0 { 4 } else { 3 },
            surface: 1,
            flags,
            group,
            bounds: 0,
            x: vertices.map(|p| q(p.x)),
            y: vertices.map(|p| q(p.y)),
        }
    }

    /// A wall from (x0, y) to (x1, y), `z0`..`z1`.
    fn wall(flags: u8, group: u8, x0: f32, x1: f32, y: f32, z0: f32, z1: f32) -> CollisionFace {
        let flags = flags | FLAG_WALL | FLAG_QUAD;
        let (lo, hi) = if flags & FLAG_TALL != 0 { (-2048.0, 2048.0) } else { (z0, z1) };
        face(flags, group, &[Vec3::new(x0, y, lo), Vec3::new(x0, y, hi), Vec3::new(x1, y, hi), Vec3::new(x1, y, lo)])
    }

    /// A flat floor square `lo`..`hi` at height `z`.
    fn floor(lo: Vec2, hi: Vec2, z: f32) -> CollisionFace {
        face(
            FLAG_QUAD,
            0,
            &[Vec3::new(lo.x, lo.y, z), Vec3::new(hi.x, lo.y, z), Vec3::new(hi.x, hi.y, z), Vec3::new(lo.x, hi.y, z)],
        )
    }

    /// A building's skin: a vertical quad (two triangles) 0.5 m in front of
    /// the wall line y, from x0 to x1, 0..8 m.
    fn skin(x0: f32, x1: f32, y: f32) -> Vec<[Vec3; 3]> {
        let (a, b, c, d) = (Vec3::new(x0, y, 0.0), Vec3::new(x1, y, 0.0), Vec3::new(x1, y, 8.0), Vec3::new(x0, y, 8.0));
        vec![[a, b, c], [a, c, d]]
    }

    fn groups() -> Vec<BarrierGroup> {
        ["NONE", "BARRIERS_CAREER1"].iter().map(|n| BarrierGroup { hash: 0, name: n.to_string() }).collect()
    }

    #[test]
    fn a_face_in_several_packs_is_taken_once() {
        let w = wall(0, 0, 0.0, 10.0, 0.0, 0.0, 6.0);
        let f = floor(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0), 0.0);
        let layer = build([&w, &f, &w, &f, &w], &groups(), &[]);
        assert_eq!(layer.read, 5);
        assert_eq!((layer.walls.len(), layer.floors.len()), (1, 1));
    }

    #[test]
    fn a_wall_with_no_scenery_is_invisible_and_one_behind_a_facade_is_not() {
        let bare = wall(0, 0, 0.0, 10.0, 0.0, 0.0, 6.0);
        let built = wall(0, 0, 0.0, 10.0, 20.0, 0.0, 6.0);
        let layer = build([&bare, &built], &groups(), &skin(0.0, 10.0, 20.5));
        assert_eq!(layer.walls[0].kind(), WallKind::Invisible);
        assert_eq!(layer.walls[0].bare, vec![(0.0, 1.0)]);
        assert_eq!(layer.walls[1].kind(), WallKind::Visible);
        assert!(layer.walls[1].bare.is_empty());
        assert_eq!(layer.walls[1].covered(), vec![(0.0, 1.0)]);
        assert_eq!(layer.counts(), Counts { invisible: 1, barriers: 0, visible: 1, floors: 0 });
    }

    #[test]
    fn a_wall_longer_than_its_building_is_bare_past_it() {
        // 20 m of wall, the facade 0.5 m off over its first 10 m: pieces of
        // 2 m, tested at their middles (the one at 11 m is 1.1 m from the
        // facade's end).
        let w = wall(0, 0, 0.0, 20.0, 0.0, 0.0, 6.0);
        let layer = build([&w], &groups(), &skin(0.0, 10.0, 0.5));
        assert_eq!(layer.walls[0].kind(), WallKind::Invisible);
        assert_eq!(layer.walls[0].bare, vec![(0.5, 1.0)]);
        assert_eq!(layer.walls[0].covered(), vec![(0.0, 0.5)]);
    }

    #[test]
    fn a_far_facade_or_a_flat_road_does_not_hide_a_wall() {
        let w = wall(0, 0, 0.0, 10.0, 0.0, 0.0, 6.0);
        // The facade 3 m off; a road square right at the wall's foot.
        let mut scenery = skin(0.0, 10.0, 3.0);
        let (a, b, c) = (Vec3::new(-5.0, -5.0, 0.0), Vec3::new(15.0, -5.0, 0.0), Vec3::new(15.0, 5.0, 0.0));
        scenery.push([a, b, c]);
        let layer = build([&w], &groups(), &scenery);
        assert_eq!(layer.walls[0].kind(), WallKind::Invisible);
    }

    #[test]
    fn barrier_group_walls_are_barriers_with_their_names() {
        let w = wall(0, 1, 0.0, 10.0, 0.0, 0.0, 6.0);
        let layer = build([&w], &groups(), &skin(0.0, 10.0, 0.5));
        assert_eq!(layer.walls[0].kind(), WallKind::Barrier);
        assert_eq!(layer.group_name(1), "BARRIERS_CAREER1");
        assert_eq!(layer.group_name(0), "NONE");
        assert!(layer.walls[0].covered().is_empty());
    }

    #[test]
    fn a_tall_wall_is_drawn_from_the_floors_beside_it() {
        // Floors at 10 m on one side and 13 m on the other.
        let w = wall(FLAG_TALL, 0, 0.0, 10.0, 0.0, 0.0, 0.0);
        let low = floor(Vec2::new(-10.0, -10.0), Vec2::new(20.0, 0.0), 10.0);
        let high = floor(Vec2::new(-10.0, 0.0), Vec2::new(20.0, 10.0), 13.0);
        let layer = build([&w, &low, &high], &groups(), &[]);
        let wl = &layer.walls[0];
        assert!(wl.tall);
        assert!((wl.z0 - 10.0).abs() < 1e-4 && (wl.z1 - (13.0 + TALL_SHOWN)).abs() < 1e-4, "{wl:?}");
        assert_eq!((wl.a, wl.b), (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
        assert_eq!(wl.quad(0.0, 0.5)[2], Vec3::new(5.0, 0.0, 17.0));
    }

    #[test]
    fn a_tall_wall_off_every_floor_takes_the_nearest_floor_corner() {
        let w = wall(FLAG_TALL, 0, 0.0, 10.0, 30.0, 0.0, 0.0);
        let f = floor(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0), 7.0);
        let layer = build([&w, &f], &groups(), &[]);
        assert_eq!((layer.walls[0].z0, layer.walls[0].z1), (7.0, 7.0 + TALL_SHOWN));
    }

    #[test]
    fn the_index_finds_what_is_near_a_box() {
        let near = wall(0, 0, 0.0, 10.0, 0.0, 0.0, 6.0);
        let far = wall(0, 0, 1000.0, 1010.0, 0.0, 0.0, 6.0);
        let f = floor(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0), 0.0);
        let layer = build([&near, &far, &f], &groups(), &[]);
        assert_eq!(layer.near(Vec2::new(-5.0, -5.0), Vec2::new(5.0, 5.0)), (vec![0], vec![0]));
        assert_eq!(layer.near(Vec2::new(990.0, -5.0), Vec2::new(995.0, 5.0)), (vec![1], vec![]));
        // A box over everything (more cells than the index has).
        assert_eq!(layer.near(Vec2::splat(-1e5), Vec2::splat(1e5)), (vec![0, 1], vec![0]));
    }

    #[test]
    fn closest_point_on_a_triangle() {
        let t = [Vec3::ZERO, Vec3::X * 4.0, Vec3::Y * 4.0];
        assert_eq!(closest_on_triangle(Vec3::new(1.0, 1.0, 3.0), &t), Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(closest_on_triangle(Vec3::new(-1.0, -1.0, 0.0), &t), Vec3::ZERO);
        assert_eq!(closest_on_triangle(Vec3::new(2.0, -3.0, 0.0), &t), Vec3::new(2.0, 0.0, 0.0));
    }
}
