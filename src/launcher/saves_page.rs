//! The Saves page (docs/launcher.md, "Saves"): Overbright's profiles with
//! their career, money, car, covers and backups; finding and importing the
//! original game's saves (a folder or a `.zip`); back up, restore, export,
//! rename, duplicate and delete, each destructive step with a backup first
//! and Undo. The file work runs on a worker thread (`saves::jobs`); the
//! page shows a busy line and greys the actions meanwhile.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use super::layout::{Align, Builder, Ctrl};
use super::model::{BrowseFor, Dialog, count};
use super::saves::jobs::{self, Done, Found, Job, Row};
use super::saves::store::{Backup, How, Undo};
use super::saves::summary::{self, money, stage_label};
use super::saves::{Act, Plan};
use super::screens::{App, Outcome, Screen};
use super::ui::{Event, Kind, Mark, Rect, Widget};

/// How long Undo stays offered after a step.
pub const UNDO_FOR: Duration = Duration::from_secs(10);
/// The found sources shown (each with its Review button).
pub const SOURCE_IDS: [&str; 4] = ["src0", "src1", "src2", "src3"];

/// The page's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavesPage {
    pub rows: Vec<Row>,
    pub backups: Vec<Backup>,
    /// The original's saves found on this computer (None: not looked yet).
    pub found: Option<Vec<Found>>,
    /// An import's plan, shown before it is done.
    pub plan: Option<Plan>,
    /// The chosen profile (`rows`' index) and backup.
    pub sel: usize,
    pub backup_sel: usize,
    /// What the worker does now.
    pub busy: Option<String>,
    /// The last step's result lines.
    pub lines: Vec<(Mark, String)>,
    /// What Undo would do, and until when it is offered (None: until the
    /// next step; the renders).
    pub undo: Option<Undo>,
    pub undo_until: Option<Instant>,
    /// The name being typed for a rename.
    pub renaming: Option<String>,
    /// The chosen profile's backups are listed.
    pub show_backups: bool,
    /// A narrow window: the chosen profile's actions are shown.
    pub actions: bool,
    /// The less used actions are shown (More…).
    pub more: bool,
    /// The profiles were read once.
    pub loaded: bool,
    /// The time "last played" is counted from (fixed in the renders).
    pub now: Option<SystemTime>,
}

impl SavesPage {
    pub fn row(&self) -> Option<&Row> {
        self.rows.get(self.sel.min(self.rows.len().saturating_sub(1)))
    }

    /// The chosen profile's backups.
    pub fn backups_of(&self, name: &str) -> Vec<&Backup> {
        self.backups.iter().filter(|b| b.name.eq_ignore_ascii_case(name)).collect()
    }

    /// A job's result taken in.
    pub fn done(&mut self, d: Done) {
        let keep = self.row().map(|r| r.s.name.clone());
        self.rows = d.rows;
        self.backups = d.backups;
        if d.found.is_some() {
            self.found = d.found;
        }
        if d.imported {
            self.plan = None;
        }
        if d.plan.is_some() {
            self.plan = d.plan;
        }
        self.lines = d.lines;
        if d.undo.is_some() {
            self.undo = d.undo;
            self.undo_until = Some(Instant::now() + UNDO_FOR);
        }
        let want = d.select.or(keep);
        if let Some(n) = want
            && let Some(i) = self.rows.iter().position(|r| r.s.name.eq_ignore_ascii_case(&n))
        {
            self.sel = i;
        }
        self.sel = self.sel.min(self.rows.len().saturating_sub(1));
        self.backup_sel = 0;
        self.busy = None;
        self.loaded = true;
    }
}

/// "just now", "5 min ago", "3 h ago", "2 days ago", else the date.
pub fn ago(now: SystemTime, t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "not known".into() };
    let s = now.duration_since(t).map_or(0, |d| d.as_secs());
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86_400 => format!("{} h ago", s / 3600),
        86_400..1_209_600 => count((s / 86_400) as usize, "day ago", "days ago"),
        _ => date(t),
    }
}

/// A date as the player reads it (local, the system's form;
/// `launcher::localtime`).
pub fn date(t: SystemTime) -> String {
    super::localtime::date(t)
}

/// A profile's row in the list: (main text, note).
pub fn row_text(r: &Row, now: SystemTime) -> (String, String) {
    let s = &r.s;
    let main = if s.problem.is_some() {
        format!("{}  ·  can't be loaded by the game", s.name)
    } else {
        format!("{}  ·  {}  ·  {}", s.name, stage_label(s.stage), money(s.cash))
    };
    (main, format!("{}  ·  {}", r.car, ago(now, s.modified)))
}

