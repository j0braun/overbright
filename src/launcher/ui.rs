//! The launcher's widgets, focus and input. A screen is rebuilt from its
//! data on every change into a flat list of [`Widget`]s with their boxes
//! and their text already laid out ([`super::layout`]); drawing
//! ([`render`]) and input ([`handle`]) work on that list, and the layout
//! tests measure it.

use super::draw::{Canvas, Icon, Rgb, theme};
use super::font::{Face, Fonts};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
    /// `self` lies within `o` (half a pixel of slack for rounding).
    pub fn inside(&self, o: &Rect) -> bool {
        self.x >= o.x - 0.5
            && self.y >= o.y - 0.5
            && self.right() <= o.right() + 0.5
            && self.bottom() <= o.bottom() + 0.5
    }
    /// The two boxes share more than half a pixel each way.
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() - 0.5 && o.x < self.right() - 0.5 && self.y < o.bottom() - 0.5 && o.y < self.bottom() - 0.5
    }
    pub fn offset(&self, dy: f32) -> Rect {
        Rect { y: self.y + dy, ..*self }
    }
}

/// A widget's name, stable across rebuilds (focus is kept by it).
pub type Id = &'static str;

/// A line of text laid out: its line box (`rect.w` = the advance width).
#[derive(Clone, Debug, PartialEq)]
pub struct TextBox {
    pub rect: Rect,
    pub text: String,
    pub face: Face,
    pub size: f32,
    pub color: Rgb,
    /// The whole value when `text` is drawn shortened (a long path with
    /// its middle left out): the screen trace and assistive output carry
    /// it, the drawing does not.
    pub full: Option<String>,
}

/// A status line's mark (`qol::setup::Mark` and the like).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Ok,
    Warn,
    Info,
    Error,
}

impl From<crate::qol::setup::Mark> for Mark {
    fn from(m: crate::qol::setup::Mark) -> Mark {
        use crate::qol::setup::Mark as M;
        match m {
            M::Ok => Mark::Ok,
            M::Warn => Mark::Warn,
            M::Info => Mark::Info,
            M::Error => Mark::Error,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// Text only (`texts`).
    Label,
    /// A thin line across.
    Rule,
    /// A framed box of text lines (a log's tail).
    Panel,
    /// A panel behind the widgets that follow it inside its box (the main
    /// screen's hero, its settings and Mods): a container, drawn first.
    Card,
    /// Overbright's icon (the amber ring), `rect` square.
    Logo,
    /// A bar in the accent colour (under the wordmark).
    Accent,
    /// A mark (`icon`: its box) and text.
    Status { mark: Mark, icon: Rect },
    /// A progress bar, `fraction` (0..1) of it filled.
    Bar { fraction: f32 },
    Button {
        primary: bool,
        /// The large main action (Play on the main screen).
        big: bool,
    },
    /// A value stepped with Left / Right; the arrows' boxes.
    Cycle { left: Rect, right: Rect },
    /// A box (`check`: its square) and a label.
    Checkbox { checked: bool, check: Rect },
    /// Rows `first..first + rows` of `len`, each `row_h` high from the
    /// widget's top (inside `pad`); `texts` are the visible rows'.
    List { len: usize, selected: usize, first: usize, row_h: f32, pad: f32 },
    /// An editable line: `cursor` = its text's length (characters; the
    /// caret is `UiState::cursor`), `shown_from` characters cut from its
    /// start when the text is wider than the box; `empty`: the text shown
    /// is the placeholder; `stops`: the x of each character boundary of
    /// the text shown (a click puts the caret at the nearest), empty when
    /// the text is drawn shortened in its middle (a click: the end).
    TextField { cursor: usize, shown_from: usize, empty: bool, stops: Vec<f32> },
    /// A flat button (a place or a part of the folder browser's path):
    /// its text only, a shade under the pointer.
    Link,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    /// Some for widgets that take focus.
    pub id: Option<Id>,
    pub rect: Rect,
    pub kind: Kind,
    pub texts: Vec<TextBox>,
    pub enabled: bool,
}

impl Widget {
    pub fn focusable(&self) -> bool {
        self.id.is_some() && self.enabled
    }
}

/// The scale: one unit `u` in pixels, from the window's size and the
/// display's scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub w: f32,
    pub h: f32,
    pub u: f32,
    /// The display's scale factor (1 headless unless given).
    pub dpi: f32,
}

/// The smallest unit per display scale factor: the body text stays at
/// least 0.75 × 19 ≈ 14 px at 100 %.
pub const MIN_UNIT: f32 = 0.75;

impl Metrics {
    /// `min(w / 960, h / 720)` (1 at 960 × 720; the layout is designed
    /// there), at least [`MIN_UNIT`] × `dpi`.
    pub fn for_size(w: u32, h: u32, dpi: f32) -> Metrics {
        let (wf, hf) = (w as f32, h as f32);
        let u = (wf / 960.0).min(hf / 720.0).max(MIN_UNIT * dpi);
        Metrics { w: wf, h: hf, u, dpi }
    }
    pub fn with_unit(self, u: f32) -> Metrics {
        Metrics { u, ..self }
    }
    pub fn body(&self) -> f32 {
        (19.0 * self.u).round()
    }
    pub fn small(&self) -> f32 {
        (16.0 * self.u).round().max(12.0)
    }
    pub fn title(&self) -> f32 {
        (30.0 * self.u).round()
    }
    pub fn heading(&self) -> f32 {
        (21.0 * self.u).round()
    }
    pub fn s(&self, v: f32) -> f32 {
        (v * self.u).round()
    }
}

/// A built screen.
#[derive(Clone, Debug, Default)]
pub struct Ui {
    pub widgets: Vec<Widget>,
    /// The window's box (the canvas).
    pub view: Rect,
    /// The content's height (more than the view's: the page scrolls).
    pub content_h: f32,
    /// The widget focused when nothing is (or the focused one went).
    pub default_focus: Option<Id>,
    pub metrics: Option<Metrics>,
    /// The room left above a footer at the window's bottom (px).
    pub slack: f32,
}

/// A drag the mouse is doing (button held).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    /// A list's scroll thumb, held `grab` px below its top.
    ListThumb { id: Id, grab: f32 },
    /// The page's scroll thumb.
    PageThumb { grab: f32 },
}

