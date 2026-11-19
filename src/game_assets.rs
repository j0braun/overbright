//! Which game file is opened for a wanted name (clone tooling; the movie
//! fallback is a documented deviation, docs/playing/native-deviations.md "Movie
//! files").
//!
//! One resolver for the game and for the game-folder check: the movie
//! screen opens what [`resolve_movie`] gives, and the check reports the
//! same [`Resolved`], so the report can never disagree with what plays.
//!
//! Editions name their movies in different ways: the reference executable
//! builds `<name>_fr.vp6` for most names ([`crate::movie::file_name`]);
//! the European data has `<name>_en.vp6` (plus some logos in more
//! languages); the US 1.2 English edition has no suffix at all
//! (`Scene01.vp6`, `ealogo.vp6`, with a `movies.tag`); a single-language
//! edition carries its own suffix. Names match without case, as Win32
//! opens them ([`crate::game_fs`]).

use std::path::{Path, PathBuf};

/// The language whose text file the game loads (`LANGUAGES/English.bin`;
/// the clone loads English only, `frontend::assets`).
pub const TEXT_LANGUAGE: &str = "English";

/// The text file the game loads ([`TEXT_LANGUAGE`]): strings (chunk
/// 0x39000) and fonts (chunk 0x30201).
pub const TEXT_FILE: &str = "LANGUAGES/English.bin";

/// Language names and their movie suffixes. Seen in the data: `_en`, the
/// logos' `_dn _du _sw` (reference MOVIES) and `_fr _gr _it _sp` (the
/// movie table in GLOBALB.BUN, `ealogo_<xx>`); `_ru` is **inferred**.
pub const SUFFIXES: [(&str, &str); 9] = [
    ("English", "en"),
    ("French", "fr"),
    ("German", "gr"),
    ("Italian", "it"),
    ("Spanish", "sp"),
    ("Danish", "dn"),
    ("Dutch", "du"),
    ("Swedish", "sw"),
    ("Russian", "ru"),
];

/// The movie suffix of a language (by name, any case).
pub fn language_suffix(language: &str) -> Option<&'static str> {
    SUFFIXES.iter().find(|(l, _)| l.eq_ignore_ascii_case(language)).map(|(_, s)| *s)
}

/// The language of a movie suffix (`en` → English), any case.
pub fn suffix_language(suffix: &str) -> Option<&'static str> {
    SUFFIXES.iter().find(|(_, s)| s.eq_ignore_ascii_case(suffix)).map(|(l, _)| *l)
}

/// How a wanted file was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum How {
    /// The name the executable builds (native).
    Built,
    /// `<name>.vp6` without a language suffix (the US edition's names).
    Plain,
    /// The text language's or English's file (the suffix, `en`).
    Language(String),
    /// Another language's file, the only one there (the suffix).
    Other(String),
    /// No file: the game skips it.
    Missing,
}

impl How {
    /// Whether the file is not the one the executable names.
    pub fn fallback(&self) -> bool {
        matches!(self, How::Plain | How::Language(_) | How::Other(_))
    }
}

/// A wanted file and what the game opens for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The name asked for (`scene01`, `THX_logo_fr`).
    pub want: String,
    /// The file opened, `None` when there is none.
    pub file: Option<PathBuf>,
    pub how: How,
    /// The names tried, in order (`<name>_*.vp6` for the last step).
    pub tried: Vec<String>,
}

impl Resolved {
    /// The file's name as it is in the folder, if found.
    pub fn file_name(&self) -> Option<String> {
        self.file.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned())
    }
}

/// A movie name without a language suffix the executable put in it
/// (`THX_logo_fr` → `THX_logo`; `Scene01` stays).
pub fn movie_stem(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((stem, s)) if !stem.is_empty() && SUFFIXES.iter().any(|(_, x)| x.eq_ignore_ascii_case(s)) => stem,
        _ => name,
    }
}

/// The MOVIES folder (any case), if there is one.
pub fn movies_dir(root: &Path) -> Option<PathBuf> {
    crate::game_fs::find(root, "MOVIES").filter(|p| p.is_dir())
}

