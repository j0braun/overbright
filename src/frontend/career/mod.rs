//! Career data (GLOBAL/GlobalB, chunk 0x80034A10; see docs/formats.md,
//! "Career"), the race tracks (chunk 0x34201), race routes
//! (TRACKS/ROUTES<region>/Paths<track>.bin and TrackPosMarkersAll.bin)
//! and the player's profile.
//!
//! The game reads the first 0x80034A10 only (0x512C00, guarded by
//! 0x8641F4); its children are tables of fixed-size records:
//!
//! | child   | record | what |
//! |---------|--------|------|
//! | 0x34A11 | 0x88   | events (races) |
//! | 0x34A12 | 0xA0   | shops |
//! | 0x34A14 | 0x44   | brands (0x838400): name, display name, hash |
//! | 0x34A16 | 0x40   | magazine and DVD rewards |
//! | 0x34A1C | 0x28   | part costs: visual points, prices, unlocks |
//! | 0x34A1D | —      | the string pool the records point into |

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use glam::Vec3;

use crate::formats::bundle::{chunks, read_cstr, read_f32, read_u16, read_u32};
use crate::formats::hash::bin_hash;

mod data;
pub mod events;
mod intro;
mod lists;
mod native_map;
mod persist;
mod profile;
mod records;
mod rewards;
mod rules;
mod shops;
mod slots;
mod sms_rules;
pub mod stats;
pub mod triggers;

pub use data::*;
pub use events::*;
pub use intro::*;
pub use lists::*;
pub use native_map::*;
use persist::*;
pub use profile::*;
pub use records::*;
pub use rewards::*;
// The tracks and their routes are the map data's (crates/ob-map), at their old paths.
pub use ob_map::frontend::career::*;
pub use shops::*;
pub use slots::*;
pub use stats::{CareerStats, PAGES, Row, STAGE_GOALS, career_rows};
pub use triggers::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// A sponsor count above the five hash slots is
    /// clamped instead of reading past the 0x50-byte record.
    #[test]
    fn stage_def_clamps_the_sponsor_count() {
        let mut r = [0u8; 0x50];
        r[1] = 0xFF;
        for i in 0..5 {
            r[8 + 4 * i] = i as u8 + 1;
        }
        let d = StageDef::parse(&r);
        assert_eq!(d.sponsors, [1, 2, 3, 4, 5]);
        r[1] = 2;
        assert_eq!(StageDef::parse(&r).sponsors, [1, 2]);
    }
}
