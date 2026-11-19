//! The test kit (crates/ob-testkit, docs/testing.md), bound to the game
//! crate. One file, compiled into each place that needs it:
//! * integration tests (`tests/*.rs`): `mod common;`
//! * the library's unit tests: `crate::testkit` (src/lib.rs, `cfg(test)`)
//! * the game binary's test modules: `#[path = "../../../tests/common/mod.rs"] mod testkit;`
//!   inside the module (src/bin/game/*.rs).
//!
//! `bind!` names the game crate `nfsu2`, as these places do (the library
//! through `extern crate self as nfsu2`), and takes their `cfg(ob_game)` /
//! `cfg(ob_private)`.
#![allow(dead_code)]

ob_testkit::bind!();
