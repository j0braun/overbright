//! Studio's drawn pieces, in the shared look (ob-look through the editor's theme): lamps, the three source marks,
//! key caps and pad glyphs, the rail's icons. Painted as shapes: the faces are cut to Latin and have no ○ ● ◆.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
use ob_look::lamp::Mark;
use ob_look::legends::{Dir, Glyph, Symbol};

use crate::editor::theme::{self, LABEL, SEMI, font, rgb, tokens};

use super::workspace::Workspace;

/// Where a value comes from (Changes, the Inspector, the tree).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The original game's: a `dim` ring.
    Original,
    /// This mod's: a `lime` disc.
    Yours,
    /// Another mod's, which wins in the player's game: a `streak` diamond.
    AnotherMod,
}

/// One of the three source marks, centred at `c`, `d` across.
pub fn source_mark(p: &Painter, c: Pos2, d: f32, s: Source) {
    let t = tokens();
    match s {
        Source::Original => {
            p.circle_stroke(c, d / 2.0 - 0.75, Stroke::new(1.5, rgb(t.dim)));
        }
        Source::Yours => {
            p.circle_filled(c, d / 2.0, rgb(t.lime));
        }
        Source::AnotherMod => {
            let r = d / 2.0 + 0.5;
            p.add(Shape::convex_polygon(
                vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)],
                rgb(t.streak),
                Stroke::NONE,
            ));
        }
    }
}

/// A status lamp centred at `c` (`time`: the busy lamp's turn, seconds).
pub fn lamp(p: &Painter, c: Pos2, d: f32, m: Mark, time: f32) {
    let t = tokens();
    let col = match m {
        Mark::Ok => rgb(t.ok),
        Mark::Warn => rgb(t.warn),
        Mark::Info => rgb(t.info),
        Mark::Error => rgb(t.err),
        Mark::Busy => {
            let r = d / 2.0 - 1.0;
            let a0 = time * TAU;
            let pts: Vec<Pos2> =
                (0..=16).map(|i| a0 + i as f32 / 16.0 * 1.5 * PI).map(|a| c + vec2(a.cos(), a.sin()) * r).collect();
            p.add(Shape::line(pts, Stroke::new(1.6, rgb(t.dim))));
            return;
        }
    };
    p.circle_filled(c, d / 2.0, col);
}

/// A lamp with a check (a verdict card's "ready").
pub fn check_lamp(p: &Painter, c: Pos2, d: f32) {
    let t = tokens();
    p.circle_filled(c, d / 2.0, rgb(t.ok));
    let k = d / 2.0;
    p.add(Shape::line(
        vec![c + vec2(-0.45 * k, 0.02 * k), c + vec2(-0.12 * k, 0.35 * k), c + vec2(0.45 * k, -0.3 * k)],
        Stroke::new(2.0, rgb(t.on_mark)),
    ));
}

/// The text faces the glyphs use.
fn cap_font() -> FontId {
    font(SEMI, 12.0)
}

/// A glyph's width.
pub fn glyph_width(p: &Painter, g: Glyph) -> f32 {
    let text_w = |s: &str| p.layout_no_wrap(s.to_string(), cap_font(), Color32::WHITE).size().x;
    match g {
        Glyph::Face(_)
        | Glyph::Symbol(_)
        | Glyph::PadArrow(_)
        | Glyph::Dpad
        | Glyph::View
        | Glyph::Create
        | Glyph::Start => 22.0,
        Glyph::Bumper(s) | Glyph::Trigger(s) | Glyph::Stick(s) => text_w(s) + 14.0,
        Glyph::Key(s) => (text_w(s) + 12.0).max(22.0),
        Glyph::KeyArrow(_) => 22.0,
    }
}

