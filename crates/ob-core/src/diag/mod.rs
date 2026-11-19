//! Diagnostics for players' reports (docs/troubleshooting.md).
//!
//! * `support` (in the library, `nfsu2::diag::support`): one text file a player attaches to a report — the logs
//!   of the last starts, the settings, the game-folder check, the GPU list,
//!   the system and Overbright's folders, home folders and user names
//!   scrubbed. Written by the launcher's "Save support file", by
//!   `overbright --support-file` / `game --support-file` (a start that
//!   cannot open its window), and by the game's Ctrl+F8.
//! * [`events`]: the game's recent window, display, GPU and screen events
//!   in a small ring, written to the log on a crash, a hang, a lost GPU
//!   device and on Ctrl+F8 (a "black screen" report then shows whether
//!   frames were presented, the surface failed or the window was hidden).
//! * [`native_crash`]: on Windows, a crash that is no Rust panic (an access
//!   violation in a driver, a missing DLL's entry) leaves one line naming
//!   the exception and the module in the log before Windows ends the
//!   process.
//!
//! Nothing here reads or changes the simulation, its random numbers or its
//! timing: the hooks only read clocks, counters and window state.

pub mod events;
pub mod native_crash;