/// A backup's row: (when, what).
pub fn backup_text(b: &Backup) -> (String, String) {
    let what = match (b.why, b.covers) {
        (None, _) => "Saved by the game".to_string(),
        (Some(w), 0) => w.shown().to_string(),
        (Some(w), n) => format!("{} · {}", w.shown(), count(n, "cover", "covers")),
    };
    (b.when(), what)
}

fn btn(id: &'static str, text: &str, enabled: bool) -> Ctrl {
    Ctrl::Button { id, text: text.into(), primary: false, enabled }
}

/// The page.
pub fn build(b: &mut Builder, m: &super::model::Model) {
    let sp = &m.saves;
    let busy = sp.busy.is_some();
    let now = sp.now.unwrap_or_else(SystemTime::now);
    b.page_title("Saves", None);
    let cols = b.columns(640.0);
    b.in_column(cols.left);
    let narrow = cols.right.is_none();
    b.section_a(&format!("Your profiles ({})", sp.rows.len()));
    if sp.rows.is_empty() {
        b.dim(if sp.loaded { "No profiles yet." } else { "Reading your profiles…" });
    } else {
        let items: Vec<(String, String)> = sp.rows.iter().map(|r| row_text(r, now)).collect();
        let rows = if narrow { 4 } else { 6 };
        b.list("saves_list", &items, sp.sel, items.len().min(rows));
    }
    if narrow && let Some(r) = sp.row() {
        // The chosen profile in one line; its actions behind Actions….
        let n = sp.backups_of(&r.s.name).len();
        b.dim(&format!(
            "{} · {} · {}",
            r.car,
            count(r.s.covers.len(), "cover", "covers"),
            count(n, "backup", "backups")
        ));
        if sp.actions || sp.renaming.is_some() || sp.show_backups || sp.more {
            b.gap(10.0);
            actions(b, m, r);
        } else {
            b.gap(10.0);
            b.buttons(Align::Left, vec![btn("actions", "Actions…", !busy)]);
        }
    }
    if let Some(t) = &sp.busy {
        b.gap(6.0);
        b.status(Mark::Busy, t);
    }
    for (mark, l) in &sp.lines {
        b.status(*mark, l);
    }
    if let Some(u) = &sp.undo {
        b.gap(6.0);
        let note = if u.restore.is_some() { "A backup was made first." } else { "" };
        b.message_with(Mark::Ok, &u.said, note, vec![btn("undo", "Undo", !busy)]);
    }
    import_section(b, m);
    let left_bottom = b.y;
    if let (Some(rc), Some(r)) = (cols.right, sp.row()) {
        b.in_column(rc);
        detail(b, m, r);
    }
    b.y = left_bottom.max(b.y);
    b.default_focus = Some(if sp.plan.is_some() {
        "import_go"
    } else if !sp.rows.is_empty() {
        "saves_list"
    } else if sp.found.as_ref().is_some_and(|f| f.iter().any(|x| x.plan.to_copy() > 0)) {
        "src0"
    } else {
        "pick"
    });
}

/// The original's saves: found sources, the plan, the pickers.
fn import_section(b: &mut Builder, m: &super::model::Model) {
    let sp = &m.saves;
    let busy = sp.busy.is_some();
    b.section_a("Import from the original game");
    if let Some(p) = &sp.plan {
        b.dim(&format!("From {}", p.from.display()));
        for i in &p.items {
            let mark = match i.act {
                Act::New | Act::Copy(_) => Mark::Ok,
                Act::Same => Mark::Info,
                Act::Bad(_) => Mark::Error,
            };
            b.status(mark, &i.line());
        }
        if p.items.is_empty() {
            b.status(
                Mark::Info,
                "No saves here. A save is a folder holding a file of the same name (NIGHTOWL/NIGHTOWL).",
            );
        }
        for s in &p.skipped {
            b.status(Mark::Info, &format!("Left out: {s}"));
        }
        let n = p.to_copy();
        b.gap(6.0);
        b.buttons(
            Align::Left,
            vec![
                Ctrl::Button {
                    id: "import_go",
                    text: if n == 1 { "Import 1 save".into() } else { format!("Import {n} saves") },
                    primary: true,
                    enabled: n > 0 && !busy,
                },
                btn("import_cancel", "Cancel", true),
            ],
        );
        b.gap(6.0);
        b.dim("Nothing is overwritten, and the original's files are only read.");
        return;
    }
    match &sp.found {
        None => {}
        Some(f) if f.is_empty() => b.dim("No saves of the original game were found on this computer."),
        Some(f) => {
            for (i, x) in f.iter().take(SOURCE_IDS.len()).enumerate() {
                let n = x.plan.to_copy();
                let text = if n == 1 { "Import 1 save…".into() } else { format!("Import {n} saves…") };
                b.status(
                    if n > 0 { Mark::Ok } else { Mark::Info },
                    &format!("{}: {}", x.source.label, x.plan.counts()),
                );
                b.gap(6.0);
                b.buttons(
                    Align::Left,
                    vec![Ctrl::Button { id: SOURCE_IDS[i], text, primary: false, enabled: n > 0 && !busy }],
                );
                b.gap(6.0);
            }
        }
    }
    b.gap(12.0);
    b.buttons(
        Align::Left,
        vec![
            btn("find", "Find saves", !busy),
            btn("pick", "Choose folder…", !busy),
            btn("pickzip", "Choose zip…", !busy),
        ],
    );
}

