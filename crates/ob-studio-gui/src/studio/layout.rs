//! Studio's window layout (docs/modding/studio.md, "Layout"): where the menu bar, the rail, the docks, the view, the
//! legend row and the status bar go at a window size, and what folds first when the window is small. Pure
//! geometry in logical points, so the tests check every common window size without drawing.

use egui::{Rect, pos2, vec2};

use super::workspace::Kind;

/// The menu bar's height.
pub const MENU_H: f32 = 32.0;
/// A tool workspace's toolbar (the map editor's `TOOLBAR_H`).
pub const TOOLBAR_H: f32 = 40.0;
/// The rail: with names and Ctrl+number at 850 or more lines, icons only below.
pub const RAIL_W: f32 = 72.0;
pub const RAIL_ICONS_W: f32 = 48.0;
pub const RAIL_NAMES_FROM_H: f32 = 850.0;
/// A folded panel's strip (the map editor's fold strip).
pub const STRIP: f32 = 28.0;
/// The left panel: its width by default (wide windows / below), its drag range, and below which window width it
/// folds to its strip.
pub const LEFT_W: f32 = 270.0;
pub const LEFT_W_NARROW: f32 = 230.0;
pub const LEFT_MIN: f32 = 180.0;
pub const WIDE: f32 = 1400.0;
/// The right dock (Inspector, History, Help): its width by default, its drag range, and below which window width
/// it becomes a drawer over the view.
pub const RIGHT_W: f32 = 320.0;
pub const RIGHT_W_NARROW: f32 = 300.0;
pub const RIGHT_MIN: f32 = 260.0;
pub const DRAWER_FROM_W: f32 = 1180.0;
pub const DRAWER_W: f32 = 340.0;
/// The most a side panel takes of the window's width.
pub const SIDE_MAX_FRAC: f32 = 0.4;
/// The bottom dock (Problems, Log, Jobs, the workspace's tabs): its share of the height, its range, its folded
/// tab strip, and below which window height it starts folded.
pub const BOTTOM_FRAC: f32 = 0.2;
pub const BOTTOM_MIN: f32 = 120.0;
/// The bottom dock's share of the height where it holds graphs (Runs).
pub const READING_BOTTOM_FRAC: f32 = 0.36;
pub const BOTTOM_MAX: f32 = 240.0;
pub const BOTTOM_DRAG_MIN: f32 = 80.0;
pub const BOTTOM_TABS: f32 = 34.0;
pub const BOTTOM_FOLD_BELOW_H: f32 = 700.0;
/// The legend row and the status bar.
pub const LEGEND_H: f32 = 36.0;
pub const STATUS_H: f32 = 26.0;
/// The smallest 3D view before the docks give way.
pub const VIEW_MIN: (f32, f32) = (480.0, 270.0);
/// The smallest window; below it Studio shows one line.
pub const WINDOW_MIN: (f32, f32) = (960.0, 600.0);
/// The menu bar's parts: the ← → buttons, the Find field (narrow under the drawer width), the address's range.
pub const BACK_FWD_W: f32 = 64.0;
pub const FIND_W: f32 = 210.0;
pub const FIND_W_NARROW: f32 = 110.0;
pub const ADDRESS_MIN: f32 = 120.0;
pub const ADDRESS_MAX: f32 = 720.0;
/// Below this width the Test and Mod menus go into Go and File.
pub const MENUS_SHORT_BELOW_W: f32 = 1100.0;
/// A page's slab: its margin and its widest.
pub const PAGE_MARGIN: f32 = 24.0;
pub const PAGE_MAX_W: f32 = 1320.0;
/// Gaps between the menu bar's parts.
const GAP: f32 = 8.0;

/// The UI scale for a window of `w` × `h` physical pixels at the system's scale 1: 1.25 from 1000 lines, 1.5 from
/// 1400, 2 from 2000; never so large that the window falls under the smallest size, never below 1.
pub fn ui_scale(w: u32, h: u32) -> f32 {
    let by_height: f32 = match h {
        2000.. => 2.0,
        1400.. => 1.5,
        1000.. => 1.25,
        _ => 1.0,
    };
    let fits = (w as f32 / WINDOW_MIN.0).min(h as f32 / WINDOW_MIN.1);
    by_height.min(fits).max(1.0)
}

