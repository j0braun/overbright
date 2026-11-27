//! The race tools on the straight test road (validate_tests.rs).

use glam::{Vec2, Vec3};

use super::*;
use crate::editor::state::SnapMode;
use crate::editor::validate::tests::{sprint, straight};
use crate::tools::map::assets::GivenAssets;
use crate::tools::map::ops::RacePoint as Point;
use crate::tools::map::session::MapSession;

/// Run `op` on a session of `r` alone; the race after it.
fn ran(r: &CustomRace, op: Op) -> Result<CustomRace, crate::tools::map::edit::Refused> {
    let map = MapMod { races: vec![r.clone()], ..MapMod::default() };
    let mut s = MapSession::new("L4RA", "t", map, crate::world::World::default());
    s.run(op)?;
    Ok(s.map.races[0].clone())
}

fn cp(i: usize) -> Option<RacePoint> {
    Some(RacePoint { part: RacePart::Checkpoint, index: i })
}

#[test]
fn checkpoints_go_after_the_selected_one_or_into_the_nearest_leg() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    let pv = preview(Some(&net), &r, &|_| 0.0);
    assert!(pv.road.is_some() && pv.length > 300.0, "{pv:?}");
    assert_eq!(insert_index(&r, Some(&pv), None, Vec2::new(300.0, 0.0), false), 3);
    assert_eq!(insert_index(&r, Some(&pv), cp(0), Vec2::new(300.0, 0.0), false), 1);
    // Shift: into the leg whose road passes nearest (180 → 380).
    let k = insert_index(&r, Some(&pv), cp(0), Vec2::new(300.0, 2.0), true);
    let op = place_op(&r, Tool::Checkpoint, Vec3::new(300.0, 0.0, 0.0), None, cp(0), k, Vec2::X).unwrap();
    assert_eq!(op.label(&MapMod { races: vec![r.clone()], ..MapMod::default() }), "Insert checkpoint 3");
    r = ran(&r, op).unwrap();
    assert_eq!(
        r.checkpoints,
        vec![Vec2::new(20.0, 0.0), Vec2::new(180.0, 0.0), Vec2::new(300.0, 0.0), Vec2::new(380.0, 0.0)]
    );
}

#[test]
fn a_race_holds_at_most_64_checkpoints() {
    let mut r = sprint(&[]);
    r.checkpoints = (0..MAX_CHECKPOINTS).map(|i| Vec2::new(i as f32, 0.0)).collect();
    assert!(place_op(&r, Tool::Checkpoint, Vec3::ZERO, None, None, 0, Vec2::X).is_err());
    r.checkpoints.pop();
    let op = place_op(&r, Tool::Checkpoint, Vec3::ZERO, None, None, 0, Vec2::X).unwrap();
    assert_eq!(ran(&r, op).unwrap().checkpoints.len(), MAX_CHECKPOINTS);
}

#[test]
fn start_and_finish_face_along_the_road_the_race_way() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    let road = race::snap(&net, Vec2::new(300.0, 3.0));
    // A start at 300 m faces back toward checkpoint 1 at 20 m: -x.
    r = ran(&r, place_op(&r, Tool::Start, Vec3::new(300.0, 0.0, 0.0), road, None, 0, Vec2::X).unwrap()).unwrap();
    assert!((r.start.unwrap().heading.abs() - 180.0).abs() < 1e-3);
    // A finish faces away from the last checkpoint: +x.
    let road = race::snap(&net, Vec2::new(390.0, 0.0));
    r = ran(&r, place_op(&r, Tool::Finish, Vec3::new(390.0, 0.0, 0.0), road, None, 0, -Vec2::X).unwrap()).unwrap();
    assert_eq!(r.finish.unwrap().heading, 0.0);
    // Dragging the start keeps it along the road, nearest its heading.
    assert!(move_point(&mut r, RacePoint { part: RacePart::Start, index: 0 }, Vec3::new(50.0, 0.0, 1.0), road));
    let s = r.start.unwrap();
    assert_eq!(s.position, Vec3::new(50.0, 0.0, 1.0));
    assert!((s.heading.abs() - 180.0).abs() < 1e-3);
}

