//! The Mod page (SHARE ▸ Mod, Ctrl+0; docs/modding/studio.md, "The Mod page"): the open mod's details (name,
//! author, description, each change one step of the mod's history), its files as a shared copy holds them, its
//! clashes with the other mods in the player's game (read-only, from the Changes data), whether it is ready to
//! share, the launcher hand-off ("In your game: off · Turn on in the launcher") and the export
//! ([`ob_studio::share`]).
//!
//! The page's rows are walked with ↑ ↓ (D-pad) and Enter (A): a detail row opens its text field (Enter keeps it
//! as one step, Esc leaves it), Files folds open, Clashes opens Changes on its Clashes view, Export exports, and
//! asks first when textures come from the original game's pixels.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nfsu2::studio_api::map::assets::MapAssets;

use anyhow::{Context as _, Result, bail};
use egui::{Align2, Rect, Stroke, StrokeKind, UiBuilder, pos2, vec2};
use ob_look::lamp::Mark;
use ob_studio::changes::From;
use ob_studio::session::Level;
use ob_studio::share::{self, Derived, Point};

use crate::editor::theme::{self, TITLE, font, rgb, tokens};
use crate::tools::map::history::{self, History, Outcome};
use crate::tools::map::{MapMod, store};

use super::changes::{self, field_name};
use super::draw::{self, Source};
use super::home;
use super::words;

/// The rows the keys and the pad walk, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Name,
    Author,
    Description,
    Files,
    Clashes,
    ExportTo,
    Export,
}

pub const LINES: [Line; 7] =
    [Line::Name, Line::Author, Line::Description, Line::Files, Line::Clashes, Line::ExportTo, Line::Export];

impl Line {
    pub fn label(self) -> &'static str {
        match self {
            Line::Name => words::NAME,
            Line::Author => words::AUTHOR,
            Line::Description => words::DESCRIPTION,
            Line::Files => words::FILES,
            Line::Clashes => words::CLASHES,
            Line::ExportTo => words::EXPORT_TO,
            Line::Export => words::EXPORT,
        }
    }

    /// A text the page edits.
    pub fn text(self) -> bool {
        matches!(self, Line::Name | Line::Author | Line::Description | Line::ExportTo)
    }
}

/// One clash as the page lists it: the item, the value's name, who wins in the player's game.
#[derive(Clone, Debug, PartialEq)]
pub struct ClashLine {
    pub item: String,
    pub field: String,
    pub winner: String,
    pub source: Source,
}

/// What the page shows of a mod.
#[derive(Clone, Debug)]
pub struct Data {
    pub folder: String,
    pub map: MapMod,
    pub in_game: bool,
    pub files: Vec<share::File>,
    pub clashes: Vec<ClashLine>,
    pub derived: Vec<Derived>,
    pub ready: Vec<Point>,
    /// The map checks' warnings (the ready list says them in one line).
    pub check_warnings: usize,
}

fn read_map(maps: &Path, folder: &str) -> Result<MapMod> {
    let dir = maps.join(folder);
    let h =
        History::open_read(&dir, &store::editor_dir(maps, folder), ob_studio::MOD_DOCS, Arc::new(history::disk::Real))
            .with_context(|| format!("reading the history of {folder}"))?;
    MapMod::parse(h.doc(store::FILE).unwrap_or_default()).map_err(|e| anyhow::anyhow!("{e}"))
}

/// The race tracks the map checks read in place of the game's (tests).
pub type GivenTracks = fn() -> Box<dyn nfsu2::studio_api::map::assets::MapAssets>;

/// The map checks of `map` with the game's race tracks in `game` (None: they could not run).
fn run_checks(
    map: &MapMod,
    dir: &Path,
    game: Option<&Path>,
    given: Option<GivenTracks>,
) -> Option<Vec<ob_studio::validate::Check>> {
    use nfsu2::studio_api::map::assets::GameAssets;
    if map.races.is_empty() {
        return Some(vec![]);
    }
    let assets: Box<dyn MapAssets> = match given {
        Some(make) => make(),
        None => Box::new(GameAssets::new(game?.to_path_buf())),
    };
    let tracks: Vec<u16> = assets.tracks(ob_studio::cli::REGION).iter().map(|t| t.number).collect();
    let mut nets = HashMap::new();
    for t in tracks.iter().copied().chain(map.races.iter().map(|r| r.track)) {
        nets.entry(t).or_insert_with(|| assets.road_net(t));
    }
    if nets.values().all(Option::is_none) {
        return None;
    }
    let tested = ob_studio::validate::Tested::load(&dir.join(store::EDITOR_DIR));
    Some(ob_studio::validate::checks(map, &nets, &tracks, &tested))
}

