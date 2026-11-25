use super::*;
use ob_testkit::Scratch;

/// A small fake install: every content folder kind, mixed case as real installs have, an executable and files
/// that are not game content. No game data.
const TREE: &[(&str, &[u8])] = &[
    ("GLOBAL/GlobalB.lzc", b"global b"),
    ("GLOBAL/InGameCommon.bun", b"in game common"),
    ("Frontend/FrontB.lzc", b"front b"),
    ("FRONTEND/PLATFORMS/CarLot01.BIN", b"platform"),
    ("TRACKS/L4RA.BUN", b"track bundle"),
    ("TRACKS/STREAML4RA.BUN", &[7u8; 5000]),
    ("TRACKS/ROUTESL4RA/Paths4001.bin", b"paths"),
    ("TRACKS/TRACKMAP4001.BIN", b"minimap"),
    ("CARS/PEUGOT/GEOMETRY.BIN", &[1u8; 3000]),
    ("CARS/TEXTURES.BIN", b"car textures"),
    ("LANGUAGES/English.bin", b"strings"),
    ("LANGUAGES/agree.usa", b"legal"),
    ("LANGUAGES/LanguageTextures.bin", b"lang tex"),
    ("CREDITS/UK_ENGLISH.TXT", b"credits"),
    ("memcard/LOCALE_GERMAN.loc", b"save text"),
    ("SOUND/ENGINE/CAR_00_ENG_MB_EE.abk", b"engine"),
    ("SOUND/MIXMAPS/MAPOUTPUT.mxb", b"mix"),
    ("SDATA/sdat.viv", b"archive"),
    ("MOVIES/blank.vp6", b""),
    ("SUBTITLES/Scene01", b"cues"),
    ("NIS/Scene_DynoEnter_EvtTrk.txt", b"events"),
    ("SPEED2.EXE", b"not really an executable"),
    ("dinput8.dll", b"a loader"),
    ("SCRIPTS/mod.asi", b"a mod"),
];

fn make_tree(root: &Path) {
    for (p, b) in TREE {
        let f = root.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, b).unwrap();
    }
}

/// Every file's path, size, modification time and bytes below `root`.
fn snapshot(root: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let m = std::fs::metadata(&p).unwrap();
            if m.is_dir() {
                out.push((p.clone(), 0, m.modified().unwrap(), Vec::new()));
                stack.push(p);
            } else {
                out.push((p.clone(), m.len(), m.modified().unwrap(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn fake_edition(root: &Path) -> Edition {
    let mut s = scan(root).unwrap();
    fingerprint(&mut s, 2, &|_| {}).unwrap();
    Edition::parse("test-ed", "A test edition, imported", "eu", &Edition::manifest_of(&s)).unwrap()
}

fn opts<'a>(ed: Vec<Edition>) -> Options<'a> {
    Options { editions: ed, threads: 3, free_space: |_| Some(u64::MAX), ..Options::default() }
}

#[cfg(unix)]
fn set_read_only(root: &Path, ro: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p.clone());
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(if ro { 0o555 } else { 0o755 })).unwrap();
            } else {
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(if ro { 0o444 } else { 0o644 })).unwrap();
            }
        }
    }
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(if ro { 0o555 } else { 0o755 })).unwrap();
}

#[test]
fn scan_finds_content_folders_without_regard_to_case() {
    let s = Scratch::new("imp-scan");
    make_tree(s.path());
    let sc = scan(s.path()).unwrap();
    assert_eq!(sc.files.len(), 21);
    assert!(sc.files.iter().any(|f| f.path == "Frontend/FrontB.lzc"));
    assert!(sc.exe.is_some());
    let skipped: Vec<&str> = sc.skipped.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(skipped, ["SCRIPTS", "dinput8.dll"]);
    assert!(!sc.files.iter().any(|f| f.path.eq_ignore_ascii_case("SPEED2.EXE")));
}

#[test]
fn not_a_game_folder_is_imp1() {
    let s = Scratch::new("imp-empty");
    std::fs::write(s.path().join("readme.txt"), b"x").unwrap();
    assert_eq!(scan(s.path()).unwrap_err().rule, Rule::Imp1);
}

