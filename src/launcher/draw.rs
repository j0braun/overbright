//! CPU drawing for the launcher: a tiny-skia pixmap, the theme's tokens,
//! alpha fills, gradients, a cached glow, the look's shapes (pills, rings,
//! circled arrows, chevrons, the rail with its dot) and text through
//! [`super::font`]. No GPU: the same pixels go to the window (softbuffer)
//! and to `--render` PNGs.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::font::{Face, Fonts, Style};
use super::ui::Rect;

pub use crate::launch::theme::{Rgb, Rgba, THEME_A, Theme};

/// The colour as tiny-skia takes it.
fn color(c: Rgba) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3)
}

/// The theme in force.
pub fn theme() -> &'static Theme {
    &THEME_A
}

/// The focus ring's width at unit `u`: 3.5 u, never under 3 px.
pub fn focus_width(u: f32) -> f32 {
    (3.5 * u).max(3.0).round()
}

/// The gap in the page's colour between a control and its focus ring.
pub fn focus_gap(u: f32) -> f32 {
    (3.0 * u).max(2.0).round()
}

/// The selection bar's inset ring: 2.5 u, never under 3 px.
pub fn select_width(u: f32) -> f32 {
    (2.5 * u).max(3.0).round()
}

/// The theme's colours under the older names (the screens' text colours).
/// Contrast against BG (WCAG 2): TEXT 17:1, DIM 12:1, PLACEHOLDER 6.5:1,
/// BORDER 6:1, DISABLED 4:1 on SURFACE.
pub mod theme {
    use super::Rgb;
    pub const BG: Rgb = super::THEME_A.page;
    /// A panel (card) on the page: the slab composited over the page.
    pub const CARD: Rgb = Rgb(0x15, 0x16, 0x14);
    pub const SURFACE: Rgb = Rgb(0x1b, 0x1d, 0x19);
    pub const SURFACE_HOVER: Rgb = Rgb(0x25, 0x28, 0x22);
    /// A control held down (mouse, Enter, pad A).
    pub const SURFACE_PRESSED: Rgb = Rgb(0x30, 0x33, 0x2c);
    /// A control's outline (fields, lists, checkboxes).
    pub const BORDER: Rgb = super::THEME_A.toggle_off_edge;
    /// Lines that only part the page.
    pub const RULE: Rgb = Rgb(0x2c, 0x2f, 0x2a);
    pub const TEXT: Rgb = super::THEME_A.ink;
    pub const DIM: Rgb = super::THEME_A.dim;
    /// Quiet text on the page itself (over the backdrop, not on a panel).
    pub const PAGE_DIM: Rgb = super::THEME_A.dim;
    pub const PLACEHOLDER: Rgb = super::THEME_A.placeholder;
    pub const DISABLED: Rgb = super::THEME_A.disabled;
    /// The one accent: titles, focus, the current page.
    pub const ACCENT: Rgb = super::THEME_A.lime;
    pub const ACCENT_HOVER: Rgb = Rgb(0xd6, 0xf6, 0xa6);
    pub const ACCENT_PRESSED: Rgb = Rgb(0xa8, 0xd0, 0x6c);
    /// Words on a filled light button (the main action).
    pub const ON_ACCENT: Rgb = super::THEME_A.on_play;
    pub const SELECTED: Rgb = super::THEME_A.sel;
    pub const OK: Rgb = super::THEME_A.ok;
    pub const WARN: Rgb = super::THEME_A.warn;
    pub const INFO: Rgb = super::THEME_A.info;
    pub const ERROR: Rgb = super::THEME_A.err;
}

fn paint(c: Rgb) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, 255);
    p.anti_alias = true;
    p
}

/// The status marks drawn as shapes (no glyph needed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Check,
    Warn,
    Info,
    Cross,
    ArrowLeft,
    ArrowRight,
}