/// The chosen profile at the right: covers, career, backups, actions.
fn detail(b: &mut Builder, m: &super::model::Model, r: &Row) {
    let s = &r.s;
    b.section_a(&s.name);
    if let Some(p) = &s.problem {
        b.status(Mark::Error, &format!("The game can't load this file ({p})."));
    } else {
        b.body(&format!("{} · {} · {}", stage_label(s.stage), money(s.cash), r.car));
        b.dim(&format!(
            "Races {} · Reputation {}",
            summary::grouped(u64::from(s.races)),
            summary::grouped(u64::from(s.reputation)),
        ));
        if let Some(n) = &s.payload_name {
            b.dim(&format!("The game shows it as {n}."));
        }
    }
    covers(b, r);
    let bk = m.saves.backups_of(&s.name);
    b.dim(&match bk.first() {
        Some(n) => format!("{} (newest {})", count(bk.len(), "backup", "backups"), n.when()),
        None => "No backups yet".into(),
    });
    b.gap(6.0);
    actions(b, m, r);
}

/// The saved magazine covers and DVD pictures: a heading, then the newest
/// ones small with their captions (a placeholder that says so when a
/// picture can't be read), and how many more there are.
fn covers(b: &mut Builder, r: &Row) {
    let n = r.s.covers.len();
    b.gap(10.0);
    if n == 0 {
        b.dim("No saved covers");
        return;
    }
    b.dim(&format!("Saved covers ({n})"));
    b.gap(4.0);
    let t = super::draw::theme();
    let side = b.a(88.0).round();
    let gap = b.a(14.0).round();
    let fit = (((b.width() + gap) / (side + gap)).floor() as usize).max(1);
    let size = b.m.small();
    let lh = b.fonts.line_height(super::font::Face::Regular, size);
    let y = b.y;
    for (i, c) in r.thumbs.iter().take(fit).enumerate() {
        let x = b.x0 + i as f32 * (side + gap);
        let rect = Rect::new(x, y, side, side);
        let mut texts = Vec::new();
        let rgba = match &c.rgba {
            Some(p) => p.clone(),
            None => {
                // The placeholder: the frame and what is missing, centred.
                let m =
                    b.fonts.fit(super::font::Face::Regular, size, &c.missing, side - b.a(8.0), super::font::Cut::End);
                let w = b.fonts.width(super::font::Face::Regular, size, &m);
                let tr = Rect::new((x + (side - w) / 2.0).round(), (y + (side - lh) / 2.0).round(), w, lh);
                texts.push(super::ui::text(tr, m, super::font::Face::Regular, size, t.dim));
                Arc::new(Vec::new())
            }
        };
        b.widgets.push(Widget {
            id: None,
            rect,
            kind: Kind::Picture { rgba, size: summary::THUMB },
            texts,
            enabled: true,
        });
        let cap =
            b.fonts.fit(super::font::Face::Regular, size, &c.caption, side + gap - b.a(4.0), super::font::Cut::End);
        let w = b.fonts.width(super::font::Face::Regular, size, &cap);
        let cr = Rect::new(x, y + side + b.a(4.0), w, lh);
        b.widgets.push(Widget {
            id: None,
            rect: cr,
            kind: Kind::Label,
            texts: vec![super::ui::text(cr, cap, super::font::Face::Regular, size, t.dim)],
            enabled: true,
        });
    }
    b.y = y + side + b.a(4.0) + lh;
    let shown = r.thumbs.len().min(fit);
    if n > shown {
        b.gap(2.0);
        b.dim(&format!("and {} more", n - shown));
    }
    b.gap(8.0);
}

