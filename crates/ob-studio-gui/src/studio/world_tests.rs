//! The map editor hosted as World and Races: drawn at the review sizes with the shell's clip and overlap check
//! and the editor's own toolbar check, the editor's keys reaching it, and going into and out of World by keyboard
//! and pad.

use glam::Vec2;
use ob_look::legends::InputKind;

use super::Studio;
use super::address::Address;
use super::layout;
use super::nav::{Area, Pad};
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::editor::app::App;
use crate::editor::{menu, state::Mode};
use crate::testkit::Scratch;
use crate::tools::map::{CustomRace, RaceKind};
use crate::world::World;

fn editor(s: &Studio, sc: &Scratch) -> App {
    App::new_in(s.ctx.clone(), sc.path().join("game"), sc.path().join("maps"), Box::new(|_, _| Ok(World::default())))
}

/// A mods folder with "Harbour edits" holding one race, and Studio on it with the editor attached.
fn studio(sc: &Scratch) -> Studio {
    let maps = sc.path().join("maps");
    std::fs::create_dir_all(&maps).unwrap();
    {
        let mut a = App::new(sc.path().join("game"), maps.clone(), Box::new(|_, _| Ok(World::default())));
        a.new_mod("harbour-edits", "Harbour edits", "L4RA").unwrap();
        let st = a.state.as_mut().unwrap();
        let mut r = CustomRace::new("sprint", 1, RaceKind::Sprint);
        r.name = "Casino sprint".into();
        r.checkpoints = vec![Vec2::ZERO, Vec2::X * 100.0];
        st.session.map.races.push(r);
        crate::editor::commit(st, "New race");
        assert!(a.close_requested());
    }
    let mut s = Studio::new(maps, Some("harbour-edits"));
    let e = editor(&s, sc);
    s.attach_editor(e);
    s
}

fn key(k: egui::Key, ctrl: bool) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if ctrl { egui::Modifiers::COMMAND } else { egui::Modifiers::NONE },
    }
}

fn press(s: &mut Studio, size: (u32, u32), events: Vec<egui::Event>) {
    s.frame(raw_input(size, 1.0, 0, events));
}

/// The four review sizes.
const REVIEW: [(u32, u32); 4] = [(1920, 1080), (1280, 800), (1024, 768), (2560, 1080)];

#[test]
fn world_hosts_the_map_editor_at_every_review_size() {
    for (w, h) in REVIEW {
        for ws in [Workspace::World, Workspace::Races] {
            for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
                let sc = Scratch::new(&format!("studio-world-{w}x{h}-{ws:?}-{input:?}"));
                let mut s = studio(&sc);
                s.nav.workspace = ws;
                s.nav.input = input;
                let scale = layout::ui_scale(w, h);
                let (_, l) = frames(&mut s, (w, h), scale, 3);
                assert!(s.editing(), "{w}x{h} {ws:?}: the editor opened the mod");
                assert!(l.problems().is_empty(), "{w}x{h} {ws:?} {input:?}: {:?}", l.problems());
                let app = &s.editor.as_ref().unwrap().app;
                assert!(app.untaken.is_empty(), "{:?}", app.untaken);
                // The editor's toolbar fits the toolbar row; its view lies in Studio's view.
                let fit = menu::toolbar_fit(&s.ctx).expect("the editor's toolbar was laid out");
                assert!(fit.problems().is_empty(), "{w}x{h} {ws:?}: {:?}", fit.problems());
                assert!(l.toolbar.unwrap().expand(1.0).contains_rect(fit.bar), "{w}x{h}: {:?}", fit.bar);
                assert!(
                    l.view.expand(1.0).contains_rect(app.view_rect),
                    "{w}x{h}: {:?} in {:?}",
                    app.view_rect,
                    l.view
                );
                assert!(app.view_rect.width() >= 400.0 && app.view_rect.height() >= 200.0, "{:?}", app.view_rect);
                let mode = app.state.as_ref().unwrap().mode;
                assert_eq!(mode, if ws == Workspace::Races { Mode::Races } else { Mode::Props });
                // The address bar names where the window is.
                let c = s.crumbs();
                assert_eq!(&c[..3], &["Harbour edits".to_string(), ws.name().to_string(), "L4RA".to_string()]);
                if ws == Workspace::Races {
                    assert_eq!(c.get(3).map(String::as_str), Some("Casino sprint"));
                    assert_eq!(s.address().to_string(), "ob://harbour-edits/race/harbour-edits:sprint");
                }
                assert!(s.close_requested());
            }
        }
    }
}

