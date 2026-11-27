use std::sync::atomic::{AtomicBool, Ordering};

// The aspect mode and the views' projection factors are display values the
// camera computes with (`draw::aspect`).
pub use crate::draw::aspect::{AspectMode, aspect_mode, set_aspect_mode, view_aspect};

/// Where a front-end / HUD quad goes on a screen that is not 4:3 (clone
/// deviation, [`AspectMode::Wide`]; see docs/formats.md "Display modes
/// and aspect"). The quad keeps its canvas size (scaled evenly with the
/// screen height) and is moved by `x` / `y` (−1, 0, 1) times the anchor
/// reach ([`UiFrame`]): −1 towards the left / top edge, 1 towards the
/// right / bottom, 0 not at all. At 4:3 nothing moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub x: i8,
    pub y: i8,
    /// A flat-colour quad (a white texture tinted, e.g. RaceOverBacking):
    /// where it spans the canvas's width (height) it reaches the screen's
    /// edges, as untextured quads (fades, letterbox bars) always do. (Any
    /// quad that magnifies its texture enough does too, see
    /// `render::ui`: the flag only forces it.)
    pub fill: bool,
    /// A strip of a side-anchored element that runs from the canvas's
    /// left third into its right third (the help bar's backing under the
    /// buttons): its left end moves with the left edge and its right end
    /// with the right edge, so it stretches between them (`x` is then 0).
    pub stretch: bool,
    /// An in-game HUD element (`HUD_*`, `2PHud*`, `Chyron_IG`): moved by
    /// the HUD's reach ([`UiFrame::hud_reach`], the Enhancements row "HUD
    /// Layout") instead of the front end's.
    pub hud: bool,
}

impl Place {
    pub const CENTRE: Place = Place { x: 0, y: 0, fill: false, stretch: false, hud: false };
    /// Top centre (the rear-view mirror's quad).
    pub const TOP_CENTRE: Place = Place { x: 0, y: -1, fill: false, stretch: false, hud: false };
    pub const fn new(x: i8, y: i8) -> Place {
        Place { x, y, fill: false, stretch: false, hud: false }
    }

    /// [`Place::new`] for an in-game HUD element ([`Place::hud`]).
    pub const fn hud(x: i8, y: i8) -> Place {
        Place { x, y, fill: false, stretch: false, hud: true }
    }

    /// The canvas point a part placed so, spanning `lo`..`hi`, keeps when
    /// it is scaled (a HUD scale): the canvas edge it is anchored to on
    /// each axis, or its own middle where it is centred on that axis.
    pub fn origin(self, lo: glam::Vec2, hi: glam::Vec2) -> glam::Vec2 {
        if self.stretch {
            // Both ends anchored: scaled about its middle.
            let y = Place { stretch: false, ..self }.origin(lo, hi).y;
            return glam::Vec2::new((lo.x + hi.x) * 0.5, y);
        }
        let axis = |a: i8, lo: f32, hi: f32, size: f32| match a {
            ..0 => 0.0,
            0 => (lo + hi) * 0.5,
            _ => size,
        };
        glam::Vec2::new(axis(self.x, lo.x, hi.x, 640.0), axis(self.y, lo.y, hi.y, 480.0))
    }
}

/// The safe area (w / h) the anchored elements spread over (a
/// design choice): a centred 16:9 region. The front end's side elements (the
/// help bar's name and buttons, EA Trax) always stop at its edges, and
/// the HUD does unless "HUD Layout" is Full Width ([`hud_full_width`]).
/// At 16:9 and narrower it is the whole screen, so 4:3 / 16:9 output is
/// unchanged; on 21:9 / 32:9 screens the menus stay a centred 16:9 and the
/// tach and the map stay within sight.
pub const SAFE_SPREAD: f32 = 16.0 / 9.0;

static HUD_FULL_WIDTH: AtomicBool = AtomicBool::new(false);

