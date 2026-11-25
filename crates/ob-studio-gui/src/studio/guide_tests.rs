//! The tutorials walked through headless, by keyboard (F1 and the dialog's keys) and by pad (Home's card),
//! on a flat city with cones; the card and Home's new rows fit at every common size.

use glam::Vec3;
use ob_look::legends::InputKind;

use super::Studio;
use super::guide::{self, Step, Tutorial};
use super::nav::Pad;
use super::shell::HomeAct;
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::editor::app::App;
use crate::testkit::Scratch;
use crate::tools::map::PropEdit;
use crate::world::World;

/// A flat city 4 km across (the tutorial's spot is on it) with the cone the tutorial places.
fn city() -> World {
    let solid = |name: &str| crate::formats::solid::Solid {
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
    };
    let mut w = World::flat(2000.0);
    w.solids = vec![solid("XO_CONE_01")];
    w
}

fn studio(sc: &Scratch) -> Studio {
    let maps = sc.path().join("maps");
    std::fs::create_dir_all(&maps).unwrap();
    let mut s = Studio::new(maps.clone(), None);
    let app = App::new_in(s.ctx.clone(), sc.path().join("game"), maps, Box::new(|_, _| Ok(city())));
    s.attach_editor(app);
    s.mod_page.set_game(None);
    s
}

fn key(k: egui::Key) -> egui::Event {
    egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
}

const SIZE: (u32, u32) = (1280, 800);

thread_local! {
    /// The window the walkthroughs run at (size, UI scale): 1280x800 at 1, or one of the polish sizes.
    static VIEW: std::cell::Cell<((u32, u32), f32)> = const { std::cell::Cell::new((SIZE, 1.0)) };
}

fn size() -> (u32, u32) {
    VIEW.with(|v| v.get().0)
}

fn scale() -> f32 {
    VIEW.with(|v| v.get().1)
}

/// Two frames; the window's layout has no clipped or overlapping part.
fn fits(s: &mut Studio) {
    let (_, l) = frames(s, size(), scale(), 2);
    assert!(l.problems().is_empty(), "{:?} at {:?}: {:?}", s.nav.workspace, VIEW.with(|v| v.get()), l.problems());
}

/// A key, then two frames; the window's layout has no clipped or overlapping part.
fn press(s: &mut Studio, k: egui::Key) {
    s.frame(raw_input(size(), scale(), 0, vec![key(k)]));
    let (_, l) = frames(s, size(), scale(), 2);
    assert!(l.problems().is_empty(), "{:?} after {k:?} at {:?}: {:?}", s.nav.workspace, VIEW.with(|v| v.get()), l.problems());
}

fn step(s: &Studio) -> Step {
    s.guide.as_ref().expect("the tutorial runs").step
}

fn added(s: &Studio) -> usize {
    let st = s.editor.as_ref().unwrap().app.state.as_ref().unwrap();
    st.session.map.props.iter().filter(|p| matches!(p, PropEdit::Add { .. })).count()
}

/// Share it: the Mod page's own export, into the scratch folder (never the player's exports folder).
fn export(s: &mut Studio, sc: &Scratch) {
    assert_eq!(s.nav.workspace, Workspace::Mod);
    fits(s);
    s.mod_page.export_to = sc.path().join("out.zip").display().to_string();
    // The card's button on the Mod page is the page's Export.
    s.guide_main();
    assert!(sc.path().join("out.zip").is_file(), "{:?}", s.mod_page.note);
    frames(s, size(), scale(), 1);
}

#[test]
fn the_tutorial_by_keyboard() {
    first_mod_by_keyboard("studio-guide-keys");
}

#[test]
fn the_tutorial_by_pad() {
    first_mod_by_pad("studio-guide-pad");
}

/// The polish walkthroughs: Home → the tutorial → World → the Mod page → export, by keyboard and by pad, at the
/// smallest common window and at 4K with the UI at 2.
#[test]
fn the_tutorial_walks_through_at_1024x768_and_4k() {
    for (w, h, sc) in [(1024, 768, 1.0), (3840, 2160, 2.0)] {
        VIEW.with(|v| v.set(((w, h), sc)));
        first_mod_by_keyboard(&format!("studio-guide-keys-{w}"));
        first_mod_by_pad(&format!("studio-guide-pad-{w}"));
    }
    VIEW.with(|v| v.set((SIZE, 1.0)));
}

