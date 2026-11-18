//! The menu bar, the toolbar, the status bar and the shell's keys.
//!
//! Two kinds of request come out of them: [`Action`]s the shell runs
//! itself (files, undo, mode), and [`Command`]s for the editor's parts
//! (Delete, Test race, …): a menu item or toolbar button posts a command
//! for this frame, and the part that handles it takes it with [`take`]
//! while drawing (its own key for the same thing goes through its own
//! input handling). Commands nobody took are dropped at the frame's end.

use egui::{Key, KeyboardShortcut, Modifiers};

use super::app::App;
use super::state::{Level, Mode, View};
use super::theme;

/// What the shell does itself.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewMod,
    OpenMod,
    OpenRecent(String),
    Save,
    Details,
    Import,
    Export,
    Quit,
    Undo,
    Redo,
    Mode(Mode),
    View(View),
    Tab(&'static str),
    Controls,
}

/// A request for one of the editor's parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Duplicate,
    Delete,
    SelectAllOfSolid,
    DropToGround,
    ResetCamera,
    FrameSelection,
    ToggleRoadNetwork,
    ToggleGrid,
    NewRace,
    TestRace,
    DriveHere,
}

fn commands_id() -> egui::Id {
    egui::Id::unique("editor.commands")
}

/// Post `c` for this frame.
pub fn post(ctx: &egui::Context, c: Command) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Command>>(commands_id()).push(c));
}

/// Whether `c` was posted this frame (taking it: one part handles it).
pub fn take(ctx: &egui::Context, c: Command) -> bool {
    ctx.data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<Command>>(commands_id());
        match v.iter().position(|x| *x == c) {
            Some(i) => {
                v.remove(i);
                true
            }
            None => false,
        }
    })
}

/// Drop the commands nobody took (end of frame).
pub fn clear_commands(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Vec<Command>>(commands_id()));
}

const fn ctrl(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}
pub const SAVE: KeyboardShortcut = ctrl(Key::S);
pub const UNDO: KeyboardShortcut = ctrl(Key::Z);
pub const REDO: KeyboardShortcut = ctrl(Key::Y);
pub const REDO_SHIFT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
pub const NEW: KeyboardShortcut = ctrl(Key::N);
pub const OPEN: KeyboardShortcut = ctrl(Key::O);
pub const QUIT: KeyboardShortcut = ctrl(Key::Q);

/// The controls (Help ▸ Controls).
pub const CONTROLS: &[(&str, &str)] = &[
    ("RMB + W A S D, Q / E", "Fly; down / up"),
    ("RMB + wheel", "Fly speed"),
    ("Shift / Ctrl while flying", "Fast / slow"),
    ("Wheel", "Dolly"),
    ("F", "Frame the selection"),
    ("LMB / Ctrl+LMB", "Select / add to the selection"),
    ("Q / W / E", "Select / Move / Rotate"),
    ("Alt while dragging", "No snapping"),
    ("Ctrl+D", "Duplicate"),
    ("Del", "Delete"),
    ("End", "Drop to the ground"),
    ("Tab", "Props / Races"),
    ("C / G / S / X", "Checkpoint / grid / start / finish tool"),
    ("M", "3D view / Map"),
    ("Ctrl+Z, Ctrl+Y", "Undo, redo"),
    ("Ctrl+S", "Save"),
    ("F5 / Shift+F5", "Test race / Drive here"),
];

/// The shell's keys (not while a text field has the keyboard for the
/// single keys).
pub fn shortcuts(app: &App, ctx: &egui::Context, out: &mut Vec<Action>) {
    let open = app.state.is_some();
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input_mut(|i| {
        if i.consume_shortcut(&NEW) {
            out.push(Action::NewMod);
        }
        if i.consume_shortcut(&OPEN) {
            out.push(Action::OpenMod);
        }
        if i.consume_shortcut(&QUIT) {
            out.push(Action::Quit);
        }
        if !open {
            return;
        }
        if i.consume_shortcut(&SAVE) {
            out.push(Action::Save);
        }
        if i.consume_shortcut(&REDO_SHIFT) || i.consume_shortcut(&REDO) {
            out.push(Action::Redo);
        }
        if i.consume_shortcut(&UNDO) {
            out.push(Action::Undo);
        }
        if typing {
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Tab) {
            let m = app.state.as_ref().map_or(Mode::Props, |s| s.mode);
            out.push(Action::Mode(if m == Mode::Props { Mode::Races } else { Mode::Props }));
        }
        if i.consume_key(Modifiers::NONE, Key::M) {
            let v = app.state.as_ref().map_or(View::ThreeD, |s| s.view);
            out.push(Action::View(if v == View::ThreeD { View::Map } else { View::ThreeD }));
        }
    });
    if open && !typing {
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F5)) {
            post(ctx, Command::DriveHere);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            post(ctx, Command::TestRace);
        }
    }
}

