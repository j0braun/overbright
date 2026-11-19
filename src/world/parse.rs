use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};

use crate::collision::{BarrierSwitches, Collision, Source};
use crate::formats::bundle::{self, id};
use crate::formats::collision::{
    BARRIER_GROUPS, COLLISION_PACK, parse_barrier_groups, parse_collision_grid, parse_collision_pack,
};
use crate::formats::hash::bin_hash;
use crate::formats::lights::{FLARE_PACK, FlarePack, LIGHT_PACK, LightPack};
use crate::formats::parameter_map::parse_parameter_maps;
use crate::formats::scenery::parse_scenery_section;
use crate::formats::smackable as smack;
use crate::formats::solid::{Solid, parse_solid_list};
use crate::formats::texture::{Texture, parse_texture_pack};
use crate::formats::texture_anim::{TEXTURE_ANIMATIONS, TextureAnimation, parse_texture_animations};
use crate::formats::track::{StreamingSection, parse_streaming_sections};
use crate::formats::visibility::Visibility;

use super::*;

/// Files outside the track that hold texture packs the world refers to, in
/// the game's region ([`crate::region`]).
pub(super) fn shared_texture_files() -> [&'static str; 5] {
    shared_texture_files_in(crate::region::get())
}

/// The same in `region`: the last is the region's textures (0x57FC5E),
/// GlobalTexturesNA.bin for region 0 (US), else ROW (the European
/// release's region 0x7FA73C is 6).
pub(super) fn shared_texture_files_in(region: u32) -> [&'static str; 5] {
    [
        "GLOBAL/GLOBALB.BUN",
        "GLOBAL/GLOBALA.BUN",
        "GLOBAL/InGameCommon.bun",
        "TRACKS/LOC4DYNTEX.BIN",
        if region == 0 { "GLOBAL/GlobalTexturesNA.bin" } else { "GLOBAL/GlobalTexturesROW.bin" },
    ]
}

/// Floors and walls from the render triangles, for a track without
/// collision packs: upward faces are floors, steep ones walls (a stand-in).
pub(super) fn render_collision(world: &World) -> Collision {
    let mut triangles = Vec::new();
    for (index, placement) in world.placements.iter().enumerate() {
        let solid = &world.solids[placement.solid];
        let section_in_region = placement.section % 100;
        // The race-only barriers stay out of this stand-in, as before they
        // were placed.
        if solid.name.starts_with("PAN_")
            || placement.section >= 2400
            || section_in_region >= 90
            || placement.flags & RACE_ONLY_FLAG != 0
        {
            continue;
        }
        for m in &solid.materials {
            let see_through = solid
                .textures
                .get(m.texture_slot as usize)
                .and_then(|h| world.textures.get(h))
                .is_some_and(|t| t.alpha_test || t.blend != crate::formats::texture::Blend::None);
            if see_through {
                continue;
            }
            let range = m.first_index as usize..(m.first_index + m.index_count) as usize;
            for tri in solid.indices[range].chunks_exact(3) {
                let corners: [Vec3; 3] = std::array::from_fn(|i| {
                    placement.transform.transform_point3(Vec3::from(solid.vertices[tri[i] as usize].position))
                });
                let n = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
                triangles.push(Source { corners, tag: index as u32, wall: n.z.abs() < 0.5, flags: 0, group: 0 });
            }
        }
    }
    let collision = Collision::new(&triangles);
    log::info!("collision: {} triangles", collision.triangle_count());
    collision
}

/// Every texture pack in a file, parsed in file order.
pub(super) fn texture_packs(file: &[u8]) -> Vec<Result<Vec<Texture>>> {
    let mut packs = Vec::new();
    bundle::walk(bundle::chunks(file, 0), &mut |c| {
        if c.id == id::TEXTURE_PACK {
            packs.push(c);
        }
    });
    packs.into_iter().map(parse_texture_pack).collect()
}

