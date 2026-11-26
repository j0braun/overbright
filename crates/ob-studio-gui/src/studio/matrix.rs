//! Studio's side of the screenshot-matrix kit (`ob_testkit::matrix`, docs/developing/testing.md): one page drawn
//! headless at the standard sizes and every input, with the layout's clip and overlap check and what the page
//! says it drew inside its view. A page test says how to set the page up and what else to check; it does not
//! loop over sizes itself.

use ob_look::legends::InputKind;
use ob_testkit::matrix::{self as kit, Size};

use super::Studio;
use super::layout::{self, Layout};
use super::shot::frames;

/// The three ways of driving Studio, every page is drawn with each.
pub const INPUTS: [InputKind; 3] = [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox];

/// A rectangle for the kit.
pub fn rect(r: egui::Rect) -> kit::Rect {
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

/// The scale Studio draws a window at: the system's, times Studio's own for the window's logical size.
pub fn scale_of(s: Size) -> f32 {
    let (w, h) = s.logical();
    s.scale * layout::ui_scale(w as u32, h as u32)
}

/// Whether Studio only says "too small" at this size.
pub fn too_small(s: Size) -> bool {
    let sc = scale_of(s);
    (s.w as f32 / sc) < layout::WINDOW_MIN.0 || (s.h as f32 / sc) < layout::WINDOW_MIN.1
}

/// The problems of a page: `rows` that leave the layout's view, as the kit names them.
pub fn inside_view(l: &Layout, rows: &[egui::Rect]) -> Vec<String> {
    let items: Vec<(&str, kit::Rect)> = rows.iter().map(|r| ("row", rect(*r))).collect();
    kit::clipped(rect(l.view), &items)
}

/// Draw the page `name` at every standard size with the mouse, the keys and the pad. `setup` makes the page;
/// `frames_n` frames settle it; `check` returns what else is wrong (given what was drawn). At a size under
/// Studio's smallest window only the layout's own one line is checked. The number of windows really drawn.
pub fn page(
    name: &str,
    frames_n: usize,
    mut setup: impl FnMut(Size, InputKind) -> Studio,
    mut check: impl FnMut(&mut Studio, &Layout, Size, InputKind) -> Vec<String>,
) -> usize {
    let mut drawn = 0;
    let report = kit::run(name, &kit::STANDARD, &INPUTS, |size, input| {
        let mut s = setup(size, input);
        s.nav.input = input;
        let (_, l) = frames(&mut s, (size.w, size.h), scale_of(size), frames_n);
        let mut problems = l.problems();
        if l.too_small {
            assert!(too_small(size), "{}: only the one line, yet it is not under the smallest", size.label());
        } else {
            drawn += 1;
            problems.extend(check(&mut s, &l, size, input));
        }
        problems
    });
    report.finish(name);
    drawn
}