/// Enhancements "HUD Layout": Full Width (true) moves the HUD's anchored
/// elements out to the screen's edges at any width; Centered (false, the
/// default) keeps them in the 16:9 safe area ([`SAFE_SPREAD`]).
pub fn set_hud_full_width(on: bool) {
    HUD_FULL_WIDTH.store(on, Ordering::Relaxed);
}

pub fn hud_full_width() -> bool {
    HUD_FULL_WIDTH.load(Ordering::Relaxed)
}

/// How far (pixels, whole) an element anchored to the left / right edge
/// moves out from a canvas `cw` wide whose left edge is at `cx` on a `w` ×
/// `h` target, when it spreads over a centred region `spread` wide (w / h;
/// None: the whole width). A screen within 1 % of `spread` (1366 × 768
/// at 16:9) counts as that wide: its whole width.
pub fn side_reach(w: f32, h: f32, cx: f32, cw: f32, spread: Option<f32>) -> f32 {
    let region = match spread {
        Some(s) if w > h * s * 1.01 => h * s,
        _ => w,
    };
    (((region - cw) * 0.5).floor()).clamp(0.0, cx)
}

/// The widest front-end 3D scene (w / h; design choice: on wide screens the
/// menus are capped at 21:9): the garage behind the menus (main, career,
/// Customize, car lot, shops' menus) and the showcase / shop-entry menus
/// over the world show at most a centred region this wide, the sides
/// covered by dark bars that fade in ([`scene_bars`]). 2.4 so 21:9 panels
/// (2560 × 1080 = 2.37, 3440 × 1440 = 2.39) keep their whole width; only
/// wider (32:9) screens are capped. Hor+ keeps the vertical angle, so the
/// centred region of the full-width picture is the 21:9 projection's.
/// Driving, races, scenes and replays are never capped.
pub const SCENE_CAP: f32 = 2.4;

static SCENE_CAPPED: AtomicBool = AtomicBool::new(false);

/// Set per frame by the game: a front-end 3D scene is on screen.
pub fn set_scene_capped(on: bool) {
    SCENE_CAPPED.store(on, Ordering::Relaxed);
}

pub fn scene_capped() -> bool {
    SCENE_CAPPED.load(Ordering::Relaxed)
}

/// The capped scene's bars on a `size` target (Wide only): the x (pixels)
/// where the left bar ends (the right one mirrors it) and the width of
/// the fade from black to clear just inside the bar. None: no bars (not
/// wider than [`SCENE_CAP`], or Stretched).
pub fn scene_bars(size: (u32, u32)) -> Option<(f32, f32)> {
    if aspect_mode() == AspectMode::Stretched {
        return None;
    }
    scene_bars_at(size.0.max(1) as f32, size.1.max(1) as f32)
}

/// [`scene_bars`] for a `w` × `h` target in Wide mode.
pub fn scene_bars_at(w: f32, h: f32) -> Option<(f32, f32)> {
    let region = (h * SCENE_CAP).round();
    if w <= region {
        return None;
    }
    let x = ((w - region) * 0.5).floor();
    Some((x, (h * 0.04).round().min(x)))
}

/// The canvas on a target: `scale` target pixels per canvas unit (both
/// axes; x and y apart when stretched), the centred canvas's top left
/// `origin` in pixels, and `reach`, how far (pixels) an anchored element
/// moves towards an edge (the front end's: x within the 16:9 safe area),
/// `hud_reach` the HUD's x reach (the safe area's, or the screen's with
/// HUD Layout Full Width).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiFrame {
    pub size: (f32, f32),
    pub scale: (f32, f32),
    pub origin: (f32, f32),
    pub reach: (f32, f32),
    pub hud_reach: f32,
}

impl UiFrame {
    pub fn new(size: (u32, u32)) -> UiFrame {
        UiFrame::with_hud(size, hud_full_width())
    }

