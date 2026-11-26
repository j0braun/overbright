//! The ground under a car's wheels, found the way the game finds it: the
//! height query 0x5E72F0 over the track's collision packs, with the
//! per-car state it keeps between calls (the collision context pointer at
//! physics-view +0x4, i.e. full-car +0x44: its pack, section and last four
//! floors found). See docs/formats.md, "Collision".

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::collision::BarrierSwitches;
use crate::formats::collision::{CollisionFace, CollisionGrid};
use crate::formats::visibility::Visibility;

/// A face as the game's polygon (0x5D7F20): corners, plane and flags.
#[derive(Clone, Debug)]
struct Polygon {
    face: CollisionFace,
    normal: Vec3,
    /// Plane offset, normal · corner 0 (+0x7C).
    d: f32,
}

impl Polygon {
    fn new(face: CollisionFace) -> Polygon {
        let v = face.vertices;
        let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
        // 0x5D806B..5D8087: PC24 x87 accumulates Z + Y, then X.
        // Changing the addition order moves the plane and creates a false
        // compression delta when the next component post measures it.
        let d = (v[0].z * normal.z + v[0].y * normal.y) + v[0].x * normal.x;
        Polygon { d, normal, face }
    }

    /// The largest of the edge functions (0x5D3340): negative inside,
    /// clamped below at −1 by the first edge. Triangles repeat corner 2 as
    /// corner 3, so the third edge runs from it back to corner 0.
    fn inside(&self, p: Vec2) -> f32 {
        let v = &self.face.vertices;
        let edge = |a: Vec3, b: Vec3| (p.x - a.x) * (b.y - a.y) - (b.x - a.x) * (p.y - a.y);
        let first = edge(v[0], v[1]);
        let mut r = -1.0f32;
        if first >= -1.0 {
            r = first;
            if r > 0.0 {
                return r;
            }
        }
        r = r.max(edge(v[1], v[2]));
        if r <= 0.0 {
            r = r.max(edge(v[3], v[0]));
            if r <= 0.0 && self.face.corners == 4 {
                r = r.max(edge(v[2], v[3]));
            }
        }
        r
    }

    /// The plane's height at `p` (x, y).
    fn height(&self, p: Vec2) -> f32 {
        // 0x5E7396..5E73B5: multiply by the rounded reciprocal. A final
        // division is algebraically equivalent but has different PC24 bits.
        (self.normal.y * p.y + self.normal.x * p.x - self.d) * (-1.0 / self.normal.z)
    }

    /// The face's stored x/y bounds against `p` in 1/65536 m (0x5E7500).
    fn bounds_contain(&self, xi: i32, yi: i32) -> bool {
        let (f, b) = (&self.face, self.face.bounds as usize);
        let at = |v: i16| v as i32 * 0x2000;
        at(f.x[b & 3]) <= xi && at(f.y[(b >> 2) & 3]) <= yi && xi <= at(f.x[(b >> 4) & 3]) && yi <= at(f.y[b >> 6])
    }
}

struct Pack {
    grid: CollisionGrid,
    polygons: Vec<Polygon>,
}

/// The track's collision packs, keyed by the drivable section they serve.
#[derive(Default)]
pub struct TrackGround {
    packs: Vec<Pack>,
    by_section: HashMap<u32, usize>,
    /// The barrier groups' switches (track bundle `0x3412F` +8).
    barriers: Arc<BarrierSwitches>,
}

