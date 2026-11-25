//! The New mod dialog (Home's "Empty map mod", File ▸ New mod…, Ctrl+N): a name, the folder made from it (or
//! typed), checked as it is typed and made the one way every program makes a mod (`ob_studio::create`, as the map
//! editor's New map mod… and `overbright-studio-cli pack create` do). The region is the one the map editor starts
//! on: a map mod's file does not depend on it.
//!
//! Keys and pad: Tab / ↓ and Shift+Tab / ↑ (D-pad) move between Name, Folder, Create and Cancel; Enter / A takes
//! the focused one (in a field: Create); Esc / B cancels.

use std::path::Path;

use egui::{Align2, Color32, CornerRadius, Rect, RichText, Stroke, StrokeKind, UiBuilder, pos2, vec2};
use ob_look::lamp::Mark;
use ob_studio::create::{self, Refused, Template};

use crate::editor::theme::{self, TITLE, font, rgb, tokens};

use super::draw;
use super::words;

/// A template's row on Home: its name and the line under it.
pub fn template_words(t: Template) -> (&'static str, &'static str) {
    match t {
        Template::Empty => (words::EMPTY_MAP_MOD, words::EMPTY_MAP_MOD_LINE),
        Template::Sprint => (words::SPRINT_RACE, words::SPRINT_RACE_LINE),
        Template::Circuit => (words::CIRCUIT_RACE, words::CIRCUIT_RACE_LINE),
        Template::Sample => (words::SAMPLE_RACE_PACK, words::SAMPLE_LINE),
    }
}

/// The dialog's controls, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Name,
    Folder,
    Create,
    Cancel,
}

impl Part {
    pub const ALL: [Part; 4] = [Part::Name, Part::Folder, Part::Create, Part::Cancel];
}

/// What the dialog asks of the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Make the mod: its folder and name.
    Create {
        folder: String,
        name: String,
        template: Template,
    },
    Cancel,
}

/// The open dialog.
#[derive(Clone, Debug, Default)]
pub struct Dialog {
    /// What the mod starts from (Home's templates; Empty map mod: nothing).
    pub template: Template,
    pub name: String,
    /// Empty: the folder made from the name.
    pub folder: String,
    pub focus: usize,
    /// Create was tried: a refusal shows even for an empty name.
    pub tried: bool,
    /// Why the last Create did not make the mod (the disk's reasons; the others show as typed).
    pub failed: Option<String>,
    /// The check of the name and folder as last typed (the inputs and the answer), read again only on a change.
    checked: Option<((String, String), Result<String, Refused>)>,
}

impl Dialog {
    pub fn new() -> Dialog {
        Dialog::default()
    }

    pub fn part(&self) -> Part {
        Part::ALL[self.focus.min(Part::ALL.len() - 1)]
    }

    /// Tab / arrows / D-pad: the next or previous control (wraps).
    pub fn step(&mut self, d: i32) {
        let n = Part::ALL.len() as i32;
        self.focus = (self.focus as i32 + d).rem_euclid(n) as usize;
    }

    /// The folder the mod would get, or why it cannot be made.
    pub fn check(&mut self, maps: &Path) -> Result<String, Refused> {
        let key = (self.name.clone(), self.folder.clone());
        if let Some((k, r)) = &self.checked
            && *k == key
        {
            return r.clone();
        }
        let r = create::plan(maps, &self.name, Some(&self.folder));
        self.checked = Some((key, r.clone()));
        r
    }

    /// Enter / A: what the focused control does.
    pub fn activate(&mut self, maps: &Path) -> Option<Ask> {
        match self.part() {
            Part::Cancel => Some(Ask::Cancel),
            _ => self.try_create(maps),
        }
    }

    /// Create: the mod to make, or nothing (the refusal shows).
    pub fn try_create(&mut self, maps: &Path) -> Option<Ask> {
        self.tried = true;
        self.failed = None;
        match self.check(maps) {
            Ok(folder) => Some(Ask::Create { folder, name: self.name.trim().to_string(), template: self.template }),
            Err(_) => None,
        }
    }