#[test]
fn the_grid_tool_takes_over_the_default_grid_and_fills_to_eight() {
    let mut r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    r.start = Some(Spot { position: Vec3::new(60.0, 0.0, 0.0), heading: 0.0 });
    // The default grid is full: a click moves the selected slot.
    assert!(place_op(&r, Tool::Grid, Vec3::new(1.0, 2.0, 0.0), None, None, 0, Vec2::X).is_err());
    let slot = Some(RacePoint { part: RacePart::Grid, index: 3 });
    let op = place_op(&r, Tool::Grid, Vec3::new(1.0, 2.0, 0.0), None, slot, 0, Vec2::X).unwrap();
    r = ran(&r, op).unwrap();
    assert_eq!(r.grid.len(), 8);
    assert_eq!(r.grid[3].position, Vec3::new(1.0, 2.0, 0.0));
    r = ran(&r, Op::DeletePoint { race: r.id.clone(), point: slot.unwrap() }).unwrap();
    assert_eq!(r.grid.len(), 7);
    let op = place_op(&r, Tool::Grid, Vec3::new(5.0, 5.0, 0.0), None, None, 0, Vec2::X).unwrap();
    assert_eq!(op.label(&MapMod { races: vec![r.clone()], ..MapMod::default() }), "Add grid slot 8");
    r = ran(&r, op).unwrap();
    assert_eq!(r.grid[7].heading, 0.0);
}

#[test]
fn points_are_picked_and_deleted() {
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    assert_eq!(pick(&r, Vec2::new(182.0, 1.0), PICK_M), cp(1));
    assert_eq!(pick(&r, Vec2::new(389.0, 0.0), PICK_M), Some(RacePoint { part: RacePart::Finish, index: 0 }));
    assert_eq!(pick(&r, Vec2::new(100.0, 0.0), PICK_M), None);
    let r = ran(&r, Op::DeletePoint { race: r.id.clone(), point: cp(1).unwrap() }).unwrap();
    assert_eq!(after_delete(&r, cp(1).unwrap()), cp(1));
    let fin = RacePoint { part: RacePart::Finish, index: 0 };
    let r = ran(&r, Op::DeletePoint { race: r.id.clone(), point: fin }).unwrap();
    assert_eq!(
        ran(&r, Op::DeletePoint { race: r.id.clone(), point: fin }),
        Err(crate::tools::map::edit::Refused::NoPoint)
    );
}

#[test]
fn a_broken_route_previews_its_legs() {
    let net = straight(true);
    let r = sprint(&[(20.0, 0.0), (150.0, 0.0), (380.0, 0.0)]);
    let pv = preview(Some(&net), &r, &|_| 0.0);
    assert!(pv.road.is_none());
    assert_eq!(pv.legs.len(), 2);
    assert!(pv.legs[0].is_some() && pv.legs[1].is_none(), "{pv:?}");
}

/// An editor state on an empty world with one race on the test road.
fn state(race: CustomRace) -> EditorState {
    let s = crate::testkit::Scratch::new("editor-races");
    let map = MapMod { races: vec![race], ..MapMod::default() };
    let cam = crate::draw::camera::Camera::looking_at(Vec3::new(100.0, -80.0, 60.0), Vec3::new(100.0, 0.0, 0.0));
    let mut st = EditorState::new(
        s.path().join("game"),
        "L4RA",
        s.path().join("maps"),
        "test",
        map,
        crate::world::World::default(),
        cam,
    );
    st.assets = Box::new(GivenAssets {
        tracks: vec![TrackInfo { number: 4001, name: "Test road".into(), modes: vec![], area: String::new() }],
        nets: [(4001, straight(false))].into(),
        ..Default::default()
    });
    st
}

