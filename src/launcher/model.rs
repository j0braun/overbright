//! What the launcher's screens show and change, and where it comes from:
//! [`Env`] is the real machine ([`RealEnv`]: the game-folder check, the
//! game's own settings code) or fixed fake data ([`FakeEnv`], for the
//! headless renders and tests; no game files needed).

use std::path::{Path, PathBuf};

use super::probe::{FirstRun, Probe};
use super::saves;
use super::ui::Mark;
use crate::frontend::options::Settings;
use crate::qol::{Enhancements, Level, Preset, graphics_original, setup};
use crate::render::display::MonitorInfo;

/// A game folder found on this computer and where it was found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    pub source: String,
}

/// A checked game folder, in plain words ([`setup::check_lines`]).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    pub root: PathBuf,
    pub lines: Vec<(Mark, String)>,
    /// No key file is missing and no line is an error (the game starts
    /// with it).
    pub usable: bool,
}

/// The Graphics row: the launcher offers the game's Graphics Mode,
/// Original or Enhanced (docs/settings-page.md "Graphics mode and quality
/// levels"); values set in the game that are neither show as they are and
/// stay until the player picks one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
    Original,
    Enhanced,
    /// Enhanced at another quality level than the launcher's (its name).
    Other(&'static str),
    /// Values that match no quality level.
    Custom,
}

/// The quality level "Enhanced" stands for here: Low (the cheap rows, the
/// same on every GPU, so it needs no GPU probe); the game's page offers
/// Medium and High.
pub const ENHANCED_LEVEL: Level = Level::Low;

impl Graphics {
    pub fn of(e: &Enhancements) -> Graphics {
        if graphics_original(e) {
            return Graphics::Original;
        }
        match Level::of(e) {
            Some(l) if l == ENHANCED_LEVEL => Graphics::Enhanced,
            Some(Level::Medium) => Graphics::Other("Enhanced Medium"),
            Some(_) => Graphics::Other("Enhanced High"),
            None => Graphics::Custom,
        }
    }

    pub fn label(self) -> String {
        match self {
            Graphics::Original => "Original".into(),
            Graphics::Enhanced => "Enhanced".into(),
            Graphics::Other(n) => format!("{n} (set in game)"),
            Graphics::Custom => "Custom (set in game)".into(),
        }
    }
}

/// The values the main screen changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Choices {
    pub fullscreen: bool,
    pub resolution: (u32, u32),
    pub graphics: Graphics,
    /// `launcher.skip`.
    pub skip: bool,
    /// The game's Skip Intro Movies row (`enh.skip_movies`, Enhancements
    /// page; an older file's `enh.skip_title 2` included).
    pub skip_movies: bool,
    /// The game's Skip Title Screen row (`enh.skip_title`).
    pub skip_title: bool,
    /// The HUD mod's folder ("" = none; `texture-pack-root.txt`).
    pub texture_pack: String,
    /// Use the HUD mod: the game's HD UI + Maps setting (`enh.hd_ui`).
    pub hud_mod: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackStatus {
    None,
    Complete,
    Incomplete,
    NotFound,
}

impl PackStatus {
    pub fn words(self) -> &'static str {
        match self {
            PackStatus::None => "No folder: the game's own HUD",
            PackStatus::Complete => "Complete",
            PackStatus::Incomplete => "Incomplete: the game's own HUD is used",
            PackStatus::NotFound => "Folder not found",
        }
    }
}

/// Everything the screens show.
#[derive(Clone, Debug)]
pub struct Model {
    pub version: String,
    /// The game folder in use (after a check, or kept).
    pub root: Option<PathBuf>,
    pub found: Vec<Found>,
    pub found_sel: usize,
    pub path_input: String,
    pub check: Option<CheckResult>,
    pub choices: Choices,
    /// The choices as last saved (a save only after a change).
    pub saved: Choices,
    /// The monitor the game would open on (the launcher window's, then
    /// the probe's): the Resolution row's list per display path
    /// ([`Model::modes`]).
    pub monitor: Option<MonitorInfo>,
    pub languages: Vec<String>,
    pub pack_status: PackStatus,
    /// options.txt exists (the game has run once; see `save_choices`).
    pub settings_file: bool,
    pub message: Option<(Mark, String)>,
    /// The widget test screen's values.
    pub demo: Demo,
    /// `game --probe`'s result (the GPU line; the display rows before the
    /// game's first run).
    pub probe: ProbeState,
    /// The first connected controller's name, if any.
    pub controller: Option<String>,
    /// How the last start failed (the failure screen).
    pub failure: Option<Failure>,
    /// The folder browser's state.
    pub browse: Browse,
    /// The save import's state.
    pub import: Import,
    /// The log folder (shown on the failure screen).
    pub logs_dir: PathBuf,
    /// Overbright's profiles folder (the import's target).
    pub saves_dir: PathBuf,
    /// The about screen's version line (`build_info::version_line`).
    pub build: String,
    /// Where this build's source code is published ([`source_url`]); None:
    /// an unofficial build without a source link.
    pub source: Option<String>,
    /// The third-party notices shown (THIRD-PARTY-NOTICES.md; the fake
    /// data has a fixed text, so the renders do not change when a notice
    /// is added).
    pub notices: &'static str,
    /// The system has its own folder dialog (Windows).
    pub native_dialog: bool,
    /// The Mods page.
    pub mods: Mods,
    /// What the HUD mod folder holds (None: no folder chosen).
    pub hud: Option<HudCheck>,
    /// The prepare step (`game --prepare`, docs/prepare.md).
    pub prepare: Prepare,
    /// Where settings, saves, caches and logs are kept (the about screen).
    pub folders: Folders,
}

/// Where Overbright keeps its files (`platform::dirs`), as the about
/// screen shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folders {
    /// A portable install: everything in [`Folders::data`] beside the
    /// programs (docs/platform-dirs.md, "Portable mode").
    pub portable: bool,
    /// Overbright's own folder (options.txt, logs, …).
    pub data: PathBuf,
    /// The log folder.
    pub logs: PathBuf,
    /// The profiles folder.
    pub saves: PathBuf,
}

/// The prepare step's state: where its run is, and what the screens ask
/// of the window, which owns the child process.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prepare {
    pub state: super::prepare::State,
    /// A run is asked for (the window starts it and clears this).
    pub wanted: bool,
    /// The run in progress is to be stopped (Skip).
    pub stop: bool,
    /// The step was offered or skipped in this session: not offered again
    /// by itself.
    pub offered: bool,
    /// The run started by itself (offered), behind the main screen: no
    /// screen of its own, a line beside Play says how far it is, and Play
    /// stops it (the game makes what is missing, docs/prepare.md).
    pub background: bool,
    /// The Graphics row's value when the hardware check said the folder is
    /// prepared (for these settings' programs): a level saved since that
    /// differs makes the step offered (once a session, as at a first run).
    pub prepared_graphics: Option<Graphics>,
}

