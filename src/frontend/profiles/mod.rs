//! Where the profiles live and which one is current: the original's
//! memory-card library on the PC (see `formats::native_save`).
//!
//! The game keeps each profile as `<root>\<NAME>\<NAME>` under
//! `%LOCALAPPDATA%\NFS Underground 2` (0x4A9C40: SHGetFolderPathA with
//! 0x801C, CSIDL_LOCAL_APPDATA | CSIDL_FLAG_CREATE, then the folder named
//! by 0x7F66A0 → "NFS Underground 2", made with CreateDirectoryA; the
//! library is given that root, 0x6FC01E, and the profiles are the root's
//! entries, FindFirstFileA "\*"). On Linux the clone uses the XDG data
//! directory, the local-application-data equivalent: `$XDG_DATA_HOME/NFS
//! Underground 2/<NAME>/<NAME>` (else `~/.local/share/…`), the very same
//! folder layout, so a profile folder copies between the two unchanged.
//!
//! Boot (**verified** in the original under Wine): with one profile the
//! game loads it ("Load complete."), with several it lists them (MC_List,
//! "Load") for the player to pick, with none it asks to create one.

#[cfg(test)]
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::formats::native_save::NativeSave;

mod current;
mod memcard_text;
mod store;

pub use current::*;
pub use memcard_text::*;
pub use store::*;

/// Load profile `name` from disk and make it current.
pub fn load(name: &str) -> Result<NativeSave> {
    let s = read(name)?;
    set_current(&resolve(name), s.clone());
    Ok(s)
}

/// What the boot does with the profiles on disk.
pub enum Boot {
    None,
    One(String),
    Pick(Vec<String>),
}

/// **Observed** under Wine (2026-11-01): with one profile and
/// 25 saved covers on disk the boot showed MC_List ("Load", the profile
/// alone listed) instead of loading the profile, so the boot counts the
/// memory-card files, covers included, while the list shows the profiles
/// only (**inferred**: the count is of the files; the list's filter is
/// not traced).
pub fn boot() -> Boot {
    let v = list();
    let files = dirs_in(&root()).iter().filter(|d| file_in_dir(&root(), d).is_some()).count();
    match v.len() {
        0 => Boot::None,
        1 if files <= 1 => Boot::One(v[0].clone()),
        _ => Boot::Pick(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test isolation (docs/test-determinism.md): two test threads in
    /// sandboxes have their own data folders and current profiles; a
    /// thread's folder goes with it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn sandboxes_keep_threads_apart() {
        let parent = std::env::temp_dir().join(format!("nfsu2-sandbox-test-{}", std::process::id()));
        let run = |name: &'static str| {
            let parent = parent.clone();
            std::thread::spawn(move || {
                let home = sandbox::enter(&parent);
                assert!(sandbox::active() && data_home() == home && root() == home.join(FOLDER));
                assert_eq!(current_name(), None, "a new sandbox has no current profile");
                set_current(name, NativeSave::fresh(name));
                std::thread::sleep(std::time::Duration::from_millis(20));
                update_current(|s| s.set_name(name));
                assert_eq!(sandbox::enter(&parent), home, "entered once per thread");
                (current_name(), home)
            })
        };
        let (a, b) = (run("AAA"), run("BBB"));
        let ((na, ha), (nb, hb)) = (a.join().unwrap(), b.join().unwrap());
        assert_eq!((na.as_deref(), nb.as_deref()), (Some("AAA"), Some("BBB")));
        assert_ne!(ha, hb);
        assert!(!ha.exists() && !hb.exists(), "removed as the threads end");
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn loc_table() {
        // A two-string table in the file's layout.
        let mut d = b"LOCH\x14\0\0\0\x01\0\0\0\x01\0\0\0\x5c\0\0\0".to_vec();
        let at = d.len();
        d.extend_from_slice(b"LOCL\0\0\0\0\0\0\0\0\x02\0\0\0");
        let first = 0x18u32;
        d.extend_from_slice(&first.to_le_bytes());
        d.extend_from_slice(&(first + 8).to_le_bytes());
        for s in ["Yes", "No"] {
            for c in s.encode_utf16().chain([0]) {
                d.extend_from_slice(&c.to_le_bytes());
            }
            while (d.len() - at) % 8 != 0 {
                d.push(0);
            }
        }
        assert_eq!(parse_loc(&d), vec!["Yes".to_string(), "No".to_string()]);
    }
}

/// A profile name usable as the folder and file name (the name editor
/// takes letters and digits, at most seven, 0x551280; a name from
/// elsewhere may hold other characters).
pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).take(31).collect();
    if s.is_empty() { "NAME".to_string() } else { s }
}

