//! The race tools in Top and on the Map, through the whole editor: a click on
//! the road places the point snapped to the base track's road at the road's
//! height, a drag moves it, each one undo step; the keys work as in 3D.

use egui::{Event, Pos2, Rect};
use glam::{Mat4, Vec2, Vec3};

use super::*;
use crate::editor::races::Tool as RaceTool;
use crate::editor::validate::tests::{sprint, straight};
use crate::testkit::Scratch;
use crate::tools::map::assets::{GivenAssets, TrackInfo};

/// The road's height in the test world: a floor well above z = 0, so a
/// height from the camera's ray or from z = 0 shows.
const FLOOR_Z: f32 = 12.5;
const SIZE: (f32, f32) = (1600.0, 960.0);

/// One floor quad 2 km across at [`FLOOR_Z`] (the render triangles stand in
/// for the collision).
fn raised_world() -> World {
    let h = 1000.0;
    let corners = [[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]];
    let solid = crate::formats::solid::Solid {
        name: "FLOOR".into(),
        hash: crate::formats::hash::bin_hash("FLOOR"),
        bbox_min: [-h, -h, 0.0],
        bbox_max: [h, h, 0.0],
        textures: Vec::new(),
        vertices: corners
            .into_iter()
            .map(|position| crate::formats::solid::Vertex { position, color: [255; 4], uv: [0.0; 2] })
            .collect(),
        normals: Vec::new(),
        light_materials: Vec::new(),
        indices: vec![0, 1, 2, 0, 2, 3],
        materials: vec![crate::formats::solid::Material {
            texture_slot: 0,
            light_material: 255,
            first_index: 0,
            index_count: 6,
            flags: 0,
        }],
        mesh_flags: 0,
        markers: Vec::new(),
        light_markers: Vec::new(),
    };
    let mut w = World::default();
    w.solids.push(solid);
    w.placements.push(crate::world::Placement {
        solid: 0,
        low_solid: None,
        mid_solid: None,
        transform: Mat4::from_translation(Vec3::new(0.0, 0.0, FLOOR_Z)),
        section: 1,
        flags: 0,
        cull_radius: 1500.0,
    });
    w
}

/// An editor with a mod holding one sprint (checkpoints at 20 and 380 m) on
/// the straight test road along +x, the race selected, in `view`.
fn editor(s: &Scratch, view: &str) -> App {
    let mut a = App::new(s.path().join("game"), s.path().join("maps"), Box::new(|_, _| Ok(raised_world())));
    a.new_mod("r", "Races", "L4RA").unwrap();
    let st = a.state.as_mut().unwrap();
    st.assets = Box::new(GivenAssets {
        tracks: vec![TrackInfo { number: 4001, name: "Test road".into() }],
        nets: [(4001, straight(false))].into(),
        ..Default::default()
    });
    st.session.map.races = vec![sprint(&[(20.0, 0.0), (380.0, 0.0)])];
    st.race = Some(0);
    st.mode = Mode::Races;
    st.camera.position = Vec3::new(200.0, 0.0, 300.0);
    crate::editor::commit(st, "New race");
    assert!(
        st.session.world.road_height_below(Vec3::new(100.0, 0.0, 100.0)).is_some_and(|z| (z - FLOOR_Z).abs() < 0.01)
    );
    match view {
        "map" => st.view = View::Map,
        "top" => {
            let st = a.state.as_mut().unwrap();
            a.panels.viewport.set_look(st, "top");
        }
        _ => unreachable!(),
    }
    for _ in 0..3 {
        frame(&mut a, vec![]);
    }
    a
}

fn frame(a: &mut App, events: Vec<Event>) {
    let _ = a.frame(egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(SIZE.0, SIZE.1))),
        events,
        ..Default::default()
    });
}

/// World x/y on screen as the view last drew it.
fn screen(a: &App, p: Vec2) -> Pos2 {
    let st = a.state.as_ref().unwrap();
    if st.view == View::Map {
        let (centre, scale, rect) = crate::editor::sim::panel::map_transform(&a.ctx).expect("the map drew");
        let c = rect.center();
        return Pos2::new(c.x + (p.x - centre.x) * scale, c.y - (p.y - centre.y) * scale);
    }
    let (top, rect) = crate::editor::viewport::top_view(&a.ctx).expect("the overview drew");
    let m = top.metres_per_point(rect);
    let c = rect.center();
    Pos2::new(c.x + (p.x - top.centre.x) / m, c.y - (p.y - top.centre.y) / m)
}

fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn key(k: egui::Key) -> [Event; 2] {
    let e = |pressed| Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    [e(true), e(false)]
}

fn click(a: &mut App, p: Vec2) {
    let at = screen(a, p);
    frame(a, vec![Event::PointerMoved(at)]);
    frame(a, vec![button(at, true)]);
    frame(a, vec![button(at, false)]);
    frame(a, vec![]);
}

fn drag(a: &mut App, from: Vec2, to: Vec2) {
    let (from, to) = (screen(a, from), screen(a, to));
    frame(a, vec![Event::PointerMoved(from)]);
    frame(a, vec![button(from, true)]);
    for k in 1..=6 {
        frame(a, vec![Event::PointerMoved(from + (to - from) * (k as f32 / 6.0))]);
    }
    frame(a, vec![button(to, false)]);
    frame(a, vec![]);
}

fn race(a: &App) -> &crate::tools::map::CustomRace {
    &a.state.as_ref().unwrap().session.map.races[0]
}

fn undo_label(a: &App) -> Option<&str> {
    a.state.as_ref().unwrap().session.undo_label()
}

/// The walkthrough in one view: keys arm the tools, clicks place the start,
/// a checkpoint and the finish on the road (snapped, at the road's height),
/// a drag moves the start, each one undo step that undo and redo take back
/// and put again exactly.
fn place_and_drag(view: &str) {
    let s = Scratch::new(&format!("editor-race-points-{view}"));
    let mut a = editor(&s, view);
    let snap_m = match view {
        // The overview opens on the region: zoom in on the road first.
        "top" => {
            let at = screen(&a, Vec2::new(200.0, 0.0));
            for _ in 0..40 {
                let (top, rect) = crate::editor::viewport::top_view(&a.ctx).unwrap();
                if top.metres_per_point(rect) < 1.2 {
                    break;
                }
                frame(
                    &mut a,
                    vec![
                        Event::PointerMoved(at),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, 120.0),
                            modifiers: Default::default(),
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Wheel,
                        },
                    ],
                );
            }
            let (top, rect) = crate::editor::viewport::top_view(&a.ctx).unwrap();
            top.metres_per_point(rect)
        }
        _ => 1.0 / crate::editor::sim::panel::map_transform(&a.ctx).unwrap().1,
    };
    assert!(snap_m < 2.0, "{view}: {snap_m} m per point");
    let before = race(&a).clone();
    let steps = |a: &App| a.state.as_ref().unwrap().session.undo_label().map(str::to_string);

    // S arms Start (the hover keeps the keys the view's); a click 4 m beside the road puts it on the road.
    let at = screen(&a, Vec2::new(60.0, 0.0));
    frame(&mut a, [vec![Event::PointerMoved(at)], key(egui::Key::S).to_vec()].concat());
    frame(&mut a, vec![]);
    assert_eq!(a.panels.races.borrow().tool, RaceTool::Start, "{view}: S arms Start");
    click(&mut a, Vec2::new(60.0, 4.0));
    let st = race(&a).start.unwrap_or_else(|| {
        panic!(
            "{view}: no start line; status {:?}, top {:?}, view {:?}",
            a.state.as_ref().unwrap().status,
            crate::editor::viewport::top_view(&a.ctx),
            a.view_rect
        )
    });
    assert!((st.position.y).abs() < 0.01 && (st.position.x - 60.0).abs() < 2.0 * snap_m, "{view}: {st:?}");
    assert!((st.position.z - FLOOR_Z).abs() < 0.05, "{view}: height {} not the road's", st.position.z);
    // Facing checkpoint 1 at 20 m: along -x.
    assert!((st.heading.abs() - 180.0).abs() < 1e-3, "{view}: {}", st.heading);
    assert_eq!(undo_label(&a), Some("Place start line"), "{view}");
    // New race's rule is not in play here; C arms Checkpoint; a click beside the road adds checkpoint 3.
    frame(&mut a, key(egui::Key::C).to_vec());
    assert_eq!(a.panels.races.borrow().tool, RaceTool::Checkpoint, "{view}: C arms Checkpoint");
    click(&mut a, Vec2::new(390.0, -3.0));
    let cps = race(&a).checkpoints.clone();
    assert_eq!(
        cps.len(),
        3,
        "{view}: {cps:?}; status {:?}, top {:?}",
        a.state.as_ref().unwrap().status,
        crate::editor::viewport::top_view(&a.ctx)
    );
    assert!(cps[2].y.abs() < 0.01 && (cps[2].x - 390.0).abs() < 2.0 * snap_m, "{view}: {cps:?}");
    assert_eq!(undo_label(&a), Some("Add checkpoint 3"), "{view}");
    // X arms Finish.
    frame(&mut a, key(egui::Key::X).to_vec());
    click(&mut a, Vec2::new(395.0, 2.0));
    let fi = race(&a).finish.expect("a finish line");
    assert!(fi.position.y.abs() < 0.01 && (fi.position.z - FLOOR_Z).abs() < 0.05, "{view}: {fi:?}");
    let placed = race(&a).clone();

    // A drag of the start moves it along the road (snapped, the road's height): one step.
    let shown = |a: &App| match view {
        "map" => crate::editor::sim::panel::map_transform(&a.ctx).unwrap().0,
        _ => crate::editor::viewport::top_view(&a.ctx).unwrap().0.centre,
    };
    let centre = shown(&a);
    drag(&mut a, Vec2::new(60.0, 0.0), Vec2::new(120.0, 5.0));
    assert_eq!(shown(&a), centre, "{view}: dragging a point does not pan");
    let st = race(&a).start.unwrap();
    assert!(st.position.y.abs() < 0.01 && (st.position.x - 120.0).abs() < 2.0 * snap_m, "{view}: {st:?}");
    assert!((st.position.z - FLOOR_Z).abs() < 0.05, "{view}: {st:?}");
    assert_eq!(undo_label(&a), Some("Move start line"), "{view}");
    let dragged = race(&a).clone();
    assert_eq!(a.state.as_ref().unwrap().session.map.races.len(), 1, "{view}: the drag did not pan into a new race");

    // Undo and redo, exact.
    let stt = a.state.as_mut().unwrap();
    assert!(crate::editor::undo::undo(stt));
    assert_eq!(race(&a), &placed, "{view}: undo of the drag");
    let stt = a.state.as_mut().unwrap();
    for _ in 0..3 {
        assert!(crate::editor::undo::undo(stt));
    }
    assert_eq!(&stt.session.map.races[0], &before, "{view}: undo of the three places");
    for _ in 0..4 {
        assert!(crate::editor::undo::redo(stt));
    }
    assert_eq!(&stt.session.map.races[0], &dragged, "{view}: redo");
    assert_eq!(steps(&a).as_deref(), Some("Move start line"), "{view}");

    // Del removes the selected point as in 3D (the start is selected after the drag).
    frame(&mut a, key(egui::Key::Delete).to_vec());
    frame(&mut a, vec![]);
    assert!(race(&a).start.is_none(), "{view}: Del");

    // Esc: back to Select; a click on empty ground then adds nothing.
    frame(&mut a, key(egui::Key::Escape).to_vec());
    let n = race(&a).checkpoints.len();
    click(&mut a, Vec2::new(250.0, 0.0));
    assert_eq!(race(&a).checkpoints.len(), n, "{view}: Select never adds");
}

