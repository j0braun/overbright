//! The launcher's font: Go Regular and Go Bold (BSD-3-Clause, by Bigelow &
//! Holmes for the Go project; src/launcher/fonts/LICENSE-Go-fonts.txt),
//! embedded, measured and rasterised with ab_glyph. Nothing from the game.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

static REGULAR: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/launcher/fonts/Go-Regular.ttf"));
static BOLD: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/launcher/fonts/Go-Bold.ttf"));

/// The licence text that goes with the embedded fonts (their own notice).
pub const LICENCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/launcher/fonts/README-Go-fonts.txt"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Regular,
    Bold,
}

pub struct Fonts {
    regular: FontRef<'static>,
    bold: FontRef<'static>,
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::new()
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        Fonts {
            regular: FontRef::try_from_slice(REGULAR).expect("Go Regular"),
            bold: FontRef::try_from_slice(BOLD).expect("Go Bold"),
        }
    }

    fn face(&self, f: Face) -> &FontRef<'static> {
        match f {
            Face::Regular => &self.regular,
            Face::Bold => &self.bold,
        }
    }

    /// The line box's height at `size` px (ascent − descent).
    pub fn line_height(&self, f: Face, size: f32) -> f32 {
        let s = self.face(f).as_scaled(PxScale::from(size));
        (s.ascent() - s.descent()).ceil()
    }

    /// The advance width of `text` at `size` px (kerning included).
    pub fn width(&self, f: Face, size: f32, text: &str) -> f32 {
        let s = self.face(f).as_scaled(PxScale::from(size));
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            let id = s.glyph_id(c);
            if let Some(p) = prev {
                w += s.kern(p, id);
            }
            w += s.h_advance(id);
            prev = Some(id);
        }
        w.ceil()
    }

    /// Whether every character of `text` has a glyph in `f`.
    pub fn covers(&self, f: Face, text: &str) -> bool {
        let font = self.face(f);
        text.chars().all(|c| c.is_whitespace() || font.glyph_id(c).0 != 0)
    }

    /// Rasterises `text` with its line box's top-left at (`x`, `y`):
    /// `plot(px, py, coverage)` for every covered pixel.
    pub fn draw(&self, f: Face, size: f32, x: f32, y: f32, text: &str, mut plot: impl FnMut(i32, i32, f32)) {
        let font = self.face(f);
        let s = font.as_scaled(PxScale::from(size));
        let baseline = (y + s.ascent()).round();
        let mut pen = x.round();
        let mut prev = None;
        for c in text.chars() {
            let id = s.glyph_id(c);
            if let Some(p) = prev {
                pen += s.kern(p, id);
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

    /// `text` shortened with "…" to at most `max` px wide, keeping its
    /// start (`End`), its end (`Start`) or both ends (`Middle`, for paths).
    pub fn fit(&self, f: Face, size: f32, text: &str, max: f32, cut: Cut) -> String {
        if self.width(f, size, text) <= max {
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
            if self.width(f, size, &make(mid)) <= max {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        make(lo)
    }

    /// `text` broken into lines of at most `max` px at spaces (a word
    /// longer than a line is cut with "…").
    pub fn wrap(&self, f: Face, size: f32, text: &str, max: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.width(f, size, &cand) <= max || line.is_empty() {
                    line = cand;
                } else {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            }
            lines.push(line);
        }
        lines.into_iter().map(|l| self.fit(f, size, &l, max, Cut::End)).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    End,
    Start,
    Middle,
}
