//! A fake game folder made by the test (no game data): names in mixed
//! case, looked up in other cases.

use super::*;

struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh folder holding `files` (relative names, `/`-separated) with
/// their names as content.
fn root(files: &[&str]) -> Root {
    let d = std::env::temp_dir().join(format!("nfsu2-gamefs-{}", crate::unique_tag()));
    for f in files {
        let p = d.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, f.as_bytes()).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    Root(d)
}

/// `got` names the file `want` names: the same path, or on Windows (whose
/// file system already ignores case, so the exact spelling "exists") the
/// same file.
#[track_caller]
fn same(got: PathBuf, want: PathBuf) {
    #[cfg(windows)]
    {
        // Case-blind: a canonical path keeps the spelling it was asked by
        // (Wine), or gives the file's own (NTFS).
        let canon = |p: &Path| std::fs::canonicalize(p).map(|c| c.to_string_lossy().to_lowercase()).unwrap();
        assert_eq!(canon(&got), canon(&want), "{got:?} vs {want:?}");
    }
    #[cfg(not(windows))]
    assert_eq!(got, want);
}

#[cfg(not(windows))]
fn listed() -> usize {
    LISTED.load(std::sync::atomic::Ordering::Relaxed)
}

#[test]
fn exact_spelling_is_the_literal_join() {
    let r = root(&["GLOBAL/GlobalB.lzc"]);
    assert_eq!(path(&r.0, "GLOBAL/GlobalB.lzc"), r.0.join("GLOBAL/GlobalB.lzc"));
    assert_eq!(find(&r.0, "GLOBAL"), Some(r.0.join("GLOBAL")));
    assert_eq!(read(&r.0, "GLOBAL/GlobalB.lzc").unwrap(), b"GLOBAL/GlobalB.lzc");
}

#[test]
fn other_case_resolves_to_the_real_spelling() {
    let r = root(&["Global/GlobalB.lzc", "LANGUAGES/English.bin", "CARS/WHEELS/TEXTURES.BIN"]);
    same(path(&r.0, "GLOBAL/GLOBALB.LZC"), r.0.join("Global/GlobalB.lzc"));
    same(path(&r.0, "languages/english.BIN"), r.0.join("LANGUAGES/English.bin"));
    same(path(&r.0, r"LANGUAGES\ENGLISH.BIN"), r.0.join("LANGUAGES/English.bin"));
    same(path(&r.0, "cars/Wheels/textures.bin"), r.0.join("CARS/WHEELS/TEXTURES.BIN"));
    assert_eq!(read(&r.0, r"global\globalb.lzc").unwrap(), b"Global/GlobalB.lzc");
    assert_eq!(read_to_string(&r.0, "Languages/ENGLISH.bin").unwrap(), "LANGUAGES/English.bin");
    same(find(&r.0, "cars/wheels/").unwrap(), r.0.join("CARS/WHEELS"));
}

#[test]
fn a_miss_is_the_literal_path_and_fails_to_open() {
    let r = root(&["GLOBAL/GlobalB.lzc"]);
    assert_eq!(find(&r.0, "GLOBAL/InGameCommon.bun"), None);
    assert_eq!(find(&r.0, "NOPE/InGameCommon.bun"), None);
    assert_eq!(path(&r.0, "GLOBAL/InGameCommon.bun"), r.0.join("GLOBAL/InGameCommon.bun"));
    let e = read(&r.0, "global/InGameCommon.bun").unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    // A file is not a directory.
    assert_eq!(find(&r.0, "GLOBAL/GlobalB.lzc/x"), None);
}

/// Not on Windows: there the exact spelling is found by the file system.
#[cfg(not(windows))]
#[test]
fn a_directory_is_listed_once() {
    let r = root(&["Sound/Engine/A.abk", "Sound/Engine/B.abk", "Sound/Engine/C.abk"]);
    // Other tests list directories in parallel: count only growth that
    // these lookups could cause, and check it stops.
    let before = listed();
    assert!(find(&r.0, "SOUND/ENGINE/a.ABK").is_some());
    let after_first = listed();
    assert!(after_first - before >= 2, "root and Sound listed");
    for n in ["b.abk", "C.ABK", "a.abk"] {
        assert!(find(&r.0, &format!("SOUND/ENGINE/{n}")).is_some());
    }
    let cache = LISTINGS.lock().unwrap();
    let cache = cache.as_ref().unwrap();
    for d in [r.0.clone(), r.0.join("Sound"), r.0.join("Sound/Engine")] {
        assert!(cache.contains_key(&d), "{} cached", d.display());
    }
}

#[test]
fn the_exact_spelling_needs_no_listing() {
    let r = root(&["TRACKS/L4RA.BUN"]);
    assert_eq!(find(&r.0, "TRACKS/L4RA.BUN"), Some(r.0.join("TRACKS/L4RA.BUN")));
    assert_eq!(read(&r.0, "TRACKS/L4RA.BUN").unwrap(), b"TRACKS/L4RA.BUN");
    let cache = LISTINGS.lock().unwrap();
    assert!(cache.as_ref().is_none_or(|c| !c.keys().any(|k| k.starts_with(&r.0))));
}

#[cfg(not(windows))]
#[test]
fn two_spellings_prefer_the_exact_then_the_smallest() {
    let r = root(&["DIR/abc.bin", "DIR/ABC.bin", "DIR/Abc.bin"]);
    assert_eq!(path(&r.0, "dir/ABC.bin"), r.0.join("DIR/ABC.bin"));
    assert_eq!(path(&r.0, "dir/aBC.BIN"), r.0.join("DIR/ABC.bin"));
}

#[test]
fn names_that_leave_the_root_are_refused() {
    let outside = root(&["SECRET.TXT"]);
    let r = root(&["GLOBAL/GlobalB.lzc"]);
    let up = format!("../{}/SECRET.TXT", outside.0.file_name().unwrap().to_string_lossy());
    for rel in [
        up.as_str(),
        "GLOBAL/../GLOBAL/GlobalB.lzc",
        "./GLOBAL/GlobalB.lzc",
        "/etc/passwd",
        r"\Windows\win.ini",
        "C:/Windows/win.ini",
        r"C:GLOBAL\GlobalB.lzc",
        r"\\?\C:\x",
        "GLOBAL//GlobalB.lzc",
    ] {
        assert!(matches!(resolve(&r.0, rel), Err(Refused::Escapes(_))), "{rel}");
        assert_eq!(find(&r.0, rel), None, "{rel}");
        let p = path(&r.0, rel);
        assert!(p.starts_with(&r.0) && !p.exists(), "{rel}: {}", p.display());
        assert_eq!(read(&r.0, rel).unwrap_err().kind(), std::io::ErrorKind::InvalidInput, "{rel}");
    }
    assert_eq!(resolve(&r.0, ""), Ok(Some(r.0.clone())));
}


/// A file added after its folder was listed is found (the launcher's
/// "check again"): a miss reads a changed folder again.
#[test]
fn a_file_added_after_the_listing_is_found() {
    let d = std::env::temp_dir().join(format!("nfsu2-game-fs-added-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("global")).unwrap();
    assert_eq!(find(&d, "GLOBAL/GlobalB.lzc"), None);
    std::fs::write(d.join("global").join("globalb.LZC"), b"x").unwrap();
    assert_eq!(find(&d, "GLOBAL/GlobalB.lzc"), Some(d.join("global").join("globalb.LZC")));
    std::fs::remove_dir_all(&d).unwrap();
}
