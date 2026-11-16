//! The output device: the cpal stream, its callback and the watchdog
//! that reopens it (platform; no native counterpart).

mod cpal;

pub(crate) use self::cpal::*;
pub use self::cpal::MixerOpening;