/// The actions on the chosen profile (and its backups list, the rename
/// field): the two everyday ones and More…, which opens the rest grouped
/// (name, copies, delete).
fn actions(b: &mut Builder, m: &super::model::Model, r: &Row) {
    let sp = &m.saves;
    let on = sp.busy.is_none();
    let ok = r.s.problem.is_none();
    if let Some(t) = &sp.renaming {
        b.buttons(
            Align::Left,
            vec![Ctrl::Field { id: "rename_field", text: t.clone(), placeholder: "New name".into() }],
        );
        b.gap(12.0);
        b.buttons(Align::Left, vec![btn("rename_ok", "Rename", on), btn("rename_cancel", "Cancel", true)]);
        b.gap(6.0);
        b.dim("1 to 7 letters or digits. The game shows the new name.");
        return;
    }
    b.buttons(
        Align::Left,
        vec![
            btn("backup", "Back up", on),
            btn("backups", if sp.show_backups { "Hide backups" } else { "Backups…" }, true),
            btn("more", if sp.more { "Less" } else { "More…" }, true),
        ],
    );
    if sp.more {
        b.gap(12.0);
        b.buttons(Align::Left, vec![btn("rename", "Rename…", on && ok), btn("duplicate", "Duplicate", on && ok)]);
        b.gap(12.0);
        b.buttons(Align::Left, vec![btn("export", "Export…", on), btn("backup_all", "Back up all", on)]);
        b.gap(12.0);
        b.buttons(Align::Left, vec![btn("delete", "Delete…", on)]);
    }
    if sp.show_backups {
        let bk = sp.backups_of(&r.s.name);
        b.gap(12.0);
        if bk.is_empty() {
            b.dim("No backups of this profile yet.");
        } else {
            let items: Vec<(String, String)> = bk.iter().map(|x| backup_text(x)).collect();
            b.list("backups_list", &items, sp.backup_sel, items.len().min(4));
            b.gap(12.0);
            b.buttons(Align::Left, vec![btn("restore", "Restore", on), btn("del_backup", "Delete backup…", on)]);
        }
    }
}

/// The dialogs of the page.
pub fn dialog(b: &mut Builder, m: &super::model::Model) {
    match &m.ask {
        Some(Dialog::SaveDelete(name)) => {
            let covers = m.saves.rows.iter().find(|r| &r.s.name == name).map_or(0, |r| r.s.covers.len());
            let with = match (covers, super::saves::store::DELETE_COVERS) {
                (0, _) | (_, false) => String::new(),
                (n, true) => format!("Its {} go with it. ", count(n, "saved cover", "saved covers")),
            };
            b.dialog(
                &format!("Delete {name}?"),
                &format!("{with}A backup is made first, and Undo brings it back."),
                &[("save_cancel", "Cancel"), ("delete_yes", "Delete")],
            );
        }
        Some(Dialog::SaveDuplicate(name)) => b.dialog(
            &format!("Make a copy of {name}?"),
            "The copy gets the next free name (with a number) and its own saved covers.",
            &[("save_cancel", "Cancel"), ("duplicate_yes", "Duplicate")],
        ),
        Some(Dialog::SaveRestore(bk)) => b.dialog(
            &format!("Restore {} from {}?", bk.name, bk.when()),
            &format!(
                "{} is already there. A copy gets the next free name; Replace backs up the current {} first.",
                bk.name, bk.name
            ),
            &[
                ("restore_copy", "Restore as a copy"),
                ("restore_replace", &format!("Replace {}", bk.name)),
                ("save_cancel", "Cancel"),
            ],
        ),
        Some(Dialog::SaveDeleteBackup(bk)) => b.dialog(
            "Delete this backup?",
            &format!("The backup of {} from {} is deleted for good. Your profile stays.", bk.name, bk.when()),
            &[("save_cancel", "Keep it"), ("del_backup_yes", "Delete backup")],
        ),
        _ => {}
    }
}

/// The page's legends.
pub fn legends(v: &mut Vec<super::legends::Action>) {
    use super::legends::{Btn, action};
    v.push(action(Btn::X, "Actions"));
    v.push(action(Btn::Y, "Back up"));
    v.push(action(Btn::Pages, "Pages"));
    v.push(action(Btn::Back, "Back"));
}

impl<'e> App<'e> {
    /// Opens the page; reads the profiles (and looks for the original's
    /// saves the first time).
    pub fn open_saves(&mut self) {
        self.go(Screen::Saves);
        let first = !self.model.saves.loaded && self.model.saves.found.is_none();
        self.saves_job(Job::Load { find: first });
    }