    /// Draw it over `screen` (a card in the middle, the window dimmed). `focused`: a key or a pad is used, so the
    /// focused control is ringed.
    pub fn draw(&mut self, ui: &mut egui::Ui, screen: Rect, maps: &Path, focused: bool) -> Option<Ask> {
        let t = tokens();
        let mut ask = None;
        let check = self.check(maps);
        let area = egui::Area::new(egui::Id::unique("studio.new-mod"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .fade_in(false)
            .interactable(true);
        area.show(ui.ctx(), |ui| {
            let p = ui.painter().clone();
            // The window under it: dimmed, and its clicks taken.
            p.rect_filled(screen, 0.0, Color32::from_black_alpha(150));
            let _ = ui.interact(screen, egui::Id::unique("studio.new-mod.scrim"), egui::Sense::click());
            let w = (screen.width() - 48.0).min(560.0);
            let field_w = w - 48.0;
            // Name, folder, the line under them, the buttons.
            let h = 60.0 + 26.0 + 2.0 * 70.0 + 46.0 + 56.0;
            let card = Rect::from_center_size(screen.center(), vec2(w, h));
            let under = t.dialog;
            p.rect_filled(card, 8.0, rgb(under));
            p.rect_filled(
                Rect::from_min_size(card.min, vec2(4.0, card.height())),
                CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 },
                rgb(t.lime),
            );
            p.text(
                card.left_top() + vec2(24.0, 32.0),
                Align2::LEFT_CENTER,
                words::NEW_MOD_TITLE,
                font(TITLE, 22.0),
                rgb(t.ink),
            );
            let line = match self.template {
                Template::Empty => words::NEW_MOD_LINE.to_string(),
                t => format!("{}: {}", words::STARTS_WITH, template_words(t).0),
            };
            p.text(card.left_top() + vec2(24.0, 62.0), Align2::LEFT_CENTER, line, font("body", 14.0), rgb(t.dim));
            let mut y = card.top() + 86.0;
            let derived = create::folder_from(&self.name);
            for part in [Part::Name, Part::Folder] {
                let label = if part == Part::Name { words::NAME } else { words::FOLDER };
                p.text(pos2(card.left() + 24.0, y + 8.0), Align2::LEFT_CENTER, label, font("body", 14.0), rgb(t.dim));
                let r = Rect::from_min_size(pos2(card.left() + 24.0, y + 20.0), vec2(field_w, 36.0));
                p.rect_filled(r, 4.0, theme::over(t.field, under));
                p.rect_stroke(r, 4.0, Stroke::new(1.0, theme::over(t.field_edge, under)), StrokeKind::Inside);
                let (text, hint) = match part {
                    Part::Name => (&mut self.name, words::NAME_HINT.to_string()),
                    _ => (&mut self.folder, derived.clone()),
                };
                let te = egui::TextEdit::singleline(text)
                    .frame(egui::Frame::NONE)
                    .font(font("body", 15.0))
                    .char_limit(if part == Part::Name { create::MAX_NAME + 1 } else { 40 })
                    .hint_text(RichText::new(hint).color(rgb(t.placeholder)));
                let resp = ui.put(r.shrink2(vec2(10.0, 6.0)), te);
                let i = Part::ALL.iter().position(|x| *x == part).unwrap_or(0);
                if resp.gained_focus() || resp.clicked() {
                    self.focus = i;
                }
                if self.focus == i && !resp.has_focus() {
                    resp.request_focus();
                }
                if self.focus == i && focused {
                    p.rect_stroke(r.expand(2.0), 6.0, Stroke::new(2.0, rgb(t.lime)), StrokeKind::Outside);
                }
                y += 70.0;
            }
            // The check: where the mod goes, or why it cannot be made (no name is only said after Create).
            let (mark, line) = match (&check, &self.failed) {
                (_, Some(f)) => (Mark::Error, f.clone()),
                (Ok(folder), None) => (Mark::Ok, format!("{} {folder}", words::NEW_MOD_GOES)),
                (Err(Refused::NoName), None) if !self.tried => (Mark::Info, words::NEW_MOD_NAME_FIRST.to_string()),
                (Err(r), None) => (Mark::Error, r.to_string()),
            };
            draw::lamp(&p, pos2(card.left() + 34.0, y + 12.0), 11.0, mark, 0.0);
            let mut clip = p.clone();
            clip.set_clip_rect(Rect::from_min_max(pos2(card.left() + 48.0, y), pos2(card.right() - 16.0, y + 26.0)));
            clip.text(pos2(card.left() + 48.0, y + 12.0), Align2::LEFT_CENTER, line, font("body", 14.0), rgb(t.ink));
            y += 46.0;
            let buttons = Rect::from_min_size(pos2(card.left() + 24.0, y), vec2(field_w, 40.0));
            ui.scope_builder(UiBuilder::new().max_rect(buttons), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = vec2(18.0, 8.0);
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let ok = check.is_ok();
                    let c = ui.add_enabled(ok, theme::main_action(ob_look::words::CREATE).min_size(vec2(0.0, 36.0)));
                    let x = ui.add(draw::pill(ob_look::words::CANCEL));
                    for (part, resp) in [(Part::Create, &c), (Part::Cancel, &x)] {
                        if self.part() == part && focused {
                            p.rect_stroke(
                                resp.rect.expand(3.0),
                                8.0,
                                Stroke::new(2.0, rgb(t.lime)),
                                StrokeKind::Outside,
                            );
                        }
                    }
                    if c.clicked() {
                        self.focus = 2;
                        ask = self.try_create(maps);
                    }
                    if x.clicked() {
                        ask = Some(Ask::Cancel);
                    }
                });
            });
        });
        ask
    }
}
