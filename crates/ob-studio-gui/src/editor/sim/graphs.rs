//! The Telemetry tab: a chosen car's speed and its distance from the racing line over the run's time, the run
//! shown in lime and the run compared (the ghost) in the info colour, a tick for each event of the car, and a
//! line at the time shown; a click or drag on a graph scrubs the playback (the timeline's own). The values are
//! `telemetry`'s, which reads them from the run file.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, vec2};

use super::panel::{Shared, Sim, clock, ghost_colour, label_name};
use super::telemetry::{self, Line, Series, Telemetry};
use crate::editor::overlay::colour;
use crate::editor::state::{EditorState, Panel};
use crate::editor::theme::{self, LABEL, font};

/// A graph's least height (points); below it the tab scrolls.
pub const GRAPH_MIN_H: f32 = 64.0;
/// Under this width the two graphs sit one over the other.
const SIDE_BY_SIDE_FROM: f32 = 560.0;
/// m/s to km/h.
const KMH: f32 = 3.6;

/// The channels the tab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Speed,
    Lateral,
}

impl Channel {
    pub fn name(self) -> &'static str {
        match self {
            Channel::Speed => "Speed",
            Channel::Lateral => "Off the racing line",
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Channel::Speed => "km/h",
            Channel::Lateral => "m",
        }
    }
    /// The factor from the run file's unit to the one shown.
    fn scale(self) -> f32 {
        match self {
            Channel::Speed => KMH,
            Channel::Lateral => 1.0,
        }
    }
}

/// The channel of `t` (the lateral one may be missing).
pub fn series(t: &Telemetry, c: Channel) -> Option<&Series> {
    match c {
        Channel::Speed => Some(&t.speed),
        Channel::Lateral => t.lateral.as_ref(),
    }
}

/// The racing line of the run's race in this mod, when it is there.
pub fn line_of(s: &EditorState, sim: &Sim) -> Option<Line> {
    let id = sim.rec.header.as_ref().map(|h| h.race.as_str()).filter(|r| !r.is_empty())?;
    Line::of(s.session.map.races.iter().find(|r| r.id == id)?)
}

/// The car the tab shows: the one chosen, else the one followed, else the first racer.
pub fn car(sim: &Sim) -> Option<u32> {
    let racers = sim.rec.racers();
    sim.tele_car
        .filter(|c| racers.contains(c))
        .or(sim.play.follow.filter(|c| racers.contains(c)))
        .or(racers.first().copied())
}

/// The bottom tab.
pub struct TelemetryTab(pub Shared);

/// What the tab drew (the layout checks).
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub graphs: Vec<Rect>,
}