    /// Runs `job`: on a worker where the environment has one (the window),
    /// else at once.
    pub fn saves_job(&mut self, job: Job) {
        if self.model.saves.busy.is_some() {
            return;
        }
        let words = match &job {
            Job::Load { find: true } => "Reading your profiles and looking for the original game's saves…",
            Job::Load { .. } => "Reading your profiles…",
            Job::Plan(_) => "Reading the saves…",
            Job::Import(_) => "Importing…",
            Job::Backup(_) | Job::BackupAll => "Backing up…",
            Job::Restore(..) => "Restoring…",
            Job::Export(..) => "Exporting…",
            Job::Rename(..) => "Renaming…",
            Job::Duplicate(_) => "Copying…",
            Job::Delete(_) => "Backing up, then deleting…",
            Job::Undo(_) => "Undoing…",
            Job::DeleteBackup(_) => "Deleting the backup…",
        };
        if !matches!(job, Job::Load { .. }) {
            self.model.saves.undo = None;
            self.model.saves.undo_until = None;
        }
        self.model.saves.lines.clear();
        match self.env.saves_ctx(self.model.root.clone()) {
            Some(ctx) => {
                let ctx = self.saves_ctx.get_or_insert(ctx);
                ctx.sources.game = self.model.root.clone();
                let ctx = ctx.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                let spawned = std::thread::Builder::new().name("saves".into()).spawn(move || {
                    let _ = tx.send(jobs::run(&ctx, job));
                });
                if spawned.is_ok() {
                    self.model.saves.busy = Some(words.into());
                    self.saves_rx = Some(rx);
                } else {
                    self.model.saves.lines = vec![(Mark::Error, "The work could not be started.".into())];
                }
            }
            None => {
                let d = self.env.saves_run(job);
                self.model.saves.done(d);
            }
        }
    }

