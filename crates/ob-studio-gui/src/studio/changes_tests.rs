//! The Changes page on a mods folder with layered mods: its rows read through the history and the enabled mods,
//! Reset and undo as steps, drawn at the review sizes, walked through by keyboard and by pad.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat3, Vec3};
use ob_look::legends::InputKind;
use ob_studio::changes::From;

use super::super::Studio;
use super::super::address::Address;
use super::super::nav::{Area, Level, Pad};
use super::super::shot::{frames, raw_input};
use super::super::workspace::Workspace;
use super::{Mode, load};
use crate::tools::map::history::{self, History};
use crate::tools::map::{CustomRace, MapMod, PropEdit, PropRef, RaceKind, store};

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

/// Write a mod through its history (one step), as Studio would.
fn put(maps: &Path, folder: &str, m: &MapMod) {
    let dir = maps.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let edir = store::editor_dir(maps, folder);
    let mut h = History::open(&dir, &edir, &[store::FILE], "test", Arc::new(history::disk::Real)).unwrap();
    h.commit("test", "Make the mod", BTreeMap::from([(store::FILE.to_string(), Some(m.write()))])).unwrap();
    h.save().unwrap();
}

/// Harbour edits (moves two props, hides one, adds a race) below Drift park (moves one of the same props).
fn maps(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ob-studio-changes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut race = CustomRace::new("casino-sprint", 1, RaceKind::Sprint);
    race.name = "Casino sprint".into();
    let harbour = MapMod {
        name: "Harbour edits".into(),
        author: String::new(),
        description: String::new(),
        props: vec![mv(-860.2, -842.4), mv(-805.1, -799.2), PropEdit::Delete { target: target(-700.0) }],
        races: vec![race],
    };
    let drift = MapMod {
        name: "Drift park".into(),
        author: String::new(),
        description: String::new(),
        props: vec![mv(-805.1, -801.0)],
        races: vec![],
    };
    put(&d, "harbour-edits", &harbour);
    put(&d, "drift-park", &drift);
    std::fs::write(d.join("enabled.txt"), "harbour-edits\ndrift-park\n").unwrap();
    d
}

