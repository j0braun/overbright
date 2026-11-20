//! A mod's history on disk: nothing done in Studio is lost, and only one program writes a mod at a time.
//!
//! - **Documents and transactions**: a mod's state is the text of each of its files ([`doc::State`]); a step
//!   changes one or more of them at once. A change made against an older step is merged item by item, and only
//!   an item changed both ways is a conflict ([`History::transact`]).
//! - **Journal**: every step, with its author, is appended to `.editor/history.journal` and flushed to the disk
//!   before it counts ([`journal`]), so a crash or `kill -9` loses at most the step being made.
//! - **Working state and saving**: the mod's files are written at quiet points ([`History::tick`]: a short
//!   pause after the last step, or at the latest every [`LONGEST_UNSAVED`] while editing goes on) and at once on
//!   [`History::save`] (Ctrl+S), each through a temporary file and a rename. Opening a mod whose journal is
//!   ahead of its files offers the unsaved steps back ([`History::pending`]).
//! - **Undo, revert, versions**: undo and redo take back the last step whoever made it; [`History::revert`]
//!   takes back any kept step, item by item; [`History::mark_version`] keeps the state under a name.
//! - **One writer and the inbox**: the writer holds the mod's lock ([`lock`]); a second opener gets the mod
//!   read-only, with who holds it. Other programs send changes through the [`inbox`]; files changed by another
//!   program behind Studio's back become steps under [`OUTSIDE`].
//!
//! All file access goes through [`disk::Disk`], so the tests stop the history at every write (fault injection)
//! and check that the mod reads back whole, as before or after that write.

pub mod disk;
pub mod doc;
pub mod inbox;
pub mod journal;
pub mod lock;

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use disk::Disk;
use doc::{Prefer, State};
use journal::Rec;

/// The journal's file, in the mod's editor folder.
pub const JOURNAL: &str = "history.journal";
/// How many steps are kept (as the editor's undo kept).
pub const MAX_STEPS: usize = 256;
/// The journal is written afresh (only what is kept) once it has this many records.
pub const COMPACT_AT: usize = 4 * MAX_STEPS;
/// The mod's files are written this long after the last step.
pub const QUIET: Duration = Duration::from_secs(2);
/// … and at the latest this long after the first unsaved step.
pub const LONGEST_UNSAVED: Duration = Duration::from_secs(30);
/// Studio's own steps.
pub const STUDIO: &str = "Overbright Studio";
/// Changes found in the mod's files that no step made (another program wrote them).
pub const OUTSIDE: &str = "outside Studio";

/// Unix time in ms.
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// One kept step.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub seq: u64,
    /// Unix time, ms.
    pub at: u64,
    pub author: String,
    pub label: String,
    pub before: State,
    pub after: State,
    /// The inbox request it answers.
    pub request: Option<String>,
}

/// A state kept under a name.
#[derive(Clone, Debug, PartialEq)]
pub struct Version {
    pub at: u64,
    pub name: String,
    pub docs: State,
}

/// Unsaved steps found on opening: the journal is ahead of the mod's files.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    /// When the last of them was made (Unix time, ms).
    pub at: u64,
    /// How many items differ from the files.
    pub items: usize,
}

/// What a change came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Made as step `seq`.
    Made(u64),
    /// Nothing to change.
    Unchanged,
    /// Not made: these items (`doc#key`) changed since the base in another way.
    Conflict(Vec<String>),
    /// Not made (the base is no longer kept).
    Refused(String),
}