/// A side panel's or the bottom dock's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dock {
    /// Shown at this size.
    Open(f32),
    /// Folded to its strip.
    Folded,
    /// Folded to its strip at this window size, opening as a drawer over the view (the right dock under 1180 wide).
    Drawer { open: bool },
}

impl Dock {
    pub fn is_open(self) -> bool {
        matches!(self, Dock::Open(_) | Dock::Drawer { open: true })
    }
}

/// What one workspace keeps of its panels (sizes and folds the player set; `None`: the rule's default).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Memory {
    pub left_w: Option<f32>,
    pub left_folded: Option<bool>,
    pub right_w: Option<f32>,
    pub right_folded: Option<bool>,
    pub bottom_h: Option<f32>,
    pub bottom_folded: Option<bool>,
}

/// What the layout depends on besides the size.
#[derive(Clone, Copy, Debug)]
pub struct Want {
    pub kind: Kind,
    /// The legend row is shown (the last input was a key that moves focus or a pad).
    pub legend: bool,
    pub memory: Memory,
    /// The right dock's drawer is open (only used while the right dock is a drawer).
    pub drawer_open: bool,
    /// The menus' and the mod name's widths as drawn (the caller measures them).
    pub menus_w: f32,
    pub mod_name_w: f32,
    /// The left panel (a list) and the bottom dock (graphs) are the workspace's main controls (Runs): open at any
    /// size the view leaves room for, the bottom taller, unless the player folded or dragged them.
    pub reading: bool,
}

impl Want {
    pub fn new(kind: Kind) -> Want {
        Want {
            kind,
            legend: false,
            memory: Memory::default(),
            drawer_open: false,
            menus_w: 340.0,
            mod_name_w: 140.0,
            reading: false,
        }
    }
}

/// The menu bar's parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuBar {
    pub bar: Rect,
    pub menus: Rect,
    /// Test and Mod sit in Go and File.
    pub short_menus: bool,
    pub back_fwd: Rect,
    pub address: Rect,
    pub find: Rect,
    pub mod_name: Rect,
}

/// The window's parts at one size.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub size: egui::Vec2,
    /// Under the smallest window: only the one line is drawn.
    pub too_small: bool,
    pub menu: MenuBar,
    pub rail: Rect,
    /// The rail shows names and keys (else icons only).
    pub rail_names: bool,
    /// The workspace's whole area (right of the rail, between the menu bar and the legend row or status bar).
    pub content: Rect,
    pub toolbar: Option<Rect>,
    pub left: Option<Rect>,
    pub left_dock: Dock,
    /// The view of a tool workspace, or a page's slab.
    pub view: Rect,
    pub right: Option<Rect>,
    pub right_dock: Dock,
    /// The right dock's drawer over the view, while open.
    pub drawer: Option<Rect>,
    pub bottom: Option<Rect>,
    pub bottom_dock: Dock,
    pub legend: Option<Rect>,
    pub status: Rect,
}

fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w.max(0.0), h.max(0.0)))
}

/// The layout of a window `w` × `h` logical points.
pub fn layout(w: f32, h: f32, want: &Want) -> Layout {
    let too_small = w < WINDOW_MIN.0 || h < WINDOW_MIN.1;
    let status = r(0.0, h - STATUS_H, w, STATUS_H);
    let rail_names = h >= RAIL_NAMES_FROM_H;
    let rail_w = if rail_names { RAIL_W } else { RAIL_ICONS_W };
    let legend = want.legend.then(|| r(rail_w, status.top() - LEGEND_H, w - rail_w, LEGEND_H));
    let bottom_edge = legend.map_or(status.top(), |l| l.top());
    let rail = r(0.0, MENU_H, rail_w, status.top() - MENU_H);
    let content = Rect::from_min_max(pos2(rail_w, MENU_H), pos2(w, bottom_edge));
    let menu = menu_bar(w, want);

    let mut out = Layout {
        size: vec2(w, h),
        too_small,
        menu,
        rail,
        rail_names,
        content,
        toolbar: None,
        left: None,
        left_dock: Dock::Folded,
        view: content,
        right: None,
        right_dock: Dock::Folded,
        drawer: None,
        bottom: None,
        bottom_dock: Dock::Folded,
        legend,
        status,
    };
    match want.kind {
        Kind::Page => {
            let slab_w = (content.width() - 2.0 * PAGE_MARGIN).min(PAGE_MAX_W);
            out.view = r(
                content.left() + PAGE_MARGIN,
                content.top() + PAGE_MARGIN,
                slab_w,
                content.height() - 2.0 * PAGE_MARGIN,
            );
        }
        Kind::Tool => tool(&mut out, want),
    }
    out
}