impl Panel for TelemetryTab {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let mut sim = self.0.borrow_mut();
        let sim = &mut *sim;
        let Some((lo, hi)) = sim.rec.span() else {
            ui.weak("No run shown. Open one in the list on the left, or Simulate a race.");
            return;
        };
        let Some(id) = car(sim) else {
            ui.weak("This run has no racers to show.");
            return;
        };
        let line = line_of(s, sim);
        let a = telemetry::of(&sim.rec, id, line.as_ref());
        let b = sim.ghost.as_ref().map(|g| telemetry::of(g, id, line.as_ref()));
        let racers = sim.rec.racers();
        ui.horizontal_wrapped(|ui| {
            let mut pick = None;
            egui::ComboBox::from_id_salt("runs.car")
                .width(150.0)
                .selected_text(super::panel::short(&label_name(&sim.rec, id)))
                .show_ui(ui, |ui| {
                    for r in &racers {
                        if ui.selectable_label(*r == id, label_name(&sim.rec, *r)).clicked() {
                            pick = Some(*r);
                        }
                    }
                })
                .response
                .on_hover_text("The car whose telemetry is shown");
            if let Some(r) = pick {
                sim.tele_car = Some(r);
            }
            key(ui, colour::SELECT, if sim.shown.is_empty() { "this run" } else { &sim.shown });
            if sim.ghost.is_some() {
                key(ui, ghost_colour(), &format!("B: {}", sim.ghost_name));
            }
            let time = sim.play.time;
            let (va, vb) = (value_at(&a.speed, time), b.as_ref().and_then(|b| value_at(&b.speed, time)));
            ui.monospace(readout("Speed", va.map(|v| v * KMH), vb.map(|v| v * KMH), "km/h"));
            if let Some(l) = a.lateral.as_ref() {
                let vb = b.as_ref().and_then(|b| b.lateral.as_ref()).and_then(|l| value_at(l, time));
                ui.monospace(readout("Off line", value_at(l, time), vb, "m"));
            }
        });
        let w = ui.available_width();
        let h = ui.available_height().max(GRAPH_MIN_H);
        let side = w >= SIDE_BY_SIDE_FROM;
        let (gw, gh) = if side { ((w - 8.0) / 2.0, h) } else { (w, GRAPH_MIN_H.max(h / 2.0)) };
        let mut drawn = Drawn::default();
        let mut scrub = None;
        let channels = [Channel::Speed, Channel::Lateral];
        let mut place = |ui: &mut egui::Ui, k: usize| {
            let (r, resp) = ui.allocate_exact_size(vec2(gw, gh), Sense::click_and_drag());
            if side && k == 0 {
                ui.add_space(-4.0);
            }
            drawn.graphs.push(r);
            (r, resp)
        };
        if side {
            ui.horizontal(|ui| {
                for (k, c) in channels.into_iter().enumerate() {
                    let (r, resp) = place(ui, k);
                    plot(ui, r, &resp, c, &a, b.as_ref(), sim, (lo, hi), &mut scrub);
                }
            });
        } else {
            for (k, c) in channels.into_iter().enumerate() {
                let (r, resp) = place(ui, k);
                plot(ui, r, &resp, c, &a, b.as_ref(), sim, (lo, hi), &mut scrub);
            }
        }
        if let Some(t) = scrub {
            sim.play.seek_frame(&sim.rec, t);
        }
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("runs.graphs"), drawn.graphs));
    }
}

fn key(ui: &mut egui::Ui, c: Color32, text: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, c);
    ui.label(text);
    ui.add_space(6.0);
}

/// The rects of the graphs last drawn (the layout check).
pub fn drawn(ctx: &egui::Context) -> Vec<Rect> {
    ctx.data(|d| d.get_temp::<Vec<Rect>>(egui::Id::new("runs.graphs"))).unwrap_or_default()
}

fn value_at(s: &[(f32, f32)], t: f32) -> Option<f32> {
    telemetry::value_at(s, t)
}

/// "Speed 123 km/h (B 120)".
pub fn readout(name: &str, a: Option<f32>, b: Option<f32>, unit: &str) -> String {
    let n = |v: Option<f32>| v.map_or("-".to_string(), |v| format!("{v:.0}"));
    match b {
        Some(_) => format!("{name} {} / B {} {unit}", n(a), n(b)),
        None => format!("{name} {} {unit}", n(a)),
    }
}

