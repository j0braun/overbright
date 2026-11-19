//! The desktop map editor (`overbright-editor`, docs/modding/map-editor.md): one
//! map mod on one region, drawn with the game's renderer inside an egui
//! window. Everything here but `app` runs without a window, so the tests
//! run headless.

pub mod app;
pub mod browser;
pub mod camera;
pub mod mapview;
pub mod menu;
pub mod modes;
pub mod overlay;
pub mod props;
pub mod races;
pub mod sim;
pub mod state;
pub mod testplay;
pub mod theme;
pub mod tree;
pub mod tree_props;
pub mod tree_races;
pub mod undo;
pub mod validate;
pub mod viewport;
pub mod worklight;

#[cfg(test)]
mod docs_tests;

pub use state::{EditorState, Issue, Level, Mode, Panel, RacePoint, View, commit, project};
