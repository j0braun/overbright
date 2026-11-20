//! The footer's button legends: what each button does on this screen,
//! drawn in the glyphs of the input the player used last, so a pad player
//! never sees keys and a keyboard player never sees pad buttons.
//!
//! Glyph sets: Xbox-style pads (A B X Y, LB RB, View), the Steam Deck
//! (A B X Y, L1 R1, View), PlayStation-style pads (cross, circle, square
//! and triangle drawn as plain shapes, L1 R1, Create) and keyboard keys.
//! Every glyph is a vector shape plus a short name; no maker's logo is
//! drawn. A mouse player sees the keys inside clickable chips.
//!
//! Sizes in layout units `u`: glyph discs 28 u, shoulder buttons 24 u high,
//! keycaps 26 u, glyph letters 16 u, labels 17 u, 26 u between items.

use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};

use super::draw::{Canvas, Dir, Rgb, Rgba, Theme};
use super::font::{Face, Fonts};
use super::platform_deck::InputKind;
use super::ui::Rect;

/// The words next to a glyph.
const LABEL_FACE: Face = Face::LabelItalic;
/// The letters inside a glyph.
const GLYPH_FACE: Face = Face::BodySemi;

// The legends' data (the buttons' roles, the glyph sets and their glyphs) is the shared look's (ob-look).
pub use ob_look::legends::{Action, Btn, Glyph, GlyphSet, Symbol, action, glyphs, hold_glyph};

/// Sizes in layout units.
mod size {
    pub const DISC: f32 = 28.0;
    pub const BUMPER_H: f32 = 24.0;
    pub const BUMPER_PAD: f32 = 8.0;
    pub const BUMPER_RADIUS: f32 = 7.0;
    pub const KEY_H: f32 = 26.0;
    pub const KEY_PAD: f32 = 7.0;
    pub const KEY_RADIUS: f32 = 5.0;
    pub const GLYPH_TEXT: f32 = 16.0;
    pub const LABEL: f32 = 17.0;
    pub const OUTLINE: f32 = 2.0;
    /// Between two glyphs of one item.
    pub const GLYPH_GAP: f32 = 4.0;
    /// Between the glyphs and the label.
    pub const LABEL_GAP: f32 = 8.0;
    /// Between items.
    pub const ITEM_GAP: f32 = 26.0;
    /// A mouse chip's padding left and right, and its height.
    pub const CHIP_PAD: f32 = 10.0;
    pub const CHIP_H: f32 = 38.0;
    /// The smallest scale the legends shrink to before dropping items.
    pub const MIN_SHRINK: f32 = 0.8;
}

/// A glyph's width at unit `u`.
pub fn glyph_width(fonts: &Fonts, g: Glyph, u: f32) -> f32 {
    match g {
        Glyph::Face(_) | Glyph::Symbol(_) | Glyph::PadArrow(_) | Glyph::Dpad | Glyph::View | Glyph::Create => {
            size::DISC * u
        }
        Glyph::Bumper(t) => fonts.width(GLYPH_FACE, size::GLYPH_TEXT * u, t) + 2.0 * size::BUMPER_PAD * u,
        Glyph::Key(t) => {
            (fonts.width(GLYPH_FACE, size::GLYPH_TEXT * u, t) + 2.0 * size::KEY_PAD * u).max(size::KEY_H * u)
        }
        Glyph::KeyArrow(_) => size::KEY_H * u,
    }
}

fn chips(kind: InputKind) -> bool {
    kind == InputKind::Mouse
}

fn item_width(fonts: &Fonts, kind: InputKind, a: &Action, u: f32) -> f32 {
    let gs = glyphs(GlyphSet::for_input(kind), a.btn);
    let mut w: f32 = gs.iter().map(|g| glyph_width(fonts, *g, u)).sum();
    w += size::GLYPH_GAP * u * gs.len().saturating_sub(1) as f32;
    if !a.label.is_empty() {
        w += size::LABEL_GAP * u + fonts.width(LABEL_FACE, size::LABEL * u, &a.label);
    }
    if chips(kind) {
        w += 2.0 * size::CHIP_PAD * u;
    }
    w
}

