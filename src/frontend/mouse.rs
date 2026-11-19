//! The front end's mouse (**verified** from SPEED2.EXE unless marked; see
//! docs/formats.md "Front-end mouse").
//!
//! * The device (0x8763D0, DirectInput mouse polled once a frame by
//!   0x5BF870): +0x4 / +0x8 the cursor in 640 × 480 units (0x5BF760:
//!   GetCursorPos less the window's top left, × 640 / client width and
//!   × 480 / client height, truncated), +0x14 / +0x18 the move since the
//!   last frame, +0x1C the wheel (DIMOUSESTATE lZ), +0x20 a swap flag
//!   (0x8763F0: then the right button is the one that clicks), per button
//!   held / pressed (edge down) / released (edge up): left +0x21..+0x23,
//!   right +0x24..+0x26, middle +0x27..+0x29.
//! * The front end's tick (0x5663C0) sends 0xC98356BA to the screens, then
//!   0x9803F6E2 (the mouse message) when a button is down or was released,
//!   the wheel turned or the cursor moved (0x5BF950). Each screen class
//!   handles it itself; this module keeps what the clone's screens share:
//!   the cursor, the last frame's object rectangles and the slider rows.
//! * The cursor (0x5378D0 → 0x50B460): texture 0x6B6A294F at the
//!   cursor less (12, 6), its own size, drawn after the front end when the
//!   front end is up (0x7F91C4, 0x8384C4), not while a movie plays (0x8383AC) and
//!   while 0x7FA29C is set (cleared only by the Take Picture paths
//!   0x4CEE11 / 0x55B2B1), and only within 2 s of the cursor's last move
//!   (0x83A9CC, 0x7844C0 = 2.0) or always while g_GameState 0x8654A4 is 3
//!   (the front end).
//!
//! Wide screens (clone deviation): the cursor point is mapped into each
//! object's own placement (`display::Place`, the anchoring of the side
//! elements) before it is tested against the object's rectangle.

use std::cell::RefCell;
use std::collections::HashMap;

use glam::Vec2;

use crate::render::display::{Place, UiFrame};

/// The cursor's texture (0x50B460 → 0x4901D0(0x6B6A294F)).
pub const CURSOR: u32 = 0x6B6A_294F;
/// The cursor quad's offset from the point (0x50B460: x − 12, y − 6).
pub const CURSOR_OFFSET: Vec2 = Vec2::new(-12.0, -6.0);
/// How long the cursor stays after it moved, ms (0x7844C0 × 4000 ticks).
pub const SHOW_MS: f32 = 2000.0;

/// Pseudo objects the icon strip records: the selected option's icon and
/// the strip's box.
pub const STRIP_SELECTED: u32 = 1;
pub const STRIP: u32 = 2;
/// A parts column's current item (`scroller_quads` down a column).
pub const COLUMN_SELECTED: u32 = 3;
/// UI_Options_PC_Controller's key cells: + row × 2 + column.
pub const CONTROL_CELL: u32 = 0x200;
/// The curve widget's point columns (+ k).
pub const CURVE_COLUMN: u32 = 0x100;

/// One drawn object's rectangle on the canvas and its placement.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub lo: Vec2,
    pub hi: Vec2,
    pub place: Place,
}

/// A slider row's mouse targets (0x52FC50, 0x52DA00, 0x565060), canvas
/// units centred on the screen: its index, the row's title box (top left,
/// size), the left and right arrows (top left, size) and the tip mark's
/// anchor (its top centre).
pub type SliderRow = (usize, Vec2, Vec2, (Vec2, Vec2), (Vec2, Vec2), Option<Vec2>);

#[derive(Default)]
struct Frame {
    /// Per (package, object hash): the union of its quads.
    rects: HashMap<(String, u32), Rect>,
    /// The images laid out hidden this frame (0x51DA00 has no visibility
    /// test, so the native hit tests still find them).
    hidden: HashMap<(String, u32), Rect>,
    sliders: Vec<SliderRow>,
    /// The scroll bars laid out this frame (0x538090), per (package, prefix).
    bars: HashMap<(String, String), Bar>,
}

/// A scroll bar's layout (0x538090 → 0x52E330, the fixed handle; centred
/// canvas units along its axis): the track's start and length, the
/// handle's length, the step (+0x34) and the handle's home centre for the
/// current position (+0xC / +0x10).
#[derive(Clone, Copy, Debug)]
pub struct Bar {
    pub horizontal: bool,
    pub start: f32,
    pub length: f32,
    pub side: f32,
    pub step: f32,
    pub home: f32,
}

