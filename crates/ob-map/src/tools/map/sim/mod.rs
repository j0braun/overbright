//! The simulation stream shared by the game and the map editor: what
//! `game --simulate` / `--simulate-world` writes ([`format`]) and the
//! problems the AI met that both sides detect from it ([`detect`]). The
//! editor's Simulate tab (crates/ob-studio-gui, `editor::sim`) records and
//! plays it back; the game's side is src/bin/game/enh/sim_stream.rs.

pub mod detect;
pub mod format;
