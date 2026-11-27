//! The pack loader (crates/ob-assets, re-exported here at the old path); its tests drive the renderer and stay
//! in the library.

pub use ob_assets::render::enh::assets::texture_pack::*;

#[cfg(test)]
mod tests;
