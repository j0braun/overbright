use super::*;
use crate::import::tests::{TREE, fake_edition, make_tree, opts};
use crate::import::{Outcome, import};
use ob_core::game_fs::Layer as _;
use ob_testkit::Scratch;

/// A fake install imported into a scratch base folder: (scratch, game folder, base folder, set folder).
fn imported(name: &str) -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let s = Scratch::new(name);
    let game = s.path().join("game");
    let base = s.path().join("app/base");
    make_tree(&game);
    let out = import(&game, &base, &opts(vec![fake_edition(&game)]), &|_| {}).unwrap();
    let Outcome::Done { dir, .. } = out else { panic!("{out:?}") };
    (s, game, base, dir)
}

/// The layer is one per process: tests that mount it take turns.
static MOUNTED: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn content(p: &str) -> bool {
    let top = p.split('/').next().unwrap().to_ascii_uppercase();
    crate::import::CONTENT_FOLDERS.iter().any(|c| c.eq_ignore_ascii_case(&top))
}

#[test]
fn a_mounted_set_gives_the_folders_bytes() {
    let (s, _game, _base, dir) = imported("mount-same");
    let stamps = s.path().join("cache");
    let set = BaseSet::open(&dir, Some(&stamps)).unwrap();
    assert!(set.hashed > 0, "the first mount verifies every bundle");
    let mut n = 0;
    for (p, b) in TREE.iter().filter(|(p, _)| content(p)) {
        let got = set.get(&p.to_ascii_uppercase()).unwrap_or_else(|| panic!("{p} not in the set"));
        assert_eq!(&got[..], *b, "{p}");
        n += 1;
    }
    assert_eq!(set.len(), n, "only content files");
    assert!(set.get("SPEED2.EXE").is_none() && set.get("SCRIPTS/MOD.ASI").is_none());
    // The second mount trusts the stamps.
    assert_eq!(BaseSet::open(&dir, Some(&stamps)).unwrap().hashed, 0);
}

#[test]
fn reads_go_through_the_mounted_layer() {
    let (_s, game, _base, dir) = imported("mount-read");
    let set = BaseSet::open(&dir, None).unwrap();
    // The folder's copy changed after the import: the layer's bytes are the imported ones.
    std::fs::write(game.join("TRACKS/L4RA.BUN"), b"changed after the import").unwrap();
    let _turn = MOUNTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    ob_core::game_fs::mount(&game, std::sync::Arc::new(set));
    let via_rel = ob_core::game_fs::read(&game, "tracks\\l4ra.bun").unwrap();
    let via_path = ob_core::game_fs::read_file(&ob_core::game_fs::path(&game, "TRACKS/L4RA.BUN")).unwrap();
    let via_map = ob_core::cache::SourceBytes::open(&game.join("TRACKS/L4RA.BUN")).unwrap().to_vec();
    let not_content = ob_core::game_fs::read(&game, "SPEED2.EXE").unwrap();
    let (layer, bytes, _) = ob_core::game_fs::counts();
    ob_core::game_fs::unmount();
    assert_eq!(via_rel, b"track bundle");
    assert_eq!(via_path, b"track bundle");
    assert_eq!(via_map, b"track bundle");
    assert_eq!(not_content, b"not really an executable", "files the set lacks come from the folder");
    assert!(layer >= 3 && bytes >= 36);
    // Unmounted: the folder again.
    assert_eq!(ob_core::game_fs::read(&game, "TRACKS/L4RA.BUN").unwrap(), b"changed after the import");
    // The changed file is named (the mount's warning), found in any case.
    assert_eq!(BaseSet::open(&dir, None).unwrap().changed_in(&game), ["TRACKS/L4RA.BUN"]);
}

#[test]
fn a_damaged_or_incomplete_set_is_refused() {
    let (s, _game, _base, dir) = imported("mount-bad");
    let stamps = s.path().join("cache");
    BaseSet::open(&dir, Some(&stamps)).unwrap();
    // A byte changed in place (same size, new time): the stamp no longer matches, the digest is checked.
    let b = dir.join("bundles/world-L4RA.obz");
    let mut bytes = std::fs::read(&b).unwrap();
    let at = bytes.len() / 2;
    bytes[at] ^= 0xff;
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&b, &bytes).unwrap();
    let e = BaseSet::open(&dir, Some(&stamps)).err().unwrap();
    assert_eq!(e.rule, Rule::Lay3, "{e:?}");
    bytes[at] ^= 0xff;
    std::fs::write(&b, &bytes).unwrap();
    BaseSet::open(&dir, None).unwrap();
    // A missing bundle.
    std::fs::rename(&b, dir.join("world-L4RA.obz.away")).unwrap();
    assert_eq!(BaseSet::open(&dir, None).err().unwrap().rule, Rule::Lay3);
    std::fs::rename(dir.join("world-L4RA.obz.away"), &b).unwrap();
    // A set made by a newer importer.
    let rec = dir.join("import.toml");
    let text = std::fs::read_to_string(&rec).unwrap();
    std::fs::write(&rec, text.replace(&format!("epoch = {}", crate::import::EPOCH), "epoch = 999")).unwrap();
    assert_eq!(BaseSet::open(&dir, None).err().unwrap().rule, Rule::Ver4);
    std::fs::write(&rec, text.replace("read_back = \"pass\"", "read_back = \"fail\"")).unwrap();
    assert_eq!(BaseSet::open(&dir, None).err().unwrap().rule, Rule::Lay3);
    std::fs::remove_file(&rec).unwrap();
    assert!(BaseSet::open(&dir, None).is_err());
}