/// The legends' width at unit `u`.
pub fn width(fonts: &Fonts, kind: InputKind, actions: &[Action], u: f32) -> f32 {
    let items: f32 = actions.iter().map(|a| item_width(fonts, kind, a, u)).sum();
    items + size::ITEM_GAP * u * actions.len().saturating_sub(1) as f32
}

/// Fits the legends into `max` px: the unit to draw at and how many items
/// (from the start; put the most important first) are shown.
pub fn fit(fonts: &Fonts, kind: InputKind, actions: &[Action], max: f32, u: f32) -> (f32, usize) {
    let need = width(fonts, kind, actions, u);
    if need <= max {
        return (u, actions.len());
    }
    let k = (max / need).clamp(size::MIN_SHRINK, 1.0);
    if width(fonts, kind, actions, u * k) <= max {
        return (u * k, actions.len());
    }
    let small = u * size::MIN_SHRINK;
    let mut n = actions.len();
    while n > 0 && width(fonts, kind, &actions[..n], small) > max {
        n -= 1;
    }
    (small, n)
}

/// Where each shown item sits, and the unit they are drawn at.
pub fn items(fonts: &Fonts, rect: Rect, kind: InputKind, actions: &[Action], u: f32) -> (f32, Vec<(Btn, Rect)>) {
    let (u, n) = fit(fonts, kind, actions, rect.w, u);
    let shown = &actions[..n];
    let h = if chips(kind) { size::CHIP_H } else { size::DISC } * u;
    let mut x = rect.right() - width(fonts, kind, shown, u);
    let mut out = Vec::with_capacity(n);
    for a in shown {
        let w = item_width(fonts, kind, a, u);
        out.push((a.btn, Rect::new(x, rect.cy() - h / 2.0, w, h)));
        x += w + size::ITEM_GAP * u;
    }
    (u, out)
}

/// The legends right-aligned in `rect`, vertically centred.
pub fn draw(c: &mut Canvas, fonts: &Fonts, rect: Rect, kind: InputKind, actions: &[Action], u: f32, t: &Theme) {
    let set = GlyphSet::for_input(kind);
    let (u, placed) = items(fonts, rect, kind, actions, u);
    let cy = rect.cy();
    for (a, (_, r)) in actions.iter().zip(&placed) {
        let mut x = r.x;
        if chips(kind) {
            chip(c, *r, u, t);
            x += size::CHIP_PAD * u;
        }
        for (i, g) in glyphs(set, a.btn).iter().enumerate() {
            if i > 0 {
                x += size::GLYPH_GAP * u;
            }
            x += draw_glyph(c, fonts, *g, x, cy, u, t);
        }
        if !a.label.is_empty() {
            x += size::LABEL_GAP * u;
            let px = size::LABEL * u;
            let lh = fonts.line_height(LABEL_FACE, px);
            c.text(fonts, LABEL_FACE, px, x, cy - lh / 2.0, &a.label, t.ink);
        }
    }
}

/// The legend under (`x`, `y`), for a click.
pub fn hit(fonts: &Fonts, rect: Rect, kind: InputKind, actions: &[Action], u: f32, x: f32, y: f32) -> Option<Btn> {
    items(fonts, rect, kind, actions, u).1.into_iter().find(|(_, r)| r.contains(x, y)).map(|(b, _)| b)
}

