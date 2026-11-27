//! The Data page on a mods folder: its table from the Changes rows, sort and search, a cell edit as one undo step
//! of the mod's history, the jump to the Changes entry, by keyboard and by pad, drawn at the review sizes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat3, Vec2, Vec3};
use ob_look::legends::InputKind;

use super::super::Studio;
use super::super::address::Address;
use super::super::matrix;
use super::super::nav::{Area, Pad};
use super::super::shot::{frames, raw_input};
use super::super::workspace::Workspace;
use super::super::words;
use crate::tools::map::history::{self, History};
use crate::tools::map::{CustomRace, MapMod, PropEdit, PropRef, RaceKind, store};

fn put(maps: &Path, folder: &str, m: &MapMod) {
    let dir = maps.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let edir = store::editor_dir(maps, folder);
    let mut h = History::open(&dir, &edir, &[store::FILE], "test", Arc::new(history::disk::Real)).unwrap();
    h.commit("test", "Make the mod", BTreeMap::from([(store::FILE.to_string(), Some(m.write()))])).unwrap();
    h.save().unwrap();
}

fn maps(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-data-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let race = |id: &str, name: &str, track: u16, kind: RaceKind, laps: u32| {
        let mut r = CustomRace::new(id, track, kind);
        r.name = name.into();
        r.laps = laps;
        r.checkpoints = vec![Vec2::ZERO; 4];
        r
    };
    let m = MapMod {
        name: "Harbour edits".into(),
        author: String::new(),
        description: String::new(),
        props: vec![PropEdit::Move {
            target: PropRef {
                region: "L4RA".into(),
                section: 1407,
                solid: "XO_DOME_ROOF".into(),
                at: Vec3::new(-860.2, -297.1, 12.0),
                nth: 0,
            },
            position: Vec3::new(-842.4, -297.1, 12.0),
            rotation: Mat3::IDENTITY,
        }],
        races: vec![
            race("b", "Bravo", 4002, RaceKind::Circuit, 3),
            race("a", "alpha", 4001, RaceKind::Sprint, 1),
            race("c", "Charlie", 4010, RaceKind::Drift, 1),
        ],
        collision: vec![],
    };
    put(&d, "harbour-edits", &m);
    std::fs::write(d.join("enabled.txt"), "harbour-edits\n").unwrap();
    d
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

fn open(dir: &Path, size: (u32, u32)) -> Studio {
    let mut s = Studio::new(dir.to_path_buf(), Some("harbour-edits"));
    s.go(&Address::view("harbour-edits", "@world")).unwrap();
    frames(&mut s, size, 1.0, 1);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::G, true)]));
    assert_eq!(s.nav.workspace, Workspace::Data);
    assert_eq!(s.nav.workspace.on_rail(), Workspace::World);
    frames(&mut s, size, 1.0, 1);
    s
}

fn press(s: &mut Studio, size: (u32, u32), k: egui::Key) {
    s.frame(raw_input(size, 1.0, 0, vec![key(k, false)]));
}

fn names(s: &Studio) -> Vec<String> {
    let g = s.data.grid.as_ref().unwrap();
    (0..g.counts().1).map(|i| g.cell(i, 0).unwrap().text).collect()
}

fn laps_in_file(dir: &Path, id: &str) -> u32 {
    let text = std::fs::read_to_string(dir.join("harbour-edits").join(store::FILE)).unwrap();
    MapMod::parse(&text).unwrap().races.iter().find(|r| r.id == id).unwrap().laps
}