/// A pixel canvas.
pub struct Canvas {
    pub pixmap: Pixmap,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Canvas {
        Canvas { pixmap: Pixmap::new(w.max(1), h.max(1)).expect("canvas size") }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn clear(&mut self, c: Rgb) {
        self.pixmap.fill(tiny_skia::Color::from_rgba8(c.0, c.1, c.2, 255));
    }

    /// A filled box with rounded corners (`radius` px).
    pub fn fill_round(&mut self, r: Rect, radius: f32, c: Rgb) {
        if let Some(path) = round_rect(r, radius) {
            self.pixmap.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// A rounded box's outline, `width` px wide, inside `r`.
    pub fn stroke_round(&mut self, r: Rect, radius: f32, width: f32, c: Rgb) {
        let h = width / 2.0;
        let inner = Rect { x: r.x + h, y: r.y + h, w: r.w - width, h: r.h - width };
        if let Some(path) = round_rect(inner, (radius - h).max(0.0)) {
            let stroke = Stroke { width, ..Stroke::default() };
            self.pixmap.stroke_path(&path, &paint(c), &stroke, Transform::identity(), None);
        }
    }

    pub fn fill_rect(&mut self, r: Rect, c: Rgb) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(r.x, r.y, r.w.max(0.5), r.h.max(0.5)) {
            self.pixmap.fill_rect(rect, &paint(c), Transform::identity(), None);
        }
    }

    /// An icon fitted in the square `r`.
    pub fn icon(&mut self, r: Rect, icon: Icon, c: Rgb) {
        let s = r.w.min(r.h);
        let (x, y) = (r.x + (r.w - s) / 2.0, r.y + (r.h - s) / 2.0);
        let p = |fx: f32, fy: f32| (x + fx * s, y + fy * s);
        let stroke = Stroke {
            width: (s * 0.13).max(1.5),
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        let mut pb = PathBuilder::new();
        let line = |pb: &mut PathBuilder, pts: &[(f32, f32)]| {
            pb.move_to(pts[0].0, pts[0].1);
            for q in &pts[1..] {
                pb.line_to(q.0, q.1);
            }
        };
        match icon {
            Icon::Check => line(&mut pb, &[p(0.16, 0.54), p(0.40, 0.76), p(0.84, 0.26)]),
            Icon::Cross => {
                line(&mut pb, &[p(0.22, 0.22), p(0.78, 0.78)]);
                line(&mut pb, &[p(0.78, 0.22), p(0.22, 0.78)]);
            }
            Icon::Warn => {
                line(&mut pb, &[p(0.5, 0.10), p(0.94, 0.88), p(0.06, 0.88)]);
                pb.close();
                line(&mut pb, &[p(0.5, 0.40), p(0.5, 0.60)]);
                line(&mut pb, &[p(0.5, 0.74), p(0.5, 0.75)]);
            }
            Icon::Info => {
                pb.push_circle(x + s * 0.5, y + s * 0.5, s * 0.42);
                line(&mut pb, &[p(0.5, 0.46), p(0.5, 0.72)]);
                line(&mut pb, &[p(0.5, 0.29), p(0.5, 0.30)]);
            }
            Icon::ArrowLeft | Icon::ArrowRight => {
                let pts = if icon == Icon::ArrowLeft {
                    [p(0.68, 0.18), p(0.30, 0.5), p(0.68, 0.82)]
                } else {
                    [p(0.32, 0.18), p(0.70, 0.5), p(0.32, 0.82)]
                };
                line(&mut pb, &pts);
                pb.close();
                if let Some(path) = pb.finish() {
                    self.pixmap.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
                }
                return;
            }
        }
        if let Some(path) = pb.finish() {
            self.pixmap.stroke_path(&path, &paint(c), &stroke, Transform::identity(), None);
        }
    }

    /// `text` in `face` at `size` px with its line box's top-left at
    /// (`x`, `y`).
    pub fn text(&mut self, fonts: &Fonts, face: Face, size: f32, x: f32, y: f32, text: &str, c: Rgb) {
        let (w, h) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let data = self.pixmap.data_mut();
        fonts.draw(face, size, x, y, text, |px, py, cov| {
            if px < 0 || py < 0 || px >= w || py >= h || cov <= 0.0 {
                return;
            }
            let a = cov.min(1.0);
            let i = ((py * w + px) * 4) as usize;
            for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
                let d = f32::from(data[i + k]);
                data[i + k] = (f32::from(v) * a + d * (1.0 - a)).round() as u8;
            }
            data[i + 3] = 255;
        });
    }

    /// `text` in a style (face, size, tracking) with its line box's
    /// top-left at (`x`, `y`).
    pub fn text_t(&mut self, fonts: &Fonts, style: Style, x: f32, y: f32, text: &str, c: Rgb) {
        let (w, h) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let data = self.pixmap.data_mut();
        fonts.draw_s(style, x, y, text, |px, py, cov| {
            if px < 0 || py < 0 || px >= w || py >= h || cov <= 0.0 {
                return;
            }
            let a = cov.min(1.0);
            let i = ((py * w + px) * 4) as usize;
            for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
                let d = f32::from(data[i + k]);
                data[i + k] = (f32::from(v) * a + d * (1.0 - a)).round() as u8;
            }
            data[i + 3] = 255;
        });
    }

    /// Straight RGBA `n`×`n` pixels drawn at (`x`, `y`) over the canvas.
    pub fn blit_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) {
        let (cw, ch) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let data = self.pixmap.data_mut();
        for row in 0..h as i32 {
            let py = y + row;
            if py < 0 || py >= ch {
                continue;
            }
            for col in 0..w as i32 {
                let px = x + col;
                if px < 0 || px >= cw {
                    continue;
                }
                let s = ((row * w as i32 + col) * 4) as usize;
                let a = f32::from(rgba[s + 3]) / 255.0;
                if a <= 0.0 {
                    continue;
                }
                let d = ((py * cw + px) * 4) as usize;
                for k in 0..3 {
                    let v = f32::from(rgba[s + k]) * a + f32::from(data[d + k]) * (1.0 - a);
                    data[d + k] = v.round() as u8;
                }
                data[d + 3] = 255;
            }
        }
    }

