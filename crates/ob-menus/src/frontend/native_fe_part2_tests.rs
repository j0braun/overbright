//! Library-side tests of the front-end fixes, part 2
//! (docs/native-fixes/N-FE-2.tsv).

use crate::frontend::settings::{Settings, card_score};

/// 0x5BE880 above detail 3 (0x5BE97C–0x5BE9BC): ftol(0x5C0CE0) > 4 — only
/// the score-5 cards of 0x8005C0 — sets the resolution index to 1 (800 ×
/// 600) after Default Settings' index 0 (0x5CED3E).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn default_settings_on_a_score_five_card_take_800_by_600() {
    assert_eq!(card_score(0x10DE, 0x0040), 5, "GeForce 6800");
    assert_eq!(card_score(0x1002, 0x744C), 1, "an unlisted card");
    for card in [(0x10DE, 0x0040), (0x1002, 0x744C)] {
        let mut s = Settings::default();
        s.reset_display(card);
        let want = if s.detail > 3 && card_score(card.0, card.1) > 4 { (800, 600) } else { (640, 480) };
        assert_eq!(s.resolution, want, "{card:x?} at detail {}", s.detail);
    }
}
