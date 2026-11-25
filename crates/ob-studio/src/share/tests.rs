//! Export and import of a shared copy: the round trip, what is left out, the derived-textures refusal.

use std::path::{Path, PathBuf};

use super::*;
use crate::looks::{self, Domain, Entry, Looks, Origin, TextureId};
use ob_map::tools::map::store;
use ob_map::tools::map::format::{CustomRace, RaceKind};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-share-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// A mod with a race, a texture (from `origin`), work files and editor files.
fn make(maps: &Path, folder: &str, origin: Origin) -> MapMod {
    let mut m = MapMod { name: "Harbour edits".into(), author: "Sam".into(), ..Default::default() };
    m.description = "Cones at the docks".into();
    m.races.push(CustomRace::new("dock-sprint", 1, RaceKind::Sprint));
    let dir = maps.join(folder);
    put(&dir.join(store::FILE), m.write().as_bytes());
    let id = TextureId::new(Domain::World, 0x1A2B3C4D);
    let mut l = Looks::default();
    l.entries.insert(
        id,
        Entry { id, name: "DOCK_SIGN".into(), origin, image: "00112233aabbccdd".into(), width: 4, height: 4 },
    );
    put(&dir.join(looks::FILE), l.write().as_bytes());
    put(&dir.join("textures/world/1A2B3C4D.png"), b"\x89PNG fake pixels");
    put(&dir.join("work/looks/world/DOCK_SIGN.png"), b"work file");
    put(&dir.join(".editor/journal"), b"steps");
    put(&dir.join(".studio/state"), b"open");
    m
}

#[test]
fn shared_files_leave_out_work_and_editor_folders() {
    let maps = scratch("files");
    make(&maps, "harbour", Origin::Blank);
    let f: Vec<String> = files(&maps.join("harbour")).unwrap().into_iter().map(|f| f.path).collect();
    assert_eq!(f, ["looks.toml", "map.obmap", "textures/world/1A2B3C4D.png"]);
    assert!(!shared("work/x.png") && !shared(".editor/a") && !shared("a/.b") && shared("textures/world/A.png"));
    let _ = std::fs::remove_dir_all(&maps);
}