/// One streaming section's contents, as pass 1 of [`World::load`] reads
/// them (each list in the section's chunk order).
#[derive(Default)]
pub(super) struct SectionData {
    solids: Vec<Solid>,
    textures: Vec<Texture>,
    texture_animations: Vec<TextureAnimation>,
    light_packs: Vec<LightPack>,
    flare_packs: Vec<FlarePack>,
    placed_emitters: Vec<crate::particles::Placed>,
    objects: Vec<smack::ObjectInstance>,
    scenery: Vec<(u32, crate::formats::scenery::ScenerySection)>,
}

/// Whether the game loads a streaming section's range. Sections are loaded
/// only by number (0x5DACC0, fed by 0x5E5120's fixed list, the visible
/// lists and the extra numbers, each taken only when not zero), so the
/// table's first record, `--` (number 0, +0x10 = 0, a range from offset 0
/// over whole chunks of other sections), is never loaded: in L4RH and L4RR
/// its range holds Z0's scenery section (the SKYDOME), which would else be
/// placed a second time.
fn loads(section: &StreamingSection) -> bool {
    section.number != 0
}

impl SectionData {
    fn parse(stream: &[u8], section: &StreamingSection) -> Result<SectionData> {
        let mut d = SectionData::default();
        let start = section.file_offset as usize;
        let end = (start + section.size as usize).min(stream.len());
        for chunk in bundle::chunks(&stream[start..end], start) {
            match chunk.id {
                id::SOLID_LIST => {
                    d.solids.extend(parse_solid_list(chunk).with_context(|| format!("section {}", section.name))?)
                }
                id::TEXTURE_PACK => {
                    d.textures.extend(parse_texture_pack(chunk).with_context(|| format!("section {}", section.name))?)
                }
                TEXTURE_ANIMATIONS => d
                    .texture_animations
                    .extend(parse_texture_animations(chunk).with_context(|| format!("section {}", section.name))?),
                LIGHT_PACK => d.light_packs.push(LightPack::parse(chunk)),
                FLARE_PACK => d.flare_packs.extend(FlarePack::parse(chunk)),
                crate::particles::PLACED_EMITTERS => {
                    d.placed_emitters.extend(crate::particles::parse_placed(chunk.aligned(16)))
                }
                smack::OBJECT_INSTANCES => d.objects.extend(smack::parse_instances(chunk)),
                id::SCENERY_SECTION => d.scenery.push((
                    section.number,
                    parse_scenery_section(chunk).with_context(|| format!("section {}", section.name))?,
                )),
                _ => {}
            }
        }
        Ok(d)
    }
}

impl World {
    /// Load `TRACKS/<name>.BUN` + `TRACKS/STREAM<name>.BUN`, e.g. `name = "L4RA"`.
    pub fn load(game_root: &Path, name: &str) -> Result<World> {
        let _t = crate::prof::span("World::load");
        let sources = World::sources(name);
        let srcs: Vec<&str> = sources.iter().map(String::as_str).collect();
        let kind = crate::cache::TrackCache::kind(name);
        if crate::cache::dir().is_none() {
            let mut world = World::parse(game_root, name)?;
            let mut w = crate::cache::Writer::default();
            world.put_rest(&mut w);
            world.loaded = w.buf.into();
            return Ok(world);
        }
        if let Some(file) = crate::cache::CacheFile::open(crate::BUILD_ID, game_root, &kind, &srcs) {
            let _t = crate::prof::span("World::load: from the cache");
            match World::from_sections(&file) {
                Some(mut w) => {
                    w.loaded = file.bytes("w.rest").unwrap_or_default();
                    w.cache = Some(crate::cache::TrackCache::hit(crate::BUILD_ID, game_root, kind, sources, file));
                    return Ok(w);
                }
                None => log::warn!("cache {kind}: does not decode, parsing"),
            }
        }
        let mut world = World::parse(game_root, name)?;
        let sections = {
            let _t = crate::prof::span("World::load: cache sections");
            world.to_sections()
        };
        world.loaded = sections.iter().find(|s| s.0 == "w.rest").map(|s| s.1.clone()).unwrap_or_default();
        world.cache = Some(crate::cache::TrackCache::miss(crate::BUILD_ID, game_root, kind, sources, sections));
        Ok(world)
    }

