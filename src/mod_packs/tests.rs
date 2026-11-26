//! Mod packs in the game: export, import, mounting in the player's order, clashes, refusal for another base, and the
//! native default (nothing switched on: nothing mounted).

use glam::{Mat3, Vec3};
use ob_asset_format::modpack::{self, BaseId};

use super::*;
use crate::tools::map::{MapMod, PropEdit, PropRef};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-modpacks-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn target() -> PropRef {
    PropRef { region: "L4RA".into(), section: 1407, solid: "XO_CONE_01".into(), at: Vec3::new(10.0, 20.0, 3.0), nth: 0 }
}

fn mov(name: &str, x: f32) -> MapMod {
    MapMod {
        name: name.into(),
        props: vec![PropEdit::Move { target: target(), position: Vec3::new(x, 0.0, 0.0), rotation: Mat3::IDENTITY }],
        ..Default::default()
    }
}

fn opts() -> ExportOpts<'static> {
    ExportOpts { version: semver::Version::new(1, 0, 0), licence: None, derived_ok: false, build: "test".into() }
}

fn make(maps: &Path, folder: &str, m: &MapMod, texture: bool) {
    store::save(maps, folder, m).unwrap();
    if texture {
        let t = maps.join(folder).join("textures/world");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("12345678.png"), b"png").unwrap();
    }
}

#[test]
fn a_map_mod_becomes_a_pack_the_game_mounts() {
    let home = scratch("home");
    crate::platform::dirs::set_thread_data_home(home.clone());
    let cache = scratch("cache");
    let _c = crate::cache::override_dir(cache.clone());
    set_base(None);
    let maps = scratch("maps");
    make(&maps, "docks", &mov("Docks", 1.0), true);
    make(&maps, "cones", &mov("Cones", 2.0), false);
    let out = scratch("out");
    let base = BaseId { id: "nfsu2-pc12-eu".into(), identity: "sha256:aa".into() };
    // A texture with no texture list entry may start from the original's pixels: the author must say yes.
    assert!(export_map_mod(&maps, "docks", &out, Some(&base), &opts()).unwrap_err().contains("original game"));
    let yes = || ExportOpts { derived_ok: true, ..opts() };
    let e = export_map_mod(&maps, "docks", &out, Some(&base), &yes()).unwrap();
    assert_eq!(e.to, out.join("docks-1.0.0.obpk"));
    assert_eq!(
        std::fs::read(&e.to).unwrap(),
        std::fs::read(export_map_mod(&maps, "docks", &out, Some(&base), &yes()).unwrap().to).unwrap(),
        "the same export is the same bytes"
    );
    export_map_mod(&maps, "cones", &out, None, &opts()).unwrap();

    // Native default: no order.txt, nothing mounted, and no folder made.
    assert!(mount(&[]).mods.is_empty());
    assert!(!mods_dir().exists());

    // Import: new packs are off.
    assert_eq!(modpack::import(&mods_dir(), &out.join("docks-1.0.0.obpk"), &engine()).unwrap(), "docks");
    assert_eq!(modpack::import(&mods_dir(), &out.join("cones-1.0.0.obpk"), &engine()).unwrap(), "cones");
    assert!(mount(&[]).mods.is_empty(), "imported, not switched on");

    // The docks pack was made for a base set that is not in use here: refused with its words.
    modpack::set_enabled(&mods_dir(), "docks", true).unwrap();
    modpack::set_enabled(&mods_dir(), "cones", true).unwrap();
    let names = |m: Mounted| m.mods.into_iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(names(mount(&[])), ["cones"]);
    let plan = modpack::plan(&mods_dir(), None, &engine());
    assert!(plan.refused[0].1.contains("nfsu2-pc12-eu"), "{:?}", plan.refused);
    set_base(Some(base.clone()));
    // docks is first in order.txt = strongest: it comes last in the weakest-first list.
    assert_eq!(names(mount(&[])), ["cones", "docks"]);
    assert!(root_of("docks").unwrap().join("textures/world/12345678.png").is_file(), "its textures come along");
    // Both moved the same prop: the stronger one (docks) wins, and the clash says so.
    let all = mount(&[]).mods;
    let c = clashes(&all);
    assert!(
        c.iter().any(|c| c.weaker == "cones" && c.stronger == "docks" && c.field.as_deref() == Some("position")),
        "{c:?}"
    );
    // A plain map mod with the same name keeps its folder.
    assert_eq!(names(mount(&["docks".to_string()])), ["cones"]);
    modpack::move_pack(&mods_dir(), "cones", -1).unwrap();
    assert_eq!(names(mount(&[])), ["docks", "cones"]);
    modpack::set_enabled(&mods_dir(), "docks", false).unwrap();
    modpack::set_enabled(&mods_dir(), "cones", false).unwrap();
    assert!(mount(&[]).mods.is_empty(), "all off: native again");
    set_base(None);
    for d in [home, cache, maps, out] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn derived_textures_need_a_yes_and_keep_the_pack_local() {
    let maps = scratch("derived");
    make(&maps, "m", &mov("M", 1.0), false);
    let out = scratch("derived-out");
    let e = export_map_mod(&maps, "m", &out, None, &ExportOpts { licence: Some("CC-BY-4.0"), ..opts() }).unwrap();
    let b = ob_asset_format::bundle::Bundle::open(&e.to, ob_asset_format::bundle::Kind::PackFile).unwrap();
    b.verify().unwrap();
    let first = b.entries().iter().min_by_key(|e| e.data_offset).unwrap();
    assert_eq!(first.path, "pack.toml", "the manifest comes first");
    let text = String::from_utf8(b.read(b.get("pack.toml").unwrap()).unwrap().into_owned()).unwrap();
    assert!(text.contains("share = \"public\"") && text.contains("licence = \"CC-BY-4.0\""), "{text}");
    assert!(export_map_mod(&maps, "Bad Name", &out, None, &opts()).is_err());
    let _ = std::fs::remove_dir_all(maps);
    let _ = std::fs::remove_dir_all(out);
}
