//! The Runs workspace (docs/modding/studio.md, "Runs"; `ob://<mod>/@runs`, Ctrl+5): the run files of the open mod
//! in the left panel (`editor::sim::library`), the run opened in the editor's own view with its timeline and
//! cameras (no second player), the Telemetry tab of a chosen car (`editor::sim::graphs`) and a second run
//! compared as ghosts in the view and ticks under the timeline's track. This is the left panel: the list, with
//! Open, Compare, Rename and Delete on the row the cursor is on.
//!
//! ↑ ↓ move over the rows (Tab over a row's buttons), Enter / A opens, C compares the row with the run shown,
//! F2 renames, Delete asks to delete (Enter confirms, Esc keeps).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Align2, Key, Rect, Sense, Stroke, vec2};

use super::changes::small_button;
use crate::editor::sim::library::{self, Entry};
use crate::editor::sim::panel::{Shared, ghost_colour, load_recording};
use crate::editor::theme::{self, LABEL, font, rgb, tokens};
use super::words;

/// A run's row (points): its name, and what it is under it.
pub const ROW_H: f32 = 46.0;
/// The button line under the row the cursor is on.
pub const BUTTONS_H: f32 = 30.0;

/// The left panel's state.
#[derive(Default)]
pub struct Page {
    entries: Vec<Entry>,
    stamp: Option<(PathBuf, Option<SystemTime>, usize)>,
    /// The row the cursor is on (a run's name).
    pub cursor: Option<String>,
    /// A rename under way: the run, the new name typed.
    pub rename: Option<(String, String)>,
    /// A delete asked for: the run.
    pub confirm: Option<String>,
    /// The last thing that did not work, or what was done.
    pub note: Option<String>,
    /// The rows as drawn, for the layout check.
    pub rows: Vec<Rect>,
    /// The cursor row asks for the keyboard focus next frame.
    want_focus: bool,
    /// The rows' widget ids and names as last drawn (↑ ↓ move over them; the shell takes those keys from egui,
    /// whose own arrow keys would step on to the buttons).
    ids: Vec<(egui::Id, String)>,
    /// A key of the list the shell took from the editor (which would delete the selection on Delete): the row with
    /// the focus acts on it in the frame.
    pub key: Option<Key>,
}

impl Page {
    /// The runs in `dir`, read again when the folder changed.
    pub fn refresh(&mut self, dir: &Path) {
        let now = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
        let count = std::fs::read_dir(dir).map_or(0, |d| d.count());
        let stamp = (dir.to_path_buf(), now, count);
        if self.stamp.as_ref() != Some(&stamp) {
            self.entries = library::list(dir);
            self.stamp = Some(stamp);
        }
    }

