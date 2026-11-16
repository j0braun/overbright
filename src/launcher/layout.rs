//! Laying a screen out: a centred column (at most 1100 units wide) filled
//! top to bottom; label / control rows that stack when the window is
//! narrow; controls that flow onto the next line when a line is full; and
//! a footer of buttons kept at the bottom when there is room. Every text is
//! measured and shortened ("…") or wrapped to its box here, so nothing is
//! clipped when drawn ([`super::ui::render`]).

use super::draw::{Rgb, theme};
use super::font::{Cut, Face, Fonts};
use super::ui::{Id, Kind, Mark, Metrics, Rect, TextBox, Ui, UiState, Widget, text};

/// A control in a row ([`Builder::row`]) or a button line.
#[derive(Clone, Debug)]
pub enum Ctrl {
    /// A value stepped with Left / Right; `values` are all it can show
    /// (its width fits the widest).
    Cycle {
        id: Id,
        value: String,
        values: Vec<String>,
        enabled: bool,
    },
    Button {
        id: Id,
        text: String,
        primary: bool,
        enabled: bool,
    },
    /// Plain text (wrapped in what is left of the line, or below).
    Text {
        text: String,
        color: Rgb,
    },
    /// One line of text in the body size, shortened in its middle ("…") to
    /// what the line leaves after the controls that follow it (a path).
    Path {
        text: String,
        color: Rgb,
    },
    /// An editable line taking the rest of the line (at least 240 units).
    Field {
        id: Id,
        text: String,
        placeholder: String,
    },
}

impl Ctrl {
    pub fn button(id: Id, text: &str) -> Ctrl {
        Ctrl::Button { id, text: text.into(), primary: false, enabled: true }
    }
    pub fn primary(id: Id, text: &str) -> Ctrl {
        Ctrl::Button { id, text: text.into(), primary: true, enabled: true }
    }
    pub fn dim(text: impl Into<String>) -> Ctrl {
        Ctrl::Text { text: text.into(), color: theme::DIM }
    }
    pub fn path(text: impl Into<String>) -> Ctrl {
        Ctrl::Path { text: text.into(), color: theme::TEXT }
    }
}

/// A card's inner padding and the gap after it (units).
pub const CARD_PAD: f32 = 12.0;
pub const CARD_GAP: f32 = 10.0;

/// The longest a paragraph's line gets (units): about 80 characters of
/// body text, so long sentences stay easy to read on a wide window.
pub const MEASURE: f32 = 760.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

pub struct Builder<'a> {
    pub fonts: &'a Fonts,
    pub m: Metrics,
    pub st: &'a mut UiState,
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub widgets: Vec<Widget>,
    pub default_focus: Option<Id>,
    /// The label column's width ([`Builder::label_column`]).
    pub label_w: f32,
    /// The page's content panel, open until the footer (or the end).
    pub content: Option<usize>,
    /// Rows a growing list ([`Builder::list_grow`]) adds to its least.
    pub grow: usize,
    /// The room left between the content and a footer at the window's
    /// bottom (px), for a growing list.
    pub slack: f32,
}

/// An item of a row of links ([`Builder::links`]).
pub enum Link {
    /// A flat button; `strong`: in bold (the folder shown); `whole`: the
    /// value the trace carries when it is more than the text (the whole
    /// path of the folder shown).
    Go { id: Id, text: String, strong: bool, whole: Option<String> },
    /// A dim separator between links.
    Sep(String),
}

impl<'a> Builder<'a> {
    pub fn new(fonts: &'a Fonts, m: Metrics, st: &'a mut UiState) -> Builder<'a> {
        let margin = m.s(32.0);
        let max_w = m.s(1100.0);
        let x0 = margin.max(((m.w - max_w) / 2.0).floor());
        Builder {
            fonts,
            m,
            st,
            x0,
            x1: m.w - x0,
            y: m.s(24.0),
            widgets: Vec::new(),
            default_focus: None,
            label_w: 0.0,
            content: None,
            grow: 0,
            slack: 0.0,
        }
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    /// A paragraph's line length: the column, at most [`MEASURE`].
    pub fn measure(&self) -> f32 {
        self.width().min(self.m.s(MEASURE))
    }

    fn lh(&self, size: f32) -> f32 {
        self.fonts.line_height(Face::Regular, size)
    }

    pub fn ctrl_h(&self) -> f32 {
        self.m.s(42.0).max(self.lh(self.m.body()) + self.m.s(14.0))
    }

    pub fn gap(&mut self, units: f32) {
        self.y += self.m.s(units);
    }

