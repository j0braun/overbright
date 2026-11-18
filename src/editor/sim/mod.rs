//! Simulate: the game races the current custom race (or runs the traffic
//! around the camera) headless with the AI driving every car, streaming
//! each tick (`format`); the editor records the stream, plays it back
//! over the 3D view and the map, lists results and events, and turns the
//! problems the AI met (stuck, wrong way, crashes) into validation
//! warnings. The game runs as a child process (`run`); nothing here waits
//! on it.

pub mod detect;
pub mod format;
pub mod panel;
pub mod record;
pub mod run;

pub use panel::{ISSUE_PREFIX, Sim, SimHook, SimOverlay, SimTab, SimTool, TAB, issues};
