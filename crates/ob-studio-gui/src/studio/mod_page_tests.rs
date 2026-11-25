//! The Mod page on a mods folder with two layered mods: its data, a detail kept as a step, the export with the
//! warning for textures from the original game's pixels (and the copy imported again), drawn at the review sizes,
//! walked by keyboard and by pad.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat3, Vec3};
use ob_look::legends::InputKind;

use super::super::Studio;
use super::super::address::Address;
use super::super::nav::{Area, Level, Pad};
use super::super::shot::{frames, raw_input};
use super::super::workspace::Workspace;
use super::{LINES, Line, load};
use crate::tools::map::history::{self, History};
use crate::tools::map::{MapMod, PropEdit, PropRef, store};

fn target(x: f32) -> PropRef {
    PropRef {
        region: "L4RA".into(),
        section: 1407,
        solid: "XO_DOME_ROOF".into(),
        at: Vec3::new(x, -297.1, 12.0),
        nth: 0,
    }
}

fn mv(x: f32, to: f32) -> PropEdit {
    PropEdit::Move { target: target(x), position: Vec3::new(to, -297.1, 12.0), rotation: Mat3::IDENTITY }
}

fn put(maps: &Path, folder: &str, m: &MapMod) {
    let dir = maps.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let edir = store::editor_dir(maps, folder);
    let mut h = History::open(&dir, &edir, &[store::FILE], "test", Arc::new(history::disk::Real)).unwrap();
    h.commit("test", "Make the mod", BTreeMap::from([(store::FILE.to_string(), Some(m.write()))])).unwrap();
    h.save().unwrap();
}

/// Harbour edits (two moves, one of them also moved by Drift park above it; a texture from the game's pixels).
fn maps(name: &str, derived: bool) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-modpage-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let harbour = MapMod {
        name: "Harbour edits".into(),
        author: "Sam".into(),
        description: "Cones at the docks".into(),
        props: vec![mv(-860.2, -842.4), mv(-805.1, -799.2)],
        races: vec![],
    };
    let drift = MapMod { name: "Drift park".into(), props: vec![mv(-805.1, -801.0)], ..Default::default() };
    put(&d, "harbour-edits", &harbour);
    put(&d, "drift-park", &drift);
    std::fs::write(d.join("enabled.txt"), "harbour-edits\ndrift-park\n").unwrap();
    let dir = d.join("harbour-edits");
    let origin = if derived { "game" } else { "blank" };
    std::fs::write(
        dir.join("looks.toml"),
        format!(
            "format = 1\n\n[[texture]]\nid = \"world:0x1A2B3C4D\"\nname = \"DOCK_SIGN\"\norigin = \"{origin}\"\n\
             image = \"00112233aabbccdd\"\nwidth = 4\nheight = 4\n"
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("textures/world")).unwrap();
    std::fs::write(dir.join("textures/world/1A2B3C4D.png"), b"\x89PNG pixels").unwrap();
    std::fs::create_dir_all(dir.join("work/looks/world")).unwrap();
    std::fs::write(dir.join("work/looks/world/DOCK_SIGN.png"), b"work").unwrap();
    d
}

fn studio(dir: &Path) -> Studio {
    let mut s = Studio::new(dir.to_path_buf(), Some("harbour-edits"));
    s.mod_page.set_game(None);
    s
}

fn key(k: egui::Key, ctrl: bool) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers { ctrl, command: ctrl, ..Default::default() },
    }
}

