//! The Game data screen (`game-data`): your own copy of the game, made once from the installed game by the
//! importer (`ob_asset_format::import`, docs/modding/asset-format/import.md) and kept in `<app>/base`. The first
//! run offers it as the next step after the game check (one confirm; "Not now" is kept); Settings › Advanced ›
//! Game data shows where the copy stands (ready, not made, needs repair) and checks or repairs it.
//!
//! The import, the check and the repair run on a worker thread ([`Job`]) while the launcher stays usable; Stop
//! asks the importer to stop between its units, and the next start goes on from its journal. The game folder is
//! only ever read.

use super::layout::{Builder, Ctrl};
use super::model::Model;
use super::ui::Mark;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// What a run does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// Makes the copy (or goes on with a stopped one).
    #[default]
    Import,
    /// Makes it again from the start (a copy whose record cannot be repaired).
    ImportAgain,
    /// Re-checks every part of the copy.
    Verify,
    /// Rebuilds the damaged parts from the game folder.
    Repair,
}

/// What is in Overbright's own folder, as found at the start (and after each run).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    /// No copy.
    #[default]
    None,
    /// A stopped import: its staging folder and journal wait for "Go on".
    Partial,
    /// A finished copy whose parts are all there (`known`: the files are the known release's).
    Ready { edition: String, known: bool },
    /// A finished copy with this many parts missing, cut short or failing their check.
    Damaged(usize),
}

/// Where the screen stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum State {
    /// No copy yet: the offer, with the space it takes.
    #[default]
    Offer,
    /// A run goes on: the bar's fill and its words.
    Running { fraction: f32, words: String },
    /// Stopped: what is copied so far is kept; the next start goes on from there (`of` 0: not known).
    Stopped { done: u64, of: u64 },
    /// Not enough space: what it needs, what is free, where.
    NoSpace { need: u64, free: u64, place: String },
    /// Made (or found whole): the copy's name and whether the files are the known release's
    /// (`differing`: how many differ, when known).
    Done { edition: String, known: bool, differing: Option<usize> },
    /// Parts of the copy are damaged: Repair.
    Damaged(usize),
    /// Failed: the importer's one line.
    Failed(String),
}

/// The Game data screen's model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameData {
    pub state: State,
    /// What the folder holds.
    pub status: Status,
    /// The game folder's content (bytes): what the copy takes (0: not known).
    pub size: u64,
    /// Where the copy goes (`<app>/base`).
    pub place: String,
    /// The last run asked for (Try again runs it again).
    pub last: Kind,
    /// Stop was pressed; the run ends at its next unit.
    pub stopping: bool,
    /// Opened from Settings › Advanced (Back and Continue return there; else to the main screen).
    pub from_settings: bool,
}

impl GameData {
    /// The screen's state for what the folder holds (a run not going on).
    pub fn state_for(status: &Status) -> State {
        match status {
            Status::None => State::Offer,
            Status::Partial => State::Stopped { done: 0, of: 0 },
            Status::Ready { edition, known } => {
                State::Done { edition: edition.clone(), known: *known, differing: None }
            }
            Status::Damaged(n) => State::Damaged(*n),
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.state, State::Running { .. })
    }

    /// The Advanced page's row value.
    pub fn row_words(&self) -> String {
        match (&self.state, &self.status) {
            (State::Running { fraction, .. }, _) => format!("Working: {} %", (fraction * 100.0).round() as u32),
            (_, Status::Ready { .. }) => "Your own copy: ready".into(),
            (_, Status::Damaged(_)) => "Your own copy needs repair".into(),
            (_, Status::Partial) => "Copy stopped: go on".into(),
            (_, Status::None) => "Your own copy: not made yet".into(),
        }
    }

    /// The main screen's state line while a run goes on behind it.
    pub fn background_words(&self) -> Option<String> {
        match &self.state {
            State::Running { fraction, .. } => Some(format!(
                "Ready to play · {} in the background ({} %)",
                if self.last == Kind::Verify { "checking your own copy" } else { "making your own copy" },
                (fraction * 100.0).round() as u32
            )),
            _ => None,
        }
    }
}

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1e9)
}

/// "1,250".
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

const WHY: &str = "Overbright can keep its own copy of your game files, made once from your installed game. Your \
game folder is only read, never changed. Once the copy is made, Overbright no longer needs the game folder.";

