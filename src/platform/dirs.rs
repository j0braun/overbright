//! Where Overbright keeps its files, per OS (docs/platform-dirs.md). One
//! place builds every such path; [`resolve`] is a pure function of the
//! environment so each OS's rules are tested on any OS.
//!
//! Linux and other Unix:
//! * data home (the profiles' parent, the original's `%LOCALAPPDATA%`):
//!   `$XDG_DATA_HOME`, else `$HOME/.local/share`, else `.`;
//! * Overbright's folder: `<data home>/overbright` ([`APP`]; options.txt,
//!   career.txt, game-root.txt, crashes/, logs/, marks/, perf/,
//!   Screenshots/, texture-pack/);
//! * cache: `$XDG_CACHE_HOME/overbright` (set and not empty), else
//!   `$HOME/.cache/overbright`.
//!
//! Windows: Overbright keeps a folder of its own and never writes into the
//! original's save folder (`%LOCALAPPDATA%\NFS Underground 2`, 0x4A9C40):
//! * data home and Overbright's folder: `<Local AppData>\Overbright`
//!   (FOLDERID_LocalAppData through SHGetKnownFolderPath, `%LOCALAPPDATA%`
//!   when that fails), so the profiles are
//!   `<Local AppData>\Overbright\NFS Underground 2\<NAME>\<NAME>`;
//! * cache: `<Local AppData>\Overbright\cache`.
//! * `XDG_DATA_HOME` / `XDG_CACHE_HOME` (set, not empty) are honoured as on
//!   Linux, with Linux's layout: the developer harness isolates its runs
//!   through them.
//!
//! Every OS: `NFSU2_DATA_DIR` (set, not empty) puts everything in that one
//! folder — the profiles' folder, Overbright's files and `cache/` — a
//! portable install. `NFSU2_CACHE_DIR` / `NFSU2_NO_CACHE` still decide the
//! cache over it (`cache::dir`).

use std::ffi::OsString;
use std::path::PathBuf;

/// Overbright's folder inside a shared, XDG-style data or cache folder.
pub const APP: &str = "overbright";
/// Overbright's own folder under Windows' Local AppData.
pub const WINDOWS_APP: &str = "Overbright";
/// The explicit override: everything in this one folder.
pub const DATA_DIR_VAR: &str = "NFSU2_DATA_DIR";

/// Which rules [`resolve`] applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// Linux and every other non-Windows system.
    Unix,
    Windows,
}

impl Os {
    /// The OS this build runs on.
    pub const CURRENT: Os = if cfg!(windows) { Os::Windows } else { Os::Unix };
}

/// The folders [`resolve`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The profiles' parent: `<data home>/NFS Underground 2` holds them
    /// (`frontend::profiles`), `<data home>/NFS Underground 2 Backups` the
    /// backups.
    pub data_home: PathBuf,
    /// Overbright's own folder (options.txt, career.txt, game-root.txt and
    /// the subfolders of [`crashes`], [`logs`], …).
    pub app: PathBuf,
    /// The asset cache's folder, or None when there is no place for it.
    pub cache: Option<PathBuf>,
}

