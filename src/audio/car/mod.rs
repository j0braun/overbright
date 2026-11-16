//! The player car's effect sounds: the nitrous (`CARSFX_Nitrous`), the turbo
//! (`CARSFX_Turbo`: spool whine and blow-off) and the
//! gear shift (`CARSFX_Shift` driven by `SFXCTL_Shifting`). Registry
//! records 0x7ED3BC / 0x7ED398 / 0x7ED2A8 (name, list head, factory, type);
//! the objects play AEMS events in `SOUND/NOS` and `SOUND/SHIFTING`.
//!
//! Verified (SPEED2.EXE) unless marked: the start / stop rules, the banks,
//! the shift delays and volume, and the events' parameter blocks. The
//! banks' AEMS graphs ([`crate::audio::aems`]) turn the events into voices
//! (samples, pitch, volume); the levels and azimuths come from the mix map (`crate::audio::mixmap`).

pub mod nitrous;
pub mod player;
pub mod tables;
pub mod turbo;
pub mod transients;
pub mod hydraulics;
pub mod scrape;
pub mod scripted;
pub mod skids;
