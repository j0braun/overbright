use super::*;
use crate::tools::map::{MapMod, PropEdit};

fn l4ra() -> World {
    World::load(&crate::testkit::game_root(), "L4RA").unwrap()
}

/// Survey (prints): each solid's shapes over its originals.
#[test]
#[ignore = "survey; prints"]
fn survey_solid_shapes() {
    let w = l4ra();
    for name in std::env::var("OB_SURVEY").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        let Some(s) = w.solid_named(name) else {
            println!("{name}: no solid");
            continue;
        };
        let so = &w.solids[s];
        let all: Vec<usize> = (0..w.placements.len()).filter(|&i| w.placements[i].solid == s).collect();
        let props = all.iter().filter(|&&i| w.smackables.iter().any(|p| p.placement == i)).count();
        let counts: Vec<usize> =
            all.iter().take(40).map(|&i| w.local_faces(s, w.placements[i].transform).len()).collect();
        println!(
            "{name}: box {:?}..{:?} placements {} props {} counts {:?}",
            so.bbox_min,
            so.bbox_max,
            all.len(),
            props,
            counts
        );
        let shape = w.solid_collision(s);
        for f in &shape {
            println!("   flags {:#x} surf {} {:?}", f.flags, f.surface, &f.corners[..if f.quad { 4 } else { 3 }]);
        }
    }
}

/// Every face of every L4RA pack lists in the cells `cells_of` gives it,
/// with its masks: the rule new faces are written by is the one the
/// shipped packs follow (all but a few faces that only touch a cell's edge).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn new_faces_list_in_cells_as_the_shipped_packs_do() {
    let w = l4ra();
    let (mut cells, mut same_set, mut same_mask) = (0usize, 0usize, 0usize);
    for (pi, g) in w.ground.grids().enumerate() {
        let mut want: Vec<HashMap<u16, u16>> = vec![HashMap::new(); g.cells.len()];
        let mut fi = 0;
        while let Some(f) = w.ground.face_at(pi, fi) {
            for (c, m) in cells_of(f, g.origin, g.width, g.height) {
                want[c].insert(fi as u16, m);
            }
            fi += 1;
        }
        for (c, &(first, count)) in g.cells.iter().enumerate() {
            let got: HashMap<u16, u16> = g.refs[first as usize..(first + count) as usize].iter().copied().collect();
            cells += 1;
            let keys = |m: &HashMap<u16, u16>| m.keys().copied().collect::<HashSet<u16>>();
            if keys(&got) == keys(&want[c]) {
                same_set += 1;
                same_mask += (got == want[c]) as usize;
            }
        }
    }
    eprintln!("cells {cells}: same faces {same_set}, same masks {same_mask}");
    assert!(cells > 14_000);
    assert!(cells - same_set <= 5, "{} cells list other faces", cells - same_set);
    assert!(cells - same_mask <= 20, "{} cells have other masks", cells - same_mask);
}

/// A shipped face written again where it stands (`world_face` of itself)
/// is the same face: its stored corners, heights and bounds byte.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_face_written_where_it_stands_is_the_same_face() {
    let w = l4ra();
    let mut n = 0;
    for pi in 0..w.ground.grids().count() {
        let mut fi = 0;
        while let Some(f) = w.ground.face_at(pi, fi) {
            fi += 1;
            if f.flags & FLAG_TALL != 0 {
                continue;
            }
            let local = LocalFace { corners: f.vertices, quad: f.corners == 4, surface: f.surface, flags: f.flags };
            let g = world_face(&local, Mat4::IDENTITY);
            assert_eq!(
                (g.x, g.y, g.bounds, g.flags, g.corners),
                (f.x, f.y, f.bounds, f.flags, f.corners),
                "pack {pi} face {fi}"
            );
            for k in 0..f.corners {
                assert!(
                    (g.vertices[k] - f.vertices[k]).abs().max_element() < 1.0 / 256.0,
                    "pack {pi} face {fi}: {:?} written as {:?}",
                    f.vertices,
                    g.vertices
                );
            }
            n += 1;
        }
    }
    assert!(n > 100_000, "{n} faces");
}

const LIGHT: &str = "XO_STREETLIGHTD_1A_00";
/// An open spot in the plaza east of the Bayview fountains (the mod in the
/// map editor's walk-through puts its lights here), and the way in.
const SPOT: Vec3 = Vec3::new(-327.0, -535.0, 21.269472);
const WAY_IN: Vec3 = Vec3::new(0.0, -1.0, 0.0);

fn with_mod(props: Vec<PropEdit>) -> World {
    let mut w = l4ra();
    let m = MapMod { props, ..Default::default() };
    w.apply_map_mods("L4RA", &[("test".to_string(), m)]);
    w
}

fn add(solid: &str, at: Vec3) -> PropEdit {
    PropEdit::Add {
        id: "a".into(),
        region: "L4RA".into(),
        solid: solid.into(),
        position: at,
        rotation: glam::Mat3::IDENTITY,
    }
}