/// Read the page's data for the mod in `maps/<folder>`; `game` is the game folder the map checks read.
pub fn load(maps: &Path, folder: &str, game: Option<&Path>, given: Option<GivenTracks>) -> Result<Data> {
    let map = read_map(maps, folder)?;
    let c = changes::load(maps, folder)?;
    let dir = maps.join(folder);
    let name_of =
        |pack: &str| c.stack.iter().find(|(f, _)| f == pack).map_or_else(|| pack.to_string(), |(_, n)| n.clone());
    let clashes = c
        .clashes
        .iter()
        .map(|k| {
            let row = c.rows.iter().find(|r| r.kind == k.kind && r.key == k.key);
            let item = row.map_or_else(|| k.key.clone(), |r| changes::item_words(&c, r).0);
            let (winner, source) = match &k.winner {
                From::Yours => (words::YOURS_WINS.to_string(), Source::Yours),
                From::Original => (words::THE_ORIGINAL_STAYS.to_string(), Source::Original),
                From::Other(p) => (format!("{} wins", name_of(p)), Source::AnotherMod),
            };
            ClashLine { item, field: field_name(&k.field), winner, source }
        })
        .collect();
    let derived = share::derived(&dir);
    let checks = run_checks(&map, &dir, game, given);
    let mut ready = share::ready(&map, checks.as_deref(), derived.len());
    ready.extend(share::looks(&dir));
    let check_warnings = checks
        .as_deref()
        .map_or(0, |c| c.iter().filter(|c| c.level == ob_studio::session::Level::Warning && !c.ok).count());
    Ok(Data {
        check_warnings,
        folder: folder.into(),
        files: share::files(&dir).unwrap_or_default(),
        in_game: c.in_game,
        map,
        clashes,
        derived,
        ready,
    })
}

/// Change one of the mod's details as one step of its history (its writer's lock taken for the step and let go).
pub fn set_detail(maps: &Path, folder: &str, line: Line, value: &str) -> Result<bool> {
    let dir = maps.join(folder);
    let mut h = History::open(&dir, &store::editor_dir(maps, folder), ob_studio::MOD_DOCS, history::STUDIO, {
        Arc::new(history::disk::Real)
    })?;
    if let Some(owner) = h.read_only_by() {
        let who = owner.map_or("another program".to_string(), |o| o.program.clone());
        bail!("{who} has this mod open; changes wait until it closes the mod");
    }
    let mut m = MapMod::parse(h.doc(store::FILE).unwrap_or_default()).map_err(|e| anyhow::anyhow!("{e}"))?;
    if !apply(&mut m, line, value) {
        return Ok(false);
    }
    let docs = BTreeMap::from([(store::FILE.to_string(), Some(m.write()))]);
    match h.commit(history::STUDIO, words::MOD_DETAILS, docs)? {
        Outcome::Made(_) => {}
        Outcome::Unchanged => return Ok(false),
        other => bail!("not changed: {other:?}"),
    }
    h.save()?;
    Ok(true)
}

/// Put `value` into the detail `line` of `m`; false when it already was so.
pub fn apply(m: &mut MapMod, line: Line, value: &str) -> bool {
    let value = value.trim().to_string();
    let f = match line {
        Line::Name => &mut m.name,
        Line::Author => &mut m.author,
        Line::Description => &mut m.description,
        _ => return false,
    };
    if *f == value {
        return false;
    }
    *f = value;
    true
}

/// What was drawn where (for the tests and the shot check).
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub rows: Vec<Rect>,
    pub focus: Option<Rect>,
    pub warning: Option<Rect>,
}

/// The Mod page.
pub struct Page {
    maps: PathBuf,
    /// The game folder the map checks read (the launcher's), looked up once.
    game: Option<Option<PathBuf>>,
    pub data: Option<Data>,
    pub error: Option<String>,
    pub focus: usize,
    /// The detail row's text while it is being edited.
    pub editing: Option<String>,
    pub files_open: bool,
    /// The warning before exporting textures from the original game's pixels is up.
    pub confirm: bool,
    /// Where Export writes (the `.zip`, or a folder).
    pub export_to: String,
    /// What the last action did.
    pub note: Option<(Mark, String)>,
    /// Exports written from this page so far (the tutorial's Share it step ticks on it).
    pub exports: u64,
    /// The race tracks the map checks read in place of the game's (tests).
    pub given_tracks: Option<GivenTracks>,
    pub last: Drawn,
}