/// Whether `file` is `<stem>_<xx>.vp6` (any case; `xx` 1–4 letters or
/// digits): its suffix.
pub fn suffix_of<'a>(file: &'a str, stem: &str) -> Option<&'a str> {
    let n = stem.len();
    let lower = file.to_ascii_lowercase();
    if file.len() < n + 6 || !lower.ends_with(".vp6") || !lower.starts_with(&stem.to_ascii_lowercase()) {
        return None;
    }
    let rest = file.get(n..file.len() - 4)?;
    let s = rest.strip_prefix('_')?;
    (!s.is_empty() && s.len() <= 4 && s.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(s)
}

/// The movie file the game opens for `name`, case-insensitively, in this
/// order:
///
/// 1. the name the executable builds ([`crate::movie::file_name`];
///    native: 0x566250 asks whether it exists and plays it);
/// 2. `<stem>.vp6`, no suffix (the US edition; THX_logo.vp6 for
///    THX_logo_fr);
/// 3. `<stem>_<xx>.vp6` of the text language ([`TEXT_LANGUAGE`]);
/// 4. `<stem>_en.vp6`;
/// 5. any `<stem>_<xx>.vp6` (sorted without case, the first): another
///    edition's single language;
///
/// none: [`How::Missing`], the screen ends at once as in the original.
/// Steps 2–5 are the deviation (docs/playing/native-deviations.md, "Movie files").
pub fn resolve_movie(root: &Path, name: &str) -> Resolved {
    let stem = movie_stem(name);
    let mut steps: Vec<(String, How)> =
        vec![(crate::movie::file_name(name), How::Built), (format!("{stem}.vp6"), How::Plain)];
    for s in language_suffix(TEXT_LANGUAGE).into_iter().chain(["en"]) {
        steps.push((format!("{stem}_{s}.vp6"), How::Language(s.to_string())));
    }
    let mut tried: Vec<String> = Vec::new();
    for (file, how) in steps {
        if tried.iter().any(|t| t.eq_ignore_ascii_case(&file)) {
            continue;
        }
        let found = crate::game_fs::find(root, &format!("MOVIES/{file}")).filter(|p| p.is_file());
        tried.push(file);
        if let Some(p) = found {
            return Resolved { want: name.to_string(), file: Some(p), how, tried };
        }
    }
    tried.push(format!("{stem}_*.vp6"));
    // (lower-case name, suffix, path), the first by name.
    let any = movies_dir(root).and_then(|dir| {
        let mut v: Vec<(String, String, PathBuf)> = std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                let s = suffix_of(&n, stem)?.to_ascii_lowercase();
                Some((n.to_ascii_lowercase(), s, e.path()))
            })
            .collect();
        v.sort();
        v.into_iter().next()
    });
    match any {
        Some((_, s, p)) => Resolved { want: name.to_string(), file: Some(p), how: How::Other(s), tried },
        None => Resolved { want: name.to_string(), file: None, how: How::Missing, tried },
    }
}

/// A file found by its name only (any case): [`How::Built`] or
/// [`How::Missing`]. The text file and the menus' files.
pub fn resolve_file(root: &Path, rel: &str) -> Resolved {
    let file = crate::game_fs::find(root, rel).filter(|p| p.is_file());
    let how = if file.is_some() { How::Built } else { How::Missing };
    Resolved { want: rel.to_string(), file, how, tried: vec![rel.to_string()] }
}

// The check: what an install holds, in plain words (report only; nothing
// here changes what the game opens).

use crate::qol::setup::Mark;

/// The boot movies in their order (0x7F6620 via 0x4D9580), as the screens
/// ask for them.
pub const BOOT_MOVIES: [&str; 5] = ["blank", "ealogo", "THX_logo_fr", "PSA", "FMVOpening"];