    /// The game files a load of `name` reads (paths under the game's
    /// folder), which its cache is keyed on.
    pub fn sources(name: &str) -> Vec<String> {
        let mut v = vec![format!("TRACKS/{name}.BUN"), format!("TRACKS/STREAM{name}.BUN")];
        v.extend(shared_texture_files().iter().map(|s| s.to_string()));
        v
    }

    /// Parse the track from the game's files (what [`World::load`] does
    /// without a valid cache).
    pub fn parse(game_root: &Path, name: &str) -> Result<World> {
        let t_read = crate::prof::span("World::load: read");
        // The two track files and the shared texture files are read side
        // by side (each its own allocation; the reads themselves are cheap,
        // the first touch of the pages is not).
        let ((track_bun, stream), shared_files) = rayon::join(
            || {
                rayon::join(
                    || crate::game_fs::read(game_root, &format!("TRACKS/{name}.BUN")),
                    || crate::cache::SourceBytes::open(&crate::game_fs::path(game_root, &format!("TRACKS/STREAM{name}.BUN"))),
                )
            },
            || {
                use rayon::prelude::*;
                shared_texture_files().par_iter().map(|shared| crate::game_fs::read(game_root, shared)).collect::<Vec<_>>()
            },
        );
        let track_bun = track_bun.with_context(|| format!("reading {name}.BUN"))?;
        let stream = stream.with_context(|| format!("reading STREAM{name}.BUN"))?;
        drop(t_read);
        let t_head = crate::prof::span("World::load: track bundle");
        let sections = parse_streaming_sections(&track_bun)?;
        let mut world = World {
            visibility: Visibility::parse(&track_bun)?,
            parameter_maps: parse_parameter_maps(&track_bun)?,
            horizon_fog: crate::horizon_fog::parse_sources(&track_bun),
            ..Default::default()
        };
        drop(t_head);

        // Pass 1: solids and textures from every section the game loads ([`loads`]): they are
        // parsed side by side, each into lists of its own, and merged in
        // file order with the same rules as one pass in that order: the
        // first solid and texture of a hash win, the last flip-book of a
        // hash wins, the rest are appended in order.
        let t_pass1 = crate::prof::span("World::load: sections");
        let parsed: Vec<Result<SectionData>> = {
            use rayon::prelude::*;
            sections.par_iter().filter(|s| loads(s)).map(|section| SectionData::parse(&stream, section)).collect()
        };
        let mut scenery = Vec::new();
        let mut objects = Vec::new();
        for data in parsed {
            let data = data?;
            for solid in data.solids {
                world.add_solid(solid);
            }
            for texture in data.textures {
                world.textures.entry(texture.hash).or_insert(texture);
            }
            for a in data.texture_animations {
                world.texture_animations.insert(a.hash, a);
            }
            world.light_packs.extend(data.light_packs);
            world.flare_packs.extend(data.flare_packs);
            world.placed_emitters.extend(data.placed_emitters);
            objects.extend(data.objects);
            scenery.extend(data.scenery);
        }

        drop(t_pass1);
        world.sections = sections;
        let t_coll = crate::prof::span("World::load: collision");

        // Barrier groups: race and career-stage barriers. Every face is
        // kept; a face of a group other than 0 counts while its group is
        // on. All are off once the track is in (0x5D8360); a race or free
        // roam switches its own on (`set_up_barriers`).
        world.barrier_groups = bundle::chunks(&track_bun, 0)
            .find(|c| c.id == BARRIER_GROUPS)
            .map(parse_barrier_groups)
            .unwrap_or_default();
        world.barriers = Arc::new(BarrierSwitches::new(world.barrier_groups.len()));

        // Collision packs sit at the top level of the stream file (parsed
        // side by side, merged in file order).
        let pack_chunks: Vec<_> = bundle::chunks(&stream, 0).filter(|c| c.id == COLLISION_PACK).collect();
        let parsed_packs: Vec<_> = {
            use rayon::prelude::*;
            pack_chunks
                .into_par_iter()
                .map(|chunk| (parse_collision_pack(chunk), parse_collision_grid(chunk)))
                .collect()
        };
        let mut packs = Vec::new();
        for ((section, faces), grid) in parsed_packs {
            for f in &faces {
                let fans: &[[usize; 3]] = if f.corners == 4 { &[[0, 1, 2], [0, 2, 3]] } else { &[[0, 1, 2]] };
                for t in fans {
                    world.collision_faces.push(Source {
                        corners: t.map(|i| f.vertices[i]),
                        tag: section.unwrap_or(0) as u32,
                        wall: f.is_wall(),
                        flags: f.flags,
                        group: f.group,
                    });
                }
            }
            if let Some(grid) = grid {
                packs.push((grid, faces));
            }
        }
        world.ground = crate::ground::TrackGround::new(packs, world.visibility.detail_limit, world.barriers.clone());

        drop(t_coll);
        let t_shared = crate::prof::span("World::load: shared textures");
        // Textures the track shares with the rest of the game. Track textures
        // win over these when a hash appears in both. (Parsed side by side,
        // merged in the listed order.)
        let shared_packs: Vec<_> = {
            use rayon::prelude::*;
            shared_texture_files()
                .par_iter()
                .zip(&shared_files)
                .map(|(shared, bytes)| {
                    let bytes = bytes.as_ref().ok()?;
                    let emitters =
                        (*shared == "GLOBAL/GLOBALB.BUN").then(|| crate::particles::parse_descriptions(bytes));
                    Some((emitters, texture_packs(bytes)))
                })
                .collect()
        };
        for ((shared, bytes), parsed) in shared_texture_files().iter().zip(&shared_files).zip(shared_packs) {
            match (bytes, parsed) {
                (Ok(_), Some((emitters, packs))) => {
                    if let Some(e) = emitters {
                        world.emitters.descriptions.extend(e);
                        // GLOBALB's emitter records: the car effects'
                        // templates (0x48B8B0).
                        if let Ok(b) = bytes {
                            world.emitters.add_templates(b);
                        }
                    }
                    world.add_parsed_texture_packs(packs, shared);
                }
                (Err(e), _) => log::warn!("{shared}: {e}"),
                _ => {}
            }
        }
        drop(shared_files);

        // The track's own emitter descriptions follow GLOBALB's (0x491C70
        // appends each loaded chunk to 0x830D08).
        world.emitters.add(&track_bun);

        drop(t_shared);
        let _t_rest = crate::prof::span("World::load: placements, props, doors");
        // Pass 2: place every scenery instance at its highest available LOD.
        // (section, index in the section) → placement, for the props.
        let mut by_instance = HashMap::new();
        for (section, group) in scenery {
            for (index, inst) in group.instances.iter().enumerate() {
                let Some(info) = group.infos.get(inst.info as usize) else { continue };
                let solid = info.lod_solids.iter().find_map(|h| world.solid_by_hash.get(h).copied());
                let Some(solid) = solid else {
                    world.missing_solids += 1;
                    continue;
                };
                if world.solids[solid].indices.is_empty() {
                    continue; // placeholder with no geometry
                }
                let transform = Mat4::from_cols(
                    inst.rotation.x_axis.extend(0.0),
                    inst.rotation.y_axis.extend(0.0),
                    inst.rotation.z_axis.extend(0.0),
                    inst.position.extend(1.0),
                );
                let low = if inst.flags & 0x0400 != 0 { 1 } else { 2 };
                let low_solid = world
                    .solid_by_hash
                    .get(&info.lod_solids[low])
                    .copied()
                    .filter(|&s| !world.solids[s].indices.is_empty());
                by_instance.insert((group.section_number as u32, index), world.placements.len());
                let mid_solid = world
                    .solid_by_hash
                    .get(&info.lod_solids[1])
                    .copied()
                    .filter(|&s| !world.solids[s].indices.is_empty());
                world.placements.push(Placement {
                    solid,
                    low_solid,
                    mid_solid,
                    transform,
                    section,
                    flags: inst.flags,
                    cull_radius: info.radius + 6.0,
                });
            }
        }

        // Knock-over props: the infos, their collision volumes and the
        // scenery-instance table in the track bundle, the instances in the
        // sections; each stands in for a scenery placement.
        let mut infos = Vec::new();
        let mut links = Vec::new();
        let mut volumes = Vec::new();
        bundle::walk(bundle::chunks(&track_bun, 0), &mut |c| match c.id {
            smack::OBJECT_INFOS => infos.extend(smack::parse_infos(c)),
            smack::SCENERY_LINKS => links = smack::parse_scenery_links(c),
            smack::SCENERY_GROUPS => world.scenery_groups = smack::parse_scenery_groups(c),
            crate::formats::collision::COLLISION_VOLUMES => {
                volumes.extend(crate::formats::collision::parse_collision_volumes(c))
            }
            _ => {}
        });
        let placement_of = |scenery: u16| {
            let link = links.get(scenery as usize)?;
            by_instance.get(&(link.section as u32, link.instance as usize)).map(|&p| (p, link.kind))
        };
        // The props pair with the groups as loaded (all off on L4RA, and
        // no group but `SMOKEABLE` lists a prop's entry there).
        let groups = world.scenery_groups.clone();
        world.smackables = crate::smackable::Prop::pair(&world, &infos, &objects, &volumes, placement_of, &groups);
        world.link_placements =
            links.iter().map(|l| by_instance.get(&(l.section as u32, l.instance as usize)).copied()).collect();
        world.disk_link_flags = links.iter().map(|l| l.instance_flags).collect();
        world.disk_groups_on = world.scenery_groups.iter().map(|g| g.on).collect();
        world.links = links;
        world.add_doors(&track_bun);
        world.add_loops();
        world.add_gps_aid(game_root);
        world.add_dyno_rollers();
        Ok(world)
    }

