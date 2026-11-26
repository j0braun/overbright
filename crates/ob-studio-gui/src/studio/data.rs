//! The Data page (docs/modding/studio.md, "Data"; `ob://<mod>/@data`, Ctrl+G): one kind of record at a time as a
//! table, its columns the kind's schema ([`ob_studio::data`]), its rows the Changes view's rows, so every cell says
//! whether it is yours (the dot), the original's (the ring) or another mod's (the diamond) and shows the original
//! beside a value of yours. Click a header or Enter on it to sort; type in the search box to narrow the rows; Enter
//! on a cell edits it, Enter keeps it as one undo step (Ctrl+Z takes it back), Esc leaves it. A column the format
//! cannot write yet is read-only.

use egui::{Align2, Rect, Stroke, StrokeKind, pos2, vec2};
use ob_studio::changes::map::PLACEMENT;
use ob_studio::changes::{From, Row};
use ob_studio::data::{Grid, KINDS};

use crate::editor::theme::{self, LABEL, SEMI, TITLE, font, rgb, tokens};

use super::draw::{self, Source};
use super::words;

const ROW_H: f32 = 28.0;
const HEAD_H: f32 = 30.0;
/// The room at the right of a row for its "In Changes" button.
const SHOW_W: f32 = 112.0;

/// The kind's name on its tab.
pub fn kind_name(kind: &str) -> &'static str {
    if kind == PLACEMENT { words::PROPS } else { words::RACES }
}

/// What the page asks the shell to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Write `text` in a cell, as one undo step.
    Edit { kind: &'static str, key: String, field: &'static str, text: String },
    /// Show the row's entry on the Changes page.
    Changes { kind: &'static str, key: String },
    /// The page's own state moved (nothing for the shell to do).
    Moved,
}

/// What the page drew where (the tests' clip check).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub rows: Vec<Rect>,
    pub header: Option<Rect>,
    pub focus: Option<Rect>,
}

/// The page's state in one window.
#[derive(Debug)]
pub struct Page {
    pub kind: &'static str,
    pub grid: Option<Grid>,
    /// The focused row of what is shown; None: the header (Enter sorts the focused column).
    pub row: Option<usize>,
    pub col: usize,
    /// The text of the cell being edited.
    pub editing: Option<String>,
    /// The search box has the keys.
    pub searching: bool,
    release: bool,
    pub note: Option<String>,
    /// A kind tab was clicked (the shell loads it: the rows are the Changes page's).
    pub kind_pick: Option<&'static str>,
    scroll: f32,
    /// The rows the last frame had room for.
    pub shown: usize,
    pub last: Drawn,
}

impl Default for Page {
    fn default() -> Page {
        Page {
            kind: KINDS[0],
            grid: None,
            row: Some(0),
            col: 0,
            editing: None,
            searching: false,
            release: false,
            note: None,
            kind_pick: None,
            scroll: 0.0,
            shown: 10,
            last: Drawn::default(),
        }
    }
}

impl Page {
    /// Take the Changes view's rows (again), keeping the kind, the sort, the search and the focus.
    pub fn load(&mut self, rows: &[Row]) {
        match &mut self.grid {
            Some(g) if g.kind == self.kind => g.reload(rows),
            _ => self.grid = Grid::from_rows(self.kind, rows),
        }
        self.clamp();
    }

    fn clamp(&mut self) {
        let (cols, rows) = self.grid.as_ref().map_or((0, 0), |g| (g.columns.len(), g.counts().1));
        self.col = self.col.min(cols.saturating_sub(1));
        self.row = self.row.map(|r| r.min(rows.saturating_sub(1)));
        if rows == 0 {
            self.row = None;
        }
    }

    pub fn typing(&self) -> bool {
        self.editing.is_some() || self.searching
    }

    pub fn set_kind(&mut self, kind: &'static str, rows: &[Row]) {
        if self.kind != kind {
            self.kind = kind;
            self.grid = None;
            self.col = 0;
            self.row = Some(0);
            self.editing = None;
            self.load(rows);
        }
    }