pub fn build(b: &mut Builder, m: &Model) {
    let g = &m.game_data;
    b.page_title("Game data", Some(if g.from_settings { "Advanced" } else { "Settings" }));
    b.body(WHY);
    b.gap(12.0);
    let later = Ctrl::button("later", "Not now");
    match &g.state {
        State::Offer => b.message_with(
            Mark::Info,
            "Make your own copy",
            &if g.size == 0 {
                "Takes as much space as the game folder and about a minute. You can stop at any time and go on later.".to_string()
            } else {
                format!("Takes {} in {} and about a minute. You can stop at any time and go on later.", gb(g.size), g.place)
            },
            vec![Ctrl::primary("import", "Make copy"), later],
        ),
        State::Running { fraction, words } => b.message_progress(
            Mark::Busy,
            if g.last == Kind::Verify {
                "Checking your own copy"
            } else if g.last == Kind::Repair {
                "Repairing your own copy"
            } else {
                "Making your own copy"
            },
            if g.stopping {
                "Stopping after the part it is on; what is copied so far is kept."
            } else if g.last == Kind::Verify {
                "Every part is read back and checked. You can go on using the launcher meanwhile."
            } else if g.last == Kind::Repair {
                "The damaged parts are made again from your game folder. You can go on using the launcher meanwhile."
            } else {
                "You can stop at any time; what is copied so far is kept. You can go on using the launcher meanwhile."
            },
            Some((*fraction, words.clone())),
            // The check and the repair are short and not stopped halfway; Back leaves them running.
            if matches!(g.last, Kind::Import | Kind::ImportAgain) {
                vec![Ctrl::Button { id: "stop", text: "Stop".into(), primary: false, enabled: !g.stopping }]
            } else {
                Vec::new()
            },
        ),
        State::Stopped { done, of } => b.message_progress(
            Mark::Info,
            "Copy stopped",
            "What is copied so far is kept. Go on takes up where it stopped.",
            (*of > 0).then(|| ((*done as f64 / *of as f64) as f32, format!("{} of {}", gb(*done), gb(*of)))),
            vec![Ctrl::primary("import", "Go on"), later],
        ),
        State::NoSpace { need, free, place } => b.message_with(
            Mark::Warn,
            "Not enough space",
            &format!("The copy needs {}, and {} is free on {place}. Free some space or choose another place in Storage; nothing was written.", gb(*need), gb(*free)),
            vec![Ctrl::primary("import", "Try again"), Ctrl::button("storage", "Storage…")],
        ),
        State::Done { edition, known: true, .. } => b.message_with(
            Mark::Ok,
            "Your own copy is ready",
            &format!("{edition}. Every file was checked. The game folder is no longer needed; nothing in it was changed or deleted."),
            vec![Ctrl::primary("continue", "Continue"), Ctrl::button("verify", "Check copy")],
        ),
        State::Done { edition, differing, .. } => b.message_with(
            Mark::Warn,
            "Your own copy is ready",
            &format!(
                "{edition}. Your game files differ from the versions Overbright was checked against{}; it will run, but may not match the original exactly.",
                differing.map_or(String::new(), |n| format!(" ({})", super::model::count(n, "file", "files")))
            ),
            vec![Ctrl::primary("continue", "Continue"), Ctrl::button("verify", "Check copy")],
        ),
        State::Damaged(n) => b.message_with(
            Mark::Warn,
            "Your own copy needs repair",
            &format!(
                "{} of your copy {} missing or damaged. Repair makes {} again from your game folder, which is only read; until then the game plays from the game folder.",
                super::model::count(*n, "part", "parts"),
                if *n == 1 { "is" } else { "are" },
                if *n == 1 { "it" } else { "them" }
            ),
            vec![Ctrl::primary("repair", "Repair"), later],
        ),
        State::Failed(why) => {
            let mut v = vec![Ctrl::primary("retry", "Try again")];
            if g.last == Kind::Repair {
                v.push(Ctrl::button("import_again", "Make a new copy"));
            }
            v.push(later);
            b.message_with(
                Mark::Error,
                if g.last == Kind::Verify { "The copy could not be checked" } else { "The copy could not be made" },
                &format!("{why}. Nothing was changed in your game folder; the game still plays from it."),
                v,
            )
        }
    }
    b.default_focus = Some(match g.state {
        State::Running { .. } if !matches!(g.last, Kind::Import | Kind::ImportAgain) => return,
        State::Running { .. } => "stop",
        State::Done { .. } => "continue",
        State::Damaged(_) => "repair",
        State::Failed(_) => "retry",
        _ => "import",
    });
}

