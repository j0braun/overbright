//! The Runs workspace on recorded runs: the list, opening a run into the editor's own playback, comparing two,
//! the telemetry against the run file, rename and delete, the keys, and the layout at the review sizes.

use glam::Vec2;
use ob_look::legends::InputKind;

use super::Studio;
use super::layout;
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::editor::app::App;
use crate::editor::sim::record::Recording;
use crate::editor::sim::summary::tests::lines;
use crate::editor::sim::{graphs, library, telemetry};
use crate::testkit::Scratch;
use crate::tools::map::{CustomRace, RaceKind};
use crate::world::World;

/// A mod "harbour-edits" with the race "r" along the x axis (the recorded runs' race), kept runs `seed1` (a prop
/// crash, as `lines(1, true, false)`) and `seed2` (a crash at the start), and Studio on it with the editor.
fn studio(sc: &Scratch) -> Studio {
    let maps = sc.path().join("maps");
    std::fs::create_dir_all(&maps).unwrap();
    {
        let mut a = App::new(sc.path().join("game"), maps.clone(), Box::new(|_, _| Ok(World::default())));
        a.new_mod("harbour-edits", "Harbour edits", "L4RA").unwrap();
        let st = a.state.as_mut().unwrap();
        let mut r = CustomRace::new("r", 1, RaceKind::Sprint);
        r.checkpoints = vec![Vec2::new(-100.0, 0.0), Vec2::new(1300.0, 0.0)];
        st.session.map.races.push(r);
        crate::editor::commit(st, "New race");
        assert!(a.close_requested());
    }
    let dir = library::dir(&maps, "harbour-edits");
    for (name, seed, start_crash) in [("seed1", 1, false), ("seed2", 2, true)] {
        let text: String = lines(seed, true, start_crash).iter().map(|l| l.write() + "\n").collect();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(library::path(&dir, name), text).unwrap();
    }
    let mut s = Studio::new(maps, Some("harbour-edits"));
    let e = App::new_in(
        s.ctx.clone(),
        sc.path().join("game"),
        sc.path().join("maps"),
        Box::new(|_, _| Ok(World::default())),
    );
    s.attach_editor(e);
    s
}

fn dir(s: &Studio) -> std::path::PathBuf {
    library::dir(&s.maps, "harbour-edits")
}

fn sim(s: &Studio) -> crate::editor::sim::panel::Shared {
    s.editor.as_ref().unwrap().app.panels.sim.clone()
}

fn ctrl(k: egui::Key) -> egui::Event {
    egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }
}

const SIZE: (u32, u32) = (1280, 800);

/// Open the Runs workspace and let the mod open in the editor.
fn enter(s: &mut Studio) {
    s.frame(raw_input(SIZE, 1.0, 0, vec![ctrl(egui::Key::Num5)]));
    assert_eq!(s.nav.workspace, Workspace::Runs);
    frames(s, SIZE, 1.0, 3);
}

#[test]
fn the_list_opens_a_run_into_the_editors_playback_and_compares_two() {
    let sc = Scratch::new("runs-open");
    let mut s = studio(&sc);
    enter(&mut s);
    assert_eq!(s.address().to_string(), "ob://harbour-edits/@runs");
    assert!(s.editing());
    let names: Vec<_> = s.runs.entries().iter().map(|e| e.name.clone()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"seed1".to_string()) && names.contains(&"seed2".to_string()));
    // The tab of the graphs is the one shown, first among the editor's.
    assert_eq!(s.editor_tab(), Some("telemetry"));
    assert_eq!(s.editor_tabs()[0].0, "telemetry");

    let d = dir(&s);
    let (sm, ctx) = (sim(&s), s.ctx.clone());
    s.runs.open(&d, &sm, &ctx, "seed1");
    frames(&mut s, SIZE, 1.0, 3);
    {
        let b = sm.borrow();
        assert_eq!(b.shown, "seed1");
        assert!(!b.rec.frames.is_empty() && b.rec.end.as_deref() == Some("limit"));
        assert_eq!(b.rec.header.as_ref().unwrap().seed, 1);
        assert!(b.ghost.is_none());
    }
    assert_eq!(s.crumbs().last().map(String::as_str), Some("seed1"));

    // Compare: the other run is drawn as ghosts on the same clock, and the runs part where seed 2 crashed.
    s.runs.compare(&d, &sm, "seed2");
    frames(&mut s, SIZE, 1.0, 2);
    let mut b = sm.borrow_mut();
    assert_eq!(b.ghost_name, "seed2");
    let rec = b.rec.clone();
    b.play.seek_frame(&rec, 10.0);
    let ghosts = b.ghost_cars();
    assert_eq!(ghosts.len(), 3, "every racer of run B");
    let at = rec.cars_at(10.0);
    let g = ghosts.iter().find(|c| c.id == 2).unwrap();
    assert_eq!(g.position, at.iter().find(|c| c.id == 2).unwrap().position, "the runs drove the same until 1.5 s");
    drop(b);
    // Stop comparing.
    s.runs.compare(&d, &sm, "seed2");
    assert!(sm.borrow().ghost.is_none());
}