    /// The other kind's tab.
    pub fn other_kind(&self) -> &'static str {
        KINDS.into_iter().find(|k| *k != self.kind).unwrap_or(self.kind)
    }

    /// ↑ ↓: a row on (the header above the first); false past the ends.
    pub fn step_row(&mut self, d: i32) -> bool {
        let n = self.grid.as_ref().map_or(0, |g| g.counts().1) as i32;
        let cur = self.row.map_or(-1, |r| r as i32);
        let to = (cur + d).clamp(-1, n - 1);
        self.row = (to >= 0).then_some(to as usize);
        to != cur
    }

    /// ← →: a column on.
    pub fn step_col(&mut self, d: i32) -> bool {
        let n = self.grid.as_ref().map_or(0, |g| g.columns.len()) as i32;
        let to = (self.col as i32 + d).clamp(0, (n - 1).max(0)) as usize;
        let moved = to != self.col;
        self.col = to;
        moved
    }

    /// Enter / pad A: sort on the header, start editing a cell, keep what is typed.
    pub fn activate(&mut self) -> Option<Ask> {
        let g = self.grid.as_ref()?;
        if let Some(text) = self.editing.take() {
            let (key, c) = (g.key(self.row?)?.to_string(), g.columns[self.col]);
            return Some(Ask::Edit { kind: g.kind, key, field: c.field, text });
        }
        let Some(r) = self.row else {
            self.grid.as_mut()?.sort_by(self.col);
            return Some(Ask::Moved);
        };
        let c = g.columns[self.col];
        if !c.editable {
            self.note = Some(format!("{} cannot be changed here yet", c.label));
            return None;
        }
        // A switch flips; the rest is typed.
        if c.ty == ob_studio::data::Ty::Bool {
            let key = g.key(r)?.to_string();
            let text = if g.cell(r, self.col)?.text == "yes" { "no" } else { "yes" };
            return Some(Ask::Edit { kind: g.kind, key, field: c.field, text: text.into() });
        }
        self.editing = Some(g.cell(r, self.col)?.text);
        None
    }

    /// Esc / pad B: leave an edit or the search; false when there was none.
    pub fn back(&mut self) -> bool {
        let was = self.editing.take().is_some() || self.searching;
        if self.searching {
            self.searching = false;
            self.release = true;
        }
        was
    }

    /// The focused row's Changes entry.
    pub fn focused_changes(&self) -> Option<Ask> {
        let g = self.grid.as_ref()?;
        Some(Ask::Changes { kind: g.kind, key: g.key(self.row?)?.to_string() })
    }

    /// Draw the page into the slab's `inner` rect; what it asks and what was drawn where.
    pub fn draw(&mut self, ui: &mut egui::Ui, inner: Rect, focused: bool) -> (Option<Ask>, Drawn) {
        let (ask, drawn) = self.draw_page(ui, inner, focused);
        self.last = drawn.clone();
        (ask, drawn)
    }

    fn draw_page(&mut self, ui: &mut egui::Ui, inner: Rect, focused: bool) -> (Option<Ask>, Drawn) {
        let t = tokens();
        let p = ui.painter().clone();
        let mut ask = None;
        let mut drawn = Drawn::default();
        let y0 = inner.top();
        let tg = p.layout_no_wrap(words::DATA.into(), font(TITLE, 24.0), rgb(t.lime));
        let tw = tg.size().x;
        p.galley(pos2(inner.left(), y0 + 14.0 - tg.size().y / 2.0), tg, rgb(t.lime));
        let mut x = inner.left() + tw + 18.0;
        for k in KINDS {
            let label = kind_name(k);
            let g = p.layout_no_wrap(label.into(), draw::label_font(16.0), rgb(t.ink));
            let r = Rect::from_min_size(pos2(x, y0), vec2(g.size().x + 20.0, 28.0));
            let cur = k == self.kind;
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
            if ui.interact(r, egui::Id::new(("data-kind", k)), egui::Sense::click()).clicked() {
                self.kind_pick = Some(k);
            }
            x = r.right() + 6.0;
        }
        // Search.
        let search = Rect::from_min_size(pos2(inner.right() - 220.0, y0), vec2(220.0, 28.0));
        if search.left() > x + 8.0 {
            let (rows_n, shown_n) = self.grid.as_ref().map_or((0, 0), |g| g.counts());
            let mut text = self.grid.as_ref().map_or(String::new(), |g| g.search.clone());
            let before = text.clone();
            let resp =
                ui.put(search, egui::TextEdit::singleline(&mut text).hint_text(words::SEARCH).font(font("body", 15.0)));
            if std::mem::take(&mut self.release) {
                resp.surrender_focus();
            }
            self.searching = resp.has_focus() && !self.release;
            if text != before
                && let Some(g) = self.grid.as_mut()
            {
                g.set_search(&text);
                self.clamp();
            }
            let count = format!("{shown_n} of {rows_n}");
            let g = p.layout_no_wrap(count, font("body", 13.0), rgb(t.dim));
            if search.left() - g.size().x - 12.0 > x {
                p.galley(pos2(search.left() - g.size().x - 10.0, y0 + 14.0 - g.size().y / 2.0), g, rgb(t.dim));
            }
        }
        let Some(grid) = self.grid.clone() else {
            p.text(
                pos2(inner.left(), y0 + 60.0),
                Align2::LEFT_TOP,
                words::NO_MOD_OPEN_OPEN_ONE_ON_3,
                font("body", 15.0),
                rgb(t.dim),
            );
            return (ask, drawn);
        };
        let mut top = y0 + 40.0;
        if let Some(n) = &self.note {
            p.text(pos2(inner.left(), top + 8.0), Align2::LEFT_CENTER, n, font("body", 13.0), rgb(t.lime));
            top += 20.0;
        }
        let width = (inner.width() - SHOW_W - 8.0).max(200.0);
        let n = grid.columns.len().max(1) as f32;
        let widths: Vec<f32> = grid
            .columns
            .iter()
            .map(|c| match c.field {
                "name" | "position" => width / n * 1.4,
                "backward" | "disabled" | "laps" | "track" => width / n * 0.7,
                _ => width / n,
            })
            .collect();
        let sum: f32 = widths.iter().sum();
        let widths: Vec<f32> = widths.iter().map(|w| w * width / sum).collect();
        // The header.
        let head = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), HEAD_H));
        p.rect_filled(head, 6.0, theme::over(t.panel, t.slab.over(t.page)));
        drawn.header = Some(head);
        let mut cx = inner.left();
        for (i, c) in grid.columns.iter().enumerate() {
            let r = Rect::from_min_size(pos2(cx, top), vec2(widths[i], HEAD_H));
            let arrow = match grid.sort {
                Some((s, up)) if s == i => {
                    if up {
                        " ▲"
                    } else {
                        " ▼"
                    }
                }
                _ => "",
            };
            let on = self.row.is_none() && self.col == i;
            if on && focused {
                p.rect_stroke(r, 6.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Inside);
                drawn.focus = Some(r);
            }
            let mut clip = p.clone();
            clip.set_clip_rect(r.shrink2(vec2(4.0, 0.0)));
            clip.text(
                pos2(r.left() + 10.0, r.center().y),
                Align2::LEFT_CENTER,
                format!("{}{arrow}", c.label),
                font(LABEL, 14.0),
                rgb(if c.editable { t.ink } else { t.dim }),
            );
            if ui.interact(r, egui::Id::new(("data-head", i)), egui::Sense::click()).clicked() {
                self.col = i;
                self.row = None;
                if let Some(g) = self.grid.as_mut() {
                    g.sort_by(i);
                }
                ask = Some(Ask::Moved);
            }
            cx += widths[i];
        }
        // The rows: only the ones in view are drawn (a mod's tables may be long), scrolled to keep the focus in.
        let body = Rect::from_min_max(pos2(inner.left(), head.bottom() + 4.0), inner.max);
        let total = grid.counts().1;
        self.shown = ((body.height() / ROW_H).floor() as usize).max(1);
        if ui.rect_contains_pointer(body) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if let Some(r) = self.row {
            let (a, b) = (r as f32 * ROW_H, (r + 1) as f32 * ROW_H);
            if b - self.scroll > body.height() {
                self.scroll = b - body.height();
            }
            if a < self.scroll {
                self.scroll = a;
            }
        }
        self.scroll = self.scroll.clamp(0.0, (total as f32 * ROW_H - body.height()).max(0.0));
        let first = (self.scroll / ROW_H).floor() as usize;
        let mut clip = p.clone();
        clip.set_clip_rect(body);
        if total == 0 {
            p.text(
                pos2(body.left() + 6.0, body.top() + 10.0),
                Align2::LEFT_TOP,
                if grid.counts().0 == 0 {
                    words::THIS_MOD_GIVES_NO_VALUES_OF
                } else {
                    words::NO_ROW_MATCHES_THE_SEARCH
                },
                font("body", 15.0),
                rgb(t.dim),
            );
        }
        for i in first..total.min(first + self.shown + 2) {
            let y = body.top() + i as f32 * ROW_H - self.scroll;
            let line = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), ROW_H));
            if line.bottom() > body.bottom() + 0.5 {
                break;
            }
            let on_row = self.row == Some(i);
            if i % 2 == 1 {
                clip.rect_filled(line, 0.0, theme::over(t.panel, t.slab.over(t.page)).gamma_multiply(0.5));
            }
            let mut cx = inner.left();
            for (ci, c) in grid.columns.iter().enumerate() {
                let r = Rect::from_min_size(pos2(cx, y), vec2(widths[ci], ROW_H));
                let cell = grid.cell(i, ci).unwrap_or_else(|| ob_studio::data::Cell {
                    text: String::new(),
                    from: From::Original,
                    yours: false,
                    original: None,
                });
                let here = on_row && self.col == ci;
                if here && focused {
                    clip.rect_stroke(r.shrink(1.0), 4.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Inside);
                    drawn.focus = Some(r);
                }
                let mut cc = clip.clone();
                cc.set_clip_rect(clip.clip_rect().intersect(r.shrink2(vec2(2.0, 0.0))));
                if here && self.editing.is_some() {
                    let mut text = self.editing.clone().unwrap_or_default();
                    let resp = ui
                        .put(r.shrink2(vec2(3.0, 2.0)), egui::TextEdit::singleline(&mut text).font(font("body", 15.0)));
                    resp.request_focus();
                    self.editing = Some(text);
                } else {
                    let src = match &cell.from {
                        From::Original => Source::Original,
                        From::Yours => Source::Yours,
                        From::Other(_) => Source::AnotherMod,
                    };
                    let mut tx = r.left() + 8.0;
                    if cell.yours || !matches!(cell.from, From::Original) {
                        draw::source_mark(&cc, pos2(r.left() + 10.0, r.center().y), 6.0, src);
                        tx += 14.0;
                    }
                    let main = if cell.text.is_empty() { "-".to_string() } else { cell.text.clone() };
                    let g = cc.layout_no_wrap(main, font(SEMI, 15.0), rgb(if c.editable { t.ink } else { t.dim }));
                    let w = g.size().x;
                    cc.galley(pos2(tx, r.center().y - g.size().y / 2.0), g, rgb(t.ink));
                    if let Some(o) = &cell.original {
                        cc.text(
                            pos2(tx + w + 6.0, r.center().y),
                            Align2::LEFT_CENTER,
                            format!("was {o}"),
                            font("body", 13.0),
                            rgb(t.dim),
                        );
                    }
                    if ui
                        .interact(r.intersect(body), egui::Id::new(("data-cell", i, ci)), egui::Sense::click())
                        .clicked()
                    {
                        self.row = Some(i);
                        self.col = ci;
                        self.editing = None;
                    }
                }
                cx += widths[ci];
            }
            if on_row {
                let br = Rect::from_min_size(pos2(line.right() - SHOW_W, y + 2.0), vec2(SHOW_W, ROW_H - 4.0));
                if br.bottom() <= body.bottom()
                    && ui.put(br, super::changes::small_button(words::IN_CHANGES, false)).clicked()
                {
                    ask = self.focused_changes();
                }
            }
            drawn.rows.push(line);
        }
        (ask, drawn)
    }
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
