use super::*;

pub(super) const ROWS: &[GraphicsRow] = &[
    // Image.
    GraphicsRow {
        id: RowId::PostAa,
        label: POST_AA,
        group: GraphicsGroup::Image,
        help: "Edge smoothing.",
        tiers: &[
            t("Native", "Native: Customize Display's Full Screen Anti-Aliasing (8x MSAA at World Detail 5)."),
            tc("FXAA", "Fast post filter; also softens texture detail.", 0.11, 0.0),
            tc("SMAA 1x", "Morphological filter; sharper than FXAA.", 0.36, 0.0),
            tc("MSAA 2x", "2 samples a pixel plus alpha-to-coverage on foliage and fences.", 0.01, 0.0),
            tc("MSAA 4x", "4 samples a pixel plus alpha-to-coverage.", 0.01, 0.0),
            tc("MSAA 8x", "As native 8x plus alpha-to-coverage on foliage and fences.", 0.01, 0.0),
            tc("SSAA 2x", "Renders twice the pixels and scales down; replaces MSAA.", SSAA_COSTS_4K[0], 0.0),
            tc("SSAA 4x", "Renders four times the pixels.", SSAA_COSTS_4K[1], 0.0),
            tc("SSAA 8x", "Renders eight times the pixels.", SSAA_COSTS_4K[2], 0.0),
            tc("SSAA 9x", "Renders nine times the pixels.", SSAA_COSTS_4K[3], 0.0),
            tc("SSAA 16x", "Renders sixteen times the pixels.", SSAA_COSTS_4K[4], 0.0),
            // docs/temporal.md: exclusive with MSAA / SSAA / filters.
            tc(
                "TAA",
                "Temporal AA: smooth, stable edges in motion, but softer (add Sharpening); MSAA is the original's look. Integrated GPU: ~+1.5 ms.",
                TAA_COST_4K,
                0.0,
            ),
        ],
        native: None,
        get: |s| usize::from(s.enh.post_aa.min(qol::TAA)),
        set: |s, i| s.enh.post_aa = i as u8,
        offered: Some(|s, i| qol::aa_offered(i as u8, qol::output_size_or(s.resolution))),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::AaMsaa,
        label: AA_MSAA,
        group: GraphicsGroup::Image,
        help: "Multisample count under the Anti-Aliasing choice (for combinations).",
        tiers: &[
            t("Auto", "Auto: as the Anti-Aliasing row sets it."),
            t("Off", "No multisampling (saves time)."),
            tc("2x", "2 samples a pixel.", 0.01, 0.0),
            tc("4x", "4 samples a pixel.", 0.01, 0.0),
            tc("8x", "8 samples a pixel.", 0.01, 0.0),
            tc("16x", "16 samples a pixel (8x where the GPU has no 16x).", 0.01, 0.0),
        ],
        native: None,
        get: |s| index_of(&qol::MSAA_STEPS, s.enh.aa_msaa),
        set: |s, i| s.enh.aa_msaa = qol::MSAA_STEPS[i],
        offered: Some(|_, i| qol::msaa_offered(qol::MSAA_STEPS[i])),
        legacy: false,
    },
    GraphicsRow {
        id: RowId::AaFilter,
        label: AA_FILTER,
        group: GraphicsGroup::Image,
        help: "Post-process edge filter on top of the Anti-Aliasing choice.",
        tiers: &[
            t("Auto", "Auto: as the Anti-Aliasing row sets it."),
            t("Off", "No post filter."),
            tc("FXAA", "Fast post filter.", 0.12, 0.0),
            tc("SMAA", "Morphological filter.", 0.35, 0.0),
            tc("SMAA + FXAA", "Both filters, the softest edges.", 0.46, 0.0),
        ],
        native: None,
        get: |s| usize::from(s.enh.aa_filter.min(4)),
        set: |s, i| s.enh.aa_filter = i as u8,
        offered: None,
        legacy: false,
    },
    GraphicsRow {
        id: RowId::Sharpen,
        label: SHARPEN,
        group: GraphicsGroup::Image,
        help: "Contrast-adaptive sharpening of the final image.",
        tiers: &[
            t("Off", "Off: as the original."),
            tc("Low", "Light sharpening (25%).", 0.14, 0.0),
            tc("Medium", "Clear sharpening (75%).", 0.14, 0.0),
            tc("High", "Strong sharpening (100%).", 0.14, 0.0),
        ],
        native: None,
        get: |s| index_of(&qol::SHARPEN_STEPS, s.enh.sharpen),
        set: |s, i| s.enh.sharpen = qol::SHARPEN_STEPS[i],
        offered: None,
        legacy: false,
    },
    // Customize Display's Texture Filtering (g_TextureFiltering 0x870D18);
    // absorbs the removed clone row (enh.anisotropy: 2x–8x only lowered
    // native 16x).
    GraphicsRow {
        id: RowId::TextureFiltering,
        label: 0x61A5_0B8F, // Texture Filtering
        group: GraphicsGroup::Image,
        help: "Texture sharpness at a distance and at angles (the original's row).",
        tiers: &[
            t("Point", "Original value: point sampling."),
            t("Linear", "Original value: linear filtering."),
            t("16x Anisotropic", "Original value, World Detail 5: sharp textures at angles."),
        ],
        native: Some(20),
        get: |s| if s.enh.anisotropy != 0 { 2 } else { custom_get(s, 20, 0) },
        set: |s, i| {
            s.enh.anisotropy = 0;
            custom_part(s, 20, i);
        },
        offered: None,
        legacy: false,
    },
    // docs/texture-pack-generator.md: the pack "Generate Texture
    // Pack" makes from the player's own files. Greyed until one exists.
    GraphicsRow {
        id: RowId::TexturePack,
        label: TEXTURE_PACK,
        group: GraphicsGroup::Image,
        help: "The textures Generate Texture Pack upscales from your game files (applies at the next city or car load). TexMod packages from the launcher's Mods apply on or off.",
        tiers: &[
            t("Off", "Off: the original textures."),
            t(
                "On",
                "Roads, signs, buildings, decals, rims and traffic skins at up to 4x; buildings held to 2x on GPUs under 8 GB. About +0.6 GB of video memory.",
            ),
            t("Lean", "Buildings and car parts held to 2x (less video memory); roads, signs and decals keep 4x."),
        ],
        native: None,
        get: |s| usize::from(s.enh.texture_pack.min(2)),
        set: |s, i| s.enh.texture_pack = i as u8,
        offered: None,
        legacy: false,
    },
];