    fn add_parsed_texture_packs(&mut self, packs: Vec<Result<Vec<Texture>>>, what: &str) {
        for pack in packs {
            match pack {
                Ok(textures) => {
                    for t in textures {
                        self.textures.entry(t.hash).or_insert(t);
                    }
                }
                Err(e) => log::warn!("{what}: texture pack skipped: {e:#}"),
            }
        }
    }

    /// MARKER_GPS_AID from GLOBAL/InGameCommon.bun (0x5F3E20), placed in
    /// the always-drawn section and hidden.
    fn add_gps_aid(&mut self, game_root: &Path) {
        let Ok(bytes) = crate::game_fs::read(game_root, "GLOBAL/InGameCommon.bun") else { return };
        let mut lists = Vec::new();
        bundle::walk(bundle::chunks(&bytes, 0), &mut |c| {
            if c.id == id::SOLID_LIST {
                lists.push(c);
            }
        });
        let hash = bin_hash("MARKER_GPS_AID");
        for list in lists {
            let Ok(solids) = parse_solid_list(list) else { continue };
            for solid in solids.into_iter().filter(|s| s.hash == hash) {
                let radius = solid.vertices.iter().map(|v| Vec3::from(v.position).length()).fold(0.0f32, f32::max);
                self.add_solid(solid);
                let solid = self.solid_by_hash[&hash];
                self.gps_aid = Some(self.placements.len());
                self.placements.push(Placement {
                    solid,
                    low_solid: None,
                    mid_solid: Some(solid),
                    transform: Mat4::IDENTITY,
                    section: WORLD_ANIM_SECTION,
                    flags: HIDDEN_FLAG,
                    cull_radius: radius + 6.0,
                });
                return;
            }
        }
        log::warn!("MARKER_GPS_AID not found in InGameCommon.bun");
    }