#[test]
fn sort_search_and_a_cell_edit_by_keyboard() {
    let dir = maps("keys");
    let size = (1280, 800);
    let mut s = open(&dir, size);
    // Props first; Tab-less: pick the Races tab as a click would.
    s.data.kind_pick = None;
    let rows = s.changes.data().unwrap().rows.clone();
    s.data.set_kind("race", &rows);
    assert_eq!(s.data.grid.as_ref().unwrap().counts(), (3, 3));
    // ↑ to the header, Enter sorts the Name column; Enter again turns it round.
    press(&mut s, size, egui::Key::ArrowUp);
    assert_eq!(s.data.row, None);
    press(&mut s, size, egui::Key::Enter);
    assert_eq!(names(&s), ["alpha", "Bravo", "Charlie"]);
    press(&mut s, size, egui::Key::Enter);
    assert_eq!(names(&s), ["Charlie", "Bravo", "alpha"]);
    s.data.grid.as_mut().unwrap().set_search("circuit");
    assert_eq!(names(&s), ["Bravo"]);
    s.data.grid.as_mut().unwrap().set_search("");
    // Down onto Charlie (sorted down), across to Laps, Enter edits, Enter keeps: one step in the file.
    press(&mut s, size, egui::Key::ArrowDown);
    press(&mut s, size, egui::Key::ArrowDown);
    assert_eq!(s.data.row, Some(1));
    for _ in 0..3 {
        press(&mut s, size, egui::Key::ArrowRight);
    }
    assert_eq!(s.data.col, 3);
    press(&mut s, size, egui::Key::Enter);
    assert_eq!(s.data.editing.as_deref(), Some("3"));
    s.data.editing = Some("4".into());
    press(&mut s, size, egui::Key::Enter);
    assert!(s.data.editing.is_none());
    assert_eq!(laps_in_file(&dir, "b"), 4, "{:?}", s.data.note);
    assert!(s.data.note.as_deref().unwrap().starts_with("Laps 4"));
    let (_, l) = frames(&mut s, size, 1.0, 1);
    assert!(l.problems().is_empty());
    // The cell reads 4 and says it is yours.
    let c = s.data.grid.as_ref().unwrap().cell(1, 3).unwrap();
    assert_eq!((c.text.as_str(), c.yours), ("4", true));
    // Ctrl+Z takes it back through the history.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Z, true)]));
    assert_eq!(laps_in_file(&dir, "b"), 3);
    assert_eq!(s.data.grid.as_ref().unwrap().cell(1, 3).unwrap().text, "3");
    // Esc leaves an edit without writing.
    press(&mut s, size, egui::Key::Enter);
    s.data.editing = Some("9".into());
    press(&mut s, size, egui::Key::Escape);
    assert!(s.data.editing.is_none());
    assert_eq!(laps_in_file(&dir, "b"), 3);
    // A refused value changes nothing and says why.
    press(&mut s, size, egui::Key::Enter);
    s.data.editing = Some("99".into());
    press(&mut s, size, egui::Key::Enter);
    assert!(s.data.note.as_deref().unwrap().starts_with("Not changed"), "{:?}", s.data.note);
    assert_eq!(laps_in_file(&dir, "b"), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_only_column_and_props_without_the_editor_say_so() {
    let dir = maps("ro");
    let size = (1280, 800);
    let mut s = open(&dir, size);
    // Props: Object cannot be changed here.
    assert_eq!(s.data.grid.as_ref().unwrap().kind, "placement");
    press(&mut s, size, egui::Key::Enter);
    assert!(s.data.editing.is_none());
    assert!(s.data.note.as_deref().unwrap().contains("cannot be changed here yet"));
    // Position needs World's loaded city when World has no session: the note says so, the file is as it was.
    press(&mut s, size, egui::Key::ArrowRight);
    press(&mut s, size, egui::Key::Enter);
    s.data.editing = Some("1, 2, 3".into());
    press(&mut s, size, egui::Key::Enter);
    assert!(s.data.note.as_deref().unwrap().contains("open World"), "{:?}", s.data.note);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_row_leads_to_its_changes_entry() {
    let dir = maps("jump");
    let size = (1280, 800);
    let mut s = open(&dir, size);
    let rows = s.changes.data().unwrap().rows.clone();
    s.data.set_kind("race", &rows);
    s.data.row = Some(0);
    let key = s.data.grid.as_ref().unwrap().key(0).unwrap().to_string();
    let ask = s.data.focused_changes().unwrap();
    assert!(matches!(ask, super::Ask::Changes { .. }));
    s.data_show_changes("race", &key);
    assert_eq!(s.nav.workspace, Workspace::Changes);
    let p = &s.changes;
    let (g, _) = p.lines()[p.focus];
    assert_eq!(p.rows()[g].key, key);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pad_walks_and_edits_a_switch() {
    let dir = maps("pad");
    let size = (1024, 768);
    let mut s = open(&dir, size);
    let rows = s.changes.data().unwrap().rows.clone();
    s.data.set_kind("race", &rows);
    s.data.grid.as_mut().unwrap().sort_by(0);
    s.pad(Pad::Down, InputKind::Xbox);
    assert_eq!(s.data.row, Some(1));
    for _ in 0..4 {
        s.pad(Pad::Right, InputKind::Xbox);
    }
    assert_eq!(s.data.col, 4);
    s.pad(Pad::A, InputKind::Xbox); // Backward flips at once
    let text = std::fs::read_to_string(dir.join("harbour-edits").join(store::FILE)).unwrap();
    let m = MapMod::parse(&text).unwrap();
    assert!(m.races.iter().find(|r| r.id == "b").unwrap().backward, "{:?}", s.data.note);
    let (_, l) = frames(&mut s, size, 1.0, 1);
    assert!(l.legend.is_some() && s.data.last.focus.is_some());
    assert_eq!(s.nav.area, Area::View);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn drawn_at_the_review_sizes_inside_the_page() {
    let dir = maps("draw");
    for kind in ["placement", "race"] {
        let drawn = matrix::page(
            &format!("data {kind}"),
            2,
            |_, _| {
                let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
                s.go(&Address::parse("ob://harbour-edits/@data").unwrap()).unwrap();
                let rows = s.changes.data().unwrap().rows.clone();
                s.data.set_kind(kind, &rows);
                // Several cells selected, so the highlight and the focus are in the review too.
                let last = s.data.grid.as_ref().map_or(0, |g| g.counts().1).saturating_sub(1);
                s.data.row = Some(last.min(1));
                s.data.anchor = Some((0, 0));
                s
            },
            |s, l, size, input| {
                let last = &s.data.last;
                let mut p = matrix::inside_view(l, &last.rows);
                if last.rows.is_empty() || last.header.is_none() {
                    p.push(format!("{}: no rows or header drawn", size.label()));
                }
                if last.focus.is_some() != (input != InputKind::Mouse) {
                    p.push("the focus ring does not follow the input".into());
                }
                p
            },
        );
        assert!(drawn >= 12, "{drawn}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn races_open(name: &str) -> (PathBuf, Studio) {
    let dir = maps(name);
    let mut s = open(&dir, (1280, 800));
    let rows = s.changes.data().unwrap().rows.clone();
    s.data.set_kind("race", &rows);
    s.data.grid.as_mut().unwrap().sort_by(0); // alpha, Bravo, Charlie
    (dir, s)
}

fn shift(k: egui::Key) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers { shift: true, ..Default::default() },
    }
}

fn backward_in_file(dir: &Path) -> Vec<(String, bool)> {
    let text = std::fs::read_to_string(dir.join("harbour-edits").join(store::FILE)).unwrap();
    MapMod::parse(&text).unwrap().races.iter().map(|r| (r.id.clone(), r.backward)).collect()
}

#[test]
fn shift_arrows_select_and_one_value_fills_the_selection_as_one_step() {
    let (dir, mut s) = races_open("fill");
    let size = (1280, 800);
    s.data.row = Some(1);
    s.data.col = 4;
    s.frame(raw_input(size, 1.0, 0, vec![shift(egui::Key::ArrowDown)]));
    assert_eq!(s.data.span().map(|x| x.cells()), Some(2));
    press(&mut s, size, egui::Key::Enter);
    assert_eq!(s.data.editing.as_deref(), Some("no"));
    s.data.editing = Some("yes".into());
    press(&mut s, size, egui::Key::Enter);
    let now = backward_in_file(&dir);
    assert!(now.iter().filter(|(id, _)| id != "a").all(|(_, b)| *b), "{now:?} {:?}", s.data.note);
    assert!(s.data.note.as_deref().unwrap().starts_with("Fill 2 cells"), "{:?}", s.data.note);
    // One Ctrl+Z takes both back.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Z, true)]));
    assert!(backward_in_file(&dir).iter().all(|(_, b)| !*b));
    // Esc ends the selection first.
    press(&mut s, size, egui::Key::Escape);
    assert!(!s.data.multi());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_paste_goes_in_as_one_step_and_a_bad_cell_refuses_it_all() {
    let (dir, mut s) = races_open("paste");
    let size = (1280, 800);
    let before = backward_in_file(&dir);
    s.data.row = Some(0);
    s.data.col = 0;
    s.frame(raw_input(size, 1.0, 0, vec![egui::Event::Paste("One\tcircuit\nTwo\tdrift".into())]));
    assert!(s.data.note.as_deref().unwrap().starts_with("Paste 4 cells"), "{:?}", s.data.note);
    let text = std::fs::read_to_string(dir.join("harbour-edits").join(store::FILE)).unwrap();
    assert!(text.contains("name = One") && text.contains("name = Two"));
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Z, true)]));
    assert_eq!(names(&s), ["alpha", "Bravo", "Charlie"]);
    // Laps 99 in the second row: nothing is written, the note names the cell.
    let file = std::fs::read(dir.join("harbour-edits").join(store::FILE)).unwrap();
    s.frame(raw_input(size, 1.0, 0, vec![egui::Event::Paste("X\tcircuit\t4002\t99".into())]));
    let note = s.data.note.clone().unwrap();
    assert!(note.starts_with("Not pasted") && note.contains("Laps") && note.contains("range"), "{note}");
    assert_eq!(std::fs::read(dir.join("harbour-edits").join(store::FILE)).unwrap(), file);
    assert_eq!(backward_in_file(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_table_goes_out_as_csv_and_comes_back_by_the_same_checks() {
    let (dir, mut s) = races_open("csv");
    let size = (1280, 800);
    let file = dir.join("harbour-edits").join(".editor").join("reference").join("race.csv");
    // Importing before exporting says what to do.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::I, true)]));
    assert_eq!(s.data.note.as_deref(), Some(words::NO_TABLE_FILE_YET));
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::E, true)]));
    let csv = std::fs::read_to_string(&file).unwrap();
    assert_eq!(csv, s.data.grid.as_ref().unwrap().csv());
    let mod_file = std::fs::read(dir.join("harbour-edits").join(store::FILE)).unwrap();
    // Nothing changed in it: nothing is made, the mod's file is byte-identical.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::I, true)]));
    assert_eq!(s.data.note.as_deref(), Some(words::NOTHING_TO_CHANGE));
    assert_eq!(std::fs::read(dir.join("harbour-edits").join(store::FILE)).unwrap(), mod_file);
    // A bad cell refuses the file.
    std::fs::write(&file, csv.replace(",3,no,", ",99,no,")).unwrap();
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::I, true)]));
    assert!(s.data.note.as_deref().unwrap().starts_with("Not imported"), "{:?}", s.data.note);
    assert_eq!(std::fs::read(dir.join("harbour-edits").join(store::FILE)).unwrap(), mod_file);
    // An edited one is one step.
    std::fs::write(&file, csv.replace(",3,no,", ",4,no,")).unwrap();
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::I, true)]));
    assert_eq!(laps_in_file(&dir, "b"), 4, "{:?}", s.data.note);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Z, true)]));
    assert_eq!(laps_in_file(&dir, "b"), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pad_extends_a_selection_and_fills_it() {
    let (dir, mut s) = races_open("padfill");
    let size = (1024, 768);
    s.data.row = Some(1);
    s.data.col = 4;
    s.pad(Pad::View, InputKind::Xbox);
    s.pad(Pad::Down, InputKind::Xbox);
    assert!(s.data.multi());
    s.pad(Pad::A, InputKind::Xbox);
    assert!(s.data.editing.is_some());
    s.data.editing = Some("yes".into());
    s.pad(Pad::A, InputKind::Xbox);
    assert!(backward_in_file(&dir).iter().filter(|(id, _)| id != "a").all(|(_, b)| *b));
    s.pad(Pad::B, InputKind::Xbox);
    assert!(!s.data.multi() && !s.data.extend);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fifty_thousand_rows_draw_only_the_visible_ones() {
    let (dir, mut s) = races_open("big");
    let size = (1280, 800);
    let rows: Vec<ob_studio::changes::Row> = (0..50_000)
        .map(|i| ob_studio::changes::Row {
            kind: "race",
            key: format!("big:r{i}"),
            added: true,
            values: vec![ob_studio::changes::Value {
                field: "laps".into(),
                from: ob_studio::changes::From::Yours,
                game: Some(toml::Value::Integer(1 + i % 20)),
                yours: Some(toml::Value::Integer(1 + i % 20)),
                original: None,
            }],
        })
        .collect();
    s.data.grid = ob_studio::data::Grid::from_rows("race", &rows);
    s.data.row = Some(0);
    let t = std::time::Instant::now();
    frames(&mut s, size, 1.0, 3);
    assert!(s.data.last.rows.len() <= s.data.shown + 2, "{} rows drawn", s.data.last.rows.len());
    // Down to the far end: still only the visible rows, and the focus is on screen.
    s.data.row = Some(49_999);
    frames(&mut s, size, 1.0, 3);
    assert!(s.data.last.rows.len() <= s.data.shown + 2);
    assert!(s.data.last.focus.is_some());
    s.data.select_all();
    assert_eq!(s.data.span().map(|x| x.cells()), Some(50_000 * 6));
    frames(&mut s, size, 1.0, 2);
    assert!(s.data.last.rows.len() <= s.data.shown + 2);
    assert!(t.elapsed().as_secs() < 30, "{:?}", t.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
