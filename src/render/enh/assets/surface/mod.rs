//! The surface maps (crates/ob-assets, re-exported here at the old path); their GPU code stays in the library.

pub use ob_assets::render::enh::assets::surface::*;

pub(crate) mod gpu;
