//! The launcher's screens: what each shows ([`build`]) and what its
//! controls do ([`App::event`]): first run (find or type the game folder),
//! the check result, the main screen, about (+ the licence and the
//! third-party notices), the start failure, the folder browser, the save
//! import, and a widget test screen (docs/launcher.md).

use std::path::{Path, PathBuf};

use super::draw::theme;
use super::font::{Cut, Face, Fonts};
use super::layout::{Align, Builder, Ctrl, Link, build_fit};
use super::model::{BrowseFor, DirEntry, Env, Graphics, Model, ProbeState, other_languages, path_parts, windows_form};
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
    Import,
    Mods,
    /// The prepare step (`game --prepare`): its bar, Skip.
    Prepare,
    Widgets,
}

impl Screen {
    pub const ALL: [Screen; 12] = [
        Screen::FirstRun,
        Screen::Check,
        Screen::Main,
        Screen::About,
        Screen::Licence,
        Screen::Notices,
        Screen::Failure,
        Screen::Browse,
        Screen::Import,
        Screen::Mods,
        Screen::Prepare,
        Screen::Widgets,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Screen::FirstRun => "first-run",
            Screen::Check => "check",
            Screen::Main => "main",
            Screen::About => "about",
            Screen::Licence => "licence",
            Screen::Notices => "notices",
            Screen::Failure => "failure",
            Screen::Browse => "browse",
            Screen::Import => "import",
            Screen::Mods => "mods",
            Screen::Prepare => "prepare",
            Screen::Widgets => "widgets",
        }
    }

    pub fn parse(s: &str) -> Option<Screen> {
        Screen::ALL.into_iter().find(|x| x.name() == s)
    }

    /// A long document: the page scrolls instead of shrinking.
    pub fn document(self) -> bool {
        matches!(self, Screen::Licence | Screen::Notices)
    }
}

/// What the window must do after an input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Quit,
    /// Start the game (saved first).
    Play,
}

