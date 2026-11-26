//! The Browser workspace (docs/modding/studio.md, "Browser"; `ob://<mod>/@browser`, Ctrl+4): the game's models and
//! textures of the region the open mod is edited on, searched by name across kinds, each with where it is used.
//! The index and the search are `ob_studio::browser`; this is the page that shows them. It only reads: a result
//! is looked at here (a model's picture from the prop browser's thumbnail renderer, a texture's pixels as Looks
//! reads them) and taken to where it is worked on (a model to World, a texture to Looks on the Changes page).
//!
//! The focus runs over the page's lines: the kind chips, the filter chips of that kind, then the results.
//! ↑ ↓ / Tab move over them, ← → change a chip, Enter / A takes the result to its place; typing searches.

use std::collections::HashMap;

use egui::{Align2, Color32, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use ob_studio::browse::{Filter, categories};
use ob_studio::browser::{Hit, Index, Kind, Query, Signature, TexFilter};

use super::changes::{small_button, thumb_image_at};
use super::draw;
use crate::editor::theme::{self, LABEL, TITLE, font, rgb, tokens};
use crate::editor::thumbs::Thumbs;
use crate::world::World;

/// A result row's height (points).
pub const ROW_H: f32 = 34.0;
/// The picture in a row (points) and the preview in the viewer.
const PIC: f32 = 26.0;
const PREVIEW: f32 = 176.0;
/// Below this page width the viewer sits under the list, and the filters use their short names.
pub const WIDE_FROM: f32 = 820.0;
/// The most entries a "used by" list shows.
const LIST_MAX: usize = 6;
/// Texture pictures made per frame (the rest follow on the next).
const TEX_PER_FRAME: usize = 8;
/// A texture's thumbnail side (pixels) and its preview's.
const TEX_THUMB_PX: u32 = 48;
const TEX_PREVIEW_PX: u32 = 256;

/// What the window gives the page for a frame: the loaded region, and the prop thumbnails.
pub struct Source<'a> {
    pub region: &'a str,
    pub world: &'a World,
    /// The mod's own props (the index is made again when they change).
    pub mod_props: usize,
    /// The prop browser's pictures (None: models show no picture).
    pub thumbs: Option<&'a mut Thumbs>,
}

/// What the page asks the window to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// Show a model in World: its first shown placement, when it has one (else it is chosen for Ctrl+D).
    ShowModel { solid: usize, name: String, placement: Option<usize> },
    /// Open a texture in Looks.
    OpenTexture { hash: u32, name: String },
    /// Export a model as a glTF reference for an outside 3D program (into the mod's `.editor/reference/`).
    ExportModel { solid: usize, name: String },
}

/// What the page drew where (the layout check).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub search: Option<Rect>,
    pub chips: Vec<Rect>,
    pub rows: Vec<Rect>,
    pub viewer: Option<Rect>,
    pub focus: Option<Rect>,
    pub buttons: Vec<Rect>,
}

/// The first line the results have (lines 0 and 1 are the chip rows).
pub const FIRST_RESULT: usize = 2;

pub struct Page {
    pub query: Query,
    /// The focused line: 0 the kinds, 1 their filters, 2… the results.
    pub focus: usize,
    index: Option<(Signature, Index)>,
    hits: Vec<Hit>,
    hits_for: Option<(Signature, Query)>,
    scroll: f32,
    tex_thumbs: HashMap<u32, Option<egui::TextureHandle>>,
    preview: Option<(u32, Option<egui::TextureHandle>)>,
    pub last: Drawn,
    /// A "used by" link or a chip clicked in the last frame, taken at the start of the next.
    pick: Option<Pick>,
    /// The focus to keep when the results are made again for a picked result.
    pending_focus: Option<usize>,
}

#[derive(Clone, Debug)]
enum Pick {
    Hit(Hit),
}

impl Default for Page {
    fn default() -> Page {
        Page::new()
    }
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "browser::Page({:?}, focus {})", self.query.text, self.focus)
    }
}

impl Page {
    pub fn new() -> Page {
        Page {
            query: Query::default(),
            focus: FIRST_RESULT,
            index: None,
            hits: vec![],
            hits_for: None,
            scroll: 0.0,
            tex_thumbs: HashMap::new(),
            preview: None,
            last: Drawn::default(),
            pick: None,
            pending_focus: None,
        }
    }