#[test]
fn data_has_details_files_clashes_and_readiness() {
    let dir = maps("data", false);
    let d = load(&dir, "harbour-edits", None, None).unwrap();
    assert!(d.in_game);
    assert_eq!(d.map.author, "Sam");
    let files: Vec<&str> = d.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["looks.toml", "map.obmap", "textures/world/1A2B3C4D.png"], "no work or editor files");
    assert_eq!(d.clashes.len(), 2, "the dome's position and heading: {:?}", d.clashes);
    assert_eq!(d.clashes[0].winner, "Drift park wins");
    assert!(ob_studio::share::is_ready(&d.ready), "{:?}", d.ready);
    assert!(d.derived.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keyboard_keeps_a_detail_as_one_step_and_esc_leaves_it() {
    let dir = maps("keys", false);
    let size = (1280, 800);
    let mut s = studio(&dir);
    frames(&mut s, size, 1.0, 1);
    // Ctrl+0: the Mod page.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Num0, true)]));
    assert_eq!(s.nav.workspace, Workspace::Mod);
    assert_eq!(s.nav.area, Area::View);
    // ↓ to Author, Enter opens its field, type, Enter keeps it.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowDown, false)]));
    assert_eq!(s.mod_page.line(), Line::Author);
    assert_eq!(s.nav.level, Level::Row);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    assert_eq!(s.mod_page.editing.as_deref(), Some("Sam"));
    s.mod_page.editing = Some("Sam and Kim".into());
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    assert!(s.mod_page.editing.is_none());
    let file = store::read_file(&dir.join("harbour-edits").join(store::FILE)).unwrap();
    assert_eq!(file.author, "Sam and Kim");
    let h = History::open_read(
        &dir.join("harbour-edits"),
        &store::editor_dir(&dir, "harbour-edits"),
        &[store::FILE],
        Arc::new(history::disk::Real),
    )
    .unwrap();
    assert_eq!(h.steps().last().unwrap().label, "Mod details");
    // Esc leaves a field without a step.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    s.mod_page.editing = Some("nobody".into());
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Escape, false)]));
    assert!(s.mod_page.editing.is_none());
    assert_eq!(s.mod_page.data.as_ref().unwrap().map.author, "Sam and Kim");
    assert_eq!(s.nav.workspace, Workspace::Mod);
    // Ctrl+Z on the page takes the step back.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Z, true)]));
    assert_eq!(s.mod_page.data.as_ref().unwrap().map.author, "Sam");
    // Clashes: Enter opens Changes on its Clashes view.
    for _ in 0..3 {
        s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowDown, false)]));
    }
    assert_eq!(s.mod_page.line(), Line::Clashes);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    assert_eq!(s.nav.workspace, Workspace::Changes);
    assert_eq!(s.address().to_string(), "ob://harbour-edits/@changes?mode=clashes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_warns_for_derived_textures_and_imports_again_as_the_same_mod() {
    let dir = maps("export", true);
    let size = (1920, 1080);
    let mut s = studio(&dir);
    s.go(&Address::view("harbour-edits", "@mod")).unwrap();
    // Proposed: Overbright's exports folder, never the home folder itself.
    let proposed = PathBuf::from(&s.mod_page.export_to);
    assert_eq!(proposed, ob_studio::share::default_zip(&crate::platform::dirs::exports(), "harbour-edits"));
    assert_ne!(proposed.parent(), crate::platform::dirs::home().as_deref(), "not the home folder");
    // A folder that does not exist yet is made.
    let zip = dir.join("shared/harbour-edits.zip");
    s.mod_page.export_to = zip.display().to_string();
    frames(&mut s, size, 1.0, 1);
    for _ in 0..LINES.len() {
        s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowDown, false)]));
    }
    assert_eq!(s.mod_page.line(), Line::Export);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    assert!(s.mod_page.confirm, "the warning comes first");
    assert!(!zip.exists());
    let (_, l) = frames(&mut s, size, 1.0, 1);
    let card = s.mod_page.last.warning.expect("the warning card is drawn");
    assert!(l.view.contains_rect(card), "{card:?} in {:?}", l.view);
    // Esc: not exported. Enter again, then Enter on the warning: exported.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Escape, false)]));
    assert!(!s.mod_page.confirm && !zip.exists());
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false)]));
    assert!(zip.is_file(), "{:?}", s.mod_page.note);
    assert!(s.mod_page.note.as_ref().unwrap().1.starts_with("Exported to"));
    // Imported into a fresh mods folder: the same mod, its texture, no work files.
    let fresh = dir.join("fresh");
    let f = ob_studio::share::import(&fresh, &zip).unwrap();
    assert_eq!(
        store::read_file(&fresh.join(&f).join(store::FILE)).unwrap(),
        store::read_file(&dir.join("harbour-edits").join(store::FILE)).unwrap()
    );
    assert!(fresh.join(&f).join("textures/world/1A2B3C4D.png").is_file());
    assert!(!fresh.join(&f).join("work").exists() && !fresh.join(&f).join(".editor").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The four review sizes.
const REVIEW: [(u32, u32); 4] = [(1920, 1080), (1280, 800), (1024, 768), (2560, 1080)];

#[test]
fn drawn_at_the_review_sizes_inside_the_page() {
    let dir = maps("draw", false);
    for size in REVIEW {
        for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
            let mut s = studio(&dir);
            s.go(&Address::view("harbour-edits", "@mod")).unwrap();
            s.mod_page.files_open = true;
            s.nav.input = input;
            let (_, l) = frames(&mut s, size, 1.0, 2);
            assert!(l.problems().is_empty(), "{size:?} {input:?}: {:?}", l.problems());
            let last = &s.mod_page.last;
            assert!(last.rows.len() >= 10, "{size:?}: {} rows drawn", last.rows.len());
            for r in &last.rows {
                assert!(l.view.expand(1.0).contains_rect(*r), "{size:?}: row {r:?} outside the page {:?}", l.view);
            }
            assert_eq!(last.focus.is_some(), input != InputKind::Mouse, "{size:?} {input:?}: the focus ring");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pad_walks_the_rows_and_folds_files_with_a() {
    let dir = maps("pad", false);
    let size = (1024, 768);
    let mut s = studio(&dir);
    s.go(&Address::view("harbour-edits", "@mod")).unwrap();
    frames(&mut s, size, 1.0, 1);
    for _ in 0..3 {
        s.pad(Pad::Down, InputKind::Xbox);
    }
    assert_eq!(s.mod_page.line(), Line::Files);
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.mod_page.files_open);
    let (_, l) = frames(&mut s, size, 1.0, 1);
    assert!(l.legend.is_some(), "the pad brings the legend row");
    assert!(s.mod_page.last.focus.is_some());
    // A on Name opens its field, B leaves it; B again leaves the row, then the page.
    for _ in 0..3 {
        s.pad(Pad::Up, InputKind::Xbox);
    }
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.mod_page.editing.is_some());
    s.pad(Pad::B, InputKind::Xbox);
    assert!(s.mod_page.editing.is_none());
    assert_eq!(s.nav.workspace, Workspace::Mod);
    let _ = std::fs::remove_dir_all(&dir);
}
