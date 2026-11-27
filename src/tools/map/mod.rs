//! Map mods' world side (crates/ob-assets, re-exported here at the old path), the map data (crates/ob-map) under
//! it; the adapter over the career data and the tests stay in the library.

pub use ob_assets::tools::map::*;

pub mod assets;
#[cfg(test)]
mod tests;