    /// The dyno rollers' placements (see [`World::dyno_rollers`]): four
    /// of the region's XO_DYNOROLLERA_1A_01 (0x56D060: the model by its
    /// name hash, 0x637DC0), in the always-drawn section and hidden.
    fn add_dyno_rollers(&mut self) {
        let Some(&solid) = self.solid_by_hash.get(&bin_hash(crate::dyno::ROLLER_MODEL)) else { return };
        if self.solids[solid].indices.is_empty() {
            return;
        }
        let radius = self.solids[solid].vertices.iter().map(|v| Vec3::from(v.position).length()).fold(0.0f32, f32::max);
        self.dyno_rollers = Some(self.placements.len());
        for _ in 0..4 {
            self.placements.push(Placement {
                solid,
                low_solid: None,
                mid_solid: Some(solid),
                transform: Mat4::IDENTITY,
                section: WORLD_ANIM_SECTION,
                flags: HIDDEN_FLAG,
                cull_radius: radius + 6.0,
            });
        }
    }

    fn add_solid(&mut self, solid: Solid) {
        if self.solid_by_hash.contains_key(&solid.hash) {
            return;
        }
        self.solid_by_hash.insert(solid.hash, self.solids.len());
        self.solids.push(solid);
    }
}

#[cfg(test)]
mod region_tests {
    /// 0x57FC5E: the NA texture pack for region 0, ROW for any other.
    #[test]
    fn shared_textures_by_region() {
        assert_eq!(super::shared_texture_files_in(0)[4], "GLOBAL/GlobalTexturesNA.bin");
        assert_eq!(super::shared_texture_files_in(6)[4], "GLOBAL/GlobalTexturesROW.bin");
        assert_eq!(super::shared_texture_files_in(0)[..4], super::shared_texture_files_in(6)[..4]);
    }
}