#[test]
fn into_and_out_of_world_by_keyboard_and_pad() {
    let sc = Scratch::new("studio-world-nav");
    let size = (1280, 800);
    let mut s = studio(&sc);
    frames(&mut s, size, 1.0, 1);
    assert_eq!(s.nav.workspace, Workspace::Home);
    assert!(!s.editing());
    // Ctrl+2: World, the editor on the open mod.
    press(&mut s, size, vec![key(egui::Key::Num2, true)]);
    frames(&mut s, size, 1.0, 1);
    assert_eq!(s.nav.workspace, Workspace::World);
    assert!(s.editing());
    // Ctrl+Z is the editor's undo (the race made before).
    assert_eq!(s.editor.as_ref().unwrap().app.state.as_ref().unwrap().session.map.races.len(), 1);
    press(&mut s, size, vec![key(egui::Key::Z, true)]);
    assert!(s.editor.as_ref().unwrap().app.state.as_ref().unwrap().session.map.races.is_empty(), "undone");
    // The pad's Undo / Redo (Deck grips) too.
    s.pad(Pad::R4, InputKind::Deck);
    frames(&mut s, size, 1.0, 1);
    assert_eq!(s.editor.as_ref().unwrap().app.state.as_ref().unwrap().session.map.races.len(), 1, "redone");
    // A single letter is the editor's tool key (W: Move), not the shell's.
    s.nav.mouse();
    press(&mut s, size, vec![key(egui::Key::W, false)]);
    frames(&mut s, size, 1.0, 1);
    let app = &s.editor.as_ref().unwrap().app;
    let tool = menu::current_tool(app.state.as_ref().unwrap(), &app.panels.props.borrow(), &app.panels.races.borrow());
    assert_eq!(tool, menu::Tool::Move);
    // RB: Races, the race tools forward; areas step through the editor's docks.
    s.pad(Pad::RB, InputKind::Xbox);
    frames(&mut s, size, 1.0, 1);
    assert_eq!(s.nav.workspace, Workspace::Races);
    assert_eq!(s.editor.as_ref().unwrap().app.state.as_ref().unwrap().mode, Mode::Races);
    s.pad(Pad::RT, InputKind::Xbox);
    assert_ne!(s.nav.area, Area::View);
    // B never leaves the workspace; View goes back to World, Ctrl+1 out to Home.
    s.pad(Pad::B, InputKind::Xbox);
    s.pad(Pad::B, InputKind::Xbox);
    assert_eq!(s.nav.workspace, Workspace::Races);
    s.pad(Pad::View, InputKind::Xbox);
    assert_eq!(s.nav.workspace, Workspace::World);
    press(&mut s, size, vec![key(egui::Key::Num1, true)]);
    frames(&mut s, size, 1.0, 1);
    assert_eq!(s.nav.workspace, Workspace::Home);
    assert!(!s.editing());
    // An item's address goes into the workspace that shows it.
    s.go(&Address::parse("ob://harbour-edits/race/harbour-edits:sprint").unwrap()).unwrap();
    assert_eq!(s.nav.workspace, Workspace::Races);
    assert_eq!(s.editor.as_ref().unwrap().app.state.as_ref().unwrap().race, Some(0));
    assert!(s.go(&Address::parse("ob://harbour-edits/race/harbour-edits:nope").unwrap()).is_err());
    assert!(s.close_requested());
}

#[test]
fn ctrl_d_is_changes_inside_studio_and_duplicate_shows_no_key() {
    let sc = Scratch::new("studio-world-ctrl-d");
    // The editor in its own window: Duplicate is Ctrl+D.
    let own = App::new(sc.path().join("game"), sc.path().join("maps"), Box::new(|_, _| Ok(World::default())));
    assert_eq!(menu::duplicate_key(&own.ctx), "Ctrl+D");
    assert!(!menu::hosted(&own.ctx));
    // Hosted: one key, one meaning. Ctrl+D opens Changes; Duplicate (Edit, the prop menu) shows no key.
    let mut s = studio(&sc);
    assert!(menu::hosted(&s.ctx));
    assert_eq!(menu::duplicate_key(&s.ctx), "");
    let size = (1280, 800);
    s.go(&Address::view("harbour-edits", "@world")).unwrap();
    frames(&mut s, size, 1.0, 2);
    assert!(s.editing());
    s.editor.as_mut().unwrap().app.state.as_mut().unwrap().mode = Mode::Props;
    frames(&mut s, size, 1.0, 1);
    press(&mut s, size, vec![key(egui::Key::D, true)]);
    assert_eq!(s.nav.workspace, Workspace::Changes);
}

