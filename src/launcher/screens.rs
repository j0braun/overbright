//! The launcher's screens: what each shows ([`build`]) and what its
//! controls do ([`App::event`]): first run (find or type the game folder),
//! the check result, the main screen, about (+ the licence and the
//! third-party notices), the start failure, the folder browser, the save
//! import, and a widget test screen (docs/launcher.md).

use ob_look::words;
use std::path::{Path, PathBuf};

use super::draw::theme;
use super::font::{Cut, Face, Fonts};
use super::layout::{Align, Builder, Ctrl, Link, build_fit};
use super::model::{
    BrowseFor, Dialog, DialogAsk, DirEntry, Env, Graphics, Issue, Model, Picked, ProbeState, Problem, Verdict,
    path_parts, windows_form,
};
use super::run::ErrorKind;
use super::ui::{Event, Input, Key, Mark, Rect, Ui, UiState, apply_edit, handle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    FirstRun,
    Check,
    Main,
    About,
    Licence,
    Notices,
    Failure,
    Browse,
    /// The saved games: profiles, backups, import and export.
    Saves,
    Mods,
    /// The prepare step (`game --prepare`): its bar, Skip.
    Prepare,
    Widgets,
    /// Settings: screen, resolution, graphics, movies, the start choice.
    Settings,
    /// Settings › Advanced: the rarely used rows and this computer.
    Advanced,
    /// The last start did not finish: safe settings offered.
    Recover,
    /// Settings › Advanced › Storage: portable mode and every folder.
    Storage,
    /// The developer tools: the map editor, the game with the editor
    /// tools, the support file and the logs.
    Dev,
    /// Your own copy of the game: offered after the game check, and in Settings › Advanced.
    GameData,
}

impl Screen {
    pub const ALL: [Screen; 18] = [
        Screen::FirstRun,
        Screen::Check,
        Screen::Main,
        Screen::About,
        Screen::Licence,
        Screen::Notices,
        Screen::Failure,
        Screen::Browse,
        Screen::Saves,
        Screen::Mods,
        Screen::Prepare,
        Screen::Widgets,
        Screen::Settings,
        Screen::Advanced,
        Screen::Recover,
        Screen::Storage,
        Screen::Dev,
        Screen::GameData,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Screen::FirstRun => "first-run",
            Screen::Check => "check",
            Screen::Main => "main",
            Screen::About => "about",
            Screen::Licence => "licence",
            Screen::Notices => "notices",
            Screen::Failure => "crash",
            Screen::Browse => "browse",
            Screen::Saves => "saves",
            Screen::Mods => "mods",
            Screen::Prepare => "prepare",
            Screen::Widgets => "widgets",
            Screen::Settings => "settings",
            Screen::Advanced => "settings-adv",
            Screen::Recover => "recover",
            Screen::Storage => "storage",
            Screen::Dev => "dev",
            Screen::GameData => "game-data",
        }
    }

    pub fn parse(s: &str) -> Option<Screen> {
        match s {
            "failure" => Some(Screen::Failure),
            _ => Screen::ALL.into_iter().find(|x| x.name() == s),
        }
    }

    /// The page in the top bar this screen belongs to (Game, Settings,
    /// Mods, Saves, Dev), if any.
    pub fn page(self) -> Option<usize> {
        match self {
            Screen::Main | Screen::Prepare => Some(0),
            Screen::Settings
            | Screen::Advanced
            | Screen::Storage
            | Screen::About
            | Screen::Licence
            | Screen::Notices
            | Screen::GameData => Some(1),
            Screen::Mods => Some(2),
            Screen::Saves => Some(3),
            Screen::Dev => Some(4),
            _ => None,
        }
    }

    /// A long document: the page scrolls instead of shrinking.
    pub fn document(self) -> bool {
        matches!(self, Screen::Licence | Screen::Notices)
    }

    /// A page that shows Studio's state (the Dev page, the Mods page's map
    /// mods): Studio's locks and recent list are read while it is shown.
    pub fn sees_studio(self) -> bool {
        matches!(self, Screen::Dev | Screen::Mods)
    }
}

/// How often the Dev and Mods pages read Studio's state again.
pub const STUDIO_LOOK: std::time::Duration = std::time::Duration::from_secs(2);

/// What the window must do after an input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Quit,
    /// Start the game (saved first).
    Play,
    /// End and start the launcher again (a change that applies at a start).
    Restart,
}

/// The name shown.
pub const NAME: &str = "Overbright";
/// The line every first screen carries.
pub const OWN_COPY: &str = "Overbright needs your own copy of Need for Speed Underground 2 for PC, any region, \
patched to 1.2. Nothing from the original game is included; Overbright reads your copy and never changes it.";
/// What the game folder is, by what it holds.
pub const GAME_FOLDER: &str = "The game folder is the one the game was installed to: it holds the folders GLOBAL, \
FRONTEND and TRACKS.";
/// The first-run screen's line on mods.
pub const NO_MODS: &str = "No mods are included either. Texture and HUD mods you have can be added later under Mods; \
they belong to their authors.";
pub const NOT_AFFILIATED: &str =
    "Overbright is a free reimplementation (GPL-3.0-or-later), not affiliated with or endorsed by Electronic Arts.";
/// The about screen's disclaimer: Electronic Arts' own wording for fan
/// projects, word for word (also README.md's and README.txt's).
pub const EA_DISCLAIMER: &str =
    "Overbright is not affiliated with Electronic Arts. EA has not endorsed and does not support this product.";
/// The about screen's line for a build without a source link (no
/// `OB_SOURCE_URL`; the release check fails on it).
pub const UNOFFICIAL: &str = "This is an unofficial build without a source link: \
ask whoever gave it to you for its source code.";
/// The about screen's line under Make portable.
pub const PORTABLE: &str = "Make portable keeps settings, saves, caches and logs in a folder named data beside \
     Overbright, for a game carried on a portable drive. What is kept now stays where it is: copy it over to take it along.";
pub const FREE: &str = "Overbright is free software: you can share and change it under the terms of the GNU General \
Public License, version 3 or (at your option) any later version. It comes with no warranty.";

/// The licence and the third-party notices, built in.
pub const LICENCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/LICENSE"));
pub const NOTICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/THIRD-PARTY-NOTICES.md"));

pub struct App<'e> {
    pub env: &'e dyn Env,
    pub model: Model,
    pub screen: Screen,
    pub st: UiState,
    pub size: (u32, u32),
    pub dpi: f32,
    pub ui: Ui,
    /// Where the folder browser, about and the import go back to.
    pub back: Screen,
    /// The page slide and Play's glow.
    pub motion: super::anim::Motion,
    /// The own copy's run (import, check, repair) on its worker ([`super::game_data`]).
    pub game_data_job: Option<super::game_data::Job>,
    /// Files being copied to a new folder on a worker.
    pub copy_job: Option<
        std::sync::mpsc::Receiver<(crate::platform::dirs::Location, PathBuf, Result<super::transfer::Done, String>)>,
    >,
    /// Keys are ignored until then (just after the system's folder dialog).
    pub keys_after: Option<std::time::Instant>,
    /// The support file being written on a worker.
    pub support_job: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    /// The system's errors for the crashed game being asked on a worker.
    pub crash_job: Option<std::sync::mpsc::Receiver<Vec<String>>>,
    /// The map editor started from the Mods page, while it is open.
    pub editor_job: Option<Box<dyn super::model::EditorJob>>,
    /// Studio, when the launcher started it (its end polled by the window).
    pub studio_job: Option<Box<dyn super::model::EditorJob>>,
    /// When Studio's locks and recent list were last read (the Dev and
    /// Mods pages read them again every [`STUDIO_LOOK`]).
    pub studio_looked: Option<std::time::Instant>,
    /// The Saves page's worker: its result on the way, and what its jobs
    /// keep between runs.
    pub saves_rx: Option<std::sync::mpsc::Receiver<super::saves::jobs::Done>>,
    pub saves_ctx: Option<super::saves::jobs::Ctx>,
}

fn res_text(m: &Model, r: (u32, u32)) -> String {
    let base = format!("{} × {}", r.0, r.1);
    if m.desktop() == Some(r) { format!("{base} (monitor)") } else { base }
}

const GRAPHICS: [Graphics; 2] = [Graphics::Original, Graphics::Enhanced];
const DEMO_VALUES: [&str; 4] = ["Small", "Medium", "Large", "A much longer value"];
/// The failure screen's log lines (the last ones).
const TAIL_SHOWN: usize = 8;

/// Display and graphics can be set: options.txt exists, or the probe gave
/// the first-run values (it knew the card and a usable game folder).
pub fn display_rows_on(m: &Model) -> bool {
    m.settings_file || m.probe.first_run().is_some()
}

fn gpu_text(m: &Model) -> (String, super::draw::Rgb) {
    match &m.probe {
        ProbeState::Pending => ("Checking this computer…".into(), theme::DIM),
        ProbeState::Ready(p) => match p.gpu_line() {
            Some(l) => (l, theme::TEXT),
            // The probe's own words (on Linux what is missing and what to
            // install, `platform::vulkan`).
            None => (p.error.clone().unwrap_or_else(|| "Not found: no graphics adapter".into()), theme::ERROR),
        },
        ProbeState::Failed(e) => (format!("Not checked: {e}"), theme::ERROR),
    }
}

/// A document's paragraphs: lines joined within a paragraph (blank lines
/// part them), list, table and heading lines kept on their own; characters
/// the font lacks shown as "?".
pub fn paragraphs(doc: &str, fonts: &Fonts) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let own_line = |l: &str| {
        let t = l.trim_start();
        t.starts_with(['-', '*', '|', '#']) || t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains(". ")
    };
    for raw in doc.lines().filter(|l| !l.trim_start().starts_with("```")) {
        let l: String = raw
            .trim_end()
            .chars()
            .map(|c| {
                if c == '\t' {
                    ' '
                } else if c.is_whitespace() || fonts.covers(Face::Regular, &c.to_string()) {
                    c
                } else {
                    '?'
                }
            })
            .collect();
        if l.trim().is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if own_line(&l) && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if cur.is_empty() {
            cur = l.trim().to_string();
        } else {
            cur.push(' ');
            cur += l.trim();
        }
        if l.trim_start().starts_with('#') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Lays out `screen` for `m`.
/// The browser's place links' ids (one per place shown).
pub const PLACE_IDS: [&str; 16] = [
    "home", "place1", "place2", "place3", "place4", "place5", "place6", "place7", "place8", "place9", "place10",
    "place11", "place12", "place13", "place14", "place15",
];
/// The browser's path parts' ids: `crumb0` the "…" before the parts
/// shown when the path is deep, then one per part.
pub const CRUMB_IDS: [&str; 10] =
    ["crumb0", "crumb1", "crumb2", "crumb3", "crumb4", "crumb5", "crumb6", "crumb7", "crumb8", "crumb9"];
/// The separator drawn between the path's parts.
const SEP: &str = "›";

/// The path's parts the browser shows (the last ones when it is deep)
/// and whether earlier ones were left out (a "…" link goes to the one
/// before the first shown).
pub fn crumbs(dir: &str) -> (Vec<(String, String)>, bool) {
    let all = path_parts(dir, windows_form(dir, cfg!(windows)));
    let keep = CRUMB_IDS.len() - 1;
    if all.len() > keep { (all[all.len() - keep..].to_vec(), true) } else { (all, false) }
}

/// The folder a path link goes to (`crumb0`: the part before the shown).
fn crumb_target(dir: &str, id: &str) -> Option<PathBuf> {
    let i: usize = id.strip_prefix("crumb")?.parse().ok()?;
    let all = path_parts(dir, windows_form(dir, cfg!(windows)));
    let keep = CRUMB_IDS.len() - 1;
    let skipped = all.len().saturating_sub(keep);
    let at = if i == 0 { skipped.checked_sub(1)? } else { skipped + i - 1 };
    all.get(at).map(|(_, p)| PathBuf::from(p))
}

pub fn build(b: &mut Builder, m: &Model, screen: Screen) {
    let setup = matches!(screen, Screen::FirstRun | Screen::Check)
        || (screen == Screen::Browse && m.browse.purpose == BrowseFor::Game && m.root.is_none());
    match screen.page() {
        Some(p) => b.top_bar(&PAGES, Some(p), screen == Screen::Main, None),
        None if setup => b.top_bar(&[], None, false, Some("Setup")),
        None => b.top_bar(&[], None, false, None),
    }
    match screen {
        Screen::FirstRun => build_first(b, m),
        Screen::Check => build_check(b, m),
        Screen::Settings => build_settings(b, m),
        Screen::Advanced => build_advanced(b, m),
        Screen::Storage => build_storage(b, m),
        Screen::Dev => build_dev(b, m),
        Screen::Recover => build_recover(b, m),
        Screen::Failure => build_crash(b, m),
        Screen::Saves => super::saves_page::build(b, m),
        _ => build_page(b, m, screen),
    }
    let version =
        format!("{}{}", m.version, portable_note(m));
    b.footer_legends(&version, legends_for(m, screen));
    match &m.ask {
        Some(Dialog::Quit) if matches!(screen, Screen::Main | Screen::FirstRun) => {
            b.dialog("Quit Overbright?", "The game will not start.", &[("stay", "Stay"), ("quit", words::QUIT)])
        }
        Some(Dialog::Reset) if screen == Screen::Settings => b.dialog(
            "Back to defaults?",
            "Screen, resolution, graphics, intro movies and the start choice go back to what Overbright picked \
             for this computer. Mods and the game folder stay as they are.",
            &[("keep_settings", "Keep my settings"), ("reset_yes", "Back to defaults")],
        ),
        _ => {}
    }
    if screen == Screen::Saves {
        super::saves_page::dialog(b, m);
    }
    if screen == Screen::Storage {
        match &m.ask {
            Some(Dialog::MoveLocation(loc, to)) => {
                let what = loc_name(*loc);
                let plan = copy_words(m);
                let mut buttons = vec![("loc_copy", "Copy and use it"), ("loc_empty", "Use it, copy nothing")];
                if m.copy_plan.as_ref().is_some_and(super::transfer::Plan::replaceable) {
                    buttons.push(("loc_copy_replace", "Copy, replace (backups kept)"));
                }
                buttons.push(("loc_cancel", words::CANCEL));
                b.dialog(
                    &format!("Put {} here?", what.to_lowercase()),
                    &format!(
                        "{}\n{plan}\nNothing already there is overwritten, and the old folder and its files stay \
                         until you delete them. Overbright and the game use the new folder from their next start.",
                        to.display()
                    ),
                    &buttons,
                );
            }
            Some(Dialog::PortableOn) => {
                let data = m.portable.data.as_ref().map_or(String::new(), |d| d.display().to_string());
                let plan = copy_words(m);
                let mut buttons =
                    vec![("port_copy", "Copy my files and turn on"), ("port_on", "Turn on, copy nothing")];
                if m.copy_plan.as_ref().is_some_and(super::transfer::Plan::replaceable) {
                    buttons.push(("port_copy_replace", "Copy, replace (backups kept)"));
                }
                buttons.push(("port_cancel", words::CANCEL));
                b.dialog(
                    "Keep everything beside Overbright?",
                    &format!(
                        "From the next start settings, saves, caches, logs and mods are kept in {data}. \
                         Overbright writes portable.txt beside the program. Nothing is deleted: what is kept now \
                         stays in {} unless you copy it.\n{plan}",
                        m.portable.home.display()
                    ),
                    &buttons,
                );
            }
            Some(Dialog::PortableOff) => {
                let data = m.portable.data.as_ref().map_or(String::new(), |d| d.display().to_string());
                let plan = copy_words(m);
                let mut buttons =
                    vec![("port_copy_back", "Copy my files back and turn off"), ("port_off", "Turn off, copy nothing")];
                if m.copy_plan.as_ref().is_some_and(super::transfer::Plan::replaceable) {
                    buttons.push(("port_copy_back_replace", "Copy, replace (backups kept)"));
                }
                buttons.push(("port_cancel", words::CANCEL));
                b.dialog(
                    "Keep the files in your home folders again?",
                    &format!(
                        "From the next start they are kept in {}. Overbright removes portable.txt; the folder {data} \
                         and everything in it stays.\n{plan}",
                        m.portable.home.display()
                    ),
                    &buttons,
                );
            }
            _ => {}
        }
    }
    if screen == Screen::Mods
        && let Some(Dialog::MapExportDerived(f, _, n)) = &m.ask
    {
        let name = m.mods.maps.iter().find(|r| &r.folder == f).map_or(f.as_str(), |r| r.name.as_str());
        b.dialog(
            words::DERIVED_TITLE,
            &format!("{name}: {}", crate::tools::map::share::derived_warning(*n)),
            &[("map_export_cancel", words::CANCEL), ("map_export_yes", words::EXPORT_ANYWAY)],
        );
    }
    if screen == Screen::Mods
        && let Some(key) = &m.mods.confirm
    {
        let lines = pack_lines(m);
        let name = lines.iter().find(|l| &l.key == key).map_or(key.as_str(), |l| l.name.as_str());
        b.dialog(
            &format!("Remove {name}?"),
            "Overbright deletes its copy. Your downloaded file stays where it is.",
            &[("keep", "Keep it"), ("remove_yes", "Remove")],
        );
    }
}

/// The open copy question's lines: what would be copied, what is already
/// there, what differs (`transfer::Plan::words`).
fn copy_words(m: &Model) -> String {
    m.copy_plan.as_ref().map_or_else(|| "Nothing to copy.".to_string(), super::transfer::Plan::words)
}

/// The footer's portable note: the state in force, and what the next
/// start brings when the switch was changed in this session.
pub fn portable_note(m: &Model) -> &'static str {
    let now = m.folders.portable || m.portable.on;
    match (now, m.portable.marker) {
        (true, false) if m.portable.program.is_some() && m.portable.changed => " · Portable · off after restart",
        (false, true) => " · Portable after restart",
        (true, _) => " · Portable",
        _ => "",
    }
}

/// The pages in the top bar.
pub const PAGES: [(&str, &str); 5] = [
    ("page:game", "Game"),
    ("page:settings", "Settings"),
    ("page:mods", "Mods"),
    ("page:saves", "Saves"),
    ("page:dev", "Dev"),
];

/// The legends under a screen: what the buttons do there.
pub fn legends_for(m: &Model, screen: Screen) -> Vec<super::legends::Action> {
    use super::legends::{Btn, action};
    if m.ask.is_some() {
        return vec![action(Btn::Confirm, words::SELECT), action(Btn::Back, words::CANCEL)];
    }
    let mut v = vec![action(Btn::Confirm, words::SELECT)];
    match screen {
        Screen::Main => {
            v.push(action(Btn::Back, words::QUIT));
            v.push(action(Btn::Pages, "Pages"));
        }
        Screen::Settings => {
            v.push(action(Btn::Change, "Change"));
            v.push(action(Btn::Y, "Back to defaults"));
            v.push(action(Btn::Back, words::BACK));
        }
        Screen::Advanced => {
            v.push(action(Btn::Change, "Change"));
            v.push(action(Btn::Back, words::BACK));
        }
        Screen::Dev => {
            v.push(action(Btn::Pages, "Pages"));
            v.push(action(Btn::Back, words::BACK));
        }
        Screen::Mods => {
            v[0] = action(Btn::Confirm, "Turn on or off");
            v.push(action(Btn::Y, "Add a mod"));
            v.push(action(Btn::X, "Remove"));
            v.push(action(Btn::Back, words::BACK));
        }
        Screen::Saves => super::saves_page::legends(&mut v),
        Screen::FirstRun => v.push(action(Btn::Back, words::QUIT)),
        // Details as a legend only while the screen offers it.
        Screen::Check if m.check.as_ref().is_some_and(|c| c.summary.is_some()) => {
            v.push(action(Btn::Y, if m.show_details { "Hide details" } else { "Details" }));
            v.push(action(Btn::Back, words::BACK));
        }
        Screen::Browse => {
            v.push(action(Btn::X, "Use this folder"));
            v.push(action(Btn::Back, words::BACK));
        }
        _ => v.push(action(Btn::Back, words::BACK)),
    }
    v
}