/// One car's ground-query state (its collision context).
#[derive(Clone, Debug, Default)]
pub struct GroundQuery {
    /// Where the section was last chosen, and how far inside it that was.
    anchor: Option<Vec2>,
    margin: f32,
    /// The drivable section (index into the outlines) and its pack.
    section: Option<usize>,
    pack: Option<usize>,
    /// The last floors found, newest first: (pack, face). At most four.
    recent: Vec<(usize, usize)>,
    /// The drivable outlines, most recently found first (the list at
    /// 0x883E70); empty until first used.
    order: Vec<usize>,
    /// The polygons cached around the car for its contacts (+0x3C, count
    /// +0x2E0): floors, then walls, then 0x08 faces; and the box they were
    /// gathered for (+0x2C..+0x38).
    nearby: Vec<(usize, usize)>,
    nearby_box: Option<(Vec2, Vec2)>,
    /// The context's own barrier groups (count +0x2F0, entry pointers from
    /// +0x2F4; added by 0x5D8100, which finds a group by name hash among
    /// entries 1..). The car set-up 0x604270 → 0x5F54B0 gives a racer's
    /// (owner kind 1 or 2) car its race's `PLAYER_BARRIERS_<n>` and
    /// `BARRIERS_<n>` and, in career free roam, `BARRIERS_CAREER<stage>`;
    /// traffic (kind 3) and props have none. The height query and the
    /// gather switch these on for their walk and off again after it
    /// (0x5E7403 / 0x5E76B2, 0x5E58E2 / 0x5E5915), so they always count for
    /// this car and, once it has looked, for nobody else.
    pub barriers: Vec<u8>,
}

impl GroundQuery {
    /// The pack the query last chose (debug aid).
    pub fn pack(&self) -> Option<usize> {
        self.pack
    }
}

/// A track polygon near the car, as the contact step sees it.
#[derive(Clone, Copy, Debug)]
pub struct TrackPolygon {
    /// (pack, face): the game sorts contacts by face address.
    pub id: (usize, usize),
    pub corners: [Vec3; 4],
    pub count: usize,
    /// Unit normal from corners 0..2 (+0x70) and the plane offset (+0x7C).
    pub normal: Vec3,
    pub d: f32,
    pub flags: u8,
    /// The surface (polygon +0x0B, the face's byte 2).
    pub surface: u8,
    /// X+0x64 of the track contact (0x5AA1FD): the face's byte 0 (group)
    /// is nonzero. 0x593E00 turns the normal toward the body only then (the
    /// gate at 0x593E34); a face with byte 0 zero keeps its normal.
    pub turn: bool,
    /// Stored x/y bounds, m (+0x14..+0x20).
    pub min: Vec2,
    pub max: Vec2,
}

impl TrackPolygon {
    /// Distance in x/y from `p` to the polygon (0x5D3460): 0 inside a
    /// non-wall, else the first edge closer than `limit`, or the nearest.
    pub fn distance_xy(&self, p: Vec2, limit: f32, inside: f32) -> f32 {
        if self.flags & 0x02 == 0 && inside <= 0.0 {
            return 0.0;
        }
        let mut best = limit;
        for i in 0..self.count {
            let (a, b) = (self.corners[i].truncate(), self.corners[(i + 1) % self.count].truncate());
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
            if d < best {
                best = d;
                if d < limit {
                    break;
                }
            }
        }
        best
    }
}

/// The query's slack around the section choice, m (0x5E72F0 → 0x5E68E0).
const SECTION_SLACK: f32 = 9.0;

impl TrackGround {
    /// `packs` are each pack's grid and faces; `barriers` says which
    /// barrier groups count.
    pub fn new(packs: Vec<(CollisionGrid, Vec<CollisionFace>)>, detail_limit: u32, barriers: Arc<BarrierSwitches>) -> TrackGround {
        let mut ground = TrackGround { barriers, ..Default::default() };
        for (grid, faces) in packs {
            // Sections limit..2·limit of a letter share the pack of
            // section − limit (0x5D3C20).
            let s = grid.section as u32;
            let n = s % 100;
            let key = if detail_limit > 0 && n >= detail_limit && n < 2 * detail_limit { s - detail_limit } else { s };
            ground.by_section.insert(key, ground.packs.len());
            ground.packs.push(Pack { grid, polygons: faces.into_iter().map(Polygon::new).collect() });
        }
        ground
    }

