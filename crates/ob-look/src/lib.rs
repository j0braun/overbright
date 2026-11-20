//! Overbright's shared look (docs/STYLE.md): what the launcher, the map editor
//! and Studio draw with, defined once so they look alike.
//!
//! * [`theme`]: the colour tokens (look A, "Slab & Rail").
//! * [`lamp`]: the status lamps.
//! * [`fonts`]: the type, Sofia Sans Semi Condensed (SIL OFL 1.1).
//! * [`legends`]: the button legends' roles, glyph sets and glyphs.
//!
//! Pure data: each program draws it its own way (the launcher on the CPU,
//! the map editor and Studio through egui).

pub mod fonts;
pub mod lamp;
pub mod legends;
pub mod theme;
