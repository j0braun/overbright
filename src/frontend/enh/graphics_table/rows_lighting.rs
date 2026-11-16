use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Lighting.
    // docs/modern-lighting.md: the city relit from dynamic lights.
    GraphicsRow {
        id: RowId::ModernLighting,
        label: MODERN_LIGHTING,
        group: GraphicsGroup::Lighting,
        help: "The city lit by its street lamps, neon and signs and the cars' lights, with shadows, instead of the baked light alone.",
        tiers: &[
            t("Original", "Original: the baked lighting of the game."),
            tc(
                "Modern Low",
                "The nearest lights only, fewer and smaller shadows (integrated GPU: ~20 ms in a race).",
                MODERN_LOW_COST_4K,
                0.3,
            ),
            tc(
                "Modern",
                "Round lamp pools per pixel, neon and car lights, 24 shadowed lamps, wet specular, cars lit by the same lights.",
                MODERN_COST_4K,
                0.4,
            ),
            tc(
                "Modern + RT",
                "Modern with ray-traced shadows from every nearby light (cars included). Vulkan desktop GPUs; applies after a restart.",
                MODERN_RT_COST_4K,
                0.6,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.modern_lighting.min(3)),
        set: |s, i| s.enh.modern_lighting = i as u8,
        // Greyed out where the device has no ray queries (as SSR's tier).
        offered: Some(|s, i| i <= 2 || qol::rt_offered() || s.enh.modern_lighting == 3),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::ModernStrength,
        label: MODERN_STRENGTH,
        group: GraphicsGroup::Lighting,
        help: "How far Modern Lighting departs from the baked look: Classic keeps the original's light, Bold deepens the dark and lets the lamps' pools and neon stand out.",
        tiers: &[
            t("Classic", "The baked light kept; only shadows and highlights added."),
            t("25%", "A little deeper darkness and brighter pools."),
            t("50%", "Half way."),
            t("75%", "Bold pools and neon, the original's mood."),
            t("Bold", "Full dynamic lighting: dark streets, bright round pools."),
        ],
        native: None,
        get: |s| index_of(&qol::MODERN_STRENGTH_STEPS, s.enh.modern_strength),
        set: |s, i| s.enh.modern_strength = qol::MODERN_STRENGTH_STEPS[i],
        // Only with Modern Lighting on (else the current value, greyed).
        offered: Some(|s, i| {
            s.enh.modern_lighting != 0 || index_of(&qol::MODERN_STRENGTH_STEPS, s.enh.modern_strength) == i
        }),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::Ssao,
        label: SSAO,
        group: GraphicsGroup::Lighting,
        help: "Screen-space ambient occlusion: contact shadows in corners and under cars.",
        tiers: &[
            t("Off", "Off: as the original."),
            tc("Low", "Subtle shading (25%).", 0.91, 0.0),
            tc("Medium", "Clear shading (50%).", 0.93, 0.0),
            tc("High", "Full-strength shading (100%).", 0.96, 0.0),
        ],
        native: None,
        get: |s| index_of(&qol::SSAO_STEPS, s.enh.ssao),
        set: |s, i| s.enh.ssao = qol::SSAO_STEPS[i],
        offered: None,
        legacy: false,
    },
    // Customize Display's Car Shadow / Neon (g_CarShadowEnable 0x870CC0)
    // with the clone's shadow filtering (enh.shadows) above it.
    GraphicsRow {
        id: RowId::ShadowQuality,
        label: 0xA7A6_83AC, // Car Shadow / Neon
        group: GraphicsGroup::Lighting,
        help: "The cars' shadow and underglow.",
        tiers: &[
            t("Off", "Original value: no car shadow or neon."),
            t("Low", "Original value: the car shadow."),
            t("High", "Original value, World Detail 5: shadow and projected hull."),
            tc("Ultra", "Smoother shadow edge; clean, band-free neon glow.", 0.17, 0.15),
        ],
        native: Some(2),
        get: |s| custom_get(s, 2, usize::from(s.enh.shadows != 0)),
        set: |s, i| s.enh.shadows = if custom_part(s, 2, i).is_some() { qol::SHADOW_STEPS[1] } else { 0 },
        offered: None,
        legacy: false,
    },
    // Lamps and shadows (docs/lamps-shadows.md; costs: --bench explore 600 at night,
    // 1080p / 4K / 2-CU RDNA2 iGPU at 1080p, docs/evidence/lamps-shadows).
    GraphicsRow {
        id: RowId::DynamicLamps,
        label: DYNAMIC_LAMPS,
        group: GraphicsGroup::Lighting,
        help: "Street lamps light the road and the cars with their own colour and radius. Replaced by Modern Lighting while that is on (greyed).",
        tiers: &[
            t("Off", "Off: as the original (baked lighting only)."),
            // +0.03 ms 1080p, +0.03 4K, +3.6 iGPU.
            tc("On", "Per-pixel lamp pools; dark gaps between lamps stay dark (integrated GPU: ~3.6 ms).", 0.0, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.dynamic_lamps),
        set: |s, i| s.enh.dynamic_lamps = i != 0,
        // The settings model's conflict rule: Modern Lighting relights the lamps in
        // its place, so the row is greyed at its value while that is on.
        offered: Some(|s, i| s.enh.modern_lighting == 0 || usize::from(s.enh.dynamic_lamps) == i),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::LampShadows,
        label: LAMP_SHADOWS,
        group: GraphicsGroup::Lighting,
        help: "Nearby lamps and your headlights cast soft shadows (cars, posts, walls). With Modern Lighting the lamps' shadows are its own; this row then keeps only your headlights' shadow (Low and High alike).",
        tiers: &[
            t("Off", "Off: as the original."),
            // Over Dynamic Lamps: +0.13 ms 1080p, +0.25 4K, +5.0 iGPU.
            tc("Low", "4 nearest lamps and the headlights, 512 maps (integrated GPU: ~5 ms).", 0.23, 0.09),
            // +0.21 ms 1080p, +0.46 4K, +9.3 iGPU.
            tc(
                "High",
                "12 nearest lamps and the headlights, 1024 maps, softer edges (integrated GPU: ~9 ms).",
                0.47,
                0.13,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.lamp_shadows.min(2)),
        set: |s, i| s.enh.lamp_shadows = i as u8,
        // High adds only lamp maps, which Modern Lighting replaces.
        offered: Some(|s, i| s.enh.modern_lighting == 0 || i < 2 || usize::from(s.enh.lamp_shadows.min(2)) == i),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::ContactShadows,
        label: CONTACT_SHADOWS,
        group: GraphicsGroup::Lighting,
        help: "Soft darkening of the ground right under each car; adds to the car shadow, does not replace it. With Modern Lighting it gives the darker of itself and the lights' shadows, not both.",
        tiers: &[
            t("Off", "Off: as the original."),
            // +0.02 ms 1080p, +0.01 4K, +2.7 iGPU.
            tc("On", "Grounded cars (integrated GPU: ~2.7 ms).", 0.94, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.contact_shadows),
        set: |s, i| s.enh.contact_shadows = i != 0,
        offered: None,
        legacy: false,
    },
    // docs/lighting2.md: every car's headlights as projected beams;
    // probes along the roads for the cars' light.
    GraphicsRow {
        id: RowId::HeadlightBeams,
        label: HEADLIGHT_BEAMS,
        group: GraphicsGroup::Lighting,
        help: "Every car's headlights light the road and walls ahead as a soft beam with a bright centre. Next-Gen: every car's lights also glow in the air (rain, fog, smoke), tail, brake and neon light the road and the cars, with lens streaks.",
        tiers: &[
            t("Off", "Off: as the original (your car's projected headlight only)."),
            tc(
                "On",
                "Your car's beam with the headlight shadow (Lamp Shadows), traffic and rivals without (integrated GPU: cheaper than Off, −2.3 ms in a race).",
                BEAMS_COST_4K,
                0.0,
            ),
            // Next-Gen car lights (docs/car-lights.md "Cost").
            tc(
                "Next-Gen",
                "The beams plus volumetric headlight cones and tail / brake halos in rain, fog and smoke, tail, brake and underglow light on the road and the cars' bodies, other cars' lights on your paint, lens streaks (integrated GPU: ~+8 ms).",
                CAR_LIGHTS_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.headlight_beams.min(2)),
        set: |s, i| s.enh.headlight_beams = i as u8,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::LightProbes,
        label: LIGHT_PROBES,
        group: GraphicsGroup::Lighting,
        help: "Cars pick up the colour of nearby neon, signs and lamps; dark streets stay dark.",
        tiers: &[
            t("Off", "Off: as the original (street lamps and the fixed rig only)."),
            tc(
                "On",
                "Probes along the roads, captured from the world as you drive (~0.1 ms of CPU while new ones are captured).",
                PROBES_COST_4K,
                PROBES_CPU_MS,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.light_probes),
        set: |s, i| s.enh.light_probes = i != 0,
        offered: None,
        legacy: false,
    },
    // docs/surface-maps.md: material-ID roughness / specular, and
    // normal maps made on this machine from the game's own textures.
    GraphicsRow {
        id: RowId::SurfaceDetail,
        label: SURFACE_DETAIL,
        group: GraphicsGroup::Lighting,
        help: "Street lamps and headlights glint on metal, glass and wet asphalt; roads and walls show their grain in the light.",
        tiers: &[
            t("Off", "Off: as the original (no surface materials)."),
            tc(
                "Materials",
                "Roughness and shine per surface (asphalt, concrete, metal, glass, signs).",
                SURFACE_MAT_COST_4K,
                0.0,
            ),
            tc(
                "Materials + Normals",
                "Also relief from normal maps made once from your game's textures (cached; a few seconds the first time).",
                SURFACE_NRM_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.surface_detail.min(2)),
        set: |s, i| s.enh.surface_detail = i as u8,
        offered: None,
        legacy: false,
    },
    // See docs/temporal.md; costs: --bench explore at night, reference desktop GPU.
    GraphicsRow {
        id: RowId::VolumetricFog,
        label: VOLUMETRIC_FOG,
        group: GraphicsGroup::Lighting,
        help: "Light scattered in the night air under the street lamps and in the headlight beams.",
        tiers: &[
            t("Off", "Off: as the original."),
            tc(
                "Low",
                "The haze of Next-Gen at a coarser grid and range: soft light under the 12 nearest street lamps, the nearest signs and your headlight beam (integrated GPU: ~2 ms).",
                VOLUMETRIC_LOW_COST_4K,
                0.0,
            ),
            tc(
                "High",
                "The haze of Next-Gen at a finer grid: light under 32 lamps with shafts through trees and bridges, glows round the nearest signs, your headlight beam (integrated GPU: ~3 ms).",
                VOLUMETRIC_HIGH_COST_4K,
                0.0,
            ),
            tc(
                "Next-Gen",
                "A volumetric haze over the whole city: light shafts under the lamps through trees and bridges, coloured glows round neon and signs, distance haze in the night's own colour, heavier and lower in rain, hiding behind smoke (integrated GPU: ~4–5 ms).",
                VOLUMETRIC_NEXT_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.volumetric_fog.min(3)),
        set: |s, i| s.enh.volumetric_fog = qol::VOLUMETRIC_STEPS[i],
        offered: None,
        legacy: false,
    },
    // See docs/atmosphere.md.
    GraphicsRow {
        id: RowId::Sky,
        label: SKY,
        group: GraphicsGroup::Lighting,
        help: "The night sky over Bayview.",
        tiers: &[
            t("Original", "Original: the painted sky dome."),
            tc(
                "Next-Gen",
                "A moon, stars and drifting clouds lit from below by the city's glow, in the original sky's colours; lower, heavier clouds in rain; seen in the car and road reflections too (integrated GPU: ~3.5 ms).",
                SKY_NEXT_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.sky.min(1)),
        set: |s, i| s.enh.sky = i as u8,
        offered: None,
        legacy: false,
    },
    // Reflections.
    GraphicsRow {
        id: RowId::EnhancedReflections,
        label: ENHANCED_REFLECTIONS,
        group: GraphicsGroup::Reflections,
        help: "Surface-aware car reflection shading and filtered road reflections.",
        tiers: &[OFF_ON[0], tc("On", "Glossier cars, cleaner road mirror.", 0.23, 0.0)],
        native: None,
        get: |s| usize::from(s.enh.enhanced_reflections),
        set: |s, i| s.enh.enhanced_reflections = i != 0,
        offered: None,
        legacy: false,
    },
    // Customize Display's Road Reflection Detail (0x870CD4) with the
    // clone's reflection image size (enh.road_reflection) above it.
    GraphicsRow {
        id: RowId::RoadReflection,
        label: 0x2A2A_4AB3, // Road Reflection Detail
        group: GraphicsGroup::Reflections,
        help: "The road's mirror image of the world.",
        tiers: &[
            t("Off", "Original value: no road reflection."),
            t("Low", "Original value: road reflection on."),
            t("Medium", "Original value."),
            t("High", "Original value, World Detail 5 (320x240 image)."),
            tc("Very High", "Reflection at half the screen's resolution.", 0.0, 0.0),
            tc("Ultra", "Reflection at the screen's resolution.", 0.21, 0.0),
        ],
        native: Some(7),
        get: |s| custom_get(s, 7, usize::from(s.enh.road_reflection.min(2))),
        set: |s, i| s.enh.road_reflection = custom_part(s, 7, i).unwrap_or(0) as u8,
        offered: Some(|s, i| i <= 3 || !qol::software_gpu() || s.enh.road_reflection != 0),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::WetRoads,
        label: WET_ROADS,
        group: GraphicsGroup::Reflections,
        help: "Rain-driven ripples, puddles and streaked reflections on the road.",
        tiers: &[OFF_ON[0], tc("On", "Wet look while it rains.", 0.0, 0.0)],
        native: None,
        get: |s| usize::from(s.enh.wet_roads),
        set: |s, i| s.enh.wet_roads = i != 0,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::Ssr,
        label: SSR,
        group: GraphicsGroup::Reflections,
        help: "Screen-space reflections of what is on screen (cars, lights) on wet roads and car paint.",
        tiers: &[
            t("Off", "Off: as the original."),
            tc("High", "Traced at half resolution (integrated GPU: est. ~15 ms).", SSR_COSTS_4K[0], 0.0),
            tc("Ultra", "Traced at full resolution.", SSR_COSTS_4K[1], 0.0),
            tc(
                "Ray Traced (experimental)",
                "Ultra + hardware ray queries for what the screen misses (behind the camera, off screen). Vulkan desktop GPUs; applies after a restart.",
                RT_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.ssr.min(3)),
        set: |s, i| s.enh.ssr = i as u8,
        // Greyed out where the device has no ray queries.
        offered: Some(|s, i| i <= 2 || qol::rt_offered() || s.enh.ssr == 3),
        legacy: false,
    },
];