/// The graphs show what the run file says, car by car.
#[test]
fn telemetry_values_match_the_run_file() {
    let sc = Scratch::new("runs-telemetry");
    let mut s = studio(&sc);
    enter(&mut s);
    let d = dir(&s);
    let (sm, ctx) = (sim(&s), s.ctx.clone());
    s.runs.open(&d, &sm, &ctx, "seed1");
    frames(&mut s, SIZE, 1.0, 3);
    let file = Recording::read(&std::fs::read_to_string(library::path(&d, "seed1")).unwrap());
    let b = sm.borrow();
    let state = s.editor.as_ref().unwrap().app.state.as_ref().unwrap();
    let line = graphs::line_of(state, &b).expect("the run's race is in the mod");
    for id in [0u32, 1, 2] {
        let t = telemetry::of(&b.rec, id, Some(&line));
        let want = telemetry::of(&file, id, Some(&line));
        assert_eq!(t, want, "car {id}: the same as read from the file");
        let lat = t.lateral.as_ref().unwrap();
        for f in &file.frames {
            let c = f.cars.iter().find(|c| c.id == id).unwrap();
            assert_eq!(telemetry::value_at(&t.speed, f.time), Some(c.speed));
            assert!((telemetry::value_at(lat, f.time).unwrap() - c.position.y).abs() < 1e-3, "car {id} at {}", f.time);
        }
    }
    // The player's slot went 40 m off the line between 30 s and 34 s.
    let t = telemetry::of(&b.rec, 0, Some(&line));
    assert!((telemetry::value_at(t.lateral.as_ref().unwrap(), 32.0).unwrap() - 40.0).abs() < 1e-3);
    assert_eq!(graphs::car(&b), Some(0), "the first racer, until one is chosen");
}

#[test]
fn rename_and_delete_keep_the_list_and_the_view_honest() {
    let sc = Scratch::new("runs-files");
    let mut s = studio(&sc);
    enter(&mut s);
    let d = dir(&s);
    let (sm, ctx) = (sim(&s), s.ctx.clone());
    s.runs.open(&d, &sm, &ctx, "seed1");
    frames(&mut s, SIZE, 1.0, 3);
    s.runs.compare(&d, &sm, "seed2");
    s.runs.rename(&d, &sm, "seed1", "baseline");
    s.runs.rename(&d, &sm, "seed2", "baseline");
    assert!(s.runs.note.as_deref().unwrap().contains("already"), "{:?}", s.runs.note);
    frames(&mut s, SIZE, 1.0, 2);
    let names: Vec<_> = s.runs.entries().iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"baseline".to_string()) && !names.contains(&"seed1".to_string()), "{names:?}");
    assert_eq!(sm.borrow().shown, "baseline", "the run shown keeps showing under its new name");
    s.runs.delete(&d, &sm, "seed2");
    assert!(sm.borrow().ghost.is_none(), "a deleted run is not compared any more");
    s.runs.delete(&d, &sm, "baseline");
    frames(&mut s, SIZE, 1.0, 2);
    assert!(s.runs.entries().is_empty());
    assert!(sm.borrow().rec.frames.is_empty() && sm.borrow().shown.is_empty());
    assert_eq!(library::list(&d).len(), 0);
}

