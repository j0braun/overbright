//! Map mods: the file format's round trip and refusals, the store's
//! folders and archives, and (with the game) the world and route hooks.

use glam::{Mat3, Vec2, Vec3};

use super::*;
use crate::testkit;

fn sample() -> MapMod {
    let target = PropRef {
        region: "L4RA".into(),
        section: 1407,
        solid: "XO_CONE_01".into(),
        at: Vec3::new(10.25, -20.5, 3.125),
        nth: 0,
    };
    let mut race = CustomRace::new("docks-loop", 4001, RaceKind::Circuit);
    race.name = "Docks Loop, fast".into();
    race.laps = 3;
    race.checkpoints = vec![Vec2::new(100.1, 200.2), Vec2::new(300.3, 400.4), Vec2::new(-1.0e-3, 7.0)];
    race.grid = vec![Spot { position: Vec3::new(1.0, 2.0, 3.0), heading: 91.5 }];
    race.start = Some(Spot { position: Vec3::new(4.0, 5.0, 6.0), heading: -30.0 });
    let mut sprint = CustomRace::new("s_2", 4126, RaceKind::Sprint);
    sprint.backward = true;
    sprint.checkpoints = vec![Vec2::ZERO, Vec2::ONE];
    sprint.finish = Some(Spot { position: Vec3::ONE, heading: 0.1 });
    MapMod {
        name: "Docks".into(),
        author: "someone".into(),
        description: "Cones, a loop and a sprint.".into(),
        props: vec![
            PropEdit::Move {
                target: target.clone(),
                position: Vec3::new(11.0, -20.5, 3.125),
                rotation: Mat3::from_rotation_z(0.3),
            },
            PropEdit::Delete { target: PropRef { section: 1408, nth: 2, ..target.clone() } },
            PropEdit::Delete { target: PropRef { section: 1408, ..target } },
            PropEdit::Add {
                id: "a1".into(),
                region: "L4RB".into(),
                solid: "XO_BARREL".into(),
                position: Vec3::new(0.1, 0.2, 0.3),
                rotation: Mat3::IDENTITY,
            },
            PropEdit::Add {
                id: "kiosk-7".into(),
                region: "L4RB".into(),
                solid: "XO_BARREL".into(),
                position: Vec3::new(0.1, 0.2, 0.3),
                rotation: Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0)),
            },
        ],
        races: vec![race, sprint],
        collision: vec![
            CollisionEdit::Remove {
                region: "L4RA".into(),
                corners: vec![Vec3::new(1.0, 2.5, 3.125), Vec3::new(1.0, 2.5, 7.0), Vec3::new(4.0, 2.5, 7.0)],
            },
            CollisionEdit::Wall {
                id: "wall-1".into(),
                region: "L4RA".into(),
                from: Vec3::new(10.0, 20.0, 3.5),
                to: Vec3::new(30.0, 25.5, 4.0),
                height: 2.5,
                surface: 16,
            },
            CollisionEdit::Ground {
                id: "ground-1".into(),
                region: "L4RA".into(),
                outline: vec![Vec3::new(0.0, 0.0, 1.0), Vec3::new(10.0, 0.0, 1.0), Vec3::new(10.0, 10.0, 2.0)],
                surface: 0,
            },
        ],
    }
}

#[test]
fn a_map_mod_reads_back_as_written() {
    let m = sample();
    let text = m.write();
    assert!(text.starts_with("overbright-map 1\n"));
    let back = MapMod::parse(&text).unwrap();
    assert_eq!(back, m);
    // Written again: the same text.
    assert_eq!(back.write(), text);
    // Comments, blank lines and spacing do not matter.
    let spaced = text.replace(" = ", "=").replace('\n', "\n\n# a comment\n");
    assert_eq!(MapMod::parse(&spaced).unwrap(), m);
    // A value may hold `=`; a line break in a name is written as a space.
    let named = MapMod { name: "a = b\nc".into(), ..Default::default() };
    assert_eq!(MapMod::parse(&named.write()).unwrap().name, "a = b c");
}

