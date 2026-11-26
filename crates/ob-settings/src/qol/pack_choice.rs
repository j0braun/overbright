//! The World Texture Pack row's value as the loader reads it (`render::enh::assets::pack_select`).

use std::sync::atomic::{AtomicU8, Ordering};

static CHOICE: AtomicU8 = AtomicU8::new(0);

/// The World Texture Pack row's value (`qol::Enhancements::texture_pack`).
pub fn set_choice(v: u8) {
    CHOICE.store(v.min(2), Ordering::Relaxed);
}

pub fn choice() -> u8 {
    CHOICE.load(Ordering::Relaxed)
}
