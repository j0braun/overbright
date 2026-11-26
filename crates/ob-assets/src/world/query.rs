use std::collections::HashMap;
use std::sync::Arc;

use glam::Vec3;

use crate::collision::{Collision, Source};
use crate::formats::solid::Solid;
use crate::formats::texture::Texture;

use super::*;

impl World {
    /// The sections the game would draw with the player at `position`.
    /// The placements whose main-view solids (LOD A or B) have light
    /// markers (see `glow::Pools::add_markers`).
    pub fn light_placements(&self) -> &[usize] {
        self.light_placements.get_or_init(|| {
            let has = |s: Option<usize>| s.is_some_and(|s| !self.solids[s].light_markers.is_empty());
            let props: std::collections::HashSet<usize> = self.smackables.iter().map(|p| p.placement).collect();
            (0..self.placements.len())
                .filter(|&i| {
                    !props.contains(&i) && (has(Some(self.placements[i].solid)) || has(self.placements[i].mid_solid))
                })
                .collect()
        })
    }

    /// The [`World::light_placements`] in the sections `sections` lists,
    /// into `out` (cleared first) in the same ascending order: the list
    /// filtered by `sections.contains(&placement.section)`, without a test
    /// per light.
    pub fn light_placements_in(&self, sections: &[u32], out: &mut Vec<usize>) {
        out.clear();
        let by_section = self.light_by_section.get_or_init(|| {
            let mut m: crate::fxhash::FxHashMap<u32, Vec<usize>> = Default::default();
            for &i in self.light_placements() {
                m.entry(self.placements[i].section).or_default().push(i);
            }
            m
        });
        for s in sections {
            if let Some(l) = by_section.get(s) {
                out.extend_from_slice(l);
            }
        }
        // A section listed twice adds its lights twice: once each.
        out.sort_unstable();
        out.dedup();
    }

    /// The sections the main view draws with the eye at `position`: the
    /// native list ([`World::visible_sections_at`]), or with Keep All
    /// Loaded (clone deviation) that list widened by
    /// [`World::lookahead_sections`]. `keep_loaded`: Keep All Loaded as
    /// the moment that asks took it (a frame's or a tick's settings).
    pub fn drawn_sections_at(&self, position: Vec3, keep_loaded: bool) -> Option<(u32, std::borrow::Cow<'_, [u32]>)> {
        let (section, list) = self.visible_sections_at(position)?;
        if !keep_loaded {
            return Some((section, std::borrow::Cow::Borrowed(list)));
        }
        Some((
            section,
            std::borrow::Cow::Owned(self.lookahead_sections(section, position.truncate(), LOOKAHEAD_RING).to_vec()),
        ))
    }

    /// Whether some native visible list holds both sections.
    pub fn listed_together(&self, a: u32, b: u32) -> bool {
        let lists_of = self.lookahead.lists_of.get_or_init(|| {
            let mut of: HashMap<u32, Vec<usize>> = HashMap::new();
            for (i, l) in self.visibility.lists.iter().enumerate() {
                for &x in &l.visible {
                    let v = of.entry(x).or_default();
                    if v.last() != Some(&i) {
                        v.push(i);
                    }
                }
            }
            of
        });
        lists_of.get(&a).is_some_and(|ls| ls.iter().any(|&i| self.visibility.lists[i].visible.contains(&b)))
    }

