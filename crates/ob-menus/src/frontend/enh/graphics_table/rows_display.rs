use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Performance.
    GraphicsRow {
        id: RowId::RenderScale,
        label: RENDER_SCALE,
        group: GraphicsGroup::Performance,
        help: "The 3D scene's resolution; the menus and HUD stay sharp.",
        tiers: &[
            tc("50%", "Half resolution: fastest, blurry.", -1.32, 0.0),
            tc("67%", "Lower resolution.", -0.9, 0.0),
            tc("75%", "Lower resolution.", -0.74, 0.0),
            tc("85%", "Slightly lower resolution.", -0.4, 0.0),
            t("100%", "Native: the screen's resolution."),
        ],
        native: None,
        get: |s| index_of(&qol::SCALE_STEPS[..5], s.enh.render_scale.min(100)),
        set: |s, i| s.enh.render_scale = qol::SCALE_STEPS[i],
        offered: Some(|s, i| qol::scale_offered(qol::SCALE_STEPS[i], qol::output_size_or(s.resolution))),
        legacy: false,
    },
    // docs/notes/graphics/temporal.md: the temporal upscaler; costs are the savings
    // against the screen's resolution with TAA (measured, 4K).
    GraphicsRow {
        id: RowId::Upscale,
        label: UPSCALE,
        group: GraphicsGroup::Performance,
        help: "Renders fewer pixels and rebuilds the screen's: from several frames (turns on TAA), AMD FSR 1 from one frame, or AMD FSR 2 from several frames (replaces SSAA / MSAA).",
        tiers: &[
            t("Off", "Off: the scene at the Render Scale."),
            tc("Quality", "67% of each side, reconstructed; close to native sharpness.", UPSCALE_COSTS_4K[0], 0.0),
            tc("Balanced", "58% of each side, reconstructed.", UPSCALE_COSTS_4K[1], 0.0),
            tc(
                "Performance",
                "50% of each side, reconstructed; softest (integrated GPU: saves ~1.3 ms against 8x MSAA).",
                UPSCALE_COSTS_4K[2],
                0.0,
            ),
            // docs/playing/modern-extras.md: AMD FSR 1, spatial, the native
            // anti-aliasing kept at the lower size.
            tc(
                "FSR Quality",
                "AMD FSR 1: 67% of each side, edge-adaptive upscale and sharpening; no temporal smearing, keeps the MSAA setting.",
                FSR_COSTS_4K[0],
                0.0,
            ),
            tc("FSR Balanced", "AMD FSR 1: 58% of each side.", FSR_COSTS_4K[1], 0.0),
            tc("FSR Performance", "AMD FSR 1: 50% of each side; softest.", FSR_COSTS_4K[2], 0.0),
            // docs/notes/graphics/fsr2.md: AMD FSR 2, temporal; the Anti-Aliasing
            // row (SSAA, MSAA, filters) is off under it.
            tc(
                "FSR 2 Native AA",
                "AMD FSR 2 at the screen's resolution: temporal anti-aliasing with FSR 2's reconstruction, sharper than TAA. Replaces SSAA / MSAA.",
                FSR2_COSTS_4K[0],
                0.0,
            ),
            tc(
                "FSR 2 Quality",
                "AMD FSR 2: 67% of each side, rebuilt from several frames; close to SSAA 2x at a fraction of its cost.",
                FSR2_COSTS_4K[1],
                0.0,
            ),
            tc("FSR 2 Balanced", "AMD FSR 2: 59% of each side.", FSR2_COSTS_4K[2], 0.0),
            tc("FSR 2 Performance", "AMD FSR 2: 50% of each side.", FSR2_COSTS_4K[3], 0.0),
            tc(
                "FSR 2 Ultra Performance",
                "AMD FSR 2: 33% of each side; for 8K or weak GPUs, visibly softer.",
                FSR2_COSTS_4K[4],
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.upscale.min(qol::UPSCALE_MAX)),
        set: |s, i| s.enh.upscale = qol::UPSCALE_STEPS[i],
        offered: None,
        legacy: false,
    },
    // Customize Display's World Detail (g_WorldLodLevel 0x870CF4) with the
    // clone's scenery draw distance (enh.detail_distance) above it; the
    // top tier also widens the drawn sections (enh.keep_loaded: every
    // section is resident in the clone anyway, so the old Keep All Loaded
    // row was draw distance too, and is folded in here).
    GraphicsRow {
        id: RowId::DetailDistance,
        label: 0xBDE2_E94B, // World Detail
        group: GraphicsGroup::Performance,
        help: "Scenery detail and draw distance. Ultra and Extreme keep all of Bayview loaded (the old Keep All Loaded row is part of them) and stop pop-in: scenery and cars drawn until they are specks, with LOD Fade section swaps, lamp lights and traffic fade in.",
        tiers: &[
            t("Lowest", "Original value: low-detail scenery models."),
            t("Low", "Original value: low-detail scenery models."),
            t("High", "Original value, World Detail 5: full scenery models."),
            t("High (max)", "Original value, the card's cap: draws as High."),
            tc("Very High", "Small scenery drawn 1.5x as far (pop-in 19% -> 6% of frames).", 0.0, 0.0),
            tc(
                "Ultra",
                "Keep All Loaded: scenery and cars drawn down to a few pixels at any resolution, nearby sections drawn early; with LOD Fade the swaps, lamp lights and traffic fade in (no visible pops on the test drives).",
                0.0,
                0.1,
            ),
            tc(
                "Extreme",
                "Ultra, and the sections seen from up to 400 m ahead drawn far off with their lowest model; fog as the original.",
                FAR_DRAW_COST_4K,
                0.1,
            ),
        ],
        native: Some(6),
        get: |s| {
            custom_get(
                s,
                6,
                if s.enh.far_draw && s.enh.keep_loaded {
                    3
                } else if s.enh.keep_loaded {
                    2
                } else {
                    index_of(&qol::DETAIL_STEPS, s.enh.detail_distance)
                },
            )
        },
        set: |s, i| {
            let e = custom_part(s, 6, i).unwrap_or(0);
            s.enh.detail_distance = qol::DETAIL_STEPS[e.min(2)];
            s.enh.keep_loaded = e >= 2;
            s.enh.far_draw = e == 3;
        },
        offered: None,
        legacy: false,
    },
    // Removed (audit: no visible effect), readable while a saved file
    // has it on.
    GraphicsRow {
        id: RowId::ReversedZ,
        label: REVERSED_Z,
        group: GraphicsGroup::Performance,
        help: "Removed: no visible effect in any tested scene. Any change turns it off.",
        tiers: &[t("Off", "Off: as the original."), t("On", "Reversed depth range.")],
        native: None,
        get: |s| usize::from(s.enh.reversed_z),
        set: |s, i| s.enh.reversed_z = i != 0,
        offered: None,
        legacy: true,
    },
    // Display (docs/playing/hdr.md): HDR Output and its calibration.
    GraphicsRow {
        id: RowId::HdrOutput,
        label: HDR_OUTPUT,
        group: GraphicsGroup::Display,
        help: "HDR signal (HDR10 PQ or scRGB): the original's grade up to white, headroom for lights, neon and glow.",
        tiers: &[
            t("Off", "Off: SDR, as the original."),
            tc("On", "Needs HDR switched on in the system.", HDR_OUTPUT_COST_4K, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.hdr_output),
        set: |s, i| s.enh.hdr_output = i != 0,
        offered: Some(|_, i| i == 0 || qol::hdr_output_signal().is_some()),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::HdrPeak,
        label: HDR_PEAK,
        group: GraphicsGroup::Display,
        help: "The display's peak brightness: raise it until the small square in the pattern's top box just disappears.",
        tiers: &[
            t("Auto", "Auto: what the display reports, else 1000 nits."),
            t("400 nits", "HDR400-class monitors."),
            t("600 nits", "HDR600-class monitors."),
            t("800 nits", "Brighter monitors and laptops."),
            t("1000 nits", "HDR1000 monitors and most TVs."),
            t("1400 nits", "Bright mini-LED displays."),
            t("2000 nits", "Very bright TVs."),
            t("4000 nits", "Mastering displays."),
            t("10000 nits", "The PQ signal's maximum (no roll-off below it)."),
        ],
        native: None,
        get: |s| if s.enh.hdr_peak == 0 { 0 } else { 1 + index_of(&qol::HDR_PEAK_STEPS, s.enh.hdr_peak) },
        set: |s, i| s.enh.hdr_peak = if i == 0 { 0 } else { qol::HDR_PEAK_STEPS[i - 1] },
        offered: Some(|_, i| i == 0 || qol::hdr_output_signal().is_some()),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::HdrPaper,
        label: HDR_PAPER,
        group: GraphicsGroup::Display,
        help: "How bright the original's white (menus, HUD, midtones) is: match the left patch to a white window beside the game.",
        tiers: &[
            t("Auto", "Auto: the system's SDR white level where it reports one, else 203 nits (BT.2408)."),
            t("80 nits", "sRGB's reference white (dark room)."),
            t("100 nits", "Dim room."),
            t("120 nits", "Paper white: 120 nits."),
            t("160 nits", "Paper white: 160 nits."),
            t("203 nits", "BT.2408's reference white."),
            t("250 nits", "Paper white: 250 nits."),
            t("300 nits", "Bright room."),
            t("400 nits", "Very bright room."),
        ],
        native: None,
        get: |s| if s.enh.hdr_paper == 0 { 0 } else { 1 + index_of(&qol::HDR_PAPER_STEPS, s.enh.hdr_paper) },
        set: |s, i| s.enh.hdr_paper = if i == 0 { 0 } else { qol::HDR_PAPER_STEPS[i - 1] },
        offered: Some(|_, i| i == 0 || qol::hdr_output_signal().is_some()),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::HdrGamma,
        label: HDR_GAMMA,
        group: GraphicsGroup::Display,
        help: "How the original's SDR image is decoded: pick the one where the pattern's step 2 is barely visible and the striped patch matches the solid one.",
        tiers: &[
            t("Gamma 2.2", "Gamma 2.2: as most SDR monitors show the original."),
            t("sRGB", "sRGB piecewise: brighter shadows (Windows' SDR mapping)."),
            t("Gamma 2.4", "Gamma 2.4: deeper shadows (dark rooms)."),
        ],
        native: None,
        get: |s| usize::from(s.enh.hdr_gamma.min(2)),
        set: |s, i| s.enh.hdr_gamma = i as u8,
        offered: Some(|_, i| i == 0 || qol::hdr_output_signal().is_some()),
        legacy: false,
    },
];