#[test]
fn clicks_place_snapped_points_as_undo_steps() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    let mut races = Races::new();
    s.mode = Mode::Races;
    races.tool = Tool::Checkpoint;
    // A click beside the road puts checkpoint 3 on it.
    races.click(&mut s, Vec3::new(390.0, 6.0, 0.0), false);
    assert_eq!(s.session.map.races[0].checkpoints[2], Vec2::new(390.0, 0.0));
    assert_eq!(s.point, cp(2));
    assert_eq!(s.session.undo.undo_label(), Some("Add checkpoint 3"));
    // Alt places it where clicked.
    s.snap_mode = SnapMode::Free;
    races.click(&mut s, Vec3::new(395.0, 6.0, 0.0), false);
    s.snap_mode = SnapMode::default();
    assert_eq!(s.session.map.races[0].checkpoints[3], Vec2::new(395.0, 6.0));
    // A click on a point picks it; dragging moves it, one step on release.
    races.click(&mut s, Vec3::new(21.0, 1.0, 0.0), false);
    assert_eq!(s.point, cp(0));
    // Esc during a drag puts the point back and leaves no step.
    let (before, label) = (s.session.map.races[0].checkpoints[0], s.session.undo.undo_label().map(String::from));
    races.drag(&mut s, Vec3::new(50.0, 3.0, 0.0));
    assert_ne!(s.session.map.races[0].checkpoints[0], before);
    races.cancel_drag(&mut s);
    races.release(&mut s);
    assert_eq!(s.session.map.races[0].checkpoints[0], before);
    assert_eq!(s.session.undo.undo_label().map(String::from), label);
    races.click(&mut s, Vec3::new(21.0, 1.0, 0.0), false);
    races.drag(&mut s, Vec3::new(50.0, 3.0, 0.0));
    races.release(&mut s);
    assert_eq!(s.session.map.races[0].checkpoints[0], Vec2::new(50.0, 0.0));
    assert_eq!(s.session.undo.undo_label(), Some("Move checkpoint 1"));
    races.delete_selected(&mut s);
    assert_eq!(s.session.map.races[0].checkpoints.len(), 3);
    assert!(crate::editor::undo::undo(&mut s));
    assert_eq!(s.session.map.races[0].checkpoints.len(), 4);
    // The validation reaches the shared state.
    races.update(&mut s);
    assert!(!s.session.issues.is_empty());
    assert!(races.test_blocked(&mut s).is_none(), "{:?}", races.checks);
}

#[test]
fn the_panels_draw_headless() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    s.session.map.races[0].start = Some(Spot { position: Vec3::new(10.0, 40.0, 0.0), heading: 0.0 });
    s.point = cp(1);
    let mut races = Races::new();
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let _ = crate::editor::run_headless(&ctx, egui::RawInput::default(), |ui| {
            crate::editor::tree_races::tree(&mut races, &mut s, ui);
            races.ui(&mut s, ui);
            races.validation(&mut s, ui);
            let rect = ui.max_rect();
            races.paint_overlay(&mut s, ui.painter(), rect, false);
            races.paint_map(&mut s, ui.painter(), &|p| egui::pos2(p.x, p.y));
        });
    }
    assert!(races.checks.iter().any(|c| c.fix == Some(validate::Fix::SnapStart)), "{:?}", races.checks);
    assert_eq!(races.counts().0, 0);
}