    /// The index of the loaded region, once made.
    pub fn index(&self) -> Option<&Index> {
        self.index.as_ref().map(|i| &i.1)
    }

    /// The results of the current query.
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    /// The query the results are for: the filters of a kind count only while that kind is chosen.
    fn effective(&self) -> Query {
        let mut q = self.query.clone();
        match q.kind {
            Kind::All => {
                q.models = Filter::All;
                q.textures = TexFilter::Any;
            }
            Kind::Models => q.textures = TexFilter::Any,
            Kind::Textures => q.models = Filter::All,
        }
        q
    }

    /// Make the index and the results current for `src` and the query.
    pub fn ensure(&mut self, src: &Source) {
        let sig = Index::signature(src.region, src.world, src.mod_props);
        if self.index.as_ref().is_none_or(|i| i.0 != sig) {
            self.index = Some((sig.clone(), Index::build(src.region, src.world)));
            self.hits_for = None;
            self.tex_thumbs.clear();
            self.preview = None;
        }
        let q = self.effective();
        if self.hits_for.as_ref().is_none_or(|h| h.0 != sig || h.1 != q) {
            let first = self.hits_for.is_none();
            self.hits = self.index.as_ref().map_or(vec![], |i| i.1.search(&q));
            self.hits_for = Some((sig, q));
            if let Some(f) = self.pending_focus.take() {
                self.focus = f;
            } else if !first {
                // The search changed: back to the first result (a chip line keeps the focus).
                self.focus = self.focus.min(FIRST_RESULT);
                self.scroll = 0.0;
            }
        }
        self.focus = self.focus.min(FIRST_RESULT + self.hits.len().saturating_sub(1));
    }

    /// The lines the focus runs over.
    pub fn lines(&self) -> usize {
        FIRST_RESULT + self.hits.len().max(1)
    }

    /// Move the focus by `d` lines; false past the ends.
    pub fn step_focus(&mut self, d: i32) -> bool {
        let last = self.lines().saturating_sub(1) as i32;
        let j = self.focus as i32 + d;
        if j < 0 || j > last {
            return false;
        }
        self.focus = j as usize;
        true
    }

    /// ← → on a chip line: the next or previous kind or filter.
    pub fn step_chip(&mut self, d: i32) -> bool {
        fn cycle<T: Copy + PartialEq>(all: &[T], cur: T, d: i32) -> T {
            let i = all.iter().position(|c| *c == cur).unwrap_or(0) as i32;
            all[(i + d).rem_euclid(all.len() as i32) as usize]
        }
        match self.focus {
            0 => self.query.kind = cycle(&Kind::ALL, self.query.kind, d),
            1 => match self.query.kind {
                Kind::Models => self.query.models = cycle(&Self::model_filters(), self.query.models, d),
                Kind::Textures => self.query.textures = cycle(&TexFilter::ALL, self.query.textures, d),
                Kind::All => return false,
            },
            _ => return false,
        }
        true
    }

    /// The models' filters in the chips' order (All first).
    pub fn model_filters() -> [Filter; 6] {
        [Filter::All, Filter::KnockOver, Filter::Barriers, Filter::Signs, Filter::StreetFurniture, Filter::Trees]
    }

    /// The focused result.
    pub fn focused_hit(&self) -> Option<Hit> {
        self.hits.get(self.focus.checked_sub(FIRST_RESULT)?).copied()
    }

    /// What Enter / A does: on a chip line the next chip; on a result its action.
    pub fn activate(&mut self) -> Option<Act> {
        if self.focus < FIRST_RESULT {
            self.step_chip(1);
            return None;
        }
        self.focused_hit().and_then(|h| self.act_for(h))
    }

    fn act_for(&self, h: Hit) -> Option<Act> {
        let idx = self.index()?;
        match h {
            Hit::Model(i) => {
                let m = idx.models.get(i)?;
                let placement = m.placements.iter().map(|&p| p as usize).next();
                Some(Act::ShowModel { solid: m.solid, name: m.name.clone(), placement })
            }
            Hit::Texture(i) => {
                let t = idx.textures.get(i)?;
                Some(Act::OpenTexture { hash: t.hash, name: t.name.clone() })
            }
        }
    }

