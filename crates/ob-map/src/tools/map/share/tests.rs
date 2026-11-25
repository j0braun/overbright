//! A shared copy's round trip keeps the mod's textures; what is left out; the derived-texture refusal; the
//! texture count.

use std::path::{Path, PathBuf};

use super::*;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-map-share-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

const LOOKS_TEXT: &str = "format = 1\n\n[[texture]]\nid = \"world:0x1A2B3C4D\"\nname = \"DOCK_SIGN\"\norigin = \
                          \"blank\"\nimage = \"00112233aabbccdd\"\nwidth = 4\nheight = 4\n\n[[texture]]\nid = \
                          \"car:0x0000BEEF\"\nname = \"HOOD\"\norigin = \"game\"\nimage = \"0011\"\nwidth = 4\nheight \
                          = 4\n";

/// A mod with two textures (one blank, one from the game), work files and editor files.
fn make(maps: &Path, folder: &str) -> MapMod {
    let m = MapMod { name: "Harbour edits".into(), ..Default::default() };
    let dir = maps.join(folder);
    put(&dir.join(store::FILE), m.write().as_bytes());
    put(&dir.join(LOOKS), LOOKS_TEXT.as_bytes());
    put(&dir.join("textures/world/1A2B3C4D.png"), b"\x89PNG world pixels");
    put(&dir.join("textures/car/0000BEEF.png"), b"\x89PNG car pixels");
    put(&dir.join("work/looks/world/DOCK_SIGN.png"), b"work file");
    put(&dir.join(".editor/looks-work.toml"), b"work list");
    m
}

#[test]
fn textures_are_the_pack_files() {
    let maps = scratch("textures");
    make(&maps, "harbour");
    let dir = maps.join("harbour");
    put(&dir.join("textures/world/readme.txt"), b"not a texture");
    put(&dir.join("textures/world/NOTHEX.png"), b"not a hash");
    assert_eq!(textures(&dir), ["car:0x0000BEEF", "world:0x1A2B3C4D"]);
    assert!(textures(&maps.join("missing")).is_empty());
    let _ = std::fs::remove_dir_all(&maps);
}

#[test]
fn round_trip_keeps_the_textures_and_leaves_out_work_and_editor_files() {
    let maps = scratch("round");
    let m = make(&maps, "harbour");
    let zip = maps.join("out/harbour.zip");
    assert!(matches!(export(&maps, "harbour", &zip, false, "test build"), Err(Refused::Derived(d)) if d.len() == 1));
    assert!(!zip.exists(), "nothing written without the yes");
    let e = export(&maps, "harbour", &zip, true, "test build").unwrap();
    assert_eq!((e.files, e.derived, e.textures), (5, 1, 2), "four files and the provenance");
    let fresh = scratch("round-fresh");
    let f = import(&fresh, &zip).unwrap();
    let dir = fresh.join(&f);
    assert_eq!(store::read_file(&dir.join(store::FILE)).unwrap(), m);
    for p in [LOOKS, "textures/world/1A2B3C4D.png", "textures/car/0000BEEF.png"] {
        assert_eq!(std::fs::read(dir.join(p)).unwrap(), std::fs::read(maps.join("harbour").join(p)).unwrap(), "{p}");
    }
    assert!(!dir.join("work").exists() && !dir.join(".editor").exists());
    let prov = std::fs::read_to_string(dir.join(PROVENANCE)).unwrap();
    assert!(prov.contains("build = \"test build\"") && prov.contains("id = \"car:0x0000BEEF\""), "{prov}");
    // An older map-only zip still imports.
    let old = maps.join("old.zip");
    store::export(&maps, "harbour", &old).unwrap();
    let g = import(&fresh, &old).unwrap();
    assert!(fresh.join(&g).join(store::FILE).is_file() && !fresh.join(&g).join(store::TEXTURES).exists());
    for d in [maps, fresh] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn pack_files_without_a_record_count_as_derived() {
    let maps = scratch("unlisted");
    let dir = maps.join("bay");
    put(&dir.join(store::FILE), MapMod { name: "Bay".into(), ..Default::default() }.write().as_bytes());
    put(&dir.join("textures/world/193acff8.png"), b"\x89PNG");
    let d = derived(&dir);
    assert_eq!(d, [Derived { id: "world:0x193ACFF8".into(), name: String::new() }]);
    let _ = std::fs::remove_dir_all(&maps);
}
