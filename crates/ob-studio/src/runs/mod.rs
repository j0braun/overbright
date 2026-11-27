//! The runs of a mod, without a window: the run files' list and names ([`library`]), a run read as a recording
//! with its playback and findings ([`record`]) and a car's telemetry ([`telemetry`]). The window's Runs workspace
//! and the command line's `runs` commands read the same functions.

pub use ob_map::tools::map::sim::{detect, format};

#[doc(hidden)]
pub mod fixtures;
pub mod library;
pub mod record;
pub mod telemetry;
