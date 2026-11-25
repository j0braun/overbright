//! What the launcher's screens show and change, and where it comes from:
//! [`Env`] is the real machine ([`RealEnv`]: the game-folder check, the
//! game's own settings code) or fixed fake data ([`FakeEnv`], for the
//! headless renders and tests; no game files needed).

use ob_look::words;
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
    /// The folder checked: the one chosen, or the game folder one level
    /// above or below it ([`game_dir_near`]), with its long names.
    pub root: PathBuf,
    pub lines: Vec<(Mark, String)>,
    /// No key file is missing and no line is an error (the game starts
    /// with it).
    pub usable: bool,
    /// Why the folder cannot be used (None: it can).
    pub problem: Option<Problem>,
    /// The check in the player's terms (the check screen's verdict,
    /// summary rows and details); None for a folder refused before the
    /// file check (unreadable, Overbright's own).
    pub summary: Option<super::check::CheckSummary>,
}

/// Why a chosen folder cannot be used: each has a title, one sentence and
/// one fixing button on the check screen (`screens::problem_words`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Overbright's own folder (the commonest wrong pick).
    OwnFolder,
    /// No game here at all (or most of it missing).
    NoGame,
    /// Some of the files Overbright needs are missing (how many).
    Missing(usize),
    /// The folder cannot be read (no access).
    Unreadable,
    /// The text file the game reads its menus and text from is the only
    /// file missing (a copy with another language's text only): the game
    /// does not start without it.
    NoText,
}

/// A problem the launcher shows, one per kind of situation: its title,
/// cause and fix in the player's words ([`Issue::words`]), and the buttons
/// that do the fix (the first is focused). Detected from the check's and
/// the start's own results, never from message text.
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// Overbright's own folder was chosen.
    OwnFolder,
    /// No game files in the folder.
    NoGame,
    /// Another game's folder (by its name).
    OtherGame { name: String },
    /// Files the game needs are not there.
    Missing { files: Vec<String> },
    /// The text file the game reads is not there (the languages the copy
    /// has instead).
    NoText { others: Vec<String> },
    /// An older version than 1.2 (its version).
    NeedsPatch { version: String },
    /// The folder cannot be read.
    Unreadable,
    /// Some files differ from 1.2 (a mod in the game folder): not blocking.
    Modified { files: Vec<String> },
    /// The saved game folder is gone (an unplugged drive, a moved game).
    FolderGone { path: PathBuf },
    /// This computer has no usable Vulkan driver.
    NoVulkan,
    /// The game is running already.
    AlreadyRunning,
    /// The game's window could not open.
    WindowFailed,
    /// The last start did not reach the game.
    StartUnfinished,
    /// The game stopped during play (after `minutes`, when known).
    Crashed { minutes: Option<u32> },
    /// Overbright cannot write its own files.
    CannotWrite { path: PathBuf },
    /// A mod file Overbright cannot read (its extension).
    ModUnreadable { ext: String },
    /// The HUD mod lacks files.
    HudIncomplete { missing: usize },
}

/// An issue's words: a title naming the problem, the cause and the fix,
/// and the buttons (`(id, label)`, the first focused).
#[derive(Clone, Debug, PartialEq)]
pub struct Words {
    pub title: String,
    pub text: String,
    pub buttons: Vec<(super::ui::Id, &'static str)>,
}

/// The button that opens the folder choice.
pub const CHOOSE_GAME: (&str, &str) = ("another", "Choose the game folder…");
/// The button that checks the folder again.
pub const CHECK_AGAIN: (&str, &str) = ("recheck", "Check again");

impl Issue {
    pub fn words(&self) -> Words {
        let w = |title: &str, text: String, buttons: Vec<(super::ui::Id, &'static str)>| Words {
            title: title.into(),
            text,
            buttons,
        };
        let another = ("another", "Choose another folder…");
        match self {
            Issue::OwnFolder => w(
                "This is Overbright's folder",
                "Overbright and the game live in separate folders. Choose the folder the game was installed to.".into(),
                vec![CHOOSE_GAME],
            ),
            Issue::NoGame => w(
                "No game here",
                "This folder doesn't contain Need for Speed Underground 2. Choose the folder with GLOBAL, FRONTEND \
                 and TRACKS in it."
                    .into(),
                vec![CHOOSE_GAME],
            ),
            Issue::OtherGame { name } => Words {
                title: format!("This is {name}"),
                text: "Overbright only plays Need for Speed Underground 2. Choose that game's folder.".into(),
                buttons: vec![CHOOSE_GAME],
            },
            Issue::Missing { files } => w(
                "Some game files are missing",
                format!(
                    "{} the game needs {} not in this folder. This happens when an install or a copy stopped \
                     early. Reinstall the game from its discs, or copy the whole folder again. Then check again.",
                    count(files.len(), "file", "files"),
                    if files.len() == 1 { "is" } else { "are" }
                ),
                vec![CHECK_AGAIN, ("details", "Show the missing files"), another],
            ),
            Issue::NoText { others } => w(
                "The game's text file is missing",
                format!(
                    "The game reads its menus and text from a file in LANGUAGES, such as {}, and this copy has no \
                     file Overbright can show{}. The game can't start without one. Choose a copy that has one.",
                    crate::game_assets::text_file_of(crate::game_assets::DEFAULT_LANGUAGE),
                    match others.len() {
                        0 => String::new(),
                        1 => format!(" (its text is in {}, which isn't supported yet)", others[0]),
                        _ => format!(" (its text is in {} and other languages not supported yet)", others[0]),
                    }
                ),
                vec![another, ("details", "Details"), CHECK_AGAIN],
            ),
            Issue::NeedsPatch { version } => w(
                "Patch 1.2 needed",
                format!(
                    "This copy is version {version}. Overbright works with version 1.2. Install the official 1.2 \
                     patch, then check again."
                ),
                vec![CHECK_AGAIN, ("patch_help", "How to get patch 1.2"), another],
            ),
            Issue::Unreadable => w(
                "Overbright can't open this folder",
                "Your account isn't allowed to read it. Choose a folder you can open, or copy the game to one you \
                 own."
                    .into(),
                vec![another],
            ),
            Issue::Modified { files } => w(
                "Ready",
                format!(
                    "{} {} not the original ones. The game may look or play differently.",
                    count(files.len(), "game file", "game files"),
                    if files.len() == 1 { "is" } else { "are" }
                ),
                vec![("details", "Details")],
            ),
            Issue::FolderGone { .. } => w(
                "The game folder is not there",
                "If it is on a drive that is unplugged, plug it in and check again. If you moved the game, choose \
                 where it is now."
                    .into(),
                vec![CHECK_AGAIN, CHOOSE_GAME],
            ),
            Issue::NoVulkan => w(
                "This computer has no Vulkan graphics driver",
                "Overbright draws with Vulkan. Install the latest driver for your graphics card, then check again."
                    .into(),
                vec![("probe_again", "Check again"), ("driver_help", "How to install a driver")],
            ),
            Issue::AlreadyRunning => {
                w("The game is already running", "Switch to its window, or close it first.".into(), vec![("ok", words::OK)])
            }
            Issue::WindowFailed => w(
                "The game window didn't open",
                "This often happens with a full-screen size the screen doesn't support. Start once in a window, \
                 then choose another size."
                    .into(),
                vec![("windowed", "Start in a window"), ("settings", "Settings")],
            ),
            Issue::StartUnfinished => w(
                "The last start didn't finish",
                "The game closed before its menu appeared. Start with safe settings: the original graphics, in a \
                 window, without mods. You can change them back in Settings."
                    .into(),
                vec![("safe", "Start with safe settings"), ("play", "Start as usual"), ("settings", "Settings")],
            ),
            Issue::Crashed { minutes } => Words {
                title: "The game closed unexpectedly".into(),
                text: format!(
                    "{}Your progress up to the last save is kept. Start it again. If it keeps happening, save a \
                     support file and attach it to your report.",
                    match minutes {
                        Some(0) => "It stopped in its first minute of play. ".to_string(),
                        Some(m) => format!("It stopped after {} of play. ", count(*m as usize, "minute", "minutes")),
                        None => String::new(),
                    }
                ),
                buttons: vec![
                    ("retry", "Play again"),
                    ("support", "Save support file…"),
                    ("safe", "Start with safe settings"),
                ],
            },
            Issue::CannotWrite { path } => Words {
                title: "Overbright can't save its files".into(),
                text: format!(
                    "The disk is full or this folder is read-only: {}. Free some space, or move Overbright to a \
                     folder you own.",
                    path.display()
                ),
                buttons: vec![CHECK_AGAIN],
            },
            Issue::ModUnreadable { ext } => Words {
                title: "This file isn't a texture pack Overbright can read".into(),
                text: format!("It is a .{ext} archive. Extract it first, then add the folder."),
                buttons: vec![("add", "Add a mod…")],
            },
            Issue::HudIncomplete { missing } => Words {
                title: "The HUD mod is incomplete".into(),
                text: format!(
                    "{} of its files {} missing. Add the missing download, or turn the HUD mod off.",
                    missing,
                    if *missing == 1 { "is" } else { "are" }
                ),
                buttons: vec![("hud_add", "Add a mod…"), ("hud", "Turn it off")],
            },
        }
    }

