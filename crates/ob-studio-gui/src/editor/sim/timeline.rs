//! The run's timeline under the 3D view / map: play / pause, a recorded
//! frame back and on, the speed, the time, the camera, and the track the
//! mouse scrubs along with a marker for each thing that went wrong (What went
//! wrong's findings of the run shown, a colour per kind; a click jumps there
//! as a What went wrong row does). The live positions board sits over the
//! view's top right. Everything reads the recording only ([`Recording`]),
//! so it serves any recorded run, not only a fresh simulation.

use egui::{Color32, Pos2, Rect, Sense, Stroke};

use super::cams::CamMode;
use super::format::SimMode;
use super::panel::{Shared, Sim, clock, label_name, short};
use super::record::{Recording, SPEEDS};
use super::summary::{Finding, Kind, RunFindings};
use crate::editor::overlay::colour;
use crate::editor::state::{EditorState, Panel};
use crate::editor::theme;

/// The track's height (points).
const TRACK_H: f32 = 22.0;
/// The extra row under the track while a second run is compared: its events.
const GHOST_ROW: f32 = 12.0;
/// How near (points) the pointer must be to a marker to take it.
const MARKER_REACH: f32 = 5.0;

/// The markers of the run shown: its findings (the summary's, Spread left
/// out: it has no moment), by time.
pub fn markers<'a>(batch: &'a [RunFindings], rec: &Recording) -> Vec<&'a Finding> {
    let seed = rec.header.as_ref().map(|h| h.seed);
    let mut v: Vec<&Finding> = batch
        .iter()
        .filter(|r| Some(r.seed) == seed)
        .flat_map(|r| r.findings.iter())
        .filter(|f| f.kind != Kind::Spread)
        .collect();
    v.sort_by(|a, b| a.time.total_cmp(&b.time));
    v
}

/// A marker's kind in the legend.
pub fn kind_name(k: &Kind) -> &'static str {
    match k {
        Kind::Start => "Start trouble",
        Kind::Stuck => "Stuck",
        Kind::CrashProp(_) | Kind::CrashCar | Kind::CrashWall => "Crash",
        Kind::WrongWay => "Wrong way",
        Kind::Missed => "Missed checkpoint",
        Kind::OffRoad => "Off the roads",
        Kind::Spread => "Finish",
    }
}

/// A marker's colour (the look's status tokens).
pub fn kind_colour(k: &Kind) -> Color32 {
    let t = theme::tokens();
    theme::rgb(match k {
        Kind::Start | Kind::Stuck => t.warn,
        Kind::CrashProp(_) | Kind::CrashCar | Kind::CrashWall => t.err,
        Kind::WrongWay => t.info,
        Kind::Missed => t.ink,
        Kind::OffRoad | Kind::Spread => t.dim,
    })
}

/// The time at `x` on a track over `rect` spanning `lo..hi`.
pub fn time_at(rect: Rect, lo: f32, hi: f32, x: f32) -> f32 {
    let w = rect.width().max(1.0);
    lo + ((x - rect.left()) / w).clamp(0.0, 1.0) * (hi - lo)
}

/// Where time `t` sits on the track.
pub fn x_of(rect: Rect, lo: f32, hi: f32, t: f32) -> f32 {
    let span = (hi - lo).max(1e-6);
    rect.left() + ((t - lo) / span).clamp(0.0, 1.0) * rect.width()
}

/// The timeline strip (shown while a run is loaded).
pub struct Timeline(pub Shared);

impl Panel for Timeline {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        let mut sim = self.0.borrow_mut();
        let sim = &mut *sim;
        let Some((lo, hi)) = sim.rec.span() else { return };
        egui::Panel::bottom("sim.timeline").resizable(false).show(ui, |ui| {
            ui.add_space(4.0);
            track(ui, s, sim, lo, hi);
            ui.horizontal_wrapped(|ui| controls(ui, sim, hi));
            ui.add_space(2.0);
        });
        keys(ui.ctx(), sim);
    }
}