#[test]
fn units_follow_the_layout() {
    let cases = [
        ("GLOBAL/GlobalB.lzc", "global"),
        ("SDATA/sdat.viv", "global"),
        ("TRACKS/TRACKMAP4001.BIN", "global"),
        ("TRACKS/LOC4DYNTEX.BIN", "global"),
        ("TRACKS/PrecullerBooBooScript.hoo", "global"),
        ("TRACKS/L4RA.BUN", "world-L4RA"),
        ("TRACKS/STREAML4RB.BUN", "world-L4RB"),
        ("TRACKS/ROUTESL4RC/x.bin", "world-L4RC"),
        ("CARS/PEUGOT/GEOMETRY.BIN", "cars-PEUGOT"),
        ("CARS/TEXTURES.BIN", "cars-common"),
        ("FRONTEND/ENVMAPS/a.BIN", "frontend"),
        ("LANGUAGES/LanguageTextures.bin", "frontend"),
        ("LANGUAGES/English.bin", "text-english"),
        ("LANGUAGES/Labels.bin", "text-labels"),
        ("LANGUAGES/agree.usa", "text-english"),
        ("LANGUAGES/agree-ucs2.chi", "text-chinese"),
        ("CREDITS/NA_ENGLISH.TXT", "text-english"),
        ("CREDITS/GERMAN.TXT", "text-german"),
        ("memcard/LOCALE_BRAZILIAN_PORTUGUESE.loc", "text-brazilian_portuguese"),
        ("SOUND/FE/FE_MB.abk", "audio-frontend"),
        ("SOUND/NOS/a.abk", "audio-ingame"),
        ("SOUND/ENGINE/a.abk", "audio-engine"),
        ("SOUND/EVT_SYS/a.csi", "audio-common"),
        ("MOVIES/Scene01_en.vp6", "movies"),
        ("SUBTITLES/Scene01", "movies"),
        ("NIS/Scene_X_BundleB.bun", "scenes"),
    ];
    for (p, u) in cases {
        assert_eq!(unit_of(p), u, "{p}");
    }
}

#[test]
fn imports_a_known_edition_and_never_writes_the_game_folder() {
    let s = Scratch::new("imp-full");
    let game = s.path().join("game");
    let base = s.path().join("app/base");
    make_tree(&game);
    let ed = fake_edition(&game);
    let before = snapshot(&game);
    #[cfg(unix)]
    set_read_only(&game, true);
    let lines = Mutex::new(Vec::new());
    let out = import(&game, &base, &opts(vec![ed]), &|l| lines.lock().unwrap().push(l.to_string()));
    #[cfg(unix)]
    set_read_only(&game, false);
    let out = out.unwrap();
    assert_eq!(snapshot(&game), before, "the game folder changed");
    let Outcome::Done { id, dir, fresh: true } = out else { panic!("{out:?}") };
    assert_eq!(id, "nfsu2-test-ed");
    let lines = lines.into_inner().unwrap();
    assert!(lines.contains(&"edition=test-ed known".to_string()), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("space_needed=")));
    assert!(!base.join(format!(".staging-{id}")).exists());
    assert_eq!(
        crate::order::parse_active(&std::fs::read(base.join("active.txt")).unwrap()).unwrap().as_deref(),
        Some(id.as_str())
    );
    // Every original byte is in its unit's bundle, stored and aligned.
    let b = Bundle::open(&dir.join("bundles/world-L4RA.obz"), bundle::Kind::Bundle).unwrap();
    let e = b.get("residual/TRACKS/STREAML4RA.BUN.kept.bin").unwrap();
    assert!(e.mapped());
    assert_eq!(&b.read(e).unwrap()[..], &[7u8; 5000][..]);
    let rec = std::fs::read_to_string(dir.join("import.toml")).unwrap();
    assert!(rec.contains("verdict = \"known\"") && rec.contains("[[source.skipped]]") && rec.contains("exe = {"));
    assert!(!rec.contains(&s.path().display().to_string()), "the record names a folder of the machine");
    let (m, _) = crate::manifest::Manifest::parse(
        &std::fs::read(dir.join("pack.toml")).unwrap(),
        crate::manifest::Container::Folder(&id),
    )
    .unwrap();
    assert_eq!(m.id, id);
    assert!(verify(&dir, 2, &|_| {}).unwrap().is_empty());
    // A second run does nothing.
    let again = import(&game, &base, &opts(vec![fake_edition(&game)]), &|_| {}).unwrap();
    assert!(matches!(again, Outcome::Done { fresh: false, .. }));
}

