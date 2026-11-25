//! The game folder's own checks against the player's game and stand-in
//! folders: the region, the edition fingerprint, the game-folder check
//! (through the library's public API; moved out of the low-level modules,
//! whose own tests need no game data).

use std::path::PathBuf;

use nfsu2::fingerprint::{EDITIONS, FILES, Verdict, compute};
use nfsu2::region::{EUROPE, US, detect, european};

mod common;

/// A fresh empty folder named for `tag`.
fn fake_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nfsu2-fp-{tag}-{}", nfsu2::unique_tag()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A stand-in game folder, removed when dropped.
struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh folder holding `files` (relative names, `/`-separated) with
/// their names as content.
fn root(files: &[&str]) -> Root {
    let d = std::env::temp_dir().join(format!("nfsu2-gamefs-{}", nfsu2::unique_tag()));
    for f in files {
        let p = d.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, f.as_bytes()).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    Root(d)
}

/// The installed European data is region 6.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn european_install() {
    let root = common::game_root();
    assert_eq!(detect(&root), EUROPE);
    assert!(european(detect(&root)));
}

/// A US-style CARS listing is region 0; with either European car (or
/// without either US one) it is 6.
#[test]
fn us_listing() {
    let scratch = common::Scratch::new("region-us");
    let root = scratch.path();
    for car in ["civic", "RSX", "350Z", "MIATA", "PEUGOT"] {
        std::fs::create_dir_all(root.join("CARS").join(car)).unwrap();
    }
    assert_eq!(detect(root), US);
    assert!(!european(US));
    std::fs::create_dir_all(root.join("CARS/CORSA")).unwrap();
    assert_eq!(detect(root), EUROPE);
    std::fs::remove_dir(root.join("CARS/CORSA")).unwrap();
    std::fs::remove_dir(root.join("CARS/RSX")).unwrap();
    assert_eq!(detect(root), EUROPE);
    assert_eq!(detect(&root.join("missing")), EUROPE);
}

/// The player's install (NFSU2_ROOT) is the known edition.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_reference_install_is_the_known_edition() {
    let root = common::game_root();
    let f = compute(&root);
    assert_eq!(f.verdict(), Verdict::Known(EDITIONS[0].name), "{}", f.report());
}

/// A patched copy of GlobalB.lzc (one byte changed) is flagged, the
/// rest still known (the OpenUG2 study's case). Unix only: the other
/// files are symlinked (Windows symlinks need a privilege).
#[cfg(unix)]
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_patched_global_lzc_is_flagged() {
    let root = common::game_root();
    let mut lzc = std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap();
    let d = fake_root("patched");
    for f in FILES {
        let to = d.join(f);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        if f == "GLOBAL/GlobalB.lzc" {
            lzc[0x3E0] ^= 1;
            std::fs::write(&to, &lzc).unwrap();
        } else {
            std::os::unix::fs::symlink(root.join(f), &to).unwrap();
        }
    }
    let f = compute(&d);
    assert_eq!(f.verdict(), Verdict::Unknown(vec!["GLOBAL/GlobalB.lzc".into()]));
    assert!(f.report().contains("GLOBAL/GlobalB.lzc: 5145778 bytes"));
    std::fs::remove_dir_all(&d).unwrap();
}

/// The game-folder check, the start test and the fingerprint accept an
/// install whose names are spelled in other cases (the key files here are
/// empty: present, of a different size).
#[test]
fn the_game_folder_check_takes_any_case() {
    // With the watched files (the sky pack): the fingerprint hashes them too.
    let watched = nfsu2::qol::setup::WATCHED_FILES.iter().map(|(f, ..)| f);
    let lower: Vec<String> =
        nfsu2::qol::setup::KEY_FILES.iter().map(|(f, ..)| f).chain(watched).map(|f| f.to_ascii_lowercase()).collect();
    let mut files: Vec<&str> = lower.iter().map(String::as_str).collect();
    files.push("speed2.exe");
    let r = root(&files);
    let c = nfsu2::qol::setup::check(&r.0, true);
    assert!(c.ok(), "missing {:?}", c.missing);
    assert_eq!(c.different.len(), nfsu2::qol::setup::KEY_FILES.len() + nfsu2::qol::setup::WATCHED_FILES.len());
    assert!(nfsu2::qol::setup::looks_like_game(&r.0));
    let f = nfsu2::fingerprint::compute(&r.0);
    assert!(f.files.iter().all(|p| !matches!(p.state, nfsu2::fingerprint::State::Missing)), "{:?}", f.files);
    // And a folder without them still fails.
    let empty = root(&[]);
    assert_eq!(nfsu2::qol::setup::check(&empty.0, false).missing.len(), nfsu2::qol::setup::KEY_FILES.len());
    assert!(!nfsu2::qol::setup::looks_like_game(&empty.0));
}
