//! The shell drawn headless (egui without a GPU): every common window size and display scale, every workspace,
//! mouse / keyboard / pad, with the clip and overlap check; walkthroughs by keyboard and by pad.

use std::path::PathBuf;

use ob_look::legends::InputKind;

use super::Studio;
use super::address::Address;
use super::layout::{self, Dock};
use super::nav::{Area, Pad};
use super::shot::{frames, raw_input};
use super::workspace::Workspace;
use crate::tools::map::store;

fn maps(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-shell-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (folder, title) in [("harbour-edits", "Harbour edits"), ("drift-park", "Drift park")] {
        std::fs::create_dir_all(d.join(folder)).unwrap();
        std::fs::write(d.join(folder).join(store::FILE), format!("overbright-map 1\nname = {title}\n")).unwrap();
    }
    d
}

/// The resolution matrix (the launcher's and the map editor's shot checks) and high-density scales.
const SIZES: [(u32, u32, f32); 28] = [
    (640, 480, 1.0),
    (800, 600, 1.0),
    (1024, 768, 1.0),
    (1280, 960, 1.0),
    (1600, 1200, 1.0),
    (1280, 1024, 1.0),
    (1280, 800, 1.0),
    (1440, 900, 1.0),
    (1680, 1050, 1.0),
    (1920, 1200, 1.0),
    (2560, 1600, 1.0),
    (1280, 720, 1.0),
    (1366, 768, 1.0),
    (1600, 900, 1.0),
    (1920, 1080, 1.0),
    (2560, 1440, 1.0),
    (3840, 2160, 1.0),
    (2160, 1440, 1.0),
    (2256, 1504, 1.0),
    (2560, 1080, 1.0),
    (3440, 1440, 1.0),
    (3840, 1600, 1.0),
    (3840, 1080, 1.0),
    (5120, 1440, 1.0),
    (1080, 1920, 1.0),
    // A display scale of 1.5 and 2 on top of Studio's own.
    (2560, 1440, 1.5),
    (3840, 2160, 2.0),
    (2880, 1800, 2.0),
];

