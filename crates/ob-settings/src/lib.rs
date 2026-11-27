//! Overbright's settings layer (ARCHITECTURE.md "Layers"): the quality-of-life settings store and its option
//! table (`qol`: the Enhancements page's values, options.txt's `enh.*` lines, the values in force, the presets, durable
//! saves, crash reports, the waits log) and the extras' and pad options they hold (`extras`, `pad::extras`).
//! On ob-core and ob-formats only; the library (`nfsu2`) re-exports every module at its old path
//! (`qol::setup`, which needs the game's assets, stays there).

// The lower layers' modules at the paths these files name (`crate::…`).
pub(crate) use ob_core::{diag, draw, fingerprint, log_file, platform, prof};
#[cfg(test)]
pub(crate) use ob_core::{frame_clock, game_fs, unique_tag};
#[cfg(test)]
pub(crate) use ob_formats::formats;

// The test kit (crates/ob-testkit, docs/developing/testing.md) for this crate's unit
// tests, bound to this crate under the name the kit expects.
#[cfg(test)]
extern crate self as nfsu2;
#[cfg(test)]
mod testkit {
    ob_testkit::bind!();
}

pub mod extras;
pub mod pad {
    pub mod extras;
}
pub mod qol;