#[test]
fn race_points_are_placed_and_dragged_on_the_map() {
    place_and_drag("map");
}

#[test]
fn race_points_are_placed_and_dragged_in_top() {
    place_and_drag("top");
}

/// A drag on the Map that does not start on a race point still pans.
#[test]
fn a_map_drag_off_the_points_pans() {
    let s = Scratch::new("editor-race-points-pan");
    let mut a = editor(&s, "map");
    frame(&mut a, key(egui::Key::C).to_vec());
    let before = crate::editor::sim::panel::map_transform(&a.ctx).unwrap().0;
    // Off the road and its points: 60 points above the map's middle (the road runs through it).
    let from = crate::editor::sim::panel::map_transform(&a.ctx).unwrap().2.center() - egui::vec2(0.0, 60.0);
    let to = from + egui::vec2(80.0, 0.0);
    frame(&mut a, vec![Event::PointerMoved(from)]);
    frame(
        &mut a,
        vec![Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    for k in 1..=6 {
        frame(&mut a, vec![Event::PointerMoved(from + (to - from) * (k as f32 / 6.0))]);
    }
    frame(
        &mut a,
        vec![Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    frame(&mut a, vec![]);
    let after = crate::editor::sim::panel::map_transform(&a.ctx).unwrap().0;
    assert!((after.x - before.x).abs() > 1.0, "{before} -> {after}");
    assert_eq!(race(&a).checkpoints.len(), 2);
}
