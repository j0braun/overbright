//! The map editor's session on the map overlay: the live re-apply after each op (and undo, redo, refused groups)
//! gives the world a full apply of the mod makes, on a made-up region with twins and knock-over props and on
//! Bayview; and the time one drag step takes (an ignored bench).

use glam::{Mat3, Mat4, Vec3};
use nfsu2::studio_api::smackable;
use nfsu2::studio_api::world::{HIDDEN_FLAG, Placement, World, parts_of};
use ob_formats::formats::solid::Solid;
use ob_map::tools::map::format::{MapMod, PropEdit};
use ob_map::tools::map::ops::{Op, PropKey};
use ob_studio::session::MapSession;

mod testkit {
    #![allow(dead_code)]
    ob_testkit::bind!();
}

/// Flag 0x0100: a placement an add never takes after (as the library's map overlay has it).
const GROUPED: u16 = 0x0100;

/// The same placement, bit for bit (as the library's map overlay compares them).
fn same_placement(a: &Placement, b: &Placement) -> bool {
    a.solid == b.solid
        && a.low_solid == b.low_solid
        && a.mid_solid == b.mid_solid
        && a.transform == b.transform
        && a.section == b.section
        && a.flags == b.flags
        && a.cull_radius.to_bits() == b.cull_radius.to_bits()
}

const REGION: &str = "L4RA";
const FOLDER: &str = "my-map";
const SOLIDS: [&str; 5] = ["XC_CONE_01", "XC_BARRIER_03", "XC_LAMP_02", "XC_TREE_07", "XC_NOWHERE_01"];

struct Rng(u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    pub fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
}

fn solid(name: &str) -> Solid {
    Solid {
        name: name.into(),
        hash: ob_formats::formats::hash::bin_hash(name),
        bbox_min: [-0.5, -0.5, 0.0],
        bbox_max: [0.5, 0.5, 1.0],
        textures: Vec::new(),
        vertices: Vec::new(),
        normals: Vec::new(),
        light_materials: Vec::new(),
        indices: Vec::new(),
        materials: Vec::new(),
        mesh_flags: 0,
        markers: Vec::new(),
        light_markers: Vec::new(),
    }
}

fn prop(placement: usize, rest: Mat4, key: u32) -> smackable::Prop {
    smackable::Prop {
        placement,
        scenery: key as u16,
        info: ob_formats::formats::smackable::ObjectInfo {
            name: format!("ZCS_Prop{key}"),
            key,
            mass: 0.1 * key as f32,
            inertia: Mat3::IDENTITY,
            centre_of_mass: Vec3::Z * 0.5,
            break_impulse: 1.0,
            restitution: 0.0,
            friction: 0.5,
            material: key as usize % 19,
        },
        shape: vec![Vec3::X, Vec3::Y * key as f32],
        offset: Vec3::ZERO,
        raw_shape: vec![Vec3::Z],
        links: Default::default(),
        reach: 1.0,
        above: 1.0,
        below: 0.5,
        rest,
        world_min: Vec3::splat(-1.0),
        world_max: Vec3::splat(1.0),
        radius: 1.0,
    }
}

/// A region of 300 placements of four solids over 300 m, in three
/// sections: twins (several of one solid exactly at one place), some
/// hidden as loaded, some with flag 0x0100, knock-over props (one
/// placement with two) and scenery links.
fn world() -> World {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut w = World::flat(400.0);
    w.solids = SOLIDS.iter().map(|s| solid(s)).collect();
    let mut placements = Vec::new();
    while placements.len() < 300 {
        let at = Vec3::new(r.f(-150.0, 150.0), r.f(-150.0, 150.0), r.f(0.0, 3.0));
        let mut flags = 0u16;
        if r.one_in(8) {
            flags |= GROUPED;
        }
        if r.one_in(10) {
            flags |= HIDDEN_FLAG;
        }
        if r.one_in(5) {
            flags |= 0x0004;
        }
        let p = Placement {
            solid: r.below(4),
            low_solid: r.one_in(2).then(|| r.below(4)),
            mid_solid: r.one_in(3).then(|| r.below(4)),
            transform: Mat4::from_rotation_z(r.f(0.0, 6.0))
                * Mat4::from_scale(Vec3::splat(r.f(0.8, 1.2)))
                * Mat4::IDENTITY,
            section: [618, 619, 702][r.below(3)],
            flags,
            cull_radius: r.f(4.0, 40.0),
        };
        let p = Placement { transform: Mat4::from_translation(at) * p.transform, ..p };
        // Twins: two to four of one solid at exactly one place.
        let twins = if r.one_in(12) { 2 + r.below(3) } else { 1 };
        for _ in 0..twins {
            placements.push(p.clone());
        }
    }
    w.placements = placements;
    let n = w.placements.len();
    let mut props = Vec::new();
    for i in 0..n {
        if r.one_in(5) {
            props.push(prop(i, w.placements[i].transform, i as u32));
        }
    }
    props.insert(3, prop(props[2].placement, Mat4::IDENTITY, 999));
    w.smackables = props;
    w.link_placements = (0..80).map(|_| (!r.one_in(4)).then(|| r.below(n))).collect();
    w
}

