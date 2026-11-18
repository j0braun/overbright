//! Portable mode end to end (docs/platform-dirs.md, "Portable mode"): the
//! game binary, linked into a program folder with `portable.txt` beside
//! it, run under a fake home (HOME, XDG unset, LOCALAPPDATA, APPDATA,
//! TMPDIR all inside the test's folder): everything it writes lands in
//! `<program>/data`, nothing in the fake home or anywhere else in the
//! test's folder. Without the marker the same run keeps using the home
//! folders, unchanged. No display, no window, no sound.

#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The test's folder: `prog/` (the game binary, hard-linked, else copied),
/// `home/`, `appdata/`, `tmp/` and `cwd/` (the current folder of the run).
/// Under the target folder, so the hard link works; removed when dropped.
struct Install(PathBuf);

impl Install {
    fn new(tag: &str, portable: bool) -> Install {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("portable-{tag}-{}", nfsu2::unique_tag()));
        for d in ["prog", "home", "appdata", "tmp", "cwd"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        for (name, src) in
            [("game", env!("CARGO_BIN_EXE_game")), ("overbright-editor", env!("CARGO_BIN_EXE_overbright-editor"))]
        {
            let exe = base.join("prog").join(name);
            if std::fs::hard_link(src, &exe).is_err() {
                std::fs::copy(src, &exe).unwrap();
            }
        }
        if portable {
            std::fs::write(base.join("prog/portable.txt"), "").unwrap();
        }
        Install(base)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// The game with `args`, in `cwd` (default: the test's `cwd/`), with
    /// nothing of this machine's environment but PATH.
    fn run(&self, args: &[&str], cwd: Option<&Path>) -> Output {
        self.run_program("game", args, cwd, &[])
    }

    /// The program `name` beside the game (`overbright-editor`) the same
    /// way, with the extra variables `vars`.
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
            .expect("run the game")
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
                f != "prog/game"
                    && f != "prog/overbright-editor"
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

/// A plain start that finds no game: it fails with the player's message,
/// and whatever it wrote (its log file) is in the portable data folder.
#[test]
fn portable_start_without_a_game_writes_only_beside_the_program() {
    let i = Install::new("nogame", true);
    let out = i.run(&[], None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && !stderr.contains("panicked"), "{stderr}");
    assert!(stderr.contains("could not find your copy"), "{stderr}");
    i.assert_only_data("a start without a game");
    for f in i.files("prog/data") {
        assert!(f == "prog/data/logs" || f.starts_with("prog/data/logs/game-"), "only the log: {f}");
    }
}

/// `--set-root` in a portable install keeps the game folder in
/// `data/game-root.txt`, relative to the program folder when both are on
/// one file system; a later start from another folder finds it there.
/// Without the marker the folder is kept as before: the absolute path in
/// `~/.local/share/overbright/game-root.txt`.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn portable_install_keeps_the_game_folder_relative() {
    let root = nfsu2::platform::paths::absolute(&common::game_root());
    let root_s = root.to_str().expect("a UTF-8 game folder path");
    let i = Install::new("root", true);
    let out = i.run(&["--set-root", root_s], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let kept = std::fs::read_to_string(i.path("prog/data/game-root.txt")).expect("game-root.txt in data/");
    let kept = kept.trim_end_matches('\n');
    use std::os::unix::fs::MetadataExt;
    let same_fs = std::fs::metadata(i.path("prog")).unwrap().dev() == std::fs::metadata(&root).unwrap().dev();
    if same_fs {
        assert!(Path::new(kept).is_relative(), "kept relative: {kept}");
        assert_eq!(nfsu2::platform::dirs::join_relative(&i.path("prog"), Path::new(kept)), root, "{kept}");
    } else {
        assert_eq!(kept, root_s, "another file system: kept absolute");
    }
    // Found again from another current folder.
    let out = i.run(&["--check-game"], Some(Path::new("/")));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout.lines().next(), Some(format!("game folder: {}", root.display()).as_str()), "{stdout}");
    i.assert_only_data("--set-root and --check-game");

    // The default, unchanged: the home folder, the absolute path.
    let d = Install::new("default", false);
    let out = d.run(&["--set-root", root_s], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let kept = std::fs::read(d.path("home/.local/share/overbright/game-root.txt")).expect("the default place");
    assert_eq!(kept, format!("{root_s}\n").into_bytes());
    assert!(!d.path("prog/data").exists());
}

/// A folder put elsewhere (the locations file, Settings › Advanced ›
/// Storage) is where the game writes: its log goes into the chosen logs
/// folder, not the default one; in a portable install the locations file
/// sits in `data/` and a relative path is taken from the program folder.
#[test]
fn the_game_writes_its_log_where_the_locations_file_says() {
    let i = Install::new("loc-home", false);
    std::fs::create_dir_all(i.path("home/.local/share/overbright")).unwrap();
    std::fs::write(
        i.path("home/.local/share/overbright/locations.txt"),
        format!("logs {}\n", i.path("elsewhere/logs").display()),
    )
    .unwrap();
    let out = i.run(&[], None);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    let logs = i.files("elsewhere");
    assert!(logs.iter().any(|f| f.starts_with("elsewhere/logs/game-")), "the log in the chosen folder: {logs:?}");
    assert!(
        !i.files("home").iter().any(|f| f.contains("/logs/")),
        "nothing in the default logs folder: {:?}",
        i.files("home")
    );

    let p = Install::new("loc-portable", true);
    std::fs::create_dir_all(p.path("prog/data")).unwrap();
    std::fs::write(p.path("prog/data/locations.txt"), "logs ../portable-logs\n").unwrap();
    let out = p.run(&[], None);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    let logs = p.files("portable-logs");
    assert!(logs.iter().any(|f| f.starts_with("portable-logs/game-")), "relative to the program folder: {logs:?}");
    assert!(!p.files("prog/data").iter().any(|f| f.starts_with("prog/data/logs")), "{:?}", p.files("prog/data"));
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