/// The name shown.
pub const NAME: &str = "Overbright";
/// The line every first screen carries.
pub const OWN_COPY: &str = "Overbright needs your own installed copy of Need for Speed Underground 2 (PC). \
Nothing from the original game is included; Overbright reads your copy and never changes it.";
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
    if m.folders.portable {
        b.header(NAME, &format!("{} · Portable", m.version));
    } else {
        b.header(NAME, &m.version);
    }
    if screen != Screen::Main {
        b.content_begin();
    }
    match screen {
        Screen::FirstRun => {
            b.heading("Find your game");
            b.body(OWN_COPY);
            b.gap(4.0);
            b.dim(NO_MODS);
            b.gap(14.0);
            if m.found.is_empty() {
                b.body("No game folder was found on this computer.");
            } else {
                b.dim("Found on this computer:");
                b.gap(6.0);
                let items: Vec<(String, String)> =
                    m.found.iter().map(|f| (f.path.display().to_string(), f.source.clone())).collect();
                b.list("found", &items, m.found_sel, items.len().clamp(1, 4));
                b.default_focus = Some("found");
            }
            b.gap(16.0);
            b.dim(if m.found.is_empty() {
                "Type the path of the folder the game was installed to (the one that holds the GLOBAL folder), \
                 or browse for it:"
            } else {
                "Or type the folder's path (the folder that holds the GLOBAL folder), or browse for it:"
            });
            b.gap(6.0);
            b.buttons(
                Align::Left,
                vec![
                    Ctrl::button("browse", "Browse…"),
                    Ctrl::Field { id: "path", text: m.path_input.clone(), placeholder: "Game folder".into() },
                ],
            );
            if m.found.is_empty() {
                b.default_focus = Some("path");
            }
            message(b, m);
            b.gap(12.0);
            b.dim(NOT_AFFILIATED);
            b.footer(vec![Ctrl::button("quit", "Quit"), Ctrl::primary("check", "Check folder")]);
        }
        Screen::Check => {
            b.heading("Game folder");
            let path = m.check.as_ref().map(|c| c.root.display().to_string()).unwrap_or_default();
            line(b, &path);
            b.gap(14.0);
            if let Some(c) = &m.check {
                for (mark, text) in &c.lines {
                    b.status(*mark, text);
                }
                if !c.usable {
                    b.gap(6.0);
                    b.body(advice(c));
                }
            }
            message(b, m);
            let usable = m.check.as_ref().is_some_and(|c| c.usable);
            if usable {
                b.footer(vec![Ctrl::button("back", "Back"), Ctrl::primary("use", "Use this folder")]);
            } else {
                // The way out of a refused folder is the main button.
                b.footer(vec![
                    Ctrl::button("back", "Back"),
                    Ctrl::Button { id: "use", text: "Use this folder".into(), primary: false, enabled: false },
                    Ctrl::primary("another", "Choose another folder…"),
                ]);
            }
            b.default_focus = Some(if usable { "use" } else { "another" });
        }
        Screen::Main => build_main(b, m),
        Screen::About => {
            b.heading("About Overbright");
            b.body(&m.build);
            b.gap(6.0);
            match &m.source {
                Some(url) => b.body(&format!("Source code: {url}")),
                None => b.status(Mark::Warn, UNOFFICIAL),
            }
            b.gap(16.0);
            b.body(FREE);
            b.gap(16.0);
            b.body(OWN_COPY);
            b.gap(16.0);
            b.dim(EA_DISCLAIMER);
            b.gap(18.0);
            b.buttons(
                Align::Left,
                vec![Ctrl::button("licence", "Licence (GPL-3.0)"), Ctrl::button("notices", "Third-party notices")],
            );
            b.gap(18.0);
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
            b.footer(vec![Ctrl::primary("back", "Back")]);
            b.default_focus = Some("back");
        }
        Screen::Licence | Screen::Notices => {
            b.buttons(Align::Left, vec![Ctrl::primary("back", "Back")]);
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
        Screen::Failure => {
            let f = m.failure.as_ref();
            b.heading(if f.is_some_and(|f| f.during_start) {
                "The game stopped during its start"
            } else {
                "The game stopped"
            });
            b.body(&format!("It ended with {}.", f.map_or("an unknown result", |f| f.how.as_str())));
            b.gap(10.0);
            let tail: Vec<String> =
                f.map(|f| f.tail.iter().rev().take(TAIL_SHOWN).rev().cloned().collect()).unwrap_or_default();
            if tail.is_empty() {
                b.dim("It printed nothing.");
            } else {
                b.dim("Its last lines:");
                b.gap(6.0);
                b.panel(&tail);
            }
            b.gap(12.0);
            b.body("What you can try: update the graphics driver; set Display to Windowed or Graphics to Original, then Try again.");
            b.gap(8.0);
            let where_ = match f.and_then(|f| f.log.as_ref()) {
                Some(l) => format!("The whole log: {}", l.display()),
                None => format!("Logs folder: {}", m.logs_dir.display()),
            };
            b.dim(&where_);
            b.dim("When you ask for help, send this log file along.");
            b.gap(10.0);
            b.buttons(Align::Left, vec![Ctrl::button("open_logs", "Open log folder")]);
            message(b, m);
            b.footer(vec![
                Ctrl::button("quit", "Quit"),
                Ctrl::button("settings", "Settings"),
                Ctrl::primary("retry", "Try again"),
            ]);
            b.default_focus = Some("retry");
        }
        Screen::Browse => {
            let br = &m.browse;
            b.heading(match br.purpose {
                BrowseFor::Game => "Choose the game folder",
                BrowseFor::TexturePack => "Choose the texture-pack folder",
                BrowseFor::Saves => "Choose the original game's save folder",
                BrowseFor::Packages => "Add texture packages",
                BrowseFor::HudMod => "Add the HUD mod",
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
                _ => {
                    "A double click (or Enter, or Open) opens the folder chosen in the list; “Use this folder” takes \
                     the one shown above."
                }
            });
            message(b, m);
            let mut foot = vec![Ctrl::button("cancel", "Cancel")];
            if m.native_dialog {
                foot.push(Ctrl::button("system", "Windows dialog…"));
            }
            foot.push(Ctrl::Button { id: "open", text: "Open".into(), primary: false, enabled: !items.is_empty() });
            foot.push(Ctrl::Button {
                id: "use",
                text: if br.purpose == BrowseFor::Packages { "Import all here" } else { "Use this folder" }.into(),
                primary: true,
                enabled: !br.dir.as_os_str().is_empty(),
            });
            b.footer(foot);
        }
        Screen::Import => {
            b.heading("Import saves from the original game");
            b.body(
                "Copies your profiles from the original game's save folder into Overbright's own. \
                 The original's files are only read: nothing there is changed, moved or deleted.",
            );
            b.gap(12.0);
            b.dim("From:");
            match m.import.scan.as_ref() {
                Some(s) => line(b, &s.from.display().to_string()),
                None => line(b, "no folder chosen yet"),
            }
            b.gap(6.0);
            b.buttons(Align::Left, vec![Ctrl::button("pick", "Choose folder…")]);
            b.gap(8.0);
            b.dim("To (Overbright's save folder):");
            line(b, &m.saves_dir.display().to_string());
            b.gap(12.0);
            let mut copy_n = 0;
            if let Some(s) = &m.import.scan {
                let lines = s.lines();
                if lines.is_empty() {
                    b.status(
                        Mark::Info,
                        "No saves in this folder. A save is a folder holding a file of the same name (NIGHTOWL/NIGHTOWL).",
                    );
                }
                for (new, l) in lines {
                    b.status(if new { Mark::Ok } else { Mark::Info }, &l);
                }
                copy_n = s.to_copy().count();
            }
            if let Some(done) = &m.import.done {
                b.gap(4.0);
                if done.is_empty() {
                    b.status(Mark::Info, "Nothing was copied.");
                } else {
                    b.status(
                        Mark::Ok,
                        &format!(
                            "Copied {} {}: {}",
                            done.len(),
                            if done.len() == 1 { "file" } else { "files" },
                            done.join(", ")
                        ),
                    );
                }
                copy_n = 0;
            }
            message(b, m);
            b.footer(vec![
                Ctrl::button("back", "Back"),
                Ctrl::Button {
                    id: "copy",
                    text: if copy_n == 1 { "Copy 1 save".into() } else { format!("Copy {copy_n} saves") },
                    primary: true,
                    enabled: copy_n > 0,
                },
            ]);
            b.default_focus = Some(if copy_n > 0 { "copy" } else { "pick" });
        }
        Screen::Mods => build_mods(b, m),
        Screen::Prepare => build_prepare(b, m),
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
            b.footer(vec![Ctrl::button("quit", "Quit")]);
        }
    }
}

