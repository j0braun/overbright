//! The prop browser (bottom tab): the region's placeable solids as tiles
//! with their pictures (drawn by the game's renderer, kept in the asset
//! cache: [`super::thumbs`]), a search over every category (each
//! category's button counts its matches), category filters and how many
//! of each are placed; the tile under the pointer, else the chosen one, is
//! named in full on the details line under the tiles. A tile is dragged
//! into the viewport (it lands on the ground under the pointer, its front
//! towards the camera) or chosen and placed with Ctrl+D. The tiles are
//! buttons: Tab / the arrows reach them and Enter or Space chooses one.


use egui::{Color32, Rect, RichText, Sense, Stroke, Ui};
use glam::Vec3;

use super::props::{colour, ground_under, heading_to_camera, place_new};
use super::thumbs::Thumbs;
use super::state::{EditorState, Panel, project};
use crate::launch::theme::THEME_A;
#[cfg(test)]
use crate::world::World;

pub use ob_studio::browse::{Entry, Filter, categories, counts, details, entries, filtered};

/// The drag payload of a tile.
#[derive(Clone, Debug, PartialEq)]
pub struct DragSolid(pub String);

/// A tile's size (points): the picture, the name, the kind.
pub const TILE: egui::Vec2 = egui::vec2(112.0, 92.0);
/// Below this width (points) the filter buttons use their short names.
pub const NARROW: f32 = 900.0;
/// The picture's side in a tile (points).
pub const PICTURE: f32 = 54.0;

/// The prop browser.
#[derive(Default)]
pub struct Browser {
    pub search: String,
    pub filter: Filter,
    /// The chosen tile's solid name (Ctrl+D places it).
    pub chosen: Option<String>,
    /// The tile under the pointer or with the keyboard focus this frame.
    pub hovered: Option<usize>,
    /// The filter in force before a search began (back when it is
    /// cleared: a search looks in every category).
    before_search: Option<Filter>,
    /// The entries and the world they were counted on (its placement
    /// count and the mod's edit count).
    cache: Option<((usize, usize), Vec<Entry>)>,
    /// The tiles' pictures.
    pub thumbs: Thumbs,
    /// What the last frame laid out (tests).
    pub fit: Fit,
}

/// The browser's last layout: the area it was given (its clip), each
/// tile drawn and the details line with its words.
#[derive(Clone, Debug)]
pub struct Fit {
    pub clip: Rect,
    pub tiles: Vec<Rect>,
    /// The search field and the filter buttons.
    pub row: Vec<Rect>,
    pub details: Option<(Rect, String)>,
}

impl Default for Fit {
    fn default() -> Fit {
        Fit { clip: Rect::NOTHING, tiles: Vec::new(), row: Vec::new(), details: None }
    }
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
        let h = heading_to_camera(s, at);
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
        let h = heading_to_camera(s, at);
        place_new(s, &payload.0, at, h).is_some()
    }

    fn tile(&mut self, ui: &mut Ui, e: &Entry) {
        let chosen = self.chosen.as_deref() == Some(e.name.as_str());
        let id = ui.id().with(("prop-tile", e.solid));
        let picture = self.thumbs.texture(ui.ctx(), e.solid);
        let mut hovered = false;
        let tile = ui.dnd_drag_source(id, DragSolid(e.name.clone()), |ui| {
            let (rect, resp) = ui.allocate_exact_size(TILE, Sense::click());
            self.fit.tiles.push(rect);
            // Over the tile (the drag source above it takes the hover).
            let over = ui.rect_contains_pointer(rect);
            let p = ui.painter();
            let focus = resp.has_focus();
            let edge = if chosen || focus { colour(THEME_A.lime) } else { Color32::from_white_alpha(23) };
            let fill = if over { 34 } else { 22 };
            p.rect(
                rect,
                4.0,
                Color32::from_rgba_unmultiplied(168, 176, 160, fill),
                Stroke::new(if focus { 2.0 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );
            let pic = Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 5.0 + PICTURE / 2.0), egui::vec2(PICTURE, PICTURE));
            match picture {
                Some(t) => {
                    p.image(t, pic, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => glyph(p, Rect::from_center_size(pic.center(), egui::vec2(40.0, 40.0)), e),
            }
            let name = one_line(ui, &e.name, 12.0, colour(THEME_A.ink), rect.width() - 12.0);
            p.galley(rect.left_bottom() + egui::vec2(6.0, -32.0), name, colour(THEME_A.ink));
            let kind = format!("{} · {}", if e.knock_over { "knock-over" } else { "static" }, e.placed);
            p.text(
                rect.left_bottom() + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                kind,
                egui::FontId::proportional(11.0),
                colour(THEME_A.dim),
            );
            if resp.clicked() {
                self.chosen = if chosen { None } else { Some(e.name.clone()) };
            }
            hovered = over || focus;
        });
        let how = if super::menu::hosted(ui.ctx()) {
            "Drag into the viewport."
        } else {
            "Drag into the viewport, or choose it and press Ctrl+D."
        };
        tile.response.on_hover_text(format!("{}\n{}. {how}", e.name, categories(e)));
        if hovered {
            self.hovered = Some(e.solid);
        }
    }
}

/// `text` on one line at most `width` wide, cut with an ellipsis.
fn one_line(ui: &Ui, text: &str, size: f32, col: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), egui::FontId::proportional(size), col);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}