/// The keyboard alone: Tab into the list, Enter opens the run, C compares another with it, F2 renames,
/// Delete and Enter delete.
#[test]
fn the_keyboard_opens_compares_renames_and_deletes() {
    let sc = Scratch::new("runs-keys");
    let mut s = studio(&sc);
    enter(&mut s);
    let sm = sim(&s);
    let key = |k: egui::Key| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let step = |s: &mut Studio, ev: Vec<egui::Event>| {
        s.frame(raw_input(SIZE, 1.0, 0, ev));
        frames(s, SIZE, 1.0, 2);
    };
    // The first row takes the focus (as Tab from the view's controls would).
    s.ctx.memory_mut(|m| m.request_focus(egui::Id::new(("runs.row", s.runs.entries()[0].name.clone()))));
    step(&mut s, vec![]);
    let first = s.runs.entries()[0].name.clone();
    step(&mut s, vec![key(egui::Key::Enter)]);
    assert_eq!(sm.borrow().shown, first, "Enter opens the row");
    // Down to the other row, C compares it.
    step(&mut s, vec![key(egui::Key::ArrowDown)]);
    let second = s.runs.entries()[1].name.clone();
    step(&mut s, vec![key(egui::Key::C)]);
    assert_eq!(sm.borrow().ghost_name, second);
    // F2 renames it, Enter takes the name.
    step(&mut s, vec![key(egui::Key::F2)]);
    assert!(s.runs.rename.is_some());
    s.runs.rename.as_mut().unwrap().1 = "other".into();
    step(&mut s, vec![key(egui::Key::Enter)]);
    assert!(library::path(&dir(&s), "other").is_file(), "{:?}", s.runs.note);
    // Delete asks, Esc keeps, Delete and Enter delete.
    s.ctx.memory_mut(|m| m.request_focus(egui::Id::new(("runs.row", "other"))));
    step(&mut s, vec![]);
    step(&mut s, vec![key(egui::Key::Delete)]);
    assert_eq!(s.runs.confirm.as_deref(), Some("other"));
    step(&mut s, vec![key(egui::Key::Escape)]);
    assert!(library::path(&dir(&s), "other").is_file() && s.runs.confirm.is_none());
    step(&mut s, vec![key(egui::Key::Delete)]);
    step(&mut s, vec![key(egui::Key::Enter)]);
    assert!(!library::path(&dir(&s), "other").exists(), "{:?}", s.runs.note);
}

/// The pad: LT / RT to the list, the D-pad over the runs, A opens, X's menu compares.
#[test]
fn the_pad_moves_over_the_list_and_opens() {
    use super::nav::{Area, Pad};
    let sc = Scratch::new("runs-pad");
    let mut s = studio(&sc);
    s.nav.input = InputKind::Xbox;
    enter(&mut s);
    s.nav.area = Area::Left;
    let sm = sim(&s);
    let go = |s: &mut Studio, b: Pad| {
        s.pad(b, InputKind::Xbox);
        frames(s, SIZE, 1.0, 2);
    };
    go(&mut s, Pad::Down);
    let first = s.runs.entries()[0].name.clone();
    assert_eq!(s.runs.cursor.as_deref(), Some(first.as_str()));
    go(&mut s, Pad::Down);
    let second = s.runs.entries()[1].name.clone();
    assert_eq!(s.runs.cursor.as_deref(), Some(second.as_str()));
    go(&mut s, Pad::A);
    assert_eq!(sm.borrow().shown, second);
    go(&mut s, Pad::Up);
    s.runs.key = Some(egui::Key::C);
    frames(&mut s, SIZE, 1.0, 2);
    assert_eq!(sm.borrow().ghost_name, first, "the menu's Compare takes the row the cursor is on");
}

/// The compared run plays on the same race clock at any display rate.
#[test]
fn the_ghost_is_where_the_clock_says_at_any_display_rate() {
    let sc = Scratch::new("runs-rates");
    let mut s = studio(&sc);
    enter(&mut s);
    let d = dir(&s);
    let (sm, ctx) = (sim(&s), s.ctx.clone());
    s.runs.open(&d, &sm, &ctx, "seed1");
    frames(&mut s, SIZE, 1.0, 3);
    s.runs.compare(&d, &sm, "seed2");
    let mut seen = Vec::new();
    for fps in [30u32, 60, 120, 200] {
        let mut b = sm.borrow_mut();
        let rec = b.rec.clone();
        b.play.seek(&rec, 0.0);
        b.play.speed = 2;
        if !b.play.playing {
            b.play.toggle(&rec);
        }
        for _ in 0..fps * 10 {
            b.play.tick(&rec, 1.0 / fps as f32);
        }
        seen.push((b.play.time, b.ghost_cars().iter().map(|c| (c.id, c.position)).collect::<Vec<_>>()));
    }
    for w in seen.windows(2) {
        assert!((w[0].0 - w[1].0).abs() < 1e-4, "{} vs {}", w[0].0, w[1].0);
        for (a, b) in w[0].1.iter().zip(&w[1].1) {
            assert!(a.1.distance(b.1) < 0.05, "ghost {} at {} vs {}", a.0, a.1, b.1);
        }
    }
}

