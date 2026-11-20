//! Loading a whole track into memory: every streaming section's solids,
//! textures and scenery, resolved into a flat list of placed meshes.
//!
//! The real game streams sections around the car. Loading everything at once
//! is a deliberate simplification for now; the section each placement came
//! from is kept so streaming can be layered on later.

use std::collections::HashMap;
use std::sync::Arc;

use crate::collision::{BarrierSwitches, Collision, Source};
use crate::formats::collision::BarrierGroup;
use crate::formats::lights::{FlarePack, LightPack};
use crate::formats::parameter_map::ParameterMap;
use crate::formats::solid::Solid;
use crate::formats::texture::Texture;
use crate::formats::texture_anim::TextureAnimation;
use crate::formats::track::StreamingSection;
use crate::formats::visibility::Visibility;

mod anims;
mod cache;
mod data;
mod map_overlay;
mod parse;
mod query;
mod scenery;

pub use self::anims::*;
pub use self::data::*;
pub use self::map_overlay::{MATCH_DISTANCE, MapIndex, parts_of, transform_of};
use self::parse::*;
pub use self::scenery::*;

#[derive(Default)]
pub struct World {
    /// MARKER_GPS_AID's placement (0x5F3E20 takes the solid by its bin
    /// hash from GLOBAL/InGameCommon.bun's solid list), hidden until the
    /// GPS aid is drawn (0x5EACA0 gives it its matrix each frame).
    pub gps_aid: Option<usize>,
    /// The dyno's four rollers (XO_DYNOROLLERA_1A_01, made by the dyno
    /// manager 0x56E620 → 0x56D060 from the region's own solid): the first
    /// of four hidden placements, posed while the dyno race runs (see
    /// `nfsu2::dyno::Rollers`). None when the region has no such solid.
    pub dyno_rollers: Option<usize>,
    pub sections: Vec<StreamingSection>,
    pub solids: Vec<Solid>,
    pub solid_by_hash: HashMap<u32, usize>,
    pub textures: HashMap<u32, Texture>,
    pub placements: Vec<Placement>,
    pub visibility: Visibility,
    /// Flip-book textures by base hash.
    pub texture_animations: HashMap<u32, TextureAnimation>,
    /// Position-dependent settings (fog, …) by name hash.
    pub parameter_maps: HashMap<u32, ParameterMap>,
    /// The horizon fog's sources (chunk 0x34250, type 6).
    pub horizon_fog: Vec<crate::horizon_fog::Source>,
    /// Scenery instances whose solid could not be found.
    pub missing_solids: usize,
    /// Floors and walls for driving, built on first use (see
    /// [`World::collision`]).
    collision: std::sync::OnceLock<Collision>,
    /// The placements whose solids carry light markers (0x5D6580's flares),
    /// built on first use.
    light_placements: std::sync::OnceLock<Vec<usize>>,
    /// [`World::light_placements`] by their sections (each list ascending),
    /// built on first use from that list.
    light_by_section: std::sync::OnceLock<crate::fxhash::FxHashMap<u32, Vec<usize>>>,
    /// Keep All Loaded's look-ahead (clone deviation, built on first use).
    lookahead: LookaheadCache,
    /// The world clock 0x7FB718 (`weather::set_clock`).
    pub world_clock: crate::weather::WorldClock,
    /// The game's collision faces as triangles (stream `0x80034130` packs).
    collision_faces: Vec<Source>,
    /// The game's ground query over the collision packs.
    pub ground: crate::ground::TrackGround,
    /// Each section's light pack (0x80135000), in section order: the
    /// lights the cars are lit by.
    pub light_packs: Vec<LightPack>,
    /// The light groups of the list 0x835850 (0x48E380), which 0x48E3D0
    /// walks after the packs: the flag girl's two flare lights while a race
    /// start scene's props object lives (0x433090; see `flag_girl_lights`).
    /// Set each frame, not cached.
    pub scene_lights: Vec<crate::formats::lights::WorldLight>,
    /// Each section's flare pack (0x80135100): the street lamps' glows.
    pub flare_packs: Vec<FlarePack>,
    /// Knock-over props, each standing in for one placement.
    pub smackables: Vec<crate::smackable::Prop>,
    /// The track bundle's scenery groups (`0x34108`).
    pub scenery_groups: Vec<crate::formats::smackable::SceneryGroup>,
    /// The track bundle's barrier groups (`0x3412F`), by face group byte.
    pub barrier_groups: Vec<BarrierGroup>,
    /// Their switches, shared with the ground lookup and the collision
    /// triangles.
    pub barriers: Arc<BarrierSwitches>,
    /// The scenery-instance table (`0x34107`) as the game keeps it: an
    /// entry's flags change when a scenery group listing it is switched,
    /// and go to its instance (0x5DB0F0).
    links: Vec<crate::formats::smackable::SceneryLink>,
    /// The placement each entry stands for.
    pub link_placements: Vec<Option<usize>>,
    /// The entries' flags and the scenery groups' `on` as loaded.
    disk_link_flags: Vec<u16>,
    disk_groups_on: Vec<bool>,
    /// The race-only scenery is mirrored (a section's +9, 0x5DB4C0).
    race_only_mirrored: bool,
    /// The scenery groups the current race or free roam switched on (the
    /// track loader 0x8654A8 +0x18, +0x1C, +0x28, +0x2C), switched off when
    /// it ends (0x5815D0 → 0x5711E0).
    held_groups: Vec<usize>,
    /// Placements whose flags or transform changed since
    /// [`World::take_changed`].
    changed: Vec<usize>,