/// The other movies the game asks for: the career's intro (scene01,
/// 0x53D720), the race dialog's and Return to Explore Mode's movies (the
/// career events' +0x02 and +0x04, Scene07 / Scene15 / Scene16 and
/// UpperClass, DriftTut after Scene20), the shops' first-entry movies
/// (SCENE05, SCENE06) and the neighbourhood flybys. The reference data's
/// MOVIES holds exactly these (a test checks the career's names against
/// the list).
pub const GAME_MOVIES: [&str; 24] = [
    "Scene01",
    "Scene05",
    "Scene06",
    "Scene07",
    "Scene08",
    "Scene09",
    "Scene10",
    "Scene11",
    "Scene12",
    "Scene13",
    "Scene14",
    "Scene15",
    "Scene16",
    "Scene17",
    "Scene18",
    "Scene20",
    "DragTut",
    "DriftTut",
    "OrganicsTut",
    "StreetXTut",
    "IndustEast",
    "IndustWest",
    "Suburban",
    "UpperClass",
];

/// A group of wanted files, in report order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Start,
    Story,
    Tutorials,
    Neighbourhoods,
    Text,
}

impl Group {
    pub fn words(self) -> &'static str {
        match self {
            Group::Start => "Start movies",
            Group::Story => "Story movies",
            Group::Tutorials => "Tutorial movies",
            Group::Neighbourhoods => "Neighbourhood movies",
            Group::Text => "Menu and text files",
        }
    }

    /// A movie's group by its name.
    pub fn of_movie(name: &str) -> Group {
        let l = name.to_ascii_lowercase();
        if BOOT_MOVIES.iter().any(|b| b.eq_ignore_ascii_case(name)) {
            Group::Start
        } else if l.ends_with("tut") {
            Group::Tutorials
        } else if l.starts_with("scene") {
            Group::Story
        } else {
            Group::Neighbourhoods
        }
    }
}

/// One wanted file: its group, the name shown and what the game opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub group: Group,
    pub label: String,
    pub resolved: Resolved,
}

/// How the movies are named in this install (from the files there).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    /// No MOVIES folder.
    NoMovies,
    /// No language suffix (the US edition; `movies.tag` there or not).
    Plain { tag: bool },
    /// One language's suffix (`ru`).
    Single(String),
    /// Several languages' suffixes (the European data: `en`, the logos in
    /// more), sorted.
    Multi(Vec<String>),
    /// Names without a suffix and with (the suffixes).
    Mixed(Vec<String>),
    /// No language movie at all.
    Unknown,
}

/// The edition as the files show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edition {
    pub layout: Layout,
    /// The language files in LANGUAGES (`qol::setup::installed_languages`).
    pub languages: Vec<String>,
    /// The suffix the player's SPEED2.EXE builds (`fr`), read only for the
    /// report; `None` when there is no executable or the name is not found
    /// (a protected or other build).
    pub exe_suffix: Option<String>,
}

/// A language's name for a suffix, or the suffix itself (`_xx`).
fn suffix_words(s: &str) -> String {
    suffix_language(s).map_or_else(|| format!("_{s}"), str::to_string)
}

impl Layout {
    /// The layout in a player's words.
    pub fn words(&self) -> String {
        let langs = |v: &[String]| v.iter().map(|s| suffix_words(s)).collect::<Vec<_>>().join(", ");
        match self {
            Layout::NoMovies => "no MOVIES folder".into(),
            Layout::Plain { tag: true } => "US layout (movies without a language suffix, movies.tag)".into(),
            Layout::Plain { tag: false } => "movies without a language suffix (as the US edition)".into(),
            Layout::Single(s) => format!("single-language layout ({} movies)", suffix_words(s)),
            Layout::Multi(v) => format!("European multi-language layout (movies in {})", langs(v)),
            Layout::Mixed(v) => format!("mixed layout (movies without a suffix and in {})", langs(v)),
            Layout::Unknown => "no language movies".into(),
        }
    }
}