    /// Play cannot do its job while this issue stands.
    pub fn blocks_play(&self) -> bool {
        !matches!(self, Issue::Modified { .. } | Issue::ModUnreadable { .. } | Issue::HudIncomplete { .. })
    }

    pub fn mark(&self) -> super::ui::Mark {
        match self {
            Issue::Modified { .. } => super::ui::Mark::Info,
            Issue::HudIncomplete { .. } | Issue::StartUnfinished => super::ui::Mark::Warn,
            _ => super::ui::Mark::Error,
        }
    }

    /// The issue of a refused folder check.
    pub fn of_problem(p: Problem) -> Issue {
        match p {
            Problem::OwnFolder => Issue::OwnFolder,
            Problem::NoGame => Issue::NoGame,
            Problem::Missing(n) => Issue::Missing { files: vec![String::new(); n] },
            Problem::Unreadable => Issue::Unreadable,
            Problem::NoText => Issue::NoText { others: Vec::new() },
        }
    }
}

impl From<Problem> for Issue {
    fn from(p: Problem) -> Issue {
        Issue::of_problem(p)
    }
}

/// "1 file", "12 files".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// How a check reads at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ready,
    /// Usable, with notes (files that differ from the known release).
    Notes,
    Refused(Problem),
}

impl CheckResult {
    pub fn verdict(&self) -> Verdict {
        match self.state() {
            super::check::CheckState::Refused(p) => Verdict::Refused(p),
            super::check::CheckState::Ready => Verdict::Ready,
            _ => Verdict::Notes,
        }
    }

    /// The check screen's state: refused, or the summary's
    /// ([`super::check::CheckSummary::state`]); without a summary, by the
    /// lines (a warning: files that differ).
    pub fn state(&self) -> super::check::CheckState {
        use super::check::CheckState;
        match (self.problem, &self.summary) {
            (Some(p), _) => CheckState::Refused(p),
            _ if !self.usable => CheckState::Refused(Problem::NoGame),
            (None, Some(s)) => s.state(),
            (None, None) if self.lines.iter().any(|(m, _)| *m == Mark::Warn) => CheckState::Changed,
            (None, None) => CheckState::Ready,
        }
    }

    /// The issue of a refused folder, with what the check knows (the
    /// missing files, the languages a copy without the text file has).
    pub fn issue(&self) -> Option<Issue> {
        let p = self.problem.or((!self.usable).then_some(Problem::NoGame))?;
        let s = self.summary.as_ref();
        Some(match p {
            Problem::NoText => Issue::NoText { others: s.and_then(|s| s.no_text()).unwrap_or_default() },
            Problem::Missing(_) if s.is_some() => {
                Issue::Missing { files: s.map(|s| s.files_missing.clone()).unwrap_or_default() }
            }
            p => Issue::of_problem(p),
        })
    }
}

/// The Graphics row: the launcher offers the game's Graphics Mode,
/// Original or Enhanced (docs/developing/settings-page.md "Graphics mode and quality
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
    /// The game's text language (`language.txt`; "" = none kept: English,
    /// or the copy's own language when it has no English).
    pub language: String,
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

/// A question over a page (its buttons; the safe one focused).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// Quit the launcher without starting the game?
    Quit,
    /// Settings back to the suggested values?
    Reset,
    /// Remove this texture pack (its id)?
    RemoveMod(String),
    /// Two packs change the same textures.
    Conflict,
    /// Put this folder there (copy the files first, or start empty)?
    MoveLocation(crate::platform::dirs::Location, PathBuf),
    /// Turn portable mode on?
    PortableOn,
    /// Turn portable mode off?
    PortableOff,
    /// Delete this profile (Saves)?
    SaveDelete(String),
    /// Make a copy of this profile?
    SaveDuplicate(String),
    /// Restore this backup over a profile of its name, or as a copy?
    SaveRestore(super::saves::store::Backup),
    /// Delete this backup?
    SaveDeleteBackup(super::saves::store::Backup),
    /// Export this map mod into that folder although this many of its
    /// textures are made from the original game's pixels?
    MapExportDerived(String, PathBuf, usize),
}

/// Everything the screens show.
#[derive(Clone, Debug)]
pub struct Model {
    /// The question open over the page.
    pub ask: Option<Dialog>,
    /// Play was pressed: "Starting…" until the game's window appears.
    pub starting: bool,
    /// The check's details list is open (the check screen; the main
    /// screen's drop-down).
    pub show_details: bool,
    /// The details list's highlighted row.
    pub details_sel: usize,
    /// The support file written last (Open folder shows it).
    pub support: Option<PathBuf>,
    /// The last start did not reach the game (the recovery screen).
    pub start_unfinished: bool,
    /// What "Use mods: Off" set aside (also kept in `mods-paused.txt`).
    pub set_aside: Option<Paused>,
    /// The launcher's own preferences.
    pub prefs: Prefs,
    /// The game folder was found on this computer at this start (not kept
    /// from before): the main screen says where.
    pub just_found: bool,
    /// A support file is being written (on a worker).
    pub support_busy: bool,
    /// The Storage page's folders.
    pub locations: Vec<LocRow>,
    /// Portable mode's state.
    pub portable: PortableInfo,
    /// The desktop map editor is beside the launcher (`dirs::editor_exe`).
    pub editor_here: bool,
    /// Studio is beside the launcher (`dirs::studio_exe`).
    pub studio_here: bool,
    /// Studio's command-line tool is beside the launcher (`dirs::studio_cli_exe`).
    pub studio_cli_here: bool,
    /// Studio as the Dev page shows it (started from the launcher).
    pub studio: EditorRun,
    /// What is seen of Studio without starting it: the mods a live program
    /// has open for writing, the last mod opened ([`Env::studio_seen`]).
    pub studio_seen: super::studio_seen::StudioSeen,
    /// The folder the folder choice is for.
    pub pick_loc: Option<crate::platform::dirs::Location>,
    /// Files are being copied (on a worker).
    pub copy_busy: bool,
    /// What the open copy question would copy (looked at before it is
    /// asked; `transfer::plan`).
    pub copy_plan: Option<super::transfer::Plan>,
    /// Arguments for the next start only (the Dev page's editor tools).
    pub play_args: Vec<String>,
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
    /// The Saves page.
    pub saves: super::saves_page::SavesPage,
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
    /// The prepare step (`game --prepare`, docs/developing/prepare.md).
    pub prepare: Prepare,
    /// The Game data screen: your own copy of the game ([`super::game_data`]).
    pub game_data: super::game_data::GameData,
    /// The system folder dialog asked for or open.
    pub dialog: Option<DialogAsk>,
    /// The last input came from a gamepad: folders are then chosen in
    /// Overbright's own browser (pad-friendly), not the system dialog.
    pub pad_input: bool,
    /// The failure screen shows the game's own last lines.
    pub failure_details: bool,
    /// Running inside gamescope (Steam Deck Game Mode, [`in_gamescope`]):
    /// it has no system file chooser, so folders are chosen in
    /// Overbright's own browser.
    pub gamescope: bool,
    /// This computer's check asked for again (the window runs it).
    pub probe_again: bool,
    /// Where settings, saves, caches and logs are kept (the about screen).
    pub folders: Folders,
}

/// Where Overbright keeps its files (`platform::dirs`), as the about
/// screen shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folders {
    /// A portable install: everything in [`Folders::data`] beside the
    /// programs (docs/playing/platform-dirs.md, "Portable mode").
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
    /// stops it (the game makes what is missing, docs/developing/prepare.md).
    pub background: bool,
    /// The Graphics row's value when the hardware check said the folder is
    /// prepared (for these settings' programs): a level saved since that
    /// differs makes the step offered (once a session, as at a first run).
    pub prepared_graphics: Option<Graphics>,
    /// When the running prepare step started (the time left is estimated
    /// from it).
    pub started: Option<std::time::Instant>,
    /// When the step that ran behind the main screen finished: "Shaders
    /// ready" shows for a few seconds.
    pub done_at: Option<std::time::Instant>,
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
    /// Overbright's own packs (`obpack`), in the loader's order (they come
    /// before the imported packages).
    pub own: Vec<OwnPack>,
    /// The map mods (`tools::map::store`).
    pub maps: Vec<MapRow>,
    /// "Use mods" is off: every mod is off for the game, the list kept
    /// ([`Paused`]).
    pub paused: bool,
    /// The order is shown (Move up / Move down) after "Change the order".
    pub show_order: bool,
    /// Two packs that are on change the same textures.
    pub conflict: Option<Conflict>,
    /// The map mod chosen for Export and Open in map editor (its folder).
    pub map_sel: Option<String>,
    /// The map editor started from this page.
    pub editor: EditorRun,
}

