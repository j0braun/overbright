//! Portable mode end to end for the map editor (docs/playing/platform-dirs.md,
//! "Portable mode"; the game's side is the root package's
//! tests/portable_mode.rs): the editor binary, linked into a program folder
//! with `portable.txt` beside it, run under a fake home (HOME, XDG unset,
//! LOCALAPPDATA, APPDATA, TMPDIR all inside the test's folder): everything
//! it writes lands in `<program>/data`, nothing in the fake home or
//! anywhere else in the test's folder. Without the marker the same run
//! keeps using the home folders, unchanged. No display, no window, no sound.

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The test's folder: `prog/` (the editor binary, hard-linked, else copied),
/// `home/`, `appdata/`, `tmp/` and `cwd/` (the current folder of the run).
/// Under the target folder, so the hard link works; removed when dropped.
struct Install(PathBuf);

impl Install {
    fn new(tag: &str, portable: bool) -> Install {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("portable-{tag}-{}", nfsu2::unique_tag()));
        for d in ["prog", "home", "appdata", "tmp", "cwd"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        let exe = base.join("prog/overbright-editor");
        if std::fs::hard_link(env!("CARGO_BIN_EXE_overbright-editor"), &exe).is_err() {
            std::fs::copy(env!("CARGO_BIN_EXE_overbright-editor"), &exe).unwrap();
        }
        if portable {
            std::fs::write(base.join("prog/portable.txt"), "").unwrap();
        }
        Install(base)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// The program `name` (`overbright-editor`) with `args`, in `cwd`
    /// (default: the test's `cwd/`), with nothing of this machine's
    /// environment but PATH and the extra variables `vars`.
    fn run_program(&self, name: &str, args: &[&str], cwd: Option<&Path>, vars: &[(&str, &Path)]) -> Output {
        Command::new(self.path("prog").join(name))
            .args(args)
            .current_dir(cwd.map(Path::to_path_buf).unwrap_or_else(|| self.path("cwd")))
            .env_clear()
            .envs(vars.iter().map(|(k, v)| (*k, *v)))
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.path("home"))
            .env("LOCALAPPDATA", self.path("appdata"))
            .env("APPDATA", self.path("appdata"))
            .env("USERPROFILE", self.path("home"))
            .env("TMPDIR", self.path("tmp"))
            .stdin(Stdio::null())
            .output()
            .expect("run the program")
    }

    /// Every file and folder under `rel`, relative to the test's folder.
    fn files(&self, rel: &str) -> Vec<String> {
        fn walk(p: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
                out.push(e.path());
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    walk(&e.path(), out);
                }
            }
        }
        let mut v = Vec::new();
        walk(&self.path(rel), &mut v);
        let mut v: Vec<String> = v.iter().map(|p| p.strip_prefix(&self.0).unwrap().display().to_string()).collect();
        v.sort();
        v
    }

    /// Nothing written outside `prog/data` (the binary and the marker are
    /// the test's own).
    fn assert_only_data(&self, what: &str) {
        for d in ["home", "appdata", "tmp", "cwd"] {
            assert_eq!(self.files(d), Vec::<String>::new(), "{what}: written outside the portable folder");
        }
        let stray: Vec<String> = self
            .files("prog")
            .into_iter()
            .filter(|f| {
                f != "prog/overbright-editor"
                    && f != "prog/portable.txt"
                    && f != "prog/data"
                    && !f.starts_with("prog/data/")
            })
            .collect();
        assert!(stray.is_empty(), "{what}: written beside the program, not in data/: {stray:?}");
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The map editor in a portable install follows the same rules as the
/// game: it finds the game folder kept in `data/game-root.txt`, opens the
/// map mod in `data/maps`, keeps its recent list in `data/` and the
/// driver's shader caches in `data/cache/driver`; nothing is written in
/// the fake home or anywhere else. Without the marker the same run uses
/// the home folders. Drawn off-screen (`--shot`): no window.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_map_editor_writes_only_beside_the_program_when_portable() {
    let root = nfsu2::platform::paths::absolute(&common::game_root());
    // The asset cache: the tests' own (not the install's), so the run
    // does not rebuild it.
    let cache = nfsu2::platform::dirs::test_cache_folder();
    let map = "overbright-map 1\nname = Portable test\n";
    for portable in [true, false] {
        let i = Install::new(if portable { "editor" } else { "editor-home" }, portable);
        let data = if portable { i.path("prog/data") } else { i.path("home/.local/share/overbright") };
        std::fs::create_dir_all(data.join("maps/portable-test")).unwrap();
        std::fs::write(data.join("maps/portable-test/map.obmap"), map).unwrap();
        std::fs::write(data.join("game-root.txt"), format!("{}\n", root.display())).unwrap();
        let shot = data.join("shot.png");
        let shot_s = shot.to_str().expect("a UTF-8 test path");
        let out = i.run_program(
            "overbright-editor",
            &["--shot", shot_s, "--size", "320x240", "--mod", "portable-test"],
            None,
            &[("NFSU2_CACHE_DIR", &cache)],
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{stderr}");
        assert!(shot.is_file(), "the picture: {stderr}");
        assert_eq!(
            std::fs::read_to_string(data.join("editor-recent.txt")).expect("the recent list in Overbright's folder"),
            "portable-test\n"
        );
        if portable {
            i.assert_only_data("the map editor");
        } else {
            assert!(!i.path("prog/data").exists(), "no data folder without the marker");
        }
    }
}
