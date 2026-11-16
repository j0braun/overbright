//! The files the game reads ahead (docs/read-ahead.md) are the ones its
//! loads read: a car's cache files under the names the cache gives them,
//! its game files while there are none. It points the cache at a folder
//! of its own, process-wide, so it runs in a binary of its own. Ignored
//! without `NFSU2_ROOT` (docs/testing.md).

mod common;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn read_ahead_lists_name_the_files_the_loads_read() {
    let root = common::game_root();
    let root = root.as_path();
    let scratch = common::Scratch::new("read-ahead-files");
    let cache = scratch.path().join("cache");
    // The only test in this binary: the override is process-wide.
    let _guard = nfsu2::cache::override_dir(cache.clone());
    let path = |rel: &str| nfsu2::game_fs::path(root, rel);

    // An empty cache: the game files a parse reads.
    let own_textures = path("CARS/240SX/TEXTURES.BIN");
    let before = nfsu2::car::read_ahead_files(root, "240SX");
    assert!(before.contains(&own_textures), "no cache file: the car's TEXTURES.BIN");
    assert!(before.contains(&path("CARS/240SX/GEOMETRY.BIN")));
    assert!(before.iter().all(|p| !p.starts_with(&cache)), "nothing in the empty cache folder: {before:?}");

    // Loaded once (its cache files written): the list names those files.
    let car = nfsu2::car::Car::load_upgraded(root, "240SX", &Default::default(), &[], &|_| false).unwrap();
    drop(car);
    nfsu2::cache::flush();
    let after = nfsu2::car::read_ahead_files(root, "240SX");
    let entries: Vec<&std::path::PathBuf> = after.iter().filter(|p| p.starts_with(&cache)).collect();
    // The car's solids and textures, the shared car, rim and GLOBALB
    // texture packs.
    assert_eq!(entries.len(), 5, "{after:?}");
    assert!(entries.iter().all(|p| p.is_file()), "{entries:?}");
    assert!(!after.contains(&own_textures), "the cache file in place of the game file");
    assert!(after.contains(&path("CARS/240SX/GEOMETRY.BIN")), "read whole by every load (the collision hull)");
}