/// The headless states (`overbright --render <name>`).
pub fn state(name: &str, m: &mut Model) -> bool {
    let g = &mut m.game_data;
    g.size = 1_801_794_145;
    let pct = |f: f32| f;
    g.state = match name {
        "game-data-offer" => State::Offer,
        "game-data-running" => State::Running { fraction: pct(0.62), words: "Copying · 0.8 GB of 1.8 GB".into() },
        "game-data-checking" => {
            g.last = Kind::Verify;
            g.from_settings = true;
            g.status = Status::Ready { edition: "PC 1.2, European release".into(), known: true };
            State::Running { fraction: pct(0.4), words: "Checking the copy · 48 of 120 parts".into() }
        }
        "game-data-stopped" => State::Stopped { done: 812_000_000, of: g.size },
        "game-data-space" => State::NoSpace { need: 1_811_000_000, free: 1_100_000_000, place: "/home".into() },
        "game-data-done" => State::Done { edition: "PC 1.2, European release".into(), known: true, differing: None },
        "game-data-custom" => State::Done { edition: "Your own install".into(), known: false, differing: Some(3) },
        "game-data-damaged" => {
            g.from_settings = true;
            g.status = Status::Damaged(2);
            State::Damaged(2)
        }
        "game-data-failed" => State::Failed("A file could not be read: TRACKS/STREAML4RA.BUN".into()),
        _ => return false,
    };
    true
}

// ---- the folder -------------------------------------------------------------------------------------------

/// The finished set in `base`: `active.txt`'s, else the only one there (as `overbright-import verify`).
fn active_set(base: &Path) -> Option<PathBuf> {
    if let Ok(b) = std::fs::read(base.join("active.txt"))
        && let Ok(Some(id)) = ob_asset_format::order::parse_active(&b)
        && base.join(&id).join("import.toml").is_file()
    {
        return Some(base.join(id));
    }
    let mut sets = std::fs::read_dir(base).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| {
        p.join("import.toml").is_file() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
    });
    let one = sets.next()?;
    sets.next().is_none().then_some(one)
}

/// The words for an edition key.
pub fn edition_words(key: &str, known: bool) -> String {
    match key {
        "pc12-eu" if known => "PC 1.2, European release".into(),
        _ => "Your own install".into(),
    }
}

/// What `base` holds: a quick look (the record read, every part there at its recorded size); "Check copy"
/// reads every part back ([`Kind::Verify`]).
pub fn status(base: &Path) -> Status {
    if let Some(set) = active_set(base) {
        let Ok(rec) = ob_asset_format::import::read_record(&set) else { return Status::Damaged(1) };
        let bad = rec
            .bundles
            .iter()
            .filter(|(u, (size, _))| {
                std::fs::metadata(set.join("bundles").join(format!("{u}.obz"))).map(|m| m.len()).ok() != Some(*size)
            })
            .count();
        if bad > 0 {
            return Status::Damaged(bad);
        }
        let text = std::fs::read_to_string(set.join("import.toml")).unwrap_or_default();
        let field = |k: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(k).and_then(|r| r.trim_start().strip_prefix('=')))
                .map(|v| v.trim().trim_matches('"').to_string())
                .unwrap_or_default()
        };
        let known = field("verdict") == "known";
        return Status::Ready { edition: edition_words(&field("edition"), known), known };
    }
    let partial =
        std::fs::read_dir(base).into_iter().flatten().filter_map(|e| e.ok()).any(|e| {
            e.file_name().to_string_lossy().starts_with(".staging-") && e.path().join("journal.txt").is_file()
        });
    if partial { Status::Partial } else { Status::None }
}

/// The game folder's content in bytes (the importer's scan: names and sizes only); 0 when it cannot be read.
pub fn size(root: &Path) -> u64 {
    ob_asset_format::import::scan(root).map_or(0, |s| s.total())
}

// ---- the run ----------------------------------------------------------------------------------------------

/// How a run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// The copy is whole (made, checked or repaired).
    Ready {
        status: Status,
        differing: Option<usize>,
    },
    Stopped {
        done: u64,
        of: u64,
    },
    NoSpace {
        need: u64,
        free: u64,
        place: String,
    },
    Damaged(usize),
    Failed(String),
}

/// What a run says.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Progress { fraction: f32, words: String },
    End(End),
}

