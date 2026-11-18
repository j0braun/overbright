//! Where Overbright keeps its files, per OS (docs/platform-dirs.md). One
//! place builds every such path; [`resolve`] is a pure function of the
//! environment so each OS's rules are tested on any OS.
//!
//! Linux and other Unix:
//! * data home (the profiles' parent, the original's `%LOCALAPPDATA%`):
//!   `$XDG_DATA_HOME`, else `$HOME/.local/share`, else `.`;
//! * Overbright's folder: `<data home>/overbright` ([`APP`]; options.txt,
//!   game-root.txt, crashes/, logs/, marks/, perf/,
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
//!
//! Portable mode (every OS): a file `portable.txt` or a folder `portable`
//! beside the executables ([`portable_base`]) puts everything in the
//! folder `data` beside them, laid out as with `NFSU2_DATA_DIR` (which
//! still comes first). Paths Overbright keeps in its files (the game
//! folder, the HUD mod folder) are then kept relative to the program
//! folder when they are on the same drive ([`store_path`], [`load_path`]).

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

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
    /// Overbright's own folder (options.txt, game-root.txt and
    /// the subfolders of [`crashes`], [`logs`], …).
    pub app: PathBuf,
    /// The asset cache's folder, or None when there is no place for it.
    pub cache: Option<PathBuf>,
}

/// The folders for `os`, from the environment `var` and, on Windows, the
/// Local AppData known folder (`local_app_data`; None falls back to
/// `%LOCALAPPDATA%`). Pure: no file-system access.
pub fn resolve(os: Os, var: &dyn Fn(&str) -> Option<OsString>, local_app_data: Option<PathBuf>) -> Layout {
    resolve_portable(os, var, local_app_data, None)
}

