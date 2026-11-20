//! The game check in the player's terms (docs/launcher.md, "The game
//! check"): a typed summary of a checked folder ([`CheckSummary`], built
//! beside `qol::setup::check_lines` from the same check), the state it is
//! in ([`CheckState`]) and the words the check screen shows for it: one
//! verdict, at most three short summary rows, the per-area details. The
//! command line's and the support file's wording stays `check_lines`.

use crate::game_assets::{Group, GroupCount, Report};
use crate::qol::setup;

use super::ui::Mark;

/// Whether the game files are a release Overbright knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Release {
    /// Every data file is the known release's.
    Known,
    /// The known release with some files changed (a mod): the files.
    Changed(Vec<String>),
    /// No data file is the known release's (another release or build).
    Unknown,
}

/// A checked folder in the player's terms ([`summarize`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckSummary {
    /// The files Overbright needs ([`setup::KEY_FILES`]).
    pub files_total: usize,
    /// Of them, not there.
    pub files_missing: Vec<String>,
    pub release: Release,
    /// The movies and the menu and text files, per group
    /// ([`Report::group_counts`]); empty without the full check.
    pub groups: Vec<GroupCount>,
    /// The install has a MOVIES folder.
    pub movies_folder: bool,
    /// The text languages installed (LANGUAGES), by name.
    pub languages: Vec<String>,
    /// The text language the check was made for: its file is the text file
    /// the game needs.
    pub language: String,
    /// That language's file is there but has no known size or checksum.
    pub text_unverified: bool,
    /// The language most movies play in, when it is one.
    pub movie_language: Option<String>,
}

/// What the check screen shows for a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState {
    /// Everything the game needs is there.
    Ready,
    /// Usable; some movies or menu files are missing.
    MoviesMissing,
    /// Usable; some game files are not the original ones.
    Changed,
    /// The files are not a release Overbright knows: asked before use.
    UnknownEdition,
    /// Not usable.
    Refused(super::model::Problem),
}

/// The check's summary from the file check and the fingerprint (None:
/// a quick check without hashes, judged by sizes and XXH3 alone).
pub fn summarize(c: &setup::Check, fp: Option<&crate::fingerprint::Fingerprint>) -> CheckSummary {
    use crate::fingerprint::{EXE, Verdict};
    let release = match fp.map(|f| (f, f.verdict_skipping(c.verdict_skip()))) {
        Some((_, Verdict::Known(_))) => Release::Known,
        Some((f, Verdict::Unknown(files))) => {
            let some_known = f.files.iter().any(|p| p.file != EXE && p.edition.is_some());
            // Missing files are said by the missing rows, not as changes.
            let changed: Vec<String> = files.into_iter().filter(|x| !c.missing.contains(x)).collect();
            match () {
                _ if changed.is_empty() => Release::Known,
                _ if some_known => Release::Changed(changed),
                _ => Release::Unknown,
            }
        }
        None if c.different.is_empty() => Release::Known,
        None => Release::Changed(c.different.clone()),
    };
    let a: Option<&Report> = c.assets.as_ref();
    CheckSummary {
        files_total: setup::KEY_FILES.len(),
        files_missing: c.missing.clone(),
        release,
        groups: a.map(Report::group_counts).unwrap_or_default(),
        movies_folder: a.is_none_or(|a| a.edition.layout != crate::game_assets::Layout::NoMovies),
        languages: a.map(|a| a.edition.languages.clone()).unwrap_or_default(),
        language: if c.language.is_empty() { crate::game_assets::DEFAULT_LANGUAGE.into() } else { c.language.clone() },
        text_unverified: !c.unverified.is_empty(),
        movie_language: a.and_then(Report::movie_language).filter(|l| crate::game_assets::language_suffix(l).is_some()),
    }
}

/// "1 movie", "3 movies".
fn count(n: usize, one: &str, many: &str) -> String {
    super::model::count(n, one, many)
}

/// "is" / "are" for `n`.
fn are(n: usize) -> &'static str {
    if n == 1 { "is" } else { "are" }
}

/// A group's name in a sentence ("story", "menu and text").
fn group_word(g: Group) -> &'static str {
    match g {
        Group::Start => "start",
        Group::Story => "story",
        Group::Tutorials => "tutorial",
        Group::Neighbourhoods => "neighbourhood",
        Group::Text => "menu",
    }
}

/// Up to `n` names, then "…".
fn names(v: &[String], n: usize) -> String {
    let mut s = v.iter().take(n).cloned().collect::<Vec<_>>().join(", ");
    if v.len() > n {
        s += ", …";
    }
    s
}

impl CheckSummary {
    fn movie_groups(&self) -> impl Iterator<Item = &GroupCount> {
        self.groups.iter().filter(|g| g.group != Group::Text)
    }

    /// Movies the game asks for, and of them found.
    pub fn movies(&self) -> (usize, usize) {
        self.movie_groups().fold((0, 0), |(f, t), g| (f + g.found, t + g.total))
    }

