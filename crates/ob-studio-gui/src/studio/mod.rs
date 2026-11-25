//! Overbright Studio's window (`overbright-studio`, docs/modding/studio.md): the shell every workspace lives in.
//!
//! * [`workspace`]: the workspaces and the rail's order.
//! * [`layout`]: where every part goes at a window size (pure geometry; the clip and overlap check).
//! * [`nav`]: the areas, the key table, the pad's buttons, Esc / B and the legend row's rule.
//! * [`address`]: `ob://` addresses of items and screens, back and forward.
//! * [`changes`]: the Changes page (what the open mod changes, value by value, and who wins).
//! * [`mod_page`]: the Mod page (details, files, clashes, ready to share, export).
//! * [`new_mod`]: the New mod dialog.
//! * [`guide`]: the tutorials, "Your first mod" and "Make your first race".
//! * [`home`]: the mods as Home and the status bar show them (read through the mod's history).
//! * [`memory`]: the panels' sizes and folds kept per workspace.
//! * [`shell`]: the window drawn with egui; [`draw`]: its painted pieces; [`words`]: its words.
//! * [`shot`]: the window drawn off-screen into a PNG.
//! * [`world`]: the World and Races workspaces: the map editor hosted in the window.

pub mod address;
pub mod changes;
pub mod draw;
pub mod guide;
pub mod home;
pub mod layout;
pub mod memory;
pub mod mod_page;
pub mod nav;
pub mod new_mod;
pub mod shell;
pub mod shot;
pub mod words;
pub mod workspace;
pub mod world;

pub use shell::Studio;

#[cfg(test)]
mod guide_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod world_tests;