/// The HUD mod folder's check (`hd_ui_pack::inspect`).
pub type HudCheck = crate::hd_ui_pack::Inspect;

/// The Mods page's state: the installed texture packages in the game's
/// order (first wins), the one chosen, an import running or queued, the
/// last imports' result lines, a removal waiting for its confirmation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mods {
    pub list: Vec<(crate::texture_mods::Entry, crate::texture_mods::Info)>,
    pub sel: usize,
    /// Files and folders to import (taken by the window's worker, or
    /// imported at once where the environment has no worker).
    pub queued: Vec<PathBuf>,
    /// What the running import does now.
    pub busy: Option<String>,
    pub results: Vec<(Mark, String)>,
    /// The package id a Remove waits to be confirmed for.
    pub confirm: Option<String>,
    /// The HUD mod's downloads found so far (`hud_mod`: its HUD part
    /// and its map part; converted once both are known).
    pub hud_parts: crate::hud_mod::Parts,
    /// Both parts are known and wait for the window's worker.
    pub hud_queued: bool,
    /// What the HUD mod's conversion does now.
    pub hud_busy: Option<String>,
    /// The HUD mod's last result lines.
    pub hud_results: Vec<(Mark, String)>,
}

impl Mods {
    pub fn entries(&self) -> Vec<crate::texture_mods::Entry> {
        self.list.iter().map(|(e, _)| e.clone()).collect()
    }
}

/// `game --probe`'s state.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeState {
    Pending,
    Ready(Probe),
    Failed(String),
}

impl ProbeState {
    /// The card (PCI ids).
    pub fn card(&self) -> Option<(u32, u32)> {
        match self {
            ProbeState::Ready(p) => p.card(),
            _ => None,
        }
    }

    /// The display values the game's first start would choose (the probe
    /// gives them once it knew a usable game folder).
    pub fn first_run(&self) -> Option<&FirstRun> {
        match self {
            ProbeState::Ready(p) => p.first_run.as_ref(),
            _ => None,
        }
    }
}

impl Model {
    /// The Resolution row's list for the display path chosen (windowed /
    /// full screen), as the game builds it (`display::modes_for`: windowed
    /// sizes up to the monitor, the monitor's modes for exclusive full
    /// screen, the desktop size alone on Wayland); the fixed list before a
    /// monitor is known.
    pub fn modes(&self) -> Vec<(u32, u32)> {
        crate::render::display::modes_for(self.monitor.as_ref(), self.choices.fullscreen)
    }

    /// The monitor's desktop size, if known.
    pub fn desktop(&self) -> Option<(u32, u32)> {
        self.monitor.as_ref().map(|m| m.desktop).filter(|d| d.0 > 0 && d.1 > 0)
    }
}

/// Where the source code of this build is published: `OB_SOURCE_URL` as
/// the release build sets it (docs/building-releases.md), when it is a web
/// address; None otherwise (an ordinary build, or the release script's
/// placeholder): the about screen then says it is an unofficial build.
pub fn source_url() -> Option<String> {
    source_of(env!("OB_SOURCE_URL"))
}

/// [`source_url`] for a given value.
pub fn source_of(v: &str) -> Option<String> {
    let v = v.trim();
    let web = v.strip_prefix("https://").or_else(|| v.strip_prefix("http://"));
    web.filter(|rest| rest.contains('.') && !rest.contains(char::is_whitespace)).map(|_| v.to_string())
}

/// A failed start, for the failure screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub during_start: bool,
    /// "exit code 101" and the like (`run::Ended::how`).
    pub how: String,
    pub tail: Vec<String>,
    pub log: Option<PathBuf>,
}

/// What the folder browser picks a folder for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrowseFor {
    #[default]
    Game,
    TexturePack,
    Saves,
    /// Texture packages: TexMod `.tpf` files, TexWizard pack folders and
    /// ZIPs of them, listed with the folders.
    Packages,
    /// The HUD mod's downloads (archives listed with the folders).
    HudMod,
}

/// A folder in the browser's list.
#[derive(Clone, Debug, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    /// "game folder", "saves", "" — a dim note at the right.
    pub note: String,
}

/// The folder browser (keyboard and pad friendly; the same on every OS).
/// `dir` empty: the list of drives (Windows).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Browse {
    pub purpose: BrowseFor,
    pub dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub sel: usize,
    pub error: Option<String>,
    /// The typed path.
    pub input: String,
    /// The places shown above the list (name, folder; an empty folder:
    /// Windows' drive list).
    pub places: Vec<(String, PathBuf)>,
}

/// Whether `path` is split as a Windows path on this system (`windows`):
/// the drive list (empty), a drive letter's path or a share's; any other
/// (the fake data's `/data/player`) is split as written, so the renders
/// are the same on every system.
pub fn windows_form(path: &str, windows: bool) -> bool {
    windows
        && (path.is_empty()
            || path.as_bytes().get(1) == Some(&b':')
            || path.starts_with(r"\\")
            || path.starts_with("//"))
}