    /// What Ctrl+E / X does: export the focused result when it is a model (a texture has no glTF).
    pub fn export_act(&self) -> Option<Act> {
        match self.focused_hit()? {
            Hit::Model(i) => {
                let m = self.index()?.models.get(i)?;
                Some(Act::ExportModel { solid: m.solid, name: m.name.clone() })
            }
            Hit::Texture(_) => None,
        }
    }

    /// The address of the focused result, as the shell copies it: `looks`' texture id or the model's name.
    pub fn focused_name(&self) -> Option<String> {
        self.focused_hit().and_then(|h| self.index().map(|i| i.name(h).to_string()))
    }

    /// Look at `h`: the search narrows to its name and it is the focused result.
    pub fn select(&mut self, h: Hit) {
        let Some(name) = self.index().map(|i| i.name(h).to_string()) else { return };
        self.query.text = name;
        self.query.kind = Kind::All;
        let q = self.effective();
        let hits = self.index().map(|i| i.search(&q)).unwrap_or_default();
        self.focus = FIRST_RESULT + hits.iter().position(|x| *x == h).unwrap_or(0);
        self.hits_for = None;
        self.pending_focus = Some(self.focus);
    }

    /// How the page says what it shows (the address bar's crumb after Browser).
    pub fn crumb(&self) -> Option<String> {
        self.focused_name()
    }

