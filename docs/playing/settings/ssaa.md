# Spatial anti-aliasing

Open **Options → Display → Enhancements → Graphics → Image** (docs/notes/graphics/graphics-settings-audit.md; Render Scale is under Performance and offers 50–100 % only, since above 100 % it gave the same image as SSAA). Three independent rows configure AA:

- **Anti-Aliasing:** Native, FXAA, SMAA 1x, MSAA 2x/4x/8x, SSAA 2x/4x/8x/9x/16x.
- **AA: MSAA Override:** Preset, Off, or supported 2x/4x/8x/16x. Preset follows the first row; explicit values replace its MSAA choice.
- **AA: Post Filter:** Preset, Off, FXAA, SMAA, or SMAA + FXAA. Preset follows the first row. These run after downsampling and before optional CAS sharpening.

For example, SSAA 4x + MSAA Override 8x + Post Filter SMAA combines all three. SMAA + FXAA is available for experimentation but may soften detail. These are spatial techniques; there is no temporal history, motion-vector AA, FSR, DLAA or frame generation in this change.

| Output | SSAA 2x | SSAA 4x | SSAA 8x | SSAA 9x | SSAA 16x |
|---|---|---|---|---|---|
| 1920×1080 | 2715×1527 | 3840×2160 | 5431×3055 | 5760×3240 | 7680×4320 |
| 3840×2160 | 5431×3055 | 7680×4320 | 10861×6109 | 11520×6480 | 15360×8640 |

These are requested dimensions before hardware and allocation limits. SSAA multipliers count fully shaded scene pixels. MSAA adds coverage/depth samples; it is not equivalent to that many additional fully shaded pixels. The source-pixel footprint is area-averaged, including fractional edge overlaps. UI and HUD draw at output resolution.

## Original AA setting

When a preset or explicit MSAA override replaces the original **Full Screen Anti-Aliasing**, that original row is greyed, shows **Disabled - Enhancements**, and ignores adjustments. Its saved value is preserved. Native/FXAA/SMAA presets with MSAA Override set to Preset leave the original MSAA setting active; pure post filtering does not disable it.

SSAA disables original MSAA by default, but explicit MSAA Override enables a hybrid. Selecting Native in the first row does not clear independently selected overrides. Set the override rows to Preset, or use Restore Native, to restore original behavior fully. No simulation timing or FPS limit changes.

## Limits and persistence

Discrete GPUs allow at most 16 × 3840 × 2160 scene pixels and 64 × 3840 × 2160 multisample coverage samples, also bounded by the GPU's maximum texture dimension. Integrated/software budgets remain conservative. This is a fixed allocation guard, not a measurement of free VRAM: other applications and high-resolution reflections also use memory.

For instance, 4K SSAA 16x + MSAA 8x exceeds the coverage budget and is reduced to approximately SSAA 8x. The preset reads **limited** and the disabled Render Scale row reports the effective dimension percentage. Unsupported MSAA choices are skipped in the override row; a saved unsupported request is marked limited and falls back to the highest supported count below it. On the tested reference desktop GPU, 2x/4x/8x are supported; 16x falls back to 8x.

SSAA controls Render Scale without overwriting its saved value. Existing `enh.post_aa` IDs 0–7 retain their meaning; 8/9/10 select SSAA 8x/9x/16x. `enh.aa_msaa` stores 0=Preset, 1=Off, or 2/4/8/16. `enh.aa_filter` stores 0=Preset, 1=Off, 2=FXAA, 3=SMAA, 4=both. Native defaults omit these lines.

## How it works

How it works, what it costs, how it was checked and what is open: [Spatial anti-aliasing: how it works](../../notes/graphics/ssaa.md).
