//! The desktop map editor (`overbright-editor`, docs/modding/map-editor.md): one
//! map mod on one region, drawn with the game's renderer inside an egui
//! window. Everything here but `app` runs without a window, so the tests
//! run headless.

pub mod app;
pub mod browser;
pub mod camera;
pub mod collision;
pub mod menu;
pub mod modes;
pub mod overlay;
pub mod props;
pub mod race_strip;
pub mod races;
pub mod roads;
pub mod sim;
pub mod state;
pub mod street;
pub mod testplay;
pub mod theme;
pub mod thumbs;
pub mod tree;
pub mod tree_props;
pub mod tree_races;
pub mod undo;
pub mod validate;
pub mod viewport;
pub mod views;
pub mod weak;
pub mod worklight;

#[cfg(test)]
mod docs_tests;
#[cfg(test)]
mod views_tests;

pub use state::{EditorState, Issue, Level, Mode, Panel, RacePoint, commit, project};

/// One frame of `ui` on `ctx` without a window or a GPU (the tests). The frame's texture changes (the font
/// atlas) have no renderer to go to, so they are dropped on purpose (egui asserts in debug builds that a
/// `TexturesDelta` is handled before it is dropped); the window's frames keep theirs (`App::frame`).
#[cfg(test)]
pub(crate) fn run_headless(
    ctx: &egui::Context,
    raw: egui::RawInput,
    ui: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut out = ctx.run_ui(raw, ui);
    out.textures_delta.clear();
    out
}
