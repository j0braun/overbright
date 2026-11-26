use std::collections::HashMap;
use std::sync::Arc;

use crate::collision::BarrierSwitches;
use crate::formats::solid::Solid;
use crate::formats::texture::Texture;

use super::*;

crate::persist!(Placement { solid, low_solid, mid_solid, transform, section, flags, cull_radius });
crate::persist!(WorldDoor { keys, transform, pieces, state });
crate::persist!(WorldLoop { id, transform, player, pieces, window, active });
crate::persist!(LoopPlayer { flags, speed, delay, range });
crate::persist!(LoopPiece {
    hash,
    placement,
    animation,
    rest,
    parent,
    length,
    frame,
    delay_time,
    pause_time,
    back,
    pause,
    stopped
});

/// How many sections the solids and the textures are split into (written
/// and read side by side).
pub const CACHE_SPLIT: usize = 8;

/// All solids' vertices and indices back to back (the indices padded to
/// a 4-byte multiple, as buffer sizes must be).
pub fn geometry(world: &World) -> (crate::cache::Bytes, crate::cache::Bytes) {
    let mut vertices: Vec<u8> = Vec::with_capacity(
        world.solids.iter().map(|s| s.vertices.len() * std::mem::size_of::<crate::formats::solid::Vertex>()).sum(),
    );
    let mut indices: Vec<u16> = Vec::with_capacity(world.solids.iter().map(|s| s.indices.len()).sum::<usize>() + 1);
    for solid in &world.solids {
        vertices.extend_from_slice(bytemuck::cast_slice(&solid.vertices));
        indices.extend_from_slice(&solid.indices);
    }
    if indices.len() % 2 == 1 {
        indices.push(0);
    }
    (vertices.into(), bytemuck::cast_slice::<u16, u8>(&indices).into())
}