#[test]
fn damage_the_stamps_would_trust_is_hashed_after_forget_verified() {
    use std::io::{Seek, SeekFrom, Write};
    let (s, _game, _base, dir) = imported("mount-forget");
    let stamps = s.path().join("cache");
    BaseSet::open(&dir, Some(&stamps)).unwrap();
    // One byte flipped in place, size, time and inode kept: the stamp matches, the damage is not seen.
    let b = dir.join("bundles/world-L4RA.obz");
    let mtime = std::fs::metadata(&b).unwrap().modified().unwrap();
    let at = std::fs::metadata(&b).unwrap().len() / 2;
    let old = std::fs::read(&b).unwrap()[at as usize];
    let mut f = std::fs::OpenOptions::new().write(true).open(&b).unwrap();
    f.seek(SeekFrom::Start(at)).unwrap();
    f.write_all(&[old ^ 0xff]).unwrap();
    f.set_modified(mtime).unwrap();
    drop(f);
    assert_eq!(BaseSet::open(&dir, Some(&stamps)).unwrap().hashed, 0, "the stamps are trusted");
    // A full check found it (import::verify): the stamps forgotten, the next mount hashes and refuses.
    assert_eq!(crate::import::verify(&dir, 1, &|_| {}).unwrap().len(), 1);
    forget_verified(&stamps, &crate::import::read_record(&dir).unwrap().id);
    assert_eq!(BaseSet::open(&dir, Some(&stamps)).err().unwrap().rule, Rule::Lay3);
}

#[test]
fn the_flag_names_a_folder_an_id_or_the_active_set() {
    let (_s, _game, base, dir) = imported("mount-resolve");
    let id = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert_eq!(resolve(dir.to_str().unwrap(), &base).unwrap(), dir);
    assert_eq!(resolve(&id, &base).unwrap(), dir);
    assert_eq!(resolve("active", &base).unwrap(), dir);
    std::fs::remove_file(base.join("active.txt")).unwrap();
    assert_eq!(resolve("active", &base).unwrap(), dir, "the only set");
    assert_eq!(resolve("nfsu2-other", &base).err().unwrap().rule, Rule::Lay5);
    assert_eq!(resolve("../x", &base).err().unwrap().rule, Rule::Lay5);
}

/// The reference install's import mounts and gives the folder's bytes for every content file (reads only
/// NFSU2_ROOT; writes to a scratch folder).
#[test]
#[ignore = "needs the game files (NFSU2_ROOT) and ~2 GB of scratch space"]
fn reference_install_mounts() {
    let Some(root) = std::env::var_os("NFSU2_ROOT") else { return };
    let root = PathBuf::from(root);
    let s = Scratch::new("mount-real");
    let out = import(&root, &s.path().join("base"), &crate::import::Options::default(), &|_| {}).unwrap();
    let Outcome::Done { dir, .. } = out else { panic!() };
    let t = std::time::Instant::now();
    let set = BaseSet::open(&dir, Some(&s.path().join("cache"))).unwrap();
    eprintln!("first mount: {:.2} s, {} files", t.elapsed().as_secs_f64(), set.len());
    let t = std::time::Instant::now();
    BaseSet::open(&dir, Some(&s.path().join("cache"))).unwrap();
    eprintln!("stamped mount: {:.3} s", t.elapsed().as_secs_f64());
    let rec = crate::import::read_record(&dir).unwrap();
    for f in &rec.scan.files {
        let want = std::fs::read(root.join(&f.path)).unwrap();
        assert!(set.get(&f.path.to_ascii_uppercase()).is_some_and(|b| b[..] == want[..]), "{}", f.path);
    }
}

#[test]
fn a_name_the_install_lacks_is_absent_without_asking_the_folder() {
    let (_s, game, _base, dir) = imported("mount-absent");
    let set = BaseSet::open(&dir, None).unwrap();
    // Content folders were scanned whole: a name in them the set lacks was not in the install.
    assert!(set.absent("NIS/SCENE_ENDINGNIS04_EVTTRK.TXT"));
    assert!(!set.absent("TRACKS/L4RA.BUN"), "held");
    assert!(!set.absent("TRACKS"), "a folder, not a file");
    assert!(!set.absent("SPEED2.EXE") && !set.absent("SCRIPTS/MOD.ASI"), "outside the content folders: the folder's");
    // Placed in the folder after the import: still absent (the set is what the game reads).
    std::fs::write(game.join("NIS/Scene_EndingNis04_EvtTrk.txt"), b"added later").unwrap();
    let _turn = MOUNTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    ob_core::game_fs::mount(&game, std::sync::Arc::new(set));
    let before = ob_core::game_fs::counts().2;
    let rel = ob_core::game_fs::read(&game, "NIS/Scene_EndingNis04_EvtTrk.txt");
    let path = ob_core::game_fs::read_file(&game.join("NIS/Scene_EndingNis04_EvtTrk.txt"));
    let map = ob_core::cache::SourceBytes::open(&game.join("NIS/Scene_EndingNis04_EvtTrk.txt")).map(|b| b.to_vec());
    let after = ob_core::game_fs::counts().2;
    let absent = ob_core::game_fs::absent_count();
    ob_core::game_fs::unmount();
    for r in [rel, path, map] {
        assert_eq!(r.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }
    assert_eq!(after, before, "no read fell through to the folder");
    assert!(absent >= 3);
}
