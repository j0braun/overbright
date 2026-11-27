//! Ambient Detail's street props (docs/notes/graphics/ambient-detail.md; a **clone
//! deviation**, opt-in; nothing here exists in SPEED2.EXE).
//!
//! Ported and finished from the earlier clutter prototype (`NFSU2_CLUTTER`):
//! placement is CPU-side at load and
//! **deterministic** (a SplitMix64 hash of the triangle: the same props every
//! run), from the world's own geometry and the Surface Detail material
//! classes ([`crate::surface_maps::slot_class`]): pavements are the Concrete
//! slots named SIDEWALK / CURB / KERB / PAVE / COBBLE / COURTGD; road is the
//! worldreflect Asphalt slots, of which only the **gutter** (within 1 m of a
//! pavement triangle) takes props, so nothing lands on the driving lanes or
//! the racing line. The prototype drew flat shader-made decals only; here most props
//! are **low-poly 3D meshes made in code** ([`meshes`]: cans, bottles, cups,
//! cardboard boxes, crumpled paper; leaves and newspaper as curled cards),
//! plus flat decals for oil stains and kerb-side **drain grates** (a share of
//! which steam, `ambient_detail`). Nothing comes from EA's art or a mod.
//!
//! Also here: the ground the blown debris lands on ([`Ground`]: the same
//! upward-facing road and pavement triangles in a 2D grid).

use glam::{Quat, Vec2, Vec3};

