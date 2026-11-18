//! Small in-game panels for the modding tools (the texture editor, the map
//! editor): a panel with a title, text lines, a row of buttons, a scrolling
//! list and a one-line text field, drawn with the game's own UI quads
//! ([`Quad`], the front end's fonts). Immediate mode: the tool builds its
//! panel every frame from its own state and the frame's [`Input`]; the
//! result is the quads and whether the panel took the click.
//!
//! Coordinates are canvas points (640 × 480) placed at the left edge
//! ([`PLACE`]), so the panel stays at the left on every aspect ratio. The
//! caller converts the cursor with the placement it gets from [`PLACE`]
//! (`frontend::mouse::canvas_point`).
//!
//! Nothing here reads or writes game state; a tool that is off builds no
//! panel and draws nothing.

use crate::frontend::font::Font;
use crate::frontend::{Assets, Place, Quad, text_quads};
use glam::Vec2;

/// Where the panels are placed: the canvas's left edge, top.
pub const PLACE: Place = Place::new(-1, -1);

/// Colours (straight RGBA).
pub mod colour {
    pub const BACKING: [f32; 4] = [0.02, 0.02, 0.03, 0.82];
    pub const TITLE: [f32; 4] = [1.0, 0.78, 0.25, 1.0];
    pub const TEXT: [f32; 4] = [0.92, 0.92, 0.92, 1.0];
    pub const DIM: [f32; 4] = [0.6, 0.6, 0.62, 1.0];
    pub const BUTTON: [f32; 4] = [0.18, 0.2, 0.26, 0.95];
    pub const HOVER: [f32; 4] = [0.28, 0.32, 0.42, 0.95];
    pub const SELECTED: [f32; 4] = [0.55, 0.38, 0.08, 0.9];
    pub const FIELD: [f32; 4] = [0.08, 0.08, 0.1, 0.95];
    pub const FOCUS: [f32; 4] = [0.95, 0.7, 0.2, 1.0];
    pub const WARN: [f32; 4] = [1.0, 0.4, 0.35, 1.0];
    pub const OK: [f32; 4] = [0.5, 0.9, 0.5, 1.0];
}

/// The keys a panel uses (the game maps its key codes onto these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Escape,
    Backspace,
    Tab,
}

/// One frame's input as the panel sees it.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// The cursor as a canvas point placed by [`PLACE`] (None: no cursor).
    pub cursor: Option<Vec2>,
    /// The clicking button went down this frame.
    pub clicked: bool,
    /// Wheel lines this frame (> 0: away from the player, up the list).
    pub wheel: f32,
    /// Typed characters this frame (control characters left out).
    pub text: String,
    /// Keys pressed this frame, in order.
    pub keys: Vec<NavKey>,
}

impl Input {
    fn over(&self, min: Vec2, max: Vec2) -> bool {
        self.cursor.is_some_and(|c| c.x >= min.x && c.x < max.x && c.y >= min.y && c.y < max.y)
    }
    fn click(&self, min: Vec2, max: Vec2) -> bool {
        self.clicked && self.over(min, max)
    }
}

/// A list's scroll position and selection, kept by the tool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListState {
    pub selected: Option<usize>,
    /// The first row shown.
    pub top: usize,
}

impl ListState {
    /// Keep the selection within `len` items and in view of `rows` rows.
    pub fn clamp(&mut self, len: usize, rows: usize) {
        if len == 0 {
            self.selected = None;
            self.top = 0;
            return;
        }
        if let Some(s) = self.selected {
            self.selected = Some(s.min(len - 1));
        }
        let rows = rows.max(1);
        if let Some(s) = self.selected {
            if s < self.top {
                self.top = s;
            } else if s >= self.top + rows {
                self.top = s + 1 - rows;
            }
        }
        self.top = self.top.min(len.saturating_sub(rows));
    }