fn names(s: &Studio) -> Vec<String> {
    menu::toolbar_fit(&s.ctx).expect("toolbar laid out").items.into_iter().map(|i| i.0).collect()
}

fn ed(s: &mut Studio) -> &mut crate::editor::state::EditorState {
    s.editor.as_mut().unwrap().app.state.as_mut().unwrap()
}

#[test]
fn world_and_races_show_their_own_tools_layers_and_state() {
    use crate::editor::collision::Show;
    use crate::editor::state::Focus;
    let sc = Scratch::new("studio-world-races-distinct");
    let size = (1280, 800);
    let mut s = studio(&sc);
    s.nav.workspace = Workspace::World;
    frames(&mut s, size, 1.0, 3);
    // World: the props' side; no race tools, Test race or Simulate in the toolbar.
    assert_eq!(ed(&mut s).focus, Focus::World);
    assert_eq!(ed(&mut s).mode, Mode::Props);
    let world = names(&s);
    assert!(world.contains(&"Move".to_string()) && world.contains(&"Drive here".to_string()), "{world:?}");
    for gone in ["Checkpoint", "Start", "Finish", "Grid", "Test race", "Simulate"] {
        assert!(!world.contains(&gone.to_string()), "World shows {gone}: {world:?}");
    }
    assert_eq!(super::world::focus_of(Workspace::World), Focus::World);
    assert_eq!(s.editor_tabs().iter().map(|t| t.0).collect::<Vec<_>>().contains(&"browser"), true);
    assert!(!s.editor_tabs().iter().any(|t| t.0 == crate::editor::sim::TAB));
    // World: a selection, the collision layer, the camera.
    ed(&mut s).selection = vec![];
    ed(&mut s).collision = Show::All;
    ed(&mut s).camera.position.x += 123.0;
    let world_cam = ed(&mut s).camera;
    // Races: the race side; race tools, Test race and Simulate; the props' tab out; its own layers.
    s.workspace(Workspace::Races);
    frames(&mut s, size, 1.0, 3);
    assert_eq!(ed(&mut s).focus, Focus::Races);
    assert_eq!(ed(&mut s).mode, Mode::Races);
    assert_eq!(ed(&mut s).race, Some(0));
    assert_eq!(ed(&mut s).collision, Show::Off, "the collision layer belongs to World");
    let races = names(&s);
    for shown in ["Checkpoint", "Start", "Finish", "Grid", "Test race", "Simulate"] {
        assert!(races.contains(&shown.to_string()), "Races lacks {shown}: {races:?}");
    }
    assert_ne!(world, races);
    assert!(!s.editor_tabs().iter().any(|t| t.0 == "browser"));
    assert!(s.editor_tabs().iter().any(|t| t.0 == crate::editor::sim::TAB));
    // The race branch alone in the left panel's rail labels.
    let labels = crate::editor::tree::rail_labels(Some(ed(&mut s)));
    assert_eq!(labels, ["Mod".to_string(), "Races 1".to_string()]);
    ed(&mut s).point = Some(crate::editor::state::RacePoint { part: crate::tools::map::ops::RacePart::Checkpoint, index: 1 });
    ed(&mut s).camera.position.x -= 77.0;
    let races_cam = ed(&mut s).camera;
    // Back in World: its own camera and layer come back, the race's point is not there.
    s.workspace(Workspace::World);
    frames(&mut s, size, 1.0, 3);
    assert_eq!(ed(&mut s).collision, Show::All);
    assert_eq!(ed(&mut s).camera.position, world_cam.position);
    assert_eq!(ed(&mut s).point, None);
    assert_eq!(crate::editor::tree::rail_labels(Some(ed(&mut s))), ["Mod".to_string(), "Props 0".to_string()]);
    s.workspace(Workspace::Races);
    frames(&mut s, size, 1.0, 3);
    assert_eq!(ed(&mut s).camera.position, races_cam.position);
    assert_eq!(ed(&mut s).point, Some(crate::editor::state::RacePoint { part: crate::tools::map::ops::RacePart::Checkpoint, index: 1 }));
    assert!(s.close_requested());
}
