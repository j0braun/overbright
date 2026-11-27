//! The Race tab: the selected race as one strip, S · 1 · 2 · … · F, with each leg's length and whether a road leads
//! through it. A click on a stop selects that point (the Inspector shows it); a broken leg has Show (the Validation
//! tab's) and its fix; the "+" between two checkpoints puts a new one into the leg, on its road. The badge is the
//! tree's: can't race, not tested or tested. Every edit is an op (`AddPoint`), so the strip, the tree and the command
//! line do the same.

use glam::Vec2;

use super::races::{Preview, Tool, place_op, point_xy};
use super::state::{EditorState, Level, Mode, RacePoint};
use super::theme::{rgb, tokens};
use super::undo;
use super::validate::{self, Check};
use crate::tools::map::edit::RacePart;
use crate::tools::map::CustomRace;

/// The bottom tab's key.
pub const TAB: &str = "race";

/// How a race stands (the tree's badges).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    CantRace,
    NotTested,
    Tested,
}

impl Badge {
    pub fn text(self) -> &'static str {
        match self {
            Badge::CantRace => "can't race",
            Badge::NotTested => "not tested",
            Badge::Tested => "tested",
        }
    }
}

/// Whether a road leads through a leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegState {
    /// The base track's roads lead through it.
    Road,
    /// They do not (a race barrier or no road there).
    NoRoad,
    /// From the start line or to the finish line: a straight line, not checked on its own.
    Line,
}

/// A stop of the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub label: String,
    pub point: RacePoint,
}

/// The way between two stops.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    pub metres: f32,
    pub state: LegState,
    /// A checkpoint put into the leg goes to this index, at this spot (the middle of the leg's road).
    pub insert: Option<(usize, Vec2)>,
    /// Why the leg is broken: the check that has it (its text, its Show).
    pub broken: Option<usize>,
}

/// The strip of one race.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub stops: Vec<Stop>,
    /// `stops.len() - 1` legs; a circuit has one more: the last checkpoint back to the first.
    pub legs: Vec<Leg>,
    pub badge: Badge,
}

