//! The opt-in modding tools (docs/modding/texture-editor.md, docs/modding/map-editor.md):
//! off by default and unreachable until the game is started with
//! `--editor`, then opened with their hotkey.
//! Native play never reaches this code.

pub mod map;
pub mod overlay;
pub mod texed;