fn item(ui: &mut egui::Ui, text: &str, key: Option<&KeyboardShortcut>, enabled: bool) -> bool {
    let mut b = egui::Button::new(text);
    if let Some(k) = key {
        b = b.shortcut_text(ui.ctx().format_shortcut(k));
    }
    ui.add_enabled(enabled, b).clicked()
}

fn item_text(ui: &mut egui::Ui, text: &str, key: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(key)).clicked()
}

/// The menu bar.
pub fn bar(app: &App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let open = app.state.is_some();
    let ctx = ui.ctx().clone();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if item(ui, "New map mod…", Some(&NEW), true) {
                out.push(Action::NewMod);
            }
            if item(ui, "Open map mod…", Some(&OPEN), true) {
                out.push(Action::OpenMod);
            }
            let recent = super::app::recent(&app.maps);
            if !recent.is_empty() {
                ui.separator();
                ui.weak("Recent");
                for f in recent.into_iter().take(5) {
                    if ui.button(&f).clicked() {
                        out.push(Action::OpenRecent(f));
                    }
                }
            }
            ui.separator();
            if item(ui, "Save", Some(&SAVE), open) {
                out.push(Action::Save);
            }
            if item(ui, "Mod details…", None, open) {
                out.push(Action::Details);
            }
            ui.separator();
            if item(ui, "Import…", None, true) {
                out.push(Action::Import);
            }
            if item(ui, "Export…", None, open) {
                out.push(Action::Export);
            }
            ui.separator();
            if item(ui, "Quit", Some(&QUIT), true) {
                out.push(Action::Quit);
            }
        });
        ui.menu_button("Edit", |ui| {
            let (u, r) = app.state.as_ref().map_or((None, None), |s| {
                (s.undo.undo_label().map(str::to_string), s.undo.redo_label().map(str::to_string))
            });
            let ut = u.as_ref().map_or("Undo".to_string(), |l| format!("Undo: {l}"));
            if item(ui, &ut, Some(&UNDO), u.is_some()) {
                out.push(Action::Undo);
            }
            let rt = r.as_ref().map_or("Redo".to_string(), |l| format!("Redo: {l}"));
            if item(ui, &rt, Some(&REDO), r.is_some()) {
                out.push(Action::Redo);
            }
            ui.separator();
            if item_text(ui, "Duplicate", "Ctrl+D", open) {
                post(&ctx, Command::Duplicate);
            }
            if item_text(ui, "Delete", "Del", open) {
                post(&ctx, Command::Delete);
            }
            if item_text(ui, "Select all of this solid", "", open) {
                post(&ctx, Command::SelectAllOfSolid);
            }
            if item_text(ui, "Drop to ground", "End", open) {
                post(&ctx, Command::DropToGround);
            }
        });
        ui.menu_button("View", |ui| {
            let v = app.state.as_ref().map(|s| s.view);
            if ui
                .add_enabled(open, egui::Button::selectable(v == Some(View::ThreeD), "3D view").shortcut_text("M"))
                .clicked()
            {
                out.push(Action::View(View::ThreeD));
            }
            if ui.add_enabled(open, egui::Button::selectable(v == Some(View::Map), "Map").shortcut_text("M")).clicked()
            {
                out.push(Action::View(View::Map));
            }
            ui.separator();
            if item_text(ui, "Show road network", "", open) {
                post(&ctx, Command::ToggleRoadNetwork);
            }
            if item_text(ui, "Show grid", "", open) {
                post(&ctx, Command::ToggleGrid);
            }
            if item_text(ui, "Frame selection", "F", open) {
                post(&ctx, Command::FrameSelection);
            }
            if item_text(ui, "Reset camera", "", open) {
                post(&ctx, Command::ResetCamera);
            }
        });
        ui.menu_button("Race", |ui| {
            if item_text(ui, "New race", "", open) {
                out.push(Action::Mode(Mode::Races));
                post(&ctx, Command::NewRace);
            }
            if item_text(ui, "Test race", "F5", open) {
                post(&ctx, Command::TestRace);
            }
            if item_text(ui, "Drive here", "Shift+F5", open) {
                post(&ctx, Command::DriveHere);
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Controls").clicked() {
                out.push(Action::Controls);
            }
        });
    });
}