/// The screens kept from before the redesign, on the slab.
fn build_page(b: &mut Builder, m: &Model, screen: Screen) {
    match screen {
        Screen::Main => build_main(b, m),
        Screen::About => {
            b.page_title("About Overbright", Some("Settings"));
            b.body(&m.build);
            b.gap(6.0);
            match &m.source {
                Some(url) => b.body(&format!("Source code: {url}")),
                None => b.status(Mark::Warn, UNOFFICIAL),
            }
            b.gap(10.0);
            b.body(FREE);
            b.gap(10.0);
            b.body(OWN_COPY);
            b.gap(10.0);
            b.dim(EA_DISCLAIMER);
            b.gap(10.0);
            b.buttons(
                Align::Left,
                vec![Ctrl::button("licence", "Licence (GPL-3.0)"), Ctrl::button("notices", "Third-party notices")],
            );
            b.gap(10.0);
            if m.folders.portable {
                b.status(Mark::Ok, "Portable: settings, saves, caches and logs are kept beside the program, in:");
                line(b, &m.folders.data.display().to_string());
            } else {
                b.dim("Settings and logs are kept in:");
                line(b, &m.folders.data.display().to_string());
                b.gap(6.0);
                b.buttons(Align::Left, vec![Ctrl::button("portable", "Make portable")]);
                b.dim(PORTABLE);
            }
            message(b, m);
            b.footer(vec![Ctrl::primary("back", words::BACK)]);
            b.default_focus = Some("back");
        }
        Screen::Licence | Screen::Notices => {
            b.buttons(Align::Left, vec![Ctrl::primary("back", words::BACK)]);
            b.gap(8.0);
            b.dim("Arrow keys, Page Up / Page Down or the mouse wheel scroll the text.");
            b.gap(14.0);
            let doc = if screen == Screen::Licence { LICENCE } else { m.notices };
            for p in paragraphs(doc, b.fonts) {
                if let Some(h) = p.strip_prefix('#') {
                    b.gap(6.0);
                    b.heading(h.trim_start_matches('#').trim());
                } else {
                    let size = b.m.small();
                    b.para(&p, Face::Regular, size, theme::TEXT);
                    b.gap(8.0);
                }
            }
            b.default_focus = Some("back");
        }
        Screen::Browse => {
            let br = &m.browse;
            b.heading(match br.purpose {
                BrowseFor::Game => "Choose the game folder",
                BrowseFor::TexturePack => "Choose the texture-pack folder",
                BrowseFor::Saves => "Choose the original game's save folder",
                BrowseFor::Packages => "Add texture packages",
                BrowseFor::HudMod => "Add the HUD mod",
                BrowseFor::MapMod => "Import a map mod",
                BrowseFor::MapExport => "Export the map mod to…",
                BrowseFor::SavesExport => "Export the profile to…",
                BrowseFor::Location => "Choose the new folder",
            });
            // The places, then the folder shown as clickable parts.
            let places: Vec<Link> = std::iter::once(Link::Sep("Places:".into()))
                .chain(br.places.iter().zip(PLACE_IDS).map(|((name, _), id)| Link::Go {
                    id,
                    text: name.clone(),
                    strong: false,
                    whole: None,
                }))
                .collect();
            if places.len() > 1 {
                b.links(places);
            }
            let (crumbs, cut) = crumbs(&br.dir.display().to_string());
            let mut parts: Vec<Link> = Vec::new();
            if cut {
                parts.push(Link::Go { id: CRUMB_IDS[0], text: "…".into(), strong: false, whole: None });
            }
            let last = crumbs.len().saturating_sub(1);
            for (i, (name, _)) in crumbs.iter().enumerate() {
                if !parts.is_empty() {
                    parts.push(Link::Sep(SEP.into()));
                }
                let whole = (i == last).then(|| br.dir.display().to_string());
                parts.push(Link::Go { id: CRUMB_IDS[i + 1], text: name.clone(), strong: i == last, whole });
            }
            if br.dir.as_os_str().is_empty() && !cfg!(windows) {
                line(b, "This computer's drives");
            } else {
                b.links(parts);
            }
            b.gap(6.0);
            let mut items: Vec<(String, String)> = Vec::new();
            if !br.dir.as_os_str().is_empty() {
                items.push(("..".into(), "up one folder".into()));
            }
            items.extend(br.entries.iter().map(|e| (e.name.clone(), e.note.clone())));
            if let Some(e) = &br.error {
                b.status(Mark::Error, e);
            }
            if items.is_empty() {
                b.dim("No folders here.");
            } else {
                b.list_grow("dirs", &items, br.sel.min(items.len() - 1), 4);
                b.default_focus = Some("dirs");
            }
            b.gap(12.0);
            b.buttons(
                Align::Left,
                vec![
                    Ctrl::Button {
                        id: "up",
                        text: "Up one folder".into(),
                        primary: false,
                        enabled: !br.dir.as_os_str().is_empty(),
                    },
                    Ctrl::Field {
                        id: "browse_path",
                        text: br.input.clone(),
                        placeholder: "Type a path, then Enter".into(),
                    },
                ],
            );
            b.gap(6.0);
            b.dim(match br.purpose {
                BrowseFor::Packages => {
                    "A double click (or Enter, or Open) opens the folder or imports the package chosen in the list; \
                     “Import all here” takes every package in the folder shown above (or the folder itself when it \
                     holds TexMod's texmod.def or a TexWizard pack; in the game folder, the packs TexWizard loads)."
                }
                BrowseFor::HudMod => {
                    "A double click (or Enter, or Open) opens the folder or takes the .zip chosen in the list; “Use \
                     this folder” takes the folder shown above: the mod's extracted folder, or the one that holds \
                     both downloads."
                }
                BrowseFor::Game => {
                    "Open (or a double click) enters a folder; “Use this folder” takes the chosen game folder, else \
                     the one above."
                }
                _ => {
                    "A double click (or Enter, or Open) opens the folder chosen in the list; “Use this folder” takes \
                     the one shown above."
                }
            });
            message(b, m);
            let mut foot = vec![Ctrl::button("cancel", words::CANCEL)];
            if m.native_dialog {
                foot.push(Ctrl::button("system", "Windows dialog…"));
            }
            foot.push(Ctrl::Button { id: "open", text: words::OPEN.into(), primary: false, enabled: !items.is_empty() });
            foot.push(Ctrl::Button {
                id: "use",
                text: if br.purpose == BrowseFor::Packages { "Import all here" } else { "Use this folder" }.into(),
                primary: true,
                enabled: !br.dir.as_os_str().is_empty(),
            });
            b.footer(foot);
        }
        Screen::Mods => build_mods(b, m),
        Screen::Prepare => build_prepare(b, m),
        Screen::GameData => super::game_data::build(b, m),
        Screen::FirstRun
        | Screen::Check
        | Screen::Failure
        | Screen::Settings
        | Screen::Advanced
        | Screen::Recover
        | Screen::Storage
        | Screen::Saves
        | Screen::Dev => {}
        Screen::Widgets => {
            b.label_column(&["Cycle", "Text field", "Buttons"]);
            b.heading("Widget test screen");
            b.dim("A label: long enough to wrap onto a second line at the smaller window sizes.");
            b.gap(8.0);
            b.row(
                "Cycle",
                vec![Ctrl::Cycle {
                    id: "cycle",
                    value: DEMO_VALUES[m.demo.cycle].into(),
                    values: DEMO_VALUES.map(String::from).to_vec(),
                    enabled: true,
                }],
            );
            b.row(
                "Text field",
                vec![Ctrl::Field { id: "text", text: m.demo.text.clone(), placeholder: "Type here".into() }],
            );
            b.row(
                "Buttons",
                vec![
                    Ctrl::button("b1", "Button"),
                    Ctrl::primary("b2", "Main button"),
                    Ctrl::Button { id: "b3", text: "Disabled".into(), primary: false, enabled: false },
                ],
            );
            b.checkbox("check", "A checkbox", m.demo.checked);
            b.gap(8.0);
            let items: Vec<(String, String)> =
                ["First item", "Second item, selected", "Third item", "Fourth item (scrolled)"]
                    .iter()
                    .map(|s| (s.to_string(), "note".to_string()))
                    .collect();
            b.list("list", &items, m.demo.sel, 2);
            b.gap(10.0);
            b.status(Mark::Ok, "A fine status line");
            b.status(Mark::Warn, "A warning status line");
            b.status(Mark::Error, "An error status line");
            b.footer(vec![Ctrl::button("quit", words::QUIT)]);
        }
    }
}

/// A problem in the player's words: a title naming it, one sentence with
/// the fix, and the one button that does it (its id and text). No file
/// sizes, checksums, addresses or command lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Words {
    pub title: &'static str,
    pub sentence: &'static str,
    pub fix: (&'static str, &'static str),
}

/// The check screen's words for a folder Overbright cannot use.
pub fn problem_words(p: Problem) -> Words {
    let another = ("another", "Choose another folder…");
    match p {
        Problem::OwnFolder => Words {
            title: "This is Overbright's folder",
            sentence: "Choose the folder the game was installed to: it holds GLOBAL, FRONTEND and TRACKS.",
            fix: another,
        },
        Problem::NoGame => Words {
            title: "No game files here",
            sentence: "Choose the folder the game was installed to: it holds GLOBAL, FRONTEND and TRACKS.",
            fix: another,
        },
        Problem::Missing(_) => Words {
            title: "Some game files are missing",
            sentence: "Install the game again from its discs, or copy the whole game folder again, then check again.",
            fix: ("recheck", "Check again"),
        },
        Problem::Unreadable => Words {
            title: "Overbright cannot read this folder",
            sentence: "Choose a folder you have access to, or copy the game to one.",
            fix: another,
        },
        Problem::NoText => Words {
            title: "The game's text file is missing",
            sentence: "Choose a copy of the game that has a text file Overbright can show, such as LANGUAGES/English.bin.",
            fix: another,
        },
    }
}

/// The failure screen's words for how the game stopped.
pub fn failure_words(k: ErrorKind, during_start: bool) -> Words {
    let retry = ("retry", "Try again");
    match k {
        ErrorKind::GameFiles => Words {
            title: "The game files are not complete",
            sentence: "The game could not read all of its files. Check the game folder again, or choose the one \
                       the game was installed to.",
            fix: ("folder", "Check the game folder"),
        },
        ErrorKind::NoGpu => Words {
            title: "No graphics driver the game can use",
            sentence: "Overbright draws with Vulkan. Install or update the graphics driver for your card, then try \
                       again.",
            fix: retry,
        },
        ErrorKind::NoWindow => Words {
            title: "The game window could not open",
            sentence: "Start it in a window instead of full screen.",
            fix: ("windowed", "Play windowed"),
        },
        ErrorKind::Crash => Words {
            title: "The game stopped unexpectedly",
            sentence: "Try again. If it happens again, send the log file when you ask for help.",
            fix: retry,
        },
        ErrorKind::NoProgram => Words {
            title: "The game could not be started",
            sentence: "Overbright's game program is missing or cannot run. Unpack the whole Overbright download \
                       again, into a folder of its own.",
            fix: retry,
        },
        ErrorKind::Other if during_start => Words {
            title: "The game stopped during its start",
            sentence: "Try again. If it happens again, set Display to Windowed or Graphics to Original, and send \
                       the log file when you ask for help.",
            fix: retry,
        },
        ErrorKind::Other => Words {
            title: "The game stopped",
            sentence: "Try again. If it happens again, send the log file when you ask for help.",
            fix: retry,
        },
    }
}

/// The game's state on the main screen, beside Play: its mark and words,
/// from the full check of the folder in use (never assumed) and this
/// computer's check.
pub fn game_state(m: &Model) -> (Mark, String) {
    let Some(root) = &m.root else { return (Mark::Warn, "Not found yet: choose the game's folder".into()) };
    let (mark, folder) = match m.check.as_ref().filter(|c| &c.root == root).map(|c| c.verdict()) {
        Some(Verdict::Ready) => (Mark::Ok, "Ready to play"),
        // The notes (files that differ from the known release) are the
        // check screen's; the game starts with them.
        Some(Verdict::Notes) => (Mark::Ok, "Ready to play"),
        Some(Verdict::Refused(p)) => {
            return (Mark::Error, format!("{}: choose another folder", problem_words(p).title));
        }
        None => (Mark::Info, "Game folder not checked yet"),
    };
    // "Ready to play" only once this computer is checked too; before,
    // the folder's part alone.
    let found = if mark == Mark::Ok { "Game folder checked" } else { folder };
    match &m.probe {
        ProbeState::Pending => (Mark::Info, format!("{found} · checking this computer…")),
        ProbeState::Ready(p) if !p.ok() => (Mark::Warn, format!("{found} · no graphics adapter answered (see below)")),
        ProbeState::Failed(_) => (Mark::Warn, format!("{found} · this computer was not checked")),
        // The own copy being made behind the main screen.
        _ if m.game_data.running() => (Mark::Info, m.game_data.background_words().unwrap_or_default()),
        _ => match &m.prepare.state {
            // The prepare step running by itself (`Prepare::background`).
            super::prepare::State::Idle | super::prepare::State::Running { .. } if m.prepare.background => {
                (Mark::Info, m.prepare.state.background_words())
            }
            _ => (mark, folder.into()),
        },
    }
}

/// The title the system folder dialog (and the own browser) carries.
pub fn dialog_title(p: BrowseFor) -> &'static str {
    match p {
        BrowseFor::Game => "Choose the game folder (it holds GLOBAL, FRONTEND and TRACKS)",
        BrowseFor::TexturePack => "Choose the HUD mod folder",
        BrowseFor::Saves => "Choose the original game's save folder",
        BrowseFor::Packages => "Choose the folder that holds the texture packages",
        BrowseFor::HudMod => "Choose the HUD mod's folder",
        BrowseFor::MapMod => "Choose the map mod (a folder with map.obmap, its .zip) or a mod pack (.obpk)",
        BrowseFor::MapExport => "Choose the folder to export the map mod to",
        BrowseFor::SavesExport => "Choose the folder to export the profile to",
        BrowseFor::Location => "Choose the new folder",
    }
}

/// Whether Play can do its job now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Ready,
    /// This computer is being checked (the probe runs).
    Checking,
    /// The check found no graphics adapter the game can use.
    NoGpu,
}

pub fn play_state(m: &Model) -> PlayState {
    match &m.probe {
        ProbeState::Pending => PlayState::Checking,
        ProbeState::Ready(p) if !p.ok() => PlayState::NoGpu,
        _ => PlayState::Ready,
    }
}

/// The main screen: the game's name and state, Play (or the fix in its
/// place), the summary rows that open their pages.
fn build_main(b: &mut Builder, m: &Model) {
    let c = &m.choices;
    let t = super::draw::theme();
    b.game_title("Need for Speed Underground 2");
    let issue = main_issue(m);
    let (mark, state) = main_status(m, issue.as_ref());
    b.status(mark, &state);
    if let Some(r) = &m.root {
        let size = b.fs_min(Face::Body, 15.0, 14.0);
        let full = r.display().to_string();
        // The game check's drop-down at the path's right (closed by
        // default): the summary rows under the path when open.
        let summary = m.check.as_ref().filter(|c| &c.root == r && c.usable).and_then(|c| c.summary.as_ref());
        let link = summary.map(|_| if m.show_details { "Hide the game check" } else { "Game check" });
        let pad = b.a(8.0);
        let lw = link.map_or(0.0, |l| b.fonts.width(Face::Body, size, l).ceil() + 2.0 * pad);
        let path_w = (b.width() - lw - if lw > 0.0 { b.a(16.0) } else { 0.0 }).max(0.0);
        let shown = b.fonts.fit(Face::Body, size, &full, path_w, Cut::Middle);
        let (sw, sh) = (b.fonts.width(Face::Body, size, &shown), b.fonts.line_height(Face::Body, size));
        let rect = Rect::new(b.x0, b.y, path_w, sh);
        b.widgets.push(super::ui::Widget {
            id: None,
            rect,
            kind: super::ui::Kind::Label,
            texts: vec![super::ui::text_of(Rect::new(b.x0, b.y, sw, sh), shown, &full, Face::Body, size, t.dim)],
            enabled: true,
        });
        if let Some(l) = link {
            let tw = b.fonts.width(Face::Body, size, l);
            let x = b.x1 - lw;
            b.widgets.push(super::ui::Widget {
                id: Some("details"),
                rect: Rect::new(x, b.y, lw, sh),
                kind: super::ui::Kind::Link,
                texts: vec![super::ui::text_of(Rect::new(x + pad, b.y, tw, sh), l.into(), l, Face::Body, size, theme::TEXT)],
                enabled: true,
            });
        }
        b.y += sh;
        if let Some(s) = summary.filter(|_| m.show_details) {
            b.gap(6.0);
            let rows = s.rows();
            let labels: Vec<&str> = rows.iter().map(|r| r.2).collect();
            let lw = b.lamp_label_w(&labels);
            for (_, mark, label, value) in &rows {
                b.lamp_row(None, *mark, label, lw, value);
            }
        }
        if m.just_found {
            b.gap(4.0);
            b.status(Mark::Info, "Found your game in this folder. Play keeps it for the next start.");
        }
    }
    b.gap(6.0);
    if let Some(i) = issue.as_ref().filter(|i| i.blocks_play()) {
        // The fix takes Play's place; the summary rows wait for it.
        let w = i.words();
        // No driver: the check's own line says what to install here.
        let text = match (&m.probe, i) {
            (ProbeState::Ready(p), Issue::NoVulkan) if p.error.is_some() => {
                let e = p.error.clone().unwrap_or_default();
                let mut c = e.chars();
                c.next().map_or(e.clone(), |f| f.to_uppercase().chain(c).collect::<String>() + ".")
            }
            _ => w.text.clone(),
        };
        b.message_with(i.mark(), &w.title, &text, issue_buttons(&w));
        b.default_focus = w.buttons.first().map(|x| x.0);
    } else {
        let checking = play_state(m) == PlayState::Checking;
        b.play("play", if m.starting { "Starting…" } else { "Play" }, m.starting || checking);
        b.default_focus = if checking { None } else { Some("play") };
        let screen = if display_rows_on(m) {
            format!("{} · {}", res_text(m, c.resolution), if c.fullscreen { "Full screen" } else { "Window" })
        } else {
            "Set at the game's first start".into()
        };
        b.sum_row("row_graphics", "Graphics", &c.graphics.label());
        b.sum_row("row_screen", "Screen", &screen);
        b.sum_row("row_mods", "Mods", &mods_summary(m));
        shader_block(b, m, true);
    }
    message(b, m);
}

/// What the shaders' preparation is at, for its block.
#[derive(Clone, Debug, PartialEq)]
pub enum Shaders {
    /// Nothing to show (prepared, or not known yet).
    Quiet,
    /// The hardware check says they are not prepared, and no run goes on.
    NotPrepared,
    /// A run goes on.
    Running,
    /// A run just finished ("Shaders ready", for a few seconds).
    JustReady,
    /// A run did not finish (its reason).
    Failed(String),
}

pub fn shaders(m: &Model) -> Shaders {
    use super::prepare::State;
    let p = &m.prepare;
    match &p.state {
        State::Running { .. } => Shaders::Running,
        State::Idle if p.wanted || (p.background && p.started.is_some()) => Shaders::Running,
        State::Done if p.done_at.is_some() => Shaders::JustReady,
        State::Failed(e) => Shaders::Failed(e.clone()),
        _ => match &m.probe {
            ProbeState::Ready(pr) if pr.ok() && pr.prepared == Some(false) && p.state != State::Done && m.root.is_some() => {
                Shaders::NotPrepared
            }
            _ => Shaders::Quiet,
        },
    }
}

/// The shaders' block: what preparing them gives, how far a run is (bar,
/// percent, step, time left), its button. Play stays the main button.
/// `main`: on the main screen (Prepare now / Stop); else the prepare
/// screen (Stop / Continue).
fn shader_block(b: &mut Builder, m: &Model, main: bool) {
    let why = "Preparing them once makes the first race start smoothly, without stutter.";
    let p = &m.prepare;
    let left = p.started.and_then(|s| p.state.time_left(s.elapsed()));
    let progress = Some((p.state.fraction(), p.state.progress_words(left)));
    let stop = Ctrl::button(if main { "prepare_stop" } else { "skip" }, "Stop");
    match shaders(m) {
        Shaders::Quiet => {}
        Shaders::NotPrepared => {
            b.gap(10.0);
            b.message_with(
                Mark::Info,
                "Shaders aren't prepared yet",
                &format!("{why} It takes from a few seconds to about a minute; you can play while it runs."),
                vec![Ctrl::button("prepare_now", "Prepare now")],
            );
        }
        Shaders::Running => {
            b.gap(10.0);
            let text = if main { format!("{why} You can play now; the first race may load a little slower.") } else { why.to_string() };
            b.message_progress(Mark::Busy, "Getting the shaders ready", &text, progress, vec![stop]);
        }
        Shaders::JustReady => {
            b.gap(10.0);
            b.message_progress(
                Mark::Ok,
                "Shaders ready",
                "The first race starts without stutter.",
                Some((1.0, "100 %".into())),
                Vec::new(),
            );
        }
        Shaders::Failed(e) => {
            b.gap(10.0);
            b.message_with(
                Mark::Warn,
                "Shaders aren't prepared yet",
                &format!("Preparing did not finish: {e}. The game works without it; the first race may load a little slower."),
                vec![Ctrl::button("prepare_now", "Try again")],
            );
        }
    }
}

/// What stops Play on the main screen (or notes beside it), if anything:
/// the folder gone or refused, no graphics driver.
pub fn main_issue(m: &Model) -> Option<Issue> {
    let root = m.root.as_ref()?;
    if let Some(c) = m.check.as_ref().filter(|c| &c.root == root) {
        match c.verdict() {
            Verdict::Refused(Problem::NoGame) if !root.is_dir() => {
                return Some(Issue::FolderGone { path: root.clone() });
            }
            Verdict::Refused(p) => return Some(c.issue().unwrap_or(Issue::of_problem(p))),
            Verdict::Notes => {}
            Verdict::Ready => {}
        }
    }
    if play_state(m) == PlayState::NoGpu {
        return Some(Issue::NoVulkan);
    }
    None
}

/// The status line under the game's name.
fn main_status(m: &Model, issue: Option<&Issue>) -> (Mark, String) {
    if m.root.is_none() {
        return (Mark::Warn, "No game folder chosen".into());
    }
    if issue.is_some_and(|i| i.blocks_play()) {
        return (Mark::Error, "Not ready: see below".into());
    }
    match play_state(m) {
        PlayState::Checking => return (Mark::Busy, "Checking…".into()),
        PlayState::NoGpu => return (Mark::Error, "Not ready: see below".into()),
        PlayState::Ready => {}
    }
    use super::check::CheckState;
    match m.check.as_ref().filter(|c| Some(&c.root) == m.root.as_ref()).map(|c| c.state()) {
        Some(CheckState::Changed) => (Mark::Ok, "Ready · version 1.2 · some game files are not the original ones".into()),
        Some(CheckState::MoviesMissing) => (Mark::Ok, "Ready · version 1.2 · some movies will not play".into()),
        Some(CheckState::UnknownEdition) => (Mark::Ok, "Ready · a copy Overbright doesn't recognise".into()),
        _ => (Mark::Ok, "Ready · version 1.2 · English".into()),
    }
}

/// An issue's buttons as controls, the first the main one.
fn issue_buttons(w: &super::model::Words) -> Vec<Ctrl> {
    w.buttons
        .iter()
        .enumerate()
        .map(|(i, (id, l))| if i == 0 { Ctrl::primary(id, l) } else { Ctrl::button(id, l) })
        .collect()
}

/// The first-run screen: the rule, the copies found (or none), the way to
/// choose the folder.
fn build_first(b: &mut Builder, m: &Model) {
    b.page_title("Find your game", None);
    b.body(RULE);
    b.gap(14.0);
    if m.found.is_empty() {
        b.body("No installed copy was found on this computer.");
        b.gap(10.0);
        b.buttons(Align::Left, vec![Ctrl::primary("browse", "Choose the game folder…")]);
        b.default_focus = Some("browse");
        b.gap(8.0);
        b.dim(&format!("It is the folder with GLOBAL, FRONTEND and TRACKS in it. {}", USUAL_PLACE));
    } else {
        b.dim("Found on this computer:");
        b.gap(6.0);
        let items: Vec<(String, String)> =
            m.found.iter().map(|f| (f.path.display().to_string(), f.source.clone())).collect();
        b.list("found", &items, m.found_sel, items.len().clamp(1, 4));
        b.default_focus = Some("found");
        b.gap(10.0);
        b.buttons(
            Align::Left,
            vec![Ctrl::primary("check", "Use this copy"), Ctrl::button("browse", "Choose another folder…")],
        );
    }
    b.gap(12.0);
    b.dim("Or type the folder's path:");
    b.gap(4.0);
    b.buttons(
        Align::Left,
        vec![Ctrl::Field { id: "path", text: m.path_input.clone(), placeholder: "Game folder".into() }],
    );
    message(b, m);
    b.gap(12.0);
    b.dim(NO_MODS);
    b.gap(4.0);
    b.dim(NOT_AFFILIATED);
}