    /// Move the selection by `delta` rows (clamped; none yet: the first).
    pub fn step(&mut self, delta: isize, len: usize, rows: usize) {
        if len == 0 {
            return;
        }
        let s = match self.selected {
            None => 0,
            Some(s) => (s as isize + delta).clamp(0, len as isize - 1) as usize,
        };
        self.selected = Some(s);
        self.clamp(len, rows);
    }
}

/// A one-line text field, kept by the tool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextField {
    pub text: String,
    pub focused: bool,
    /// The longest text it takes, in characters (0: 64).
    pub max: usize,
}

impl TextField {
    pub fn new(text: &str) -> TextField {
        TextField { text: text.to_string(), focused: false, max: 0 }
    }
}

/// What happened to a list this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListEvent {
    /// The selection changed (by a click or a key).
    pub changed: bool,
    /// A row was clicked a second time, or Enter was pressed on it.
    pub activated: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Prim {
    Rect { min: Vec2, max: Vec2, color: [f32; 4] },
    Text { at: Vec2, text: String, color: [f32; 4] },
}

/// Sizes: the text's scale (× the font's pixels), the padding, the gap
/// between items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub text_scale: f32,
    pub pad: f32,
    pub gap: f32,
}

impl Default for Style {
    fn default() -> Style {
        Style { text_scale: 0.3, pad: 4.0, gap: 2.0 }
    }
}

/// One panel being built this frame: a column at `origin`, `width` wide.
pub struct Panel<'a> {
    font: &'a Font,
    input: &'a Input,
    style: Style,
    origin: Vec2,
    width: f32,
    /// The next item's top.
    y: f32,
    prims: Vec<Prim>,
    /// The cursor or a key was used by an item (the game should not
    /// act on it).
    took: bool,
    /// A text field has the keyboard.
    typing: bool,
}

/// A built panel: its quads' primitives and what it took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    prims: Vec<Prim>,
    /// The panel's rectangle (canvas, [`PLACE`]).
    pub rect: [Vec2; 2],
    /// The click landed on the panel (consumed: the game ignores it).
    pub took_click: bool,
    /// A text field has the keyboard: the game takes no key.
    pub typing: bool,
    style_scale: f32,
}