/// The movie layout of `root` (from MOVIES' names: a name the executable
/// suffixes, with a two-letter suffix or none).
pub fn layout(root: &Path) -> Layout {
    let Some(dir) = movies_dir(root) else { return Layout::NoMovies };
    let names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let tag = names.iter().any(|n| n.eq_ignore_ascii_case("movies.tag"));
    let mut suffixes: Vec<String> = Vec::new();
    let mut plain = 0;
    for n in &names {
        let Some(stem) = n
            .len()
            .checked_sub(4)
            .filter(|&i| n.is_char_boundary(i) && n[i..].eq_ignore_ascii_case(".vp6"))
            .map(|i| &n[..i])
        else {
            continue;
        };
        match stem.rsplit_once('_') {
            Some((base, s)) if s.len() == 2 && s.bytes().all(|b| b.is_ascii_alphabetic()) => {
                // A language file of a name the executable suffixes
                // (THX_logo_en is the plain THX_logo's).
                if !crate::movie::file_name(base).eq_ignore_ascii_case(&format!("{base}.vp6")) {
                    suffixes.push(s.to_ascii_lowercase());
                }
            }
            _ if !crate::movie::file_name(stem).eq_ignore_ascii_case(&format!("{stem}.vp6")) => plain += 1,
            _ => {}
        }
    }
    suffixes.sort();
    suffixes.dedup();
    match (plain > 0, suffixes.len()) {
        (true, 0) => Layout::Plain { tag },
        (true, _) => Layout::Mixed(suffixes),
        (false, 0) => Layout::Unknown,
        (false, 1) => Layout::Single(suffixes.remove(0)),
        (false, _) => Layout::Multi(suffixes),
    }
}

/// The movie suffix an executable builds: the string after the name
/// format "%sMOVIES\%s%s" (0x79B1DC; "_fr.vp6" at 0x79B1EC in the
/// reference build). `None` when the format is not there or no `_xx.vp6`
/// follows it.
pub fn exe_movie_suffix(exe: &[u8]) -> Option<String> {
    const FORMAT: &[u8] = b"%sMOVIES\\%s%s\0";
    let at = exe.windows(FORMAT.len()).position(|w| w == FORMAT)? + FORMAT.len();
    let rest = exe.get(at..(at + 32).min(exe.len()))?;
    let start = rest.iter().position(|&b| b != 0)?;
    let s = &rest[start..];
    let end = s.iter().position(|&b| b == 0)?;
    let s = std::str::from_utf8(&s[..end]).ok()?;
    let suffix = s.strip_prefix('_')?.strip_suffix(".vp6")?;
    (suffix.len() == 2 && suffix.bytes().all(|b| b.is_ascii_alphabetic())).then(|| suffix.to_ascii_lowercase())
}

/// One group's counts ([`Report::group_counts`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCount {
    pub group: Group,
    /// The files the game asks for in this group.
    pub total: usize,
    /// Of them, found.
    pub found: usize,
    /// The names not found, in report order.
    pub missing: Vec<String>,
    /// Found under this install's own names, not the name the executable
    /// builds.
    pub own_names: usize,
}

/// What [`scan`] found: the edition and every wanted file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub edition: Edition,
    pub items: Vec<Item>,
}

/// The movies the game asks for, in report order, each once (any case).
pub fn wanted_movies() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for n in BOOT_MOVIES.iter().chain(GAME_MOVIES.iter()).copied() {
        if !v.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            v.push(n);
        }
    }
    v
}

/// The asset report's lines for `root` ([`scan`]), logged once per start
/// (after the fingerprint, `fingerprint::start`): the edition and the
/// movies as the game will find them (the same lines as `--check-game`).
pub fn log_report(root: &Path) {
    let a = scan(root);
    for (mark, line) in a.summary() {
        match mark {
            crate::qol::setup::Mark::Warn | crate::qol::setup::Mark::Error => log::warn!("game assets: {line}"),
            _ => log::info!("game assets: {line}"),
        }
    }
    for (_, line) in a.groups() {
        log::info!("game assets: {line}");
    }
}

