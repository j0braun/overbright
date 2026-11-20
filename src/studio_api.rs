//! The one interface Studio's crates use into the game library
//! (docs/ARCHITECTURE.md, "Studio's interface").
//!
//! Studio (crates/ob-studio, crates/ob-studio-gui) names this library only
//! as `nfsu2::studio_api::…`; `scripts/checks/crate_layers.sh` refuses any
//! other `nfsu2::` path in those crates. What the library offers Studio is
//! listed here item by item, under the library's own module names, so the
//! library can move and split its modules behind it without touching Studio:
//! a move changes a line here, not Studio's code.
//!
//! Studio reaches the lower layers (ob-core, ob-formats, ob-map, ob-look)
//! through their own crates. The check refuses a glob re-export of the
//! library's own modules here, so nothing becomes part of the interface
//! without a line of its own.

/// The world: placements, the map overlay, loading a region.
pub mod world {
    pub use crate::world::{HIDDEN_FLAG, Placement, World, is_panorama, parts_of, transform_of};
}

/// Drawing a world without the game: the renderer, its options and the
/// overview camera.
pub mod render {
    pub use crate::render::{
        FrameParams, GLOBAL_SECTION, Gpu, MAIN_MIN_SIZE, RenderOptions, WorldRenderer, create_depth, overview_camera,
    };
    pub mod display {
        pub use crate::render::display::sdr_surface_format;
    }
}

/// The fog a world is drawn with.
pub mod fog {
    pub use crate::fog::{Fog, FogState};
}

/// The time of day the sky and the lights follow.
pub mod sky {
    pub use crate::sky::TimeOfDay;
}

/// The program's window icon.
pub mod platform {
    pub mod icon {
        pub use crate::platform::icon::window_icon;
    }
}

/// The game folder the launcher's setup saved.
pub mod qol {
    pub mod setup {
        pub use crate::qol::setup::saved_root;
    }
}

/// The map editor's logic on a world: the edits and the game data it reads
/// (the map data itself is crates/ob-map).
pub mod map {
    pub use crate::tools::map::{Applied, assets, edit, session};
}

/// The game's career data (the base tracks' events), for checks against the
/// real tracks.
pub mod career {
    pub use crate::frontend::career::CareerData;
}