    /// Test-only projection of a captured history. No default section search:
    /// callers must assert every query stays inside the retained anchor slack.
    #[cfg(test)]
    pub(crate) fn native_history(&self, visibility: &Visibility, anchor: Vec2, margin: f32, recent: &[usize], barriers: Vec<u8>) -> GroundQuery {
        let pack = *self.by_section.get(&716).expect("native section 716 missing");
        assert_eq!(self.packs[pack].grid.section, 736);
        assert_eq!(self.packs[pack].polygons.len(), 745);
        // Compare original assets to the live native geometry, excluding only
        // the mutable face cache slot. Never resolve a history by proximity.
        for (poly, raw) in self.packs[pack].polygons.iter().zip(crate::testkit::private("docs/evidence/full-phase-inputs/ground-faces.bin").chunks_exact(32)) {
            assert_eq!((poly.face.group, poly.face.bounds, poly.face.surface, poly.face.flags), (raw[0], raw[1], raw[2], raw[3]));
            let short = |o| i16::from_le_bytes([raw[o], raw[o + 1]]) as f32;
            for k in 0..poly.face.corners {
                let z = if raw[3] & 4 != 0 { [-2048.0, 2048.0, 2048.0, -2048.0][k] } else { short(4) + short(24 + k * 2) / 256.0 };
                assert_eq!(poly.face.vertices[k], Vec3::new(short(8 + k * 2) / 8.0, short(16 + k * 2) / 8.0, z));
            }
        }
        let section = visibility.boundaries.iter().position(|b| b.section == 716).expect("native outline missing");
        assert!(recent.iter().all(|&face| face < self.packs[pack].polygons.len()));
        GroundQuery { anchor: Some(anchor), margin, section: Some(section), pack: Some(pack),
            recent: recent.iter().map(|&face| (pack, face)).collect(),
            barriers, ..GroundQuery::default() }
    }

    #[cfg(test)]
    pub(crate) fn native_recent(&self, q: &GroundQuery) -> Vec<usize> {
        q.recent.iter().map(|&(pack, face)| { assert_eq!(Some(pack), q.pack); face }).collect()
    }

    /// Diagnostics: the section and pack a query is using.
    pub fn describe(&self, visibility: &Visibility, q: &GroundQuery) -> String {
        format!(
            "section {:?} pack {:?} (section {:?}) margin {} anchor {:?}",
            q.section.map(|i| visibility.boundaries[i].section),
            q.pack,
            q.pack.map(|p| self.packs[p].grid.section),
            q.margin,
            q.anchor
        )
    }

    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    /// How many collision packs the ground holds (the pack ids of
    /// [`TrackGround::pack`] run below it).
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Every pack's faces as loaded, pack by pack (a face stored in several
    /// packs comes once per pack). Read-only; for the map editor's collision
    /// layer (`tools::map::collision`).
    pub fn faces(&self) -> impl Iterator<Item = &CollisionFace> {
        self.packs.iter().flat_map(|p| p.polygons.iter().map(|q| &q.face))
    }

    /// The faces a cell of their pack lists, pack by pack: the collision in
    /// play (a map mod's removed faces stay in the face list, listed in no
    /// cell). For the map editor's collision layer.
    pub fn listed_faces(&self) -> impl Iterator<Item = &CollisionFace> {
        self.packs.iter().flat_map(|p| {
            let mut listed = vec![false; p.polygons.len()];
            for &(f, _) in &p.grid.refs {
                if let Some(l) = listed.get_mut(f as usize) {
                    *l = true;
                }
            }
            p.polygons.iter().zip(listed).filter(|(_, l)| *l).map(|(q, _)| &q.face)
        })
    }

    /// The packs' grids, pack by pack (index = the pack ids of
    /// [`TrackGround::face_at`]).
    pub fn grids(&self) -> impl Iterator<Item = &CollisionGrid> {
        self.packs.iter().map(|p| &p.grid)
    }

    /// Face `face` of pack `pack` as stored.
    pub fn face_at(&self, pack: usize, face: usize) -> Option<&CollisionFace> {
        self.packs.get(pack)?.polygons.get(face).map(|p| &p.face)
    }

    /// What [`TrackGround::restore_pack`] needs to take pack `pack` back to
    /// how it is now after [`TrackGround::edit_pack`]s: its grid and its
    /// face count (an edit only appends faces).
    pub fn pack_state(&self, pack: usize) -> Option<(CollisionGrid, usize)> {
        self.packs.get(pack).map(|p| (p.grid.clone(), p.polygons.len()))
    }