/// Draw `g` with its left edge at `x`, centred on `cy`; returns its width. Pad faces are filled `play` discs; keys,
/// bumpers, triggers and sticks are outlined caps (the launcher's legends).
pub fn glyph(p: &Painter, g: Glyph, x: f32, cy: f32) -> f32 {
    let t = tokens();
    let w = glyph_width(p, g);
    let c = pos2(x + w / 2.0, cy);
    let (fill, on, edge) = (rgb(t.play), rgb(t.on_play), rgb(t.outline));
    let line = Stroke::new(1.5, on);
    let disc = || p.circle_filled(c, 11.0, fill);
    match g {
        Glyph::Face(s) => {
            disc();
            p.text(c, Align2::CENTER_CENTER, s, font(SEMI, 13.0), on);
        }
        Glyph::Symbol(sym) => {
            disc();
            let r = 5.0;
            match sym {
                Symbol::Cross => {
                    p.line_segment([c + vec2(-r, -r) * 0.85, c + vec2(r, r) * 0.85], line);
                    p.line_segment([c + vec2(r, -r) * 0.85, c + vec2(-r, r) * 0.85], line);
                }
                Symbol::Circle => {
                    p.circle_stroke(c, r, line);
                }
                Symbol::Square => {
                    p.rect_stroke(Rect::from_center_size(c, vec2(9.0, 9.0)), 0.0, line, StrokeKind::Middle);
                }
                Symbol::Triangle => {
                    p.add(Shape::closed_line(vec![c + vec2(0.0, -6.0), c + vec2(5.5, 3.6), c + vec2(-5.5, 3.6)], line));
                }
            }
        }
        Glyph::PadArrow(d) => {
            disc();
            let (dx, dy) = dir(d);
            let r = 5.0;
            p.add(Shape::convex_polygon(
                vec![
                    c + vec2(dx, dy) * r,
                    c + vec2(-dx * 0.6 - dy * 0.9, -dy * 0.6 + dx * 0.9) * r,
                    c + vec2(-dx * 0.6 + dy * 0.9, -dy * 0.6 - dx * 0.9) * r,
                ],
                on,
                Stroke::NONE,
            ));
        }
        Glyph::Dpad => {
            disc();
            p.rect_filled(Rect::from_center_size(c, vec2(14.0, 5.0)), 1.0, on);
            p.rect_filled(Rect::from_center_size(c, vec2(5.0, 14.0)), 1.0, on);
        }
        Glyph::View | Glyph::Create => {
            disc();
            if g == Glyph::View {
                let back = Rect::from_min_size(c + vec2(-7.0, -6.0), vec2(9.0, 7.0));
                p.rect_stroke(back, 1.5, line, StrokeKind::Middle);
                let front = back.translate(vec2(4.0, 4.0));
                p.rect_filled(front, 1.5, fill);
                p.rect_stroke(front, 1.5, line, StrokeKind::Middle);
            } else {
                p.rect_stroke(Rect::from_min_size(c + vec2(-6.0, 1.0), vec2(12.0, 5.0)), 2.5, line, StrokeKind::Middle);
                for dx in [-4.0, 0.0, 4.0] {
                    p.line_segment([c + vec2(dx, -1.5), c + vec2(dx * 1.2, -5.5)], line);
                }
            }
        }
        Glyph::Start => {
            disc();
            for dy in [-4.0, 0.0, 4.0] {
                p.line_segment([c + vec2(-5.5, dy), c + vec2(5.5, dy)], line);
            }
        }
        Glyph::Bumper(s) | Glyph::Trigger(s) | Glyph::Stick(s) | Glyph::Key(s) => {
            let h = if matches!(g, Glyph::Key(_)) { 22.0 } else { 20.0 };
            let radius = match g {
                Glyph::Key(_) => 4,
                Glyph::Stick(_) => 10,
                _ => 7,
            };
            let r = Rect::from_center_size(c, vec2(w - 1.5, h - 1.5));
            p.rect_stroke(r, CornerRadius::same(radius), Stroke::new(1.5, edge), StrokeKind::Middle);
            p.text(c, Align2::CENTER_CENTER, s, cap_font(), edge);
        }
        Glyph::KeyArrow(d) => {
            let r = Rect::from_center_size(c, vec2(w - 1.5, 20.5));
            p.rect_stroke(r, 4.0, Stroke::new(1.5, edge), StrokeKind::Middle);
            let (dx, dy) = dir(d);
            let tip = c + vec2(dx, dy) * 5.0;
            let s = Stroke::new(1.5, edge);
            p.line_segment([c - vec2(dx, dy) * 5.0, tip], s);
            p.line_segment([tip, tip + vec2(-dx * 3.5 - dy * 3.5, -dy * 3.5 + dx * 3.5)], s);
            p.line_segment([tip, tip + vec2(-dx * 3.5 + dy * 3.5, -dy * 3.5 - dx * 3.5)], s);
        }
    }
    w
}

fn dir(d: Dir) -> (f32, f32) {
    match d {
        Dir::Left => (-1.0, 0.0),
        Dir::Right => (1.0, 0.0),
        Dir::Up => (0.0, -1.0),
        Dir::Down => (0.0, 1.0),
    }
}