/// A simple drawn symbol for the tile's category (until its picture is
/// ready, or when the solid draws nothing).
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

impl Browser {
    /// A search in progress looks in every category ("All in region");
    /// cleared, the category before it comes back.
    fn follow_search(&mut self) {
        match (self.search.trim().is_empty(), self.before_search) {
            (false, None) => {
                self.before_search = Some(self.filter);
                self.filter = Filter::All;
            }
            (true, Some(f)) => {
                self.before_search = None;
                self.filter = f;
            }
            _ => {}
        }
    }
}

impl Panel for Browser {
    fn ui(&mut self, s: &mut EditorState, ui: &mut Ui) {
        let all = self.entries(s).to_vec();
        self.hovered = None;
        self.fit = Fit { clip: ui.clip_rect(), ..Default::default() };
        // Narrower than the row's long names: the short ones.
        let narrow = ui.available_width() < NARROW;
        ui.horizontal(|ui| {
            if narrow {
                ui.spacing_mut().item_spacing.x = 4.0;
            }
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(if narrow { "Search… cone" } else { "Search all solids… cone" })
                    .desired_width(if narrow { 112.0 } else { 180.0 }),
            );
            self.fit.row.push(r.rect);
            if r.changed() {
                self.follow_search();
            }
            for (f, n) in counts(&all, &self.search) {
                let name = if narrow { f.short() } else { f.name() };
                let r = ui.selectable_label(self.filter == f, format!("{name} ({n})"));
                let r = if narrow { r.on_hover_text(f.name()) } else { r };
                if r.clicked() {
                    self.filter = f;
                }
                self.fit.row.push(r.rect);
            }
        });
        let shown = filtered(&all, self.filter, &self.search);
        // The pictures of every solid shown (read from the cache now, the
        // others drawn after this frame).
        self.thumbs.request(&s.session.region, &s.session.world, shown.iter().map(|e| e.solid));
        if self.thumbs.pending() > 0 {
            ui.ctx().request_repaint();
        }
        if shown.is_empty() {
            ui.label(RichText::new("No solid matches").color(colour(THEME_A.dim)));
            return;
        }
        egui::ScrollArea::horizontal().id_salt("prop-browser").show(ui, |ui| {
            ui.horizontal(|ui| {
                for e in &shown {
                    self.tile(ui, e);
                }
            });
        });
        // The details line: the tile under the pointer, else the chosen one.
        let about = self
            .hovered
            .and_then(|i| shown.iter().copied().find(|e| e.solid == i))
            .or_else(|| self.chosen.as_deref().and_then(|n| all.iter().find(|e| e.name == n)));
        let mut line = match about {
            Some(e) => details(e),
            None if super::menu::hosted(ui.ctx()) => {
                format!("{} solids shown · point at a tile for its full name · drag one into the view", shown.len())
            }
            None => format!(
                "{} solids shown · point at a tile for its full name · drag one into the view, or choose it and press \
                 Ctrl+D at the pointer",
                shown.len()
            ),
        };
        if self.thumbs.pending() > 0 {
            line.push_str(&format!(" · drawing {} pictures…", self.thumbs.pending()));
        }
        let r = ui.label(RichText::new(&line).small().color(colour(THEME_A.dim)));
        self.fit.details = Some((r.rect, line));
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
    fn a_search_counts_its_matches_in_every_category_and_looks_in_all() {
        let all = vec![e("XC_CONE_01", true), e("XS_SIGN_STOP", false), e("XT_PALM_02", false), e("XB_BENCH_02", true)];
        let c = counts(&all, "02");
        let n = |f| c.iter().find(|x| x.0 == f).unwrap().1;
        assert_eq!((n(Filter::Trees), n(Filter::StreetFurniture), n(Filter::Signs), n(Filter::All)), (1, 1, 0, 2));
        let mut b = Browser { thumbs: Thumbs::new(None), filter: Filter::Signs, ..Default::default() };
        b.search = "cone".into();
        b.follow_search();
        assert_eq!(b.filter, Filter::All, "a search looks everywhere");
        b.search = "cones".into();
        b.follow_search();
        assert_eq!(b.filter, Filter::All);
        b.search.clear();
        b.follow_search();
        assert_eq!(b.filter, Filter::Signs, "cleared: the category before it");
    }