    /// The label column: the widest of `labels` plus a gap, at most 40 %
    /// of the column.
    pub fn label_column(&mut self, labels: &[&str]) {
        let size = self.m.body();
        let w = labels.iter().map(|l| self.fonts.width(Face::Regular, size, l)).fold(0.0, f32::max);
        self.label_w = (w + self.m.s(24.0)).min(self.width() * 0.4);
    }

    /// The name and, at the right (or below when there is no room), a
    /// short note; a line under both.
    /// The wordmark (Overbright's icon, the name in bold) and, at the
    /// right (or below when there is no room), a short note; a line under
    /// both with an accent bar under the name.
    pub fn header(&mut self, title: &str, note: &str) {
        let (ts, ns) = (self.m.title(), self.m.small());
        let (tw, th) = (self.fonts.width(Face::Bold, ts, title), self.fonts.line_height(Face::Bold, ts));
        let logo = th.round();
        let tx = self.x0 + logo + self.m.s(12.0);
        let nw = self.fonts.width(Face::Regular, ns, note);
        let nh = self.lh(ns);
        let ty = self.y + ((logo - th) / 2.0).round();
        self.push(None, Rect::new(self.x0, self.y, logo, logo), Kind::Logo, Vec::new());
        let mut texts = vec![text(Rect::new(tx, ty, tw, th), title, Face::Bold, ts, theme::TEXT)];
        let h = if tx - self.x0 + tw + self.m.s(24.0) + nw <= self.width() {
            let ny = ty + th - nh - self.m.s(3.0);
            texts.push(text(Rect::new(self.x1 - nw, ny.max(self.y), nw, nh), note, Face::Regular, ns, theme::PAGE_DIM));
            logo
        } else {
            let note = self.fonts.fit(Face::Regular, ns, note, self.width(), Cut::End);
            let nw = self.fonts.width(Face::Regular, ns, &note);
            texts.push(text(Rect::new(self.x0, self.y + logo, nw, nh), note, Face::Regular, ns, theme::PAGE_DIM));
            logo + nh
        };
        self.push(None, Rect::new(tx, self.y, self.x1 - tx, h), Kind::Label, texts);
        self.y += h + self.m.s(10.0);
        let t = self.m.s(1.0).max(1.0);
        let bar = self.m.s(3.0).max(2.0);
        self.push(None, Rect::new(self.x0, self.y, self.width(), t), Kind::Rule, Vec::new());
        self.push(
            None,
            Rect::new(self.x0, self.y + t, (tx - self.x0 + tw).min(self.width()), bar),
            Kind::Accent,
            Vec::new(),
        );
        self.y += t + bar + self.m.s(12.0);
    }

    /// Starts a card (a panel) at the current line: what follows is laid
    /// out inside it, `pad` units in; [`Builder::card_end`] closes it.
    pub fn card_begin(&mut self) -> usize {
        let i = self.widgets.len();
        self.push(None, Rect::new(self.x0, self.y, self.width(), 0.0), Kind::Card, Vec::new());
        let pad = self.m.s(CARD_PAD);
        self.x0 += pad;
        self.x1 -= pad;
        self.y += pad;
        i
    }

    /// Closes the card `i`: its box ends `pad` under the last widget.
    pub fn card_end(&mut self, i: usize) {
        let pad = self.m.s(CARD_PAD);
        self.x0 -= pad;
        self.x1 += pad;
        let r = &mut self.widgets[i].rect;
        r.h = self.y + pad - r.y;
        self.y = r.bottom() + self.m.s(CARD_GAP);
    }

    /// A small heading inside a card (bold, the body size).
    pub fn section(&mut self, s: &str) {
        let size = self.m.body();
        self.para(s, Face::Bold, size, theme::TEXT);
        self.y += self.m.s(6.0);
    }

    /// The large main button at `r` (its text in the heading size, bold).
    pub fn big_button(&mut self, id: Id, label: &str, r: Rect) {
        let size = self.m.heading() * 1.25;
        let size = size.round();
        let v = self.fonts.fit(Face::Bold, size, label, r.w - self.m.s(24.0), Cut::End);
        let vw = self.fonts.width(Face::Bold, size, &v);
        let lh = self.fonts.line_height(Face::Bold, size);
        let t = text(
            Rect::new(r.x + ((r.w - vw) / 2.0).round(), r.y + ((r.h - lh) / 2.0).round(), vw, lh),
            v,
            Face::Bold,
            size,
            theme::ON_ACCENT,
        );
        self.widgets.push(Widget {
            id: Some(id),
            rect: r,
            kind: Kind::Button { primary: true, big: true },
            texts: vec![t],
            enabled: true,
        });
    }

