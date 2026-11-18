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
            PropEdit::Delete { target: PropRef { section: 1408, ..target } },
            PropEdit::Add {
                region: "L4RB".into(),
                solid: "XO_BARREL".into(),
                position: Vec3::new(0.1, 0.2, 0.3),
                rotation: Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0)),
            },
        ],
        races: vec![race, sprint],
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
    refuse(&good.replacen("section = 1408", "section = 1407", 1), "already changed");
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
            target: PropRef { region: "L4RB".into(), section: 0, solid: "X".into(), at: Vec3::ZERO },
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
                region: "L4RA".into(),
                solid: ra.solid.clone(),
                position: to + Vec3::Y,
                rotation: Mat3::IDENTITY,
            },
            PropEdit::Add {
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
    assert_eq!(w.map_overlay.added.get(&n), Some(&("docks".to_string(), 2)));
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
