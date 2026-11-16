//! Profile names are case-insensitive, as on the original's Windows file
//! system: the public API in a data home of its own (XDG_DATA_HOME set in
//! this test binary's only test, so it never sees the player's saves).

use nfsu2::formats::native_save::NativeSave;
use nfsu2::frontend::profiles;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn profiles_resolve_case_insensitively() {
    let home = std::env::temp_dir().join(format!("nfsu2-profiles-case-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    // SAFETY: the only test in this binary, before anything reads it.
    unsafe { std::env::set_var("XDG_DATA_HOME", &home) };
    assert!(profiles::root().starts_with(&home));

    // Create "Kim", then save as "KIM": one folder, one file.
    profiles::write("Kim", &NativeSave::fresh("Kim")).unwrap();
    assert!(profiles::exists("KIM") && profiles::exists("kim"));
    profiles::write("KIM", &NativeSave::fresh("KIM")).unwrap();
    assert_eq!(profiles::list(), ["Kim"]);
    assert!(!profiles::root().join("KIM").exists());
    // The payload keeps the name it was saved under (the help bar's).
    assert_eq!(profiles::read("kim").unwrap().name(), "KIM");
    // Loading by any case makes the folder's spelling current.
    profiles::load("kIm").unwrap();
    assert_eq!(profiles::current_name().as_deref(), Some("Kim"));

    // A duplicate made outside (Linux only): one entry, the newest file.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::create_dir_all(profiles::root().join("KIM")).unwrap();
    std::fs::write(profiles::root().join("KIM/KIM"), NativeSave::fresh("Dup").to_bytes()).unwrap();
    assert_eq!(profiles::list(), ["KIM"]);
    assert_eq!(profiles::read("Kim").unwrap().name(), "Dup");
    // Delete by any case removes only that one; the other is listed again.
    profiles::delete("kim").unwrap();
    assert!(!profiles::root().join("KIM").exists());
    assert_eq!(profiles::list(), ["Kim"]);
    profiles::delete("KIM").unwrap();
    assert!(profiles::list().is_empty() && profiles::current_name().is_none());
    let _ = std::fs::remove_dir_all(&home);
}
