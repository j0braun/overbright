use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use super::work::{Imported, Workbench, materialize};
use super::*;
use crate::history::{History, disk};

/// The game's textures for the tests: 4x2 RGBA of one colour each.
struct Fake(HashMap<TextureId, Original>);

impl Textures for Fake {
    fn original(&self, id: TextureId) -> Option<Original> {
        self.0.get(&id).cloned()
    }
}

const ROAD: TextureId = TextureId { domain: Domain::World, hash: 0x1A2B_3C4D };
const WALL: TextureId = TextureId { domain: Domain::World, hash: 0x0000_00FE };

fn fake() -> Fake {
    let tex = |name: &str, c: u8| Original {
        name: name.into(),
        width: 4,
        height: 2,
        rgba: [c, c, c, 255].repeat(8),
        used_by: Vec::new(),
    };
    Fake(HashMap::from([(ROAD, tex("ROAD_MAIN", 10)), (WALL, tex("wall odd name", 20))]))
}

fn png(c: u8) -> Vec<u8> {
    encode_png(4, 2, &[c, 0, 0, 255].repeat(8)).unwrap()
}

/// A mod folder with an empty map (`<tmp>/<tag>/docks`).
fn mod_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ob-looks-{tag}-{}", std::process::id())).join("docks");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(ob_map::tools::map::store::FILE), "").unwrap();
    dir
}

fn history(dir: &Path) -> History {
    History::open(dir, &dir.join(".editor"), crate::MOD_DOCS, "test", Arc::new(disk::Real)).unwrap()
}

fn first_pixel(path: &Path) -> u8 {
    decode_png(&std::fs::read(path).unwrap()).unwrap().2[0]
}

#[test]
fn ids_read_and_print() {
    assert_eq!(ROAD.to_string(), "world:0x1A2B3C4D");
    assert_eq!("world:0x1a2b3c4d".parse::<TextureId>(), Ok(ROAD));
    assert_eq!("car:FE".parse::<TextureId>(), Ok(TextureId::new(Domain::Car, 0xFE)));
    for bad in ["", "world", "ui:0x1", "world:", "world:0x123456789", "world:zz"] {
        assert!(bad.parse::<TextureId>().is_err(), "{bad}");
    }
    assert_eq!(ROAD.pack_file(), "world/1A2B3C4D.png");
}

#[test]
fn the_list_reads_back_and_merges_per_texture() {
    let e = |id: TextureId, image: &str| Entry {
        id,
        name: "n".into(),
        origin: Origin::Game,
        image: image.into(),
        width: 4,
        height: 2,
    };
    let base = Looks { entries: BTreeMap::from([(ROAD, e(ROAD, "aa"))]) };
    assert_eq!(Looks::parse(&base.write()), Ok(base.clone()));
    assert_eq!(Looks::parse(""), Ok(Looks::default()));
    assert!(Looks::parse("format = 99\n").is_err(), "a newer format is refused");
    // Two writers: one paints the road, the other adds the wall; both stay.
    let mut ours = base.clone();
    ours.entries.insert(ROAD, e(ROAD, "bb"));
    let mut theirs = base.clone();
    theirs.entries.insert(WALL, e(WALL, "cc"));
    let st = |l: &Looks| crate::history::doc::State::from([(FILE.to_string(), l.write())]);
    let (m, conflicts) =
        crate::history::doc::merge(&st(&base), &st(&ours), &st(&theirs), crate::history::doc::Prefer::Ours);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    let m = Looks::parse(&m[FILE]).unwrap();
    assert_eq!((m.entries[&ROAD].image.as_str(), m.entries[&WALL].image.as_str()), ("bb", "cc"));
    // The same texture painted two ways: a conflict on that texture only.
    let mut theirs = base.clone();
    theirs.entries.insert(ROAD, e(ROAD, "dd"));
    let (_, conflicts) =
        crate::history::doc::merge(&st(&base), &st(&ours), &st(&theirs), crate::history::doc::Prefer::Ours);
    assert_eq!(conflicts, vec![format!("{FILE}#texture:{ROAD}")]);
}

