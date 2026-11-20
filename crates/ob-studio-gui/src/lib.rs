//! The desktop map editor (`overbright-editor`, docs/modding/map-editor.md), a
//! leaf crate on the game library `nfsu2`. The editor keeps its module
//! folder (`src/editor/`), and the library's modules it uses are imported
//! here under their own names, so its `crate::…` paths read as before.

pub mod editor;

// The game library through its one interface for Studio (`nfsu2::studio_api`;
// scripts/checks/crate_layers.sh refuses any other `nfsu2::` path here), and
// the lower layers through their own crates.
use nfsu2::studio_api::{fog, render, sky, world};
use ob_core::{draw, platform};
use ob_formats::formats;
use ob_look as launch;

/// The atomic file write.
mod qol {
    pub mod saves {
        pub use ob_core::platform::fs::write_atomic;
    }
}

/// The map data (ob-map) and the map editor's logic on a world.
mod tools {
    pub mod map {
        pub use nfsu2::studio_api::map::*;
        pub use ob_map::tools::map::format::*;
        pub use ob_map::tools::map::{ops, race, store};
        pub use ob_studio::session;
    }
}

// The game's career data: read only through `tools::map::assets` (the tests
// read it directly to check against the real tracks).
#[cfg(test)]
mod frontend {
    pub mod career {
        pub use nfsu2::studio_api::career::CareerData;
        pub use ob_map::frontend::career::*;
    }
}

// The test kit (crates/ob-testkit, docs/developing/testing.md), bound to the game
// library.
#[cfg(test)]
mod testkit {
    #![allow(dead_code)]
    ob_testkit::bind!();
}