fn menu_bar(w: f32, want: &Want) -> MenuBar {
    let short_menus = w < MENUS_SHORT_BELOW_W;
    // Go and File take Test and Mod: two menus' words fewer.
    let menus_w = if short_menus { want.menus_w * 0.74 } else { want.menus_w };
    let find_w = if w < DRAWER_FROM_W { FIND_W_NARROW } else { FIND_W };
    let menus = r(GAP, 0.0, menus_w, MENU_H);
    let back_fwd = r(menus.right() + GAP, 0.0, BACK_FWD_W, MENU_H);
    let mod_name = r(w - GAP - want.mod_name_w, 0.0, want.mod_name_w, MENU_H);
    let free = mod_name.left() - GAP - find_w - GAP - (back_fwd.right() + GAP);
    let address = r(back_fwd.right() + GAP, 0.0, free.min(ADDRESS_MAX), MENU_H);
    // The Find field sits right after the address when the address is at its widest.
    let find = r(address.right() + GAP, 0.0, find_w, MENU_H);
    MenuBar { bar: r(0.0, 0.0, w, MENU_H), menus, short_menus, back_fwd, address, find, mod_name }
}

fn side_max(w: f32) -> f32 {
    w * SIDE_MAX_FRAC
}

fn tool(out: &mut Layout, want: &Want) {
    let c = out.content;
    let w = out.size.x;
    let m = want.memory;
    let toolbar = r(c.left(), c.top(), c.width(), TOOLBAR_H);
    let body = Rect::from_min_max(pos2(c.left(), toolbar.bottom()), c.max);
    out.toolbar = Some(toolbar);

    // The rules' first choice, then what the player kept.
    let mut left = if m.left_folded.unwrap_or(w < WIDE && !want.reading) {
        Dock::Folded
    } else {
        let d = if w >= WIDE { LEFT_W } else { LEFT_W_NARROW };
        Dock::Open(m.left_w.unwrap_or(d).clamp(LEFT_MIN, side_max(w)))
    };
    let mut right = if w < DRAWER_FROM_W {
        Dock::Drawer { open: want.drawer_open }
    } else if m.right_folded == Some(true) {
        Dock::Folded
    } else {
        let d = if w >= WIDE { RIGHT_W } else { RIGHT_W_NARROW };
        Dock::Open(m.right_w.unwrap_or(d).clamp(RIGHT_MIN, side_max(w)))
    };
    let width = |d: Dock| match d {
        Dock::Open(x) => x,
        _ => STRIP,
    };
    // The view keeps its smallest size: the right dock becomes a drawer first, then the left panel folds.
    if body.width() - width(left) - width(right) < VIEW_MIN.0 && matches!(right, Dock::Open(_)) {
        right = Dock::Drawer { open: want.drawer_open };
    }
    if body.width() - width(left) - width(right) < VIEW_MIN.0 && matches!(left, Dock::Open(_)) {
        left = Dock::Folded;
    }
    let (lw, rw) = (width(left), width(right));
    out.left = Some(r(body.left(), body.top(), lw, body.height()));
    out.left_dock = left;
    out.right = Some(r(body.right() - rw, body.top(), rw, body.height()));
    out.right_dock = right;

    let mid = Rect::from_min_max(pos2(body.left() + lw, body.top()), pos2(body.right() - rw, body.bottom()));
    let mut bottom = if m.bottom_folded.unwrap_or(out.size.y < BOTTOM_FOLD_BELOW_H && !want.reading) {
        Dock::Folded
    } else {
        let frac = if want.reading { READING_BOTTOM_FRAC } else { BOTTOM_FRAC };
        let d = (mid.height() * frac).clamp(BOTTOM_MIN, BOTTOM_MAX);
        Dock::Open(m.bottom_h.map_or(d, |x| x.clamp(BOTTOM_DRAG_MIN, BOTTOM_MAX)))
    };
    if let Dock::Open(bh) = bottom
        && mid.height() - bh < VIEW_MIN.1
    {
        bottom = Dock::Folded;
    }
    let bh = match bottom {
        Dock::Open(x) => x,
        _ => BOTTOM_TABS,
    };
    out.bottom = Some(r(mid.left(), mid.bottom() - bh, mid.width(), bh));
    out.bottom_dock = bottom;
    out.view = r(mid.left(), mid.top(), mid.width(), mid.height() - bh);
    if right == (Dock::Drawer { open: true }) {
        let dw = DRAWER_W.min(mid.width());
        out.drawer = Some(r(body.right() - rw - dw, body.top(), dw, body.height()));
    }
}

