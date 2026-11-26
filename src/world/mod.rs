//! The world (crates/ob-assets, re-exported here at the old path); the prop collision's tests that run the
//! simulation stay in the library.

pub use ob_assets::world::*;

#[cfg(test)]
mod prop_collision {
    mod tests;
}