/// A file written before `nth` and add ids: read and written again byte
/// for byte; its adds take their default ids and keep them.
#[test]
fn a_file_without_nth_or_ids_writes_back_byte_identical() {
    let old = "overbright-map 1\nname = Old\n\n[prop.move]\nregion = L4RA\nsection = 1407\nsolid = XO_CONE_01\n\
               at = 10.25 -20.5 3.125\nposition = 11 -20.5 3.125\nrotation = 1 0 0 0 1 0 0 0 1\n\n[prop.delete]\n\
               region = L4RA\nsection = 1408\nsolid = XO_CONE_01\nat = 10.25 -20.5 3.125\n\n[prop.add]\n\
               region = L4RA\nsolid = XO_BARREL\nposition = 0.1 0.2 0.3\nrotation = 1 0 0 0 1 0 0 0 1\n\n\
               [prop.add]\nregion = L4RA\nsolid = XO_CONE_01\nposition = 1 2 3\nrotation = 1 0 0 0 1 0 0 0 1\n";
    let m = MapMod::parse(old).unwrap();
    assert_eq!(m.write(), old);
    assert!(m.props.iter().filter_map(PropEdit::target).all(|t| t.nth == 0));
    let ids: Vec<_> = m.props.iter().filter_map(PropEdit::add_id).collect();
    assert_eq!(ids, ["a1", "a2"]);
    assert_eq!(m.add_index("a2"), Some(3));
    // Without the first add the second keeps its id, now written.
    let mut less = m.clone();
    less.props.remove(2);
    let text = less.write();
    assert!(text.contains("[prop.add]\nid = a2\nregion = L4RA\nsolid = XO_CONE_01"), "{text}");
    assert_eq!(MapMod::parse(&text).unwrap(), less);
    assert_eq!(less.fresh_add_id(), "a3");
    assert_eq!(MapMod::default().fresh_add_id(), "a1");
    // `nth = 0` given is the default (not written back).
    let zero = old.replacen("at = 10.25 -20.5 3.125\n", "at = 10.25 -20.5 3.125\nnth = 0\n", 1);
    assert_eq!(MapMod::parse(&zero).unwrap(), m);
}

#[test]
fn bad_map_mods_are_refused_with_their_line() {
    let good = sample().write();
    let refuse = |text: &str, want: &str| {
        let e = MapMod::parse(text).expect_err(want);
        assert!(e.to_string().contains(want), "{e} (wanted {want})");
        e
    };
    refuse("", "empty");
    refuse("# only a comment\n", "empty");
    refuse("hello 1\n", "not a map mod");
    refuse("overbright-map\n", "version is missing");
    refuse("overbright-map 2\n", "newer");
    refuse("overbright-map 0\n", "newer");
    refuse("overbright-map 1 x\n", "after the version");
    let e = refuse("overbright-map 1\nname = a\n[prop.paint]\n", "unknown section");
    assert_eq!(e.line, 3);
    refuse("overbright-map 1\ncolour = red\n", "unknown key");
    refuse("overbright-map 1\nname\n", "key = value");
    refuse("overbright-map 1\n[race\n", "must end with");
    refuse(&good.replacen("track = 4001\n", "", 1), "needs `track`");
    refuse(&good.replacen("track = 4001", "track = 99999", 1), "not a track number");
    refuse(&good.replacen("laps = 3", "laps = 0", 1), "laps must be");
    refuse(&good.replacen("laps = 1", "laps = 2", 1), "runs one lap");
    refuse(&good.replacen("mode = sprint", "mode = drag", 1), "not a race mode");
    // One checkpoint: a race being made, kept.
    assert_eq!(MapMod::parse(&good.replacen("checkpoint = 0 0\n", "", 1)).unwrap().races[1].checkpoints.len(), 1);
    refuse(&good.replacen("checkpoint = 0 0", "checkpoint = 0 NaN", 1), "out of range");
    refuse(&good.replacen("checkpoint = 0 0", "checkpoint = 0 1e9", 1), "out of range");
    refuse(&good.replacen("checkpoint = 0 0", "checkpoint = 0 zero", 1), "not a number");
    refuse(&good.replacen("checkpoint = 0 0", "checkpoint = 0 0 0", 1), "expected 2 numbers");
    refuse(&good.replacen("id = s_2", "id = docks-loop", 1), "used twice");
    refuse(&good.replacen("id = s_2", "id = Has Space", 1), "not a race id");
    refuse(&good.replacen("nth = 2\n", "", 1), "already changed");
    refuse(&good.replacen("nth = 2", "nth = -1", 1), "not a count");
    refuse(&good.replacen("nth = 2", "nth = 70000", 1), "not a count");
    refuse(&good.replacen("nth = 2", "nth = 2\nnth = 3", 1), "given twice");
    refuse(&good.replacen("id = kiosk-7", "id = a1", 1), "add id `a1` used twice");
    refuse(&good.replacen("id = kiosk-7", "id = Kiosk 7", 1), "not an id");
    refuse(&good.replacen("[prop.add]\n", "[prop.add]\nnth = 1\n", 1), "unknown key `nth`");
    refuse(&good.replacen("backward = yes", "backward = maybe", 1), "yes or no");
    refuse(&good.replacen("region = L4RB", "region = ../x", 1), "not a region");
    refuse(&format!("{good}\n[race]\nid = x\ntrack = 1\nmode = sprint\nname = a\nname = b\n"), "given twice");
    let scaled = good.replacen("rotation = -1 0 0 0 1 0 0 0 1", "rotation = 2 0 0 0 1 0 0 0 1", 1);
    refuse(&scaled, "unit axes");
    let flat = good.replacen("rotation = -1 0 0 0 1 0 0 0 1", "rotation = 1 0 0 1 0 0 0 0 1", 1);
    refuse(&flat, "degenerate");
    let mut many = String::from("overbright-map 1\n[race]\nid = a\ntrack = 1\nmode = circuit\n");
    for k in 0..65 {
        many += &format!("checkpoint = {k} 0\n");
    }
    refuse(&many, "at most 64");
}

