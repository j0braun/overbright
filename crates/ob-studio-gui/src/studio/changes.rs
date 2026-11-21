//! The Changes page (docs/modding/studio.md, "Changes"; `ob://<mod>/@changes`, Ctrl+D): every value the open mod
//! gives, grouped by item, each with where the game's value comes from (the original's ring, yours' dot, another
//! mod's diamond) and the original beside it; a history step or a kept version compared with now; the clashes as
//! the stack in the game's order. Rows come from `ob_studio::changes` (the layering tables and the mod's history);
//! Reset writes one undoable step through the mod's history.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use ob_studio::changes::map::{self, Game};
use ob_studio::changes::{self as model, Clash, From, Row, Show};

use crate::editor::theme::{self, LABEL, SEMI, TITLE, font, rgb, tokens};
use crate::tools::map::history::{self, History, Outcome};
use crate::tools::map::session::MapSession;
use crate::tools::map::{MapMod, store};

use super::draw::{self, Source};

/// What the page compares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Your game without this mod → with it.
    ThisMod,
    /// One history step: before it → after it (the step's number).
    Step(u64),
    /// A kept version → now (the version's name).
    Version(String),
    /// The values another mod also gives, as the stack.
    Clashes,
}

impl Mode {
    pub fn name(&self) -> &'static str {
        match self {
            Mode::ThisMod => "This mod",
            Mode::Step(_) => "History step",
            Mode::Version(_) => "Compare versions",
            Mode::Clashes => "Clashes",
        }
    }

    /// The address query for this mode (`?step=14`, `?version=beta`, `?mode=clashes`).
    pub fn query(&self) -> Vec<(String, String)> {
        match self {
            Mode::ThisMod => vec![],
            Mode::Step(n) => vec![("step".into(), n.to_string())],
            Mode::Version(v) => vec![("version".into(), v.clone())],
            Mode::Clashes => vec![("mode".into(), "clashes".into())],
        }
    }

    /// The mode an address query names (None: not one of the page's).
    pub fn from_query(q: &[(String, String)]) -> Option<Mode> {
        match q {
            [] => Some(Mode::ThisMod),
            [(k, v)] if k == "step" => v.parse().ok().map(Mode::Step),
            [(k, v)] if k == "version" => Some(Mode::Version(v.clone())),
            [(k, v)] if k == "mode" && v == "clashes" => Some(Mode::Clashes),
            [(k, v)] if k == "mode" && v == "mod" => Some(Mode::ThisMod),
            _ => None,
        }
    }
}

/// One kept step as the page lists it.
#[derive(Clone, Debug)]
pub struct StepInfo {
    pub seq: u64,
    pub label: String,
    pub at: u64,
    /// The items it changed, by kind and key.
    pub items: BTreeSet<(String, String)>,
    pub rows: Vec<Row>,
}

/// What the page shows of one mod, read through its history and the enabled mods.
#[derive(Clone, Debug)]
pub struct Data {
    pub folder: String,
    pub name: String,
    pub in_game: bool,
    /// The mods in the player's game (with this one), weakest first: (folder, name).
    pub stack: Vec<(String, String)>,
    pub rows: Vec<Row>,
    pub clashes: Vec<Clash>,
    pub steps: Vec<StepInfo>,
    pub versions: Vec<(String, Vec<Row>)>,
}

fn read_doc(maps: &Path, folder: &str) -> Result<(MapMod, History)> {
    let dir = maps.join(folder);
    let edir = store::editor_dir(maps, folder);
    let h = History::open_read(&dir, &edir, ob_studio::MOD_DOCS, Arc::new(history::disk::Real))
        .with_context(|| format!("reading the history of {folder}"))?;
    let m = MapMod::parse(h.doc(store::FILE).unwrap_or_default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((m, h))
}

fn mod_name(m: &MapMod, folder: &str) -> String {
    if m.name.trim().is_empty() { folder.to_string() } else { m.name.trim().to_string() }
}

/// Read the page's data for the mod in `maps/<folder>`: the enabled mods in their order, this one where the
/// game has it (on top, as turning it on puts it, while it is off).
pub fn load(maps: &Path, folder: &str) -> Result<Data> {
    let (m, h) = read_doc(maps, folder)?;
    let enabled = store::enabled(maps);
    let in_game = enabled.iter().any(|f| f == folder);
    let mut mods: Vec<(String, MapMod)> = Vec::new();
    let mut names = Vec::new();
    for f in &enabled {
        if f == folder {
            mods.push((f.clone(), m.clone()));
            names.push((f.clone(), mod_name(&m, f)));
        } else if let Ok((o, _)) = read_doc(maps, f) {
            names.push((f.clone(), mod_name(&o, f)));
            mods.push((f.clone(), o));
        }
    }
    if !in_game {
        mods.push((folder.into(), m.clone()));
        names.push((folder.into(), mod_name(&m, folder)));
    }
    let game = Game::new(&mods);
    let order = |mut rows: Vec<Row>| {
        for r in &mut rows {
            r.values.sort_by_key(|v| field_rank(&v.field));
        }
        rows
    };
    let parse = |s: &history::doc::State| s.get(store::FILE).and_then(|t| MapMod::parse(t).ok()).unwrap_or_default();
    let steps = h
        .steps()
        .iter()
        .map(|s| {
            let rows = order(map::compare_maps(&parse(&s.before), &parse(&s.after), folder));
            StepInfo {
                seq: s.seq,
                label: s.label.clone(),
                at: s.at,
                items: rows.iter().map(|r| (r.kind.to_string(), r.key.clone())).collect(),
                rows,
            }
        })
        .collect();
    let versions =
        h.versions().iter().map(|v| (v.name.clone(), order(map::compare_maps(&parse(&v.docs), &m, folder)))).collect();
    Ok(Data {
        folder: folder.into(),
        name: mod_name(&m, folder),
        in_game,
        stack: names,
        rows: order(game.this_mod(folder)),
        clashes: game.clashes(folder),
        steps,
        versions,
    })
}

/// Give values back as one step in the mod's history (its writer's lock taken for the step and let go); the
/// step's name.
pub fn reset(maps: &Path, folder: &str, picks: &[(String, String, String)], label: &str) -> Result<String> {
    let mut h = open_write(maps, folder)?;
    let m = MapMod::parse(h.doc(store::FILE).unwrap_or_default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    let Some(new) = map::reset_all(&m, folder, picks) else { bail!("nothing to reset: the mod no longer gives it") };
    let docs = BTreeMap::from([(store::FILE.to_string(), Some(new.write()))]);
    match h.commit(history::STUDIO, label, docs)? {
        Outcome::Made(_) => {}
        other => bail!("not reset: {other:?}"),
    }
    h.save()?;
    Ok(label.to_string())
}

/// The editor session of this window that writes the mod `folder` (World or Races has it open): Changes makes
/// its steps there, since that session holds the mod's one write lock (a second writer in this process would be
/// refused as "has this mod open").
fn writer<'a>(session: Option<&'a mut MapSession>, folder: &str) -> Option<&'a mut MapSession> {
    session.filter(|s| s.folder == folder && s.history.as_ref().is_some_and(History::writable))
}

