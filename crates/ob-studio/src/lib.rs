//! Overbright Studio's operation layer, without a window (ARCHITECTURE.md, "Studio"): what Studio's window
//! (crates/ob-studio-gui), its command-line tool and its MCP adapter share.
//!
//! It uses the game library only through [`nfsu2::studio_api`], its one interface for Studio, so the library can
//! move its modules without touching Studio (`scripts/checks/crate_layers.sh` refuses any other `nfsu2::` path).

pub use nfsu2::studio_api;

pub mod cli;
pub mod history;
pub mod keys;
pub mod looks;
pub mod merge;
pub mod registry;
pub mod session;
pub mod validate;

/// The documents of a mod's history: the map (`map.obmap`) and the texture list (`looks.toml`, [`looks`]).
pub const MOD_DOCS: &[&str] = &[ob_map::tools::map::store::FILE, looks::FILE];