impl Page {
    pub fn new(maps: PathBuf) -> Page {
        Page {
            maps,
            game: None,
            data: None,
            error: None,
            focus: 0,
            editing: None,
            files_open: false,
            confirm: false,
            export_to: String::new(),
            note: None,
            exports: 0,
            given_tracks: None,
            last: Drawn::default(),
        }
    }

    /// Set the game folder the map checks read (tests; otherwise the launcher's).
    pub fn set_game(&mut self, game: Option<PathBuf>) {
        self.game = Some(game);
    }

    /// Read the mod again (every visit: World may have changed it since).
    pub fn load(&mut self, folder: Option<&str>) {
        let Some(folder) = folder else {
            self.data = None;
            self.error = None;
            return;
        };
        if self.game.is_none() {
            self.game = Some(nfsu2::studio_api::qol::setup::game_root());
        }
        let game = self.game.clone().flatten();
        let other = self.data.as_ref().is_none_or(|d| d.folder != folder);
        match load(&self.maps, folder, game.as_deref(), self.given_tracks) {
            Ok(d) => {
                self.data = Some(d);
                self.error = None;
            }
            Err(e) => {
                self.data = None;
                self.error = Some(format!("{e:#}"));
            }
        }
        if other {
            self.focus = 0;
            self.editing = None;
            self.confirm = false;
            self.note = None;
            // Overbright's exports folder (made by the export), never the home folder itself.
            self.export_to = share::default_zip(&crate::platform::dirs::exports(), folder).display().to_string();
        }
    }

    /// What the ready list says is wrong: (problems: points that stop the export, warnings: the rest, the map
    /// checks' warnings among them). The status bar shows them.
    pub fn counts(&self) -> Option<(usize, usize)> {
        let d = self.data.as_ref()?;
        let bad = |l| d.ready.iter().filter(|p| !p.ok && p.level == l).count();
        Some((bad(ob_studio::session::Level::Error), bad(ob_studio::session::Level::Warning) + d.check_warnings))
    }

    pub fn line(&self) -> Line {
        LINES[self.focus.min(LINES.len() - 1)]
    }

    pub fn step_focus(&mut self, d: i32) {
        if self.editing.is_some() || self.confirm {
            return;
        }
        self.focus = (self.focus as i32 + d).clamp(0, LINES.len() as i32 - 1) as usize;
    }

    fn value(&self, line: Line) -> String {
        let Some(d) = &self.data else { return String::new() };
        match line {
            Line::Name => d.map.name.clone(),
            Line::Author => d.map.author.clone(),
            Line::Description => d.map.description.clone(),
            Line::ExportTo => self.export_to.clone(),
            _ => String::new(),
        }
    }

    /// Enter / A on the focused row: what the shell must do (go to an address, export, keep a detail).
    pub fn activate(&mut self) -> Option<Act> {
        if self.data.is_none() {
            return None;
        }
        if self.confirm {
            self.confirm = false;
            return Some(Act::Export { derived_ok: true });
        }
        if let Some(text) = self.editing.take() {
            return match self.line() {
                Line::ExportTo => {
                    self.export_to = text.trim().to_string();
                    None
                }
                l => Some(Act::Detail(l, text)),
            };
        }
        match self.line() {
            l if l.text() => {
                self.editing = Some(self.value(l));
                None
            }
            Line::Files => {
                self.files_open = !self.files_open;
                None
            }
            Line::Clashes => {
                let d = self.data.as_ref()?;
                Some(Act::Go(format!("ob://{}/@changes?mode=clashes", d.folder)))
            }
            _ => Some(Act::Export { derived_ok: false }),
        }
    }

    /// Esc / B: out of the text field or the warning (true), else nothing of the page's (false).
    pub fn back(&mut self) -> bool {
        if self.editing.take().is_some() {
            return true;
        }
        if self.confirm {
            self.confirm = false;
            self.note = Some((Mark::Info, words::NOT_EXPORTED.into()));
            return true;
        }
        false
    }