#[test]
fn the_store_saves_lists_imports_and_exports() {
    let s = testkit::Scratch::new("map-store");
    let dir = s.path().join("maps");
    let m = sample();
    assert!(store::enabled(&dir).is_empty() && store::list(&dir).is_empty() && store::load_enabled(&dir).is_empty());
    store::save(&dir, "docks", &m).unwrap();
    assert!(store::save(&dir, "../up", &m).is_err());
    assert_eq!(store::read_file(&dir.join("docks").join(store::FILE)).unwrap(), m);
    store::set_enabled(&dir, "docks", true).unwrap();
    store::set_enabled(&dir, "docks", true).unwrap();
    assert_eq!(store::enabled(&dir), vec!["docks".to_string()]);
    // A listed mod that does not read is skipped; the others load.
    std::fs::create_dir_all(dir.join("broken")).unwrap();
    std::fs::write(dir.join("broken").join(store::FILE), "overbright-map 9\n").unwrap();
    store::set_enabled(&dir, "broken", true).unwrap();
    let loaded = store::load_enabled(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0], ("docks".to_string(), m.clone()));
    store::set_enabled(&dir, "broken", false).unwrap();
    assert_eq!(store::enabled(&dir), vec!["docks".to_string()]);
    // Export as a zip and a folder, import both back as new folders.
    let zip = s.path().join("docks.zip");
    store::export(&dir, "docks", &zip).unwrap();
    let a = store::import(&dir, &zip).unwrap();
    assert_eq!(a, "docks-2");
    let folder = s.path().join("out");
    store::export(&dir, "docks", &folder).unwrap();
    let b = store::import(&dir, &folder).unwrap();
    assert_eq!(b, "docks-3");
    for f in [&a, &b] {
        assert_eq!(store::read_file(&dir.join(f).join(store::FILE)).unwrap(), m);
    }
    assert_eq!(store::list(&dir), vec!["broken", "docks", "docks-2", "docks-3"]);
    // Bad imports leave nothing behind.
    std::fs::write(s.path().join("bad.zip"), b"not a zip").unwrap();
    assert!(store::import(&dir, &s.path().join("bad.zip")).is_err());
    std::fs::write(s.path().join("x.txt"), b"x").unwrap();
    assert!(store::import(&dir, &s.path().join("x.txt")).is_err());
    let bad = s.path().join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join(store::FILE), "overbright-map 1\n[race]\n").unwrap();
    assert!(store::import(&dir, &bad).is_err());
    assert_eq!(store::list(&dir).len(), 4);
    // Too large: refused before it is read.
    let big = s.path().join("big");
    std::fs::create_dir_all(&big).unwrap();
    std::fs::write(big.join(store::FILE), vec![b'#'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
    assert!(store::read_file(&big.join(store::FILE)).unwrap_err().to_string().contains("more than"));
}

#[test]
fn the_run_copy_and_the_editor_files_live_in_dot_editor() {
    let s = testkit::Scratch::new("map-sidecars");
    let dir = s.path().join("maps");
    let m = sample();
    store::save(&dir, "docks", &m).unwrap();
    let saved = std::fs::read(dir.join("docks").join(store::FILE)).unwrap();
    // The run copy: written whole beside the editor's files, the mod untouched.
    let mut unsaved = m.clone();
    unsaved.name = "Docks, unsaved".into();
    let run = store::write_run_copy(&dir, "docks", &unsaved).unwrap();
    assert_eq!(run, dir.join("docks/.editor/run/map.obmap"));
    assert_eq!(store::read_file(&run).unwrap(), unsaved);
    assert_eq!(std::fs::read(dir.join("docks").join(store::FILE)).unwrap(), saved);
    assert!(store::write_run_copy(&dir, "../up", &m).is_err());
    assert_eq!(store::folder_of_file(&run), "docks", "a run copy plays as its mod");
    assert_eq!(store::folder_of_file(std::path::Path::new("/x/yard/map.obmap")), "yard");
    assert_eq!(store::folder_of_file(std::path::Path::new("/x/My Yard/map.obmap")), "map-file");
    assert_eq!(store::folder_of_file(std::path::Path::new("map.obmap")), "map-file");
    assert_eq!(store::list(&dir), ["docks"], "`.editor` is no mod");
    // An older editor's files move into `.editor/` once; one there already stays.
    for (i, name) in store::SIDECARS.iter().enumerate() {
        std::fs::write(dir.join("docks").join(name), format!("old {i}")).unwrap();
    }
    std::fs::write(store::editor_dir(&dir, "docks").join("tested.txt"), "new").unwrap();
    assert_eq!(store::move_old_sidecars(&dir, "docks").unwrap(), 3);
    for (i, name) in store::SIDECARS.iter().enumerate() {
        let moved = std::fs::read_to_string(store::editor_dir(&dir, "docks").join(name)).unwrap();
        if *name == "tested.txt" {
            assert_eq!(moved, "new");
            assert!(dir.join("docks").join(name).is_file(), "kept when the new one exists");
        } else {
            assert_eq!(moved, format!("old {i}"));
            assert!(!dir.join("docks").join(name).exists());
        }
    }
    assert_eq!(store::move_old_sidecars(&dir, "docks").unwrap(), 0, "once");
    // Export takes only the mod.
    let out = s.path().join("out");
    store::export(&dir, "docks", &out).unwrap();
    let names: Vec<_> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(names, [std::ffi::OsString::from(store::FILE)]);
}

#[test]
fn the_default_grid_stands_behind_the_start() {
    let start = Spot { position: Vec3::new(10.0, 0.0, 2.0), heading: 0.0 };
    let g = race::default_grid(start);
    assert_eq!(g.len(), 8);
    assert!(g.iter().all(|s| s.position.x < 10.0 && s.position.z == 2.0 && s.heading == 0.0));
    assert!(g[0].position.y > 0.0 && g[1].position.y < 0.0, "pole on the left");
    assert!(g[7].position.x < g[0].position.x - 20.0);
}

// ---- with the game's files ----------------------------------------------

fn track(root: &std::path::Path, number: u16) -> crate::frontend::career::Track {
    let career = crate::frontend::career::CareerData::load(root).unwrap();
    career.tracks.get(&number).unwrap_or_else(|| panic!("track {number}")).clone()
}

/// No mod: the world is the one `World::load` gives, placement for
/// placement, and its loaded state too.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn no_map_mod_leaves_the_world_as_loaded() {
    let root = testkit::game_root();
    let plain = crate::world::World::load(&root, "L4RA").unwrap();
    let mut w = crate::world::World::load(&root, "L4RA").unwrap();
    w.apply_map_mods("L4RA", &[]);
    // A mod for another region only.
    let other = MapMod {
        props: vec![PropEdit::Delete {
            target: PropRef { region: "L4RB".into(), section: 0, solid: "X".into(), at: Vec3::ZERO, nth: 0 },
        }],
        ..Default::default()
    };
    w.apply_map_mods("L4RA", &[("other".into(), other)]);
    assert_eq!(w.placements.len(), plain.placements.len());
    for (a, b) in w.placements.iter().zip(&plain.placements) {
        assert_eq!((a.solid, a.section, a.flags, a.transform), (b.solid, b.section, b.flags, b.transform));
    }
    assert!(w.map_overlay.originals.is_empty() && w.map_overlay.added.is_empty());
    assert!(w.take_changed().is_empty());
}