    /// The worker's result taken in; Undo's offer timed out. True when the
    /// page changed.
    pub fn poll_saves(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.saves_rx {
            match rx.try_recv() {
                Ok(d) => {
                    self.saves_rx = None;
                    self.model.saves.done(d);
                    changed = true;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.saves_rx = None;
                    self.model.saves.busy = None;
                    self.model.saves.lines = vec![(Mark::Error, "The work stopped unexpectedly.".into())];
                    changed = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.model.saves.undo_until.is_some_and(|t| Instant::now() >= t) {
            self.model.saves.undo = None;
            self.model.saves.undo_until = None;
            changed = true;
        }
        changed
    }

    /// When the window must look again (Undo's end).
    pub fn saves_wake(&self) -> Option<Instant> {
        self.model.saves.undo_until
    }

    fn sel_name(&self) -> Option<String> {
        self.model.saves.row().map(|r| r.s.name.clone())
    }

    fn sel_backup(&self) -> Option<Backup> {
        let name = self.sel_name()?;
        self.model.saves.backups_of(&name).get(self.model.saves.backup_sel).map(|b| (*b).clone())
    }

    /// The page's dialogs' buttons (None: not one of them).
    pub fn saves_dialog_event(&mut self, e: &Event) -> Option<Outcome> {
        let ask = self.model.ask.clone()?;
        if !matches!(
            ask,
            Dialog::SaveDelete(_) | Dialog::SaveDuplicate(_) | Dialog::SaveRestore(_) | Dialog::SaveDeleteBackup(_)
        ) {
            return None;
        }
        let focus_back = match &ask {
            Dialog::SaveDelete(_) => "saves_list",
            Dialog::SaveDuplicate(_) => "duplicate",
            Dialog::SaveRestore(_) => "restore",
            _ => "del_backup",
        };
        match (ask, e) {
            (Dialog::SaveDelete(n), Event::Activate("delete_yes")) => {
                self.model.ask = None;
                self.saves_job(Job::Delete(n));
                self.st.focus = Some("saves_list");
            }
            (Dialog::SaveDuplicate(n), Event::Activate("duplicate_yes")) => {
                self.model.ask = None;
                self.saves_job(Job::Duplicate(n));
                self.st.focus = Some("saves_list");
            }
            (Dialog::SaveRestore(bk), Event::Activate(id @ ("restore_copy" | "restore_replace"))) => {
                self.model.ask = None;
                self.saves_job(Job::Restore(bk, if *id == "restore_copy" { How::AsCopy } else { How::Replace }));
                self.st.focus = Some("saves_list");
            }
            (Dialog::SaveDeleteBackup(bk), Event::Activate("del_backup_yes")) => {
                self.model.ask = None;
                self.saves_job(Job::DeleteBackup(bk));
                self.st.focus = Some("backups");
            }
            (_, Event::Activate(_) | Event::Back) => {
                self.model.ask = None;
                self.st.focus = Some(focus_back);
            }
            _ => {}
        }
        Some(Outcome::Stay)
    }

    /// The page's events (None: not handled here).
    pub fn saves_event(&mut self, e: &Event) -> Option<Outcome> {
        let sp = &mut self.model.saves;
        match e {
            Event::Select("saves_list", i) => {
                if *i != sp.sel {
                    sp.show_backups = false;
                    sp.more = false;
                    sp.renaming = None;
                }
                sp.sel = *i;
                sp.backup_sel = 0;
            }
            Event::Select("backups_list", i) => sp.backup_sel = *i,
            // Enter on a profile: its actions.
            Event::Activate("saves_list") | Event::Activate("actions") => {
                sp.actions = true;
                self.st.focus = Some("backup");
            }
            Event::Activate("backups_list") => self.st.focus = Some("restore"),
            Event::Activate("find") => {
                sp.plan = None;
                self.saves_job(Job::Load { find: true });
            }
            Event::Activate("pick") => {
                let start = self.saves_start();
                self.choose_folder(BrowseFor::Saves, start);
            }
            Event::Activate("pickzip") => {
                let start = self.saves_start();
                self.browse(BrowseFor::Saves, start);
            }
            Event::Activate(id) if SOURCE_IDS.contains(id) => {
                let i = SOURCE_IDS.iter().position(|s| s == id).unwrap_or(0);
                if let Some(f) = sp.found.as_ref().and_then(|f| f.get(i)) {
                    sp.plan = Some(f.plan.clone());
                    self.st.focus = Some("import_go");
                }
            }
            Event::Activate("import_go") => {
                if let Some(p) = sp.plan.clone() {
                    self.saves_job(Job::Import(p));
                    self.st.focus = Some("saves_list");
                }
            }
            Event::Activate("import_cancel") => {
                sp.plan = None;
                self.st.focus = Some("find");
            }
            Event::Activate("backup") => {
                if let Some(n) = self.sel_name() {
                    self.saves_job(Job::Backup(n));
                }
            }
            Event::Activate("backup_all") => self.saves_job(Job::BackupAll),
            Event::Activate("backups") => sp.show_backups = !sp.show_backups,
            Event::Activate("more") => sp.more = !sp.more,
            Event::Activate("export") => {
                let start = super::dirs::home();
                self.choose_folder(BrowseFor::SavesExport, start);
            }
            Event::Activate("rename") => {
                sp.renaming = Some(String::new());
                self.st.cursor = 0;
                self.st.focus = Some("rename_field");
            }
            Event::Edit("rename_field", ed) => {
                if let Some(t) = sp.renaming.as_mut() {
                    super::ui::apply_edit(t, &mut self.st.cursor, ed);
                }
            }
            Event::Activate("rename_ok" | "rename_field") => {
                let to = sp.renaming.clone().unwrap_or_default().trim().to_string();
                if !super::saves::store::valid_name(&to) {
                    sp.lines = vec![(Mark::Error, "A name is 1 to 7 letters or digits.".into())];
                } else if let Some(from) = self.sel_name() {
                    let taken =
                        self.model.saves.rows.iter().any(|r| r.s.name.eq_ignore_ascii_case(&to) && r.s.name != from);
                    if taken {
                        self.model.saves.lines = vec![(Mark::Error, format!("{to} is already a profile."))];
                    } else {
                        self.model.saves.renaming = None;
                        self.saves_job(Job::Rename(from, to));
                        self.st.focus = Some("saves_list");
                    }
                }
            }
            Event::Activate("rename_cancel") => {
                sp.renaming = None;
                self.st.focus = Some("rename");
            }
            Event::Activate("duplicate") => {
                if let Some(n) = self.sel_name() {
                    self.model.ask = Some(Dialog::SaveDuplicate(n));
                }
            }
            Event::Activate("delete") => {
                if let Some(n) = self.sel_name() {
                    self.model.ask = Some(Dialog::SaveDelete(n));
                }
            }
            Event::Activate("restore") => {
                if let Some(bk) = self.sel_backup() {
                    let here = self.model.saves.rows.iter().any(|r| r.s.name.eq_ignore_ascii_case(&bk.name));
                    if here {
                        self.model.ask = Some(Dialog::SaveRestore(bk));
                    } else {
                        self.saves_job(Job::Restore(bk, How::Back));
                    }
                }
            }
            Event::Activate("del_backup") => {
                if let Some(bk) = self.sel_backup() {
                    self.model.ask = Some(Dialog::SaveDeleteBackup(bk));
                }
            }
            Event::Activate("undo") => {
                if let Some(u) = sp.undo.take() {
                    sp.undo_until = None;
                    self.saves_job(Job::Undo(u));
                    self.st.focus = Some("saves_list");
                }
            }
            Event::Back if sp.renaming.is_some() => {
                sp.renaming = None;
                self.st.focus = Some("rename");
            }
            Event::Back if sp.plan.is_some() => {
                sp.plan = None;
                self.st.focus = Some("find");
            }
            Event::Back if sp.actions || sp.show_backups || sp.more => {
                sp.actions = false;
                sp.more = false;
                sp.show_backups = false;
                self.st.focus = Some("saves_list");
            }
            Event::Back => self.go(Screen::Main),
            _ => return None,
        }
        Some(Outcome::Stay)
    }

    /// A key of the page: X Actions, Y Back up, Delete on a profile,
    /// F2 Rename, Ctrl+B / Ctrl+E Back up / Export, Ctrl+Z Undo.
    pub fn saves_key(&mut self, k: super::ui::Key) -> Option<Outcome> {
        use super::ui::Key;
        if self.model.ask.is_some() {
            return None;
        }
        let on_list = self.ui.focused(&self.st) == Some("saves_list");
        let id = match k {
            Key::ActionX => "actions",
            Key::ActionY | Key::BackUp => "backup",
            Key::Delete if on_list => "delete",
            Key::Rename => "rename",
            Key::Export => "export",
            Key::Undo if self.model.saves.undo.is_some() => "undo",
            _ => return None,
        };
        if self.model.saves.row().is_none() && id != "undo" {
            return Some(Outcome::Stay);
        }
        if matches!(id, "rename" | "export" | "delete") {
            self.model.saves.more = true;
            self.model.saves.actions = true;
        }
        Some(self.event(&Event::Activate(id)))
    }

    /// A folder (or a zip) chosen for the page: the import's plan, or the
    /// export's destination.
    pub fn saves_folder(&mut self, purpose: BrowseFor, dir: PathBuf) {
        self.go(Screen::Saves);
        match purpose {
            BrowseFor::SavesExport => {
                if let Some(n) = self.sel_name() {
                    self.saves_job(Job::Export(n, dir));
                }
            }
            _ => self.saves_job(Job::Plan(dir)),
        }
    }

    /// Where the save pickers start: the original's folder when known,
    /// inside the Wine prefix of the game folder in use (`drive_c/users`),
    /// else the home folder.
    pub fn saves_start(&self) -> Option<PathBuf> {
        if let Some(d) = self.env.original_saves().filter(|d| d.is_dir()) {
            return Some(d);
        }
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
}

/// A saved cover in the renders: no picture (the renders carry nothing of
/// the game's), its placeholder saying so; odd numbers DVDs, even magazines
/// as `fake_row` names them.
pub fn fake_cover(n: u32) -> jobs::CoverPic {
    jobs::CoverPic {
        caption: format!("{} {n}", if n % 2 == 1 { "DVD" } else { "Magazine" }),
        rgba: None,
        missing: "Cover".into(),
    }
}

/// The renders' clock: 18 Nov 2026, 12:00 UTC.
pub const FAKE_NOW: u64 = 1_795_003_200;

fn fake_row(name: &str, stage: u32, cash: u32, car: &str, age_s: u64, covers: u32, races: u32, rep: u32) -> Row {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(FAKE_NOW);
    let cv: Vec<summary::Cover> = (1..=covers)
        .map(|n| summary::Cover {
            name: format!("{name} {} {n}", if n % 2 == 1 { "DVD" } else { "Magazine" }),
            dvd: n % 2 == 1,
            n,
            bytes: 1_048_612,
        })
        .collect();
    Row {
        s: summary::Summary {
            name: name.into(),
            payload_name: None,
            file: PathBuf::from("/data/player/.local/share/NFS Underground 2").join(name).join(name),
            stage,
            cash,
            races,
            reputation: rep,
            car: summary::Car::Hash(0),
            modified: now.checked_sub(Duration::from_secs(age_s)),
            bytes: 54_966,
            covers: cv,
            problem: None,
        },
        car: car.into(),
        thumbs: (1..=covers.min(jobs::THUMBS_SHOWN as u32)).map(|n| fake_cover(n)).collect(),
    }
}

fn fake_rows() -> Vec<Row> {
    vec![
        fake_row("NIGHTOWL", 3, 48_200, "NISSAN 350Z", 2 * 86_400 + 600, 3, 112, 15_400),
        fake_row("TUNER", 1, 3_100, "PEUGEOT 206", 8 * 86_400, 0, 14, 2_150),
        fake_row("ROOKIE", 0, 0, "No car yet", 30, 0, 0, 0),
    ]
}

fn fake_backups() -> Vec<Backup> {
    use super::saves::store::Why;
    let dir = PathBuf::from("/data/player/.local/share/NFS Underground 2 Backups/NIGHTOWL");
    vec![
        Backup {
            path: dir.join("launcher-20261117-101500"),
            name: "NIGHTOWL".into(),
            stamp: "20261117-101500".into(),
            why: Some(Why::Manual),
            covers: 3,
        },
        Backup {
            path: dir.join("NIGHTOWL.20261116-214000"),
            name: "NIGHTOWL".into(),
            stamp: "20261116-214000".into(),
            why: None,
            covers: 0,
        },
    ]
}

/// The fake plan: one profile already here, one new, one a name clash.
pub fn fake_plan(from: PathBuf) -> Plan {
    use super::saves::PlanItem;
    use super::saves::store::{FileCopy, Files};
    let item = |name: &str, covers: usize, act: Act| PlanItem {
        files: Files {
            profile: FileCopy { name: name.into(), bytes: Vec::new(), mtime: None },
            covers: (1..=covers)
                .map(|n| FileCopy { name: format!("{name} DVD {n}"), bytes: Vec::new(), mtime: None })
                .collect(),
        },
        act,
    };
    Plan {
        from,
        zip: false,
        items: vec![
            item("NIGHTOWL", 3, Act::Same),
            item("RACER", 1, Act::New),
            item("TUNER", 0, Act::Copy("TUNER2".into())),
        ],
        loose: Vec::new(),
        skipped: Vec::new(),
    }
}

fn fake_found() -> Vec<Found> {
    vec![Found {
        source: super::saves::sources::Source {
            path: PathBuf::from(super::model::FAKE_SAVES),
            label: "the game's Wine prefix ~/.wine".into(),
        },
        plan: fake_plan(PathBuf::from(super::model::FAKE_SAVES)),
    }]
}

/// The fake data's page (`FakeEnv::load`).
pub fn fake_page() -> SavesPage {
    SavesPage {
        rows: fake_rows(),
        backups: fake_backups(),
        found: Some(fake_found()),
        loaded: true,
        now: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(FAKE_NOW)),
        ..Default::default()
    }
}

/// The fake data's jobs: no files, what the real ones would answer.
pub fn fake_run(job: Job) -> Done {
    let mut d = Done { rows: fake_rows(), backups: fake_backups(), ..Default::default() };
    match job {
        Job::Load { find } => d.found = find.then(fake_found),
        Job::Plan(from) => d.plan = Some(fake_plan(from)),
        Job::Import(_) => {
            d.rows.push(fake_row("RACER", 2, 9_800, "MAZDA RX-8", 40 * 86_400, 1, 30, 4_200));
            d.rows.push(fake_row("TUNER2", 1, 1_200, "PEUGEOT 206", 60 * 86_400, 0, 9, 800));
            d.lines = vec![(Mark::Ok, "Imported 2 profiles.".into())];
            d.select = Some("RACER".into());
            d.imported = true;
            d.found = Some(Vec::new());
        }
        Job::Backup(n) => d.lines = vec![(Mark::Ok, format!("{n}: backed up."))],
        Job::BackupAll => d.lines = vec![(Mark::Ok, "Backed up 3 profiles.".into())],
        Job::Restore(b, how) => {
            let n = if how == How::AsCopy { format!("{}2", b.name) } else { b.name.clone() };
            d.lines =
                vec![(Mark::Ok, if n == b.name { format!("{n}: restored.") } else { format!("Restored as {n}.") })];
            d.select = Some(n);
        }
        Job::Export(n, to) => {
            d.lines = vec![(Mark::Ok, format!("{n}: exported to {}", to.join(format!("{n}.zip")).display()))]
        }
        Job::Rename(from, to) => {
            for r in &mut d.rows {
                if r.s.name == from {
                    r.s.name = to.clone();
                }
            }
            d.undo = Some(Undo { remove: Some(to.clone()), restore: None, said: format!("Renamed {from} to {to}") });
            d.select = Some(to);
        }
        Job::Duplicate(n) => {
            let mut c = d.rows.iter().find(|r| r.s.name == n).cloned().unwrap_or_else(|| fake_rows()[0].clone());
            c.s.name = format!("{n}2");
            d.select = Some(c.s.name.clone());
            d.undo = Some(Undo {
                remove: Some(c.s.name.clone()),
                restore: None,
                said: format!("Made {}, a copy of {n}", c.s.name),
            });
            d.rows.push(c);
        }
        Job::Delete(n) => {
            d.rows.retain(|r| r.s.name != n);
            d.undo = Some(Undo { remove: None, restore: None, said: format!("Deleted {n}") });
        }
        Job::Undo(u) => d.lines = vec![(Mark::Ok, format!("Undone: {}.", u.said))],
        Job::DeleteBackup(b) => {
            d.backups.retain(|x| x.path != b.path);
            d.lines = vec![(Mark::Ok, format!("The backup of {} from {} was deleted.", b.name, b.when()))];
        }
    }
    d
}
