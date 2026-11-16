//! CPU drawing for the launcher: a tiny-skia pixmap, the plain dark theme,
//! shapes (boxes, marks, arrows) and text through [`super::font`]. No GPU:
//! the same pixels go to the window (softbuffer) and to `--render` PNGs.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::font::{Face, Fonts};
use super::ui::Rect;

/// A straight (not premultiplied) RGB colour, opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The launcher's colours: dark, one accent (focus, the main button), and
/// the status marks' own colours. Contrast against BG (WCAG 2): TEXT 15:1,
/// DIM 7:1, PLACEHOLDER 5.4:1, a control's outline (BORDER) 3:1, DISABLED
/// 3.4:1 on SURFACE (readable, yet clearly not usable).
pub mod theme {
    use super::Rgb;
    pub const BG: Rgb = Rgb(0x12, 0x14, 0x18);
    /// A panel (card) on the page: the hero, the start settings, Mods.
    pub const CARD: Rgb = Rgb(0x17, 0x1a, 0x20);
    pub const SURFACE: Rgb = Rgb(0x1c, 0x1f, 0x26);
    pub const SURFACE_HOVER: Rgb = Rgb(0x25, 0x29, 0x32);
    /// A control held down (mouse, Enter, pad A).
    pub const SURFACE_PRESSED: Rgb = Rgb(0x2e, 0x33, 0x3e);
    /// A control's outline (buttons, fields, lists, cycles, checkboxes).
    pub const BORDER: Rgb = Rgb(0x5a, 0x62, 0x73);
    /// Lines that only part the page (rules, the log box, a disabled
    /// control's outline).
    pub const RULE: Rgb = Rgb(0x34, 0x39, 0x44);
    pub const TEXT: Rgb = Rgb(0xe8, 0xea, 0xee);
    pub const DIM: Rgb = Rgb(0x9a, 0xa1, 0xac);
    /// Quiet text on the page itself (over the backdrop, not on a panel):
    /// lighter than DIM so it keeps 4.5:1 over the game's picture.
    pub const PAGE_DIM: Rgb = Rgb(0xd4, 0xd8, 0xde);
    /// An empty text field's hint.
    pub const PLACEHOLDER: Rgb = Rgb(0x85, 0x8b, 0x96);
    pub const DISABLED: Rgb = Rgb(0x6b, 0x71, 0x7b);
    /// The one accent, the amber of Overbright's icon ring: focus ring,
    /// the main button, selection, the wordmark's rule.
    pub const ACCENT: Rgb = Rgb(0xff, 0xa1, 0x2e);
    pub const ACCENT_HOVER: Rgb = Rgb(0xff, 0xb5, 0x5a);
    pub const ACCENT_PRESSED: Rgb = Rgb(0xd9, 0x84, 0x1c);
    pub const ON_ACCENT: Rgb = Rgb(0x1c, 0x11, 0x02);
    pub const SELECTED: Rgb = Rgb(0x3a, 0x2c, 0x17);
    pub const OK: Rgb = Rgb(0x63, 0xc0, 0x8b);
    pub const WARN: Rgb = Rgb(0xe2, 0xb0, 0x4e);
    pub const INFO: Rgb = Rgb(0x93, 0xa6, 0xc2);
    pub const ERROR: Rgb = Rgb(0xe8, 0x6c, 0x6c);
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