fn first_mod_by_keyboard(name: &str) {
    let sc = Scratch::new(name);
    let mut s = studio(&sc);
    frames(&mut s, size(), scale(), 2);
    // F1 starts it on Home; F1 again opens the New mod dialog with the tutorial's name; Enter makes the mod.
    press(&mut s, egui::Key::F1);
    assert_eq!((s.nav.workspace, step(&s)), (Workspace::Home, Step::MakeMod));
    press(&mut s, egui::Key::F1);
    assert_eq!(s.new_mod.as_ref().unwrap().name, guide::MOD_NAME);
    press(&mut s, egui::Key::Enter);
    assert!(s.new_mod.is_none());
    assert_eq!(s.open_mod().unwrap().folder, "my-first-mod");
    assert_eq!(step(&s), Step::GoToStreet);
    // Take me there: World, the camera at street level, the ground in view.
    press(&mut s, egui::Key::F1);
    assert_eq!(s.nav.workspace, Workspace::World);
    let cam = s.editor.as_ref().unwrap().app.state.as_ref().unwrap().camera.position;
    assert!(cam.distance(Vec3::from(guide::SPOT)) < 60.0, "{cam}");
    assert_eq!(step(&s), Step::PlaceProp);
    // Place a cone: the editor's own add, one step (Ctrl+Z would take it back).
    press(&mut s, egui::Key::F1);
    assert_eq!(added(&s), 1);
    assert_eq!(step(&s), Step::DriveIt);
    // Drive here is asked of the editor (no game in the test: it says so), then a started run ticks it.
    press(&mut s, egui::Key::F1);
    assert!(!s.editor.as_ref().unwrap().app.state.as_ref().unwrap().status.is_empty());
    assert_eq!(step(&s), Step::DriveIt);
    crate::editor::testplay::count_start(&s.ctx);
    frames(&mut s, size(), scale(), 1);
    assert_eq!(step(&s), Step::ShareIt);
    // The card floats over the view and fits in it.
    let r = s.guide_rect.expect("the card is drawn over World");
    assert!(s.last.as_ref().unwrap().view.contains_rect(r), "{r:?}");
    press(&mut s, egui::Key::F1);
    export(&mut s, &sc);
    assert_eq!(step(&s), Step::Done);
    press(&mut s, egui::Key::F1);
    assert!(s.guide.is_none());
}

fn first_mod_by_pad(name: &str) {
    let sc = Scratch::new(name);
    let mut s = studio(&sc);
    s.nav.input = InputKind::Xbox;
    frames(&mut s, size(), scale(), 2);
    // START HERE is one of Home's controls: down to it, A.
    let at = s.home_targets().iter().position(|a| *a == HomeAct::StartTutorial(guide::Tutorial::FirstMod)).unwrap();
    for _ in 0..at {
        s.pad(Pad::Down, InputKind::Xbox);
    }
    s.pad(Pad::A, InputKind::Xbox);
    assert_eq!(step(&s), Step::MakeMod);
    // Every step from Home's card: A does it, LB … back to Home.
    let card = |s: &mut Studio| {
        if s.nav.workspace != Workspace::Home {
            s.command(super::nav::Command::Workspace(Workspace::Home), Some(InputKind::Xbox));
        }
        frames(s, size(), scale(), 1);
        for _ in 0..12 {
            s.pad(Pad::Up, InputKind::Xbox);
        }
        assert_eq!(s.home_targets()[s.home_focus], HomeAct::GuideMain);
        s.pad(Pad::A, InputKind::Xbox);
        frames(s, size(), scale(), 1);
        fits(s);
    };
    card(&mut s);
    s.pad(Pad::A, InputKind::Xbox);
    frames(&mut s, size(), scale(), 2);
    assert_eq!(step(&s), Step::GoToStreet);
    card(&mut s);
    assert_eq!(step(&s), Step::PlaceProp);
    card(&mut s);
    assert_eq!(added(&s), 1);
    card(&mut s);
    crate::editor::testplay::count_start(&s.ctx);
    frames(&mut s, size(), scale(), 1);
    assert_eq!(step(&s), Step::ShareIt);
    card(&mut s);
    export(&mut s, &sc);
    assert_eq!(step(&s), Step::Done);
    // The last card's second button: Races, the tutorial closed.
    s.command(super::nav::Command::Workspace(Workspace::Home), Some(InputKind::Xbox));
    frames(&mut s, size(), scale(), 1);
    s.pad(Pad::Down, InputKind::Xbox);
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.guide.is_none());
    assert_eq!(s.nav.workspace, Workspace::Races);
}