mod placement;
pub use placement::*;
mod ground;
pub use ground::*;
mod meshes;
pub use meshes::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_inside_and_out() {
        let t = [Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 2.0, 0.0)];
        assert_eq!(tri_distance(Vec2::new(0.5, 0.5), &t), 0.0);
        assert!((tri_distance(Vec2::new(-1.0, 0.5), &t) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rng_is_deterministic() {
        let (mut a, mut b) = (Rng(42), Rng(42));
        for _ in 0..100 {
            let x = a.f();
            assert_eq!(x, b.f());
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn names() {
        assert!(is_pavement("ARC_SIDEWALK_01"));
        assert!(is_pavement("ARC_CURB_A") && !is_pavement("RDP_ROADSTRIP_COBBLESTO"));
        assert!(!is_pavement("ARC_BRICK_RED"));
        assert!(is_grill("RDP_ROAD_GRILL_WH"));
        assert!(
            is_gravel("RDP_GRAVEL_A_CK") && is_gravel("RDP_SHOULDER2GRAVEL_VK") && !is_gravel("RDP_LANEA_YS_AA_KT")
        );
    }

    #[test]
    fn kinds_and_meshes() {
        for k in Kind::ALL {
            assert!((k.mesh() as usize) < MESHES);
            assert!(!(k.decal() && k.loose()));
        }
        let (v, i, ranges) = meshes();
        assert!(v.len() < u16::MAX as usize);
        for (first, count, base) in ranges {
            assert!(count > 0 && count % 3 == 0);
            let hi = i[first as usize..(first + count) as usize].iter().map(|&x| x as i32 + base).max().unwrap();
            assert!((hi as usize) < v.len());
        }
        // Lying meshes rest on z = 0 (nothing under the ground).
        assert!(v.iter().all(|v| v.pos[2] >= -1e-4 && v.normal.iter().all(|c| c.is_finite())));
    }

    #[test]
    fn ground_picks_the_surface_under() {
        let t = |z: f32| Tri {
            p: [Vec3::new(0.0, 0.0, z), Vec3::new(4.0, 0.0, z), Vec3::new(0.0, 4.0, z)],
            light: [Vec3::ONE; 3],
            section: 0,
        };
        let s = Surfaces {
            pave: vec![t(0.15)],
            road: vec![t(0.0)],
            other: vec![],
            gravel: vec![false, true],
            grills: vec![],
        };
        let g = Ground::new(&s);
        // Over both: the higher one (the pavement) unless it is above the probe.
        assert_eq!(g.at(Vec3::new(1.0, 1.0, 1.0), 0.0).map(|(z, _, l)| (z, l)), Some((0.15, false)));
        assert_eq!(g.at(Vec3::new(1.0, 1.0, 0.05), 0.0).map(|(z, _, l)| (z, l)), Some((0.0, true)));
        assert!(g.at(Vec3::new(9.0, 9.0, 1.0), 0.0).is_none());
    }

    /// Bayview's placement is the same twice, props only near pavements on
    /// the road (needs the game).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_placement_deterministic() {
        let world = crate::world::World::load(&crate::testkit::game_root(), "L4RA").unwrap();
        let s = surfaces(&world);
        let a = place(&s);
        let b = place(&s);
        assert!(a.props.len() > 10_000, "{}", a.props.len());
        assert_eq!(a.props.len(), b.props.len());
        assert!(a.props.iter().zip(&b.props).all(|(x, y)| bytemuck::bytes_of(&x.inst) == bytemuck::bytes_of(&y.inst)));
        assert!(!a.drains.is_empty() && a.drains.iter().any(|d| d.1));
        assert!(!grill_vents(&s.grills).is_empty());
        let mut kinds = std::collections::HashSet::new();
        kinds.extend(a.props.iter().map(|p| p.kind));
        assert!(kinds.len() >= 9, "{kinds:?}");
    }

    /// [`surfaces`] as it was made before the slot decisions were kept per
    /// texture: each material's class and name tests made again for every
    /// placement. The reference the kept decisions are checked against.
    fn surfaces_per_material(world: &crate::world::World) -> Surfaces {
        let mut s =
            Surfaces { pave: Vec::new(), road: Vec::new(), other: Vec::new(), gravel: Vec::new(), grills: Vec::new() };
        let mut gravel = (Vec::new(), Vec::new(), Vec::new());
        for pl in &world.placements {
            if pl.flags & 0x10 != 0 {
                continue;
            }
            let solid = &world.solids[pl.solid];
            let reflect = solid.mesh_flags & crate::formats::solid::MESH_FLAG_REFLECTIVE != 0;
            for m in &solid.materials {
                let Some(t) = solid.textures.get(m.texture_slot as usize).and_then(|h| world.textures.get(h)) else {
                    continue;
                };
                let class = crate::surface_maps::slot_class(t, reflect, false);
                let upper = t.name.to_ascii_uppercase();
                let (target, mut flags) = if is_grill(&t.name) {
                    (&mut s.grills, None)
                } else {
                    match class {
                        crate::surface_maps::Class::Concrete if is_pavement(&t.name) => {
                            (&mut s.pave, Some(&mut gravel.0))
                        }
                        crate::surface_maps::Class::Asphalt if reflect => (&mut s.road, Some(&mut gravel.1)),
                        _ if upper.starts_with("RDP_") => (&mut s.other, Some(&mut gravel.2)),
                        _ => continue,
                    }
                };
                let loose = is_gravel(&t.name);
                let idx = &solid.indices[m.first_index as usize..(m.first_index + m.index_count) as usize];
                for tri in idx.chunks_exact(3) {
                    let v: [&crate::formats::solid::Vertex; 3] = [
                        &solid.vertices[tri[0] as usize],
                        &solid.vertices[tri[1] as usize],
                        &solid.vertices[tri[2] as usize],
                    ];
                    let p = v.map(|v| pl.transform.transform_point3(Vec3::from(v.position)));
                    let n = (p[1] - p[0]).cross(p[2] - p[0]);
                    if n.length_squared() < 1e-8 || n.normalize().z.abs() < 0.9 {
                        continue;
                    }
                    let light = v.map(|v| Vec3::new(v.color[2] as f32, v.color[1] as f32, v.color[0] as f32) / 255.0);
                    target.push(Tri { p, light, section: pl.section });
                    if let Some(f) = flags.as_mut() {
                        f.push(loose);
                    }
                }
            }
        }
        s.gravel = [gravel.0, gravel.1, gravel.2].concat();
        s
    }

    /// A triangle as bits (positions, light, section).
    fn tri_bits(t: &Tri) -> ([u32; 9], [u32; 9], u32) {
        let bits = |v: &[Vec3; 3]| {
            let mut out = [0u32; 9];
            for (i, p) in v.iter().enumerate() {
                for (k, c) in p.to_array().into_iter().enumerate() {
                    out[i * 3 + k] = c.to_bits();
                }
            }
            out
        };
        (bits(&t.p), bits(&t.light), t.section)
    }

    /// The same triangles in the same order in every list, the same
    /// loose-ground flags.
    fn same_surfaces(a: &Surfaces, b: &Surfaces) -> bool {
        let list =
            |x: &[Tri], y: &[Tri]| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| tri_bits(p) == tri_bits(q));
        list(&a.pave, &b.pave)
            && list(&a.road, &b.road)
            && list(&a.other, &b.other)
            && list(&a.grills, &b.grills)
            && a.gravel == b.gravel
    }

    /// The same props (instances as bytes, kinds, sections) in the same
    /// order and the same drains.
    fn same_placement(a: &Placement, b: &Placement) -> bool {
        a.props.len() == b.props.len()
            && a.props.iter().zip(&b.props).all(|(x, y)| {
                bytemuck::bytes_of(&x.inst) == bytemuck::bytes_of(&y.inst) && x.kind == y.kind && x.section == y.section
            })
            && a.drains.len() == b.drains.len()
            && a.drains
                .iter()
                .zip(&b.drains)
                .all(|(x, y)| x.0.to_array().map(f32::to_bits) == y.0.to_array().map(f32::to_bits) && x.1 == y.1)
    }

    /// The barrier set-ups a region's scenery groups name: free roam at
    /// every career stage (with and without a car of its own, in and out
    /// of a career), and each race track it has groups for, run forward and
    /// backward.
    fn barrier_setups(world: &crate::world::World) -> Vec<crate::world::BarrierSetup> {
        let mut tracks: std::collections::BTreeSet<i32> = world
            .scenery_groups
            .iter()
            .filter_map(|g| {
                let n = g.name.as_str();
                n.strip_prefix("BARRIERS_").or_else(|| n.strip_prefix("PLAYER_BARRIERS_")).and_then(|x| x.parse().ok())
            })
            .collect();
        tracks.remove(&4000);
        let mut out = Vec::new();
        for stage in 0..=5u8 {
            for (career, owns_car) in [(true, true), (true, false), (false, true)] {
                out.push(crate::world::BarrierSetup {
                    track: 4000,
                    free_roam: true,
                    career,
                    stage,
                    reversed: false,
                    owns_car,
                });
            }
        }
        for track in tracks {
            for reversed in [false, true] {
                out.push(crate::world::BarrierSetup {
                    track,
                    free_roam: false,
                    career: true,
                    stage: 1,
                    reversed,
                    owns_car: true,
                });
            }
        }
        out
    }

    /// `world` set up for `setup` as a drive's load sets it up (from its
    /// groups as loaded): the barriers, then the track's world animations.
    /// The number of placements that changed.
    fn set_up(world: &mut crate::world::World, setup: &crate::world::BarrierSetup) -> usize {
        world.reset_groups();
        let _ = world.take_changed();
        world.set_up_barriers(setup);
        world.set_track(setup.track as u32);
        world.take_changed().len()
    }

    /// The slot decisions kept per texture give the props the decisions
    /// made per material gave: the same surface triangles in the same
    /// order and the same props, for the city as loaded, at free roam's
    /// barriers (stage 0 with a car of its own, stage 2) and at a race's
    /// (forward and backward), and on a drift track whose race switches
    /// road pieces on.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn kept_slot_decisions_give_the_same_props() {
        let root = crate::testkit::game_root();
        let free_roam = |stage, owns_car| crate::world::BarrierSetup {
            track: 4000,
            free_roam: true,
            career: true,
            stage,
            reversed: false,
            owns_car,
        };
        let race = |track, reversed| crate::world::BarrierSetup {
            track,
            free_roam: false,
            career: true,
            stage: 1,
            reversed,
            owns_car: true,
        };
        let cases: [(&str, Vec<Option<crate::world::BarrierSetup>>); 2] = [
            (
                "L4RA",
                vec![
                    None,
                    Some(free_roam(0, true)),
                    Some(free_roam(2, true)),
                    Some(race(4001, false)),
                    Some(race(4001, true)),
                ],
            ),
            ("L4RC", vec![None, Some(race(4306, false)), Some(race(4306, true))]),
        ];
        for (region, setups) in cases {
            let mut world = crate::world::World::load(&root, region).unwrap();
            for setup in setups {
                if let Some(setup) = &setup {
                    assert!(set_up(&mut world, setup) > 0, "{region} {setup:?}: nothing changed");
                }
                let old = surfaces_per_material(&world);
                let new = surfaces_with(&world, &mut SlotUses::default());
                assert!(!old.road.is_empty() && !old.other.is_empty(), "{region} {setup:?}");
                assert!(same_surfaces(&old, &new), "{region} {setup:?}: the surfaces differ");
                assert!(same_placement(&place(&old), &place(&new)), "{region} {setup:?}: the props differ");
            }
        }
    }

    /// [`SurfaceInputs`] taken from a region as loaded still match after a
    /// barrier set-up only when the set-up gives the same surfaces: at
    /// every set-up the region's scenery groups name, in every region.
    /// The city's groups (and the backward races' mirrored barriers, and
    /// the tracks' world animations) hold none of its surfaces, so its
    /// props made for the world as loaded are those of every drive there;
    /// on the drift tracks a race switches road pieces on.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn surface_inputs_follow_the_barriers() {
        let root = crate::testkit::game_root();
        for region in ["L4RA", "L4RB", "L4RC", "L4RD", "L4RF", "L4RG", "L4RH", "L4RR"] {
            let mut world = crate::world::World::load(&root, region).unwrap();
            let mut uses = SlotUses::default();
            let loaded = surfaces_with(&world, &mut uses);
            let inputs = SurfaceInputs::of(&world, &mut uses);
            assert!(inputs.same(&world), "{region}");
            let setups = barrier_setups(&world);
            let mut differ = 0;
            for setup in &setups {
                set_up(&mut world, setup);
                let now = surfaces_with(&world, &mut uses);
                let same = same_surfaces(&loaded, &now);
                assert!(same || !inputs.same(&world), "{region} {setup:?}: other surfaces, the same inputs");
                differ += usize::from(!same);
                if region == "L4RA" {
                    assert!(inputs.same(&world), "{region} {setup:?}: a city set-up changed a surface's placement");
                }
            }
            if region == "L4RC" {
                assert!(differ > 0, "{region}: no race switches a road piece");
            }
            eprintln!(
                "{region}: {} of {} placements of use; {differ} of {} set-ups give other surfaces",
                inputs.placements_of_use(),
                world.placements.len(),
                setups.len()
            );
        }
    }

    /// The Animated Water mesh a world kept for the next drive carries
    /// (`Water::for_new_drive`; built once per world, its records holding
    /// each water placement's flags and transform then) is the one built for
    /// any drive on that world: no barrier set-up a region's scenery groups
    /// name (nor a backward race's mirrored barriers, nor the track's world
    /// animations) shows, hides or moves a placement that holds water, in
    /// any region.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn water_is_the_same_at_every_barrier_set_up() {
        let root = crate::testkit::game_root();
        let mut with_water = 0;
        for region in ["L4RA", "L4RB", "L4RC", "L4RD", "L4RF", "L4RG", "L4RH", "L4RR"] {
            let mut world = crate::world::World::load(&root, region).unwrap();
            let water: std::collections::BTreeSet<usize> =
                crate::render::water::build(&world).records.iter().map(|r| r.placement).collect();
            with_water += usize::from(!water.is_empty());
            for setup in &barrier_setups(&world) {
                world.reset_groups();
                let _ = world.take_changed();
                world.set_up_barriers(setup);
                world.set_track(setup.track as u32);
                let hit: Vec<usize> = world.take_changed().into_iter().filter(|p| water.contains(p)).collect();
                assert!(hit.is_empty(), "{region} {setup:?}: water placements switched: {hit:?}");
            }
        }
        assert!(with_water >= 2, "water in {with_water} regions");
    }
}