/// `text` shortened with "…" so it is at most `width` points wide in `font`: the end is cut, or with `middle` the
/// middle (a path keeps its start and its file name). The text itself when it fits.
pub fn elide(p: &Painter, text: &str, font: FontId, width: f32, middle: bool) -> String {
    let wide = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if wide(text) <= width {
        return text.to_string();
    }
    let ch: Vec<char> = text.chars().collect();
    let make = |keep: usize| -> String {
        if middle {
            let tail = keep / 2;
            let head = keep - tail;
            format!("{}…{}", ch[..head].iter().collect::<String>(), ch[ch.len() - tail..].iter().collect::<String>())
        } else {
            format!("{}…", ch[..keep].iter().collect::<String>())
        }
    };
    let (mut lo, mut hi) = (0usize, ch.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if wide(&make(mid)) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    make(lo)
}

/// A key cap with its text (the address bar's "Ctrl+L", the Find field's "Ctrl+K"), right edge at `right`.
pub fn key_cap_right(p: &Painter, text: &'static str, right: f32, cy: f32) -> f32 {
    let w = glyph_width(p, Glyph::Key(text));
    glyph(p, Glyph::Key(text), right - w, cy)
}

/// A rail icon, centred at `c`, in `col` (line drawings, 20 points across).
pub fn rail_icon(p: &Painter, w: Workspace, c: Pos2, col: Color32) {
    let s = Stroke::new(1.6, col);
    match w {
        Workspace::Home => {
            p.add(Shape::line(vec![c + vec2(-8.0, -1.0), c + vec2(0.0, -8.0), c + vec2(8.0, -1.0)], s));
            p.add(Shape::line(
                vec![
                    c + vec2(-6.0, -2.5),
                    c + vec2(-6.0, 8.0),
                    c + vec2(-2.0, 8.0),
                    c + vec2(-2.0, 3.0),
                    c + vec2(2.0, 3.0),
                    c + vec2(2.0, 8.0),
                    c + vec2(6.0, 8.0),
                    c + vec2(6.0, -2.5),
                ],
                s,
            ));
        }
        Workspace::World | Workspace::Changes | Workspace::Data => {
            p.circle_stroke(c, 9.0, s);
            p.line_segment([c + vec2(-9.0, 0.0), c + vec2(9.0, 0.0)], s);
            let pts = |k: f32| -> Vec<Pos2> {
                (0..=16)
                    .map(|i| -FRAC_PI_2 + i as f32 / 16.0 * PI)
                    .map(|a| c + vec2(a.cos() * 4.0 * k, a.sin() * 9.0))
                    .collect()
            };
            p.add(Shape::line(pts(1.0), s));
            p.add(Shape::line(pts(-1.0), s));
        }
        Workspace::Races => {
            p.line_segment([c + vec2(-6.0, -9.0), c + vec2(-6.0, 9.0)], s);
            p.add(Shape::line(
                vec![
                    c + vec2(-6.0, -8.0),
                    c + vec2(7.0, -8.0),
                    c + vec2(4.0, -4.0),
                    c + vec2(7.0, 0.0),
                    c + vec2(-6.0, 0.0),
                ],
                s,
            ));
        }
        Workspace::Browser => {
            // A magnifier.
            p.circle_stroke(c + vec2(-2.0, -2.0), 6.0, s);
            p.line_segment([c + vec2(2.5, 2.5), c + vec2(8.0, 8.0)], s);
        }
        Workspace::Runs => {
            // A play triangle over a track line.
            p.add(Shape::closed_line(vec![c + vec2(-5.0, -8.0), c + vec2(6.0, -2.5), c + vec2(-5.0, 3.0)], s));
            p.line_segment([c + vec2(-8.0, 8.0), c + vec2(8.0, 8.0)], s);
        }
        Workspace::Mod => {
            let top = [c + vec2(0.0, -9.0), c + vec2(8.0, -4.5), c + vec2(0.0, 0.0), c + vec2(-8.0, -4.5)];
            p.add(Shape::closed_line(top.to_vec(), s));
            p.add(Shape::line(vec![top[3], c + vec2(-8.0, 4.5), c + vec2(0.0, 9.0), c + vec2(8.0, 4.5), top[1]], s));
            p.line_segment([top[2], c + vec2(0.0, 9.0)], s);
        }
        Workspace::Settings => {
            p.circle_stroke(c, 3.5, s);
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 6.0, c + d * 9.0], s);
            }
            p.circle_stroke(c, 6.0, s);
        }
    }
}

