use super::*;

/// Ray Traced's 4K cost on the reference desktop GPU: SSR Ultra + the step up to
/// Ray Traced under max (rain 4K +2.65; docs/notes/performance/budget.md).
pub(super) const RT_COST_4K: f32 = 7.87;

/// The temporal tier's measured costs (docs/notes/graphics/temporal.md "Cost"; `--bench explore 600` at
/// night, GPU busy ms at 4K on the reference desktop GPU over Original, whose 8x MSAA
/// TAA and Upscaling replace: TAA 1.58 vs 2.38; over no AA it is +0.50).
pub(super) const TAA_COST_4K: f32 = -0.8;
/// The budget table's fixes (docs/notes/performance/budget.md, the maintainer's private capture):
/// the hints of the rows the budget table measures are its 4K numbers — the
/// tier's cost in the player's play setup with every newest row on ("everything
/// max", leave-one-out, the row's heavy scene noted in the TSV), not alone
/// over Original as before (SSAA 2x was 0.01 there, 10.15 ms under max).
/// SSAA: 2x and 4x measured (racing, over native 8x MSAA), 8x, 9x and 16x
/// from 4x's ~9.5 ms per extra screen of pixels (**derived**, not measured).
pub(super) const SSAA_COSTS_4K: [f32; 5] = [10.15, 28.39, 66.3, 75.8, 142.2];
/// SSR High and Ultra (rain).
pub(super) const SSR_COSTS_4K: [f32; 2] = [3.27, 5.22];
pub(super) const UPSCALE_COSTS_4K: [f32; 3] = [-0.92, -0.9, -0.96];
/// The FSR 1 tiers (docs/playing/modern-extras.md "FSR"): GPU ms against the
/// Original at 4K (native 8x MSAA kept at the lower size; measured).
pub(super) const FSR_COSTS_4K: [f32; 3] = [-0.83, -0.94, -1.07];
/// The FSR 2 modes (docs/notes/graphics/fsr2.md "Cost"): GPU ms against the Original
/// at 4K (`--bench explore 600` at night, reference desktop GPU, measured).
pub(super) const FSR2_COSTS_4K: [f32; 5] = [1.6, 0.5, 0.3, 0.1, -0.1];
/// Next-Gen atmosphere's measured costs (docs/notes/graphics/atmosphere.md "Cost": GPU busy ms at 4K on the reference desktop GPU, over fog Off / Ultra).
pub(super) const VOLUMETRIC_NEXT_COST_4K: f32 = 0.38;
pub(super) const SKY_NEXT_COST_4K: f32 = 0.78;
/// Volumetric Fog Low / High: tiers of the froxel medium (docs/notes/graphics/atmosphere.md "The tiers": GPU ms over
/// Original at 4K, `--bench drive` in rain, reference desktop GPU, measured).
pub(super) const VOLUMETRIC_LOW_COST_4K: f32 = 0.25;
pub(super) const VOLUMETRIC_HIGH_COST_4K: f32 = 0.26;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// GPU particles' measured costs (docs/notes/graphics/smoke.md "Cost"; burnout at the
/// race start, GPU busy ms over Original at 4K on the reference desktop GPU).
pub(super) const PARTICLES_HIGH_COST_4K: f32 = 2.37;
pub(super) const PARTICLES_ULTRA_COST_4K: f32 = 3.68;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// The volumetric Cinematic smoke (docs/notes/graphics/smoke.md "Cost": GPU
/// busy ms over Original at 4K, light drift smoke, same bench as High / Ultra).
pub(super) const PARTICLES_CINEMATIC_COST_4K: f32 = 1.35;
/// Sparks & Fire Next-Gen at 4K (docs/notes/graphics/sparks-fire.md, "Cost"): a scrape or a shot.
pub(super) const SPARKS_FIRE_COST_4K: f32 = 0.2;
/// Ambient Detail's measured costs (docs/notes/graphics/ambient-detail.md "Cost": GPU busy ms over
/// Off at 4K on the reference desktop GPU, free roam at night).
pub(super) const AMBIENT_COST_4K: f32 = 0.03;
pub(super) const AMBIENT_NEXTGEN_COST_4K: f32 = 0.37;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Animated Water's measured cost (docs/notes/graphics/water.md "Cost": GPU busy ms over Off at 4K
/// on the reference desktop GPU with the Beacon Hill fountain steps filling the view,
/// the worst case; nothing measurable where no water is in view).
pub(super) const WATER_COST_4K: f32 = -0.67;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Realistic water's measured cost (docs/notes/graphics/water.md "Realistic", the same view and
/// machine, over Off).
pub(super) const WATER_REALISTIC_COST_4K: f32 = 0.65;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Next-Gen water's measured cost (docs/notes/graphics/water.md "Next-Gen", the same view and
/// machine, over Off).
pub(super) const WATER_NEXTGEN_COST_4K: f32 = 2.4;
/// Lighting II's measured costs (docs/notes/graphics/lighting2.md "Cost": `--bench racing 600`,
/// GPU ms of the pass they add to over Off at 4K on the reference desktop GPU, with
/// native 8x MSAA; the probes' bake as main-thread CPU, free roam).
pub(super) const BEAMS_COST_4K: f32 = 0.0;
/// Next-Gen car lights (docs/notes/graphics/car-lights.md "Cost": +0.85..0.98 ms over
/// On at 4K Extreme with traffic / a race field, plus On's own).
pub(super) const CAR_LIGHTS_COST_4K: f32 = 0.43;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Modern Lighting's measured 4K GPU cost over Original on the reference
/// desktop GPU (racing, docs/notes/graphics/modern-lighting.md "Cost"): Low, Modern, + RT.
pub(super) const MODERN_LOW_COST_4K: f32 = 1.06;
pub(super) const MODERN_COST_4K: f32 = 2.06;
pub(super) const MODERN_RT_COST_4K: f32 = 8.99;
pub(super) const PROBES_COST_4K: f32 = 0.2;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Surface Detail (docs/notes/graphics/surface-maps.md): 4K GPU ms over Original,
/// `--bench explore` at night, reference desktop GPU.
pub(super) const SURFACE_MAT_COST_4K: f32 = 0.88;
pub(super) const SURFACE_NRM_COST_4K: f32 = 1.47;
pub(super) const PROBES_CPU_MS: f32 = 0.1;
pub(super) const GLASS_COST_4K: f32 = 0.35;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// The weather effects' measured costs (docs/notes/graphics/weather.md "Cost": `--bench rain 600`,
/// GPU busy ms over Original at 4K on the reference desktop GPU, heavy rain).
pub(super) const WET_CAR_COST_4K: f32 = 0.0;
pub(super) const RAIN_DETAIL_COST_4K: f32 = 0.11;
pub(super) const RAIN_DETAIL_HIGH_COST_4K: f32 = 0.12;
pub(super) const SPRAY_COST_4K: f32 = 0.01;
pub(super) const LENS_DROPS_COST_4K: f32 = 0.0;
/// Next-Gen rain (docs/notes/graphics/rain-nextgen.md "Cost"): GPU ms over Original
/// at 4K, `--bench drive 600` at clock 162 (heavy rain at night).
pub(super) const RAIN_DETAIL_NEXT_GEN_COST_4K: f32 = 0.45;
/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Wet Road Interaction's measured costs (docs/notes/graphics/wet-road-car.md "Cost"): GPU ms over
/// Original at 4K on the reference desktop GPU, heavy rain at night, four cars.
/// volumetric spray: the row Off → On / High at the play setup
/// without SSAA, `--bench drive 600` at clock 162, 4K, mean of 2 runs.
pub(super) const WET_INTERACTION_COST_4K: f32 = 0.97;
pub(super) const WET_INTERACTION_HIGH_COST_4K: f32 = 1.09;
/// Assets and distance (docs/notes/graphics/assets-distance.md, the maintainer's private captures),
/// measured GPU ms at 4K (racing, 5 + 5 runs): Smooth Car Shapes over
/// Original (−0.03 ms, not significant) and World Detail Extreme over
/// Ultra's world detail (+0.10 ms, p 0.095; +0.15 ms at 1080p Ultra).
pub(super) const SMOOTH_CARS_COST_4K: f32 = 0.15;
pub(super) const FAR_DRAW_COST_4K: f32 = 0.0;

/// HDR Output's cost over Original at 4K on the reference desktop GPU (docs/playing/hdr.md
/// "Cost": the high precision chain, the grade LUT, the two-target last
/// pass and the display transform; measured headless).
pub(super) const HDR_OUTPUT_COST_4K: f32 = 2.4;

/// The values are now the budget table's (`SSAA_COSTS_4K` above);
/// before:
/// Motion blur's costs over Original at 4K on the reference desktop GPU (docs/notes/graphics/motionblur-dof.md
/// "Cost": `--bench racing 600`, GPU busy median, 8x MSAA).
pub(super) const MOTION_CAMERA_COST_4K: f32 = 1.61;
pub(super) const MOTION_OBJECT_COST_4K: f32 = 1.93;
/// Depth of Field keeps its own: it draws only in scenes, replays and photo
/// mode (the budget's racing leave-one-out, 0.02 ms, is not its scene).
pub(super) const DOF_COST_4K: f32 = 1.3;

pub(super) const OFF_ON: [Tier; 2] = [t("Off", "Off: as the original."), t("On", "")];
