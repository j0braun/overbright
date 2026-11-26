//! The screenshot-matrix kit (docs/developing/testing.md): the one list of window sizes every page of the
//! launcher and of Studio is drawn at, and the fit / clip check on the rectangles a page drew. A page test names
//! its page and what to draw; the sizes, the report of every failing size at once and the geometry checks are here
//! and nowhere else.
//!
//! Plain numbers only (no egui, no launcher types), so any program's test can feed it: a rectangle is
//! `[left, top, right, bottom]` in logical pixels.

/// A window size in physical pixels and the system's display scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub w: u32,
    pub h: u32,
    pub scale: f32,
}

impl Size {
    pub const fn new(w: u32, h: u32, scale: f32) -> Size {
        Size { w, h, scale }
    }

    /// `1366x768@1.5` (the scale only when it is not 1).
    pub fn label(&self) -> String {
        if self.scale == 1.0 {
            format!("{}x{}", self.w, self.h)
        } else {
            format!("{}x{}@{}", self.w, self.h, self.scale)
        }
    }

    /// The window in logical pixels.
    pub fn logical(&self) -> (f32, f32) {
        (self.w as f32 / self.scale, self.h as f32 / self.scale)
    }
}

/// The standard matrix: Studio's smallest window, the Steam Deck, the common laptop at 150 %, 1080p, 21:9 and 4K
/// at 200 %.
pub const STANDARD: [Size; 6] = [
    Size::new(960, 600, 1.0),
    Size::new(1280, 800, 1.0),
    Size::new(1366, 768, 1.5),
    Size::new(1920, 1080, 1.0),
    Size::new(2560, 1080, 1.0),
    Size::new(3840, 2160, 2.0),
];

/// A rectangle: `[left, top, right, bottom]`.
pub type Rect = [f32; 4];

fn width(r: Rect) -> f32 {
    r[2] - r[0]
}

fn height(r: Rect) -> f32 {
    r[3] - r[1]
}

/// Whether `inner` lies inside `outer`, with `slack` pixels of rounding.
pub fn inside(outer: Rect, inner: Rect, slack: f32) -> bool {
    inner[0] >= outer[0] - slack
        && inner[1] >= outer[1] - slack
        && inner[2] <= outer[2] + slack
        && inner[3] <= outer[3] + slack
}

/// The area two rectangles share, `(w, h)`; zero sides when they do not meet.
pub fn overlap(a: Rect, b: Rect) -> (f32, f32) {
    ((a[2].min(b[2]) - a[0].max(b[0])).max(0.0), (a[3].min(b[3]) - a[1].max(b[1])).max(0.0))
}

/// What is wrong with the named `parts` of a window `window` (logical pixels): one outside the window, one with no
/// room, two that overlap (parts that are meant to lie over each other are not given here).
pub fn fit(window: (f32, f32), parts: &[(&str, Rect)]) -> Vec<String> {
    let win = [0.0, 0.0, window.0, window.1];
    let mut p = vec![];
    for (i, (n, a)) in parts.iter().enumerate() {
        if !inside(win, *a, 0.01) {
            p.push(format!("{n} {a:?} is outside the window"));
        }
        if width(*a) < 1.0 || height(*a) < 1.0 {
            p.push(format!("{n} has no room ({a:?})"));
        }
        for (m, b) in &parts[i + 1..] {
            let (ow, oh) = overlap(*a, *b);
            if ow > 0.5 && oh > 0.5 {
                p.push(format!("{n} and {m} overlap ({ow:.1} x {oh:.1})"));
            }
        }
    }
    p
}

/// What is clipped: each of `items` (a row, a graph, a card) must lie inside `page`, with a pixel of rounding.
pub fn clipped(page: Rect, items: &[(&str, Rect)]) -> Vec<String> {
    items
        .iter()
        .filter(|(_, r)| !inside(page, *r, 1.0))
        .map(|(n, r)| format!("{n} {r:?} outside the page {page:?}"))
        .collect()
}

/// The failing sizes of a run, and how many sizes were drawn.
#[derive(Default, Debug)]
pub struct Report {
    pub drawn: usize,
    pub failures: Vec<String>,
}

impl Report {
    /// Panics with every failure at once (a page broken at three sizes says so in one run).
    pub fn finish(self, page: &str) -> usize {
        assert!(
            self.failures.is_empty(),
            "{page}: {} problem(s):\n  {}",
            self.failures.len(),
            self.failures.join("\n  ")
        );
        self.drawn
    }
}

/// Draw `page` at every size of `sizes` and every `variant` (an input kind, a state; `"-"` for none): `draw`
/// returns the problems it found (empty when the page fits). Nothing stops at the first failure.
pub fn run<V: Copy + std::fmt::Debug>(
    page: &str,
    sizes: &[Size],
    variants: &[V],
    mut draw: impl FnMut(Size, V) -> Vec<String>,
) -> Report {
    let mut r = Report::default();
    for &s in sizes {
        for &v in variants {
            r.drawn += 1;
            for p in draw(s, v) {
                r.failures.push(format!("{page} {} {v:?}: {p}", s.label()));
            }
        }
    }
    r
}

/// [`run`] at [`STANDARD`], panicking on any problem; the number of drawings.
pub fn check<V: Copy + std::fmt::Debug>(page: &str, variants: &[V], draw: impl FnMut(Size, V) -> Vec<String>) -> usize {
    run(page, &STANDARD, variants, draw).finish(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_sizes() {
        let l: Vec<String> = STANDARD.iter().map(Size::label).collect();
        assert_eq!(l, ["960x600", "1280x800", "1366x768@1.5", "1920x1080", "2560x1080", "3840x2160@2"]);
        assert_eq!(STANDARD[2].logical(), (910.666_7, 512.0));
    }

    #[test]
    fn fit_finds_outside_empty_and_overlapping() {
        let ok = fit((100.0, 100.0), &[("a", [0.0, 0.0, 50.0, 100.0]), ("b", [50.0, 0.0, 100.0, 100.0])]);
        assert!(ok.is_empty(), "{ok:?}");
        let bad = fit(
            (100.0, 100.0),
            &[("a", [0.0, 0.0, 60.0, 100.0]), ("b", [50.0, 0.0, 120.0, 100.0]), ("c", [0.0, 0.0, 0.0, 5.0])],
        );
        assert_eq!(bad.len(), 3, "{bad:?}");
        assert!(bad.iter().any(|p| p.contains("outside")) && bad.iter().any(|p| p.contains("overlap")));
        assert!(bad.iter().any(|p| p.contains("no room")));
    }

    #[test]
    fn clipped_allows_a_pixel() {
        let page = [0.0, 0.0, 10.0, 10.0];
        assert!(clipped(page, &[("r", [-1.0, 0.0, 11.0, 10.0])]).is_empty());
        assert_eq!(clipped(page, &[("r", [0.0, 0.0, 12.0, 10.0])]).len(), 1);
    }

    #[test]
    fn run_reports_every_failing_size_at_once() {
        let r =
            run(
                "page",
                &STANDARD,
                &["a", "b"],
                |s, v| if s.w == 1920 && v == "b" { vec!["clipped".into()] } else { vec![] },
            );
        assert_eq!(r.drawn, 12);
        assert_eq!(r.failures, ["page 1920x1080 \"b\": clipped"]);
    }

    #[test]
    #[should_panic(expected = "2 problem(s)")]
    fn finish_panics_with_all_of_them() {
        run("p", &STANDARD, &[()], |s, _| if s.w >= 2560 { vec!["x".into()] } else { vec![] }).finish("p");
    }
}
