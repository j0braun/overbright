//! The launcher's type: Sofia Sans Semi Condensed (SIL OFL 1.1, the Sofia
//! Sans Project Authors; src/launcher/fonts/OFL.txt), five static instances
//! cut from the variable fonts by tools/cut_fonts.py (subset to the Latin the
//! launcher needs, GPOS pair kerning flattened into a `kern` table, which is
//! the only kerning ab_glyph reads). Embedded, measured and rasterised with
//! ab_glyph. Nothing from the game.
//!
//! Two size conventions:
//! - [`Style`] (`text(face, px, tracking)`, `*_s` calls, [`Fonts::cap_height`]):
//!   `px` is the em size, as CSS `font-size` and the look-A mockups use it;
//!   tracking is in em, as CSS `letter-spacing`.
//! - The older calls (`line_height`, `width`, `draw`, `fit`, `wrap`) keep
//!   today's meaning: `size` is the line box (ascent − descent), no tracking.
//!   For this font the line box is 1.2 em.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

macro_rules! face_file {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/launcher/fonts/SofiaSansSemiCondensed-",
            $name,
            ".ttf"
        ))
    };
}

static BODY: &[u8] = face_file!("Body");
static BODY_SEMI: &[u8] = face_file!("BodySemi");
static LABEL_ITALIC: &[u8] = face_file!("LabelItalic");
static TITLE_ITALIC: &[u8] = face_file!("TitleItalic");
static PLAY_ITALIC: &[u8] = face_file!("PlayItalic");

/// The licence text that goes with the embedded fonts (their own notice).
pub const LICENCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/launcher/fonts/OFL.txt"));

/// The launcher's faces (all Sofia Sans Semi Condensed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// 450 upright: reading text.
    Body,
    /// 600 upright: status, values.
    BodySemi,
    /// 600 italic: page names, row labels, buttons, legends.
    LabelItalic,
    /// 700 italic: page titles, the brand.
    TitleItalic,
    /// 800 italic: Play.
    PlayItalic,
}

impl Face {
    /// Old name, until every caller has moved to the A faces.
    #[allow(non_upper_case_globals)]
    pub const Regular: Face = Face::Body;
    /// Old name, until every caller has moved to the A faces.
    #[allow(non_upper_case_globals)]
    pub const Bold: Face = Face::BodySemi;

    pub const ALL: [Face; 5] = [Face::Body, Face::BodySemi, Face::LabelItalic, Face::TitleItalic, Face::PlayItalic];
}

/// A face at an em size (`px`, CSS `font-size`) with tracking in em (CSS
/// `letter-spacing`, added between characters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub face: Face,
    pub px: f32,
    pub tracking: f32,
}

/// `Style { face, px, tracking }`.
pub fn text(face: Face, px: f32, tracking: f32) -> Style {
    Style { face, px, tracking }
}

impl Style {
    /// The same style at another em size.
    pub fn at(self, px: f32) -> Style {
        Style { px, ..self }
    }
}

pub struct Fonts {
    faces: [FontRef<'static>; 5],
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::new()
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        let f = |b: &'static [u8]| FontRef::try_from_slice(b).expect("embedded launcher font");
        Fonts { faces: [f(BODY), f(BODY_SEMI), f(LABEL_ITALIC), f(TITLE_ITALIC), f(PLAY_ITALIC)] }
    }