impl<'a> Panel<'a> {
    pub fn new(font: &'a Font, input: &'a Input, origin: Vec2, width: f32) -> Panel<'a> {
        Panel::with_style(font, input, origin, width, Style::default())
    }

    pub fn with_style(font: &'a Font, input: &'a Input, origin: Vec2, width: f32, style: Style) -> Panel<'a> {
        // The backing first, its size set at the end.
        let prims = vec![Prim::Rect { min: origin, max: origin, color: colour::BACKING }];
        Panel { font, input, style, origin, width, y: origin.y + style.pad, prims, took: false, typing: false }
    }

    /// One line's height.
    pub fn line(&self) -> f32 {
        self.font.line_height * self.style.text_scale
    }

    fn measure(&self, text: &str) -> f32 {
        self.font.measure(text) * self.style.text_scale
    }

    fn left(&self) -> f32 {
        self.origin.x + self.style.pad
    }

    fn inner(&self) -> f32 {
        self.width - 2.0 * self.style.pad
    }

    /// `text` cut with "…" to fit `width` (canvas points).
    pub fn fit(&self, text: &str, width: f32) -> String {
        if self.measure(text) <= width {
            return text.to_string();
        }
        let mut s: String = text.to_string();
        while !s.is_empty() && self.measure(&format!("{s}...")) > width {
            s.pop();
        }
        format!("{s}...")
    }

    fn text_at(&mut self, at: Vec2, text: &str, color: [f32; 4]) {
        if !text.is_empty() {
            self.prims.push(Prim::Text { at, text: text.to_string(), color });
        }
    }

    /// The title line.
    pub fn title(&mut self, text: &str) {
        let t = self.fit(text, self.inner());
        self.text_at(Vec2::new(self.left(), self.y), &t, colour::TITLE);
        self.y += self.line() + self.style.gap;
    }

    /// A text line (cut to the panel's width).
    pub fn label(&mut self, text: &str, color: [f32; 4]) {
        let t = self.fit(text, self.inner());
        self.text_at(Vec2::new(self.left(), self.y), &t, color);
        self.y += self.line();
    }

    /// Empty space.
    pub fn space(&mut self, h: f32) {
        self.y += h;
    }

    /// A row of buttons as wide as the panel, split evenly; the index of the
    /// one clicked.
    pub fn buttons(&mut self, labels: &[&str]) -> Option<usize> {
        if labels.is_empty() {
            return None;
        }
        let gap = self.style.gap;
        let w = (self.inner() - gap * (labels.len() - 1) as f32) / labels.len() as f32;
        let h = self.line() + 2.0 * gap;
        let mut hit = None;
        for (i, label) in labels.iter().enumerate() {
            let min = Vec2::new(self.left() + i as f32 * (w + gap), self.y);
            let max = min + Vec2::new(w, h);
            let over = self.input.over(min, max);
            self.prims.push(Prim::Rect { min, max, color: if over { colour::HOVER } else { colour::BUTTON } });
            let t = self.fit(label, w - 2.0 * gap);
            let tw = self.measure(&t);
            self.text_at(Vec2::new(min.x + (w - tw) * 0.5, min.y + gap), &t, colour::TEXT);
            if self.input.click(min, max) {
                hit = Some(i);
                self.took = true;
            }
        }
        self.y += h + gap;
        hit
    }

    /// A list of `items`, `rows` rows tall: click selects (a second click
    /// on the selected row activates it), the wheel scrolls, Up / Down /
    /// Page Up / Page Down / Home / End move the selection and Enter
    /// activates it (when no text field has the keyboard).
    pub fn list(&mut self, state: &mut ListState, items: &[String], rows: usize) -> ListEvent {
        let rows = rows.max(1);
        let mut ev = ListEvent::default();
        let before = state.selected;
        if !self.typing {
            for k in &self.input.keys {
                match k {
                    NavKey::Up => state.step(-1, items.len(), rows),
                    NavKey::Down => state.step(1, items.len(), rows),
                    NavKey::PageUp => state.step(-(rows as isize), items.len(), rows),
                    NavKey::PageDown => state.step(rows as isize, items.len(), rows),
                    NavKey::Home => state.step(-(items.len() as isize), items.len(), rows),
                    NavKey::End => state.step(items.len() as isize, items.len(), rows),
                    NavKey::Enter if state.selected.is_some() => ev.activated = true,
                    _ => continue,
                }
                self.took = true;
            }
        }
        let line = self.line();
        let min = Vec2::new(self.left(), self.y);
        let max = min + Vec2::new(self.inner(), line * rows as f32);
        if self.input.over(min, max) && self.input.wheel != 0.0 {
            let lines = (self.input.wheel.abs().ceil() as usize).max(1) * 3;
            state.top = if self.input.wheel > 0.0 { state.top.saturating_sub(lines) } else { state.top + lines };
            self.took = true;
        }
        state.top = state.top.min(items.len().saturating_sub(rows));
        if let Some(s) = state.selected
            && s >= items.len()
        {
            state.selected = items.len().checked_sub(1);
        }
        self.prims.push(Prim::Rect { min, max, color: colour::FIELD });
        for row in 0..rows {
            let i = state.top + row;
            let Some(item) = items.get(i) else { break };
            let rmin = Vec2::new(min.x, min.y + row as f32 * line);
            let rmax = Vec2::new(max.x, rmin.y + line);
            if state.selected == Some(i) {
                self.prims.push(Prim::Rect { min: rmin, max: rmax, color: colour::SELECTED });
            } else if self.input.over(rmin, rmax) {
                self.prims.push(Prim::Rect { min: rmin, max: rmax, color: colour::HOVER });
            }
            let t = self.fit(item, self.inner() - 2.0 * self.style.gap);
            self.text_at(Vec2::new(rmin.x + self.style.gap, rmin.y), &t, colour::TEXT);
            if self.input.click(rmin, rmax) {
                if state.selected == Some(i) {
                    ev.activated = true;
                }
                state.selected = Some(i);
                self.took = true;
            }
        }
        // A scroll bar when the list is longer than its rows.
        if items.len() > rows {
            let h = max.y - min.y;
            let bar = (h * rows as f32 / items.len() as f32).max(4.0);
            let at = min.y + (h - bar) * state.top as f32 / (items.len() - rows) as f32;
            self.prims.push(Prim::Rect {
                min: Vec2::new(max.x - 2.0, at),
                max: Vec2::new(max.x, at + bar),
                color: colour::DIM,
            });
        }
        ev.changed = state.selected != before;
        self.y = max.y + self.style.gap;
        ev
    }

    /// A one-line text field with `label` before it. A click on it gives it
    /// the keyboard, a click elsewhere takes it away; typed characters are
    /// added, Backspace removes the last, Escape leaves it. True when Enter
    /// was pressed in it (the field keeps the keyboard until a click).
    pub fn text_field(&mut self, label: &str, field: &mut TextField) -> bool {
        let gap = self.style.gap;
        let lw = if label.is_empty() { 0.0 } else { self.measure(label) + 2.0 * gap };
        self.text_at(Vec2::new(self.left(), self.y + gap), label, colour::DIM);
        let min = Vec2::new(self.left() + lw, self.y);
        let max = Vec2::new(self.left() + self.inner(), self.y + self.line() + 2.0 * gap);
        if self.input.clicked && self.input.cursor.is_some() {
            let inside = self.input.over(min, max);
            if inside {
                self.took = true;
            }
            field.focused = inside;
        }
        let mut submitted = false;
        if field.focused {
            let max_len = if field.max == 0 { 64 } else { field.max };
            for c in self.input.text.chars().filter(|c| !c.is_control()) {
                if field.text.chars().count() < max_len {
                    field.text.push(c);
                }
            }
            for k in &self.input.keys {
                match k {
                    NavKey::Backspace => {
                        field.text.pop();
                    }
                    NavKey::Enter => submitted = true,
                    NavKey::Escape => field.focused = false,
                    _ => {}
                }
            }
            self.took = true;
        }
        self.typing |= field.focused;
        self.prims.push(Prim::Rect { min, max, color: colour::FIELD });
        if field.focused {
            // A frame line along the bottom.
            self.prims.push(Prim::Rect { min: Vec2::new(min.x, max.y - 1.0), max, color: colour::FOCUS });
        }
        let shown = if field.focused { format!("{}_", field.text) } else { field.text.clone() };
        // The end of a long text stays in view.
        let room = max.x - min.x - 2.0 * gap;
        let mut s = shown.as_str();
        while !s.is_empty() && self.measure(s) > room {
            let mut it = s.chars();
            it.next();
            s = it.as_str();
        }
        let s = s.to_string();
        self.text_at(Vec2::new(min.x + gap, min.y + gap), &s, colour::TEXT);
        self.y = max.y + gap;
        submitted
    }

    /// The panel done: its backing sized to what was added.
    pub fn finish(mut self) -> Built {
        let max = Vec2::new(self.origin.x + self.width, self.y + self.style.pad);
        if let Some(Prim::Rect { max: m, .. }) = self.prims.first_mut() {
            *m = max;
        }
        let took_click = self.took || (self.input.clicked && self.input.over(self.origin, max));
        Built {
            prims: self.prims,
            rect: [self.origin, max],
            took_click,
            typing: self.typing,
            style_scale: self.style.text_scale,
        }
    }
}

