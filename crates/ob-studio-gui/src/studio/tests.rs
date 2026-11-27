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

fn text(t: &str) -> egui::Event {
    egui::Event::Text(t.into())
}

#[test]
fn a_new_mod_from_home_by_keyboard_opens_ready() {
    let dir = maps("new-keys");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), None);
    frames(&mut s, size, 1.0, 2);
    // Home with no mod open: Open a mod, New mod…, the places, the tutorials, the two mods, Empty map mod, the
    // templates.
    let targets = s.home_targets();
    let empty = targets.iter().rposition(|a| *a == super::shell::HomeAct::NewMod).unwrap();
    assert!(targets[empty + 1..].iter().all(|a| matches!(a, super::shell::HomeAct::Template(_))), "{targets:?}");
    for _ in 0..empty {
        press(&mut s, size, vec![key(egui::Key::ArrowDown, false, false)]);
    }
    assert_eq!(s.home_focus, empty, "Empty map mod");
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert!(s.new_mod.is_some(), "Enter on Empty map mod opens the dialog");
    // The name field has the keys: typing reaches it.
    frames(&mut s, size, 1.0, 1);
    press(&mut s, size, vec![text("Casino sprints")]);
    assert_eq!(s.new_mod.as_ref().unwrap().name, "Casino sprints");
    press(&mut s, size, vec![key(egui::Key::Enter, false, false)]);
    assert!(s.new_mod.is_none(), "made and closed");
    assert_eq!(s.open_mod().map(|m| m.folder.as_str()), Some("casino-sprints"));
    assert_eq!(s.open_mod().map(|m| m.name.as_str()), Some("Casino sprints"));
    assert_eq!(s.nav.workspace, Workspace::Home);
    assert!(dir.join("casino-sprints").join(store::FILE).is_file());
    // Home says it is ready.
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t == super::words::READY_TITLE));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_mod_by_ctrl_n_and_by_pad_refuses_clashes_and_cancels() {
    let dir = maps("new-pad");
    let size = (1024, 768);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.nav.workspace = Workspace::World;
    frames(&mut s, size, 1.0, 1);
    // Ctrl+N from any workspace (World: the map editor never sees it).
    press(&mut s, size, vec![key(egui::Key::N, true, false)]);
    assert!(s.new_mod.is_some());
    // A name another mod has: refused, the dialog stays with the reason.
    s.new_mod.as_mut().unwrap().name = "Harbour edits".into();
    s.pad(Pad::A, InputKind::Xbox);
    let d = s.new_mod.as_ref().expect("refused: the dialog stays");
    assert!(d.tried);
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t.contains("folder harbour-edits is in use")), "{:?}", shapes_text(&out));
    // The same name in another folder: refused as a name in use.
    s.new_mod.as_mut().unwrap().folder = "harbour-2".into();
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t.contains("mod with this name already")));
    // A bad folder name.
    s.new_mod.as_mut().unwrap().folder = "Bad Folder".into();
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t.contains("is not a folder name")));
    // D-pad down to Cancel, A: closed, nothing made.
    s.pad(Pad::Down, InputKind::Xbox);
    s.pad(Pad::Down, InputKind::Xbox);
    s.pad(Pad::Down, InputKind::Xbox);
    assert_eq!(s.new_mod.as_ref().unwrap().part(), super::new_mod::Part::Cancel);
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.new_mod.is_none());
    assert_eq!(store::list(&dir), ["drift-park", "harbour-edits"]);
    // Again with a new name, B cancels; then made with A.
    s.open_new_mod();
    s.pad(Pad::B, InputKind::Xbox);
    assert!(s.new_mod.is_none());
    s.open_new_mod();
    s.new_mod.as_mut().unwrap().name = "Night run".into();
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.new_mod.is_none());
    assert_eq!(s.open_mod().map(|m| m.folder.as_str()), Some("night-run"));
    assert_eq!(s.nav.workspace, Workspace::Home);
    // An empty name: said only after Create.
    s.open_new_mod();
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t == super::words::NEW_MOD_NAME_FIRST));
    s.pad(Pad::A, InputKind::Xbox);
    let (out, _) = frames(&mut s, size, 1.0, 1);
    assert!(shapes_text(&out).iter().any(|t| t == "Give the mod a name"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_new_mod_dialog_draws_at_every_size() {
    let dir = maps("new-sizes");
    for (w, h, sys) in SIZES {
        let scale = sys * layout::ui_scale((w as f32 / sys) as u32, (h as f32 / sys) as u32);
        for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Deck] {
            let mut s = Studio::new(dir.clone(), None);
            s.nav.input = input;
            s.open_new_mod();
            s.new_mod.as_mut().unwrap().name = "A rather long mod name to see it fit".into();
            let (_, l) = frames(&mut s, (w, h), scale, 2);
            assert!(l.problems().is_empty(), "{w}x{h}@{sys}: {:?}", l.problems());
            // The card (at most 560 × 328 points) fits in the smallest window Studio draws.
            assert!(l.too_small || (l.size.x >= 560.0 + 48.0 && l.size.y >= 328.0), "{w}x{h}@{sys}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every text drawn in a frame.
fn shapes_text(out: &egui::FullOutput) -> Vec<String> {
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

/// Where a text is drawn (its top-left corner), when it is.
fn text_at(out: &egui::FullOutput, want: &str) -> Option<egui::Pos2> {
    fn walk(s: &egui::Shape, want: &str) -> Option<egui::Pos2> {
        match s {
            egui::Shape::Text(t) if t.galley.text() == want => Some(t.pos + t.galley.rect.center().to_vec2()),
            egui::Shape::Vec(list) => list.iter().find_map(|s| walk(s, want)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|c| walk(&c.shape, want))
}

fn click(s: &mut Studio, size: (u32, u32), at: egui::Pos2) -> egui::FullOutput {
    let ev = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    s.frame(raw_input(size, 1.0, 0, vec![egui::Event::PointerMoved(at)]));
    s.frame(raw_input(size, 1.0, 0, vec![ev(true)]));
    s.frame(raw_input(size, 1.0, 0, vec![ev(false)]));
    s.frame(raw_input(size, 1.0, 0, vec![]))
}

#[test]
fn a_new_mod_from_the_file_menu_and_by_mouse() {
    let dir = maps("new-menu");
    let size = (1536, 864);
    let mut s = Studio::new(dir.clone(), Some("drift-park"));
    let (out, _) = frames(&mut s, size, 1.0, 2);
    let file = text_at(&out, "File").expect("the File menu");
    let out = click(&mut s, size, file);
    let item = text_at(&out, super::words::NEW_MOD).expect("File ▸ New mod…");
    click(&mut s, size, item);
    assert!(s.new_mod.is_some(), "File ▸ New mod… opens the dialog");
    // Cancel by mouse.
    let (out, _) = frames(&mut s, size, 1.0, 1);
    let cancel = text_at(&out, ob_look::words::CANCEL).expect("Cancel");
    click(&mut s, size, cancel);
    assert!(s.new_mod.is_none());
    // Home's Empty map mod by mouse, a name typed, Create clicked.
    s.nav.workspace = Workspace::Home;
    let (out, _) = frames(&mut s, size, 1.0, 1);
    let row = text_at(&out, super::words::EMPTY_MAP_MOD).expect("Empty map mod");
    click(&mut s, size, row);
    assert!(s.new_mod.is_some(), "Empty map mod opens the dialog");
    s.frame(raw_input(size, 1.0, 0, vec![text("Bridge jumps")]));
    let out = s.frame(raw_input(size, 1.0, 0, vec![]));
    assert!(text_at(&out, &format!("{} bridge-jumps", super::words::NEW_MOD_GOES)).is_some());
    let create = text_at(&out, ob_look::words::CREATE).expect("Create");
    click(&mut s, size, create);
    assert!(s.new_mod.is_none());
    assert_eq!(s.open_mod().map(|m| m.folder.as_str()), Some("bridge-jumps"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn home_says_how_many_mods_do_not_fit_and_the_row_opens_the_list() {
    use super::shell::HomeAct;
    let d = std::env::temp_dir().join(format!("ob-studio-shell-more-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for n in 0..5 {
        let folder = format!("mod-{n}");
        std::fs::create_dir_all(d.join(&folder)).unwrap();
        std::fs::write(d.join(&folder).join(store::FILE), format!("overbright-map 1\nname = Mod {n}\n")).unwrap();
    }
    let mut seen = (false, false);
    for (w, h, scale) in SIZES {
        let mut s = Studio::new(d.clone(), None);
        let (_, l) = frames(&mut s, (w, h), scale, 3);
        assert!(l.problems().is_empty(), "{w}x{h}: {:?}", l.problems());
        let t = s.home_targets();
        let rows = t.iter().filter(|a| matches!(a, HomeAct::Mod(_))).count();
        let more = t.iter().filter(|a| **a == HomeAct::MoreMods).count();
        // The count and the rows agree: a "more" row exactly when a mod has no row of its own.
        assert_eq!(more, usize::from(rows < 5), "{w}x{h}: {rows} rows, {more} more rows");
        seen = (seen.0 || more == 1, seen.1 || more == 0);
        if more == 1 {
            // Enter on the row opens Find on the mods.
            s.home_focus = t.iter().position(|a| *a == HomeAct::MoreMods).unwrap();
            s.frame(raw_input((w, h), scale, 0, vec![key(egui::Key::Enter, false, false)]));
            assert!(s.nav.find_open, "{w}x{h}");
        }
    }
    assert_eq!(seen, (true, true), "the sizes cover both a short list and a full one");
}