    /// The emitter descriptions (`0x35021`): GLOBALB's, then the track's.
    pub emitters: crate::particles::Library,
    /// The sections' placed emitters (chunk 0x35020, see
    /// [`crate::particles::Placed`]).
    pub placed_emitters: Vec<crate::particles::Placed>,
    /// The switches held (the list of 8 at 0x88A28C): the hashes of the
    /// names 0x571170 switched on, in the first free slots (0x5D7480);
    /// emptied when a race or free roam ends (0x5711E0) and by the
    /// collision manager's constructor (0x5E6489). A placed emitter with a
    /// switch runs only while it is here (0x5D7450).
    pub switches: [u32; 8],
    /// The track bundle's world animations (`formats::world_anim`).
    pub world_anims: crate::formats::world_anim::WorldAnimations,
    /// The shop doors among them, each slat a placement.
    pub doors: Vec<WorldDoor>,
    /// The other world animations (searchlights, planes, cranes, graffiti
    /// trains, sails), each entity a placement played by its own player.
    pub loops: Vec<WorldLoop>,
    /// The track [`World::set_track`] last set the windowed world
    /// animations for (None: as loaded, none of them made).
    pub anim_track: Option<u32>,
    /// The track's cache entry (see [`crate::cache`]): the renderer's
    /// upload-ready data comes from it, or goes into it.
    pub cache: Option<Arc<crate::cache::TrackCache>>,
    /// Everything but the solids and textures as loaded (the cache's
    /// "w.rest" section): what [`World::reset_to_loaded`] restores.
    loaded: crate::cache::Bytes,
    /// The map mods applied to this world (`tools::map`; empty without
    /// any, docs/modding/map-editor.md).
    pub map_overlay: crate::tools::map::Applied,
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use std::path::Path;