impl Built {
    /// The panel's quads, in front of everything (z −1), placed by [`PLACE`].
    pub fn quads(&self, assets: &Assets, font: &Font, out: &mut Vec<Quad>) {
        let start = out.len();
        for p in &self.prims {
            match p {
                Prim::Rect { min, max, color } => out.push(Quad {
                    texture: None,
                    rect: [*min, *max],
                    uv: [Vec2::ZERO, Vec2::ONE],
                    color: *color,
                    z: -1.0,
                    angle: 0.0,
                    place: PLACE,
                }),
                Prim::Text { at, text, color } => {
                    text_quads(assets, font, text, 0, Vec2::splat(self.style_scale), *at, *color, -1.0, out);
                }
            }
        }
        for q in &mut out[start..] {
            q.place = PLACE;
        }
    }

    /// The texts drawn, in order (tests and the developer harness).
    pub fn texts(&self) -> Vec<&str> {
        self.prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Whether canvas point `p` lies on the panel.
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.rect[0].x && p.x < self.rect[1].x && p.y >= self.rect[0].y && p.y < self.rect[1].y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::font::Glyph;

    /// A font of printable ASCII, every glyph 10 wide, lines 40 high (so 3 × 12
    /// canvas points at the default scale).
    fn font() -> Font {
        let glyph = Glyph {
            width: 10.0,
            height: 30.0,
            x: 0.0,
            y: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
            advance: 10.0,
            kerning: Vec::new(),
        };
        let glyphs = (32u8..127).map(|c| (u32::from(c), glyph.clone())).collect();
        Font { name: "t".into(), texture: "t".into(), line_height: 40.0, texture_size: [64.0, 64.0], glyphs }
    }

    fn items(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("item {i}")).collect()
    }