/// Home with the new rows, and every workspace with the card, at the resolution matrix: nothing clipped or
/// overlapping, the card inside the view.
#[test]
fn home_and_the_card_fit_at_every_size() {
    let sizes = [
        (1024, 768, 1.0),
        (1280, 720, 1.0),
        (1366, 768, 1.0),
        (1920, 1080, 1.0),
        (2560, 1440, 1.5),
        (3840, 2160, 2.0),
        (2560, 1080, 1.0),
        (3440, 1440, 1.0),
        (1280, 800, 1.0),
    ];
    for (w, h, scale) in sizes {
        let sc = Scratch::new(&format!("studio-guide-fit-{w}x{h}"));
        let mut s = studio(&sc);
        // Home with no mod open has every start row too.
        let (_, l) = frames(&mut s, (w, h), scale, 3);
        assert!(l.problems().is_empty(), "{w}x{h} no mod: {:?}", l.problems());
        let t = s.home_targets();
        let templates = t.iter().filter(|a| matches!(a, HomeAct::Template(_))).count();
        assert!(templates == 3 && t.contains(&HomeAct::StartTutorial(Tutorial::FirstRace)), "{w}x{h}: {t:?}");
        s.create_mod("harbour-edits", "Harbour edits").unwrap();
        s.create_mod("my-first-mod", "My first mod").unwrap();
        for (tutorial, which) in [(false, None), (true, Some(Tutorial::FirstMod)), (true, Some(Tutorial::FirstRace))] {
            if let Some(t) = which {
                s.start_tutorial(t);
            }
            for ws in [Workspace::Home, Workspace::World, Workspace::Races, Workspace::Mod, Workspace::Changes] {
                s.nav.workspace = ws;
                let (_, l) = frames(&mut s, (w, h), scale, 3);
                assert!(l.problems().is_empty(), "{w}x{h} {ws:?}: {:?}", l.problems());
                if tutorial && ws != Workspace::Home {
                    let r = s.guide_rect.unwrap_or_else(|| panic!("{w}x{h} {ws:?}: no card"));
                    assert!(l.view.contains_rect(r), "{w}x{h} {ws:?}: {r:?} in {:?}", l.view);
                }
                if ws == Workspace::Home && !tutorial {
                    let t = s.home_targets();
                    assert!(t.contains(&HomeAct::StartTutorial(guide::Tutorial::FirstRace)), "{w}x{h}");
                    assert!(t.contains(&HomeAct::Place(Workspace::World)), "{w}x{h}: {t:?}");
                    // Every size has both tutorials, Empty map mod, both templates and the sample.
                    let templates = t.iter().filter(|a| matches!(a, HomeAct::Template(_))).count();
                    assert!(t.contains(&HomeAct::StartTutorial(Tutorial::FirstMod)), "{w}x{h}: {t:?}");
                    assert!(t.contains(&HomeAct::NewMod) && templates == 3, "{w}x{h}: {t:?}");
                }
            }
        }
    }
}

// --- the race tutorial -------------------------------------------------------------------------------------------

/// A straight base track (4001) under the race tutorial's spot, running the race's way (south) for 400 m.
fn dock_road() -> crate::formats::roads::RoadNet {
    let mut net = crate::editor::validate::tests::straight(false);
    let spot = glam::Vec2::new(guide::RACE_SPOT[0], guide::RACE_SPOT[1]);
    let at = |p: glam::Vec2| spot + glam::Vec2::new(0.0, 20.0 - p.x);
    for n in &mut net.nodes {
        n.0 = at(n.0);
    }
    for e in &mut net.barrier_ends {
        *e = [at(e[0]), at(e[1])];
    }
    net
}

fn dock_assets() -> Box<dyn crate::tools::map::assets::MapAssets> {
    Box::new(crate::tools::map::assets::GivenAssets {
        tracks: vec![crate::tools::map::assets::TrackInfo { number: 4001, name: "Docks".into() }],
        nets: [(4001, dock_road())].into(),
        ..Default::default()
    })
}