    pub fn rule(&mut self) {
        let t = self.m.s(1.0).max(1.0);
        self.push(None, Rect::new(self.x0, self.y, self.width(), t), Kind::Rule, Vec::new());
        self.y += t;
    }

    fn push(&mut self, id: Option<Id>, rect: Rect, kind: Kind, texts: Vec<TextBox>) {
        self.widgets.push(Widget { id, rect, kind, texts, enabled: true });
    }

    fn lines(&self, x: f32, y: f32, max: f32, s: &str, face: Face, size: f32, color: Rgb) -> (Vec<TextBox>, f32) {
        let lh = self.fonts.line_height(face, size);
        let step = (lh * 1.25).round();
        let lines = self.fonts.wrap(face, size, s, max);
        let n = lines.len();
        let boxes = lines
            .into_iter()
            .enumerate()
            .map(|(i, l)| {
                let w = self.fonts.width(face, size, &l);
                text(Rect::new(x, y + i as f32 * step, w, lh), l, face, size, color)
            })
            .collect();
        (boxes, (n.saturating_sub(1)) as f32 * step + lh)
    }

    /// A wrapped paragraph.
    pub fn para(&mut self, s: &str, face: Face, size: f32, color: Rgb) {
        let (texts, h) = self.lines(self.x0, self.y, self.measure(), s, face, size, color);
        self.push(None, Rect::new(self.x0, self.y, self.width(), h), Kind::Label, texts);
        self.y += h;
    }

    pub fn body(&mut self, s: &str) {
        let size = self.m.body();
        self.para(s, Face::Regular, size, theme::TEXT);
    }

    pub fn dim(&mut self, s: &str) {
        let size = self.m.small();
        self.para(s, Face::Regular, size, theme::DIM);
    }

    pub fn heading(&mut self, s: &str) {
        let size = self.m.heading();
        self.para(s, Face::Bold, size, theme::TEXT);
        self.y += self.m.s(6.0);
    }

    /// A status line: its mark and wrapped text.
    pub fn status(&mut self, mark: Mark, s: &str) {
        let size = self.m.body();
        let lh = self.lh(size);
        let is = (lh * 0.85).round();
        let icon = Rect::new(self.x0, self.y + (lh - is) / 2.0, is, is);
        let tx = self.x0 + is + self.m.s(12.0);
        let (texts, h) = self.lines(tx, self.y, self.x0 + self.measure() - tx, s, Face::Regular, size, theme::TEXT);
        self.push(None, Rect::new(self.x0, self.y, self.width(), h), Kind::Status { mark, icon }, texts);
        self.y += h + self.m.s(8.0);
    }

    /// A progress bar across the content, `fraction` (0..1) of it filled.
    pub fn bar(&mut self, fraction: f32) {
        let h = self.m.s(10.0).round().max(4.0);
        self.push(None, Rect::new(self.x0, self.y, self.width(), h), Kind::Bar { fraction }, Vec::new());
        self.y += h + self.m.s(12.0);
    }

    /// A label in the label column and its controls beside it, or the
    /// label above them when the column is too narrow for both.
    pub fn row(&mut self, label: &str, ctrls: Vec<Ctrl>) {
        let size = self.m.body();
        let lh = self.lh(size);
        let ch = self.ctrl_h();
        let stacked = self.width() - self.label_w < self.m.s(330.0);
        let label_max = if stacked { self.width() } else { self.label_w - self.m.s(16.0) };
        let l = self.fonts.fit(Face::Regular, size, label, label_max, Cut::End);
        let lw = self.fonts.width(Face::Regular, size, &l);
        let ly = if stacked { self.y } else { self.y + (ch - lh) / 2.0 };
        self.push(
            None,
            Rect::new(self.x0, ly, lw, lh),
            Kind::Label,
            vec![text(Rect::new(self.x0, ly, lw, lh), l, Face::Regular, size, theme::DIM)],
        );
        let (cx, cy) = if stacked { (self.x0, self.y + lh + self.m.s(6.0)) } else { (self.x0 + self.label_w, self.y) };
        let bottom = self.flow(cx, cy, ctrls, Align::Left);
        self.y = bottom + self.m.s(12.0);
    }

    /// A line of buttons (wrapping onto more lines when needed).
    pub fn buttons(&mut self, align: Align, ctrls: Vec<Ctrl>) {
        let y = self.y;
        self.y = self.flow(self.x0, y, ctrls, align);
    }