/// Text in the label face (buttons, tabs, rail names).
pub fn label_font(size: f32) -> FontId {
    font(LABEL, size)
}

/// The panels' fill (the slab over the page) and the page.
pub fn panel_fill() -> Color32 {
    theme::panel()
}
pub fn page_fill() -> Color32 {
    rgb(tokens().page)
}

/// A line between rows.
pub fn rule() -> Color32 {
    theme::over(tokens().row_rule, tokens().slab.over(tokens().page))
}

/// A secondary button: an outlined pill, the label in the italic label face (beside a main action).
pub fn pill(text: &str) -> egui::Button<'static> {
    let t = tokens();
    egui::Button::new(egui::RichText::new(text.to_string()).font(font(LABEL, 17.0)).color(rgb(t.ink)))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.5, rgb(t.outline)))
        .corner_radius(CornerRadius::same(18))
        .min_size(vec2(0.0, 36.0))
}

/// A summary row's height (Home's rows under the verdict card).
pub const SUMMARY_ROW_H: f32 = 38.0;

/// The head every row of a list of facts starts with: a lamp, then the name in the dim body face, the row's
/// vertical centre `r.center().y`. The one place the launcher's Game-check rows are built (Home, the Mod page).
pub fn row_head(p: &Painter, r: Rect, mark: Mark, name: &str, name_size: f32, time: f32) {
    let t = tokens();
    lamp(p, pos2(r.left() + 14.0, r.center().y), 11.0, mark, time);
    p.text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, name, font("body", name_size), rgb(t.dim));
}

/// One summary row: lamp, name and value.
pub struct SummaryRow<'a> {
    pub mark: Mark,
    pub name: &'a str,
    pub value: String,
}

impl<'a> SummaryRow<'a> {
    pub fn new(mark: Mark, name: &'a str, value: impl Into<String>) -> Self {
        SummaryRow { mark, name, value: value.into() }
    }

    /// The busy line (the launcher's Game check): the turning lamp and what runs now.
    pub fn busy(name: &'a str, what: impl Into<String>) -> Self {
        SummaryRow::new(Mark::Busy, name, what)
    }
}

/// The summary rows from `y` down in `area`'s width, the values at `label_w`, each with a rule under it; the `y`
/// below the last.
pub fn summary_rows(p: &Painter, area: Rect, mut y: f32, rows: &[SummaryRow], label_w: f32, time: f32) -> f32 {
    let t = tokens();
    for r in rows {
        let row = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), SUMMARY_ROW_H));
        row_head(p, row, r.mark, r.name, 16.0, time);
        let mut clip = p.clone();
        clip.set_clip_rect(Rect::from_min_max(pos2(row.left() + label_w, row.top()), row.max));
        clip.text(
            pos2(row.left() + label_w, row.center().y),
            Align2::LEFT_CENTER,
            &r.value,
            font("body", 16.0),
            rgb(t.ink),
        );
        p.line_segment([row.left_bottom(), row.right_bottom()], Stroke::new(1.0, rule()));
        y += SUMMARY_ROW_H;
    }
    y
}

/// What a verdict card shows (Home's card, the tutorial's card): its lamp, a small line over the title, the title,
/// its words, the main action and the second one.
pub struct Card<'a> {
    /// A check lamp (ready) or a numbered step lamp.
    pub step: Option<usize>,
    pub kicker: Option<&'a str>,
    pub head: &'a str,
    pub line: &'a str,
    pub main: &'a str,
    pub second: &'a str,
}

/// The card's height at `width` (as high as its words).
pub fn card_height(p: &Painter, c: &Card, width: f32) -> f32 {
    let g = p.layout(c.line.to_string(), font("body", 15.0), Color32::WHITE, width - 48.0);
    let kick = if c.kicker.is_some() { 20.0 } else { 0.0 };
    kick + 52.0 + g.size().y + 14.0 + 40.0 + 16.0
}