    /// [`UiFrame::new`] with HUD Layout Full Width `full` (or Centered).
    pub fn with_hud(size: (u32, u32), full: bool) -> UiFrame {
        let (cx, cy, cw, ch) = canvas_rect(size);
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let scale = (cw / 640.0, ch / 480.0);
        let (reach, hud_reach) = if aspect_mode() == AspectMode::Stretched {
            ((0.0, 0.0), 0.0)
        } else {
            let safe = side_reach(w, h, cx, cw, Some(SAFE_SPREAD));
            ((safe, cy), if full { side_reach(w, h, cx, cw, None) } else { safe })
        };
        UiFrame { size: (w, h), scale, origin: (cx, cy), reach, hud_reach }
    }

    /// The x reach an element placed by `place` moves by: the HUD's or
    /// the front end's.
    pub fn reach_x(&self, place: Place) -> f32 {
        if place.hud { self.hud_reach } else { self.reach.0 }
    }

    /// A canvas point of a quad placed by `place`, in target pixels.
    pub fn to_pixels(&self, p: glam::Vec2, place: Place) -> glam::Vec2 {
        glam::Vec2::new(
            self.origin.0 + p.x * self.scale.0 + place.x as f32 * self.reach_x(place),
            self.origin.1 + p.y * self.scale.1 + place.y as f32 * self.reach.1,
        )
    }

    /// A target pixel as a canvas point for elements placed by `place` (the inverse of [`Self::to_pixels`]).
    pub fn from_pixels(&self, px: glam::Vec2, place: Place) -> glam::Vec2 {
        glam::Vec2::new(
            (px.x - self.origin.0 - place.x as f32 * self.reach_x(place)) / self.scale.0,
            (px.y - self.origin.1 - place.y as f32 * self.reach.1) / self.scale.1,
        )
    }

    /// The target's rectangle, in canvas units, as `place` sees it
    /// (top left, bottom right).
    pub fn edges(&self, place: Place) -> (glam::Vec2, glam::Vec2) {
        let off = glam::Vec2::new(
            self.origin.0 + place.x as f32 * self.reach_x(place),
            self.origin.1 + place.y as f32 * self.reach.1,
        );
        let s = glam::Vec2::new(self.scale.0, self.scale.1);
        (-off / s, (glam::Vec2::new(self.size.0, self.size.1) - off) / s)
    }

    /// The canvas, moved as `place` moves it, on the target (x, y, w, h in
    /// whole pixels, clipped to the target): what lies outside it is off
    /// screen in the game and stays hidden.
    pub fn scissor(&self, place: Place) -> (u32, u32, u32, u32) {
        let a = self.to_pixels(glam::Vec2::ZERO, place).max(glam::Vec2::ZERO).round();
        let b =
            self.to_pixels(glam::Vec2::new(640.0, 480.0), place).min(glam::Vec2::new(self.size.0, self.size.1)).round();
        let (x, y) = (a.x as u32, a.y as u32);
        (x, y, (b.x as u32).saturating_sub(x), (b.y as u32).saturating_sub(y))
    }
}

/// The 640 × 480 canvas's rectangle on a `size` target (x, y, w, h in
/// pixels): the whole target when stretched, else the largest centred 4:3
/// box (rounded to whole pixels; the whole target at 4:3), which the
/// elements placed at the centre ([`Place::CENTRE`]) use.
pub fn canvas_rect(size: (u32, u32)) -> (f32, f32, f32, f32) {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    if aspect_mode() == AspectMode::Stretched {
        return (0.0, 0.0, w, h);
    }
    if size.0 * 3 > size.1 * 4 {
        let cw = (h * 4.0 / 3.0).round().min(w);
        (((w - cw) * 0.5).floor(), 0.0, cw, h)
    } else if size.0 * 3 < size.1 * 4 {
        let ch = (w * 0.75).round().min(h);
        (0.0, ((h - ch) * 0.5).floor(), w, ch)
    } else {
        (0.0, 0.0, w, h)
    }
}