/// [`reset`] made in the session that writes the mod.
fn reset_in(s: &mut MapSession, folder: &str, picks: &[(String, String, String)], label: &str) -> Result<String> {
    let Some(new) = map::reset_all(&s.map, folder, picks) else {
        bail!("nothing to reset: the mod no longer gives it")
    };
    s.map = new;
    if !s.commit(label) {
        bail!("not reset: {}", s.note.take().unwrap_or_else(|| "nothing changed".into()));
    }
    Ok(label.to_string())
}

/// Undo (or redo) the mod's last step; its name.
pub fn undo(maps: &Path, folder: &str, redo: bool) -> Result<Option<String>> {
    let mut h = open_write(maps, folder)?;
    let looks = h.doc(ob_studio::looks::FILE).map(str::to_string);
    let r = if redo { h.redo(history::STUDIO)? } else { h.undo(history::STUDIO)? };
    h.save()?;
    // A paint taken back or made again: the pack data follows the texture list.
    let (dir, edir) = (maps.join(folder), store::editor_dir(maps, folder));
    ob_studio::looks::work::follow(&dir, &edir, looks.as_deref(), h.doc(ob_studio::looks::FILE))
        .map_err(anyhow::Error::msg)?;
    Ok(r)
}

fn open_write(maps: &Path, folder: &str) -> Result<History> {
    let dir = maps.join(folder);
    let edir = store::editor_dir(maps, folder);
    let h = History::open(&dir, &edir, ob_studio::MOD_DOCS, history::STUDIO, Arc::new(history::disk::Real))?;
    if let Some(owner) = h.read_only_by() {
        let who = owner.map_or("another program".to_string(), |o| o.program.clone());
        bail!("{who} has this mod open; changes wait until it closes the mod");
    }
    Ok(h)
}

// ---- words ---------------------------------------------------------------------------------------------------

/// Where a field sits among an item's rows: where it is, how it turns, what it is, then the rest.
pub fn field_rank(field: &str) -> usize {
    const ORDER: [&str; 11] = [
        "name",
        "type",
        "position",
        "heading",
        "solid",
        "disabled",
        "track",
        "laps",
        "backward",
        "checkpoints",
        "grid",
    ];
    ORDER.iter().position(|f| *f == field).unwrap_or(ORDER.len())
}

/// A field's name on a row.
pub fn field_name(field: &str) -> String {
    match field {
        "position" => "Position".into(),
        "heading" => "Heading".into(),
        "disabled" => "Hidden".into(),
        "solid" => "Prop".into(),
        "name" => "Name".into(),
        "type" => "Race type".into(),
        "laps" => "Laps".into(),
        "track" => "Base track".into(),
        "backward" => "Backward".into(),
        "checkpoints" => "Checkpoints".into(),
        "grid" => "Start grid".into(),
        "race" => "Race".into(),
        other => {
            let mut c = other.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
        }
    }
}

/// A value as a row shows it: numbers to one decimal, a heading in degrees, a list of points by its length.
pub fn value_text(field: &str, v: &toml::Value) -> String {
    use toml::Value as V;
    let num = |x: &V| match x {
        V::Float(f) => format!("{f:.1}"),
        V::Integer(i) => i.to_string(),
        other => format!("{other:?}"),
    };
    match v {
        V::Float(f) if field == "heading" => format!("{f:.1}°"),
        V::Boolean(b) if field == "disabled" => (if *b { "hidden" } else { "shown" }).into(),
        V::Boolean(b) => (if *b { "yes" } else { "no" }).into(),
        V::String(s) => s.clone(),
        V::Array(a) if a.iter().all(|x| x.is_float() || x.is_integer()) => {
            a.iter().map(num).collect::<Vec<_>>().join(", ")
        }
        V::Array(a) => home_plural(a.len(), "point", "points"),
        other => num(other),
    }
}

fn home_plural(n: usize, one: &str, many: &str) -> String {
    super::home::plural(n, one, many)
}

/// An item's title and its line under it ("XO_DOME", "Prop · L4RA · moved").
pub fn item_words(r: &Row, mine: &str) -> (String, String) {
    let field = |f: &str| r.values.iter().find(|v| v.field == f);
    if r.kind == map::RACE {
        let name = field("name")
            .and_then(|v| v.game.clone().or(v.yours.clone()))
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| r.key.clone());
        return (name, "Race · added by you".into());
    }
    let parts: Vec<&str> = r.key.split('/').collect();
    let region = parts.first().copied().unwrap_or_default();
    let solid = if r.added {
        field("solid")
            .and_then(|v| v.yours.clone().or(v.game.clone()))
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| r.key.clone())
    } else {
        parts.get(2).map_or_else(|| r.key.clone(), |s| s.to_string())
    };
    let mut what = vec![];
    if r.added {
        what.push("added");
    } else if r.values.iter().any(|v| v.field == "disabled" && v.yours.is_some()) {
        what.push("hidden");
    } else if r.values.iter().any(|v| v.yours.is_some()) {
        what.push("moved");
    }
    if r.loses() {
        what.push("loses");
    }
    let _ = mine;
    (solid, format!("Prop · {region} · {}", what.join(", ")))
}

fn source(f: &From) -> Source {
    match f {
        From::Original => Source::Original,
        From::Yours => Source::Yours,
        From::Other(_) => Source::AnotherMod,
    }
}

// ---- the page --------------------------------------------------------------------------------------------------

/// A row's action, as its button says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Reset,
    Remove,
    ShowInClashes,
}

/// What the page asks the shell to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Go to an item's address (Show in World / Races).
    Go(String),
    /// The page's own mode changed: the window's address changes.
    Moved,
}

/// The page's state in one window.
#[derive(Debug)]
pub struct Page {
    pub mode: Mode,
    pub show: Show,
    pub data: Option<std::result::Result<Data, String>>,
    maps: PathBuf,
    /// The focused value row (an index into [`Page::lines`]).
    pub focus: usize,
    /// The value rows picked for one Reset (Shift+↓, pad A).
    pub picked: BTreeSet<usize>,
    /// The last Reset's or undo's outcome.
    pub note: Option<String>,
    scroll: f32,
    /// The Inspector drawer is open (narrow windows).
    pub inspector_open: bool,
    /// What the last frame drew where.
    pub last: Drawn,
}

impl Page {
    pub fn new(maps: PathBuf) -> Page {
        Page {
            mode: Mode::ThisMod,
            show: Show::Mine,
            data: None,
            maps,
            focus: 0,
            picked: BTreeSet::new(),
            note: None,
            scroll: 0.0,
            inspector_open: false,
            last: Drawn::default(),
        }
    }