/// What to do about a folder Overbright cannot use, by what is wrong with
/// it (the check's lines).
pub fn advice(c: &super::model::CheckResult) -> &'static str {
    let errors = || c.lines.iter().filter(|(m, _)| *m == Mark::Error).map(|(_, t)| t.as_str());
    if errors().any(|t| t.contains("not a complete game folder")) {
        "Overbright cannot use this folder: the game's files are not here. Choose the folder the game was \
         installed to: the one that holds the GLOBAL, FRONTEND and TRACKS folders."
    } else if errors().any(|t| t.starts_with("Missing")) {
        "Overbright cannot use this folder: some of the game's files are missing. Reinstall the game or copy \
         the whole game folder again, then choose it."
    } else {
        "Overbright cannot use this folder. Choose a complete install of the game (PC, version 1.2)."
    }
}

/// The game's state on the main screen, beside Play: its mark and words.
pub fn game_state(m: &Model) -> (Mark, String) {
    match (&m.root, &m.probe) {
        (None, _) => (Mark::Warn, "Not found yet: choose the game's folder".into()),
        (Some(_), ProbeState::Pending) => (Mark::Info, "Found · PC version 1.2 · checking this computer…".into()),
        (Some(_), ProbeState::Ready(p)) if !p.ok() => {
            (Mark::Warn, "Found · PC version 1.2 · no graphics adapter answered (see below)".into())
        }
        (Some(_), ProbeState::Failed(_)) => {
            (Mark::Warn, "Found · PC version 1.2 · this computer was not checked".into())
        }
        (Some(_), _) => match &m.prepare.state {
            // The prepare step running by itself (`Prepare::background`).
            super::prepare::State::Idle | super::prepare::State::Running { .. }
                if m.prepare.background =>
            {
                (Mark::Info, m.prepare.state.background_words())
            }
            _ => (Mark::Ok, "Found · PC version 1.2 · ready to play".into()),
        },
    }
}

/// The game's Skip Intro Movies and Skip Title Screen rows (Enhancements
/// page, options.txt `enh.skip_movies` / `enh.skip_title`): independent.
fn game_skip_boxes(b: &mut Builder, c: &super::model::Choices) {
    b.checkboxes(&[
        ("skip_movies", "Skip the intro movies", c.skip_movies),
        ("skip_title", "Skip the title screen", c.skip_title),
    ]);
}

/// The launcher's own start choice.
fn launcher_skip_box(b: &mut Builder, c: &super::model::Choices) {
    b.checkbox("skip", "Start the game directly next time (hold Shift while starting to see this window)", c.skip);
}