#[test]
fn export_and_import_give_the_same_mod() {
    let maps = scratch("round");
    let m = make(&maps, "harbour", Origin::Blank);
    let zip = maps.join("out/harbour.zip");
    let e = export(&maps, "harbour", &zip, false).unwrap();
    assert_eq!(e.files, 4, "three files and the provenance");
    assert_eq!(e.derived, 0);
    // The launcher's and editor's import reads it too (the map part).
    let fresh = scratch("round-fresh");
    let f = store::import(&fresh, &zip).unwrap();
    assert_eq!(store::read_file(&fresh.join(&f).join(store::FILE)).unwrap(), m);
    // Studio's import brings everything shared back, nothing else.
    let fresh2 = scratch("round-fresh2");
    let f2 = import(&fresh2, &zip).unwrap();
    let dir = fresh2.join(&f2);
    assert_eq!(store::read_file(&dir.join(store::FILE)).unwrap(), m);
    for p in ["looks.toml", "map.obmap", "textures/world/1A2B3C4D.png"] {
        assert_eq!(std::fs::read(dir.join(p)).unwrap(), std::fs::read(maps.join("harbour").join(p)).unwrap(), "{p}");
    }
    assert!(!dir.join("work").exists() && !dir.join(".editor").exists() && !dir.join(".studio").exists());
    let prov = std::fs::read_to_string(dir.join(PROVENANCE)).unwrap();
    assert!(prov.contains(&format!("build = \"{}\"", build())), "{prov}");
    // A folder export imports the same way, and a second import never writes over the first.
    let out = maps.join("out/folder");
    export(&maps, "harbour", &out, false).unwrap();
    let f3 = import(&fresh2, &out).unwrap();
    assert_ne!(f3, f2);
    assert_eq!(store::read_file(&fresh2.join(&f3).join(store::FILE)).unwrap(), m);
    assert!(export(&maps, "harbour", &out, false).is_err(), "a folder that is not empty is not written into");
    for d in [maps, fresh, fresh2] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn derived_textures_need_a_yes_and_are_named_in_the_provenance() {
    let maps = scratch("derived");
    make(&maps, "harbour", Origin::Game);
    let zip = maps.join("harbour.zip");
    match export(&maps, "harbour", &zip, false) {
        Err(Refused::Derived(d)) => {
            assert_eq!(d.len(), 1);
            assert!(Refused::Derived(d).to_string().contains("original game's pixels"));
        }
        other => panic!("{other:?}"),
    }
    assert!(!zip.exists(), "nothing written without the yes");
    let e = export(&maps, "harbour", &zip, true).unwrap();
    assert_eq!(e.derived, 1);
    let fresh = scratch("derived-fresh");
    let f = import(&fresh, &zip).unwrap();
    let prov = std::fs::read_to_string(fresh.join(f).join(PROVENANCE)).unwrap();
    assert!(prov.contains("id = \"world:0x1A2B3C4D\"") && prov.contains("origin = \"game\""), "{prov}");
    for d in [maps, fresh] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn ready_to_share_points() {
    let mut m = MapMod::default();
    let p = ready(&m, None, 0);
    assert!(!is_ready(&p), "no name");
    m.name = "X".into();
    assert!(is_ready(&ready(&m, None, 2)), "derived textures warn, never block");
    m.races.push(CustomRace::new("a", 1, RaceKind::Sprint));
    assert!(!is_ready(&ready(&m, None, 0)), "races need the checks");
    assert!(is_ready(&ready(&m, Some(&[]), 0)));
}

#[test]
fn names_outside_the_folder_are_never_written() {
    let maps = scratch("unsafe");
    let zip = maps.join("bad.zip");
    let m = MapMod { name: "Bad".into(), ..Default::default() }.write();
    let z = ob_formats::formats::zip::build(&[("bad/map.obmap", m.as_bytes(), 8), ("bad/../escape.txt", b"x", 0)]);
    std::fs::write(&zip, z).unwrap();
    let fresh = scratch("unsafe-fresh");
    // The archive reader leaves such a name out; nothing is written outside the new folder.
    let f = import(&fresh, &zip).unwrap();
    assert!(!fresh.join("escape.txt").exists() && !fresh.join(f).join("escape.txt").exists());
    assert!(!safe("../x") && !safe("a/../x") && !safe("/x") && !safe("a\\b") && !safe("c:x") && safe("textures/a.png"));
    for d in [maps, fresh] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn the_ready_list_names_textures_whose_picture_is_missing_or_not_the_listed_one() {
    let maps = scratch("looks-ready");
    make(&maps, "m", Origin::Blank);
    let dir = maps.join("m");
    let png = looks::encode_png(4, 4, &[200; 64]).unwrap();
    let texts = |dir: &Path| looks(dir).into_iter().map(|p| (p.ok, p.text)).collect::<Vec<_>>();
    // The fake picture: not a PNG.
    assert!(
        texts(&dir)[0].1.starts_with("Texture DOCK_SIGN: its picture is not a PNG the game reads"),
        "{:?}",
        texts(&dir)
    );
    put(&dir.join("textures/world/1A2B3C4D.png"), &png);
    assert_eq!(
        texts(&dir),
        vec![(
            false,
            "Texture DOCK_SIGN: its picture (textures/world/1A2B3C4D.png) is not the one the texture list names".into()
        )]
    );
    let mut l = Looks::parse(&std::fs::read_to_string(dir.join(looks::FILE)).unwrap()).unwrap();
    l.entries.values_mut().for_each(|e| e.image = looks::image_hash(&png));
    put(&dir.join(looks::FILE), l.write().as_bytes());
    assert_eq!(texts(&dir), vec![(true, "1 texture in place".into())]);
    std::fs::remove_file(dir.join("textures/world/1A2B3C4D.png")).unwrap();
    assert_eq!(
        texts(&dir),
        vec![(false, "Texture DOCK_SIGN: its picture is missing (textures/world/1A2B3C4D.png)".into())]
    );
    assert!(!is_ready(&looks(&dir)));
    put(&dir.join(looks::FILE), b"format = [");
    assert!(texts(&dir)[0].1.starts_with("The texture list does not read"));
    let _ = std::fs::remove_dir_all(&maps);
}