/// The chosen folder and its verdict (docs/launcher.md, "The game
/// check"): the path, the verdict card (lamp, title, one sentence, the
/// fix or Use this folder focused), the summary rows of a usable folder
/// and, on demand, the details list by area.
fn build_check(b: &mut Builder, m: &Model) {
    let t = super::draw::theme();
    b.page_title("Game folder", None);
    let path = m.check.as_ref().map(|c| c.root.display().to_string()).unwrap_or_default();
    let size = b.fs_min(Face::Body, 15.0, 14.0);
    let shown = b.fonts.fit(Face::Body, size, &path, b.width(), Cut::Middle);
    let (sw, sh) = (b.fonts.width(Face::Body, size, &shown), b.fonts.line_height(Face::Body, size));
    b.widgets.push(super::ui::Widget {
        id: None,
        rect: Rect::new(b.x0, b.y, b.width(), sh),
        kind: super::ui::Kind::Label,
        texts: vec![super::ui::text_of(Rect::new(b.x0, b.y, sw, sh), shown, &path, Face::Body, size, t.dim)],
        enabled: true,
    });
    b.y += sh;
    b.gap(12.0);
    let open = m.show_details;
    let check = m.check.as_ref();
    let summary = check.and_then(|c| c.summary.as_ref());
    let issue = match check {
        Some(c) => c.issue(),
        None => Some(Issue::of_problem(Problem::NoGame)),
    };
    match (&issue, summary) {
        (Some(i), _) => {
            let mut w = i.words();
            for x in &mut w.buttons {
                if x.0 == "details" && open {
                    x.1 = "Hide details";
                }
            }
            b.message_with(Mark::Error, &w.title, &w.text, issue_buttons(&w));
            b.default_focus = w.buttons.first().map(|x| x.0);
        }
        (None, Some(s)) => {
            let v = super::check::verdict(s, open);
            let ctrls = v
                .buttons
                .iter()
                .enumerate()
                .map(|(i, (id, l))| if i == 0 { Ctrl::primary(id, l) } else { Ctrl::button(id, l) })
                .collect();
            b.message_with(v.mark, v.title, &v.text, ctrls);
            b.default_focus = Some(v.focus);
        }
        (None, None) => {
            // A usable folder without the full check (not met: the check
            // screen always has one).
            b.message_with(Mark::Ok, super::check::READY, "", vec![Ctrl::primary("use", "Use this folder")]);
            b.default_focus = Some("use");
        }
    }
    if let Some(s) = summary {
        // The summary rows of a usable folder; at the smallest heights the
        // open details take their place.
        let tight = b.m.h < b.m.s(560.0);
        if issue.is_none() && !(open && tight) {
            summary_rows(b, s);
        }
        if open && (issue.is_none() || b.widgets.iter().any(|w| w.id == Some("details"))) {
            details_list(b, m, s);
        }
    }
    message(b, m);
}

/// The summary rows: a lamp, the area, its value; each opens the details
/// at its area.
fn summary_rows(b: &mut Builder, s: &super::check::CheckSummary) {
    let rows = s.rows();
    let labels: Vec<&str> = rows.iter().map(|r| r.2).collect();
    let lw = b.lamp_label_w(&labels);
    for (id, mark, label, value) in &rows {
        b.lamp_row(Some(id), *mark, label, lw, value);
    }
}

/// The details list (scrolls with the pad, the keys and the wheel).
fn details_list(b: &mut Builder, m: &Model, s: &super::check::CheckSummary) {
    b.gap(8.0);
    let items: Vec<(String, String)> = s.details().into_iter().map(|(_, t)| (t, String::new())).collect();
    let n = items.len();
    let rows = (n.min(3) + b.grow).min(n);
    b.list("details_list", &items, m.details_sel, rows);
}

/// The suggested graphics for this computer (the probe's first-run
/// values), if known.
fn suggested_graphics(m: &Model) -> Option<Graphics> {
    m.probe.first_run().map(|f| if f.detail == 0 { Graphics::Original } else { Graphics::Enhanced })
}

/// Settings: the start settings as value rows, the description of the
/// focused row at the right.
fn build_settings(b: &mut Builder, m: &Model) {
    let c = &m.choices;
    b.page_title("Settings", None);
    let cols = b.columns(700.0);
    b.in_column(cols.left);
    let on = display_rows_on(m);
    if !m.gamescope {
        b.value_row(
            "display",
            "Screen",
            if c.fullscreen { "Full screen" } else { "Window" },
            &["Full screen".into(), "Window".into()],
            false,
            on,
        );
    }
    let modes: Vec<String> = m.modes().iter().map(|&r| res_text(m, r)).collect();
    if m.gamescope {
        b.value_row(
            "resolution",
            "Resolution",
            &format!("{} · set by Steam", res_text(m, c.resolution)),
            &modes,
            false,
            false,
        );
    } else {
        b.value_row("resolution", "Resolution", &res_text(m, c.resolution), &modes, false, on);
    }
    let mut gvals: Vec<String> = GRAPHICS.iter().map(|g| g.label()).collect();
    gvals.push(c.graphics.label());
    let suggested = suggested_graphics(m) == Some(c.graphics.clone());
    b.value_row("graphics", "Graphics", &c.graphics.label(), &gvals, suggested, on);
    b.value_row(
        "skip_movies",
        "Intro movies",
        if c.skip_movies { "Skip" } else { "Play" },
        &["Play".into(), "Skip".into()],
        false,
        on,
    );
    b.value_row(
        "skip_title",
        "Title screen",
        if c.skip_title { "Skip" } else { "Show" },
        &["Show".into(), "Skip".into()],
        false,
        on,
    );
    b.value_row(
        "skip",
        "When Overbright starts",
        if c.skip { "Start the game directly" } else { "Show this window" },
        &["Start the game directly".into(), "Show this window".into()],
        false,
        true,
    );
    {
        let set = super::legends::GlyphSet::for_input(b.st.input);
        b.row_note_glyph("To come back here, hold", super::legends::hold_glyph(set), "while Overbright starts.");
    }
    if !on {
        b.status(
            Mark::Info,
            match &m.probe {
                ProbeState::Pending => "Screen and graphics: available once this computer is checked.",
                ProbeState::Ready(p) if p.ok() && m.root.is_none() => {
                    "Screen and graphics: available once the game folder is chosen."
                }
                _ => "Screen and graphics: set them in the game's Options after its first start.",
            },
        );
    }
    b.sum_row("advanced", "Advanced settings", "");
    b.gap(18.0);
    b.buttons(Align::Left, vec![Ctrl::button("reset", "Back to defaults")]);
    message(b, m);
    let left_bottom = b.y;
    if let Some(r) = cols.right {
        let focus = b.st.focus.unwrap_or("display");
        let (h, text) = describe_setting(m, focus, b.st.input);
        b.describe(r, &h, &text);
    }
    b.y = left_bottom.max(b.widgets.last().map_or(0.0, |w| w.rect.bottom()));
    b.default_focus = Some(if m.gamescope { "graphics" } else { "display" });
}

/// The words beside the focused settings row.
pub fn describe_setting(m: &Model, id: &str, input: super::platform_deck::InputKind) -> (String, String) {
    let c = &m.choices;
    match id {
        "resolution" => (
            res_text(m, c.resolution),
            "The size of the game's picture. \"your screen\" fills the screen at its own sharpness.".into(),
        ),
        "graphics" => {
            let sug = if suggested_graphics(m) == Some(c.graphics.clone()) {
                "Suggested for your graphics card.\n"
            } else {
                ""
            };
            (
                c.graphics.label(),
                format!(
                    "{sug}• Original: exactly the 2004 game.\n• Enhanced: more detail and light; the same art and \\
                     colours as the 2004 game.\nEach graphics row can be changed in the game: Options › Display › \\
                     Enhancements."
                ),
            )
        }
        "skip_movies" => ("Intro movies".into(), "Play or skip the publisher and intro movies at each start.".into()),
        "skip_title" => (
            "Title screen".into(),
            "Show or skip the \"Press Enter\" title screen: Skip goes straight on to the menus.".into(),
        ),
        "skip" => (
            "When Overbright starts".into(),
            format!(
                "Show this window: this page comes first, Play starts the game.\nStart the game directly: the game \\
                 opens at once. {} Starting with --launcher works too.",
                come_back(input)
            ),
        ),
        "advanced" => (
            "Advanced settings".into(),
            "Text size, motion, launcher sounds, shaders, the support file, Storage (where files are kept), saves \
             from the original game and About."
                .into(),
        ),
        "reset" => (
            "Back to defaults".into(),
            "Puts screen, resolution, graphics, intro movies and the start choice back to what Overbright picked \\
             for this computer."
                .into(),
        ),
        _ => (
            if c.fullscreen { "Full screen" } else { "Window" }.into(),
            "Full screen fills the screen; Window keeps the game in a window on the desktop.".into(),
        ),
    }
}

/// The hint on the way back to this window, for the last input (a pad
/// player is never told about keys).
pub fn come_back(k: super::platform_deck::InputKind) -> &'static str {
    use super::platform_deck::InputKind as K;
    match k {
        K::Xbox | K::Deck => "To come back here, hold View while Overbright starts.",
        K::PlayStation => "To come back here, hold Create while Overbright starts.",
        K::Keyboard | K::Mouse => "To come back here, hold Shift while Overbright starts.",
    }
}
/// The rule every first screen states.
pub const RULE: &str = "Overbright plays your own copy of Need for Speed Underground 2 for PC. Any disc install with patch 1.2 works, \
     from any region.";
#[cfg(windows)]
const USUAL_PLACE: &str = "Usually: C:\\Program Files (x86)\\EA GAMES\\Need for Speed Underground 2";
#[cfg(not(windows))]
const USUAL_PLACE: &str = "With Wine or Proton it is inside the prefix, under drive_c.";

/// Settings › Advanced.
fn build_advanced(b: &mut Builder, m: &Model) {
    b.page_title("Advanced", Some("Settings"));
    let cols = b.columns(700.0);
    b.in_column(cols.left);
    let langs = m.supported_languages();
    b.value_row("language", "Game language", &m.language_name(), &langs, false, m.root.is_some() && langs.len() > 1);
    let no = m.unsupported_languages();
    if !no.is_empty() {
        // Languages with the same reason share it: a shorter line, so the page keeps fitting.
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for (l, why) in &no {
            match groups.iter_mut().find(|(w, _)| w == why) {
                Some((_, ls)) => ls.push(l),
                None => groups.push((why, vec![l])),
            }
        }
        let list: Vec<String> = groups.iter().map(|(why, ls)| format!("{} ({why})", ls.join(", "))).collect();
        b.dim(&format!("Not supported yet: {}.", list.join("; ")));
    }
    b.value_row(
        "text_size",
        "Text size",
        &format!("{} %", m.prefs.text),
        &super::model::Prefs::TEXT_SIZES.map(|v| format!("{v} %")),
        false,
        true,
    );
    b.value_row(
        "motion",
        "Motion",
        if m.prefs.reduced_motion { "Reduced" } else { "Full" },
        &["Full".into(), "Reduced".into()],
        false,
        true,
    );
    b.value_row(
        "sounds",
        "Launcher sounds",
        if m.prefs.sounds { "On" } else { "Off" },
        &["Off".into(), "On".into()],
        false,
        true,
    );
    if m.root.is_some() {
        b.sum_row("prepare", "Get shaders ready again", "");
        b.sum_row("game_data", "Game data", &m.game_data.row_words());
    }
    b.sum_row("import", "Import saves", "from the original game");
    b.sum_row("storage", "Storage", if m.portable.on { "Portable: beside the program" } else { "Where Overbright keeps its files" });
    b.sum_row("about", "About Overbright", "");
    message(b, m);
    if let Some(p) = &m.support {
        b.gap(8.0);
        b.buttons(Align::Left, vec![Ctrl::button("open_support", "Open folder")]);
        let _ = p;
    }
    let left_bottom = b.y;
    if let Some(r) = cols.right {
        let (gpu, _) = gpu_text(m);
        let pad = m.controller.clone().unwrap_or_else(|| "None (keyboard and mouse work)".into());
        b.describe(r, "This computer", &format!("Graphics card: {gpu}\nController: {pad}"));
    }
    b.y = left_bottom.max(b.widgets.last().map_or(0.0, |w| w.rect.bottom()));
    // The first row, the Game language.
    b.default_focus = Some("language");
}

/// The Dev page: Studio (its button and its command-line tool), the
/// standalone map editor, the game with the editor tools, the support file
/// and the logs, the overlay's key; what the focused row does at the right.
fn build_dev(b: &mut Builder, m: &Model) {
    b.page_title("Dev", None);
    let cols = b.columns(700.0);
    b.in_column(cols.left);
    b.section_a("Studio");
    b.buttons(Align::Left, vec![studio_button(m)]);
    studio_status(b, m);
    let rows = studio_rows(m);
    // Clear of the button's focus ring.
    b.gap(6.0);
    let lw = b.lamp_label_w(&rows.iter().map(|r| r.2.as_str()).collect::<Vec<_>>());
    for (id, mark, label, value) in &rows {
        b.lamp_row(Some(id), *mark, label, lw, value);
    }
    b.sum_row("studio_cli", "Command-line tool", if m.studio_cli_here { "Open folder" } else { "Not here" });
    b.section_a("Standalone map editor");
    b.buttons(Align::Left, vec![map_editor_button(m)]);
    if !m.editor_here {
        b.status(Mark::Error, "The map editor's program (overbright-editor) is not beside the launcher.");
    }
    if m.mods.editor != super::model::EditorRun::Closed {
        editor_status(b, m);
    }
    b.section_a("Editor tools in the game");
    b.sum_row("editor_play", "Play with editor tools", "F7 · F8");
    b.dim("F7: the texture editor · F8: the free camera · F3: the performance overlay");
    b.section_a("Reports");
    b.sum_row("support", "Support file", "Save…");
    b.sum_row("dev_logs", "Logs folder", "Open");
    if let Some(p) = &m.support {
        b.gap(6.0);
        b.status(Mark::Ok, &format!("Support file saved: {}", p.display()));
        b.buttons(Align::Left, vec![Ctrl::button("open_support", "Open folder")]);
    }
    message(b, m);
    let left_bottom = b.y;
    if let Some(r) = cols.right {
        let cli = if m.studio_cli_here {
            "overbright-studio-cli, beside the launcher: Studio's edits and checks from a shell or a script, one \
             command per edit, with JSON output. Run it in a terminal; \"overbright-studio-cli help\" lists every \
             command. Open folder shows where it is."
        } else {
            "overbright-studio-cli: Studio's edits and checks from a shell or a script. It is not beside the \
             launcher in this copy of Overbright."
        };
        let state_help = studio_state_help(m);
        let (h, t) = match b.st.focus.unwrap_or(dev_focus(m)) {
            "support" => (
                "Support file",
                "One text file for a bug report: this computer, the game's check, the settings and the last logs, \
                 with your home folder left out. It is saved in the logs folder.",
            ),
            "dev_logs" => ("Logs folder", "The game's and the launcher's logs, crash reports and support files."),
            "studio" => (
                "Studio (preview)",
                "Overbright Studio in its own window: the place where mods are made. The map editor is its World \
                 workspace. It is a preview: not every part is finished; keep a copy of mods you care about.",
            ),
            "studio_cli" => ("Command-line tool", cli),
            "studio_state" => ("Studio", state_help.as_str()),
            "studio_last" => (
                "Last mod",
                "The mod opened last in Studio's World workspace or in the map editor (they keep one list). Studio \
                 lists it on its Home.",
            ),
            "map_editor" => (
                "Standalone map editor (preview)",
                "The desktop map editor on its own, without Studio: props, custom races, test runs in the game and \
                 AI simulations. It is a preview: not every part is finished.",
            ),
            _ => (
                "Editor tools",
                "Starts the game once with --editor: F7 opens the texture editor, F8 the free camera. Your next \
                 start from Play is a normal one. F3 shows the performance overlay in every start.",
            ),
        };
        b.describe(r, h, t);
    }
    b.y = left_bottom.max(b.widgets.last().map_or(0.0, |w| w.rect.bottom()));
    b.default_focus = Some(dev_focus(m));
}

/// The Dev page's first focus: Studio's button (the page leads there), its
/// state row while the button is greyed.
fn dev_focus(m: &Model) -> &'static str {
    let open = matches!(m.studio, super::model::EditorRun::Open(_));
    if m.studio_here && !open { "studio" } else { "studio_state" }
}

/// Studio's button: starts Studio on its Home; greyed when it is not
/// beside the launcher (`dirs::studio_exe`) or while it is open.
pub fn studio_button(m: &Model) -> Ctrl {
    let open = matches!(m.studio, super::model::EditorRun::Open(_));
    Ctrl::Button { id: "studio", text: "Open Studio (preview)".into(), primary: false, enabled: m.studio_here && !open }
}

/// Why Studio failed, under its button (its other states are its rows).
fn studio_status(b: &mut Builder, m: &Model) {
    if let super::model::EditorRun::Failed(e) = &m.studio {
        b.status(Mark::Error, &format!("Studio: {e}"));
    }
}

/// A map mod's name on the Dev page (its folder when it is not listed).
fn mod_name<'a>(m: &'a Model, folder: &'a str) -> &'a str {
    m.mods.maps.iter().find(|r| r.folder == folder).map_or(folder, |r| r.name.as_str())
}

/// Studio's rows on the Dev page: Studio with its version (found beside
/// the launcher or not) and its state (open or not, the mod it has open,
/// from the mods' write locks); the last mod opened.
pub fn studio_rows(m: &Model) -> [(&'static str, Mark, String, String); 2] {
    use super::model::EditorRun;
    let seen = &m.studio_seen;
    let by_studio: Vec<&str> = seen.held.iter().filter(|h| h.by_studio()).map(|h| mod_name(m, &h.folder)).collect();
    let on = match by_studio.as_slice() {
        [] => None,
        [one] => Some(format!("on {one}")),
        [one, rest @ ..] => Some(format!("on {one} and {}", super::model::count(rest.len(), "more", "more"))),
    };
    let ours = matches!(m.studio, EditorRun::Open(_));
    let (mark, state) = match (m.studio_here, ours, on) {
        (_, true, Some(on)) => (Mark::Busy, format!("Open {on}")),
        (_, true, None) => (Mark::Busy, "Open, no mod open yet".to_string()),
        (_, false, Some(on)) => (Mark::Busy, format!("Open elsewhere {on}")),
        (true, false, None) => (Mark::Ok, "Ready, not open".to_string()),
        (false, false, None) => (Mark::Error, "Not beside the launcher".to_string()),
    };
    let label = if m.studio_here { format!("Studio {}", m.version) } else { "Studio".to_string() };
    let last = seen.last.as_deref().map_or_else(|| "None yet".to_string(), |f| mod_name(m, f).to_string());
    [("studio_state", mark, label, state), ("studio_last", Mark::Info, "Last mod".to_string(), last)]
}

/// The State row's help: what is open, and every mod's write lock with who
/// holds it (as Studio's note beside the lock says).
fn studio_state_help(m: &Model) -> String {
    let mut t = if m.studio_here {
        format!("overbright-studio {}, beside the launcher (it comes with this copy of Overbright). ", m.version)
    } else {
        "Studio's program (overbright-studio) is not beside the launcher in this copy of Overbright, so Open Studio \
         is greyed. "
            .to_string()
    };
    t.push_str(
        "Studio is open when the launcher started it or when it has a mod open (Studio holds a mod's lock while it \
         edits it, so no other program writes to it). ",
    );
    if m.studio_seen.held.is_empty() {
        t.push_str("No mod is open for writing now.");
    }
    for h in &m.studio_seen.held {
        let who = match (&h.program, h.pid) {
            // The command-line tool's own name ("overbright-studio (command line)").
            (Some(p), Some(pid)) if p.contains("command line") => format!("Studio's command-line tool (process {pid})"),
            (Some(p), Some(pid)) => format!("{p} (process {pid})"),
            _ => "another program".into(),
        };
        let since = h
            .since
            .map(|ms| {
                let t = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms);
                format!(" since {}", super::localtime::date_time(t))
            })
            .unwrap_or_default();
        t.push_str(&format!("{} is open for writing in {who}{since}. ", mod_name(m, &h.folder)));
    }
    t.trim_end().to_string()
}

/// The map editor's button: starts the desktop editor on its own Open /
/// New; greyed when it is not beside the launcher (`dirs::editor_exe`) or
/// while it is open.
pub fn map_editor_button(m: &Model) -> Ctrl {
    let open = matches!(m.mods.editor, super::model::EditorRun::Open(_));
    Ctrl::Button {
        id: "map_editor",
        text: "Open map editor (preview)".into(),
        primary: false,
        enabled: m.editor_here && !open,
    }
}

/// The Storage page's rows: ids for Change…, Open and Reset.
pub const LOC_IDS: [(&str, &str, &str); 7] = [
    ("loc_saves_change", "loc_saves_open", "loc_saves_reset"),
    ("loc_settings_change", "loc_settings_open", "loc_settings_reset"),
    ("loc_cache_change", "loc_cache_open", "loc_cache_reset"),
    ("loc_logs_change", "loc_logs_open", "loc_logs_reset"),
    ("loc_screenshots_change", "loc_screenshots_open", "loc_screenshots_reset"),
    ("loc_packs_change", "loc_packs_open", "loc_packs_reset"),
    ("loc_maps_change", "loc_maps_open", "loc_maps_reset"),
];