#[cfg(test)]
mod case_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn put(root: &Path, dir: &str, file: &str, age_s: u64) {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        let p = root.join(dir).join(file);
        std::fs::write(&p, b"x").unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_s)).unwrap();
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nfsu2-profiles-case-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn one_entry_per_name_the_newest_wins() {
        let r = tmp("newest");
        put(&r, "Kim", "Kim", 100);
        put(&r, "KIM", "KIM", 10);
        put(&r, "ray", "RAY", 5);
        put(&r, "Empty", "other", 1);
        assert_eq!(list_in(&r), ["KIM", "ray"]);
        for n in ["kim", "Kim", "KIM", "kIm"] {
            assert_eq!(resolve_in(&r, n), "KIM");
            assert_eq!(find_in(&r, n), r.join("KIM/KIM"));
        }
        // The file inside matched case-insensitively too.
        assert_eq!(find_in(&r, "RAY"), r.join("ray/RAY"));
        // The older one becomes newest: it wins; nothing was removed.
        put(&r, "Kim", "Kim", 0);
        assert_eq!(list_in(&r), ["Kim", "ray"]);
        assert!(r.join("KIM/KIM").is_file());
        // A new name: its own spelling; an empty folder of any case is reused.
        assert_eq!(resolve_in(&r, "Neu"), "Neu");
        assert_eq!(resolve_in(&r, "EMPTY"), "Empty");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn equal_times_prefer_the_exact_spelling() {
        let r = tmp("ties");
        put(&r, "abc", "abc", 50);
        put(&r, "ABC", "ABC", 50);
        let t = std::fs::metadata(r.join("abc/abc")).unwrap().modified().unwrap();
        std::fs::File::options().write(true).open(r.join("ABC/ABC")).unwrap().set_modified(t).unwrap();
        assert_eq!(resolve_in(&r, "abc"), "abc");
        assert_eq!(resolve_in(&r, "ABC"), "ABC");
        assert_eq!(resolve_in(&r, "Abc"), "ABC");
        assert_eq!(list_in(&r).len(), 1);
        let _ = std::fs::remove_dir_all(&r);
    }
}

/// unwrap audit: a missing, cut-off or foreign profile file and a
/// profiles folder that cannot be written are errors the callers report
/// ("Unable to load …" / SAVE_FAILED), never panics; a poisoned profile
/// lock is not one either.
#[cfg(test)]
mod failure_tests {
    use super::*;

    fn sandboxed(tag: &str) -> PathBuf {
        sandbox::enter(&std::env::temp_dir().join(format!("nfsu2-profiles-fail-{tag}-{}", std::process::id())))
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn missing_corrupt_and_cut_off_profiles_are_errors() {
        std::thread::spawn(|| {
            sandboxed("read");
            assert!(read("NOBODY").is_err(), "missing");
            assert!(load("NOBODY").is_err() && current_name().is_none(), "a failed load leaves no current profile");
            let good = NativeSave::fresh("GOOD");
            write("GOOD", &good).unwrap();
            assert_eq!(read("GOOD").unwrap(), good);
            let path = find("GOOD");
            let bytes = std::fs::read(&path).unwrap();
            for (what, bad) in [
                ("empty", Vec::new()),
                ("cut off", bytes[..bytes.len() / 2].to_vec()),
                ("one byte", bytes[..1].to_vec()),
                ("text", b"not a save at all".to_vec()),
            ] {
                std::fs::write(&path, &bad).unwrap();
                assert!(read("GOOD").is_err(), "{what}");
                assert!(read_cover("GOOD").is_err(), "{what} as a cover");
            }
            // A folder where the file should be.
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir_all(&path).unwrap();
            assert!(read("GOOD").is_err());
        })
        .join()
        .unwrap();
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn an_unwritable_profiles_folder_is_an_error() {
        std::thread::spawn(|| {
            let home = sandboxed("write");
            // A file where the profiles' folder goes: nothing can be made
            // under it (read-only in effect, for any user).
            std::fs::write(home.join(FOLDER), b"in the way").unwrap();
            assert!(write("BLOCKED", &NativeSave::fresh("BLOCKED")).is_err());
            assert!(write_cover("BLOCKED DVD 1", &crate::formats::native_save::picture(&[0; 4], 1, 1)).is_err());
            assert!(delete("BLOCKED").is_err());
            assert!(list().is_empty());
            assert_eq!(std::fs::read(home.join(FOLDER)).unwrap(), b"in the way", "left as it was");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn a_poisoned_profile_lock_keeps_working() {
        // Outside a sandbox: the process-wide profile. Its value is not
        // changed here (other tests may read it), only the lock poisoned.
        let poisoned = std::thread::spawn(|| {
            let _held = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            panic!("a panic while the profile is held (expected by this test)");
        })
        .join();
        assert!(poisoned.is_err() && CURRENT.is_poisoned());
        std::thread::spawn(|| {
            let _ = current_name();
            let _ = current_save();
            update_current(|_| {});
        })
        .join()
        .expect("the profile is still readable");
    }
}
