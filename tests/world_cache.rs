//! The asset cache against a fresh parse (moved from track_l4ra.rs: it
//! points the cache at a folder of its own, process-wide, so it runs in a
//! binary of its own). Ignored without `NFSU2_ROOT` (docs/testing.md).

mod common;

use nfsu2::world::World;

/// The asset cache and the resident world: a world from the cache, a
/// fresh parse, and a world kept in memory, played with and reset
/// ([`World::reset_to_loaded`]), all come out byte for byte the same (the
/// cache's canonical dump, which covers every field but the lazily built
/// collision triangles).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn cached_and_resident_worlds_equal_a_fresh_load() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    // A cache folder of this test's own, set without `set_var` (the only
    // test in this binary: the override is process-wide).
    let scratch = common::Scratch::new("world-cache");
    let guard = nfsu2::cache::override_dir(scratch.path().to_path_buf());
    let dump = |w: &World| w.to_sections();
    let fresh = World::parse(root, "L4RA").unwrap();
    let first = World::load(root, "L4RA").unwrap();
    drop(first);
    nfsu2::cache::flush();
    let cached = World::load(root, "L4RA").unwrap();
    assert!(cached.cache.as_ref().is_some_and(|c| c.file().is_some()), "the second load reads the cache");
    assert!(dump(&cached) == dump(&fresh), "a cached world equals a parsed one");

    // Played with: a backward race's barriers and race-only scenery, a
    // door opened and run, a switch, a scenery group.
    let mut world = cached;
    let setup = nfsu2::world::BarrierSetup { track: 1001, free_roam: false, career: true, stage: 1, reversed: true, owns_car: true };
    world.set_up_barriers(&setup);
    let _ = world.take_changed();
    if let Some(key) = world.doors.first().map(|d| d.keys[0]) {
        world.door_message(key, 1.0, 1.0);
        let _ = world.step_doors(0.5);
    }
    let _ = world.switch_on("FREE_ROAM");
    if !world.scenery_groups.is_empty() {
        world.show_scenery_group(0, true);
    }
    assert!(dump(&world) != dump(&fresh), "the test changes the world");
    let before: Vec<bool> = (0..world.barriers.len()).map(|g| world.barriers.is_on(g as u8)).collect();
    assert!(world.reset_to_loaded(false));
    assert!(dump(&world) == dump(&fresh), "a resident world reset equals a fresh load");

    // Keeping the barrier switches (a reload of the same region) equals a
    // fresh load with them copied over.
    world.set_up_barriers(&setup);
    let _ = world.take_changed();
    let switches: Vec<bool> = (0..world.barriers.len()).map(|g| world.barriers.is_on(g as u8)).collect();
    assert!(world.reset_to_loaded(true));
    let copied = World::parse(root, "L4RA").unwrap();
    copied.copy_barriers_from(&world);
    assert!(dump(&world) == dump(&copied), "a resident world kept for the region equals a fresh load with its switches");
    assert_eq!(switches, (0..world.barriers.len()).map(|g| world.barriers.is_on(g as u8)).collect::<Vec<_>>());
    let _ = before;
    drop(world);
    nfsu2::cache::flush();
    drop(guard);
}