    /// Read (again) the mod `folder`'s data.
    pub fn load(&mut self, folder: &str) {
        self.data = Some(load(&self.maps, folder).map_err(|e| format!("{e:#}")));
        let n = self.lines().len();
        self.focus = self.focus.min(n.saturating_sub(1));
        self.picked.retain(|i| *i < n);
    }

    /// Make sure the data is the open mod's.
    pub fn ensure(&mut self, folder: Option<&str>) {
        match folder {
            None => self.data = None,
            Some(f) => {
                if !matches!(&self.data, Some(Ok(d)) if d.folder == f) {
                    self.focus = 0;
                    self.picked.clear();
                    self.load(f);
                }
            }
        }
    }

    pub fn data(&self) -> Option<&Data> {
        self.data.as_ref().and_then(|d| d.as_ref().ok())
    }

    /// The modes with content, in the tabs' order.
    pub fn modes(&self) -> Vec<Mode> {
        let mut v = vec![Mode::ThisMod];
        if let Some(d) = self.data() {
            if let Some(s) = d.steps.last() {
                v.push(match &self.mode {
                    Mode::Step(n) => Mode::Step(*n),
                    _ => Mode::Step(s.seq),
                });
            }
            if let Some((name, _)) = d.versions.last() {
                v.push(match &self.mode {
                    Mode::Version(n) => Mode::Version(n.clone()),
                    _ => Mode::Version(name.clone()),
                });
            }
            if !d.clashes.is_empty() {
                v.push(Mode::Clashes);
            }
        }
        v
    }

    /// The groups the current mode lists.
    pub fn rows(&self) -> Vec<Row> {
        let Some(d) = self.data() else { return vec![] };
        match &self.mode {
            Mode::ThisMod if self.show == Show::All => model::filter(&d.rows, self.show),
            Mode::ThisMod => model::filter(&d.rows, self.show).into_iter().map(race_as_one).collect(),
            Mode::Clashes => {
                let keys: BTreeSet<(&str, &str)> = d.clashes.iter().map(|c| (c.kind, c.key.as_str())).collect();
                let rows: Vec<Row> =
                    d.rows.iter().filter(|r| keys.contains(&(r.kind, r.key.as_str()))).cloned().collect();
                model::filter(&rows, Show::Mine)
            }
            Mode::Step(n) => d.steps.iter().find(|s| s.seq == *n).map(|s| s.rows.clone()).unwrap_or_default(),
            Mode::Version(v) => d.versions.iter().find(|(n, _)| n == v).map(|(_, r)| r.clone()).unwrap_or_default(),
        }
    }

    /// The value rows in order: (group, value).
    pub fn lines(&self) -> Vec<(usize, usize)> {
        self.rows().iter().enumerate().flat_map(|(g, r)| (0..r.values.len()).map(move |v| (g, v))).collect()
    }

    /// The actions a value row offers.
    pub fn actions(&self, r: &Row, v: &model::Value) -> Vec<Action> {
        let mut a = vec![];
        if self.mode != Mode::ThisMod && self.mode != Mode::Clashes {
            return a;
        }
        if r.added {
            if v.yours.is_some() {
                a.push(Action::Remove);
            }
        } else if v.yours.is_some() && !v.loses() {
            a.push(Action::Reset);
        }
        if v.loses() && self.mode != Mode::Clashes {
            a.push(Action::ShowInClashes);
        }
        a
    }

    pub fn set_mode(&mut self, m: Mode) {
        if self.mode != m {
            self.mode = m;
            self.focus = 0;
            self.picked.clear();
            self.scroll = 0.0;
        }
    }

    /// ↑ / ↓ (pad D-pad); `extend` (Shift) picks the rows passed over.
    pub fn step_focus(&mut self, by: i32, extend: bool) {
        let n = self.lines().len();
        if n == 0 {
            return;
        }
        if extend {
            self.picked.insert(self.focus);
        }
        self.focus = (self.focus as i32 + by).clamp(0, n as i32 - 1) as usize;
        if extend {
            self.picked.insert(self.focus);
        }
    }

    /// Enter / pad A: pick the focused row or take it back out.
    pub fn toggle_pick(&mut self) {
        if !self.lines().is_empty() && !self.picked.remove(&self.focus) {
            self.picked.insert(self.focus);
        }
    }

    /// The rows Reset takes: the picked ones, else the focused one; (kind, key, field) of those with a Reset or
    /// Remove.
    pub fn reset_picks(&self) -> Vec<(String, String, String)> {
        let rows = self.rows();
        let lines = self.lines();
        let which: Vec<usize> =
            if self.picked.is_empty() { vec![self.focus] } else { self.picked.iter().copied().collect() };
        which
            .into_iter()
            .filter_map(|i| lines.get(i))
            .filter_map(|(g, v)| {
                let (r, val) = (&rows[*g], &rows[*g].values[*v]);
                let a = self.actions(r, val);
                (a.contains(&Action::Reset) || a.contains(&Action::Remove))
                    .then(|| (r.kind.to_string(), r.key.clone(), val.field.clone()))
            })
            .collect()
    }

    /// Reset the picked (or focused) rows as one step.
    pub fn reset(&mut self) {
        self.reset_with(None);
    }

    /// [`reset`](Self::reset), made in `session` when it is the one that writes the mod (the editor hosted in
    /// this window).
    pub fn reset_with(&mut self, session: Option<&mut MapSession>) {
        let picks = self.reset_picks();
        let Some(d) = self.data() else { return };
        let folder = d.folder.clone();
        if picks.is_empty() {
            self.note = Some("Nothing to reset here: pick a value of yours".into());
            return;
        }
        let rows = self.rows();
        let label = if picks.len() == 1 {
            let r = rows.iter().find(|r| r.key == picks[0].1).cloned();
            let item = r.map_or(picks[0].1.clone(), |r| item_words(self.data().map_or(&r, |d| full(d, &r)), &folder).0);
            format!("Reset {} of {item}", field_name(&picks[0].2))
        } else {
            format!("Reset {} values", picks.len())
        };
        let done = match writer(session, &folder) {
            Some(s) => reset_in(s, &folder, &picks, &label),
            None => reset(&self.maps, &folder, &picks, &label),
        };
        self.note = Some(match done {
            Ok(l) => format!("{l} · Ctrl+Z takes it back"),
            Err(e) => format!("{e:#}"),
        });
        self.picked.clear();
        self.load(&folder);
    }

    /// Ctrl+Z / Ctrl+Y on the page.
    pub fn undo(&mut self, redo: bool) {
        self.undo_with(redo, None);
    }