    /// Export the mod to [`Self::export_to`]; the warning first when textures come from the original game's
    /// pixels and the author has not said yes.
    pub fn export(&mut self, derived_ok: bool) {
        let Some(d) = &self.data else { return };
        if !share::is_ready(&d.ready) {
            self.note = Some((Mark::Error, words::NOT_READY_TO_SHARE_YET_SEE.into()));
            return;
        }
        let to = PathBuf::from(self.export_to.trim());
        if self.export_to.trim().is_empty() {
            self.note = Some((Mark::Error, words::SAY_WHERE_TO_EXPORT_TO.into()));
            return;
        }
        match share::export(&self.maps, &d.folder, &to, derived_ok) {
            Ok(e) => {
                self.exports += 1;
                self.note = Some((
                    Mark::Ok,
                    format!("Exported to {} · {}", e.to.display(), home::plural(e.files, "file", "files")),
                ))
            }
            Err(share::Refused::Derived(_)) => {
                self.confirm = true;
                self.focus = LINES.len() - 1;
            }
            Err(e) => self.note = Some((Mark::Error, format!("Could not export: {e}"))),
        }
    }

    /// Draw the page into the slab's `inner` rect.
    pub fn draw(&mut self, ui: &mut egui::Ui, inner: Rect, focused: bool, time: f32) -> Option<Act> {
        let mut drawn = Drawn::default();
        let act = self.draw_page(ui, inner, focused, time, &mut drawn);
        self.last = drawn;
        act
    }