#[test]
fn custom_install_names_the_differing_files() {
    let s = Scratch::new("imp-custom");
    let game = s.path().join("game");
    make_tree(&game);
    let ed = fake_edition(&game);
    std::fs::write(game.join("GLOBAL/GlobalB.lzc"), b"modded").unwrap();
    std::fs::write(game.join("CARS/PEUGOT/EXTRA.BIN"), b"new").unwrap();
    std::fs::remove_file(game.join("CREDITS/UK_ENGLISH.TXT")).unwrap();
    let mut sc = scan(&game).unwrap();
    fingerprint(&mut sc, 2, &|_| {}).unwrap();
    let Verdict::Custom { differing } = Edition::lookup(&sc, &[ed]) else { panic!("known") };
    assert!(differing.contains(&("GLOBAL/GlobalB.lzc".into(), "MODIFIED")));
    assert!(differing.contains(&("CARS/PEUGOT/EXTRA.BIN".into(), "unknown")));
    assert!(differing.contains(&("CREDITS/UK_ENGLISH.TXT".into(), "missing")));
    let id = base_id(&Edition::lookup(&sc, &[]), &sc.identity().unwrap());
    assert!(id.starts_with("nfsu2-custom-") && id.len() == 21, "{id}");
}

#[test]
fn identity_ignores_order_and_case() {
    let f = |p: &str, d: u8| SourceFile { path: p.into(), size: 1, sha256: Some([d; 32]) };
    let a = Scan { root: PathBuf::new(), files: vec![f("A/x", 1), f("B/y", 2)], skipped: vec![], exe: None };
    let b = Scan { root: PathBuf::new(), files: vec![f("b/Y", 2), f("a/X", 1)], skipped: vec![], exe: None };
    assert_eq!(a.identity(), b.identity());
}

#[test]
fn short_space_stops_before_writing() {
    let s = Scratch::new("imp-space");
    let game = s.path().join("game");
    let base = s.path().join("base");
    make_tree(&game);
    let o = Options { free_space: |_| Some(1000), ..opts(vec![]) };
    let r = import(&game, &base, &o, &|_| {}).unwrap_err();
    assert_eq!(r.rule, Rule::Imp2);
    assert!(r.message.contains("needs") && r.message.contains("free on"), "{}", r.message);
    assert!(!base.exists(), "something was written");
}