/// The main screen: the game and Play (the hero), the start settings and
/// Mods in their own panels, the computer's facts as a quiet strip, the
/// rarely used pages as small buttons at the bottom.
fn build_main(b: &mut Builder, m: &Model) {
    let c = &m.choices;
    // The hero: the game, its state and folder; Play beside it.
    let hero = b.card_begin();
    let (x0, x1) = (b.x0, b.x1);
    let narrow = b.width() < b.m.s(560.0);
    let play_w = if narrow { b.width() } else { b.m.s(220.0).min(b.width() * 0.4) };
    let play_h = b.m.s(72.0);
    if !narrow {
        b.x1 = x1 - play_w - b.m.s(24.0);
    }
    let top = b.y;
    let hs = b.m.heading();
    b.para("Need for Speed Underground 2", Face::Bold, hs, theme::TEXT);
    b.gap(6.0);
    let (mark, state) = game_state(m);
    b.status(mark, &state);
    let root = m.root.as_ref().map_or("No game folder chosen".into(), |r| r.display().to_string());
    b.buttons(
        Align::Left,
        vec![Ctrl::path(root), Ctrl::button("change", if m.root.is_some() { "Change…" } else { "Find game…" })],
    );
    let left_bottom = b.y;
    b.x1 = x1;
    let r = if narrow {
        b.gap(12.0);
        Rect::new(x0, b.y, play_w, play_h)
    } else {
        Rect::new(x1 - play_w, top + ((left_bottom - top - play_h) / 2.0).max(0.0).round(), play_w, play_h)
    };
    b.big_button("play", "Play", r);
    b.y = left_bottom.max(r.bottom());
    b.card_end(hero);
    b.default_focus = Some("play");

    // The start settings and Mods: side by side when wide, else stacked.
    let wide = b.width() >= b.m.s(760.0);
    let (cx0, cx1) = (b.x0, b.x1);
    let gap = b.m.s(super::layout::CARD_GAP);
    let split = if wide { cx0 + ((cx1 - cx0 - gap) * 0.62).round() } else { cx1 };
    let row_y = b.y;
    b.x1 = split;
    let set = b.card_begin();
    b.section("Start settings");
    b.label_column(&["Display", "Resolution", "Graphics"]);
    let on = display_rows_on(m);
    let modes: Vec<String> = m.modes().iter().map(|&r| res_text(m, r)).collect();
    b.row(
        "Display",
        vec![Ctrl::Cycle {
            id: "display",
            value: if c.fullscreen { "Full screen" } else { "Windowed" }.into(),
            values: vec!["Full screen".into(), "Windowed".into()],
            enabled: on,
        }],
    );
    b.row(
        "Resolution",
        vec![Ctrl::Cycle { id: "resolution", value: res_text(m, c.resolution), values: modes, enabled: on }],
    );
    let mut gvals: Vec<String> = GRAPHICS.iter().map(|g| g.label()).collect();
    gvals.push(c.graphics.label());
    b.row(
        "Graphics",
        vec![
            Ctrl::Cycle { id: "graphics", value: c.graphics.label(), values: gvals, enabled: on },
            Ctrl::dim(match c.graphics {
                Graphics::Original => "Exactly the 2004 game",
                Graphics::Enhanced => "Enhanced, quality Low; Medium, High and every row in the game's Graphics page",
                _ => "Set in the game's Graphics page; pick one here to replace it",
            }),
        ],
    );
    let (gpu, gpu_col) = gpu_text(m);
    if !on && gpu_col == theme::ERROR && matches!(m.probe, ProbeState::Ready(_)) {
        // The probe ran and found no adapter: the game cannot start until
        // a driver is installed (the line says which).
        b.status(Mark::Error, &format!("Graphics card: {gpu}"));
    } else if !on && gpu_col == theme::ERROR {
        b.status(Mark::Error, &format!("Graphics card: {gpu} (display and graphics: in the game's Options)"));
    } else if !on {
        b.status(
            Mark::Info,
            match &m.probe {
                ProbeState::Pending => "Display and graphics: available once this computer is checked.",
                ProbeState::Ready(p) if p.ok() && m.root.is_none() => {
                    "Display and graphics: available once the game folder is chosen."
                }
                _ => "Display and graphics: set them in the game's Options after its first start.",
            },
        );
    }
    // The game's skip boxes, under Graphics: like the display rows they
    // are options.txt values, which cannot be written before the game's
    // first run values are known (`SaveError::FirstRunPending`), so they
    // are shown only once they can be.
    if on {
        game_skip_boxes(b, c);
    }
    if !wide {
        launcher_skip_box(b, c);
    }
    b.card_end(set);
    let set_bottom = b.y;
    if wide {
        b.x0 = split + gap;
        b.x1 = cx1;
        b.y = row_y;
    } else {
        b.x0 = cx0;
        b.x1 = cx1;
    }
    let mods = b.card_begin();
    b.section("Mods");
    b.dim(&mods_summary(m));
    let on: Vec<&str> = m.mods.list.iter().filter(|(e, _)| e.on).map(|(_, i)| i.name.as_str()).take(3).collect();
    for n in &on {
        b.gap(4.0);
        let size = b.m.small();
        let t = b.fonts.fit(Face::Regular, size, &format!("· {n}"), b.width(), super::font::Cut::End);
        b.para(&t, Face::Regular, size, theme::TEXT);
    }
    b.gap(10.0);
    b.buttons(Align::Left, vec![Ctrl::button("mods", "Mods…")]);
    b.card_end(mods);
    // Wide: the start choice under Mods, the right column's second card.
    let next = wide.then(|| {
        let i = b.card_begin();
        launcher_skip_box(b, c);
        b.card_end(i);
        i
    });
    // Both panels end on the same line when side by side.
    // The right column's last card ends where the settings do.
    let last = next.unwrap_or(mods);
    if wide {
        let bottom = b.y.max(set_bottom);
        let g = b.m.s(super::layout::CARD_GAP);
        b.widgets[set].rect.h = bottom - g - b.widgets[set].rect.y;
        b.widgets[last].rect.h = bottom - g - b.widgets[last].rect.y;
        b.y = bottom;
    }
    b.x0 = cx0;
    b.x1 = cx1;

    // This computer: a quiet strip.
    let others = other_languages(&m.languages);
    let lang = if others.is_empty() {
        "English".to_string()
    } else {
        format!("English (also installed, not supported yet: {})", others.join(", "))
    };
    let pad = m.controller.clone().unwrap_or_else(|| "No controller (keyboard and mouse work)".into());
    let strip_from = b.widgets.len();
    let strip_text = format!("Language: {lang}   ·   Graphics card: {gpu}   ·   Controller: {pad}");
    let size = b.m.small();
    if gpu_col == theme::ERROR && !display_rows_on(m) {
        b.para(&format!("Language: {lang}   ·   Controller: {pad}"), Face::Regular, size, theme::PAGE_DIM);
    } else if gpu_col == theme::ERROR {
        b.status(Mark::Error, &format!("Graphics card: {gpu}"));
        b.para(&format!("Language: {lang}   ·   Controller: {pad}"), Face::Regular, size, theme::PAGE_DIM);
    } else {
        b.para(&strip_text, Face::Regular, size, theme::PAGE_DIM);
    }
    if let Some((mark, text)) = &m.message {
        b.gap(6.0);
        b.status(*mark, text);
    }
    let strip_end = b.widgets.len();
    let content_bottom = b.y;
    b.footer_split(
        vec![
            Ctrl::button("about", "About"),
            Ctrl::button("import", "Import saves…"),
            Ctrl::Button {
                id: "prepare",
                text: "Prepare again".into(),
                primary: false,
                enabled: m.root.is_some(),
            },
        ],
        vec![Ctrl::button("quit", "Quit")],
    );
    // No dead half-screen: the panels grow to the footer, the strip moves
    // down with them.
    let footer_top = b.widgets[strip_end..].iter().map(|w| w.rect.y).fold(f32::INFINITY, f32::min);
    let free = (footer_top - b.m.s(12.0) - content_bottom).floor();
    if free > 0.0 && footer_top.is_finite() {
        b.widgets[set].rect.h += free;
        if wide {
            b.widgets[last].rect.h += free;
        } else {
            for w in &mut b.widgets[mods..strip_from] {
                w.rect.y += free;
                for t in &mut w.texts {
                    t.rect.y += free;
                }
            }
        }
        for w in &mut b.widgets[strip_from..strip_end] {
            w.rect.y += free;
            for t in &mut w.texts {
                t.rect.y += free;
            }
        }
    }
}

