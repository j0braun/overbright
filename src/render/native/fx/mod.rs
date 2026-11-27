//! The effects' tables and ticks (crates/ob-assets, re-exported here at the old path); the effect controller
//! (simulation layer) stays in the library.

pub use ob_assets::render::native::fx::*;

pub use ob_sim::render::native::fx::effect_controller;