/// Why the history cannot make a change.
#[derive(Debug)]
pub enum Error {
    /// Another program has the mod open for writing.
    ReadOnly(Option<lock::Owner>),
    Io(io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ReadOnly(Some(o)) => write!(f, "read-only: {} (process {}) has this mod open", o.program, o.pid),
            Error::ReadOnly(None) => write!(f, "read-only: another program has this mod open"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// What [`History::tick`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ticked {
    /// The mod's files were written.
    pub saved: bool,
    /// Steps made from changes another program wrote into the files.
    pub outside: Vec<u64>,
    /// Inbox requests answered: (id, reply).
    pub answered: Vec<(String, inbox::Reply)>,
    /// The read-only view was read again (another writer made steps).
    pub reloaded: bool,
}

/// What the replay of a journal gives.
#[derive(Clone, Debug, Default)]
struct Replay {
    base_seq: u64,
    base: State,
    past: Vec<Step>,
    future: Vec<Step>,
    current: State,
    next_seq: u64,
    versions: Vec<Version>,
    done: BTreeSet<String>,
    /// The documents' hashes as last written, and the state they were written from (when the record matches it).
    saved_hashes: Option<BTreeMap<String, u64>>,
    saved_state: Option<State>,
    records: usize,
}

impl Replay {
    fn apply(&mut self, r: Rec) {
        self.records += 1;
        match r {
            Rec::Base { seq, at: _, docs, done } => {
                self.base_seq = seq;
                self.base = docs.clone();
                self.current = docs.clone();
                self.past.clear();
                self.future.clear();
                self.next_seq = seq + 1;
                self.done.extend(done);
                self.saved_state = Some(docs);
                self.saved_hashes = None;
            }
            Rec::Step { seq, at, author, label, docs, request } => {
                let before = self.current.clone();
                let after = with(&before, &docs);
                if let Some(id) = &request {
                    self.done.insert(id.clone());
                }
                self.current = after.clone();
                self.future.clear();
                self.past.push(Step { seq, at, author, label, before, after, request });
                self.next_seq = self.next_seq.max(seq + 1);
                self.cap();
            }
            Rec::Undo { .. } => {
                if let Some(s) = self.past.pop() {
                    self.current = s.before.clone();
                    self.future.push(s);
                }
            }
            Rec::Redo { .. } => {
                if let Some(s) = self.future.pop() {
                    self.current = s.after.clone();
                    self.past.push(s);
                }
            }
            Rec::Saved { docs, .. } => {
                self.saved_state = (hashes(&self.current) == docs).then(|| self.current.clone());
                self.saved_hashes = Some(docs);
            }
            Rec::Version { at, name, docs } => {
                self.versions.retain(|v| v.name != name);
                self.versions.push(Version { at, name, docs });
            }
        }
    }

    fn cap(&mut self) {
        while self.past.len() > MAX_STEPS {
            let s = self.past.remove(0);
            self.base_seq = s.seq;
            self.base = s.after;
        }
    }

    fn seq(&self) -> u64 {
        self.past.last().map_or(self.base_seq, |s| s.seq)
    }

    /// The state after step `seq`, if kept.
    fn state_at(&self, seq: u64) -> Option<&State> {
        if seq == self.base_seq {
            return Some(&self.base);
        }
        self.past.iter().chain(&self.future).find(|s| s.seq == seq).map(|s| &s.after)
    }

    fn saved_hashes(&self) -> BTreeMap<String, u64> {
        self.saved_hashes.clone().unwrap_or_else(|| hashes(self.saved_state.as_ref().unwrap_or(&self.base)))
    }
}

/// `s` with `docs` put in (None removes).
fn with(s: &State, docs: &BTreeMap<String, Option<String>>) -> State {
    let mut out = s.clone();
    for (k, v) in docs {
        match v {
            Some(t) => out.insert(k.clone(), t.clone()),
            None => out.remove(k),
        };
    }
    out
}

/// The documents of `to` that differ from `from` (None: removed).
fn diff(from: &State, to: &State) -> BTreeMap<String, Option<String>> {
    let mut out = BTreeMap::new();
    for (k, v) in to {
        if from.get(k) != Some(v) {
            out.insert(k.clone(), Some(v.clone()));
        }
    }
    for k in from.keys() {
        if !to.contains_key(k) {
            out.insert(k.clone(), None);
        }
    }
    out
}

fn hashes(s: &State) -> BTreeMap<String, u64> {
    s.iter().map(|(k, v)| (k.clone(), journal::hash(v))).collect()
}

/// A file's size and time, to notice another program's write cheaply.
type Stamp = Option<(u64, Option<SystemTime>)>;

fn stamp(path: &Path) -> Stamp {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))
}