    #[test]
    fn the_details_line_names_the_solid_in_full() {
        let long = e("XO_UC_PARKRESTAURANTSIGN_1A_00", false);
        assert_eq!(details(&long), "XO_UC_PARKRESTAURANTSIGN_1A_00 · Signs · static · 1 placed in the region");
        assert_eq!(categories(&e("XC_BARRIERSIGN", true)), "Barriers, Signs");
        assert_eq!(categories(&e("XO_ROCK", true)), "Other");
    }

    /// A flat region with `n` props of long-named solids.
    fn props_world(n: usize) -> World {
        let mut w = World::flat(400.0);
        for k in 0..n {
            w.solids.push(crate::formats::solid::Solid {
                name: format!("XO_UC_PARKRESTAURANTSIGN_{k:02}_1A_00"),
                hash: k as u32 + 1,
                bbox_min: [-0.5, -0.5, 0.0],
                bbox_max: [0.5, 0.5, 1.0],
                textures: vec![],
                vertices: vec![],
                normals: vec![],
                light_materials: vec![],
                indices: vec![],
                materials: vec![],
                mesh_flags: 0,
                markers: vec![],
                light_markers: vec![],
            });
            w.placements.push(crate::world::Placement {
                solid: k,
                low_solid: None,
                mid_solid: Some(k),
                transform: glam::Mat4::from_translation(Vec3::new(k as f32 * 3.0, 0.0, 0.0)),
                section: 1,
                flags: 0,
                cull_radius: 1.0,
            });
        }
        w
    }

    /// The browser in the editor's window at four sizes (two at display
    /// scale 2): every tile laid out, at least a row of whole tiles in
    /// view, a long name cut inside its tile, the details line in view and
    /// naming the tile under the pointer in full.
    #[test]
    fn the_browser_fits_at_four_sizes() {
        use crate::editor::app::App;
        let sc = crate::testkit::Scratch::new("editor-browser");
        let mut a = App::new(sc.path().join("game"), sc.path().join("maps"), Box::new(|_, _| Ok(props_world(24))));
        a.new_mod("b", "Browser", "L4RA").unwrap();
        a.tab = "browser";
        a.panels.browser.borrow_mut().filter = Filter::Signs;
        for (w, h, scale) in [(1024.0, 768.0, 1.0), (1366.0, 768.0, 1.0), (2560.0, 1440.0, 2.0), (3840.0, 2160.0, 2.0)] {
            let frame = |a: &mut App, events: Vec<egui::Event>| {
                let mut raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w / scale, h / scale))),
                    events,
                    ..Default::default()
                };
                raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
                let mut out = a.frame(raw);
                out.textures_delta.clear();
            };
            for _ in 0..3 {
                frame(&mut a, vec![]);
            }
            let fit = a.panels.browser.borrow().fit.clone();
            assert_eq!(fit.tiles.len(), 24, "{w}x{h}: every tile laid out");
            let cut: Vec<_> = fit.row.iter().filter(|r| !fit.clip.expand(0.5).contains_rect(**r)).collect();
            assert!(cut.is_empty(), "{w}x{h}: search and filters cut off: {cut:?} in {:?}", fit.clip);
            let whole = fit.tiles.iter().filter(|t| fit.clip.contains_rect(**t)).count();
            assert!(whole >= 4, "{w}x{h}: {whole} whole tiles in {:?}", fit.clip);
            let (line, words) = fit.details.clone().expect("a details line");
            assert!(fit.clip.expand(0.5).contains_rect(line), "{w}x{h}: details {line:?} in {:?}", fit.clip);
            assert!(words.contains("24 solids shown"), "{words}");
            // The pointer on the first tile: its full name on the line.
            let first = fit.tiles[0];
            frame(&mut a, vec![egui::Event::PointerMoved(first.center())]);
            frame(&mut a, vec![egui::Event::PointerMoved(first.center())]);
            let fit = a.panels.browser.borrow().fit.clone();
            let words = fit.details.map(|d| d.1).unwrap_or_default();
            assert!(words.starts_with("XO_UC_PARKRESTAURANTSIGN_00_1A_00 · Signs · static · 1 placed"), "{w}x{h}: {words}");
            frame(&mut a, vec![egui::Event::PointerGone]);
        }
    }

}