/// A folder's name on the Storage page.
pub fn loc_name(l: crate::platform::dirs::Location) -> &'static str {
    use crate::platform::dirs::Location as L;
    match l {
        L::Saves => "Saved games",
        L::Settings => "Settings",
        L::Cache => "Cache (assets and shaders)",
        L::Logs => "Logs, crash reports and support files",
        L::Screenshots => "Screenshots",
        L::Packs => "Texture packs",
        L::Maps => "Map mods",
    }
}

/// Settings › Advanced › Storage: portable mode, then every folder with its
/// path, Change…, Open and Reset to default.
fn build_storage(b: &mut Builder, m: &Model) {
    let t = super::draw::theme();
    b.page_title("Storage", Some("Advanced"));
    let p = &m.portable;
    b.toggle("portable", "Portable mode", p.marker);
    if p.marker != p.on && p.changed {
        b.message_with(
            Mark::Info,
            if p.marker { "Portable mode: on after a restart" } else { "Portable mode: off after a restart" },
            "Overbright uses the new folders when it starts again.",
            vec![Ctrl::primary("restart", "Restart now")],
        );
    }
    b.row_note(&if p.on {
        format!("Everything is kept beside the program, in {}.", p.data.as_ref().map_or(String::new(), |d| d.display().to_string()))
    } else if !p.writable {
        "Overbright's folder can't be written to, so portable mode can't be turned on here: move Overbright to a \
         folder you own."
            .into()
    } else {
        "On: settings, saves, caches and logs are kept beside the program (for a portable drive).".into()
    });
    let small = b.fs_min(Face::Body, 15.0, 14.0);
    let row = |b: &mut Builder, label: &str, path: &str, ctrls: Vec<Ctrl>, err: Option<&str>| {
        b.section_a(label);
        let shown = b.fonts.fit(Face::Body, small, path, b.width(), Cut::Middle);
        let (sw, sh) = (b.fonts.width(Face::Body, small, &shown), b.fonts.line_height(Face::Body, small));
        let r = Rect::new(b.x0, b.y, b.width(), sh);
        b.widgets.push(super::ui::Widget {
            id: None,
            rect: r,
            kind: super::ui::Kind::Label,
            texts: vec![super::ui::text_of(Rect::new(b.x0, b.y, sw, sh), shown, path, Face::Body, small, t.ink)],
            enabled: true,
        });
        b.y += sh + b.a(6.0);
        if let Some(e) = err {
            b.status(Mark::Error, e);
        }
        b.buttons(Align::Left, ctrls);
    };
    // Two columns when there is room: the game's folders and the saves and
    // settings at the left, the rest at the right.
    let cols = b.columns(600.0);
    let x1_full = b.x1;
    b.in_column(cols.left);
    let game = m.root.as_ref().map_or("None chosen".to_string(), |r| r.display().to_string());
    row(b, "Game folder", &game, vec![Ctrl::button("change", "Change…"), Ctrl::Button {
        id: "game_open",
        text: words::OPEN.into(),
        primary: false,
        enabled: m.root.is_some(),
    }], None);
    let hud = m.choices.texture_pack.trim();
    row(
        b,
        "HUD mod folder",
        if hud.is_empty() { "None chosen" } else { hud },
        vec![
            Ctrl::button("pack_browse", "Change…"),
            Ctrl::Button { id: "hud_open", text: words::OPEN.into(), primary: false, enabled: !hud.is_empty() },
            Ctrl::Button { id: "pack_forget", text: "Reset to default".into(), primary: false, enabled: !hud.is_empty() },
        ],
        None,
    );
    let mut left_bottom = 0.0;
    for (i, l) in m.locations.iter().enumerate().take(LOC_IDS.len()) {
        if i == 2 {
            left_bottom = b.y;
            if let Some(r) = cols.right {
                b.in_column(r);
            }
        }
        let (c, o, r) = LOC_IDS[i];
        let path = l.path.as_ref().map_or("None (no place for it)".to_string(), |p| p.display().to_string());
        let label = if l.custom { format!("{} · chosen by you", loc_name(l.loc)) } else { loc_name(l.loc).to_string() };
        row(
            b,
            &label,
            &path,
            vec![
                Ctrl::button(c, "Change…"),
                Ctrl::Button { id: o, text: words::OPEN.into(), primary: false, enabled: !l.missing && l.path.is_some() },
                Ctrl::Button { id: r, text: "Reset to default".into(), primary: false, enabled: l.custom },
            ],
            l.missing.then_some("This folder is not there: its drive may be unplugged. Plug it in, or reset it."),
        );
    }
    b.y = b.y.max(left_bottom);
    b.x0 = cols.left.x;
    b.x1 = x1_full;
    if m.copy_busy {
        b.gap(6.0);
        b.status(Mark::Busy, "Copying the files…");
    }
    message(b, m);
    b.default_focus = Some("portable");
}

/// The last start did not reach the game.
fn build_recover(b: &mut Builder, m: &Model) {
    b.page_title("Need for Speed Underground 2", None);
    let w = Issue::StartUnfinished.words();
    let mut ctrls = issue_buttons(&w);
    ctrls.push(Ctrl::button("support", "Save support file…"));
    b.message_with(Mark::Warn, &w.title, &w.text, ctrls);
    b.default_focus = Some("safe");
    support_line(b, m);
    message(b, m);
}

/// The game stopped: what happened, the fix, the support file.
fn build_crash(b: &mut Builder, m: &Model) {
    let f = m.failure.as_ref();
    let kind = f.map_or(ErrorKind::Other, |f| f.kind);
    let during = f.is_some_and(|f| f.during_start);
    let issue = match kind {
        ErrorKind::NoGpu => Some(Issue::NoVulkan),
        ErrorKind::NoWindow => Some(Issue::WindowFailed),
        ErrorKind::Crash if !during => Some(Issue::Crashed { minutes: None }),
        _ => None,
    };
    let (title, text, mut ctrls, first) = match &issue {
        Some(i) => {
            let w = i.words();
            let first = w.buttons[0].0;
            (w.title.clone(), w.text.clone(), issue_buttons(&w), first)
        }
        None => {
            let w = failure_words(kind, during);
            (w.title.to_string(), w.sentence.to_string(), vec![Ctrl::primary(w.fix.0, w.fix.1)], w.fix.0)
        }
    };
    if !ctrls.iter().any(|c| matches!(c, Ctrl::Button { id: "support", .. })) {
        ctrls.push(Ctrl::button("support", "Save support file…"));
    }
    ctrls.push(Ctrl::button("settings", "Settings"));
    b.page_title("Need for Speed Underground 2", None);
    b.message_with(Mark::Error, &title, &text, ctrls);
    b.default_focus = Some(first);
    let where_ = match f.and_then(|f| f.log.as_ref()) {
        Some(l) => format!("The log file: {}", l.display()),
        None => format!("Logs folder: {}", m.logs_dir.display()),
    };
    b.dim(&where_);
    b.gap(8.0);
    let report = f.and_then(|f| f.report.as_ref());
    let mut row = vec![
        Ctrl::button("open_logs", "Open log folder"),
        Ctrl::button("details", if m.failure_details { "Hide details" } else { "Details" }),
    ];
    if report.is_some() {
        row.insert(0, Ctrl::button("copy_report", "Copy for a bug report"));
    }
    b.buttons(Align::Left, row);
    if let Some(r) = report {
        crash_report_block(b, r);
    }
    if m.failure_details {
        b.gap(10.0);
        b.dim(&format!("It ended with {}.", f.map_or("an unknown result", |f| f.how.as_str())));
        let tail: Vec<String> =
            f.map(|f| f.tail.iter().rev().take(TAIL_SHOWN).rev().cloned().collect()).unwrap_or_default();
        if tail.is_empty() {
            b.dim("It printed nothing.");
        } else {
            b.gap(6.0);
            b.panel(&tail);
        }
    }
    support_line(b, m);
    message(b, m);
}

/// The crash report window's part of the failure screen: how the game
/// ended in words, the system's errors for it, the logs of the run.
fn crash_report_block(b: &mut Builder, r: &super::model::CrashReport) {
    b.gap(10.0);
    b.dim(&format!("What happened: the game ran {:.0} s, then {}.", r.secs, r.reason));
    b.gap(6.0);
    if r.asking {
        b.dim("Asking the system for its record of the crash…");
    } else if r.errors.is_empty() {
        b.dim("The system has no record of it that can be read without admin rights.");
    } else {
        b.dim("The system's record of it:");
        b.gap(4.0);
        b.panel(&r.errors.iter().take(TAIL_SHOWN).cloned().collect::<Vec<_>>());
    }
    b.gap(6.0);
    if r.logs.is_empty() {
        b.dim("No log file was written during this run.");
    } else {
        let name = |p: &Path| p.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_string());
        let mut names: Vec<String> = r.logs.iter().rev().take(3).map(|p| name(p)).collect();
        if r.logs.len() > 3 {
            names.push(format!("and {} more", r.logs.len() - 3));
        }
        b.dim(&format!("Logs of this run: {}.", names.join(", ")));
    }
}

/// The support file written: where, and Open folder.
fn support_line(b: &mut Builder, m: &Model) {
    if let Some(p) = &m.support {
        b.gap(10.0);
        b.status(Mark::Ok, &format!("Support file saved: {}", p.display()));
        b.buttons(Align::Left, vec![Ctrl::button("open_support", "Open folder")]);
    }
}

/// The main screen's Mods line: what is on.
/// The prepare step's lines.
pub const PREPARE_WHY: &str = "Overbright can build its shader programs and read your game's city once now, \
so the first drive starts without a loading screen. It takes from a few seconds to about a minute. \
You can skip it: the game then does the same work during its first loads.";

/// The prepare step (`game --prepare`, docs/developing/prepare.md): what it is for, the
/// bar and the step under it while it runs, how it ended; Skip while it
/// runs, Continue after.
fn build_prepare(b: &mut Builder, m: &Model) {
    use super::prepare::State;
    b.page_title("Get shaders ready", Some("Settings"));
    b.body(PREPARE_WHY);
    b.gap(12.0);
    let state = &m.prepare.state;
    let left = m.prepare.started.and_then(|s| state.time_left(s.elapsed()));
    let running = matches!(state, State::Idle | State::Running { .. });
    match state {
        State::Done => b.message_progress(
            Mark::Ok,
            "Shaders ready",
            "The first race starts without stutter.",
            Some((1.0, "100 %".into())),
            vec![Ctrl::primary("continue", "Continue")],
        ),
        State::Failed(_) => b.message_with(
            Mark::Warn,
            "Shaders aren't prepared yet",
            &state.words(),
            vec![Ctrl::primary("continue", "Continue")],
        ),
        _ => b.message_progress(
            Mark::Busy,
            "Getting the shaders ready",
            "You can stop at any time; what is ready so far is kept.",
            Some((state.fraction(), state.progress_words(left))),
            vec![Ctrl::button("skip", "Stop")],
        ),
    }
    message(b, m);
    b.default_focus = Some(if running { "skip" } else { "continue" });
}

pub fn mods_summary(m: &Model) -> String {
    if m.mods.paused {
        return "Off: the game's own textures".into();
    }
    let packs = m.mods.list.iter().filter(|(e, _)| e.on).count() + m.mods.own.iter().filter(|o| o.on).count();
    let hud = m.choices.hud_mod && !m.choices.texture_pack.trim().is_empty();
    let maps = m.mods.maps.iter().filter(|r| r.on).count();
    let mut parts = Vec::new();
    if packs > 0 {
        parts.push(format!("{packs} texture {}", if packs == 1 { "pack" } else { "packs" }));
    }
    if hud {
        parts.push("HUD mod".to_string());
    }
    if maps > 0 {
        parts.push(format!("{maps} map {}", if maps == 1 { "mod" } else { "mods" }));
    }
    if parts.is_empty() { "None: the game's own textures".into() } else { parts.join(" · ") }
}

/// The Mods page's one sentence on what can be added.
pub const MODS_INTRO: &str = "Overbright ships no texture art. Add mods you have: TexMod packages (.tpf), TexWizard \
packs (folder or .zip) and the HD HUD mod. They are copied into Overbright's folder; your files stay unchanged. Mods \
belong to their authors.";

/// The HUD mod section's sentence on what it takes.
pub const HUD_INTRO: &str = "Not set up: the game shows its own HUD and maps. Add HUD mod… takes the downloads.";

/// The copy's choice for the files that differ: replace only when the
/// button says so.
fn copy_choice(id: &str) -> super::transfer::Choice {
    if id.ends_with("_replace") { super::transfer::Choice::Replace } else { super::transfer::Choice::Keep }
}

/// What a copy kept beside or backed up, for its message.
fn kept_words(d: &super::transfer::Done) -> String {
    let mut s = String::new();
    if !d.beside.is_empty() {
        s += &format!(
            "; {} already there kept, the copies beside them",
            super::model::count(d.beside.len(), "different file", "different files")
        );
    }
    if !d.backups.is_empty() {
        s += &format!(
            "; {} kept as .before-copy",
            super::model::count(d.backups.len(), "replaced file", "replaced files")
        );
    }
    s
}

fn mb(bytes: u64) -> String {
    if bytes >= 10_000_000 { format!("{} MB", bytes / 1_000_000) } else { format!("{:.1} MB", bytes as f64 / 1e6) }
}

/// The ids of the Mods page's pack rows and map rows (a row each, in order).
pub const PACK_IDS: [&str; 24] = [
    "pk0", "pk1", "pk2", "pk3", "pk4", "pk5", "pk6", "pk7", "pk8", "pk9", "pk10", "pk11", "pk12", "pk13", "pk14",
    "pk15", "pk16", "pk17", "pk18", "pk19", "pk20", "pk21", "pk22", "pk23",
];
pub const MODPACK_IDS: [&str; 16] = [
    "mk0", "mk1", "mk2", "mk3", "mk4", "mk5", "mk6", "mk7", "mk8", "mk9", "mk10", "mk11", "mk12", "mk13", "mk14", "mk15",
];
pub const MAP_IDS: [&str; 16] = [
    "mp0", "mp1", "mp2", "mp3", "mp4", "mp5", "mp6", "mp7", "mp8", "mp9", "mp10", "mp11", "mp12", "mp13", "mp14", "mp15",
];

/// A texture pack row: Overbright's own packs first, then the imported
/// packages (the loader's order).
#[derive(Clone, Debug, PartialEq)]
pub struct PackLine {
    pub name: String,
    pub detail: String,
    pub on: bool,
    pub usable: bool,
    /// `own:<id>` for an own pack, the package's id otherwise.
    pub key: String,
}

pub fn pack_lines(m: &Model) -> Vec<PackLine> {
    let own = m.mods.own.iter().map(|o| PackLine {
        name: o.name.clone(),
        detail: match &o.refused {
            Some(why) => format!("Can't be used: {why}"),
            None if o.author.is_empty() => format!("{} textures · Overbright pack", o.textures),
            None => format!("{} textures · by {}", o.textures, o.author),
        },
        on: o.on,
        usable: o.refused.is_none(),
        key: format!("own:{}", o.id),
    });
    let pk = m.mods.list.iter().map(|(e, i)| PackLine {
        name: i.name.clone(),
        detail: format!(
            "{} textures · {}{}",
            i.files,
            mb(i.bytes),
            if i.kind.is_empty() { String::new() } else { format!(" · {}", i.kind) }
        ),
        on: e.on,
        usable: true,
        key: e.id.clone(),
    });
    own.chain(pk).collect()
}

/// The words beside the HUD mod's switch.
fn hud_words(m: &Model) -> Option<(Mark, String)> {
    let dir = m.choices.texture_pack.trim();
    if dir.is_empty() {
        return None;
    }
    Some(match &m.hud {
        Some(h) if h.complete => (Mark::Ok, "complete".into()),
        Some(h) if h.is_pack => {
            let mut t = format!("incomplete: {} of {} files", h.present, h.listed);
            if !h.missing.is_empty() {
                t += &format!("; missing {}", h.missing.join(", "));
            }
            if !h.maps {
                t += "; the map calibration is missing";
            }
            (Mark::Warn, t + ". The game shows its own HUD until it is complete.")
        }
        Some(_) => (Mark::Error, "not a HUD mod folder (no hd-ui-pack.list)".into()),
        None => (Mark::Info, dir.to_string()),
    })
}

/// The Mods page: Use mods, the texture packs (own and imported) with
/// their switches, the order only when two packs collide, the HUD mod,
/// the map mods; About mods at the right.
fn build_mods(b: &mut Builder, m: &Model) {
    let mods = &m.mods;
    let busy = mods.busy.is_some();
    b.page_title("Mods", None);
    let cols = b.columns(700.0);
    b.in_column(cols.left);
    b.toggle("use_mods", "Use mods", !mods.paused);
    if mods.paused {
        b.row_note("Off: the game starts with its own textures, HUD and maps. Your list is kept.");
    }
    let live = !mods.paused;
    b.section_a("Texture packs");
    let lines = pack_lines(m);
    if lines.is_empty() {
        b.dim("None added: the game shows its own textures.");
    }
    for (i, l) in lines.iter().enumerate().take(PACK_IDS.len()) {
        b.toggle_sub(PACK_IDS[i], &l.name, &l.detail, l.on && live, live && l.usable && !busy);
    }
    if live && let Some(c) = &mods.conflict {
        b.gap(6.0);
        let ctrls = if !c.same_list {
            Vec::new()
        } else if mods.show_order {
            let sel = mods.sel.min(lines.len().saturating_sub(1));
            let own_n = mods.own.len();
            let (lo, hi) = if sel < own_n { (0, own_n) } else { (own_n, lines.len()) };
            let btn = |id, text: &str, enabled| Ctrl::Button { id, text: text.into(), primary: false, enabled };
            vec![btn("up", "Move up", sel > lo), btn("down", "Move down", sel + 1 < hi)]
        } else {
            vec![Ctrl::button("order", "Change the order")]
        };
        b.message_with(
            Mark::Warn,
            "Two packs change the same textures",
            &if c.same_list {
                format!(
                    "{} and {} both change {} textures. The one higher in the list is used.",
                    c.first, c.second, c.textures
                )
            } else {
                format!(
                    "{} and {} both change {} textures. Overbright packs come before imported packs, so {} is \
                     used there. Turn one off to choose.",
                    c.first, c.second, c.textures, c.first
                )
            },
            ctrls,
        );
    }
    b.gap(10.0);
    b.buttons(Align::Left, vec![Ctrl::Button { id: "add", text: "Add a mod…".into(), primary: false, enabled: !busy }]);
    if let Some(t) = &mods.busy {
        b.gap(6.0);
        b.status(Mark::Busy, t);
    }
    for (mark, line) in &mods.results {
        b.status(*mark, line);
    }
    message(b, m);
    b.section_a("Mod packs");
    let editing = matches!(mods.editor, super::model::EditorRun::Open(_));
    if mods.packs.is_empty() {
        b.dim("None added. A pack (.obpk) holds one mod; the first in the list is the strongest.");
    }
    for (i, r) in mods.packs.iter().enumerate().take(MODPACK_IDS.len()) {
        b.toggle_sub(MODPACK_IDS[i], &r.name, &pack_detail(r), r.on && r.refused.is_none() && live, live && r.refused.is_none());
        if let Some(why) = r.refused.as_ref().filter(|_| r.on || r.new) {
            b.status(Mark::Warn, &format!("{}: not used, {why}", r.name));
        }
    }
    b.gap(6.0);
    let sel = mods.pack_sel.as_ref().and_then(|s| mods.packs.iter().position(|r| &r.id == s));
    b.buttons(
        Align::Left,
        vec![
            Ctrl::Button { id: "pack_up", text: "Move up".into(), primary: false, enabled: live && sel.is_some_and(|i| i > 0) },
            Ctrl::Button {
                id: "pack_down",
                text: "Move down".into(),
                primary: false,
                enabled: live && sel.is_some_and(|i| i + 1 < mods.packs.len()),
            },
            Ctrl::Button { id: "pack_import", text: "Import…".into(), primary: false, enabled: !editing },
            Ctrl::Button {
                id: "map_export_pack",
                text: "Export a map mod…".into(),
                primary: false,
                enabled: !mods.maps.is_empty() && !editing,
            },
        ],
    );
    let left_bottom = b.y;
    // The HUD and the map mods: the right column (under the left one when
    // the window is narrow).
    match cols.right {
        Some(r) => b.in_column(r),
        None => b.gap(8.0),
    }
    b.section_a("HUD");
    match hud_words(m) {
        Some((mark, words)) => {
            b.toggle_sub("hud", "HD HUD and maps", &words, m.choices.hud_mod && live, live);
            if mark == Mark::Warn || mark == Mark::Error {
                b.status(mark, &words);
            }
        }
        None => b.dim(HUD_INTRO),
    }
    if let Some(t) = &mods.hud_busy {
        b.status(Mark::Busy, t);
    }
    for (mark, line) in &mods.hud_results {
        b.status(*mark, line);
    }
    let hud_busy = mods.hud_busy.is_some();
    let dir = m.choices.texture_pack.trim();
    b.gap(6.0);
    b.buttons(
        Align::Left,
        vec![
            Ctrl::Button { id: "hud_add", text: "Add HUD mod…".into(), primary: false, enabled: !hud_busy },
            Ctrl::button("pack_browse", "Choose folder…"),
            Ctrl::Button { id: "pack_forget", text: "Forget folder".into(), primary: false, enabled: !dir.is_empty() },
        ],
    );
    b.section_a("Map mods");
    if mods.maps.is_empty() {
        b.dim("None added: the game's own city. Make one in the map editor.");
    }
    let editing = matches!(mods.editor, super::model::EditorRun::Open(_));
    for (i, r) in mods.maps.iter().enumerate().take(MAP_IDS.len()) {
        let detail = match m.studio_seen.held_on(&r.folder) {
            Some(h) if h.by_studio() => format!("{} · being edited in Studio", map_detail(r)),
            Some(_) => format!("{} · being edited", map_detail(r)),
            None => map_detail(r),
        };
        b.toggle_sub(MAP_IDS[i], &r.name, &detail, r.on && live, live && !editing);
    }
    b.gap(6.0);
    let any = !mods.maps.is_empty();
    b.buttons(
        Align::Left,
        vec![
            Ctrl::Button {
                id: "map_open",
                text: "Open in map editor".into(),
                primary: false,
                enabled: any && !editing && m.editor_here,
            },
            Ctrl::Button { id: "map_import", text: "Import a map mod…".into(), primary: false, enabled: !editing },
            Ctrl::Button { id: "map_export", text: "Export…".into(), primary: false, enabled: any && !editing },
        ],
    );
    editor_status(b, m);
    b.gap(14.0);
    b.dim(ABOUT_MODS);
    b.y = left_bottom.max(b.y);
    b.default_focus = Some(if lines.is_empty() { "add" } else { PACK_IDS[0] });
}