/// The editor opened on the tutorial's mod in Races, its base tracks the dock road (the scratch game folder has
/// none), then back to Home.
fn open_on_the_dock_road(s: &mut Studio) {
    s.command(super::nav::Command::Workspace(Workspace::Races), None);
    frames(s, size(), scale(), 2);
    let app = &mut s.editor.as_mut().unwrap().app;
    // The race tools read the tracks once: from the new assets.
    *app.panels.races.borrow_mut() = crate::editor::races::Races::new();
    let st = app.state.as_mut().expect("the mod is open in the editor");
    st.assets = dock_assets();
    s.mod_page.given_tracks = Some(dock_assets);
    s.command(super::nav::Command::Workspace(Workspace::Home), None);
    frames(s, size(), scale(), 1);
}

fn race(s: &Studio) -> crate::tools::map::CustomRace {
    let st = s.editor.as_ref().unwrap().app.state.as_ref().unwrap();
    st.session.map.races.first().cloned().unwrap_or_else(|| panic!("a race: {}", st.status))
}

/// Steps 2 to 4 from the card's button (`main`), checked as they tick: the race on the dock road's base track,
/// its start on the road facing down it, its points along the road, no error left.
fn race_steps(s: &mut Studio, main: &dyn Fn(&mut Studio)) {
    assert_eq!(step(s), Step::BaseTrack);
    main(s);
    assert_eq!(s.nav.workspace, Workspace::Races);
    assert_eq!(race(s).track, 4001, "the base track under the camera");
    assert_eq!(step(s), Step::PlaceStart);
    main(s);
    let st = race(s).start.expect("a start line");
    assert!((st.position.x - guide::RACE_SPOT[0]).abs() < 0.5, "on the road: {st:?}");
    assert!((st.heading + 90.0).abs() < 1.0, "facing down the road (south): {st:?}");
    assert_eq!(step(s), Step::RacePoints);
    main(s);
    let r = race(s);
    assert_eq!(r.checkpoints.len(), guide::CHECKPOINTS_AT.len(), "{r:?}");
    assert!(r.checkpoints.iter().all(|c| (c.x - guide::RACE_SPOT[0]).abs() < 0.5), "on the road: {r:?}");
    assert!(r.finish.is_some());
    let issues = &s.editor.as_ref().unwrap().app.state.as_ref().unwrap().session.issues;
    assert_eq!(step(s), Step::TestRace, "{issues:?}");
}

#[test]
fn the_race_tutorial_by_keyboard() {
    let sc = Scratch::new("studio-race-guide-keys");
    let mut s = studio(&sc);
    frames(&mut s, size(), scale(), 2);
    // START HERE's second row: down to it, Enter.
    let at = s.home_targets().iter().position(|a| *a == HomeAct::StartTutorial(Tutorial::FirstRace)).unwrap();
    for _ in 0..at {
        press(&mut s, egui::Key::ArrowDown);
    }
    press(&mut s, egui::Key::Enter);
    assert_eq!((s.nav.workspace, step(&s)), (Workspace::Home, Step::MakeMod));
    press(&mut s, egui::Key::F1);
    assert_eq!(s.new_mod.as_ref().unwrap().name, guide::RACE_MOD_NAME);
    press(&mut s, egui::Key::Enter);
    assert_eq!(s.open_mod().unwrap().folder, "my-first-race");
    open_on_the_dock_road(&mut s);
    race_steps(&mut s, &|s| press(s, egui::Key::F1));
    // Test race is asked of the editor (no game in the test), then a started run ticks it.
    press(&mut s, egui::Key::F1);
    assert_eq!(step(&s), Step::TestRace);
    crate::editor::testplay::count_start(&s.ctx);
    frames(&mut s, size(), scale(), 1);
    assert_eq!(step(&s), Step::ShareIt);
    let r = s.guide_rect.expect("the card is drawn over Races");
    assert!(s.last.as_ref().unwrap().view.contains_rect(r), "{r:?}");
    press(&mut s, egui::Key::F1);
    export(&mut s, &sc);
    assert_eq!(step(&s), Step::Done);
    // The race is in the export and on the disk as it was made.
    let m = crate::tools::map::store::read_file(&s.open_mod().unwrap().dir.join("map.obmap"));
    assert_eq!(m.map(|m| m.races.len()).ok(), Some(1));
    // The last card's second button opens the sample.
    s.guide_second();
    assert!(s.guide.is_none());
    assert_eq!(s.open_mod().unwrap().folder, super::shell::SAMPLE_FOLDER);
}

