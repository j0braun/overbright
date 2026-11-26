//! Floors and walls for driving: the ground under a point, and where a
//! moving point first meets a wall.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use glam::{Vec2, Vec3};

const CELL: f32 = 16.0;

/// The track's barrier groups' switches: the `on` byte (+8) of each
/// 32-byte entry of the track bundle's `0x3412F` table, which the ground
/// query (0x5E75D9) and the polygon gather (0x5E1037) read for a face whose
/// byte 0 (its group) is not 0: such a face counts only while its group is
/// on. Group 0 (`NONE`) is never tested, so it always counts. Every
/// shipped entry has 1 there; 0x57A590 switches all but entry 0 off as
/// soon as the track bundle is in (0x5D8360 at 0x57A5DF: +8 = (index ==
/// 0)), and a race or free roam switches its own on (0x578070, see
/// `World::set_up_barriers`). Shared by the ground lookup, this module's
/// triangles and the cars' own lists ([`crate::ground::GroundQuery`]), so
/// a switch reaches all of them at once.
#[derive(Debug, Default)]
pub struct BarrierSwitches(Vec<AtomicBool>);

impl BarrierSwitches {
    /// `count` groups (the table's entries, `NONE` included), all off.
    pub fn new(count: usize) -> BarrierSwitches {
        BarrierSwitches((0..count).map(|_| AtomicBool::new(false)).collect())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether faces of `group` count: always for group 0, else while its
    /// entry's +8 is set (`movsx`, nonzero).
    pub fn is_on(&self, group: u8) -> bool {
        group == 0 || self.0.get(group as usize).is_some_and(|b| b.load(Ordering::Relaxed))
    }

    pub fn set(&self, group: usize, on: bool) {
        if let Some(b) = self.0.get(group) {
            b.store(on, Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    /// Unit normal, pointing up for floors.
    normal: Vec3,
    /// Caller's tag for where the triangle came from.
    tag: u32,
    wall: bool,
    /// The face's flag byte (collision face byte 3).
    flags: u8,
    /// The face's barrier group (collision face byte 0).
    group: u8,
}

/// A triangle to build a [`Collision`] from.
#[derive(Clone, Copy, Debug)]
pub struct Source {
    pub corners: [Vec3; 3],
    /// Returned by [`Collision::wall_hit_tag`].
    pub tag: u32,
    /// Walls stop the body; the rest are floors for the wheels.
    pub wall: bool,
    /// The game's face flags (byte 3): 0x01 a floor on one of several
    /// levels, 0x02 wall, 0x08 not a floor for the wheels, 0x40 see
    /// [`Collision::ground_at`]. 0 for triangles not from collision packs.
    pub flags: u8,
    /// The face's barrier group (byte 0): a triangle of a group other
    /// than 0 counts only while [`BarrierSwitches`] has it on. 0 for
    /// triangles not from collision packs.
    pub group: u8,
}

/// Collision triangles bucketed on a 2D grid.
#[derive(Default)]
pub struct Collision {
    triangles: Vec<Triangle>,
    min: Vec2,
    cells: (usize, usize),
    buckets: Vec<Vec<u32>>,
    /// The barrier groups' switches; without them every group counts.
    barriers: Option<Arc<BarrierSwitches>>,
}

/// A floor must face at least this far up to carry a wheel.
const FLOOR_NZ: f32 = 0.3;

impl Collision {
    pub fn new(source: &[Source]) -> Collision {
        let mut triangles = Vec::new();
        for &Source { corners: [a, b, c], tag, wall, flags, group } in source {
            let n = (b - a).cross(c - a);
            if n.length_squared() < 1e-8 {
                continue;
            }
            let mut normal = n.normalize();
            if normal.z < 0.0 {
                normal = -normal;
            }
            triangles.push(Triangle { a, b, c, normal, tag, wall, flags, group });
        }
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for t in &triangles {
            for p in [t.a, t.b, t.c] {
                lo = lo.min(p.truncate());
                hi = hi.max(p.truncate());
            }
        }
        if triangles.is_empty() {
            return Collision::default();
        }
        let cells = (((hi.x - lo.x) / CELL) as usize + 1, ((hi.y - lo.y) / CELL) as usize + 1);
        let mut buckets = vec![Vec::new(); cells.0 * cells.1];
        for (i, t) in triangles.iter().enumerate() {
            let tlo = t.a.truncate().min(t.b.truncate()).min(t.c.truncate());
            let thi = t.a.truncate().max(t.b.truncate()).max(t.c.truncate());
            let (x0, y0) = (((tlo.x - lo.x) / CELL) as usize, ((tlo.y - lo.y) / CELL) as usize);
            let (x1, y1) = (((thi.x - lo.x) / CELL) as usize, ((thi.y - lo.y) / CELL) as usize);
            for y in y0..=y1.min(cells.1 - 1) {
                for x in x0..=x1.min(cells.0 - 1) {
                    buckets[y * cells.0 + x].push(i as u32);
                }
            }
        }
        Collision { triangles, min: lo, cells, buckets, barriers: None }
    }

    /// The same, with its barrier-group triangles following `barriers`.
    pub fn with_barriers(mut self, barriers: Arc<BarrierSwitches>) -> Collision {
        self.barriers = Some(barriers);
        self
    }

    /// Whether a triangle counts: its barrier group is on.
    fn counts(&self, t: &Triangle) -> bool {
        t.group == 0 || self.barriers.as_ref().is_none_or(|b| b.is_on(t.group))
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    fn cell_of(&self, p: Vec2) -> Option<(usize, usize)> {
        let c = (p - self.min) / CELL;
        if c.x < 0.0 || c.y < 0.0 {
            return None;
        }
        let (x, y) = (c.x as usize, c.y as usize);
        (x < self.cells.0 && y < self.cells.1).then_some((x, y))
    }

    /// Every triangle whose x/y bounds meet the box `lo..hi`: its index,
    /// corners, unit normal (pointing up for floors) and whether it is a wall.
    pub fn triangles_near(&self, lo: Vec2, hi: Vec2) -> Vec<(usize, [Vec3; 3], Vec3, bool)> {
        let mut out = Vec::new();
        if self.triangles.is_empty() {
            return out;
        }
        let top = self.min + Vec2::new(self.cells.0 as f32, self.cells.1 as f32) * CELL - 0.01;
        let (Some((x0, y0)), Some((x1, y1))) = (self.cell_of(lo.max(self.min)), self.cell_of(hi.min(top))) else {
            return out;
        };
        let mut seen = std::collections::HashSet::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                for &i in &self.buckets[y * self.cells.0 + x] {
                    let t = &self.triangles[i as usize];
                    let (tlo, thi) = (t.a.truncate().min(t.b.truncate()).min(t.c.truncate()), t.a.truncate().max(t.b.truncate()).max(t.c.truncate()));
                    if thi.x < lo.x || thi.y < lo.y || tlo.x > hi.x || tlo.y > hi.y || !self.counts(t) || !seen.insert(i) {
                        continue;
                    }
                    out.push((i as usize, [t.a, t.b, t.c], t.normal, t.wall));
                }
            }
        }
        out
    }

    /// The ground under a wheel as the game finds it (0x5E72F0, called by
    /// 0x59D260 at the wheel's rest point): walls and faces flagged 0x08
    /// are not ground. The first ordinary floor whose outline contains `p`
    /// (in x/y) is the ground, at whatever height. Floors flagged 0x01 —
    /// where roads cross over each other — compete instead: the highest at
    /// or below `p.z + 1`, taken at once if it is within 4 m of that and not
    /// flagged 0x40. Failing all that, the search runs again with no height
    /// limit. Returns the height and the upward normal.
    ///
    /// The game walks its own grid cells (pack chunk `0x34137`), floors
    /// first; this walks faces in pack order, which only matters where two
    /// ordinary floors overlap. Its cache of the last four floors found is
    /// left out.
    pub fn ground_at(&self, p: Vec3) -> Option<(f32, Vec3)> {
        let (x, y) = self.cell_of(p.truncate())?;
        let mut limit = p.z + 1.0;
        for _ in 0..2 {
            let mut best: Option<(f32, Vec3)> = None;
            for &i in &self.buckets[y * self.cells.0 + x] {
                let t = &self.triangles[i as usize];
                if t.wall || t.flags & 0x0A != 0 || t.normal.z.abs() < 1e-6 || !self.counts(t) || !inside_xy(p.truncate(), t) {
                    continue;
                }
                let z = t.a.z - ((p.x - t.a.x) * t.normal.x + (p.y - t.a.y) * t.normal.y) / t.normal.z;
                if t.flags & 0x01 == 0 {
                    return Some((z, t.normal));
                }
                if best.is_none_or(|(bz, _)| z > bz) && z <= limit {
                    best = Some((z, t.normal));
                    if t.flags & 0x40 == 0 && limit - z < 4.0 {
                        return best;
                    }
                }
            }
            if best.is_some() {
                return best;
            }
            limit = f32::MAX;
        }
        None
    }

    /// The highest floor at or below `p`, with its upward normal.
    pub fn ground_below(&self, p: Vec3) -> Option<(f32, Vec3)> {
        let (x, y) = self.cell_of(p.truncate())?;
        let mut best: Option<(f32, Vec3)> = None;
        for &i in &self.buckets[y * self.cells.0 + x] {
            let t = &self.triangles[i as usize];
            if t.wall || t.normal.z < FLOOR_NZ || !self.counts(t) {
                continue;
            }
            if !inside_xy(p.truncate(), t) {
                continue;
            }
            // Height of the plane at p.
            let z = t.a.z - ((p.x - t.a.x) * t.normal.x + (p.y - t.a.y) * t.normal.y) / t.normal.z;
            if z <= p.z && best.is_none_or(|(bz, _)| z > bz) {
                best = Some((z, t.normal));
            }
        }
        best
    }

    /// The first wall the segment `from → to` crosses: the fraction along
    /// the segment and the wall's normal turned to face `from`.
    pub fn wall_hit(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        self.wall_hit_tag(from, to).map(|(s, n, _)| (s, n))
    }

    /// Like [`Collision::wall_hit`], with the wall's tag.
    pub fn wall_hit_tag(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3, u32)> {
        let lo = from.truncate().min(to.truncate());
        let hi = from.truncate().max(to.truncate());
        let (x0, y0) = self.cell_of(lo.max(self.min))?;
        let top = self.min + Vec2::new(self.cells.0 as f32, self.cells.1 as f32) * CELL - 0.01;
        let (x1, y1) = self.cell_of(hi.min(top))?;
        let mut best: Option<(f32, Vec3, u32)> = None;
        let dir = to - from;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for &i in &self.buckets[y * self.cells.0 + x] {
                    let t = &self.triangles[i as usize];
                    if !t.wall || !self.counts(t) {
                        continue;
                    }
                    let n = t.normal;
                    let (d0, d1) = ((from - t.a).dot(n), (to - t.a).dot(n));
                    if d0 * d1 > 0.0 || d0 == d1 {
                        continue;
                    }
                    let s = d0 / (d0 - d1);
                    let hit = from + dir * s;
                    if !inside(hit, t) {
                        continue;
                    }
                    if best.is_none_or(|(bs, _, _)| s < bs) {
                        best = Some((s, if d0 >= 0.0 { n } else { -n }, t.tag));
                    }
                }
            }
        }
        best
    }
}

fn inside_xy(p: Vec2, t: &Triangle) -> bool {
    let (a, b, c) = (t.a.truncate(), t.b.truncate(), t.c.truncate());
    let cross = |o: Vec2, u: Vec2, v: Vec2| (u - o).perp_dot(v - o);
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn inside(p: Vec3, t: &Triangle) -> bool {
    let n = t.normal;
    let edge = |u: Vec3, v: Vec3| (v - u).cross(p - u).dot(n);
    let (d1, d2, d3) = (edge(t.a, t.b), edge(t.b, t.c), edge(t.c, t.a));
    let neg = d1 < -1e-4 || d2 < -1e-4 || d3 < -1e-4;
    let pos = d1 > 1e-4 || d2 > 1e-4 || d3 > 1e-4;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_triangles_follow_their_switch() {
        let floor = |z: f32, group: u8| Source { corners: [Vec3::new(0.0, 0.0, z), Vec3::new(10.0, 0.0, z), Vec3::new(0.0, 10.0, z)], tag: 0, wall: false, flags: 0, group };
        let wall = Source { corners: [Vec3::new(5.0, -5.0, -5.0), Vec3::new(5.0, 15.0, -5.0), Vec3::new(5.0, 15.0, 15.0)], tag: 7, wall: true, flags: 0x02, group: 1 };
        let switches = Arc::new(BarrierSwitches::new(2));
        let c = Collision::new(&[floor(0.0, 0), floor(2.0, 1), wall]).with_barriers(switches.clone());
        let p = Vec3::new(2.0, 2.0, 3.0);
        assert_eq!(c.ground_below(p).map(|g| g.0), Some(0.0));
        assert!(c.wall_hit(Vec3::new(4.0, 3.0, 1.0), Vec3::new(6.0, 3.0, 1.0)).is_none());
        switches.set(1, true);
        assert_eq!(c.ground_below(p).map(|g| g.0), Some(2.0));
        assert!(c.wall_hit(Vec3::new(4.0, 3.0, 1.0), Vec3::new(6.0, 3.0, 1.0)).is_some());
        // Without switches every group counts.
        let all = Collision::new(&[floor(0.0, 0), floor(2.0, 1)]);
        assert_eq!(all.ground_below(p).map(|g| g.0), Some(2.0));
    }
}

crate::persist!(Source { corners, tag, wall, flags, group });
/// The switches as they stand (a fresh set holding the same values).
impl crate::cache::Persist for BarrierSwitches {
    fn put(&self, w: &mut crate::cache::Writer) {
        let on: Vec<bool> = self.0.iter().map(|b| b.load(Ordering::Relaxed)).collect();
        crate::cache::Persist::put(&on, w)
    }
    fn get(r: &mut crate::cache::Reader) -> crate::cache::Got<Self> {
        let on: Vec<bool> = crate::cache::Persist::get(r)?;
        Ok(BarrierSwitches(on.into_iter().map(AtomicBool::new).collect()))
    }
}
