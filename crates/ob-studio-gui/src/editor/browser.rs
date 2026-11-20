//! The prop browser (bottom tab): the region's placeable solids with a
//! search, category filters and how many of each are placed; a tile is
//! dragged into the viewport (it lands on the ground under the pointer,
//! facing the camera's heading) or chosen and placed with Ctrl+D.

use std::collections::HashSet;

use egui::{Color32, Rect, RichText, Sense, Stroke, Ui};
use glam::Vec3;

use super::props::{camera_heading, colour, ground_under, place_new};
use super::state::{EditorState, Panel, project};
use crate::launch::theme::THEME_A;
use crate::tools::map::edit::MAX_PICK_RADIUS;
use crate::world::{HIDDEN_FLAG, World};

/// A browser filter. The categories other than knock-over go by the
/// solid's name (the game's own naming: XC_CONE, XB_BENCH …); a solid
/// can be in more than one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    KnockOver,
    Barriers,
    Signs,
    StreetFurniture,
    Trees,
    All,
}

impl Filter {
    pub const ALL: [Filter; 6] =
        [Filter::KnockOver, Filter::Barriers, Filter::Signs, Filter::StreetFurniture, Filter::Trees, Filter::All];

    pub fn name(self) -> &'static str {
        match self {
            Filter::KnockOver => "Knock-over",
            Filter::Barriers => "Barriers",
            Filter::Signs => "Signs",
            Filter::StreetFurniture => "Street furniture",
            Filter::Trees => "Trees",
            Filter::All => "All in region",
        }
    }

    fn words(self) -> &'static [&'static str] {
        match self {
            Filter::Barriers => &["BARRIER", "BARRICADE", "JERSEY", "FENCE", "CONE", "BARREL", "BLOCK"],
            Filter::Signs => &["SIGN", "BILLBOARD", "ARROW"],
            Filter::StreetFurniture => &[
                "BENCH", "HYDRANT", "DUMP", "TRASH", "BIN", "MAIL", "PHONE", "METER", "LAMP", "LIGHT", "POLE", "NEWS",
                "TABLE", "CHAIR", "PLANTER", "BOLLARD",
            ],
            Filter::Trees => &["TREE", "PALM", "BUSH", "PLANT", "SHRUB", "HEDGE"],
            Filter::KnockOver | Filter::All => &[],
        }
    }

    /// Whether `e` belongs to this filter.
    pub fn matches(self, e: &Entry) -> bool {
        match self {
            Filter::All => true,
            Filter::KnockOver => e.knock_over,
            f => {
                let n = e.name.to_ascii_uppercase();
                f.words().iter().any(|w| n.contains(w))
            }
        }
    }
}

/// One placeable solid.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub solid: usize,
    pub name: String,
    pub knock_over: bool,
    /// Shown placements of it in the world (with the mod).
    pub placed: usize,
}

/// The drag payload of a tile.
#[derive(Clone, Debug, PartialEq)]
pub struct DragSolid(pub String);