#[test]
fn every_size_draws_without_clips_or_overlaps() {
    let dir = maps("matrix");
    let mut drawn = 0;
    for (w, h, sys) in SIZES {
        let scale = sys * layout::ui_scale((w as f32 / sys) as u32, (h as f32 / sys) as u32);
        for ws in Workspace::ALL {
            for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
                let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
                s.nav.workspace = ws;
                s.nav.input = input;
                let (_, l) = frames(&mut s, (w, h), scale, 2);
                let small = (w as f32 / scale) < layout::WINDOW_MIN.0 || (h as f32 / scale) < layout::WINDOW_MIN.1;
                assert_eq!(l.too_small, small, "{w}x{h}@{sys}");
                if !small {
                    assert_eq!(l.legend.is_some(), input != InputKind::Mouse, "the legend row follows the input");
                }
                assert!(l.problems().is_empty(), "{w}x{h}@{sys} {ws:?} {input:?}: {:?}", l.problems());
                if !small {
                    drawn += 1;
                    // The address keeps at least its smallest width beside the drawn menus.
                    assert!(l.menu.address.width() >= layout::ADDRESS_MIN);
                }
            }
        }
    }
    assert!(drawn >= 300, "{drawn}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn under_the_smallest_window_one_line() {
    let dir = maps("small");
    let mut s = Studio::new(dir.clone(), None);
    let (out, l) = frames(&mut s, (800, 600), 1.0, 2);
    assert!(l.too_small);
    let texts: usize = out.shapes.iter().filter(|c| matches!(c.shape, egui::Shape::Text(_))).count();
    assert_eq!(texts, 1, "only the one line");
    let _ = std::fs::remove_dir_all(&dir);
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

fn press(s: &mut Studio, size: (u32, u32), events: Vec<egui::Event>) {
    s.frame(raw_input(size, 1.0, 0, events));
}

#[test]
fn keyboard_walkthrough() {
    let dir = maps("keys");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    frames(&mut s, size, 1.0, 1);
    assert!(!s.nav.legend(), "the mouse is assumed at the start: no legend row");
    // Ctrl+PgDn to World; F6 shows the legend row and moves the focus.
    press(&mut s, size, vec![key(egui::Key::PageDown, true, false)]);
    assert_eq!(s.nav.workspace, Workspace::World);
    assert_eq!(s.address().to_string(), "ob://harbour-edits/@world");
    press(&mut s, size, vec![key(egui::Key::F6, false, false)]);
    assert!(s.nav.legend());
    let l = s.last.clone().unwrap();
    assert!(l.legend.is_some());
    // The Deck size: the tree is folded, so F6 goes from the view to the right dock.
    assert_eq!(l.left_dock, Dock::Folded);
    assert_eq!(s.nav.area, Area::Right);
    press(&mut s, size, vec![key(egui::Key::F6, false, true)]);
    assert_eq!(s.nav.area, Area::View);
    // Ctrl+L, a typed address, Enter: Home.
    press(&mut s, size, vec![key(egui::Key::L, true, false)]);
    assert!(s.nav.address_text);
    s.address_edit = "ob://studio/@home".into();
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert_eq!(s.nav.workspace, Workspace::Home);
    assert!(!s.nav.address_text);
    // Alt+← back to World.
    press(
        &mut s,
        size,
        vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers { alt: true, ..Default::default() },
        }],
    );
    assert_eq!(s.nav.workspace, Workspace::World);
    // Ctrl+K, Esc.
    press(&mut s, size, vec![key(egui::Key::K, true, false)]);
    assert!(s.nav.find_open);
    press(&mut s, size, vec![key(egui::Key::Escape, false, false)]);
    assert!(!s.nav.find_open);
    // A click hides the legend row again.
    press(
        &mut s,
        size,
        vec![egui::Event::PointerButton {
            pos: egui::pos2(600.0, 400.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    assert!(!s.nav.legend());
    // An address Studio cannot follow is not followed silently: a verdict, closed by Esc.
    press(&mut s, size, vec![key(egui::Key::L, true, false)]);
    s.address_edit = "ob://nowhere/@world".into();
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert!(s.verdict.as_deref().is_some_and(|v| v.contains("nowhere")), "{:?}", s.verdict);
    assert_eq!(s.nav.workspace, Workspace::World);
    press(&mut s, size, vec![key(egui::Key::Escape, false, false)]);
    assert!(s.verdict.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pad_walkthrough_matches_the_keyboard() {
    let dir = maps("pad");
    let size = (1024, 768);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    frames(&mut s, size, 1.0, 1);
    s.pad(Pad::RB, InputKind::Deck);
    assert_eq!(s.nav.workspace, Workspace::World);
    frames(&mut s, size, 1.0, 1);
    // 1024 × 768: the Inspector is a drawer; RT opens it, B closes it.
    s.pad(Pad::RT, InputKind::Deck);
    assert_eq!(s.nav.area, Area::Right);
    assert!(s.nav.drawer_open);
    frames(&mut s, size, 1.0, 1);
    assert!(s.last.as_ref().unwrap().drawer.is_some(), "the drawer is drawn");
    s.pad(Pad::B, InputKind::Deck);
    assert!(!s.nav.drawer_open);
    // B at the area level never leaves the workspace.
    s.pad(Pad::B, InputKind::Deck);
    assert_eq!(s.nav.workspace, Workspace::World);
    // View: the last place; LB back the other way.
    s.pad(Pad::View, InputKind::Deck);
    assert_eq!(s.nav.workspace, Workspace::Home);
    s.pad(Pad::LB, InputKind::Deck);
    assert_eq!(s.nav.workspace, Workspace::Settings);
    assert_eq!(s.nav.input, InputKind::Deck);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn addresses_go_to_screens_and_refuse_what_is_not_there() {
    let dir = maps("addr");
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    for w in Workspace::ALL {
        let a = Address::view("harbour-edits", w.view());
        s.go(&a).unwrap();
        assert_eq!(s.nav.workspace, w);
        // The window's own address reads back to the same screen.
        let back = Address::parse(&s.address().to_string()).unwrap();
        s.go(&back).unwrap();
        assert_eq!(s.nav.workspace, w);
    }
    // Another of the player's mods opens it.
    s.go(&Address::parse("ob://drift-park/@world").unwrap()).unwrap();
    assert_eq!(s.open_mod().map(|m| m.folder.as_str()), Some("drift-park"));
    assert!(s.go(&Address::parse("ob://nowhere/@world").unwrap()).is_err());
    assert!(s.go(&Address::parse("ob://studio/@changes").unwrap()).is_err());
    s.go(&Address::parse("ob://drift-park/race/drift-park:sprint").unwrap()).unwrap();
    assert_eq!(s.nav.workspace, Workspace::Races);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn panel_folds_are_kept_per_workspace_in_the_mod() {
    let dir = maps("memory");
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.nav.workspace = Workspace::World;
    let mut m = s.memories.get(Workspace::World);
    m.bottom_folded = Some(true);
    s.memories.set(Workspace::World, m);
    s.memories.save(&dir.join("harbour-edits")).unwrap();
    let s2 = Studio::new(dir.clone(), Some("harbour-edits"));
    assert_eq!(s2.memories.get(Workspace::World).bottom_folded, Some(true));
    assert_eq!(s2.memories.get(Workspace::Races).bottom_folded, None);
    let _ = std::fs::remove_dir_all(&dir);
}