/// One of Overbright's own packs as the Mods page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnPack {
    pub id: String,
    pub name: String,
    pub author: String,
    /// World and car textures it replaces.
    pub textures: usize,
    pub on: bool,
    /// Why the pack cannot be used (never on then).
    pub refused: Option<String>,
}

/// The build a map mod's shared copy names in its provenance
/// (`Overbright 0.5.0 (build …)`, as Studio writes it).
pub fn share_build() -> String {
    format!("Overbright {} (build {:016x})", env!("CARGO_PKG_VERSION"), crate::studio_api::build::BUILD_ID.0)
}

/// A map mod as the Mods page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapRow {
    pub folder: String,
    pub name: String,
    pub on: bool,
    /// Its custom races and prop edits.
    pub races: usize,
    pub props: usize,
    /// The game's textures it changes (its `textures/` folder, Studio Looks).
    pub textures: usize,
}

/// The map editor as the Mods page shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum EditorRun {
    #[default]
    Closed,
    /// Open on this mod's folder (None: on its own Open / New).
    Open(Option<String>),
    /// It could not start, or ended with an error: the words.
    Failed(String),
}

/// A map editor the launcher started (its end polled by the window).
pub trait EditorJob {
    /// None while it runs; then Ok, or the words for how it ended.
    fn poll(&mut self) -> Option<Result<(), String>>;
}

thread_local! {
    /// What the fake data's clipboard holds (tests).
    pub static COPIED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The real editor process: a started program (the map editor or Studio) and its name for the
/// words of a bad end.
struct EditorProcess(std::process::Child, &'static str);

impl EditorJob for EditorProcess {
    fn poll(&mut self) -> Option<Result<(), String>> {
        match self.0.try_wait() {
            Ok(None) => None,
            Ok(Some(st)) if st.success() => Some(Ok(())),
            Ok(Some(st)) => {
                #[cfg(unix)]
                let signal = std::os::unix::process::ExitStatusExt::signal(&st);
                #[cfg(not(unix))]
                let signal = None;
                Some(Err(format!("{} stopped: {}.", self.1, crate::launcher::crash_report::reason(st.code(), signal, false))))
            }
            Err(e) => Some(Err(e.to_string())),
        }
    }
}

/// The fake data's editor: open until the test ends it.
#[derive(Default)]
pub struct FakeEditor(pub std::rc::Rc<std::cell::Cell<Option<Result<(), String>>>>);

impl EditorJob for FakeEditor {
    fn poll(&mut self) -> Option<Result<(), String>> {
        self.0.take()
    }
}

/// Two packs that change the same textures: the first wins.
#[derive(Clone, Debug, PartialEq)]
pub struct Conflict {
    pub first: String,
    pub second: String,
    pub textures: usize,
    /// Both in the same list (own packs, or imported packages): their
    /// order can be changed; else the own pack always comes first.
    pub same_list: bool,
}

/// A folder on the Storage page: where it is, whether the player put it
/// there, whether it is missing (a drive not there).
#[derive(Clone, Debug, PartialEq)]
pub struct LocRow {
    pub loc: crate::platform::dirs::Location,
    pub path: Option<PathBuf>,
    pub custom: bool,
    pub missing: bool,
}

/// Portable mode as the Storage page shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortableInfo {
    /// Portable mode in force for this run.
    pub on: bool,
    /// The marker beside the program now (the next start's state).
    pub marker: bool,
    /// The switch was changed in this session (a restart applies it).
    pub changed: bool,
    /// The program folder (None: not known).
    pub program: Option<PathBuf>,
    /// `<program>/data`.
    pub data: Option<PathBuf>,
    /// Where things are kept outside portable mode (Overbright's folder).
    pub home: PathBuf,
    /// The profiles' parent outside portable mode (the data home: the
    /// saves are in its `NFS Underground 2`).
    pub home_saves: PathBuf,
    /// The program folder can be written (portable mode can be turned on).
    pub writable: bool,
}

/// The launcher's own preferences (`launcher.txt` in Overbright's folder;
/// the game's settings stay in options.txt).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Text size in percent: 100, 125 or 150.
    pub text: u16,
    pub reduced_motion: bool,
    /// Launcher sounds (off by default).
    pub sounds: bool,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs { text: 100, reduced_motion: false, sounds: false }
    }
}

impl Prefs {
    pub const FILE: &'static str = "launcher.txt";
    pub const TEXT_SIZES: [u16; 3] = [100, 125, 150];

    pub fn text(&self) -> String {
        format!(
            "# The launcher's own preferences.\ntext_size {}\nmotion {}\nsounds {}\n",
            self.text,
            if self.reduced_motion { "reduced" } else { "full" },
            if self.sounds { "on" } else { "off" }
        )
    }

    pub fn parse(t: &str) -> Prefs {
        let mut p = Prefs::default();
        for l in t.lines().map(str::trim) {
            match l.split_once(' ') {
                Some(("text_size", v)) => {
                    p.text = v.trim().parse().ok().filter(|v| Prefs::TEXT_SIZES.contains(v)).unwrap_or(100)
                }
                Some(("motion", v)) => p.reduced_motion = v.trim() == "reduced",
                Some(("sounds", v)) => p.sounds = v.trim() == "on",
                _ => {}
            }
        }
        p
    }
}

/// The file that holds what "Use mods: Off" set aside.
pub const PAUSED_FILE: &str = "mods-paused.txt";

/// An own pack's row from the loader's list.
fn own_row(l: crate::obpack::Listed) -> OwnPack {
    match l.pack {
        Ok(p) => OwnPack {
            textures: p.index.world.len() + p.index.car.len(),
            name: p.manifest.name.clone(),
            author: p.manifest.author.clone(),
            on: l.on,
            refused: None,
            id: l.id,
        },
        Err(e) => OwnPack { name: l.id.clone(), id: l.id, author: String::new(), textures: 0, on: false, refused: Some(e) },
    }
}

/// The first pair of texture sets (in order) that share textures, and how many.
pub fn first_conflict<T: Ord>(sets: &[(String, std::collections::BTreeSet<T>)]) -> Option<Conflict> {
    for (i, (a, sa)) in sets.iter().enumerate() {
        for (b, sb) in &sets[i + 1..] {
            let n = sa.intersection(sb).count();
            if n > 0 {
                return Some(Conflict { first: a.clone(), second: b.clone(), textures: n, same_list: true });
            }
        }
    }
    None
}

/// A mod folder's kind ([`Env::mod_kind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModKind {
    Own,
    Map,
}

/// What "Use mods: Off" set aside, to give back when it is turned on
/// again: each pack's and map mod's on / off and the HUD mod's switch
/// (`mods-paused.txt` in Overbright's folder).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paused {
    pub own: Vec<(String, bool)>,
    pub packages: Vec<(String, bool)>,
    pub maps: Vec<String>,
    pub hud: bool,
}

impl Paused {
    pub fn text(&self) -> String {
        let mut s = String::from("# Mods set aside while \"Use mods\" is off (the launcher gives them back).\n");
        for (id, on) in &self.own {
            s += &format!("own {} {id}\n", if *on { "on" } else { "off" });
        }
        for (id, on) in &self.packages {
            s += &format!("package {} {id}\n", if *on { "on" } else { "off" });
        }
        for f in &self.maps {
            s += &format!("map on {f}\n");
        }
        s += &format!("hud {}\n", if self.hud { "on" } else { "off" });
        s
    }

    pub fn parse(text: &str) -> Paused {
        let mut p = Paused::default();
        for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut w = l.split_whitespace();
            match (w.next(), w.next(), w.next()) {
                (Some("own"), Some(s), Some(id)) => p.own.push((id.into(), s == "on")),
                (Some("package"), Some(s), Some(id)) => p.packages.push((id.into(), s == "on")),
                (Some("map"), Some("on"), Some(f)) => p.maps.push(f.into()),
                (Some("hud"), Some(s), None) => p.hud = s == "on",
                _ => {}
            }
        }
        p
    }
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
/// the release build sets it (docs/developing/building-releases.md), when it is a web
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
    /// What went wrong (the game's `error.kind=` line, or inferred).
    pub kind: super::run::ErrorKind,
    pub during_start: bool,
    /// "exit code 101" and the like (`run::Ended::how`).
    pub how: String,
    pub tail: Vec<String>,
    pub log: Option<PathBuf>,
    /// The crash report window's part: set when the game ran and ended
    /// badly (None: it could not be started).
    pub report: Option<CrashReport>,
}