/// The map editor's state under its buttons (Mods and Dev pages): open on
/// which mod, why it failed, or what it is.
fn editor_status(b: &mut Builder, m: &Model) {
    let mods = &m.mods;
    match &mods.editor {
        super::model::EditorRun::Open(f) => {
            let name = f.as_ref().and_then(|f| mods.maps.iter().find(|r| &r.folder == f)).map(|r| r.name.as_str());
            b.status(
                Mark::Busy,
                &match name {
                    Some(n) => format!("The map editor is open on {n}. This page waits until it closes."),
                    None => "The map editor is open. This page waits until it closes.".into(),
                },
            );
        }
        super::model::EditorRun::Failed(e) => b.status(Mark::Error, &format!("The map editor: {e}")),
        super::model::EditorRun::Closed => b.dim("The map editor is its own window. Races are tested from it."),
    }
}

/// A mod pack's line under its name: version, its place in the order, what it clashes with.
pub fn pack_detail(r: &super::model::PackRow) -> String {
    let mut parts = vec![format!("version {}", r.version)];
    if r.new {
        parts.push("new, off".into());
    }
    if let Some(c) = &r.clash {
        parts.push(c.clone());
    }
    parts.join(" · ")
}

/// A map mod's line under its name: its races, prop edits and the
/// textures it changes.
pub fn map_detail(r: &super::model::MapRow) -> String {
    use super::model::count;
    let parts: Vec<String> = [(r.races, "race", "races"), (r.props, "prop edit", "prop edits"), (r.textures, "texture", "textures")]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many)| count(n, one, many))
        .collect();
    if parts.is_empty() { "Empty".into() } else { parts.join(" · ") }
}

/// The Mods page's side note.
pub const ABOUT_MODS: &str = "Overbright copies each mod into its own folder; your game folder and your downloads are \
never changed. Add a mod takes Overbright packs, TexMod packs (.tpf), TexWizard packs and map mods. Mods belong to their \
authors.";

/// One line of text shortened in the middle (a path).
fn line(b: &mut Builder, s: &str) {
    let size = b.m.body();
    let t = b.fonts.fit(Face::Regular, size, s, b.width(), Cut::Middle);
    let w = b.fonts.width(Face::Regular, size, &t);
    let h = b.fonts.line_height(Face::Regular, size);
    let r = Rect::new(b.x0, b.y, b.width(), h);
    b.widgets.push(super::ui::Widget {
        id: None,
        rect: r,
        kind: super::ui::Kind::Label,
        texts: vec![super::ui::text_of(Rect::new(b.x0, b.y, w, h), t, s, Face::Regular, size, theme::TEXT)],
        enabled: true,
    });
    b.y += h;
}

fn message(b: &mut Builder, m: &Model) {
    if let Some((mark, text)) = &m.message {
        b.gap(10.0);
        b.status(*mark, text);
    }
}