    /// World Detail Extreme: off, no far sections (native); on, the
    /// far set leaves out the main list, holds what Ultra's ring adds, and
    /// is one set per section (the same anywhere in it).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn far_sections_extend_the_ultra_set_per_section() {
        let root = crate::testkit::game_root();
        let world = World::load(Path::new(&root), "L4RA").unwrap();
        let _g = crate::qol::test_globals();
        let (_, b) = world.visibility.drivable().next().unwrap();
        let c = b.points.iter().copied().sum::<glam::Vec2>() / b.points.len() as f32;
        let at = Vec3::new(c.x, c.y, 0.0);
        let section = world.visibility.section_at(c).unwrap();
        let native = world.visibility.visible_from(section).unwrap().to_vec();
        assert!(world.far_sections_at(at, &native, crate::qol::far_draw()).is_none(), "off: native");
        crate::qol::Enhancements { far_draw: true, keep_loaded: true, detail_distance: 200, ..Default::default() }
            .apply();
        let far = world.far_sections_at(at, &native, crate::qol::far_draw()).unwrap();
        assert!(far.iter().all(|s| !native.contains(s)));
        let ultra = world.lookahead_sections(section, c, LOOKAHEAD_RING);
        let extreme = world.lookahead_sections(section, c, crate::qol::FAR_RING);
        assert!(ultra.iter().all(|s| extreme.contains(s)), "Extreme's ring holds Ultra's set");
        assert!(far.len() > ultra.len() - native.len().min(ultra.len()), "{} far sections", far.len());
        // Anywhere in the section: the same set.
        let p0 = b.points[0] * 0.9 + c * 0.1;
        if world.visibility.section_at(p0) == Some(section) {
            assert_eq!(world.far_sections_at(Vec3::new(p0.x, p0.y, 0.0), &native, crate::qol::far_draw()).unwrap(), far);
        }
    }

    /// The far set kept for the eye's section (`LookaheadCache::far`)
    /// is the one worked out afresh, over a walk through Bayview's
    /// drivable outlines that leaves and re-enters sections and stays
    /// in some (eyes anywhere in an outline, the drawn list varying).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn kept_far_sets_are_the_fresh_ones() {
        let root = crate::testkit::game_root();
        let world = World::load(Path::new(&root), "L4RA").unwrap();
        let _g = crate::qol::test_globals();
        crate::qol::Enhancements { far_draw: true, keep_loaded: true, detail_distance: 200, ..Default::default() }
            .apply();
        let fresh = |at: Vec3, drawn: &[u32]| -> Option<Vec<u32>> {
            let section = world.visibility.section_at(at.truncate())?;
            world.visibility.visible_from(section)?;
            let centre =
                world.visibility.drivable().find(|(_, b)| b.section == section).map_or(at.truncate(), |(_, b)| {
                    b.points.iter().copied().sum::<glam::Vec2>() / b.points.len().max(1) as f32
                });
            let far = world.lookahead_sections(section, centre, crate::qol::FAR_RING);
            Some(far.iter().copied().filter(|s| !drawn.contains(s)).collect())
        };
        let outlines: Vec<&crate::formats::visibility::DrivableBoundary> =
            world.visibility.drivable().map(|(_, b)| b).step_by(7).take(40).collect();
        let mut checked = 0;
        for (k, b) in outlines.iter().enumerate() {
            let c = b.points.iter().copied().sum::<glam::Vec2>() / b.points.len() as f32;
            for at in [c, b.points[0] * 0.8 + c * 0.2, c] {
                let at = Vec3::new(at.x, at.y, 0.0);
                let drawn: Vec<u32> = world
                    .visibility
                    .section_at(at.truncate())
                    .and_then(|s| world.visibility.visible_from(s))
                    .map(|l| l.iter().copied().skip(k % 3).collect())
                    .unwrap_or_default();
                assert_eq!(world.far_sections_at(at, &drawn, crate::qol::far_draw()), fresh(at, &drawn), "outline {k}");
                checked += 1;
            }
        }
        assert!(checked > 60);
    }

    /// Keep All Loaded and World Detail Extreme draw only the panoramas
    /// (x90–x99) the eye section's own list names, never one lent by
    /// another outline's list: by the Marquis cinema (section 1404) the
    /// 400 m ring lent ten, and region 5's (596, its freeway stand-in)
    /// stood as a dark streaked band across the trees; downtown (1608)
    /// the 60 m ring lent seven beside the list's own 799, one a flat
    /// slab over the buildings (as reported in the graphics issues
    /// list). Over every drivable outline of Bayview, both rings.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn look_ahead_lends_no_panorama() {
        let world = World::load(&crate::testkit::game_root(), "L4RA").unwrap();
        let _g = crate::qol::test_globals();
        crate::qol::Enhancements { far_draw: true, keep_loaded: true, detail_distance: 200, ..Default::default() }
            .apply();
        let own = |s: u32| -> Vec<u32> {
            world.visibility.visible_from(s).unwrap().iter().copied().filter(|&x| is_panorama(x)).collect()
        };
        assert_eq!(own(1608), [799]);
        let mut checked = 0;
        for (_, b) in world.visibility.drivable() {
            let c = b.points.iter().copied().sum::<glam::Vec2>() / b.points.len() as f32;
            let Some(section) = world.visibility.section_at(c) else { continue };
            if world.visibility.visible_from(section).is_none() {
                continue;
            }
            for ring in [LOOKAHEAD_RING, crate::qol::FAR_RING] {
                let set = world.lookahead_sections(section, c, ring);
                let pans: Vec<u32> = set.iter().copied().filter(|&x| is_panorama(x)).collect();
                assert_eq!(pans, own(section), "section {section}, ring {ring}");
            }
            let at = Vec3::new(c.x, c.y, 0.0);
            let drawn = world.lookahead_sections(section, c, LOOKAHEAD_RING);
            let far = world.far_sections_at(at, &drawn, crate::qol::far_draw()).unwrap();
            assert!(far.iter().all(|&s| !is_panorama(s)), "section {section}: {far:?}");
            checked += 1;
        }
        assert!(checked > 100, "{checked} outlines");
    }

    /// The look-ahead's lenders (`World::lookahead_near`, its box test
    /// before the list search) are the ones the plain filter gives — every
    /// drivable outline with a visible list within the ring, nearest first —
    /// at eyes over all of Bayview (centres, corners, between outlines),
    /// both rings.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn look_ahead_lenders_are_the_plain_filters() {
        let world = World::load(&crate::testkit::game_root(), "L4RA").unwrap();
        let vis = &world.visibility;
        let plain = |section: u32, eye: glam::Vec2, ring: f32| {
            let mut near: Vec<(f32, u32)> = vis
                .drivable()
                .map(|(_, b)| b)
                .filter(|b| b.section != section && vis.visible_from(b.section).is_some())
                .filter_map(|b| {
                    let d = b.distance_outside(eye, ring + 1.0);
                    (d <= ring).then_some((d, b.section))
                })
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            near
        };
        let bits = |v: &[(f32, u32)]| v.iter().map(|(d, s)| (d.to_bits(), *s)).collect::<Vec<_>>();
        let mut checked = 0;
        for (k, (_, b)) in vis.drivable().enumerate() {
            let c = b.points.iter().copied().sum::<glam::Vec2>() / b.points.len() as f32;
            for eye in [c, b.points[0], b.points[0] * 0.5 + c * 0.5, c + glam::Vec2::new(37.0, -23.0)] {
                let Some(section) = vis.section_at(eye) else { continue };
                for ring in [LOOKAHEAD_RING, crate::qol::FAR_RING] {
                    let near = world.lookahead_near(section, eye, ring);
                    assert_eq!(bits(&near), bits(&plain(section, eye, ring)), "outline {k}, ring {ring}");
                    checked += !near.is_empty() as usize;
                }
            }
        }
        assert!(checked > 400, "{checked} non-empty lender lists");
    }

    /// The look-ahead's two relations made per section on demand
    /// (`listed_together` from the lists naming a section, `sections_overlap`
    /// from each section's own cells) answer as the whole-map tables did:
    /// every pair (with itself) of Bayview's listed and placed sections,
    /// against the pair set of every list and the cell lists of every
    /// placement built in one pass.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn look_ahead_relations_per_section_are_the_whole_map_ones() {
        let world = World::load(&crate::testkit::game_root(), "L4RA").unwrap();
        let mut pairs = std::collections::HashSet::new();
        for l in &world.visibility.lists {
            for &x in &l.visible {
                for &y in &l.visible {
                    pairs.insert((x, y));
                }
            }
        }
        let mut cells: HashMap<u32, Vec<u64>> = HashMap::new();
        for p in &world.placements {
            let Some(solid) = world.solids.get(p.solid) else { continue };
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
            if x1 - x0 > 128 || y1 - y0 > 128 {
                continue;
            }
            let list = cells.entry(p.section).or_default();
            for x in x0..=x1 {
                for y in y0..=y1 {
                    list.push(((x as i32 as u32 as u64) << 32) | y as i32 as u32 as u64);
                }
            }
        }
        for list in cells.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let shared = |a: &[u64], b: &[u64]| a.iter().any(|c| b.binary_search(c).is_ok());
        let mut sections: Vec<u32> = world.visibility.lists.iter().flat_map(|l| l.visible.iter().copied()).collect();
        sections.extend(world.placements.iter().map(|p| p.section));
        sections.extend([0, 1, 9999]);
        sections.sort_unstable();
        sections.dedup();
        let (mut together, mut overlap) = (0, 0);
        for &a in &sections {
            for &b in &sections {
                let t = pairs.contains(&(a, b));
                assert_eq!(world.listed_together(a, b), t, "listed together {a} {b}");
                let o = match (cells.get(&a), cells.get(&b)) {
                    (Some(x), Some(y)) => shared(x, y),
                    _ => false,
                };
                assert_eq!(world.sections_overlap(a, b), o, "overlap {a} {b}");
                together += t as usize;
                overlap += o as usize;
            }
        }
        assert!(sections.len() > 200 && together > 1000 && overlap > 1000, "{} sections, {together} / {overlap}", sections.len());
    }

    /// The light placements of a section list (`light_placements_in`,
    /// the flare pools' per-frame list) are the full list filtered by the
    /// sections, in its order: every native visible list of Bayview, one
    /// with a section twice, and a few hundred sections at once (Keep All
    /// Loaded's size).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn light_placements_in_is_the_filtered_list() {
        let root = crate::testkit::game_root();
        let world = World::load(Path::new(&root), "L4RA").unwrap();
        let all = world.light_placements().to_vec();
        assert!(all.len() > 100, "{} light placements", all.len());
        let mut lists: Vec<Vec<u32>> = world.visibility.lists.iter().map(|l| l.visible.clone()).collect();
        lists.push(vec![]);
        lists.push(lists[0].iter().chain(&lists[0]).copied().collect());
        lists.push(world.visibility.lists.iter().flat_map(|l| l.visible.iter().copied()).take(400).collect());
        let mut out = Vec::new();
        for list in &lists {
            world.light_placements_in(list, &mut out);
            let want: Vec<usize> =
                all.iter().copied().filter(|&i| list.contains(&world.placements[i].section)).collect();
            assert_eq!(out, want);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_shop_doors_are_placed_shut_and_open_on_their_message() {
        let root = crate::testkit::game_root();
        let mut world = World::load(Path::new(&root), "L4RA").unwrap();
        assert_eq!(world.doors.len(), 23);
        assert!(world.doors.iter().all(|d| d.pieces.len() == 4 && d.state.length == 91.0));
        // The slats' scenery instances (at the doors, shut) are all hidden
        // (flag 0x10): only the world animation draws them.
        let slats: Vec<usize> = world.doors[0].pieces.iter().map(|&(p, ..)| world.placements[p].solid).collect();
        let first_door = world.doors[0].pieces[0].0;
        let scenery: Vec<&Placement> =
            world.placements[..first_door].iter().filter(|p| slats.contains(&p.solid)).collect();
        assert!(scenery.len() >= 80 && scenery.iter().all(|p| p.flags & HIDDEN_FLAG != 0));
        // CC_CAR_LOT_1's door (trigger 0xFFCC6A4B) at about (−712, −873).
        let lot = world.doors.iter().position(|d| d.keys[0] == 0xFFCC_6A4B).unwrap();
        let at = world.doors[lot].transform.w_axis.truncate();
        assert!((at.truncate() - glam::Vec2::new(-712.0, -873.1)).length() < 1.0);
        let shut: Vec<f32> = world.door_poses(&world.doors[lot]).iter().map(|(_, t)| t.w_axis.z).collect();
        world.door_message(0xFFCC_6A4B, 1.0, 0.0);
        let open: Vec<f32> = world.door_poses(&world.doors[lot]).iter().map(|(_, t)| t.w_axis.z).collect();
        // Shut the slats stand 1.5 m apart; open all are up at 6 m.
        assert!(
            shut.iter().zip(&open).all(|(s, o)| o - s > -0.01 && (o - at.z - 6.0).abs() < 0.1),
            "{shut:?} {open:?}"
        );
    }

    /// 0x435140 with the flags the self-playing instances use.
    #[test]
    fn a_loop_player_waits_wraps_and_ping_pongs() {
        // 0x28 | 0x80 | 0x100: forward, wrapping; no delay, a 1 s pause.
        let p = LoopPlayer { flags: 0x1A8, speed: 0.5, delay: [0.0, 1.0], range: [0, 0] };
        let mut piece = LoopPiece { length: 10.0, pause: 1.0, ..Default::default() };
        // The pause first: 1 s ≙ 30 frames at speed 0.5 → 2 s of play time.
        for _ in 0..59 {
            piece.step(&p, 1.0 / 30.0);
        }
        assert_eq!(piece.frame, 0.0);
        piece.step(&p, 1.0 / 30.0);
        piece.step(&p, 1.0 / 30.0);
        assert!(piece.frame > 0.0 && piece.frame < 1.0, "{}", piece.frame);
        // 15 frames a second: past 10 it wraps by the length and the pause
        // starts over.
        for _ in 0..20 {
            piece.step(&p, 1.0 / 30.0);
        }
        assert!(piece.frame < 1.0 && piece.pause_time < 0.5, "{} {}", piece.frame, piece.pause_time);
        // 0x10 | 0x800 without 0x08: there and back once, then stopped.
        let q = LoopPlayer { flags: 0x810, speed: 1.0, delay: [0.0, 0.0], range: [0, 0] };
        let mut piece = LoopPiece { length: 3.0, ..Default::default() };
        piece.step(&q, 0.1);
        piece.step(&q, 0.05);
        assert!(piece.back && piece.stopped && (piece.frame - 1.5).abs() < 1e-4, "{piece:?}");
        assert!(!piece.step(&q, 0.1));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn l4ra_plays_its_world_animations() {
        let root = crate::testkit::game_root();
        let mut world = World::load(Path::new(&root), "L4RA").unwrap();
        assert_eq!(world.loops.len(), 22);
        // The three vista searchlights' beams turn: speed 0.025, flags 0x28.
        let beams: Vec<usize> = world
            .loops
            .iter()
            .filter(|l| {
                l.pieces.iter().any(|p| {
                    p.placement != usize::MAX
                        && world.solids[world.placements[p.placement].solid].name == "XO_VISTA_SPOTLIGHT_1A_DW_00"
                })
            })
            .map(|l| l.pieces[0].placement)
            .collect();
        assert_eq!(beams.len(), 3);
        let before = world.placements[beams[0]].transform;
        let moved = world.step_loops(2.0);
        assert!(moved.iter().any(|&(p, t)| p == beams[0] && !t.abs_diff_eq(before, 1e-4)));
        // The length is the channel's frame count (0x6E2110).
        let l = &world.loops[0].pieces[0];
        let a = &world.world_anims.animations[&l.animation];
        assert_eq!(l.length, a.frames() as f32);
    }
}

#[cfg(test)]
mod lookahead_tests {
    use super::*;

    /// Keep All Loaded (clone deviation): a section joins the drawn set only
    /// when everything it overlaps is drawn with it by some native list;
    /// the shared sections always.
    #[test]
    fn lookahead_adds_only_what_the_lists_draw_together() {
        let set = [101, 121, 199, 2401];
        let together = |a: u32, b: u32| (a, b) == (101, 105) || (a, b) == (121, 105);
        // 105 overlaps 101 and 121, both listed with it; 199 lies apart.
        let overlap = |a: u32, b: u32| a != 199 && b != 199 && a / 100 == b / 100;
        assert_eq!(lookahead_admits(&set, 105, &together, &overlap), Ok(()));
        // 106 overlaps 101 and was never listed with it.
        assert!(lookahead_admits(&set, 106, &together, &overlap).is_err());
        // A panorama over its region's detail, never listed with it.
        let all = |_: u32, _: u32| true;
        assert_eq!(lookahead_admits(&set, 198, &|_, _| false, &all), Err("panorama and detail of one region"));
        // Another region's section lying apart: kept.
        assert_eq!(lookahead_admits(&set, 305, &|_, _| false, &|_, _| false), Ok(()));
        // Shared sections (X / Y): always.
        assert_eq!(lookahead_admits(&set, 2405, &|_, _| false, &all), Ok(()));
    }
}
