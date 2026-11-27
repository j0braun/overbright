//! The view's own state (docs/modding/map-editor.md "Hiding, presets, Go to and saved views"): hiding, isolating
//! and the this-mod-only filter never reach the mod; the camera goals, Go to and the saved views.

use egui::{Key, Modifiers};
use glam::{Mat4, Vec2, Vec3};

use super::app::{App, ModView};
use super::menu::Action;
use super::state::{EditorState, Mode};
use super::views::*;
use crate::testkit::Scratch;

fn solid(name: &str) -> crate::formats::solid::Solid {
    crate::formats::solid::Solid {
        name: name.into(),
        hash: crate::formats::hash::bin_hash(name),
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

/// Three props: two cones (10 and 20 m east) and a tree (30 m east).
fn world() -> crate::world::World {
    let mut w = crate::world::World::flat(500.0);
    w.solids = vec![solid("XC_CONE_01"), solid("XC_CONE_02"), solid("TREE_A")];
    w.placements = (0..3)
        .map(|i| crate::world::Placement {
            solid: i,
            low_solid: None,
            mid_solid: None,
            transform: Mat4::from_translation(Vec3::new(10.0 * (i as f32 + 1.0), 0.0, 0.0)),
            section: 618,
            flags: 0,
            cull_radius: 5.0,
        })
        .collect();
    w
}

fn app(dir: &Scratch) -> App {
    App::new(dir.path().join("game"), dir.path().join("maps"), Box::new(|_: &std::path::Path, _: &str| Ok(world())))
}

fn frame(app: &mut App, events: Vec<egui::Event>) {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 960.0))),
        events,
        ..Default::default()
    };
    let _ = app.frame(raw);
}

