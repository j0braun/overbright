//! The settings store's tests that need the library: the game-folder setup and the
//! front end's settings page (the store itself is `ob_settings::qol`).

use super::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("nfsu2-qol-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn setup_check() {
    let d = scratch("setup");
    let c = setup::check(&d, true);
    assert_eq!(c.missing.len(), setup::KEY_FILES.len());
    assert!(!c.ok() && !setup::looks_like_game(&d));
    assert!(setup::help(Some(&d), Some(&c)).contains("GLOBAL/GLOBALB.BUN"));
    // A missing sky pack is no change; a replaced one (an HD sky mod) is one (#178).
    assert!(!c.different.iter().any(|f| f == "TRACKS/LOC4DYNTEX.BIN"), "{c:?}");
    std::fs::create_dir_all(d.join("tracks")).unwrap();
    std::fs::write(d.join("tracks/loc4dyntex.bin"), b"an HD sky").unwrap();
    for hash in [false, true] {
        let c = setup::check(&d, hash);
        assert!(c.different.iter().any(|f| f == "TRACKS/LOC4DYNTEX.BIN"), "{c:?}");
    }
    if let Some(root) = std::env::var_os("NFSU2_ROOT").map(std::path::PathBuf::from) {
        let c = setup::check(&root, true);
        assert!(c.ok() && c.different.is_empty(), "{c:?}");
        assert!(setup::looks_like_game(&root));
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// The text language: the kept choice (`language.txt`, in a sandboxed
/// data folder), what the engine can show, a copy without English, and
/// the file a check requires for it.
#[test]
fn text_language_choice_and_check() {
    let d = scratch("language");
    crate::frontend::profiles::sandbox::enter(&d);
    std::fs::create_dir_all(d.join("LANGUAGES")).unwrap();
    for f in ["English.bin", "German.bin", "Korean.bin", "Labels.bin"] {
        std::fs::write(d.join("LANGUAGES").join(f), b"x").unwrap();
    }
    // Nothing kept: English. `--language` (the request) names a file in
    // any case.
    assert_eq!(setup::saved_language(), None);
    assert_eq!(setup::choose_language(&d, None), "English");
    assert_eq!(setup::choose_language(&d, Some("german")), "German");
    // The kept choice is read, and a name that cannot sit in a path is not.
    setup::save_language(Some("German")).unwrap();
    assert!(setup::saved_language_file().starts_with(&d));
    assert_eq!(setup::saved_language().as_deref(), Some("German"));
    assert_eq!(setup::choose_language(&d, None), "German");
    assert_eq!(setup::choose_language(&d, Some("English")), "English", "the request beats the kept choice");
    std::fs::write(setup::saved_language_file(), "../x\n").unwrap();
    assert_eq!(setup::saved_language(), None);
    // A language the engine cannot show, or the copy lacks: English.
    assert_eq!(setup::choose_language(&d, Some("Korean")), "English");
    assert_eq!(setup::choose_language(&d, Some("Dutch")), "English");
    setup::save_language(None).unwrap();
    setup::save_language(None).unwrap();
    assert_eq!(setup::saved_language(), None);
    // The check requires the chosen language's file in English's place;
    // without known size or checksum it is "not verified", not an error.
    let c = setup::check_for(&d, false, "German");
    assert_eq!(c.unverified, ["LANGUAGES/German.bin"]);
    assert!(!c.missing.iter().chain(&c.different).any(|m| m.contains("German") || m.contains("English")));
    assert!(c.key_files().iter().any(|f| f == "LANGUAGES/German.bin"));
    assert!(!c.key_files().iter().any(|f| f == "LANGUAGES/English.bin"));
    assert_eq!(c.verdict_skip(), Some("LANGUAGES/English.bin"));
    let lines = setup::check_lines(&c, None);
    let note = |(m, t): &(setup::Mark, String)| *m == setup::Mark::Info && t.contains("German.bin: not verified");
    assert!(lines.iter().any(note), "{lines:?}");
    // A file the copy lacks is missing, as English's is.
    let c = setup::check_for(&d, false, "French");
    assert!(c.missing.iter().any(|m| m == "LANGUAGES/French.bin") && c.unverified.is_empty());
    let c = setup::check_for(&d, false, "English");
    assert!(c.unverified.is_empty() && c.verdict_skip().is_none());
    assert_eq!(c.different, ["LANGUAGES/English.bin"], "the dummy file differs from the known release");
    // A copy without English.bin plays its first supported language;
    // with only one the engine cannot show, English (and the check
    // says the file is missing).
    std::fs::remove_file(d.join("LANGUAGES/English.bin")).unwrap();
    assert_eq!(setup::choose_language(&d, None), "German");
    std::fs::remove_file(d.join("LANGUAGES/German.bin")).unwrap();
    assert_eq!(setup::choose_language(&d, None), "English");
    std::fs::remove_dir_all(&d).unwrap();
}

/// L1: the language list of a LANGUAGES folder (any case; the shared
/// tables left out), SPEED2.EXE not needed, the shared check
/// wording, and the kept texture-pack folder (in a sandboxed data
/// folder).
#[test]
fn launcher_prerequisites() {
    let d = scratch("launcher-l1");
    std::fs::create_dir_all(d.join("languages")).unwrap();
    for f in
        ["English.bin", "french.BIN", "Labels.bin", "Largest.bin", "LanguageTextures.bin", "agree.eng", "German.bin"]
    {
        std::fs::write(d.join("languages").join(f), b"x").unwrap();
    }
    assert_eq!(setup::installed_languages(&d), ["English", "French", "German"]);
    assert!(setup::installed_languages(&d.join("nope")).is_empty());
    let c = setup::check_for(&d, false, "English");
    assert!(!c.missing.iter().chain(&c.different).any(|m| m == "SPEED2.EXE"));
    assert!(!setup::KEY_FILES.iter().any(|(f, ..)| f.eq_ignore_ascii_case("SPEED2.EXE")));
    // The wording: one line per missing file, the differing ones named.
    let lines = setup::check_lines(&c, None);
    assert_eq!(lines.iter().filter(|(m, _)| *m == setup::Mark::Error).count(), 1, "many missing: one line");
    assert!(lines.iter().any(|(_, t)| t.contains(&format!(
        "{} of the {} files",
        c.missing.len(),
        setup::KEY_FILES.len()
    ))));
    let few = setup::Check { missing: vec!["TRACKS/L4RA.BUN".into()], ..Default::default() };
    assert_eq!(
        setup::check_lines(&few, None),
        [(setup::Mark::Error, "Missing: TRACKS/L4RA.BUN — reinstall or copy the full game folder".to_string())]
    );
    // (The folder check finds every key file in any case, so the dummy `languages/English.bin` differs too.)
    assert!(
        lines.iter().any(|(m, t)| *m == setup::Mark::Warn && t.contains("1 file differs") && t.contains("English.bin"))
    );
    let fine = setup::Check::default();
    let lines = setup::check_lines(&fine, None);
    assert_eq!(lines[0].0, setup::Mark::Ok);
    assert!(
        lines.iter().any(|(_, t)| t == &format!("All {} files Overbright needs are present", setup::KEY_FILES.len()))
    );
    crate::frontend::profiles::sandbox::enter(&d);
    assert_eq!(setup::saved_texture_pack(), None);
    setup::save_texture_pack(Some(&d.join("pack"))).unwrap();
    assert_eq!(setup::saved_texture_pack(), Some(d.join("pack")));
    assert!(setup::saved_texture_pack_file().starts_with(&d));
    setup::save_texture_pack(None).unwrap();
    setup::save_texture_pack(None).unwrap();
    assert_eq!(setup::saved_texture_pack(), None);
    std::fs::remove_dir_all(&d).unwrap();
}

/// L1: `launcher.skip` is a declared row: read, written (off writes
/// nothing), kept by the presets and by Restore Native, and kept by a
/// rewrite of options.txt.
#[test]
fn launcher_skip_row() {
    let _g = test_globals();
    let mut e = Enhancements::default();
    assert!(!e.launcher_skip);
    assert!(e.parse("launcher.skip", "1") && e.launcher_skip);
    let mut out = String::new();
    e.write(&mut out);
    assert_eq!(out, "launcher.skip 1\n");
    assert!(e.deviations().is_empty(), "not a deviation of the game");
    for p in PRESETS {
        let mut x = e;
        p.apply_for(GpuClass::High, &mut x);
        assert!(x.launcher_skip, "{p:?}");
    }
    let mut s = crate::frontend::options::Settings::from_text("resolution 1280x960\nlauncher.skip 1\n");
    assert!(s.enh.launcher_skip);
    let text = s.options_text();
    assert!(text.lines().any(|l| l == "launcher.skip 1"), "{text}");
    assert!(crate::frontend::options::Settings::from_text(&text).enh.launcher_skip);
    s.restore_native();
    assert!(s.enh.launcher_skip);
}

/// options.txt's text for `e`.
fn new_text(e: &Enhancements) -> String {
    let mut out = String::new();
    e.write(&mut out);
    out
}

/// options.txt's line split (`Settings::from_text`) through the parser.
fn read(text: &str) -> (Enhancements, Vec<bool>) {
    let mut e = Enhancements::default();
    let mut known = Vec::new();
    for line in text.lines() {
        let (k, v) = line.split_once(' ').unwrap_or((line, ""));
        known.push(e.parse(k, v));
    }
    (e, known)
}

fn all_keys() -> Vec<&'static str> {
    let mut k: Vec<&'static str> = OPTION_ROWS.iter().filter_map(|r| r.key).collect();
    k.extend(NESTED_KEYS);
    k
}

/// Every value the settings page can set (each Graphics row's tiers, each
/// QoL row stepped both ways) is written under a key of the table (or of a
/// nested group) and read back to the same value. Lists the keys no row
/// sets.
#[test]
fn every_ui_value_is_kept_by_the_table() {
    use crate::frontend::enh::graphics_table;
    use crate::frontend::options::{RowId, Settings, change};
    let mut seen = std::collections::BTreeSet::new();
    let mut check = |s: &Settings, what: &str| {
        let text = new_text(&s.enh);
        let (back, known) = read(&text);
        assert_eq!(back, s.enh, "{what}: {text}");
        assert!(known.iter().all(|&k| k), "{what}");
        for l in text.lines() {
            seen.insert(l.split_once(' ').unwrap().0.to_string());
        }
    };
    for r in graphics_table::ROWS.iter() {
        for i in 0..r.tiers.len() {
            let mut s = Settings::default();
            (r.set)(&mut s, i);
            check(&s, &format!("{:?} tier {i}", r.id));
        }
    }
    let qol = [
        RowId::FovOffset,
        RowId::HudScale,
        RowId::HudLayout,
        RowId::FrameCap,
        RowId::DurableSaves,
        RowId::SaveBackups,
        RowId::CrashReports,
        RowId::ListsOpenInstalled,
        RowId::OutrunVariety,
        RowId::FixModels,
        RowId::TrafficDensity,
        RowId::SkipMovies,
        RowId::SkipTitle,
        RowId::PhotoMode,
        RowId::LowLatency,
        RowId::MarkSpot,
        RowId::Background,
        RowId::PowerSaving,
        RowId::ReplayCameras,
        RowId::HudPalette,
        RowId::HudMinimap,
        RowId::HudSpeedo,
        RowId::HudInfo,
        RowId::ReducedFlashing,
        RowId::PadLayout,
        RowId::PadDeadzone,
        RowId::PadTriggerDeadzone,
        RowId::PadSaturation,
        RowId::PadSteerCurve,
        RowId::PadRumble,
        RowId::PadPauseOnDisconnect,
        RowId::PadSafeBind,
        RowId::PadSticksQuit,
        RowId::HdUi,
        RowId::ReloadPacks,
    ];
    for id in qol {
        for right in [true, false] {
            let mut s = Settings::default();
            for n in 0..24 {
                change(id, &mut s, 0, right, false);
                check(&s, &format!("{id:?} step {n} {right}"));
            }
        }
    }
    let unset: Vec<_> = all_keys().into_iter().filter(|k| !seen.contains(*k)).collect();
    // enh.anisotropy and enh.reflection_rate are removed clone rows: their
    // Customize Display rows (Texture Filtering, Car Reflection Update
    // Rate) read them and put them to 0 on any change (graphics_table);
    // boot_loading_min_ms has no row (options.txt only); launcher.skip is
    // the launcher's (L1).
    assert_eq!(
        unset,
        ["enh.anisotropy", "enh.reflection_rate", "enh.boot_loading_min_ms", "launcher.skip"],
        "keys no settings-page row sets"
    );
}
