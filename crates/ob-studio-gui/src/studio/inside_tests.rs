//! Inside an area (focus, Find, the places, the docks' edges, the status bar's jobs and counts), headless.

use std::path::PathBuf;

use ob_look::legends::InputKind;

use super::Studio;
use super::focus::{RightTab, Spot};
use super::inside::FindItem;
use super::layout::{self, Dock};
use super::memory::Memories;
use super::nav::{self, Area, Pad};
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::tools::map::store;

fn maps(name: &str, mods: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-inside-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (folder, title) in mods {
        std::fs::create_dir_all(d.join(folder)).unwrap();
        std::fs::write(d.join(folder).join(store::FILE), format!("overbright-map 1\nname = {title}\n")).unwrap();
    }
    d
}

fn two(name: &str) -> PathBuf {
    maps(name, &[("harbour-edits", "Harbour edits"), ("drift-park", "Drift park")])
}

fn key(k: egui::Key, ctrl: bool, shift: bool) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers { ctrl, command: ctrl, shift, ..Default::default() },
    }
}

fn press(s: &mut Studio, size: (u32, u32), events: Vec<egui::Event>) -> egui::FullOutput {
    s.frame(raw_input(size, 1.0, 0, events))
}

fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn walk(s: &egui::Shape, v: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => v.push(t.galley.text().to_string()),
            egui::Shape::Vec(list) => list.iter().for_each(|s| walk(s, v)),
            _ => {}
        }
    }
    let mut v = vec![];
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

/// Where the keys are: the area and its focused control (None on a page's rows: Home's focus then).
fn at(s: &Studio) -> (Area, Option<Spot>, usize) {
    (s.nav.area, s.spots.focused(s.nav.area), s.home_focus)
}