/// [`resolve`] for a program folder `portable` that is a portable install
/// ([`portable_base`]): everything in `<portable>/data`, laid out as with
/// `NFSU2_DATA_DIR`, which still comes first. None: [`resolve`]. Pure.
pub fn resolve_portable(
    os: Os,
    var: &dyn Fn(&str) -> Option<OsString>,
    local_app_data: Option<PathBuf>,
    portable: Option<&Path>,
) -> Layout {
    let name = APP;
    let non_empty = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = non_empty(DATA_DIR_VAR).or_else(|| portable.map(portable_data)) {
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
    resolve_portable(Os::CURRENT, &|name: &str| std::env::var_os(name), known, portable_base().as_deref())
}

/// The marker file of a portable install, beside the executables.
pub const PORTABLE_FILE: &str = "portable.txt";
/// The marker folder of a portable install (the same as [`PORTABLE_FILE`]).
pub const PORTABLE_FOLDER: &str = "portable";
/// The data folder of a portable install, beside the executables.
pub const PORTABLE_DATA: &str = "data";

/// The data folder of the portable install in `program_dir`.
pub fn portable_data(program_dir: &Path) -> PathBuf {
    program_dir.join(PORTABLE_DATA)
}

/// Whether `program_dir` (the executables' folder) holds a portable
/// install's marker: a file `portable.txt` or a folder `portable`.
pub fn is_portable(program_dir: &Path) -> bool {
    program_dir.join(PORTABLE_FILE).is_file() || program_dir.join(PORTABLE_FOLDER).is_dir()
}

/// [`portable_base`] once looked up (None inside: not portable).
static PORTABLE: std::sync::RwLock<Option<Option<PathBuf>>> = std::sync::RwLock::new(None);
/// Set by [`ignore_portable`].
static NO_PORTABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The program folder (the running executable's) when it is a portable
/// install ([`is_portable`]), else None. Looked up once per process and
/// kept; [`refresh_portable`] looks again.
pub fn portable_base() -> Option<PathBuf> {
    if NO_PORTABLE.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    if let Some(p) = PORTABLE.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
        return p;
    }
    refresh_portable()
}

/// Looks for the portable marker again (the launcher after turning
/// portable mode on or off) and returns [`portable_base`].
pub fn refresh_portable() -> Option<PathBuf> {
    let found = program_dir().filter(|d| is_portable(d));
    *PORTABLE.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(found.clone());
    if NO_PORTABLE.load(std::sync::atomic::Ordering::Relaxed) { None } else { found }
}

/// The developer harness's isolated runs (shots, flows, the selftest, the
/// playthrough) keep their data in the folder they chose, also when the
/// build sits in a portable install: portable mode is off for the rest of
/// the process.
pub fn ignore_portable() {
    NO_PORTABLE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The running executable's folder.
pub fn program_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent().map(Path::to_path_buf)
}

/// Turns portable mode on for the program folder `program_dir`: writes
/// [`PORTABLE_FILE`] there (nothing else is written or moved). Takes
/// effect for this process after [`refresh_portable`].
pub fn make_portable(program_dir: &Path) -> std::io::Result<PathBuf> {
    let f = program_dir.join(PORTABLE_FILE);
    if !f.is_file() {
        std::fs::write(
            &f,
            "Overbright portable mode: settings, saves, caches and logs are kept in the folder `data` beside \
             this file. Delete this file to keep them in the home folders again.\n",
        )?;
    }
    Ok(portable_data(program_dir))
}

/// The form a path is kept in Overbright's files: in a portable install
/// ([`portable_base`]), relative to the program folder when both are on
/// the same drive; else (and outside portable mode) `p` as it is.
pub fn store_path(p: &Path) -> PathBuf {
    match portable_base() {
        Some(base) if same_drive(&base, p) => relative_to(&base, p).unwrap_or_else(|| p.to_path_buf()),
        _ => p.to_path_buf(),
    }
}

/// A path read from Overbright's files: a relative one ([`store_path`])
/// is taken from the program folder of a portable install; anything else
/// (and every path outside portable mode) as it is.
pub fn load_path(p: &Path) -> PathBuf {
    match portable_base() {
        Some(base) if p.is_relative() && !p.as_os_str().is_empty() => join_relative(&base, p),
        _ => p.to_path_buf(),
    }
}

/// `base` joined with the relative path `rel`, its `..` and `.` applied
/// to the text (so the folder shows without them). Pure.
pub fn join_relative(base: &Path, rel: &Path) -> PathBuf {
    let mut out = base.to_path_buf();
    for c in rel.components() {
        match c {
            Component::ParentDir => {
                // Never above the root (`/..` is `/`).
                if out.parent().is_some() {
                    out.pop();
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `target` relative to the folder `base`, both absolute and plain (no
/// `.` or `..`): the parts after their shared start, with one `..` per
/// part of `base` left. None when they have no shared root (another drive
/// letter or network share on Windows, compared without case) or either
/// is not absolute and plain. Pure.
pub fn relative_to(base: &Path, target: &Path) -> Option<PathBuf> {
    let plain =
        |p: &Path| p.is_absolute() && p.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir));
    if !plain(base) || !plain(target) {
        return None;
    }
    let key = |c: Component| -> OsString {
        match c {
            // Windows' drive letters and shares, without case.
            Component::Prefix(p) => p.as_os_str().to_ascii_uppercase(),
            other => other.as_os_str().to_os_string(),
        }
    };
    let b: Vec<OsString> = base.components().map(key).collect();
    let t: Vec<Component> = target.components().collect();
    let shared = b.iter().zip(t.iter().map(|c| key(*c))).take_while(|(x, y)| **x == *y).count();
    // The root (and on Windows the drive or share) must be shared.
    let root = base.components().take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir)).count();
    if shared < root {
        return None;
    }
    let mut out = PathBuf::new();
    for _ in shared..b.len() {
        out.push("..");
    }
    for c in &t[shared..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    Some(out)
}

/// Whether `a` and `b` are on the same drive: on Windows the same drive
/// letter or share ([`relative_to`] checks it), elsewhere the same file
/// system (the device of the nearest folder that exists).
fn same_drive(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let dev = |p: &Path| p.ancestors().find_map(|d| std::fs::metadata(d).ok()).map(|m| m.dev());
        matches!((dev(a), dev(b)), (Some(x), Some(y)) if x == y)
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        true
    }
}

/// The driver's own shader caches, which live in the home folders
/// unless told otherwise: in a portable install, the variables that move
/// them into `<data>/cache/driver` (Mesa's `MESA_SHADER_CACHE_DIR`,
/// NVIDIA's `__GL_SHADER_DISK_CACHE_PATH`), those the environment does not
/// set already. Linux and other Unix only: Windows' drivers keep theirs
/// where they decide. Empty outside portable mode.
pub fn driver_cache_vars(
    os: Os,
    var: &dyn Fn(&str) -> Option<OsString>,
    portable: Option<&Path>,
) -> Vec<(&'static str, PathBuf)> {
    let Some(base) = portable else { return Vec::new() };
    if os != Os::Unix || var(DATA_DIR_VAR).is_some_and(|v| !v.is_empty()) {
        return Vec::new();
    }
    let dir = portable_data(base).join("cache").join("driver");
    ["MESA_SHADER_CACHE_DIR", "__GL_SHADER_DISK_CACHE_PATH"]
        .into_iter()
        .filter(|n| var(n).is_none())
        .map(|n| (n, dir.clone()))
        .collect()
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

/// The profiles' parent ([`Layout::data_home`], or where the player put
/// the saves: [`Location::Saves`]).
pub fn data_home() -> PathBuf {
    override_of(Location::Saves).unwrap_or_else(|| thread_data_home().unwrap_or_else(|| layout().data_home))
}

/// Overbright's own folder ([`Layout::app`], or where the player put the
/// settings: [`Location::Settings`]).
pub fn app() -> PathBuf {
    override_of(Location::Settings).unwrap_or_else(default_app)
}

/// The settings files' folder: options.txt, game-root.txt.
pub fn config() -> PathBuf {
    app()
}

/// Crash reports (`qol::crash`): `crashes` beside the logs' default, or
/// inside the folder the player chose for the logs.
pub fn crashes() -> PathBuf {
    match override_of(Location::Logs) {
        Some(d) => d.join("crashes"),
        None => app().join("crashes"),
    }
}

/// The game's log files (`qol::log_file`), the support files too.
pub fn logs() -> PathBuf {
    override_of(Location::Logs).unwrap_or_else(|| app().join("logs"))
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
    override_of(Location::Screenshots).unwrap_or_else(|| app().join("Screenshots"))
}

/// The folder that holds the texture packs: Overbright packs (`packs/`),
/// the imported packages and the generated pack (`texture-pack/`).
pub fn mods_root() -> PathBuf {
    override_of(Location::Packs).unwrap_or_else(app)
}

/// The default texture-pack folder (`render::enh::assets::pack_select`).
pub fn packs() -> PathBuf {
    mods_root().join("texture-pack")
}

/// The map mods' folder (`tools::map::store`).
pub fn maps() -> PathBuf {
    override_of(Location::Maps).unwrap_or_else(|| app().join("maps"))
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
    if DEVELOPER.load(std::sync::atomic::Ordering::Relaxed) {
        developer_cache()
    } else {
        override_of(Location::Cache).or_else(|| layout().cache)
    }
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
/// or in a portable install, the data folder's `cache-dev`).
pub fn developer_cache() -> Option<PathBuf> {
    #[cfg(windows)]
    let known = super::win::local_app_data();
    #[cfg(not(windows))]
    let known = None;
    resolve_developer_cache(Os::CURRENT, &|name: &str| std::env::var_os(name), known, portable_base().as_deref())
}

/// The developer cache's folder name in the XDG-style places.
pub const DEV_APP: &str = "overbright-dev";

/// [`developer_cache`] for `os` and the environment `var` (pure, as
/// [`resolve`]).
pub fn resolve_developer_cache(
    os: Os,
    var: &dyn Fn(&str) -> Option<OsString>,
    local_app_data: Option<PathBuf>,
    portable: Option<&Path>,
) -> Option<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(dir) = non_empty(DATA_DIR_VAR).or_else(|| portable.map(portable_data)) {
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

// ------------------------------------------------------------ locations

/// A folder the player can put elsewhere (Settings › Advanced › Storage).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Location {
    /// The profiles' parent ([`data_home`]; the saves are in its
    /// `NFS Underground 2`).
    Saves,
    /// Overbright's own folder ([`app`]: options.txt, game-root.txt, …).
    Settings,
    /// The asset and pipeline cache ([`cache`]; the shipping game's).
    Cache,
    /// Logs, crash reports and support files ([`logs`], [`crashes`]).
    Logs,
    /// Photo Mode's pictures ([`screenshots`]).
    Screenshots,
    /// Texture packs: Overbright packs, imported packages, the generated
    /// pack ([`mods_root`]).
    Packs,
    /// Map mods ([`maps`]).
    Maps,
}

impl Location {
    pub const ALL: [Location; 7] = [
        Location::Saves,
        Location::Settings,
        Location::Cache,
        Location::Logs,
        Location::Screenshots,
        Location::Packs,
        Location::Maps,
    ];

    /// Its key in the locations file.
    pub fn key(self) -> &'static str {
        match self {
            Location::Saves => "saves",
            Location::Settings => "settings",
            Location::Cache => "cache",
            Location::Logs => "logs",
            Location::Screenshots => "screenshots",
            Location::Packs => "packs",
            Location::Maps => "maps",
        }
    }

    pub fn from_key(k: &str) -> Option<Location> {
        Location::ALL.into_iter().find(|l| l.key() == k)
    }
}

/// The locations file's name: one `<key> <path>` line per folder put
/// elsewhere. In a portable install it is in the data folder (its paths
/// kept relative to the program folder on the same drive, as
/// [`store_path`]); else in Overbright's default folder. Read before any
/// other of Overbright's files.
pub const LOCATIONS_FILE: &str = "locations.txt";

/// The locations file of this process.
pub fn locations_file() -> PathBuf {
    default_app().join(LOCATIONS_FILE)
}

/// Overbright's folder without a Settings override (where the locations
/// file lives).
pub fn default_app() -> PathBuf {
    match thread_data_home() {
        Some(d) => d.join(APP),
        None => layout().app,
    }
}

/// The locations file's text read: the folders put elsewhere (relative
/// paths taken from the program folder of a portable install). Pure apart
/// from [`load_path`].
pub fn parse_locations(text: &str) -> Vec<(Location, PathBuf)> {
    let mut out: Vec<(Location, PathBuf)> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Some((k, p)) = l.split_once(char::is_whitespace) else { continue };
        let (Some(loc), p) = (Location::from_key(k), p.trim()) else { continue };
        if !p.is_empty() && !out.iter().any(|(o, _)| *o == loc) {
            out.push((loc, load_path(Path::new(p))));
        }
    }
    out
}

/// The locations file's text for `list` (paths as [`store_path`] keeps them).
pub fn format_locations(list: &[(Location, PathBuf)]) -> String {
    let mut s = String::from("# Overbright: folders kept elsewhere (Settings › Advanced › Storage).\n");
    for (l, p) in list {
        s += &format!("{} {}\n", l.key(), store_path(p).display());
    }
    s
}

/// The folders put elsewhere, as the locations file says now (read again
/// when it changed).
pub fn overrides() -> Vec<(Location, PathBuf)> {
    static SEEN: std::sync::Mutex<Option<(PathBuf, Option<std::time::SystemTime>, Vec<(Location, PathBuf)>)>> =
        std::sync::Mutex::new(None);
    let f = locations_file();
    let mtime = std::fs::metadata(&f).and_then(|m| m.modified()).ok();
    let mut seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((p, t, v)) = seen.as_ref()
        && *p == f
        && *t == mtime
    {
        return v.clone();
    }
    let v = mtime.map_or_else(Vec::new, |_| std::fs::read_to_string(&f).map(|t| parse_locations(&t)).unwrap_or_default());
    *seen = Some((f, mtime, v.clone()));
    v
}

/// Where `loc` is put by the player, if anywhere.
pub fn override_of(loc: Location) -> Option<PathBuf> {
    overrides().into_iter().find(|(l, _)| *l == loc).map(|(_, p)| p)
}

/// Puts `loc` at `to` (None: back to its default) in the locations file.
/// Nothing is moved here (the launcher copies first, when asked).
pub fn set_location(loc: Location, to: Option<&Path>) -> std::io::Result<()> {
    let mut list: Vec<(Location, PathBuf)> = overrides().into_iter().filter(|(l, _)| *l != loc).collect();
    if let Some(p) = to {
        list.push((loc, p.to_path_buf()));
    }
    list.sort_by_key(|(l, _)| *l);
    let f = locations_file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    if list.is_empty() {
        return match std::fs::remove_file(&f) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let tmp = f.with_extension("txt.tmp");
    std::fs::write(&tmp, format_locations(&list))?;
    std::fs::rename(&tmp, &f)
}

/// Where `loc` is by default (no override).
pub fn default_location(loc: Location) -> Option<PathBuf> {
    match loc {
        Location::Saves => Some(thread_data_home().unwrap_or_else(|| layout().data_home)),
        Location::Settings => Some(default_app()),
        Location::Cache => layout().cache,
        Location::Logs => Some(app().join("logs")),
        Location::Screenshots => Some(app().join("Screenshots")),
        Location::Packs => Some(app()),
        Location::Maps => Some(app().join("maps")),
    }
}

/// Where `loc` is now: the player's choice, else the default.
pub fn location(loc: Location) -> Option<PathBuf> {
    override_of(loc).or_else(|| default_location(loc))
}

/// Whether a chosen folder is on a drive that is not there (its root, or
/// on Unix the mount point's parent chain, does not exist): the Storage
/// page says so and offers Reset.
pub fn location_missing(p: &Path) -> bool {
    !p.exists() && p.parent().is_some_and(|parent| !parent.exists())
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

    /// Portable mode: everything in `<program>/data`, on every OS, over
    /// the XDG folders and Windows' Local AppData; `NFSU2_DATA_DIR` still
    /// first. Without a program folder the layout is [`resolve`]'s.
    #[test]
    fn portable_install_keeps_everything_in_its_data_folder() {
        let prog = Path::new("/media/u/SSD/Overbright");
        let data = prog.join("data");
        let full = env(&[
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/x"),
            ("XDG_CACHE_HOME", "/c"),
            ("LOCALAPPDATA", "C:/Users/u/AppData/Local"),
        ]);
        for os in [Os::Unix, Os::Windows] {
            let l = resolve_portable(os, &full, Some(PathBuf::from("/lad")), Some(prog));
            assert_eq!(l, Layout { data_home: data.clone(), app: data.clone(), cache: Some(data.join("cache")) });
            let dev = resolve_developer_cache(os, &full, Some(PathBuf::from("/lad")), Some(prog));
            assert_eq!(dev, Some(data.join("cache-dev")));
            let over = env(&[("HOME", "/home/u"), ("NFSU2_DATA_DIR", "/p")]);
            assert_eq!(resolve_portable(os, &over, None, Some(prog)).app, Path::new("/p"), "{os:?}");
            // An empty NFSU2_DATA_DIR is not set.
            let empty = env(&[("HOME", "/home/u"), ("NFSU2_DATA_DIR", "")]);
            assert_eq!(resolve_portable(os, &empty, None, Some(prog)).app, data, "{os:?}");
            for c in [&[][..], &[("HOME", "/home/u")][..], &[("HOME", "/home/u"), ("XDG_DATA_HOME", "/x")][..]] {
                let var = env(c);
                assert_eq!(resolve_portable(os, &var, None, None), resolve(os, &var, None), "{os:?} {c:?}");
            }
        }
        let home = env(&[("HOME", "/home/u")]);
        assert_eq!(
            resolve_developer_cache(Os::Unix, &home, None, None),
            Some(PathBuf::from("/home/u/.cache/overbright-dev"))
        );
    }

    /// The marker: a file `portable.txt` or a folder `portable` beside the
    /// programs; anything else (a folder named portable.txt, a file named
    /// portable, nothing) is no portable install. `make_portable` writes
    /// the file and nothing else.
    #[test]
    fn portable_marker_file_or_folder() {
        let base = std::env::temp_dir().join(format!("nfsu2-portable-marker-{}", crate::unique_tag()));
        let d = |n: &str| {
            let p = base.join(n);
            std::fs::create_dir_all(&p).unwrap();
            p
        };
        let none = d("none");
        assert!(!is_portable(&none));
        let file = d("file");
        std::fs::write(file.join("portable.txt"), "").unwrap();
        assert!(is_portable(&file));
        let folder = d("folder");
        std::fs::create_dir(folder.join("portable")).unwrap();
        assert!(is_portable(&folder));
        let wrong = d("wrong");
        std::fs::create_dir(wrong.join("portable.txt")).unwrap();
        std::fs::write(wrong.join("portable"), "").unwrap();
        assert!(!is_portable(&wrong));
        let made = d("made");
        assert_eq!(make_portable(&made).unwrap(), made.join("data"));
        assert!(is_portable(&made));
        let names: Vec<_> = std::fs::read_dir(&made).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![OsString::from("portable.txt")], "only the marker is written");
        // Again: the file is kept as it is.
        std::fs::write(made.join("portable.txt"), "mine").unwrap();
        make_portable(&made).unwrap();
        assert_eq!(std::fs::read_to_string(made.join("portable.txt")).unwrap(), "mine");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Kept paths: relative to the program folder (`..` up, then down),
    /// absolute when there is no shared root; read back to the same
    /// folder.
    #[cfg(unix)]
    #[test]
    fn kept_paths_relative_to_the_program_folder() {
        let prog = Path::new("/media/u/SSD/Overbright");
        let cases = [
            ("/media/u/SSD/Games/Underground", "../Games/Underground"),
            ("/media/u/SSD/Overbright/game", "game"),
            ("/media/u/SSD/Overbright", "."),
            ("/media/u/SSD", ".."),
            ("/", "../../../.."),
            ("/home/u/Games/Need for Speed Underground 2", "../../../../home/u/Games/Need for Speed Underground 2"),
        ];
        for (target, rel) in cases {
            let got = relative_to(prog, Path::new(target)).unwrap_or_else(|| panic!("{target}"));
            assert_eq!(got, Path::new(rel), "{target}");
            assert_eq!(join_relative(prog, &got), Path::new(target), "{target} back");
        }
        // Not absolute or not plain: kept as given.
        assert_eq!(relative_to(prog, Path::new("Games/Underground")), None);
        assert_eq!(relative_to(prog, Path::new("/media/u/../u/SSD")), None);
        assert_eq!(relative_to(Path::new("rel"), Path::new("/a")), None);
        assert_eq!(join_relative(Path::new("/"), Path::new("../../a")), Path::new("/a"));
    }

    /// Windows: another drive letter or share is no shared root, so the
    /// path stays absolute; the drive letter's case does not matter.
    #[cfg(windows)]
    #[test]
    fn kept_paths_on_windows_drives() {
        let prog = Path::new(r"E:\Overbright");
        assert_eq!(relative_to(prog, Path::new(r"E:\Games\Underground")).unwrap(), Path::new(r"..\Games\Underground"));
        assert_eq!(relative_to(prog, Path::new(r"e:\Games\Underground")).unwrap(), Path::new(r"..\Games\Underground"));
        assert_eq!(join_relative(prog, Path::new(r"..\Games\Underground")), Path::new(r"E:\Games\Underground"));
        assert_eq!(relative_to(prog, Path::new(r"C:\Games\Underground")), None);
        assert_eq!(relative_to(Path::new(r"\\srv\share\Overbright"), Path::new(r"\\srv\other\Underground")), None);
    }

    /// The driver's shader caches go into the data folder of a portable
    /// install on Linux, unless the environment sets them; nothing on
    /// Windows, outside portable mode or with `Underground_DATA_DIR`.
    #[test]
    fn driver_caches_follow_a_portable_install_on_linux() {
        let prog = Path::new("/media/u/SSD/Overbright");
        let dir = prog.join("data/cache/driver");
        let got = driver_cache_vars(Os::Unix, &env(&[("HOME", "/home/u")]), Some(prog));
        assert_eq!(got, vec![("MESA_SHADER_CACHE_DIR", dir.clone()), ("__GL_SHADER_DISK_CACHE_PATH", dir.clone())]);
        let got = driver_cache_vars(Os::Unix, &env(&[("MESA_SHADER_CACHE_DIR", "/mine")]), Some(prog));
        assert_eq!(got, vec![("__GL_SHADER_DISK_CACHE_PATH", dir)]);
        assert!(driver_cache_vars(Os::Unix, &env(&[]), None).is_empty());
        assert!(driver_cache_vars(Os::Windows, &env(&[]), Some(prog)).is_empty());
        assert!(driver_cache_vars(Os::Unix, &env(&[("NFSU2_DATA_DIR", "/p")]), Some(prog)).is_empty());
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
