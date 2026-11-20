//! The launcher's widgets, focus and input. A screen is rebuilt from its
//! data on every change into a flat list of [`Widget`]s with their boxes
//! and their text already laid out ([`super::layout`]); drawing
//! ([`render`]) and input ([`handle`]) work on that list, and the layout
//! tests measure it.

use super::draw::{Canvas, Dir, Icon, Rgb, Rgba, Theme, select_width};
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

// The lamps are the shared look's (ob-look).
pub use ob_look::lamp::Mark;

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
    /// A page name in the top bar (`texts[0]`); the mouse's way to a page.
    PageTab { current: bool },
    /// The rail under the page names, its dot at `dot_x`.
    Rail { dot_x: f32 },
    /// The big Play button (a button; disabled while starting).
    Play,
    /// A row that opens something: `texts[0]` its label, `texts[1]` its
    /// value, `arrow` the circled arrow's box (a button).
    SumRow { arrow: Rect },
    /// A label and a value stepped with Left / Right (a cycle); `tag`:
    /// the "suggested" tag's box.
    ValueRow { left: Rect, right: Rect, tag: Option<Rect> },
    /// A switch (a checkbox); `knob`: the switch's box.
    Toggle { on: bool, knob: Rect },
    /// A message box: `texts[0]` its title, the rest its text.
    Message { mark: Mark },
    /// The description panel: `texts[0]` its heading, the rest its text.
    Describe,
    /// The dim layer behind a dialog.
    Scrim,
    /// A dialog's box: `texts[0]` its title.
    Dialog,
    /// The footer's button legends.
    Legends { actions: Vec<super::legends::Action> },
    /// One button glyph (beside the page names, inside a note).
    Glyph { glyph: super::legends::Glyph },
    /// A small picture (`size` × `size` RGBA), drawn scaled into the box
    /// (a saved cover on the Saves page).
    Picture { rgba: std::sync::Arc<Vec<u8>>, size: u32 },
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

    /// Reached with the keys and the pad: the page names are not (LB / RB
    /// and Q / E change the page; the mouse clicks them).
    pub fn key_focusable(&self) -> bool {
        self.focusable() && !matches!(self.kind, Kind::PageTab { .. })
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
    /// The backdrop behind the page.
    pub scene: super::backdrop::Scene,
    /// The slab's box (the content's surface), if the page has one.
    pub slab: Option<Rect>,
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
    /// The last input's kind (the legends and the pointer follow it).
    pub input: super::platform_deck::InputKind,
    /// The motion values of the frame being drawn.
    pub motion: super::anim::Frame,
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
            input: Default::default(),
            motion: Default::default(),
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
    /// The page before / after (LB / RB, Q / E, Ctrl+Shift+Tab / Ctrl+Tab).
    PagePrev,
    PageNext,
    /// A page's own actions (X / Space, Y / R).
    ActionX,
    ActionY,
    /// Shortcuts of the Saves page: Ctrl+B, F2, Ctrl+E, Ctrl+Z.
    BackUp,
    Rename,
    Export,
    Undo,
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
        self.widgets.iter().filter(|w| w.key_focusable()).filter_map(|w| w.id).collect()
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
            Some(
                Kind::Button { .. }
                | Kind::Link
                | Kind::Cycle { .. }
                | Kind::Checkbox { .. }
                | Kind::List { .. }
                | Kind::Play
                | Kind::SumRow { .. }
                | Kind::Status { .. }
                | Kind::PageTab { .. }
                | Kind::ValueRow { .. }
                | Kind::Toggle { .. },
            ) => Pointer::Hand,
            _ => Pointer::Default,
        }
    }

    /// The nearest focusable widget from `from` in a direction (dx, dy):
    /// among those wholly that way, the nearest line (least distance
    /// along it, within 8 units), then the nearest across it.
    pub fn neighbour(&self, from: Id, dx: i32, dy: i32) -> Option<Id> {
        let a = self.get(from)?.rect;
        let mut cands: Vec<(f32, f32, Id)> = Vec::new();
        for w in self.widgets.iter().filter(|w| w.key_focusable() && w.id != Some(from)) {
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
                (Key::Left | Key::Right, Some(Kind::Cycle { .. } | Kind::ValueRow { .. })) => {
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
                        Kind::Button { .. }
                        | Kind::Link
                        | Kind::Play
                        | Kind::SumRow { .. }
                        | Kind::Status { .. }
                        | Kind::PageTab { .. } => ev.push(Event::Activate(id)),
                        Kind::Checkbox { .. } | Kind::Toggle { .. } => ev.push(Event::Toggle(id)),
                        Kind::Cycle { .. } | Kind::ValueRow { .. } => ev.push(Event::Step(id, 1)),
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
                Kind::Button { .. }
                | Kind::Link
                | Kind::Checkbox { .. }
                | Kind::Play
                | Kind::SumRow { .. }
                | Kind::Status { .. }
                | Kind::PageTab { .. }
                | Kind::Toggle { .. } => st.pressed = Some(id),
                Kind::Cycle { left, .. } | Kind::ValueRow { left, .. } => ev.push(Event::Step(
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
                    Some(Kind::Checkbox { .. } | Kind::Toggle { .. }) => ev.push(Event::Toggle(p)),
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

/// Draws the screen on the look's own backdrop (the page's scene).
pub fn render(ui: &Ui, st: &UiState, fonts: &Fonts, c: &mut Canvas) {
    let page = super::backdrop::backdrop_a(c.width(), c.height(), ui.scene, None);
    render_on(ui, st, fonts, c, &page);
}

/// The look's unit for skins: 1 at the mockups' 1366 × 768.
fn look_unit(m: &Metrics) -> f32 {
    (m.w / 1366.0).min(m.h / 768.0).max(0.5)
}

/// `r` on whole pixels (a ring drawn inside it stays solid).
fn snap(r: Rect) -> Rect {
    let (x0, y0, x1, y1) = (r.x.round(), r.y.round(), r.right().round(), r.bottom().round());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// A row that shows focus as the selection bar (the near-black bar with
/// lime words), not as a ring outside.
fn shows_bar(k: &Kind) -> bool {
    matches!(
        k,
        Kind::Cycle { .. }
            | Kind::ValueRow { .. }
            | Kind::SumRow { .. }
            | Kind::Toggle { .. }
            | Kind::Checkbox { .. }
            | Kind::Link
            | Kind::Status { .. }
    )
}

/// The busy spinner's turn for the frame being drawn (`Frame::spin`).
static SPIN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Whether the screen shows a busy spinner (the window keeps drawing).
pub fn spinning(ui: &Ui) -> bool {
    ui.widgets.iter().any(|w| matches!(w.kind, Kind::Status { mark: Mark::Busy, .. }))
}

/// A status mark: a filled disc with its glyph cut in the page's dark
/// (Busy: a lime arc).
fn mark(c: &mut Canvas, r: Rect, mark: Mark, t: &Theme) {
    let d = r.w.min(r.h);
    let (cx, cy, rad) = (r.cx(), r.cy(), d / 2.0);
    let glyph = Rect::new(cx - rad * 0.62, cy - rad * 0.62, rad * 1.24, rad * 1.24);
    match mark {
        Mark::Ok => {
            c.disc(cx, cy, rad, t.ok.a(255));
            c.icon(glyph, Icon::Check, t.on_mark);
        }
        Mark::Error => {
            c.disc(cx, cy, rad, t.err.a(255));
            c.icon(Rect::new(cx - rad * 0.5, cy - rad * 0.5, rad, rad), Icon::Cross, t.on_err);
        }
        Mark::Warn | Mark::Info => {
            let (fill, on) = if mark == Mark::Warn { (t.warn, t.on_warn) } else { (t.info, t.on_mark) };
            c.disc(cx, cy, rad, fill.a(255));
            let w = (rad * 0.26).max(1.5);
            let (top, bottom) = if mark == Mark::Warn { (-0.5, 0.12) } else { (-0.12, 0.5) };
            let dot = if mark == Mark::Warn { 0.42 } else { -0.45 };
            c.fill_round_a(Rect::new(cx - w / 2.0, cy + top * rad, w, (bottom - top) * rad), w / 2.0, on.a(255));
            c.disc(cx, cy + dot * rad, w * 0.62, on.a(255));
        }
        Mark::Busy => {
            let w = (rad * 0.24).max(2.0);
            c.ring(cx, cy, rad, w, t.lime.a(60));
            let turn = f32::from_bits(SPIN.load(std::sync::atomic::Ordering::Relaxed)) * 360.0;
            c.arc(cx, cy, rad - w / 2.0, -90.0 + turn, 270.0, w, t.lime.a(255));
        }
    }
}

/// Draws the screen over `page` (the backdrop's pixels, the canvas's size)
/// in the look's skins: the slab, pills, the selection bar for rows and a
/// focus ring outside buttons (a page-coloured gap, then lime, at least
/// 3 px), never the primary button's fill.
pub fn render_on(ui: &Ui, st: &UiState, fonts: &Fonts, c: &mut Canvas, page: &[u8]) {
    SPIN.store(st.motion.spin.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let t = super::draw::theme();
    c.clear(t.page);
    c.fill_from(page);
    let m = ui.metrics.unwrap_or(Metrics::for_size(c.width(), c.height(), 1.0));
    let ua = look_unit(&m);
    let px = |v: f32| (v * ua).round().max(1.0);
    let focus = ui.focused(st);
    let dy = -st.scroll;
    let slide = st.motion.slide_px;
    let row_r = px(12.0);
    let line = px(2.0);
    if let Some(s) = ui.slab {
        c.slab(s.offset(dy), px(16.0), t);
    }
    for w in &ui.widgets {
        // The slab's content slides on a page change; the bars stay.
        let dx = if slide != 0.0 && ui.slab.is_some_and(|s| w.rect.inside(&s)) { slide } else { 0.0 };
        let shift = |q: Rect| Rect::new(q.x + dx, q.y + dy, q.w, q.h);
        let r = shift(w.rect);
        if r.bottom() < 0.0 || r.y > ui.view.h {
            continue;
        }
        let focused = w.focusable() && w.id == focus;
        let hover = w.focusable() && w.id == st.hover;
        let pressed = w.enabled && st.pressed.is_some() && st.pressed == w.id;
        let bar = focused && shows_bar(&w.kind);
        if bar {
            c.fill_round_a(r, row_r, t.sel.a(255));
        } else if hover && shows_bar(&w.kind) && w.enabled {
            c.fill_round_a(r, row_r, t.hover);
        }
        match &w.kind {
            Kind::Label => {}
            Kind::Rule => c.fill_round_a(Rect::new(r.x, r.y, r.w, r.h.max(1.0)), 0.0, t.row_rule.fade(2.0)),
            Kind::Panel | Kind::Describe => {
                let pr = px(14.0);
                c.fill_round_a(r, pr, t.panel);
                c.top_edge(r, pr, t.slab_edge);
            }
            Kind::Card => c.slab(r, px(16.0), t),
            Kind::Picture { rgba, size } => {
                let (w, h) = (r.w.round().max(1.0) as u32, r.h.round().max(1.0) as u32);
                let n = *size as usize;
                if rgba.len() < n * n * 4 {
                    // No picture: a placeholder (its words are the widget's text).
                    c.fill_round_a(r, px(4.0), t.panel);
                    c.stroke_round(r, px(4.0), 1.0, t.dim);
                } else {
                    // Bilinear from the small picture to the box.
                    let mut out = vec![0u8; (w * h * 4) as usize];
                    for y in 0..h as usize {
                        let fy = ((y as f32 + 0.5) * n as f32 / h as f32 - 0.5).clamp(0.0, (n - 1) as f32);
                        let (y0, ty) = (fy.floor() as usize, fy.fract());
                        let y1 = (y0 + 1).min(n - 1);
                        for x in 0..w as usize {
                            let fx = ((x as f32 + 0.5) * n as f32 / w as f32 - 0.5).clamp(0.0, (n - 1) as f32);
                            let (x0, tx) = (fx.floor() as usize, fx.fract());
                            let x1 = (x0 + 1).min(n - 1);
                            for ch in 0..4 {
                                let p = |xx: usize, yy: usize| f32::from(rgba[(yy * n + xx) * 4 + ch]);
                                let top = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
                                let bot = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
                                out[(y * w as usize + x) * 4 + ch] = (top * (1.0 - ty) + bot * ty).round() as u8;
                            }
                        }
                    }
                    c.blit_rgba(r.x.round() as i32, r.y.round() as i32, w, h, &out);
                    c.stroke_round(r, px(4.0), 1.0, t.dim);
                }
            }
            Kind::Accent => c.pill(r, Some(t.lime.a(255)), None),
            Kind::Logo => {
                let ring = (r.w * 0.16).max(2.0);
                c.ring(r.cx(), r.cy(), r.w.min(r.h) / 2.0, ring, t.lime.a(255));
            }
            Kind::Bar { fraction } => {
                c.pill(r, Some(t.bar_track), None);
                let fw = (r.w * fraction.clamp(0.0, 1.0)).max(r.h);
                c.pill(Rect::new(r.x, r.y, fw.min(r.w), r.h), Some(t.lime.a(255)), None);
            }
            Kind::Status { mark: mk, icon } => mark(c, shift(*icon), *mk, t),
            Kind::Button { primary, big } => {
                let edge = line;
                if !w.enabled {
                    c.pill(r, Some(t.button_fill), Some((edge, t.disabled)));
                } else if *primary {
                    let fill = if pressed {
                        t.play_pressed
                    } else if hover {
                        t.play_hover
                    } else {
                        t.play
                    };
                    if focused && *big {
                        c.glow(r, r.h / 2.0, px(40.0), t.lime.a(90));
                    }
                    c.pill(r, Some(fill.a(255)), None);
                } else {
                    c.pill(r, Some(t.button_fill), Some((edge, t.outline)));
                    if hover || pressed {
                        c.pill(r, Some(if pressed { t.pressed } else { t.hover }), None);
                    }
                }
            }
            Kind::Play => {
                let glow = st.motion.play_glow.clamp(0.0, 1.0);
                if focused || glow > 0.0 {
                    let a = (90.0 + 120.0 * glow).min(255.0) as u8;
                    c.glow(r, r.h / 2.0, px(40.0), t.lime.a(a));
                }
                let fill = if !w.enabled {
                    t.play_busy
                } else if pressed {
                    t.play_pressed
                } else if hover {
                    t.play_hover
                } else {
                    t.play
                };
                c.pill(r, Some(fill.a(255)), None);
            }
            Kind::Cycle { left, right } | Kind::ValueRow { left, right, .. } => {
                let col = if !w.enabled {
                    t.disabled
                } else if focused {
                    t.lime
                } else {
                    t.ink
                };
                for (a, d) in [(shift(*left), Dir::Left), (shift(*right), Dir::Right)] {
                    let rad = (a.w.min(a.h) / 2.0).min(px(14.0));
                    c.circled_arrow((a.cx(), a.cy()), rad, d, line, col);
                }
                if let Kind::ValueRow { tag: Some(tag), .. } = &w.kind {
                    let g = shift(*tag);
                    c.fill_round_a(g, px(6.0), t.sel.a(200));
                    c.stroke_round_a(g, px(6.0), 1.0, t.tag_edge);
                }
            }
            Kind::SumRow { arrow } => {
                if !focused {
                    c.fill_round_a(Rect::new(r.x, r.bottom() - 1.0, r.w, 1.0), 0.0, t.row_rule);
                }
                let col = if !w.enabled {
                    t.disabled
                } else if focused {
                    t.lime
                } else {
                    t.dim
                };
                let a = shift(*arrow);
                let s = a.w.min(a.h).min(px(22.0));
                c.chevron(Rect::new(a.cx() - s / 2.0, a.cy() - s / 2.0, s, s), Dir::Right, line, col);
            }
            Kind::Checkbox { checked, check } => {
                let b = shift(*check);
                let br = px(5.0);
                if *checked {
                    c.fill_round_a(b, br, t.lime.a(255));
                    let inset = b.w * 0.14;
                    c.icon(
                        Rect::new(b.x + inset, b.y + inset, b.w - 2.0 * inset, b.h - 2.0 * inset),
                        Icon::Check,
                        t.on_mark,
                    );
                } else {
                    c.fill_round_a(b, br, t.toggle_off.a(255));
                    c.stroke_round_a(b, br, line, if w.enabled { t.toggle_off_edge } else { t.disabled }.a(255));
                }
            }
            Kind::Toggle { on, knob } => {
                let k = shift(*knob);
                let (fill, edge) = if *on { (t.toggle_on, t.lime) } else { (t.toggle_off, t.toggle_off_edge) };
                let edge = if w.enabled { edge } else { t.disabled };
                c.pill(k, Some(fill.a(255)), Some((line, edge)));
                let kr = (k.h / 2.0 - line - px(2.0)).max(2.0);
                let kx = if *on { k.right() - k.h / 2.0 } else { k.x + k.h / 2.0 };
                let knob_c = if !w.enabled {
                    t.disabled
                } else if *on {
                    t.lime
                } else {
                    t.dim
                };
                c.disc(kx, k.cy(), kr, knob_c.a(255));
            }
            Kind::List { selected, first, row_h, pad, .. } => {
                let lr = px(10.0);
                c.fill_round_a(r, lr, t.field);
                c.stroke_round_a(r, lr, 1.0, t.field_edge);
                let bar = list_bar(w, m.u);
                let row_w = r.w - 2.0 * pad - bar.map_or(0.0, |(tr, _)| tr.w + m.s(4.0));
                if let Some((id, row)) = st.hover_row
                    && Some(id) == w.id
                    && row != *selected
                    && row >= *first
                    && w.enabled
                {
                    let hr = Rect::new(r.x + pad, r.y + pad + (row - first) as f32 * row_h, row_w, *row_h);
                    if hr.bottom() <= r.bottom() - pad + 0.5 {
                        c.fill_round_a(hr, px(8.0), t.hover);
                    }
                }
                if let Some((track, thumb)) = bar {
                    let dragging = matches!(st.drag, Some(Drag::ListThumb { id, .. }) if Some(id) == w.id);
                    c.pill(shift(track), Some(t.bar_track.fade(0.6)), None);
                    let col = if dragging || hover { t.ink } else { t.toggle_off_edge };
                    c.pill(shift(thumb), Some(col.a(255)), None);
                }
                if *selected >= *first {
                    let row = (*selected - *first) as f32;
                    let sel = Rect::new(r.x + pad, r.y + pad + row * row_h, row_w, *row_h);
                    if sel.bottom() <= r.bottom() - pad + 0.5 {
                        c.fill_round_a(sel, px(8.0), t.sel.a(255));
                        if focused {
                            c.stroke_round_a(snap(sel), px(8.0), select_width(ua), t.lime.a(255));
                        } else {
                            c.stroke_round_a(sel, px(8.0), 1.0, t.tag_edge);
                        }
                    }
                }
            }
            Kind::TextField { empty, .. } => {
                let fr = px(10.0);
                c.fill_round_a(r, fr, t.field);
                c.stroke_round_a(r, fr, 1.0, t.field_edge);
                if focused && st.field_all && !*empty {
                    let tr = shift(w.texts[0].rect);
                    c.fill_round_a(Rect::new(tr.x - 1.0, tr.y, tr.w + 2.0, tr.h), 2.0, t.lime.a(70));
                }
            }
            Kind::Link => {
                if pressed {
                    c.fill_round_a(r, row_r, t.pressed);
                }
            }
            Kind::PageTab { .. } => {
                if pressed {
                    c.fill_round_a(r, px(8.0), t.pressed);
                }
            }
            Kind::Rail { dot_x } => c.rail(r.x, r.right(), r.cy(), *dot_x + dx, ua, t),
            Kind::Message { mark: mk } => {
                let mr = px(14.0);
                c.fill_round_a(r, mr, t.message);
                let col = match mk {
                    Mark::Error => t.err,
                    Mark::Warn => t.warn,
                    Mark::Ok | Mark::Busy => t.lime,
                    Mark::Info => t.dim,
                };
                c.left_edge(r, mr, px(4.0), col.a(255));
                // The mark before the title, when the layout left room.
                if let Some(tt) = w.texts.first() {
                    let tr = shift(tt.rect);
                    let d = (tr.h * 0.92).round();
                    let x = tr.x - d - px(10.0);
                    if x >= r.x + px(8.0) {
                        mark(c, Rect::new(x, tr.cy() - d / 2.0, d, d), *mk, t);
                    }
                }
            }
            Kind::Scrim => c.fill_round_a(r, 0.0, t.scrim),
            Kind::Dialog => {
                let dr = px(16.0);
                c.glow(Rect::new(r.x, r.y + px(20.0), r.w, r.h), dr, px(60.0), Rgba(0, 0, 0, 150));
                let e = px(1.5);
                c.stroke_round_a(Rect::new(r.x - e, r.y - e, r.w + 2.0 * e, r.h + 2.0 * e), dr + e, e, t.dialog_edge);
                c.fill_round_a(r, dr, t.dialog.a(255));
            }
            Kind::Legends { actions } => super::legends::draw(c, fonts, r, st.input, actions, m.u, t),
            Kind::Glyph { glyph } => {
                super::legends::draw_glyph(c, fonts, *glyph, r.x, r.cy(), m.u, t);
            }
        }
        if bar {
            c.stroke_round_a(snap(r), row_r, select_width(ua), t.lime.a(255));
        }
        // The focused list's selected row: lime words like a bar.
        let sel_row = match &w.kind {
            Kind::List { selected, first, row_h, pad, .. } if focused && selected >= first => {
                let y = w.rect.y + pad + (*selected - *first) as f32 * row_h;
                Some((y, y + row_h))
            }
            _ => None,
        };
        let in_sel = |q: Rect| sel_row.is_some_and(|(a, b)| q.cy() > a && q.cy() < b);
        for tx in &w.texts {
            let col = if !w.enabled {
                t.disabled
            } else if let Kind::PageTab { current } = w.kind {
                if current {
                    t.lime
                } else if hover {
                    t.ink
                } else {
                    t.dim
                }
            } else if (bar || in_sel(tx.rect)) && tx.color == t.ink {
                t.lime
            } else {
                tx.color
            };
            c.text(fonts, tx.face, tx.size, tx.rect.x + dx, tx.rect.y + dy, &tx.text, col);
        }
        if let (true, Kind::TextField { cursor, shown_from, empty, .. }) = (focused, &w.kind) {
            // The caret at the state's cursor (the end when not set).
            let tb = &w.texts[0];
            let shown: Vec<char> = if *empty { Vec::new() } else { tb.text.chars().collect() };
            let at = st.cursor.min(*cursor).saturating_sub(*shown_from).min(shown.len());
            let x = tb.rect.x + dx + fonts.width(tb.face, tb.size, &shown[..at].iter().collect::<String>());
            c.fill_rect(Rect::new(x, tb.rect.y + dy, line, tb.rect.h), t.ink);
        }
        if focused {
            match &w.kind {
                // A field: the ring inside its own edge (the caret shows
                // where typing goes).
                Kind::TextField { .. } => {
                    c.stroke_round_a(snap(r), px(10.0), select_width(ua), t.lime.a(255));
                }
                // A list: its selected row carries the ring.
                Kind::List { .. } => {}
                k if shows_bar(k) => {}
                Kind::Button { .. } | Kind::Play => c.focus_ring(r, r.h / 2.0, ua, t),
                _ => c.focus_ring(r, px(8.0), ua, t),
            }
        }
    }
    // A scroll bar when the page is taller than the window (dragged by
    // its thumb, paged by a click beside it).
    if let Some((_, thumb)) = ui.page_bar(st) {
        let col = if matches!(st.drag, Some(Drag::PageThumb { .. })) { t.ink } else { t.toggle_off_edge };
        c.pill(thumb, Some(col.a(255)), None);
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

#[cfg(test)]
mod skin_tests {
    use super::*;
    use crate::launcher::draw::theme;

    /// A page with one of each look-A widget, laid out by hand at `w` × `h`
    /// (mock units: 1 = 1 px at 1366 × 768), `focus` focused.
    fn sample(w: u32, h: u32, focus: Id) -> (Ui, UiState) {
        let m = Metrics::for_size(w, h, 1.0);
        let k = (w as f32 / 1366.0).min(h as f32 / 768.0);
        let r = |x: f32, y: f32, ww: f32, hh: f32| Rect::new(x * k, y * k, ww * k, hh * k);
        let t = theme();
        let tx = |rc: Rect, s: &str, face: Face, size: f32, col: Rgb| text(rc, s, face, (size * k).round(), col);
        let mut ws = Vec::new();
        let mut push = |id: Option<Id>, rect: Rect, kind: Kind, texts: Vec<TextBox>| {
            ws.push(Widget { id, rect, kind, texts, enabled: true });
        };
        push(None, r(64.0, 38.0, 22.0, 22.0), Kind::Logo, vec![]);
        push(
            Some("page:game"),
            r(966.0, 36.0, 70.0, 30.0),
            Kind::PageTab { current: false },
            vec![tx(r(976.0, 38.0, 60.0, 26.0), "Game", Face::LabelItalic, 21.0, t.dim)],
        );
        push(
            Some("page:settings"),
            r(1058.0, 36.0, 92.0, 30.0),
            Kind::PageTab { current: true },
            vec![tx(r(1068.0, 38.0, 80.0, 26.0), "Settings", Face::LabelItalic, 21.0, t.dim)],
        );
        push(None, r(64.0, 84.0, 1238.0, 2.0), Kind::Rail { dot_x: 1125.0 * k }, vec![]);
        push(
            Some("play"),
            r(64.0, 262.0, 362.0, 86.0),
            Kind::Play,
            vec![tx(r(210.0, 285.0, 80.0, 40.0), "Play", Face::PlayItalic, 34.0, t.on_play)],
        );
        push(
            Some("graphics"),
            r(64.0, 286.0 + 200.0, 680.0, 50.0),
            Kind::ValueRow {
                left: r(344.0, 497.0, 28.0, 28.0),
                right: r(605.0, 497.0, 28.0, 28.0),
                tag: Some(r(645.0, 498.0, 81.0, 26.0)),
            },
            vec![
                tx(r(82.0, 498.0, 120.0, 26.0), "Graphics", Face::LabelItalic, 19.0, t.ink),
                tx(r(414.0, 498.0, 160.0, 26.0), "Enhanced · Medium", Face::LabelItalic, 19.0, t.ink),
            ],
        );
        push(
            Some("row"),
            r(64.0, 380.0, 680.0, 52.0),
            Kind::SumRow { arrow: r(700.0, 392.0, 20.0, 28.0) },
            vec![
                tx(r(82.0, 392.0, 120.0, 26.0), "Screen", Face::LabelItalic, 19.0, t.ink),
                tx(r(344.0, 392.0, 200.0, 26.0), "1920 × 1080", Face::LabelItalic, 19.0, t.ink),
            ],
        );
        push(
            Some("mods"),
            r(64.0, 560.0, 680.0, 52.0),
            Kind::Toggle { on: true, knob: r(344.0, 574.0, 44.0, 24.0) },
            vec![tx(r(82.0, 573.0, 120.0, 26.0), "City Roads HD", Face::LabelItalic, 19.0, t.ink)],
        );
        push(
            Some("back"),
            r(64.0, 630.0, 168.0, 48.0),
            Kind::Button { primary: false, big: false },
            vec![tx(r(88.0, 642.0, 130.0, 26.0), "Back to defaults", Face::LabelItalic, 17.0, t.ink)],
        );
        push(
            None,
            r(808.0, 122.0, 494.0, 200.0),
            Kind::Describe,
            vec![tx(r(834.0, 144.0, 200.0, 28.0), "Enhanced · Medium", Face::TitleItalic, 20.0, t.lime)],
        );
        push(
            None,
            r(808.0, 340.0, 494.0, 150.0),
            Kind::Message { mark: Mark::Error },
            vec![tx(r(860.0, 362.0, 300.0, 28.0), "This is Overbright's folder", Face::TitleItalic, 20.0, t.ink)],
        );
        push(
            None,
            r(834.0, 420.0, 26.0, 26.0),
            Kind::Status { mark: Mark::Ok, icon: r(834.0, 420.0, 24.0, 24.0) },
            vec![],
        );
        push(
            None,
            r(870.0, 420.0, 26.0, 26.0),
            Kind::Status { mark: Mark::Warn, icon: r(870.0, 420.0, 24.0, 24.0) },
            vec![],
        );
        push(
            None,
            r(906.0, 420.0, 26.0, 26.0),
            Kind::Status { mark: Mark::Busy, icon: r(906.0, 420.0, 24.0, 24.0) },
            vec![],
        );
        push(None, r(834.0, 460.0, 300.0, 8.0), Kind::Bar { fraction: 0.4 }, vec![]);
        let ui = Ui {
            widgets: ws,
            view: Rect::new(0.0, 0.0, w as f32, h as f32),
            content_h: h as f32,
            default_focus: Some(focus),
            metrics: Some(m),
            slack: 0.0,
            scene: super::super::backdrop::Scene::Plain,
            slab: Some(r(36.0, 104.0, 1294.0, 578.0)),
        };
        let st = UiState { focus: Some(focus), ..UiState::default() };
        (ui, st)
    }

    fn draw(w: u32, h: u32, focus: Id) -> (Ui, Canvas) {
        let (ui, st) = sample(w, h, focus);
        let fonts = Fonts::new();
        let mut c = Canvas::new(w, h);
        render(&ui, &st, &fonts, &mut c);
        if let Some(dir) = std::env::var_os("OB_SKIN_PNG") {
            let p = std::path::Path::new(&dir).join(format!("skins-{focus}-{w}x{h}.png"));
            c.to_image().save(p).unwrap();
        }
        (ui, c)
    }

    fn px(c: &Canvas, x: i32, y: i32) -> Rgb {
        let p = c.pixmap.pixel(x as u32, y as u32).unwrap();
        Rgb(p.red(), p.green(), p.blue())
    }

    fn near(a: Rgb, b: Rgb) -> bool {
        (i32::from(a.0) - i32::from(b.0)).abs()
            + (i32::from(a.1) - i32::from(b.1)).abs()
            + (i32::from(a.2) - i32::from(b.2)).abs()
            < 60
    }

    /// Solid lime pixels in a row from (x, y) going right (`dir` 1) or
    /// left, within `span` px: the widest run.
    fn lime_run(c: &Canvas, x: f32, y: f32, dir: i32, span: i32) -> i32 {
        let (mut best, mut run) = (0, 0);
        for i in 0..span {
            let xx = x as i32 + dir * i;
            if xx < 0 || xx >= c.width() as i32 {
                break;
            }
            if near(px(c, xx, y as i32), theme().lime) {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        best
    }

    /// The focus ring (outside buttons) and the selection bar's ring are at
    /// least 3 px at every size, and the focused primary button keeps its
    /// own fill (focus is a ring, not the primary style).
    #[test]
    fn focus_rings_at_least_3px() {
        for (w, h) in [(640, 480), (800, 600), (1280, 800), (1366, 768), (1920, 1080), (3840, 2160)] {
            let (ui, c) = draw(w, h, "play");
            let p = ui.get("play").unwrap().rect;
            let run = lime_run(&c, p.right(), p.cy(), 1, 40);
            assert!(run >= 3, "{w}x{h}: Play's ring {run} px");
            assert!(near(px(&c, (p.x + 12.0) as i32, p.cy() as i32), theme().play), "{w}x{h}: Play's fill");
            let (ui, c) = draw(w, h, "back");
            let b = ui.get("back").unwrap().rect;
            let run = lime_run(&c, b.right(), b.cy(), 1, 40);
            assert!(run >= 3, "{w}x{h}: a button's ring {run} px");
            let (ui, c) = draw(w, h, "graphics");
            let g = ui.get("graphics").unwrap().rect;
            let run = lime_run(&c, g.x, g.cy(), 1, 20);
            assert!(run >= 3, "{w}x{h}: the selection bar's ring {run} px");
            assert!(
                near(px(&c, (g.x + g.w * 0.45) as i32, (g.y + g.h * 0.85) as i32), theme().sel),
                "{w}x{h}: the bar's fill"
            );
        }
    }

    /// The backdrop is cached per size and the streak stays out of the
    /// plain scene.
    #[test]
    fn backdrop_cached_and_plain_has_no_streak() {
        use super::super::backdrop::{Scene, backdrop_a};
        let a = backdrop_a(320, 200, Scene::Main, None);
        let b = backdrop_a(320, 200, Scene::Main, None);
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        let plain = backdrop_a(320, 200, Scene::Plain, None);
        let max = |v: &[u8]| v.chunks_exact(4).map(|p| u32::from(p[0])).max().unwrap();
        assert!(max(&a) > 150, "the streak's hot end");
        assert!(max(&plain) < 64, "plain stays dark: {}", max(&plain));
    }
}