    /// Opens the page's content panel (a card the footer closes): every
    /// text of a page then sits on a panel, not on the backdrop.
    pub fn content_begin(&mut self) {
        self.content = Some(self.card_begin());
    }

    fn content_end(&mut self) {
        if let Some(i) = self.content.take() {
            self.card_end(i);
        }
    }

    /// Buttons at the window's bottom when the content leaves room, else
    /// after it.
    pub fn footer(&mut self, ctrls: Vec<Ctrl>) {
        self.content_end();
        let margin = self.m.s(24.0);
        let probe_h = self.flow_height(ctrls.clone());
        let at_bottom = self.m.h - margin - probe_h;
        self.slack = (at_bottom - self.y - self.m.s(16.0)).max(0.0);
        let y = at_bottom.max(self.y + self.m.s(16.0));
        self.y = self.flow(self.x0, y, ctrls, Align::Right);
    }

    /// A footer with `left` at the column's left edge and `right` at its
    /// right edge (the main button last), on one line when they fit; else
    /// as [`Builder::footer`] with all of them.
    pub fn footer_split(&mut self, left: Vec<Ctrl>, right: Vec<Ctrl>) {
        self.content_end();
        let gap = self.m.s(12.0);
        let wide = |b: &Self, v: &[Ctrl]| {
            v.iter().map(|c| b.natural_w(c)).sum::<f32>() + gap * v.len().saturating_sub(1) as f32
        };
        let (lw, rw) = (wide(self, &left), wide(self, &right));
        if left.is_empty() || lw + self.m.s(32.0) + rw > self.width() {
            return self.footer(left.into_iter().chain(right).collect());
        }
        let margin = self.m.s(24.0);
        let ch = self.ctrl_h();
        self.slack = (self.m.h - margin - ch - self.y - self.m.s(16.0)).max(0.0);
        let y = (self.m.h - margin - ch).max(self.y + self.m.s(16.0));
        let bl = self.flow(self.x0, y, left, Align::Left);
        let br = self.flow(self.x1 - rw, y, right, Align::Right);
        self.y = bl.max(br);
    }

    fn natural_w(&self, c: &Ctrl) -> f32 {
        let size = self.m.body();
        let f = self.fonts;
        let ch = self.ctrl_h();
        match c {
            Ctrl::Cycle { values, value, .. } => {
                let w = values.iter().chain([value]).map(|v| f.width(Face::Regular, size, v)).fold(0.0, f32::max);
                w + 2.0 * ch + self.m.s(8.0)
            }
            Ctrl::Button { text, primary, .. } => {
                let face = if *primary { Face::Bold } else { Face::Regular };
                // The main button a little wider: the one thing to press.
                let min = if *primary { 150.0 } else { 110.0 };
                (f.width(face, size, text) + self.m.s(40.0)).max(self.m.s(min))
            }
            Ctrl::Text { text, .. } => f.width(Face::Regular, self.m.small(), text),
            Ctrl::Path { text, .. } => f.width(Face::Regular, size, text),
            Ctrl::Field { .. } => self.m.s(240.0),
        }
    }

    fn flow_height(&mut self, ctrls: Vec<Ctrl>) -> f32 {
        let (n, y0) = (self.widgets.len(), self.y);
        let fd = self.default_focus;
        let b = self.flow(self.x0, 0.0, ctrls, Align::Right);
        self.widgets.truncate(n);
        self.y = y0;
        self.default_focus = fd;
        b
    }