#[test]
fn a_save_becomes_one_step_and_the_pack_follows() {
    let dir = mod_dir("save");
    let edir = dir.join(".editor");
    let mut h = history(&dir);
    let mut w = Workbench::open(&dir, &edir);
    let f = w.start(&Looks::default(), ROAD, &fake(), false).unwrap();
    assert_eq!((f.origin, f.file.as_str()), (Origin::Game, "work/looks/world/ROAD_MAIN.png"));
    let work = w.path(&f);
    assert_eq!(first_pixel(&work), 10, "the game's own pixels");
    assert!(w.poll().is_empty(), "writing the work file is not a save");
    // The paint program saves: taken in once it has settled (one look later).
    std::fs::write(&work, png(200)).unwrap();
    assert!(w.poll().is_empty(), "not settled yet");
    let saves = w.poll();
    assert_eq!(saves.len(), 1);
    let saved = saves[0].clone().unwrap();
    let seq = match w.import(&mut h, "paint", &saved).unwrap() {
        Imported::Step(s) => s,
        o => panic!("{o:?}"),
    };
    assert_eq!(h.steps().last().map(|s| (s.seq, s.label.as_str())), Some((seq, "Paint ROAD_MAIN")));
    let looks = Looks::parse(h.doc(FILE).unwrap()).unwrap();
    let e = &looks.entries[&ROAD];
    assert_eq!((e.origin, e.width, e.height, e.image.clone()), (Origin::Game, 4, 2, image_hash(&png(200))));
    let pack = dir.join(PACK_DIR).join(ROAD.pack_file());
    assert_eq!(first_pixel(&pack), 200, "the pack data the game reads");
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), looks.write(), "the list saved");
    assert!(w.poll().is_empty(), "taken in once");
    // The same image again: no step.
    assert_eq!(w.import(&mut h, "paint", &saved).unwrap(), Imported::Unchanged);
    // A second save, then undo: the pack goes back to the first save's image.
    std::fs::write(&work, png(100)).unwrap();
    let _ = w.poll();
    let saved = w.poll().pop().unwrap().unwrap();
    assert!(matches!(w.import(&mut h, "paint", &saved).unwrap(), Imported::Step(_)));
    assert_eq!(first_pixel(&pack), 100);
    h.undo("test").unwrap();
    materialize(&dir, &edir, &Looks::parse(h.doc(FILE).unwrap()).unwrap()).unwrap();
    assert_eq!(first_pixel(&pack), 200, "undo: the image the step before named");
    // Undo the first paint too: the texture leaves the pack (the original shows again).
    h.undo("test").unwrap();
    materialize(&dir, &edir, &Looks::parse(h.doc(FILE).unwrap_or_default()).unwrap()).unwrap();
    assert!(!pack.exists());
    // The work list holds across a restart; a work file that exists is never overwritten.
    let mut w = Workbench::open(&dir, &edir);
    assert_eq!(w.file(ROAD).map(|f| f.origin), Some(Origin::Game));
    w.start(&Looks::default(), ROAD, &fake(), false).unwrap();
    assert_eq!(first_pixel(&work), 100, "the modder's paint kept");
    assert!(w.start(&Looks::default(), ROAD, &fake(), true).is_err(), "blank over paint is refused");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn repaint_from_blank_is_marked_and_broken_saves_wait() {
    let dir = mod_dir("blank");
    let edir = dir.join(".editor");
    let mut h = history(&dir);
    let mut w = Workbench::open(&dir, &edir);
    let f = w.start(&Looks::default(), WALL, &fake(), true).unwrap();
    assert_eq!(f.origin, Origin::Blank);
    assert!(!f.origin.derived());
    assert_eq!(f.file, "work/looks/world/0x000000FE.png", "a name that is no file name: the hash");
    let work = w.path(&f);
    let (fw, fh, px) = decode_png(&std::fs::read(&work).unwrap()).unwrap();
    assert_eq!((fw, fh), (4, 2), "the texture's size");
    assert!(px.chunks(4).all(|p| p == work::BLANK), "an empty canvas");
    // Half a file (a program still writing): not taken in, the pack untouched.
    let full = png(50);
    std::fs::write(&work, &full[..full.len() / 2]).unwrap();
    let _ = w.poll();
    assert!(w.poll().iter().all(|r| r.is_err()));
    assert!(!dir.join(PACK_DIR).exists());
    std::fs::write(&work, &full).unwrap();
    let _ = w.poll();
    let saved = w.poll().pop().unwrap().unwrap();
    w.import(&mut h, "paint", &saved).unwrap();
    let looks = Looks::parse(h.doc(FILE).unwrap()).unwrap();
    assert_eq!(looks.entries[&WALL].origin, Origin::Blank, "the provenance stays with the texture");
    // No texture the game lacks.
    let none = TextureId::new(Domain::Car, 1);
    assert!(w.start(&looks, none, &fake(), false).is_err());
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn materialize_removes_only_textures_the_list_lacks() {
    let dir = mod_dir("mat");
    let edir = dir.join(".editor");
    let world = dir.join(PACK_DIR).join("world");
    std::fs::create_dir_all(&world).unwrap();
    std::fs::write(world.join("DEADBEEF.png"), png(1)).unwrap();
    std::fs::write(world.join("readme.txt"), "mine").unwrap();
    assert_eq!(materialize(&dir, &edir, &Looks::default()), Ok(1));
    assert!(!world.join("DEADBEEF.png").exists());
    assert!(world.join("readme.txt").exists(), "not a texture file of the list's form");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