/// The world a full apply of `map` makes.
fn full(map: &MapMod) -> World {
    let mut w = world();
    w.apply_map_mods(REGION, &[(FOLDER.to_string(), map.clone())]);
    w
}

fn assert_same(a: &World, b: &World, what: &str) {
    assert_eq!(a.placements.len(), b.placements.len(), "{what}: placements");
    for (i, (p, q)) in a.placements.iter().zip(&b.placements).enumerate() {
        assert!(same_placement(p, q), "{what}: placement {i}: {p:?} / {q:?}");
    }
    assert_eq!(format!("{:?}", a.smackables), format!("{:?}", b.smackables), "{what}: knock-over props");
    assert_eq!(a.link_placements, b.link_placements, "{what}: links");
    let (o, p) = (&a.map_overlay, &b.map_overlay);
    assert_eq!(o.originals, p.originals, "{what}: originals");
    assert_eq!(o.original_at, p.original_at, "{what}: original_at");
    assert_eq!(o.added, p.added, "{what}: added");
    assert_eq!(o.first_added, p.first_added, "{what}: first_added");
    assert_eq!((o.moved, o.deleted, o.missing), (p.moved, p.deleted, p.missing), "{what}: counts");
    assert_eq!(o.collision, p.collision, "{what}: static collision counts");
    assert!(static_collision(a) == static_collision(b), "{what}: static collision");
}

/// The collision packs, cell by cell and face by face.
fn static_collision(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    for (pi, g) in w.ground.grids().enumerate() {
        out.push(format!("{:?} {:?}", g.cells, g.refs));
        let mut fi = 0;
        while let Some(f) = w.ground.face_at(pi, fi) {
            out.push(format!("{f:?}"));
            fi += 1;
        }
    }
    out
}

fn rotation(r: &mut Rng) -> Mat3 {
    match r.below(3) {
        0 => Mat3::IDENTITY,
        _ => Mat3::from_rotation_z(r.f(-3.0, 3.0)),
    }
}

/// Somewhere: near an existing placement (often the same solid's, to
/// change which one an add takes after), or anywhere.
fn spot(s: &MapSession, r: &mut Rng) -> Vec3 {
    if r.one_in(4) {
        return Vec3::new(r.f(-170.0, 170.0), r.f(-170.0, 170.0), r.f(0.0, 2.0));
    }
    let p = &s.world.placements[r.below(s.world.placements.len())];
    p.transform.w_axis.truncate() + Vec3::new(r.f(-3.0, 3.0), r.f(-3.0, 3.0), 0.0)
}

fn some_key(s: &MapSession, r: &mut Rng) -> Option<PropKey> {
    let adds: Vec<&str> = s.map.props.iter().filter_map(PropEdit::add_id).collect();
    if !adds.is_empty() && r.one_in(3) {
        return Some(PropKey::Added(adds[r.below(adds.len())].to_string()));
    }
    // Mostly an original the mod already edits (twins among them),
    // sorted: the map's own order differs from run to run, and the
    // ops must not.
    let mut edited: Vec<usize> = s.world.map_overlay.originals.keys().copied().collect();
    edited.sort_unstable();
    let i = if !edited.is_empty() && r.one_in(2) {
        edited[r.below(edited.len())]
    } else {
        r.below(s.base.placements.len())
    };
    s.key_of(i)
}

