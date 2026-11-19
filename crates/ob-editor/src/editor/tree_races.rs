//! The mod tree's Races branch: each race with its start grid, start
//! line, checkpoints and finish line, an error / tested badge per race,
//! and "New race…". A click selects (and switches to Races mode); a
//! double click also takes the camera there.

use super::races::{Races, point_xy};
use super::state::{EditorState, Level, Mode, RacePoint};
use super::validate::{self, Check};
use crate::tools::map::edit::RacePart;
use crate::tools::map::race;

fn rgb(c: crate::launch::theme::Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(c.0, c.1, c.2)
}

/// Draw the branch.
pub fn tree(races: &mut Races, s: &mut EditorState, ui: &mut egui::Ui) {
    races.update(s);
    let t = crate::launch::theme::THEME_A;
    let checks = races.checks.clone();
    let mut pick: Option<(usize, Option<RacePoint>, bool)> = None;
    egui::CollapsingHeader::new(format!("Races  ({})", s.session.map.races.len()))
        .id_salt("tree-races")
        .default_open(true)
        .show(ui, |ui| {
            for (k, r) in s.session.map.races.iter().enumerate() {
                let errors = checks.iter().filter(|c| c.race == Some(k) && c.level == Level::Error).count();
                let tested = checks.iter().any(|c| c.race == Some(k) && c.ok && c.text.ends_with(": tested"));
                let title = if r.name.is_empty() { r.id.clone() } else { r.name.clone() };
                let current = s.race == Some(k);
                let id = ui.make_persistent_id(("tree-race", k));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, current)
                    .show_header(ui, |ui| {
                        let resp = ui.selectable_label(current && s.point.is_none(), title);
                        if resp.clicked() {
                            pick = Some((k, None, false));
                        }
                        if errors > 0 {
                            let text = if errors == 1 { "1 error".to_string() } else { format!("{errors} errors") };
                            ui.label(egui::RichText::new(text).small().color(rgb(t.err)));
                        } else if tested {
                            ui.label(egui::RichText::new("tested").small().color(rgb(t.ok)));
                        }
                    })
                    .body(|ui| {
                        let mut row =
                            |ui: &mut egui::Ui, text: String, p: RacePoint, note: Option<(String, egui::Color32)>| {
                                ui.horizontal(|ui| {
                                    let resp = ui.selectable_label(current && s.point == Some(p), text);
                                    if resp.clicked() {
                                        pick = Some((k, Some(p), false));
                                    }
                                    if resp.double_clicked() {
                                        pick = Some((k, Some(p), true));
                                    }
                                    if let Some((n, c)) = note {
                                        ui.label(egui::RichText::new(n).small().color(c));
                                    }
                                });
                            };
                        let grid = race::effective_grid(r);
                        let note = if r.grid.is_empty() { "default" } else { "" };
                        row(
                            ui,
                            "Start grid".into(),
                            RacePoint { part: RacePart::Grid, index: 0 },
                            Some((format!("{} slots {note}", grid.len()), rgb(t.dim))),
                        );
                        if r.start.is_some() {
                            row(ui, "Start line".into(), RacePoint { part: RacePart::Start, index: 0 }, None);
                        }
                        for i in 0..r.checkpoints.len() {
                            let broken = checks.iter().any(|c: &Check| {
                                c.race == Some(k)
                                    && c.level == Level::Error
                                    && c.point == Some(RacePoint { part: RacePart::Checkpoint, index: i })
                            });
                            let note = broken.then(|| ("no road onward".to_string(), rgb(t.err)));
                            row(
                                ui,
                                format!("Checkpoint {}", i + 1),
                                RacePoint { part: RacePart::Checkpoint, index: i },
                                note,
                            );
                        }
                        if r.finish.is_some() {
                            row(ui, "Finish line".into(), RacePoint { part: RacePart::Finish, index: 0 }, None);
                        }
                    });
            }
            if ui.button("New race…").clicked() {
                races.new_race(s);
            }
        });
    if let Some((k, p, fly)) = pick {
        (s.race, s.point, s.mode) = (Some(k), p, Mode::Races);
        if fly && let Some(at) = p.and_then(|p| point_xy(&s.session.map.races[k], p)) {
            let c = Check {
                level: Level::Info,
                ok: true,
                text: String::new(),
                race: Some(k),
                point: p,
                at: Some(at),
                fix: None,
            };
            validate::show(s, &c);
        }
    }
}