fn segment(ui: &mut egui::Ui, on: bool, text: &str, key: &str) -> bool {
    let rt = egui::RichText::new(text).font(theme::font(theme::LABEL, 15.0));
    let rt = if on { rt.color(theme::lime()) } else { rt };
    ui.add(egui::Button::selectable(on, rt)).on_hover_text(key).clicked()
}

/// The toolbar: mode, the parts' tools, view; Drive here and Test race on
/// the right.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui, out: &mut Vec<Action>) {
    let Some(s) = app.state.as_mut() else { return };
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        if segment(ui, s.mode == Mode::Props, "Props", "Tab") {
            out.push(Action::Mode(Mode::Props));
        }
        if segment(ui, s.mode == Mode::Races, "Races", "Tab") {
            out.push(Action::Mode(Mode::Races));
        }
        ui.separator();
        for t in app.panels.tools.iter_mut() {
            t.ui(s, ui);
        }
        if !app.panels.tools.is_empty() {
            ui.separator();
        }
        if segment(ui, s.view == View::ThreeD, "3D", "M") {
            out.push(Action::View(View::ThreeD));
        }
        if segment(ui, s.view == View::Map, "Map", "M") {
            out.push(Action::View(View::Map));
        }
        ui.separator();
        if ui.add_enabled(s.undo.undo_label().is_some(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() {
            out.push(Action::Undo);
        }
        if ui.add_enabled(s.undo.redo_label().is_some(), egui::Button::new("Redo")).on_hover_text("Ctrl+Y").clicked() {
            out.push(Action::Redo);
        }
        if ui.add_enabled(s.dirty, egui::Button::new("Save")).on_hover_text("Ctrl+S").clicked() {
            out.push(Action::Save);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let errors = s.issues.iter().any(|i| i.level == Level::Error);
            let r = ui.add_enabled(!errors, theme::main_action("Test race"));
            let r = if errors {
                r.on_disabled_hover_text("Fix the errors in Validation first")
            } else {
                r.on_hover_text("F5")
            };
            if r.clicked() {
                post(ui.ctx(), Command::TestRace);
            }
            if ui.button("Drive here").on_hover_text("Shift+F5").clicked() {
                post(ui.ctx(), Command::DriveHere);
            }
        });
    });
}

/// The status bar: save state, the last message, issues, camera, frame
/// time.
pub fn status(app: &App, ui: &mut egui::Ui) {
    ui.horizontal_centered(|ui| {
        let Some(s) = app.state.as_ref() else {
            ui.weak("No map mod open");
            return;
        };
        let (text, color) = if s.dirty { ("Unsaved changes", theme::warn()) } else { ("Saved", theme::lime()) };
        ui.colored_label(color, egui::RichText::new(text).font(theme::font(theme::SEMI, 13.0)));
        if !app.autosave_note.is_empty() {
            ui.weak(format!("· {}", app.autosave_note));
        }
        ui.separator();
        ui.label(egui::RichText::new(&s.status).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(egui::RichText::new(format!("UI {:.1} ms", app.frame_ms)).size(12.0));
            ui.separator();
            let c = s.camera;
            ui.weak(
                egui::RichText::new(format!(
                    "{:.0}, {:.0}, {:.0}  heading {:.0}°  FOV {:.0}°",
                    c.position.x,
                    c.position.y,
                    c.position.z,
                    c.yaw.to_degrees().rem_euclid(360.0),
                    c.fov_y.to_degrees()
                ))
                .size(12.0),
            );
            ui.separator();
            let e = s.issues.iter().filter(|i| i.level == Level::Error).count();
            let w = s.issues.iter().filter(|i| i.level == Level::Warning).count();
            ui.colored_label(
                if w > 0 { theme::warn() } else { theme::dim() },
                egui::RichText::new(format!("{w} warnings")).size(12.0),
            );
            ui.colored_label(
                if e > 0 { theme::error() } else { theme::dim() },
                egui::RichText::new(format!("{e} errors")).size(12.0),
            );
            ui.separator();
            ui.weak(egui::RichText::new(format!("{} · {}", s.folder, s.region)).size(12.0));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_taken_once_and_cleared() {
        let ctx = egui::Context::default();
        post(&ctx, Command::Delete);
        post(&ctx, Command::TestRace);
        assert!(take(&ctx, Command::Delete));
        assert!(!take(&ctx, Command::Delete));
        clear_commands(&ctx);
        assert!(!take(&ctx, Command::TestRace));
    }
}