    /// A row has the keyboard focus (its ↑ ↓ are the list's).
    pub fn row_focused(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.focused()).is_some_and(|f| self.ids.iter().any(|(i, _)| *i == f))
    }

    /// Move the focus `d` rows (the first or last row at the ends); with no row focused, to the cursor row or the
    /// first.
    pub fn step(&mut self, ctx: &egui::Context, d: i32) {
        if self.ids.is_empty() {
            return;
        }
        let f = ctx.memory(|m| m.focused());
        let at = self.ids.iter().position(|(id, _)| Some(*id) == f).or_else(|| {
            let c = self.cursor.as_ref()?;
            self.ids.iter().position(|(_, n)| n == c).map(|i| if d > 0 { i.saturating_sub(1) } else { i + 1 })
        });
        let i = at.map_or(if d > 0 { -1 } else { self.ids.len() as i32 }, |i| i as i32);
        let k = (i + d).clamp(0, self.ids.len() as i32 - 1) as usize;
        let id = self.ids[k].0;
        self.cursor = Some(self.ids[k].1.clone());
        ctx.memory_mut(|m| m.request_focus(id));
    }

    /// Read the folder again at the next [`Page::refresh`] (after a run was saved, renamed or deleted).
    pub fn changed(&mut self) {
        self.stamp = None;
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Open run `name` in the view (the editor's Simulate playback reads it next frame).
    pub fn open(&mut self, dir: &Path, sim: &Shared, ctx: &egui::Context, name: &str) {
        let path = library::path(dir, name);
        if !path.is_file() {
            self.note = Some(format!("{name} is not in the list any more"));
            self.changed();
            return;
        }
        {
            let mut s = sim.borrow_mut();
            s.stop();
            if s.ghost_name == name {
                s.clear_ghost();
            }
        }
        load_recording(ctx, &path.to_string_lossy());
        self.cursor = Some(name.to_string());
        self.note = None;
    }

    /// Compare run `name` with the one shown (or stop comparing, when it already is the one compared).
    pub fn compare(&mut self, dir: &Path, sim: &Shared, name: &str) {
        let mut s = sim.borrow_mut();
        if s.ghost_name == name {
            s.clear_ghost();
            self.note = None;
        } else if s.shown == name {
            self.note = Some(words::THAT_RUN_IS_THE_ONE_SHOWN.into());
        } else {
            self.note = s.set_ghost(&library::path(dir, name)).err();
        }
    }

    /// Delete run `name`; one shown or compared stops showing.
    pub fn delete(&mut self, dir: &Path, sim: &Shared, name: &str) {
        match library::delete(dir, name) {
            Ok(()) => {
                let mut s = sim.borrow_mut();
                if s.shown == name {
                    s.unload();
                }
                if s.ghost_name == name {
                    s.clear_ghost();
                }
                self.note = Some(format!("Deleted {name}"));
                if self.cursor.as_deref() == Some(name) {
                    self.cursor = None;
                }
            }
            Err(e) => self.note = Some(e),
        }
        self.confirm = None;
        self.changed();
    }

    /// Rename a run to `to`; the run shown or compared keeps showing under its new name.
    pub fn rename(&mut self, dir: &Path, sim: &Shared, from: &str, to: &str) {
        match library::rename(dir, from, to) {
            Ok(()) => {
                let mut s = sim.borrow_mut();
                if s.shown == from {
                    s.shown = to.to_string();
                }
                if s.ghost_name == from {
                    s.ghost_name = to.to_string();
                }
                self.cursor = Some(to.to_string());
                self.note = None;
                self.rename = None;
            }
            Err(e) => self.note = Some(e),
        }
        self.changed();
    }

    /// The list in `r` (the left panel's body).
    pub fn ui(&mut self, ui: &mut egui::Ui, r: Rect, sim: &Shared, dir: &Path) {
        self.refresh(dir);
        self.rows.clear();
        self.ids.clear();
        let ctx = ui.ctx().clone();
        let t = tokens();
        let shown = sim.borrow().shown.clone();
        let ghost = sim.borrow().ghost_name.clone();
        let running = sim.borrow().running();
        let count = self.entries.len();
        let mut act: Option<(&'static str, String)> = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
            ui.set_clip_rect(r);
            egui::ScrollArea::vertical().id_salt("runs.list").auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(r.width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} · {}", crate::studio::words::RUNS, count))
                            .font(font(LABEL, 14.0))
                            .color(theme::dim()),
                    );
                    if running {
                        ui.label(egui::RichText::new("simulating…").color(theme::dim()));
                    }
                });
                if ui
                    .add(small_button(words::DRIVE_TO_RECORD, false))
                    .on_hover_text(
                        words::START_THE_GAME_ON_THE_CHOSEN,
                    )
                    .clicked()
                {
                    crate::editor::menu::post(&ctx, crate::editor::menu::Command::RecordDrive);
                }
                if count == 0 {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crate::studio::words::NO_RUNS).color(rgb(t.ink)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::studio::words::NO_RUNS_HOW).color(theme::dim()));
                }
                for e in self.entries.clone().iter() {
                    let id = egui::Id::new(("runs.row", &e.name));
                    let (row, _) = ui.allocate_exact_size(vec2(r.width(), ROW_H), Sense::hover());
                    self.rows.push(row);
                    self.ids.push((id, e.name.clone()));
                    let resp = ui.interact(row, id, Sense::click());
                    let on = self.cursor.as_deref() == Some(e.name.as_str());
                    if self.want_focus && on {
                        resp.request_focus();
                        self.want_focus = false;
                    }
                    if resp.has_focus() || resp.clicked() {
                        self.cursor = Some(e.name.clone());
                    }
                    let _ = resp
                        .clone()
                        .on_hover_text(words::ENTER_OPEN_IT_C_COMPARE_WITH);
                    let p = ui.painter_at(row);
                    let is_shown = shown == e.name;
                    let is_ghost = ghost == e.name;
                    if is_shown || on || resp.has_focus() {
                        let fill = theme::over(t.pressed, t.slab.over(t.page));
                        p.rect_filled(row.shrink2(vec2(0.0, 1.0)), 4.0, fill);
                    }
                    if resp.has_focus() {
                        p.rect_stroke(
                            row.shrink2(vec2(0.0, 1.0)),
                            4.0,
                            Stroke::new(2.0, rgb(t.lime)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let mark = if is_shown {
                        Some(("shown", rgb(t.lime)))
                    } else if is_ghost {
                        Some(("B", ghost_colour()))
                    } else {
                        None
                    };
                    let name_w = row.width() - 16.0 - mark.map_or(0.0, |_| 52.0);
                    p.with_clip_rect(Rect::from_min_size(row.min, vec2(name_w + 8.0, row.height()))).text(
                        row.left_top() + vec2(8.0, 6.0),
                        Align2::LEFT_TOP,
                        &e.name,
                        font(LABEL, 15.0),
                        rgb(t.ink),
                    );
                    if let Some((m, c)) = mark {
                        p.text(row.right_top() + vec2(-8.0, 6.0), Align2::RIGHT_TOP, m, font(LABEL, 13.0), c);
                    }
                    p.with_clip_rect(row.shrink2(vec2(4.0, 0.0))).text(
                        row.left_top() + vec2(8.0, 26.0),
                        Align2::LEFT_TOP,
                        describe(e),
                        font("body", 13.0),
                        theme::dim(),
                    );
                    if resp.clicked()
                        || (resp.has_focus() && ctx.input(|i| i.key_pressed(Key::Enter))) && self.rename.is_none()
                    {
                        act = Some(("open", e.name.clone()));
                    }
                    if on {
                        self.row_buttons(ui, &e.name, is_shown, is_ghost, &mut act);
                    }
                }
                if let Some(n) = &self.note {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(n).font(font("body", 13.0)).color(rgb(t.warn)));
                }
            });
        });
        // A key the shell took (C, F2, Delete, or Enter from the pad's A, or the menu) acts on the cursor row.
        if let Some(k) = self.key.take()
            && act.is_none()
            && self.rename.is_none()
            && self.confirm.is_none()
            && let Some(name) = self.cursor.clone().filter(|c| count > 0 && self.entries.iter().any(|e| &e.name == c))
        {
            act = match k {
                Key::C => Some(("compare", name)),
                Key::F2 => Some(("rename", name)),
                Key::Delete => Some(("delete", name)),
                Key::Enter => Some(("open", name)),
                _ => None,
            };
        }
        if let Some((what, name)) = act {
            match what {
                "open" => self.open(dir, sim, &ctx, &name),
                "compare" => self.compare(dir, sim, &name),
                "rename" => {
                    self.cursor = Some(name.clone());
                    self.rename = Some((name.clone(), name));
                    self.confirm = None;
                }
                "delete" => {
                    self.cursor = Some(name.clone());
                    self.confirm = Some(name);
                    self.rename = None;
                }
                "do-rename" => {
                    if let Some((from, to)) = name.split_once('\n') {
                        self.rename(dir, sim, from, to.trim());
                    }
                }
                "do-delete" => self.delete(dir, sim, &name),
                _ => {
                    // Esc took the focus from the row (egui's rule): it goes back to it.
                    self.rename = None;
                    self.confirm = None;
                    self.want_focus = true;
                }
            }
        }
    }

    /// The buttons under the cursor row, or the rename field, or the delete question.
    fn row_buttons(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        shown: bool,
        ghost: bool,
        act: &mut Option<(&'static str, String)>,
    ) {
        let t = tokens();
        if let Some((from, text)) = self.rename.as_mut().filter(|(f, _)| f == name) {
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(text).desired_width(150.0).font(font("body", 14.0)));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let esc = ui.input(|i| i.key_pressed(Key::Escape));
                let ok = ui.add(small_button(words::RENAME, false)).clicked() || enter;
                if ok {
                    *act = Some(("do-rename", format!("{from}\n{text}")));
                }
                if esc || ui.add(small_button(words::CANCEL, false)).clicked() {
                    *act = Some(("cancel", String::new()));
                }
            });
            return;
        }
        if self.confirm.as_deref() == Some(name) {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("Delete {name}?")).color(rgb(t.warn)));
                let del = ui.add(small_button(words::DELETE, false));
                if del.clicked() || ui.input(|i| i.key_pressed(Key::Enter)) && !del.has_focus() {
                    *act = Some(("do-delete", name.to_string()));
                }
                if ui.add(small_button(words::KEEP, false)).clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    *act = Some(("cancel", String::new()));
                }
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.add(small_button(words::OPEN, shown)).on_hover_text(words::SHOW_IT_IN_THE_VIEW_ENTER).clicked() {
                *act = Some(("open", name.to_string()));
            }
            let cmp = if ghost { words::STOP_COMPARING } else { words::COMPARE };
            if ui.add(small_button(cmp, ghost)).on_hover_text(words::DRAW_IT_AS_GHOSTS_OVER_THE).clicked() {
                *act = Some(("compare", name.to_string()));
            }
            if ui.add(small_button(words::RENAME, false)).on_hover_text("F2").clicked() {
                *act = Some(("rename", name.to_string()));
            }
            if ui.add(small_button(words::DELETE, false)).on_hover_text(words::DELETE).clicked() {
                *act = Some(("delete", name.to_string()));
            }
        });
    }
}

/// What a row says under its name: the race, the seed, how it ended, how big.
pub fn describe(e: &Entry) -> String {
    let what = if e.what == "world" { "free roam".to_string() } else { e.what.clone() };
    let seed = e.seed.map_or(String::new(), |s| format!(" · seed {s}"));
    let end = match e.end.as_deref() {
        None => " · not finished".to_string(),
        Some("finish") => String::new(),
        Some("limit") => " · time limit".to_string(),
        Some("quit") => " · stopped".to_string(),
        Some(w) => format!(" · {w}"),
    };
    format!("{what}{seed}{end} · {}", size(e.bytes))
}

fn size(b: u64) -> String {
    if b >= 1 << 20 { format!("{:.1} MB", b as f64 / (1u64 << 20) as f64) } else { format!("{} KB", b.div_ceil(1024)) }
}