impl Layout {
    /// The parts that must lie inside the window without overlapping each other, by name.
    pub fn parts(&self) -> Vec<(&'static str, Rect)> {
        let mut v = vec![
            ("menus", self.menu.menus),
            ("back/forward", self.menu.back_fwd),
            ("address", self.menu.address),
            ("find", self.menu.find),
            ("mod name", self.menu.mod_name),
            ("rail", self.rail),
            ("view", self.view),
            ("status", self.status),
        ];
        let opt = [
            ("toolbar", self.toolbar),
            ("left", self.left),
            ("right", self.right),
            ("bottom", self.bottom),
            ("legend", self.legend),
        ];
        v.extend(opt.into_iter().filter_map(|(n, r)| Some((n, r?))));
        v
    }

    /// What is wrong with this layout: a part outside the window, two parts overlapping, a view or an address
    /// under its smallest size. Empty for a good layout (and for a window under the smallest, which draws only
    /// its one line).
    pub fn problems(&self) -> Vec<String> {
        if self.too_small {
            return vec![];
        }
        let window = Rect::from_min_size(pos2(0.0, 0.0), self.size);
        let parts = self.parts();
        let mut p = vec![];
        for (i, (n, a)) in parts.iter().enumerate() {
            if !window.expand(0.01).contains_rect(*a) {
                p.push(format!("{n} {a:?} is outside the window"));
            }
            if a.width() < 1.0 || a.height() < 1.0 {
                p.push(format!("{n} has no room ({a:?})"));
            }
            for (m, b) in &parts[i + 1..] {
                let o = a.intersect(*b);
                if o.width() > 0.5 && o.height() > 0.5 {
                    p.push(format!("{n} and {m} overlap ({o:?})"));
                }
            }
        }
        if self.toolbar.is_some() && (self.view.width() < VIEW_MIN.0 || self.view.height() < VIEW_MIN.1) {
            p.push(format!("the view is {:.0} × {:.0}, under its smallest", self.view.width(), self.view.height()));
        }
        if self.menu.address.width() < ADDRESS_MIN {
            p.push(format!("the address is {:.0} wide, under {ADDRESS_MIN}", self.menu.address.width()));
        }
        if let Some(d) = self.drawer
            && !self.view.union(self.right.unwrap_or(self.view)).expand(0.01).contains_rect(d)
        {
            p.push("the drawer leaves the view".into());
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every common window size (the resolution matrix of the launcher's and the editor's shot checks) and the
    /// display scales of high-density screens.
    pub const SIZES: [(u32, u32); 25] = [
        (640, 480),
        (800, 600),
        (1024, 768),
        (1280, 960),
        (1600, 1200),
        (1280, 1024),
        (1280, 800),
        (1440, 900),
        (1680, 1050),
        (1920, 1200),
        (2560, 1600),
        (1280, 720),
        (1366, 768),
        (1600, 900),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
        (2160, 1440),
        (2256, 1504),
        (2560, 1080),
        (3440, 1440),
        (3840, 1600),
        (3840, 1080),
        (5120, 1440),
        (1080, 1920),
    ];

    fn logical(w: u32, h: u32) -> (f32, f32) {
        let s = ui_scale(w, h);
        (w as f32 / s, h as f32 / s)
    }

    #[test]
    fn scale_follows_the_design() {
        assert_eq!(logical(1920, 1080), (1536.0, 864.0));
        assert_eq!(logical(2560, 1080), (2048.0, 864.0));
        assert_eq!(logical(1280, 800), (1280.0, 800.0));
        assert_eq!(logical(1024, 768), (1024.0, 768.0));
        assert_eq!(ui_scale(2560, 1440), 1.5);
        assert_eq!(ui_scale(3840, 2160), 2.0);
        // A portrait screen keeps its width over the smallest window.
        assert!(logical(1080, 1920).0 >= WINDOW_MIN.0);
    }

    #[test]
    fn every_common_size_lays_out_without_clips_or_overlaps() {
        use super::super::workspace::Workspace;
        let mut drawn = 0;
        for (w, h) in SIZES {
            let (lw, lh) = logical(w, h);
            for ws in Workspace::ALL {
                for legend in [false, true] {
                    for drawer_open in [false, true] {
                        let want = Want { legend, drawer_open, ..Want::new(ws.kind()) };
                        let l = layout(lw, lh, &want);
                        let small = w < 960 || h < 600;
                        assert_eq!(l.too_small, small, "{w}x{h}");
                        if !small {
                            drawn += 1;
                        }
                        assert!(l.problems().is_empty(), "{w}x{h} {ws:?} legend {legend}: {:#?}", l.problems());
                    }
                }
            }
        }
        assert!(drawn > 300);
    }

    #[test]
    fn the_four_review_sizes() {
        let world = |w, h| {
            let (lw, lh) = logical(w, h);
            layout(lw, lh, &Want::new(Kind::Tool))
        };
        // 1080p: everything docked, the rail with names.
        let l = world(1920, 1080);
        assert!(l.rail_names);
        assert_eq!(l.left_dock, Dock::Open(LEFT_W));
        assert_eq!(l.right_dock, Dock::Open(RIGHT_W));
        assert!(matches!(l.bottom_dock, Dock::Open(_)));
        // The Deck: the tree folded to its strip, the Inspector docked, icons only.
        let l = world(1280, 800);
        assert!(!l.rail_names);
        assert_eq!(l.left_dock, Dock::Folded);
        assert_eq!(l.right_dock, Dock::Open(RIGHT_W_NARROW));
        // 1024 × 768: the tree and the Inspector as strip and drawer.
        let l = world(1024, 768);
        assert_eq!(l.left_dock, Dock::Folded);
        assert_eq!(l.right_dock, Dock::Drawer { open: false });
        // 21:9: as 1080p, the extra width goes to the view; a page's slab stops at its widest.
        let wide = world(2560, 1080);
        let narrow = world(1920, 1080);
        assert_eq!(wide.left, wide.left.map(|r| r.with_max_x(r.left() + LEFT_W)));
        assert!((wide.view.width() - narrow.view.width() - 512.0).abs() < 0.5);
        let page = layout(2048.0, 864.0, &Want::new(Kind::Page));
        assert_eq!(page.view.width(), PAGE_MAX_W);
    }

    #[test]
    fn kept_sizes_and_folds_win_within_their_ranges() {
        let mut want = Want::new(Kind::Tool);
        want.memory =
            Memory { left_w: Some(5000.0), right_folded: Some(true), bottom_folded: Some(true), ..Default::default() };
        let l = layout(1536.0, 864.0, &want);
        assert_eq!(l.left_dock, Dock::Open(1536.0 * SIDE_MAX_FRAC));
        assert!(l.problems().is_empty(), "{:?}", l.problems());
        want.memory.left_w = Some(200.0);
        let l = layout(1536.0, 864.0, &want);
        assert_eq!(l.left_dock, Dock::Open(200.0));
        assert_eq!(l.right_dock, Dock::Folded);
        assert_eq!(l.bottom_dock, Dock::Folded);
        assert_eq!(l.bottom.map(|b| b.height()), Some(BOTTOM_TABS));
    }

    #[test]
    fn short_windows_fold_the_bottom_dock_and_narrow_ones_the_menus() {
        let l = layout(1280.0, 690.0, &Want::new(Kind::Tool));
        assert_eq!(l.bottom_dock, Dock::Folded);
        let l = layout(1024.0, 768.0, &Want::new(Kind::Tool));
        assert!(l.menu.short_menus);
        assert_eq!(l.menu.find.width(), FIND_W_NARROW);
        assert!(layout(959.0, 700.0, &Want::new(Kind::Page)).too_small);
        assert!(layout(1200.0, 599.0, &Want::new(Kind::Page)).too_small);
    }
}