impl<'e> App<'e> {
    pub fn new(env: &'e dyn Env, model: Model, screen: Screen, size: (u32, u32), dpi: f32, fonts: &Fonts) -> App<'e> {
        let mut a = App {
            env,
            model,
            screen,
            st: UiState::default(),
            size,
            dpi,
            ui: Ui::default(),
            back: Screen::Main,
            motion: Default::default(),
            support_job: None,
            crash_job: None,
            editor_job: None,
            studio_job: None,
            studio_looked: None,
            keys_after: None,
            copy_job: None,
            game_data_job: None,
            saves_rx: None,
            saves_ctx: None,
        };
        a.rebuild(fonts);
        a
    }

    pub fn rebuild(&mut self, fonts: &Fonts) {
        let (m, s) = (&self.model, self.screen);
        // The state as it was: a second pass (the list grown) starts from
        // it, not from what the first pass's shorter list scrolled to.
        let before = matches!(s, Screen::Browse | Screen::Check).then(|| self.st.clone());
        let scale = f32::from(m.prefs.text) / 100.0;
        self.ui = super::layout::build_fit_scaled(
            fonts,
            self.size.0,
            self.size.1,
            self.dpi,
            // A long page scrolls instead of shrinking.
            !s.document() && s != Screen::Storage,
            scale,
            &mut self.st,
            |b| build(b, m, s),
        );
        // The folder browser's and the check's details list take the
        // room the window leaves.
        let grows = match s {
            Screen::Browse => Some("dirs"),
            Screen::Check => Some("details_list"),
            _ => None,
        };
        if let Some(list) = grows
            && let Some(super::ui::Kind::List { row_h, .. }) = self.ui.get(list).map(|w| &w.kind)
        {
            let full = super::ui::Metrics::for_size(self.size.0, self.size.1, self.dpi).u;
            let grow = (self.ui.slack / row_h).floor() as usize;
            if grow > 0 && self.ui.metrics.is_some_and(|mm| (mm.u - full).abs() < 1e-3) {
                if let Some(st) = before {
                    self.st = st;
                }
                self.ui = build_fit(fonts, self.size.0, self.size.1, self.dpi, true, &mut self.st, |b| {
                    b.grow = grow;
                    build(b, m, s)
                });
            }
        }
    }

    pub fn resize(&mut self, size: (u32, u32), dpi: f32, fonts: &Fonts) {
        self.size = size;
        self.dpi = dpi;
        self.rebuild(fonts);
    }

    /// The prepare step asked for (the settings' button, or offered by
    /// itself): its screen, the run started by the window.
    pub fn start_prepare(&mut self) {
        let prepared_graphics = self.model.prepare.prepared_graphics.clone();
        self.model.prepare = super::model::Prepare {
            state: Default::default(),
            wanted: true,
            stop: false,
            offered: true,
            background: false,
            prepared_graphics,
            started: None,
            done_at: None,
        };
        self.go(Screen::Prepare);
    }

    /// The prepare step started by itself behind the main screen
    /// ([`super::model::Prepare::background`]).
    pub fn start_prepare_background(&mut self) {
        let prepared_graphics = self.model.prepare.prepared_graphics.clone();
        self.model.prepare = super::model::Prepare {
            state: Default::default(),
            wanted: true,
            stop: false,
            offered: true,
            background: true,
            prepared_graphics,
            started: None,
            done_at: None,
        };
    }

    /// The prepare step offered by itself, once a session: the hardware
    /// check says this build, adapter and driver (and the settings'
    /// programs, the stamp's key) have no prepare run yet, or the Graphics
    /// row saved a level other than the one the check found prepared; the
    /// game folder is in use and the main screen is up.
    pub fn offer_prepare(&mut self) -> bool {
        let prepared = match &self.model.probe {
            ProbeState::Ready(p) if p.ok() => p.prepared,
            _ => None,
        };
        if prepared == Some(true) && self.model.prepare.prepared_graphics.is_none() {
            self.model.prepare.prepared_graphics = Some(self.model.saved.graphics.clone());
        }
        let changed = self.model.prepare.prepared_graphics.as_ref().is_some_and(|g| *g != self.model.saved.graphics);
        let unprepared = prepared == Some(false) || (prepared == Some(true) && changed);
        if unprepared && !self.model.prepare.offered && self.screen == Screen::Main && self.model.root.is_some() {
            self.start_prepare_background();
            return true;
        }
        false
    }

    /// The own copy offered as the next step: after the game check of a first run, or at a start without a
    /// copy (unless "Not now" was chosen before). Not when a copy is there (whole or to repair) or a run goes on.
    pub fn offer_game_data(&mut self) -> bool {
        use super::game_data::Status;
        let g = &self.model.game_data;
        if self.model.root.is_none() || g.running() || matches!(g.status, Status::Ready { .. } | Status::Damaged(_)) {
            return false;
        }
        self.open_game_data(false);
        true
    }

    /// The Game data screen (`from_settings`: from Settings › Advanced, Back returns there).
    pub fn open_game_data(&mut self, from_settings: bool) {
        let root = self.model.root.clone();
        let g = &mut self.model.game_data;
        g.from_settings = from_settings;
        if !g.running() {
            g.state = super::game_data::GameData::state_for(&g.status);
            g.stopping = false;
        }
        if g.size == 0
            && let Some(r) = root
        {
            g.size = self.env.game_data_size(&r);
        }
        self.go(Screen::GameData);
    }

    fn game_data_leave(&mut self) {
        if self.model.game_data.from_settings {
            self.go(Screen::Advanced);
            self.st.focus = Some("game_data");
        } else {
            self.go(Screen::Main);
        }
    }

    /// Starts `kind` in the background (one run at a time).
    fn game_data_run(&mut self, kind: super::game_data::Kind) {
        use super::game_data::{Kind, State};
        let Some(root) = self.model.root.clone() else {
            self.model.message = Some((Mark::Warn, "Choose the game's folder first.".into()));
            return;
        };
        if self.game_data_job.is_some() {
            return;
        }
        let g = &mut self.model.game_data;
        g.last = kind;
        g.stopping = false;
        g.state = State::Running {
            fraction: 0.0,
            words: if kind == Kind::Verify { "Checking the copy" } else { "Reading your game files" }.into(),
        };
        log::info!("game data: {kind:?} started");
        self.game_data_job = Some(self.env.game_data_start(kind, &root));
        self.st.focus = None;
    }

    /// A run's messages applied. True when the screen changed.
    fn poll_game_data(&mut self) -> bool {
        use super::game_data::{End, GameData, Msg, State, Status};
        let Some(job) = self.game_data_job.as_mut() else { return false };
        let msgs = job.poll();
        if msgs.is_empty() {
            return false;
        }
        let g = &mut self.model.game_data;
        for m in msgs {
            match m {
                Msg::Progress { fraction, words } => {
                    if g.running() {
                        g.state = State::Running { fraction, words };
                    }
                }
                Msg::End(end) => {
                    self.game_data_job = None;
                    g.stopping = false;
                    g.state = match end {
                        End::Ready { status, differing } => {
                            g.status = status;
                            match &g.status {
                                Status::Ready { edition, known } => {
                                    State::Done { edition: edition.clone(), known: *known, differing }
                                }
                                other => GameData::state_for(other),
                            }
                        }
                        End::Stopped { done, of } => {
                            g.status = Status::Partial;
                            State::Stopped { done, of }
                        }
                        End::Damaged(n) => {
                            g.status = Status::Damaged(n);
                            State::Damaged(n)
                        }
                        End::NoSpace { need, free, place } => State::NoSpace { need, free, place },
                        End::Failed(e) => State::Failed(e),
                    };
                    if self.screen == Screen::GameData {
                        self.st.focus = None;
                    }
                    break;
                }
            }
        }
        true
    }

    /// A run goes on (the window wakes for its messages).
    pub fn game_data_busy(&self) -> bool {
        self.game_data_job.is_some()
    }

    /// The launcher closes: a run still going is asked to stop (the next start goes on from it).
    pub fn finish_game_data(&mut self) {
        if let Some(j) = self.game_data_job.take() {
            log::info!("game data: stopped with the launcher");
            j.finish(std::time::Duration::from_secs(3));
        }
    }

    pub fn go(&mut self, s: Screen) {
        if let (Some(a), Some(b)) = (self.screen.page(), s.page())
            && a != b
        {
            self.motion.page_changed(if b > a { 1 } else { -1 });
        }
        self.screen = s;
        if s.sees_studio() {
            self.look_at_studio();
        }
        let pointer = self.st.pointer;
        self.st = UiState { pointer, ..UiState::default() };
        self.model.message = None;
    }

    /// One input: the events it makes, acted on; the screen rebuilt.
    pub fn input(&mut self, input: &Input, fonts: &Fonts) -> Outcome {
        if matches!(input, Input::Key(_) | Input::Text(_))
            && self.keys_after.is_some_and(|t| std::time::Instant::now() < t)
        {
            return Outcome::Stay;
        }
        match input {
            // The mouse's back button: the browser goes up a folder;
            // elsewhere it is Back (Escape).
            Input::MouseBack => {
                let o = if self.screen == Screen::Browse {
                    self.event(&Event::Activate("up"))
                } else {
                    self.event(&Event::Back)
                };
                self.rebuild(fonts);
                return o;
            }
            Input::Drop(paths) => {
                let o = self.drop_paths(paths);
                self.rebuild(fonts);
                return o;
            }
            _ => {}
        }
        // A document scrolls with the keys (its one control is Back).
        if self.screen.document()
            && let Input::Key(k @ (Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End)) = input
        {
            let m = self.ui.metrics.unwrap_or(super::ui::Metrics::for_size(self.size.0, self.size.1, self.dpi));
            let max = (self.ui.content_h - self.ui.view.h).max(0.0);
            let page = (self.ui.view.h - m.s(60.0)).max(m.s(40.0));
            self.st.scroll = match k {
                Key::Up => self.st.scroll - m.s(40.0),
                Key::Down => self.st.scroll + m.s(40.0),
                Key::PageUp => self.st.scroll - page,
                Key::PageDown => self.st.scroll + page,
                Key::Home => 0.0,
                _ => max,
            }
            .clamp(0.0, max);
            return Outcome::Stay;
        }
        // The pages (LB / RB, Q / E) and the page actions (X, Y); a click
        // on a legend does what its button does.
        let input = &match input {
            Input::MouseDown(x, y) => match self.legend_at(*x, *y) {
                Some(k) => Input::Key(k),
                None => input.clone(),
            },
            _ => input.clone(),
        };
        if self.screen == Screen::Saves
            && let Input::Key(k) = input
            && let Some(o) = self.saves_key(*k)
        {
            self.rebuild(fonts);
            return o;
        }
        if let Input::Key(k @ (Key::PagePrev | Key::PageNext | Key::ActionX | Key::ActionY)) = input {
            let o = self.page_key(*k);
            self.rebuild(fonts);
            return o;
        }
        let events = handle(&self.ui, &mut self.st, input);
        // The Mods page's chosen pack and map mod follow the focus.
        if self.screen == Screen::Mods
            && let Some(f) = self.ui.focused(&self.st)
        {
            if let Some(i) = PACK_IDS.iter().position(|p| *p == f) {
                self.model.mods.sel = i;
            } else if let Some(i) = MAP_IDS.iter().position(|p| *p == f) {
                self.model.mods.map_sel = self.model.mods.maps.get(i).map(|r| r.folder.clone());
            } else if let Some(i) = MODPACK_IDS.iter().position(|p| *p == f) {
                self.model.mods.pack_sel = self.model.mods.packs.get(i).map(|r| r.id.clone());
            }
        }
        let mut out = Outcome::Stay;
        for e in &events {
            let o = self.event(e);
            if o != Outcome::Stay {
                out = o;
            }
        }
        self.rebuild(fonts);
        out
    }

    /// The key a click on a legend stands for.
    fn legend_at(&self, x: f32, y: f32) -> Option<Key> {
        use super::legends::Btn;
        let w = self.ui.widgets.iter().find(|w| matches!(w.kind, super::ui::Kind::Legends { .. }))?;
        let super::ui::Kind::Legends { actions } = &w.kind else { return None };
        let u = self.ui.metrics.map_or(1.0, |m| m.u);
        let fonts = super::font::Fonts::new();
        Some(match super::legends::hit(&fonts, w.rect, self.st.input, actions, u, x, y)? {
            Btn::Confirm => Key::Enter,
            Btn::Back => Key::Escape,
            Btn::X => Key::ActionX,
            Btn::Y => Key::ActionY,
            Btn::Pages => Key::PageNext,
            // The roles only Studio's legends show.
            Btn::Move | Btn::Change | Btn::Area | Btn::Start | Btn::LastPlace | Btn::Sticks | Btn::DeckGrips => {
                return None;
            }
        })
    }

    /// LB / RB and the page actions.
    fn page_key(&mut self, k: Key) -> Outcome {
        if self.model.ask.is_some() {
            return Outcome::Stay;
        }
        match (k, self.screen.page()) {
            (Key::PagePrev, Some(p)) if !matches!(self.screen, Screen::Licence | Screen::Notices) => {
                self.go_page((p + PAGES.len() - 1) % PAGES.len())
            }
            (Key::PageNext, Some(p)) if !matches!(self.screen, Screen::Licence | Screen::Notices) => {
                self.go_page((p + 1) % PAGES.len())
            }
            (Key::ActionY, _) if self.screen == Screen::Settings => self.model.ask = Some(Dialog::Reset),
            (Key::ActionY, _) if self.screen == Screen::Mods => return self.event(&Event::Activate("add")),
            (Key::ActionY, _) if self.screen == Screen::Check && self.ui.get("details").is_some() => {
                return self.event(&Event::Activate("details"));
            }
            (Key::ActionX, _) if self.screen == Screen::Browse => return self.event(&Event::Activate("use")),
            (Key::ActionX, _)
                if self.screen == Screen::Mods
                    && self.ui.focused(&self.st).is_some_and(|f| PACK_IDS.contains(&f)) =>
            {
                let f = self.ui.focused(&self.st).unwrap_or("pk0");
                self.model.mods.sel = PACK_IDS.iter().position(|p| *p == f).unwrap_or(0);
                return self.event(&Event::Activate("remove"));
            }
            // Space (X) on a switch or a box turns it; on a button presses it.
            (Key::ActionX, _) => {
                if let Some(w) = self.ui.focused(&self.st).and_then(|id| self.ui.get(id)) {
                    let id = w.id.unwrap_or("");
                    match w.kind {
                        super::ui::Kind::Toggle { .. } | super::ui::Kind::Checkbox { .. } => {
                            return self.event(&Event::Toggle(id));
                        }
                        super::ui::Kind::Button { .. } | super::ui::Kind::Play | super::ui::Kind::SumRow { .. } => {
                            return self.event(&Event::Activate(id));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn step<T: PartialEq + Clone>(list: &[T], v: &T, d: i32) -> T {
        let n = list.len() as i32;
        match list.iter().position(|x| x == v) {
            Some(i) => list[((i as i32 + d).rem_euclid(n)) as usize].clone(),
            None => list[if d > 0 { 0 } else { (n - 1) as usize }].clone(),
        }
    }

    /// Choosing a folder for `purpose`, starting at `start`: the system's
    /// folder dialog (opened by the window on a worker where it would block
    /// the window, else at once), and Overbright's own browser when there
    /// is none, the last input came from a gamepad (the system dialogs
    /// are not made for one) or the launcher runs in gamescope (none
    /// there). One dialog at a time.
    pub fn choose_folder(&mut self, purpose: BrowseFor, start: Option<PathBuf>) {
        if self.model.dialog.is_some() {
            return;
        }
        if self.model.pad_input || self.model.gamescope {
            self.browse(purpose, start);
            return;
        }
        let title = dialog_title(purpose);
        if self.env.background_dialog() {
            self.model.dialog = Some(DialogAsk { purpose, title: title.into(), start, open: false });
            self.model.message = Some((Mark::Info, "Choose the folder in the dialog that opened.".into()));
        } else {
            let r = self.env.folder_dialog(title, start.as_deref());
            // The key that closed the dialog (Esc, Enter) may reach the
            // window as well: keys are let through again shortly after.
            if r != Picked::NoDialog {
                self.keys_after = Some(std::time::Instant::now() + std::time::Duration::from_millis(300));
            }
            self.dialog_done(purpose, start, r);
        }
    }

    /// The system dialog's answer: the folder taken for `purpose`; a
    /// cancel changes nothing; no dialog: Overbright's own browser.
    pub fn dialog_done(&mut self, purpose: BrowseFor, start: Option<PathBuf>, r: Picked) {
        self.model.dialog = None;
        if self.model.message.as_ref().is_some_and(|(m, _)| *m == Mark::Info) {
            self.model.message = None;
        }
        match r {
            Picked::Folder(d) => {
                self.model.browse.purpose = purpose;
                self.use_folder(d);
            }
            Picked::Cancelled => {}
            Picked::NoDialog => self.browse(purpose, start),
        }
    }

    /// Opens the folder browser for `purpose` at `dir` (the home folder
    /// when None or unreadable).
    pub fn browse(&mut self, purpose: BrowseFor, dir: Option<PathBuf>) {
        if self.screen != Screen::Browse {
            self.back = self.screen;
        }
        self.go(Screen::Browse);
        self.model.browse.purpose = purpose;
        self.model.browse.input.clear();
        // The places: the system's, then the game folder and the
        // texture-pack folder when chosen.
        let mut places = self.env.places();
        if let Some(r) = &self.model.root {
            places.insert(1.min(places.len()), ("Game folder".into(), r.clone()));
        }
        let pack = self.model.choices.texture_pack.trim();
        if !pack.is_empty() {
            places.insert(2.min(places.len()), ("Texture pack".into(), PathBuf::from(pack)));
        }
        places.truncate(PLACE_IDS.len());
        self.model.browse.places = places;
        let dir = dir.filter(|d| self.env.list_dirs(d).is_ok()).or_else(|| self.env.home()).unwrap_or_default();
        self.open_dir(dir);
    }

    fn open_dir(&mut self, dir: PathBuf) {
        let b = &mut self.model.browse;
        match self.env.list_dirs(&dir) {
            Ok(mut entries) => {
                if b.purpose == BrowseFor::Packages {
                    // A TexWizard pack folder is listed once, as a package.
                    let packages = self.env.list_packages(&dir);
                    entries.retain(|e| !packages.iter().any(|p| p.path == e.path));
                    entries.extend(packages);
                }
                if matches!(b.purpose, BrowseFor::HudMod | BrowseFor::MapMod | BrowseFor::Saves) {
                    entries.extend(self.env.list_archives(&dir));
                }
                b.entries = entries;
                b.dir = dir;
                b.sel = 0;
                b.error = None;
                self.st.set_first("dirs", 0);
                self.st.set_last_sel("dirs", 0);
                self.st.clicked = None;
                self.st.hover_row = None;
                self.st.wheel_rows = 0.0;
            }
            Err(e) => b.error = Some(format!("That folder cannot be read: {e}")),
        }
    }

    /// The browser's list row `i` (".." first when there is a parent).
    fn browse_row(&self, i: usize) -> Option<PathBuf> {
        let b = &self.model.browse;
        let up = !b.dir.as_os_str().is_empty();
        if up && i == 0 {
            return Some(self.env.parent(&b.dir).unwrap_or_else(|| b.dir.clone()));
        }
        b.entries.get(i - up as usize).map(|e: &DirEntry| e.path.clone())
    }

    /// The browser's folder taken for its purpose.
    pub(super) fn use_folder(&mut self, dir: PathBuf) {
        match self.model.browse.purpose {
            BrowseFor::Game => {
                self.model.check = Some(self.env.check(&dir));
                self.back = Screen::FirstRun;
                self.go(Screen::Check);
            }
            BrowseFor::TexturePack => {
                self.model.choices.texture_pack = dir.display().to_string();
                self.model.pack_status = self.env.set_pack(&self.model.choices.texture_pack);
                self.model.hud = self.env.hud(&self.model.choices.texture_pack);
                self.go(Screen::Mods);
            }
            BrowseFor::HudMod => self.hud_pick(dir),
            BrowseFor::Location => {
                self.go(Screen::Storage);
                if let Some(loc) = self.model.pick_loc.take() {
                    let from = self.model.locations.iter().find(|r| r.loc == loc).and_then(|r| r.path.clone());
                    let base = if self.model.portable.on { self.model.portable.program.clone() } else { None };
                    self.model.copy_plan = from.map(|f| super::transfer::location_plan(loc, &f, &dir, base.as_deref()));
                    self.model.ask = Some(Dialog::MoveLocation(loc, dir));
                }
            }
            BrowseFor::MapMod => {
                self.go(Screen::Mods);
                self.import_map(&dir);
            }
            BrowseFor::MapExport => {
                self.go(Screen::Mods);
                if let Some(f) = self.model.mods.map_sel.clone().or_else(|| self.model.mods.maps.first().map(|r| r.folder.clone())) {
                    // Textures from the original game's pixels: the question first.
                    match self.env.map_derived(&f) {
                        0 => self.export_map(&f, &dir, false),
                        n => self.model.ask = Some(Dialog::MapExportDerived(f, dir, n)),
                    }
                }
            }
            BrowseFor::Packages if self.env.mod_kind(&dir).is_some() => {
                self.go(Screen::Mods);
                match self.env.mod_kind(&dir) {
                    Some(super::model::ModKind::Map) => self.import_map(&dir),
                    _ => self.install_own(&dir),
                }
            }
            BrowseFor::Packages => {
                // A folder of loose TexMod files is one package; else every
                // package file in it.
                let paths = if self.env.list_dirs(&dir).is_ok() {
                    super::model::packages_here(&dir, self.env.list_packages(&dir))
                } else {
                    Vec::new()
                };
                if paths.is_empty() {
                    self.model.message =
                        Some((Mark::Info, "No texture packages (TexMod .tpf, TexWizard packs) in this folder.".into()));
                } else {
                    self.go(Screen::Mods);
                    self.queue_import(paths);
                }
            }
            BrowseFor::Saves | BrowseFor::SavesExport => {
                let purpose = self.model.browse.purpose;
                self.saves_folder(purpose, dir);
            }
        }
    }

    /// A browser row is a package file (listed by [`Env::list_packages`]).
    fn is_package_row(&self, p: &Path) -> bool {
        self.model
            .browse
            .entries
            .iter()
            .any(|e| e.path == p && (e.note.starts_with("TexMod package") || e.note.starts_with("TexWizard pack")))
    }

    /// A HUD mod download chosen: added to the parts found; both known:
    /// the conversion is queued (run at once without a worker).
    pub fn hud_pick(&mut self, path: PathBuf) {
        self.go(Screen::Mods);
        let m = &mut self.model.mods;
        m.hud_results.clear();
        self.env.hud_add(&mut m.hud_parts, &path);
        if m.hud_parts.complete() {
            if self.env.background_import() {
                m.hud_busy = Some("Starting the conversion…".into());
                m.hud_queued = true;
            } else {
                let out = self.env.hud_convert(&m.hud_parts, &|_| {});
                self.finish_hud(out);
            }
            return;
        }
        let notes = std::mem::take(&mut m.hud_parts.notes);
        m.hud_results = notes.into_iter().map(|t| (Mark::Error, t)).collect();
        if let Some(t) = crate::hud_mod::waiting_line(&m.hud_parts) {
            m.hud_results.push((Mark::Info, t));
            self.st.focus = Some("hud_add");
        }
    }

    /// A HUD mod conversion's end: its lines; a pack written becomes the
    /// HUD mod folder, checked, and the switch is offered (focused).
    pub fn finish_hud(&mut self, out: super::model::HudOutcome) {
        let m = &mut self.model.mods;
        m.hud_busy = None;
        m.hud_queued = false;
        m.hud_parts = Default::default();
        m.hud_results = out.lines;
        if let Some(dir) = out.dir {
            self.model.choices.texture_pack = dir.display().to_string();
            self.model.pack_status = self.env.set_pack(&self.model.choices.texture_pack);
            self.model.hud = self.env.hud(&self.model.choices.texture_pack);
            self.st.focus = Some("hud");
        }
    }

    /// Queues an import; without a worker (the fake data) it runs at once.
    pub fn queue_import(&mut self, paths: Vec<PathBuf>) {
        self.model.mods.results.clear();
        self.model.mods.confirm = None;
        if self.env.background_import() {
            self.model.mods.busy = Some("Starting the import…".into());
            self.model.mods.queued.extend(paths);
        } else {
            let out = self.env.import(&paths, self.model.root.as_deref(), &|_| {});
            self.finish_import(out);
        }
    }

    /// An Overbright pack folder installed (checked, copied).
    fn install_own(&mut self, dir: &Path) {
        let name = dir.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        self.model.mods.results = vec![match self.env.install_own(dir) {
            Ok(id) => (Mark::Ok, format!("{name}: added as an Overbright pack ({id})")),
            Err(e) => (Mark::Error, format!("{name}: not added: {e}")),
        }];
        self.refresh_mods();
    }

    /// A mod pack's switch.
    fn toggle_mod_pack(&mut self, i: usize) {
        let Some(r) = self.model.mods.packs.get(i).cloned() else { return };
        self.model.mods.pack_sel = Some(r.id.clone());
        self.model.mods.results = match self.env.set_mod_pack(&r.id, !r.on) {
            Ok(also) if !also.is_empty() => {
                vec![(Mark::Info, format!("{} turned on too, below {}: {} needs {}", also.join(", "), r.name, r.name, also.join(", ")))]
            }
            Ok(_) => Vec::new(),
            Err(e) => vec![(Mark::Error, format!("{}: {e}", r.name))],
        };
        self.refresh_mods();
    }

    /// Moves the chosen mod pack one place; the page keeps it chosen.
    fn move_mod_pack(&mut self, by: isize) {
        let Some(id) = self.model.mods.pack_sel.clone() else { return };
        if let Err(e) = self.env.move_pack(&id, by) {
            self.model.mods.results = vec![(Mark::Error, format!("{id}: {e}"))];
        }
        self.refresh_mods();
    }

    /// A map mod imported.
    fn import_map(&mut self, from: &Path) {
        let name = from.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        if from.extension().is_some_and(|e| e.eq_ignore_ascii_case("obpk")) {
            self.model.mods.results = vec![match self.env.import_pack(from) {
                Ok(id) => (Mark::Ok, format!("{name}: mod pack added ({id}); turn it on below")),
                Err(e) => (Mark::Error, format!("{name}: not added: {e}")),
            }];
            self.refresh_mods();
            return;
        }
        self.model.mods.results = vec![match self.env.import_map(from) {
            Ok(f) => (Mark::Ok, format!("{name}: map mod added ({f}); turn it on below")),
            Err(e) => (Mark::Error, format!("{name}: not a map mod Overbright can read: {e}")),
        }];
        self.refresh_mods();
    }

    /// The Mods page's lists read again, and the collision check.
    pub fn refresh_mods(&mut self) {
        let m = &mut self.model.mods;
        m.list = self.env.mods();
        m.own = self.env.own_packs();
        m.maps = self.env.maps();
        m.packs = self.env.packs();
        self.check_conflict();
        self.look_at_studio();
    }

    /// Reads what is seen of Studio again (its locks on the map mods, the
    /// last mod opened; read only, nothing is started). True when it changed.
    pub fn look_at_studio(&mut self) -> bool {
        let folders: Vec<String> = self.model.mods.maps.iter().map(|r| r.folder.clone()).collect();
        let seen = self.env.studio_seen(&folders);
        self.studio_looked = Some(std::time::Instant::now());
        let changed = seen != self.model.studio_seen;
        self.model.studio_seen = seen;
        changed
    }

    /// When the Dev or Mods page reads Studio's state again (None: another
    /// page; nothing is read there).
    pub fn studio_wake(&self) -> Option<std::time::Instant> {
        self.screen.sees_studio().then(|| self.studio_looked.map_or_else(std::time::Instant::now, |t| t + STUDIO_LOOK))
    }

    fn check_conflict(&mut self) {
        let m = &mut self.model.mods;
        m.conflict = self.env.conflict(&m.own, &m.entries());
        if m.conflict.is_none() {
            m.show_order = false;
        }
    }

    /// "Use mods" turned off (every mod off for the game, what was on set
    /// aside) or on (given back).
    /// Export the map mod `f` into `dir` (`derived_ok`: the player said yes
    /// to its textures from the original game's pixels).
    fn export_map(&mut self, f: &str, dir: &Path, derived_ok: bool) {
        let packed = self.model.mods.export_pack;
        let r = if packed { self.env.export_pack(f, dir, derived_ok) } else { self.env.export_map(f, dir, derived_ok) };
        self.model.mods.results = vec![match r {
            Ok(p) => (Mark::Ok, format!("Exported: {}", p.display())),
            Err(e) => (Mark::Error, format!("{f}: not exported: {e}")),
        }];
    }

    fn toggle_mods_in_use(&mut self) {
        use super::model::Paused;
        let m = &mut self.model;
        if !m.mods.paused {
            let p = Paused {
                own: m.mods.own.iter().map(|o| (o.id.clone(), o.on)).collect(),
                packages: m.mods.list.iter().map(|(e, _)| (e.id.clone(), e.on)).collect(),
                maps: m.mods.maps.iter().filter(|r| r.on).map(|r| r.folder.clone()).collect(),
                packs: m.mods.packs.iter().filter(|r| r.on).map(|r| r.id.clone()).collect(),
                hud: m.choices.hud_mod,
            };
            if let Err(e) = self.env.set_paused(Some(&p)) {
                m.mods.results = vec![(Mark::Error, format!("Mods could not be turned off: {e}"))];
                return;
            }
            m.set_aside = Some(p);
            m.mods.own.iter_mut().for_each(|o| o.on = false);
            m.mods.list.iter_mut().for_each(|(e, _)| e.on = false);
            m.choices.hud_mod = false;
            m.mods.paused = true;
            for r in m.mods.maps.iter_mut().filter(|r| r.on) {
                r.on = false;
                let _ = self.env.set_map(&r.folder, false);
            }
            for r in m.mods.packs.iter_mut().filter(|r| r.on) {
                r.on = false;
                let _ = self.env.set_mod_pack(&r.id, false);
            }
        } else {
            let p = self.env.paused().or_else(|| m.set_aside.take()).unwrap_or_default();
            let was = |l: &[(String, bool)], id: &str| l.iter().find(|(i, _)| i == id).is_none_or(|(_, on)| *on);
            m.mods.own.iter_mut().for_each(|o| o.on = was(&p.own, &o.id) && o.refused.is_none());
            m.mods.list.iter_mut().for_each(|(e, _)| e.on = was(&p.packages, &e.id));
            m.choices.hud_mod = p.hud;
            for r in m.mods.maps.iter_mut() {
                r.on = p.maps.contains(&r.folder);
                if r.on {
                    let _ = self.env.set_map(&r.folder, true);
                }
            }
            for r in m.mods.packs.iter_mut() {
                r.on = p.packs.contains(&r.id) && r.refused.is_none();
                if r.on {
                    let _ = self.env.set_mod_pack(&r.id, true);
                }
            }
            m.mods.paused = false;
            let _ = self.env.set_paused(None);
        }
        self.save_pack_lists();
        self.check_conflict();
    }

    /// The own packs' and the packages' order and on / off kept.
    fn save_pack_lists(&mut self) {
        let m = &mut self.model.mods;
        let own: Vec<(String, bool)> = m.own.iter().map(|o| (o.id.clone(), o.on)).collect();
        let r1 = self.env.save_own(&own);
        let r2 = self.env.save_mods(&m.entries());
        if let Err(e) = r1.and(r2) {
            m.results = vec![(Mark::Error, format!("The list could not be kept: {e}"))];
        }
    }

    /// The pack row `i` (own packs first, then the packages) turned on or off.
    fn toggle_pack(&mut self, i: usize) {
        let m = &mut self.model.mods;
        m.sel = i;
        let n = m.own.len();
        if i < n {
            if m.own[i].refused.is_none() {
                m.own[i].on = !m.own[i].on;
            }
        } else if let Some((e, _)) = m.list.get_mut(i - n) {
            e.on = !e.on;
        }
        self.save_pack_lists();
        self.check_conflict();
    }

    /// The chosen pack moved by `d` within its own list.
    fn move_pack(&mut self, d: i32) {
        let m = &mut self.model.mods;
        let n = m.own.len();
        let i = m.sel;
        let to = i as i32 + d;
        let ok = if i < n { to >= 0 && (to as usize) < n } else { to >= n as i32 && (to as usize) < n + m.list.len() };
        if !ok {
            return;
        }
        let to = to as usize;
        if i < n {
            m.own.swap(i, to);
        } else {
            m.list.swap(i - n, to - n);
        }
        m.sel = to;
        self.save_pack_lists();
        self.check_conflict();
        self.model.mods.show_order = true;
    }

    /// An import's results shown and the list read again.
    pub fn finish_import(&mut self, out: Vec<crate::texture_mods::Outcome>) {
        self.model.mods.busy = None;
        self.model.mods.results =
            out.into_iter().map(|o| (if o.ok { Mark::Ok } else { Mark::Error }, o.line)).collect();
        self.refresh_mods();
    }

    /// Changes the chosen package's line (`f(list, chosen)`, a move by
    /// `d`), keeps the list and follows the moved package.
    fn edit_mods(
        &mut self,
        f: impl Fn(&mut Vec<(crate::texture_mods::Entry, crate::texture_mods::Info)>, usize),
        d: i32,
    ) {
        let m = &mut self.model.mods;
        let i = m.sel;
        let to = i as i32 + d;
        if i >= m.list.len() || to < 0 || to as usize >= m.list.len() {
            return;
        }
        m.confirm = None;
        let before = m.list.clone();
        f(&mut m.list, i);
        m.sel = to as usize;
        if let Err(e) = self.env.save_mods(&m.entries()) {
            m.list = before;
            m.sel = i;
            m.results = vec![(Mark::Error, format!("The list could not be kept: {e}"))];
        }
    }

    /// Files or folders dropped on the window: what choosing them does
    /// on the screen shown (a game folder on the first-run and check
    /// screens and in the game folder's browser; packages, package
    /// folders and the HUD mod's archives on the main and Mods screens
    /// and in their browsers; a folder in the other browsers).
    pub fn drop_paths(&mut self, paths: &[PathBuf]) -> Outcome {
        let Some(first) = paths.first().cloned() else { return Outcome::Stay };
        // A dropped SPEED2.EXE (or any file) stands for its folder.
        let folder = |p: &Path| -> PathBuf {
            if p.is_file() { p.parent().map(Path::to_path_buf).unwrap_or_default() } else { p.to_path_buf() }
        };
        let archive = |p: &Path| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["zip", "7z", "rar"].contains(&e.to_ascii_lowercase().as_str()))
        };
        let purpose = (self.screen == Screen::Browse).then_some(self.model.browse.purpose);
        match (self.screen, purpose) {
            (Screen::FirstRun | Screen::Check, _) | (Screen::Browse, Some(BrowseFor::Game)) => {
                let d = folder(&first);
                self.model.check = Some(self.env.check(&d));
                self.back = Screen::FirstRun;
                self.go(Screen::Check);
            }
            (Screen::Browse, Some(BrowseFor::TexturePack)) => self.use_folder(folder(&first)),
            // A folder of saves, a profile's folder or a zip of saves.
            (Screen::Saves, _) | (Screen::Browse, Some(BrowseFor::Saves)) => {
                let p = if archive(&first) || first.is_dir() { first.clone() } else { folder(&first) };
                self.saves_folder(BrowseFor::Saves, p);
            }
            (Screen::Main | Screen::Mods, _) | (Screen::Browse, Some(BrowseFor::Packages | BrowseFor::HudMod)) => {
                // A ZIP holding a TexWizard pack is a package, not the HUD mod.
                let (hud, packs): (Vec<PathBuf>, Vec<PathBuf>) = paths.iter().cloned().partition(|p| {
                    (archive(p) && !crate::texture_mods::zip_has_texwizard_pack(p))
                        || (purpose == Some(BrowseFor::HudMod) && p.is_dir())
                });
                // A dropped folder that is no package: the TexWizard packs in it.
                let packs = super::model::expand_packages(packs);
                if !packs.is_empty() {
                    self.go(Screen::Mods);
                    self.model.mods.list = self.env.mods();
                    self.queue_import(packs);
                }
                for h in hud {
                    self.hud_pick(h);
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    pub fn event(&mut self, e: &Event) -> Outcome {
        if let Some(o) = self.event_a(e) {
            return o;
        }
        let before = self.model.choices.clone();
        let o = self.event_old(e);
        // Settings are saved when they change (the launcher may not be
        // shown again before the next start).
        if matches!(self.screen, Screen::Settings | Screen::Advanced | Screen::Mods) && self.model.choices != before {
            self.save();
        }
        o
    }

    /// The new screens' and dialogs' events (None: not handled here).
    fn event_a(&mut self, e: &Event) -> Option<Outcome> {
        if self.screen == Screen::Saves {
            if let Some(o) = self.saves_dialog_event(e) {
                return Some(o);
            }
            if self.model.ask.is_none()
                && let Some(o) = self.saves_event(e)
            {
                return Some(o);
            }
        }
        let m = &mut self.model;
        if m.ask.is_some() {
            match e {
                Event::Activate("quit") => return Some(self.quit()),
                Event::Activate(id @ ("loc_copy" | "loc_empty" | "loc_copy_replace")) => {
                    if let Some(Dialog::MoveLocation(loc, to)) = m.ask.take() {
                        let choice = copy_choice(id);
                        let plan = m.copy_plan.take();
                        self.move_location(loc, to, (*id != "loc_empty").then_some(plan).flatten(), choice);
                    }
                }
                Event::Activate(
                    id @ ("port_copy"
                    | "port_on"
                    | "port_copy_back"
                    | "port_off"
                    | "port_copy_replace"
                    | "port_copy_back_replace"),
                ) => {
                    m.ask = None;
                    let id = *id;
                    self.switch_portable(id);
                }
                Event::Activate("map_export_yes") => {
                    if let Some(Dialog::MapExportDerived(f, dir, _)) = m.ask.take() {
                        self.export_map(&f, &dir, true);
                    }
                }
                Event::Activate("reset_yes") => {
                    self.reset_defaults();
                    self.model.ask = None;
                    self.st.focus = Some("reset");
                }
                Event::Activate(_) | Event::Back => {
                    m.copy_plan = None;
                    let was = m.ask.take();
                    self.st.focus = match was {
                        Some(Dialog::Reset) => Some("reset"),
                        _ => None,
                    };
                }
                _ => {}
            }
            return Some(Outcome::Stay);
        }
        match (self.screen, e) {
            (_, Event::Activate("page:game")) => self.go_page(0),
            (_, Event::Activate("page:settings")) => self.go_page(1),
            (_, Event::Activate("page:mods")) => self.go_page(2),
            (_, Event::Activate("page:saves")) => self.go_page(3),
            (_, Event::Activate("page:dev")) => self.go_page(4),
            (Screen::Dev, Event::Activate("editor_play")) => {
                self.model.play_args = vec!["--editor".into()];
                self.motion.play_pressed();
                if self.save() {
                    return Some(Outcome::Play);
                }
            }
            (Screen::Dev, Event::Activate("dev_logs")) => {
                let d = self.model.logs_dir.clone();
                self.open_folder(&d);
            }
            (Screen::Dev, Event::Back) => self.go(Screen::Main),
            (Screen::Main, Event::Activate("row_graphics")) => {
                self.go(Screen::Settings);
                self.st.focus = Some("graphics");
            }
            (Screen::Main, Event::Activate("row_screen")) => {
                self.go(Screen::Settings);
                self.st.focus = Some(if self.model.gamescope { "resolution" } else { "display" });
            }
            (Screen::Main, Event::Activate("another" | "recheck")) => self.go_folder_fix(e),
            (Screen::Main, Event::Activate("driver_help" | "patch_help")) => {
                self.model.message = Some((
                    Mark::Info,
                    "See docs/launcher.md in Overbright's folder (\"When the game does not start\").".into(),
                ))
            }
            (Screen::Mods, Event::Toggle("use_mods")) => self.toggle_mods_in_use(),
            (Screen::Mods, Event::Toggle(id)) if PACK_IDS.contains(id) => {
                let i = PACK_IDS.iter().position(|p| p == id).unwrap_or(0);
                self.toggle_pack(i);
            }
            (Screen::Mods, Event::Toggle(id)) if MAP_IDS.contains(id) => {
                let i = MAP_IDS.iter().position(|p| p == id).unwrap_or(0);
                if let Some(r) = self.model.mods.maps.get_mut(i) {
                    r.on = !r.on;
                    let (f, on) = (r.folder.clone(), r.on);
                    self.model.mods.map_sel = Some(f.clone());
                    if let Err(e) = self.env.set_map(&f, on) {
                        self.model.mods.results = vec![(Mark::Error, format!("{f}: {e}"))];
                    }
                }
            }
            (Screen::Mods, Event::Toggle(id)) if MODPACK_IDS.contains(id) => {
                let i = MODPACK_IDS.iter().position(|p| p == id).unwrap_or(0);
                self.toggle_mod_pack(i);
            }
            (Screen::Mods, Event::Activate("pack_up")) => self.move_mod_pack(-1),
            (Screen::Mods, Event::Activate("pack_down")) => self.move_mod_pack(1),
            (Screen::Mods, Event::Activate("pack_import")) => self.browse(BrowseFor::MapMod, None),
            (Screen::Mods, Event::Activate("map_export_pack")) => {
                self.model.mods.export_pack = true;
                let start = super::dirs::home();
                self.choose_folder(BrowseFor::MapExport, start);
            }
            (Screen::Mods, Event::Activate("order")) => {
                self.model.mods.show_order = true;
                self.st.focus = Some("up");
            }
            (Screen::Mods, Event::Activate("up")) => self.move_pack(-1),
            (Screen::Mods, Event::Activate("down")) => self.move_pack(1),
            (Screen::Mods, Event::Activate("remove")) => {
                let lines = pack_lines(&self.model);
                self.model.mods.confirm = lines.get(self.model.mods.sel).map(|l| l.key.clone());
                self.st.focus = None;
            }
            (Screen::Mods, Event::Activate("remove_yes")) if self.model.mods.confirm.as_deref().is_some_and(|k| k.starts_with("own:")) => {
                let key = self.model.mods.confirm.take().unwrap_or_default();
                let id = key.trim_start_matches("own:").to_string();
                let name = self.model.mods.own.iter().find(|o| o.id == id).map_or(id.clone(), |o| o.name.clone());
                self.model.mods.results = vec![match self.env.remove_own(&id) {
                    Ok(()) => (Mark::Ok, format!("{name}: removed (your downloaded files are kept)")),
                    Err(e) => (Mark::Error, format!("{name}: could not be removed: {e}")),
                }];
                self.refresh_mods();
                self.model.mods.own.retain(|o| o.id != id || !matches!(self.model.mods.results[0].0, Mark::Ok));
                let n = pack_lines(&self.model).len();
                self.model.mods.sel = self.model.mods.sel.min(n.saturating_sub(1));
            }
            (Screen::Mods, Event::Activate("map_import")) => self.browse(BrowseFor::MapMod, None),
            (Screen::Mods, Event::Activate("map_open")) => {
                let f = self.model.mods.map_sel.clone().or_else(|| self.model.mods.maps.first().map(|r| r.folder.clone()));
                if f.is_some() {
                    self.open_editor(f);
                }
            }
            (Screen::Dev, Event::Activate("map_editor")) => self.open_editor(None),
            (Screen::Dev, Event::Activate("studio")) => self.open_studio(),
            (Screen::Dev, Event::Activate("studio_cli")) => {
                let d = super::dirs::studio_cli_exe().parent().map(Path::to_path_buf).unwrap_or_default();
                self.open_folder(&d);
            }
            (Screen::Mods, Event::Activate("map_export")) => {
                self.model.mods.export_pack = false;
                let start = super::dirs::home();
                self.choose_folder(BrowseFor::MapExport, start);
            }
            (Screen::Main, Event::Activate("prepare_now")) => {
                self.start_prepare_background();
                self.st.focus = Some("play");
            }
            (Screen::Main, Event::Activate("prepare_stop")) => {
                let p = &mut self.model.prepare;
                p.stop = true;
                p.wanted = false;
                p.state = super::prepare::State::Idle;
                p.background = false;
                p.started = None;
                self.st.focus = Some("play");
            }
            (Screen::Advanced, Event::Activate("storage")) => {
                self.refresh_storage();
                self.go(Screen::Storage);
            }
            (Screen::Storage, Event::Back) => {
                self.go(Screen::Advanced);
                self.st.focus = Some("storage");
            }
            (Screen::Storage, Event::Activate("restart")) => return Some(Outcome::Restart),
            (Screen::Storage, Event::Toggle("portable")) => {
                let p = &self.model.portable;
                if !p.marker && !p.writable {
                    self.model.message = Some((
                        Mark::Error,
                        "Overbright's folder can't be written to: portable mode stays off. Move Overbright to a \
                         folder you own."
                            .into(),
                    ));
                } else {
                    let on = !p.marker;
                    self.model.copy_plan = super::transfer::portable_plan(p, &self.model.saves_dir, on);
                    self.model.ask = Some(if on { Dialog::PortableOn } else { Dialog::PortableOff });
                }
            }
            (Screen::Storage, Event::Activate("change")) => self.go(Screen::FirstRun),
            (Screen::Storage, Event::Activate(id @ ("game_open" | "hud_open"))) => {
                let p = if *id == "game_open" {
                    self.model.root.clone()
                } else {
                    Some(PathBuf::from(self.model.choices.texture_pack.trim()))
                };
                if let Some(p) = p {
                    self.open_folder(&p);
                }
            }
            (Screen::Storage, Event::Activate("pack_browse")) => {
                let t = self.model.choices.texture_pack.trim();
                let start = (!t.is_empty()).then(|| PathBuf::from(t));
                self.choose_folder(BrowseFor::TexturePack, start);
            }
            (Screen::Storage, Event::Activate("pack_forget")) => {
                self.model.choices.texture_pack.clear();
                self.model.pack_status = self.env.set_pack("");
                self.model.hud = None;
            }
            (Screen::Storage, Event::Activate(id)) if LOC_IDS.iter().any(|(c, o, r)| c == id || o == id || r == id) => {
                let i = LOC_IDS.iter().position(|(c, o, r)| c == id || o == id || r == id).unwrap_or(0);
                if let Some(row) = self.model.locations.get(i).cloned() {
                    let (c, o, _) = LOC_IDS[i];
                    if *id == c {
                        self.model.pick_loc = Some(row.loc);
                        let start = row.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf));
                        self.choose_folder(BrowseFor::Location, start);
                    } else if *id == o {
                        if let Some(p) = &row.path {
                            self.open_folder(p);
                        }
                    } else {
                        let r = self.env.set_location(row.loc, None);
                        self.model.message = Some(match r {
                            Ok(()) => (
                                Mark::Ok,
                                format!(
                                    "{}: back to the default folder from the next start. The files in the folder you \
                                     chose stay there.",
                                    loc_name(row.loc)
                                ),
                            ),
                            Err(e) => (Mark::Error, format!("{}: not reset: {e}", loc_name(row.loc))),
                        });
                        self.refresh_storage();
                    }
                }
            }
            (Screen::Settings, Event::Activate("advanced")) => self.go(Screen::Advanced),
            (Screen::Settings, Event::Activate("reset")) => self.model.ask = Some(Dialog::Reset),
            (Screen::Settings, Event::Back) => self.go(Screen::Main),
            (Screen::Advanced, Event::Back) => {
                self.go(Screen::Settings);
                self.st.focus = Some("advanced");
            }
            (Screen::Advanced, Event::Step(id @ ("motion" | "sounds" | "text_size"), d)) => {
                let p = &mut self.model.prefs;
                match *id {
                    "motion" => p.reduced_motion = !p.reduced_motion,
                    "sounds" => p.sounds = !p.sounds,
                    _ => {
                        let sizes = super::model::Prefs::TEXT_SIZES;
                        let i = sizes.iter().position(|v| *v == p.text).unwrap_or(0) as i32;
                        p.text = sizes[(i + d).rem_euclid(sizes.len() as i32) as usize];
                    }
                }
                self.apply_prefs();
                if let Err(e) = self.env.save_prefs(&self.model.prefs) {
                    self.model.message = Some((Mark::Error, format!("The launcher's settings could not be saved: {e}")));
                }
            }
            (Screen::Check | Screen::Main, Event::Activate("details")) => {
                self.model.show_details = !self.model.show_details;
                self.model.details_sel = 0;
                if self.model.show_details && self.screen == Screen::Check {
                    self.st.focus = Some("details_list");
                }
            }
            (Screen::Check | Screen::Main, Event::Select("details_list", i)) => self.model.details_sel = *i,
            (Screen::Check | Screen::Main, Event::Activate("details_list")) => {}
            // A summary row opens the details at its area.
            (Screen::Check | Screen::Main, Event::Activate(id @ ("sum_files" | "sum_movies" | "sum_edition"))) => {
                let row = self.model.check.as_ref().and_then(|c| c.summary.as_ref()).map(|s| s.detail_row_of(id));
                self.model.show_details = true;
                self.model.details_sel = row.unwrap_or(0);
                self.st.focus = Some("details_list");
            }
            (Screen::Check, Event::Activate("support")) => self.save_support(),
            (Screen::Recover, Event::Activate("play")) => {
                self.model.start_unfinished = false;
                return Some(Outcome::Play);
            }
            (Screen::Recover | Screen::Failure, Event::Activate("safe")) => {
                self.safe_settings();
                self.model.start_unfinished = false;
                if self.save() {
                    return Some(Outcome::Play);
                }
            }
            (Screen::Recover, Event::Activate("settings")) => {
                self.model.start_unfinished = false;
                self.go(Screen::Settings);
            }
            (Screen::Recover, Event::Back) => {
                self.model.start_unfinished = false;
                self.go(Screen::Main);
            }
            (Screen::Recover | Screen::Failure | Screen::Dev, Event::Activate("support")) => self.save_support(),
            (_, Event::Activate("open_support")) => {
                if let Some(p) = &self.model.support {
                    self.env.reveal(p);
                    if let Some(d) = p.parent()
                        && let Err(err) = self.env.open_folder(d)
                    {
                        self.model.message =
                            Some((Mark::Info, format!("Open this folder yourself: {} ({err})", d.display())));
                    }
                }
            }
            _ => return None,
        }
        Some(Outcome::Stay)
    }

    /// The Storage page's state read again.
    pub fn refresh_storage(&mut self) {
        self.model.locations = self.env.locations();
        self.model.portable = self.env.portable();
    }

    fn open_folder(&mut self, p: &Path) {
        if let Err(e) = self.env.open_folder(p) {
            self.model.message = Some((Mark::Info, format!("Open this folder yourself: {} ({e})", p.display())));
        }
    }

    /// A folder put elsewhere: the files of `plan` copied first (on a
    /// worker; then checked) when asked, the locations file written once
    /// they are there.
    fn move_location(
        &mut self,
        loc: crate::platform::dirs::Location,
        to: PathBuf,
        plan: Option<super::transfer::Plan>,
        choice: super::transfer::Choice,
    ) {
        let Some(plan) = plan.filter(|p| !p.copy.is_empty() || !p.differ.is_empty()) else {
            let r = self.env.set_location(loc, Some(&to));
            self.location_done(loc, &to, r.map(|()| super::transfer::Done::default()));
            return;
        };
        for l in plan.log_lines() {
            log::info!("storage copy: {l}");
        }
        if self.env.background_support() {
            let (tx, rx) = std::sync::mpsc::channel();
            let to2 = to.clone();
            let spawned = std::thread::Builder::new().name("copy-folder".into()).spawn(move || {
                let r = super::transfer::apply(&plan, choice).and_then(|n| {
                    crate::platform::dirs::set_location(loc, Some(&to2)).map(|()| n).map_err(|e| e.to_string())
                });
                let _ = tx.send((loc, to2, r));
            });
            if spawned.is_ok() {
                self.model.copy_busy = true;
                self.copy_job = Some(rx);
                return;
            }
            return self.location_done(loc, &to, Err("the copy could not be started".into()));
        }
        let r = super::transfer::apply(&plan, choice).and_then(|n| self.env.set_location(loc, Some(&to)).map(|()| n));
        self.location_done(loc, &to, r);
    }

    fn location_done(
        &mut self,
        loc: crate::platform::dirs::Location,
        to: &Path,
        r: Result<super::transfer::Done, String>,
    ) {
        self.model.copy_busy = false;
        self.model.message = Some(match r {
            Ok(d) if d.copied == 0 && d.beside.is_empty() => {
                (Mark::Ok, format!("{}: {} from the next start.", loc_name(loc), to.display()))
            }
            Ok(d) => (
                Mark::Ok,
                format!(
                    "{}: {} ({}) copied to {} and checked{}; used from the next start. The old folder is kept.",
                    loc_name(loc),
                    super::model::count(d.copied, "file", "files"),
                    mb(d.bytes),
                    to.display(),
                    kept_words(&d)
                ),
            ),
            Err(e) => (Mark::Error, format!("{}: not moved ({e}); it stays where it was.", loc_name(loc))),
        });
        self.refresh_storage();
    }

    /// Portable mode turned on or off, the files copied when asked (the
    /// plan the question showed; files already there win).
    fn switch_portable(&mut self, how: &str) {
        let p = self.model.portable.clone();
        let (Some(data), home) = (p.data.clone(), p.home.clone()) else { return };
        let plan = self.model.copy_plan.take();
        let copy = matches!(how, "port_copy" | "port_copy_replace" | "port_copy_back" | "port_copy_back_replace");
        let turn_on = matches!(how, "port_copy" | "port_on" | "port_copy_replace");
        let copied = match plan.filter(|_| copy) {
            Some(plan) => {
                for l in plan.log_lines() {
                    log::info!("portable copy: {l}");
                }
                super::transfer::apply(&plan, copy_choice(how))
            }
            None => Ok(super::transfer::Done::default()),
        };
        let r = copied
            .and_then(|n| if turn_on { self.env.make_portable() } else { self.env.unmake_portable() }.map(|()| n));
        let copied_words = |d: &super::transfer::Done| {
            if d.copied > 0 || !d.beside.is_empty() {
                format!(" ({} copied there{})", super::model::count(d.copied, "file", "files"), kept_words(d))
            } else {
                String::new()
            }
        };
        self.model.message = Some(match r {
            Err(e) => (Mark::Error, format!("Portable mode not changed: {e}")),
            Ok(d) if turn_on => (
                Mark::Ok,
                format!(
                    "Portable mode is on: after a restart everything is kept in {}{}.",
                    data.display(),
                    copied_words(&d)
                ),
            ),
            Ok(d) => (
                Mark::Ok,
                format!(
                    "Portable mode is off: after a restart everything is kept in {}{}. The folder {} stays.",
                    home.display(),
                    copied_words(&d),
                    data.display()
                ),
            ),
        });
        let ok = self.model.message.as_ref().is_some_and(|(k, _)| *k == Mark::Ok);
        let before = self.model.portable.on;
        self.refresh_storage();
        // This run keeps the folders it started with until the restart.
        self.model.portable.on = before;
        if ok {
            // The switch's new state, whatever the environment reports.
            self.model.portable.marker = turn_on;
            self.model.portable.changed = true;
            self.st.focus = Some("restart");
        }
    }

    /// The launcher's own preferences put into force (motion, sounds; the
    /// text size at the next build).
    pub fn apply_prefs(&mut self) {
        super::anim::set_reduced(self.model.prefs.reduced_motion);
        super::sounds::set_enabled(self.model.prefs.sounds);
    }

    /// "Shaders ready" is on the screen (it goes away after a few seconds).
    pub fn shaders_ready_shown(&self) -> bool {
        self.screen == Screen::Main && shaders(&self.model) == Shaders::JustReady
    }

    /// The page `i` of the top bar (Game, Settings, Mods).
    pub fn go_page(&mut self, i: usize) {
        match i {
            0 => self.go(Screen::Main),
            1 => self.go(Screen::Settings),
            3 => self.open_saves(),
            4 => self.go(Screen::Dev),
            _ => {
                self.model.mods.list = self.env.mods();
                self.model.mods.confirm = None;
                self.go(Screen::Mods);
            }
        }
    }

    /// The main screen's folder fix: check the saved folder again, or
    /// choose another.
    fn go_folder_fix(&mut self, e: &Event) {
        let root = self.model.root.clone();
        if *e == Event::Activate("recheck") {
            if let Some(r) = root {
                self.model.check = Some(self.env.check(&r));
            }
        } else {
            self.choose_folder(BrowseFor::Game, root.and_then(|r| r.parent().map(Path::to_path_buf)));
        }
    }

    /// The safe start: the original graphics, in a window, without the HUD
    /// mod or texture packs.
    fn safe_settings(&mut self) {
        let m = &mut self.model;
        m.choices.graphics = Graphics::Original;
        if m.choices.fullscreen {
            m.choices.fullscreen = false;
            m.choices.resolution = crate::render::display::fit_mode(&m.modes(), m.choices.resolution);
        }
        m.choices.hud_mod = false;
        let list = self.env.mods();
        if list.iter().any(|(e, _)| e.on) {
            let off: Vec<_> = list
                .into_iter()
                .map(|(mut e, i)| {
                    e.on = false;
                    (e, i)
                })
                .collect();
            let entries: Vec<_> = off.iter().map(|(e, _)| e.clone()).collect();
            if let Err(err) = self.env.save_mods(&entries) {
                log::warn!("texture packs not turned off for the safe start: {err}");
            }
            self.model.mods.list = off;
        }
    }

    /// Settings back to what Overbright picks for this computer (the
    /// probe's first-run values); mods and the game folder stay.
    fn reset_defaults(&mut self) {
        let m = &mut self.model;
        if let Some(f) = m.probe.first_run().cloned() {
            m.choices.fullscreen = f.fullscreen;
            m.choices.resolution = f.resolution;
            m.choices.graphics = if f.detail == 0 { Graphics::Original } else { Graphics::Enhanced };
        } else if let Some(d) = m.desktop() {
            m.choices.fullscreen = true;
            m.choices.resolution = d;
        }
        m.choices.skip_movies = false;
        m.choices.skip = false;
        self.save();
    }

    /// The support file written (the failure's words and the game's last
    /// lines in it); its place shown with Open folder.
    fn save_support(&mut self) {
        use crate::diag::support::SupportRequest;
        let reason = match self.screen {
            Screen::Recover => Some(Issue::StartUnfinished.words().title),
            Screen::Failure => self.model.failure.as_ref().map(|f| format!("{:?}: {}", f.kind, f.how)),
            Screen::Check => self.model.check.as_ref().map(|c| format!("game check: {:?}", c.state())),
            _ => None,
        };
        let mut req = SupportRequest::launcher(reason);
        req.game_root = self.model.root.clone();
        if self.screen == Screen::Check
            && let Some(c) = &self.model.check
        {
            req.game_root = Some(c.root.clone());
        }
        if let Some(f) = &self.model.failure {
            req.extra.push(("The game's last lines".into(), f.tail.join("\n")));
        }
        if self.model.support_busy {
            return;
        }
        if self.env.background_support() {
            // Asking the game program takes up to 20 s: on a worker, the
            // window stays live ("Saving the support file…").
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new().name("support-file".into()).spawn(move || {
                let _ = tx.send(crate::diag::support::write_support_file(&req).map_err(|e| e.to_string()));
            });
            if spawned.is_ok() {
                self.model.support_busy = true;
                self.model.message = Some((Mark::Busy, "Saving the support file…".into()));
                self.support_job = Some(rx);
                return;
            }
            self.model.message = Some((Mark::Error, "The support file could not be started.".into()));
            return;
        }
        let r = self.env.support_file(&req).map_err(|e| e.to_string());
        self.support_done(r);
    }

    /// The crashed game's system errors, asked on a worker (the tools can
    /// take seconds); shown when they come.
    pub fn ask_system_errors(&mut self, pid: u32) {
        if self.env.background_support() {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new().name("crash-errors".into()).spawn(move || {
                let _ = tx.send(crate::launcher::crash_report::system_errors(pid, "game"));
            });
            if spawned.is_ok() {
                self.crash_job = Some(rx);
                return;
            }
        }
        let errors = self.env.crash_errors(pid, "game");
        self.crash_errors_done(errors);
    }

    fn crash_errors_done(&mut self, errors: Vec<String>) {
        if let Some(r) = self.model.failure.as_mut().and_then(|f| f.report.as_mut()) {
            r.errors = errors;
            r.asking = false;
        }
    }

    /// "Copy for a bug report": the redacted text onto the clipboard; with
    /// no clipboard tool, saved as a file whose folder opens.
    fn copy_report(&mut self) {
        let Some(f) = self.model.failure.as_ref() else { return };
        let Some(r) = f.report.as_ref() else { return };
        let facts = crate::launcher::crash_report::Facts {
            program: "the game".into(),
            reason: r.reason.clone(),
            secs: r.secs,
            during_start: f.during_start,
            tail: f.tail.clone(),
            errors: r.errors.clone(),
            logs: r.logs.clone(),
        };
        let text = crate::launcher::crash_report::report(&facts);
        self.model.message = Some(match self.env.copy_text(&text) {
            Ok(()) => (Mark::Ok, "Copied. Paste it into your bug report; nothing was sent.".into()),
            Err(e) => match crate::launcher::crash_report::save_text(&self.model.logs_dir, &text) {
                Ok(p) => {
                    self.model.support = Some(p.clone());
                    (Mark::Info, format!("The clipboard isn't available ({e}). The report is saved: {}", p.display()))
                }
                Err(e2) => (Mark::Error, format!("The report could not be copied ({e}) or saved ({e2}).")),
            },
        });
    }

    fn support_done(&mut self, r: Result<PathBuf, String>) {
        self.model.support_busy = false;
        match r {
            Ok(p) => {
                self.model.message = Some((Mark::Ok, format!("Support file saved: {}", p.display())));
                self.model.support = Some(p);
            }
            Err(err) => self.model.message = Some((Mark::Error, format!("The support file could not be saved: {err}"))),
        }
    }

    /// Start the map editor on the mod in `folder` (None: its own Open /
    /// New). One at a time: the page waits while it is open.
    pub fn open_editor(&mut self, folder: Option<String>) {
        if self.editor_job.is_some() {
            return;
        }
        match self.env.start_editor(folder.as_deref(), self.model.root.as_deref()) {
            Ok(job) => {
                self.editor_job = Some(job);
                self.model.mods.editor = super::model::EditorRun::Open(folder);
            }
            Err(e) => self.model.mods.editor = super::model::EditorRun::Failed(e),
        }
    }

    /// Starts Studio on its Home (one at a time); a failed start is shown
    /// under its button.
    pub fn open_studio(&mut self) {
        if self.studio_job.is_some() {
            return;
        }
        match self.env.start_studio(self.model.root.as_deref()) {
            Ok(job) => {
                self.studio_job = Some(job);
                self.model.studio = super::model::EditorRun::Open(None);
            }
            Err(e) => self.model.studio = super::model::EditorRun::Failed(e),
        }
    }

    /// Studio's end: its button usable again, the map mods read again (it
    /// may have made or changed one). True when it ended.
    fn poll_studio(&mut self) -> bool {
        let Some(end) = self.studio_job.as_mut().and_then(|j| j.poll()) else { return false };
        self.studio_job = None;
        self.model.studio = match end {
            Ok(()) => super::model::EditorRun::Closed,
            Err(e) => super::model::EditorRun::Failed(e),
        };
        self.refresh_mods();
        true
    }

    /// The map editor's end: the page usable again, its map mods read
    /// again (the editor may have made or changed one). True when it ended.
    fn poll_editor(&mut self) -> bool {
        let Some(end) = self.editor_job.as_mut().and_then(|j| j.poll()) else { return false };
        self.editor_job = None;
        self.model.mods.editor = match end {
            Ok(()) => super::model::EditorRun::Closed,
            Err(e) => super::model::EditorRun::Failed(e),
        };
        self.refresh_mods();
        true
    }

    /// The window's poll: a finished support file shown, the map editor's
    /// end. True when the screen changed.
    pub fn poll_jobs(&mut self, fonts: &Fonts) -> bool {
        if self.poll_editor() | self.poll_studio() | self.poll_game_data() || self.poll_saves() {
            self.rebuild(fonts);
            return true;
        }
        if self.studio_wake().is_some_and(|t| std::time::Instant::now() >= t) && self.look_at_studio() {
            self.rebuild(fonts);
            return true;
        }
        if let Some(rx) = &self.copy_job {
            match rx.try_recv() {
                Ok((loc, to, r)) => {
                    self.copy_job = None;
                    self.location_done(loc, &to, r);
                    self.rebuild(fonts);
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.copy_job = None;
                    self.model.copy_busy = false;
                    self.rebuild(fonts);
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.crash_job {
            match rx.try_recv() {
                Ok(v) => {
                    self.crash_job = None;
                    self.crash_errors_done(v);
                    self.rebuild(fonts);
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.crash_job = None;
                    self.crash_errors_done(Vec::new());
                    self.rebuild(fonts);
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(rx) = &self.support_job else { return false };
        match rx.try_recv() {
            Ok(r) => {
                self.support_job = None;
                self.support_done(r);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.support_job = None;
                self.support_done(Err("the worker stopped".into()));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
        }
        self.rebuild(fonts);
        true
    }

    fn event_old(&mut self, e: &Event) -> Outcome {
        match (self.screen, e) {
            (_, Event::Activate("quit")) => return self.quit(),
            // Back never quits at once: it asks (as on the main screen).
            (Screen::FirstRun, Event::Back) => self.model.ask = Some(Dialog::Quit),
            (Screen::FirstRun, Event::Select("found", i)) => self.model.found_sel = *i,
            (Screen::FirstRun, Event::Edit("path", ed)) => {
                apply_edit(&mut self.model.path_input, &mut self.st.cursor, ed)
            }
            (Screen::FirstRun, Event::Activate("browse")) => {
                let start = self.model.found.get(self.model.found_sel).map(|f| f.path.clone());
                self.choose_folder(BrowseFor::Game, start.and_then(|p| p.parent().map(Path::to_path_buf)));
            }
            (Screen::FirstRun, Event::Activate("check" | "found" | "path")) => {
                let m = &mut self.model;
                let typed = m.path_input.trim();
                let dir = if !typed.is_empty() {
                    Some(PathBuf::from(typed))
                } else {
                    m.found.get(m.found_sel).map(|f| f.path.clone())
                };
                match dir {
                    Some(d) => {
                        m.check = Some(self.env.check(&d));
                        self.go(Screen::Check);
                    }
                    None => {
                        m.message = Some((Mark::Warn, "Choose a folder from the list or type its path first.".into()))
                    }
                }
            }
            (Screen::Check, Event::Activate("back") | Event::Back) => self.go(Screen::FirstRun),
            (Screen::Check, Event::Activate("another")) => {
                let start = self.model.check.as_ref().and_then(|c| c.root.parent().map(Path::to_path_buf));
                self.choose_folder(BrowseFor::Game, start);
            }
            (Screen::Check, Event::Activate("recheck")) => {
                if let Some(r) = self.model.check.as_ref().map(|c| c.root.clone()) {
                    self.model.check = Some(self.env.check(&r));
                    self.st.focus = None;
                }
            }
            (Screen::Check, Event::Activate("use")) => {
                if let Some(c) = self.model.check.clone().filter(|c| c.usable) {
                    match self.env.keep_root(&c.root) {
                        Ok(()) => {
                            self.model.languages = self.env.languages(&c.root);
                            self.model.root = Some(c.root);
                            self.go(Screen::Main);
                            // The next step of the setup: the own copy, offered with Make copy chosen.
                            self.offer_game_data();
                        }
                        Err(err) => {
                            self.model.message = Some((Mark::Error, format!("The folder could not be kept: {err}")))
                        }
                    }
                }
            }
            (Screen::Advanced, Event::Activate("change")) => self.go(Screen::FirstRun),
            // Back (Escape, a pad's B) never leaves at once: it asks.
            (Screen::Main, Event::Back) => self.model.ask = Some(Dialog::Quit),
            (
                Screen::Settings | Screen::Advanced,
                Event::Step(id @ ("display" | "resolution" | "graphics" | "skip_movies" | "skip_title"), _),
            ) if !display_rows_on(&self.model) => {
                let _ = id;
            }
            (Screen::Settings, Event::Step("display", _)) => {
                // The size moves to one the other path offers, as the
                // game's Screen Mode row does (`display::fit_mode`).
                let m = &mut self.model;
                m.choices.fullscreen = !m.choices.fullscreen;
                m.choices.resolution = crate::render::display::fit_mode(&m.modes(), m.choices.resolution);
            }
            (Screen::Settings, Event::Step("resolution", d)) => {
                let m = &mut self.model;
                let modes = m.modes();
                if !modes.is_empty() {
                    m.choices.resolution = Self::step(&modes, &m.choices.resolution, *d);
                }
            }
            (Screen::Advanced, Event::Step("language", d)) => {
                let m = &mut self.model;
                let list = m.supported_languages();
                let now = m.language_name();
                let next = Self::step(&list, &now, *d);
                // English is the default: nothing is kept for it.
                m.choices.language =
                    if next.eq_ignore_ascii_case(crate::game_assets::DEFAULT_LANGUAGE) { String::new() } else { next };
            }
            (Screen::Settings, Event::Step("graphics", d)) => {
                self.model.choices.graphics = Self::step(&GRAPHICS, &self.model.choices.graphics, *d)
            }
            (Screen::Settings, Event::Toggle("skip") | Event::Step("skip", _)) => {
                self.model.choices.skip = !self.model.choices.skip
            }
            (Screen::Settings, Event::Toggle("skip_movies") | Event::Step("skip_movies", _)) => {
                self.model.choices.skip_movies = !self.model.choices.skip_movies
            }
            (Screen::Settings, Event::Toggle("skip_title") | Event::Step("skip_title", _)) => {
                self.model.choices.skip_title = !self.model.choices.skip_title
            }
            (Screen::Main, Event::Activate("mods" | "row_mods")) => {
                self.model.mods.list = self.env.mods();
                self.model.mods.confirm = None;
                self.go(Screen::Mods);
            }
            (Screen::Mods, Event::Activate("back") | Event::Back) => {
                if self.model.mods.confirm.take().is_none() {
                    self.go(Screen::Main);
                } else {
                    self.st.focus = Some("remove");
                }
            }
            (Screen::Mods, Event::Activate("pack_browse")) => {
                let t = self.model.choices.texture_pack.trim();
                let start = (!t.is_empty()).then(|| PathBuf::from(t));
                self.choose_folder(BrowseFor::TexturePack, start);
            }
            (Screen::Mods, Event::Activate("pack_forget")) => {
                self.model.choices.texture_pack.clear();
                self.model.choices.hud_mod = false;
                self.model.pack_status = self.env.set_pack("");
                self.model.hud = None;
            }
            (Screen::Mods, Event::Toggle("hud")) => self.model.choices.hud_mod = !self.model.choices.hud_mod,
            (Screen::Mods, Event::Activate("hud_add")) => {
                let start = self.model.browse.dir.clone();
                let start = (self.model.browse.purpose == BrowseFor::HudMod && !start.as_os_str().is_empty())
                    .then_some(start)
                    .or_else(|| self.env.home());
                self.browse(BrowseFor::HudMod, start);
            }
            (Screen::Mods, Event::Activate("add")) => {
                let start = self.model.browse.dir.clone();
                let start = (self.model.browse.purpose == BrowseFor::Packages && !start.as_os_str().is_empty())
                    .then_some(start)
                    .or_else(|| self.env.home());
                self.browse(BrowseFor::Packages, start);
            }
            (Screen::Mods, Event::Select("pkgs", i)) => {
                self.model.mods.sel = *i;
                self.model.mods.confirm = None;
            }
            (Screen::Mods, Event::Activate("pkgs" | "toggle")) => self.edit_mods(|l, i| l[i].0.on = !l[i].0.on, 0),
            (Screen::Mods, Event::Activate("keep")) => {
                self.model.mods.confirm = None;
                self.st.focus = Some("remove");
            }
            (Screen::Mods, Event::Activate("remove_yes")) => {
                if let Some(id) = self.model.mods.confirm.take() {
                    let name = self
                        .model
                        .mods
                        .list
                        .iter()
                        .find(|(e, _)| e.id == id)
                        .map_or(id.clone(), |(_, i)| i.name.clone());
                    let line = match self.env.remove_mod(&id) {
                        Ok(()) => (Mark::Ok, format!("{name}: removed (your package file is kept)")),
                        Err(e) => (Mark::Error, format!("{name}: could not be removed: {e}")),
                    };
                    self.model.mods.results = vec![line];
                    self.model.mods.list = self.env.mods();
                    self.model
                        .mods
                        .list
                        .retain(|(e, _)| e.id != id || !matches!(self.model.mods.results[0].0, Mark::Ok));
                    self.model.mods.sel = self.model.mods.sel.min(self.model.mods.list.len().saturating_sub(1));
                }
            }
            (Screen::Advanced, Event::Activate("about")) => {
                self.back = Screen::Advanced;
                self.go(Screen::About);
            }
            (Screen::Advanced, Event::Activate("import")) => {
                self.open_saves();
                self.st.focus = Some("find");
            }
            (Screen::Main, Event::Activate("play")) => {
                self.motion.play_pressed();
                if self.model.just_found
                    && let Some(r) = self.model.root.clone()
                {
                    match self.env.keep_root(&r) {
                        Ok(()) => self.model.just_found = false,
                        Err(e) => log::warn!("the found game folder was not kept: {e}"),
                    }
                }
                if self.save() {
                    return Outcome::Play;
                }
            }
            (Screen::About, Event::Activate("back") | Event::Back) => self.go(Screen::Advanced),
            (Screen::About, Event::Activate("portable")) => {
                let before = self.model.folders.clone();
                self.model.message = Some(match self.env.make_portable() {
                    Ok(()) => {
                        let now = self.env.folders();
                        // The game folder and the launcher's choices are kept in the
                        // new folder at once, so Play and a later start of the game
                        // alone use them (the rest of options.txt starts native).
                        if let Some(r) = self.model.root.clone()
                            && let Err(e) = self.env.keep_root(&r)
                        {
                            log::warn!("game folder not kept in the portable folder: {e}");
                        }
                        if !self.model.choices.texture_pack.trim().is_empty() {
                            self.model.pack_status = self.env.set_pack(&self.model.choices.texture_pack);
                        }
                        let first = self.model.probe.first_run().cloned();
                        self.model.settings_file = false;
                        if self.env.save(&self.model.choices, first.as_ref()).is_ok() {
                            self.model.saved = self.model.choices.clone();
                            self.model.settings_file = true;
                        }
                        self.model.logs_dir = now.logs.clone();
                        self.model.saves_dir = now.saves.clone();
                        // On Windows the profiles are inside Overbright's folder.
                        let saves = if before.saves.starts_with(&before.data) {
                            String::new()
                        } else {
                            format!(" and the folder {}", before.saves.display())
                        };
                        let text = format!(
                            "Portable mode is on: from now on everything is kept in {}, the game folder and \
                             the choices on this launcher's main page already. To take your other settings, saves \
                             and mods along, copy what is in {}{saves} into it.",
                            now.data.display(),
                            before.data.display(),
                        );
                        self.model.folders = now;
                        (Mark::Ok, text)
                    }
                    Err(e) => (Mark::Error, format!("Portable mode could not be turned on: {e}")),
                });
            }
            (Screen::About, Event::Activate("licence")) => self.go(Screen::Licence),
            (Screen::About, Event::Activate("notices")) => self.go(Screen::Notices),
            (Screen::Licence | Screen::Notices, Event::Activate("back") | Event::Back) => self.go(Screen::About),
            (Screen::Failure, Event::Activate("settings")) => self.go(Screen::Settings),
            (Screen::Failure, Event::Back) => self.go(Screen::Main),
            (Screen::Failure, Event::Activate("retry")) => return Outcome::Play,
            (Screen::Failure, Event::Activate("details")) => self.model.failure_details = !self.model.failure_details,
            (Screen::Failure, Event::Activate("folder")) => match self.model.root.clone() {
                // The folder in use checked again: the check screen says
                // what is missing and offers the fix.
                Some(r) => {
                    self.model.check = Some(self.env.check(&r));
                    self.back = Screen::FirstRun;
                    self.go(Screen::Check);
                }
                None => self.go(Screen::FirstRun),
            },
            (Screen::Failure, Event::Activate("windowed")) => {
                let m = &mut self.model;
                if m.choices.fullscreen {
                    m.choices.fullscreen = false;
                    m.choices.resolution = crate::render::display::fit_mode(&m.modes(), m.choices.resolution);
                }
                if self.save() {
                    return Outcome::Play;
                }
            }
            (Screen::Failure, Event::Activate("copy_report")) => self.copy_report(),
            (Screen::Failure, Event::Activate("open_logs")) => {
                let dir = self
                    .model
                    .failure
                    .as_ref()
                    .and_then(|f| f.log.as_ref())
                    .and_then(|l| l.parent().map(Path::to_path_buf));
                let dir = dir.unwrap_or_else(|| self.model.logs_dir.clone());
                if let Err(e) = self.env.open_folder(&dir) {
                    self.model.message =
                        Some((Mark::Info, format!("Open this folder yourself: {} ({e})", dir.display())));
                }
            }
            (Screen::Browse, Event::Activate("cancel") | Event::Back) => {
                let back = self.back;
                self.go(back);
            }
            (Screen::Browse, Event::Select("dirs", i)) => self.model.browse.sel = *i,
            (Screen::Browse, Event::Activate("up")) => {
                let b = &self.model.browse;
                if !b.dir.as_os_str().is_empty() {
                    let up = self.env.parent(&b.dir).unwrap_or_else(|| b.dir.clone());
                    let from = b.dir.clone();
                    self.open_dir(up);
                    // The folder come from stays chosen (a second Up, or
                    // Enter on it, is one step).
                    let at = self.model.browse.entries.iter().position(|e| e.path == from);
                    if let Some(i) = at {
                        self.model.browse.sel = i + usize::from(!self.model.browse.dir.as_os_str().is_empty());
                    }
                }
            }
            (Screen::Browse, Event::Activate(id)) if PLACE_IDS.contains(id) => {
                let i = PLACE_IDS.iter().position(|p| p == id).unwrap_or(0);
                if let Some((_, d)) = self.model.browse.places.get(i).cloned() {
                    self.open_dir(d);
                }
            }
            (Screen::Browse, Event::Activate(id)) if id.starts_with("crumb") => {
                if let Some(d) = crumb_target(&self.model.browse.dir.display().to_string(), id) {
                    self.open_dir(d);
                }
            }
            (Screen::Browse, Event::Activate("dirs" | "open")) => {
                if let Some(d) = self.browse_row(self.model.browse.sel) {
                    if self.model.browse.purpose == BrowseFor::Packages && self.is_package_row(&d) {
                        self.go(Screen::Mods);
                        self.queue_import(vec![d]);
                    } else if self.model.browse.purpose == BrowseFor::HudMod
                        && self.model.browse.entries.iter().any(|e| e.path == d && e.note.contains("archive"))
                    {
                        self.hud_pick(d);
                    } else if self.model.browse.purpose == BrowseFor::Saves
                        && self.model.browse.entries.iter().any(|e| e.path == d && e.note.contains("archive"))
                    {
                        self.use_folder(d);
                    } else if self.model.browse.purpose == BrowseFor::MapMod
                        && (self.model.browse.entries.iter().any(|e| e.path == d && e.note.contains("archive"))
                            || self.env.mod_kind(&d) == Some(super::model::ModKind::Map))
                    {
                        self.use_folder(d);
                    } else {
                        self.open_dir(d);
                    }
                }
            }
            (Screen::Browse, Event::Edit("browse_path", ed)) => {
                apply_edit(&mut self.model.browse.input, &mut self.st.cursor, ed)
            }
            (Screen::Browse, Event::Activate("browse_path")) => {
                let t = self.model.browse.input.trim().to_string();
                if !t.is_empty() {
                    self.open_dir(PathBuf::from(t));
                    if self.model.browse.error.is_none() {
                        self.model.browse.input.clear();
                    }
                }
            }
            (Screen::Browse, Event::Activate("system")) => {
                let title = dialog_title(self.model.browse.purpose);
                let start = Some(self.model.browse.dir.clone()).filter(|d| !d.as_os_str().is_empty());
                match self.env.folder_dialog(title, start.as_deref()) {
                    Picked::Folder(d) => self.use_folder(d),
                    Picked::Cancelled => {}
                    Picked::NoDialog => {
                        self.model.message = Some((Mark::Info, "The system's folder dialog could not open.".into()))
                    }
                }
            }
            (Screen::Browse, Event::Activate("use")) => {
                // The highlighted row when it is the game folder (its note
                // says so): what a player choosing the game means; else the
                // folder shown above.
                let b = &self.model.browse;
                let up = usize::from(!b.dir.as_os_str().is_empty());
                let game_row = (b.purpose == BrowseFor::Game)
                    .then(|| b.sel.checked_sub(up).and_then(|i| b.entries.get(i)))
                    .flatten()
                    .filter(|e| e.note == super::model::GAME_FOLDER_NOTE)
                    .map(|e| e.path.clone());
                let d = game_row.unwrap_or_else(|| b.dir.clone());
                if !d.as_os_str().is_empty() {
                    self.use_folder(d);
                }
            }
            // The prepare step: Skip stops the run (the window owns it),
            // Continue leaves the finished one.
            (Screen::Prepare, Event::Activate("skip" | "continue") | Event::Back) => {
                self.model.prepare.stop = true;
                self.model.prepare.wanted = false;
                self.go(Screen::Main);
            }
            (Screen::GameData, Event::Activate("import")) => self.game_data_run(super::game_data::Kind::Import),
            (Screen::GameData, Event::Activate("import_again")) => {
                self.game_data_run(super::game_data::Kind::ImportAgain)
            }
            (Screen::GameData, Event::Activate("retry")) => self.game_data_run(self.model.game_data.last),
            (Screen::GameData, Event::Activate("repair")) => self.game_data_run(super::game_data::Kind::Repair),
            (Screen::GameData, Event::Activate("verify")) => self.game_data_run(super::game_data::Kind::Verify),
            (Screen::GameData, Event::Activate("stop")) => {
                if let Some(j) = &self.game_data_job {
                    j.stop();
                    self.model.game_data.stopping = true;
                }
            }
            (Screen::GameData, Event::Activate("storage")) => {
                self.refresh_storage();
                self.go(Screen::Storage);
            }
            (Screen::GameData, Event::Activate("later")) => {
                self.env.set_game_data_later();
                self.game_data_leave();
            }
            // Back while a run goes on: it goes on behind the other screens.
            (Screen::GameData, Event::Activate("continue") | Event::Back) => self.game_data_leave(),
            (Screen::Advanced, Event::Activate("game_data")) => self.open_game_data(true),
            (Screen::Advanced, Event::Activate("prepare")) => self.start_prepare(),
            // This computer checked again (a driver installed meanwhile):
            // the window runs the probe.
            (Screen::Main | Screen::Failure, Event::Activate("probe_again")) => {
                self.model.probe = ProbeState::Pending;
                self.model.probe_again = true;
            }
            (Screen::Widgets, Event::Step("cycle", d)) => {
                self.model.demo.cycle = (self.model.demo.cycle as i32 + d).rem_euclid(DEMO_VALUES.len() as i32) as usize
            }
            (Screen::Widgets, Event::Toggle("check")) => self.model.demo.checked = !self.model.demo.checked,
            (Screen::Widgets, Event::Edit("text", ed)) => {
                apply_edit(&mut self.model.demo.text, &mut self.st.cursor, ed)
            }
            (Screen::Widgets, Event::Select("list", i)) => self.model.demo.sel = *i,
            (Screen::Widgets, Event::Back) => return Outcome::Quit,
            _ => {}
        }
        Outcome::Stay
    }

    /// Saves the changed choices; false (with a message) when that failed.
    /// Before the game's first run (no options.txt) it writes only with
    /// the probe's first-run values, as the game's first start would
    /// (`model::save_choices`).
    fn save(&mut self) -> bool {
        let first = self.model.probe.first_run().cloned();
        let m = &mut self.model;
        if !matches!(
            self.screen,
            Screen::Main | Screen::Failure | Screen::Settings | Screen::Advanced | Screen::Recover
        ) {
            return true;
        }
        if m.choices.texture_pack != m.saved.texture_pack {
            m.pack_status = self.env.set_pack(&m.choices.texture_pack);
        }
        if m.choices.language != m.saved.language {
            match self.env.set_language(&m.choices.language) {
                Ok(()) => m.saved.language = m.choices.language.clone(),
                Err(e) => {
                    m.message = Some((Mark::Error, format!("The game language could not be kept: {e}")));
                    m.choices.language = m.saved.language.clone();
                }
            }
        }
        let mut rest = m.choices.clone();
        rest.texture_pack = m.saved.texture_pack.clone();
        if rest == m.saved || (!m.settings_file && first.is_none()) {
            m.saved.texture_pack = m.choices.texture_pack.clone();
            return true;
        }
        match self.env.save(&m.choices, if m.settings_file { None } else { first.as_ref() }) {
            Ok(()) => {
                m.saved = m.choices.clone();
                m.settings_file = true;
                true
            }
            Err(e) => {
                m.message = Some((Mark::Error, format!("The settings could not be saved: {e}")));
                false
            }
        }
    }

    fn quit(&mut self) -> Outcome {
        if self.save() { Outcome::Quit } else { Outcome::Stay }
    }
}