/// The folders for `os`, from the environment `var` and, on Windows, the
/// Local AppData known folder (`local_app_data`; None falls back to
/// `%LOCALAPPDATA%`). Pure: no file-system access.
pub fn resolve(os: Os, var: &dyn Fn(&str) -> Option<OsString>, local_app_data: Option<PathBuf>) -> Layout {
    let name = APP;
    let non_empty = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = non_empty(DATA_DIR_VAR) {
        return Layout { data_home: dir.clone(), app: dir.clone(), cache: Some(dir.join("cache")) };
    }
    let xdg_cache = non_empty("XDG_CACHE_HOME").map(|d| d.join(name));
    match os {
        Os::Unix => {
            let home = var("HOME").map(PathBuf::from);
            // XDG_DATA_HOME as before: set (even empty) wins.
            let data_home = var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(".local/share")))
                .unwrap_or_else(|| PathBuf::from("."));
            let app = data_home.join(name);
            let cache = xdg_cache.or_else(|| home.map(|h| h.join(".cache").join(name)));
            Layout { data_home, app, cache }
        }
        Os::Windows => {
            let own = local_app_data
                .filter(|p| !p.as_os_str().is_empty())
                .or_else(|| non_empty("LOCALAPPDATA"))
                .map(|l| l.join(WINDOWS_APP));
            let (data_home, app) = match (non_empty("XDG_DATA_HOME"), &own) {
                (Some(x), _) => (x.clone(), x.join(name)),
                (None, Some(o)) => (o.clone(), o.clone()),
                (None, None) => (PathBuf::from("."), PathBuf::from(".").join(name)),
            };
            let cache = xdg_cache.or_else(|| own.map(|o| o.join("cache")));
            Layout { data_home, app, cache }
        }
    }
}

/// The folders for this process (the environment now).
pub fn layout() -> Layout {
    #[cfg(windows)]
    let known = super::win::local_app_data();
    #[cfg(not(windows))]
    let known = None;
    resolve(Os::CURRENT, &|name: &str| std::env::var_os(name), known)
}

/// Test isolation: a thread's own data home (`frontend::profiles::sandbox`).
#[cfg(any(test, feature = "dev-harness"))]
mod thread_override {
    use std::cell::RefCell;
    use std::path::PathBuf;