/// The parts of a folder's path for the browser's clickable path, each
/// with the folder it goes to: the root first (`/`; on Windows "This PC"
/// (the drive list, an empty path), then `C:\` or a share's
/// `\\server\share\`), then each folder. Windows' long-path prefix
/// (`\\?\`, `\\?\UNC\`) is left out of the names and kept in the paths;
/// `/` counts as a separator there too. Pure text: tested on every system.
pub fn path_parts(path: &str, windows: bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if !windows {
        if path.starts_with('/') {
            out.push(("/".to_string(), "/".to_string()));
        }
        let mut acc = if path.starts_with('/') { String::from("/") } else { String::new() };
        for part in path.split('/').filter(|p| !p.is_empty()) {
            if !acc.is_empty() && !acc.ends_with('/') {
                acc.push('/');
            }
            acc.push_str(part);
            out.push((part.to_string(), acc.clone()));
        }
        return out;
    }
    out.push(("This PC".to_string(), String::new()));
    if path.is_empty() {
        return out;
    }
    let p = path.replace('/', "\\");
    // The prefix kept in the paths, the rest split by `\`.
    let (prefix, rest) = if let Some(r) = p.strip_prefix(r"\\?\UNC\") {
        (r"\\?\UNC\", r)
    } else if let Some(r) = p.strip_prefix(r"\\?\") {
        (r"\\?\", r)
    } else if let Some(r) = p.strip_prefix(r"\\") {
        (r"\\", r)
    } else {
        ("", p.as_str())
    };
    let unc = prefix.ends_with(r"UNC\") || prefix == r"\\";
    let mut parts = rest.split('\\').filter(|s| !s.is_empty());
    let root = if unc {
        let server = parts.next().unwrap_or_default();
        let share = parts.next().unwrap_or_default();
        let shown = format!(r"\\{server}\{share}\");
        (shown, format!("{prefix}{server}\\{share}\\"))
    } else {
        let drive = parts.next().unwrap_or_default();
        (format!("{drive}\\"), format!("{prefix}{drive}\\"))
    };
    let mut acc = root.1.clone();
    out.push(root);
    for part in parts {
        if !acc.ends_with('\\') {
            acc.push('\\');
        }
        acc.push_str(part);
        out.push((part.to_string(), acc.clone()));
    }
    out
}

/// The save import.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Import {
    pub scan: Option<saves::Scan>,
    /// The files copied by the last import.
    pub done: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Demo {
    pub cycle: usize,
    pub checked: bool,
    pub text: String,
    pub sel: usize,
}

/// Where the launcher's data comes from and goes to.
pub trait Env {
    /// The first model (folders found, settings read).
    fn load(&self) -> Model;
    /// Checks a game folder (reads only).
    fn check(&self, root: &Path) -> CheckResult;
    /// Keeps `root` as the game folder.
    fn keep_root(&self, root: &Path) -> Result<(), String>;
    /// Writes the choices into options.txt (only after a change); `first`
    /// (the probe's first-run values) for a first write before the game's
    /// first run.
    fn save(&self, c: &Choices, first: Option<&FirstRun>) -> Result<(), String>;
    /// Keeps the texture-pack folder and says what is in it.
    fn set_pack(&self, dir: &str) -> PackStatus;
    fn languages(&self, root: &Path) -> Vec<String>;
    /// The folders in `dir` (empty: the drives), named, sorted, with a
    /// note for game and save folders.
    fn list_dirs(&self, dir: &Path) -> Result<Vec<DirEntry>, String>;
    /// The parent folder (`dir` empty or a drive's root on Windows: the
    /// drives, as an empty path; None at the top).
    fn parent(&self, dir: &Path) -> Option<PathBuf> {
        match dir.parent() {
            Some(p) if !p.as_os_str().is_empty() => Some(p.to_path_buf()),
            _ if cfg!(windows) && !dir.as_os_str().is_empty() => Some(PathBuf::new()),
            _ => None,
        }
    }
    fn home(&self) -> Option<PathBuf>;
    /// The browser's places: the home folder and its usual folders, then
    /// the drives (Windows) or the file system's root and the mounted
    /// drives (elsewhere).
    fn places(&self) -> Vec<(String, PathBuf)> {
        self.home().map(|h| vec![("Home".to_string(), h)]).unwrap_or_default()
    }
    /// Opens a folder in the system's file manager.
    fn open_folder(&self, dir: &Path) -> Result<(), String>;
    /// The original game's save folder, where the system has a known one.
    fn original_saves(&self) -> Option<PathBuf>;
    /// Where Overbright keeps its files now.
    fn folders(&self) -> Folders;
    /// Turns portable mode on (`portable.txt` beside the programs); from
    /// then on this launcher and the game keep everything in the data
    /// folder beside them.
    fn make_portable(&self) -> Result<(), String>;
    /// What a save folder holds, against Overbright's profiles.
    fn scan_saves(&self, from: &Path) -> saves::Scan;
    /// Copies the new saves; the files copied.
    fn import_saves(&self, s: &saves::Scan) -> Result<Vec<String>, String>;
    /// The system's own folder dialog (Windows); None when there is none
    /// or it was cancelled.
    fn native_folder_dialog(&self, _title: &str) -> Option<PathBuf> {
        None
    }
    /// The installed texture packages in the kept order.
    fn mods(&self) -> Vec<(crate::texture_mods::Entry, crate::texture_mods::Info)>;
    /// Keeps the packages' order and on / off.
    fn save_mods(&self, list: &[crate::texture_mods::Entry]) -> Result<(), String>;
    /// Deletes a package's converted folder (never the player's file).
    fn remove_mod(&self, id: &str) -> Result<(), String>;
    /// The texture packages in `dir` (TexMod `.tpf` files, TexWizard pack
    /// folders and ZIPs holding one), sorted.
    fn list_packages(&self, dir: &Path) -> Vec<DirEntry>;
    /// Imports the packages `paths` for the game in `root` (reading its
    /// textures first), adds the installed ones to the list (on, at the
    /// end); one result per path. `progress` gets what is being done.
    fn import(
        &self,
        paths: &[PathBuf],
        root: Option<&Path>,
        progress: &(dyn Fn(String) + Sync),
    ) -> Vec<crate::texture_mods::Outcome>;
    /// The window runs [`Env::import`] on a worker thread (the real
    /// machine); else the screen runs it at once (the fake data).
    fn background_import(&self) -> bool {
        false
    }
    /// What a HUD mod folder holds.
    fn hud(&self, dir: &str) -> Option<HudCheck> {
        let d = dir.trim();
        (!d.is_empty()).then(|| crate::hd_ui_pack::inspect(Path::new(d)))
    }
    /// The archives in `dir` the HUD mod may come as (ZIP, and 7-Zip / RAR
    /// to say they need extracting), sorted.
    fn list_archives(&self, dir: &Path) -> Vec<DirEntry> {
        read_archives(dir)
    }
    /// What the player chose added to the HUD mod's parts
    /// (`hud_mod::add`).
    fn hud_add(&self, parts: &mut crate::hud_mod::Parts, path: &Path) {
        crate::hud_mod::add(parts, path)
    }
    /// Converts the HUD mod's two parts into Overbright's folder.
    fn hud_convert(&self, parts: &crate::hud_mod::Parts, progress: &(dyn Fn(String) + Sync)) -> HudOutcome {
        convert_hud(parts, progress)
    }
}

/// A HUD mod conversion's result lines and, when a pack was written, its
/// folder (to be used and checked).
#[derive(Clone, Debug, PartialEq)]
pub struct HudOutcome {
    pub lines: Vec<(Mark, String)>,
    pub dir: Option<PathBuf>,
}

/// The real conversion (`hud_mod::convert` into `<packs>/hud-mod`).
pub fn convert_hud(parts: &crate::hud_mod::Parts, progress: &(dyn Fn(String) + Sync)) -> HudOutcome {
    let (Some(hud), Some(map)) = (&parts.hud, &parts.map) else {
        return HudOutcome { lines: vec![(Mark::Error, "Both parts of the HUD mod are needed.".into())], dir: None };
    };
    progress("Converting the HUD mod: reading its files…".into());
    let r = crate::hud_mod::convert(hud, map, &crate::platform::dirs::packs(), &|d, t| {
        if d == t {
            progress("Converting the HUD mod: checking the result…".into());
        } else if d % 5 == 0 {
            progress(format!("Converting the HUD mod: {d} of {t} images…"));
        }
    });
    let lines = crate::hud_mod::result_lines(&r, parts)
        .into_iter()
        .map(|(ok, t)| {
            (
                match ok {
                    Some(true) => Mark::Ok,
                    Some(false) => Mark::Error,
                    None => Mark::Warn,
                },
                t,
            )
        })
        .collect();
    HudOutcome { lines, dir: r.ok().map(|s| s.dir) }
}

/// The ZIP, 7-Zip and RAR files in `dir`, sorted by name, with their kind
/// and size as the note.
pub fn read_archives(dir: &Path) -> Vec<DirEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<DirEntry> = rd
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let ext = e.path().extension()?.to_str()?.to_ascii_lowercase();
            let kind = match ext.as_str() {
                "zip" => "ZIP archive",
                "7z" => "7-Zip archive (extract it first)",
                "rar" => "RAR archive (extract it first)",
                _ => return None,
            };
            let mb = e.metadata().map_or(0, |m| m.len()) as f64 / 1e6;
            Some(DirEntry { note: format!("{kind}, {mb:.0} MB"), path: e.path(), name })
        })
        .collect();
    v.sort_by_key(|e| e.name.to_lowercase());
    v
}

/// The texture packages in `dir`, sorted by name, with their kind and size
/// as the note: `.tpf` files ("TexMod package"), TexWizard pack folders
/// and ZIPs holding a TexWizard pack ("TexWizard pack").
pub fn read_packages(dir: &Path) -> Vec<DirEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<DirEntry> = rd
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().into_string().ok()?;
            let ext = |x: &str| path.extension().is_some_and(|e| e.eq_ignore_ascii_case(x));
            let mb = |bytes: u64| bytes as f64 / 1e6;
            let note = if path.is_dir() && crate::formats::texwizard::is_pack_dir(&path) {
                let bin = std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|f| f.file_name().to_string_lossy().eq_ignore_ascii_case("textures.bin"))
                    .map(|f| f.metadata().map_or(0, |m| m.len()))
                    .sum::<u64>();
                format!("TexWizard pack, {:.0} MB", mb(bin))
            } else if path.is_file() && ext("tpf") {
                format!("TexMod package, {:.0} MB", mb(e.metadata().map_or(0, |m| m.len())))
            } else if path.is_file() && ext("zip") && crate::texture_mods::zip_has_texwizard_pack(&path) {
                format!("TexWizard pack (ZIP), {:.0} MB", mb(e.metadata().map_or(0, |m| m.len())))
            } else {
                return None;
            };
            Some(DirEntry { note, path, name })
        })
        .collect();
    v.sort_by_key(|e| e.name.to_lowercase());
    v
}

