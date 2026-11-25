//! Race validation: what keeps a custom race from being raced (errors,
//! they block Test race), what is likely a mistake (warnings) and what is
//! fine (info), each with the spot "Show" takes the camera to and, where
//! one exists, a one-click fix ("Try base track N", "Snap grid to road").
//!
//! The checks are pure functions of the mod and the base tracks' road
//! networks; [`refresh`] runs them on the editor state and fills
//! `EditorState::issues`. Whether a race was tested since its last change
//! is kept in the mod's `.editor/tested.txt` ([`Tested`]); the mod's
//! exports leave it out.

use glam::Vec2;

pub use ob_studio::validate::*;

use super::state::{EditorState, Issue, Level, commit};

/// The ground under `p` in the edited world (0 with none).
pub fn ground(s: &EditorState, p: Vec2) -> f32 {
    s.session.world.grid_ground(p.extend(1.0e5)).unwrap_or(0.0)
}

/// Run the checks on `s` and fill `s.session.issues` with them.
pub fn refresh(s: &mut EditorState, region_tracks: &[u16], tested: &Tested) -> Vec<Check> {
    let mut out = checks(&s.session.map, &s.nets, region_tracks, tested);
    // The floor under the start, finish and grid, and the mod's drawn and removed collision (a world that was not
    // read has none to check against).
    let w = &s.session.world;
    if w.has_floors() {
        let under = |p| w.road_height_below(p);
        out.extend(ob_studio::validate::floor_checks(&s.session.map, &under));
        let c = &w.map_overlay.collision;
        out.extend(ob_studio::validate::collision_checks(&s.session.map, &under, &c.unmatched_at, &c.outside_at));
        out.sort_by_key(|c| (std::cmp::Reverse(c.level), c.ok));
    }
    s.session.issues = out
        .iter()
        .map(|c| Issue { level: c.level, text: c.text.clone(), race: c.race, at: c.at.map(|p| p.extend(ground(s, p))) })
        .collect();
    out
}

/// Apply `fix` to race `k` as one undo step.
pub fn apply(s: &mut EditorState, k: usize, fix: &Fix) {
    let Some(track) = s.session.map.races.get(k).map(|r| r.track) else { return };
    let net = s.nets.get(&track).and_then(|n| n.clone());
    let heights = |p: Vec2| s.session.world.grid_ground(p.extend(1.0e5)).unwrap_or(0.0);
    let mut map = s.session.map.clone();
    if fix_race(&mut map, k, fix, net.as_ref(), &heights) {
        s.session.map = map;
        commit(s, &fix.label());
    }
}

/// Take the camera to `at` (from above and behind, looking down at it)
/// and select the race part.
pub fn show(s: &mut EditorState, c: &Check) {
    if let Some(k) = c.race {
        s.race = Some(k);
        s.mode = super::state::Mode::Races;
    }
    s.point = c.point;
    if let Some(at) = c.at {
        let target = at.extend(ground(s, at));
        let f = s.camera.forward();
        let back = Vec2::new(f.x, f.y).normalize_or(Vec2::Y);
        let eye = target + (-back * 45.0).extend(40.0);
        let look = crate::draw::camera::Camera::looking_at(eye, target);
        (s.camera.position, s.camera.yaw, s.camera.pitch) = (look.position, look.yaw, look.pitch);
    }
}

/// The colour of a level (the launcher's semantic colours).
pub fn colour(c: &Check) -> egui::Color32 {
    let t = crate::launch::theme::THEME_A;
    let rgb = |c: crate::launch::theme::Rgb| egui::Color32::from_rgb(c.0, c.1, c.2);
    match c.level {
        Level::Error => rgb(t.err),
        Level::Warning => rgb(t.warn),
        Level::Info if c.ok => rgb(t.ok),
        Level::Info => rgb(t.info),
    }
}

/// A level's mark, drawn (the editor's fonts have no symbol glyphs): a
/// cross in a disc for an error, a triangle for a warning, a tick for a
/// passed check, a dot for information.
pub fn mark(ui: &mut egui::Ui, c: &Check) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let p = ui.painter();
    let col = colour(c);
    let o = rect.center();
    let stroke = egui::Stroke::new(1.8, col);
    match (c.level, c.ok) {
        (Level::Error, _) => {
            p.circle_stroke(o, 6.0, egui::Stroke::new(1.4, col));
            p.line_segment([o + egui::vec2(-3.0, -3.0), o + egui::vec2(3.0, 3.0)], stroke);
            p.line_segment([o + egui::vec2(-3.0, 3.0), o + egui::vec2(3.0, -3.0)], stroke);
        }
        (Level::Warning, _) => {
            let tri = vec![o + egui::vec2(0.0, -6.5), o + egui::vec2(6.5, 5.5), o + egui::vec2(-6.5, 5.5)];
            p.add(egui::Shape::closed_line(tri, egui::Stroke::new(1.4, col)));
            p.line_segment([o + egui::vec2(0.0, -2.5), o + egui::vec2(0.0, 1.5)], stroke);
            p.circle_filled(o + egui::vec2(0.0, 3.5), 0.9, col);
        }
        (_, true) => {
            p.line(vec![o + egui::vec2(-5.0, 0.5), o + egui::vec2(-1.5, 4.0), o + egui::vec2(5.0, -4.0)], stroke);
        }
        _ => {
            p.circle_filled(o, 2.5, col);
        }
    }
}

/// The Validation tab: one row per entry with Show and its fix.
pub fn panel(s: &mut EditorState, checks: &[Check], ui: &mut egui::Ui) -> bool {
    let mut changed = false;
    if checks.is_empty() {
        ui.label("No races yet: Race > New race.");
    }
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        for (i, c) in checks.iter().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal(|ui| {
                    mark(ui, c);
                    ui.label(&c.text);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let (Some(fix), Some(k)) = (&c.fix, c.race)
                            && ui.button(fix.label()).clicked()
                        {
                            apply(s, k, fix);
                            changed = true;
                        }
                        if (c.at.is_some() || c.point.is_some()) && ui.button("Show").clicked() {
                            show(s, c);
                        }
                    });
                });
            });
        }
        ui.label(
            egui::RichText::new(
                "Test race starts the game with this mod only, straight on the grid; the career save is not changed.",
            )
            .color(egui::Color32::from_rgb(0xc8, 0xcc, 0xc0)),
        );
    });
    changed
}

#[cfg(test)]
#[path = "validate_tests.rs"]
pub(crate) mod tests;