fn random_op(s: &MapSession, r: &mut Rng) -> Option<Op> {
    Some(match r.below(12) {
        0..=3 => {
            let key = some_key(s, r)?;
            Op::Place { key, position: spot(s, r), rotation: rotation(r) }
        }
        4 => Op::Delete { key: some_key(s, r)? },
        5 => Op::Reset { key: some_key(s, r)? },
        6 => {
            let id = match r.below(4) {
                0 => Some(format!("x{}", r.below(40))),
                1 => s.map.props.iter().filter_map(PropEdit::add_id).next().map(str::to_string),
                _ => None,
            };
            Op::Add {
                solid: SOLIDS[r.below(SOLIDS.len())].to_string(),
                position: spot(s, r),
                rotation: rotation(r),
                id,
                ground: r.one_in(4),
            }
        }
        7 => Op::Duplicate { key: some_key(s, r)?, position: spot(s, r) },
        8 => Op::DropToGround { key: some_key(s, r)? },
        9 => {
            // A group, sometimes with an op that is refused.
            let mut ops = Vec::new();
            for _ in 0..2 + r.below(2) {
                ops.push(random_op(s, r)?);
            }
            if r.one_in(3) {
                ops.push(Op::Delete { key: PropKey::Added("no-such-add".into()) });
            }
            Op::Group { label: "Group".into(), ops }
        }
        _ => {
            // The mod put in place whole: its edits in another order,
            // an add twice under one id now and then.
            let mut map = s.map.clone();
            let n = map.props.len();
            for k in (1..n).rev() {
                if r.one_in(2) {
                    map.props.swap(k, r.below(k + 1));
                }
            }
            if r.one_in(3)
                && let Some(a) = map.props.iter().find(|e| matches!(e, PropEdit::Add { .. })).cloned()
            {
                map.props.push(a);
            }
            Op::PutMod { map }
        }
    })
}

/// Random op sequences (moves, deletes, resets, adds keyed by id,
/// duplicates, groups, undo and redo, whole mods in another order) on a
/// region with twins: after every step the live re-apply's world is the
/// one a full apply of the mod makes.
#[test]
fn the_live_apply_is_a_full_apply() {
    for seed in 1..=12u64 {
        let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let mut s = MapSession::new(REGION, FOLDER, MapMod::default(), world());
        for step in 0..150 {
            let what = match r.below(10) {
                0 => format!("undo {:?}", s.undo()),
                1 => format!("redo {:?}", s.redo()),
                _ => match random_op(&s, &mut r) {
                    Some(op) => {
                        let label = op.label(&s.map);
                        format!("{label}: {:?}", s.run(op).map(|c| c.changed))
                    }
                    None => continue,
                },
            };
            assert_same(&s.world, &full(&s.map), &format!("seed {seed} step {step} ({what})"));
        }
        assert!(s.map.props.len() > 10, "seed {seed}: {} edits", s.map.props.len());
    }
}

/// An original deleted and put back at the place its move already
/// gives it, in one group (the mod the same as before): its knock-over
/// prop comes back too, whether the group is made or refused.
#[test]
fn a_delete_taken_back_in_one_group_keeps_the_knock_over_prop() {
    for refused in [false, true] {
        let mut s = MapSession::new(REGION, FOLDER, MapMod::default(), world());
        let i = s.world.smackables[0].placement;
        let key = s.key_of(i).unwrap();
        let (position, rotation) = (Vec3::new(24.5, 29.5, 0.0), Mat3::IDENTITY);
        s.run(Op::Place { key: key.clone(), position, rotation }).unwrap();
        let props = s.world.smackables.len();
        let mut ops = vec![Op::Delete { key: key.clone() }, Op::Place { key, position, rotation }];
        if refused {
            ops.push(Op::Delete { key: PropKey::Added("no-such-add".into()) });
        }
        let r = s.run(Op::Group { label: "Group".into(), ops });
        assert_eq!(r.is_err(), refused);
        assert_eq!(s.world.smackables.len(), props, "refused {refused}");
        assert_same(&s.world, &full(&s.map), &format!("refused {refused}"));
    }
}

/// On Bayview with a mod of 2,000 edits: after random gestures the
/// live re-apply's world is the full apply's.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_live_apply_is_a_full_apply_on_bayview() {
    let root = testkit::game_root();
    let load = || World::load(std::path::Path::new(&root), REGION).unwrap();
    let w = load();
    let map = mod_of(&w, REGION, 2000);
    let mut s = MapSession::new(REGION, FOLDER, map, w);
    let mut r = Rng(4242);
    for _ in 0..60 {
        if let Some(op) = random_op(&s, &mut r).filter(|o| !matches!(o, Op::PutMod { .. })) {
            let _ = s.run(op);
        }
        if r.one_in(8) {
            s.undo();
        }
    }
    let mut again = load();
    again.apply_map_mods(REGION, &[(FOLDER.to_string(), s.map.clone())]);
    assert_same(&s.world, &again, "Bayview");
}