/// Paths given as packages (dropped, or on the command line) with each
/// folder that is no package (no `texmod.def`, no TexWizard pack) standing
/// for the TexWizard packs in it, when it holds any.
pub fn expand_packages(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .flat_map(|p| {
            let found = if p.is_dir() && !p.join("texmod.def").is_file() && !crate::formats::texwizard::is_pack_dir(&p) {
                crate::formats::texwizard::find_packs(&p)
            } else {
                Vec::new()
            };
            if found.is_empty() { vec![p] } else { found }
        })
        .collect()
}

/// What "Import all here" takes in `dir`: the folder itself when it is a
/// package (loose TexMod files, a TexWizard pack); in a game folder with
/// TexWizard, or its TexturePacks folder, the packs TexWizard loads (most
/// important first); else the packages listed in it, then TexWizard packs
/// in folders below it.
pub fn packages_here(dir: &Path, listed: Vec<DirEntry>) -> Vec<PathBuf> {
    if dir.join("texmod.def").is_file() || crate::formats::texwizard::is_pack_dir(dir) {
        return vec![dir.to_path_buf()];
    }
    let mut out: Vec<PathBuf> = listed.into_iter().map(|e| e.path).collect();
    for p in crate::formats::texwizard::find_packs(dir) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The real import: the game's textures read once, then each path read
/// and installed (`texture_mods::install`), the installed ones added to the
/// kept list.
pub fn import_paths(
    paths: &[PathBuf],
    root: Option<&Path>,
    progress: &(dyn Fn(String) + Sync),
) -> Vec<crate::texture_mods::Outcome> {
    use crate::texture_mods as tm;
    let name_of =
        |p: &Path| p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
    let fail = |why: String| -> Vec<tm::Outcome> {
        paths
            .iter()
            .map(|p| tm::Outcome {
                id: None,
                name: name_of(p),
                line: format!("{}: not imported: {why}", name_of(p)),
                ok: false,
            })
            .collect()
    };
    let Some(root) = root else { return fail("choose the game folder first (the import reads its textures)".into()) };
    progress("Reading your game's textures (a few seconds)…".into());
    let sources = match crate::tpf_import::collect(root) {
        Ok(s) => s,
        Err(e) => return fail(format!("the game's textures could not be read: {e:#}")),
    };
    let packages = tm::packages_dir();
    let mut out = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        let name = name_of(p);
        let n = paths.len();
        let head = if n > 1 { format!("Importing {name} ({} of {n})", i + 1) } else { format!("Importing {name}") };
        progress(format!("{head}: reading the package…"));
        let read = match tm::read_sources(p) {
            Ok(v) => v,
            Err(e) => {
                out.push(tm::outcome_line(&name, &Err(e)));
                continue;
            }
        };
        for (label, pkg) in read {
            let r = tm::install(&pkg, p, &sources, &packages, &|d, t| {
                progress(format!("{head}: {d} of {t} textures converted…"))
            });
            out.push(keep_installed(tm::outcome_line(&label, &r)));
        }
    }
    out
}

/// An installed package added to the kept list (on, at the end).
fn keep_installed(o: crate::texture_mods::Outcome) -> crate::texture_mods::Outcome {
    use crate::texture_mods as tm;
    if let Some(id) = &o.id {
        let mut list = tm::load();
        list.push(tm::Entry { id: id.clone(), on: true });
        if let Err(e) = tm::save(&list) {
            return tm::Outcome { line: format!("{}: imported, but the list could not be kept: {e}", o.name), ok: false, ..o };
        }
    }
    o
}

/// Where a found folder came from, in a few plain words.
pub fn source_words(source: &str, path: &Path) -> String {
    match source {
        "NFSU2_ROOT" => "set by NFSU2_ROOT".into(),
        "kept" => "used last time".into(),
        "this folder" => "the current folder".into(),
        _ if !cfg!(windows) && path.components().any(|c| c.as_os_str() == "drive_c") => "inside a Wine prefix".into(),
        _ => "found on this computer".into(),
    }
}