/// A race made with the tools, validated clean, then picked from its
/// validation entry; every race is drawn in Props mode too.
#[test]
fn a_race_is_made_validated_and_selected() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    let mut races = Races::new();
    races.new_race(&mut s);
    assert_eq!((s.session.map.races.len(), s.race, s.mode, races.tool), (2, Some(1), Mode::Races, Tool::Start));
    races.click(&mut s, Vec3::new(10.0, 3.0, 0.0), false);
    assert_eq!(races.tool, Tool::Checkpoint, "the start placed arms Checkpoint");
    for x in [120.0, 220.0, 360.0] {
        races.click(&mut s, Vec3::new(x, -4.0, 0.0), false);
    }
    races.tool = Tool::Finish;
    races.click(&mut s, Vec3::new(395.0, 2.0, 0.0), false);
    let r = &s.session.map.races[1];
    assert_eq!(r.checkpoints, vec![Vec2::new(120.0, 0.0), Vec2::new(220.0, 0.0), Vec2::new(360.0, 0.0)]);
    assert!(r.start.is_some() && r.finish.is_some());
    races.update(&mut s);
    let mine: Vec<_> = races.checks.iter().filter(|c| c.race == Some(1)).collect();
    assert!(mine.iter().all(|c| c.level != crate::editor::Level::Error), "{mine:#?}");
    assert!(mine.iter().any(|c| c.ok && c.text.contains("3 checkpoints")), "{mine:#?}");
    assert!(races.test_blocked(&mut s).is_none());

    // Picked again from the validation list after selecting the other one.
    s.race = Some(0);
    let entry = races.checks.iter().find(|c| c.race == Some(1)).unwrap().clone();
    validate::show(&mut s, &entry);
    assert_eq!(s.race, Some(1));

    // Props mode draws every race: the other race's route adds shapes.
    s.mode = Mode::Props;
    let shapes = |s: &mut EditorState, races: &mut Races| {
        let ctx = egui::Context::default();
        let out = crate::editor::run_headless(&ctx, egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 960.0));
            races.paint_overlay(s, ui.painter(), rect, false);
        });
        out.shapes.len()
    };
    let both = shapes(&mut s, &mut races);
    let other = s.session.map.races.remove(0);
    s.race = Some(0);
    let one = shapes(&mut s, &mut races);
    assert!(one > 0 && both > one, "{one} / {both}");
    drop(other);
}

#[test]
fn new_races_get_free_ids() {
    let mut s = state(sprint(&[]));
    s.session.map.races[0].id = "race-2".into();
    s.race = None;
    let mut races = Races::new();
    races.new_race(&mut s);
    assert_eq!(s.session.map.races[1].id, "race-3");
    assert_eq!(s.session.map.races[1].name, "Race 3");
    assert_eq!(s.session.map.races[1].laps, 1);
    assert_eq!(s.session.undo.undo_label(), Some("New race on track 4001"));
}

/// The race tools start at Select, which never adds: a click on the road
/// changes nothing and records no step; on a point it only picks.
#[test]
fn a_select_click_on_the_road_adds_nothing() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    let mut races = Races::new();
    assert_eq!(races.tool, Tool::Select);
    s.mode = Mode::Races;
    let before = s.session.map.clone();
    for at in [Vec3::new(200.0, 0.0, 0.0), Vec3::new(300.0, 2.0, 0.0)] {
        races.click(&mut s, at, false);
        races.click(&mut s, at, true);
        s.snap_mode = SnapMode::Free;
        races.click(&mut s, at, false);
        s.snap_mode = SnapMode::default();
    }
    assert_eq!(s.session.map, before);
    assert_eq!(s.session.undo.undo_label(), None);
    assert!(!s.session.dirty);
    races.click(&mut s, Vec3::new(21.0, 0.0, 0.0), false);
    races.release(&mut s);
    assert_eq!(s.point, cp(0));
    assert_eq!(s.session.map, before, "a pick without a move is no edit");
}