/// The solids a player can place: those the region places with a matrix
/// and small enough to pick, with their counts, by name.
pub fn entries(world: &World) -> Vec<Entry> {
    let knock: HashSet<usize> =
        world.smackables.iter().filter_map(|p| world.placements.get(p.placement)).map(|p| p.solid).collect();
    let mut placed = vec![0usize; world.solids.len()];
    let mut placeable = vec![false; world.solids.len()];
    for p in &world.placements {
        let Some(s) = world.solids.get(p.solid) else { continue };
        if p.flags & 0x0100 != 0 {
            continue;
        }
        let r = (glam::Vec3::from_array(s.bbox_max) - glam::Vec3::from_array(s.bbox_min)).length() * 0.5;
        if r > MAX_PICK_RADIUS || r < 0.05 {
            continue;
        }
        placeable[p.solid] = true;
        if p.flags & HIDDEN_FLAG == 0 {
            placed[p.solid] += 1;
        }
    }
    let mut out: Vec<Entry> = (0..world.solids.len())
        .filter(|&i| placeable[i])
        .map(|i| Entry {
            solid: i,
            name: world.solids[i].name.clone(),
            knock_over: knock.contains(&i),
            placed: placed[i],
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The entries passing `filter` and the search text (any word, case
/// ignored).
pub fn filtered<'a>(all: &'a [Entry], filter: Filter, search: &str) -> Vec<&'a Entry> {
    let words: Vec<String> = search.split_whitespace().map(|w| w.to_ascii_uppercase()).collect();
    all.iter()
        .filter(|e| filter.matches(e))
        .filter(|e| {
            let n = e.name.to_ascii_uppercase();
            words.iter().all(|w| n.contains(w.as_str()))
        })
        .collect()
}

/// The prop browser.
#[derive(Clone, Debug, Default)]
pub struct Browser {
    pub search: String,
    pub filter: Filter,
    /// The chosen tile's solid name (Ctrl+D places it).
    pub chosen: Option<String>,
    /// The entries and the world they were counted on (its placement
    /// count and the mod's edit count).
    cache: Option<((usize, usize), Vec<Entry>)>,
}

impl Browser {
    fn entries(&mut self, s: &EditorState) -> &[Entry] {
        let key = (s.session.world.placements.len(), s.session.map.props.len());
        if self.cache.as_ref().is_none_or(|c| c.0 != key) {
            self.cache = Some((key, entries(&s.session.world)));
        }
        &self.cache.as_ref().unwrap().1
    }

    /// Place the chosen tile's solid at `at` (Ctrl+D with no selection).
    /// True when one was placed.
    pub fn place_chosen(&mut self, s: &mut EditorState, at: Vec3) -> bool {
        let Some(solid) = self.chosen.clone() else { return false };
        let h = camera_heading(s);
        place_new(s, &solid, at, h).is_some()
    }

    /// The viewport as the drop target of a tile: a marker on the ground
    /// under the pointer while one is dragged over `rect`, the prop placed
    /// on release. True when one was placed.
    pub fn drop_target(
        &mut self,
        s: &mut EditorState,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: Rect,
    ) -> bool {
        if !egui::DragAndDrop::has_payload_of_type::<DragSolid>(ctx) {
            return false;
        }
        let (pos, released) = ctx.input(|i| (i.pointer.interact_pos(), i.pointer.any_released()));
        let Some(pos) = pos.filter(|p| rect.contains(*p)) else { return false };
        let ground = ground_under(s, rect, pos);
        if let Some(g) = ground
            && let Some(c) = project(s, rect, g)
        {
            let name = egui::DragAndDrop::payload::<DragSolid>(ctx).map(|p| p.0.clone()).unwrap_or_default();
            painter.circle_stroke(c, 10.0, Stroke::new(2.0, colour(THEME_A.lime)));
            painter.text(
                c + egui::vec2(14.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                egui::FontId::proportional(13.0),
                colour(THEME_A.lime),
            );
        }
        if !released {
            return false;
        }
        let Some(payload) = egui::DragAndDrop::take_payload::<DragSolid>(ctx) else { return false };
        let Some(at) = ground else {
            s.status = "No ground under the pointer".into();
            return false;
        };
        let h = camera_heading(s);
        place_new(s, &payload.0, at, h).is_some()
    }

    fn tile(&mut self, ui: &mut Ui, e: &Entry) {
        let chosen = self.chosen.as_deref() == Some(e.name.as_str());
        let id = ui.id().with(("prop-tile", e.solid));
        ui.dnd_drag_source(id, DragSolid(e.name.clone()), |ui| {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(104.0, 112.0), Sense::click());
            let p = ui.painter();
            let edge = if chosen { colour(THEME_A.lime) } else { Color32::from_white_alpha(23) };
            p.rect(
                rect,
                4.0,
                Color32::from_rgba_unmultiplied(168, 176, 160, 22),
                Stroke::new(1.0, edge),
                egui::StrokeKind::Inside,
            );
            glyph(p, Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 36.0), egui::vec2(40.0, 40.0)), e);
            let font = egui::FontId::proportional(12.0);
            let name = ellipsize(&e.name, 14);
            p.text(
                rect.left_bottom() + egui::vec2(6.0, -22.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                font.clone(),
                colour(THEME_A.ink),
            );
            let kind = format!("{} · {}", if e.knock_over { "knock-over" } else { "static" }, e.placed);
            p.text(
                rect.left_bottom() + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                kind,
                font,
                colour(THEME_A.dim),
            );
            if resp.clicked() {
                self.chosen = if chosen { None } else { Some(e.name.clone()) };
            }
            resp.on_hover_text(format!("{} — {} placed. Drag into the viewport.", e.name, e.placed));
        });
    }
}

/// `s` cut to `n` characters with an ellipsis.
fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

/// A simple drawn symbol for the tile's category (no thumbnails yet).
fn glyph(p: &egui::Painter, r: Rect, e: &Entry) {
    let col = if e.knock_over { colour(THEME_A.lime) } else { colour(THEME_A.dim) };
    let stroke = Stroke::new(2.0, col);
    let c = r.center();
    if Filter::Trees.matches(e) {
        p.circle_stroke(c - egui::vec2(0.0, 6.0), 13.0, stroke);
        p.line_segment([c + egui::vec2(0.0, 7.0), c + egui::vec2(0.0, 20.0)], stroke);
    } else if Filter::Signs.matches(e) {
        p.rect_stroke(
            Rect::from_center_size(c - egui::vec2(0.0, 8.0), egui::vec2(26.0, 16.0)),
            2.0,
            stroke,
            egui::StrokeKind::Middle,
        );
        p.line_segment([c, c + egui::vec2(0.0, 20.0)], stroke);
    } else if Filter::Barriers.matches(e) {
        let pts = vec![c + egui::vec2(-14.0, 18.0), c + egui::vec2(0.0, -18.0), c + egui::vec2(14.0, 18.0)];
        p.add(egui::Shape::closed_line(pts, stroke));
    } else {
        p.rect_stroke(Rect::from_center_size(c, egui::vec2(28.0, 28.0)), 3.0, stroke, egui::StrokeKind::Middle);
    }
}

impl Panel for Browser {
    fn ui(&mut self, s: &mut EditorState, ui: &mut Ui) {
        let all = self.entries(s).to_vec();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search solids… cone").desired_width(180.0));
            for f in Filter::ALL {
                let n = all.iter().filter(|e| f.matches(e)).count();
                if ui.selectable_label(self.filter == f, format!("{} ({n})", f.name())).clicked() {
                    self.filter = f;
                }
            }
            ui.label(
                RichText::new("Drag into the viewport, or choose one and press Ctrl+D at the pointer")
                    .small()
                    .color(colour(THEME_A.dim)),
            );
        });
        let shown = filtered(&all, self.filter, &self.search);
        if shown.is_empty() {
            ui.label(RichText::new("No solid matches").color(colour(THEME_A.dim)));
            return;
        }
        egui::ScrollArea::horizontal().id_salt("prop-browser").show(ui, |ui| {
            ui.horizontal(|ui| {
                for e in shown {
                    self.tile(ui, e);
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, knock: bool) -> Entry {
        Entry { solid: 0, name: name.into(), knock_over: knock, placed: 1 }
    }

    #[test]
    fn filters_and_search() {
        let all = vec![e("XC_CONE_01", true), e("XS_SIGN_STOP", false), e("XT_PALM_02", false), e("XB_BENCH_02", true)];
        let names = |f, q| filtered(&all, f, q).iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(Filter::KnockOver, ""), ["XC_CONE_01", "XB_BENCH_02"]);
        assert_eq!(names(Filter::Barriers, ""), ["XC_CONE_01"]);
        assert_eq!(names(Filter::Signs, ""), ["XS_SIGN_STOP"]);
        assert_eq!(names(Filter::Trees, ""), ["XT_PALM_02"]);
        assert_eq!(names(Filter::StreetFurniture, ""), ["XB_BENCH_02"]);
        assert_eq!(names(Filter::All, "cone"), ["XC_CONE_01"]);
        assert_eq!(names(Filter::All, "x 02"), ["XT_PALM_02", "XB_BENCH_02"]);
        assert!(names(Filter::Signs, "cone").is_empty());
    }

    #[test]
    fn long_names_are_cut() {
        assert_eq!(ellipsize("XC_BARRIER_03_LONG", 10), "XC_BARRIE…");
        assert_eq!(ellipsize("XC_CONE", 10), "XC_CONE");
    }
}
