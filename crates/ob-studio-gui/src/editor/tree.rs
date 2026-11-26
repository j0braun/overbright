//! The left column: one tree for the whole mod. The Mod card on top (its
//! name, author and what it changes; a click opens its details), then the
//! Races branch, then the Props branch. There are no modes: selecting a
//! race or a race point brings the race tools forward, selecting a prop
//! the prop tools. Folded, the column is a narrow rail with the same
//! three names written upwards; a click opens it again.

use super::menu::Action;
use super::state::{EditorState, Panel};
use super::theme;

/// The rail's width (points).
pub const RAIL: f32 = 28.0;

/// The column's header: the title and the fold button.
pub fn header(ui: &mut egui::Ui, title: &str, fold: Fold) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(theme::title(title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clicked = chevron(ui, fold).clicked();
        });
    });
    clicked
}

/// Which way a fold button points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    Left,
    Right,
    Down,
    Up,
}

/// A small square button with a double chevron (drawn: the faces have no
/// arrow glyphs).
pub fn chevron(ui: &mut egui::Ui, dir: Fold) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    let tip = match dir {
        Fold::Left | Fold::Down => "Fold",
        Fold::Right | Fold::Up => "Unfold",
    };
    let v = ui.style().interact(&resp);
    ui.painter().rect(rect, 3.0, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
    let c = rect.center();
    let stroke = egui::Stroke::new(1.3, if resp.hovered() { theme::lime() } else { theme::dim() });
    for off in [-2.5_f32, 2.5] {
        let pts = match dir {
            Fold::Left => {
                [c + egui::vec2(off + 2.0, -4.0), c + egui::vec2(off - 2.0, 0.0), c + egui::vec2(off + 2.0, 4.0)]
            }
            Fold::Right => {
                [c + egui::vec2(off - 2.0, -4.0), c + egui::vec2(off + 2.0, 0.0), c + egui::vec2(off - 2.0, 4.0)]
            }
            Fold::Down => {
                [c + egui::vec2(-4.0, off - 2.0), c + egui::vec2(0.0, off + 2.0), c + egui::vec2(4.0, off - 2.0)]
            }
            Fold::Up => {
                [c + egui::vec2(-4.0, off + 2.0), c + egui::vec2(0.0, off - 2.0), c + egui::vec2(4.0, off + 2.0)]
            }
        };
        ui.painter().line(pts.to_vec(), stroke);
    }
    resp.on_hover_text(tip)
}

/// How many prop edits the mod has.
fn prop_count(s: &EditorState) -> usize {
    s.session.map.props.len()
}

/// The Mod card: name, "by <author> · Details…", chips with what it
/// changes. True when it was clicked (the details open).
pub fn mod_card(s: &EditorState, ui: &mut egui::Ui) -> bool {
    let name = if s.session.map.name.is_empty() { s.session.folder.clone() } else { s.session.map.name.clone() };
    let by = if s.session.map.author.trim().is_empty() { "No author yet".to_string() } else { format!("by {}", s.session.map.author) };
    let r = egui::Frame::new()
        .fill(theme::rgb(theme::tokens().page))
        .stroke(egui::Stroke::new(1.0, theme::dim().gamma_multiply(0.4)))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(egui::Label::new(egui::RichText::new(name).font(theme::font(theme::SEMI, 15.0))).truncate());
            ui.add(
                egui::Label::new(egui::RichText::new(format!("{by} · Details…")).size(12.0).color(theme::dim()))
                    .truncate(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                chip(ui, &format!("Races {}", s.session.map.races.len()));
                chip(ui, &format!("Props {}", prop_count(s)));
            });
        });
    let resp = ui.interact(r.response.rect, ui.scope_id().with("mod-card"), egui::Sense::click());
    resp.on_hover_text("The mod's name, author and description").clicked()
}

fn chip(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, theme::dim().gamma_multiply(0.6)))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| ui.label(egui::RichText::new(text).size(12.0).color(theme::ink())));
}

/// The whole tree: header with the fold button (when `with_header`; Studio
/// names the panel in its own tab strip), the Mod card, then each branch
/// (Races, Props) in a scroll area. True when the fold button was clicked.
pub fn show(
    s: &mut EditorState,
    ui: &mut egui::Ui,
    branches: &mut [Box<dyn Panel>],
    out: &mut Vec<Action>,
    with_header: bool,
) -> bool {
    ui.set_min_width(ui.available_width());
    let fold = with_header && header(ui, "Mod", Fold::Left);
    if mod_card(s, ui) {
        out.push(Action::Details);
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]).show(ui, |ui| {
        // Branch 0 is Races, branch 1 Props: a workspace shows its own.
        for (k, b) in branches.iter_mut().enumerate() {
            let show = match s.focus {
                super::state::Focus::Both => true,
                super::state::Focus::World => k == 1,
                super::state::Focus::Races => k == 0,
            };
            if show {
                b.ui(s, ui);
            }
        }
    });
    fold
}

/// The rail's labels: "Mod", "Races N", "Props N".
pub fn rail_labels(s: Option<&EditorState>) -> Vec<String> {
    match s {
        Some(s) => {
            let races = format!("Races {}", s.session.map.races.len());
            let props = format!("Props {}", prop_count(s));
            match s.focus {
                super::state::Focus::Both => vec!["Mod".into(), races, props],
                super::state::Focus::World => vec!["Mod".into(), props],
                super::state::Focus::Races => vec!["Mod".into(), races],
            }
        }
        None => vec!["Mod".into()],
    }
}

/// A folded column: the unfold chevron, then `labels` written upwards.
/// True when it was clicked.
pub fn rail(ui: &mut egui::Ui, labels: &[String], dir: Fold) -> bool {
    let mut open = chevron(ui, dir).clicked();
    ui.add_space(6.0);
    for l in labels {
        let galley = ui.painter().layout_no_wrap(l.clone(), theme::font(theme::LABEL, 14.0), theme::ink());
        let size = galley.size();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(RAIL - 8.0, size.x + 14.0), egui::Sense::click());
        let col = if resp.hovered() { theme::lime() } else { theme::ink() };
        // Turned a quarter anticlockwise: read from the bottom up.
        let pos = egui::pos2(rect.center().x - size.y / 2.0, rect.bottom() - 7.0);
        ui.painter().add(egui::epaint::TextShape::new(pos, galley, col).with_angle(-std::f32::consts::FRAC_PI_2));
        open |= resp.on_hover_text("Unfold").clicked();
    }
    open
}