/// A race made with the tools from New race to the finish is what its
/// ops make in a fresh session: the tools edit only through ops, one
/// undo step each (the drag one step on release).
#[test]
fn a_race_is_built_only_through_ops() {
    let mut s = state(sprint(&[]));
    s.session.map.races.clear();
    s.session.undo = crate::editor::undo::Undo::new(&s.session.map);
    s.race = None;
    let mut races = Races::new();
    races.new_race(&mut s);
    races.click(&mut s, Vec3::new(10.0, 3.0, 0.0), false);
    for x in [120.0, 220.0, 360.0] {
        races.click(&mut s, Vec3::new(x, -4.0, 0.0), false);
    }
    // Drag checkpoint 2 along the road; the live change is one step.
    races.click(&mut s, Vec3::new(220.0, 0.0, 0.0), false);
    races.drag(&mut s, Vec3::new(230.0, 1.0, 0.0));
    races.drag(&mut s, Vec3::new(240.0, 2.0, 0.0));
    races.release(&mut s);
    crate::editor::menu::set_tool(&mut s, &mut Default::default(), &mut races, crate::editor::menu::Tool::Finish);
    races.click(&mut s, Vec3::new(395.0, 2.0, 0.0), false);
    races.delete_selected(&mut s);
    let mut labels = Vec::new();
    let mut u = s.session.undo.clone();
    while let Some(l) = u.undo_label().map(str::to_string) {
        labels.push(l);
        u.undo();
    }
    labels.reverse();
    assert_eq!(
        labels,
        [
            "New race on track 4001",
            "Place start line",
            "Add checkpoint 1",
            "Add checkpoint 2",
            "Add checkpoint 3",
            "Move checkpoint 2",
            "Place finish line",
            "Delete finish line",
        ]
    );
    // The same ops in a fresh session make the same mod.
    let built = s.session.map.clone();
    let r = &built.races[0];
    let mut fresh = MapSession::new("L4RA", "t", MapMod::default(), crate::world::World::default());
    let id = r.id.clone();
    let start = r.start.unwrap();
    let mut ops = vec![
        Op::NewRace { track: 4001, kind: RaceKind::Sprint, id: None },
        Op::AddPoint {
            race: id.clone(),
            part: RacePart::Start,
            index: None,
            position: start.position,
            heading: start.heading,
        },
    ];
    for (i, x) in [120.0, 220.0, 360.0].into_iter().enumerate() {
        ops.push(Op::AddPoint {
            race: id.clone(),
            part: RacePart::Checkpoint,
            index: Some(i),
            position: Vec3::new(x, 0.0, 0.0),
            heading: 0.0,
        });
    }
    ops.push(Op::MovePoint {
        race: id.clone(),
        point: Point { part: RacePart::Checkpoint, index: 1 },
        position: Vec3::new(240.0, 0.0, 0.0),
        heading: None,
    });
    for op in ops {
        assert!(fresh.run(op).unwrap().changed);
    }
    assert_eq!(fresh.map, built);
}

/// The race tools and the races' tree make no in-place edits: no
/// `commit` (every change is an op run through `undo::run`).
#[test]
fn races_edit_only_through_ops() {
    for file in ["races.rs", "tree_races.rs"] {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/editor/").to_string() + file;
        let src = std::fs::read_to_string(&path).unwrap();
        let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
        assert!(!code.contains("commit("), "{file} edits in place");
        assert!(!code.contains("CareerData") && !code.contains("race::road_net"), "{file} reads the game's files");
    }
}

/// In the top view a race point is grabbed within the same number of screen points at any zoom; the 3D view keeps
/// its metres.
#[test]
fn the_top_views_grab_keeps_its_screen_size_at_any_zoom() {
    use crate::editor::camera::TopView;
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 450.0));
    assert_eq!(pick_radius(None), PICK_M);
    for half_height in [20.0_f32, 150.0, 2000.0] {
        let top = TopView { centre: Vec2::ZERO, half_height };
        let m = pick_radius(Some((top, rect)));
        assert!((m / top.metres_per_point(rect) - PICK_POINTS).abs() < 1e-3, "{half_height}: {m} m");
    }
    // Zoomed out a checkpoint 30 m off is grabbed (it is a few points on screen); zoomed in it is not.
    let r = sprint(&[(100.0, 0.0), (300.0, 0.0)]);
    let near = Vec2::new(100.0, 30.0);
    let out = pick_radius(Some((TopView { centre: Vec2::ZERO, half_height: 2000.0 }, rect)));
    let inn = pick_radius(Some((TopView { centre: Vec2::ZERO, half_height: 20.0 }, rect)));
    assert!(pick(&r, near, out).is_some() && pick(&r, near, inn).is_none());
}
