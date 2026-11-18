//! The race tools on the straight test road (validate_tests.rs).

use glam::{Vec2, Vec3};

use super::*;
use crate::editor::validate::tests::{sprint, straight};

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
    let (p, label) = place(&mut r, Tool::Checkpoint, Vec3::new(300.0, 0.0, 0.0), None, cp(0), k, Vec2::X).unwrap();
    assert_eq!(p, cp(2).unwrap());
    assert_eq!(label, "Insert checkpoint 3");
    assert_eq!(
        r.checkpoints,
        vec![Vec2::new(20.0, 0.0), Vec2::new(180.0, 0.0), Vec2::new(300.0, 0.0), Vec2::new(380.0, 0.0)]
    );
}

#[test]
fn a_race_holds_at_most_64_checkpoints() {
    let mut r = sprint(&[]);
    for i in 0..MAX_CHECKPOINTS {
        let n = r.checkpoints.len();
        place(&mut r, Tool::Checkpoint, Vec3::new(i as f32, 0.0, 0.0), None, None, n, Vec2::X).unwrap();
    }
    assert!(place(&mut r, Tool::Checkpoint, Vec3::ZERO, None, None, 0, Vec2::X).is_err());
}

#[test]
fn start_and_finish_face_along_the_road_the_race_way() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    let road = race::snap(&net, Vec2::new(300.0, 3.0));
    // A start at 300 m faces back toward checkpoint 1 at 20 m: -x.
    place(&mut r, Tool::Start, Vec3::new(300.0, 0.0, 0.0), road, None, 0, Vec2::X).unwrap();
    assert!((r.start.unwrap().heading.abs() - 180.0).abs() < 1e-3);
    // A finish faces away from the last checkpoint: +x.
    let road = race::snap(&net, Vec2::new(390.0, 0.0));
    place(&mut r, Tool::Finish, Vec3::new(390.0, 0.0, 0.0), road, None, 0, -Vec2::X).unwrap();
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
    assert!(place(&mut r, Tool::Grid, Vec3::new(1.0, 2.0, 0.0), None, None, 0, Vec2::X).is_err());
    let slot = Some(RacePoint { part: RacePart::Grid, index: 3 });
    let (p, _) = place(&mut r, Tool::Grid, Vec3::new(1.0, 2.0, 0.0), None, slot, 0, Vec2::X).unwrap();
    assert_eq!(p, slot.unwrap());
    assert_eq!(r.grid.len(), 8);
    assert_eq!(r.grid[3].position, Vec3::new(1.0, 2.0, 0.0));
    assert_eq!(delete_point(&mut r, slot.unwrap()).as_deref(), Some("Delete grid slot 4"));
    assert_eq!(r.grid.len(), 7);
    let (p, label) = place(&mut r, Tool::Grid, Vec3::new(5.0, 5.0, 0.0), None, None, 0, Vec2::X).unwrap();
    assert_eq!((p.index, label.as_str()), (7, "Add grid slot 8"));
    assert_eq!(r.grid[7].heading, 0.0);
}

#[test]
fn points_are_picked_and_deleted() {
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    assert_eq!(pick(&r, Vec2::new(182.0, 1.0), PICK_M), cp(1));
    assert_eq!(pick(&r, Vec2::new(389.0, 0.0), PICK_M), Some(RacePoint { part: RacePart::Finish, index: 0 }));
    assert_eq!(pick(&r, Vec2::new(100.0, 0.0), PICK_M), None);
    assert_eq!(delete_point(&mut r, cp(1).unwrap()).as_deref(), Some("Delete checkpoint 2"));
    assert_eq!(after_delete(&r, cp(1).unwrap()), cp(1));
    assert_eq!(
        delete_point(&mut r, RacePoint { part: RacePart::Finish, index: 0 }).as_deref(),
        Some("Delete finish line")
    );
    assert_eq!(delete_point(&mut r, RacePoint { part: RacePart::Finish, index: 0 }), None);
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

#[test]
fn new_races_get_free_ids() {
    let mut m = MapMod::default();
    assert_eq!(new_race(&mut m, 4001, RaceKind::Sprint), 0);
    m.races[0].id = "race-2".into();
    new_race(&mut m, 4001, RaceKind::Sprint);
    assert_eq!(m.races[1].id, "race-3");
    assert_eq!(m.races[1].laps, 1);
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
    st.nets.insert(4001, Some(straight(false)));
    st
}

#[test]
fn clicks_place_snapped_points_as_undo_steps() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    let mut races = Races::new();
    s.mode = Mode::Races;
    // A click beside the road puts checkpoint 3 on it.
    races.click(&mut s, Vec3::new(390.0, 6.0, 0.0), false, false);
    assert_eq!(s.map.races[0].checkpoints[2], Vec2::new(390.0, 0.0));
    assert_eq!(s.point, cp(2));
    assert_eq!(s.undo.undo_label(), Some("Add checkpoint 3"));
    // Alt places it where clicked.
    races.click(&mut s, Vec3::new(395.0, 6.0, 0.0), false, true);
    assert_eq!(s.map.races[0].checkpoints[3], Vec2::new(395.0, 6.0));
    // A click on a point picks it; dragging moves it, one step on release.
    races.click(&mut s, Vec3::new(21.0, 1.0, 0.0), false, false);
    assert_eq!(s.point, cp(0));
    races.drag(&mut s, Vec3::new(50.0, 3.0, 0.0), false);
    races.release(&mut s);
    assert_eq!(s.map.races[0].checkpoints[0], Vec2::new(50.0, 0.0));
    assert_eq!(s.undo.undo_label(), Some("Move checkpoint 1"));
    races.delete_selected(&mut s);
    assert_eq!(s.map.races[0].checkpoints.len(), 3);
    assert!(crate::editor::undo::undo(&mut s));
    assert_eq!(s.map.races[0].checkpoints.len(), 4);
    // The validation reaches the shared state.
    races.update(&mut s);
    assert!(!s.issues.is_empty());
    assert!(races.test_blocked(&mut s).is_none(), "{:?}", races.checks);
}

#[test]
fn the_panels_draw_headless() {
    let mut s = state(sprint(&[(20.0, 0.0), (380.0, 0.0)]));
    s.map.races[0].start = Some(Spot { position: Vec3::new(10.0, 40.0, 0.0), heading: 0.0 });
    s.point = cp(1);
    let mut races = Races::new();
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.horizontal(|ui| races.toolbar(ui));
            crate::editor::tree_races::tree(&mut races, &mut s, ui);
            races.ui(&mut s, ui);
            races.validation(&mut s, ui);
            let rect = ui.max_rect();
            races.paint_overlay(&mut s, ui.painter(), rect);
            races.paint_map(&mut s, ui.painter(), &|p| egui::pos2(p.x, p.y));
        });
    }
    assert!(races.checks.iter().any(|c| c.fix == Some(validate::Fix::SnapStart)), "{:?}", races.checks);
    assert_eq!(races.counts().0, 0);
}