fn length(line: &[Vec2]) -> f32 {
    line.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The point half way along `line`.
fn middle(line: &[Vec2]) -> Option<Vec2> {
    let mut left = length(line) / 2.0;
    for w in line.windows(2) {
        let d = w[0].distance(w[1]);
        if d >= left && d > 0.0 {
            return Some(w[0].lerp(w[1], left / d));
        }
        left -= d;
    }
    line.first().copied()
}

/// The strip of race `k` (`r`), from its route preview and the checks.
pub fn strip(r: &CustomRace, k: usize, preview: Option<&Preview>, checks: &[Check]) -> Strip {
    let mut stops = Vec::new();
    let cp = |index: usize| RacePoint { part: RacePart::Checkpoint, index };
    let start = RacePoint { part: RacePart::Start, index: 0 };
    let finish = RacePoint { part: RacePart::Finish, index: 0 };
    if r.start.is_some() {
        stops.push(Stop { label: "S".into(), point: start });
    }
    for i in 0..r.checkpoints.len() {
        stops.push(Stop { label: (i + 1).to_string(), point: RacePoint { part: RacePart::Checkpoint, index: i } });
    }
    let has_finish = r.finish.is_some() && !r.kind.loops();
    if has_finish {
        stops.push(Stop { label: "F".into(), point: finish });
    }
    let broken_at = |leg: usize| {
        let p = RacePoint { part: RacePart::Checkpoint, index: leg };
        checks.iter().position(|c| c.race == Some(k) && c.level == Level::Error && c.point == Some(p) && c.at.is_some())
    };
    let cp_leg = |i: usize, insert_at: Option<usize>| -> Leg {
        let (pa, pb) = (point_xy(r, cp(i)), point_xy(r, cp((i + 1) % r.checkpoints.len().max(1))));
        let straight = pa.zip(pb).map_or(0.0, |(a, b)| a.distance(b));
        let known = preview.is_some_and(|p| !p.legs.is_empty());
        match preview.and_then(|p| p.legs.get(i)).cloned().flatten() {
            Some(line) => Leg {
                metres: length(&line),
                state: LegState::Road,
                insert: insert_at.zip(middle(&line)),
                broken: None,
            },
            // Without a route preview (too few checkpoints, no network) the legs are not known.
            None if !known => Leg { metres: straight, state: LegState::Line, insert: insert_at.zip(pa.zip(pb).map(|(a, b)| a.lerp(b, 0.5))), broken: None },
            None => Leg {
                metres: straight,
                state: LegState::NoRoad,
                insert: insert_at.zip(pa.zip(pb).map(|(a, b)| a.lerp(b, 0.5))),
                broken: broken_at(i),
            },
        }
    };
    let mut legs = Vec::new();
    for w in stops.windows(2) {
        let (a, b) = (w[0].point, w[1].point);
        legs.push(if a.part == RacePart::Checkpoint && b.part == RacePart::Checkpoint {
            cp_leg(a.index, Some(b.index))
        } else {
            let line = point_xy(r, a).zip(point_xy(r, b)).map_or(0.0, |(a, b)| a.distance(b));
            Leg { metres: line, state: LegState::Line, insert: None, broken: None }
        });
    }
    if r.kind.loops() && r.checkpoints.len() > 2 {
        legs.push(cp_leg(r.checkpoints.len() - 1, None));
    }
    let errors = checks.iter().any(|c| c.race == Some(k) && c.level == Level::Error);
    let tested = checks.iter().any(|c| c.race == Some(k) && c.ok && c.text.ends_with(": tested"));
    let badge = if errors {
        Badge::CantRace
    } else if tested {
        Badge::Tested
    } else {
        Badge::NotTested
    };
    Strip { stops, legs, badge }
}

fn km(m: f32) -> String {
    if m >= 1000.0 { format!("{:.1} km", m / 1000.0) } else { format!("{m:.0} m") }
}

/// What the strip does in a frame: a stop to select, a checkpoint to insert, a check to Show or fix.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pick {
    pub stop: Option<RacePoint>,
    pub insert: Option<(usize, Vec2)>,
    pub show: Option<usize>,
    pub fix: Option<usize>,
}

/// The stop's button size (points).
const STOP: f32 = 30.0;
/// The room of a leg between two stops.
const LEG_W: f32 = 96.0;

/// Draw the tab for the selected race; the strip scrolls sideways when it is longer than the tab.
pub fn tab(s: &mut EditorState, checks: &[Check], previews: &[Preview], ui: &mut egui::Ui) {
    let Some(k) = s.race.filter(|&k| k < s.session.map.races.len()) else {
        ui.label("No race selected: pick one in the Mod tree, or Race > New race.");
        return;
    };
    let r = s.session.map.races[k].clone();
    let strip = strip(&r, k, previews.get(k).filter(|p| p.road.is_some() || !p.legs.is_empty()), checks);
    let t = tokens();
    let title = if r.name.is_empty() { r.id.clone() } else { r.name.clone() };
    let mut pick = Pick::default();
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(title).strong());
        let total: f32 = strip.legs.iter().map(|l| l.metres).sum();
        ui.label(
            egui::RichText::new(format!("{} · track {} · {}", r.kind.name(), r.track, km(total))).color(rgb(t.dim)),
        );
        let c = match strip.badge {
            Badge::CantRace => t.err,
            Badge::NotTested => t.warn,
            Badge::Tested => t.ok,
        };
        ui.label(egui::RichText::new(strip.badge.text()).small().color(rgb(c)));
    });
    if strip.stops.is_empty() {
        ui.label("Nothing placed yet: the Start tool, then Checkpoint, puts the first stops on the road.");
    }
    egui::ScrollArea::horizontal().id_salt("race-strip").auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (j, stop) in strip.stops.iter().enumerate() {
                let current = s.point == Some(stop.point);
                let resp = ui.add_sized(
                    [STOP, STOP],
                    egui::Button::new(egui::RichText::new(&stop.label).strong())
                        .corner_radius(STOP / 2.0)
                        .selected(current),
                );
                if resp.on_hover_text(stop_hint(stop)).clicked() {
                    pick.stop = Some(stop.point);
                }
                if let Some(leg) = strip.legs.get(j) {
                    leg_ui(ui, leg, checks, &mut pick);
                }
            }
            // A circuit's way back to the first checkpoint.
            if strip.legs.len() >= strip.stops.len().max(1)
                && let Some(leg) = strip.legs.last()
            {
                ui.label(egui::RichText::new("↻").color(rgb(t.dim)));
                let first = strip.stops.iter().find(|st| st.point.part == RacePart::Checkpoint);
                ui.vertical(|ui| {
                    ui.set_width(LEG_W);
                    ui.label(
                        egui::RichText::new(format!("back to {} · {}", first.map_or("1", |f| f.label.as_str()), km(leg.metres)))
                            .small()
                            .color(rgb(if leg.state == LegState::NoRoad { t.err } else { t.dim })),
                    );
                });
            }
        });
    });
    take(s, k, &r, &pick, checks);
}

