//! Quality-of-life options: **clone deviations**, every one opt-in, every
//! default native (docs/formats.md, "QoL (clone deviation)"). The settings store
//! and its option table are `ob_settings::qol`, and `setup` (finding and
//! checking the game folder on a first run) is `ob_assets::qol::setup`
//! (re-exported here at the old path); the library's own tests stay here.

pub use ob_assets::qol::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_kit;
#[cfg(test)]
pub(crate) use test_kit::test_globals;
