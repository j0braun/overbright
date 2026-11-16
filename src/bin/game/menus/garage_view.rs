//! The screens' returns of GarageMain to its base view (0x4C2140(0,
//! 1)) that `Game::garage_view_returns` (garage/request.rs) lists as open,
//! settled from SPEED2.EXE (GarageViewPhase, read under Wine and in
//! assembly). Kept here as a function of the two phases so the garage
//! side needs one call (`|| screen_base_view_returns(was, is)`).

use crate::GarageViewPhase;

/// Whether the change from `was` to `is` (the screens GarageMain stood
/// under last frame and now) is one of these returns to the base view:
/// * UI_DecalMain left with Back from its top (0x5682B0's 0x911AB364 case:
///   no list open (+0x174) and a top node (+0x154 >> 16 = 0) → the
///   package's 0xDE6EFF34, +0x170 = 1 at 0x5683C2; its next tick runs
///   0x56B8D0 → 0x4C2140(0, 1)). Back inside a list or below the top node
///   (0x81F0B71A, 0x560030(−1)) does not set it.
/// * UI_CustomNeonMain (handler 0x56A890, symbols.tsv): its constructor
///   0x553500 sets +0x15B = 1 (0x553547), so its first tick returns the
///   view (0x56AAD5); its Back (0x911AB364) sets it again when
///   0x537D50(0x47FF4E7C, 0xDE6EFF34) returns 0 (0x56AA4B), on the way out.
pub(crate) fn screen_base_view_returns(was: GarageViewPhase, is: GarageViewPhase) -> bool {
    let on = |p: GarageViewPhase, name: &str| matches!(p, GarageViewPhase::Browser { package, .. } if package == name);
    let decal_left = on(was, "UI_DecalMain") && !on(is, "UI_DecalMain");
    let neon_entered = on(is, "UI_CustomNeonMain") && !on(was, "UI_CustomNeonMain");
    let neon_left = matches!(was, GarageViewPhase::Browser { package: "UI_CustomNeonMain", list: false, .. })
        && !on(is, "UI_CustomNeonMain");
    decal_left || neon_entered || neon_left
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser(package: &'static str, list: bool) -> GarageViewPhase {
        GarageViewPhase::Browser { package, list, sub: false }
    }

    /// 0x5683C2 / 0x56B8D0, 0x553547 / 0x56AA4B / 0x56AAD5.
    #[test]
    fn decal_and_neon_screens_return_the_base_view() {
        let other = GarageViewPhase::Other;
        assert!(screen_base_view_returns(browser("UI_DecalMain", false), other), "UI_DecalMain left");
        assert!(!screen_base_view_returns(browser("UI_DecalMain", true), browser("UI_DecalMain", false)), "a list closed");
        assert!(screen_base_view_returns(browser("UI_ICEMAIN", false), browser("UI_CustomNeonMain", false)), "neon's first tick");
        assert!(screen_base_view_returns(browser("UI_CustomNeonMain", false), browser("UI_ICEMAIN", false)), "neon's Back");
        assert!(!screen_base_view_returns(browser("UI_CustomNeonMain", false), browser("UI_CustomNeonMain", true)));
        assert!(!screen_base_view_returns(browser("UI_ICEMAIN", false), browser("UI_ICEMAIN", true)));
    }
}
