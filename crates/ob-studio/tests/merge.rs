//! The keyed merge of Studio's kinds through a layer stack, with per-field attribution (layering.md §3 and §4): a
//! pack's own race, merged into and disabled by a pack that depends on it, its grid slot by slot; placements moved,
//! hidden and added by two packs.

use ob_asset_format::Rule;
use ob_asset_format::layering::{Layer, Merged, merge};
use ob_studio::merge::entries;
use ob_studio::registry::{Kind, PLACEMENT, RACE};

fn layer(kind: &Kind, unit: &str, pack: &str, deps: &[&str], files: &[&str]) -> Layer {
    let mut all = Vec::new();
    for text in files {
        all.extend(entries(kind, unit, pack, &text.parse().unwrap()).unwrap());
    }
    Layer { pack: pack.into(), dependencies: deps.iter().map(|d| d.to_string()).collect(), entries: all }
}

const SPRINT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/modding/asset-format/examples/roads-ai/race-docks-sprint.toml"
));

fn races(layers: &[Layer]) -> Merged {
    merge(Vec::new(), layers, &RACE.rules)
}

#[test]
fn a_dependent_pack_merges_into_a_race_and_disables_it() {
    let own = layer(&RACE, "", "docks-remix", &[], &[SPRINT]);
    let addon = layer(
        &RACE,
        "",
        "addon",
        &["docks-remix"],
        &["schema = \"ob.race/1.0\"\nkey = \"docks-remix:docks-sprint\"\nname = \"Docks sprint, at night\"\n\
           grid = [{ index = \"docks-remix:1\", heading_deg = 40.0 }, { op = \"add\", position = [-1209.0, 439.0, 13.5], heading_deg = 33.5 }]"],
    );
    let m = races(&[own.clone(), addon]);
    assert!(m.refused.is_empty(), "{:?}", m.refused);
    let r = m.get("docks-remix:docks-sprint").unwrap();
    assert_eq!(r.fields["name"].as_str(), Some("Docks sprint, at night"));
    assert_eq!(r.from_of("name"), Some("addon"));
    assert_eq!(r.from_of("track"), Some("docks-remix"));
    assert_eq!(r.from_of("checkpoints[2]"), Some("docks-remix"), "a list given whole");
    // The grid slot by slot: one field of the second slot changed, one slot added.
    assert_eq!(r.from_of("grid[docks-remix:1].heading_deg"), Some("addon"));
    assert_eq!(r.from_of("grid[docks-remix:1].position"), Some("docks-remix"));
    assert_eq!(r.packs_within("grid[docks-remix:1]"), ["addon", "docks-remix"]);
    // The race is docks-remix's own: its slots are docks-remix's additions (layering.md §3.2).
    assert_eq!(r.list_keys["grid"], ["docks-remix:0", "docks-remix:1", "addon:0"]);
    assert_eq!(r.packs_within("grid[addon:0]"), ["addon"]);
    assert_eq!(r.fields["grid"].as_array().unwrap().len(), 3);
    // Disabled by the dependent.
    let off = layer(
        &RACE,
        "",
        "addon",
        &["docks-remix"],
        &["schema = \"ob.race/1.0\"\nop = \"merge\"\nkey = \"docks-remix:docks-sprint\"\ndisabled = true"],
    );
    let m = races(&[own.clone(), off]);
    let r = m.get("docks-remix:docks-sprint").unwrap();
    assert!(r.disabled);
    assert_eq!((r.from_of("disabled"), r.from_of("name")), (Some("addon"), Some("docks-remix")));
    // A pack that does not depend on it may not (ORD.2); a merge of a race no pack added is MRG.1.
    let stranger =
        layer(&RACE, "", "other", &[], &["schema = \"ob.race/1.0\"\nkey = \"docks-remix:docks-sprint\"\nlaps = 1"]);
    assert_eq!(races(&[own.clone(), stranger]).refused[0].1.rule, Rule::Ord2);
    let nothing =
        layer(&RACE, "", "addon", &[], &["schema = \"ob.race/1.0\"\nkey = \"docks-remix:docks-sprint\"\nlaps = 1"]);
    assert_eq!(races(&[nothing]).refused[0].1.rule, Rule::Mrg1);
}

#[test]
fn placements_merge_by_key_with_the_pack_of_each_field() {
    let base = vec![(
        "L4RA/1407/0".to_string(),
        "solid = \"solid:XO_CONE_01\"\nposition = [-215.25, -1037.5, 146.0]\nrotation = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]"
            .parse::<toml::Table>()
            .unwrap(),
    )];
    let cones = layer(
        &PLACEMENT,
        "L4RA",
        "cone-pack",
        &[],
        &["schema = \"ob.placements/1.0\"\n\
         [[placement]]\nkey = \"1407/0\"\nguard = { solid = \"solid:XO_CONE_01\", position = [-215.25, -1037.5, 146.0] }\nposition = [-214.0, -1036.0, 146.0]\n\
         [[placement]]\nid = \"a3\"\nscenery = 1407\nsolid = \"solid:XO_CONE_01\"\nposition = [-210.0, -1030.0, 146.0]"],
    );
    let remix = layer(
        &PLACEMENT,
        "L4RA",
        "docks-remix",
        &["cone-pack"],
        &["schema = \"ob.placements/1.0\"\n\
         [[placement]]\nkey = \"1407/0\"\nguard = { solid = \"solid:XO_CONE_01\", position = [-215.25, -1037.5, 146.0] }\nposition = [-213.0, -1036.0, 146.0]\n\
         [[placement]]\nkey = \"cone-pack:a3\"\ndisabled = true"],
    );
    let m = merge(base, &[cones, remix], &PLACEMENT.rules);
    assert!(m.refused.is_empty(), "{:?}", m.refused);
    let original = m.get("L4RA/1407/0").unwrap();
    assert_eq!((original.from_of("position"), original.from_of("rotation")), (Some("docks-remix"), None));
    assert_eq!(m.clashes.len(), 1, "both moved the original");
    let added = m.get("L4RA/cone-pack:a3").unwrap();
    assert_eq!((added.from_of("solid"), added.from_of("disabled")), (Some("cone-pack"), Some("docks-remix")));
    assert!(added.disabled);
    // The guard and the key are not fields of the item.
    assert!(!original.fields.contains_key("guard") && !original.fields.contains_key("key"));
}