/// A run on its worker thread (or, for the fake data, its messages written in advance).
pub struct Job {
    rx: Receiver<Msg>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Job {
    /// A run that says `msgs` (the fake data and the tests).
    pub fn scripted(msgs: Vec<Msg>) -> Job {
        let (tx, rx) = channel();
        for m in msgs {
            let _ = tx.send(m);
        }
        Job { rx, stop: Arc::new(AtomicBool::new(false)), thread: None }
    }

    /// The messages since the last call.
    pub fn poll(&mut self) -> Vec<Msg> {
        self.rx.try_iter().collect()
    }

    /// Asks the run to stop between its units (what is done stays; the next import goes on from it).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// The launcher closes: the run asked to stop and waited for at most `wait`. A run still going after that
    /// ends with the process; its journal keeps what was finished (a torn last part is made again).
    pub fn finish(mut self, wait: std::time::Duration) {
        self.stop();
        let until = std::time::Instant::now() + wait;
        if let Some(t) = self.thread.take() {
            while !t.is_finished() && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

/// Turns the importer's `key=value` lines into the bar's fill and words.
#[derive(Default)]
pub struct Tracker {
    kind: Kind,
    step: u32,
    /// The parts to check (a verify run).
    pub parts: usize,
    pub need: u64,
    pub bytes: (u64, u64),
    pub edition: String,
    pub known: bool,
    pub differing: usize,
    last: Option<(u32, String)>,
}

impl Tracker {
    pub fn new(kind: Kind, parts: usize) -> Tracker {
        Tracker { kind, parts, ..Tracker::default() }
    }

    /// The bar after `line`, when it moved.
    pub fn line(&mut self, line: &str) -> Option<(f32, String)> {
        let (k, v) = line.split_once('=')?;
        let ratio = |v: &str| {
            let (a, b) = v.split_whitespace().next()?.split_once('/')?;
            Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?))
        };
        let out = match k {
            "step" => {
                self.step = v.split('/').next()?.parse().ok()?;
                let (f, w) = match self.step {
                    1 | 2 => (0.0, "Reading your game files"),
                    3 => (0.35, "Reading your game files"),
                    4 => (0.35, "Copying"),
                    5 => (0.9, "Checking the copy"),
                    _ => (0.98, "Finishing"),
                };
                Some((f, w.to_string()))
            }
            "edition" => {
                let mut w = v.split_whitespace();
                self.edition = w.next().unwrap_or_default().to_string();
                self.known = w.next() == Some("known");
                None
            }
            "differs" => {
                self.differing += 1;
                None
            }
            "space_needed" => {
                self.need = v.trim().parse().ok()?;
                None
            }
            "bytes" => {
                let (d, t) = ratio(v)?;
                self.bytes = (d, t);
                Some((0.35 + 0.55 * d as f32 / t.max(1) as f32, format!("Copying · {} of {}", gb(d), gb(t))))
            }
            "progress" => {
                let (d, n) = ratio(v)?;
                let part = d as f32 / n.max(1) as f32;
                if v.ends_with("files") {
                    Some((
                        0.35 * part,
                        format!("Reading your game files · {} of {}", thousands(d as usize), thousands(n as usize)),
                    ))
                } else if v.ends_with("checked") {
                    Some((0.9 + 0.08 * part, format!("Checking the copy · {} of {} parts", d, n)))
                } else {
                    None
                }
            }
            // `verify` and `repair` say one line per part checked.
            "unit"
                if matches!(self.kind, Kind::Verify | Kind::Repair)
                    && (v.ends_with(" ok") || v.ends_with(" damaged")) =>
            {
                self.step += 1;
                let n = self.parts.max(1);
                let d = (self.step as usize).min(n);
                Some((d as f32 / n as f32, format!("Checking the copy · {d} of {n} parts")))
            }
            _ => None,
        };
        // Sent only when the words or the whole percent change.
        let (f, w) = out?;
        let key = ((f * 1000.0) as u32, w.clone());
        if self.last.as_ref() == Some(&key) {
            return None;
        }
        self.last = Some(key);
        Some((f, w))
    }
}

/// Capitalised for the screen ("a file …" → "A file …").
fn sentence(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// Starts `kind` on a worker thread: the game folder `game` (only read), the copy in `base`.
pub fn start(kind: Kind, game: &Path, base: &Path) -> Job {
    use ob_asset_format::import::{self, Options, Outcome};
    let (tx, rx) = channel::<Msg>();
    let stop = Arc::new(AtomicBool::new(false));
    let (game, base, flag) = (game.to_path_buf(), base.to_path_buf(), stop.clone());
    let thread = std::thread::Builder::new()
        .name("game-data".into())
        .spawn(move || {
            let set = active_set(&base);
            let parts = set.as_deref().and_then(|s| import::read_record(s).ok()).map_or(0, |r| r.bundles.len());
            let tracker = Mutex::new(Tracker::new(kind, parts));
            let send: Mutex<Sender<Msg>> = Mutex::new(tx.clone());
            let progress = |l: &str| {
                if let Some((fraction, words)) = tracker.lock().unwrap().line(l) {
                    let _ = send.lock().unwrap().send(Msg::Progress { fraction, words });
                }
            };
            // Half the cores at most: the launcher stays smooth while it runs.
            let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4));
            let opt = Options { threads, force: kind == Kind::ImportAgain, stop: Some(&flag), ..Options::default() };
            let ready = |differing| End::Ready { status: status(&base), differing };
            let end = match kind {
                Kind::Import | Kind::ImportAgain => match import::import(&game, &base, &opt, &progress) {
                    Ok(Outcome::Done { .. }) => {
                        let t = tracker.lock().unwrap();
                        ready((!t.known && t.differing > 0).then_some(t.differing))
                    }
                    Ok(Outcome::Stopped { .. }) => {
                        let (done, of) = tracker.lock().unwrap().bytes;
                        End::Stopped { done, of }
                    }
                    Err(r) if r.rule == ob_asset_format::Rule::Imp2 => {
                        let mut place = base.clone();
                        while !place.exists() && place.pop() {}
                        End::NoSpace {
                            need: tracker.lock().unwrap().need,
                            free: ob_core::platform::fs::free_space(&base).unwrap_or(0),
                            place: place.display().to_string(),
                        }
                    }
                    Err(r) => End::Failed(sentence(&r.message)),
                },
                Kind::Verify => match set.as_deref().map(|s| import::verify(s, threads, &progress)) {
                    Some(Ok(d)) if d.is_empty() => ready(None),
                    Some(Ok(d)) => End::Damaged(d.len()),
                    Some(Err(r)) => End::Failed(sentence(&r.message)),
                    None => End::Failed("There is no copy to check".into()),
                },
                Kind::Repair => match set.as_deref().map(|s| import::repair(s, &game, &opt, &progress)) {
                    Some(Ok(_)) => ready(None),
                    Some(Err(r)) => End::Failed(sentence(&r.message)),
                    None => End::Failed("There is no copy to repair".into()),
                },
            };
            log::info!("game data: {kind:?} ended: {end:?}");
            let _ = tx.send(Msg::End(end));
        })
        .ok();
    if thread.is_none() {
        let (tx2, rx2) = channel();
        let _ = tx2.send(Msg::End(End::Failed("The worker could not be started".into())));
        return Job { rx: rx2, stop, thread: None };
    }
    Job { rx, stop, thread }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_follows_the_importers_lines() {
        let mut t = Tracker::new(Kind::Import, 0);
        assert_eq!(t.line("import=1"), None);
        assert_eq!(t.line("step=1/6 scan"), Some((0.0, "Reading your game files".into())));
        let (f, w) = t.line("progress=625/1250 files").unwrap();
        assert!((f - 0.175).abs() < 1e-6, "{f}");
        assert_eq!(w, "Reading your game files · 625 of 1,250");
        t.line("edition=custom custom");
        t.line("differs=GLOBAL/X.BIN size");
        t.line("space_needed=1811000000");
        assert_eq!((t.known, t.differing, t.need), (false, 1, 1_811_000_000));
        t.line("step=4/6 convert");
        let (f, w) = t.line("bytes=900000000/1800000000").unwrap();
        assert!((f - 0.625).abs() < 1e-6, "{f}");
        assert_eq!(w, "Copying · 0.9 GB of 1.8 GB");
        assert_eq!(t.bytes, (900_000_000, 1_800_000_000));
        assert_eq!(t.line("bytes=900000000/1800000000"), None, "the same again is not sent");
        assert!(t.line("progress=3/4 checked").unwrap().1.contains("3 of 4 parts"));
        // A verify run counts its parts.
        let mut v = Tracker::new(Kind::Verify, 4);
        assert_eq!(v.line("unit=global ok"), Some((0.25, "Checking the copy · 1 of 4 parts".into())));
        assert_eq!(v.line("unit=tracks damaged").unwrap().0, 0.5);
    }

    #[test]
    fn the_folder_says_where_the_copy_stands() {
        let d = std::env::temp_dir().join(format!("ob-game-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(status(&d), Status::None, "no folder");
        std::fs::create_dir_all(d.join(".staging-x")).unwrap();
        assert_eq!(status(&d), Status::None, "a staging folder without its journal");
        std::fs::write(d.join(".staging-x/journal.txt"), "# Overbright import journal v1\n").unwrap();
        assert_eq!(status(&d), Status::Partial);
        // A set whose record cannot be read needs repair.
        std::fs::create_dir_all(d.join("set")).unwrap();
        std::fs::write(d.join("set/import.toml"), "not a record").unwrap();
        assert_eq!(status(&d), Status::Damaged(1));
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(GameData::state_for(&Status::Partial), State::Stopped { done: 0, of: 0 });
        assert_eq!(thousands(1250), "1,250");
        assert_eq!(thousands(999), "999");
    }
}