    /// Places `ctrls` from (`x`, `y`) to the column's right edge, a new
    /// line when one does not fit; returns the bottom.
    fn flow(&mut self, x: f32, y: f32, ctrls: Vec<Ctrl>, align: Align) -> f32 {
        let gap = self.m.s(12.0);
        let ch = self.ctrl_h();
        let avail = self.x1 - x;
        // Split into lines by natural width.
        let mut lines: Vec<Vec<(Ctrl, f32)>> = vec![Vec::new()];
        let mut used = 0.0;
        let widths: Vec<f32> = ctrls.iter().map(|c| self.natural_w(c)).collect();
        for (i, c) in ctrls.into_iter().enumerate() {
            let mut w = widths[i].min(avail);
            if matches!(c, Ctrl::Path { .. }) {
                // Room for what follows (when that fits beside a shortened
                // path of at least 160 units).
                let rest: f32 = widths[i + 1..].iter().map(|w| w + gap).sum();
                if avail - used - rest >= self.m.s(160.0) {
                    w = w.min(avail - used - rest);
                }
            }
            let flexible = matches!(c, Ctrl::Text { .. } | Ctrl::Field { .. } | Ctrl::Path { .. });
            let need = if lines.last().unwrap().is_empty() { 0.0 } else { gap };
            let fits = used + need + w <= avail || (flexible && used + need + self.m.s(160.0) <= avail);
            if !lines.last().unwrap().is_empty() && !fits {
                lines.push(Vec::new());
                used = 0.0;
            }
            let need = if lines.last().unwrap().is_empty() { 0.0 } else { gap };
            used += need + w;
            lines.last_mut().unwrap().push((c, w));
        }
        let mut cy = y;
        for line in lines {
            let total: f32 = line.iter().map(|(_, w)| *w).sum::<f32>() + gap * line.len().saturating_sub(1) as f32;
            let mut cx = if align == Align::Right { (self.x1 - total).max(x) } else { x };
            let mut line_h = ch;
            let count = line.len();
            for (i, (c, w)) in line.into_iter().enumerate() {
                // The last flexible control takes the rest of the line.
                let rest = self.x1 - cx;
                let w = if i + 1 == count && matches!(c, Ctrl::Text { .. } | Ctrl::Field { .. }) && align == Align::Left
                {
                    rest
                } else {
                    w.min(rest)
                };
                let h = self.place(c, Rect::new(cx, cy, w, ch));
                line_h = line_h.max(h);
                cx += w + gap;
            }
            cy += line_h + gap;
        }
        cy - gap
    }

    /// One control in `r` (its height may grow for wrapped text); returns
    /// its height.
    fn place(&mut self, c: Ctrl, r: Rect) -> f32 {
        let size = self.m.body();
        let lh = self.lh(size);
        let f = self.fonts;
        let ty = r.y + ((r.h - lh) / 2.0).round();
        match c {
            Ctrl::Cycle { id, value, enabled, .. } => {
                let a = r.h * 0.5;
                let left = Rect::new(r.x + (r.h - a) / 2.0, r.y + (r.h - a) / 2.0, a, a);
                let right = Rect::new(r.right() - (r.h + a) / 2.0, left.y, a, a);
                let inner = r.w - 2.0 * r.h;
                let v = f.fit(Face::Regular, size, &value, inner, Cut::End);
                let vw = f.width(Face::Regular, size, &v);
                let t = super::ui::text_of(Rect::new(r.x + (r.w - vw) / 2.0, ty, vw, lh), v, &value, Face::Regular, size, theme::TEXT);
                self.widgets.push(Widget {
                    id: Some(id),
                    rect: r,
                    kind: Kind::Cycle { left, right },
                    texts: vec![t],
                    enabled,
                });
                r.h
            }
            Ctrl::Button { id, text: s, primary, enabled } => {
                let face = if primary { Face::Bold } else { Face::Regular };
                let v = f.fit(face, size, &s, r.w - self.m.s(20.0), Cut::End);
                let vw = f.width(face, size, &v);
                let col = if primary { theme::ON_ACCENT } else { theme::TEXT };
                let t = super::ui::text_of(Rect::new(r.x + ((r.w - vw) / 2.0).round(), ty, vw, lh), v, &s, face, size, col);
                self.widgets.push(Widget {
                    id: Some(id),
                    rect: r,
                    kind: Kind::Button { primary, big: false },
                    texts: vec![t],
                    enabled,
                });
                r.h
            }
            Ctrl::Text { text: s, color } => {
                let size = self.m.small();
                let slh = self.lh(size);
                let (texts, h) = self.lines(r.x, 0.0, r.w, &s, Face::Regular, size, color);
                // One line: centred on the controls' line; more: from its top.
                let y0 = if texts.len() == 1 { r.y + ((r.h - slh) / 2.0).round() } else { r.y + self.m.s(4.0) };
                let texts: Vec<TextBox> = texts
                    .into_iter()
                    .map(|mut t| {
                        t.rect.y += y0;
                        t
                    })
                    .collect();
                self.push(None, Rect::new(r.x, y0, r.w, h), Kind::Label, texts);
                (y0 - r.y + h).max(r.h)
            }
            Ctrl::Path { text: s, color } => {
                let v = f.fit(Face::Regular, size, &s, r.w, Cut::Middle);
                let vw = f.width(Face::Regular, size, &v);
                let t = super::ui::text_of(Rect::new(r.x, ty, vw, lh), v, &s, Face::Regular, size, color);
                self.push(None, Rect::new(r.x, ty, r.w, lh), Kind::Label, vec![t]);
                r.h
            }
            Ctrl::Field { id, text: s, placeholder } => {
                let pad = self.m.s(12.0);
                let inner = r.w - 2.0 * pad;
                let chars: Vec<char> = s.chars().collect();
                let empty = chars.is_empty();
                let (shown, from, col) = if empty {
                    (f.fit(Face::Regular, size, &placeholder, inner, Cut::End), 0, theme::PLACEHOLDER)
                } else if self.st.focus != Some(id) {
                    // Not being typed in: both ends of a long path.
                    (f.fit(Face::Regular, size, s.as_str(), inner - 2.0, Cut::Middle), 0, theme::TEXT)
                } else {
                    // The end of a long text (the caret's side when typing).
                    let mut from = 0;
                    while from < chars.len()
                        && f.width(Face::Regular, size, &chars[from..].iter().collect::<String>()) > inner - 2.0
                    {
                        from += 1;
                    }
                    (chars[from..].iter().collect(), from, theme::TEXT)
                };
                let sw = f.width(Face::Regular, size, &shown);
                // The caret's places for a click: exact unless the text
                // is drawn shortened in its middle.
                let middle_cut = !empty && from == 0 && shown != s;
                let stops: Vec<f32> = if middle_cut {
                    Vec::new()
                } else {
                    let sc: Vec<char> = if empty { Vec::new() } else { shown.chars().collect() };
                    (0..=sc.len())
                        .map(|i| r.x + pad + f.width(Face::Regular, size, &sc[..i].iter().collect::<String>()))
                        .collect()
                };
                let t = if empty {
                    text(Rect::new(r.x + pad, ty, sw, lh), shown, Face::Regular, size, col)
                } else {
                    super::ui::text_of(Rect::new(r.x + pad, ty, sw, lh), shown, &s, Face::Regular, size, col)
                };
                self.widgets.push(Widget {
                    id: Some(id),
                    rect: r,
                    kind: Kind::TextField { cursor: chars.len(), shown_from: from, empty, stops },
                    texts: vec![t],
                    enabled: true,
                });
                r.h
            }
        }
    }