/// What the failure screen adds for a game that ran and ended badly
/// (docs/troubleshooting.md): the end in words, the system's errors for
/// that process, the logs written during the run.
#[derive(Clone, Debug, PartialEq)]
pub struct CrashReport {
    /// "it tried to use memory it must not (a segmentation fault, signal 11)".
    pub reason: String,
    pub pid: u32,
    pub secs: f32,
    /// The system's recent errors for the process, when readable.
    pub errors: Vec<String>,
    /// The system is still being asked (a worker).
    pub asking: bool,
    /// Overbright's log files written during the run.
    pub logs: Vec<PathBuf>,
}

impl Failure {
    /// The game program could not be started at all (`how`: why).
    pub fn not_started(how: String) -> Failure {
        Failure { kind: super::run::ErrorKind::NoProgram, during_start: true, how, tail: Vec::new(), log: None, report: None }
    }
}

/// What the system's folder dialog gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    Folder(PathBuf),
    /// The player closed it without a choice: nothing changes.
    Cancelled,
    /// There is no system dialog here (none installed, or it failed to
    /// open): Overbright's own folder browser is used instead.
    NoDialog,
}

/// A system folder dialog asked for and not answered yet (the window opens
/// it on a worker thread where the dialog would block it, Linux).
#[derive(Clone, Debug, PartialEq)]
pub struct DialogAsk {
    pub purpose: BrowseFor,
    pub title: String,
    pub start: Option<PathBuf>,
    /// The worker has it (the dialog is open).
    pub open: bool,
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
    /// A map mod to import: a folder holding `map.obmap`, the file, or a
    /// `.zip` of one.
    MapMod,
    /// The folder a map mod is exported to.
    MapExport,
    /// The folder a profile is exported to (Saves).
    SavesExport,
    /// A new place for one of Overbright's folders (Storage).
    Location,
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
    /// Keeps the text language the game starts in ("" forgets it).
    fn set_language(&self, name: &str) -> Result<(), String>;
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
    /// The Saves page's worker context (None: no worker; the jobs run at
    /// once through [`Env::saves_run`]).
    fn saves_ctx(&self, _game: Option<PathBuf>) -> Option<saves::jobs::Ctx> {
        None
    }
    /// A Saves page job run at once.
    fn saves_run(&self, job: saves::jobs::Job) -> saves::jobs::Done;
    /// The system's own folder dialog ([`system_folder_dialog`]); the
    /// fake data has none (Overbright's own browser opens).
    fn folder_dialog(&self, _title: &str, _start: Option<&Path>) -> Picked {
        Picked::NoDialog
    }
    /// The dialog is opened by the window on a worker thread (it would
    /// block the window's drawing); else [`Env::folder_dialog`] is called
    /// at once.
    fn background_dialog(&self) -> bool {
        false
    }
    /// Writes the support file (`diag::support`); its path.
    fn support_file(&self, req: &crate::diag::support::SupportRequest) -> std::io::Result<PathBuf> {
        crate::diag::support::write_support_file(req)
    }
    /// The system's recent errors for a program that ended badly
    /// (`launcher::crash_report::system_errors`).
    fn crash_errors(&self, pid: u32, exe_name: &str) -> Vec<String> {
        crate::launcher::crash_report::system_errors(pid, exe_name)
    }
    /// Puts text on the clipboard.
    fn copy_text(&self, text: &str) -> Result<(), String> {
        crate::launcher::crash_report::copy_text(text)
    }
    /// Shows a written file in the system's file manager.
    fn reveal(&self, path: &Path) {
        crate::diag::support::reveal(path)
    }
    /// Overbright's own packs, in the loader's order.
    fn own_packs(&self) -> Vec<OwnPack>;
    /// Keeps the own packs' order and on / off (`order.txt`).
    fn save_own(&self, list: &[(String, bool)]) -> Result<(), String>;
    /// Installs an Overbright pack folder; its id.
    fn install_own(&self, src: &Path) -> Result<String, String>;
    /// Deletes an installed Overbright pack (its compiled files too).
    fn remove_own(&self, id: &str) -> Result<(), String>;
    /// The first two packs that are on and change the same textures, own
    /// packs first, then the packages, each in its order.
    fn conflict(&self, own: &[OwnPack], packages: &[crate::texture_mods::Entry]) -> Option<Conflict>;
    /// The map mods with their on / off.
    fn maps(&self) -> Vec<MapRow>;
    fn set_map(&self, folder: &str, on: bool) -> Result<(), String>;
    /// Imports a map mod (folder, `map.obmap` or `.zip`); its folder name.
    fn import_map(&self, from: &Path) -> Result<String, String>;
    /// Exports a map mod as a `.zip` into `to`, with its textures; the file
    /// written. `derived_ok`: the player said yes to sharing textures made
    /// from the original game's pixels (without it such a mod is refused).
    fn export_map(&self, folder: &str, to: &Path, derived_ok: bool) -> Result<PathBuf, String>;
    /// How many of the map mod's textures are made from the original
    /// game's pixels (the question before its export).
    fn map_derived(&self, folder: &str) -> usize;
    /// The Storage page's folders.
    fn locations(&self) -> Vec<LocRow> {
        use crate::platform::dirs;
        dirs::Location::ALL
            .into_iter()
            .map(|loc| {
                let custom = dirs::override_of(loc);
                let path = dirs::location(loc);
                let missing = custom.as_deref().is_some_and(dirs::location_missing);
                LocRow { loc, path, custom: custom.is_some(), missing }
            })
            .collect()
    }
    /// Puts a folder elsewhere (None: back to its default).
    fn set_location(&self, loc: crate::platform::dirs::Location, to: Option<&Path>) -> Result<(), String> {
        crate::platform::dirs::set_location(loc, to).map_err(|e| e.to_string())
    }
    /// Portable mode's state.
    fn portable(&self) -> PortableInfo {
        PortableInfo::default()
    }
    /// Turns portable mode off (removes `portable.txt`; the data folder stays).
    fn unmake_portable(&self) -> Result<(), String> {
        Ok(())
    }
    /// The map editor's program is there (`overbright-editor` beside the
    /// launcher, [`super::dirs::editor_exe`]).
    fn editor_present(&self) -> bool {
        super::dirs::editor_exe().is_file()
    }
    /// Starts the map editor (`overbright-editor`, [`super::dirs::editor_exe`])
    /// on the mod in `folder`, or on its own Open / New; `root`: the game
    /// folder shown, given as `NFSU2_ROOT`.
    fn start_editor(&self, _folder: Option<&str>, _root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        Err("no map editor here".into())
    }
    /// Studio's program is there (`overbright-studio` beside the launcher,
    /// [`super::dirs::studio_exe`]).
    fn studio_present(&self) -> bool {
        super::dirs::studio_exe().is_file()
    }
    /// Studio's command-line tool is there ([`super::dirs::studio_cli_exe`]).
    fn studio_cli_present(&self) -> bool {
        super::dirs::studio_cli_exe().is_file()
    }
    /// Starts Studio (`overbright-studio`, [`super::dirs::studio_exe`]) on
    /// its Home; `root`: the game folder shown, given as `NFSU2_ROOT`.
    fn start_studio(&self, _root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        Err("no Studio here".into())
    }
    /// Overbright's own copy of the game ([`super::game_data`]): where it is kept (`<app>/base`).
    fn game_data_base(&self) -> PathBuf {
        PathBuf::from("/data/player/.local/share/overbright/base")
    }
    /// What that folder holds; the fake data has no copy.
    fn game_data_status(&self) -> super::game_data::Status {
        super::game_data::Status::None
    }
    /// The game folder's content in bytes (what the copy takes).
    fn game_data_size(&self, _root: &Path) -> u64 {
        1_801_794_145
    }
    /// Starts a run of the importer in the background; the fake data's run ends at once, made.
    fn game_data_start(&self, _kind: super::game_data::Kind, _root: &Path) -> super::game_data::Job {
        use super::game_data::{End, Job, Msg, Status};
        Job::scripted(vec![
            Msg::Progress { fraction: 0.5, words: "Copying · 0.9 GB of 1.8 GB".into() },
            Msg::End(End::Ready {
                status: Status::Ready { edition: "PC 1.2, European release".into(), known: true },
                differing: None,
            }),
        ])
    }
    /// "Not now" on the offer: kept, so the launcher does not offer the copy at every start (Settings ›
    /// Advanced › Game data still does).
    fn game_data_later(&self) -> bool {
        false
    }
    fn set_game_data_later(&self) {}
    /// What is seen of Studio for the map mods `folders`: their write locks
    /// held by a live program and the last mod opened, read only
    /// ([`super::studio_seen`]); the fake data sees nothing.
    fn studio_seen(&self, _folders: &[String]) -> super::studio_seen::StudioSeen {
        Default::default()
    }
    /// The launcher's own preferences.
    fn prefs(&self) -> Prefs {
        Prefs::default()
    }
    fn save_prefs(&self, _p: &Prefs) -> Result<(), String> {
        Ok(())
    }
    /// Support files are written on a worker (the window's); else at once.
    fn background_support(&self) -> bool {
        false
    }
    /// What "Use mods: Off" set aside (None: mods are in use).
    fn paused(&self) -> Option<Paused>;
    fn set_paused(&self, p: Option<&Paused>) -> Result<(), String>;
    /// Whether `dir` is an Overbright pack (a `pack.toml`, or `world/`,
    /// `car/` or `any/` holding images) or a map mod (`map.obmap`).
    fn mod_kind(&self, dir: &Path) -> Option<ModKind> {
        if dir.join("map.obmap").is_file()
            || dir.extension().is_some_and(|e| e.eq_ignore_ascii_case("obmap"))
        {
            return Some(ModKind::Map);
        }
        if dir.join("pack.toml").is_file() || ["world", "car", "any"].iter().any(|d| dir.join(d).is_dir()) {
            return Some(ModKind::Own);
        }
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
            let found = if p.is_dir() && !p.join("texmod.def").is_file() && !crate::formats::texwizard::is_pack_dir(&p)
            {
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
            return tm::Outcome {
                line: format!("{}: imported, but the list could not be kept: {e}", o.name),
                ok: false,
                ..o
            };
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
        GAME_FOLDER_NOTE.into()
    } else if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(crate::frontend::profiles::FOLDER)) {
        "saves".into()
    } else {
        String::new()
    }
}