/// One graph in `rect`.
#[allow(clippy::too_many_arguments)]
fn plot(
    ui: &mut egui::Ui,
    rect: Rect,
    resp: &egui::Response,
    c: Channel,
    a: &Telemetry,
    b: Option<&Telemetry>,
    sim: &Sim,
    (lo, hi): (f32, f32),
    scrub: &mut Option<f32>,
) {
    let t = theme::tokens();
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, theme::rgb(t.rail).gamma_multiply(0.5));
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, theme::rgb(t.outline)), egui::StrokeKind::Inside);
    p.text(
        rect.left_top() + vec2(8.0, 4.0),
        Align2::LEFT_TOP,
        format!("{} ({})", c.name(), c.unit()),
        font(LABEL, 13.0),
        theme::dim(),
    );
    let Some(sa) = series(a, c) else {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No racing line: this run's race is not in the open mod",
            font("body", 14.0),
            theme::dim(),
        );
        return;
    };
    let sb = b.and_then(|b| series(b, c));
    let k = c.scale();
    let (mut ymin, mut ymax) = (f32::INFINITY, f32::NEG_INFINITY);
    for s in [Some(sa), sb].into_iter().flatten() {
        if let Some((l, h)) = telemetry::range(s) {
            ymin = ymin.min(l * k);
            ymax = ymax.max(h * k);
        }
    }
    if !ymin.is_finite() {
        return;
    }
    match c {
        Channel::Speed => ymin = 0.0,
        Channel::Lateral => {
            let m = ymin.abs().max(ymax.abs()).max(2.0);
            (ymin, ymax) = (-m, m);
        }
    }
    if ymax - ymin < 1e-3 {
        ymax = ymin + 1.0;
    }
    let area = Rect::from_min_max(rect.left_top() + vec2(8.0, 24.0), rect.right_bottom() - vec2(8.0, 12.0));
    let to = |time: f32, v: f32| {
        Pos2::new(
            area.left() + (time - lo) / (hi - lo).max(1e-6) * area.width(),
            area.bottom() - (v - ymin) / (ymax - ymin) * area.height(),
        )
    };
    let grid = Stroke::new(1.0, theme::rgb(t.outline).gamma_multiply(0.5));
    if c == Channel::Lateral {
        p.line_segment([to(lo, 0.0), to(hi, 0.0)], grid);
    }
    for (v, side) in [(ymin, Align2::LEFT_BOTTOM), (ymax, Align2::LEFT_TOP)] {
        p.text(Pos2::new(area.left() + 2.0, to(lo, v).y), side, format!("{v:.0}"), font(LABEL, 11.0), theme::dim());
    }
    // The green light.
    if lo < 0.0 && hi > 0.0 {
        let x = to(0.0, 0.0).x;
        p.line_segment([Pos2::new(x, area.top()), Pos2::new(x, area.bottom())], grid);
    }
    let draw = |s: &Series, col: Color32, w: f32| {
        // At most a point a pixel: the graph is a view, the file keeps every frame.
        let step = (s.len() as f32 / area.width().max(1.0)).ceil().max(1.0) as usize;
        let pts: Vec<Pos2> = s.iter().step_by(step).map(|&(time, v)| to(time, v * k)).collect();
        p.add(egui::Shape::line(pts, Stroke::new(w, col)));
    };
    if let Some(s) = sb {
        draw(s, ghost_colour(), 1.5);
    }
    draw(sa, colour::SELECT, 1.8);
    // The car's events: a tick on the bottom edge, the checkpoints dim.
    for &(time, kind, _) in &a.events {
        let col = match kind {
            super::format::EventKind::Checkpoint | super::format::EventKind::Lap => colour::DIM,
            super::format::EventKind::Crash | super::format::EventKind::Stuck => colour::ERROR,
            _ => colour::WARN,
        };
        let x = to(time, 0.0).x;
        p.line_segment([Pos2::new(x, area.bottom() - 6.0), Pos2::new(x, area.bottom())], Stroke::new(2.0, col));
    }
    if let Some(t) = sim.parts_at {
        let x = to(t, 0.0).x;
        p.line_segment(
            [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
            Stroke::new(1.0, ghost_colour().gamma_multiply(0.7)),
        );
    }
    let x = to(sim.play.time.clamp(lo, hi), 0.0).x;
    p.line_segment([Pos2::new(x, area.top()), Pos2::new(x, area.bottom())], Stroke::new(1.5, colour::SELECT));
    if let Some(at) = resp.hover_pos().filter(|_| !resp.dragged()) {
        let time = lo + ((at.x - area.left()) / area.width().max(1.0)).clamp(0.0, 1.0) * (hi - lo);
        let tip = match value_at(sa, time) {
            Some(v) => format!("{}: {:.1} {} at {}", c.name(), v * k, c.unit(), clock(time)),
            None => clock(time),
        };
        resp.clone().on_hover_text(format!("{tip} (click: show it)"));
    }
    if (resp.clicked() || resp.dragged())
        && let Some(at) = resp.interact_pointer_pos()
    {
        *scrub = Some(lo + ((at.x - area.left()) / area.width().max(1.0)).clamp(0.0, 1.0) * (hi - lo));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readouts_say_both_runs() {
        assert_eq!(readout("Speed", Some(72.0), None, "km/h"), "Speed 72 km/h");
        assert_eq!(readout("Speed", Some(72.0), Some(70.4), "km/h"), "Speed 72 / B 70 km/h");
        assert_eq!(readout("Off line", None, None, "m"), "Off line - m");
    }
}