/// A mod of `edits` edits on `w` (70 % moves, 10 % deletes, 20 % adds),
/// spread over the region.
fn mod_of(w: &World, region: &str, edits: usize) -> MapMod {
    let mut m = MapMod::default();
    let n = w.placements.len();
    let stride = (n / (edits + 1)).max(1);
    for k in 0..edits {
        let i = (k * stride + 7) % n;
        let p = &w.placements[i];
        let (at, rot) = parts_of(p.transform);
        // A scaled placement's rotation is no mod file's (rows must be unit axes).
        let rot = if ob_map::tools::map::format::check_rotation(&rot).is_ok() { rot } else { Mat3::IDENTITY };
        match k % 10 {
            0..=6 => {
                let target = w.placement_ref(region, i).unwrap();
                m.props.push(PropEdit::Move { target, position: at + Vec3::new(1.0, 0.5, 0.0), rotation: rot });
            }
            7 => m.props.push(PropEdit::Delete { target: w.placement_ref(region, i).unwrap() }),
            _ => m.props.push(PropEdit::Add {
                id: m.fresh_add_id(),
                region: region.into(),
                solid: w.solids[p.solid].name.clone(),
                position: at + Vec3::new(3.0, 0.0, 0.0),
                rotation: rot,
            }),
        }
    }
    m
}

fn median(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(f64::total_cmp);
    (v[v.len() / 2], v[v.len() * 9 / 10])
}

/// Times `f` `reps` times (ms): median and 90th percentile.
fn time(reps: usize, mut f: impl FnMut(usize)) -> (f64, f64) {
    f(0);
    median(
        (1..=reps)
            .map(|k| {
                let t = std::time::Instant::now();
                f(k);
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect(),
    )
}

/// One drag step (one `Op::Place`) on a mod of 10 and of 2,000 edits on
/// Bayview: run with `--ignored --nocapture` (`NFSU2_ROOT` set).
#[test]
#[ignore = "bench: needs NFSU2_ROOT; run with --ignored --nocapture"]
fn bench_one_gesture() {
    let root = testkit::game_root();
    let load = || World::load(std::path::Path::new(&root), "L4RA").unwrap();
    let world = load();
    let reps: usize = std::env::var("BENCH_REPS").ok().and_then(|r| r.parse().ok()).unwrap_or(101);
    println!("L4RA: {} placements, {} knock-over props", world.placements.len(), world.smackables.len());
    for edits in [10, 2000] {
        let map = mod_of(&world, "L4RA", edits);
        let mut s = MapSession::new("L4RA", "bench", map, load());
        // An original already moved (its edit in the middle of the list).
        let moved =
            s.map.props.iter().filter_map(PropEdit::target).nth(edits / 3).cloned().map(PropKey::Original).unwrap();
        let i = s.placement_of(&moved).unwrap();
        let (at, rot) = parts_of(s.world.placements[i].transform);
        let (m1, p1) = time(reps, |k| {
            let position = at + Vec3::new(0.01 * k as f32, 0.0, 0.0);
            if let Err(e) = s.run(Op::Place { key: moved.clone(), position, rotation: rot }) {
                panic!("{e:?}: {:?}", s.refused_why);
            }
        });
        // An untouched original (a fresh drag's first step).
        let fresh_from = (0..s.world.placements.len())
            .find(|&j| j % 13 == 5 && !s.world.map_overlay.originals.contains_key(&j))
            .unwrap();
        let mut next = fresh_from;
        let (m2, p2) = time(reps.min(31), |_| {
            while s.world.map_overlay.originals.contains_key(&next) || s.world.map_overlay.added.contains_key(&next) {
                next += 1;
            }
            let key = s.key_of(next).unwrap();
            let (at, rot) = parts_of(s.world.placements[next].transform);
            let rot = if ob_map::tools::map::format::check_rotation(&rot).is_ok() { rot } else { Mat3::IDENTITY };
            s.run(Op::Place { key, position: at + Vec3::Z, rotation: rot }).unwrap();
        });
        // An added prop (its add in the middle of the list).
        let id = s.map.props.iter().filter_map(PropEdit::add_id).nth(edits / 20).unwrap().to_string();
        let key = PropKey::Added(id);
        let j = s.placement_of(&key).unwrap();
        let (at, _) = parts_of(s.world.placements[j].transform);
        let (m3, p3) = time(reps, |k| {
            let position = at + Vec3::new(0.0, 0.01 * k as f32, 0.0);
            s.run(Op::Place { key: key.clone(), position, rotation: Mat3::IDENTITY }).unwrap();
        });
        println!(
            "bench {edits} edits: move a moved original {m1:.3} ms (p90 {p1:.3}); \
             first move of an original {m2:.3} ms (p90 {p2:.3}); move an added prop {m3:.3} ms (p90 {p3:.3})"
        );
    }
}