/// A mod's move, delete and add reach the world, survive a kept world's
/// reset, and name their placements as the original has them.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_map_mod_moves_hides_and_adds_placements() {
    let root = testkit::game_root();
    let plain = crate::world::World::load(&root, "L4RA").unwrap();
    // Two small shown placements with a matrix.
    let small: Vec<usize> = (0..plain.placements.len())
        .filter(|&i| {
            let p = &plain.placements[i];
            p.flags & 0x0110 == 0 && edit::placement_sphere(&plain, i).is_some_and(|s| s.1 < 5.0)
        })
        .take(2)
        .collect();
    assert_eq!(small.len(), 2);
    let (a, b) = (small[0], small[1]);
    let ra = plain.placement_ref("L4RA", a).unwrap();
    let rb = plain.placement_ref("L4RA", b).unwrap();
    assert_eq!(plain.find_placement(&ra), Some(a));
    let to = ra.at + Vec3::new(3.0, 0.0, 0.0);
    let m = MapMod {
        props: vec![
            PropEdit::Move { target: ra.clone(), position: to, rotation: Mat3::IDENTITY },
            PropEdit::Delete { target: rb.clone() },
            PropEdit::Add {
                id: "a1".into(),
                region: "L4RA".into(),
                solid: ra.solid.clone(),
                position: to + Vec3::Y,
                rotation: Mat3::IDENTITY,
            },
            PropEdit::Add {
                id: "a2".into(),
                region: "l4ra".into(),
                solid: "NO_SUCH_SOLID".into(),
                position: to,
                rotation: Mat3::IDENTITY,
            },
        ],
        ..Default::default()
    };
    // Through the file format, as the game reads it.
    let m = MapMod::parse(&m.write()).unwrap();
    let mut w = crate::world::World::load(&root, "L4RA").unwrap();
    w.apply_map_mods("L4RA", &[("docks".into(), m)]);
    let n = plain.placements.len();
    assert_eq!(w.placements.len(), n + 1);
    assert_eq!(w.placements[a].transform.w_axis.truncate(), to);
    assert!(w.placements[b].flags & crate::world::HIDDEN_FLAG != 0);
    assert_eq!(w.placements[n].solid, plain.placements[a].solid);
    assert_eq!(w.placements[n].transform.w_axis.truncate(), to + Vec3::Y);
    assert_eq!(w.map_overlay.added.get(&n), Some(&("docks".to_string(), "a1".to_string())));
    assert_eq!(w.map_overlay.missing, 1);
    // Named by the original's place, not the moved one.
    assert_eq!(w.placement_ref("L4RA", a), Some(ra));
    assert_eq!(w.placement_ref("L4RA", n), None);
    // A kept world's reset comes back to the world with the mod.
    w.placements[a].transform = glam::Mat4::IDENTITY;
    assert!(w.reset_to_loaded(false));
    assert_eq!(w.placements.len(), n + 1);
    assert_eq!(w.placements[a].transform.w_axis.truncate(), to);
}