    thread_local! {
        static DATA_HOME: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    pub(super) fn get() -> Option<PathBuf> {
        DATA_HOME.with_borrow(Clone::clone)
    }

    pub(super) fn set(dir: PathBuf) {
        DATA_HOME.with_borrow_mut(|d| *d = Some(dir));
    }
}

/// Tests: the calling thread's data home is `dir` from now on (Overbright's
/// folder `dir/overbright`, as with `XDG_DATA_HOME=dir`).
#[cfg(any(test, feature = "dev-harness"))]
#[doc(hidden)]
pub fn set_thread_data_home(dir: PathBuf) {
    thread_override::set(dir);
}

fn thread_data_home() -> Option<PathBuf> {
    #[cfg(any(test, feature = "dev-harness"))]
    return thread_override::get();
    #[cfg(not(any(test, feature = "dev-harness")))]
    None
}

/// The profiles' parent ([`Layout::data_home`]).
pub fn data_home() -> PathBuf {
    thread_data_home().unwrap_or_else(|| layout().data_home)
}

/// Overbright's own folder ([`Layout::app`]).
pub fn app() -> PathBuf {
    match thread_data_home() {
        Some(d) => d.join(APP),
        None => layout().app,
    }
}

/// The settings files' folder: options.txt, career.txt, game-root.txt.
pub fn config() -> PathBuf {
    app()
}

/// Crash reports (`qol::crash`).
pub fn crashes() -> PathBuf {
    app().join("crashes")
}

/// The game's log files (`qol::log_file`).
pub fn logs() -> PathBuf {
    app().join("logs")
}

/// Mark This Spot reports (`mark_spot`).
pub fn marks() -> PathBuf {
    app().join("marks")
}

/// The F3 overlay's performance CSVs (`overlay::csv`).
pub fn perf() -> PathBuf {
    app().join("perf")
}

/// Photo Mode's pictures.
pub fn screenshots() -> PathBuf {
    app().join("Screenshots")
}

/// The default texture-pack folder (`render::enh::assets::pack_select`).
pub fn packs() -> PathBuf {
    app().join("texture-pack")
}

/// The asset cache's default folder; `cache::dir` puts its own overrides
/// (`NFSU2_NO_CACHE`, `NFSU2_CACHE_DIR`) first. The player's folder
/// ([`Layout::cache`]) only for the shipping game: a build with the
/// developer harness (`dev-harness`: shots, flows, the selftest, benches,
/// the playthrough; the probes) and a program that called
/// [`developer_program`] (the tools and examples that load the game) get
/// the developer cache ([`developer_cache`]); tests the folder they were
/// given ([`use_test_cache`]), and every cargo test binary (run from a
/// `deps` folder) of a harness build [`test_cache_folder`]. So a developer
/// run never writes into the player's cache, whatever its environment.
pub fn cache() -> Option<PathBuf> {
    if let Some(t) = TEST_CACHE.get() {
        return Some(t.clone());
    }
    if cfg!(test) {
        return Some(test_cache_folder());
    }
    if cfg!(feature = "dev-harness") {
        if in_test_binary() {
            return Some(test_cache_folder());
        }
        return developer_cache();
    }
    if DEVELOPER.load(std::sync::atomic::Ordering::Relaxed) { developer_cache() } else { layout().cache }
}

/// Set by [`developer_program`].
static DEVELOPER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A developer tool says so first thing: its asset cache defaults to
/// [`developer_cache`], not the player's, also when built without the
/// developer harness.
pub fn developer_program() {
    DEVELOPER.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Whether this process is a cargo test binary (`<target>/<profile>/deps/`).
fn in_test_binary() -> bool {
    std::env::current_exe().ok().and_then(|e| e.parent().and_then(|d| d.file_name()).map(|n| n == "deps")).unwrap_or(false)
}

/// The developer cache: the player's cache folder's sibling,
/// `$XDG_CACHE_HOME/overbright-dev` (else `~/.cache/overbright-dev`;
/// Windows `<Local AppData>\Overbright\cache-dev`; with `NFSU2_DATA_DIR`,
/// its `cache-dev`).
pub fn developer_cache() -> Option<PathBuf> {
    #[cfg(windows)]
    let known = super::win::local_app_data();
    #[cfg(not(windows))]
    let known = None;
    resolve_developer_cache(Os::CURRENT, &|name: &str| std::env::var_os(name), known)
}

/// The developer cache's folder name in the XDG-style places.
pub const DEV_APP: &str = "overbright-dev";

/// [`developer_cache`] for `os` and the environment `var` (pure, as
/// [`resolve`]).
pub fn resolve_developer_cache(
    os: Os,
    var: &dyn Fn(&str) -> Option<OsString>,
    local_app_data: Option<PathBuf>,
) -> Option<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = non_empty(DATA_DIR_VAR) {
        return Some(dir.join("cache-dev"));
    }
    if let Some(x) = non_empty("XDG_CACHE_HOME") {
        return Some(x.join(DEV_APP));
    }
    match os {
        Os::Unix => var("HOME").map(|h| PathBuf::from(h).join(".cache").join(DEV_APP)),
        Os::Windows => local_app_data
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| non_empty("LOCALAPPDATA"))
            .map(|l| l.join(WINDOWS_APP).join("cache-dev")),
    }
}

/// Set by [`use_test_cache`].
static TEST_CACHE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Tests: the cache defaults to `dir` for the rest of the process (the
/// test kit's `game()` and the game's test sandbox call it with
/// [`test_cache_folder`]; `NFSU2_CACHE_DIR` still comes first). The first
/// call wins.
pub fn use_test_cache(dir: PathBuf) {
    let _ = TEST_CACHE.set(dir);
}

/// The tests' cache folder: `test-cache` in the build's target folder
/// (beside the test binary's `deps`), kept between runs; else
/// `<temp>/overbright-test-cache`.
pub fn test_cache_folder() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().find(|d| d.file_name().is_some_and(|n| n == "deps")).map(|d| d.to_path_buf()))
        .and_then(|deps| deps.parent().map(|p| p.join("test-cache")))
        .unwrap_or_else(|| std::env::temp_dir().join("overbright-test-cache"))
}