/// With a run open and another compared, at the review sizes: nothing clipped or overlapped, the graphs and the
/// list inside their panels.
#[test]
fn it_draws_at_the_review_sizes() {
    for (w, h, sys) in [(960, 600, 1.0), (1280, 800, 1.0), (1920, 1080, 1.0), (3840, 2160, 2.0)] {
        for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
            let sc = Scratch::new(&format!("runs-draw-{w}x{h}-{input:?}"));
            let mut s = studio(&sc);
            s.nav.workspace = Workspace::Runs;
            s.nav.input = input;
            let scale = sys * layout::ui_scale((w as f32 / sys) as u32, (h as f32 / sys) as u32);
            frames(&mut s, (w, h), scale, 3);
            let d = dir(&s);
            let (sm, ctx) = (sim(&s), s.ctx.clone());
            s.runs.open(&d, &sm, &ctx, "seed1");
            frames(&mut s, (w, h), scale, 3);
            s.runs.compare(&d, &sm, "seed2");
            {
                let mut b = sm.borrow_mut();
                let rec = b.rec.clone();
                b.play.seek_frame(&rec, 31.0);
            }
            let (_, l) = frames(&mut s, (w, h), scale, 3);
            assert!(l.problems().is_empty(), "{w}x{h}@{sys} {input:?}: {:?}", l.problems());
            assert!(!s.runs.rows.is_empty(), "{w}x{h}: the list drew");
            if let Some(left) = l.left {
                for r in &s.runs.rows {
                    assert!(left.expand(1.0).contains_rect(*r), "{w}x{h}: row {r:?} outside {left:?}");
                }
            }
            if let Some(bottom) = l.bottom.filter(|_| matches!(l.bottom_dock, layout::Dock::Open(_))) {
                let g = graphs::drawn(&s.ctx);
                assert!(!g.is_empty(), "{w}x{h}: the graphs drew");
                for r in g {
                    assert!(
                        r.min.x >= bottom.min.x - 1.0 && r.max.x <= bottom.max.x + 1.0,
                        "{w}x{h}: graph {r:?} outside {bottom:?}"
                    );
                }
            }
        }
    }
}

/// A run recorded with the drive channels shows them as graphs beside speed and the racing line, inside the dock,
/// and the same run's tick-hash compare names the tick the runs parted at.
#[test]
fn a_recorded_drive_shows_gear_and_inputs_and_compares_by_tick() {
    use ob_studio::runs::fixtures::driven;
    for (w, h, sys) in [(960, 600, 1.0), (1920, 1080, 1.0)] {
        let sc = Scratch::new(&format!("runs-drive-{w}x{h}"));
        let mut s = studio(&sc);
        let d = dir(&s);
        for (name, seed, at) in [("drive1", 1, 70), ("drive2", 2, 70)] {
            let text: String = driven(seed, at).iter().map(|l| l.write() + "\n").collect();
            std::fs::write(library::path(&d, name), text).unwrap();
        }
        s.runs.changed();
        s.nav.workspace = Workspace::Runs;
        let scale = sys * layout::ui_scale((w as f32 / sys) as u32, (h as f32 / sys) as u32);
        frames(&mut s, (w, h), scale, 3);
        let (sm, ctx) = (sim(&s), s.ctx.clone());
        s.runs.open(&d, &sm, &ctx, "drive1");
        s.runs.compare(&d, &sm, "drive2");
        let (_, l) = frames(&mut s, (w, h), scale, 3);
        assert!(l.problems().is_empty(), "{w}x{h}: {:?}", l.problems());
        {
            let b = sm.borrow();
            let t = telemetry::of(&b.rec, 0, None);
            assert_eq!(graphs::Channel::of(&t).len(), 6, "speed, line, gear, throttle, brake, steering");
            let p = telemetry::parting(&b.rec, b.ghost.as_ref().unwrap(), 0);
            assert_eq!(p.hash_tick, Some(70), "the tick itself");
        }
        if let Some(bottom) = l.bottom.filter(|_| matches!(l.bottom_dock, layout::Dock::Open(_))) {
            let g = graphs::drawn(&s.ctx);
            assert_eq!(g.len(), 6, "{w}x{h}: every channel drew");
            for r in g {
                assert!(r.min.x >= bottom.min.x - 1.0 && r.max.x <= bottom.max.x + 1.0, "{w}x{h}: {r:?} outside {bottom:?}");
            }
        }
    }
}