/// Check `root` (reads only): the edition, and for every movie the game
/// asks for and every menu and text file what the game opens
/// ([`resolve_movie`], [`resolve_file`]: the game's own resolvers).
pub fn scan(root: &Path) -> Report {
    let mut items: Vec<Item> = wanted_movies()
        .into_iter()
        .map(|n| Item { group: Group::of_movie(n), label: movie_stem(n).to_string(), resolved: resolve_movie(root, n) })
        .collect();
    items.extend(crate::frontend::assets::ASSET_FILES.iter().map(|f| Item {
        group: Group::Text,
        label: f.to_string(),
        resolved: resolve_file(root, f),
    }));
    // Stable: the boot movies keep their order.
    items.sort_by_key(|i| i.group);
    let exe_suffix = crate::game_fs::find(root, crate::fingerprint::EXE)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| exe_movie_suffix(&b));
    let edition = Edition { layout: layout(root), languages: crate::qol::setup::installed_languages(root), exe_suffix };
    Report { edition, items }
}

/// How a found file reads in the lists.
fn how_words(h: &How) -> String {
    match h {
        How::Built | How::Missing => String::new(),
        How::Plain => " (no language suffix)".into(),
        How::Language(s) | How::Other(s) => format!(" ({} file)", suffix_words(s)),
    }
}

/// Up to `n` names, then "…".
fn names(v: &[&str], n: usize) -> String {
    let mut s = v.iter().take(n).copied().collect::<Vec<_>>().join(", ");
    if v.len() > n {
        s += ", …";
    }
    s
}