    /// The menu and text files that are not there (not the key files:
    /// those are the missing files).
    pub fn menus_missing(&self) -> Vec<String> {
        self.groups
            .iter()
            .filter(|g| g.group == Group::Text)
            .flat_map(|g| g.missing.iter())
            .filter(|f| !setup::KEY_FILES.iter().any(|k| k.0.eq_ignore_ascii_case(f)))
            .cloned()
            .collect()
    }

    pub fn changed(&self) -> &[String] {
        match &self.release {
            Release::Changed(v) => v,
            _ => &[],
        }
    }

    /// Movies that will not play (all of them without a MOVIES folder).
    pub fn movies_missing(&self) -> usize {
        let (found, total) = self.movies();
        if self.movies_folder { total - found } else { total }
    }

    /// The text file the game reads is not there; the languages the copy
    /// has instead.
    pub fn no_text(&self) -> Option<Vec<String>> {
        let file = crate::game_assets::text_file_of(&self.language);
        self.files_missing.iter().any(|f| f.eq_ignore_ascii_case(&file)).then(|| {
            self.languages
                .iter()
                .filter(|l| !l.eq_ignore_ascii_case(&self.language))
                .cloned()
                .collect()
        })
    }

    /// The edition row's value: the release, the text, the movies.
    pub fn edition_words(&self) -> String {
        let mut v = vec![match self.release {
            Release::Known => "Version 1.2".to_string(),
            Release::Changed(_) => "Version 1.2, changed".to_string(),
            Release::Unknown => "Not recognised".to_string(),
        }];
        if self.no_text().is_none() {
            v.push(format!("{} text{}", self.language, if self.text_unverified { " (not verified)" } else { "" }));
        }
        if let Some(l) = &self.movie_language {
            v.push(format!("{l} movies"));
        }
        v.join(" · ")
    }

    /// The summary rows: (id, lamp, area, value). At most three.
    pub fn rows(&self) -> Vec<(super::ui::Id, Mark, &'static str, String)> {
        let n = self.files_total;
        let found = n - self.files_missing.len().min(n);
        let changed = self.changed().len();
        let menus = self.menus_missing().len();
        let mut files = format!("{found} of {n}");
        files += &match (changed, menus) {
            (0, 0) => " · original".to_string(),
            (c, 0) => format!(" · {c} changed"),
            (0, m) => format!(" · {} missing", count(m, "menu file", "menu files")),
            (c, m) => format!(" · {c} changed · {} missing", count(m, "menu file", "menu files")),
        };
        let files_mark = if found < n {
            Mark::Error
        } else if changed + menus > 0 {
            Mark::Warn
        } else {
            Mark::Ok
        };
        let mut v = vec![("sum_files", files_mark, "Game files", files)];
        if !self.groups.is_empty() {
            let (found, total) = self.movies();
            let gone = self.movies_missing();
            let movies = match () {
                _ if !self.movies_folder => "No movies folder · none will play".to_string(),
                _ if gone == 0 => format!("All {total}"),
                _ => format!("{found} of {total} · {gone} won't play"),
            };
            v.push(("sum_movies", if gone == 0 { Mark::Ok } else { Mark::Warn }, "Movies", movies));
        }
        let ed_mark = match self.release {
            Release::Known => Mark::Ok,
            _ => Mark::Warn,
        };
        v.push(("sum_edition", ed_mark, "Edition", self.edition_words()));
        v
    }

    /// The details list, by area: the missing and changed game files by
    /// name, then one row per group of movies and menu files. The first
    /// row of each area is given for the summary rows' clicks.
    pub fn details(&self) -> Vec<(Mark, String)> {
        let mut v = Vec::new();
        for f in &self.files_missing {
            v.push((Mark::Error, format!("Missing: {f}")));
        }
        for f in self.changed() {
            v.push((Mark::Warn, format!("Changed: {f}")));
        }
        if self.files_missing.is_empty() && self.changed().is_empty() {
            v.push((Mark::Ok, format!("Game files: all {} there, the original ones", self.files_total)));
        }
        if self.release == Release::Unknown {
            v.push((Mark::Warn, "Release: not one Overbright knows".into()));
        }
        if !self.movies_folder {
            v.push((Mark::Warn, "No MOVIES folder: no movie will play".into()));
        }
        for g in &self.groups {
            if g.group != Group::Text && !self.movies_folder {
                continue;
            }
            let (mark, value) = if g.missing.is_empty() {
                (Mark::Ok, format!("all {}", g.total))
            } else {
                (Mark::Warn, format!("{} of {} · missing {}", g.found, g.total, names(&g.missing, 5)))
            };
            v.push((mark, format!("{}: {value}", g.group.words())));
        }
        let own: usize = self.movie_groups().map(|g| g.own_names).sum();
        if own > 0 && self.movies_folder {
            v.push((Mark::Info, format!("Movies found under this copy's own names: {own}")));
        }
        v.push((Mark::Info, format!("Text: {}", self.languages_words())));
        v
    }