/// A handle being dragged (the bar's +2): its package and prefix, the
/// grab offset (+0x14 / +0x18, along the axis) and the handle's top left
/// now (0x52E4F0, along the axis, centred canvas units).
#[derive(Clone, Debug)]
pub struct Drag {
    pub package: String,
    pub prefix: String,
    pub grab: f32,
    pub top_left: f32,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static RECORDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CURRENT: RefCell<Frame> = RefCell::new(Frame::default());
    static LAST: RefCell<Frame> = RefCell::new(Frame::default());
}

/// A new layout: what the last one recorded becomes what the mouse tests.
/// `recording`: whether this layout's front-end objects are recorded (only
/// while a menu is up, so races pay nothing). The game lays the front end
/// out for the map on each tick (`Game::fe_map_tick`, the
/// tick's state, `begin_frame(true)` … layout … `begin_frame(false)`), so
/// a click is tested against the layout of the tick it is applied on and
/// the draw's layouts (recording off) write nothing here.
pub fn begin_frame(recording: bool) {
    RECORDING.with(|r| r.set(recording));
    let done = CURRENT.with(|c| std::mem::take(&mut *c.borrow_mut()));
    LAST.with(|l| *l.borrow_mut() = done);
}

/// Whether this frame records (`begin_frame`).
pub(crate) fn recording() -> bool {
    RECORDING.with(|r| r.get())
}

/// Record an object's quads (from `layout`).
pub(crate) fn record(package: &str, hash: u32, quads: &[super::Quad]) {
    let package = package.strip_suffix(".fng").unwrap_or(package);
    let Some(first) = quads.first() else { return };
    let mut lo = first.rect[0].min(first.rect[1]);
    let mut hi = first.rect[0].max(first.rect[1]);
    for q in &quads[1..] {
        lo = lo.min(q.rect[0].min(q.rect[1]));
        hi = hi.max(q.rect[0].max(q.rect[1]));
    }
    let place = Place { stretch: false, fill: false, ..first.place };
    CURRENT.with(|c| {
        let mut c = c.borrow_mut();
        let e = c.rects.entry((package.to_string(), hash)).or_insert(Rect { lo, hi, place });
        e.lo = e.lo.min(lo);
        e.hi = e.hi.max(hi);
    });
}

/// Record a hidden image's quads (from `layout`).
pub(crate) fn record_hidden(package: &str, hash: u32, quads: &[super::Quad]) {
    let package = package.strip_suffix(".fng").unwrap_or(package);
    let Some(q) = quads.first() else { return };
    let (lo, hi) = (q.rect[0].min(q.rect[1]), q.rect[0].max(q.rect[1]));
    let place = Place { stretch: false, fill: false, ..q.place };
    CURRENT.with(|c| {
        c.borrow_mut().hidden.insert((package.to_string(), hash), Rect { lo, hi, place });
    });
}

/// Record a scroll bar's layout (from `scroll_bar_at`).
pub(crate) fn record_bar(package: &str, prefix: &str, bar: Bar) {
    if !recording() {
        return;
    }
    let package = package.strip_suffix(".fng").unwrap_or(package);
    CURRENT.with(|c| {
        c.borrow_mut().bars.insert((package.to_string(), prefix.to_string()), bar);
    });
}

/// The last frame's layout of a package's scroll bar.
pub fn bar(package: &str, prefix: &str) -> Option<Bar> {
    LAST.with(|l| l.borrow().bars.get(&(package.to_string(), prefix.to_string())).copied())
}

/// The handle being dragged, if any.
pub fn drag() -> Option<Drag> {
    DRAG.with(|d| d.borrow().clone())
}

pub fn set_drag(drag: Option<Drag>) {
    DRAG.with(|d| *d.borrow_mut() = drag);
}

/// The dragged handle's top left on this bar (the layout leaves it there
/// while it is dragged, 0x52E210 / 0x52E330 skip +2).
pub(crate) fn dragged(package: &str, prefix: &str) -> Option<f32> {
    let package = package.strip_suffix(".fng").unwrap_or(package);
    DRAG.with(|d| d.borrow().as_ref().filter(|d| d.package == package && d.prefix == prefix).map(|d| d.top_left))
}

/// Record a rectangle the screen's code draws itself (canvas units; only
/// while recording, `begin_frame`).
pub fn record_rect(package: &str, hash: u32, lo: Vec2, hi: Vec2, place: Place) {
    if !recording() {
        return;
    }
    CURRENT.with(|c| {
        c.borrow_mut().rects.insert((package.to_string(), hash), Rect { lo, hi, place });
    });
}