impl Report {
    fn movies(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| i.group != Group::Text)
    }

    /// The movies that will not play.
    pub fn missing_movies(&self) -> Vec<&str> {
        self.movies().filter(|i| i.resolved.file.is_none()).map(|i| i.label.as_str()).collect()
    }

    /// The language the movies play in, in words: the suffix most found
    /// movies have (none: "no suffix").
    pub fn movie_language(&self) -> Option<String> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for i in
            self.movies().filter(|i| i.group != Group::Neighbourhoods && i.label != "blank" && i.label != "FMVOpening")
        {
            let key = match &i.resolved.how {
                How::Missing => continue,
                How::Plain => "-".to_string(),
                How::Language(s) | How::Other(s) => s.clone(),
                How::Built => {
                    let name = i.resolved.file_name().unwrap_or_default();
                    let stem = movie_stem(&i.resolved.want);
                    suffix_of(&name, stem).map_or_else(|| "-".to_string(), str::to_ascii_lowercase)
                }
            };
            match counts.iter_mut().find(|c| c.0 == key) {
                Some(c) => c.1 += 1,
                None => counts.push((key, 1)),
            }
        }
        let best = counts.into_iter().max_by_key(|c| c.1)?.0;
        Some(if best == "-" { "no language suffix".to_string() } else { suffix_words(&best) })
    }

    /// The check's lines in plain words: the edition, the movies (all
    /// found, or which will not play) and missing menu files. Shared by
    /// `--check-game`, the launcher, the start log and the support file
    /// (through [`crate::qol::setup::check_lines`]).
    pub fn summary(&self) -> Vec<(Mark, String)> {
        let mut v = Vec::new();
        let e = &self.edition;
        let mut edition = format!("Edition (from the files): {}; text: {TEXT_LANGUAGE}", e.layout.words());
        if let Some(l) = self.movie_language() {
            edition += &format!("; movies: {l}");
        }
        v.push((Mark::Info, edition));
        let n = self.movies().count();
        let missing = self.missing_movies();
        let fallback: Vec<&Item> = self.movies().filter(|i| i.resolved.how.fallback()).collect();
        if e.layout == Layout::NoMovies {
            v.push((Mark::Warn, "No MOVIES folder: the game runs, but no movie will play".into()));
        } else if !missing.is_empty() {
            let k = missing.len();
            v.push((
                Mark::Warn,
                format!(
                    "{k} {} will not play: {} — not in MOVIES in any language",
                    if k == 1 { "movie" } else { "movies" },
                    names(&missing, 6)
                ),
            ));
        } else if let Some(i) = fallback.first() {
            v.push((
                Mark::Ok,
                format!(
                    "All {n} movies found ({} under this edition's names, e.g. {} → {})",
                    fallback.len(),
                    i.label,
                    i.resolved.file_name().unwrap_or_default()
                ),
            ));
        } else {
            v.push((Mark::Ok, format!("All {n} movies found")));
        }
        // Menu files that are not among the key files (those have lines
        // of their own).
        let menus: Vec<&str> = self
            .items
            .iter()
            .filter(|i| i.group == Group::Text && i.resolved.file.is_none())
            .filter(|i| !crate::qol::setup::KEY_FILES.iter().any(|k| k.0.eq_ignore_ascii_case(&i.label)))
            .map(|i| i.label.as_str())
            .collect();
        if !menus.is_empty() {
            v.push((Mark::Warn, format!("Menu files missing: {} — menus may not load", names(&menus, 4))));
        }
        if let Some(s) = &e.exe_suffix
            && !fallback.is_empty()
        {
            v.push((
                Mark::Info,
                format!("SPEED2.EXE asks for _{s} movie files; Overbright plays this install's own movie files"),
            ));
        }
        v
    }

    /// Each group's counts, in report order: how many files it has, how
    /// many were found, which are missing and how many were found under
    /// this install's own names (the launcher's check summary).
    pub fn group_counts(&self) -> Vec<GroupCount> {
        let mut groups: Vec<Group> = self.items.iter().map(|i| i.group).collect();
        groups.dedup();
        groups
            .into_iter()
            .map(|g| {
                let items: Vec<&Item> = self.items.iter().filter(|i| i.group == g).collect();
                let missing: Vec<String> =
                    items.iter().filter(|i| i.resolved.file.is_none()).map(|i| i.label.clone()).collect();
                GroupCount {
                    group: g,
                    total: items.len(),
                    found: items.len() - missing.len(),
                    own_names: items.iter().filter(|i| i.resolved.how.fallback()).count(),
                    missing,
                }
            })
            .collect()
    }

    /// One line per group: how many found, which will not play (the
    /// `--check-game` and start-log lines).
    pub fn groups(&self) -> Vec<(Mark, String)> {
        self.group_counts()
            .into_iter()
            .map(|c| {
                let (g, n) = (c.group, c.total);
                if !c.missing.is_empty() {
                    let missing: Vec<&str> = c.missing.iter().map(String::as_str).collect();
                    (Mark::Warn, format!("{}: {} of {n} found; missing: {}", g.words(), c.found, names(&missing, 5)))
                } else if c.own_names > 0 {
                    (Mark::Ok, format!("{}: all {n} found ({} under this edition's names)", g.words(), c.own_names))
                } else {
                    (Mark::Ok, format!("{}: all {n} found", g.words()))
                }
            })
            .collect()
    }

    /// Every wanted file, grouped, with the file the game opens or the
    /// names tried (`--check-game`, the support file).
    pub fn details(&self) -> Vec<String> {
        let mut v = Vec::new();
        let mut last = None;
        for i in &self.items {
            if last != Some(i.group) {
                v.push(format!("{}:", i.group.words()));
                last = Some(i.group);
            }
            let r = &i.resolved;
            v.push(match r.file_name() {
                Some(_) if i.group == Group::Text => format!("  {}: found", i.label),
                Some(f) => format!("  {}: {f}{}", i.label, how_words(&r.how)),
                None => format!("  {}: missing (tried {})", i.label, r.tried.join(", ")),
            });
        }
        let e = &self.edition;
        v.push(format!(
            "Languages: {}",
            if e.languages.is_empty() { "none".to_string() } else { e.languages.join(", ") }
        ));
        v.push(format!(
            "SPEED2.EXE movie names: {}",
            e.exe_suffix
                .as_ref()
                .map_or_else(|| "not read (no executable, or another build)".to_string(), |s| format!("_{s}.vp6"))
        ));
        v
    }
}

#[cfg(test)]
mod tests;