/// A folder's note in the browser: the game's folder, a save folder.
pub fn dir_note(p: &Path) -> String {
    if setup::looks_like_game(p) {
        "game folder".into()
    } else if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(crate::frontend::profiles::FOLDER)) {
        "saves".into()
    } else {
        String::new()
    }
}

/// The folders in `dir`, sorted by name (hidden ones left out).
pub fn read_dirs(dir: &Path) -> Result<Vec<DirEntry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut v: Vec<DirEntry> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            (!name.starts_with('.')).then(|| DirEntry { note: dir_note(&e.path()), path: e.path(), name })
        })
        .collect();
    v.sort_by_key(|e| e.name.to_lowercase());
    Ok(v)
}

/// The drives mounted for the person logged in (Linux and the like): the folders in
/// `/media/<user>`, `/run/media/<user>` and `/mnt`, sorted.
pub fn mounted() -> Vec<(String, PathBuf)> {
    let user = std::env::var("USER").unwrap_or_default();
    let mut tops = vec![PathBuf::from("/mnt")];
    if !user.is_empty() {
        tops.insert(0, PathBuf::from("/run/media").join(&user));
        tops.insert(0, PathBuf::from("/media").join(&user));
    }
    let mut v: Vec<(String, PathBuf)> = Vec::new();
    for t in tops {
        if let Ok(rd) = std::fs::read_dir(&t) {
            let mut here: Vec<(String, PathBuf)> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| Some((e.file_name().into_string().ok()?, e.path())))
                .collect();
            here.sort();
            v.extend(here);
        }
    }
    v.truncate(6);
    v
}

/// Windows' drives (`A:\` … `Z:\` that exist).
pub fn drives() -> Vec<DirEntry> {
    (b'A'..=b'Z')
        .map(|c| PathBuf::from(format!("{}:\\", c as char)))
        .filter(|p| cfg!(windows) && p.is_dir())
        .map(|p| DirEntry { name: p.display().to_string(), note: String::new(), path: p })
        .collect()
}

pub fn version() -> String {
    format!("version {}", crate::build_info::VERSION)
}

/// The languages other than English, for the "not supported yet" note.
pub fn other_languages(all: &[String]) -> Vec<String> {
    all.iter().filter(|l| !l.eq_ignore_ascii_case("english")).cloned().collect()
}

/// The real machine.
pub struct RealEnv;

impl Env for RealEnv {
    fn load(&self) -> Model {
        let (choices, file) = load_choices();
        let mut found: Vec<Found> = Vec::new();
        let mut add = |p: PathBuf, source: &str| {
            if setup::looks_like_game(&p) && !found.iter().any(|f| f.path == p) {
                let source = source_words(source, &p);
                found.push(Found { path: p, source });
            }
        };
        if let Some(r) = std::env::var_os("NFSU2_ROOT") {
            add(PathBuf::from(r), "NFSU2_ROOT");
        }
        if let Some(r) = crate::qol::setup::saved_root() {
            add(r, "kept");
        }
        if let Ok(cwd) = std::env::current_dir() {
            add(cwd, "this folder");
        }
        for c in setup::candidates() {
            add(c, "found");
        }
        let pack = choices.texture_pack.clone();
        Model {
            version: version(),
            root: None,
            found,
            found_sel: 0,
            path_input: String::new(),
            check: None,
            saved: choices.clone(),
            choices,
            monitor: None,
            languages: Vec::new(),
            pack_status: pack_status(&pack),
            settings_file: file,
            message: None,
            demo: Demo::default(),
            probe: ProbeState::Pending,
            controller: None,
            failure: None,
            browse: Browse::default(),
            import: Import::default(),
            logs_dir: super::dirs::logs(),
            saves_dir: super::dirs::profiles(),
            build: crate::build_info::version_line(),
            source: source_url(),
            notices: super::screens::NOTICES,
            native_dialog: cfg!(windows),
            mods: Mods { list: self.mods(), ..Default::default() },
            hud: self.hud(&pack),
            prepare: Prepare::default(),
            folders: self.folders(),
        }
    }

    fn check(&self, root: &Path) -> CheckResult {
        let c = setup::check(root, true);
        let fp = crate::fingerprint::compute(root);
        let lines: Vec<(Mark, String)> =
            setup::check_lines(&c, Some(&fp)).into_iter().map(|(m, t)| (m.into(), t)).collect();
        // A folder with an error line cannot be used, files all there or
        // not.
        let usable = c.ok() && !lines.iter().any(|(m, _)| *m == Mark::Error);
        CheckResult { root: root.to_path_buf(), lines, usable }
    }

    fn keep_root(&self, root: &Path) -> Result<(), String> {
        crate::qol::setup::save_root(root).map_err(|e| e.to_string())
    }

    fn save(&self, c: &Choices, first: Option<&FirstRun>) -> Result<(), String> {
        save_choices(c, first).map_err(|e| e.to_string())
    }

    fn set_pack(&self, dir: &str) -> PackStatus {
        let d = dir.trim();
        let r = crate::qol::setup::save_texture_pack(if d.is_empty() { None } else { Some(Path::new(d)) });
        if let Err(e) = r {
            log::warn!("texture-pack folder not kept: {e}");
        }
        pack_status(d)
    }

    fn languages(&self, root: &Path) -> Vec<String> {
        setup::installed_languages(root)
    }

    fn list_dirs(&self, dir: &Path) -> Result<Vec<DirEntry>, String> {
        if dir.as_os_str().is_empty() { Ok(drives()) } else { read_dirs(dir) }
    }

    fn home(&self) -> Option<PathBuf> {
        super::dirs::home()
    }

    fn places(&self) -> Vec<(String, PathBuf)> {
        let mut v = Vec::new();
        if let Some(h) = self.home() {
            v.push(("Home".to_string(), h.clone()));
            for n in ["Desktop", "Documents", "Downloads"] {
                if h.join(n).is_dir() {
                    v.push((n.to_string(), h.join(n)));
                }
            }
        }
        if cfg!(windows) {
            v.push(("This PC".to_string(), PathBuf::new()));
            v.extend(drives().into_iter().map(|d| (d.name.trim_end_matches('\\').to_string(), d.path)));
        } else {
            v.push(("File system".to_string(), PathBuf::from("/")));
            v.extend(mounted());
        }
        v
    }

    fn open_folder(&self, dir: &Path) -> Result<(), String> {
        super::run::open_folder(dir)
    }

    fn folders(&self) -> Folders {
        Folders {
            portable: crate::platform::dirs::portable_base().is_some(),
            data: super::dirs::data_dir(),
            logs: super::dirs::logs(),
            saves: super::dirs::profiles(),
        }
    }