/// The folder browser's note on a row that holds the game.
pub const GAME_FOLDER_NOTE: &str = "game folder";

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

/// The header's version: the number alone, beside the name
/// ("Overbright … 0.3.0"), never a bare "version" next to the game's own.
pub fn version() -> String {
    crate::build_info::VERSION.to_string()
}

/// `p` with its long names: Windows' registry gives the install folder in
/// 8.3 short form (`C:\PROGRA~2\EAGAME~1\…`); the file system's own
/// spelling (`canonicalize`, its `\\?\` prefix taken off) is shown and
/// kept instead. Unchanged elsewhere, for a path without a `~`, and when
/// the folder cannot be read.
pub fn long_path(p: &Path) -> PathBuf {
    // Only a path with a short name in it: canonicalize would also turn
    // a mapped drive (`Z:\Games`) into its share's name.
    if !cfg!(windows) || !p.to_string_lossy().contains('~') {
        return p.to_path_buf();
    }
    match std::fs::canonicalize(p) {
        Ok(c) => plain_windows_path(&c.to_string_lossy()).map_or(c, PathBuf::from),
        Err(_) => p.to_path_buf(),
    }
}

/// A `\\?\` verbatim path in its ordinary form (`\\?\C:\x` →
/// `C:\x`, `\\?\UNC\srv\share` → `\\srv\share`); None when it is
/// not one.
pub fn plain_windows_path(s: &str) -> Option<String> {
    let rest = s.strip_prefix(r"\\?\")?;
    Some(match rest.strip_prefix(r"UNC\") {
        Some(unc) => format!(r"\\{unc}"),
        None => rest.to_string(),
    })
}

/// Whether `dir` is Overbright's own folder: the launcher beside it, or
/// the folder this program runs from (`exe_dir`).
pub fn is_overbright_dir(dir: &Path, exe_dir: Option<&Path>) -> bool {
    let own = ["overbright", "overbright.exe"].iter().any(|n| dir.join(n).is_file());
    own || exe_dir.is_some_and(|e| same_dir(e, dir))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || std::fs::canonicalize(a).ok().zip(std::fs::canonicalize(b).ok()).is_some_and(|(x, y)| x == y)
}

/// The game folder for a chosen `dir`: itself when it holds the game,
/// else the folder above it (a pick of GLOBAL, or of Overbright unpacked
/// inside the game folder), else the one folder below it that does (a
/// pick of `EA GAMES`); `dir` itself when none does.
pub fn game_dir_near(dir: &Path, looks: &dyn Fn(&Path) -> bool) -> PathBuf {
    if looks(dir) {
        return dir.to_path_buf();
    }
    if let Some(up) = dir.parent().filter(|u| !u.as_os_str().is_empty() && looks(u)) {
        return up.to_path_buf();
    }
    let mut below: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir() && looks(p)).collect())
        .unwrap_or_default();
    below.sort();
    if below.len() == 1 { below.remove(0) } else { dir.to_path_buf() }
}

