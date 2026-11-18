//! The opt-in modding tools (docs/texture-editor.md, docs/map-editor.md):
//! off by default and unreachable until the game is started with
//! `--editor`, then opened with their hotkey.
//! Native play never reaches this code.

pub mod overlay;
pub mod texed;