/// The pointer's shape over a spot of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Default,
    /// Something to click (a button, a row, an arrow, a box).
    Hand,
    /// A text field.
    Text,
}

/// Rows a wheel notch scrolls a list by (as file dialogs do).
pub const WHEEL_ROWS: f32 = 3.0;
/// Two presses on a text field this close (time, px) are a double click.
pub const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(500);
pub const DOUBLE_CLICK_PX: f32 = 6.0;

/// What a screen keeps between rebuilds.
#[derive(Clone, Debug)]
pub struct UiState {
    pub focus: Option<Id>,
    pub hover: Option<Id>,
    /// The button the mouse went down on.
    pub pressed: Option<Id>,
    /// The page's scroll (px) when the content is taller than the window.
    pub scroll: f32,
    /// The focused text field's cursor (characters).
    pub cursor: usize,
    /// Lists' first visible row, by id.
    pub list_first: Vec<(Id, usize)>,
    /// Lists' selected row when last laid out: a list scrolls to its
    /// selection only when that changed (the wheel and the scroll bar
    /// move the rows without moving the selection).
    pub list_sel: Vec<(Id, usize)>,
    /// The pointer's last position (window px), for the wheel.
    pub pointer: Option<(f32, f32)>,
    /// The list row under the pointer.
    pub hover_row: Option<(Id, usize)>,
    /// The list row the last click selected: a click on it again chooses
    /// it (a double click; a slower second click too).
    pub clicked: Option<(Id, usize)>,
    pub drag: Option<Drag>,
    /// Wheel travel not yet turned into whole list rows (rows; a
    /// touchpad's pixel deltas add up here instead of being lost).
    pub wheel_rows: f32,
    /// The focused text field's whole text is selected (a double click):
    /// typing replaces it.
    pub field_all: bool,
    /// The last press (time, x, y, widget) for double clicks on a field.
    pub last_down: Option<(std::time::Instant, f32, f32, Id)>,
    /// The page scrolled by the mouse (wheel, scroll bar, a click): a
    /// rebuild leaves it where the mouse put it instead of scrolling the
    /// focused control back into view; a key turns this off.
    pub mouse_scrolled: bool,
}

impl Default for UiState {
    /// Nothing focused; a text field focused by default has its caret at
    /// the end of its text (a path kept from before is edited from there).
    fn default() -> UiState {
        UiState {
            focus: None,
            hover: None,
            pressed: None,
            scroll: 0.0,
            cursor: usize::MAX,
            list_first: Vec::new(),
            list_sel: Vec::new(),
            pointer: None,
            hover_row: None,
            clicked: None,
            drag: None,
            wheel_rows: 0.0,
            field_all: false,
            last_down: None,
            mouse_scrolled: false,
        }
    }
}