/// The system's folder dialog: Windows' own (`IFileOpenDialog`), on Linux
/// and other systems the desktop's file chooser through the XDG portal,
/// else zenity (rfd). `OVERBRIGHT_OWN_PICKER=1` and gamescope
/// ([`in_gamescope`]) ask for Overbright's own browser instead.
pub fn system_folder_dialog(title: &str, start: Option<&Path>) -> Picked {
    if std::env::var_os("OVERBRIGHT_OWN_PICKER").is_some_and(|v| v != "0") || in_gamescope(&|k| std::env::var_os(k)) {
        return Picked::NoDialog;
    }
    #[cfg(windows)]
    {
        let _ = start;
        match super::win_dialog::pick_folder(title) {
            Ok(Some(d)) => Picked::Folder(d),
            Ok(None) => Picked::Cancelled,
            Err(()) => Picked::NoDialog,
        }
    }
    #[cfg(target_os = "linux")]
    {
        // rfd gives None both for a cancel and for no dialog at all (no
        // portal answering and no zenity). Without a session bus and
        // without zenity there is none; else an answer faster than a
        // person can cancel means none opened.
        let bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
            || std::env::var_os("XDG_RUNTIME_DIR").is_some_and(|d| Path::new(&d).join("bus").exists());
        if !bus && !on_path("zenity") {
            return Picked::NoDialog;
        }
        let mut d = rfd::FileDialog::new().set_title(title);
        if let Some(s) = start.filter(|s| s.is_dir()) {
            d = d.set_directory(s);
        }
        let t = std::time::Instant::now();
        match d.pick_folder() {
            Some(p) => Picked::Folder(p),
            None if t.elapsed() < NO_DIALOG_WITHIN => {
                log::info!("folder dialog: none answered ({} ms)", t.elapsed().as_millis());
                Picked::NoDialog
            }
            None => Picked::Cancelled,
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (title, start);
        Picked::NoDialog
    }
}

/// Whether the launcher runs inside gamescope (`var`: the environment):
/// Steam Deck Game Mode (`SteamDeck=1`, `SteamGamepadUI`) or a gamescope
/// session (`XDG_CURRENT_DESKTOP` naming it, `GAMESCOPE_WAYLAND_DISPLAY`).
/// Its portals offer no FileChooser (Valve's gamescope and holo backends),
/// so no system folder dialog is tried there.
pub fn in_gamescope(var: &dyn Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    var("SteamDeck").is_some_and(|v| v == "1")
        || var("SteamGamepadUI").is_some()
        || var("GAMESCOPE_WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        || var("XDG_CURRENT_DESKTOP").is_some_and(|v| v.to_string_lossy().to_ascii_lowercase().contains("gamescope"))
}

/// A dialog that ends without a choice within this time never opened.
#[cfg(target_os = "linux")]
const NO_DIALOG_WITHIN: std::time::Duration = std::time::Duration::from_millis(300);

#[cfg(target_os = "linux")]
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

impl Model {
    /// The installed languages the engine can show
    /// ([`crate::game_assets::language_support`]); English alone when no
    /// language file is known (no game folder yet).
    pub fn supported_languages(&self) -> Vec<String> {
        use crate::game_assets::{Support, language_support};
        let v: Vec<String> = self.languages.iter().filter(|l| language_support(l) == Support::Yes).cloned().collect();
        if v.is_empty() { vec![crate::game_assets::DEFAULT_LANGUAGE.to_string()] } else { v }
    }

    /// The installed languages the engine cannot show, with the reason.
    pub fn unsupported_languages(&self) -> Vec<(String, &'static str)> {
        use crate::game_assets::{Support, language_support};
        self.languages
            .iter()
            .filter_map(|l| match language_support(l) {
                Support::No(why) => Some((l.clone(), why)),
                Support::Yes => None,
            })
            .collect()
    }

    /// The language the game will start in: the chosen one, else English,
    /// else (a copy without English) its first supported language.
    pub fn language_name(&self) -> String {
        if !self.choices.language.is_empty() {
            return self.choices.language.clone();
        }
        let sup = self.supported_languages();
        if sup.iter().any(|l| l.eq_ignore_ascii_case(crate::game_assets::DEFAULT_LANGUAGE)) {
            crate::game_assets::DEFAULT_LANGUAGE.to_string()
        } else {
            sup[0].clone()
        }
    }
}

/// The real machine.
pub struct RealEnv;

/// The Saves page's jobs on the player's folders.
fn real_saves_ctx(game: Option<PathBuf>) -> saves::jobs::Ctx {
    let p = saves::store::Paths::player();
    let src = saves::sources::Ctx::here(game, p.profiles.clone());
    saves::jobs::Ctx::new(p, src)
}

/// The marker of "Not now" on the own-copy offer, in Overbright's own folder.
const GAME_DATA_LATER: &str = "game-data-later.txt";

impl Env for RealEnv {
    fn load(&self) -> Model {
        let (choices, file) = load_choices();
        let mut found: Vec<Found> = Vec::new();
        let mut add = |p: PathBuf, source: &str| {
            let p = long_path(&p);
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
            ask: None,
            starting: false,
            show_details: false,
            details_sel: 0,
            support: None,
            start_unfinished: false,
            set_aside: None,
            prefs: self.prefs(),
            just_found: false,
            support_busy: false,
            locations: self.locations(),
            portable: self.portable(),
            editor_here: self.editor_present(),
            studio_here: self.studio_present(),
            studio_cli_here: self.studio_cli_present(),
            studio: EditorRun::Closed,
            studio_seen: Default::default(),
            pick_loc: None,
            copy_busy: false,
            copy_plan: None,
            play_args: Vec::new(),
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
            saves: Default::default(),
            logs_dir: super::dirs::logs(),
            saves_dir: super::dirs::profiles(),
            build: crate::build_info::version_line(),
            source: source_url(),
            notices: super::screens::NOTICES,
            native_dialog: cfg!(windows),
            mods: {
                let list = self.mods();
                let own = self.own_packs();
                let conflict = self.conflict(&own, &list.iter().map(|(e, _)| e.clone()).collect::<Vec<_>>());
                Mods { list, own, maps: self.maps(), paused: self.paused().is_some(), conflict, ..Default::default() }
            },
            hud: self.hud(&pack),
            prepare: Prepare::default(),
            game_data: {
                let status = self.game_data_status();
                super::game_data::GameData {
                    state: super::game_data::GameData::state_for(&status),
                    status,
                    place: self.game_data_base().display().to_string(),
                    ..Default::default()
                }
            },
            dialog: None,
            pad_input: false,
            failure_details: false,
            gamescope: in_gamescope(&|k| std::env::var_os(k)),
            probe_again: false,
            folders: self.folders(),
        }
    }

    fn check(&self, root: &Path) -> CheckResult {
        let chosen = long_path(root);
        let root = game_dir_near(&chosen, &setup::looks_like_game);
        let refused =
            |problem| CheckResult {
                root: root.clone(),
                lines: Vec::new(),
                usable: false,
                problem: Some(problem),
                summary: None,
            };
        if let Err(e) = std::fs::read_dir(&root)
            && e.kind() == std::io::ErrorKind::PermissionDenied
        {
            return refused(Problem::Unreadable);
        }
        let exe = std::env::current_exe().ok();
        if !setup::looks_like_game(&root) && is_overbright_dir(&root, exe.as_deref().and_then(Path::parent)) {
            return refused(Problem::OwnFolder);
        }
        let c = setup::check(&root, true);
        let fp = crate::fingerprint::compute(&root);
        check_result(&root, &c, Some(&fp))
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

    fn set_language(&self, name: &str) -> Result<(), String> {
        setup::save_language((!name.is_empty()).then_some(name)).map_err(|e| e.to_string())
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

    fn saves_ctx(&self, game: Option<PathBuf>) -> Option<saves::jobs::Ctx> {
        Some(real_saves_ctx(game))
    }

    fn saves_run(&self, job: saves::jobs::Job) -> saves::jobs::Done {
        saves::jobs::run(&real_saves_ctx(None), job)
    }

    fn folder_dialog(&self, title: &str, start: Option<&Path>) -> Picked {
        system_folder_dialog(title, start)
    }

    // Windows' dialog runs its own message loop on the window's thread
    // (it keeps the window drawn); the portal's and zenity's would block
    // it, so they run on a worker.
    fn background_dialog(&self) -> bool {
        !cfg!(windows)
    }

    fn portable(&self) -> PortableInfo {
        use crate::platform::dirs;
        let program = dirs::program_dir();
        let writable = program.as_ref().is_some_and(|p| {
            let t = p.join(".overbright-write-test");
            let ok = std::fs::write(&t, b"").is_ok();
            let _ = std::fs::remove_file(&t);
            ok
        });
        #[cfg(windows)]
        let known = crate::platform::win::local_app_data();
        #[cfg(not(windows))]
        let known = None;
        let plain = dirs::resolve(dirs::Os::CURRENT, &|n: &str| std::env::var_os(n), known);
        let (home, home_saves) = (plain.app, plain.data_home);
        PortableInfo {
            on: dirs::portable_base().is_some(),
            marker: program.as_deref().is_some_and(dirs::is_portable),
            changed: false,
            data: program.as_deref().map(dirs::portable_data),
            program,
            home,
            home_saves,
            writable,
        }
    }

    fn unmake_portable(&self) -> Result<(), String> {
        let dir = crate::platform::dirs::program_dir().ok_or("the program's folder is not known")?;
        if dir.join(crate::platform::dirs::PORTABLE_FOLDER).is_dir() {
            return Err(format!(
                "this install is marked by the folder {}; remove that folder to turn portable mode off",
                dir.join(crate::platform::dirs::PORTABLE_FOLDER).display()
            ));
        }
        match std::fs::remove_file(dir.join(crate::platform::dirs::PORTABLE_FILE)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    fn prefs(&self) -> Prefs {
        std::fs::read_to_string(super::dirs::data_dir().join(Prefs::FILE)).map(|t| Prefs::parse(&t)).unwrap_or_default()
    }

    fn save_prefs(&self, p: &Prefs) -> Result<(), String> {
        let f = super::dirs::data_dir().join(Prefs::FILE);
        crate::qol::saves::write_atomic(&f, p.text().as_bytes(), false).map_err(|e| e.to_string())
    }

    fn background_support(&self) -> bool {
        true
    }

    fn own_packs(&self) -> Vec<OwnPack> {
        crate::obpack::list_in(&crate::obpack::packs_dir()).into_iter().map(own_row).collect()
    }

    fn save_own(&self, list: &[(String, bool)]) -> Result<(), String> {
        let l: Vec<crate::obpack::OrderEntry> =
            list.iter().map(|(id, on)| crate::obpack::OrderEntry { id: id.clone(), on: *on }).collect();
        crate::obpack::save_order(&crate::obpack::packs_dir(), &l).map_err(|e| e.to_string())
    }

    fn install_own(&self, src: &Path) -> Result<String, String> {
        crate::obpack::install_folder(src, &crate::obpack::packs_dir()).map_err(|e| format!("{e:#}"))
    }

    fn remove_own(&self, id: &str) -> Result<(), String> {
        crate::obpack::remove(&crate::obpack::packs_dir(), &crate::obpack::compiled_root(), id)
            .map_err(|e| format!("{e:#}"))
    }

    fn conflict(&self, own: &[OwnPack], packages: &[crate::texture_mods::Entry]) -> Option<Conflict> {
        let packs_dir = crate::obpack::packs_dir();
        let mut sets: Vec<(String, std::collections::BTreeSet<(bool, u32)>)> = Vec::new();
        for o in own.iter().filter(|o| o.on && o.refused.is_none()) {
            let Ok(p) = crate::obpack::Pack::open(&packs_dir.join(&o.id)) else { continue };
            let mut set = std::collections::BTreeSet::new();
            for (car, d) in [(false, crate::obpack::Domain::World), (true, crate::obpack::Domain::Car)] {
                set.extend(p.index.domain(d).keys().map(|h| (car, *h)));
            }
            sets.push((o.name.clone(), set));
        }
        let pk = crate::texture_mods::packages_dir();
        for e in packages.iter().filter(|e| e.on) {
            let mut set = std::collections::BTreeSet::new();
            for (car, sub) in [(false, "world"), (true, "car")] {
                for f in std::fs::read_dir(pk.join(&e.id).join(sub)).into_iter().flatten().flatten() {
                    let stem = f.path().file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    let hex = stem.trim_start_matches("0x").trim_start_matches("0X");
                    if let Ok(h) = u32::from_str_radix(hex, 16) {
                        set.insert((car, h));
                    }
                }
            }
            let name = crate::texture_mods::info(&pk, &e.id).map_or(e.id.clone(), |i| i.name);
            sets.push((name, set));
        }
        let own_n = own.iter().filter(|o| o.on && o.refused.is_none()).count();
        let names: Vec<&String> = sets.iter().map(|(n, _)| n).collect();
        first_conflict(&sets).map(|mut c| {
            let at = |n: &String| names.iter().position(|x| *x == n).unwrap_or(0) < own_n;
            c.same_list = at(&c.first) == at(&c.second);
            c
        })
    }

    fn maps(&self) -> Vec<MapRow> {
        let dir = crate::tools::map::store::maps_dir();
        let on = crate::tools::map::store::enabled(&dir);
        crate::tools::map::store::list(&dir)
            .into_iter()
            .map(|f| {
                let m = crate::tools::map::store::read_file(&dir.join(&f).join(crate::tools::map::store::FILE)).ok();
                let (races, props) = m.as_ref().map_or((0, 0), |m| (m.races.len(), m.props.len()));
                let name = m.map(|m| m.name).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| f.clone());
                let textures = crate::tools::map::share::textures(&dir.join(&f)).len();
                MapRow { on: on.contains(&f), name, folder: f, races, props, textures }
            })
            .collect()
    }

    fn set_map(&self, folder: &str, on: bool) -> Result<(), String> {
        crate::tools::map::store::set_enabled(&crate::tools::map::store::maps_dir(), folder, on)
            .map_err(|e| format!("{e:#}"))
    }

    fn import_map(&self, from: &Path) -> Result<String, String> {
        crate::tools::map::share::import(&crate::tools::map::store::maps_dir(), from)
    }

    fn start_editor(&self, folder: Option<&str>, root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        let exe = super::dirs::editor_exe();
        let mut cmd = std::process::Command::new(&exe);
        if let Some(f) = folder {
            cmd.args(["--mod", f]);
        }
        if let Some(r) = root {
            cmd.env("NFSU2_ROOT", r);
        }
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let child = cmd.spawn().map_err(|e| format!("the map editor could not be started ({}): {e}", exe.display()))?;
        log::info!("map editor started (pid {}) on {}", child.id(), folder.unwrap_or("its own choice"));
        Ok(Box::new(EditorProcess(child, "the map editor")))
    }

    fn game_data_base(&self) -> PathBuf {
        crate::platform::dirs::app().join("base")
    }

    fn game_data_status(&self) -> super::game_data::Status {
        super::game_data::status(&self.game_data_base())
    }

    fn game_data_size(&self, root: &Path) -> u64 {
        super::game_data::size(root)
    }

    fn game_data_start(&self, kind: super::game_data::Kind, root: &Path) -> super::game_data::Job {
        super::game_data::start(kind, root, &self.game_data_base())
    }

    fn game_data_later(&self) -> bool {
        crate::platform::dirs::config().join(GAME_DATA_LATER).is_file()
    }

    fn set_game_data_later(&self) {
        let p = crate::platform::dirs::config().join(GAME_DATA_LATER);
        if let Err(e) = crate::platform::fs::write_atomic(&p, b"The own copy of the game was offered; Not now was chosen.\n", false) {
            log::warn!("{}: {e}", p.display());
        }
    }

    fn studio_seen(&self, folders: &[String]) -> super::studio_seen::StudioSeen {
        let recent = crate::platform::dirs::config().join(super::studio_seen::RECENT_FILE);
        super::studio_seen::read(&crate::tools::map::store::maps_dir(), folders, &recent)
    }

    fn start_studio(&self, root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        let exe = super::dirs::studio_exe();
        let mut cmd = std::process::Command::new(&exe);
        if let Some(r) = root {
            cmd.env("NFSU2_ROOT", r);
        }
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let child = cmd.spawn().map_err(|e| format!("Studio could not be started ({}): {e}", exe.display()))?;
        log::info!("Studio started (pid {})", child.id());
        Ok(Box::new(EditorProcess(child, "Studio")))
    }

    fn export_map(&self, folder: &str, to: &Path, derived_ok: bool) -> Result<PathBuf, String> {
        use crate::tools::map::share;
        let out = share::default_zip(to, folder);
        share::export(&crate::tools::map::store::maps_dir(), folder, &out, derived_ok, &share_build())
            .map(|e| e.to)
            .map_err(|e| e.to_string())
    }

    fn map_derived(&self, folder: &str) -> usize {
        crate::tools::map::share::derived(&crate::tools::map::store::maps_dir().join(folder)).len()
    }

    fn paused(&self) -> Option<Paused> {
        std::fs::read_to_string(super::dirs::data_dir().join(PAUSED_FILE)).ok().map(|t| Paused::parse(&t))
    }

    fn set_paused(&self, p: Option<&Paused>) -> Result<(), String> {
        let f = super::dirs::data_dir().join(PAUSED_FILE);
        match p {
            Some(p) => crate::qol::saves::write_atomic(&f, p.text().as_bytes(), false).map_err(|e| e.to_string()),
            None => match std::fs::remove_file(&f) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
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

/// A check's result from the file check (and the fingerprint): the
/// lines, usable, and the problem when not. Two notes are left out, as
/// nothing to act on: the mod files found (Overbright ignores them) and
/// what SPEED2.EXE is (Overbright does not use it).
pub fn check_result(root: &Path, c: &setup::Check, fp: Option<&crate::fingerprint::Fingerprint>) -> CheckResult {
    let lines: Vec<(Mark, String)> = setup::check_lines(c, fp)
        .into_iter()
        .map(|(m, t)| (Mark::from(m), t))
        .filter(|(m, t)| !(*m == Mark::Info && (t.starts_with("Mod files found") || t.starts_with("SPEED2.EXE"))))
        .collect();
    // A folder with an error line cannot be used, files all there or
    // not.
    let usable = c.ok() && !lines.iter().any(|(m, _)| *m == Mark::Error);
    let only_text =
        c.missing.len() == 1 && c.missing[0].eq_ignore_ascii_case(&c.text_file()) && setup::looks_like_game(root);
    let problem = match c.missing.len() {
        _ if usable => None,
        _ if only_text => Some(Problem::NoText),
        0 => Some(Problem::NoGame),
        n if n > 3 || !setup::looks_like_game(root) => Some(Problem::NoGame),
        n => Some(Problem::Missing(n)),
    };
    let summary = Some(super::check::summarize(c, fp));
    CheckResult { root: root.to_path_buf(), lines, usable, problem, summary }
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
        language: crate::qol::setup::saved_language().unwrap_or_default(),
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
        // The launcher does not run in the game's language: the choice sets
        // the Metric default as the game's own start would.
        d.metric = Settings::metric_default(if c.language.is_empty() { crate::game_assets::DEFAULT_LANGUAGE } else { &c.language });
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

/// A fake asset report for the renders: the US layout (every movie
/// without a language suffix, movies.tag), all found but `missing`.
pub fn fake_assets(root: &Path, missing: &[&str]) -> crate::game_assets::Report {
    use crate::game_assets::{Edition, Group, How, Item, Layout, Report, Resolved, movie_stem, wanted_movies};
    let mut items: Vec<Item> = wanted_movies()
        .into_iter()
        .map(|n| {
            let stem = movie_stem(n);
            let gone = missing.iter().any(|m| m.eq_ignore_ascii_case(stem));
            let plain = crate::movie::file_name(n) == format!("{n}.vp6");
            let tried = vec![crate::movie::file_name(n), format!("{stem}.vp6"), format!("{stem}_en.vp6"), format!("{stem}_*.vp6")];
            let (file, how) = match () {
                _ if gone => (None, How::Missing),
                _ if plain && stem == n => (Some(root.join("MOVIES").join(format!("{n}.vp6"))), How::Built),
                _ => (Some(root.join("MOVIES").join(format!("{stem}.vp6"))), How::Plain),
            };
            Item { group: Group::of_movie(n), label: stem.to_string(), resolved: Resolved { want: n.to_string(), file, how, tried } }
        })
        .collect();
    items.extend(crate::frontend::assets::asset_files(crate::game_assets::DEFAULT_LANGUAGE).into_iter().map(|f| Item {
        group: Group::Text,
        resolved: Resolved { want: f.clone(), file: Some(root.join(&f)), how: How::Built, tried: vec![f.clone()] },
        label: f,
    }));
    items.sort_by_key(|i| i.group);
    let edition = Edition {
        layout: Layout::Plain { tag: true },
        languages: vec!["English".into()],
        text_language: "English".into(),
        exe_suffix: None,
    };
    Report { edition, items }
}

impl FakeEnv {
    pub fn root() -> PathBuf {
        PathBuf::from("/data/player/Games/Need for Speed Underground 2")
    }
}

impl Env for FakeEnv {
    fn load(&self) -> Model {
        // Times in a fixed form: the renders and tests do not depend on the
        // machine's time zone or language.
        super::localtime::fixed();
        let choices = Choices {
            fullscreen: true,
            resolution: (1920, 1080),
            graphics: Graphics::Original,
            skip: false,
            skip_movies: false,
            skip_title: false,
            texture_pack: String::new(),
            hud_mod: false,
            language: String::new(),
        };
        Model {
            ask: None,
            starting: false,
            show_details: false,
            details_sel: 0,
            support: None,
            start_unfinished: false,
            set_aside: None,
            prefs: self.prefs(),
            just_found: false,
            support_busy: false,
            locations: self.locations(),
            portable: self.portable(),
            editor_here: self.editor_present(),
            studio_here: self.studio_present(),
            studio_cli_here: self.studio_cli_present(),
            studio: EditorRun::Closed,
            studio_seen: self.studio_seen(&[]),
            pick_loc: None,
            copy_busy: false,
            copy_plan: None,
            play_args: Vec::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
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
                kind: super::run::ErrorKind::Crash,
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
                report: None,
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
            saves: super::saves_page::fake_page(),
            logs_dir: PathBuf::from("/data/player/.local/share/overbright/logs"),
            saves_dir: PathBuf::from("/data/player/.local/share/NFS Underground 2"),
            build: format!("Overbright {} (commit unknown, linux-x86_64)", env!("CARGO_PKG_VERSION")),
            source: Some("https://example.org/overbright".into()),
            notices: FAKE_NOTICES,
            native_dialog: false,
            mods: {
                let list = self.mods();
                let own = self.own_packs();
                let conflict = self.conflict(&own, &list.iter().map(|(e, _)| e.clone()).collect::<Vec<_>>());
                Mods { list, own, maps: self.maps(), paused: self.paused().is_some(), conflict, ..Default::default() }
            },
            hud: None,
            prepare: Prepare::default(),
            game_data: super::game_data::GameData {
                place: self.game_data_base().display().to_string(),
                ..Default::default()
            },
            dialog: None,
            pad_input: false,
            failure_details: false,
            gamescope: false,
            probe_again: false,
            folders: self.folders(),
        }
    }

    fn check(&self, root: &Path) -> CheckResult {
        let c = setup::Check {
            missing: Vec::new(),
            different: vec!["GLOBAL/GlobalB.lzc".into(), "FRONTEND/FrontB.lzc".into()],
            assets: Some(fake_assets(root, &[])),
            ..Default::default()
        };
        let mut r = check_result(root, &c, None);
        r.lines.insert(
            0,
            (
                Mark::Warn,
                "Version not recognised (2 files differ from the known release); the game may look or play differently"
                    .into(),
            ),
        );
        r
    }

    fn keep_root(&self, _root: &Path) -> Result<(), String> {
        Ok(())
    }

    fn support_file(&self, _req: &crate::diag::support::SupportRequest) -> std::io::Result<PathBuf> {
        Ok(PathBuf::from("/data/player/.local/share/overbright/logs/overbright-support-20261118-120000.txt"))
    }

    fn crash_errors(&self, _pid: u32, _exe_name: &str) -> Vec<String> {
        Vec::new()
    }

    fn copy_text(&self, text: &str) -> Result<(), String> {
        COPIED.with(|c| *c.borrow_mut() = Some(text.to_string()));
        Ok(())
    }

    fn reveal(&self, _path: &Path) {}

    fn save(&self, _c: &Choices, _first: Option<&FirstRun>) -> Result<(), String> {
        Ok(())
    }

    fn set_pack(&self, dir: &str) -> PackStatus {
        if dir.trim().is_empty() { PackStatus::None } else { PackStatus::Incomplete }
    }

    fn languages(&self, _root: &Path) -> Vec<String> {
        ["English", "French", "German"].map(String::from).to_vec()
    }

    fn set_language(&self, _name: &str) -> Result<(), String> {
        Ok(())
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

    fn saves_run(&self, job: saves::jobs::Job) -> saves::jobs::Done {
        super::saves_page::fake_run(job)
    }

    fn locations(&self) -> Vec<LocRow> {
        use crate::platform::dirs::Location as L;
        let base = PathBuf::from("/data/player/.local/share");
        L::ALL
            .into_iter()
            .map(|loc| {
                let path = match loc {
                    L::Saves => base.clone(),
                    L::Settings | L::Packs => base.join("overbright"),
                    L::Maps => base.join("overbright/maps"),
                    L::Cache => PathBuf::from("/data/player/.cache/overbright"),
                    L::Logs => base.join("overbright/logs"),
                    L::Screenshots => base.join("overbright/Screenshots"),
                };
                LocRow { loc, path: Some(path), custom: false, missing: false }
            })
            .collect()
    }

    fn set_location(&self, _loc: crate::platform::dirs::Location, _to: Option<&Path>) -> Result<(), String> {
        Ok(())
    }

    fn portable(&self) -> PortableInfo {
        PortableInfo {
            on: false,
            marker: false,
            changed: false,
            program: Some(PathBuf::from("/data/player/Games/overbright-0.3.0")),
            data: Some(PathBuf::from("/data/player/Games/overbright-0.3.0/data")),
            home: PathBuf::from("/data/player/.local/share/overbright"),
            home_saves: PathBuf::from("/data/player/.local/share"),
            writable: true,
        }
    }

    fn own_packs(&self) -> Vec<OwnPack> {
        vec![OwnPack {
            id: "night-roads".into(),
            name: "Night Roads".into(),
            author: "an author".into(),
            textures: 214,
            on: false,
            refused: None,
        }]
    }

    fn save_own(&self, _list: &[(String, bool)]) -> Result<(), String> {
        Ok(())
    }

    fn install_own(&self, src: &Path) -> Result<String, String> {
        Ok(src.file_name().map_or("pack".into(), |n| n.to_string_lossy().to_lowercase()))
    }

    fn remove_own(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    fn conflict(&self, own: &[OwnPack], packages: &[crate::texture_mods::Entry]) -> Option<Conflict> {
        // The fake packs: Night Roads and City roads HD change the same 214 road textures.
        let night = own.iter().any(|o| o.id == "night-roads" && o.on);
        let roads = packages.iter().any(|e| e.id == "roads-hd" && e.on);
        (night && roads).then(|| Conflict {
            first: "Night Roads".into(),
            second: "City roads HD".into(),
            textures: 214,
            same_list: false,
        })
    }

    fn maps(&self) -> Vec<MapRow> {
        vec![
            MapRow { folder: "drift-park".into(), name: "Drift park".into(), on: true, races: 2, props: 9, textures: 0 },
            MapRow { folder: "open-docks".into(), name: "Open docks".into(), on: false, races: 0, props: 14, textures: 5 },
        ]
    }

    fn set_map(&self, _folder: &str, _on: bool) -> Result<(), String> {
        Ok(())
    }

    fn import_map(&self, from: &Path) -> Result<String, String> {
        Ok(from.file_stem().map_or("map".into(), |n| n.to_string_lossy().to_lowercase()))
    }

    fn export_map(&self, folder: &str, to: &Path, derived_ok: bool) -> Result<PathBuf, String> {
        if self.map_derived(folder) > 0 && !derived_ok {
            return Err(crate::tools::map::share::derived_warning(self.map_derived(folder)));
        }
        Ok(to.join(format!("{folder}.zip")))
    }

    fn map_derived(&self, folder: &str) -> usize {
        if folder == "open-docks" { 5 } else { 0 }
    }

    fn editor_present(&self) -> bool {
        true
    }

    fn start_editor(&self, _folder: Option<&str>, _root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        Ok(Box::new(FakeEditor::default()))
    }

    fn studio_present(&self) -> bool {
        true
    }

    fn studio_cli_present(&self) -> bool {
        true
    }

    fn start_studio(&self, _root: Option<&Path>) -> Result<Box<dyn EditorJob>, String> {
        Ok(Box::new(FakeEditor::default()))
    }

    fn studio_seen(&self, _folders: &[String]) -> super::studio_seen::StudioSeen {
        super::studio_seen::StudioSeen { held: Vec::new(), last: Some("drift-park".into()) }
    }

    fn paused(&self) -> Option<Paused> {
        None
    }

    fn set_paused(&self, _p: Option<&Paused>) -> Result<(), String> {
        Ok(())
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
            return Some(HudCheck {
                is_pack: true,
                listed: 710,
                present: 710,
                missing: Vec::new(),
                maps: true,
                complete: true,
            });
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