    fn draw_page(
        &mut self,
        ui: &mut egui::Ui,
        inner: Rect,
        focused: bool,
        time: f32,
        drawn: &mut Drawn,
    ) -> Option<Act> {
        let t = tokens();
        let p = ui.painter().clone();
        let Some(d) = self.data.clone() else {
            p.text(inner.left_top(), Align2::LEFT_TOP, words::MOD, font(TITLE, 34.0), rgb(t.lime));
            let line = self.error.clone().unwrap_or_else(|| words::MOD_PAGE_NO_MOD.to_string());
            p.text(inner.left_top() + vec2(0.0, 58.0), Align2::LEFT_TOP, line, font("body", 16.0), rgb(t.dim));
            return None;
        };
        let mut act = None;
        let title = if d.map.name.trim().is_empty() { d.folder.clone() } else { d.map.name.clone() };
        let mut clip = p.clone();
        clip.set_clip_rect(inner);
        clip.text(inner.left_top(), Align2::LEFT_TOP, &title, font(TITLE, 34.0), rgb(t.lime));
        let top = inner.top() + 58.0;
        let wide = inner.width() >= 820.0;
        let left_w = if wide { inner.width() * 0.56 } else { inner.width() };
        let left = Rect::from_min_max(pos2(inner.left(), top), pos2(inner.left() + left_w, inner.bottom()));
        let right = if wide { Rect::from_min_max(pos2(left.right() + 28.0, top), inner.max) } else { left };
        let row_h = if inner.height() < 560.0 { 32.0 } else { 36.0 };
        let label_w = (left.width() * 0.3).clamp(110.0, 180.0);
        let ok_mark = |ok: bool, l: Level| {
            if ok {
                Mark::Ok
            } else if l == Level::Error {
                Mark::Error
            } else {
                Mark::Warn
            }
        };

        // A row: lamp, its name, its value; the focus ring when the keys are on it.
        let mut y = left.top();
        let row = |ui: &mut egui::Ui,
                   y: &mut f32,
                   area: Rect,
                   line: Option<Line>,
                   mark: Mark,
                   name: &str,
                   value: &str,
                   this: &mut Page,
                   drawn: &mut Drawn|
         -> bool {
            let r = Rect::from_min_size(pos2(area.left(), *y), vec2(area.width(), row_h));
            *y += row_h;
            if r.bottom() > inner.bottom() + 0.5 {
                return false;
            }
            drawn.rows.push(r);
            let is_focus = line.is_some_and(|l| this.line() == l);
            draw::row_head(&p, r, mark, name, 15.0, time);
            let cell_x = if name.is_empty() { r.left() + 32.0 } else { r.left() + label_w };
            let cell = Rect::from_min_max(pos2(cell_x, r.top() + 3.0), r.max - vec2(6.0, 3.0));
            if is_focus && this.editing.is_some() {
                let mut text = this.editing.clone().unwrap_or_default();
                let resp = ui.put(cell, egui::TextEdit::singleline(&mut text).font(font("body", 15.0)));
                resp.request_focus();
                this.editing = Some(text);
            } else {
                let mut c = p.clone();
                c.set_clip_rect(cell.intersect(inner));
                let shown = if value.is_empty() { words::NOT_SET } else { value };
                let shown = draw::elide(&p, shown, font("body", 15.0), cell.width(), true);
                c.text(pos2(cell.left(), r.center().y), Align2::LEFT_CENTER, &shown, font("body", 15.0), rgb(t.ink));
            }
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, draw::rule()));
            if is_focus && focused && !this.confirm {
                p.rect_stroke(r.expand2(vec2(2.0, 0.0)), 6.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Inside);
                drawn.focus = Some(r);
            }
            let clicked =
                line.is_some() && ui.interact(r, egui::Id::unique(("mod-row", name)), egui::Sense::click()).clicked();
            if clicked && let Some(l) = line {
                this.focus = LINES.iter().position(|x| *x == l).unwrap_or(0);
            }
            clicked
        };

        // Details.
        let mut clicked = None;
        for l in [Line::Name, Line::Author, Line::Description] {
            let v = self.value(l);
            if row(ui, &mut y, left, Some(l), Mark::Info, l.label(), &v, self, drawn) {
                clicked = Some(l);
            }
        }
        let game = if d.in_game { words::IN_GAME_ON_SHORT.to_string() } else { words::OFF_IN_GAME.to_string() };
        row(ui, &mut y, left, None, if d.in_game { Mark::Ok } else { Mark::Info }, words::IN_YOUR_GAME, &game, self, drawn);
        let size: u64 = d.files.iter().map(|f| f.size).sum();
        let files = format!("{} · {}", home::plural(d.files.len(), "file", "files"), bytes(size));
        let files = format!("{files} · {}", if self.files_open { words::HIDE } else { words::SHOW });
        if row(ui, &mut y, left, Some(Line::Files), Mark::Info, Line::Files.label(), &files, self, drawn) {
            clicked = Some(Line::Files);
        }
        if self.files_open {
            for f in d.files.iter().take(12) {
                if y + 24.0 > inner.bottom() {
                    break;
                }
                let mut c = p.clone();
                c.set_clip_rect(Rect::from_min_max(pos2(left.left(), y), pos2(left.right(), y + 24.0)));
                c.text(
                    pos2(left.left() + label_w, y + 12.0),
                    Align2::LEFT_CENTER,
                    &f.path,
                    font("body", 13.0),
                    rgb(t.ink),
                );
                c.text(
                    pos2(left.right() - 6.0, y + 12.0),
                    Align2::RIGHT_CENTER,
                    bytes(f.size),
                    font("body", 13.0),
                    rgb(t.dim),
                );
                y += 24.0;
            }
            if d.files.len() > 12 && y + 24.0 <= inner.bottom() {
                let more = format!("… and {} more", d.files.len() - 12);
                p.text(
                    pos2(left.left() + label_w, y + 12.0),
                    Align2::LEFT_CENTER,
                    more,
                    font("body", 13.0),
                    rgb(t.dim),
                );
                y += 24.0;
            }
        }
        let clash_word = if d.clashes.is_empty() {
            words::NONE_WITH_THE_MODS_IN_YOUR.to_string()
        } else {
            format!("{} · Show in Changes", home::plural(d.clashes.len(), "value", "values"))
        };
        let cm = if d.clashes.is_empty() { Mark::Ok } else { Mark::Warn };
        if row(ui, &mut y, left, Some(Line::Clashes), cm, words::CLASHES, &clash_word, self, drawn) {
            clicked = Some(Line::Clashes);
        }
        for c in d.clashes.iter().take(6) {
            if y + 26.0 > inner.bottom() {
                break;
            }
            let r = Rect::from_min_size(pos2(left.left(), y), vec2(left.width(), 26.0));
            draw::source_mark(&p, pos2(r.left() + label_w - 14.0, r.center().y), 9.0, c.source);
            let mut cl = p.clone();
            cl.set_clip_rect(r);
            let text = format!("{} · {} · {}", c.item, c.field, c.winner);
            cl.text(pos2(r.left() + label_w, r.center().y), Align2::LEFT_CENTER, text, font("body", 13.0), rgb(t.ink));
            y += 26.0;
        }

        // Ready to share, Export.
        let mut ry = if wide { right.top() } else { y + 16.0 };
        if ry + 22.0 <= inner.bottom() {
            p.text(pos2(right.left(), ry), Align2::LEFT_TOP, words::READY_TO_SHARE, font("body", 12.0), rgb(t.dim));
        }
        ry += 22.0;
        for pt in &d.ready {
            row(ui, &mut ry, right, None, ok_mark(pt.ok, pt.level), "", &pt.text, self, drawn);
        }
        if row(
            ui,
            &mut ry,
            right,
            Some(Line::ExportTo),
            Mark::Info,
            Line::ExportTo.label(),
            &self.export_to.clone(),
            self,
            drawn,
        ) {
            clicked = Some(Line::ExportTo);
        }
        let ready = share::is_ready(&d.ready);
        let b = Rect::from_min_size(pos2(right.left(), ry + 10.0), vec2(right.width().min(320.0), 38.0));
        if b.bottom() <= inner.bottom() {
            drawn.rows.push(b);
            let resp = ui
                .scope_builder(UiBuilder::new().max_rect(b), |ui| {
                    ui.spacing_mut().button_padding = vec2(18.0, 8.0);
                    ui.add_enabled(ready, theme::main_action(words::EXPORT).min_size(vec2(0.0, 36.0)))
                })
                .inner;
            if self.line() == Line::Export && focused && !self.confirm {
                p.rect_stroke(resp.rect.expand(3.0), 8.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Outside);
                drawn.focus = Some(resp.rect);
            }
            if resp.clicked() {
                self.focus = LINES.len() - 1;
                act = Some(Act::Export { derived_ok: false });
            }
            ry = b.bottom() + 12.0;
        }
        if let Some((mark, note)) = &self.note
            && ry + 24.0 <= inner.bottom()
        {
            draw::lamp(&p, pos2(right.left() + 14.0, ry + 12.0), 11.0, *mark, time);
            let g = p.layout(note.clone(), font("body", 14.0), rgb(t.ink), right.width() - 36.0);
            let h = g.size().y;
            p.galley(pos2(right.left() + 32.0, ry + 12.0 - 9.0), g, rgb(t.ink));
            ry += h.max(24.0) + 8.0;
        }

        // The warning before textures from the original game's pixels are shared: a card over the column.
        if self.confirm {
            let line = share::derived_warning(d.derived.len());
            let g = p.layout(line, font("body", 15.0), rgb(t.ink), right.width() - 48.0);
            let h = 52.0 + g.size().y + 14.0 + 40.0 + 16.0;
            let top = (ry + 4.0).min(inner.bottom() - h).max(right.top());
            let card = Rect::from_min_size(pos2(right.left(), top), vec2(right.width(), h));
            p.rect_filled(card, 8.0, theme::over(t.panel, t.slab.over(t.page)));
            p.rect_stroke(card, 8.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Inside);
            draw::lamp(&p, card.left_top() + vec2(30.0, 30.0), 18.0, Mark::Warn, time);
            p.text(
                card.left_top() + vec2(48.0, 30.0),
                Align2::LEFT_CENTER,
                words::DERIVED_TITLE,
                font(TITLE, 20.0),
                rgb(t.ink),
            );
            p.galley(card.left_top() + vec2(20.0, 52.0), g, rgb(t.ink));
            let buttons = Rect::from_min_size(card.left_top() + vec2(20.0, h - 56.0), vec2(card.width() - 40.0, 40.0));
            ui.scope_builder(UiBuilder::new().max_rect(buttons), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = vec2(18.0, 8.0);
                    if ui.add(theme::main_action(words::EXPORT_ANYWAY).min_size(vec2(0.0, 36.0))).clicked() {
                        self.confirm = false;
                        act = Some(Act::Export { derived_ok: true });
                    }
                    if ui.add(egui::Button::new(ob_look::words::BACK).min_size(vec2(0.0, 36.0))).clicked() {
                        self.back();
                    }
                });
            });
            drawn.warning = Some(card);
        }

        if let Some(l) = clicked {
            self.focus = LINES.iter().position(|x| *x == l).unwrap_or(0);
            if self.editing.is_none() {
                act = act.or_else(|| self.activate());
            }
        }
        act
    }
}

/// What the shell does for the page.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Go(String),
    Detail(Line, String),
    Export { derived_ok: bool },
}

/// A size in plain words (`12 KB`).
pub fn bytes(n: u64) -> String {
    match n {
        n if n < 1024 => format!("{n} bytes"),
        n if n < 1 << 20 => format!("{} KB", n.div_ceil(1024)),
        n => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
    }
}

#[cfg(test)]
#[path = "mod_page_tests.rs"]
mod tests;