    /// [`undo`](Self::undo), made in `session` when it is the one that writes the mod.
    pub fn undo_with(&mut self, redo: bool, session: Option<&mut MapSession>) {
        let Some(folder) = self.data().map(|d| d.folder.clone()) else { return };
        let done = match writer(session, &folder) {
            Some(s) => {
                let r = if redo { s.redo() } else { s.undo() };
                match s.note.take() {
                    Some(n) if r.is_none() => Err(anyhow::anyhow!(n)),
                    _ => Ok(r),
                }
            }
            None => undo(&self.maps, &folder, redo),
        };
        self.note = Some(match done {
            Ok(Some(l)) => format!("{}: {l}", if redo { "Redone" } else { "Undone" }),
            Ok(None) => "Nothing to take back".into(),
            Err(e) => format!("{e:#}"),
        });
        self.load(&folder);
    }

    /// The focused item's address (Show in World / Races).
    pub fn focused_address(&self) -> Option<String> {
        let rows = self.rows();
        let (g, v) = *self.lines().get(self.focus)?;
        Some(item_address(&self.data()?.folder, &rows[g], Some(&rows[g].values[v].field)))
    }
}

/// The item with every value (a row shown as one keeps its name there).
fn full<'a>(d: &'a Data, r: &'a Row) -> &'a Row {
    d.rows.iter().find(|x| x.kind == r.kind && x.key == r.key).unwrap_or(r)
}

/// A race of the mod's own as one row ("Sprint · 3 checkpoints · base track 4001"): it is all yours, so its
/// values one by one say nothing more until "All values".
fn race_as_one(r: Row) -> Row {
    if r.kind != map::RACE || !r.added || r.loses() {
        return r;
    }
    let get = |f: &str| r.values.iter().find(|v| v.field == f).and_then(|v| v.yours.clone().or(v.game.clone()));
    let mut parts = vec![];
    if let Some(toml::Value::String(k)) = get("type") {
        let mut c = k.chars();
        parts.push(c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()));
    }
    if let Some(toml::Value::Integer(n)) = get("laps")
        && n > 1
    {
        parts.push(format!("{n} laps"));
    }
    if let Some(toml::Value::Array(a)) = get("checkpoints") {
        parts.push(home_plural(a.len(), "checkpoint", "checkpoints"));
    }
    if let Some(toml::Value::Integer(n)) = get("track") {
        parts.push(format!("base track {n}"));
    }
    let summary = toml::Value::String(parts.join(" · "));
    let v = model::Value {
        field: "race".into(),
        from: From::Yours,
        game: Some(summary.clone()),
        yours: Some(summary),
        original: None,
    };
    Row { values: vec![v], ..r }
}

/// An item's address (`ob://<mod>/placement/<key>#<field>`); a field that is not a plain name is left out.
pub fn item_address(folder: &str, r: &Row, field: Option<&str>) -> String {
    let mut a = format!("ob://{folder}/{}/{}", r.kind, r.key);
    if let Some(f) = field.filter(|f| f.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')) {
        a.push('#');
        a.push_str(f);
    }
    a
}

// ---- drawing ---------------------------------------------------------------------------------------------------

const GROUP_H: f32 = 32.0;
const ROW_H: f32 = 30.0;
const NOTE_H: f32 = 22.0;
/// The Inspector beside the list from this page width; a strip with a drawer below it.
const INSPECTOR_FROM_W: f32 = 1100.0;
const INSPECTOR_W: f32 = 300.0;

fn small_button(text: &str, on: bool) -> egui::Button<'static> {
    let t = tokens();
    let b = egui::Button::new(RichText::new(text.to_string()).font(font(LABEL, 14.0)).color(if on {
        rgb(t.lime)
    } else {
        rgb(t.ink)
    }))
    .stroke(Stroke::new(1.0, rgb(t.outline)))
    .corner_radius(CornerRadius::same(4))
    .wrap_mode(egui::TextWrapMode::Extend)
    .min_size(vec2(0.0, 22.0));
    if on { b.fill(theme::over(t.pressed, t.slab.over(t.page))) } else { b.fill(Color32::TRANSPARENT) }
}

fn who_text(f: &From, stack: &[(String, String)]) -> String {
    match f {
        From::Original => "original".into(),
        From::Yours => "yours".into(),
        From::Other(p) => stack.iter().find(|(id, _)| id == p).map_or(p.clone(), |(_, n)| n.clone()),
    }
}

fn who_color(f: &From) -> Color32 {
    let t = tokens();
    match f {
        From::Original => rgb(t.dim),
        From::Yours => rgb(t.lime),
        From::Other(_) => rgb(t.streak),
    }
}

/// What the page drew where (for the tests' clip check): the list's rows and the Inspector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub rows: Vec<Rect>,
    pub inspector: Option<Rect>,
    pub focus: Option<Rect>,
}

impl Page {
    /// Draw the page into the slab's `inner` rect; returns what the shell must do and what was drawn where.
    pub fn draw(&mut self, ui: &mut egui::Ui, inner: Rect, focused: bool, time: f32) -> (Option<Ask>, Drawn) {
        let (ask, drawn) = self.draw_page(ui, inner, focused, time);
        self.last = drawn.clone();
        (ask, drawn)
    }