/// A custom race through a track's own checkpoints builds the road the
/// track's own route has.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_custom_race_on_native_checkpoints_is_the_native_road() {
    let root = testkit::game_root();
    for number in [4001u16, 4126] {
        let t = track(&root, number);
        let native = crate::frontend::career::load_route(&root, &t, false).unwrap();
        let mut r = CustomRace::new("t", number, if native.loops { RaceKind::Circuit } else { RaceKind::Sprint });
        r.checkpoints = native.checkpoints.clone();
        let net = race::road_net(&root, &t).unwrap();
        let route = race::route(&net, &r, &|_| None).unwrap();
        assert!(
            (route.road.length - native.road.length).abs() < 1.0,
            "{number}: {} vs {}",
            route.road.length,
            native.road.length
        );
        assert_eq!(route.loops, native.loops);
        assert_eq!(route.grid.len(), 8);
    }
}

/// A leg across the base track's closed barriers or off the network is
/// named.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_custom_race_with_too_few_checkpoints_is_refused() {
    let root = testkit::game_root();
    let t = track(&root, 4001);
    let net = race::road_net(&root, &t).unwrap();
    let mut r = CustomRace::new("t", 4001, RaceKind::Sprint);
    r.checkpoints = vec![Vec2::new(0.0, 0.0)];
    assert!(race::check(&net, &r).is_err());
    assert!(race::route(&net, &r, &|_| None).is_err());
}

/// The original placements of one section and solid at exactly one place
/// in L4RA, beyond the first of each such group ("twins", named apart
/// only by `nth`).
const L4RA_TWINS: usize = 257;