#[test]
fn the_race_tutorial_by_pad() {
    let sc = Scratch::new("studio-race-guide-pad");
    let mut s = studio(&sc);
    s.nav.input = InputKind::Xbox;
    frames(&mut s, size(), scale(), 2);
    let at = s.home_targets().iter().position(|a| *a == HomeAct::StartTutorial(Tutorial::FirstRace)).unwrap();
    for _ in 0..at {
        s.pad(Pad::Down, InputKind::Xbox);
    }
    s.pad(Pad::A, InputKind::Xbox);
    assert_eq!(step(&s), Step::MakeMod);
    let card = |s: &mut Studio| {
        if s.nav.workspace != Workspace::Home {
            s.command(super::nav::Command::Workspace(Workspace::Home), Some(InputKind::Xbox));
        }
        frames(s, size(), scale(), 1);
        for _ in 0..16 {
            s.pad(Pad::Up, InputKind::Xbox);
        }
        assert_eq!(s.home_targets()[s.home_focus], HomeAct::GuideMain);
        s.pad(Pad::A, InputKind::Xbox);
        frames(s, size(), scale(), 1);
        fits(s);
    };
    card(&mut s);
    s.pad(Pad::A, InputKind::Xbox);
    frames(&mut s, size(), scale(), 2);
    assert_eq!(s.open_mod().unwrap().folder, "my-first-race");
    open_on_the_dock_road(&mut s);
    race_steps(&mut s, &|s| card(s));
    card(&mut s);
    crate::editor::testplay::count_start(&s.ctx);
    frames(&mut s, size(), scale(), 1);
    assert_eq!(step(&s), Step::ShareIt);
    card(&mut s);
    export(&mut s, &sc);
    assert_eq!(step(&s), Step::Done);
}

/// Home's templates and the sample, by keyboard and pad: a template is the New mod dialog with it, the mod made
/// with its race; the sample opens your copy (made once).
#[test]
fn templates_and_the_sample_from_home() {
    use ob_studio::create::Template;
    let sc = Scratch::new("studio-templates");
    let mut s = studio(&sc);
    frames(&mut s, size(), scale(), 2);
    let to = |s: &mut Studio, a: HomeAct| {
        let at = s.home_targets().iter().position(|x| *x == a).unwrap_or_else(|| panic!("{a:?}"));
        for _ in 0..30 {
            press(s, egui::Key::ArrowUp);
        }
        for _ in 0..at {
            press(s, egui::Key::ArrowDown);
        }
        assert_eq!(s.home_targets()[s.home_focus], a);
    };
    to(&mut s, HomeAct::Template(Template::Circuit));
    press(&mut s, egui::Key::Enter);
    assert_eq!(s.new_mod.as_ref().map(|d| d.template), Some(Template::Circuit));
    s.frame(raw_input(size(), scale(), 0, vec![egui::Event::Text("Downtown laps".into())]));
    frames(&mut s, size(), scale(), 1);
    press(&mut s, egui::Key::Enter);
    let m = s.open_mod().expect("made and open");
    assert_eq!((m.folder.as_str(), m.races), ("downtown-laps", 1));
    // The sample by pad: A on its row, twice: one copy.
    s.nav.input = InputKind::Xbox;
    for round in 0..2 {
        frames(&mut s, size(), scale(), 1);
        let at = s.home_targets().iter().position(|x| *x == HomeAct::Template(Template::Sample)).unwrap();
        for _ in 0..30 {
            s.pad(Pad::Up, InputKind::Xbox);
        }
        for _ in 0..at {
            s.pad(Pad::Down, InputKind::Xbox);
        }
        s.pad(Pad::A, InputKind::Xbox);
        let m = s.open_mod().expect("the sample is open");
        assert_eq!((m.folder.as_str(), m.races, m.added), (super::shell::SAMPLE_FOLDER, 3, 66), "round {round}");
    }
    assert_eq!(s.mods.len(), 2);
}