fn stop_hint(stop: &Stop) -> String {
    match stop.point.part {
        RacePart::Start => "The start line".into(),
        RacePart::Finish => "The finish line".into(),
        _ => format!("Checkpoint {}", stop.label),
    }
}

fn leg_ui(ui: &mut egui::Ui, leg: &Leg, checks: &[Check], pick: &mut Pick) {
    let t = tokens();
    let broken = leg.state == LegState::NoRoad;
    let colour = rgb(if broken { t.err } else { t.ok });
    ui.vertical(|ui| {
        ui.set_width(LEG_W);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(LEG_W - 26.0, STOP), egui::Sense::hover());
            let y = rect.center().y;
            let stroke = egui::Stroke::new(3.0, if leg.state == LegState::Line { rgb(t.dim) } else { colour });
            if broken {
                // Dashed.
                let mut x = rect.left();
                while x < rect.right() {
                    ui.painter().line_segment([egui::pos2(x, y), egui::pos2((x + 6.0).min(rect.right()), y)], stroke);
                    x += 10.0;
                }
            } else {
                ui.painter().line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], stroke);
            }
            if let Some(at) = leg.insert
                && ui.small_button("+").on_hover_text("Put a checkpoint here, on the road").clicked()
            {
                pick.insert = Some(at);
            }
        });
        let text = if broken { "no road".to_string() } else { km(leg.metres) };
        ui.label(egui::RichText::new(text).small().color(if broken { colour } else { rgb(t.ink) }));
        if let Some(c) = leg.broken.and_then(|i| checks.get(i)) {
            ui.horizontal(|ui| {
                if ui.small_button("Show").clicked() {
                    pick.show = leg.broken;
                }
                if let (Some(fix), true) = (&c.fix, c.race.is_some())
                    && ui.small_button(fix.label()).clicked()
                {
                    pick.fix = leg.broken;
                }
            });
        }
    });
}

/// Do what the frame picked.
fn take(s: &mut EditorState, k: usize, r: &CustomRace, pick: &Pick, checks: &[Check]) {
    if let Some(p) = pick.stop {
        s.mode = Mode::Races;
        s.point = Some(p);
    }
    if let Some((index, at)) = pick.insert {
        let at3 = at.extend(validate::ground(s, at));
        match place_op(r, Tool::Checkpoint, at3, None, None, index, Vec2::X) {
            Ok(op) => {
                if undo::run(s, op).is_ok_and(|c| c.changed) {
                    s.point = Some(RacePoint { part: RacePart::Checkpoint, index });
                }
            }
            Err(e) => s.status = e,
        }
    }
    if let Some(c) = pick.show.and_then(|i| checks.get(i)) {
        validate::show(s, c);
    }
    if let Some(c) = pick.fix.and_then(|i| checks.get(i))
        && let Some(fix) = &c.fix
    {
        validate::apply(s, k, fix);
    }
}

#[cfg(test)]
#[path = "race_strip_tests.rs"]
mod tests;