/// The main screen's Mods line: what is on.
/// The prepare step's lines.
pub const PREPARE_WHY: &str = "Overbright can build its shader programs and read your game's city once now, \
so the first drive starts without a loading screen. It takes from a few seconds to about a minute. \
You can skip it: the game then does the same work during its first loads.";

/// The prepare step (`game --prepare`, docs/prepare.md): what it is for, the
/// bar and the step under it while it runs, how it ended; Skip while it
/// runs, Continue after.
fn build_prepare(b: &mut Builder, m: &Model) {
    use super::prepare::State;
    b.heading("Getting the game ready");
    b.body(PREPARE_WHY);
    b.gap(16.0);
    let state = &m.prepare.state;
    match state {
        State::Idle => line(b, "Starting…"),
        State::Running { .. } => {
            b.bar(state.fraction());
            line(b, &state.words());
        }
        State::Done => {
            b.bar(1.0);
            b.status(Mark::Ok, &state.words());
        }
        State::Failed(_) => b.status(Mark::Warn, &state.words()),
    }
    message(b, m);
    let running = matches!(state, State::Idle | State::Running { .. });
    b.footer(vec![if running {
        Ctrl::button("skip", "Skip")
    } else {
        Ctrl::Button { id: "continue", text: "Continue".into(), primary: true, enabled: true }
    }]);
    b.default_focus = Some(if running { "skip" } else { "continue" });
}

pub fn mods_summary(m: &Model) -> String {
    let on = m.mods.list.iter().filter(|(e, _)| e.on).count();
    let hud = m.choices.hud_mod && !m.choices.texture_pack.trim().is_empty();
    match (on, hud) {
        (0, false) => "None: the game's own textures".into(),
        (0, true) => "HUD mod on".into(),
        (n, h) => format!(
            "{n} texture {} on{}",
            if n == 1 { "package" } else { "packages" },
            if h { ", HUD mod on" } else { "" }
        ),
    }
}

/// The Mods page's one sentence on what can be added.
pub const MODS_INTRO: &str = "Overbright ships no texture art. Add mods you have: TexMod packages (.tpf), TexWizard \
packs (folder or .zip) and the HD HUD mod. They are copied into Overbright's folder; your files stay unchanged. Mods \
belong to their authors.";

