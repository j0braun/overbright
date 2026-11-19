//! Operating-system specifics shared by the game and the tools: where
//! Overbright keeps its files ([`dirs`]), path forms ([`paths`]), the window
//! icon (`icon`) and a monitor's modes read safely (`monitor`; both in the
//! library, `nfsu2::platform`, as they use winit), the timer
//! resolution ([`timer`]), the player's line when no Vulkan driver is found
//! ([`vulkan`]) and the Windows-only calls behind them (`win`, `cfg(windows)`).
//! docs/platform-dirs.md.

pub mod dirs;
pub mod fs;
pub mod paths;
pub mod timer;
pub mod vulkan;
#[cfg(windows)]
pub mod win;
