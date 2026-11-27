//! The computer drivers: the low-level driver (see docs/formats.md, "AI
//! driver") and the route layer (route following, target speed, catch-up,
//! upgrades from skill).

pub mod actions;
pub mod encounter_controller;
pub mod belt;
pub mod drag;
pub mod gps;
pub mod driver;
pub mod route;
mod junction_cache;
pub mod outrun_search;
pub mod traffic;