    /// The ground cells ([`LOOKAHEAD_CELL`]) section `s`'s placements'
    /// boxes cover (the solids as placed), sorted; None when it has none.
    fn section_cells(&self, s: u32) -> Option<Arc<[u64]>> {
        let mut cells = self.lookahead.cells.lock().unwrap();
        if let Some(c) = cells.get(&s) {
            return (!c.is_empty()).then(|| c.clone());
        }
        let placements_of = self.lookahead.placements_of.get_or_init(|| {
            let mut of: HashMap<u32, Vec<usize>> = HashMap::new();
            for (i, p) in self.placements.iter().enumerate() {
                of.entry(p.section).or_default().push(i);
            }
            of
        });
        let mut list = Vec::new();
        for &i in placements_of.get(&s).map_or(&[][..], Vec::as_slice) {
            let p = &self.placements[i];
            let Some(solid) = self.solids.get(p.solid) else { continue };
            let (lo, hi) = (Vec3::from(solid.bbox_min), Vec3::from(solid.bbox_max));
            if !(lo.cmple(hi).all()) {
                continue;
            }
            let (mut min, mut max) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
            for i in 0..8 {
                let c = Vec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                );
                let w = p.transform.transform_point3(c).truncate();
                min = min.min(w);
                max = max.max(w);
            }
            let (x0, y0) = ((min.x / LOOKAHEAD_CELL).floor() as i64, (min.y / LOOKAHEAD_CELL).floor() as i64);
            let (x1, y1) = ((max.x / LOOKAHEAD_CELL).floor() as i64, (max.y / LOOKAHEAD_CELL).floor() as i64);
            // A box wider than 2 km is not a place (a sky or a stray).
            if x1 - x0 > 128 || y1 - y0 > 128 {
                continue;
            }
            for x in x0..=x1 {
                for y in y0..=y1 {
                    list.push(((x as i32 as u32 as u64) << 32) | y as i32 as u32 as u64);
                }
            }
        }
        list.sort_unstable();
        list.dedup();
        let list: Arc<[u64]> = list.into();
        cells.insert(s, list.clone());
        (!list.is_empty()).then_some(list)
    }

    /// Whether two sections' placements share a ground cell
    /// ([`LOOKAHEAD_CELL`]; boxes of the solids as placed).
    pub fn sections_overlap(&self, a: u32, b: u32) -> bool {
        let (Some(a), Some(b)) = (self.section_cells(a), self.section_cells(b)) else { return false };
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }

    /// The drivable outlines other than `section`'s with a visible list
    /// within `ring` metres of `eye`, nearest first (ties by section): the
    /// look-ahead's lenders and its cache key. The outline's box test (in
    /// `distance_outside`) runs before the list search, so the lists are
    /// searched for the few outlines near the eye, not for all of them.
    pub fn lookahead_near(&self, section: u32, eye: glam::Vec2, ring: f32) -> Vec<(f32, u32)> {
        let vis = &self.visibility;
        let mut near: Vec<(f32, u32)> = vis
            .drivable()
            .map(|(_, b)| b)
            .filter(|b| b.section != section)
            .filter_map(|b| {
                let d = b.distance_outside(eye, ring + 1.0);
                (d <= ring && vis.visible_from(b.section).is_some()).then_some((d, b.section))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        near
    }

    /// Keep All Loaded's look-ahead (clone deviation): the eye section's
    /// list, then, outline by outline from the nearest, the lists of the
    /// drivable outlines within `ring` metres of `eye` — what the native
    /// list would add when the eye crossed into them, so it is drawn
    /// before — each section only when [`lookahead_admits`] it beside the
    /// set so far. Cached by the eye's section and the lending outlines.
    pub fn lookahead_sections(&self, section: u32, eye: glam::Vec2, ring: f32) -> Arc<[u32]> {
        let vis = &self.visibility;
        let near = self.lookahead_near(section, eye, ring);
        let key: Vec<u32> = std::iter::once(section).chain(near.iter().map(|n| n.1)).collect();
        if let Some(set) = self.lookahead.sets.lock().unwrap().get(&key) {
            return set.clone();
        }
        let mut set: Vec<u32> = vis.visible_from(section).map(<[u32]>::to_vec).unwrap_or_default();
        let together = |a: u32, b: u32| self.listed_together(a, b);
        let overlap = |a: u32, b: u32| self.sections_overlap(a, b);
        for &(_, s) in &near {
            for &x in vis.visible_from(s).unwrap_or_default() {
                // A region's panorama (x90–x99) is a stand-in built for the
                // outlines whose own lists name it: lent from another
                // outline it stands in front of the scene (seen as a
                // flat, streaked slab over the buildings and trees). The
                // eye section's own panoramas are in the set already.
                if is_panorama(x) {
                    continue;
                }
                if !set.contains(&x) && lookahead_admits(&set, x, &together, &overlap).is_ok() {
                    set.push(x);
                }
            }
        }
        let set: Arc<[u32]> = set.into();
        let mut sets = self.lookahead.sets.lock().unwrap();
        // A bound on the cache (the sets are small; a free roam meets a few
        // thousand keys at most).
        if sets.len() > 8192 {
            sets.clear();
        }
        sets.insert(key, set.clone());
        set
    }

    /// World Detail Extreme (clone deviation, docs/notes/graphics/assets-distance.md):
    /// the sections the look-ahead adds within [`crate::qol::FAR_RING`] of
    /// the eye beyond `drawn` (the main list), which the main view draws
    /// with their lowest LOD. None when the option is off (`far_draw`: as
    /// the moment that asks took it, a frame's or a tick's settings).
    pub fn far_sections_at(&self, position: Vec3, drawn: &[u32], far_draw: bool) -> Option<Vec<u32>> {
        if !far_draw {
            return None;
        }
        let section = self.visibility.section_at(position.truncate())?;
        self.visibility.visible_from(section)?;
        // Measured from the eye section's outline centre, not the eye: one
        // set per section (the look-ahead caches it), computed once when
        // the section is entered (~2 ms cold) instead of whenever the
        // outlines within the ring change.
        let cached = self.lookahead.far.lock().unwrap().as_ref().filter(|(s, _)| *s == section).map(|(_, f)| f.clone());
        let far = match cached {
            Some(f) => f,
            None => {
                // A section without a drivable outline measures from the
                // eye: not the section's alone, so not kept.
                let outline = self.visibility.drivable().find(|(_, b)| b.section == section);
                let centre = outline.map_or(position.truncate(), |(_, b)| {
                    b.points.iter().copied().sum::<glam::Vec2>() / b.points.len().max(1) as f32
                });
                let f = self.lookahead_sections(section, centre, crate::qol::FAR_RING);
                if outline.is_some() {
                    *self.lookahead.far.lock().unwrap() = Some((section, f.clone()));
                }
                f
            }
        };
        Some(far.iter().copied().filter(|s| !drawn.contains(s)).collect())
    }

    pub fn visible_sections_at(&self, position: Vec3) -> Option<(u32, &[u32])> {
        let section = self.visibility.section_at(position.truncate())?;
        Some((section, self.visibility.visible_from(section)?))
    }

    /// The sections whose light packs 0x48E3D0 walks for a camera at
    /// `position` (**verified** for the list, the stand-in
    /// **inferred**): natively the list 0x830D14 holds the packs of the
    /// resident (streamed-in) chunks — the loader 0x48E070 links a pack in
    /// as its chunk loads, 0x48E270 unlinks it — and the camera plays no
    /// part. The clone stands the camera section's visible list in for the
    /// resident set, as the world draw does ([`World::visible_sections_at`]
    /// takes the nearest section when the camera is outside every
    /// outline). A world without visibility lists has none — never every
    /// pack of the world (that pulls in section 121's 9999.99 m moon
    /// light).
    pub fn light_pack_sections(&self, position: Vec3) -> &[u32] {
        self.visible_sections_at(position).map_or(&[], |(_, list)| list)
    }

    /// Height of the highest walkable surface directly below `p`.
    pub fn road_height_below(&self, p: Vec3) -> Option<f32> {
        self.road_below(p).map(|(z, _)| z)
    }

    /// The ground a start-grid marker is put on (0x5EB5E0 → 0x5E72F0):
    /// the highest floor at or below the marker's height + 1 m, or, with
    /// none there, the highest floor at its x/y at any height (some
    /// tracks' markers lie under the map: 4702's at −20 m, the road at
    /// 38 m). The marker's own height when there is no floor at all.
    pub fn grid_height(&self, at: Vec3) -> f32 {
        self.grid_ground(at).unwrap_or(at.z)
    }

    /// The same query (0x5E72F0) with its found flag: `None` when there is
    /// no floor at the point's x/y at all.
    pub fn grid_ground(&self, at: Vec3) -> Option<f32> {
        self.road_height_below(at + Vec3::Z).or_else(|| self.road_height_below(Vec3::new(at.x, at.y, 1.0e6)))
    }

    /// Like `road_height_below`, with the surface's upward normal.
    pub fn road_below(&self, p: Vec3) -> Option<(f32, Vec3)> {
        self.collision().ground_below(p)
    }

    /// The ground under a wheel at `p` for one car, as the game finds it
    /// ([`crate::ground::TrackGround::query`]): height, normal, surface.
    /// Without collision packs, the render triangles' floors stand in.
    pub fn ground_for(&self, query: &mut crate::ground::GroundQuery, p: Vec3) -> Option<(f32, Vec3, u8)> {
        if self.ground.is_empty() {
            return self.collision().ground_at(p).map(|(z, n)| (z, n, 0));
        }
        self.ground.query(&self.visibility, query, p)
    }

    /// A world of loose models placed by hand (the front end's platform
    /// scenes, `garage::Platform`): no sections, visibility or collision.
    pub fn from_models(solids: Vec<Solid>, textures: HashMap<u32, Texture>, placements: Vec<Placement>) -> World {
        let solid_by_hash = solids.iter().enumerate().map(|(i, s)| (s.hash, i)).collect();
        World { solids, solid_by_hash, textures, placements, ..World::default() }
    }

    /// A world that is only a flat floor at z = 0, `half` metres each way
    /// from the origin (for driving a car with nothing around it).
    pub fn flat(half: f32) -> World {
        let c = [
            Vec3::new(-half, -half, 0.0),
            Vec3::new(half, -half, 0.0),
            Vec3::new(half, half, 0.0),
            Vec3::new(-half, half, 0.0),
        ];
        let floor = |corners| Source { corners, tag: 0, wall: false, flags: 0, group: 0 };
        World { collision_faces: vec![floor([c[0], c[1], c[2]]), floor([c[0], c[2], c[3]])], ..World::default() }
    }

    /// [`World::collision`] made now, off the frame that would first ask for
    /// it (a world loaded ahead in the menus: the drive's placing made it,
    /// 16–20 ms of the main thread in the region's first frame). Only from
    /// the game's collision packs, which a load fixes for good (the barrier
    /// switches are read through the shared [`crate::collision::BarrierSwitches`] when
    /// queried); a track without packs builds its stand-in from the
    /// placements, which a drive's set-up may still change, so it keeps
    /// making it on first use.
    pub fn collision_ahead(&self) {
        if !self.collision_faces.is_empty() {
            let _t = crate::prof::span("collision ahead");
            let _ = self.collision();
        }
    }

    /// Floors and walls (built from the render triangles on first use).
    /// Whether the world has any floor (a world that was not read has none: the map tools then leave heights
    /// as given and skip the checks that need a floor).
    pub fn has_floors(&self) -> bool {
        self.collision().triangle_count() > 0
    }

    pub fn collision(&self) -> &Collision {
        self.collision.get_or_init(|| {
            if self.collision_faces.is_empty() {
                render_collision(self)
            } else {
                let c = Collision::new(&self.collision_faces).with_barriers(self.barriers.clone());
                log::info!("collision: {} triangles from the game's collision packs", c.triangle_count());
                c
            }
        })
    }

    /// Where a point moving from `from` to `to` first crosses a wall: the
    /// fraction of the way and the wall normal facing back towards `from`.
    pub fn wall_hit(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        self.collision().wall_hit(from, to)
    }

    /// Axis-aligned bounds of everything placed, in game coordinates.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for p in &self.placements {
            let s = &self.solids[p.solid];
            for corner in [s.bbox_min, s.bbox_max] {
                let w = p.transform.transform_point3(Vec3::from(corner));
                lo = lo.min(w);
                hi = hi.max(w);
            }
        }
        (lo, hi)
    }
}