/// The player's home folders as they appear in text, with the token that
/// replaces each in what Overbright writes for others to read (crash
/// reports, marks, logs): `$HOME` and, on Windows, `%USERPROFILE%`. Values
/// of one character or less are left out.
pub fn home_tokens(var: &dyn Fn(&str) -> Option<String>) -> Vec<(String, &'static str)> {
    [("HOME", "$HOME"), ("USERPROFILE", "%USERPROFILE%")]
        .into_iter()
        .filter_map(|(name, token)| var(name).filter(|h| h.len() > 1).map(|h| (h, token)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let v: Vec<(String, String)> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| v.iter().find(|(k, _)| k == name).map(|(_, v)| OsString::from(v))
    }

    /// The Linux rules written out on their own: the data home
    /// (`XDG_DATA_HOME`, set even empty, else `~/.local/share`), Overbright's
    /// folder in it, and the cache (`XDG_CACHE_HOME` when not empty, else
    /// `~/.cache`).
    fn plain_unix(var: &dyn Fn(&str) -> Option<OsString>) -> Layout {
        let data_home = var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        let cache = if let Some(d) = var("XDG_CACHE_HOME").filter(|d| !d.is_empty()) {
            Some(PathBuf::from(d).join(APP))
        } else {
            var("HOME").map(|h| PathBuf::from(h).join(".cache").join(APP))
        };
        Layout { app: data_home.join(APP), data_home, cache }
    }

    #[test]
    fn unix_follows_the_xdg_rules_byte_for_byte() {
        let cases: &[&[(&str, &str)]] = &[
            &[],
            &[("HOME", "/home/u")],
            &[("HOME", "/home/u/")],
            &[("HOME", "/home/u"), ("XDG_DATA_HOME", "/x/data")],
            &[("HOME", "/home/u"), ("XDG_DATA_HOME", "")],
            &[("XDG_DATA_HOME", "/x/data")],
            &[("HOME", "/home/u"), ("XDG_CACHE_HOME", "/x/cache")],
            &[("HOME", "/home/u"), ("XDG_CACHE_HOME", "")],
            &[("XDG_CACHE_HOME", "/x/cache")],
            &[("HOME", "/home/u"), ("XDG_DATA_HOME", "/tmp/nfsu2-shot-1"), ("XDG_CACHE_HOME", "/c")],
            &[("HOME", "/home/u"), ("LOCALAPPDATA", "C:\\Users\\u\\AppData\\Local")],
            &[("HOME", "/home/u"), ("NFSU2_DATA_DIR", "")],
        ];
        for c in cases {
            let var = env(c);
            let got = resolve(Os::Unix, &var, None);
            let want = plain_unix(&var);
            assert_eq!(got, want, "{c:?}");
            assert_eq!(got.app.as_os_str().as_encoded_bytes(), want.app.as_os_str().as_encoded_bytes(), "{c:?}");
        }
        let l = resolve(Os::Unix, &env(&[("HOME", "/home/u")]), None);
        assert_eq!(l.data_home, Path::new("/home/u/.local/share"));
        assert_eq!(l.app, Path::new("/home/u/.local/share/overbright"));
        assert_eq!(l.cache.as_deref(), Some(Path::new("/home/u/.cache/overbright")));
        assert_eq!(l.data_home.join("NFS Underground 2"), Path::new("/home/u/.local/share/NFS Underground 2"));
    }

    #[test]
    fn data_dir_override_takes_everything_on_every_os() {
        for os in [Os::Unix, Os::Windows] {
            let var = env(&[
                ("HOME", "/home/u"),
                ("XDG_DATA_HOME", "/x"),
                ("XDG_CACHE_HOME", "/c"),
                ("NFSU2_DATA_DIR", "/p"),
            ]);
            let l = resolve(os, &var, Some(PathBuf::from("/lad")));
            assert_eq!(
                l,
                Layout { data_home: "/p".into(), app: "/p".into(), cache: Some("/p/cache".into()) },
                "{os:?}"
            );
        }
    }

    #[test]
    fn windows_has_a_folder_of_its_own() {
        let lad = PathBuf::from("C:/Users/u/AppData/Local");
        let var = env(&[("USERPROFILE", "C:/Users/u"), ("LOCALAPPDATA", "D:/elsewhere")]);
        let l = resolve(Os::Windows, &var, Some(lad.clone()));
        let own = lad.join("Overbright");
        assert_eq!(l, Layout { data_home: own.clone(), app: own.clone(), cache: Some(own.join("cache")) });
        // Never the original's save folder.
        assert_ne!(l.data_home.join("NFS Underground 2"), lad.join("NFS Underground 2"));
        assert!(l.data_home.join("NFS Underground 2").starts_with(&own));
        // The known folder failing: %LOCALAPPDATA%.
        let l = resolve(Os::Windows, &var, None);
        assert_eq!(l.app, Path::new("D:/elsewhere").join("Overbright"));
        let l = resolve(Os::Windows, &var, Some(PathBuf::new()));
        assert_eq!(l.app, Path::new("D:/elsewhere").join("Overbright"));
        // Neither: the current folder (as Linux without HOME), no cache.
        let l = resolve(Os::Windows, &env(&[]), None);
        assert_eq!((l.data_home, l.app, l.cache), (PathBuf::from("."), Path::new(".").join("overbright"), None));
        // HOME means nothing on Windows.
        let l = resolve(Os::Windows, &env(&[("HOME", "/home/u")]), Some(lad.clone()));
        assert_eq!(l.app, own);
    }

    #[test]
    fn windows_honours_the_harness_isolation() {
        let var = env(&[("XDG_DATA_HOME", "T:/nfsu2-shot-1"), ("XDG_CACHE_HOME", "T:/c")]);
        let l = resolve(Os::Windows, &var, Some(PathBuf::from("C:/lad")));
        assert_eq!(l.data_home, Path::new("T:/nfsu2-shot-1"));
        assert_eq!(l.app, Path::new("T:/nfsu2-shot-1").join("overbright"));
        assert_eq!(l.cache.as_deref(), Some(Path::new("T:/c").join("overbright").as_path()));
        // Empty XDG values are not set.
        let l =
            resolve(Os::Windows, &env(&[("XDG_DATA_HOME", ""), ("XDG_CACHE_HOME", "")]), Some(PathBuf::from("C:/lad")));
        assert_eq!(l.app, Path::new("C:/lad").join("Overbright"));
        assert_eq!(l.cache, Some(Path::new("C:/lad").join("Overbright").join("cache")));
    }

    #[test]
    fn subfolders_and_the_thread_override() {
        let parent = std::env::temp_dir().join(format!("nfsu2-dirs-test-{}", std::process::id()));
        std::thread::spawn(move || {
            set_thread_data_home(parent.clone());
            assert_eq!(data_home(), parent);
            let own = parent.join("overbright");
            assert_eq!(app(), own);
            assert_eq!(config(), own);
            assert_eq!(crashes(), own.join("crashes"));
            assert_eq!(logs(), own.join("logs"));
            assert_eq!(marks(), own.join("marks"));
            assert_eq!(perf(), own.join("perf"));
            assert_eq!(screenshots(), own.join("Screenshots"));
            assert_eq!(packs(), own.join("texture-pack"));
        })
        .join()
        .unwrap();
    }

    #[test]
    fn home_tokens_cover_both_oses() {
        let var = |n: &str| match n {
            "HOME" => Some("/home/u".to_string()),
            "USERPROFILE" => Some("C:\\Users\\u".to_string()),
            _ => None,
        };
        assert_eq!(
            home_tokens(&var),
            vec![("/home/u".to_string(), "$HOME"), ("C:\\Users\\u".to_string(), "%USERPROFILE%")]
        );
        assert!(home_tokens(&|_| Some("/".to_string())).is_empty());
    }
}