/// The track: the played part, the markers, the time shown; the mouse
/// scrubs (to the nearest recorded frame), a marker's click jumps to it.
fn track(ui: &mut egui::Ui, s: &mut EditorState, sim: &mut Sim, lo: f32, hi: f32) {
    let extra = if sim.ghost.is_some() { GHOST_ROW } else { 0.0 };
    let (full, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TRACK_H + extra), Sense::click_and_drag());
    let rect = Rect::from_min_size(full.min, egui::vec2(full.width(), TRACK_H));
    sim.track_rect = rect;
    let marks: Vec<Finding> = markers(&sim.batch, &sim.rec).into_iter().cloned().collect();
    let p = ui.painter_at(full.expand(2.0));
    let bar = Rect::from_min_max(
        Pos2::new(rect.left(), rect.center().y - 3.0),
        Pos2::new(rect.right(), rect.center().y + 3.0),
    );
    p.rect_filled(bar, 3.0, theme::rgb(theme::tokens().rail));
    let now = x_of(rect, lo, hi, sim.play.time);
    p.rect_filled(Rect::from_min_max(bar.min, Pos2::new(now, bar.max.y)), 3.0, colour::SELECT.gamma_multiply(0.45));
    // The green light.
    if lo < 0.0 && hi > 0.0 {
        let x = x_of(rect, lo, hi, 0.0);
        p.line_segment(
            [Pos2::new(x, rect.top() + 3.0), Pos2::new(x, rect.bottom() - 3.0)],
            Stroke::new(1.0, colour::DIM),
        );
    }
    let hover = r.hover_pos();
    let near = hover.and_then(|h| {
        marks
            .iter()
            .enumerate()
            .map(|(k, f)| (k, (x_of(rect, lo, hi, f.time) - h.x).abs()))
            .filter(|&(_, d)| d <= MARKER_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    });
    for (k, f) in marks.iter().enumerate() {
        let x = x_of(rect, lo, hi, f.time);
        let c = kind_colour(&f.kind);
        let big = near == Some(k);
        let h = if big { 7.0 } else { 5.0 };
        let top = rect.top() + 1.0;
        p.add(egui::Shape::convex_polygon(
            vec![Pos2::new(x, top + 2.0 * h), Pos2::new(x - h * 0.8, top), Pos2::new(x + h * 0.8, top)],
            c,
            Stroke::new(1.0, colour::SHADOW),
        ));
        p.line_segment([Pos2::new(x, top + 2.0 * h), Pos2::new(x, rect.bottom() - 2.0)], Stroke::new(1.5, c));
    }
    if let Some(g) = sim.ghost.as_ref() {
        // The second run under the track: its events (not the checkpoints) as ticks in the info colour, and
        // where it parts from the first.
        let gc = super::panel::ghost_colour();
        let row =
            Rect::from_min_size(Pos2::new(rect.left(), rect.bottom() + 1.0), egui::vec2(rect.width(), GHOST_ROW - 2.0));
        p.rect_filled(row, 2.0, theme::rgb(theme::tokens().rail).gamma_multiply(0.6));
        for e in g.events.iter().filter(|e| e.kind != super::format::EventKind::Checkpoint) {
            let x = x_of(rect, lo, hi, e.time);
            p.line_segment([Pos2::new(x, row.top() + 1.0), Pos2::new(x, row.bottom() - 1.0)], Stroke::new(2.0, gc));
        }
        if let Some(t) = sim.parts_at {
            let x = x_of(rect, lo, hi, t);
            p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, row.bottom())], Stroke::new(1.0, gc));
        }
    }
    p.line_segment([Pos2::new(now, rect.top()), Pos2::new(now, rect.bottom())], Stroke::new(2.0, colour::SELECT));
    let r = match near {
        Some(k) => r.on_hover_text(format!("{} at {} (click: show it)", marks[k].text, clock(marks[k].time))),
        None => r.on_hover_text(
            "Click or drag to move through the run (Space: play / pause, Left / Right: a frame, with Shift: a second)",
        ),
    };
    if r.clicked()
        && let Some(k) = near
    {
        sim.show_finding(s, ui.ctx(), &marks[k]);
    } else if (r.clicked() || r.dragged())
        && let Some(at) = r.interact_pointer_pos()
    {
        sim.play.seek_frame(&sim.rec, time_at(rect, lo, hi, at.x));
    }
}

/// Play / pause, a frame back and on, the speed, the time, the camera and
/// the legend of the markers.
fn controls(ui: &mut egui::Ui, sim: &mut Sim, hi: f32) {
    if ui.small_button("<").on_hover_text("One recorded frame back (Left; Shift+Left: a second)").clicked() {
        sim.play.step(&sim.rec, -1);
    }
    if ui.button(if sim.play.playing { "Pause" } else { "Play" }).on_hover_text("Space").clicked() {
        sim.play.toggle(&sim.rec);
    }
    if ui.small_button(">").on_hover_text("One recorded frame on (Right; Shift+Right: a second)").clicked() {
        sim.play.step(&sim.rec, 1);
    }
    egui::ComboBox::from_id_salt("sim.speed")
        .width(64.0)
        .selected_text(format!("{}x", SPEEDS[sim.play.speed]))
        .show_ui(ui, |ui| {
            for (k, v) in SPEEDS.iter().enumerate() {
                ui.selectable_value(&mut sim.play.speed, k, format!("{v}x"));
            }
        })
        .response
        .on_hover_text("Playback speed");
    ui.monospace(format!("{} / {}", clock(sim.play.time), clock(hi)));
    if sim.ghost.is_some() {
        let n = if sim.ghost_name.is_empty() { "run B".to_string() } else { format!("B: {}", sim.ghost_name) };
        let text = match sim.parts_at {
            Some(t) => format!("{n}, parts at {}", clock(t)),
            None => format!("{n}, cars on the same course"),
        };
        ui.label(egui::RichText::new(text).color(super::panel::ghost_colour()))
            .on_hover_text("The second run, drawn as ghosts in the view and as ticks under the track");
    }
    ui.separator();
    // The camera: free (the editor's own) or riding with a car.
    let mode = match sim.play.follow {
        None => "Free camera",
        Some(_) => sim.cam.label(),
    };
    egui::ComboBox::from_id_salt("sim.camera").width(96.0).selected_text(mode).show_ui(ui, |ui| {
        if ui
            .selectable_label(sim.play.follow.is_none(), "Free camera")
            .on_hover_text("Fly the 3D view yourself")
            .clicked()
        {
            sim.play.follow = None;
        }
        for m in CamMode::ALL {
            let on = sim.play.follow.is_some() && sim.cam == m;
            if ui.selectable_label(on, m.label()).on_hover_text(m.tip()).clicked() {
                sim.cam = m;
                if sim.play.follow.is_none() {
                    sim.play.follow = sim.rec.racers().first().copied();
                }
            }
        }
    });
    if sim.play.follow.is_some() {
        let name = sim.play.follow.map_or(String::new(), |id| sim.rec.name(id));
        egui::ComboBox::from_id_salt("sim.follow").width(130.0).selected_text(name).show_ui(ui, |ui| {
            for id in sim.rec.racers() {
                let n = sim.rec.name(id);
                ui.selectable_value(&mut sim.play.follow, Some(id), n);
            }
        });
    }
    // The markers' colours, the kinds this run has.
    let mut kinds: Vec<&'static str> = Vec::new();
    let marks = markers(&sim.batch, &sim.rec);
    for f in &marks {
        let n = kind_name(&f.kind);
        if !kinds.contains(&n) {
            kinds.push(n);
            // Only as far as the row has room (the markers name themselves
            // when hovered).
            let font = egui::TextStyle::Body.resolve(ui.style());
            let w = ui.painter().layout_no_wrap(n.to_string(), font, Color32::WHITE).size().x;
            if ui.available_size_before_wrap().x < w + 2.0 * ui.spacing().item_spacing.x + 14.0 {
                break;
            }
            ui.add_space(4.0);
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, kind_colour(&f.kind));
            ui.weak(n);
        }
    }
}

