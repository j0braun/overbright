//! Overbright's map data (ARCHITECTURE.md "Layers", docs/modding/map-editor.md):
//! the map mod file and its store in Overbright's data folder, the edit
//! operations as data, custom races on the original's road network and the
//! simulation record. No world, window or GPU: the game library applies the
//! mods to its worlds and runs the edit sessions; Studio and the command-line
//! tool read and write the same data. The library (`nfsu2`) re-exports these
//! modules at their old paths (`tools::map::…`, `frontend::career`'s tracks
//! and routes).

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{game_fs, platform};
pub(crate) use ob_formats::formats;

/// This crate's build id (build_support/build_id.rs).
pub const BUILD_ID: ob_core::cache::BuildId = ob_core::cache::BuildId::from_hex(env!("OB_BUILD_ID"));

pub mod tools {
    /// Map mods (docs/modding/map-editor.md); the library's `tools::map`
    /// adds the world side (applying, sessions, the in-game editor).
    pub mod map {
        pub mod format;
        pub mod ops;
        pub mod race;
        pub mod sim;
        pub mod store;
    }
}

pub mod frontend {
    /// The original's race tracks and their routes: what a custom race is
    /// raced as. The library's `frontend::career` re-exports them.
    pub mod career {
        pub mod routes;
        pub mod tracks;

        pub use routes::*;
        pub use tracks::*;
    }
}