/// Tab from the rail's first control all the way round: every control of every area is reached, in order, and the
/// walk comes back to where it began.
fn tab_round(ws: Workspace, size: (u32, u32)) {
    let dir = two(&format!("tab-{}-{}", ws.name(), size.0));
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.workspace(ws);
    frames(&mut s, size, 1.0, 2);
    press(&mut s, size, vec![key(egui::Key::F6, false, false)]);
    s.nav.area = Area::Rail;
    s.spots.set(Area::Rail, 0);
    frames(&mut s, size, 1.0, 1);
    let l = s.last.clone().unwrap();
    let areas = nav::shown(&l);
    let mut want: Vec<(Area, Option<Spot>)> = vec![];
    for a in &areas {
        if *a == Area::View && ws.kind() == super::workspace::Kind::Page {
            want.extend((0..s.home_targets().len()).map(|_| (Area::View, None)));
        } else {
            want.extend(s.spots.of(*a).into_iter().map(|x| (*a, Some(x))));
        }
    }
    assert!(want.len() > 6, "{want:?}");
    let start = (s.nav.area, s.spots.focused(s.nav.area));
    let mut seen = vec![];
    for _ in 0..want.len() {
        let (a, spot, _) = at(&s);
        seen.push((a, if a == Area::View && ws.kind() == super::workspace::Kind::Page { None } else { spot }));
        press(&mut s, size, vec![key(egui::Key::Tab, false, false)]);
        assert_eq!(s.nav.workspace, ws, "Tab never takes a control");
    }
    assert_eq!(seen, want, "{} at {size:?}", ws.name());
    assert_eq!((s.nav.area, s.spots.focused(s.nav.area)), start, "round again");
    // Shift+Tab goes back the same way.
    press(&mut s, size, vec![key(egui::Key::Tab, false, true)]);
    assert_eq!((s.nav.area, s.spots.focused(s.nav.area)), *want.last().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_reaches_every_control_of_every_area() {
    for size in [(1920, 1080), (1280, 800), (1024, 768)] {
        tab_round(Workspace::World, size);
        tab_round(Workspace::Home, size);
    }
}

#[test]
fn the_arrows_and_the_d_pad_reach_every_row_of_each_area() {
    let dir = two("rows");
    for (size, input) in
        [((1920, 1080), None), ((1280, 800), Some(InputKind::Deck)), ((1024, 768), Some(InputKind::Xbox))]
    {
        let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
        s.workspace(Workspace::World);
        frames(&mut s, size, 1.0, 2);
        let l = s.last.clone().unwrap();
        for a in nav::shown(&l).into_iter().filter(|a| *a != Area::View) {
            // To the area by F6 / RT, then up to its first control.
            while s.nav.area != a {
                match input {
                    Some(k) => s.pad(Pad::RT, k),
                    None => drop(press(&mut s, size, vec![key(egui::Key::F6, false, false)])),
                }
                frames(&mut s, size, 1.0, 1);
            }
            let all = s.spots.of(a);
            assert!(!all.is_empty(), "{a:?} at {size:?}");
            for _ in 0..all.len() {
                match input {
                    Some(k) => s.pad(Pad::Up, k),
                    None => drop(press(&mut s, size, vec![key(egui::Key::ArrowUp, false, false)])),
                }
            }
            let mut seen = vec![s.spots.focused(a).unwrap()];
            for _ in 1..all.len() + 2 {
                match input {
                    Some(k) => s.pad(Pad::Down, k),
                    None => drop(press(&mut s, size, vec![key(egui::Key::ArrowDown, false, false)])),
                }
                let f = s.spots.focused(a).unwrap();
                if seen.last() != Some(&f) {
                    seen.push(f);
                }
                assert_eq!(s.nav.area, a, "the arrows stay inside the area");
            }
            assert_eq!(seen, all, "{a:?} at {size:?}");
            // The ring is on the focused control, inside the window.
            let (out, _) = frames(&mut s, size, 1.0, 1);
            let r = s.spots.focused_rect(a).unwrap();
            assert!(r.width() > 0.0 && r.right() <= l.size.x && r.bottom() <= l.size.y, "{a:?} {r:?}");
            let _ = out;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enter_and_a_take_the_focused_control() {
    let dir = two("take");
    let size = (1920, 1080);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.workspace(Workspace::World);
    frames(&mut s, size, 1.0, 2);
    // The right dock: → on its tabs changes the tab.
    press(&mut s, size, vec![key(egui::Key::F6, false, false)]);
    s.nav.area = Area::Right;
    s.spots.set(Area::Right, 0);
    frames(&mut s, size, 1.0, 1);
    press(&mut s, size, vec![key(egui::Key::ArrowRight, false, false)]);
    assert_eq!(s.right_tab, RightTab::History);
    s.pad(Pad::Right, InputKind::Xbox);
    assert_eq!(s.right_tab, RightTab::Help);
    // The bottom dock's Jobs tab by D-pad and A.
    s.nav.area = Area::Bottom;
    frames(&mut s, size, 1.0, 1);
    assert!(s.spots.focus(Area::Bottom, Spot::BottomTab(2)));
    s.pad(Pad::A, InputKind::Xbox);
    assert_eq!(s.bottom_tab, super::shell::BottomTab::Jobs);
    // The rail: Enter on Changes goes there.
    s.nav.area = Area::Rail;
    frames(&mut s, size, 1.0, 1);
    assert!(s.spots.focus(Area::Rail, Spot::Rail(Workspace::Mod)));
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert_eq!(s.nav.workspace, Workspace::Mod);
    // The address bar's ← by A: back to World.
    s.nav.area = Area::Address;
    frames(&mut s, size, 1.0, 1);
    assert!(s.spots.focus(Area::Address, Spot::Back));
    s.pad(Pad::A, InputKind::Xbox);
    assert_eq!(s.nav.workspace, Workspace::World);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn find_runs_commands_and_goes_to_places() {
    let dir = two("find");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    frames(&mut s, size, 1.0, 2);
    // Ctrl+K, "new mod", Enter: the command runs (the New mod dialog).
    press(&mut s, size, vec![key(egui::Key::K, true, false)]);
    assert!(s.nav.find_open);
    frames(&mut s, size, 1.0, 1);
    press(&mut s, size, vec![egui::Event::Text("new mod".into())]);
    assert_eq!(s.find_items(), [FindItem::Run(nav::Command::NewMod)]);
    let out = press(&mut s, size, vec![]);
    assert!(texts(&out).iter().any(|t| t == "Ctrl+N"), "the command's keys beside it");
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert!(!s.nav.find_open);
    assert!(s.new_mod.is_some(), "Find ran New mod");
    s.new_mod = None;
    // Places you were: World, then Mod; Find lists them by name, Enter on one goes there.
    s.workspace(Workspace::World);
    s.workspace(Workspace::Mod);
    frames(&mut s, size, 1.0, 1);
    press(&mut s, size, vec![key(egui::Key::K, true, false)]);
    let items = s.find_items();
    let k = items.iter().position(|i| *i == FindItem::Place(1)).unwrap_or_else(|| panic!("{items:?}"));
    let out = press(&mut s, size, vec![]);
    assert!(texts(&out).iter().any(|t| t == "Harbour edits › World"), "{:?}", texts(&out));
    for _ in 0..k {
        press(&mut s, size, vec![key(egui::Key::ArrowDown, false, false)]);
    }
    assert_eq!(s.find_focus, k);
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert_eq!(s.nav.workspace, Workspace::World);
    // By pad: Y, the D-pad to Changes (its command, Ctrl+D), A.
    s.pad(Pad::Y, InputKind::Xbox);
    assert!(s.nav.find_open);
    let k =
        s.find_items().iter().position(|i| *i == FindItem::Run(nav::Command::Workspace(Workspace::Changes))).unwrap();
    for _ in 0..k {
        s.pad(Pad::Down, InputKind::Xbox);
    }
    s.pad(Pad::A, InputKind::Xbox);
    assert!(!s.nav.find_open);
    assert_eq!(s.nav.workspace, Workspace::Changes);
    // A click on a command runs it too.
    assert!(s.find_items().is_empty() || !s.nav.find_open);
    s.find_run(FindItem::Run(nav::Command::EditAddress));
    assert!(s.nav.address_text);
    let _ = std::fs::remove_dir_all(&dir);
}

fn mouse(b: egui::PointerButton, at: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: at, button: b, pressed, modifiers: Default::default() }
}

#[test]
fn mouse_buttons_4_and_5_and_the_places_under_back() {
    let dir = two("places");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    frames(&mut s, size, 1.0, 1);
    s.workspace(Workspace::World);
    s.workspace(Workspace::Mod);
    frames(&mut s, size, 1.0, 1);
    let p = egui::pos2(600.0, 400.0);
    press(
        &mut s,
        size,
        vec![mouse(egui::PointerButton::Extra1, p, true), mouse(egui::PointerButton::Extra1, p, false)],
    );
    assert_eq!(s.nav.workspace, Workspace::World, "button 4: back");
    press(
        &mut s,
        size,
        vec![mouse(egui::PointerButton::Extra1, p, true), mouse(egui::PointerButton::Extra1, p, false)],
    );
    assert_eq!(s.nav.workspace, Workspace::Home);
    press(
        &mut s,
        size,
        vec![mouse(egui::PointerButton::Extra2, p, true), mouse(egui::PointerButton::Extra2, p, false)],
    );
    press(
        &mut s,
        size,
        vec![mouse(egui::PointerButton::Extra2, p, true), mouse(egui::PointerButton::Extra2, p, false)],
    );
    assert_eq!(s.nav.workspace, Workspace::Mod, "button 5: forward");
    assert!(!s.nav.legend(), "the mouse stays the input");
    // A right-click on ←: the places, newest first, by name; one taken goes back to it.
    let back = s.last.as_ref().unwrap().menu.back_fwd.left_center() + egui::vec2(12.0, 0.0);
    press(&mut s, size, vec![egui::Event::PointerMoved(back)]);
    press(&mut s, size, vec![mouse(egui::PointerButton::Secondary, back, true)]);
    press(&mut s, size, vec![mouse(egui::PointerButton::Secondary, back, false)]);
    assert!(s.places_menu);
    let out = press(&mut s, size, vec![]);
    let t = texts(&out);
    let w = t.iter().position(|x| x == "Harbour edits › World").expect("World listed");
    let h = t.iter().position(|x| x == "Overbright Studio › Home").expect("Home listed");
    assert!(w < h, "newest first: {t:?}");
    press(&mut s, size, vec![key(egui::Key::Escape, false, false)]);
    assert!(!s.places_menu, "Esc closes it");
    assert_eq!(s.nav.workspace, Workspace::Mod);
    s.go_back(2);
    assert_eq!(s.nav.workspace, Workspace::Home);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dragging_a_docks_edge_sizes_it_and_the_size_is_kept() {
    let dir = two("drag");
    let size = (1920, 1080);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.workspace(Workspace::World);
    let (_, l) = frames(&mut s, size, 1.0, 2);
    let drag = |s: &mut Studio, from: egui::Pos2, by: egui::Vec2| {
        press(s, size, vec![egui::Event::PointerMoved(from)]);
        press(s, size, vec![mouse(egui::PointerButton::Primary, from, true)]);
        press(s, size, vec![egui::Event::PointerMoved(from + by * 0.5)]);
        press(s, size, vec![egui::Event::PointerMoved(from + by)]);
        press(s, size, vec![mouse(egui::PointerButton::Primary, from + by, false)]);
        frames(s, size, 1.0, 1).1
    };
    let (Dock::Open(lw), Dock::Open(rw), Dock::Open(bh)) = (l.left_dock, l.right_dock, l.bottom_dock) else {
        panic!("all docked at 1080p: {l:?}");
    };
    let left = l.left.unwrap();
    let l2 = drag(&mut s, egui::pos2(left.right(), left.center().y), egui::vec2(60.0, 0.0));
    let near = |d: Dock, w: f32| matches!(d, Dock::Open(x) if (x - w).abs() < 0.5);
    assert!(near(l2.left_dock, lw + 60.0), "{:?}", l2.left_dock);
    let right = l2.right.unwrap();
    let l3 = drag(&mut s, egui::pos2(right.left(), right.center().y), egui::vec2(-40.0, 0.0));
    assert!(near(l3.right_dock, rw + 40.0), "{:?}", l3.right_dock);
    let bottom = l3.bottom.unwrap();
    let l4 = drag(&mut s, egui::pos2(bottom.center().x, bottom.top()), egui::vec2(0.0, -30.0));
    assert!(near(l4.bottom_dock, (bh + 30.0).min(layout::BOTTOM_MAX)), "{:?}", l4.bottom_dock);
    assert!(l4.problems().is_empty(), "{:?}", l4.problems());
    // Never past the rule's range.
    let left = l4.left.unwrap();
    let l5 = drag(&mut s, egui::pos2(left.right(), left.center().y), egui::vec2(-400.0, 0.0));
    assert_eq!(l5.left_dock, Dock::Open(layout::LEFT_MIN));
    // Kept with the mod, per workspace: a new window has them; Races keeps its own.
    let m = Memories::load(&dir.join("harbour-edits"));
    let w = m.get(Workspace::World);
    assert_eq!(w.left_w, Some(layout::LEFT_MIN));
    assert!((w.right_w.unwrap() - (rw + 40.0)).abs() < 0.5);
    assert_eq!(m.get(Workspace::Races).left_w, None);
    let mut s2 = Studio::new(dir.clone(), Some("harbour-edits"));
    s2.workspace(Workspace::World);
    let (_, l6) = frames(&mut s2, size, 1.0, 2);
    assert!(near(l6.right_dock, rw + 40.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_status_bar_shows_the_running_job_and_what_the_checks_found() {
    let dir = maps("status", &[("harbour-edits", "Harbour edits"), ("no-name", "")]);
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    let (out, _) = frames(&mut s, size, 1.0, 2);
    assert!(texts(&out).iter().any(|t| t == "Problems 0"), "{:?}", texts(&out));
    assert!(s.jobs().is_empty());
    // A test runs (the map editor's Test race says so): the busy line, Home's Jobs row, the Jobs tab.
    crate::editor::testplay::set_strip(&s.ctx, Some("You are racing Casino sprint".into()));
    let (out, _) = frames(&mut s, size, 1.0, 1);
    let n = texts(&out).iter().filter(|t| *t == "You are racing Casino sprint").count();
    assert_eq!(n, 2, "the status bar and Home: {:?}", texts(&out));
    s.workspace(Workspace::World);
    s.bottom_tab = super::shell::BottomTab::Jobs;
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert_eq!(texts(&out).iter().filter(|t| *t == "You are racing Casino sprint").count(), 2, "bar and tab");
    crate::editor::testplay::set_strip(&s.ctx, None);
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(texts(&out).iter().any(|t| t == super::words::NO_JOBS));
    // A mod without a name: the Mod page's ready list has a problem; the bar counts it, Problems lists it.
    let mut s = Studio::new(dir.clone(), Some("no-name"));
    assert_eq!(s.open_mod().map(|m| m.folder.as_str()), Some("no-name"));
    assert_eq!(s.counts(), Some(super::jobs::Counts { problems: 1, warnings: 0 }));
    s.workspace(Workspace::World);
    s.bottom_tab = super::shell::BottomTab::Problems;
    let (out, _) = frames(&mut s, size, 1.0, 2);
    let t = texts(&out);
    assert!(t.iter().any(|t| t == "Problems 1"), "{t:?}");
    assert!(t.iter().any(|t| t == "Give the mod a name"), "{t:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
