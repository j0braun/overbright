//! Operating-system specifics shared by the game and the tools: where
//! Overbright keeps its files ([`dirs`]), path forms ([`paths`]), the window
//! icon ([`icon`]), a monitor's modes read safely ([`monitor`]), the timer
//! resolution ([`timer`]) and the Windows-only
//! calls behind them (`win`, `cfg(windows)`).
//! docs/platform-dirs.md.

pub mod dirs;
pub mod fs;
pub mod icon;
pub mod monitor;
pub mod paths;
pub mod timer;
#[cfg(windows)]
pub mod win;