/// Space plays / pauses, Left / Right a recorded frame, with Shift a
/// second; not while a text field or a What went wrong row has the keys.
fn keys(ctx: &egui::Context, sim: &mut Sim) {
    let focused = ctx.memory(|m| m.focused());
    if ctx.egui_wants_keyboard_input() || focused.is_some_and(|f| sim.row_ids.contains(&f)) {
        return;
    }
    let (space, left, right, shift) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Space),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.modifiers.shift,
        )
    });
    if space {
        sim.play.toggle(&sim.rec);
    }
    for (pressed, dir) in [(left, -1.0f32), (right, 1.0)] {
        if !pressed {
            continue;
        }
        if shift {
            let t = sim.play.time + dir;
            sim.play.seek_frame(&sim.rec, t);
        } else {
            sim.play.step(&sim.rec, dir as i32);
        }
    }
}

/// The live positions over the view's top right: each racer's place at the
/// time shown, its car, and its lap or finish; the car followed in lime, a
/// click follows a car. True when the pointer is over it.
pub fn board(ctx: &egui::Context, sim: &mut Sim, view: Rect) -> bool {
    let race = sim.rec.header.as_ref().is_some_and(|h| h.mode == SimMode::Race);
    if !race || view.width() < 360.0 || view.height() < 260.0 {
        return false;
    }
    let st = sim.rec.standings(sim.play.time);
    if st.is_empty() {
        return false;
    }
    let laps = sim.rec.header.as_ref().map_or(0, |h| h.laps);
    let mut pick = None;
    let r = egui::Area::new(egui::Id::unique("sim.board"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::RIGHT_TOP)
        // Under the view's look bar, in its frame.
        .fixed_pos(view.right_top() + egui::vec2(-10.0, 52.0))
        .constrain_to(view)
        .fade_in(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE.fill(colour::PAGE.gamma_multiply(0.85)).corner_radius(6.0).inner_margin(8.0);
            frame.show(ui, |ui| {
                ui.label(egui::RichText::new("Positions").font(theme::font(theme::LABEL, 13.0)).color(theme::dim()));
                egui::Grid::new("sim.board.rows").num_columns(3).spacing([8.0, 2.0]).show(ui, |ui| {
                    for row in st.iter().take(8) {
                        let place = if row.place == 0 { "-".to_string() } else { format!("P{}", row.place) };
                        ui.monospace(place);
                        let name = short(&label_name(&sim.rec, row.car));
                        let followed = sim.play.follow == Some(row.car);
                        let text = if followed {
                            egui::RichText::new(name).color(theme::lime())
                        } else {
                            egui::RichText::new(name)
                        };
                        if ui.selectable_label(followed, text).on_hover_text("Follow this car").clicked() {
                            pick = Some(row.car);
                        }
                        let what = if row.dnf {
                            "did not finish".to_string()
                        } else if let Some(t) = row.time {
                            clock(t)
                        } else if laps > 1 {
                            format!("lap {} of {laps}", (row.laps.len() as u32 + 1).min(laps))
                        } else {
                            String::new()
                        };
                        ui.weak(what);
                        ui.end_row();
                    }
                });
            });
        });
    if let Some(id) = pick {
        sim.play.follow = Some(id);
    }
    r.response.hovered() || r.response.contains_pointer()
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