/// A car at `speed` m/s, full throttle, at `hz` steps a second, from 40 m
/// before `target` straight at it: how far past `target` its centre gets.
fn drive_at(w: &World, car: &crate::car::Car, target: Vec3, dir: Vec3, speed: f32, hz: f32) -> f32 {
    let start = target - dir * 40.0 + Vec3::Z * 0.6;
    let mut s = crate::drive::CarState::new(start, dir.y.atan2(dir.x));
    s.velocity = dir * speed;
    let controls = crate::drive::Controls { throttle: 1.0, ..Default::default() };
    let mut furthest = f32::MIN;
    for _ in 0..(hz * 1.5) as usize {
        s.step(car, w, controls, 1.0 / hz);
        furthest = furthest.max((s.position - target).dot(dir));
    }
    furthest
}

fn index_of_light_with_collision(w: &World) -> usize {
    let s = w.solid_named(LIGHT).unwrap();
    (0..w.placements.len())
        .find(|&i| w.placements[i].solid == s && !w.collision_of_placement(s, w.placements[i].transform).is_empty())
        .unwrap()
}

/// A street light added in the open plaza stops a car at top speed (it
/// drives through the empty spot) at 30, 60, 120 and 200 steps a second,
/// with the original street light's own six walls.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn an_added_street_light_stops_a_car_at_top_speed_at_every_rate() {
    let root = crate::testkit::game_root();
    let car = crate::car::Car::load(&root, "240SX").unwrap();
    let plain = l4ra();
    let lit = with_mod(vec![add(LIGHT, SPOT)]);
    assert_eq!(lit.map_overlay.collision.solids_with_collision, 1);
    let added = lit.map_overlay.collision.added;
    assert!(added >= 6 && added % 6 == 0, "{added} faces written (six per pack)");
    for hz in [30.0, 60.0, 120.0, 200.0] {
        let through = drive_at(&plain, &car, SPOT, WAY_IN, 70.0, hz);
        let stopped = drive_at(&lit, &car, SPOT, WAY_IN, 70.0, hz);
        eprintln!("{hz} Hz: without the light {through:.2} m past it, with it {stopped:.2} m");
        assert!(through > 3.0, "{hz} Hz: the spot is open without the light ({through} m)");
        assert!(stopped < 0.0, "{hz} Hz: the car's centre passed the added light ({stopped} m)");
    }
}

/// A moved original street light takes its collision along: nothing is
/// left where it stood, and it blocks at its new place; a deleted one
/// leaves no collision.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_moved_street_light_takes_its_collision_along() {
    let root = crate::testkit::game_root();
    let car = crate::car::Car::load(&root, "240SX").unwrap();
    let plain = l4ra();
    let i = index_of_light_with_collision(&plain);
    let s = plain.placements[i].solid;
    let old = plain.placements[i].transform;
    let own = plain.collision_of_placement(s, old).len();
    let target = plain.placement_ref("L4RA", i).unwrap();
    let rotation = glam::Mat3::from_quat(glam::Quat::from_rotation_z(0.7));
    let moved = with_mod(vec![PropEdit::Move { target: target.clone(), position: SPOT, rotation }]);
    assert!(moved.collision_of_placement(s, old).is_empty(), "nothing left at its old place");
    assert_eq!(moved.map_overlay.collision.cut, own);
    assert_eq!(moved.collision_of_placement(s, moved.placements[i].transform).len() % 6, 0);
    assert!(!moved.collision_of_placement(s, moved.placements[i].transform).is_empty());
    for hz in [30.0, 60.0, 120.0, 200.0] {
        let stopped = drive_at(&moved, &car, SPOT, WAY_IN, 70.0, hz);
        assert!(stopped < 0.0, "{hz} Hz: the car's centre passed the moved light ({stopped} m)");
    }
    let deleted = with_mod(vec![PropEdit::Delete { target }]);
    assert!(deleted.collision_of_placement(s, old).is_empty());
    assert_eq!(deleted.map_overlay.collision.added, 0);
}

/// Without a mod, and with a mod that only adds knock-over props and
/// solids the original gives no collision, the packs are the original's.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn knock_over_props_and_solids_without_collision_leave_the_packs_alone() {
    let plain = l4ra();
    let w = with_mod(vec![
        add("XO_ROADBARRIERB_1A_00", SPOT),
        add("XO_FENCEF_1A_00", SPOT + Vec3::X * 10.0),
        add("XO_PARKBENCHA_1A_00", SPOT + Vec3::X * 20.0),
    ]);
    assert_eq!(w.map_overlay.collision, PropCollision::default());
    let grids = |w: &World| w.ground.grids().map(|g| (g.cells.clone(), g.refs.clone())).collect::<Vec<_>>();
    assert!(grids(&plain) == grids(&w), "the packs changed");
    assert_eq!(w.collision_faces.len(), plain.collision_faces.len());
    assert_eq!(w.smackables.len(), plain.smackables.len() + 2, "the barrier and the fence knock over");
}
