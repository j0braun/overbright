//! The mod tree's Props branch: the mod's prop edits grouped as Moved,
//! Deleted and Added, each row naming its solid and section. A click
//! selects the placement, a double-click also asks the viewport to frame
//! it; the context menu resets or deletes.

use egui::{RichText, Ui};

use super::props::{colour, delete_selection, reset_selection};
use super::state::EditorState;
use crate::launch::theme::THEME_A;
use crate::tools::map::PropEdit;

/// One row of the branch: the edit, the placement it is in the world
/// (None when it names a placement or solid the region lacks).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub edit: usize,
    pub placement: Option<usize>,
    pub solid: String,
    /// The original's section (moved / deleted) or None (added).
    pub section: Option<u32>,
}

/// The rows by group: moved, deleted, added (in the mod's order). Only
/// this region's edits.
pub fn rows(s: &EditorState) -> [Vec<Row>; 3] {
    let mut out: [Vec<Row>; 3] = Default::default();
    let overlay = &s.session.world.map_overlay;
    for (k, e) in s.session.map.props.iter().enumerate() {
        if !e.region().eq_ignore_ascii_case(&s.session.region) {
            continue;
        }
        let (group, solid, section, placement) = match e {
            PropEdit::Move { target, .. } | PropEdit::Delete { target } => {
                let i = overlay.originals.iter().find(|(_, r)| *r == target).map(|(i, _)| *i);
                (usize::from(matches!(e, PropEdit::Delete { .. })), target.solid.clone(), Some(target.section), i)
            }
            PropEdit::Add { id, solid, .. } => {
                let i = overlay.placement_of_add(&s.session.folder, id);
                (2, solid.clone(), None, i)
            }
        };
        out[group].push(Row { edit: k, placement, solid, section });
    }
    out
}

/// What the branch asks of the others this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Point the camera at the selection.
    Frame,
}

/// The Props branch.
pub fn ui(s: &mut EditorState, ui: &mut Ui) -> Option<Event> {
    let groups = rows(s);
    let total: usize = groups.iter().map(Vec::len).sum();
    let mut event = None;
    egui::CollapsingHeader::new(RichText::new(format!("Props  ·  {total} {}", if total == 1 { "edit" } else { "edits" })).color(colour(THEME_A.ink)))
        .id_salt("tree-props")
        .default_open(true)
        .show(ui, |ui| {
            for (g, name) in ["Moved", "Deleted", "Added"].into_iter().enumerate() {
                let rows = &groups[g];
                egui::CollapsingHeader::new(format!("{name}  ·  {}", rows.len()))
                    .id_salt(("tree-props", g))
                    .default_open(!rows.is_empty() && g != 1)
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            ui.label(RichText::new("none").small().color(colour(THEME_A.dim)));
                        }
                        for r in rows {
                            event = event.or(row(s, ui, r, g == 2));
                        }
                    });
            }
        });
    event
}

fn row(s: &mut EditorState, ui: &mut Ui, r: &Row, added: bool) -> Option<Event> {
    let selected = r.placement.is_some_and(|i| s.selection.contains(&i));
    let right = match r.section {
        Some(sec) => format!("§{sec}"),
        None => "added".to_string(),
    };
    let text = match r.placement {
        Some(_) => RichText::new(format!("{}   {right}", r.solid)),
        None => RichText::new(format!("{}   missing", r.solid)).color(colour(THEME_A.warn)),
    };
    let resp = ui.selectable_label(selected, text);
    let resp = match r.placement {
        Some(_) => resp,
        None => resp.on_hover_text("The region has no such placement or solid: this edit does nothing"),
    };
    let Some(i) = r.placement else { return None };
    let mut event = None;
    if resp.clicked() {
        if ui.input(|inp| inp.modifiers.command) {
            match s.selection.iter().position(|&j| j == i) {
                Some(k) => {
                    s.selection.remove(k);
                }
                None => s.selection.push(i),
            }
        } else {
            s.selection = vec![i];
        }
    }
    if resp.double_clicked() {
        s.selection = vec![i];
        event = Some(Event::Frame);
    }
    resp.context_menu(|ui| {
        if !added && ui.button("Reset to original").clicked() {
            s.selection = vec![i];
            reset_selection(s);
            ui.close();
        }
        if ui.button("Delete").clicked() {
            s.selection = vec![i];
            delete_selection(s);
            ui.close();
        }
    });
    event
}
