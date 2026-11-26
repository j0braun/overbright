//! Optional external PNG replacements; original materials remain authoritative.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

mod cache;
mod decode;
mod roots;
pub use cache::*;
pub use decode::*;
pub use roots::*;
pub use roots::{live, set_live};

pub struct Replacement {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// None: RGBA8 levels (PNG); Dxt1/Dxt5: block-compressed levels from a
    /// DDS (the upscaled world pack, `crate::upscale`).
    pub format: Option<crate::formats::texture::PixelFormat>,
    pub bytes: Vec<u8>,
}

/// Select by renderer, so identical hashes cannot leak between UI, world and cars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    Ui,
    World,
    Car,
}