    /// A label in the label column and a plain value beside it (no
    /// control; wrapped), or the label above it when narrow. Compact: a
    /// text line's height, not a control's.
    pub fn info_row(&mut self, label: &str, value: &str, color: Rgb) {
        let size = self.m.body();
        let lh = self.lh(size);
        let stacked = self.width() - self.label_w < self.m.s(330.0);
        let label_max = if stacked { self.width() } else { self.label_w - self.m.s(16.0) };
        let l = self.fonts.fit(Face::Regular, size, label, label_max, Cut::End);
        let lw = self.fonts.width(Face::Regular, size, &l);
        self.push(
            None,
            Rect::new(self.x0, self.y, lw, lh),
            Kind::Label,
            vec![text(Rect::new(self.x0, self.y, lw, lh), l, Face::Regular, size, theme::DIM)],
        );
        let (vx, vy) = if stacked { (self.x0, self.y + lh + self.m.s(4.0)) } else { (self.x0 + self.label_w, self.y) };
        let (texts, h) = self.lines(vx, vy, self.x1 - vx, value, Face::Regular, size, color);
        self.push(None, Rect::new(vx, vy, self.x1 - vx, h), Kind::Label, texts);
        self.y = vy + h + self.m.s(10.0);
    }

    /// A framed box of lines (a log's tail), each shortened at its end.
    pub fn panel(&mut self, lines: &[String]) {
        let size = self.m.small();
        let lh = self.lh(size);
        let step = (lh * 1.2).round();
        let pad = self.m.s(12.0);
        let inner = self.width() - 2.0 * pad;
        let h = 2.0 * pad + step * lines.len().saturating_sub(1) as f32 + lh;
        let texts = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let t = self.fonts.fit(Face::Regular, size, l, inner, Cut::End);
                let w = self.fonts.width(Face::Regular, size, &t);
                text(
                    Rect::new(self.x0 + pad, self.y + pad + i as f32 * step, w, lh),
                    t,
                    Face::Regular,
                    size,
                    theme::TEXT,
                )
            })
            .collect();
        self.push(None, Rect::new(self.x0, self.y, self.width(), h), Kind::Panel, texts);
        self.y += h;
    }

    /// A box with a label after it.
    pub fn checkbox(&mut self, id: Id, label: &str, checked: bool) {
        let size = self.m.body();
        let lh = self.lh(size);
        let bs = (lh * 1.05).round();
        let x = self.x0 + self.m.s(4.0);
        let tx = x + bs + self.m.s(12.0);
        let (texts, h) = self.lines(
            tx,
            self.y + self.m.s(6.0),
            self.x1 - tx - self.m.s(4.0),
            label,
            Face::Regular,
            size,
            theme::TEXT,
        );
        let total = (h + self.m.s(12.0)).max(bs + self.m.s(12.0));
        let check = Rect::new(x, self.y + self.m.s(6.0) + ((lh - bs) / 2.0).round(), bs, bs);
        let w = texts.iter().map(|t| t.rect.right()).fold(tx, f32::max) - self.x0 + self.m.s(8.0);
        self.widgets.push(Widget {
            id: Some(id),
            rect: Rect::new(self.x0, self.y, w.min(self.width()), total),
            kind: Kind::Checkbox { checked, check },
            texts,
            enabled: true,
        });
        self.y += total;
    }

    /// Short checkboxes side by side on one line when they fit (each
    /// label on one line), else one under the other.
    pub fn checkboxes(&mut self, items: &[(Id, &str, bool)]) {
        let size = self.m.body();
        let lh = self.lh(size);
        let bs = (lh * 1.05).round();
        let gap = self.m.s(28.0);
        let widths: Vec<f32> = items
            .iter()
            .map(|(_, l, _)| {
                self.m.s(4.0) + bs + self.m.s(12.0) + self.fonts.width(Face::Regular, size, l) + self.m.s(8.0)
            })
            .collect();
        let total = widths.iter().sum::<f32>() + gap * (items.len().saturating_sub(1)) as f32;
        if total > self.width() {
            for &(id, label, checked) in items {
                self.checkbox(id, label, checked);
            }
            return;
        }
        let (x0, y0) = (self.x0, self.y);
        let mut bottom = y0;
        let mut x = x0;
        for (&(id, label, checked), w) in items.iter().zip(&widths) {
            self.x0 = x;
            self.y = y0;
            self.checkbox(id, label, checked);
            bottom = bottom.max(self.y);
            x += w + gap;
        }
        self.x0 = x0;
        self.y = bottom;
    }

    /// A list that takes the room the window leaves ([`Builder::grow`]
    /// rows more than `rows`).
    pub fn list_grow(&mut self, id: Id, items: &[(String, String)], selected: usize, rows: usize) {
        self.list(id, items, selected, rows + self.grow);
    }

    /// A list of `items` (text and a dim note at the right), `rows` high;
    /// a scroll bar at its right edge when it has more.
    pub fn list(&mut self, id: Id, items: &[(String, String)], selected: usize, rows: usize) {
        let size = self.m.body();
        let lh = self.lh(size);
        let row_h = (lh + self.m.s(12.0)).round();
        let pad = self.m.s(4.0);
        let rows = rows.max(1);
        let len = items.len();
        let sel = selected.min(len.saturating_sub(1));
        let mut first = self.st.first(id);
        // Brought into view only when the selection moved: the wheel and
        // the scroll bar move the rows, not the selection.
        if self.st.last_sel(id) != Some(sel) {
            if sel < first {
                first = sel;
            }
            if sel >= first + rows {
                first = sel + 1 - rows;
            }
            self.st.set_last_sel(id, sel);
        }
        first = first.min(len.saturating_sub(rows));
        self.st.set_first(id, first);
        let r = Rect::new(self.x0, self.y, self.width(), rows as f32 * row_h + 2.0 * pad);
        let bar = if len > rows { super::ui::list_bar_w(self.m.u) + self.m.s(4.0) } else { 0.0 };
        let inner_x = r.x + pad + self.m.s(12.0);
        let inner_w = r.w - 2.0 * pad - self.m.s(24.0) - bar;
        let mut texts = Vec::new();
        for (i, (main, note)) in items.iter().enumerate().skip(first).take(rows) {
            let ty = r.y + pad + (i - first) as f32 * row_h + ((row_h - lh) / 2.0).round();
            let ns = self.m.small();
            let nlh = self.lh(ns);
            let nw = if note.is_empty() { 0.0 } else { self.fonts.width(Face::Regular, ns, note).min(inner_w * 0.35) };
            let note_fit = self.fonts.fit(Face::Regular, ns, note, nw, Cut::End);
            let main_max = inner_w - if nw > 0.0 { nw + self.m.s(16.0) } else { 0.0 };
            let m = self.fonts.fit(Face::Regular, size, main, main_max, Cut::Middle);
            let mw = self.fonts.width(Face::Regular, size, &m);
            texts.push(super::ui::text_of(Rect::new(inner_x, ty, mw, lh), m, main, Face::Regular, size, theme::TEXT));
            if nw > 0.0 {
                let w = self.fonts.width(Face::Regular, ns, &note_fit);
                let ny = ty + lh - nlh;
                texts.push(super::ui::text_of(Rect::new(inner_x + inner_w - w, ny, w, nlh), note_fit, note, Face::Regular, ns, theme::DIM));
            }
        }
        self.widgets.push(Widget {
            id: Some(id),
            rect: r,
            kind: Kind::List { len, selected: sel, first, row_h, pad },
            texts,
            enabled: len > 0,
        });
        self.y += r.h;
    }

    /// A row of flat links (and dim separators), wrapped onto more lines
    /// when it does not fit; each link shortened in its middle to at most
    /// half the column.
    pub fn links(&mut self, items: Vec<Link>) {
        let size = self.m.body();
        let lh = self.lh(size);
        let px = self.m.s(8.0);
        let h = lh + self.m.s(10.0);
        let gap = self.m.s(2.0);
        let (mut x, mut y) = (self.x0, self.y);
        let f = self.fonts;
        for it in items {
            let (face, text, color, whole) = match &it {
                Link::Go { text, strong, whole, .. } => {
                    (if *strong { Face::Bold } else { Face::Regular }, text.clone(), theme::TEXT, whole.clone())
                }
                Link::Sep(t) => (Face::Regular, t.clone(), theme::DIM, None),
            };
            let pad = if matches!(it, Link::Go { .. }) { px } else { 0.0 };
            let max = (self.width() * 0.5 - 2.0 * pad).max(self.m.s(40.0));
            let v = f.fit(face, size, &text, max, Cut::Middle);
            let w = f.width(face, size, &v) + 2.0 * pad;
            if x > self.x0 && x + w > self.x1 {
                x = self.x0;
                y += h + gap;
            }
            let mut t = super::ui::text_of(
                Rect::new(x + pad, y + ((h - lh) / 2.0).round(), w - 2.0 * pad, lh),
                v,
                &text,
                face,
                size,
                color,
            );
            // The whole value always in the trace when given (the folder
            // shown, even when its name is the whole path: `C:\`, `/`).
            if whole.is_some() {
                t.full = whole;
            }
            match it {
                Link::Go { id, .. } => self.widgets.push(Widget {
                    id: Some(id),
                    rect: Rect::new(x, y, w, h),
                    kind: Kind::Link,
                    texts: vec![t],
                    enabled: true,
                }),
                Link::Sep(_) => self.push(None, Rect::new(x, y, w, h), Kind::Label, vec![t]),
            }
            x += w + gap;
        }
        self.y = y + h;
    }

    pub fn finish(mut self) -> Ui {
        self.content_end();
        let bottom = self.widgets.iter().map(|w| w.rect.bottom()).fold(0.0, f32::max);
        Ui {
            widgets: self.widgets,
            view: Rect::new(0.0, 0.0, self.m.w, self.m.h),
            content_h: (bottom + self.m.s(24.0)).max(self.m.h).min(f32::MAX),
            default_focus: self.default_focus,
            metrics: Some(self.m),
            slack: self.slack,
        }
    }
}

/// Builds a screen at the largest unit up to the size's that fits the
/// window's height (down to the minimum; past it the page scrolls).
/// `shrink` false: a long page (a document) keeps its size and scrolls.
pub fn build_fit(
    fonts: &Fonts,
    w: u32,
    h: u32,
    dpi: f32,
    shrink: bool,
    st: &mut UiState,
    f: impl Fn(&mut Builder),
) -> Ui {
    let m0 = Metrics::for_size(w, h, dpi);
    let floor = if shrink { super::ui::MIN_UNIT * dpi } else { m0.u };
    let mut u = m0.u;
    loop {
        let mut b = Builder::new(fonts, m0.with_unit(u), st);
        f(&mut b);
        let mut ui = b.finish();
        let over = ui.content_h > m0.h + 0.5;
        if !over || u <= floor + 1e-3 {
            if !over {
                ui.content_h = m0.h;
            }
            if st.mouse_scrolled {
                st.scroll = st.scroll.clamp(0.0, (ui.content_h - ui.view.h).max(0.0));
            } else {
                ui.keep_in_view(st);
            }
            return ui;
        }
        u = (u * (m0.h / ui.content_h).max(0.5).min(0.97)).max(floor);
    }
}