/// Whether the mod is open for writing.
#[derive(Debug)]
enum Access {
    Write(#[allow(dead_code)] lock::WriteLock),
    ReadOnly(Option<lock::Owner>),
}

/// One mod's history, open for writing or read-only.
pub struct History {
    disk: Arc<dyn Disk>,
    /// The mod's folder (its documents are files in it).
    dir: PathBuf,
    /// The mod's editor folder (journal, lock, inbox).
    edir: PathBuf,
    /// The documents it keeps (file names in `dir`).
    docs: Vec<String>,
    access: Access,
    r: Replay,
    /// The documents as the files on disk have them (as last read or written).
    files: State,
    stamps: BTreeMap<String, Stamp>,
    pending: Option<Pending>,
    /// When the first step not yet in the files and the last step were made.
    unsaved_since: Option<Instant>,
    last_change: Option<Instant>,
    /// The journal's length as last seen (a read-only view reads it again when it grows).
    journal_len: u64,
}

impl History {
    /// Open the history of the mod in `dir` (its documents: `docs`, file names inside it; its editor folder:
    /// `edir`), for writing when no other program has it, else read-only.
    pub fn open(dir: &Path, edir: &Path, docs: &[&str], program: &str, disk: Arc<dyn Disk>) -> Result<History> {
        let access = match lock::try_take(edir, program)? {
            Ok(l) => Access::Write(l),
            Err(owner) => Access::ReadOnly(owner),
        };
        let mut h = History {
            disk,
            dir: dir.to_path_buf(),
            edir: edir.to_path_buf(),
            docs: docs.iter().map(|d| d.to_string()).collect(),
            access,
            r: Replay::default(),
            files: State::new(),
            stamps: BTreeMap::new(),
            pending: None,
            unsaved_since: None,
            last_change: None,
            journal_len: 0,
        };
        h.load()?;
        Ok(h)
    }

    /// Open the history of the mod in `dir` read-only, without taking its lock or writing anything (what a
    /// look or a dry run reads: the mod as its writer has it now, unsaved steps included).
    pub fn open_read(dir: &Path, edir: &Path, docs: &[&str], disk: Arc<dyn Disk>) -> Result<History> {
        let mut h = History {
            disk,
            dir: dir.to_path_buf(),
            edir: edir.to_path_buf(),
            docs: docs.iter().map(|d| d.to_string()).collect(),
            access: Access::ReadOnly(lock::owner(edir)),
            r: Replay::default(),
            files: State::new(),
            stamps: BTreeMap::new(),
            pending: None,
            unsaved_since: None,
            last_change: None,
            journal_len: 0,
        };
        h.load()?;
        Ok(h)
    }

    /// Read the journal and the files; as the writer, mend a cut journal and take in outside changes.
    fn load(&mut self) -> Result<()> {
        let jpath = self.edir.join(JOURNAL);
        let bytes = match self.disk.read(&jpath) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        let (recs, used) = journal::decode(&bytes);
        let mut r = Replay::default();
        for rec in recs {
            r.apply(rec);
        }
        self.journal_len = used as u64;
        self.files = self.read_files()?;
        self.stamps = self.docs.iter().map(|d| (d.clone(), stamp(&self.dir.join(d)))).collect();
        let writer = self.writable();
        if writer && used < bytes.len() {
            // A record cut by a crash: what follows it could never be read.
            self.disk.truncate(&jpath, used as u64)?;
        }
        let fresh = r.records == 0;
        self.r = r;
        if fresh {
            // No journal yet (or not one whole record): the history starts at the files.
            self.r.apply(Rec::Base { seq: 0, at: now_ms(), docs: self.files.clone(), done: Vec::new() });
            self.r.records = 0;
            if writer {
                self.append(&Rec::Base { seq: 0, at: now_ms(), docs: self.files.clone(), done: Vec::new() })?;
            }
            return Ok(());
        }
        if self.files == self.r.current {
            if writer && hashes(&self.files) != self.r.saved_hashes() {
                // Written, but the crash came before the journal said so.
                self.append(&Rec::Saved { at: now_ms(), docs: hashes(&self.files) })?;
                self.r.saved_state = Some(self.files.clone());
                self.r.saved_hashes = Some(hashes(&self.files));
            }
            return Ok(());
        }
        let saved = self.r.saved_hashes();
        // Per document: as last written (the journal is ahead), or changed by another program.
        let outside: Vec<&String> = self
            .docs
            .iter()
            .filter(|d| self.files.get(*d).map(|t| journal::hash(t)) != saved.get(*d).copied())
            .collect();
        if !outside.is_empty() && writer {
            self.take_outside()?;
        }
        if self.r.current != self.files {
            self.pending = Some(Pending {
                at: self.r.past.last().map_or(0, |s| s.at),
                items: doc::changed_items(&self.files, &self.r.current).len(),
            });
        }
        Ok(())
    }