    fn tex_thumb(
        &mut self,
        ctx: &egui::Context,
        world: &World,
        hash: u32,
        budget: &mut usize,
    ) -> Option<egui::TextureId> {
        if let Some(t) = self.tex_thumbs.get(&hash) {
            return t.as_ref().map(|t| t.id());
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        let t = world.textures.get(&hash).and_then(|t| {
            thumb_image_at(t.width, t.height, &t.decode_rgba(), TEX_THUMB_PX)
                .map(|i| ctx.load_texture(format!("brw-tex-{hash:08x}"), i, egui::TextureOptions::NEAREST))
        });
        let id = t.as_ref().map(|t| t.id());
        self.tex_thumbs.insert(hash, t);
        id
    }

    fn tex_preview(&mut self, ctx: &egui::Context, world: &World, hash: u32) -> Option<egui::TextureId> {
        if self.preview.as_ref().is_none_or(|p| p.0 != hash) {
            let t = world.textures.get(&hash).and_then(|t| {
                thumb_image_at(t.width, t.height, &t.decode_rgba(), TEX_PREVIEW_PX)
                    .map(|i| ctx.load_texture(format!("brw-preview-{hash:08x}"), i, egui::TextureOptions::NEAREST))
            });
            self.preview = Some((hash, t));
        }
        self.preview.as_ref().and_then(|p| p.1.as_ref()).map(|t| t.id())
    }

    /// Draw the page into the slab's `inner` rect. `focused`: the page has the keys' focus (its ring shows);
    /// `typing`: the search field takes the typed letters.
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        inner: Rect,
        focused: bool,
        typing: bool,
        mut src: Option<Source>,
    ) -> Option<Act> {
        let t = tokens();
        let p = ui.painter().clone();
        let mut drawn = Drawn::default();
        let mut act = None;
        if let Some(Pick::Hit(h)) = self.pick.take() {
            self.select(h);
        }
        if let Some(s) = &src {
            self.ensure(s);
        }
        let mut clip = p.clone();
        clip.set_clip_rect(inner);

        // The title line: the name, and what is indexed.
        let sub = match (&src, self.index()) {
            (Some(_), Some(i)) => format!("{} models · {} textures in {}", i.models.len(), i.textures.len(), i.region),
            _ => String::new(),
        };
        let tg = clip.layout_no_wrap("Browser".into(), font(TITLE, 24.0), rgb(t.lime));
        let tw = tg.size().x;
        clip.galley(pos2(inner.left(), inner.top() + 14.0 - tg.size().y / 2.0), tg, rgb(t.lime));
        clip.text(
            pos2(inner.left() + tw + 16.0, inner.top() + 14.0),
            Align2::LEFT_CENTER,
            sub,
            font("body", 14.0),
            rgb(t.dim),
        );
        let Some(mut src) = src.take() else {
            clip.text(
                pos2(inner.left(), inner.top() + 60.0),
                Align2::LEFT_TOP,
                "No mod open: open one on Home to browse the game's models and textures of its region",
                font("body", 15.0),
                rgb(t.dim),
            );
            self.last = drawn;
            return None;
        };

        // The search field.
        let narrow = inner.width() < WIDE_FROM;
        let search = Rect::from_min_size(pos2(inner.left(), inner.top() + 36.0), vec2(inner.width().min(520.0), 28.0));
        let field = ui.put(
            search,
            egui::TextEdit::singleline(&mut self.query.text)
                .hint_text("Search models and textures by name, 0x hash or section:1407")
                .font(font("body", 15.0))
                .desired_width(search.width()),
        );
        if typing && !field.has_focus() {
            field.request_focus();
        }
        drawn.search = Some(search);
        let top = search.bottom() + 10.0;

        // The chip lines.
        let (nm, nt) = self.index().map_or((0, 0), |i| i.counts(&self.query.text));
        let mut x = inner.left();
        let kinds: Vec<(String, bool, Kind)> = Kind::ALL
            .iter()
            .map(|&k| {
                let n = match k {
                    Kind::All => nm + nt,
                    Kind::Models => nm,
                    Kind::Textures => nt,
                };
                (format!("{} {n}", k.name()), self.query.kind == k, k)
            })
            .collect();
        let line0 = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), 28.0));
        for (label, cur, k) in kinds {
            let r = self.chip(ui, &clip, &mut x, line0, &label, cur, focused && self.focus == 0);
            drawn.chips.push(r.0);
            if r.1 {
                self.query.kind = k;
            }
        }
        if focused && self.focus == 0 {
            drawn.focus = Some(line0);
        }
        let line1 = Rect::from_min_size(pos2(inner.left(), line0.bottom() + 4.0), vec2(inner.width(), 28.0));
        let mut x = inner.left();
        match self.query.kind {
            Kind::Models => {
                for f in Self::model_filters() {
                    let label = if narrow { f.short() } else { f.name() };
                    let r =
                        self.chip(ui, &clip, &mut x, line1, label, self.query.models == f, focused && self.focus == 1);
                    drawn.chips.push(r.0);
                    if r.1 {
                        self.query.models = f;
                    }
                }
            }
            Kind::Textures => {
                for f in TexFilter::ALL {
                    let r = self.chip(
                        ui,
                        &clip,
                        &mut x,
                        line1,
                        f.name(),
                        self.query.textures == f,
                        focused && self.focus == 1,
                    );
                    drawn.chips.push(r.0);
                    if r.1 {
                        self.query.textures = f;
                    }
                }
            }
            Kind::All => {
                clip.text(
                    pos2(inner.left() + 4.0, line1.center().y),
                    Align2::LEFT_CENTER,
                    "Choose Models or Textures to filter them",
                    font("body", 13.0),
                    rgb(t.dim),
                );
            }
        }
        if focused && self.focus == 1 {
            drawn.focus = Some(line1);
        }

        // The list and the viewer.
        let body_top = line1.bottom() + 10.0;
        let (list, viewer) = if narrow {
            let vh = ((inner.bottom() - body_top) * 0.45).clamp(120.0, 200.0);
            (
                Rect::from_min_max(pos2(inner.left(), body_top), pos2(inner.right(), inner.bottom() - vh - 8.0)),
                Rect::from_min_max(pos2(inner.left(), inner.bottom() - vh), inner.max),
            )
        } else {
            let split = inner.left() + (inner.width() * 0.52).min(640.0);
            (
                Rect::from_min_max(pos2(inner.left(), body_top), pos2(split, inner.bottom())),
                Rect::from_min_max(pos2(split + 20.0, body_top), inner.max),
            )
        };
        self.list(ui, &list, focused, &mut src, &mut drawn, &mut act);
        self.viewer(ui, &viewer, &mut src, &mut drawn, &mut act);
        self.last = drawn;
        act
    }

    /// One chip at `*x` in `line`; returns its rect and whether it was clicked.
    fn chip(
        &self,
        ui: &mut egui::Ui,
        clip: &egui::Painter,
        x: &mut f32,
        line: Rect,
        label: &str,
        cur: bool,
        ring: bool,
    ) -> (Rect, bool) {
        let t = tokens();
        let g = clip.layout_no_wrap(label.to_string(), draw::label_font(15.0), rgb(t.ink));
        let r = Rect::from_min_size(pos2(*x, line.top()), vec2(g.size().x + 20.0, line.height()));
        if cur {
            clip.rect_filled(r, 4.0, if ring { rgb(t.lime) } else { theme::over(t.pressed, t.slab.over(t.page)) });
        }
        let col = if cur && ring {
            rgb(t.on_play)
        } else if cur {
            rgb(t.lime)
        } else {
            rgb(t.ink)
        };
        clip.text(r.center(), Align2::CENTER_CENTER, label, draw::label_font(15.0), col);
        let clicked =
            ui.interact(r, egui::Id::new(("brw-chip", label, line.top() as i32)), egui::Sense::click()).clicked();
        *x = r.right() + 6.0;
        (r, clicked)
    }

    fn list(
        &mut self,
        ui: &mut egui::Ui,
        list: &Rect,
        focused: bool,
        src: &mut Source,
        drawn: &mut Drawn,
        act: &mut Option<Act>,
    ) {
        let t = tokens();
        let p = ui.painter().clone();
        let mut clip = p.clone();
        clip.set_clip_rect(*list);
        if self.hits.is_empty() {
            let line = if self.query.text.trim().is_empty() {
                "Nothing in this region"
            } else {
                "No model or texture matches"
            };
            clip.text(
                pos2(list.left() + 4.0, list.top() + 8.0),
                Align2::LEFT_TOP,
                line,
                font("body", 15.0),
                rgb(t.dim),
            );
            if let Some(m) = self.did_you_mean() {
                clip.text(
                    pos2(list.left() + 4.0, list.top() + 32.0),
                    Align2::LEFT_TOP,
                    format!("Did you mean `{m}`?"),
                    font("body", 14.0),
                    rgb(t.dim),
                );
            }
            return;
        }
        let rows_visible = (list.height() / ROW_H).floor().max(1.0);
        let content = self.hits.len() as f32 * ROW_H;
        if ui.rect_contains_pointer(*list) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        if self.focus >= FIRST_RESULT {
            let fi = (self.focus - FIRST_RESULT) as f32;
            if (fi + 1.0) * ROW_H - self.scroll > list.height() {
                self.scroll = (fi + 1.0) * ROW_H - list.height();
            }
            if fi * ROW_H < self.scroll {
                self.scroll = fi * ROW_H;
            }
        }
        self.scroll = self.scroll.clamp(0.0, (content - list.height()).max(0.0));
        let first = (self.scroll / ROW_H).floor() as usize;
        let last = (first + rows_visible as usize + 2).min(self.hits.len());
        // The pictures the visible rows want.
        if let Some(th) = src.thumbs.as_deref_mut() {
            let want: Vec<usize> = self.hits[first..last]
                .iter()
                .filter_map(|h| match h {
                    Hit::Model(i) => {
                        self.index().and_then(|x| x.models.get(*i)).map(|m| m.solid).filter(|&s| drawable(src.world, s))
                    }
                    Hit::Texture(_) => None,
                })
                .collect();
            th.request(src.region, src.world, want);
            if th.pending() > 0 {
                ui.ctx().request_repaint();
            }
        }
        let mut budget = TEX_PER_FRAME;
        for k in first..last {
            let h = self.hits[k];
            let y = list.top() + k as f32 * ROW_H - self.scroll;
            let row = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), ROW_H - 2.0));
            let is_focus = self.focus == FIRST_RESULT + k;
            let resp = ui.interact(row.intersect(*list), egui::Id::new(("brw-row", k)), egui::Sense::click());
            if resp.hovered() {
                clip.rect_filled(row, 6.0, theme::over(t.hover, t.slab.over(t.page)));
            }
            if is_focus && focused {
                clip.rect_stroke(row, 6.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Inside);
                drawn.focus = Some(row);
            } else if is_focus {
                clip.rect_filled(row, 6.0, theme::over(t.pressed, t.slab.over(t.page)));
            }
            if resp.clicked() {
                self.focus = FIRST_RESULT + k;
            }
            if resp.double_clicked() {
                *act = self.act_for(h);
            }
            let pic = Rect::from_min_size(pos2(row.left() + 6.0, row.center().y - PIC / 2.0), vec2(PIC, PIC));
            let (name, used, tag) = {
                let Some(idx) = self.index() else { return };
                match h {
                    Hit::Model(i) => (idx.models[i].name.clone(), short_used_model(idx, i), "model"),
                    Hit::Texture(i) => (idx.textures[i].name.clone(), short_used_texture(idx, i), "texture"),
                }
            };
            let tid = match h {
                Hit::Model(i) => {
                    let solid = self.index().map(|x| x.models[i].solid);
                    solid
                        .filter(|&s| drawable(src.world, s))
                        .and_then(|s| src.thumbs.as_deref_mut().and_then(|th| th.texture(ui.ctx(), s)))
                }
                Hit::Texture(i) => {
                    let hash = self.index().map(|x| x.textures[i].hash).unwrap_or(0);
                    self.tex_thumb(ui.ctx(), src.world, hash, &mut budget)
                }
            };
            frame_picture(&clip, pic, tid, matches!(h, Hit::Model(_)));
            let used_w = clip.layout_no_wrap(used.clone(), font("body", 13.0), rgb(t.dim)).size().x;
            let name_clip = {
                let mut c = clip.clone();
                c.set_clip_rect(
                    Rect::from_min_max(
                        pos2(pic.right() + 8.0, row.top()),
                        pos2(row.right() - used_w - 70.0, row.bottom()),
                    )
                    .intersect(*list),
                );
                c
            };
            name_clip.text(
                pos2(pic.right() + 10.0, row.center().y),
                Align2::LEFT_CENTER,
                name,
                font("body", 15.0),
                rgb(t.ink),
            );
            clip.text(
                pos2(row.right() - 8.0, row.center().y),
                Align2::RIGHT_CENTER,
                used,
                font("body", 13.0),
                rgb(t.dim),
            );
            clip.text(
                pos2(row.right() - 12.0 - used_w, row.center().y),
                Align2::RIGHT_CENTER,
                tag,
                font(LABEL, 12.0),
                rgb(t.dim),
            );
            if list.expand(1.0).contains_rect(row) {
                drawn.rows.push(row);
            }
        }
        if budget < TEX_PER_FRAME {
            ui.ctx().request_repaint();
        }
    }

    /// The nearest word of the search text among the names' words, when nothing matched.
    fn did_you_mean(&self) -> Option<String> {
        let idx = self.index()?;
        let words: Vec<String> = idx
            .models
            .iter()
            .map(|m| m.name.as_str())
            .chain(idx.textures.iter().map(|t| t.name.as_str()))
            .flat_map(|n| {
                n.split(|c: char| !c.is_ascii_alphanumeric()).map(str::to_ascii_lowercase).collect::<Vec<_>>()
            })
            .filter(|w| w.len() >= 3)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let fixed: Vec<String> = self
            .query
            .text
            .split_whitespace()
            .map(|w| ob_studio::browse::did_you_mean(w, words.iter().map(String::as_str)).unwrap_or(w).to_string())
            .collect();
        let fixed = fixed.join(" ");
        (!fixed.is_empty() && !fixed.eq_ignore_ascii_case(self.query.text.trim())).then_some(fixed)
    }

    fn viewer(&mut self, ui: &mut egui::Ui, r: &Rect, src: &mut Source, drawn: &mut Drawn, act: &mut Option<Act>) {
        let t = tokens();
        let p = ui.painter().clone();
        p.rect_filled(*r, 8.0, theme::over(t.panel, t.slab.over(t.page)));
        let mut clip = p.clone();
        clip.set_clip_rect(r.shrink(1.0));
        drawn.viewer = Some(*r);
        let Some(h) = self.focused_hit() else {
            clip.text(
                r.left_top() + vec2(14.0, 14.0),
                Align2::LEFT_TOP,
                "Choose a result to see it and where it is used",
                font("body", 14.0),
                rgb(t.dim),
            );
            return;
        };
        let wide = r.width() >= 420.0 || r.height() >= 300.0;
        let pad = 14.0;
        // The picture, at the left (the preview is smaller in a strip).
        let side = if r.height() < 260.0 { (r.height() - 2.0 * pad).clamp(64.0, PREVIEW) } else { PREVIEW };
        let pic = Rect::from_min_size(r.left_top() + vec2(pad, pad), vec2(side, side.min(r.height() - 2.0 * pad)));
        let (name, kind_line, lines, links, button): (String, String, Vec<String>, Vec<(String, Hit)>, &str) = {
            let Some(idx) = self.index() else { return };
            match h {
                Hit::Model(i) => {
                    let m = &idx.models[i];
                    let e = m.entry();
                    let mut lines = vec![
                        idx.model_used(i),
                        format!(
                            "{} triangles · {} materials",
                            m.triangles,
                            m.textures.iter().map(|x| x.1).sum::<usize>()
                        ),
                    ];
                    if m.as_lod > 0 && !m.placements.is_empty() {
                        lines.push(format!("Also the lower detail of {} placements", m.as_lod));
                    }
                    if !m.sections.is_empty() {
                        lines.push(format!(
                            "Sections: {}",
                            list_words(m.sections.iter().map(|s| format!("{} ×{}", s.0, s.1)), m.sections.len())
                        ));
                    }
                    let links: Vec<(String, Hit)> = m
                        .textures
                        .iter()
                        .take(LIST_MAX)
                        .filter_map(|&(hash, n)| {
                            let ti = idx.texture_of_hash(hash)?;
                            Some((format!("{} ×{n}", idx.textures[ti].name), Hit::Texture(ti)))
                        })
                        .collect();
                    (
                        m.name.clone(),
                        format!("Model · {} · {}", categories(&e), if m.knock_over { "knock-over" } else { "static" }),
                        lines,
                        links,
                        "Show in World",
                    )
                }
                Hit::Texture(i) => {
                    let tx = &idx.textures[i];
                    let mut lines = vec![idx.texture_used(i)];
                    let secs = idx.texture_sections(i);
                    if !secs.is_empty() {
                        lines.push(format!(
                            "Seen in sections: {}",
                            list_words(secs.iter().map(|s| s.0.to_string()), secs.len())
                        ));
                    }
                    let links: Vec<(String, Hit)> = tx
                        .used_by
                        .iter()
                        .take(LIST_MAX)
                        .map(|&(m, n)| (format!("{} ×{n}", idx.models[m].name), Hit::Model(m)))
                        .collect();
                    (
                        tx.name.clone(),
                        format!(
                            "Texture · {}×{}{} · {}",
                            tx.width,
                            tx.height,
                            if tx.animated { " · animated" } else { "" },
                            tx.id()
                        ),
                        lines,
                        links,
                        "Open in Looks",
                    )
                }
            }
        };
        // The picture.
        let tid = match h {
            Hit::Model(i) => {
                let solid = self.index().map(|x| x.models[i].solid);
                solid.filter(|&s| drawable(src.world, s)).and_then(|s| {
                    src.thumbs.as_deref_mut().and_then(|th| {
                        th.request(src.region, src.world, [s]);
                        th.texture(ui.ctx(), s)
                    })
                })
            }
            Hit::Texture(i) => {
                let hash = self.index().map(|x| x.textures[i].hash).unwrap_or(0);
                self.tex_preview(ui.ctx(), src.world, hash)
            }
        };
        frame_picture(&clip, pic, tid, matches!(h, Hit::Model(_)));
        if tid.is_none()
            && let Hit::Model(i) = h
        {
            let word = match self.index().map(|x| x.models[i].solid) {
                Some(s) if drawable(src.world, s) => "Drawing…",
                _ => "No picture",
            };
            clip.text(pic.center(), Align2::CENTER_CENTER, word, font("body", 13.0), rgb(t.dim));
        }
        // The words, right of the picture (under it in a strip too narrow for both).
        let tx = pic.right() + 16.0;
        let text_w = (r.right() - pad - tx).max(40.0);
        let _ = wide;
        let mut y = r.top() + pad;
        let mut line = |clip: &egui::Painter, text: &str, size: f32, col: Color32, y: &mut f32| {
            let g = clip.layout(text.to_string(), font("body", size), col, text_w);
            let h = g.size().y;
            clip.galley(pos2(tx, *y), g, col);
            *y += h + 4.0;
        };
        {
            let g = clip.layout(name.clone(), draw::label_font(18.0), rgb(t.lime), text_w);
            let h = g.size().y;
            clip.galley(pos2(tx, y), g, rgb(t.lime));
            y += h + 4.0;
        }
        line(&clip, &kind_line, 13.0, rgb(t.dim), &mut y);
        for l in &lines {
            line(&clip, l, 14.0, rgb(t.ink), &mut y);
        }
        if !links.is_empty() && y + 20.0 < r.bottom() {
            let head = if matches!(h, Hit::Model(_)) { "Textures" } else { "Used by models" };
            clip.text(pos2(tx, y), Align2::LEFT_TOP, head, font(LABEL, 13.0), rgb(t.dim));
            y += 18.0;
            for (label, to) in links {
                if y + 20.0 > r.bottom() - 34.0 {
                    break;
                }
                let lr = Rect::from_min_size(pos2(tx, y), vec2(text_w, 20.0));
                let resp = ui.interact(lr, egui::Id::new(("brw-link", &label)), egui::Sense::click());
                let col = if resp.hovered() { rgb(t.lime) } else { rgb(t.ink) };
                let mut c = clip.clone();
                c.set_clip_rect(lr.intersect(r.shrink(1.0)));
                c.text(lr.left_center(), Align2::LEFT_CENTER, &label, font("body", 14.0), col);
                if resp.clicked() {
                    self.pick = Some(Pick::Hit(to));
                }
                y += 22.0;
            }
        }
        // The jump button, under the picture's column.
        let br = Rect::from_min_size(pos2(r.left() + pad, r.bottom() - 34.0), vec2(150.0, 24.0));
        if br.top() > pic.bottom() - 4.0 || br.left() > pic.right() {
            if ui.put(br, small_button(button, false)).clicked() {
                *act = self.act_for(h);
            }
            drawn.buttons.push(br);
            // A model also goes to an outside 3D program, as a reference marked as from the game files.
            let er = Rect::from_min_size(br.right_top() + vec2(8.0, 0.0), vec2(120.0, 24.0));
            if matches!(h, Hit::Model(_)) && er.right() <= r.right() - 4.0 {
                if ui.put(er, small_button("Export glTF", false)).clicked() {
                    *act = self.export_act();
                }
                drawn.buttons.push(er);
            }
        }
    }
}