    /// Put pack `pack` back as [`TrackGround::pack_state`] saw it.
    pub fn restore_pack(&mut self, pack: usize, (grid, faces): (CollisionGrid, usize)) {
        if let Some(p) = self.packs.get_mut(pack) {
            p.grid = grid;
            p.polygons.truncate(faces);
        }
    }

    /// Change pack `pack` for a map mod's scenery (`world::prop_collision`):
    /// the faces in `cut` leave every cell (they stay in the list, so no
    /// other face's index moves), and each face of `add` joins the end of
    /// the face list and of the cells it names (cell index, sub-cell mask),
    /// after the cell's own references; a floor (no flag 0x02 or 0x08: drawn
    /// ground) goes before them, as the height query stops at a cell's
    /// first wall and the original lists floors first. False (nothing changed) when the
    /// pack's face or reference count would pass the format's u16 limit.
    pub fn edit_pack(&mut self, pack: usize, cut: &std::collections::HashSet<u16>, add: &[(CollisionFace, Vec<(usize, u16)>)]) -> bool {
        let Some(pk) = self.packs.get_mut(pack) else { return false };
        let g = &pk.grid;
        let Some((refs, cells)) = crate::formats::collision::edit_cells(&g.cells, &g.refs, pk.polygons.len(), cut, add) else {
            return false;
        };
        pk.grid.refs = refs;
        pk.grid.cells = cells;
        pk.polygons.extend(add.iter().map(|(f, _)| Polygon::new(*f)));
        true
    }

    /// Switch the context's own barrier groups on or off (0x5E7403 /
    /// 0x5E76B2 around the height query's walk, 0x5E58E2 / 0x5E5915 around
    /// the gather): the global switches, so off means off for everyone.
    fn switch_own(&self, q: &GroundQuery, on: bool) {
        for &g in &q.barriers {
            self.barriers.set(g as usize, on);
        }
    }

