//! Per-item fingerprints (manifest.md §4.2, format 3): old packs read unchanged, new packs carry the field, a
//! changed item is named by its fingerprint.
use std::path::PathBuf;

use ob_asset_format::manifest::{Container, Manifest};
use ob_asset_format::modpack::{self, ITEMS_FILE};
use ob_asset_format::Rule;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pack-format2")
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-items-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn new_files(map: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![("map.obmap".to_string(), map.to_vec()), ("races/a.toml".to_string(), b"a = 1\n".to_vec())];
    let items = modpack::items_text(&files);
    files.push((ITEMS_FILE.into(), items.into_bytes()));
    let m = "format = 3\nkind = \"mod\"\nid = \"new-docks\"\nname = \"New docks\"\nversion = \"1.0.0\"\noverbright = \">=0.7\"\norigin = \"own-work\"\nshare = \"local-only\"\nitems = \"items.toml\"\n";
    files.push(("pack.toml".into(), m.as_bytes().to_vec()));
    files
}

fn obpk(dir: &std::path::Path, files: &[(String, Vec<u8>)]) -> PathBuf {
    let p = dir.join("new-docks-1.0.0.obpk");
    std::fs::write(&p, modpack::pack_file_bytes(files).unwrap()).unwrap();
    p
}

#[test]
fn an_old_pack_loads_byte_for_byte_and_has_no_fingerprints() {
    let bytes = std::fs::read(fixture().join("pack.toml")).unwrap();
    let (m, _) = Manifest::parse(&bytes, Container::Unknown).unwrap();
    assert_eq!((m.format, m.items.as_deref()), (2, None));
    assert_eq!(m.to_toml().as_bytes(), bytes.as_slice(), "an old manifest writes back byte for byte");
    let p = modpack::open(&fixture()).unwrap();
    assert_eq!(modpack::verify_items(&p).unwrap(), None, "nothing to check, nothing wrong");
    // As a pack file too: the same files give the same pack, still without fingerprints.
    let d = scratch("old");
    let files: Vec<(String, Vec<u8>)> = ["pack.toml", "map.obmap"]
        .iter()
        .map(|n| (n.to_string(), std::fs::read(fixture().join(n)).unwrap()))
        .collect();
    let f = d.join("old-docks-1.0.0.obpk");
    std::fs::write(&f, modpack::pack_file_bytes(&files).unwrap()).unwrap();
    let p = modpack::open(&f).unwrap();
    assert_eq!((p.manifest.format, modpack::verify_items(&p).unwrap()), (2, None));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_new_pack_round_trips_and_an_older_format_refuses_the_field() {
    let files = new_files(b"v1");
    let d = scratch("new");
    let p = modpack::open(&obpk(&d, &files)).unwrap();
    assert_eq!((p.manifest.format, p.manifest.items.as_deref()), (3, Some("items.toml")));
    assert_eq!(p.manifest.to_toml().as_bytes(), files.iter().find(|f| f.0 == "pack.toml").unwrap().1.as_slice());
    let r = modpack::verify_items(&p).unwrap().unwrap();
    assert!(r.is_clean() && r.listed == 2, "{r:?}");
    // The field in a format-2 file is refused, with its own rule.
    let bad = "format = 2\nkind = \"mod\"\nid = \"x\"\nname = \"x\"\nversion = \"1.0.0\"\noverbright = \">=0.5\"\norigin = \"own-work\"\nshare = \"local-only\"\nitems = \"items.toml\"\n";
    assert_eq!(Manifest::parse(bad.as_bytes(), Container::Unknown).unwrap_err().rule, Rule::Man10);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_changed_missing_or_added_item_is_named_by_its_fingerprint() {
    let d = scratch("changed");
    let mut files = new_files(b"v1");
    // Change one item after the fingerprints were made, drop another, add a third.
    files.iter_mut().find(|f| f.0 == "map.obmap").unwrap().1 = b"v2".to_vec();
    files.retain(|f| f.0 != "races/a.toml");
    files.push(("races/new.toml".into(), b"n".to_vec()));
    let p = modpack::open(&obpk(&d, &files)).unwrap();
    let r = modpack::verify_items(&p).unwrap().unwrap();
    assert_eq!((r.changed.as_slice(), r.missing.as_slice(), r.added.as_slice()), (&["map.obmap".to_string()][..], &["races/a.toml".to_string()][..], &["races/new.toml".to_string()][..]));
    assert!(!r.is_clean());
    // A folder pack is checked the same way.
    let dir = d.join("folder");
    for (n, b) in &files {
        let at = dir.join(n);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, b).unwrap();
    }
    let p = modpack::open(&dir).unwrap();
    assert_eq!(modpack::verify_items(&p).unwrap().unwrap(), r);
    // A damaged items file is refused, not read as "nothing listed".
    std::fs::write(dir.join(ITEMS_FILE), b"schema = \"ob.items/1.0\"\n[items]\n\"a\" = \"nope\"\n").unwrap();
    assert!(modpack::verify_items(&modpack::open(&dir).unwrap()).is_err());
    let _ = std::fs::remove_dir_all(d);
}
