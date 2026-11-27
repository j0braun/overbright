//! Career Statistics: UI_Status_Master and its pages UI_Status_Career,
//! UI_Status_Region and UI_Status_DVD (the LEGEND key, state 0x13).
//!
//! The factory 0x4E1B80 first fills the statistics block 0x835C00 from
//! the career (0x528A70); the master (0x4E1A30) embeds the page package
//! (0x555F60) named by the table 0x7F4428 and steps it with Left / Right
//! (0x4B1E60). [`CareerStats`] is that block, filled from the clone's
//! career.

#[cfg(test)]
use super::{ScreenState, widgets::rating::{VR_PARTS, shop_rating}};
#[cfg(test)]
use crate::formats::hash::fe_hash;

pub use super::career::stats::*;
pub use super::screens::status::*;
pub use super::widgets::rating::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn shop_rating_widget() {
        // 0x54B010: the preview's text and bar, coloured by the comparison.
        let assets = crate::frontend::Assets::load(&crate::testkit::game_root()).expect("loading the game's front end assets");
        let package = assets.package("UI_Menu_Asset_Reputation").expect("UI_Menu_Asset_Reputation");
        let mut state = ScreenState::default();
        shop_rating(&mut state, package, 0.0, 0.12);
        assert_eq!(state.text[&0x171D_00CB], "0.12");
        assert_eq!(state.color[&0x6EC2_F0DE], crate::frontend::argb(RATING_COLOURS[1]));
        // Vertical bar (0x540EA0): caps hidden, the knob 0.95·f·track + 7
        // above the bottom, the handle 0.95·f·track long.
        let [_, knob, handle, beg, end] = VR_PARTS.map(fe_hash);
        assert!(state.hidden.contains(&beg) && state.hidden.contains(&end));
        let track = 237.867_14 - 88.0;
        let fill = 0.12 * track * 0.95;
        assert!((state.size[&handle].x - fill).abs() < 0.01);
        assert!((state.position[&knob].y - (30.300_354 + track * 0.5 - fill - 7.0)).abs() < 0.01);
        let mut lower = ScreenState::default();
        shop_rating(&mut lower, package, 2.0, 1.5);
        assert_eq!(lower.color[&0x171D_00CB], crate::frontend::argb(RATING_COLOURS[2]));
        let mut same = ScreenState::default();
        shop_rating(&mut same, package, 0.0, 0.0);
        assert_eq!(same.color[&0x171D_00CB], crate::frontend::argb(RATING_COLOURS[0]));
        assert_eq!(same.text[&0x171D_00CB], "0.00");
    }

    #[test]
    fn slider_value_text() {
        // 0x4B1835: trunc(f × 100) in the x87's precision, "%$d %%".
        assert_eq!(slider_text(0.0), "0 %");
        assert_eq!(slider_text(0.5), "50 %");
        assert_eq!(slider_text(0.29), "28 %");
        assert_eq!(slider_text(1.0), "100 %");
    }
}