    #[test]
    fn a_button_click_is_reported_and_taken() {
        let f = font();
        // Panel at (10, 10), 200 wide: the title row is 12 high + gap 2, so
        // the button row starts at y = 14 + 14 = 28 and is 16 high; two
        // buttons each (192 − 2) / 2 = 95 wide from x = 14.
        let input = Input { cursor: Some(Vec2::new(120.0, 35.0)), clicked: true, ..Default::default() };
        let mut p = Panel::new(&f, &input, Vec2::new(10.0, 10.0), 200.0);
        p.title("Tool");
        assert_eq!(p.buttons(&["One", "Two"]), Some(1));
        let b = p.finish();
        assert!(b.took_click);
        assert_eq!(b.texts(), ["Tool", "One", "Two"]);
        // A click outside the panel is the game's.
        let input = Input { cursor: Some(Vec2::new(400.0, 300.0)), clicked: true, ..Default::default() };
        let mut p = Panel::new(&f, &input, Vec2::new(10.0, 10.0), 200.0);
        assert_eq!(p.buttons(&["One"]), None);
        assert!(!p.finish().took_click);
    }

    #[test]
    fn a_click_on_the_backing_is_taken_too() {
        let f = font();
        let input = Input { cursor: Some(Vec2::new(15.0, 15.0)), clicked: true, ..Default::default() };
        let mut p = Panel::new(&f, &input, Vec2::new(10.0, 10.0), 200.0);
        p.label("text", colour::TEXT);
        let b = p.finish();
        assert!(b.took_click && b.contains(Vec2::new(15.0, 15.0)) && !b.contains(Vec2::new(300.0, 15.0)));
    }

    #[test]
    fn list_keys_wheel_and_clicks() {
        let f = font();
        let all = items(20);
        let mut s = ListState::default();
        // Down twice: the first item, then the second.
        let input = Input { keys: vec![NavKey::Down, NavKey::Down], ..Default::default() };
        let mut p = Panel::new(&f, &input, Vec2::ZERO, 200.0);
        let ev = p.list(&mut s, &all, 5);
        assert_eq!((s.selected, ev.changed, ev.activated), (Some(1), true, false));
        // End: the last, scrolled into view.
        let input = Input { keys: vec![NavKey::End], ..Default::default() };
        Panel::new(&f, &input, Vec2::ZERO, 200.0).list(&mut s, &all, 5);
        assert_eq!((s.selected, s.top), (Some(19), 15));
        // The wheel up three lines.
        let input = Input { cursor: Some(Vec2::new(50.0, 10.0)), wheel: 1.0, ..Default::default() };
        Panel::new(&f, &input, Vec2::ZERO, 200.0).list(&mut s, &all, 5);
        assert_eq!(s.top, 12);
        // A click on the third row shown (rows 12 high from y = 4) selects
        // item 14; a second click activates it.
        let input = Input { cursor: Some(Vec2::new(50.0, 4.0 + 2.5 * 12.0)), clicked: true, ..Default::default() };
        let ev = Panel::new(&f, &input, Vec2::ZERO, 200.0).list(&mut s, &all, 5);
        assert_eq!((s.selected, ev.changed, ev.activated), (Some(14), true, false));
        let ev = Panel::new(&f, &input, Vec2::ZERO, 200.0).list(&mut s, &all, 5);
        assert!(ev.activated && !ev.changed);
        // Enter activates too.
        let input = Input { keys: vec![NavKey::Enter], ..Default::default() };
        assert!(Panel::new(&f, &input, Vec2::ZERO, 200.0).list(&mut s, &all, 5).activated);
        // The list shrinks: the selection follows.
        Panel::new(&f, &Input::default(), Vec2::ZERO, 200.0).list(&mut s, &all[..3], 5);
        assert_eq!((s.selected, s.top), (Some(2), 0));
        // Empty: nothing selected, no panic.
        Panel::new(&f, &Input::default(), Vec2::ZERO, 200.0).list(&mut s, &[], 5);
        assert_eq!(s.selected, None);
    }