    fn read_files(&self) -> Result<State> {
        let mut s = State::new();
        for d in &self.docs {
            match self.disk.read(&self.dir.join(d)) {
                Ok(b) => {
                    s.insert(d.clone(), String::from_utf8_lossy(&b).into_owned());
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(s)
    }

    /// Make the files' changes that no step made one step under [`OUTSIDE`], merged item by item with the
    /// unsaved steps (the files win an item changed both ways; undo gets the other version back).
    fn take_outside(&mut self) -> Result<Option<u64>> {
        let base = self.r.saved_state.clone().unwrap_or_else(|| self.r.current.clone());
        let (merged, _) = doc::merge(&base, &self.r.current, &self.files, Prefer::Theirs);
        let names: Vec<String> = diff(&base, &self.files).into_keys().collect();
        let label = format!("Changed outside Studio: {}", names.join(", "));
        let seq = self.step(OUTSIDE, &label, &merged, None)?;
        // The files are what they are; the journal now knows them as written.
        self.mark_saved(self.files.clone())?;
        Ok(seq)
    }

    /// The mod is open for writing.
    pub fn writable(&self) -> bool {
        matches!(self.access, Access::Write(_))
    }

    /// Who holds the mod when it is read-only.
    pub fn read_only_by(&self) -> Option<Option<&lock::Owner>> {
        match &self.access {
            Access::ReadOnly(o) => Some(o.as_ref()),
            Access::Write(_) => None,
        }
    }

    /// Try again to open for writing (the other program may have closed the mod). True when writable now.
    pub fn retry_write(&mut self, program: &str) -> Result<bool> {
        if self.writable() {
            return Ok(true);
        }
        match lock::try_take(&self.edir, program)? {
            Ok(l) => {
                self.access = Access::Write(l);
                self.pending = None;
                self.load()?;
                Ok(true)
            }
            Err(o) => {
                self.access = Access::ReadOnly(o);
                Ok(false)
            }
        }
    }

    fn need_write(&self) -> Result<()> {
        match &self.access {
            Access::Write(_) => Ok(()),
            Access::ReadOnly(o) => Err(Error::ReadOnly(o.clone())),
        }
    }

    fn append(&mut self, r: &Rec) -> Result<()> {
        let bytes = journal::encode(r);
        self.disk.append(&self.edir.join(JOURNAL), &bytes)?;
        self.journal_len += bytes.len() as u64;
        self.r.records += 1;
        Ok(())
    }

    /// The current state.
    pub fn state(&self) -> &State {
        &self.r.current
    }

    /// One document's text now.
    pub fn doc(&self, name: &str) -> Option<&str> {
        self.r.current.get(name).map(String::as_str)
    }

    /// The step the current state is at (what an inbox request names as its base).
    pub fn seq(&self) -> u64 {
        self.r.seq()
    }

    /// The kept steps, oldest first (what undo takes back, last first).
    pub fn steps(&self) -> &[Step] {
        &self.r.past
    }

    /// The steps redo makes again, next last.
    pub fn undone(&self) -> &[Step] {
        &self.r.future
    }

    pub fn versions(&self) -> &[Version] {
        &self.r.versions
    }

    /// Unsaved steps found on opening, until [`restore`](Self::restore) or [`discard`](Self::discard).
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// Steps not yet in the mod's files.
    pub fn unsaved(&self) -> bool {
        self.r.current != self.files
    }

    /// Keep the unsaved steps found on opening (they are written at the next quiet point).
    pub fn restore(&mut self) {
        if self.pending.take().is_some() {
            self.unsaved_since = Some(Instant::now());
            self.last_change = Some(Instant::now());
        }
    }

    /// Go back to the files as they are, as one undoable step (the unsaved steps stay in the history).
    pub fn discard(&mut self) -> Result<Option<u64>> {
        self.need_write()?;
        self.pending = None;
        let files = self.files.clone();
        let seq = self.step(STUDIO, "Discard unsaved changes", &files, None)?;
        self.unsaved_since = None;
        self.last_change = None;
        Ok(seq)
    }

    /// Make the state `to` one step; None when it is the current state.
    fn step(&mut self, author: &str, label: &str, to: &State, request: Option<String>) -> Result<Option<u64>> {
        self.need_write()?;
        if *to == self.r.current && request.is_none() {
            return Ok(None);
        }
        let seq = self.r.next_seq;
        let rec = Rec::Step {
            seq,
            at: now_ms(),
            author: author.to_string(),
            label: label.to_string(),
            docs: diff(&self.r.current, to),
            request,
        };
        // On the disk first: only a step the journal has is made.
        self.append(&rec)?;
        self.r.records -= 1;
        self.r.apply(rec);
        self.touch();
        Ok(Some(seq))
    }

    fn touch(&mut self) {
        let now = Instant::now();
        self.last_change = Some(now);
        if self.unsaved() {
            self.unsaved_since.get_or_insert(now);
        } else {
            self.unsaved_since = None;
        }
    }

    /// Change documents as one step: `docs` as they are to be (None removes one).
    pub fn commit(&mut self, author: &str, label: &str, docs: BTreeMap<String, Option<String>>) -> Result<Outcome> {
        let to = with(&self.r.current, &docs);
        Ok(self.step(author, label, &to, None)?.map_or(Outcome::Unchanged, Outcome::Made))
    }

    /// Change documents as one step, made against step `base` (None: the current one): items changed since
    /// `base` and not by this change stay as they are now; an item both changed is a conflict, and nothing is
    /// made. `request` names the inbox request it answers.
    pub fn transact(
        &mut self,
        author: &str,
        label: &str,
        base: Option<u64>,
        docs: &BTreeMap<String, Option<String>>,
        request: Option<String>,
    ) -> Result<Outcome> {
        self.need_write()?;
        let base_seq = base.unwrap_or(self.seq());
        let Some(base_state) = self.r.state_at(base_seq).cloned() else {
            return Ok(Outcome::Refused(format!("step {base_seq} is no longer kept; read the mod again")));
        };
        let proposed = with(&base_state, docs);
        let to = if base_seq == self.seq() {
            proposed
        } else {
            let (merged, conflicts) = doc::merge(&base_state, &self.r.current, &proposed, Prefer::Ours);
            if !conflicts.is_empty() {
                return Ok(Outcome::Conflict(conflicts));
            }
            merged
        };
        if to == self.r.current && request.is_none() {
            return Ok(Outcome::Unchanged);
        }
        Ok(self.step(author, label, &to, request)?.map_or(Outcome::Unchanged, Outcome::Made))
    }

    /// Take back the last step, whoever made it; its name.
    pub fn undo(&mut self, author: &str) -> Result<Option<String>> {
        self.need_write()?;
        let Some(label) = self.r.past.last().map(|s| s.label.clone()) else { return Ok(None) };
        let rec = Rec::Undo { at: now_ms(), author: author.to_string() };
        self.append(&rec)?;
        self.r.records -= 1;
        self.r.apply(rec);
        self.touch();
        Ok(Some(label))
    }

    /// Make the last undone step again; its name.
    pub fn redo(&mut self, author: &str) -> Result<Option<String>> {
        self.need_write()?;
        let Some(label) = self.r.future.last().map(|s| s.label.clone()) else { return Ok(None) };
        let rec = Rec::Redo { at: now_ms(), author: author.to_string() };
        self.append(&rec)?;
        self.r.records -= 1;
        self.r.apply(rec);
        self.touch();
        Ok(Some(label))
    }

    /// Take back kept step `seq` (any author, not only the last), as a new step: the items it changed go back
    /// to what they were before it, unless a later step changed them too (a conflict; nothing is made).
    pub fn revert(&mut self, seq: u64, author: &str) -> Result<Outcome> {
        self.need_write()?;
        let Some(s) = self.r.past.iter().find(|s| s.seq == seq).cloned() else {
            return Ok(Outcome::Refused(format!("step {seq} is not in the history")));
        };
        let (merged, conflicts) = doc::merge(&s.after, &self.r.current, &s.before, Prefer::Ours);
        if !conflicts.is_empty() {
            return Ok(Outcome::Conflict(conflicts));
        }
        let label = format!("Revert: {}", s.label);
        Ok(self.step(author, &label, &merged, None)?.map_or(Outcome::Unchanged, Outcome::Made))
    }

    /// Keep the current state under `name` (a later one of the same name replaces it).
    pub fn mark_version(&mut self, name: &str) -> Result<()> {
        self.need_write()?;
        let rec = Rec::Version { at: now_ms(), name: name.to_string(), docs: self.r.current.clone() };
        self.append(&rec)?;
        self.r.records -= 1;
        self.r.apply(rec);
        Ok(())
    }

    /// Go back to version `name`, as one undoable step.
    pub fn restore_version(&mut self, name: &str, author: &str) -> Result<Outcome> {
        let Some(v) = self.r.versions.iter().find(|v| v.name == name).cloned() else {
            return Ok(Outcome::Refused(format!("no version `{name}`")));
        };
        let label = format!("Back to version {name}");
        Ok(self.step(author, &label, &v.docs, None)?.map_or(Outcome::Unchanged, Outcome::Made))
    }

    /// Write the mod's files now (Ctrl+S): each changed document through a temporary file and a rename, then
    /// the journal notes it. Keeps the unsaved steps found on opening ([`restore`](Self::restore)).
    pub fn save(&mut self) -> Result<()> {
        self.need_write()?;
        self.pending = None;
        let to = self.r.current.clone();
        for d in &self.docs.clone() {
            let path = self.dir.join(d);
            match to.get(d) {
                Some(t) if self.files.get(d) != Some(t) => self.disk.write_atomic(&path, t.as_bytes())?,
                None if self.files.contains_key(d) => self.disk.remove(&path)?,
                _ => {}
            }
            self.files_set(d, to.get(d).cloned());
            self.stamps.insert(d.clone(), stamp(&path));
        }
        self.mark_saved(to)?;
        self.unsaved_since = None;
        if self.r.records >= COMPACT_AT {
            self.compact()?;
        }
        Ok(())
    }

    fn files_set(&mut self, d: &str, t: Option<String>) {
        match t {
            Some(t) => self.files.insert(d.to_string(), t),
            None => self.files.remove(d),
        };
    }

    fn mark_saved(&mut self, s: State) -> Result<()> {
        if hashes(&s) == self.r.saved_hashes() && self.r.saved_state.as_ref() == Some(&s) {
            return Ok(());
        }
        let rec = Rec::Saved { at: now_ms(), docs: hashes(&s) };
        self.append(&rec)?;
        self.r.records -= 1;
        self.r.apply(rec);
        self.r.saved_state = Some(s);
        Ok(())
    }

    /// Write the journal afresh with only what is kept (one whole-file write: the old journal or the new one).
    pub fn compact(&mut self) -> Result<()> {
        self.need_write()?;
        let r = &self.r;
        let mut recs = vec![Rec::Base {
            seq: r.base_seq,
            at: now_ms(),
            docs: r.base.clone(),
            done: r.done.iter().cloned().collect(),
        }];
        let mut cur = r.base.clone();
        for s in r.past.iter().chain(r.future.iter().rev()) {
            recs.push(Rec::Step {
                seq: s.seq,
                at: s.at,
                author: s.author.clone(),
                label: s.label.clone(),
                docs: diff(&cur, &s.after),
                request: s.request.clone(),
            });
            cur = s.after.clone();
        }
        recs.extend(r.future.iter().map(|_| Rec::Undo { at: now_ms(), author: STUDIO.into() }));
        recs.extend(r.versions.iter().map(|v| Rec::Version { at: v.at, name: v.name.clone(), docs: v.docs.clone() }));
        recs.push(Rec::Saved { at: now_ms(), docs: hashes(&self.files) });
        let bytes: Vec<u8> = recs.iter().flat_map(journal::encode).collect();
        self.disk.write_atomic(&self.edir.join(JOURNAL), &bytes)?;
        let mut fresh = Replay::default();
        for rec in recs {
            fresh.apply(rec);
        }
        fresh.saved_state = (hashes(&fresh.current) == hashes(&self.files)).then(|| self.files.clone());
        self.r = fresh;
        self.journal_len = bytes.len() as u64;
        Ok(())
    }

    /// Do what is due at `now`: take in another program's writes to the files and the inbox's requests, and
    /// write the files at a quiet point. A read-only view reads the journal again when it grew.
    pub fn tick(&mut self, now: Instant) -> Result<Ticked> {
        let mut t = Ticked::default();
        if !self.writable() {
            let len = std::fs::metadata(self.edir.join(JOURNAL)).map_or(0, |m| m.len());
            if len != self.journal_len {
                self.load()?;
                t.reloaded = true;
            }
            return Ok(t);
        }
        // Another program wrote the files.
        let moved: Vec<String> =
            self.docs.iter().filter(|d| self.stamps.get(*d) != Some(&stamp(&self.dir.join(d)))).cloned().collect();
        if !moved.is_empty() {
            let on_disk = self.read_files()?;
            for d in &moved {
                self.stamps.insert(d.clone(), stamp(&self.dir.join(d)));
            }
            if on_disk != self.files {
                self.files = on_disk;
                if let Some(seq) = self.take_outside()? {
                    t.outside.push(seq);
                }
            }
        }
        t.answered = self.answer_inbox()?;
        if self.pending.is_none()
            && self.unsaved()
            && let Some(last) = self.last_change
        {
            let since = self.unsaved_since.unwrap_or(last);
            if now.saturating_duration_since(last) >= QUIET || now.saturating_duration_since(since) >= LONGEST_UNSAVED {
                self.save()?;
                t.saved = true;
            }
        }
        Ok(t)
    }

    fn answer_inbox(&mut self) -> Result<Vec<(String, inbox::Reply)>> {
        let mut out = Vec::new();
        for req in inbox::pending(&self.edir) {
            let (id, reply) = match req {
                Err(id) => (id, inbox::Reply::Refused { reason: "the request does not read".into() }),
                Ok(r) if self.r.done.contains(&r.id) => {
                    // Made before a crash; answered again, not made again.
                    let seq = self.r.past.iter().chain(&self.r.future).find(|s| s.request.as_ref() == Some(&r.id));
                    (r.id.clone(), inbox::Reply::Ok { seq: seq.map_or(self.seq(), |s| s.seq) })
                }
                Ok(r) => {
                    let label = format!("{}: {}", r.author, r.label);
                    let reply = match self.transact(&r.author, &label, r.base, &r.docs, Some(r.id.clone()))? {
                        Outcome::Made(seq) => inbox::Reply::Ok { seq },
                        Outcome::Unchanged => inbox::Reply::Ok { seq: self.seq() },
                        Outcome::Conflict(items) => inbox::Reply::Conflict { items },
                        Outcome::Refused(reason) => inbox::Reply::Refused { reason },
                    };
                    (r.id.clone(), reply)
                }
            };
            let bytes = serde_json::to_vec_pretty(&reply).map_err(io::Error::other)?;
            self.disk.write_atomic(&inbox::reply_path(&self.edir, &id), &bytes)?;
            self.disk.remove(&inbox::request_path(&self.edir, &id))?;
            out.push((id, reply));
        }
        Ok(out)
    }

    /// The editor folder (lock, journal, inbox).
    pub fn editor_dir(&self) -> &Path {
        &self.edir
    }
}

#[cfg(test)]
mod tests;