    /// Replaces the canvas's pixels by `rgba` (same size; opaque).
    pub fn fill_from(&mut self, rgba: &[u8]) {
        let d = self.pixmap.data_mut();
        if d.len() == rgba.len() {
            d.copy_from_slice(rgba);
        }
    }

    /// The pixels as 0x00RRGGBB words (softbuffer's format).
    pub fn to_xrgb(&self, out: &mut [u32]) {
        for (o, p) in out.iter_mut().zip(self.pixmap.pixels()) {
            // Every pixel is opaque: premultiplied = straight.
            *o = (u32::from(p.red()) << 16) | (u32::from(p.green()) << 8) | u32::from(p.blue());
        }
    }

    /// The pixels as an RGBA image (opaque).
    pub fn to_image(&self) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.width(), self.height(), self.pixmap.data().to_vec()).expect("pixmap size")
    }
}

fn round_rect(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    if r.w <= 0.0 || r.h <= 0.0 {
        return None;
    }
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
    // A circle's quarter as a cubic: k = 0.5523.
    let k = rad * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + rad, y0);
    pb.line_to(x1 - rad, y0);
    pb.cubic_to(x1 - rad + k, y0, x1, y0 + rad - k, x1, y0 + rad);
    pb.line_to(x1, y1 - rad);
    pb.cubic_to(x1, y1 - rad + k, x1 - rad + k, y1, x1 - rad, y1);
    pb.line_to(x0 + rad, y1);
    pb.cubic_to(x0 + rad - k, y1, x0, y1 - rad + k, x0, y1 - rad);
    pb.line_to(x0, y0 + rad);
    pb.cubic_to(x0, y0 + rad - k, x0 + rad - k, y0, x0 + rad, y0);
    pb.close();
    pb.finish()
}

fn paint_rgba(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
}