impl World {
    /// The world as it stands, as cache sections: the solids and the
    /// textures (sorted by hash) in parts, and the rest. The collision
    /// triangles, built on first use, are left out; the ground shares the
    /// stored barrier switches.
    pub fn to_sections(&self) -> Vec<(String, crate::cache::Bytes)> {
        use crate::cache::{Persist, Writer};
        use rayon::prelude::*;
        let mut textures: Vec<(&u32, &Texture)> = self.textures.iter().collect();
        textures.sort_by_key(|t| *t.0);
        let solid_part = self.solids.len().div_ceil(CACHE_SPLIT).max(1);
        let texture_part = textures.len().div_ceil(CACHE_SPLIT).max(1);
        let mut jobs: Vec<(String, Box<dyn Fn(&mut Writer) + Sync + '_>)> = Vec::new();
        // Each part starts at these counts of vertices and indices into
        // "w.vertices" / "w.indices".
        let mut bases = Vec::with_capacity(CACHE_SPLIT);
        let (mut vbase, mut ibase) = (0u64, 0u64);
        for i in 0..CACHE_SPLIT {
            bases.push((vbase, ibase));
            for s in self.solids.get(i * solid_part..((i + 1) * solid_part).min(self.solids.len())).unwrap_or(&[]) {
                vbase += s.vertices.len() as u64;
                ibase += s.indices.len() as u64;
            }
        }
        for (i, &(vbase, ibase)) in bases.iter().enumerate() {
            let solids = self.solids.get(i * solid_part..((i + 1) * solid_part).min(self.solids.len())).unwrap_or(&[]);
            // The vertices and indices are left out: all solids' are the
            // sections "w.vertices" and "w.indices", in solid order (the
            // renderer's buffers, `prepared::geometry`, used as they are).
            jobs.push((
                format!("w.solids.{i}"),
                Box::new(move |w: &mut Writer| {
                    w.u64(vbase);
                    w.u64(ibase);
                    w.u64(solids.len() as u64);
                    for s in solids {
                        w.u64(s.vertices.len() as u64);
                        w.u64(s.indices.len() as u64);
                        Solid { vertices: Vec::new(), indices: Vec::new(), ..s.clone() }.put(w);
                    }
                }),
            ));
            let part = textures.get(i * texture_part..((i + 1) * texture_part).min(textures.len())).unwrap_or(&[]);
            jobs.push((
                format!("w.tex.{i}"),
                Box::new(move |w: &mut Writer| {
                    w.u64(part.len() as u64);
                    // The pixels are sections of their own (no copy here; the
                    // read maps them).
                    for (k, t) in part {
                        k.put(w);
                        Texture { data: crate::cache::Bytes::new(), ..(*t).clone() }.put(w);
                    }
                }),
            ));
        }
        jobs.push(("w.rest".into(), Box::new(|w: &mut Writer| self.put_rest(w))));
        let mut out: Vec<(String, crate::cache::Bytes)> = jobs
            .par_iter()
            .map(|(name, job)| {
                let mut w = Writer::default();
                job(&mut w);
                (name.clone(), w.buf.into())
            })
            .collect();
        let (vertices, indices) = geometry(self);
        out.push(("w.vertices".into(), vertices));
        out.push(("w.indices".into(), indices));
        out.extend(textures.iter().map(|(k, t)| (format!("w.td.{k:08x}"), t.data.clone())));
        out
    }

    /// A world from the sections [`World::to_sections`] made, or None if
    /// they do not decode.
    pub fn from_sections(file: &crate::cache::CacheFile) -> Option<World> {
        use crate::cache::Persist;
        use rayon::prelude::*;
        let names: Vec<String> =
            (0..CACHE_SPLIT).flat_map(|i| [format!("w.solids.{i}"), format!("w.tex.{i}")]).collect();
        let (vertices, indices) = (file.bytes("w.vertices")?, file.bytes("w.indices")?);
        let tdmap = crate::cache::TextureMap::read(file)?;
        // A part of the solids, their vertices and indices from the shared
        // sections.
        fn solids_part(r: &mut crate::cache::Reader, vertices: &[u8], indices: &[u8]) -> crate::cache::Got<Vec<Solid>> {
            use crate::cache::Corrupt;
            const V: usize = std::mem::size_of::<crate::formats::solid::Vertex>();
            let (mut v, mut i) = (Persist::get(r).map(|x: u64| x as usize)?, Persist::get(r).map(|x: u64| x as usize)?);
            let n = r.count(16)?;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let (nv, ni): (u64, u64) = (Persist::get(r)?, Persist::get(r)?);
                let mut s = Solid::get(r)?;
                let vb = vertices
                    .get(
                        v.checked_mul(V).ok_or(Corrupt)?
                            ..(v.checked_add(nv as usize).ok_or(Corrupt)?).checked_mul(V).ok_or(Corrupt)?,
                    )
                    .ok_or(Corrupt)?;
                let ib = indices
                    .get(
                        i.checked_mul(2).ok_or(Corrupt)?
                            ..(i.checked_add(ni as usize).ok_or(Corrupt)?).checked_mul(2).ok_or(Corrupt)?,
                    )
                    .ok_or(Corrupt)?;
                s.vertices = bytemuck::pod_collect_to_vec(vb);
                s.indices = bytemuck::pod_collect_to_vec(ib);
                v += nv as usize;
                i += ni as usize;
                out.push(s);
            }
            Ok(out)
        }
        enum Part {
            Solids(Vec<Solid>),
            Textures(Vec<(u32, Texture)>),
        }
        let (parts, rest) = rayon::join(
            || {
                names
                    .par_iter()
                    .map(|n| {
                        let mut r = file.reader(n)?;
                        let part = if n.starts_with("w.solids") {
                            Part::Solids(solids_part(&mut r, &vertices, &indices).ok()?)
                        } else {
                            Part::Textures(Persist::get(&mut r).ok()?)
                        };
                        r.done().then_some(part)
                    })
                    .collect::<Option<Vec<Part>>>()
            },
            || {
                let mut r = file.reader("w.rest")?;
                let w = World::get_rest(&mut r).ok()?;
                r.done().then_some(w)
            },
        );
        let (parts, mut world) = (parts?, rest?);
        for p in parts {
            match p {
                Part::Solids(s) => world.solids.extend(s),
                Part::Textures(t) => {
                    for (k, mut t) in t {
                        // Its own section, or (stored once) pieces of the
                        // renderer's arrays, gathered if something reads it.
                        t.data = match file.bytes(&format!("w.td.{k:08x}")) {
                            Some(b) => b,
                            None => tdmap.as_ref()?.bytes(file, k)?,
                        };
                        world.textures.insert(k, t);
                    }
                }
            }
        }
        Some(world)
    }

