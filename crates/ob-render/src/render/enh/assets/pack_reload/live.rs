//! Studio Looks live (docs/modding/studio-looks.md, "Live in the test game"): a
//! test run started by Studio with `--looks-live <folder>` takes the textures of
//! the mod being edited from `<folder>/world` and `<folder>/car`, before every
//! other pack, and watches them as Reload Packs When Changed does, whatever
//! that setting says (nothing is written to options.txt). Only a test run
//! (`--map-mod` / `--map-file`) takes the flag; without it nothing here is
//! set and the game reads packs as before (the native path).

use super::*;

pub use super::texture_pack::{live, set_live};

/// What the loader gives for `texture` of the world (`car` false) or the
/// cars now: its top level's size and first pixel (RGBA). Studio's test of
/// the live path reads it to see when a save has arrived.
pub fn loaded_top(car: bool, texture: &crate::formats::texture::Texture) -> (u32, u32, [u8; 4]) {
    let domain = if car { Domain::Car } else { Domain::World };
    let (key, bytes) = crate::render::texture_layer_with_pack(domain, Some(texture), false, 4096);
    let mut px = [0; 4];
    let n = bytes.len().min(4);
    px[..n].copy_from_slice(&bytes[..n]);
    (key.1, key.2, px)
}

/// Whether texture `hash` of the world (`car` false) or the cars changed
/// after generation `seen` (as the renderers ask each frame), and the
/// generation that reaches.
pub fn changed_since(seen: u64, car: bool, hash: u32) -> (u64, bool) {
    let (now, p) = since(seen, if car { Domain::Car } else { Domain::World });
    (now, p.hits(hash, ""))
}