/// A gradient's colour stops: (position 0..1, colour).
pub type Stops<'a> = &'a [(f32, Rgba)];

fn stops(s: Stops) -> Vec<tiny_skia::GradientStop> {
    s.iter().map(|(p, c)| tiny_skia::GradientStop::new(*p, color(*c))).collect()
}

/// Which way a circled arrow or chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// A glow mask: the alpha of a rounded box blurred, `pad` px around it.
struct GlowMask {
    w: usize,
    h: usize,
    pad: f32,
    alpha: Vec<f32>,
}

thread_local! {
    /// Glow masks by (w, h, radius, blur) in whole px: built once per size.
    static GLOWS: std::cell::RefCell<Vec<((u32, u32, u32, u32), std::rc::Rc<GlowMask>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn glow_mask(w: f32, h: f32, radius: f32, blur: f32) -> std::rc::Rc<GlowMask> {
    let key = (w.round() as u32, h.round() as u32, radius.round() as u32, blur.round().max(1.0) as u32);
    if let Some(m) = GLOWS.with(|g| g.borrow().iter().find(|(k, _)| *k == key).map(|(_, m)| m.clone())) {
        return m;
    }
    let b = key.3 as f32;
    let pad = (b * 2.0).ceil();
    let (mw, mh) = ((key.0 as f32 + 2.0 * pad) as u32, (key.1 as f32 + 2.0 * pad) as u32);
    let mut pm = Pixmap::new(mw.max(1), mh.max(1)).expect("glow size");
    if let Some(p) = round_rect(Rect::new(pad, pad, key.0 as f32, key.1 as f32), key.2 as f32) {
        pm.fill_path(&p, &paint(Rgb(255, 255, 255)), FillRule::Winding, Transform::identity(), None);
    }
    let (w, h) = (mw as usize, mh as usize);
    let mut a: Vec<f32> = pm.pixels().iter().map(|p| f32::from(p.alpha()) / 255.0).collect();
    // Three box passes ≈ a gaussian of sigma ≈ b / 2.
    let r = ((b / 2.0).round() as usize).max(1);
    for _ in 0..3 {
        a = box_blur_1(&a, w, h, r, true);
        a = box_blur_1(&a, w, h, r, false);
    }
    let m = std::rc::Rc::new(GlowMask { w, h, pad, alpha: a });
    GLOWS.with(|g| {
        let mut g = g.borrow_mut();
        if g.len() > 32 {
            g.remove(0);
        }
        g.push((key, m.clone()));
    });
    m
}

/// One box-blur pass over a single channel (running sum, edges clamped).
fn box_blur_1(src: &[f32], w: usize, h: usize, r: usize, horizontal: bool) -> Vec<f32> {
    let mut out = vec![0.0; src.len()];
    let (n, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let k = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut acc = 0.0;
        for d in 0..=2 * r {
            let i = d as isize - r as isize;
            acc += src[at(line, i.clamp(0, n as isize - 1) as usize)];
        }
        for i in 0..n {
            out[at(line, i)] = acc * k;
            let add = (i + r + 1).min(n - 1);
            let sub = (i as isize - r as isize).clamp(0, n as isize - 1) as usize;
            acc += src[at(line, add)] - src[at(line, sub)];
        }
    }
    out
}

impl Canvas {
    /// A rounded box filled with a colour that may be translucent.
    pub fn fill_round_a(&mut self, r: Rect, radius: f32, c: Rgba) {
        if c.3 == 0 {
            return;
        }
        if let Some(path) = round_rect(r, radius) {
            self.pixmap.fill_path(&path, &paint_rgba(c), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// A rounded box's outline, `width` px wide, inside `r`, translucent.
    pub fn stroke_round_a(&mut self, r: Rect, radius: f32, width: f32, c: Rgba) {
        if c.3 == 0 || width <= 0.0 {
            return;
        }
        let h = width / 2.0;
        let inner = Rect { x: r.x + h, y: r.y + h, w: r.w - width, h: r.h - width };
        if let Some(path) = round_rect(inner, (radius - h).max(0.0)) {
            let stroke = Stroke { width, ..Stroke::default() };
            self.pixmap.stroke_path(&path, &paint_rgba(c), &stroke, Transform::identity(), None);
        }
    }

    /// A rounded box filled with a linear gradient from `a` to `b` (px).
    pub fn fill_linear(&mut self, r: Rect, radius: f32, a: (f32, f32), b: (f32, f32), s: Stops) {
        let Some(path) = round_rect(r, radius) else { return };
        let shader = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(a.0, a.1),
            tiny_skia::Point::from_xy(b.0, b.1),
            stops(s),
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        );
        if let Some(shader) = shader {
            let p = Paint { shader, anti_alias: true, ..Paint::default() };
            self.pixmap.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }

    /// `r` filled with a linear gradient at `angle_deg` as in CSS (0 =
    /// upwards, 90 = to the right, 180 = downwards), spanning the box.
    pub fn linear_gradient(&mut self, r: Rect, angle_deg: f32, s: Stops) {
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        // CSS: the gradient line's length makes the corners hit 0 and 1.
        let half = (r.w * sin.abs() + r.h * cos.abs()) / 2.0;
        let (dx, dy) = (sin * half, -cos * half);
        self.fill_linear(r, 0.0, (r.cx() - dx, r.cy() - dy), (r.cx() + dx, r.cy() + dy), s);
    }

    /// A rounded box filled with a radial gradient: an ellipse centred at
    /// `c` with radii `rx`, `ry` (px); stop 1 is its edge.
    pub fn fill_radial(&mut self, r: Rect, radius: f32, c: (f32, f32), rx: f32, ry: f32, s: Stops) {
        let Some(path) = round_rect(r, radius) else { return };
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let t = Transform::from_row(rx, 0.0, 0.0, ry, c.0, c.1);
        let shader = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, 0.0),
            1.0,
            stops(s),
            tiny_skia::SpreadMode::Pad,
            t,
        );
        if let Some(shader) = shader {
            let p = Paint { shader, anti_alias: true, ..Paint::default() };
            self.pixmap.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
    }

    /// An elliptic radial gradient centred at `centre` (radii `rx`, `ry`
    /// px; stop 1 at its edge), drawn over the ellipse's box.
    pub fn radial_gradient(&mut self, centre: (f32, f32), rx: f32, ry: f32, s: Stops) {
        let r = Rect::new(centre.0 - rx, centre.1 - ry, 2.0 * rx, 2.0 * ry);
        self.fill_radial(r, 0.0, centre, rx, ry, s);
    }

    /// The smoked slab: its fill and a 1 px light along its top edge.
    pub fn slab(&mut self, r: Rect, radius: f32, t: &Theme) {
        self.fill_round_a(r, radius, t.slab);
        self.top_edge(r, radius, t.slab_edge);
    }

    /// A 1 px line of `c` along the inside of `r`'s top edge, following
    /// its rounded corners (CSS `inset 0 1px 0`).
    pub fn top_edge(&mut self, r: Rect, radius: f32, c: Rgba) {
        if c.3 == 0 || r.h < 2.0 {
            return;
        }
        let mut pm = match Pixmap::new(r.w.ceil().max(1.0) as u32 + 2, r.h.ceil().max(1.0) as u32 + 2) {
            Some(p) => p,
            None => return,
        };
        let (fx, fy) = (r.x.fract(), r.y.fract());
        let local = Rect::new(fx, fy, r.w, r.h);
        let white = paint(Rgb(255, 255, 255));
        if let Some(p) = round_rect(local, radius) {
            pm.fill_path(&p, &white, FillRule::Winding, Transform::identity(), None);
        }
        let mut cut = paint(Rgb(0, 0, 0));
        cut.blend_mode = tiny_skia::BlendMode::DestinationOut;
        if let Some(p) = round_rect(Rect::new(fx, fy + 1.0, r.w, r.h), radius) {
            pm.fill_path(&p, &cut, FillRule::Winding, Transform::identity(), None);
        }
        let ca = f32::from(c.3) / 255.0;
        let (x0, y0) = (r.x.floor() as i32, r.y.floor() as i32);
        let (cw, ch) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let mw = pm.width() as i32;
        let mask: Vec<f32> = pm.pixels().iter().map(|p| f32::from(p.alpha()) / 255.0).collect();
        let data = self.pixmap.data_mut();
        // Only the top rows can hold the line (the corners' height).
        let rows = (radius.ceil() as i32 + 3).min(pm.height() as i32);
        for y in 0..rows {
            let py = y0 + y;
            if py < 0 || py >= ch {
                continue;
            }
            for x in 0..mw {
                let px = x0 + x;
                let a = mask[(y * mw + x) as usize] * ca;
                if px < 0 || px >= cw || a <= 0.0 {
                    continue;
                }
                let d = ((py * cw + px) * 4) as usize;
                for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
                    data[d + k] = (f32::from(v) * a + f32::from(data[d + k]) * (1.0 - a)).round() as u8;
                }
            }
        }
    }

    /// A soft glow around the rounded box `r`: its shape blurred by `blur`
    /// px in `c` (the mask is cached per size).
    pub fn glow(&mut self, r: Rect, radius: f32, blur: f32, c: Rgba) {
        if c.3 == 0 || r.w < 1.0 || r.h < 1.0 {
            return;
        }
        let m = glow_mask(r.w, r.h, radius, blur);
        let (x0, y0) = ((r.x - m.pad).round() as i32, (r.y - m.pad).round() as i32);
        let (cw, ch) = (self.pixmap.width() as i32, self.pixmap.height() as i32);
        let ca = f32::from(c.3) / 255.0;
        let col = [f32::from(c.0), f32::from(c.1), f32::from(c.2)];
        let data = self.pixmap.data_mut();
        for y in 0..m.h as i32 {
            let py = y0 + y;
            if py < 0 || py >= ch {
                continue;
            }
            for x in 0..m.w as i32 {
                let px = x0 + x;
                if px < 0 || px >= cw {
                    continue;
                }
                let a = m.alpha[y as usize * m.w + x as usize] * ca;
                if a <= 0.002 {
                    continue;
                }
                let d = ((py * cw + px) * 4) as usize;
                for k in 0..3 {
                    data[d + k] = (col[k] * a + f32::from(data[d + k]) * (1.0 - a)).round() as u8;
                }
            }
        }
    }

    /// A pill (a box with fully round ends): `fill` inside, then an
    /// outline `stroke` (width, colour) inside its edge.
    pub fn pill(&mut self, r: Rect, fill: Option<Rgba>, stroke: Option<(f32, Rgb)>) {
        let rad = r.h.min(r.w) / 2.0;
        if let Some(f) = fill {
            self.fill_round_a(r, rad, f);
        }
        if let Some((w, c)) = stroke {
            self.stroke_round_a(r, rad, w, c.rgba());
        }
    }

    /// A filled circle.
    pub fn disc(&mut self, cx: f32, cy: f32, rad: f32, c: Rgba) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, rad.max(0.1)) {
            self.pixmap.fill_path(&p, &paint_rgba(c), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// A circle's outline `width` px wide, its outer edge at `rad`.
    pub fn ring(&mut self, cx: f32, cy: f32, rad: f32, width: f32, c: Rgba) {
        let mid = (rad - width / 2.0).max(0.1);
        if let Some(p) = PathBuilder::from_circle(cx, cy, mid) {
            let stroke = Stroke { width, ..Stroke::default() };
            self.pixmap.stroke_path(&p, &paint_rgba(c), &stroke, Transform::identity(), None);
        }
    }

    /// The focus ring around the rounded box `r` (radius: the control's
    /// own): a gap in the page's colour ([`focus_gap`]), then a lime ring
    /// [`focus_width`] wide (at least 3 px at every size). Never the
    /// primary button's style: that is a fill, this is a ring outside.
    pub fn focus_ring(&mut self, r: Rect, radius: f32, u: f32, t: &Theme) {
        self.focus_ring_with(r, radius, focus_gap(u), focus_width(u), t.lime, t.page);
    }

    /// [`Canvas::focus_ring`] with its gap, width and colours given;
    /// returns the ring's outer box.
    pub fn focus_ring_with(&mut self, r: Rect, radius: f32, gap: f32, width: f32, ring: Rgb, page: Rgb) -> Rect {
        let width = width.max(3.0).round();
        let o = gap + width;
        // On whole pixels: the ring's width stays solid at every size.
        let (x0, y0) = ((r.x - o).round(), (r.y - o).round());
        let (x1, y1) = ((r.right() + o).round(), (r.bottom() + o).round());
        let outer = Rect::new(x0, y0, x1 - x0, y1 - y0);
        if gap > 0.0 {
            let g = Rect::new(r.x - gap - 0.5, r.y - gap - 0.5, r.w + 2.0 * gap + 1.0, r.h + 2.0 * gap + 1.0);
            self.stroke_round_a(g, radius + gap + 0.5, gap + 1.0, page.rgba());
        }
        self.stroke_round_a(outer, radius + o, width, ring.rgba());
        outer
    }

    /// A filled triangle pointing `dir`, fitted in the square around
    /// (`cx`, `cy`) of half-size `s`.
    pub fn triangle(&mut self, cx: f32, cy: f32, s: f32, dir: Dir, c: Rgba) {
        let pts = match dir {
            Dir::Right => [(cx - 0.62 * s, cy - s), (cx + s, cy), (cx - 0.62 * s, cy + s)],
            Dir::Left => [(cx + 0.62 * s, cy - s), (cx - s, cy), (cx + 0.62 * s, cy + s)],
            Dir::Down => [(cx - s, cy - 0.62 * s), (cx, cy + s), (cx + s, cy - 0.62 * s)],
            Dir::Up => [(cx - s, cy + 0.62 * s), (cx, cy - s), (cx + s, cy + 0.62 * s)],
        };
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        pb.line_to(pts[1].0, pts[1].1);
        pb.line_to(pts[2].0, pts[2].1);
        pb.close();
        if let Some(p) = pb.finish() {
            self.pixmap.fill_path(&p, &paint_rgba(c), FillRule::Winding, Transform::identity(), None);
        }
    }

    /// A circled arrow (◀ ▶ in a ring) around `centre`, outer radius
    /// `radius`: a ring `width` px and a filled triangle.
    pub fn circled_arrow(&mut self, centre: (f32, f32), radius: f32, dir: Dir, width: f32, c: Rgb) {
        let (cx, cy) = centre;
        self.ring(cx, cy, radius, width.max(1.0), c.rgba());
        let (nx, ny) = Self::arrow_nudge(dir, radius);
        self.triangle(cx + nx, cy + ny, radius * 0.4, dir, c.rgba());
    }

    /// A circled arrow filled in `c`, its triangle cut out in `on`.
    pub fn circled_arrow_filled(&mut self, centre: (f32, f32), radius: f32, dir: Dir, c: Rgb, on: Rgb) {
        let (cx, cy) = centre;
        self.disc(cx, cy, radius, c.rgba());
        let (nx, ny) = Self::arrow_nudge(dir, radius);
        self.triangle(cx + nx, cy + ny, radius * 0.4, dir, on.rgba());
    }

    /// A triangle's optical centre sits a little towards its point.
    fn arrow_nudge(dir: Dir, radius: f32) -> (f32, f32) {
        let n = 0.08 * radius;
        match dir {
            Dir::Left => (-n, 0.0),
            Dir::Right => (n, 0.0),
            Dir::Up => (0.0, -n),
            Dir::Down => (0.0, n),
        }
    }

    /// A chevron (› ‹ ⌄ ⌃) in `r`, stroked `width` px.
    pub fn chevron(&mut self, r: Rect, dir: Dir, width: f32, c: Rgb) {
        let s = r.w.min(r.h) / 2.0;
        let (cx, cy) = (r.cx(), r.cy());
        let (a, b, e) = match dir {
            Dir::Right => ((cx - 0.3 * s, cy - 0.6 * s), (cx + 0.3 * s, cy), (cx - 0.3 * s, cy + 0.6 * s)),
            Dir::Left => ((cx + 0.3 * s, cy - 0.6 * s), (cx - 0.3 * s, cy), (cx + 0.3 * s, cy + 0.6 * s)),
            Dir::Down => ((cx - 0.6 * s, cy - 0.3 * s), (cx, cy + 0.3 * s), (cx + 0.6 * s, cy - 0.3 * s)),
            Dir::Up => ((cx - 0.6 * s, cy + 0.3 * s), (cx, cy - 0.3 * s), (cx + 0.6 * s, cy + 0.3 * s)),
        };
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        pb.line_to(e.0, e.1);
        if let Some(p) = pb.finish() {
            let stroke = Stroke {
                width: width.max(1.0),
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Stroke::default()
            };
            self.pixmap.stroke_path(&p, &paint(c), &stroke, Transform::identity(), None);
        }
    }

    /// The rail: a 2 u line from `x0` to `x1` at `y` (its centre) and its
    /// dot at `dot_x`: a white disc ringed by the page's colour, then lime
    /// (`u`: the unit in px).
    pub fn rail(&mut self, x0: f32, x1: f32, y: f32, dot_x: f32, u: f32, t: &Theme) {
        let th = (2.0 * u).max(1.0);
        self.fill_round_a(Rect::new(x0, y - th / 2.0, (x1 - x0).max(0.0), th), 0.0, t.rail.rgba());
        let core = (4.0 * u).max(3.0);
        let gap = (2.0 * u).max(1.5);
        let ring = (1.5 * u).max(1.5);
        self.disc(dot_x, y, core + gap + ring, t.lime.rgba());
        self.disc(dot_x, y, core + gap, t.page.rgba());
        self.disc(dot_x, y, core, t.dot.rgba());
    }

    /// The left `width` px of the rounded box `r` in `c` (a message box's
    /// edge in its mark's colour, following the box's corners).
    pub fn left_edge(&mut self, r: Rect, radius: f32, width: f32, c: Rgba) {
        let (Some(path), Some(mut mask)) = (round_rect(r, radius), tiny_skia::Mask::new(self.width(), self.height()))
        else {
            return;
        };
        if let Some(clip) = tiny_skia::Rect::from_xywh(r.x, r.y, width.max(1.0), r.h).map(PathBuilder::from_rect) {
            mask.fill_path(&clip, FillRule::Winding, false, Transform::identity());
        }
        self.pixmap.fill_path(&path, &paint_rgba(c), FillRule::Winding, Transform::identity(), Some(&mask));
    }

    /// A circular arc (a spinner's stroke) around (`cx`, `cy`), from
    /// `start` through `sweep` degrees (0 = right, clockwise).
    pub fn arc(&mut self, cx: f32, cy: f32, rad: f32, start: f32, sweep: f32, width: f32, c: Rgba) {
        let mut pb = PathBuilder::new();
        let n = ((sweep.abs() / 6.0).ceil() as usize).max(2);
        for i in 0..=n {
            let a = (start + sweep * i as f32 / n as f32).to_radians();
            let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        if let Some(p) = pb.finish() {
            let stroke = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
            self.pixmap.stroke_path(&p, &paint_rgba(c), &stroke, Transform::identity(), None);
        }
    }

    /// Overbright's mark in the look: a lime ring (outer radius `rad`).
    pub fn brand_ring(&mut self, cx: f32, cy: f32, rad: f32, width: f32, t: &Theme) {
        self.ring(cx, cy, rad, width.max(2.0), t.lime.rgba());
    }
}