    /// Back to the state [`World::load`] left it in, for a load of the same
    /// track that keeps it in memory instead of reading it again: scenery
    /// groups and switches, placements, doors, the held groups, and (unless
    /// `keep_barriers`, as for a reload of the same region that keeps the
    /// switches, see [`World::copy_barriers_from`]) the barrier switches.
    /// The geometry, textures, ground and collision never change. False
    /// (and nothing changed) if the loaded state is not at hand.
    pub fn reset_to_loaded(&mut self, keep_barriers: bool) -> bool {
        if std::mem::take(&mut self.map_overlay.loaded_stale) {
            // A live re-apply changed the world since: that is the loaded state.
            self.retake_loaded();
        }
        let Some(mut fresh) = (!self.loaded.is_empty())
            .then(|| World::get_rest(&mut crate::cache::Reader::new(&self.loaded)).ok())
            .flatten()
        else {
            return false;
        };
        for g in 0..self.barriers.len().min(fresh.barriers.len()) {
            // Kept: as a fresh load with them copied over (`copy_barriers_from`
            // takes each group's `is_on`, so group 0 reads on).
            let on = if keep_barriers { self.barriers.is_on(g as u8) } else { fresh.barriers.is_on(g as u8) && g != 0 };
            self.barriers.set(g, on);
        }
        self.gps_aid = fresh.gps_aid;
        self.dyno_rollers = fresh.dyno_rollers;
        self.sections = std::mem::take(&mut fresh.sections);
        self.solid_by_hash = std::mem::take(&mut fresh.solid_by_hash);
        self.placements = std::mem::take(&mut fresh.placements);
        self.visibility = std::mem::take(&mut fresh.visibility);
        self.texture_animations = std::mem::take(&mut fresh.texture_animations);
        self.parameter_maps = std::mem::take(&mut fresh.parameter_maps);
        self.horizon_fog = std::mem::take(&mut fresh.horizon_fog);
        self.missing_solids = fresh.missing_solids;
        self.collision_faces = std::mem::take(&mut fresh.collision_faces);
        self.light_packs = std::mem::take(&mut fresh.light_packs);
        self.flare_packs = std::mem::take(&mut fresh.flare_packs);
        self.smackables = std::mem::take(&mut fresh.smackables);
        self.light_placements = Default::default();
        self.light_by_section = Default::default();
        self.scenery_groups = std::mem::take(&mut fresh.scenery_groups);
        self.barrier_groups = std::mem::take(&mut fresh.barrier_groups);
        self.links = std::mem::take(&mut fresh.links);
        self.link_placements = std::mem::take(&mut fresh.link_placements);
        self.original_keys = std::mem::take(&mut fresh.original_keys);
        self.disk_link_flags = std::mem::take(&mut fresh.disk_link_flags);
        self.disk_groups_on = std::mem::take(&mut fresh.disk_groups_on);
        self.race_only_mirrored = fresh.race_only_mirrored;
        self.held_groups = std::mem::take(&mut fresh.held_groups);
        self.changed = std::mem::take(&mut fresh.changed);
        self.emitters = std::mem::take(&mut fresh.emitters);
        self.placed_emitters = std::mem::take(&mut fresh.placed_emitters);
        self.switches = fresh.switches;
        self.world_anims = std::mem::take(&mut fresh.world_anims);
        self.doors = std::mem::take(&mut fresh.doors);
        self.loops = std::mem::take(&mut fresh.loops);
        self.anim_track = fresh.anim_track;
        true
    }