#[cfg(test)]
mod light_pack_tests {
    use super::*;

    /// 0x48E3D0 walks the resident packs (list 0x830D14),
    /// never every pack of the world; with no visibility lists to stand in
    /// for the resident set, none.
    #[test]
    fn no_visibility_means_no_light_packs_not_all() {
        let world = World::default();
        assert!(world.light_pack_sections(Vec3::new(10.0, 20.0, 0.0)).is_empty());
    }
}

#[cfg(test)]
mod collision_ahead_tests {
    use super::*;

    /// A world with collision packs made ahead answers its ground queries
    /// as one made on first use, on another thread as on this one; a world
    /// without packs (the stand-in from its placements) is left to its
    /// first use.
    #[test]
    fn the_collision_made_ahead_is_the_one_made_on_first_use() {
        let lazy = World::flat(50.0);
        let ahead = World::flat(50.0);
        std::thread::scope(|s| s.spawn(|| ahead.collision_ahead()).join().unwrap());
        assert!(ahead.collision.get().is_some(), "made ahead");
        assert!(lazy.collision.get().is_none());
        for (x, y) in [(0.0, 0.0), (10.5, -3.25), (49.0, 49.0), (60.0, 0.0)] {
            let p = Vec3::new(x, y, 5.0);
            assert_eq!(ahead.road_below(p), lazy.road_below(p), "{p}");
            assert_eq!(ahead.collision().triangle_count(), lazy.collision().triangle_count());
        }
        let models = World::default();
        models.collision_ahead();
        assert!(models.collision.get().is_none(), "no packs: left to the first use");
    }
}