/// The HUD mod section's sentence on what it takes.
pub const HUD_INTRO: &str = "Not set up: the game shows its own HUD and maps. Add HUD mod… takes the downloads.";

fn mb(bytes: u64) -> String {
    if bytes >= 10_000_000 { format!("{} MB", bytes / 1_000_000) } else { format!("{:.1} MB", bytes as f64 / 1e6) }
}

fn build_mods(b: &mut Builder, m: &Model) {
    b.heading("Mods");
    b.dim(MODS_INTRO);
    b.gap(4.0);
    let size = b.m.body();
    b.para("Texture packages (the first in the list wins)", Face::Bold, size, theme::TEXT);
    b.gap(6.0);
    let mods = &m.mods;
    let busy = mods.busy.is_some();
    if mods.list.is_empty() {
        b.dim("None added: the game shows its own textures.");
        b.gap(4.0);
    } else {
        let items: Vec<(String, String)> = mods
            .list
            .iter()
            .map(|(e, i)| {
                let state = if e.on { "on" } else { "off" };
                (format!("{}  ({state})", i.name), format!("{} textures, {}", i.files, mb(i.bytes)))
            })
            .collect();
        b.list("pkgs", &items, mods.sel.min(items.len() - 1), items.len().clamp(1, 3));
        b.default_focus = Some("pkgs");
    }
    b.gap(6.0);
    let has = !mods.list.is_empty() && !busy;
    let sel = mods.sel.min(mods.list.len().saturating_sub(1));
    let on = mods.list.get(sel).is_some_and(|(e, _)| e.on);
    let btn = |id, text: &str, enabled| Ctrl::Button { id, text: text.into(), primary: false, enabled };
    b.buttons(
        Align::Left,
        vec![
            btn("add", "Add package…", !busy),
            btn("toggle", if on { "Turn off" } else { "Turn on" }, has),
            btn("up", "Move up", has && sel > 0),
            btn("down", "Move down", has && sel + 1 < mods.list.len()),
            btn("remove", "Remove…", has),
        ],
    );
    if mods.list.is_empty() && !busy {
        b.default_focus = Some("add");
    }
    if let Some(id) = &mods.confirm {
        let name = mods.list.iter().find(|(e, _)| &e.id == id).map_or(id.as_str(), |(_, i)| i.name.as_str());
        b.gap(8.0);
        b.status(
            Mark::Warn,
            &format!(
                "Remove “{name}”? Its converted copy in Overbright's folder is deleted; your package file is kept."
            ),
        );
        b.buttons(Align::Left, vec![Ctrl::primary("remove_yes", "Remove it"), Ctrl::button("keep", "Keep it")]);
        b.default_focus = Some("keep");
    }
    if let Some(t) = &mods.busy {
        b.gap(8.0);
        b.status(Mark::Info, t);
    }
    for (mark, line) in &mods.results {
        b.status(*mark, line);
    }
    b.gap(2.0);
    b.rule();
    b.gap(6.0);
    b.para("HUD mod", Face::Bold, size, theme::TEXT);
    b.gap(6.0);
    let dir = m.choices.texture_pack.trim();
    if dir.is_empty() {
        b.dim(HUD_INTRO);
    } else {
        line(b, dir);
        b.gap(4.0);
        match &m.hud {
            Some(h) if h.complete => {
                b.status(Mark::Ok, &format!("Complete: {} of {} files, maps calibrated", h.present, h.listed))
            }
            Some(h) if h.is_pack => {
                let mut t = format!("Incomplete: {} of {} files", h.present, h.listed);
                if !h.missing.is_empty() {
                    t += &format!("; missing {}", h.missing.join(", "));
                    if h.listed - h.present > h.missing.len() {
                        t += ", …";
                    }
                }
                if !h.maps {
                    t += "; the map calibration is missing";
                }
                b.status(Mark::Warn, &(t + ". The game shows its own HUD until it is complete."));
            }
            Some(_) => {
                b.status(Mark::Error, "Not a HUD mod folder: it has no hd-ui-pack.list (the converted mod's index).")
            }
            None => {}
        }
    }
    if let Some(t) = &mods.hud_busy {
        b.gap(4.0);
        b.status(Mark::Info, t);
    }
    for (mark, line) in &mods.hud_results {
        b.status(*mark, line);
    }
    b.checkbox("hud", "Use the HUD mod (the game's HD UI + Maps setting)", m.choices.hud_mod);
    b.gap(4.0);
    let hud_busy = mods.hud_busy.is_some();
    b.buttons(
        Align::Left,
        vec![
            Ctrl::Button { id: "hud_add", text: "Add HUD mod…".into(), primary: false, enabled: !hud_busy },
            Ctrl::button("pack_browse", "Choose folder…"),
            Ctrl::Button { id: "pack_forget", text: "Forget folder".into(), primary: false, enabled: !dir.is_empty() },
        ],
    );
    message(b, m);
    b.footer(vec![Ctrl::primary("back", "Done")]);
    if b.default_focus.is_none() {
        b.default_focus = Some("back");
    }
}

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
        let mut a =
            App { env, model, screen, st: UiState::default(), size, dpi, ui: Ui::default(), back: Screen::Main };
        a.rebuild(fonts);
        a
    }

    pub fn rebuild(&mut self, fonts: &Fonts) {
        let (m, s) = (&self.model, self.screen);
        // The state as it was: a second pass (the list grown) starts from
        // it, not from what the first pass's shorter list scrolled to.
        let before = (s == Screen::Browse).then(|| self.st.clone());
        self.ui = build_fit(fonts, self.size.0, self.size.1, self.dpi, !s.document(), &mut self.st, |b| build(b, m, s));
        // The folder browser's list takes the room the window leaves.
        if s == Screen::Browse
            && let Some(super::ui::Kind::List { row_h, .. }) = self.ui.get("dirs").map(|w| &w.kind)
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

    pub fn go(&mut self, s: Screen) {
        self.screen = s;
        let pointer = self.st.pointer;
        self.st = UiState { pointer, ..UiState::default() };
        self.model.message = None;
    }

    /// One input: the events it makes, acted on; the screen rebuilt.
    pub fn input(&mut self, input: &Input, fonts: &Fonts) -> Outcome {
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
        let events = handle(&self.ui, &mut self.st, input);
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

    fn step<T: PartialEq + Clone>(list: &[T], v: &T, d: i32) -> T {
        let n = list.len() as i32;
        match list.iter().position(|x| x == v) {
            Some(i) => list[((i as i32 + d).rem_euclid(n)) as usize].clone(),
            None => list[if d > 0 { 0 } else { (n - 1) as usize }].clone(),
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
                if b.purpose == BrowseFor::HudMod {
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
    fn use_folder(&mut self, dir: PathBuf) {
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
            BrowseFor::Saves => {
                self.model.import.scan = Some(self.env.scan_saves(&dir));
                self.model.import.done = None;
                self.go(Screen::Import);
            }
        }
    }

    /// Where the save browser starts: inside the Wine prefix of the game
    /// folder in use (`drive_c/users`), else the home folder.
    fn saves_start(&self) -> Option<PathBuf> {
        let root = self.model.root.as_ref()?;
        let mut p = root.as_path();
        while let Some(parent) = p.parent() {
            if p.file_name().is_some_and(|n| n == "drive_c") {
                return Some(p.join("users"));
            }
            p = parent;
        }
        None
    }

    /// A browser row is a package file (listed by [`Env::list_packages`]).
    fn is_package_row(&self, p: &Path) -> bool {
        self.model.browse.entries.iter().any(|e| {
            e.path == p && (e.note.starts_with("TexMod package") || e.note.starts_with("TexWizard pack"))
        })
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

    /// An import's results shown and the list read again.
    pub fn finish_import(&mut self, out: Vec<crate::texture_mods::Outcome>) {
        self.model.mods.busy = None;
        self.model.mods.results =
            out.into_iter().map(|o| (if o.ok { Mark::Ok } else { Mark::Error }, o.line)).collect();
        self.model.mods.list = self.env.mods();
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
            (Screen::Browse, Some(BrowseFor::TexturePack | BrowseFor::Saves)) => self.use_folder(folder(&first)),
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
        match (self.screen, e) {
            (_, Event::Activate("quit")) => return self.quit(),
            (Screen::FirstRun, Event::Back) => return Outcome::Quit,
            (Screen::FirstRun, Event::Select("found", i)) => self.model.found_sel = *i,
            (Screen::FirstRun, Event::Edit("path", ed)) => {
                apply_edit(&mut self.model.path_input, &mut self.st.cursor, ed)
            }
            (Screen::FirstRun, Event::Activate("browse")) => {
                let start = self.model.found.get(self.model.found_sel).map(|f| f.path.clone());
                self.browse(BrowseFor::Game, start.and_then(|p| p.parent().map(Path::to_path_buf)));
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
                self.browse(BrowseFor::Game, start);
            }
            (Screen::Check, Event::Activate("use")) => {
                if let Some(c) = self.model.check.clone().filter(|c| c.usable) {
                    match self.env.keep_root(&c.root) {
                        Ok(()) => {
                            self.model.languages = self.env.languages(&c.root);
                            self.model.root = Some(c.root);
                            self.go(Screen::Main);
                        }
                        Err(err) => {
                            self.model.message = Some((Mark::Error, format!("The folder could not be kept: {err}")))
                        }
                    }
                }
            }
            (Screen::Main, Event::Activate("change")) => self.go(Screen::FirstRun),
            (Screen::Main, Event::Back) => return self.quit(),
            (Screen::Main, Event::Step(id @ ("display" | "resolution" | "graphics"), _))
                if !display_rows_on(&self.model) =>
            {
                let _ = id;
            }
            (Screen::Main, Event::Step("display", _)) => {
                // The size moves to one the other path offers, as the
                // game's Screen Mode row does (`display::fit_mode`).
                let m = &mut self.model;
                m.choices.fullscreen = !m.choices.fullscreen;
                m.choices.resolution = crate::render::display::fit_mode(&m.modes(), m.choices.resolution);
            }
            (Screen::Main, Event::Step("resolution", d)) => {
                let m = &mut self.model;
                let modes = m.modes();
                if !modes.is_empty() {
                    m.choices.resolution = Self::step(&modes, &m.choices.resolution, *d);
                }
            }
            (Screen::Main, Event::Step("graphics", d)) => {
                self.model.choices.graphics = Self::step(&GRAPHICS, &self.model.choices.graphics, *d)
            }
            (Screen::Main, Event::Toggle("skip")) => self.model.choices.skip = !self.model.choices.skip,
            (Screen::Main, Event::Toggle("skip_movies")) => {
                self.model.choices.skip_movies = !self.model.choices.skip_movies
            }
            (Screen::Main, Event::Toggle("skip_title")) => {
                self.model.choices.skip_title = !self.model.choices.skip_title
            }
            (Screen::Main, Event::Activate("mods")) => {
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
                self.browse(BrowseFor::TexturePack, start);
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
            (Screen::Mods, Event::Activate("up")) => self.edit_mods(|l, i| l.swap(i, i - 1), -1),
            (Screen::Mods, Event::Activate("down")) => self.edit_mods(|l, i| l.swap(i, i + 1), 1),
            (Screen::Mods, Event::Activate("remove")) => {
                self.model.mods.confirm = self.model.mods.list.get(self.model.mods.sel).map(|(e, _)| e.id.clone());
                // The question's own buttons take the focus ("Keep it").
                self.st.focus = None;
            }
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
            (Screen::Main, Event::Activate("about")) => {
                self.back = Screen::Main;
                self.go(Screen::About);
            }
            (Screen::Main, Event::Activate("import")) => {
                self.model.import.done = None;
                if self.model.import.scan.is_none()
                    && let Some(d) = self.env.original_saves().filter(|d| d.is_dir())
                {
                    self.model.import.scan = Some(self.env.scan_saves(&d));
                }
                self.go(Screen::Import);
            }
            (Screen::Main, Event::Activate("play")) => {
                if self.save() {
                    return Outcome::Play;
                }
            }
            (Screen::About, Event::Activate("back") | Event::Back) => self.go(Screen::Main),
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
            (Screen::Failure, Event::Activate("settings") | Event::Back) => self.go(Screen::Main),
            (Screen::Failure, Event::Activate("retry")) => return Outcome::Play,
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
                let title = match self.model.browse.purpose {
                    BrowseFor::Game => "Choose the game folder",
                    BrowseFor::TexturePack => "Choose the texture-pack folder",
                    BrowseFor::Saves => "Choose the original game's save folder",
                    BrowseFor::Packages => "Choose the folder that holds the texture packages",
                    BrowseFor::HudMod => "Choose the HUD mod's folder",
                };
                if let Some(d) = self.env.native_folder_dialog(title) {
                    self.use_folder(d);
                }
            }
            (Screen::Browse, Event::Activate("use")) => {
                let d = self.model.browse.dir.clone();
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
            (Screen::Main, Event::Activate("prepare")) => self.start_prepare(),
            (Screen::Import, Event::Activate("back") | Event::Back) => self.go(Screen::Main),
            (Screen::Import, Event::Activate("pick")) => {
                let start = self
                    .model
                    .import
                    .scan
                    .as_ref()
                    .map(|s| s.from.clone())
                    .or_else(|| self.env.original_saves())
                    .or_else(|| self.saves_start());
                self.browse(BrowseFor::Saves, start);
            }
            (Screen::Import, Event::Activate("copy")) => {
                if let Some(s) = self.model.import.scan.clone() {
                    match self.env.import_saves(&s) {
                        Ok(done) => {
                            self.model.import.done = Some(done);
                            self.model.import.scan = Some(self.env.scan_saves(&s.from));
                        }
                        Err(e) => {
                            self.model.message = Some((Mark::Error, format!("The saves could not be copied: {e}")))
                        }
                    }
                }
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
        if !matches!(self.screen, Screen::Main | Screen::Failure) {
            return true;
        }
        if m.choices.texture_pack != m.saved.texture_pack {
            m.pack_status = self.env.set_pack(&m.choices.texture_pack);
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
