//! The desktop map editor (`overbright-editor`, docs/map-editor.md), a
//! leaf crate on the game library `nfsu2`. The editor keeps its module
//! folder (`src/editor/`), and the library's modules it uses are imported
//! here under their own names, so its `crate::…` paths read as before.

pub mod editor;

use nfsu2::{draw, fog, formats, launch, platform, qol, render, sky, tools, world};
// The game's career data: read only through `tools::map::assets` (the tests
// read it directly to check against the real tracks).
#[cfg(test)]
use nfsu2::frontend;

// The test kit (crates/ob-testkit, docs/testing.md), bound to the game
// library.
#[cfg(test)]
mod testkit {
    #![allow(dead_code)]
    ob_testkit::bind!();
}
