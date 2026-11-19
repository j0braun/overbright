//! The core of Overbright's own asset format (docs/modding/asset-format/): the rules every file of the format follows,
//! and the pieces every reader of it shares.
//!
//! - [`rules`]: the registry of rule ids; every refusal names one ([`Refusal`]).
//! - [`float`]: floats that come back with the exact bits that were written (conventions.md §3.4).
//! - [`names`]: pack ids, logical paths, game names, 32-bit hash keys, references and `ob://` addresses (§6).
//! - [`digest`]: `sha256` and `xxh3-128` digests and digests of lists (§7).
//! - [`text`]: the text and TOML rules (§3.1–3.3): encoding, syntax, the `schema` key and its versions.
//! - [`json`]: JSON values: the schemas' text, the TOML-to-JSON mapping, RFC 8785's canonical form.
//! - [`fields`]: field tables; they check a TOML table and also give the pages' tables and the JSON Schemas.
//! - [`bundle`]: bundles (`.obz`) and pack files (`.obpk`): a zero-copy reader and a deterministic writer
//!   (layout.md §4).
//! - [`manifest`]: `pack.toml` format 2, today's format-1 texture-pack manifests, the content digest,
//!   `migrations.toml` (manifest.md, versioning.md §4.3).
//! - [`order`]: `order.txt` and `active.txt` (layering.md §2, layout.md §1).
//! - [`layering`]: the layer stack and the merge of items: merge, add, replace, disable, and the clash list
//!   (layering.md).
//! - [`docs`]: the generated parts of the specification (rule tables, field tables, schemas) and the check that
//!   the pages and the code agree.
//!
//! Nothing in the game reads through this crate yet; the format is still a draft (docs/modding/asset-format/README.md).

// A refusal carries its whole context (pack, path, field, message) and is only made on the cold path.
#![allow(clippy::result_large_err)]

pub mod bundle;
pub mod digest;
pub mod docs;
pub mod fields;
pub mod float;
pub mod json;
pub mod layering;
pub mod manifest;
pub mod names;
pub mod order;
mod refusal;
pub mod rules;
pub mod text;

pub use refusal::Refusal;
pub use rules::Rule;
