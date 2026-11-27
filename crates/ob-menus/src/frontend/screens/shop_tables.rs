/// The car specialties shop's part categories (UI_ICEMAIN, 0x5593F0 mode
/// 0, each through UI_IcePartsOverlay): slot, icon, label, and the part
/// kind it lists (split hoods are the kind-5 hoods the body shop leaves
/// out). Custom Gauges list through UI_CustomHUDOverlay instead (0x53AB40
/// with that package). Neon (UI_CustomNeonMain) and the trunk audio have
/// their own screens, not in the clone.
pub const SPECIALTIES: [(u8, u32, u32, Option<u8>); 10] = [
    (0xA3, crate::frontend::fe_ids::CUSTOM_GAUGES_ICON, crate::frontend::fe_ids::CUSTOM_GAUGES_LABEL, None),    // Custom Gauges
    (0x9E, crate::frontend::fe_ids::SPECIALTIES_2, crate::frontend::fe_ids::SPECIALTIES_3, None),    // Neon: opens UI_CustomNeonMain
    (0x99, crate::frontend::fe_ids::WINDOW_TINT_ICON, crate::frontend::fe_ids::WINDOW_TINT_LABEL, None),    // Window Tint
    (0x9F, crate::frontend::fe_ids::LIGHTS_ICON, crate::frontend::fe_ids::LIGHTS_LABEL, None),    // Lights
    (0xA2, crate::frontend::fe_ids::NITROUS_PURGE_ICON, crate::frontend::fe_ids::NITROUS_PURGE_LABEL, None),    // Nitrous Purge
    (0xA1, crate::frontend::fe_ids::HYDRAULICS_ICON, crate::frontend::fe_ids::HYDRAULICS_LABEL, None),    // Hydraulics
    (0x22, crate::frontend::fe_ids::SPECIALTIES_12, crate::frontend::fe_ids::SPECIALTIES_13, None),    // Trunk audio: opens its menu
    (0x1D, crate::frontend::fe_ids::SPINNERS_ICON, crate::frontend::fe_ids::SHOP_TABLES_SPINNERS_LABEL, None),    // Spinners
    (0x09, crate::frontend::fe_ids::SPLIT_HOODS_ICON, crate::frontend::fe_ids::SPLIT_HOODS_LABEL, Some(5)), // Split Hoods
    (0xA0, crate::frontend::fe_ids::DOORS_ICON, crate::frontend::fe_ids::DOORS_LABEL, None),    // Doors
];

/// The trunk audio slots a layout opens (0x610400): none without one; by
/// the layout part's level and the car (type index; SUVs 8–10 and some
/// others take more), the slots 0x23 up to the returned bound.
pub fn trunk_slots(level: u8, car: usize) -> std::ops::Range<u8> {
    let end = match level {
        0 => 0x23,
        1 => 0x23 + if (8..=10).contains(&car) { 3 } else { 1 } + 1,
        2 => {
            if (8..=10).contains(&car) {
                0x29
            } else {
                0x27
            }
        }
        3 => match car {
            5 | 0xD | 0xF | 0x12 | 0x16 | 0x19 | 0x1A | 0x1C | 0x1E => 0x29,
            8..=10 => 0x2F,
            _ => 0x28,
        },
        _ => 0x24,
    };
    0x23..end
}

/// UI_CustomNeonMain's slots (0x548020; each through UI_NeonPartsOverlay):
/// slot, icon, label.
pub const NEON: [(u8, u32, u32); 3] = [
    (0x9A, crate::frontend::fe_ids::UNDERGLOW_NEON_ICON, crate::frontend::fe_ids::UNDERGLOW_NEON_LABEL), // Underglow Neon
    (0x9B, crate::frontend::fe_ids::ENGINE_NEON_ICON, crate::frontend::fe_ids::ENGINE_NEON_LABEL), // Engine Neon
    (0x9D, crate::frontend::fe_ids::TRUNK_NEON_ICON, crate::frontend::fe_ids::TRUNK_NEON_LABEL), // Trunk Neon
];