#[test]
fn rows_come_from_the_history_and_the_enabled_mods() {
    let dir = maps("data");
    let d = load(&dir, "harbour-edits").unwrap();
    assert!(d.in_game);
    assert_eq!(d.stack.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), ["Harbour edits", "Drift park"]);
    assert_eq!(d.rows.len(), 4, "two moves, a hidden prop, a race");
    let pos = |i: usize| d.rows[i].values.iter().find(|v| v.field == "position").unwrap().clone();
    assert_eq!(pos(0).from, From::Yours);
    assert_eq!(pos(1).from, From::Other("drift-park".into()));
    assert!(pos(1).loses());
    assert!(!d.clashes.is_empty());
    assert_eq!(d.steps.len(), 1);
    // Off in the game: the mod sits on top, as turning it on puts it; nothing of it loses.
    std::fs::write(dir.join("enabled.txt"), "drift-park\n").unwrap();
    let off = load(&dir, "harbour-edits").unwrap();
    assert!(!off.in_game);
    assert!(off.rows.iter().all(|r| !r.loses()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reset_and_undo_are_steps_of_the_mods_history() {
    let dir = maps("reset");
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.go(&Address::parse("ob://harbour-edits/@changes").unwrap()).unwrap();
    assert_eq!(s.nav.workspace, Workspace::Changes);
    let p = &mut s.changes;
    // The first row: the dome's position, yours. Reset it.
    assert_eq!(p.rows()[0].values[0].field, "position");
    p.focus = 0;
    p.reset();
    assert!(p.note.as_deref().unwrap().starts_with("Reset Position of XO_DOME_ROOF"), "{:?}", p.note);
    let d = p.data().unwrap();
    assert_eq!(d.steps.len(), 2);
    let v = d.rows[0].values.iter().find(|v| v.field == "position").unwrap();
    assert_eq!(v.from, From::Original, "back at its original place");
    let text = std::fs::read_to_string(dir.join("harbour-edits").join(store::FILE)).unwrap();
    assert!(text.contains("position = -860.2"), "saved to the file:\n{text}");
    // A history step's view lists what it changed.
    p.set_mode(Mode::Step(2));
    assert_eq!(p.rows().len(), 1);
    // Ctrl+Z takes it back.
    s.frame(raw_input((1280, 800), 1.0, 0, vec![key(egui::Key::Z, true, false)]));
    let v = s.changes.data().unwrap().rows[0].values.iter().find(|v| v.field == "position").unwrap().clone();
    assert_eq!(v.from, From::Yours);
    // Several picked rows are one step: the hidden prop and the race.
    let p = &mut s.changes;
    p.set_mode(Mode::ThisMod);
    let lines = p.lines();
    let rows = p.rows();
    let pick: Vec<usize> = (0..lines.len())
        .filter(|i| {
            let (g, v) = lines[*i];
            rows[g].values[v].field == "disabled" || (rows[g].kind == "race" && rows[g].values[v].field == "race")
        })
        .collect();
    assert_eq!(pick.len(), 2);
    p.picked = pick.into_iter().collect();
    p.reset();
    let d = p.data().unwrap();
    assert!(d.rows.iter().all(|r| r.kind != "race"));
    assert!(d.rows.iter().all(|r| r.values.iter().all(|v| v.field != "disabled" || v.yours.is_none())));
    assert_eq!(d.steps.last().unwrap().label, "Reset 2 values");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn addresses_name_the_pages_modes() {
    let dir = maps("addr");
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    for a in [
        "ob://harbour-edits/@changes",
        "ob://harbour-edits/@changes?step=1",
        "ob://harbour-edits/@changes?mode=clashes",
    ] {
        s.go(&Address::parse(a).unwrap()).unwrap();
        assert_eq!(s.address().to_string(), a);
    }
    assert_eq!(s.changes.mode, Mode::Clashes);
    assert_eq!(s.crumbs(), ["Harbour edits", "Changes", "Clashes"]);
    assert!(s.go(&Address::parse("ob://harbour-edits/@changes?step=x").unwrap()).is_err());
    // Show in World: the item's address leads to World.
    s.changes.set_mode(Mode::ThisMod);
    let a = s.changes.focused_address().unwrap();
    assert!(a.starts_with("ob://harbour-edits/placement/L4RA/1407/XO_DOME_ROOF/"), "{a}");
    s.go(&Address::parse(&a).unwrap()).unwrap();
    assert_eq!(s.nav.workspace, Workspace::World);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The four review sizes.
const REVIEW: [(u32, u32); 4] = [(1920, 1080), (1280, 800), (1024, 768), (2560, 1080)];

#[test]
fn drawn_at_the_review_sizes_inside_the_page() {
    let dir = maps("draw");
    for size in REVIEW {
        for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
            let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
            s.go(&Address::parse("ob://harbour-edits/@changes").unwrap()).unwrap();
            s.nav.input = input;
            let (_, l) = frames(&mut s, size, 1.0, 2);
            assert!(l.problems().is_empty(), "{size:?} {input:?}: {:?}", l.problems());
            let last = &s.changes.last;
            assert!(last.rows.len() >= 8, "{size:?}: {} rows drawn", last.rows.len());
            for r in &last.rows {
                assert!(l.view.expand(1.0).contains_rect(*r), "{size:?}: row {r:?} outside the page {:?}", l.view);
            }
            // The Inspector beside the list on wide windows; a strip below.
            assert_eq!(last.inspector.is_some(), size.0 >= 1280, "{size:?}");
            assert_eq!(last.focus.is_some(), input != InputKind::Mouse, "the focus ring follows the keys");
        }
    }
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

#[test]
fn keyboard_walks_the_rows_and_picks_several() {
    let dir = maps("keys");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.go(&Address::view("harbour-edits", "@world")).unwrap();
    frames(&mut s, size, 1.0, 1);
    // Ctrl+D opens Changes; the rail still shows World.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::D, true, false)]));
    assert_eq!(s.nav.workspace, Workspace::Changes);
    assert_eq!(s.nav.workspace.on_rail(), Workspace::World);
    assert_eq!(s.nav.area, Area::View);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowDown, false, false)]));
    assert_eq!(s.changes.focus, 1);
    assert_eq!(s.nav.level, Level::Row);
    // Shift+↓ picks the rows passed over.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowDown, false, true)]));
    assert_eq!(s.changes.picked.iter().copied().collect::<Vec<_>>(), [1, 2]);
    // Enter takes the focused one back out.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Enter, false, false)]));
    assert_eq!(s.changes.picked.iter().copied().collect::<Vec<_>>(), [1]);
    // ↑ stops at the first row.
    for i in 0..5 {
        s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::ArrowUp, false, false)]));
    }
    assert_eq!(s.changes.focus, 0);
    // The focused row's ring is drawn.
    let (_, _) = frames(&mut s, size, 1.0, 1);
    assert!(s.changes.last.focus.is_some());
    // Shift+F10: the menu, with Reset to original; Esc out of it, then out of the row, then back to World.
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::F10, false, true)]));
    assert_eq!(s.nav.level, Level::Menu);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Escape, false, false)]));
    assert_eq!(s.nav.level, Level::Row);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Escape, false, false)]));
    assert_eq!(s.nav.workspace, Workspace::Changes);
    s.frame(raw_input(size, 1.0, 0, vec![key(egui::Key::Escape, false, false)]));
    assert_eq!(s.nav.workspace, Workspace::World, "the back crumb: the place before");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pad_walks_the_rows_and_picks_with_a() {
    let dir = maps("pad");
    let size = (1024, 768);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.go(&Address::view("harbour-edits", "@world")).unwrap();
    s.go(&Address::parse("ob://harbour-edits/@changes").unwrap()).unwrap();
    frames(&mut s, size, 1.0, 1);
    s.pad(Pad::Down, InputKind::Xbox);
    s.pad(Pad::Down, InputKind::Xbox);
    assert_eq!(s.changes.focus, 2);
    s.pad(Pad::A, InputKind::Xbox);
    s.pad(Pad::Up, InputKind::Xbox);
    s.pad(Pad::A, InputKind::Xbox);
    assert_eq!(s.changes.picked.iter().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(s.nav.input, InputKind::Xbox);
    let (_, l) = frames(&mut s, size, 1.0, 1);
    assert!(l.legend.is_some(), "the pad brings the legend row");
    assert!(s.changes.last.focus.is_some());
    // B: out of the row, then the back crumb.
    s.pad(Pad::B, InputKind::Xbox);
    assert_eq!(s.nav.workspace, Workspace::Changes);
    s.pad(Pad::B, InputKind::Xbox);
    assert_eq!(s.nav.workspace, Workspace::World);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The mod's write lock belongs to the editor this window hosts: Home and Changes do not call that "another
/// program", Reset and undo go through the editor's session (a second writer would be refused), and once the
/// editor lets the mod go no note is left that would claim it later (a note a killed writer left is ignored).
#[test]
fn the_editor_of_this_window_is_not_another_program() {
    use crate::tools::map::session::MapSession;
    use nfsu2::studio_api::world::World;
    let dir = maps("own-lock");
    let folder = "harbour-edits";
    let (mdir, edir) = (dir.join(folder), store::editor_dir(&dir, folder));
    let open = || {
        let h = History::open(&mdir, &edir, &[store::FILE], history::STUDIO, Arc::new(history::disk::Real)).unwrap();
        assert!(h.writable());
        let m = MapMod::parse(h.doc(store::FILE).unwrap()).unwrap();
        let mut session = MapSession::new("L4RA", folder, m, World::default());
        session.keep_in(h);
        session
    };
    let mut session = open();
    assert_eq!(super::super::home::summary(&dir, folder).unwrap().read_only_by, None, "held by this window");
    let mut s = Studio::new(dir.clone(), Some(folder));
    s.go(&Address::parse("ob://harbour-edits/@changes").unwrap()).unwrap();
    let p = &mut s.changes;
    p.focus = 0;
    p.reset_with(Some(&mut session));
    assert!(p.note.as_deref().unwrap().starts_with("Reset Position of XO_DOME_ROOF"), "{:?}", p.note);
    assert_eq!(session.undo_label(), Some(p.note.as_deref().unwrap().split(" · ").next().unwrap()));
    p.undo_with(false, Some(&mut session));
    assert!(p.note.as_deref().unwrap().starts_with("Undone: Reset Position"), "{:?}", p.note);
    // The editor closes the mod: nothing claims it afterwards.
    drop(session);
    assert!(!edir.join(history::lock::OWNER).exists(), "a clean close removes the lock holder's note");
    assert_eq!(super::super::home::summary(&dir, folder).unwrap().read_only_by, None);
    // A note left by a killed Studio.
    std::fs::write(edir.join(history::lock::OWNER), "program = \"Overbright Studio\"\npid = 999999\nsince = 1\n")
        .unwrap();
    assert_eq!(super::super::home::summary(&dir, folder).unwrap().read_only_by, None, "a dead writer's note");
    let p = &mut s.changes;
    p.load(folder);
    p.focus = 0;
    p.reset();
    assert!(p.note.as_deref().unwrap().starts_with("Reset Position"), "{:?}", p.note);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Looks rows ------------------------------------------------------------------------------------------------

const DOME_TEX: &str = "world:0x0000D0E0";
const SIGN_TEX: &str = "world:0x00005160";

/// A painted texture: its image kept in the mod's image store; the list's entry.
fn painted(maps: &Path, folder: &str, hash: u32, name: &str, rgb: [u8; 3]) -> ob_studio::looks::Entry {
    use ob_studio::looks::{Domain, Entry, Origin, TextureId, encode_png, image_hash, work::store_path};
    let px: Vec<u8> = (0..16 * 16).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect();
    let png = encode_png(16, 16, &px).unwrap();
    let image = image_hash(&png);
    let p = store_path(&store::editor_dir(maps, folder), &image);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, png).unwrap();
    let id = TextureId::new(Domain::World, hash);
    Entry { id, name: name.into(), origin: Origin::Game, image, width: 16, height: 16 }
}

/// Paint `entries` as one step of the mod's history and write its pack data, as `looks import` does.
fn paint(maps: &Path, folder: &str, entries: Vec<ob_studio::looks::Entry>) {
    let mut l = ob_studio::looks::Looks::default();
    for e in entries {
        l.entries.insert(e.id, e);
    }
    let (dir, edir) = (maps.join(folder), store::editor_dir(maps, folder));
    let mut h = History::open(&dir, &edir, ob_studio::MOD_DOCS, "test", Arc::new(history::disk::Real)).unwrap();
    h.commit("test", "Paint", BTreeMap::from([(ob_studio::looks::FILE.to_string(), Some(l.write()))])).unwrap();
    h.save().unwrap();
    ob_studio::looks::work::materialize(&dir, &edir, &l).unwrap();
}

/// The layered mods, with Looks: Harbour edits paints the dome roof and a sign; Drift park, above it in the
/// game, paints the same sign.
fn maps_with_looks(name: &str) -> PathBuf {
    let d = maps(name);
    let dome = painted(&d, "harbour-edits", 0xD0E0, "XO_DOME_ROOF_TEX", [200, 40, 40]);
    let sign = painted(&d, "harbour-edits", 0x5160, "SIGN_CASINO", [40, 200, 40]);
    paint(&d, "harbour-edits", vec![dome, sign]);
    let theirs = painted(&d, "drift-park", 0x5160, "SIGN_CASINO", [40, 40, 200]);
    paint(&d, "drift-park", vec![theirs]);
    d
}

#[test]
fn looks_rows_sit_beside_props_and_races_with_the_overlapping_mod() {
    let dir = maps_with_looks("looks-data");
    let d = load(&dir, "harbour-edits").unwrap();
    // Two moves, a hidden prop, a race, two textures.
    assert_eq!(d.rows.len(), 6, "{:?}", d.rows.iter().map(|r| &r.key).collect::<Vec<_>>());
    let row = |k: &str| d.rows.iter().find(|r| r.key == k).unwrap();
    let dome = row(DOME_TEX);
    assert_eq!(dome.kind, "texture");
    assert_eq!(dome.values.len(), 1);
    let v = &dome.values[0];
    assert_eq!((v.field.as_str(), &v.from), ("image", &From::Yours));
    assert_eq!(v.original.as_ref().and_then(|o| o.as_str()), Some("game"));
    // The sign: Drift park's paint is drawn, yours loses; the clash names both.
    let sign = &row(SIGN_TEX).values[0];
    assert_eq!(sign.from, From::Other("drift-park".into()));
    assert!(sign.loses());
    assert_ne!(sign.game, sign.yours);
    assert!(d.clashes.iter().any(|c| c.key == SIGN_TEX && c.stack.len() == 3));
    // The prop clash is still there; so is the moved prop's original.
    assert!(d.clashes.iter().any(|c| c.kind == "placement"));
    let (title, sub) = super::item_words(&d, dome);
    assert_eq!(title, "XO_DOME_ROOF_TEX");
    assert_eq!(sub, "Texture · world · painted · 16×16");
    assert!(super::item_words(&d, row(SIGN_TEX)).1.ends_with("loses"));
    // Every image the rows name is in the image stores, for the thumbnails.
    for r in d.rows.iter().filter(|r| r.kind == "texture") {
        for v in &r.values {
            for h in [&v.yours, &v.game].into_iter().flatten().filter_map(|x| x.as_str()) {
                assert!(d.images.get(h).is_some_and(|p| p.is_file()), "{h}");
            }
        }
    }
    // The paint step lists the textures it changed.
    let step = d.steps.last().unwrap();
    assert_eq!(step.label, "Paint");
    assert!(step.rows.iter().all(|r| r.kind == "texture") && step.rows.len() == 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_texture_reset_is_one_step_and_one_undo() {
    let dir = maps_with_looks("looks-reset");
    let pack = dir.join("harbour-edits").join(store::TEXTURES).join("world/0000D0E0.png");
    assert!(pack.is_file());
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    // The texture's address opens Changes on its row.
    s.go(&Address::parse(&format!("ob://harbour-edits/texture/{DOME_TEX}")).unwrap()).unwrap();
    assert_eq!(s.nav.workspace, Workspace::Changes);
    let p = &mut s.changes;
    let (g, _) = p.lines()[p.focus];
    assert_eq!(p.rows()[g].key, DOME_TEX);
    // Its jump-to: the step that painted it.
    assert_eq!(p.focused_address().unwrap(), "ob://harbour-edits/@changes?step=2");
    let steps = p.data().unwrap().steps.len();
    p.reset();
    assert_eq!(p.note.as_deref(), Some("Reset Image of XO_DOME_ROOF_TEX · Ctrl+Z takes it back"));
    let d = p.data().unwrap();
    assert_eq!(d.steps.len(), steps + 1, "one step");
    assert!(d.rows.iter().all(|r| r.key != DOME_TEX), "the game's texture again");
    assert!(!pack.exists(), "the pack data follows the list");
    let looks = std::fs::read_to_string(dir.join("harbour-edits").join(ob_studio::looks::FILE)).unwrap();
    assert!(!looks.contains("0x0000D0E0") && looks.contains("0x00005160"), "{looks}");
    // One Ctrl+Z brings it back, pack file too.
    s.frame(raw_input((1280, 800), 1.0, 0, vec![key(egui::Key::Z, true, false)]));
    let d = s.changes.data().unwrap();
    assert_eq!(d.steps.len(), steps);
    assert!(d.rows.iter().any(|r| r.key == DOME_TEX));
    assert!(pack.is_file());
    // A prop and a texture picked together: one step, one undo.
    let p = &mut s.changes;
    let (rows, lines) = (p.rows(), p.lines());
    let pick: BTreeSet<usize> = (0..lines.len())
        .filter(|i| {
            let (g, v) = lines[*i];
            rows[g].key == DOME_TEX || (rows[g].kind == "placement" && rows[g].values[v].field == "disabled")
        })
        .collect();
    assert_eq!(pick.len(), 2);
    p.picked = pick;
    p.reset();
    let d = p.data().unwrap();
    assert_eq!(d.steps.last().unwrap().label, "Reset 2 values");
    assert_eq!(d.steps.len(), steps + 1);
    assert!(!pack.exists());
    assert!(
        d.rows.iter().all(|r| r.key != DOME_TEX && r.values.iter().all(|v| v.field != "disabled" || v.yours.is_none()))
    );
    s.changes.undo(false);
    let d = s.changes.data().unwrap();
    assert_eq!(d.steps.len(), steps);
    assert!(d.rows.iter().any(|r| r.key == DOME_TEX) && pack.is_file());
    assert!(d.rows.iter().any(|r| r.values.iter().any(|v| v.field == "disabled" && v.yours.is_some())));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The sizes the Looks rows are checked at (pixels, scale).
const LOOKS_SIZES: [((u32, u32), f32); 4] =
    [((1024, 768), 1.0), ((1920, 1080), 1.0), ((2560, 1440), 1.5), ((3840, 2160), 2.0)];

#[test]
fn looks_rows_fit_at_the_review_sizes() {
    let dir = maps_with_looks("looks-draw");
    for (size, scale) in LOOKS_SIZES {
        for input in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox] {
            let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
            s.go(&Address::parse(&format!("ob://harbour-edits/texture/{SIGN_TEX}")).unwrap()).unwrap();
            s.nav.input = input;
            let (_, l) = frames(&mut s, size, scale, 2);
            assert!(l.problems().is_empty(), "{size:?}@{scale} {input:?}: {:?}", l.problems());
            let last = &s.changes.last;
            assert!(last.rows.len() >= 8, "{size:?}: {} rows drawn", last.rows.len());
            for r in last.rows.iter().chain(&last.focus) {
                assert!(l.view.expand(1.0).contains_rect(*r), "{size:?}: row {r:?} outside the page {:?}", l.view);
            }
            if input != InputKind::Mouse {
                assert!(last.focus.is_some(), "{size:?}: the focused texture row is in view");
            }
            assert!(s.changes.thumbs_loaded() >= 2, "{size:?}: the painted images' thumbnails");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Walking the rows loads nothing new: no thumbnail or texture upload per press (the page's data is read once).
#[test]
fn many_presses_on_changes_load_nothing_new() {
    let dir = maps_with_looks("looks-keys");
    let size = (1280, 800);
    let mut s = Studio::new(dir.clone(), Some("harbour-edits"));
    s.go(&Address::parse("ob://harbour-edits/@changes").unwrap()).unwrap();
    frames(&mut s, size, 1.0, 2);
    let press = |s: &mut Studio, n: usize| {
        for i in 0..n {
            let k = if (i / 9) % 2 == 0 { egui::Key::ArrowDown } else { egui::Key::ArrowUp };
            s.frame(raw_input(size, 1.0, 0, vec![key(k, false, i % 7 == 0)]));
            s.frame(raw_input(size, 1.0, 1, vec![]));
        }
    };
    press(&mut s, 100);
    let uploads = s.textures.0.set.len();
    let thumbs = s.changes.thumbs_loaded();
    press(&mut s, 600);
    assert_eq!(s.textures.0.set.len(), uploads, "no texture upload per press");
    assert_eq!(s.changes.thumbs_loaded(), thumbs);
    assert!(s.changes.picked.len() <= s.changes.lines().len());
    let _ = std::fs::remove_dir_all(&dir);
}
