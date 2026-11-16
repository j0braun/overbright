//! Operating-system specifics shared by the game and the tools: where
//! Overbright keeps its files ([`dirs`]), path forms ([`paths`]), the window
//! icon ([`icon`]), a monitor's modes read safely ([`monitor`]), the timer
//! resolution ([`timer`]), the player's line when no Vulkan driver is found
//! ([`vulkan`]) and the Windows-only calls behind them (`win`, `cfg(windows)`).
//! docs/platform-dirs.md.

pub mod dirs;
pub mod fs;
pub mod icon;
pub mod monitor;
pub mod paths;
pub mod timer;
pub mod vulkan;
#[cfg(windows)]
pub mod win;
