//! The placement guard on a loaded world (docs/modding/asset-format/world.md §2, §4.3): an original placement's key
//! `<scenery>/<index>` names the instance the stream file has there, and a merge's guard is checked against that
//! placement's drawn solid and its position before any map mod moved it. On Bayview with the game's files.

use std::collections::HashMap;
use std::path::Path;

use glam::Vec3;
use nfsu2::studio_api::world::{World, parts_of};
use ob_asset_format::Rule;
use ob_formats::formats::bundle::{self, id};
use ob_formats::formats::scenery::{ScenerySection, parse_scenery_section};
use ob_map::tools::map::format::{MapMod, PropEdit};
use ob_studio::keys::{Guard, PlacementKey, base_placement, guard_holds};

mod testkit {
    #![allow(dead_code)]
    ob_testkit::bind!();
}

const REGION: &str = "L4RA";

/// Every scenery section of the region's stream file, by number, read straight from the file.
fn scenery_of(root: &Path) -> HashMap<u32, ScenerySection> {
    let stream = std::fs::read(root.join(format!("TRACKS/STREAM{REGION}.BUN"))).unwrap();
    let mut chunks = Vec::new();
    bundle::walk(bundle::chunks(&stream, 0), &mut |c| {
        if c.id == id::SCENERY_SECTION {
            chunks.push(c);
        }
    });
    let mut by_number = HashMap::new();
    for c in chunks {
        let s = parse_scenery_section(c).unwrap();
        assert!(by_number.insert(s.section_number, s).is_none(), "two scenery sections of one number");
    }
    by_number
}

fn key(scenery: u32, index: u32) -> PlacementKey {
    PlacementKey::parse(&format!("placement:{REGION}/{scenery}/{index}")).unwrap()
}

/// Every placed instance of the stream file has its key, and the key's placement stands where the instance does,
/// with a solid the instance's LODs name; the guard made from it holds. The keys of skipped instances and of no
/// instance find nothing, and a guard there is `MRG.1`. The keys survive the asset cache and a reset.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn every_original_key_names_the_files_instance_on_bayview() {
    let root = testkit::game_root();
    let root = Path::new(&root);
    let w = World::load(root, REGION).unwrap();
    let scenery = scenery_of(root);
    let (mut placed, mut skipped) = (0usize, 0usize);
    for (&number, s) in &scenery {
        for (index, inst) in s.instances.iter().enumerate() {
            let k = key(number, index as u32);
            let Some((i, solid, at)) = base_placement(&w, &k) else {
                skipped += 1;
                continue;
            };
            placed += 1;
            assert_eq!(w.original_key(i), Some((number, index as u32)), "{k}");
            assert_eq!(at, inst.position, "{k}");
            let hash = w.solids[w.placements[i].solid].hash;
            assert!(s.infos[inst.info as usize].lod_solids.contains(&hash), "{k}: {solid}");
            guard_holds(&w, &k, &Guard { solid: solid.clone(), position: inst.position }).unwrap();
        }
    }
    // Every original placement (the ones the load makes from the scenery, before its own extras) has a key.
    let keyed = (0..w.placements.len()).filter(|&i| w.original_key(i).is_some()).count();
    assert_eq!(keyed, placed);
    assert!(placed > 10_000 && skipped > 0, "{placed} placed, {skipped} skipped");
    assert!(w.original_key(w.gps_aid.unwrap()).is_none());

    let nowhere = key(99_999, 0);
    let r = guard_holds(&w, &nowhere, &Guard { solid: "XO_CONE_01".into(), position: Vec3::ZERO }).unwrap_err();
    assert_eq!(r.rule, Rule::Mrg1);
    assert!(base_placement(&w, &PlacementKey::parse("placement:L4RA/docks:a1").unwrap()).is_none());

    // The same keys from the cache and after a reset.
    let again = World::load(root, REGION).unwrap();
    let mut reset = World::load(root, REGION).unwrap();
    assert!(reset.reset_to_loaded(false));
    for i in (0..w.placements.len()).step_by(97) {
        assert_eq!(again.original_key(i), w.original_key(i), "{i}");
        assert_eq!(reset.original_key(i), w.original_key(i), "{i}");
    }
}

/// A guard holds for the base set's solid and position, within 5 cm, also after a map mod moved the placement; a
/// different solid or a position 6 cm off is `WLD.11`.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_guard_reads_the_base_set_under_a_map_mod_on_bayview() {
    let root = testkit::game_root();
    let mut w = World::load(Path::new(&root), REGION).unwrap();
    let n = w.placements.len();
    // Placements a map mod names on their own (not one of several twins it would find first).
    let picks: Vec<usize> = (0..n)
        .step_by(n / 200)
        .filter(|&i| w.original_key(i).is_some())
        .filter(|&i| w.placement_ref(REGION, i).and_then(|r| w.find_placement(&r)) == Some(i))
        .take(20)
        .collect();
    assert_eq!(picks.len(), 20);
    let base: Vec<(PlacementKey, Guard)> = picks
        .iter()
        .map(|&i| {
            let (s, idx) = w.original_key(i).unwrap();
            let p = &w.placements[i];
            (key(s, idx), Guard { solid: w.solids[p.solid].name.clone(), position: p.transform.w_axis.truncate() })
        })
        .collect();

    let mut m = MapMod::default();
    for &i in &picks {
        let (at, rotation) = parts_of(w.placements[i].transform);
        let target = w.placement_ref(REGION, i).unwrap();
        m.props.push(PropEdit::Move { target, position: at + Vec3::new(2.0, 0.0, 0.0), rotation });
    }
    w.apply_map_mods(REGION, &[("guard-test".to_string(), m)]);

    for ((k, g), &i) in base.iter().zip(&picks) {
        assert_ne!(w.placements[i].transform.w_axis.truncate(), g.position, "{k}: the mod moved it");
        guard_holds(&w, k, g).unwrap();
        let near = Guard { position: g.position + Vec3::new(0.03, 0.03, 0.0), ..g.clone() };
        guard_holds(&w, k, &near).unwrap();
        let far = Guard { position: g.position + Vec3::new(0.06, 0.0, 0.0), ..g.clone() };
        assert_eq!(guard_holds(&w, k, &far).unwrap_err().rule, Rule::Wld11, "{k}");
        let other = Guard { solid: "XO_NOT_THIS_ONE".into(), ..g.clone() };
        assert_eq!(guard_holds(&w, k, &other).unwrap_err().rule, Rule::Wld11, "{k}");
    }
}