fn press(k: Key, modifiers: Modifiers) -> egui::Event {
    egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn st(app: &mut App) -> &mut EditorState {
    app.state.as_mut().unwrap()
}

#[test]
fn hide_isolate_and_show_all() {
    let mut f = ViewFilter::default();
    assert!(!f.active() && !f.hides(0, false));
    f.hide(&[1]);
    assert!(f.active() && f.hides(1, true) && !f.hides(0, true));
    f.isolate(&[0, 1]);
    assert!(!f.hides(1, false), "isolating brings a hidden prop back");
    assert!(f.hides(2, true) && !f.hides(0, false));
    f.hide(&[0]);
    assert!(f.hides(0, true), "hiding inside an isolation takes it out");
    f.show_all();
    assert!(!f.active());
    f.set_mod_only(true);
    assert!(f.hides(0, false) && !f.hides(0, true), "only what the mod changed stays");
    f.show_all();
    assert!(f.mod_only, "Show all leaves the changes-only filter to its own item");
    let g = f.generation;
    f.hide(&[5]);
    f.prune(3);
    assert!(f.hidden.is_empty() && f.generation > g, "placements an undo took away are forgotten");
}

#[test]
fn the_keys_hide_isolate_and_show_all() {
    let dir = Scratch::new("views-keys");
    let mut a = app(&dir);
    a.new_mod("keys", "Keys", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    st(&mut a).selection = vec![0];
    frame(&mut a, vec![press(Key::H, Modifiers::NONE)]);
    assert!(st(&mut a).filter.hidden.contains(&0) && st(&mut a).selection.is_empty());
    st(&mut a).selection = vec![1];
    frame(&mut a, vec![press(Key::H, Modifiers::SHIFT)]);
    assert_eq!(st(&mut a).filter.isolated.as_ref().map(|s| s.len()), Some(1));
    frame(&mut a, vec![press(Key::H, Modifiers::ALT)]);
    assert!(!st(&mut a).filter.active());
    // With nothing selected H says so and hides nothing.
    st(&mut a).selection.clear();
    frame(&mut a, vec![press(Key::H, Modifiers::NONE)]);
    assert!(!st(&mut a).filter.active());
}

/// Hidden, isolated and changes-only are the view's: the mod, its history and its saved file stay as they were.
#[test]
fn hiding_is_never_written_to_the_mod() {
    let dir = Scratch::new("views-not-in-mod");
    let mut a = app(&dir);
    a.new_mod("view", "View", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    a.save().unwrap();
    let file = dir.path().join("maps/view/map.obmap");
    let before = std::fs::read(&file).unwrap();
    let (map, flags): (_, Vec<u16>) = {
        let s = st(&mut a);
        (s.session.map.clone(), s.session.world.placements.iter().map(|p| p.flags).collect())
    };
    st(&mut a).selection = vec![0, 1];
    a.act(Action::HideSelected);
    st(&mut a).selection = vec![2];
    a.act(Action::IsolateSelected);
    a.act(Action::ModOnly);
    frame(&mut a, Vec::new());
    {
        let s = st(&mut a);
        assert!(s.filter.active());
        assert!(!s.session.dirty, "no unsaved change");
        assert_eq!(s.session.map, map, "the mod is as it was");
        assert_eq!(s.session.world.placements.iter().map(|p| p.flags).collect::<Vec<_>>(), flags, "the world's flags too");
        assert!(super::views::view_hides(s, 0) && super::views::view_hides(s, 1));
        // What the view hides is not picked.
        let eye = Vec3::new(0.0, 0.0, 0.5);
        let hit = |s: &EditorState| {
            super::viewport::pick_where(&s.session.world, eye, Vec3::X, &|i| super::views::view_hides(s, i)).map(|h| h.placement)
        };
        assert_eq!(super::viewport::pick(&s.session.world, eye, Vec3::X).map(|h| h.placement), Some(0));
        assert_ne!(hit(s), Some(0));
    }
    a.save().unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), before, "the saved map is byte for byte the same");
    // Undo has nothing to undo: hiding is not a step.
    assert!(!super::undo::undo(st(&mut a)));
    a.act(Action::ShowAll);
    assert!(!st(&mut a).filter.hidden.is_empty() || st(&mut a).filter.isolated.is_none());
    assert!(!st(&mut a).filter.mod_only || st(&mut a).filter.active());
}

#[test]
fn changes_only_shows_what_the_mod_moved_or_added() {
    let dir = Scratch::new("views-changes-only");
    let mut a = app(&dir);
    a.new_mod("changes", "Changes", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    let s = st(&mut a);
    s.session.world.placements[1].transform = Mat4::from_translation(Vec3::new(21.0, 0.0, 0.0));
    s.filter.set_mod_only(true);
    let hidden = hidden_set(s);
    assert!(hidden.contains(&0) && hidden.contains(&2) && !hidden.contains(&1), "{hidden:?}");
}

#[test]
fn go_to_reads_coordinates_names_and_race_points() {
    let dir = Scratch::new("views-goto");
    let mut a = app(&dir);
    a.new_mod("goto", "Goto", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    let s = st(&mut a);
    s.camera = crate::draw::camera::Camera::new(Vec3::new(31.0, 0.0, 5.0), 0.0, 0.0);
    assert_eq!(resolve(s, "12.5, -40"), Ok(Target::At(Vec2::new(12.5, -40.0))));
    assert_eq!(resolve(s, "1 2 3"), Ok(Target::At(Vec2::new(1.0, 2.0))));
    assert_eq!(resolve(s, "tree"), Ok(Target::Prop { index: 2, of: 1 }));
    assert_eq!(resolve(s, "XC_CONE_02"), Ok(Target::Prop { index: 1, of: 1 }));
    // Two matches: the nearer to the camera.
    assert_eq!(resolve(s, "cone"), Ok(Target::Prop { index: 1, of: 2 }));
    assert!(resolve(s, "no such prop").unwrap_err().contains("Nothing named"));
    assert!(resolve(s, " ").is_err());
    assert_eq!(resolve(s, "start").unwrap_err(), "Select a race first");
    let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (200.0, 0.0)]);
    race.start = Some(crate::tools::map::Spot { position: Vec3::new(5.0, 6.0, 0.0), heading: 0.0 });
    s.session.map.races = vec![race];
    s.race = Some(0);
    assert_eq!(
        resolve(s, "start"),
        Ok(Target::Race(super::state::RacePoint { part: crate::tools::map::edit::RacePart::Start, index: 0 }, Vec2::new(5.0, 6.0)))
    );
    assert_eq!(
        resolve(s, "checkpoint 2"),
        Ok(Target::Race(
            super::state::RacePoint { part: crate::tools::map::edit::RacePart::Checkpoint, index: 1 },
            Vec2::new(200.0, 0.0)
        ))
    );
    assert!(resolve(s, "checkpoint 9").is_err() && resolve(s, "checkpoint 0").is_err());
    // Going to a prop selects it, frames it and brings it back from hiding.
    s.filter.hide(&[2]);
    go_to(s, "tree").unwrap();
    assert_eq!(s.selection, vec![2]);
    assert_eq!((s.mode, s.goal), (Mode::Props, Some(Goal::Frame)));
    assert!(!s.filter.hidden.contains(&2));
    go_to(s, "3, 4").unwrap();
    assert_eq!(s.goal, Some(Goal::At(Vec2::new(3.0, 4.0))));
}

#[test]
fn the_isometric_camera_looks_down_at_the_target_from_a_corner() {
    let c = isometric(Vec3::new(10.0, 20.0, 0.0), 100.0);
    let f = c.forward();
    assert!(f.z < -0.4 && f.x > 0.0 && f.y > 0.0, "{f:?}");
    assert!((c.position.distance(Vec3::new(10.0, 20.0, 0.0)) - 100.0).abs() < 0.5);
    assert!(isometric(Vec3::ZERO, 1.0).position.length() >= 39.0, "a minimum distance");
}

/// Saved views keep the camera and the layers per mod, in `editor.toml`, and bring them back.
#[test]
fn saved_views_come_back_with_the_mod() {
    use super::collision::Show;
    let dir = Scratch::new("views-saved");
    let maps = dir.path().join("maps");
    let mut a = app(&dir);
    a.new_mod("saved", "Saved", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    st(&mut a).camera = crate::draw::camera::Camera::new(Vec3::new(1.0, 2.0, 3.0), 0.5, -0.25);
    st(&mut a).collision = Show::All;
    st(&mut a).race_barriers = true;
    a.save_view("Junction");
    st(&mut a).collision = Show::Off;
    st(&mut a).camera = crate::draw::camera::Camera::new(Vec3::new(9.0, 9.0, 9.0), 0.0, 0.0);
    a.save_view("  Junction ");
    assert_eq!(st(&mut a).views.len(), 1, "the same name replaces");
    a.save_view("Overview");
    a.save_view("   ");
    assert_eq!(st(&mut a).views.len(), 2, "an empty name saves nothing");
    let on_disk = ModView::read(&maps, "saved").views;
    assert_eq!(on_disk, st(&mut a).views, "kept with the mod");
    assert!(a.close_requested());
    drop(a);
    let mut b = app(&dir);
    b.open("saved", None).unwrap();
    frame(&mut b, Vec::new());
    assert_eq!(st(&mut b).views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["Junction", "Overview"]);
    b.act(Action::RecallView(0));
    let s = st(&mut b);
    assert_eq!(s.collision, Show::Off, "the layers of the moment it was saved");
    assert_eq!(s.goal, Some(Goal::Camera(Vec3::new(9.0, 9.0, 9.0), 0.0, 0.0)));
    b.act(Action::DeleteView(0));
    assert_eq!(st(&mut b).views.len(), 1);
    // The views are not in the mod.
    let map = std::fs::read_to_string(dir.path().join("maps/saved/map.obmap")).unwrap_or_default();
    assert!(!map.contains("Junction"));
}

#[test]
fn camera_actions_switch_to_the_3d_view() {
    let dir = Scratch::new("views-goal");
    let mut a = app(&dir);
    a.new_mod("goal", "Goal", "L4RA").unwrap();
    frame(&mut a, Vec::new());
    a.act(Action::Goal(Goal::Isometric));
    assert_eq!(st(&mut a).goal, Some(Goal::Isometric));
    // The viewport takes the goal in its next frame.
    frame(&mut a, Vec::new());
    assert!(st(&mut a).goal.is_none());
    assert!(st(&mut a).camera.forward().z < 0.0);
}