/// The groups of original placements of one section and solid at exactly
/// one place (more than one each), in the original's order.
fn twin_groups(w: &crate::world::World) -> Vec<Vec<usize>> {
    let mut groups: std::collections::BTreeMap<(u32, usize, [u32; 3]), Vec<usize>> = Default::default();
    for (i, p) in w.placements.iter().enumerate() {
        // `+ 0.0`: -0 and 0 are one place.
        let at = (p.transform.w_axis.truncate() + Vec3::ZERO).to_array().map(f32::to_bits);
        groups.entry((p.section, p.solid, at)).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    out.sort();
    out
}

/// Twins are named apart, edit independently (from a file and in the
/// editor, each after the others moved) and come back the same on reload.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn twin_placements_edit_independently_and_reload() {
    let root = testkit::game_root();
    let plain = crate::world::World::load(&root, "L4RA").unwrap();
    let groups = twin_groups(&plain);
    let twins: usize = groups.iter().map(|g| g.len() - 1).sum();
    eprintln!("L4RA: {twins} twins in {} groups", groups.len());
    assert_eq!(twins, L4RA_TWINS);
    for g in &groups {
        for (n, &i) in g.iter().enumerate() {
            let r = plain.placement_ref("L4RA", i).unwrap();
            assert_eq!(r.nth as usize, n, "placement {i}");
            assert_eq!(plain.find_placement(&r), Some(i), "{r:?}");
        }
    }
    // A mod hides the first of each group and moves each twin up by its
    // nth metres.
    let mut m = MapMod::default();
    for g in &groups {
        for (n, &i) in g.iter().enumerate() {
            let target = plain.placement_ref("L4RA", i).unwrap();
            m.props.push(match n {
                0 => PropEdit::Delete { target },
                // Upright: some originals are scaled, which a move does
                // not keep.
                _ => PropEdit::Move { position: target.at + Vec3::Z * n as f32, target, rotation: Mat3::IDENTITY },
            });
        }
    }
    let text = m.write();
    let m = MapMod::parse(&text).unwrap();
    assert_eq!(m.write(), text);
    let check = |w: &crate::world::World| {
        assert_eq!(w.map_overlay.missing, 0);
        for g in &groups {
            let at = plain.placements[g[0]].transform.w_axis.truncate();
            for (n, &i) in g.iter().enumerate() {
                let (p, hidden) = (&w.placements[i], crate::world::HIDDEN_FLAG);
                // (Some originals start hidden: scenery group members.)
                assert_eq!(p.flags & hidden != 0, n == 0 || plain.placements[i].flags & hidden != 0, "placement {i}");
                assert_eq!(p.transform.w_axis.truncate(), at + Vec3::Z * n as f32, "placement {i}");
            }
        }
    };
    let mut w = crate::world::World::load(&root, "L4RA").unwrap();
    w.apply_map_mods("L4RA", &[("twins".into(), m.clone())]);
    check(&w);
    assert_eq!((w.map_overlay.moved, w.map_overlay.deleted), (twins, groups.len()));
    // Each keeps its name after the others moved.
    for g in &groups {
        for &i in g {
            assert_eq!(w.placement_ref("L4RA", i), plain.placement_ref("L4RA", i));
        }
    }
    // A kept world's reset comes back to the world with the mod.
    w.placements[groups[0][1]].transform = glam::Mat4::IDENTITY;
    assert!(w.reset_to_loaded(false));
    check(&w);
    // The editor's way, one after another: each named apart from those
    // already moved, and the saved mod gives the same world.
    let (mut e, mut ew) = (MapMod::default(), crate::world::World::load(&root, "L4RA").unwrap());
    let g = groups.iter().max_by_key(|g| g.len()).unwrap();
    for (n, &i) in g.iter().enumerate() {
        let at = ew.placements[i].transform.w_axis.truncate();
        edit::set_transform(&mut e, "twins", &mut ew, "L4RA", i, at + Vec3::X * (n + 1) as f32, Mat3::IDENTITY)
            .unwrap();
    }
    let e = MapMod::parse(&e.write()).unwrap();
    let nths: Vec<u32> = e.props.iter().filter_map(PropEdit::target).map(|t| t.nth).collect();
    assert_eq!(nths, (0..g.len() as u32).collect::<Vec<_>>());
    let mut again = crate::world::World::load(&root, "L4RA").unwrap();
    again.apply_map_mods("L4RA", &[("twins".into(), e)]);
    for &i in g {
        assert_eq!(again.placements[i].transform, ew.placements[i].transform, "placement {i}");
    }
}