#[cfg(test)]
mod unloaded_record_tests {
    use super::*;

    /// The table's `--` record (number 0) is not loaded: in L4RH and L4RR,
    /// whose `--` range holds Z0's scenery section, the SKYDOME is placed
    /// once, under 2600, and no placement is under section 0.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_dash_record_is_not_loaded() {
        let root = crate::testkit::game_root();
        for region in ["L4RH", "L4RR"] {
            let world = World::load(Path::new(&root), region).unwrap();
            assert_eq!(world.sections.first().map(|s| (s.name.as_str(), s.number)), Some(("--", 0)), "{region}");
            assert!(world.placements.iter().all(|p| p.section != 0), "{region}: a placement under section 0");
            let dome = world.solid_named("SKYDOME").unwrap();
            let domes: Vec<u32> = world.placements.iter().filter(|p| p.solid == dome).map(|p| p.section).collect();
            assert_eq!(domes, [2600], "{region}");
        }
    }

    /// No region's visible-section list names section 0, the only way
    /// besides the fixed and extra numbers (never 0) a section is loaded.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn no_visible_list_names_section_zero() {
        let root = crate::testkit::game_root();
        for region in crate::upscale_pack::ALL_TRACKS {
            let bun = crate::game_fs::read(&root, &format!("TRACKS/{region}.BUN")).unwrap();
            let v = Visibility::parse(&bun).unwrap();
            assert!(!v.lists.is_empty(), "{region}");
            assert!(v.lists.iter().all(|l| !l.visible.contains(&0)), "{region}");
            let first = parse_streaming_sections(&bun).unwrap().into_iter().next().unwrap();
            assert_eq!((first.name.as_str(), first.number), ("--", 0), "{region}");
        }
    }
}