    fn draw_page(&mut self, ui: &mut egui::Ui, inner: Rect, focused: bool, time: f32) -> (Option<Ask>, Drawn) {
        let _ = time;
        let t = tokens();
        let p = ui.painter().clone();
        let mut ask = None;
        let mut drawn = Drawn::default();
        let wide = inner.width() >= INSPECTOR_FROM_W;
        let (list, insp) = if wide {
            let l = Rect::from_min_max(inner.min, pos2(inner.right() - INSPECTOR_W - 20.0, inner.bottom()));
            (l, Some(Rect::from_min_max(pos2(l.right() + 20.0, inner.top()), inner.max)))
        } else {
            (Rect::from_min_max(inner.min, pos2(inner.right() - 34.0, inner.bottom())), None)
        };

        // The title line: back crumb, title, the modes with content.
        let y0 = list.top();
        p.text(pos2(list.left(), y0 + 14.0), Align2::LEFT_CENTER, "← World", draw::label_font(16.0), rgb(t.dim));
        let after_back = list.left() + 84.0;
        let kx = draw::glyph(&p, ob_look::legends::Glyph::Key("Esc"), after_back, y0 + 14.0);
        let title_x = after_back + kx + 16.0;
        let tg = p.layout_no_wrap("Changes".into(), font(TITLE, 24.0), rgb(t.lime));
        let tw = tg.size().x;
        p.galley(pos2(title_x, y0 + 14.0 - tg.size().y / 2.0), tg, rgb(t.lime));
        if ui
            .interact(
                Rect::from_min_size(pos2(list.left(), y0), vec2(80.0, 28.0)),
                egui::Id::unique("chg-back"),
                egui::Sense::click(),
            )
            .clicked()
        {
            ask = Some(Ask::Go("back".into()));
        }
        let mut x = title_x + tw + 18.0;
        let clash_n = self.data().map_or(0, |d| d.clashes.len());
        for m in self.modes() {
            let label = if m == Mode::Clashes { format!("Clashes {clash_n}") } else { m.name().to_string() };
            let g = p.layout_no_wrap(label.clone(), draw::label_font(16.0), rgb(t.ink));
            let r = Rect::from_min_size(pos2(x, y0), vec2(g.size().x + 20.0, 28.0));
            if r.right() > list.right() {
                break;
            }
            let cur = std::mem::discriminant(&m) == std::mem::discriminant(&self.mode);
            if cur {
                p.rect_filled(r, 4.0, if focused { rgb(t.lime) } else { theme::over(t.pressed, t.slab.over(t.page)) });
            }
            let col = if cur && focused {
                rgb(t.on_play)
            } else if cur {
                rgb(t.lime)
            } else {
                rgb(t.ink)
            };
            p.text(r.center(), Align2::CENTER_CENTER, label, draw::label_font(16.0), col);
            if ui.interact(r, egui::Id::unique(("chg-mode", m.name())), egui::Sense::click()).clicked() {
                self.set_mode(m);
                ask = Some(Ask::Moved);
            }
            x = r.right() + 6.0;
        }

        let Some(d) = self.data().cloned() else {
            let line = match &self.data {
                Some(Err(e)) => e.clone(),
                _ => "No mod open: open one on Home to see what it changes".into(),
            };
            p.text(pos2(list.left(), y0 + 60.0), Align2::LEFT_TOP, line, font("body", 15.0), rgb(t.dim));
            return (ask, drawn);
        };

        // The header line: what is compared and what the buttons do.
        let head = Rect::from_min_size(pos2(list.left(), y0 + 38.0), vec2(list.width(), 30.0));
        p.rect_filled(head, 6.0, theme::over(t.panel, t.slab.over(t.page)));
        draw::lamp(&p, pos2(head.left() + 16.0, head.center().y), 10.0, ob_look::lamp::Mark::Info, 0.0);
        let compared = match &self.mode {
            Mode::ThisMod | Mode::Clashes => format!("Comparing your game without {} → with it", d.name),
            Mode::Step(n) => {
                let s = d.steps.iter().find(|s| s.seq == *n);
                format!("Comparing before step {n} → after it · {}", s.map_or("", |s| s.label.as_str()))
            }
            Mode::Version(v) => format!("Comparing version {v} → now"),
        };
        let what = match &self.mode {
            Mode::ThisMod | Mode::Clashes => " · Reset puts one value back to the original; nothing else changes.",
            _ => " · Reset is on This mod.",
        };
        let mut clip = p.clone();
        clip.set_clip_rect(head.shrink(2.0));
        let g = clip.layout_no_wrap(compared, font(SEMI, 14.0), rgb(t.ink));
        let cw = g.size().x;
        clip.galley(pos2(head.left() + 30.0, head.center().y - g.size().y / 2.0), g, rgb(t.ink));
        clip.text(
            pos2(head.left() + 30.0 + cw, head.center().y),
            Align2::LEFT_CENTER,
            what,
            font("body", 14.0),
            rgb(t.dim),
        );

        // The filter line (This mod only).
        let mut top = head.bottom() + 8.0;
        let rows = self.rows();
        if self.mode == Mode::ThisMod {
            let fl = Rect::from_min_size(pos2(list.left(), top), vec2(list.width(), 26.0));
            p.text(pos2(fl.left(), fl.center().y), Align2::LEFT_CENTER, "Show", font("body", 14.0), rgb(t.dim));
            let mut bx = fl.left() + 44.0;
            for (s, word) in
                [(Show::Mine, "Everything I change"), (Show::Losing, "Only what loses"), (Show::All, "All values")]
            {
                let g = p.layout_no_wrap(word.into(), font(LABEL, 14.0), rgb(t.ink));
                let r = Rect::from_min_size(pos2(bx, fl.top() + 1.0), vec2(g.size().x + 28.0, 24.0));
                if ui.put(r, small_button(word, self.show == s)).clicked() {
                    self.show = s;
                    self.focus = 0;
                    self.picked.clear();
                }
                bx = r.right() + 6.0;
            }
            let n_values: usize = rows.iter().map(|r| r.values.len()).sum();
            let count = format!(
                "{} in {} · in your game: {} · {}",
                home_plural(n_values, "value", "values"),
                home_plural(rows.len(), "item", "items"),
                if d.in_game { "on" } else { "off" },
                home_plural(d.stack.len(), "mod", "mods"),
            );
            let g = p.layout_no_wrap(count, font("body", 13.0), rgb(t.dim));
            if fl.right() - g.size().x > bx + 8.0 {
                p.galley(pos2(fl.right() - g.size().x, fl.center().y - g.size().y / 2.0), g, rgb(t.dim));
            }
            top = fl.bottom() + 8.0;
        }
        if let Some(n) = &self.note {
            p.text(pos2(list.left(), top + 8.0), Align2::LEFT_CENTER, n, font("body", 13.0), rgb(t.lime));
            top += 20.0;
        }

        // The groups and their value rows, scrolled so the focused row is in view.
        let body = Rect::from_min_max(pos2(list.left() - 4.0, top), pos2(list.right() + 4.0, list.bottom()));
        let lines = self.lines();
        let mut heights = vec![];
        for (gi, r) in rows.iter().enumerate() {
            heights.push((gi, None, GROUP_H));
            for (vi, v) in r.values.iter().enumerate() {
                heights.push((gi, Some(vi), ROW_H + if v.loses() { NOTE_H } else { 0.0 }));
            }
            heights.push((gi, None, 8.0));
        }
        let clash_rows = if matches!(self.mode, Mode::ThisMod | Mode::Clashes) { d.clashes.len() } else { 0 };
        let clash_h = if clash_rows > 0 { 30.0 + 28.0 * (clash_rows as f32 + 1.0) + 8.0 } else { 0.0 };
        // Where the focused row lies in the content.
        let mut y = 0.0;
        let mut focus_span = (0.0, 0.0);
        let mut li = 0;
        for (_, v, h) in &heights {
            if v.is_some() {
                if li == self.focus {
                    focus_span = (y, y + h);
                }
                li += 1;
            }
            y += h;
        }
        let content_h = y + clash_h;
        if ui.rect_contains_pointer(body) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if focus_span.1 - self.scroll > body.height() {
            self.scroll = focus_span.1 - body.height();
        }
        if focus_span.0 < self.scroll {
            self.scroll = focus_span.0;
        }
        self.scroll = self.scroll.clamp(0.0, (content_h - body.height()).max(0.0));
        let mut clip = p.clone();
        clip.set_clip_rect(body);
        let mut y = body.top() - self.scroll;
        let mut li = 0;
        let value_x = list.left() + (list.width() * 0.3).clamp(220.0, 260.0);
        let src_x = list.left() + 130.0;
        for (gi, vi, h) in heights {
            let r = &rows[gi];
            let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), h));
            let visible = rr.bottom() > body.top() && rr.top() < body.bottom();
            match vi {
                None if h == GROUP_H => {
                    if visible {
                        let (title, sub) = item_words(full(&d, r), &d.folder);
                        draw::source_mark(&clip, pos2(rr.left() + 8.0, rr.center().y), 9.0, source(&r.from()));
                        let g = clip.layout_no_wrap(title, draw::label_font(17.0), rgb(t.ink));
                        let w = g.size().x;
                        clip.galley(pos2(rr.left() + 22.0, rr.center().y - g.size().y / 2.0), g, rgb(t.ink));
                        clip.text(
                            pos2(rr.left() + 32.0 + w, rr.center().y),
                            Align2::LEFT_CENTER,
                            sub,
                            font("body", 13.0),
                            rgb(t.dim),
                        );
                        let word = if r.kind == map::RACE { "Show in Races" } else { "Show in World" };
                        let br = Rect::from_min_size(pos2(rr.right() - 112.0, rr.top() + 4.0), vec2(108.0, 24.0));
                        if br.top() >= body.top()
                            && br.bottom() <= body.bottom()
                            && ui.put(br, small_button(word, false)).clicked()
                        {
                            ask = Some(Ask::Go(item_address(&d.folder, r, None)));
                        }
                        drawn.rows.push(rr);
                    }
                }
                None => {
                    clip.line_segment(
                        [rr.left_top() + vec2(0.0, 4.0), rr.right_top() + vec2(0.0, 4.0)],
                        Stroke::new(1.0, draw::rule()),
                    );
                }
                Some(vi) => {
                    let v = &r.values[vi];
                    let line = Rect::from_min_size(rr.min, vec2(rr.width(), ROW_H));
                    let is_focus = li == self.focus;
                    let picked = self.picked.contains(&li);
                    if visible {
                        if picked {
                            clip.rect_filled(
                                line.expand2(vec2(2.0, 0.0)),
                                6.0,
                                theme::over(t.pressed, t.slab.over(t.page)),
                            );
                        }
                        if is_focus && focused {
                            clip.rect_stroke(
                                line.expand2(vec2(2.0, 1.0)),
                                6.0,
                                Stroke::new(2.0, rgb(t.lime)),
                                StrokeKind::Inside,
                            );
                            drawn.focus = Some(line);
                        }
                        clip.text(
                            pos2(line.left() + 22.0, line.center().y),
                            Align2::LEFT_CENTER,
                            field_name(&v.field),
                            font("body", 15.0),
                            rgb(t.ink),
                        );
                        draw::source_mark(&clip, pos2(src_x + 6.0, line.center().y), 8.0, source(&v.from));
                        clip.text(
                            pos2(src_x + 16.0, line.center().y),
                            Align2::LEFT_CENTER,
                            who_text(&v.from, &d.stack),
                            font(SEMI, 13.0),
                            who_color(&v.from),
                        );
                        let shown = v.game.as_ref().or(v.yours.as_ref());
                        let main = shown.map_or("-".to_string(), |x| value_text(&v.field, x));
                        let acts = self.actions(r, v);
                        let act_w = if acts.is_empty() { 0.0 } else { 124.0 };
                        let mut vclip = clip.clone();
                        vclip.set_clip_rect(clip.clip_rect().intersect(Rect::from_min_max(
                            pos2(value_x, line.top()),
                            pos2(line.right() - act_w, line.bottom()),
                        )));
                        let g = vclip.layout_no_wrap(main, font(SEMI, 15.0), rgb(t.ink));
                        let w = g.size().x;
                        vclip.galley(pos2(value_x, line.center().y - g.size().y / 2.0), g, rgb(t.ink));
                        let beside = match (&v.from, &v.original) {
                            (From::Original, _) => None,
                            (_, Some(o)) => Some(format!(
                                " · {} {}",
                                if self.mode == Mode::ThisMod || self.mode == Mode::Clashes {
                                    "original"
                                } else {
                                    "before"
                                },
                                value_text(&v.field, o)
                            )),
                            (_, None) if !r.added && !map::original_known(r.kind, &v.field) => {
                                Some(" · original in the game's files".into())
                            }
                            _ => None,
                        };
                        if let Some(b) = beside {
                            vclip.text(
                                pos2(value_x + w, line.center().y),
                                Align2::LEFT_CENTER,
                                b,
                                font("body", 14.0),
                                rgb(t.dim),
                            );
                        }
                        let mut bx = line.right() - 4.0;
                        for a in acts.iter().rev() {
                            let word = match a {
                                Action::Reset => "Reset",
                                Action::Remove => "Remove",
                                Action::ShowInClashes => "Show in Clashes",
                            };
                            let g = p.layout_no_wrap(word.into(), font(LABEL, 14.0), rgb(t.ink));
                            let br = Rect::from_min_size(
                                pos2(bx - g.size().x - 28.0, line.top() + 3.0),
                                vec2(g.size().x + 28.0, 24.0),
                            );
                            if br.top() >= body.top()
                                && br.bottom() <= body.bottom()
                                && ui.put(br, small_button(word, false)).clicked()
                            {
                                self.focus = li;
                                match a {
                                    Action::Reset | Action::Remove => {
                                        self.picked.clear();
                                        self.reset();
                                        return (Some(Ask::Moved), drawn);
                                    }
                                    Action::ShowInClashes => {
                                        self.set_mode(Mode::Clashes);
                                        ask = Some(Ask::Moved);
                                    }
                                }
                            }
                            bx = br.left() - 6.0;
                        }
                        if v.loses() {
                            let yours = v.yours.as_ref().map_or(String::new(), |x| value_text(&v.field, x));
                            let other = who_text(&v.from, &d.stack);
                            let ny = line.bottom() + NOTE_H / 2.0 - 4.0;
                            let g = clip.layout_no_wrap(format!("yours: {yours}"), font(SEMI, 13.0), rgb(t.lime));
                            let w = g.size().x;
                            clip.galley(pos2(value_x, ny - g.size().y / 2.0), g, rgb(t.lime));
                            clip.text(
                                pos2(value_x + w, ny),
                                Align2::LEFT_CENTER,
                                format!(
                                    " · not used: {other} is above yours in your game. Editing this value edits yours."
                                ),
                                font("body", 13.0),
                                rgb(t.dim),
                            );
                        }
                        if ui.interact(line, egui::Id::unique(("chg-row", li)), egui::Sense::click()).clicked() {
                            let shift = ui.input(|i| i.modifiers.shift);
                            if shift {
                                self.picked.insert(self.focus);
                                self.picked.insert(li);
                            }
                            self.focus = li;
                        }
                        drawn.rows.push(line);
                    }
                    li += 1;
                }
            }
            y += h;
        }
        let _ = lines;
        if rows.is_empty() {
            let line = match &self.mode {
                Mode::ThisMod if self.show == Show::Losing => {
                    "Nothing of yours loses: every value you give is used in your game"
                }
                Mode::ThisMod => "Nothing changed yet: the original everywhere",
                _ => "No values differ",
            };
            clip.text(pos2(list.left(), y + 10.0), Align2::LEFT_TOP, line, font("body", 15.0), rgb(t.dim));
        }

        // The clashes: the stack as columns in the game's order, the winner last.
        if clash_rows > 0 {
            let packs: Vec<String> = {
                let mut v: Vec<String> = vec![];
                for (id, _) in &d.stack {
                    if d.clashes.iter().any(|c| c.stack.iter().any(|(p, _)| p == id)) {
                        v.push(id.clone());
                    }
                }
                v
            };
            let ty = y + 6.0;
            let g = clip.layout_no_wrap("Clashes".into(), font(TITLE, 19.0), rgb(t.lime));
            let w = g.size().x;
            clip.galley(pos2(list.left(), ty), g, rgb(t.lime));
            clip.text(
                pos2(list.left() + w + 6.0, ty + 12.0),
                Align2::LEFT_CENTER,
                "· the stack as columns, in your game's order",
                font("body", 13.0),
                rgb(t.dim),
            );
            let cols = packs.len() + 3;
            let first_w = (list.width() * 0.22).max(160.0);
            let cw = (list.width() - first_w) / (cols - 1) as f32;
            let col_x = |i: usize| if i == 0 { list.left() + 8.0 } else { list.left() + first_w + cw * (i - 1) as f32 };
            let mut ry = ty + 30.0;
            let head_y = ry + 14.0;
            clip.text(pos2(col_x(0), head_y), Align2::LEFT_CENTER, "Value", font("body", 12.0), rgb(t.dim));
            draw::source_mark(&clip, pos2(col_x(1) + 4.0, head_y), 8.0, Source::Original);
            clip.text(pos2(col_x(1) + 12.0, head_y), Align2::LEFT_CENTER, "original", font("body", 12.0), rgb(t.dim));
            for (i, id) in packs.iter().enumerate() {
                let f = if *id == d.folder { From::Yours } else { From::Other(id.clone()) };
                draw::source_mark(&clip, pos2(col_x(i + 2) + 4.0, head_y), 8.0, source(&f));
                clip.text(
                    pos2(col_x(i + 2) + 12.0, head_y),
                    Align2::LEFT_CENTER,
                    who_text(&f, &d.stack).replace("yours", &d.name),
                    font("body", 12.0),
                    rgb(t.dim),
                );
            }
            clip.text(
                pos2(col_x(cols - 1), head_y),
                Align2::LEFT_CENTER,
                "In your game",
                font("body", 12.0),
                rgb(t.dim),
            );
            ry += 28.0;
            for c in &d.clashes {
                let cy = ry + 14.0;
                clip.line_segment([pos2(list.left(), ry), pos2(list.right(), ry)], Stroke::new(1.0, draw::rule()));
                let item = d.rows.iter().find(|r| r.key == c.key).map_or(c.key.clone(), |r| item_words(r, &d.folder).0);
                clip.text(
                    pos2(col_x(0), cy),
                    Align2::LEFT_CENTER,
                    format!("{item} · {}", field_name(&c.field)),
                    font("body", 14.0),
                    rgb(t.ink),
                );
                let orig =
                    c.stack.first().and_then(|(_, v)| v.as_ref()).map_or("-".into(), |v| value_text(&c.field, v));
                clip.text(pos2(col_x(1), cy), Align2::LEFT_CENTER, orig, font("body", 14.0), rgb(t.ink));
                for (i, id) in packs.iter().enumerate() {
                    let cell = c.stack.iter().find(|(p, _)| p == id).and_then(|(_, v)| v.as_ref());
                    let won = match &c.winner {
                        From::Yours => *id == d.folder,
                        From::Other(o) => o == id,
                        From::Original => false,
                    };
                    let text = cell.map_or("-".into(), |v| value_text(&c.field, v));
                    let col = if cell.is_none() { rgb(t.dim) } else { rgb(t.ink) };
                    let mut cc = clip.clone();
                    cc.set_clip_rect(
                        clip.clip_rect().intersect(Rect::from_min_size(pos2(col_x(i + 2), ry), vec2(cw - 6.0, 28.0))),
                    );
                    let g = cc.layout_no_wrap(text, font("body", 14.0), col);
                    let gw = g.size().x;
                    cc.galley(pos2(col_x(i + 2), cy - g.size().y / 2.0), g, col);
                    if cell.is_some() && !won {
                        // Not used: struck through.
                        cc.line_segment(
                            [pos2(col_x(i + 2), cy), pos2(col_x(i + 2) + gw, cy)],
                            Stroke::new(1.0, rgb(t.dim)),
                        );
                    }
                }
                clip.text(
                    pos2(col_x(cols - 1), cy),
                    Align2::LEFT_CENTER,
                    who_text(&c.winner, &d.stack).replace("yours", &d.name),
                    font(SEMI, 14.0),
                    rgb(t.ink),
                );
                ry += 28.0;
            }
            clip.line_segment([pos2(list.left(), ry), pos2(list.right(), ry)], Stroke::new(1.0, draw::rule()));
        }

        // The Inspector: beside the list, or a strip opening a drawer.
        let insp_rect = match insp {
            Some(r) => Some(r),
            None => {
                let strip = Rect::from_min_max(pos2(inner.right() - 26.0, inner.top()), inner.max);
                p.rect_filled(strip, 4.0, theme::over(t.panel, t.slab.over(t.page)));
                let g = p.layout_no_wrap("Inspector".into(), draw::label_font(14.0), rgb(t.ink));
                let pos = Pos2::new(strip.center().x - g.size().y / 2.0, strip.top() + 16.0 + g.size().x);
                p.add(egui::epaint::TextShape::new(pos, g, rgb(t.ink)).with_angle(-std::f32::consts::FRAC_PI_2));
                if ui.interact(strip, egui::Id::unique("chg-insp-strip"), egui::Sense::click()).clicked() {
                    self.inspector_open = !self.inspector_open;
                }
                self.inspector_open.then(|| {
                    Rect::from_min_max(
                        pos2(strip.left() - INSPECTOR_W - 8.0, inner.top()),
                        pos2(strip.left() - 6.0, inner.bottom()),
                    )
                })
            }
        };
        if let Some(ir) = insp_rect {
            if insp.is_none() {
                p.rect_filled(ir, 6.0, rgb(t.dialog));
            }
            if let Some(a) = self.inspector(ui, &p, ir, &d, &rows) {
                ask = Some(a);
            }
            drawn.inspector = Some(ir);
        }
        (ask, drawn)
    }

    fn inspector(&mut self, ui: &mut egui::Ui, p: &Painter, r: Rect, d: &Data, rows: &[Row]) -> Option<Ask> {
        let t = tokens();
        let mut ask = None;
        p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, draw::rule()));
        let body = r.shrink2(vec2(14.0, 6.0));
        let mut clip = p.clone();
        clip.set_clip_rect(r);
        clip.text(body.left_top(), Align2::LEFT_TOP, "Inspector", draw::label_font(16.0), rgb(t.lime));
        let Some(&(g, vi)) = self.lines().get(self.focus) else {
            clip.text(
                body.left_top() + vec2(0.0, 34.0),
                Align2::LEFT_TOP,
                "Nothing selected",
                font(TITLE, 19.0),
                rgb(t.ink),
            );
            return None;
        };
        let row = &rows[g];
        let v = &row.values[vi];
        let (item, _) = item_words(full(d, row), &d.folder);
        let mut y = body.top() + 32.0;
        clip.text(
            pos2(body.left(), y),
            Align2::LEFT_TOP,
            format!("{item} · {}", field_name(&v.field)),
            font(TITLE, 19.0),
            rgb(t.lime),
        );
        y += 32.0;
        // A position's ghost: the original place dashed, yours solid (top-down).
        if v.field == "position"
            && let (Some(o), Some(n)) =
                (v.original.as_ref().and_then(xy), v.game.as_ref().or(v.yours.as_ref()).and_then(xy))
        {
            let bx = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), 110.0));
            clip.rect_filled(bx, 6.0, draw::page_fill());
            let span = ((n.0 - o.0).abs().max((n.1 - o.1).abs()) as f32).max(1.0);
            let scale = (bx.width().min(bx.height()) * 0.35) / span;
            let c = bx.center();
            let mid = ((o.0 + n.0) / 2.0, (o.1 + n.1) / 2.0);
            let at = |q: (f64, f64)| pos2(c.x + (q.0 - mid.0) as f32 * scale, c.y - (q.1 - mid.1) as f32 * scale);
            let size = vec2(34.0, 24.0);
            let ro = Rect::from_center_size(at(o), size);
            for s in egui::Shape::dashed_line(
                &[ro.left_top(), ro.right_top(), ro.right_bottom(), ro.left_bottom(), ro.left_top()],
                Stroke::new(1.5, rgb(t.streak)),
                5.0,
                4.0,
            ) {
                clip.add(s);
            }
            clip.rect_stroke(
                Rect::from_center_size(at(n), size),
                2.0,
                Stroke::new(1.5, rgb(t.lime)),
                StrokeKind::Inside,
            );
            clip.text(
                bx.left_bottom() + vec2(8.0, -10.0),
                Align2::LEFT_CENTER,
                "dashed: original place · solid: in your game",
                font("body", 12.0),
                rgb(t.dim),
            );
            y = bx.bottom() + 8.0;
        }
        let line = |clip: &Painter, y: &mut f32, mark: Option<Source>, k: &str, val: String| {
            if let Some(s) = mark {
                draw::source_mark(clip, pos2(body.left() + 6.0, *y + 10.0), 9.0, s);
            }
            clip.text(pos2(body.left() + 20.0, *y + 10.0), Align2::LEFT_CENTER, k, font("body", 14.0), rgb(t.dim));
            let mut vc = clip.clone();
            vc.set_clip_rect(
                clip.clip_rect()
                    .intersect(Rect::from_min_max(pos2(body.left() + 112.0, *y), pos2(body.right(), *y + 22.0))),
            );
            vc.text(pos2(body.left() + 112.0, *y + 10.0), Align2::LEFT_CENTER, val, font(SEMI, 14.0), rgb(t.ink));
            *y += 24.0;
        };
        let txt = |x: Option<&toml::Value>| x.map_or("-".to_string(), |x| value_text(&v.field, x));
        let orig = if v.original.is_none() && !row.added && !map::original_known(row.kind, &v.field) {
            "in the game's files".to_string()
        } else {
            txt(v.original.as_ref())
        };
        line(
            &clip,
            &mut y,
            Some(Source::Original),
            "Original",
            if row.added { "none: you added it".into() } else { orig },
        );
        line(&clip, &mut y, Some(Source::Yours), "Yours", txt(v.yours.as_ref()));
        if let From::Other(_) = &v.from {
            line(
                &clip,
                &mut y,
                Some(Source::AnotherMod),
                "In your game",
                format!("{} · {}", txt(v.game.as_ref()), who_text(&v.from, &d.stack)),
            );
        }
        let step = d.steps.iter().rev().find(|s| s.items.contains(&(row.kind.to_string(), row.key.clone())));
        if let Some(s) = step {
            line(&clip, &mut y, None, "Changed in", format!("step {} · {}", s.seq, s.label));
        }
        y += 6.0;
        let acts = self.actions(row, v);
        let br = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), 26.0));
        let mut bx = br.left();
        let mut button = |word: &str, bx: &mut f32| {
            let g = p.layout_no_wrap(word.into(), font(LABEL, 14.0), rgb(t.ink));
            let r = Rect::from_min_size(pos2(*bx, br.top()), vec2(g.size().x + 28.0, 24.0));
            *bx = r.right() + 6.0;
            r.right() <= body.right() && ui.put(r, small_button(word, false)).clicked()
        };
        if (acts.contains(&Action::Reset) || acts.contains(&Action::Remove))
            && button(if acts.contains(&Action::Remove) { "Remove" } else { "Reset" }, &mut bx)
        {
            self.reset();
            ask = Some(Ask::Moved);
        }
        if let Some(s) = step
            && button("Show in History", &mut bx)
        {
            self.set_mode(Mode::Step(s.seq));
            ask = Some(Ask::Moved);
        }
        let world = if row.kind == map::RACE { "Show in Races" } else { "Show in World" };
        if button(world, &mut bx) {
            ask = Some(Ask::Go(item_address(&d.folder, row, Some(&v.field))));
        }
        y += 36.0;
        let hint = "Pick several rows (Shift+↓, or A on each with the pad) to reset them in one step.";
        let g = clip.layout(hint.into(), font("body", 13.0), rgb(t.dim), body.width());
        clip.galley(pos2(body.left(), y), g, rgb(t.dim));
        let _ = SEMI;
        ask
    }
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;

fn xy(v: &toml::Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    let f = |i: usize| a.get(i).and_then(|x| x.as_float().or(x.as_integer().map(|n| n as f64)));
    Some((f(0)?, f(1)?))
}