/// The slider rows of this layout (only while recording, `begin_frame`).
pub(crate) fn set_slider_rows(rows: Vec<SliderRow>) {
    if !recording() {
        return;
    }
    CURRENT.with(|c| c.borrow_mut().sliders = rows);
}

/// The last frame's rectangle of a package's object.
pub fn rect(package: &str, hash: u32) -> Option<Rect> {
    let key = (package.to_string(), hash);
    LAST.with(|l| l.borrow().rects.get(&key).copied())
}

/// The last frame's rectangle of a package's object, drawn or a hidden
/// image (the native hit tests, 0x52D030 → 0x51DA00).
pub fn rect_any(package: &str, hash: u32) -> Option<Rect> {
    let key = (package.to_string(), hash);
    LAST.with(|l| {
        let l = l.borrow();
        l.rects.get(&key).or_else(|| l.hidden.get(&key)).copied()
    })
}

/// dev harness: the rectangles recorded so far this frame (or, when
/// none, the last frame's), sorted, for `render::native::ui::geom_dump`.
#[cfg(feature = "dev-harness")]
pub fn geom_rects() -> Vec<((String, u32), Rect)> {
    let mut v: Vec<_> = CURRENT.with(|c| c.borrow().rects.iter().map(|(k, r)| (k.clone(), *r)).collect());
    if v.is_empty() {
        v = LAST.with(|l| l.borrow().rects.iter().map(|(k, r)| (k.clone(), *r)).collect());
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// The last frame's slider rows.
pub fn slider_rows() -> Vec<SliderRow> {
    LAST.with(|l| l.borrow().sliders.clone())
}

/// The cursor (target pixels) as a canvas point for elements placed by
/// `place` (the inverse of `UiFrame::to_pixels`).
pub fn canvas_point(frame: &UiFrame, px: Vec2, place: Place) -> Vec2 {
    Vec2::new(
        (px.x - frame.origin.0 - place.x as f32 * frame.reach_x(place)) / frame.scale.0,
        (px.y - frame.origin.1 - place.y as f32 * frame.reach.1) / frame.scale.1,
    )
}

/// Whether the cursor (target pixels) lies in `r` (0x50CD00: pos ≤ p ≤
/// pos + size on both axes; 0x52D030 likewise: both edges inclusive).
pub fn inside(frame: &UiFrame, px: Vec2, r: &Rect) -> bool {
    let p = canvas_point(frame, px, r.place);
    p.x >= r.lo.x && p.x <= r.hi.x && p.y >= r.lo.y && p.y <= r.hi.y
}

/// The placement of the cursor's own quad: the centred canvas, or a side
/// anchor when the cursor is over the bars beside it on a wide screen (so
/// it is not clipped there; clone deviation).
pub fn cursor_place(frame: &UiFrame, px: Vec2) -> Place {
    let left = frame.origin.0;
    let right = frame.origin.0 + 640.0 * frame.scale.0;
    let x = if px.x < left { -1 } else if px.x >= right { 1 } else { 0 };
    Place::new(x, 0)
}

/// The mouse as the game samples it each frame (0x5BF870).
#[derive(Clone, Copy, Debug, Default)]
pub struct Mouse {
    /// The cursor in target pixels (None until it enters the window).
    pub at: Option<Vec2>,
    /// The cursor's position at the last frame (for the move test).
    pub last: Option<Vec2>,
    /// When the cursor last moved (ms, the screens' clock).
    pub moved_at: f32,
    /// The real-time clock 0x8651A8 then (1/4000 s; 0x83A9CC).
    pub moved_real: u64,
    pub held: bool,
    /// Edges since the last frame (+0x22 / +0x23).
    pub pressed: bool,
    pub released: bool,
    /// The wheel since the last frame (lZ: > 0 away from the player).
    pub wheel: f32,
    /// The middle button held (+0x27) and let go since the last frame
    /// (+0x29): GarageMain turns the car with it (`garage::DragInput`).
    pub middle: bool,
    pub middle_released: bool,
}

impl Mouse {
    /// Whether the cursor moved since the last frame (+0x14 / +0x18).
    pub fn moved(&self) -> bool {
        match (self.at, self.last) {
            (Some(a), Some(b)) => a != b,
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// 0x5BF950: anything for the screens this frame.
    pub fn active(&self) -> bool {
        self.held || self.pressed || self.released || self.wheel != 0.0 || self.moved()
    }

    /// End of a frame: edges and the wheel are used up.
    pub fn end_frame(&mut self) {
        self.last = self.at;
        self.pressed = false;
        self.released = false;
        self.wheel = 0.0;
        self.middle_released = false;
    }
}