    /// The installed text languages, in words.
    fn languages_words(&self) -> String {
        if self.languages.is_empty() { "no language files".into() } else { self.languages.join(", ") }
    }

    /// The details row a summary row opens at.
    pub fn detail_row_of(&self, id: &str) -> usize {
        let d = self.details();
        let at = |p: &dyn Fn(&str) -> bool| d.iter().position(|(_, t)| p(t)).unwrap_or(0);
        match id {
            "sum_movies" => at(&|t| t.starts_with("Start movies") || t.starts_with("No MOVIES")),
            "sum_edition" => d.len().saturating_sub(1),
            _ => 0,
        }
    }

    /// The state of a usable folder with this summary.
    pub fn state(&self) -> CheckState {
        match self.release {
            Release::Unknown => CheckState::UnknownEdition,
            Release::Changed(_) => CheckState::Changed,
            Release::Known if self.movies_missing() > 0 || !self.menus_missing().is_empty() => {
                CheckState::MoviesMissing
            }
            Release::Known => CheckState::Ready,
        }
    }

    /// What the missing movies and menu files mean, in one or two
    /// sentences ("" when nothing is missing).
    pub fn movies_sentence(&self) -> String {
        let mut s = String::new();
        let gone = self.movies_missing();
        if !self.movies_folder {
            s += "This copy has no MOVIES folder, so no movie will play. The game runs without them.";
        } else if gone > 0 {
            let by: Vec<String> = self
                .movie_groups()
                .filter(|g| !g.missing.is_empty())
                .map(|g| format!("{} {}", g.missing.len(), group_word(g.group)))
                .collect();
            s += &format!(
                "{} {} missing ({}). The game skips {}, as the original does.",
                count(gone, "movie", "movies"),
                are(gone),
                by.join(", "),
                if gone == 1 { "it" } else { "them" }
            );
        }
        let menus = self.menus_missing();
        if !menus.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s += &format!(
                "{} {} missing, so some menus may not load.",
                count(menus.len(), "menu file", "menu files"),
                are(menus.len())
            );
        }
        s
    }
}

/// The verdict card of a usable folder: lamp, title, sentence, buttons
/// (the first is the primary button; `focus` the one focused).
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub mark: Mark,
    pub title: &'static str,
    pub text: String,
    pub buttons: Vec<(super::ui::Id, &'static str)>,
    pub focus: super::ui::Id,
}

/// The titles of the usable states (the troubleshooting table names the
/// same, `troubleshooting_table_matches_the_check_screen`).
pub const READY: &str = "Ready to play";
pub const MOVIES_MISSING: &str = "Ready, some movies won't play";
pub const MENUS_MISSING: &str = "Ready, some menu files are missing";
pub const CHANGED: &str = "Ready, with changed game files";
pub const UNKNOWN: &str = "This copy isn't recognised";

/// The verdict card for a usable folder (`details_open`: the Details
/// button's words).
pub fn verdict(s: &CheckSummary, details_open: bool) -> Verdict {
    let details = ("details", if details_open { "Hide details" } else { "Details" });
    let support = ("support", "Save support file…");
    let movies = s.movies_sentence();
    let with_movies = |t: String| if movies.is_empty() { t } else { format!("{t} {movies}") };
    match s.state() {
        CheckState::Ready | CheckState::Refused(_) => Verdict {
            mark: Mark::Ok,
            title: READY,
            text: "Need for Speed Underground 2, version 1.2. Everything the game needs is here.".into(),
            buttons: vec![("use", "Use this folder"), details],
            focus: "use",
        },
        CheckState::MoviesMissing => Verdict {
            mark: Mark::Warn,
            title: if s.movies_missing() > 0 { MOVIES_MISSING } else { MENUS_MISSING },
            text: movies,
            buttons: vec![("use", "Use this folder"), details, support],
            focus: "use",
        },
        CheckState::Changed => {
            let n = s.changed().len();
            Verdict {
                mark: Mark::Warn,
                title: CHANGED,
                text: with_movies(format!(
                    "{} {} not the original ones (a mod or another release). The game may look or play \
                     differently.",
                    count(n, "game file", "game files"),
                    are(n)
                )),
                buttons: vec![("use", "Use this folder"), details, support],
                focus: "use",
            }
        }
        CheckState::UnknownEdition => Verdict {
            mark: Mark::Warn,
            title: UNKNOWN,
            text: with_movies(
                "Its game files don't match the release Overbright knows (version 1.2), so the game may look or \
                 play differently. Use it anyway?"
                    .into(),
            ),
            buttons: vec![("another", "Choose another folder…"), ("use", "Use it anyway"), support, details],
            focus: "another",
        },
    }
}