impl UiState {
    pub fn first(&self, id: Id) -> usize {
        self.list_first.iter().find(|(i, _)| *i == id).map_or(0, |e| e.1)
    }
    pub fn set_first(&mut self, id: Id, v: usize) {
        match self.list_first.iter_mut().find(|(i, _)| *i == id) {
            Some(e) => e.1 = v,
            None => self.list_first.push((id, v)),
        }
    }
    pub fn last_sel(&self, id: Id) -> Option<usize> {
        self.list_sel.iter().find(|(i, _)| *i == id).map(|e| e.1)
    }
    pub fn set_last_sel(&mut self, id: Id, v: usize) {
        match self.list_sel.iter_mut().find(|(i, _)| *i == id) {
            Some(e) => e.1 = v,
            None => self.list_sel.push((id, v)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Tab,
    BackTab,
    Enter,
    Space,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Key(Key),
    /// Typed text (a text field takes it).
    Text(String),
    MouseMove(f32, f32),
    MouseDown(f32, f32),
    MouseUp(f32, f32),
    /// Lines, up positive (a mouse wheel's notches).
    Wheel(f32),
    /// Pixels, up positive (a touchpad's smooth scrolling).
    WheelPx(f32),
    /// The mouse's back button (the browser: up one folder; else Back).
    MouseBack,
    /// Files or folders dropped on the window.
    Drop(Vec<std::path::PathBuf>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Insert(String),
    Backspace,
    Delete,
    /// The whole text removed (a selected-all text typed over).
    Clear,
}

/// What an input did, for the screen to act on.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A button pressed, a list row chosen (Enter / double use), a text
    /// field submitted.
    Activate(Id),
    /// A cycle stepped (−1 / +1).
    Step(Id, i32),
    Toggle(Id),
    /// A list's selected row.
    Select(Id, usize),
    /// A text field's text changed at the cursor (`UiState::cursor`).
    Edit(Id, Edit),
    /// Escape (pad B).
    Back,
}

impl Ui {
    pub fn get(&self, id: Id) -> Option<&Widget> {
        self.widgets.iter().find(|w| w.id == Some(id))
    }

    /// The focusable widgets in focus order (build order).
    pub fn focus_order(&self) -> Vec<Id> {
        self.widgets.iter().filter(|w| w.focusable()).filter_map(|w| w.id).collect()
    }

    /// The focus in force: the state's when that widget is still there and
    /// enabled, else the default, else the first.
    pub fn focused(&self, st: &UiState) -> Option<Id> {
        let order = self.focus_order();
        st.focus
            .filter(|f| order.contains(f))
            .or(self.default_focus.filter(|f| order.contains(f)))
            .or(order.first().copied())
    }

    fn widget_at(&self, x: f32, y: f32, st: &UiState) -> Option<&Widget> {
        let y = y + st.scroll;
        self.widgets.iter().find(|w| w.focusable() && w.rect.contains(x, y))
    }

    /// The page's scroll bar when the content is taller than the window:
    /// its track (the window's right edge) and thumb.
    pub fn page_bar(&self, st: &UiState) -> Option<(Rect, Rect)> {
        if self.content_h <= self.view.h + 0.5 {
            return None;
        }
        let m = self.metrics.unwrap_or(Metrics::for_size(self.view.w as u32, self.view.h as u32, 1.0));
        let track = self.view.h;
        let h = (track * track / self.content_h).max(m.s(24.0));
        let y = (track - h) * st.scroll / (self.content_h - self.view.h);
        let w = m.s(12.0).max(8.0);
        Some((Rect::new(self.view.w - w, 0.0, w, track), Rect::new(self.view.w - m.s(6.0), y, m.s(4.0), h)))
    }

    /// The pointer's shape at (`x`, `y`): a hand over what can be
    /// clicked, the text beam over a text field.
    pub fn pointer_at(&self, x: f32, y: f32, st: &UiState) -> Pointer {
        if st.drag.is_some() || self.page_bar(st).is_some_and(|(t, _)| t.contains(x, y)) {
            return Pointer::Default;
        }
        match self.widget_at(x, y, st).map(|w| &w.kind) {
            Some(Kind::TextField { .. }) => Pointer::Text,
            Some(Kind::Button { .. } | Kind::Link | Kind::Cycle { .. } | Kind::Checkbox { .. } | Kind::List { .. }) => {
                Pointer::Hand
            }
            _ => Pointer::Default,
        }
    }

    /// The nearest focusable widget from `from` in a direction (dx, dy):
    /// among those wholly that way, the nearest line (least distance
    /// along it, within 8 units), then the nearest across it.
    pub fn neighbour(&self, from: Id, dx: i32, dy: i32) -> Option<Id> {
        let a = self.get(from)?.rect;
        let mut cands: Vec<(f32, f32, Id)> = Vec::new();
        for w in self.widgets.iter().filter(|w| w.focusable() && w.id != Some(from)) {
            let b = w.rect;
            let (along, across) = match (dx, dy) {
                (0, 1) if b.y >= a.bottom() - 1.0 => (b.y - a.bottom(), (b.cx() - a.cx()).abs()),
                (0, -1) if b.bottom() <= a.y + 1.0 => (a.y - b.bottom(), (b.cx() - a.cx()).abs()),
                // Sideways: the same row only (vertical overlap).
                (1, 0) if b.x >= a.right() - 1.0 && b.y < a.bottom() && a.y < b.bottom() => {
                    (b.x - a.right(), (b.cy() - a.cy()).abs())
                }
                (-1, 0) if b.right() <= a.x + 1.0 && b.y < a.bottom() && a.y < b.bottom() => {
                    (a.x - b.right(), (b.cy() - a.cy()).abs())
                }
                _ => continue,
            };
            cands.push((along, across, w.id.unwrap()));
        }
        // Up / Down: only what is within 160 units sideways counts (when
        // anything is), so a move keeps to its column and does not jump
        // across the window to a nearer line on the other side.
        if dx == 0 {
            let reach = self.metrics.map_or(160.0, |m| m.s(160.0));
            let hgap = |id: Id| {
                self.get(id).map_or(f32::INFINITY, |w| (w.rect.x - a.right()).max(a.x - w.rect.right()).max(0.0))
            };
            let close: Vec<(f32, f32, Id)> = cands.iter().copied().filter(|c| hgap(c.2) <= reach).collect();
            if !close.is_empty() {
                cands = close;
            }
        }
        let near = cands.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
        let tol = self.metrics.map_or(8.0, |m| m.s(8.0));
        cands.into_iter().filter(|c| c.0 <= near + tol).min_by(|x, y| x.1.total_cmp(&y.1)).map(|c| c.2)
    }

    /// Scrolls the page so the focused widget is in view.
    pub fn keep_in_view(&self, st: &mut UiState) {
        let max = (self.content_h - self.view.h).max(0.0);
        if let Some(r) = self.focused(st).and_then(|f| self.get(f)).map(|w| w.rect) {
            let margin = self.metrics.map_or(8.0, |m| m.s(16.0));
            if r.y - margin < st.scroll {
                st.scroll = (r.y - margin).max(0.0);
            }
            if r.bottom() + margin > st.scroll + self.view.h {
                st.scroll = r.bottom() + margin - self.view.h;
            }
        }
        st.scroll = st.scroll.clamp(0.0, max);
    }
}

/// A list's scroll bar's width (px) at unit `u`.
pub fn list_bar_w(u: f32) -> f32 {
    (10.0 * u).round().max(8.0)
}

/// A list's rows shown at once.
pub fn list_rows(w: &Widget) -> usize {
    match &w.kind {
        Kind::List { row_h, pad, .. } => (((w.rect.h - 2.0 * pad) / row_h).round() as usize).max(1),
        _ => 1,
    }
}

/// A list's scroll bar (when it has more rows than it shows): its track
/// inside the frame's right edge and its thumb, in content coordinates.
pub fn list_bar(w: &Widget, u: f32) -> Option<(Rect, Rect)> {
    let Kind::List { len, first, pad, .. } = &w.kind else { return None };
    let rows = list_rows(w);
    if *len <= rows {
        return None;
    }
    let bw = list_bar_w(u);
    let track = Rect::new(w.rect.right() - pad - bw, w.rect.y + pad, bw, w.rect.h - 2.0 * pad);
    let th = (track.h * rows as f32 / *len as f32).max((24.0 * u).round()).min(track.h);
    let at = (*first).min(len - rows) as f32 / (len - rows) as f32;
    Some((track, Rect::new(track.x, track.y + (track.h - th) * at, bw, th)))
}

/// Scrolls list `w` so `first` is its first row (kept in range).
fn scroll_list(st: &mut UiState, w: &Widget, first: f32) {
    if let (Some(id), Kind::List { len, .. }) = (w.id, &w.kind) {
        let max = len.saturating_sub(list_rows(w));
        st.set_first(id, first.round().clamp(0.0, max as f32) as usize);
    }
}

/// The list row of `w` at content y `py` (None past its rows or on the bar).
fn list_row_at(w: &Widget, x: f32, py: f32, u: f32) -> Option<usize> {
    let Kind::List { len, first, row_h, pad, .. } = &w.kind else { return None };
    if list_bar(w, u).is_some_and(|(t, _)| x >= t.x) {
        return None;
    }
    let off = py - w.rect.y - pad;
    if off < 0.0 {
        return None;
    }
    let row = first + (off / row_h) as usize;
    (row < *len && row < first + list_rows(w)).then_some(row)
}

/// The text field's character boundary nearest `x` (characters from the
/// start of its whole text); None when the field shows its text shortened.
fn field_char_at(w: &Widget, x: f32) -> Option<usize> {
    let Kind::TextField { shown_from, empty, stops, .. } = &w.kind else { return None };
    if *empty {
        return Some(0);
    }
    if stops.is_empty() {
        return None;
    }
    let i = stops.iter().enumerate().min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs())).map_or(0, |(i, _)| i);
    Some(shown_from + i)
}

fn move_focus(ui: &Ui, st: &mut UiState, to: Option<Id>) {
    if let Some(t) = to {
        if st.focus != Some(t) {
            st.cursor = usize::MAX;
        }
        st.focus = Some(t);
        ui.keep_in_view(st);
    }
}

/// One input against the built screen: focus moves here; what the screen
/// must do comes back as events.
pub fn handle(ui: &Ui, st: &mut UiState, input: &Input) -> Vec<Event> {
    let mut ev = Vec::new();
    let focus = ui.focused(st);
    st.focus = focus;
    let fw = focus.and_then(|f| ui.get(f));
    let u = ui.metrics.map_or(1.0, |m| m.u);
    // A whole text selected by a double click: an edit replaces it.
    let all = st.field_all && fw.is_some_and(|w| matches!(w.kind, Kind::TextField { .. }));
    match input {
        Input::Key(k) => {
            st.clicked = None;
            st.last_down = None;
            st.mouse_scrolled = false;
            if all && matches!(k, Key::Backspace | Key::Delete) {
                st.field_all = false;
                ev.push(Event::Edit(focus.unwrap(), Edit::Clear));
                return ev;
            }
            if !matches!(k, Key::Enter) {
                st.field_all = false;
            }
            let order = ui.focus_order();
            let pos = focus.and_then(|f| order.iter().position(|&o| o == f));
            match (k, fw.map(|w| &w.kind)) {
                (Key::Tab, _) if !order.is_empty() => {
                    move_focus(ui, st, Some(order[pos.map_or(0, |p| (p + 1) % order.len())]))
                }
                (Key::BackTab, _) if !order.is_empty() => {
                    move_focus(ui, st, Some(order[pos.map_or(0, |p| (p + order.len() - 1) % order.len())]))
                }
                (Key::Escape, _) => ev.push(Event::Back),
                (Key::Up | Key::Down | Key::PageUp | Key::PageDown, Some(Kind::List { len, selected, .. })) => {
                    let page = match &fw.unwrap().kind {
                        Kind::List { row_h, pad, .. } => {
                            (((fw.unwrap().rect.h - 2.0 * pad) / row_h).floor() as usize).max(1)
                        }
                        _ => 1,
                    };
                    let (len, sel) = (*len, *selected);
                    let next = match k {
                        Key::Up if sel > 0 => Some(sel - 1),
                        Key::Down if sel + 1 < len => Some(sel + 1),
                        Key::PageUp => Some(sel.saturating_sub(page)),
                        Key::PageDown => Some((sel + page).min(len.saturating_sub(1))),
                        _ => None,
                    };
                    match next {
                        Some(n) if n != sel => ev.push(Event::Select(focus.unwrap(), n)),
                        Some(_) => {}
                        None => move_focus(ui, st, ui.neighbour(focus.unwrap(), 0, if *k == Key::Up { -1 } else { 1 })),
                    }
                }
                (Key::Up, _) => move_focus(ui, st, focus.and_then(|f| ui.neighbour(f, 0, -1))),
                (Key::Down, _) => move_focus(ui, st, focus.and_then(|f| ui.neighbour(f, 0, 1))),
                (Key::Left | Key::Right, Some(Kind::Cycle { .. })) => {
                    ev.push(Event::Step(focus.unwrap(), if *k == Key::Left { -1 } else { 1 }))
                }
                (Key::Left | Key::Right | Key::Home | Key::End, Some(Kind::TextField { .. })) => {
                    let n = fw.map_or(0, |w| text_len(ui, w));
                    let c = st.cursor.min(n);
                    st.cursor = match k {
                        Key::Left => c.saturating_sub(1),
                        Key::Right => (c + 1).min(n),
                        Key::Home => 0,
                        _ => n,
                    };
                }
                (Key::Backspace, Some(Kind::TextField { .. })) => ev.push(Event::Edit(focus.unwrap(), Edit::Backspace)),
                (Key::Delete, Some(Kind::TextField { .. })) => ev.push(Event::Edit(focus.unwrap(), Edit::Delete)),
                (Key::Left, _) => move_focus(ui, st, focus.and_then(|f| ui.neighbour(f, -1, 0))),
                (Key::Right, _) => move_focus(ui, st, focus.and_then(|f| ui.neighbour(f, 1, 0))),
                (Key::Enter | Key::Space, Some(kind)) => {
                    let id = focus.unwrap();
                    match kind {
                        Kind::Button { .. } | Kind::Link => ev.push(Event::Activate(id)),
                        Kind::Checkbox { .. } => ev.push(Event::Toggle(id)),
                        Kind::Cycle { .. } => ev.push(Event::Step(id, 1)),
                        Kind::List { .. } if *k == Key::Enter => ev.push(Event::Activate(id)),
                        Kind::TextField { .. } if *k == Key::Enter => ev.push(Event::Activate(id)),
                        Kind::TextField { .. } => {
                            if all {
                                st.field_all = false;
                                ev.push(Event::Edit(id, Edit::Clear));
                            }
                            ev.push(Event::Edit(id, Edit::Insert(" ".into())))
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Input::Text(t) => {
            st.last_down = None;
            if let (Some(id), Some(Kind::TextField { .. })) = (focus, fw.map(|w| &w.kind)) {
                let t: String = t.chars().filter(|c| !c.is_control()).collect();
                if !t.is_empty() {
                    if all {
                        st.field_all = false;
                        ev.push(Event::Edit(id, Edit::Clear));
                    }
                    ev.push(Event::Edit(id, Edit::Insert(t)));
                }
            }
        }
        Input::MouseMove(x, y) => {
            st.pointer = Some((*x, *y));
            match st.drag {
                Some(Drag::ListThumb { id, grab }) => {
                    if let Some(w) = ui.get(id)
                        && let (Some((track, thumb)), Kind::List { len, .. }) = (list_bar(w, u), &w.kind)
                    {
                        let rows = list_rows(w);
                        let span = (track.h - thumb.h).max(1.0);
                        let at = ((y + st.scroll - grab - track.y) / span).clamp(0.0, 1.0);
                        scroll_list(st, w, at * (len - rows) as f32);
                    }
                }
                Some(Drag::PageThumb { grab }) => {
                    if let Some((track, thumb)) = ui.page_bar(st) {
                        let span = (track.h - thumb.h).max(1.0);
                        let max = (ui.content_h - ui.view.h).max(0.0);
                        st.scroll = ((y - grab) / span * max).clamp(0.0, max);
                    }
                }
                None => {}
            }
            let w = ui.widget_at(*x, *y, st);
            st.hover = w.and_then(|w| w.id);
            st.hover_row = w.and_then(|w| Some((w.id?, list_row_at(w, *x, y + st.scroll, u)?)));
        }
        Input::MouseDown(x, y) => {
            st.pointer = Some((*x, *y));
            st.mouse_scrolled = true;
            let now = std::time::Instant::now();
            let last = st.last_down.take();
            // The page's scroll bar: the thumb drags, the track pages.
            if let Some((track, thumb)) = ui.page_bar(st)
                && track.contains(*x, *y)
            {
                if thumb.y <= *y && *y < thumb.bottom() {
                    st.drag = Some(Drag::PageThumb { grab: y - thumb.y });
                } else {
                    let max = (ui.content_h - ui.view.h).max(0.0);
                    let page = ui.view.h * 0.9;
                    st.scroll = (st.scroll + if *y < thumb.y { -page } else { page }).clamp(0.0, max);
                }
                return ev;
            }
            let Some(w) = ui.widget_at(*x, *y, st) else {
                st.clicked = None;
                return ev;
            };
            let id = w.id.unwrap();
            if st.focus != Some(id) {
                st.cursor = usize::MAX;
                st.field_all = false;
            }
            st.focus = Some(id);
            let py = y + st.scroll;
            if !matches!(w.kind, Kind::List { .. }) {
                st.clicked = None;
            }
            match &w.kind {
                Kind::Button { .. } | Kind::Link | Kind::Checkbox { .. } => st.pressed = Some(id),
                Kind::Cycle { left, .. } => ev.push(Event::Step(
                    id,
                    if left.contains(*x, py) || *x < w.rect.cx() - w.rect.w / 4.0 { -1 } else { 1 },
                )),
                Kind::List { selected, first, .. } => {
                    if let Some((track, thumb)) = list_bar(w, u)
                        && *x >= track.x
                    {
                        // The scroll bar: the thumb drags, the track pages.
                        if thumb.y <= py && py < thumb.bottom() {
                            st.drag = Some(Drag::ListThumb { id, grab: py - thumb.y });
                        } else {
                            let rows = list_rows(w) as f32;
                            scroll_list(st, w, *first as f32 + if py < thumb.y { -rows } else { rows });
                        }
                        return ev;
                    }
                    if let Some(row) = list_row_at(w, *x, py, u) {
                        if row == *selected && st.clicked == Some((id, row)) {
                            st.clicked = None;
                            ev.push(Event::Activate(id));
                        } else {
                            if row != *selected {
                                ev.push(Event::Select(id, row));
                            }
                            st.clicked = Some((id, row));
                        }
                    }
                }
                Kind::TextField { .. } => {
                    st.cursor = field_char_at(w, *x).unwrap_or(usize::MAX);
                    let double = last.is_some_and(|(t, lx, ly, lid)| {
                        lid == id
                            && now.duration_since(t) <= DOUBLE_CLICK
                            && (lx - x).abs() <= DOUBLE_CLICK_PX
                            && (ly - y).abs() <= DOUBLE_CLICK_PX
                    });
                    st.field_all = double;
                    if double {
                        st.cursor = usize::MAX;
                    } else {
                        st.last_down = Some((now, *x, *y, id));
                    }
                }
                _ => {}
            }
        }
        Input::MouseUp(x, y) => {
            st.drag = None;
            if let Some(p) = st.pressed.take()
                && ui.widget_at(*x, *y, st).and_then(|w| w.id) == Some(p)
            {
                match ui.get(p).map(|w| &w.kind) {
                    Some(Kind::Checkbox { .. }) => ev.push(Event::Toggle(p)),
                    _ => ev.push(Event::Activate(p)),
                }
            }
        }
        Input::Wheel(_) | Input::WheelPx(_) => {
            let m = ui.metrics.unwrap_or(Metrics::for_size(ui.view.w as u32, ui.view.h as u32, 1.0));
            // A list under the pointer that has more rows than it shows
            // scrolls by rows (3 a notch; a touchpad's pixels add up);
            // anything else scrolls the page.
            let list = st.pointer.and_then(|(x, y)| ui.widget_at(x, y, st)).filter(|w| list_bar(w, u).is_some());
            if let Some(w) = list {
                let Kind::List { first, row_h, .. } = &w.kind else { unreachable!() };
                let rows = match input {
                    Input::Wheel(l) => l * WHEEL_ROWS,
                    Input::WheelPx(p) => p / row_h,
                    _ => 0.0,
                };
                if rows.signum() != st.wheel_rows.signum() {
                    st.wheel_rows = 0.0;
                }
                st.wheel_rows += rows;
                let whole = st.wheel_rows.trunc();
                st.wheel_rows -= whole;
                scroll_list(st, w, *first as f32 - whole);
                let id = w.id.unwrap();
                let first = st.first(id);
                st.hover_row = st.pointer.and_then(|(x, y)| {
                    let mut moved = w.clone();
                    if let Kind::List { first: f, .. } = &mut moved.kind {
                        *f = first;
                    }
                    Some((id, list_row_at(&moved, x, y + st.scroll, u)?))
                });
            } else {
                let px = match input {
                    Input::Wheel(l) => l * m.s(40.0),
                    Input::WheelPx(p) => *p,
                    _ => 0.0,
                };
                let max = (ui.content_h - ui.view.h).max(0.0);
                st.scroll = (st.scroll - px).clamp(0.0, max);
                st.mouse_scrolled = true;
            }
        }
        Input::MouseBack | Input::Drop(_) => {}
    }
    ev
}

/// The full length of a text field's text (characters).
fn text_len(_ui: &Ui, w: &Widget) -> usize {
    match w.kind {
        Kind::TextField { cursor: len, .. } => len,
        _ => 0,
    }
}

/// Applies a text edit to `text` at `cursor` (characters).
pub fn apply_edit(text: &mut String, cursor: &mut usize, e: &Edit) {
    let mut chars: Vec<char> = text.chars().collect();
    let c = (*cursor).min(chars.len());
    match e {
        Edit::Insert(s) => {
            let n = s.chars().count();
            chars.splice(c..c, s.chars());
            *cursor = c + n;
        }
        Edit::Backspace if c > 0 => {
            chars.remove(c - 1);
            *cursor = c - 1;
        }
        Edit::Delete if c < chars.len() => {
            chars.remove(c);
            *cursor = c;
        }
        Edit::Clear => {
            chars.clear();
            *cursor = 0;
        }
        _ => *cursor = c,
    }
    *text = chars.into_iter().collect();
}

/// Draws the screen on the plain page (Overbright's own backdrop).
pub fn render(ui: &Ui, st: &UiState, fonts: &Fonts, c: &mut Canvas) {
    let page = super::backdrop::compose(c.width(), c.height(), None, 0.0);
    render_on(ui, st, fonts, c, &page);
}

/// Draws the screen over `page` (the backdrop's pixels, the canvas's size).
pub fn render_on(ui: &Ui, st: &UiState, fonts: &Fonts, c: &mut Canvas, page: &[u8]) {
    c.clear(theme::BG);
    c.fill_from(page);
    let m = ui.metrics.unwrap_or(Metrics::for_size(c.width(), c.height(), 1.0));
    let focus = ui.focused(st);
    let dy = -st.scroll;
    let radius = m.s(6.0);
    let ring = m.s(2.0).max(2.0);
    for w in &ui.widgets {
        let r = w.rect.offset(dy);
        if r.bottom() < 0.0 || r.y > ui.view.h {
            continue;
        }
        let focused = w.focusable() && w.id == focus;
        let hover = w.focusable() && w.id == st.hover;
        match &w.kind {
            Kind::Label => {}
            Kind::Rule => c.fill_rect(r, theme::RULE),
            Kind::Panel => {
                c.fill_round(r, radius, theme::SURFACE);
                c.stroke_round(r, radius, 1.0, theme::RULE);
            }
            Kind::Card => {
                let cr = m.s(10.0);
                c.fill_round(r, cr, theme::CARD);
                c.stroke_round(r, cr, 1.0, theme::RULE);
            }
            Kind::Accent => c.fill_round(r, r.h / 2.0, theme::ACCENT),
            Kind::Logo => {
                let n = r.w.round().max(8.0) as u32;
                c.blit_rgba(r.x.round() as i32, r.y.round() as i32, n, n, &crate::platform::icon::rgba(n));
            }
            Kind::Bar { fraction } => {
                c.fill_round(r, r.h / 2.0, theme::SURFACE_PRESSED);
                let w = (r.w * fraction.clamp(0.0, 1.0)).max(r.h);
                c.fill_round(Rect::new(r.x, r.y, w.min(r.w), r.h), r.h / 2.0, theme::ACCENT);
            }
            Kind::Status { mark, icon } => {
                let (i, col) = match mark {
                    Mark::Ok => (Icon::Check, theme::OK),
                    Mark::Warn => (Icon::Warn, theme::WARN),
                    Mark::Info => (Icon::Info, theme::INFO),
                    Mark::Error => (Icon::Cross, theme::ERROR),
                };
                c.icon(icon.offset(dy), i, col);
            }
            Kind::Button { primary, big } => {
                let pressed = w.enabled && st.pressed.is_some() && st.pressed == w.id;
                let fill = match (primary, w.enabled, pressed, hover) {
                    (_, false, _, _) => theme::SURFACE,
                    (true, _, true, _) => theme::ACCENT_PRESSED,
                    (true, _, _, true) => theme::ACCENT_HOVER,
                    (true, _, _, _) => theme::ACCENT,
                    (false, _, true, _) => theme::SURFACE_PRESSED,
                    (false, _, _, true) => theme::SURFACE_HOVER,
                    (false, _, _, false) => theme::SURFACE,
                };
                c.fill_round(r, if *big { m.s(12.0) } else { radius }, fill);
                if !w.enabled {
                    c.stroke_round(r, radius, 1.0, theme::RULE);
                } else if !primary {
                    c.stroke_round(r, radius, 1.0, theme::BORDER);
                }
            }
            Kind::Cycle { left, right } => {
                c.fill_round(r, radius, if hover && w.enabled { theme::SURFACE_HOVER } else { theme::SURFACE });
                c.stroke_round(r, radius, 1.0, if w.enabled { theme::BORDER } else { theme::RULE });
                let col = if w.enabled { if focused { theme::ACCENT } else { theme::DIM } } else { theme::DISABLED };
                c.icon(left.offset(dy), Icon::ArrowLeft, col);
                c.icon(right.offset(dy), Icon::ArrowRight, col);
            }
            Kind::Checkbox { checked, check } => {
                let b = check.offset(dy);
                c.fill_round(b, m.s(4.0), if *checked { theme::ACCENT } else { theme::SURFACE });
                c.stroke_round(b, m.s(4.0), 1.0, if *checked { theme::ACCENT } else { theme::BORDER });
                if *checked {
                    let inset = b.w * 0.12;
                    c.icon(
                        Rect::new(b.x + inset, b.y + inset, b.w - 2.0 * inset, b.h - 2.0 * inset),
                        Icon::Check,
                        theme::ON_ACCENT,
                    );
                }
            }
            Kind::List { selected, first, row_h, pad, .. } => {
                c.fill_round(r, radius, theme::SURFACE);
                c.stroke_round(r, radius, 1.0, theme::BORDER);
                let bar = list_bar(w, m.u);
                let row_w = r.w - 2.0 * pad - bar.map_or(0.0, |(t, _)| t.w + m.s(4.0));
                // The row under the pointer (not the selected one: that
                // keeps its own shade).
                if let Some((id, row)) = st.hover_row
                    && Some(id) == w.id
                    && row != *selected
                    && row >= *first
                    && w.enabled
                {
                    let hr = Rect::new(r.x + pad, r.y + pad + (row - first) as f32 * row_h, row_w, *row_h);
                    if hr.bottom() <= r.bottom() - pad + 0.5 {
                        c.fill_round(hr, m.s(4.0), theme::SURFACE_HOVER);
                    }
                }
                if let Some((track, thumb)) = bar {
                    let dragging = matches!(st.drag, Some(Drag::ListThumb { id, .. }) if Some(id) == w.id);
                    c.fill_round(track.offset(dy), track.w / 2.0, theme::CARD);
                    let col = if dragging || hover { theme::DIM } else { theme::BORDER };
                    c.fill_round(thumb.offset(dy), thumb.w / 2.0, col);
                }
                if *selected >= *first {
                    let row = (*selected - *first) as f32;
                    let sel = Rect::new(r.x + pad, r.y + pad + row * row_h, row_w, *row_h);
                    if sel.bottom() <= r.bottom() - pad + 0.5 {
                        c.fill_round(sel, m.s(4.0), theme::SELECTED);
                    }
                }
            }
            Kind::TextField { empty, .. } => {
                c.fill_round(r, radius, theme::SURFACE);
                c.stroke_round(r, radius, 1.0, theme::BORDER);
                if focused && st.field_all && !*empty {
                    let t = w.texts[0].rect.offset(dy);
                    c.fill_rect(Rect::new(t.x - 1.0, t.y, t.w + 2.0, t.h), theme::SELECTED);
                }
            }
            Kind::Link => {
                let pressed = st.pressed.is_some() && st.pressed == w.id;
                if w.enabled && (hover || pressed) {
                    c.fill_round(r, m.s(4.0), if pressed { theme::SURFACE_PRESSED } else { theme::SURFACE_HOVER });
                }
            }
        }
        for t in &w.texts {
            let col = if w.enabled { t.color } else { theme::DISABLED };
            c.text(fonts, t.face, t.size, t.rect.x, t.rect.y + dy, &t.text, col);
        }
        if let (true, Kind::TextField { cursor, shown_from, empty, .. }) = (focused, &w.kind) {
            // The caret at the state's cursor (the end when not set).
            let t = &w.texts[0];
            let shown: Vec<char> = if *empty { Vec::new() } else { t.text.chars().collect() };
            let at = st.cursor.min(*cursor).saturating_sub(*shown_from).min(shown.len());
            let x = t.rect.x + fonts.width(t.face, t.size, &shown[..at].iter().collect::<String>());
            c.fill_rect(Rect::new(x, t.rect.y + dy, ring.max(1.0), t.rect.h), theme::TEXT);
        }
        if focused {
            let o = ring + m.s(2.0);
            let fr = Rect::new(r.x - o, r.y - o, r.w + 2.0 * o, r.h + 2.0 * o);
            c.stroke_round(fr, radius + o, ring, theme::ACCENT);
        }
    }
    // A scroll bar when the page is taller than the window (dragged by
    // its thumb, paged by a click beside it).
    if let Some((_, thumb)) = ui.page_bar(st) {
        let col = if matches!(st.drag, Some(Drag::PageThumb { .. })) { theme::DIM } else { theme::BORDER };
        c.fill_round(thumb, m.s(2.0), col);
    }
}

pub fn text(rect: Rect, text: impl Into<String>, face: Face, size: f32, color: Rgb) -> TextBox {
    TextBox { rect, text: text.into(), face, size, color, full: None }
}

/// [`text`] for a value drawn as `shown`, shortened from `full` (the whole
/// value kept for the trace when they differ).
pub fn text_of(rect: Rect, shown: String, full: &str, face: Face, size: f32, color: Rgb) -> TextBox {
    let whole = (shown != full).then(|| full.to_string());
    TextBox { full: whole, ..text(rect, shown, face, size, color) }
}
