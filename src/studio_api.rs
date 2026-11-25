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

/// The world: placements and their original keys (`World::original_placement`),
/// the map overlay, loading a region.
pub mod world {
    pub use crate::world::{HIDDEN_FLAG, Placement, World, is_panorama, parts_of, transform_of};
}

/// The id of this build (the build that writes a shared mod's provenance).
pub mod build {
    pub use crate::BUILD_ID;
}

/// The knock-over props a world keeps.
pub mod smackable {
    pub use crate::smackable::Prop;
}

/// Drawing a world without the game: the renderer, its options and the
/// overview camera.
pub mod render {
    pub use crate::render::{
        FrameParams, GLOBAL_SECTION, Gpu, HeadlightPool, MAIN_MIN_SIZE, RenderOptions, WorldRenderer, create_depth,
        overview_camera,
    };
    pub mod display {
        pub use crate::render::display::sdr_surface_format;
    }
    /// The game's car drawing (a recorded run's cars over the world).
    pub mod car {
        pub use crate::render::car::CarRenderer;
        pub use crate::render::car::lighting::{CameraAxes, CarLighting};
    }
    /// The view's sprite scale (`256 / tan(fov / 2)`), as the game's flares
    /// take it.
    pub use crate::render::cull_scale;
}

/// A car as the game builds it from its model and fitted parts, and its
/// wheels as drawn.
pub mod car {
    pub use crate::car::{Car, WheelPose};
}

/// The game's light flares: a car's head and brake light sprites as the
/// game's glow pass builds them (`WorldRenderer::upload_glows` /
/// `draw_glows` draw them).
pub mod glow {
    pub use crate::glow::{View, car_sprites};
    pub use crate::particles::Quad;
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

/// The game folder the launcher's setup saved, and the lookup Studio's programs share (`NFSU2_ROOT` first).
pub mod qol {
    pub mod setup {
        pub use crate::qol::setup::{game_root, saved_root};
    }
}

/// The map editor's logic on a world: the edits and the game data it reads
/// (the map data itself is crates/ob-map).
pub mod map {
    pub use crate::tools::map::{Applied, assets, edit};
}

/// Studio Looks live in the test game (docs/modding/studio-looks.md): the
/// game's side of `--looks-live`, the pack watcher's tick, and what the
/// texture loader and the renderers see of a change. Studio starts the test
/// game with the flag; its tests make the same calls in their own process.
pub mod looks {
    pub use crate::render::pack_reload::{TICK, changed_since, frame, generation, live, loaded_top, set_live};
}

/// The game's career data (the base tracks' events), for checks against the
/// real tracks.
pub mod career {
    pub use crate::frontend::career::CareerData;
}