    /// The ground at `p` for one car (0x5E72F0): its height, normal and
    /// surface, or `None`.
    ///
    /// * The pack is the one of the drivable section under `p`. It is only
    ///   chosen again once `p` has moved more than (9 m + how deep inside
    ///   its section it was) from where it was last chosen (0x5E68E0,
    ///   0x5DFEC0); a new pack empties the list of recent floors.
    /// * The recent floors are tried first: one whose outline holds `p` is
    ///   taken if it is an ordinary floor, or within 4 m under `p.z + 1`.
    /// * Then the pack's grid cell under `p`, in its stored order, skipping
    ///   faces outside `p`'s 8 m sub-cell or their stored x/y bounds, and
    ///   switched-off barrier groups, and stopping at the first wall or
    ///   0x08 face. The first ordinary floor (no flag 0x01) whose outline
    ///   holds `p` is the ground, at whatever height. Flag-0x01 floors
    ///   compete: the highest at or below `p.z + 1`, taken at once within
    ///   4 m of that unless it has flag 0x40. Failing that, the same again
    ///   with no height limit.
    /// * A floor found, unless flagged 0x40, goes to the front of the recent
    ///   list.
    pub fn query(&self, visibility: &Visibility, q: &mut GroundQuery, p: Vec3) -> Option<(f32, Vec3, u8)> {
        self.choose_pack(visibility, q, p.truncate());
        let pack_index = q.pack?;
        let at = p.truncate();
        let mut limit = p.z + 1.0;

        for i in 0..q.recent.len() {
            let (pi, fi) = q.recent[i];
            let poly = &self.packs[pi].polygons[fi];
            if poly.inside(at) < 0.0 {
                let h = poly.height(at);
                if poly.face.flags & 0x01 == 0 || (h <= limit && limit - h < 4.0) {
                    let hit = q.recent.remove(i);
                    q.recent.insert(0, hit);
                    return Some((h, poly.normal, poly.face.surface));
                }
            }
        }

        let pack = &self.packs[pack_index];
        let g = &pack.grid;
        let (w, hgt) = (g.width as i32, g.height as i32);
        let cx = ((at.x - g.origin[0]) * 0.03125) as i32;
        let cy = ((at.y - g.origin[1]) * 0.03125) as i32;
        let sub_x = ((at.x - (cx as f32 * 32.0 + g.origin[0])) * 0.125) as i32;
        let sub_y = ((at.y - (cy as f32 * 32.0 + g.origin[1])) * 0.125) as i32;
        let mask = 1u32.wrapping_shl(sub_x as u32 & 31).wrapping_shl(((sub_y as u32) << 2) & 31);
        let (col, row) = (cx.max(0).min(w - 1), cy.max(0).min(hgt - 1));
        let (first, count) = g.cells.get((row * w + col) as usize).copied().unwrap_or((0, 0));
        let (xi, yi) = ((at.x * 65536.0) as i32, (at.y * 65536.0) as i32);

        // The context's own groups count for this walk (0x5E73EF).
        self.switch_own(q, true);
        let mut found: Option<(usize, f32)> = None;
        for _ in 0..2 {
            let mut best = -99999.0f32;
            for r in first as usize..first as usize + count as usize {
                let Some(&(face, faces_mask)) = g.refs.get(r) else { break };
                if mask & faces_mask as u32 == 0 {
                    continue;
                }
                let Some(poly) = pack.polygons.get(face as usize) else { continue };
                if poly.face.flags & 0x0A != 0 {
                    break;
                }
                if !poly.bounds_contain(xi, yi) || !self.barriers.is_on(poly.face.group) {
                    continue;
                }
                let inside = poly.inside(at);
                if inside > 0.0 {
                    continue;
                }
                let h = poly.height(at);
                if poly.face.flags & 0x01 == 0 && inside < 0.0 {
                    found = Some((face as usize, h));
                    break;
                }
                if best < h && h <= limit {
                    best = h;
                    found = Some((face as usize, h));
                    if poly.face.flags & 0x40 == 0 && limit - h < 4.0 {
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
            limit = f32::MAX;
        }
        // ... and are switched off again after it (0x5E769B).
        self.switch_own(q, false);

        let (face, h) = found?;
        let poly = &pack.polygons[face];
        if poly.face.flags & 0x40 == 0 {
            q.recent.retain(|&r| r != (pack_index, face));
            q.recent.insert(0, (pack_index, face));
            q.recent.truncate(4);
        }
        Some((h, poly.normal, poly.face.surface))
    }

    /// The polygons near a car for its contacts (0x5E7110 → 0x5E0D40).
    /// `box_min..box_max` is the car body's box (position ± radius). The
    /// pack is chosen again for the box's centre, with no slack. The cache
    /// is kept while it still covers the box; otherwise it is gathered
    /// afresh over the box grown by 0.1 m and, in the direction of travel,
    /// by 0.2 s of velocity (at most 3 m): faces of the pack's grid cells
    /// under it, within their sub-cell masks and stored bounds, barrier
    /// groups on, at most 160, floors first, then walls, then 0x08 faces.
    pub fn nearby(&self, visibility: &Visibility, q: &mut GroundQuery, box_min: Vec2, box_max: Vec2, velocity: Vec2) -> Vec<TrackPolygon> {
        let before = q.pack;
        self.choose_pack_with(visibility, q, (box_min + box_max) * 0.5, 0.0);
        if q.pack != before {
            q.nearby_box = None;
        }
        let covered = q.nearby_box.is_some_and(|(lo, hi)| lo.x <= box_min.x && lo.y <= box_min.y && box_max.x <= hi.x && box_max.y <= hi.y);
        if !covered {
            let (mut lo, mut hi) = (box_min - 0.1, box_max + 0.1);
            let reach = (velocity * 0.2).clamp(Vec2::splat(-3.0), Vec2::splat(3.0));
            if reach.x <= 0.0 { lo.x += reach.x } else { hi.x += reach.x }
            if reach.y <= 0.0 { lo.y += reach.y } else { hi.y += reach.y }
            q.nearby_box = Some((lo, hi));
            // The context's own groups are on for the gather only
            // (0x5E58C4..0x5E5924, skipped without a pack).
            let own = q.pack.is_some();
            if own {
                self.switch_own(q, true);
            }
            q.nearby = self.gather(q.pack, lo, hi);
            if own {
                self.switch_own(q, false);
            }
        }
        q.nearby
            .iter()
            .map(|&(pi, fi)| {
                let poly = &self.packs[pi].polygons[fi];
                let f = &poly.face;
                let b = f.bounds as usize;
                TrackPolygon {
                    id: (pi, fi),
                    corners: f.vertices,
                    count: f.corners,
                    normal: poly.normal,
                    d: poly.d,
                    flags: f.flags,
                    surface: f.surface,
                    turn: f.group != 0,
                    min: Vec2::new(f.x[b & 3] as f32 * 0.125, f.y[(b >> 2) & 3] as f32 * 0.125),
                    max: Vec2::new(f.x[(b >> 4) & 3] as f32 * 0.125, f.y[b >> 6] as f32 * 0.125),
                }
            })
            .collect()
    }

    /// Whether a polygon's outline holds `p` (the edge test, ≤ 0 inside).
    pub fn inside(&self, id: (usize, usize), p: Vec2) -> f32 {
        self.packs[id.0].polygons[id.1].inside(p)
    }

    /// 0x5E0D40 over the box `lo..hi`.
    fn gather(&self, pack: Option<usize>, lo: Vec2, hi: Vec2) -> Vec<(usize, usize)> {
        let Some(pi) = pack else { return Vec::new() };
        let pk = &self.packs[pi];
        let g = &pk.grid;
        let (ox, oy) = (g.origin[0], g.origin[1]);
        let cell = |v: f32, o: f32| ((v - o) * 0.03125) as i32;
        let run = |a: f32, b: f32, origin: f32| -> u32 {
            let bits = 1 - ((b - a + 8.0) * -0.125) as i32;
            let start = ((a - origin) * 0.125) as i32;
            let mask = 1u32.wrapping_shl(bits as u32 & 31).wrapping_sub(1);
            mask.wrapping_shl(start.max(0) as u32 & 31) & 0xF
        };
        let (mut floors, mut walls, mut others) = (Vec::new(), Vec::new(), Vec::new());
        let mut seen = std::collections::HashSet::new();
        let mut total = 0;
        for cy in cell(lo.y, oy)..=cell(hi.y, oy) {
            for cx in cell(lo.x, ox)..=cell(hi.x, ox) {
                let (x0, y0) = (cx as f32 * 32.0 + ox, cy as f32 * 32.0 + oy);
                let xm = run(lo.x, hi.x, x0);
                let ym = run(lo.y, hi.y, y0);
                let mut mask = 0u32;
                for b in 0..4 {
                    if ym & (1 << b) != 0 {
                        mask |= xm << (4 * b);
                    }
                }
                let col = cx.max(0).min(g.width as i32 - 1);
                let row = cy.max(0).min(g.height as i32 - 1);
                let Some(&(first, count)) = g.cells.get((row * g.width as i32 + col) as usize) else { continue };
                for r in first as usize..first as usize + count as usize {
                    let Some(&(face, face_mask)) = g.refs.get(r) else { break };
                    if mask & face_mask as u32 == 0 || seen.contains(&face) {
                        continue;
                    }
                    let Some(poly) = pk.polygons.get(face as usize) else { continue };
                    let f = &poly.face;
                    let b = f.bounds as usize;
                    let m = |v: i16| v as f32 * 0.125;
                    let overlaps = m(f.x[b & 3]) <= hi.x && m(f.y[(b >> 2) & 3]) <= hi.y && lo.x <= m(f.x[(b >> 4) & 3]) && lo.y <= m(f.y[b >> 6]);
                    if !overlaps || !self.barriers.is_on(f.group) || total >= 160 {
                        continue;
                    }
                    seen.insert(face);
                    total += 1;
                    if f.flags & 0x02 != 0 {
                        walls.push((pi, face as usize));
                    } else if f.flags & 0x08 != 0 {
                        others.push((pi, face as usize));
                    } else {
                        floors.push((pi, face as usize));
                    }
                }
            }
        }
        floors.extend(walls);
        floors.extend(others);
        floors
    }

    /// 0x5E68E0 / 0x5DFEC0: keep or re-choose the section and its pack.
    fn choose_pack(&self, visibility: &Visibility, q: &mut GroundQuery, p: Vec2) {
        self.choose_pack_with(visibility, q, p, SECTION_SLACK);
    }

    fn choose_pack_with(&self, visibility: &Visibility, q: &mut GroundQuery, p: Vec2, slack: f32) {
        if q.order.is_empty() {
            q.order = visibility.drivable().map(|(i, _)| i).collect();
        }
        if let Some(a) = q.anchor {
            if (p.x - a.x).abs() + (p.y - a.y).abs() <= slack + q.margin {
                return;
            }
            if p.distance(a) <= slack + q.margin {
                return;
            }
        }
        q.anchor = Some(p);
        let outlines = &visibility.boundaries;
        if let Some(s) = q.section
            && outlines[s].contains(p)
        {
            q.margin = outlines[s].depth(p);
            return;
        }
        // 0x5DB860: the first outline holding p, which moves to the front;
        // else the nearest, if within 0.1 m (0x5DB780).
        let holding = q.order.iter().position(|&i| outlines[i].contains(p));
        let (section, margin) = match holding {
            Some(k) => {
                let i = q.order.remove(k);
                q.order.insert(0, i);
                (Some(i), outlines[i].depth(p))
            }
            None => {
                let mut nearest: Option<(usize, f32)> = None;
                for &i in &q.order {
                    let limit = nearest.map_or(9_999_999.0, |n| n.1);
                    let d = outlines[i].distance_outside(p, limit);
                    let better = match nearest {
                        None => true,
                        Some((n, nd)) => d < nd || (d == nd && outlines[i].section < outlines[n].section),
                    };
                    if better {
                        nearest = Some((i, d));
                    }
                }
                match nearest {
                    Some((i, d)) if d < 0.1 => (Some(i), outlines[i].depth(p)),
                    Some((_, d)) => (None, d),
                    None => (None, 0.0),
                }
            }
        };
        q.section = section;
        q.margin = margin;
        let pack = section.and_then(|i| self.by_section.get(&outlines[i].section).copied());
        if pack != q.pack {
            q.pack = pack;
            q.recent.clear();
            q.nearby_box = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::visibility::DrivableBoundary;

    /// One 32 m pack (section 101) with two floors over its cell, both
    /// four-cornered and ordinary: face 0 in barrier group 1 at 5 m, face
    /// 1 (group 0) at 0 m, listed in that order.
    fn square() -> (TrackGround, Visibility, Arc<BarrierSwitches>) {
        let outline = [Vec2::new(0.0, 0.0), Vec2::new(32.0, 0.0), Vec2::new(32.0, 32.0), Vec2::new(0.0, 32.0)];
        let face = |group: u8, z: f32| CollisionFace {
            vertices: outline.map(|c| c.extend(z)),
            corners: 4,
            surface: 0,
            flags: 0x10,
            group,
            // x-min and y-min from corner 0, x-max corner 1, y-max corner 2.
            bounds: (1 << 4) | (2 << 6),
            x: [0, 256, 256, 0],
            y: [0, 0, 256, 256],
        };
        let grid = CollisionGrid { section: 101, width: 1, height: 1, origin: [0.0, 0.0], cells: vec![(0, 2)], refs: vec![(0, 0xFFFF), (1, 0xFFFF)] };
        let switches = Arc::new(BarrierSwitches::new(2));
        let ground = TrackGround::new(vec![(grid, vec![face(1, 5.0), face(0, 0.0)])], 50, switches.clone());
        let boundary = DrivableBoundary { section: 101, bbox_min: Vec2::ZERO, bbox_max: Vec2::splat(32.0), points: outline.to_vec() };
        let visibility = Visibility { boundaries: vec![boundary], lists: Vec::new(), detail_limit: 50 };
        (ground, visibility, switches)
    }

    fn height(ground: &TrackGround, visibility: &Visibility, own: &[u8]) -> f32 {
        let mut q = GroundQuery { barriers: own.to_vec(), ..Default::default() };
        ground.query(visibility, &mut q, Vec3::new(16.0, 16.0, 10.0)).expect("floor").0
    }

    fn gathered(ground: &TrackGround, visibility: &Visibility, own: &[u8]) -> usize {
        let mut q = GroundQuery { barriers: own.to_vec(), ..Default::default() };
        ground.nearby(visibility, &mut q, Vec2::splat(10.0), Vec2::splat(12.0), Vec2::ZERO).len()
    }

    #[test]
    fn a_barrier_groups_faces_count_only_while_it_is_on() {
        let (ground, visibility, switches) = square();
        // Off: the height query passes over the group's floor, the gather
        // leaves it out (0x5E75D9, 0x5E1037).
        assert_eq!(height(&ground, &visibility, &[]), 0.0);
        assert_eq!(gathered(&ground, &visibility, &[]), 1);
        switches.set(1, true);
        assert_eq!(height(&ground, &visibility, &[]), 5.0);
        assert_eq!(gathered(&ground, &visibility, &[]), 2);
        switches.set(1, false);
        assert_eq!(height(&ground, &visibility, &[]), 0.0);
        // Group 0 has no switch to turn off.
        switches.set(0, false);
        assert_eq!(gathered(&ground, &visibility, &[]), 1);
    }

    #[test]
    fn a_cars_own_groups_count_for_it_and_are_off_after() {
        let (ground, visibility, switches) = square();
        // The car's list switches its groups on for its own walk
        // (0x5E7403, 0x5E58E2) ...
        assert_eq!(height(&ground, &visibility, &[1]), 5.0);
        assert_eq!(gathered(&ground, &visibility, &[1]), 2);
        // ... and off after it (0x5E76B2, 0x5E5915), even one that was on.
        assert!(!switches.is_on(1));
        switches.set(1, true);
        assert_eq!(height(&ground, &visibility, &[1]), 5.0);
        assert!(!switches.is_on(1));
        assert_eq!(height(&ground, &visibility, &[]), 0.0);
    }
}

crate::persist!(Pack { grid, polygons });
crate::persist!(Polygon { face, normal, d });
impl TrackGround {
    /// The packs as the cache stores them (the barrier switches are the
    /// world's, stored with it).
    pub(crate) fn put_packs(&self, w: &mut crate::cache::Writer) {
        use crate::cache::Persist;
        self.packs.put(w);
        self.by_section.put(w);
    }
    pub(crate) fn get_packs(r: &mut crate::cache::Reader, barriers: Arc<crate::collision::BarrierSwitches>) -> crate::cache::Got<TrackGround> {
        use crate::cache::Persist;
        Ok(TrackGround { packs: Persist::get(r)?, by_section: Persist::get(r)?, barriers })
    }
}

#[cfg(test)]
mod native_plane_precision {
    use super::*;
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn captured_polygon_planes_match_native_pc24() {
        let faces = &crate::testkit::private("docs/evidence/full-phase-inputs/ground-faces.bin");
        let mut failures = Vec::new();
        for line in crate::testkit::private_str("docs/evidence/full-phase-inputs/planes.csv").lines().skip(1) {
            let row: Vec<f32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            let id = row[0] as usize;
            let b = &faces[id*32..id*32+32];
            let short = |o| i16::from_le_bytes([b[o],b[o+1]]);
            let x = std::array::from_fn(|i|short(8+i*2));
            let y = std::array::from_fn(|i|short(16+i*2));
            let vertices = std::array::from_fn(|i|Vec3::new(x[i] as f32/8.0,y[i] as f32/8.0,
                if b[3]&4!=0 {[-2048.0,2048.0,2048.0,-2048.0][i]} else {short(4) as f32+short(24+i*2) as f32/256.0}));
            let poly = Polygon::new(CollisionFace{vertices,corners:if b[3]&16!=0 {4}else{3},surface:b[2],flags:b[3],group:b[0],bounds:b[1],x,y});
            let actual=[poly.normal.x,poly.normal.y,poly.normal.z,poly.d];
            for i in 0..4 {
                if actual[i].to_bits()!=row[i+1].to_bits() {
                    failures.push(format!("face {id} component {i}: {} ({:08x}) != {} ({:08x})",actual[i],actual[i].to_bits(),row[i+1],row[i+1].to_bits()));
                }
            }
        }
        assert!(failures.is_empty(),"{}",failures.join("; "));
    }
}