    #[test]
    fn text_field_focus_typing_and_enter() {
        let f = font();
        let mut field = TextField::new("pa");
        // Typing without focus changes nothing.
        let input = Input { text: "x".into(), ..Default::default() };
        let mut p = Panel::new(&f, &input, Vec2::ZERO, 200.0);
        assert!(!p.text_field("Name", &mut field));
        assert!(!p.finish().typing);
        assert_eq!(field.text, "pa");
        // A click on it focuses it; the keys typed go in, Enter submits, and the
        // list after it does not take the keys.
        let input = Input {
            cursor: Some(Vec2::new(150.0, 8.0)),
            clicked: true,
            text: "ck\u{8}".into(),
            keys: vec![NavKey::Enter, NavKey::Down],
            ..Default::default()
        };
        let mut s = ListState::default();
        let mut p = Panel::new(&f, &input, Vec2::ZERO, 200.0);
        assert!(p.text_field("Name", &mut field));
        let ev = p.list(&mut s, &items(3), 3);
        let b = p.finish();
        assert!(b.typing && b.took_click && field.focused);
        assert_eq!(field.text, "pack");
        assert_eq!((s.selected, ev.activated), (None, false));
        // Backspace, then Escape leaves the field.
        let input = Input { keys: vec![NavKey::Backspace, NavKey::Escape], ..Default::default() };
        Panel::new(&f, &input, Vec2::ZERO, 200.0).text_field("Name", &mut field);
        assert_eq!((field.text.as_str(), field.focused), ("pac", false));
        // The length limit.
        field = TextField { text: String::new(), focused: true, max: 3 };
        let input = Input { text: "abcdef".into(), ..Default::default() };
        Panel::new(&f, &input, Vec2::ZERO, 200.0).text_field("", &mut field);
        assert_eq!(field.text, "abc");
    }

    #[test]
    fn long_text_is_cut_to_the_panel() {
        let f = font();
        let input = Input::default();
        let mut p = Panel::new(&f, &input, Vec2::ZERO, 100.0);
        // 92 points inside: 30 characters (3 each) do not fit.
        p.label(&"x".repeat(40), colour::TEXT);
        let b = p.finish();
        let t = b.texts()[0];
        assert!(t.ends_with("...") && t.len() * 3 <= 92, "{t}");
    }

    #[test]
    fn list_state_step_and_clamp() {
        let mut s = ListState::default();
        s.step(1, 0, 4);
        assert_eq!(s.selected, None);
        s.step(-1, 10, 4);
        assert_eq!(s.selected, Some(0));
        s.step(7, 10, 4);
        assert_eq!((s.selected, s.top), (Some(7), 4));
        s.step(100, 10, 4);
        assert_eq!((s.selected, s.top), (Some(9), 6));
        s.clamp(5, 4);
        assert_eq!((s.selected, s.top), (Some(4), 1));
    }
}
