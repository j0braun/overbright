//! The launcher (`overbright`, docs/launcher.md): a small window that
//! finds and checks the player's game folder, shows the main choices
//! (display, graphics, language, texture pack) and starts the game. It is
//! drawn on the CPU (winit + softbuffer + tiny-skia + ab_glyph with Sofia Sans
//! Semi Condensed), so it opens and can report a problem even when the graphics
//! driver cannot start the game. Its own look (smoked slab, rail, lime titles); nothing from the game.
//!
//! * [`ui`]: widgets, focus and input; [`layout`]: sizes and placement;
//!   [`draw`] / [`font`]: pixels; [`screens`]: the screens and what their
//!   controls do; [`model`]: the data (real or fake); [`dirs`]: folders.
//! * [`headless::render`]: a screen into a PNG without a window
//!   (`overbright --render <screen> --size WxH out.png`, the tests).
//! * [`window::run_with_root`]: the window (keyboard, mouse, gamepad).
//! * [`probe`]: `game --probe`'s format and its background run;
//!   [`run`]: starting and watching the game, the lock, the skip rule's
//!   Shift check; [`saves`] / [`saves_page`]: the Saves page (profiles,
//!   backups, importing the original's saves, export);
//!   [`start_log`]: the launcher's own log (`launcher.log`) and panic hook.

pub mod anim;
pub mod backdrop;
pub mod check;
pub mod crash_report;
pub mod dirs;
pub mod draw;
pub mod font;
pub mod game_data;
pub mod headless;
pub mod layout;
pub mod legends;
pub mod localtime;
pub mod model;
pub mod pad_filter;
pub mod platform_deck;
pub mod prepare;
pub mod probe;
pub mod run;
pub mod saves;
pub mod saves_page;
pub mod screens;
pub mod sounds;
pub mod start_log;
pub mod studio_seen;
pub mod transfer;
pub mod trial;
pub mod ui;
#[cfg(windows)]
mod win_dialog;
pub mod window;

#[cfg(test)]
mod tests;