fn paint_a(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

fn fill(c: &mut Canvas, pb: PathBuilder, col: Rgb) {
    if let Some(path) = pb.finish() {
        c.pixmap.fill_path(&path, &paint_a(col.a(255)), FillRule::EvenOdd, Transform::identity(), None);
    }
}

fn stroke(c: &mut Canvas, pb: PathBuilder, width: f32, col: Rgba) {
    if let Some(path) = pb.finish() {
        let s = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        c.pixmap.stroke_path(&path, &paint_a(col), &s, Transform::identity(), None);
    }
}

fn round_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = r * 0.552_284_8;
    let (x1, y1) = (x + w, y + h);
    pb.move_to(x + r, y);
    pb.line_to(x1 - r, y);
    pb.cubic_to(x1 - r + k, y, x1, y + r - k, x1, y + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x + r, y1);
    pb.cubic_to(x + r - k, y1, x, y1 - r + k, x, y1 - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
}

/// A mouse chip: the secondary button's dark fill and a faint outline.
fn chip(c: &mut Canvas, r: Rect, u: f32, t: &Theme) {
    let mut pb = PathBuilder::new();
    round_rect(&mut pb, r.x, r.y, r.w, r.h, r.h / 2.0);
    if let Some(path) = pb.finish() {
        c.pixmap.fill_path(&path, &paint_a(t.button_fill), FillRule::Winding, Transform::identity(), None);
    }
    let w = (1.5 * u).max(1.0);
    let mut pb = PathBuilder::new();
    round_rect(&mut pb, r.x + w / 2.0, r.y + w / 2.0, r.w - w, r.h - w, (r.h - w) / 2.0);
    stroke(c, pb, w, t.outline.a(0x70));
}

fn centred_text(c: &mut Canvas, fonts: &Fonts, s: &str, cx: f32, cy: f32, px: f32, col: Rgb) {
    let w = fonts.width(GLYPH_FACE, px, s);
    let lh = fonts.line_height(GLYPH_FACE, px);
    c.text(fonts, GLYPH_FACE, px, cx - w / 2.0, cy - lh / 2.0, s, col);
}

/// The unit vector a direction points along (screen y down).
fn dir_vec(d: Dir) -> (f32, f32) {
    match d {
        Dir::Left => (-1.0, 0.0),
        Dir::Right => (1.0, 0.0),
        Dir::Up => (0.0, -1.0),
        Dir::Down => (0.0, 1.0),
    }
}

/// Draws glyph `g` with its left edge at `x`, centred on `cy`; returns
/// its width. Also used for glyphs inside text (the hold note).
pub fn draw_glyph(c: &mut Canvas, fonts: &Fonts, g: Glyph, x: f32, cy: f32, u: f32, t: &Theme) -> f32 {
    let w = glyph_width(fonts, g, u);
    let cx = x + w / 2.0;
    let line = (size::OUTLINE * u).max(1.5);
    let (fill_c, on_fill, outline) = (t.play, t.on_play, t.outline);
    let disc = |c: &mut Canvas| {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, size::DISC * u / 2.0);
        fill(c, pb, fill_c);
    };
    match g {
        Glyph::Face(s) => {
            disc(c);
            centred_text(c, fonts, s, cx, cy, size::GLYPH_TEXT * u, on_fill);
        }
        Glyph::Symbol(sym) => {
            disc(c);
            let r = size::DISC * u * 0.24;
            let mut pb = PathBuilder::new();
            match sym {
                Symbol::Cross => {
                    let r = r * 0.85;
                    pb.move_to(cx - r, cy - r);
                    pb.line_to(cx + r, cy + r);
                    pb.move_to(cx + r, cy - r);
                    pb.line_to(cx - r, cy + r);
                }
                Symbol::Circle => pb.push_circle(cx, cy, r * 1.05),
                Symbol::Square => round_rect(&mut pb, cx - r * 0.9, cy - r * 0.9, r * 1.8, r * 1.8, 0.0),
                Symbol::Triangle => {
                    let rr = r * 1.15;
                    pb.move_to(cx, cy - rr);
                    pb.line_to(cx + rr * 0.92, cy + rr * 0.6);
                    pb.line_to(cx - rr * 0.92, cy + rr * 0.6);
                    pb.close();
                }
            }
            stroke(c, pb, line, on_fill.a(255));
        }
        Glyph::PadArrow(d) => {
            disc(c);
            let r = size::DISC * u * 0.22;
            let (dx, dy) = dir_vec(d);
            let mut pb = PathBuilder::new();
            pb.move_to(cx + dx * r, cy + dy * r);
            pb.line_to(cx - dx * r * 0.6 - dy * r * 0.9, cy - dy * r * 0.6 + dx * r * 0.9);
            pb.line_to(cx - dx * r * 0.6 + dy * r * 0.9, cy - dy * r * 0.6 - dx * r * 0.9);
            pb.close();
            fill(c, pb, on_fill);
        }
        Glyph::Dpad => {
            disc(c);
            let (a, b) = (8.0 * u, 2.8 * u);
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, cx - a, cy - b, 2.0 * a, 2.0 * b, 1.0 * u);
            fill(c, pb, on_fill);
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, cx - b, cy - a, 2.0 * b, 2.0 * a, 1.0 * u);
            fill(c, pb, on_fill);
        }
        Glyph::View => {
            disc(c);
            // Two overlapping frames, 10×8 u, offset 4 u; the front one
            // hides the back one where they overlap.
            let (fw, fh, o) = (10.0 * u, 8.0 * u, 4.0 * u);
            let (x0, y0) = (cx - (fw + o) / 2.0, cy - (fh + o) / 2.0);
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, x0, y0, fw, fh, 2.0 * u);
            stroke(c, pb, line, on_fill.a(255));
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, x0 + o, y0 + o, fw, fh, 2.0 * u);
            fill(c, pb, fill_c);
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, x0 + o, y0 + o, fw, fh, 2.0 * u);
            stroke(c, pb, line, on_fill.a(255));
        }
        Glyph::Create => {
            disc(c);
            let (bw, bh) = (12.0 * u, 6.0 * u);
            let by = cy + 1.0 * u;
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, cx - bw / 2.0, by, bw, bh, bh / 2.0);
            stroke(c, pb, line, on_fill.a(255));
            let mut pb = PathBuilder::new();
            for dx in [-4.0, 0.0, 4.0] {
                pb.move_to(cx + dx * u, by - 3.0 * u);
                pb.line_to(cx + dx * u * 1.2, by - 7.0 * u);
            }
            stroke(c, pb, line, on_fill.a(255));
        }
        Glyph::Bumper(s) | Glyph::Key(s) => {
            let (h, r) = if matches!(g, Glyph::Bumper(_)) {
                (size::BUMPER_H * u, size::BUMPER_RADIUS * u)
            } else {
                (size::KEY_H * u, size::KEY_RADIUS * u)
            };
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, x + line / 2.0, cy - h / 2.0 + line / 2.0, w - line, h - line, r);
            stroke(c, pb, line, outline.a(255));
            centred_text(c, fonts, s, cx, cy, size::GLYPH_TEXT * u, outline);
        }
        Glyph::KeyArrow(d) => {
            let h = size::KEY_H * u;
            let mut pb = PathBuilder::new();
            round_rect(&mut pb, x + line / 2.0, cy - h / 2.0 + line / 2.0, w - line, h - line, size::KEY_RADIUS * u);
            stroke(c, pb, line, outline.a(255));
            // A drawn arrow (shaft and head): the fonts need not carry ←→.
            let (dx, dy) = dir_vec(d);
            let (a, k) = (6.0 * u, 4.0 * u);
            let tip = (cx + dx * a, cy + dy * a);
            let mut pb = PathBuilder::new();
            pb.move_to(cx - dx * a, cy - dy * a);
            pb.line_to(tip.0, tip.1);
            pb.move_to(tip.0 - dx * k - dy * k, tip.1 - dy * k + dx * k);
            pb.line_to(tip.0, tip.1);
            pb.line_to(tip.0 - dx * k + dy * k, tip.1 - dy * k - dx * k);
            stroke(c, pb, line, outline.a(255));
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::super::draw::THEME_A;
    use super::*;

    const ALL_BTNS: [Btn; 7] = [Btn::Confirm, Btn::Back, Btn::X, Btn::Y, Btn::Pages, Btn::Move, Btn::Change];
    const ALL_SETS: [GlyphSet; 4] = [GlyphSet::Xbox, GlyphSet::Deck, GlyphSet::PlayStation, GlyphSet::Keys];
    const ALL_KINDS: [InputKind; 5] =
        [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox, InputKind::PlayStation, InputKind::Deck];

    fn is_key(g: &Glyph) -> bool {
        matches!(g, Glyph::Key(_) | Glyph::KeyArrow(_))
    }

    fn main_actions() -> Vec<Action> {
        vec![action(Btn::Confirm, "Select"), action(Btn::Back, "Quit"), action(Btn::Pages, "Pages")]
    }

    #[test]
    fn last_input_picks_the_set() {
        assert_eq!(GlyphSet::for_input(InputKind::Xbox), GlyphSet::Xbox);
        assert_eq!(GlyphSet::for_input(InputKind::Deck), GlyphSet::Deck);
        assert_eq!(GlyphSet::for_input(InputKind::PlayStation), GlyphSet::PlayStation);
        assert_eq!(GlyphSet::for_input(InputKind::Keyboard), GlyphSet::Keys);
        assert_eq!(GlyphSet::for_input(InputKind::Mouse), GlyphSet::Keys);
    }

    #[test]
    fn a_pad_player_never_sees_keys_and_back() {
        for set in ALL_SETS {
            for b in ALL_BTNS {
                let gs = glyphs(set, b);
                assert!(!gs.is_empty(), "{set:?} {b:?}");
                assert_eq!(gs.iter().any(is_key), !set.is_pad(), "{set:?} {b:?}: {gs:?}");
            }
            assert_eq!(is_key(&hold_glyph(set)), !set.is_pad(), "{set:?}");
        }
    }

    #[test]
    fn sets_name_their_own_buttons() {
        assert_eq!(glyphs(GlyphSet::Xbox, Btn::Pages), [Glyph::Bumper("LB"), Glyph::Bumper("RB")]);
        assert_eq!(glyphs(GlyphSet::Deck, Btn::Pages), [Glyph::Bumper("L1"), Glyph::Bumper("R1")]);
        assert_eq!(glyphs(GlyphSet::PlayStation, Btn::Confirm), [Glyph::Symbol(Symbol::Cross)]);
        assert_eq!(glyphs(GlyphSet::PlayStation, Btn::Back), [Glyph::Symbol(Symbol::Circle)]);
        assert_eq!(hold_glyph(GlyphSet::Deck), Glyph::View);
        assert_eq!(hold_glyph(GlyphSet::Keys), Glyph::Key("Shift"));
        assert_eq!(glyphs(GlyphSet::Keys, Btn::Back), [Glyph::Key("Esc")]);
        assert_eq!(glyphs(GlyphSet::Keys, Btn::Pages), [Glyph::Key("Q"), Glyph::Key("E")]);
        for set in [GlyphSet::Xbox, GlyphSet::Deck, GlyphSet::PlayStation] {
            let f: Vec<_> = [Btn::Confirm, Btn::Back, Btn::X, Btn::Y].iter().map(|b| glyphs(set, *b)[0]).collect();
            for i in 0..4 {
                for j in i + 1..4 {
                    assert_ne!(f[i], f[j], "{set:?}");
                }
            }
        }
    }

    fn bbox(c: &Canvas, bg: Rgb) -> Option<(u32, u32, u32, u32)> {
        let w = c.width();
        let mut b: Option<(u32, u32, u32, u32)> = None;
        for (i, p) in c.pixmap.pixels().iter().enumerate() {
            if (p.red(), p.green(), p.blue()) != (bg.0, bg.1, bg.2) {
                let (x, y) = (i as u32 % w, i as u32 / w);
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a, b2, c2, d)) => (a.min(x), b2.min(y), c2.max(x), d.max(y)),
                });
            }
        }
        b
    }

    #[test]
    fn draws_right_aligned_inside_the_bar_for_every_input() {
        let fonts = Fonts::new();
        let t = &THEME_A;
        let acts = main_actions();
        for kind in ALL_KINDS {
            for (w, h) in [(640u32, 480u32), (1280, 800), (1366, 768), (1920, 1080), (3840, 2160)] {
                let u = (w as f32 / 1366.0).min(h as f32 / 768.0);
                let mut c = Canvas::new(w, h);
                c.clear(t.page);
                let bar_h = 64.0 * u;
                let rect = Rect::new(w as f32 * 0.3, h as f32 - bar_h - 4.0, w as f32 * 0.65, bar_h);
                draw(&mut c, &fonts, rect, kind, &acts, u, t);
                let (_, placed) = items(&fonts, rect, kind, &acts, u);
                assert_eq!(placed.len(), 3, "{kind:?} {w}x{h}");
                let (x0, y0, x1, y1) = bbox(&c, t.page).expect("something drawn");
                assert!(x0 as f32 >= rect.x - 1.0 && x1 as f32 <= rect.right() + 1.0, "{kind:?} {w}x{h}");
                assert!(y0 as f32 >= rect.y - 1.0 && y1 as f32 <= rect.bottom() + 1.0, "{kind:?} {w}x{h}");
                assert!((rect.right() - placed[2].1.right()).abs() < 1.0, "{kind:?} {w}x{h}");
                for p in placed.windows(2) {
                    assert!(p[0].1.right() < p[1].1.x);
                }
                let mid = placed[1].1;
                assert_eq!(hit(&fonts, rect, kind, &acts, u, mid.cx(), mid.cy()), Some(Btn::Back));
                assert_eq!(hit(&fonts, rect, kind, &acts, u, rect.x - 5.0, mid.cy()), None);
            }
        }
    }

    #[test]
    fn mouse_gets_chips_keys_do_not() {
        let fonts = Fonts::new();
        let acts = main_actions();
        let k = width(&fonts, InputKind::Keyboard, &acts, 1.0);
        let m = width(&fonts, InputKind::Mouse, &acts, 1.0);
        assert!((m - k - 3.0 * 2.0 * size::CHIP_PAD).abs() < 0.01);
    }

    #[test]
    fn a_narrow_bar_shrinks_then_drops_the_last_items() {
        let fonts = Fonts::new();
        let kind = InputKind::Keyboard;
        let acts = main_actions();
        let full = width(&fonts, kind, &acts, 1.0);
        assert_eq!(fit(&fonts, kind, &acts, full + 1.0, 1.0), (1.0, 3));
        let (u, n) = fit(&fonts, kind, &acts, full * 0.9, 1.0);
        assert!(n == 3 && (size::MIN_SHRINK..1.0).contains(&u));
        assert!(fit(&fonts, kind, &acts, full * 0.5, 1.0).1 < 3);
        assert_eq!(fit(&fonts, kind, &acts, 1.0, 1.0).1, 0);
        let r = Rect::new(100.0, 0.0, full * 0.5, 64.0);
        for (_, a) in items(&fonts, r, kind, &acts, 1.0).1 {
            assert!(a.x >= r.x - 0.5 && a.right() <= r.right() + 0.5);
        }
    }

    #[test]
    fn every_glyph_draws_inside_its_slot() {
        let fonts = Fonts::new();
        let t = &THEME_A;
        let u = 1.5;
        let mut all: Vec<Glyph> = Vec::new();
        for set in ALL_SETS {
            for b in ALL_BTNS {
                all.extend_from_slice(glyphs(set, b));
            }
            all.push(hold_glyph(set));
        }
        for g in all {
            let mut c = Canvas::new(200, 80);
            c.clear(t.page);
            let w = draw_glyph(&mut c, &fonts, g, 50.0, 40.0, u, t);
            assert_eq!(w, glyph_width(&fonts, g, u));
            let (x0, y0, x1, y1) = bbox(&c, t.page).unwrap_or_else(|| panic!("{g:?} drew nothing"));
            assert!(x0 >= 49 && (x1 as f32) <= 51.0 + w, "{g:?}");
            let half = size::DISC * u / 2.0 + 1.0;
            assert!(y0 as f32 >= 40.0 - half && y1 as f32 <= 40.0 + half, "{g:?}");
        }
    }
}