    fn make_portable(&self) -> Result<(), String> {
        let dir = crate::platform::dirs::program_dir().ok_or("the program's folder is not known")?;
        crate::platform::dirs::make_portable(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        crate::platform::dirs::refresh_portable();
        // The rest of this session follows: the log, and the one-launcher
        // lock (kept until the process ends).
        super::start_log::follow(&super::dirs::data_dir());
        static LOCK: std::sync::Mutex<Option<std::fs::File>> = std::sync::Mutex::new(None);
        if let Ok(Some(l)) = super::run::single_instance(&super::dirs::lock_file())
            && let Ok(mut g) = LOCK.lock()
        {
            *g = Some(l);
        }
        Ok(())
    }

    fn original_saves(&self) -> Option<PathBuf> {
        super::dirs::original_saves()
    }

    fn scan_saves(&self, from: &Path) -> saves::Scan {
        saves::scan(from, &super::dirs::profiles())
    }

    fn import_saves(&self, s: &saves::Scan) -> Result<Vec<String>, String> {
        saves::import(s, &super::dirs::profiles())
    }

    fn native_folder_dialog(&self, title: &str) -> Option<PathBuf> {
        #[cfg(windows)]
        {
            super::win_dialog::pick_folder(title)
        }
        #[cfg(not(windows))]
        {
            let _ = title;
            None
        }
    }

    fn mods(&self) -> Vec<(crate::texture_mods::Entry, crate::texture_mods::Info)> {
        crate::texture_mods::installed(&crate::texture_mods::packages_dir(), &crate::texture_mods::load())
    }

    fn save_mods(&self, list: &[crate::texture_mods::Entry]) -> Result<(), String> {
        crate::texture_mods::save(list).map_err(|e| e.to_string())
    }

    fn remove_mod(&self, id: &str) -> Result<(), String> {
        let mut list = crate::texture_mods::load();
        crate::texture_mods::remove(&crate::texture_mods::packages_dir(), &mut list, id)
            .map_err(|e| format!("{e:#}"))?;
        crate::texture_mods::save(&list).map_err(|e| e.to_string())
    }

    fn list_packages(&self, dir: &Path) -> Vec<DirEntry> {
        read_packages(dir)
    }

    fn import(
        &self,
        paths: &[PathBuf],
        root: Option<&Path>,
        progress: &(dyn Fn(String) + Sync),
    ) -> Vec<crate::texture_mods::Outcome> {
        import_paths(paths, root, progress)
    }

    fn background_import(&self) -> bool {
        true
    }
}

pub fn pack_status(dir: &str) -> PackStatus {
    let d = dir.trim();
    if d.is_empty() {
        PackStatus::None
    } else if !Path::new(d).is_dir() {
        PackStatus::NotFound
    } else if crate::render::enh::assets::hd_ui::is_complete(Path::new(d)) {
        PackStatus::Complete
    } else {
        PackStatus::Incomplete
    }
}

/// The choices as options.txt has them (the game's defaults without one)
/// and whether it exists.
pub fn load_choices() -> (Choices, bool) {
    let exists = super::dirs::options_file().exists();
    let s = Settings::load_text();
    (choices_of(&s), exists)
}

pub fn choices_of(s: &Settings) -> Choices {
    Choices {
        fullscreen: s.fullscreen,
        resolution: s.resolution,
        graphics: Graphics::of(&s.enh),
        skip: s.enh.launcher_skip,
        skip_movies: s.enh.skips_movies(),
        skip_title: s.enh.skips_title(),
        texture_pack: crate::qol::setup::saved_texture_pack().map(|p| p.display().to_string()).unwrap_or_default(),
        hud_mod: s.enh.hd_ui,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    /// No options.txt yet and the first-run display values not known: the
    /// game's first run picks the display values for the hardware
    /// (0x5CECA0) only while there is none, so the launcher does not write
    /// one (the probe gives them once a usable game folder is known).
    FirstRunPending,
    Io(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::FirstRunPending => {
                write!(f, "the game has not started once yet (it chooses the display settings for this computer first)")
            }
            SaveError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// `c` applied to `s` as the game's own pages would: the display mode and
/// size, the skip row, Skip Intro Movies and Skip Title Screen, and the Graphics Mode only when the player picked
/// Original or Enhanced (Original also puts Customize Display back to the
/// original's values for the World Detail level, as the game's Original
/// does; Enhanced writes the launcher's quality level).
pub fn apply_choices(s: &mut Settings, c: &Choices) {
    s.fullscreen = c.fullscreen;
    s.resolution = c.resolution;
    s.enh.launcher_skip = c.skip;
    s.enh.set_skips(c.skip_movies, c.skip_title);
    s.enh.hd_ui = c.hud_mod;
    if Graphics::of(&s.enh) == c.graphics {
        return;
    }
    match c.graphics {
        Graphics::Original => {
            Preset::Original.apply(&mut s.enh);
            s.native_display();
        }
        Graphics::Enhanced => {
            ENHANCED_LEVEL.apply(&mut s.enh);
            crate::frontend::enh::graphics_table::normalize(s);
        }
        _ => {}
    }
}

/// Writes the choices through the game's own code (`Settings::load_text`
/// → change → `write_file`). Without options.txt it starts from the game's
/// defaults plus the first-run display values `game --probe` computed
/// (`first`: `first_run_display(card)` in the game — the launcher itself
/// reads no game file), the
/// detail estimate applied as `first_run_display` does
/// (`estimated_detail`); exactly what the game's first start writes. With
/// no first-run values it writes nothing ([`SaveError::FirstRunPending`]).
pub fn save_choices(c: &Choices, first: Option<&FirstRun>) -> Result<(), SaveError> {
    let mut s = if super::dirs::options_file().exists() {
        Settings::load_text()
    } else if let Some(f) = first {
        let mut d = Settings::default();
        d.estimated_detail(f.detail);
        d
    } else {
        return Err(SaveError::FirstRunPending);
    };
    apply_choices(&mut s, c);
    s.write_file().map_err(|e| SaveError::Io(e.to_string()))
}

/// Fixed data for the renders and tests.
pub struct FakeEnv;

impl FakeEnv {
    pub fn root() -> PathBuf {
        PathBuf::from("/data/player/Games/Need for Speed Underground 2")
    }
}

impl Env for FakeEnv {
    fn load(&self) -> Model {
        let choices = Choices {
            fullscreen: true,
            resolution: (1920, 1080),
            graphics: Graphics::Original,
            skip: false,
            skip_movies: false,
            skip_title: false,
            texture_pack: String::new(),
            hud_mod: false,
        };
        Model {
            version: format!("version {}", env!("CARGO_PKG_VERSION")),
            root: Some(FakeEnv::root()),
            found: vec![
                Found { path: FakeEnv::root(), source: "found on this computer".into() },
                Found {
                    path: PathBuf::from(
                        "/data/player/.wine/drive_c/Program Files (x86)/EA GAMES/Need for Speed Underground 2",
                    ),
                    source: "inside a Wine prefix".into(),
                },
                Found { path: PathBuf::from("/mnt/backup/old-install/NFSU2"), source: "used last time".into() },
            ],
            found_sel: 0,
            path_input: String::new(),
            check: Some(self.check(&FakeEnv::root())),
            saved: choices.clone(),
            choices,
            monitor: Some(MonitorInfo {
                name: None,
                desktop: (1920, 1080),
                modes: vec![(1280, 720), (1600, 900), (1920, 1080)],
                exclusive: true,
            }),
            languages: ["English", "French", "German", "Italian", "Spanish", "Swedish"].map(String::from).to_vec(),
            pack_status: PackStatus::None,
            settings_file: true,
            message: None,
            demo: Demo { cycle: 1, checked: true, text: "/data/player/packs/hd-ui".into(), sel: 1 },
            probe: ProbeState::Ready(FakeEnv::probe()),
            controller: Some("Xbox Wireless Controller".into()),
            failure: Some(Failure {
                during_start: true,
                how: "a crash (panic), exit code 101".into(),
                tail: [
                    "game folder /data/player/Games/Need for Speed Underground 2",
                    "allocator: mimalloc (fast)",
                    "GPU adapter: Example GPU (DiscreteGpu, Vulkan)",
                    "thread 'main' panicked at src/render/core/gpu.rs:120:14:",
                    "requesting device: the device was lost (an example message long enough to need shortening at the smaller window sizes)",
                    "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
                ]
                .map(String::from)
                .to_vec(),
                log: Some(PathBuf::from("/data/player/.local/share/overbright/logs/launcher-20261112-210501.log")),
            }),
            browse: Browse {
                purpose: BrowseFor::Game,
                dir: PathBuf::from("/data/player/Games"),
                entries: self.list_dirs(Path::new("/data/player/Games")).unwrap_or_default(),
                sel: 3,
                error: None,
                input: String::new(),
                places: self.places(),
            },
            import: Import { scan: Some(self.scan_saves(Path::new(FAKE_SAVES))), done: None },
            logs_dir: PathBuf::from("/data/player/.local/share/overbright/logs"),
            saves_dir: PathBuf::from("/data/player/.local/share/NFS Underground 2"),
            build: format!("Overbright {} (commit unknown, linux-x86_64)", env!("CARGO_PKG_VERSION")),
            source: Some("https://example.org/overbright".into()),
            notices: FAKE_NOTICES,
            native_dialog: false,
            mods: Mods { list: self.mods(), ..Default::default() },
            hud: None,
            prepare: Prepare::default(),
            folders: self.folders(),
        }
    }

    fn check(&self, root: &Path) -> CheckResult {
        let c = setup::Check {
            missing: Vec::new(),
            different: vec!["GLOBAL/GlobalB.lzc".into(), "FRONTEND/FrontB.lzc".into()],
        };
        let mut lines: Vec<(Mark, String)> =
            setup::check_lines(&c, None).into_iter().map(|(m, t)| (m.into(), t)).collect();
        lines.insert(
            0,
            (
                Mark::Warn,
                "Version not recognised (2 files differ from the known release); the game may look or play differently"
                    .into(),
            ),
        );
        lines.push((
            Mark::Info,
            "Mod files found next to the game: dinput8.dll, SCRIPTS (Overbright ignores them)".into(),
        ));
        CheckResult { root: root.to_path_buf(), lines, usable: true }
    }

    fn keep_root(&self, _root: &Path) -> Result<(), String> {
        Ok(())
    }

    fn save(&self, _c: &Choices, _first: Option<&FirstRun>) -> Result<(), String> {
        Ok(())
    }

    fn set_pack(&self, dir: &str) -> PackStatus {
        if dir.trim().is_empty() { PackStatus::None } else { PackStatus::Incomplete }
    }

    fn languages(&self, _root: &Path) -> Vec<String> {
        ["English", "French", "German"].map(String::from).to_vec()
    }

    fn list_dirs(&self, dir: &Path) -> Result<Vec<DirEntry>, String> {
        let names: &[(&str, &str)] = if dir == Path::new("/data/player/Games") {
            &[
                ("Emulators", ""),
                ("GOG", ""),
                ("Heroic", ""),
                ("Need for Speed Underground 2", "game folder"),
                ("Old saves", ""),
                ("Racing", ""),
                ("Steam library", ""),
                ("Tools", ""),
            ]
        } else {
            &[("Documents", ""), ("Games", ""), ("Music", "")]
        };
        Ok(names
            .iter()
            .map(|(n, note)| DirEntry { name: n.to_string(), path: dir.join(n), note: note.to_string() })
            .collect())
    }

    fn home(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/data/player"))
    }

    fn places(&self) -> Vec<(String, PathBuf)> {
        vec![
            ("Home".to_string(), PathBuf::from("/data/player")),
            ("Downloads".to_string(), PathBuf::from("/data/player/Downloads")),
            ("File system".to_string(), PathBuf::from("/")),
            ("USB drive".to_string(), PathBuf::from("/media/player/USB drive")),
        ]
    }

    fn open_folder(&self, _dir: &Path) -> Result<(), String> {
        Ok(())
    }

    fn folders(&self) -> Folders {
        Folders {
            portable: false,
            data: PathBuf::from("/data/player/.local/share/overbright"),
            logs: PathBuf::from("/data/player/.local/share/overbright/logs"),
            saves: PathBuf::from("/data/player/.local/share/NFS Underground 2"),
        }
    }

    fn make_portable(&self) -> Result<(), String> {
        Ok(())
    }

    fn original_saves(&self) -> Option<PathBuf> {
        None
    }

    fn scan_saves(&self, from: &Path) -> saves::Scan {
        let item = |name: &str, profile, bytes, exists| saves::Item {
            name: name.into(),
            profile,
            files: vec![name.into()],
            bytes,
            exists,
            dir: from.join(name),
        };
        saves::Scan {
            from: from.to_path_buf(),
            items: vec![
                item("NIGHTOWL", true, 151_016, false),
                item("NIGHTOWL DVD 1", false, 98_336, false),
                item("NIGHTOWL Magazine 3", false, 98_336, false),
                item("RACER", true, 151_016, true),
            ],
        }
    }

    fn import_saves(&self, s: &saves::Scan) -> Result<Vec<String>, String> {
        Ok(s.to_copy().map(|i| format!("{0}/{0}", i.name)).collect())
    }

    fn mods(&self) -> Vec<(crate::texture_mods::Entry, crate::texture_mods::Info)> {
        let info = |id: &str, name: &str, source: &str, images, matched, bytes| crate::texture_mods::Info {
            id: id.into(),
            name: name.into(),
            source: source.into(),
            kind: "TexMod".into(),
            images,
            matched,
            unmatched: images - matched,
            unsupported: 0,
            files: matched,
            bytes,
        };
        vec![
            (
                crate::texture_mods::Entry { id: "roads-hd".into(), on: true },
                info("roads-hd", "City roads HD", "roads-hd.tpf", 110, 110, 278_000_000),
            ),
            (
                crate::texture_mods::Entry { id: "garage-redux".into(), on: false },
                info("garage-redux", "Garage and shop redux", "garage-redux.tpf", 40, 31, 52_000_000),
            ),
        ]
    }

    fn save_mods(&self, _list: &[crate::texture_mods::Entry]) -> Result<(), String> {
        Ok(())
    }

    fn remove_mod(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    fn list_packages(&self, dir: &Path) -> Vec<DirEntry> {
        if dir == Path::new("/data/player/Downloads") {
            ["NFSU2 Redux HD Textures.tpf", "roads-hd.tpf"]
                .iter()
                .map(|n| DirEntry { name: n.to_string(), path: dir.join(n), note: "TexMod package, 120 MB".into() })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn import(
        &self,
        paths: &[PathBuf],
        _root: Option<&Path>,
        _progress: &(dyn Fn(String) + Sync),
    ) -> Vec<crate::texture_mods::Outcome> {
        paths
            .iter()
            .map(|p| {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if name.contains("GTA") {
                    crate::texture_mods::Outcome {
                        id: None,
                        line: format!("{name}: not imported: it was made for another game (GTA-SA.EXE), not Need for Speed Underground 2"),
                        name,
                        ok: false,
                    }
                } else {
                    crate::texture_mods::Outcome {
                        id: Some("redux".into()),
                        line: format!("{name}: 412 of 430 textures replace the game's; 18 not found in your game's files (left out)"),
                        name,
                        ok: true,
                    }
                }
            })
            .collect()
    }

    fn list_archives(&self, dir: &Path) -> Vec<DirEntry> {
        if dir == Path::new("/data/player/mods") {
            [("map-part.zip", "ZIP archive, 45 MB"), ("hud-part.7z", "7-Zip archive (extract it first), 3 MB")]
                .iter()
                .map(|(n, note)| DirEntry { name: n.to_string(), path: dir.join(n), note: note.to_string() })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn hud_add(&self, parts: &mut crate::hud_mod::Parts, path: &Path) {
        use crate::hud_mod::Source;
        parts.notes.clear();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.ends_with(".7z") {
            parts.notes.push(format!(
                "“{name}” is a 7-Zip archive, which Overbright does not open: extract it first (with 7-Zip, or your \
                 file manager), then choose the extracted folder."
            ));
        } else if name.contains("hud-part") {
            parts.hud = Some(Source::Dir(path.to_path_buf()));
        } else if name.contains("map-part") {
            parts.map = Some(Source::Dir(path.to_path_buf()));
        } else {
            parts.notes.push(format!(
                "“{name}” is not the HUD mod: it holds neither its HUD part (assets/userstart.end) nor its \
                 map part (Binary/script.end)."
            ));
        }
    }

    fn hud_convert(&self, _parts: &crate::hud_mod::Parts, _progress: &(dyn Fn(String) + Sync)) -> HudOutcome {
        HudOutcome {
            lines: vec![(
                Mark::Ok,
                "HUD mod set up: 595 HUD images and 115 maps converted, the maps calibrated for 27 tracks. The \
                 check finds it complete."
                    .into(),
            )],
            dir: Some(PathBuf::from("/data/player/.local/share/overbright/texture-pack/hud-mod")),
        }
    }

    fn hud(&self, dir: &str) -> Option<HudCheck> {
        let d = dir.trim();
        if d.ends_with("hud-mod") {
            return Some(HudCheck { is_pack: true, listed: 710, present: 710, missing: Vec::new(), maps: true, complete: true });
        }
        (!d.is_empty()).then(|| HudCheck {
            is_pack: true,
            listed: 710,
            present: 708,
            missing: vec!["4A7C21F0.png".into(), "4A7C21F1.png".into()],
            maps: true,
            complete: false,
        })
    }
}

/// The fake data's third-party notices (fixed sample text).
pub const FAKE_NOTICES: &str = "# Third-party notices\n\nOverbright is licensed under the GNU General Public \
License v3.0 or later (see LICENSE). It contains work by others, each under its own licence, whose notices \
are kept below.\n\n## Example library\n\nUsed in `src/example.rs`. Upstream: an example project.\n\n\
Copyright (c) 2020 Example Authors. All rights reserved.\n\nPermission is hereby granted, free of charge, \
to any person obtaining a copy of this software, to deal in the Software without restriction, subject to \
the following conditions:\n\n- The above copyright notice shall be included in all copies.\n- The software \
is provided \"as is\", without warranty of any kind.\n\n## Example font\n\nUsed by the launcher. Licence: \
BSD-3-Clause.\n";

/// The fake save folder (inside a Wine prefix).
pub const FAKE_SAVES: &str = "/data/player/.wine/drive_c/users/player/AppData/Local/NFS Underground 2";

impl FakeEnv {
    /// A probe as a real one reads (fixed values).
    pub fn probe() -> Probe {
        Probe {
            version: format!("Overbright {} (commit unknown, linux-x86_64)", env!("CARGO_PKG_VERSION")),
            adapter: Some(super::probe::Adapter {
                name: "Example GPU 7000".into(),
                backend: "Vulkan".into(),
                kind: "DiscreteGpu".into(),
                vendor: 0x1002,
                device: 0x1234,
                driver: "example 1.0".into(),
                vram: 16 << 30,
            }),
            gpu_class: "high".into(),
            monitor: Some((1920, 1080, 60_000)),
            monitor_modes: vec![(1280, 720), (1600, 900), (1920, 1080)],
            exclusive: true,
            modes: crate::render::display::mode_list(&[(1920, 1080)]),
            settings_file: true,
            prepared: Some(true),
            prepared_programs: Some(0),
            first_run: Some(FirstRun {
                resolution: (1280, 960),
                fullscreen: false,
                detail: 3,
                custom: vec![1, 1, 2, 1, 1, 1, 2, 2, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 0],
            }),
            error: None,
        }
    }
}

impl Default for Demo {
    fn default() -> Demo {
        Demo { cycle: 0, checked: false, text: String::new(), sel: 0 }
    }
}
