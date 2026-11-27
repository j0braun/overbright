use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Cars.
    GraphicsRow {
        id: RowId::EnhancedPaint,
        label: ENHANCED_PAINT,
        group: GraphicsGroup::Cars,
        help: "Clearcoat, metallic flake and pearl shift per paint type.",
        tiers: &[OFF_ON[0], tc("On", "Richer car paint.", 0.15, 0.0)],
        native: None,
        get: |s| usize::from(s.enh.enhanced_paint),
        set: |s, i| s.enh.enhanced_paint = i != 0,
        offered: None,
        legacy: false,
    },
    // See docs/notes/graphics/lighting2.md.
    GraphicsRow {
        id: RowId::EnhancedGlass,
        label: ENHANCED_GLASS,
        group: GraphicsGroup::Cars,
        help: "Windows and lamp covers: the original tint, refraction of what is behind and a sharp reflection.",
        tiers: &[
            OFF_ON[0],
            tc(
                "On",
                "Glass that bends and reflects (integrated GPU: ~4 ms at a race start with MSAA).",
                GLASS_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.enhanced_glass),
        set: |s, i| s.enh.enhanced_glass = i != 0,
        offered: None,
        legacy: false,
    },
    // Customize Display's Car Reflection Detail (0x870CBC) with the
    // clone's cube size (enh.car_reflection) above it.
    GraphicsRow {
        id: RowId::CarReflection,
        label: 0x8FE9_288E, // Car Reflection Detail
        group: GraphicsGroup::Cars,
        help: "What the car bodies reflect and how sharp.",
        tiers: &[
            t("Low", "Original value: the sky only."),
            t("High", "Original value, World Detail 5: sky and world, 128 px."),
            tc("Very High", "Sharper reflection (256 px).", 0.0, 0.0),
            tc("Ultra", "Sharpest reflection (1024 px).", 0.17, 0.0),
        ],
        native: Some(1),
        get: |s| custom_get(s, 1, index_of(&qol::CAR_REFLECTION_STEPS, s.enh.car_reflection)),
        set: |s, i| s.enh.car_reflection = qol::CAR_REFLECTION_STEPS[custom_part(s, 1, i).unwrap_or(0)],
        offered: Some(|s, i| i <= 1 || !qol::software_gpu() || s.enh.car_reflection != 128),
        legacy: false,
    },
    // Customize Display's Car Reflection Update Rate (0x870CB8); absorbs
    // the removed clone row (enh.reflection_rate = all faces = High).
    GraphicsRow {
        id: RowId::ReflectionRate,
        label: 0x7D39_30E7, // Car Reflection Update Rate
        group: GraphicsGroup::Cars,
        help: "How many faces of the car reflection update each frame (the original's row).",
        tiers: &[
            t("Off", "Original value: no reflection (black)."),
            t("Low", "Original value: one face a frame."),
            t("Medium", "Original value: three faces a frame."),
            t("High", "Original value, World Detail 5: all six faces every frame."),
        ],
        native: Some(0),
        get: |s| if s.enh.reflection_rate != 0 { 3 } else { custom_get(s, 0, 0) },
        set: |s, i| {
            s.enh.reflection_rate = 0;
            custom_part(s, 0, i);
        },
        offered: None,
        legacy: false,
    },
    // Customize Display's Car Geometry Detail (0x870CF8) with the clone's
    // car LOD distance (enh.car_detail) above it.
    GraphicsRow {
        id: RowId::CarDetail,
        label: 0x4AC5_0BCF, // Car Geometry Detail
        group: GraphicsGroup::Cars,
        help: "Car model detail and how far away the detailed model is kept.",
        tiers: &[
            t("Low", "Original value: starts at the second model."),
            t("High", "Original value, World Detail 5: the full model."),
            tc("Ultra", "Full model kept twice as far (fewer switches).", 0.0, 0.0),
        ],
        native: Some(4),
        get: |s| custom_get(s, 4, usize::from(s.enh.car_detail > 100)),
        set: |s, i| s.enh.car_detail = qol::CAR_DETAIL_STEPS[custom_part(s, 4, i).unwrap_or(0)],
        offered: None,
        legacy: false,
    },
    // docs/notes/graphics/assets-distance.md: load-time silhouette smoothing.
    GraphicsRow {
        id: RowId::SmoothCars,
        label: SMOOTH_CARS,
        group: GraphicsGroup::Cars,
        help: "Rounds the cars' low-poly curves (arches, roof lines, bumpers, tyres) at load; flat panels and hard edges stay as modelled. Applies to cars loaded next.",
        tiers: &[
            t("Off", "Off: the models as shipped."),
            tc("On", "Curved edges split 3x3 within 2 cm of the original surface.", SMOOTH_CARS_COST_4K, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.smooth_cars),
        set: |s, i| s.enh.smooth_cars = i != 0,
        offered: None,
        legacy: false,
    },
];