/// Whether the prop renderer can draw solid `i` (a model with no geometry has no picture, and a world of such
/// models cannot be drawn).
fn drawable(world: &World, i: usize) -> bool {
    world.solids.get(i).is_some_and(|s| !s.indices.is_empty() && !s.vertices.is_empty() && !s.materials.is_empty())
}

fn short_used_model(idx: &Index, i: usize) -> String {
    let m = &idx.models[i];
    if m.placements.is_empty() { "not placed".into() } else { format!("{} placed", m.placed()) }
}

fn short_used_texture(idx: &Index, i: usize) -> String {
    let n = idx.textures[i].used_by.len();
    if n == 0 { "unused".into() } else { format!("{n} models") }
}

/// The first few of `n` words joined, with how many more.
fn list_words(words: impl Iterator<Item = String>, n: usize) -> String {
    const SHOWN: usize = 6;
    let v: Vec<String> = words.take(SHOWN).collect();
    if n > SHOWN { format!("{} … (+{})", v.join(", "), n - SHOWN) } else { v.join(", ") }
}

/// A picture in `r`: framed; a model's on the page's colour, a texture's over a checker so its alpha shows.
fn frame_picture(p: &egui::Painter, r: Rect, t: Option<egui::TextureId>, model: bool) {
    let tk = tokens();
    p.rect_filled(r, 4.0, draw::page_fill());
    if let Some(id) = t {
        let _ = model;
        // A texture keeps its shape inside the square.
        p.image(id, r.shrink(1.0), Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
    }
    p.rect_stroke(r, 4.0, Stroke::new(1.0, rgb(tk.outline)), StrokeKind::Inside);
}