    pub fn put_rest(&self, w: &mut crate::cache::Writer) {
        use crate::cache::Persist;
        let World {
            gps_aid,
            dyno_rollers,
            sections,
            solids: _,
            solid_by_hash,
            textures: _,
            placements,
            visibility,
            texture_animations,
            parameter_maps,
            horizon_fog,
            missing_solids,
            collision: _,
            light_placements: _,
            light_by_section: _,
            lookahead: _,
            scene_lights: _,
            world_clock: _,
            collision_faces,
            ground,
            light_packs,
            flare_packs,
            smackables,
            scenery_groups,
            barrier_groups,
            barriers,
            links,
            link_placements,
            original_keys,
            disk_link_flags,
            disk_groups_on,
            race_only_mirrored,
            held_groups,
            changed,
            emitters,
            placed_emitters,
            switches,
            world_anims,
            doors,
            loops,
            anim_track,
            cache: _,
            loaded: _,
            map_overlay: _,
        } = self;
        gps_aid.put(w);
        dyno_rollers.put(w);
        sections.put(w);
        solid_by_hash.put(w);
        placements.put(w);
        visibility.put(w);
        texture_animations.put(w);
        parameter_maps.put(w);
        horizon_fog.put(w);
        missing_solids.put(w);
        collision_faces.put(w);
        barriers.as_ref().put(w);
        ground.put_packs(w);
        light_packs.put(w);
        flare_packs.put(w);
        smackables.put(w);
        scenery_groups.put(w);
        barrier_groups.put(w);
        links.put(w);
        link_placements.put(w);
        original_keys.put(w);
        disk_link_flags.put(w);
        disk_groups_on.put(w);
        race_only_mirrored.put(w);
        held_groups.put(w);
        changed.put(w);
        emitters.put(w);
        placed_emitters.put(w);
        switches.put(w);
        world_anims.put(w);
        doors.put(w);
        loops.put(w);
        anim_track.put(w);
    }

    fn get_rest(r: &mut crate::cache::Reader) -> crate::cache::Got<Self> {
        use crate::cache::Persist as P;
        let gps_aid = P::get(r)?;
        let dyno_rollers = P::get(r)?;
        let sections = P::get(r)?;
        let solids = Vec::new();
        let solid_by_hash = P::get(r)?;
        let textures = HashMap::new();
        let placements = P::get(r)?;
        let visibility = P::get(r)?;
        let texture_animations = P::get(r)?;
        let parameter_maps = P::get(r)?;
        let horizon_fog = P::get(r)?;
        let missing_solids = P::get(r)?;
        let collision_faces = P::get(r)?;
        let barriers: Arc<BarrierSwitches> = Arc::new(P::get(r)?);
        let ground = crate::ground::TrackGround::get_packs(r, barriers.clone())?;
        Ok(World {
            gps_aid,
            dyno_rollers,
            sections,
            solids,
            solid_by_hash,
            textures,
            placements,
            visibility,
            texture_animations,
            parameter_maps,
            horizon_fog,
            missing_solids,
            collision: Default::default(),
            light_placements: Default::default(),
            light_by_section: Default::default(),
            lookahead: Default::default(),
            scene_lights: Vec::new(),
            world_clock: Default::default(),
            collision_faces,
            ground,
            light_packs: P::get(r)?,
            flare_packs: P::get(r)?,
            smackables: P::get(r)?,
            scenery_groups: P::get(r)?,
            barrier_groups: P::get(r)?,
            barriers,
            links: P::get(r)?,
            link_placements: P::get(r)?,
            original_keys: P::get(r)?,
            disk_link_flags: P::get(r)?,
            disk_groups_on: P::get(r)?,
            race_only_mirrored: P::get(r)?,
            held_groups: P::get(r)?,
            changed: P::get(r)?,
            emitters: P::get(r)?,
            placed_emitters: P::get(r)?,
            switches: P::get(r)?,
            world_anims: P::get(r)?,
            doors: P::get(r)?,
            loops: P::get(r)?,
            anim_track: P::get(r)?,
            cache: None,
            loaded: Default::default(),
            map_overlay: Default::default(),
        })
    }
}
