//! The contract between the game and the launcher (docs/launcher.md): what
//! one program writes and the other reads, in the library so both sides
//! share one definition.
//!
//! * [`probe`]: `game --probe`'s `key=value` report.
//! * [`prepare`]: `game --prepare`'s progress lines.
//! * [`error`]: the game's `error.kind=` line and its window line.
//! * [`theme`]: the look's colour tokens (the launcher, the map editor, the
//!   game's free-camera panel).
//!
//! The launcher's own folders that the game and the editor need (the game's
//! executable beside them, the home folder) are `platform::dirs`.

pub mod error;
pub mod prepare;
pub mod probe;
pub mod theme;
