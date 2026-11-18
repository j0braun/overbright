//! The engine's tables: the numbers, curves and names the original game
//! keeps in its executable, written in Overbright's own code
//! (docs/exe-tables.md). Overbright does not read SPEED2.EXE: as other
//! engine reimplementations do, it reads the assets (models, textures,
//! sounds, text, tracks) from the player's game files and carries the
//! engine's rules and constants itself.
//!
//! The tables come from three places:
//! * [`generated`]: tables that are pure maths (sine, arctangent, arcsine,
//!   the mixer's dB curves), computed from their formulas;
//! * [`third_party`]: the audio and video decoders' standard tables, taken
//!   from open decoders (vgmstream, FFmpeg) with their notices;
//! * [`constants`]: the tuning numbers and facts (handling curves, sound
//!   records, camera views, front end lists), as named constants.
//!
//! Each table equals the original's entry for entry: a test checks every
//! table's hash ([`FIELD_PINS`]), and a data test compares every table
//! with the reference executable itself (`reader`, tests only).
//!
//! **One process-wide holder.** The tables are used from many places —
//! the audio threads, the car physics, the movie and audio decoders, the
//! front end and the formats — so they live in one holder ([`tables`])
//! built on first use; the frame path only reads memory that is already
//! there (an atomic load).

pub mod constants;
pub mod generated;
#[cfg(test)]
mod reader;
mod tables;
#[cfg(test)]
mod tests;
pub mod third_party;

pub use tables::*;

use std::sync::LazyLock;

/// A table as its users name it: a `static` that reads as the table
/// (through `Deref`), served from [`tables`].
pub struct Table<T: ?Sized + 'static>(fn(&'static Tables) -> &'static T);

impl<T: ?Sized + 'static> Table<T> {
    pub const fn new(field: fn(&'static Tables) -> &'static T) -> Table<T> {
        Table(field)
    }
    /// The table ([`tables`]).
    #[inline]
    pub fn loaded(&self) -> &'static T {
        (self.0)(tables())
    }
}

impl<T: ?Sized + 'static> std::ops::Deref for Table<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.loaded()
    }
}

impl<T: std::fmt::Debug + ?Sized + 'static> std::fmt::Debug for Table<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.loaded().fmt(f)
    }
}

/// Every table, built once on first use ([`Tables::builtin`]).
#[inline]
pub fn tables() -> &'static Tables {
    static TABLES: LazyLock<Tables> = LazyLock::new(|| {
        let _t = crate::prof::span("exe_tables::build");
        Tables::builtin()
    });
    &TABLES
}