/// Draw the verdict card (the launcher's Game check: a lime edge, the lamp, the title, its words, the actions)
/// with its top left at `min`, `width` wide, on `fill`. The rectangle and the two buttons' responses.
pub fn card(
    ui: &mut egui::Ui,
    p: &Painter,
    min: Pos2,
    width: f32,
    c: &Card,
    fill: Color32,
) -> (Rect, egui::Response, egui::Response) {
    let t = tokens();
    let h = card_height(p, c, width);
    let card = Rect::from_min_size(min, vec2(width, h));
    p.rect_filled(card, 8.0, fill);
    p.rect_filled(
        Rect::from_min_size(card.min, vec2(4.0, card.height())),
        CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 },
        rgb(t.lime),
    );
    let mut top = card.top();
    if let Some(k) = c.kicker {
        p.text(pos2(card.left() + 20.0, top + 14.0), Align2::LEFT_TOP, k, font("body", 12.0), rgb(t.dim));
        top += 20.0;
    }
    let lamp_c = pos2(card.left() + 30.0, top + 30.0);
    match c.step {
        Some(n) => {
            p.circle_filled(lamp_c, 10.0, rgb(t.lime));
            p.text(lamp_c, Align2::CENTER_CENTER, n.to_string(), font(SEMI, 13.0), rgb(t.on_play));
        }
        None => check_lamp(p, lamp_c, 20.0),
    }
    p.text(pos2(card.left() + 48.0, top + 30.0), Align2::LEFT_CENTER, c.head, font(theme::TITLE, 22.0), rgb(t.ink));
    let g = p.layout(c.line.to_string(), font("body", 15.0), rgb(t.ink), width - 48.0);
    p.galley(pos2(card.left() + 20.0, top + 52.0), g, rgb(t.ink));
    let buttons = Rect::from_min_size(pos2(card.left() + 20.0, card.bottom() - 56.0), vec2(width - 40.0, 40.0));
    let (a, b) = ui
        .scope_builder(egui::UiBuilder::new().max_rect(buttons), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().button_padding = vec2(18.0, 8.0);
                ui.spacing_mut().item_spacing.x = 12.0;
                let a = ui.add(theme::main_action(c.main).min_size(vec2(0.0, 36.0)));
                let b = ui.add(pill(c.second));
                (a, b)
            })
            .inner
        })
        .inner;
    (card, a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long text is cut with "…" to fit its column (the end, or the middle for a path), a short one is left alone.
    #[test]
    fn elide_fits_the_width() {
        let ctx = egui::Context::default();
        crate::editor::theme::apply(&ctx);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let p = ui.painter();
            let f = font("body", 15.0);
            let w = |s: &str| p.layout_no_wrap(s.to_string(), f.clone(), Color32::WHITE).size().x;
            assert_eq!(elide(p, "short", f.clone(), 300.0, false), "short");
            let path = "/home/someone/.local/share/overbright/exports/review-circuit.zip";
            for width in [60.0, 120.0, 250.0] {
                for middle in [false, true] {
                    let e = elide(p, path, f.clone(), width, middle);
                    assert!(w(&e) <= width, "{e} is {} wide, not {width}", w(&e));
                    assert!(e.contains('…'));
                }
            }
            let e = elide(p, path, f.clone(), 250.0, true);
            assert!(e.ends_with("circuit.zip") || e.ends_with("uit.zip"), "{e}");
        });
        out.textures_delta.clear();
    }

    /// The summary rows and the busy line are built here once: the rows stack at their height, the value clips at
    /// its column, the busy lamp turns with the clock, and the Home page and the Mod page share the row head.
    #[test]
    fn summary_rows_stack_and_the_busy_lamp_turns() {
        let ctx = egui::Context::default();
        crate::editor::theme::apply(&ctx);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let area = Rect::from_min_size(pos2(10.0, 0.0), vec2(400.0, 300.0));
        let rows = [
            SummaryRow::new(Mark::Ok, "Changes", "3 changes"),
            SummaryRow::new(Mark::Warn, "Saved", "x".repeat(200)),
            SummaryRow::busy("Jobs", "Simulating…"),
        ];
        let mut ends = vec![];
        let mut busy = vec![];
        for time in [0.0, 0.25] {
            let mut end = 0.0;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                end = summary_rows(ui.painter(), area, 20.0, &rows, 120.0, time);
            });
            out.textures_delta.clear();
            ends.push(end);
            // The rows' own shapes: three rules, lamps and texts (the busy lamp is an arc).
            let lines = out
                .shapes
                .iter()
                .filter(|c| matches!(&c.shape, Shape::LineSegment { .. }))
                .count();
            assert_eq!(lines, 3, "a rule under each row");
            busy.push(format!("{:?}", out.shapes.iter().filter(|c| matches!(&c.shape, Shape::Path(_))).map(|c| &c.shape).collect::<Vec<_>>()));
        }
        assert_eq!(ends, [20.0 + 3.0 * SUMMARY_ROW_H; 2]);
        assert_ne!(busy[0], busy[1], "the busy lamp turns with the clock");
    }
}