    fn face(&self, f: Face) -> &FontRef<'static> {
        &self.faces[f as usize]
    }

    /// ab_glyph's scale (line-box height) for an em size.
    fn em_scale(&self, f: Face, px: f32) -> f32 {
        let font = self.face(f);
        px * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0)
    }

    /// The line-box size (the older calls' `size`) for a style's em size.
    pub fn box_size(&self, s: Style) -> f32 {
        self.em_scale(s.face, s.px)
    }

    // ---- Style calls (em size, tracking) ----

    /// The line box's height (ascent − descent) of `s`.
    pub fn line_height_s(&self, s: Style) -> f32 {
        self.line_height(s.face, self.box_size(s))
    }

    /// The ascent above the baseline of `s` (the baseline sits this far below
    /// the `y` that `draw_s` takes).
    pub fn ascent_s(&self, s: Style) -> f32 {
        self.face(s.face).as_scaled(PxScale::from(self.box_size(s))).ascent()
    }

    /// The advance width of `text` in `s` (kerning and tracking included;
    /// tracking only between characters, so centring stays exact).
    pub fn width_s(&self, s: Style, text: &str) -> f32 {
        self.advance(s.face, self.box_size(s), s.tracking * s.px, text).ceil()
    }

    /// Rasterises `text` in `s` with its line box's top-left at (`x`, `y`):
    /// `plot(px, py, coverage)` for every covered pixel.
    pub fn draw_s(&self, s: Style, x: f32, y: f32, text: &str, plot: impl FnMut(i32, i32, f32)) {
        self.raster(s.face, self.box_size(s), s.tracking * s.px, x, y, text, plot)
    }

    /// `fit` for a style.
    pub fn fit_s(&self, s: Style, text: &str, max: f32, cut: Cut) -> String {
        self.fit_with(text, max, cut, |t| self.width_s(s, t))
    }

    /// `wrap` for a style.
    pub fn wrap_s(&self, s: Style, text: &str, max: f32) -> Vec<String> {
        self.wrap_with(text, max, |t| self.width_s(s, t))
    }

    /// The height of a capital (the outline of "H") at em size `px`, in
    /// pixels: for the smallest-glyph check.
    pub fn cap_height(&self, f: Face, px: f32) -> f32 {
        let font = self.face(f);
        let upem = font.units_per_em().unwrap_or(1000.0);
        font.outline(font.glyph_id('H')).map_or(0.7 * px, |o| (o.bounds.max.y - o.bounds.min.y).abs() * px / upem)
    }

    // ---- Older calls (line-box size, no tracking) ----

    /// The line box's height at `size` px (ascent − descent).
    pub fn line_height(&self, f: Face, size: f32) -> f32 {
        let s = self.face(f).as_scaled(PxScale::from(size));
        (s.ascent() - s.descent()).ceil()
    }

    /// The advance width of `text` at `size` px (kerning included).
    pub fn width(&self, f: Face, size: f32, text: &str) -> f32 {
        self.advance(f, size, 0.0, text).ceil()
    }

    /// Whether every character of `text` has a glyph in `f`.
    pub fn covers(&self, f: Face, text: &str) -> bool {
        let font = self.face(f);
        text.chars().all(|c| c.is_whitespace() || font.glyph_id(c).0 != 0)
    }

    /// Rasterises `text` with its line box's top-left at (`x`, `y`):
    /// `plot(px, py, coverage)` for every covered pixel.
    pub fn draw(&self, f: Face, size: f32, x: f32, y: f32, text: &str, plot: impl FnMut(i32, i32, f32)) {
        self.raster(f, size, 0.0, x, y, text, plot)
    }

    /// `text` shortened with "…" to at most `max` px wide, keeping its
    /// start (`End`), its end (`Start`) or both ends (`Middle`, for paths).
    pub fn fit(&self, f: Face, size: f32, text: &str, max: f32, cut: Cut) -> String {
        self.fit_with(text, max, cut, |t| self.width(f, size, t))
    }

    /// `text` broken into lines of at most `max` px at spaces (a word
    /// longer than a line is cut with "…").
    pub fn wrap(&self, f: Face, size: f32, text: &str, max: f32) -> Vec<String> {
        self.wrap_with(text, max, |t| self.width(f, size, t))
    }

    // ---- shared ----

    fn advance(&self, f: Face, size: f32, track_px: f32, text: &str) -> f32 {
        let s = self.face(f).as_scaled(PxScale::from(size));
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            let id = s.glyph_id(c);
            if let Some(p) = prev {
                w += s.kern(p, id) + track_px;
            }
            w += s.h_advance(id);
            prev = Some(id);
        }
        w
    }

    #[allow(clippy::too_many_arguments)]
    fn raster(
        &self,
        f: Face,
        size: f32,
        track_px: f32,
        x: f32,
        y: f32,
        text: &str,
        mut plot: impl FnMut(i32, i32, f32),
    ) {
        let font = self.face(f);
        let s = font.as_scaled(PxScale::from(size));
        let baseline = (y + s.ascent()).round();
        let mut pen = x.round();
        let mut prev = None;
        for c in text.chars() {
            let id = s.glyph_id(c);
            if let Some(p) = prev {
                pen += s.kern(p, id) + track_px;
            }
            let g = id.with_scale_and_position(PxScale::from(size), point(pen, baseline));
            pen += s.h_advance(id);
            prev = Some(id);
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| plot(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, cov));
            }
        }
    }

    fn fit_with(&self, text: &str, max: f32, cut: Cut, width: impl Fn(&str) -> f32) -> String {
        if width(text) <= max {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        // The longest cut that fits (binary search on the kept count).
        let make = |keep: usize| -> String {
            match cut {
                Cut::End => chars[..keep].iter().collect::<String>() + "…",
                Cut::Start => "…".to_string() + &chars[chars.len() - keep..].iter().collect::<String>(),
                Cut::Middle => {
                    let head = keep / 2;
                    let tail = keep - head;
                    chars[..head].iter().collect::<String>()
                        + "…"
                        + &chars[chars.len() - tail..].iter().collect::<String>()
                }
            }
        };
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if width(&make(mid)) <= max {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        make(lo)
    }

    fn wrap_with(&self, text: &str, max: f32, width: impl Fn(&str) -> f32) -> Vec<String> {
        let mut lines = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if width(&cand) <= max || line.is_empty() {
                    line = cand;
                } else {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            }
            lines.push(line);
        }
        lines.into_iter().map(|l| self.fit_with(&l, max, Cut::End, &width)).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    End,
    Start,
    Middle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_load_and_cover_european_latin() {
        let f = Fonts::new();
        let sample = "Überprüfen · Paramètres · Configuración · Zażółć gęślą jaźń · Příliš · Şçğ · Ș ț · ß ẞ \
                      1920×1080 … – — “” ‘’ « » ‹ › ← → ↑ ↓ € © ½ −";
        for face in Face::ALL {
            assert!(f.covers(face, sample), "{face:?}");
        }
    }

    #[test]
    fn kerning_is_read() {
        let f = Fonts::new();
        for face in Face::ALL {
            let s = f.face(face).as_scaled(PxScale::from(100.0));
            let k = s.kern(s.glyph_id('A'), s.glyph_id('V'));
            assert!(k < -1.0, "{face:?}: A–V kern {k}");
            assert!(f.width(face, 100.0, "AV") < f.width(face, 100.0, "A") + f.width(face, 100.0, "V") - 1.0);
        }
    }

    #[test]
    fn styles_use_em_size_and_tracking() {
        let f = Fonts::new();
        let s = text(Face::Body, 20.0, 0.0);
        // The line box is 1.2 em; a cap is about 0.655 em.
        assert!((f.line_height_s(s) - 24.0).abs() <= 1.0, "{}", f.line_height_s(s));
        let cap = f.cap_height(Face::Body, 20.0);
        assert!((12.5..14.0).contains(&cap), "{cap}");
        // Tracking: 0.1 em between each of the 4 gaps of "ABCDE" = 8 px.
        let plain = f.width_s(s, "ABCDE");
        let wide = f.width_s(text(Face::Body, 20.0, 0.1), "ABCDE");
        assert!((wide - plain - 8.0).abs() <= 1.0, "{plain} {wide}");
        // Heavier faces are wider.
        assert!(f.width_s(s.at(40.0), "Play") < f.width_s(text(Face::PlayItalic, 40.0, 0.0), "Play"));
        let mut n = 0;
        f.draw_s(text(Face::TitleItalic, 30.0, 0.02), 0.0, 0.0, "Settings", |_, _, c| n += (c > 0.5) as usize);
        assert!(n > 100);
    }
}