#[test]
fn a_stopped_import_resumes_with_the_units_not_done() {
    let s = Scratch::new("imp-resume");
    let game = s.path().join("game");
    let base = s.path().join("base");
    make_tree(&game);
    let stop = AtomicBool::new(false);
    let o = Options { stop: Some(&stop), threads: 1, ..opts(vec![]) };
    let out = import(&game, &base, &o, &|l| {
        if l.starts_with("unit=") {
            stop.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    let Outcome::Stopped { id, units_done, units } = out else { panic!("{out:?}") };
    assert!(units_done >= 1 && units_done < units, "{units_done}/{units}");
    assert!(base.join(format!(".staging-{id}/journal.txt")).is_file());
    assert!(!base.join(&id).exists());
    // A torn last journal line (a stop mid-write) is ignored.
    let j = base.join(format!(".staging-{id}/journal.txt"));
    let mut t = std::fs::read_to_string(&j).unwrap();
    t += "unit cars-PEU";
    std::fs::write(&j, t).unwrap();
    let lines = Mutex::new(Vec::new());
    let out = import(&game, &base, &opts(vec![]), &|l| lines.lock().unwrap().push(l.to_string())).unwrap();
    assert!(matches!(out, Outcome::Done { fresh: true, .. }), "{out:?}");
    let lines = lines.into_inner().unwrap();
    assert!(lines.iter().any(|l| l.starts_with(&format!("resume={units_done} of {units}"))), "{lines:?}");
    let written = lines.iter().filter(|l| l.starts_with("unit=")).count();
    assert_eq!(written, units - units_done);
    assert!(verify(&base.join(&id), 2, &|_| {}).unwrap().is_empty());
}

#[test]
fn a_staging_folder_of_another_import_is_discarded() {
    let s = Scratch::new("imp-imp6");
    let game = s.path().join("game");
    let base = s.path().join("base");
    make_tree(&game);
    let mut sc = scan(&game).unwrap();
    fingerprint(&mut sc, 1, &|_| {}).unwrap();
    let id = base_id(&Edition::lookup(&sc, &[]), &sc.identity().unwrap());
    let staging = base.join(format!(".staging-{id}"));
    std::fs::create_dir_all(staging.join("bundles")).unwrap();
    std::fs::write(staging.join("journal.txt"), format!("{JOURNAL_HEADER}\nidentity=00 epoch=0 unit_version=0\nunit global 1 xxh3-128:00000000000000000000000000000000\n")).unwrap();
    std::fs::write(staging.join("bundles/stray.obz"), b"x").unwrap();
    let lines = Mutex::new(Vec::new());
    import(&game, &base, &opts(vec![]), &|l| lines.lock().unwrap().push(l.to_string())).unwrap();
    assert!(lines.into_inner().unwrap().iter().any(|l| l.contains("IMP.6")));
    assert!(!base.join(&id).join("bundles/stray.obz").exists());
}

#[test]
fn damage_is_found_and_repaired_from_the_game_folder() {
    let s = Scratch::new("imp-repair");
    let game = s.path().join("game");
    let base = s.path().join("base");
    make_tree(&game);
    let Outcome::Done { dir, .. } = import(&game, &base, &opts(vec![]), &|_| {}).unwrap() else { panic!() };
    let p = dir.join("bundles/cars-PEUGOT.obz");
    let good = std::fs::read(&p).unwrap();
    let mut bad = good.clone();
    let i = bad.iter().position(|&b| b == 1).unwrap();
    bad[i] = 2;
    std::fs::write(&p, &bad).unwrap();
    let d = verify(&dir, 2, &|_| {}).unwrap();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].0, "cars-PEUGOT");
    assert_eq!(d[0].1.rule, Rule::Lay3);
    let fixed = repair(&dir, &game, &opts(vec![]), &|_| {}).unwrap();
    assert_eq!(fixed, ["cars-PEUGOT"]);
    assert_eq!(std::fs::read(&p).unwrap(), good, "the rebuilt bundle differs");
    // A game folder with other files cannot repair the set.
    std::fs::write(&p, &bad).unwrap();
    std::fs::write(game.join("GLOBAL/GlobalB.lzc"), b"changed").unwrap();
    assert_eq!(repair(&dir, &game, &opts(vec![]), &|_| {}).unwrap_err().rule, Rule::Lay3);
}

#[test]
fn the_built_in_edition_is_the_reference_install() {
    let e = &editions()[0];
    assert_eq!(e.key, "pc12-eu");
    assert_eq!(e.files.len(), 1250);
    // The files the game's own fingerprint knows agree (crates/ob-core/src/fingerprint.rs).
    for (p, size, sha) in ob_core::fingerprint::EDITIONS[0].files.iter().skip(1) {
        let (s, d) = e.files[&p.to_ascii_uppercase()];
        assert_eq!((s, hex(&d)), (*size, sha.to_string()), "{p}");
    }
}

/// The reference install imports as `pc12-eu` (reads only the configured NFSU2_ROOT; writes to a scratch folder).
#[test]
#[ignore = "needs the game files (NFSU2_ROOT) and ~2 GB of scratch space"]
fn reference_install_imports() {
    let Some(root) = std::env::var_os("NFSU2_ROOT") else { return };
    let s = Scratch::new("imp-real");
    let t = std::time::Instant::now();
    let out = import(Path::new(&root), &s.path().join("base"), &Options::default(), &|l| {
        if l.starts_with("step=") || l.starts_with("edition=") {
            eprintln!("{l}");
        }
    })
    .unwrap();
    eprintln!("import: {:.1} s", t.elapsed().as_secs_f64());
    let Outcome::Done { id, dir, .. } = out else { panic!() };
    assert_eq!(id, "nfsu2-pc12-eu");
    assert!(verify(&dir, 4, &|_| {}).unwrap().is_empty());
}
