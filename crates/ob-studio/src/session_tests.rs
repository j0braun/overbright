//! The session and its ops: each op's JSON round trip, the undo step
//! names, refusals that change nothing, replay from JSON.

use glam::{Mat3, Vec2, Vec3};

use super::*;
use nfsu2::studio_api::world::Placement;
use ob_map::tools::map::format::{PropRef, RaceKind};
use ob_map::tools::map::ops::Change;

const REGION: &str = "L4RA";

fn solid(name: &str) -> ob_formats::formats::solid::Solid {
    ob_formats::formats::solid::Solid {
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

/// A flat floor at z = 0 with two cones standing on it and a barrier
/// 2 m above it (the same world as the edit tests' in the library).
fn world() -> World {
    let mut w = World::flat(200.0);
    w.solids = vec![solid("XC_CONE_01"), solid("XC_BARRIER_03")];
    let place = |solid, at: Vec3| Placement {
        solid,
        low_solid: None,
        mid_solid: None,
        transform: glam::Mat4::from_translation(at),
        section: 618,
        flags: 0,
        cull_radius: 5.0,
    };
    w.placements = vec![
        place(0, Vec3::new(10.0, 0.0, 0.0)),
        place(0, Vec3::new(20.0, 0.0, 0.0)),
        place(1, Vec3::new(30.0, 0.0, 2.0)),
    ];
    w
}

fn session() -> MapSession {
    MapSession::new(REGION, "my-map", MapMod::default(), world())
}

fn cone(s: &MapSession, i: usize) -> PropKey {
    s.key_of(i).unwrap()
}

/// The variant's tag; exhaustive, so a new op must get a sample below.
fn tag(op: &Op) -> &'static str {
    match op {
        Op::Place { .. } => "place",
        Op::Delete { .. } => "delete",
        Op::Reset { .. } => "reset",
        Op::Add { .. } => "add",
        Op::Duplicate { .. } => "duplicate",
        Op::DropToGround { .. } => "drop_to_ground",
        Op::NewRace { .. } => "new_race",
        Op::DeleteRace { .. } => "delete_race",
        Op::RenameRace { .. } => "rename_race",
        Op::SetRaceId { .. } => "set_race_id",
        Op::SetTrack { .. } => "set_track",
        Op::SetKind { .. } => "set_kind",
        Op::SetLaps { .. } => "set_laps",
        Op::SetDirection { .. } => "set_direction",
        Op::AddPoint { .. } => "add_point",
        Op::MovePoint { .. } => "move_point",
        Op::DeletePoint { .. } => "delete_point",
        Op::MoveCheckpoint { .. } => "move_checkpoint",
        Op::PutRace { .. } => "put_race",
        Op::Details { .. } => "details",
        Op::PutMod { .. } => "put_mod",
        Op::Group { .. } => "group",
    }
}
const VARIANTS: usize = 22;

fn odd_rotation() -> Mat3 {
    Mat3::from_euler(glam::EulerRot::ZYX, 0.3, -0.1, 1.0e-7)
}

/// One sample of every op, with awkward floats and every optional field
/// both given and left out.
fn every_op() -> Vec<Op> {
    let original = PropKey::Original(PropRef {
        region: REGION.into(),
        section: 618,
        solid: "XC_CONE_01".into(),
        at: Vec3::new(10.1, -0.000_123_4, 1.0e-30),
        nth: 2,
    });
    let first = PropKey::Original(PropRef {
        nth: 0,
        ..match &original {
            PropKey::Original(r) => r.clone(),
            _ => unreachable!(),
        }
    });
    let added = PropKey::Added("kiosk-7".into());
    let mut race = CustomRace::new("loop", 4001, RaceKind::Circuit);
    race.checkpoints = vec![Vec2::new(0.1, 0.2), Vec2::new(-3.0e5_f32.recip(), 7.0)];
    race.grid = vec![Spot { position: Vec3::new(1.0, 2.0, 3.0), heading: 91.5 }];
    race.start = Some(Spot { position: Vec3::ONE, heading: -179.99 });
    let map = MapMod {
        name: "Docks".into(),
        author: "me".into(),
        description: "x".into(),
        props: vec![
            PropEdit::Delete {
                target: match &original {
                    PropKey::Original(r) => r.clone(),
                    _ => unreachable!(),
                },
            },
            PropEdit::Add {
                id: "a1".into(),
                region: REGION.into(),
                solid: "XC_CONE_01".into(),
                position: Vec3::new(1.5, 2.5, 3.5),
                rotation: odd_rotation(),
            },
        ],
        races: vec![race.clone()],
    };
    let pt = |part, index| RacePoint { part, index };
    vec![
        Op::Place { key: original.clone(), position: Vec3::new(0.1, 0.2, 0.3), rotation: odd_rotation() },
        Op::Delete { key: added.clone() },
        Op::Reset { key: first.clone() },
        Op::Add { solid: "XC_CONE_01".into(), position: Vec3::X, rotation: Mat3::IDENTITY, id: None, ground: false },
        Op::Add {
            solid: "XC_CONE_01".into(),
            position: Vec3::Y,
            rotation: odd_rotation(),
            id: Some("b".into()),
            ground: true,
        },
        Op::Duplicate { key: first.clone(), position: Vec3::new(f32::MIN_POSITIVE, -0.0, 99_999.99) },
        Op::DropToGround { key: added.clone() },
        Op::NewRace { track: 4001, kind: RaceKind::StreetX, id: None },
        Op::NewRace { track: 7, kind: RaceKind::Drift, id: Some("drift".into()) },
        Op::DeleteRace { race: "loop".into() },
        Op::RenameRace { race: "loop".into(), name: "Löop \"1\", fast\n".into() },
        Op::SetRaceId { race: "loop".into(), id: "loop-2".into() },
        Op::SetTrack { race: "loop".into(), track: 65535 },
        Op::SetKind { race: "loop".into(), kind: RaceKind::Sprint },
        Op::SetLaps { race: "loop".into(), laps: 3 },
        Op::SetDirection { race: "loop".into(), backward: true },
        Op::AddPoint {
            race: "loop".into(),
            part: RacePart::Checkpoint,
            index: Some(1),
            position: Vec3::new(5.0, 6.0, 0.0),
            heading: 0.0,
        },
        Op::AddPoint {
            race: "loop".into(),
            part: RacePart::Finish,
            index: None,
            position: Vec3::new(5.0, 6.0, 7.0),
            heading: 33.3,
        },
        Op::MovePoint { race: "loop".into(), point: pt(RacePart::Grid, 0), position: Vec3::Z, heading: Some(1.0) },
        Op::MovePoint { race: "loop".into(), point: pt(RacePart::Start, 0), position: Vec3::Z, heading: None },
        Op::DeletePoint { race: "loop".into(), point: pt(RacePart::Checkpoint, 1) },
        Op::MoveCheckpoint { race: "loop".into(), index: 0, later: true },
        Op::PutRace { race: "loop".into(), to: race },
        Op::Details { name: "N".into(), author: String::new(), description: "two\nlines".into() },
        Op::PutMod { map },
        Op::Group {
            label: "Delete 2 props".into(),
            ops: vec![
                Op::Delete { key: first },
                Op::Group { label: "inner".into(), ops: vec![Op::Delete { key: added }] },
            ],
        },
    ]
}

#[test]
fn every_op_round_trips_through_json() {
    let ops = every_op();
    let tags: std::collections::BTreeSet<_> = ops.iter().map(tag).collect();
    assert_eq!(tags.len(), VARIANTS, "a sample of every op");
    for op in &ops {
        let json = op.to_json();
        assert!(!json.contains('\n'), "one line: {json}");
        assert!(json.starts_with(&format!("{{\"op\":\"{}\"", tag(op))), "tagged by its name: {json}");
        let back = Op::from_json(&json).unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(&back, op, "{json}");
        assert_eq!(back.to_json(), json, "written again: the same text");
    }
    // As an ops.jsonl, with a comment and blank lines.
    let text = format!("# ops\n\n{}\n", ops.iter().map(Op::to_json).collect::<Vec<_>>().join("\n"));
    assert_eq!(Op::read_lines(&text).unwrap(), ops);
    let bad = format!("{}\n{{\"op\":\"fly\"}}\n", ops[0].to_json());
    assert_eq!(Op::read_lines(&bad).unwrap_err().0, 2, "the bad line is named");
}

#[test]
fn json_shapes_are_plain() {
    let op = Op::Place {
        key: PropKey::Original(PropRef {
            region: "L4RA".into(),
            section: 618,
            solid: "XC".into(),
            at: Vec3::new(1.0, 2.0, 3.0),
            nth: 0,
        }),
        position: Vec3::new(1.5, 2.0, 3.0),
        rotation: Mat3::from_cols(Vec3::new(0.0, -1.0, 0.0), Vec3::X, Vec3::Z),
    };
    // nth 0 is left out; a rotation is its rows (the local axes).
    assert_eq!(
        op.to_json(),
        r#"{"op":"place","key":{"original":{"region":"L4RA","section":618,"solid":"XC","at":[1.0,2.0,3.0]}},"position":[1.5,2.0,3.0],"rotation":[[0.0,1.0,0.0],[-1.0,0.0,0.0],[0.0,0.0,1.0]]}"#
    );
    let added = Op::Delete { key: PropKey::Added("a3".into()) };
    assert_eq!(added.to_json(), r#"{"op":"delete","key":{"added":"a3"}}"#);
    let pt = r#"{"op":"delete_point","race":"r","point":{"part":"start"}}"#;
    assert_eq!(
        Op::from_json(pt).unwrap(),
        Op::DeletePoint { race: "r".into(), point: RacePoint { part: RacePart::Start, index: 0 } }
    );
    assert!(Op::from_json(r#"{"op":"delete","key":{"added":"a3"},"extra":1}"#).is_err(), "unknown fields refused");
    assert!(Op::from_json(r#"{"op":"set_laps","race":"r","laps":-1}"#).is_err());
    let change = Change {
        label: "Add X".into(),
        changed: true,
        added: vec![PropKey::Added("a1".into())],
        race: None,
        point: None,
    };
    let json = serde_json::to_string(&change).unwrap();
    assert_eq!(json, r#"{"label":"Add X","changed":true,"added":[{"added":"a1"}]}"#);
    assert_eq!(serde_json::from_str::<Change>(&json).unwrap(), change);
}

#[test]
fn floats_round_trip_exactly() {
    // A spread of bit patterns over the finite floats.
    let mut x: u32 = 0x1234_5678;
    for _ in 0..20_000 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let f = f32::from_bits(x);
        if !f.is_finite() {
            continue;
        }
        let op = Op::Place {
            key: PropKey::Added("a1".into()),
            position: Vec3::new(f, -f, f * 0.5),
            rotation: Mat3::from_diagonal(Vec3::splat(f)),
        };
        assert_eq!(Op::from_json(&op.to_json()).unwrap(), op, "{f:e}");
    }
}

#[test]
fn a_mod_round_trips_through_json() {
    let Op::PutMod { map } = every_op().into_iter().find(|o| tag(o) == "put_mod").unwrap() else { unreachable!() };
    let json = serde_json::to_string(&map).unwrap();
    assert_eq!(serde_json::from_str::<MapMod>(&json).unwrap(), map);
    assert!(json.contains(r#""kind":"circuit""#), "{json}");
}

#[test]
fn prop_ops_are_one_undo_step_each_with_the_editors_names() {
    let mut s = session();
    let n = s.world.placements.len();
    let barrier = cone(&s, 2);
    let c = s.run(Op::DropToGround { key: barrier.clone() }).unwrap();
    assert_eq!(c.label, "Drop to ground XC_BARRIER_03");
    assert!(c.changed);
    assert!(s.world.placements[2].transform.w_axis.z.abs() < 1e-4);
    // The moved original keeps its key.
    assert_eq!(s.key_of(2), Some(barrier.clone()));
    assert_eq!(s.placement_of(&barrier), Some(2));

    let c = s.run(Op::Duplicate { key: cone(&s, 0), position: Vec3::new(10.0, 5.0, 0.0) }).unwrap();
    assert_eq!(c.label, "Duplicate XC_CONE_01");
    assert_eq!(c.added, vec![PropKey::Added("a1".into())]);
    let copy = s.placement_of(&c.added[0]).unwrap();
    assert!(copy >= n);
    assert!(s.adds_changed);

    let c = s
        .run(Op::Add {
            solid: "XC_BARRIER_03".into(),
            position: Vec3::new(0.0, 0.0, 5.0),
            rotation: Mat3::IDENTITY,
            id: Some("gate".into()),
            ground: true,
        })
        .unwrap();
    assert_eq!(c.label, "Add XC_BARRIER_03");
    assert_eq!(c.added, vec![PropKey::Added("gate".into())]);
    let gate = s.placement_of(&c.added[0]).unwrap();
    assert!(s.world.placements[gate].transform.w_axis.z.abs() < 1e-4, "stood on the ground");
    assert_eq!(s.key_of(gate), Some(PropKey::Added("gate".into())));

    let c = s
        .run(Op::Place {
            key: PropKey::Added("gate".into()),
            position: Vec3::new(1.0, 1.0, 0.0),
            rotation: Mat3::IDENTITY,
        })
        .unwrap();
    assert_eq!(c.label, "Move XC_BARRIER_03", "an add goes by its solid's name");
    let c = s.run(Op::Delete { key: cone(&s, 1) }).unwrap();
    assert_eq!(c.label, "Delete XC_CONE_01");
    let c = s.run(Op::Reset { key: barrier.clone() }).unwrap();
    assert_eq!(c.label, "Reset XC_BARRIER_03");
    assert_eq!(s.world.placements[2].transform.w_axis.z, 2.0);

    // Undo walks back through the same names.
    let names: Vec<String> = std::iter::from_fn(|| s.undo()).collect();
    assert_eq!(
        names,
        [
            "Reset XC_BARRIER_03",
            "Delete XC_CONE_01",
            "Move XC_BARRIER_03",
            "Add XC_BARRIER_03",
            "Duplicate XC_CONE_01",
            "Drop to ground XC_BARRIER_03"
        ]
    );
    assert_eq!(s.map, MapMod::default());
    assert_eq!(s.world.placements.len(), n);
    assert_eq!(s.redo().as_deref(), Some("Drop to ground XC_BARRIER_03"));
}

#[test]
fn a_refused_op_changes_nothing_and_a_group_is_all_or_nothing() {
    let mut s = session();
    let before_world: Vec<_> = s.world.placements.iter().map(|p| (p.transform, p.flags)).collect();
    assert_eq!(s.run(Op::Delete { key: PropKey::Added("nope".into()) }), Err(Refused::NoPlacement));
    assert_eq!(s.run(Op::Reset { key: cone(&s, 0) }), Err(Refused::NotEdited));
    assert_eq!(s.run(Op::SetLaps { race: "x".into(), laps: 2 }), Err(Refused::NoRace));
    let off = Op::Place { key: cone(&s, 0), position: Vec3::new(f32::NAN, 0.0, 0.0), rotation: Mat3::IDENTITY };
    assert_eq!(s.run(off), Err(Refused::BadValue));
    let group = Op::Group {
        label: "Delete 2 props".into(),
        ops: vec![
            Op::Delete { key: cone(&s, 0) },
            Op::Add {
                solid: "XC_CONE_01".into(),
                position: Vec3::ZERO,
                rotation: Mat3::IDENTITY,
                id: None,
                ground: false,
            },
            Op::Delete { key: PropKey::Added("missing".into()) },
        ],
    };
    assert_eq!(s.run(group), Err(Refused::NoPlacement));
    assert_eq!(s.map, MapMod::default());
    assert!(s.undo.undo_label().is_none(), "no step");
    let after: Vec<_> = s.world.placements.iter().map(|p| (p.transform, p.flags)).collect();
    assert_eq!(after, before_world, "the world as it was");

    // A group that goes through: one step under its name.
    let two = Op::Group {
        label: "Delete 2 props".into(),
        ops: vec![Op::Delete { key: cone(&s, 0) }, Op::Delete { key: cone(&s, 1) }],
    };
    assert_eq!(s.run(two).unwrap().label, "Delete 2 props");
    assert_eq!(s.map.props.len(), 2);
    assert_eq!(s.undo().as_deref(), Some("Delete 2 props"));
    assert!(s.map.props.is_empty());

    // No difference: no step.
    let c = s.run(Op::Details { name: String::new(), author: String::new(), description: String::new() }).unwrap();
    assert!(!c.changed);
    assert!(s.undo.undo_label().is_none());
}

#[test]
fn race_ops_carry_the_editors_names() {
    let mut s = session();
    let c = s.run(Op::NewRace { track: 4001, kind: RaceKind::Sprint, id: None }).unwrap();
    assert_eq!((c.label.as_str(), c.race.as_deref()), ("New race on track 4001", Some("race-1")));
    assert_eq!(s.map.races[0].name, "Race 1");
    let r = || "race-1".to_string();
    let pt = |part, index| RacePoint { part, index };
    let at = |x: f32| Vec3::new(x, 0.0, 0.0);
    let steps: Vec<(Op, &str)> = vec![
        (
            Op::AddPoint { race: r(), part: RacePart::Start, index: None, position: at(0.0), heading: 0.0 },
            "Place start line",
        ),
        (
            Op::AddPoint { race: r(), part: RacePart::Checkpoint, index: None, position: at(100.0), heading: 0.0 },
            "Add checkpoint 1",
        ),
        (
            Op::AddPoint { race: r(), part: RacePart::Checkpoint, index: None, position: at(300.0), heading: 0.0 },
            "Add checkpoint 2",
        ),
        (
            Op::AddPoint { race: r(), part: RacePart::Checkpoint, index: Some(1), position: at(200.0), heading: 0.0 },
            "Insert checkpoint 2",
        ),
        (
            Op::AddPoint { race: r(), part: RacePart::Finish, index: None, position: at(400.0), heading: 0.0 },
            "Place finish line",
        ),
        (Op::DeletePoint { race: r(), point: pt(RacePart::Grid, 7) }, "Delete grid slot 8"),
        (
            Op::AddPoint { race: r(), part: RacePart::Grid, index: None, position: at(-20.0), heading: 0.0 },
            "Add grid slot 8",
        ),
        (
            Op::MovePoint { race: r(), point: pt(RacePart::Checkpoint, 2), position: at(310.0), heading: None },
            "Move checkpoint 3",
        ),
        (
            Op::MovePoint { race: r(), point: pt(RacePart::Grid, 0), position: at(-5.0), heading: None },
            "Move grid slot 1",
        ),
        (
            Op::MovePoint { race: r(), point: pt(RacePart::Start, 0), position: at(1.0), heading: Some(10.0) },
            "Move start line",
        ),
        (
            Op::MovePoint { race: r(), point: pt(RacePart::Finish, 0), position: at(401.0), heading: None },
            "Move finish line",
        ),
        (Op::MoveCheckpoint { race: r(), index: 0, later: true }, "Move checkpoint 1 down"),
        (Op::MoveCheckpoint { race: r(), index: 1, later: false }, "Move checkpoint 2 up"),
        (Op::DeletePoint { race: r(), point: pt(RacePart::Grid, 7) }, "Delete grid slot 8"),
        (Op::DeletePoint { race: r(), point: pt(RacePart::Checkpoint, 1) }, "Delete checkpoint 2"),
        (Op::DeletePoint { race: r(), point: pt(RacePart::Finish, 0) }, "Delete finish line"),
        (Op::SetKind { race: r(), kind: RaceKind::Circuit }, "Mode Circuit"),
        (Op::SetLaps { race: r(), laps: 4 }, "Laps 4"),
        (Op::SetDirection { race: r(), backward: true }, "Direction Backward"),
        (Op::SetTrack { race: r(), track: 4002 }, "Base track 4002"),
        (Op::RenameRace { race: r(), name: "Docks".into() }, "Rename race"),
        (Op::Details { name: "M".into(), author: "a".into(), description: String::new() }, "Mod details"),
    ];
    for (op, label) in steps {
        let json = op.to_json();
        let c = s.run(op).unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(c.label, label, "{json}");
        assert!(c.changed, "{json}");
    }
    let race = &s.map.races[0];
    assert_eq!(race.checkpoints, vec![Vec2::new(100.0, 0.0), Vec2::new(310.0, 0.0)]);
    assert_eq!(race.grid.len(), 7, "the default grid taken over, one deleted, one added, one deleted");
    assert_eq!(race.start, Some(Spot { position: at(1.0), heading: 10.0 }));
    assert_eq!((race.kind, race.laps, race.backward, race.track), (RaceKind::Circuit, 4, true, 4002));
    assert_eq!(s.run(Op::SetRaceId { race: r(), id: "docks".into() }).unwrap().label, "Change race id");
    let mut fixed = s.map.races[0].clone();
    fixed.laps = 2;
    let c = s
        .run(Op::Group { label: "Set laps to 2".into(), ops: vec![Op::PutRace { race: "docks".into(), to: fixed }] })
        .unwrap();
    assert_eq!((c.label.as_str(), c.race.as_deref()), ("Set laps to 2", Some("docks")));
    assert_eq!(s.run(Op::SetKind { race: "docks".into(), kind: RaceKind::Sprint }).unwrap().label, "Mode Sprint");
    assert_eq!(s.map.races[0].laps, 1, "a sprint runs once");
    assert_eq!(s.run(Op::NewRace { track: 1, kind: RaceKind::Drift, id: Some("docks".into()) }), Err(Refused::IdTaken));
    assert_eq!(s.run(Op::SetLaps { race: "docks".into(), laps: 0 }), Err(Refused::BadValue));
    let full =
        Op::AddPoint { race: "docks".into(), part: RacePart::Grid, index: None, position: at(0.0), heading: 0.0 };
    s.run(full.clone()).unwrap();
    assert_eq!(s.run(full), Err(Refused::Full));
    assert_eq!(s.run(Op::DeleteRace { race: "docks".into() }).unwrap().label, "Delete race Docks");
    assert!(s.map.races.is_empty());
}

#[test]
fn ops_replayed_from_json_give_the_same_mod_and_world() {
    let mut s = session();
    let mut log = Vec::new();
    let mut run = |s: &mut MapSession, op: Op| {
        log.push(op.to_json());
        s.run(op).unwrap()
    };
    let (k0, k1, k2) = (cone(&s, 0), cone(&s, 1), cone(&s, 2));
    run(&mut s, Op::Place { key: k0, position: Vec3::new(11.0, 1.0, 0.0), rotation: odd_rotation() });
    let c = run(&mut s, Op::Duplicate { key: k1, position: Vec3::new(20.0, 9.0, 0.0) });
    run(&mut s, Op::Place { key: c.added[0].clone(), position: Vec3::new(21.0, 9.0, 0.5), rotation: Mat3::IDENTITY });
    run(&mut s, Op::Delete { key: k2 });
    run(&mut s, Op::NewRace { track: 4001, kind: RaceKind::Circuit, id: None });
    run(
        &mut s,
        Op::AddPoint {
            race: "race-1".into(),
            part: RacePart::Checkpoint,
            index: None,
            position: Vec3::X,
            heading: 0.0,
        },
    );

    let mut again = session();
    for line in Op::read_lines(&log.join("\n")).unwrap() {
        again.run(line).unwrap();
    }
    assert_eq!(again.map, s.map);
    let places = |s: &MapSession| s.world.placements.iter().map(|p| (p.transform, p.flags)).collect::<Vec<_>>();
    assert_eq!(places(&again), places(&s));
    // And the mod as a file gives that world too.
    let mut fresh = MapSession::new(REGION, "my-map", MapMod::parse(&s.map.write()).unwrap(), world());
    assert_eq!(places(&fresh), places(&s));
    // A whole mod put in place is one step.
    assert_eq!(fresh.run(Op::PutMod { map: MapMod::default() }).unwrap().label, "Replace mod");
    assert_eq!(fresh.world.placements.len(), world().placements.len());
    assert_eq!(fresh.undo().as_deref(), Some("Replace mod"));
    assert_eq!(places(&fresh), places(&s));
}

/// A start, finish or grid slot is written at the road's height (one under the road comes up onto it); a point
/// with no floor under it keeps its height (the checks flag it).
#[test]
fn race_points_get_their_height_from_the_road() {
    let mut s = session();
    s.run(Op::NewRace { track: 4001, kind: RaceKind::Sprint, id: Some("r".into()) }).unwrap();
    let r = || "r".to_string();
    let add = |part, position| Op::AddPoint { race: r(), part, index: None, position, heading: 0.0 };
    s.run(add(RacePart::Start, Vec3::new(5.0, 6.0, -30.0))).unwrap();
    assert_eq!(s.map.races[0].start.unwrap().position, Vec3::new(5.0, 6.0, 0.0));
    let to = Vec3::new(7.0, 6.0, 0.6);
    s.run(Op::MovePoint {
        race: r(),
        point: RacePoint { part: RacePart::Start, index: 0 },
        position: to,
        heading: None,
    })
    .unwrap();
    assert_eq!(s.map.races[0].start.unwrap().position, Vec3::new(7.0, 6.0, 0.0));
    s.run(add(RacePart::Finish, Vec3::new(1000.0, 0.0, -3.0))).unwrap();
    assert_eq!(s.map.races[0].finish.unwrap().position.z, -3.0, "off the map: as given");
    let mut to = s.map.races[0].clone();
    to.grid = vec![Spot { position: Vec3::new(1.0, 1.0, -9.0), heading: 0.0 }];
    s.run(Op::PutRace { race: r(), to }).unwrap();
    assert_eq!(s.map.races[0].grid[0].position.z, 0.0);
}
