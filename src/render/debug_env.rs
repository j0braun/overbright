//! Every environment variable the render tree reads, one function per
//! variable (one place to find them).
//!
//! Each function keeps the parse rule and the read frequency of the code it
//! replaced: it reads the environment on every call (several callers cache
//! the result in their own `OnceLock`, as before) and no truthiness rule was
//! unified, so `=0` still means different things per variable (see each
//! line). Unifying the rules, and caching here, are later, logic-changing
//! steps. `NFSU2_*` are Overbright's switches (measurement and tuning aids,
//! not a supported interface); the rest are the system's (`HOME`, ...).

use std::env::VarError;
use std::ffi::OsString;

/// `var_os(name)` is set at all (even empty, even `0`).
fn set(name: &str) -> bool {
    std::env::var_os(name).is_some()
}
/// `var(name)` is set and valid unicode, value `"1"`.
fn is_one(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}
/// `var_os(name)` is set and not exactly `0` (`=0` means off).
fn set_not_zero(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v != "0")
}
/// `var(name)` parsed as `T` (unset, non-unicode or unparsable: `None`).
fn parsed<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}
/// `var(name).ok()`.
fn text(name: &str) -> Option<String> {
    std::env::var(name).ok()
}
/// `var(name).unwrap_or_default()`.
fn text_or_empty(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}
/// `var_os(name)`.
fn os(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

// --- the game's own location -------------------------------------------

/// `NFSU2_ROOT`: the installed game's folder (`var_os`; the data tests skip without it).
pub fn nfsu2_root() -> Option<OsString> {
    os("NFSU2_ROOT")
}

// --- device and pipeline set-up (mod.rs) --------------------------------

/// `NFSU2_LAMP_SPECIALISE`: `=0` turns the lamp pipeline specialisation off (set to `"0"` exactly; unicode only).
pub fn nfsu2_lamp_specialise_off() -> bool {
    std::env::var("NFSU2_LAMP_SPECIALISE").is_ok_and(|v| v == "0")
}
/// `NFSU2_NO_PIPELINE_CACHE`: set and not `0` turns the pipeline cache off (`=0` leaves it on).
pub fn nfsu2_no_pipeline_cache() -> bool {
    set_not_zero("NFSU2_NO_PIPELINE_CACHE")
}
/// `NFSU2_PIPELINE_LOG`: set and not `0` prints where each pipeline (set) is built (measuring aid).
pub fn nfsu2_pipeline_log() -> bool {
    set_not_zero("NFSU2_PIPELINE_LOG")
}
/// `NFSU2_GPU_DROP`: comma list of device features to leave out (raw `var` result; `Err` = unset).
pub fn nfsu2_gpu_drop() -> Result<String, VarError> {
    std::env::var("NFSU2_GPU_DROP")
}
/// `NFSU2_NO_BOUND_ARRAYS`: set and not `0` turns the bound texture arrays off (`=0` leaves them on).
pub fn nfsu2_no_bound_arrays() -> bool {
    set_not_zero("NFSU2_NO_BOUND_ARRAYS")
}
/// `NFSU2_NO_TEXTURE_ARRAYS`: set at all (even `=0`) turns the texture arrays off.
pub fn nfsu2_no_texture_arrays() -> bool {
    set("NFSU2_NO_TEXTURE_ARRAYS")
}
/// `NFSU2_NO_MULTIVIEW`: set at all (even `=0`) turns multiview off.
pub fn nfsu2_no_multiview() -> bool {
    set("NFSU2_NO_MULTIVIEW")
}
/// `NFSU2_POP_STATS`: set at all prints the pop statistics.
pub fn nfsu2_pop_stats() -> bool {
    set("NFSU2_POP_STATS")
}
/// `NFS_ONLY_TEXTURE`: a texture name (unicode) to draw only; unset draws everything.
pub fn nfs_only_texture() -> Option<String> {
    text("NFS_ONLY_TEXTURE")
}
/// `NFSU2_DEPTH_PREPASS`: `u8` (0 off, 2 pre-pass); unset or unparsable: `None`.
pub fn nfsu2_depth_prepass() -> Option<u8> {
    parsed("NFSU2_DEPTH_PREPASS")
}
/// `NFSU2_GPU_BLOCK_MIB`: allocator block sizes, `"min, max"` or `perf` (raw text; unset: `None`).
pub fn nfsu2_gpu_block_mib() -> Option<String> {
    text("NFSU2_GPU_BLOCK_MIB")
}
/// `NFSU2_RT`: `0` never, `1` always trace-capable (raw text; unset: `None`).
pub fn nfsu2_rt() -> Option<String> {
    text("NFSU2_RT")
}
/// `NFSU2_RT_TRACE`: `fragment` keeps the ray-traced reflections' rays in
/// the trace's fragment pass (the form a device without the ray list
/// takes, `rt::list_path`); unset or anything else: the device's choice.
pub fn nfsu2_rt_trace_fragment() -> bool {
    text("NFSU2_RT_TRACE").is_some_and(|v| v == "fragment")
}
/// `NFSU2_RT_LIST_STATS`: `u64` n, every n-th ray-traced trace prints how
/// many of its pixels the ray list held (a measuring aid); unset,
/// unparsable or 0: `None`.
pub fn nfsu2_rt_list_stats() -> Option<u64> {
    parsed::<u64>("NFSU2_RT_LIST_STATS").filter(|&n| n > 0)
}
/// `NFSU2_BENCH_ENH`: the bench's `;`-separated enhancement rows (raw text).
pub fn nfsu2_bench_enh() -> Option<String> {
    text("NFSU2_BENCH_ENH")
}
/// `NFSU2_SHOT_ENH`: the shot's `;`-separated enhancement rows (raw text).
pub fn nfsu2_shot_enh() -> Option<String> {
    text("NFSU2_SHOT_ENH")
}
/// `NFSU2_RT_POOL`: `=0` turns the render-target pool off; anything else (and unset, and non-unicode) leaves it on.
pub fn nfsu2_rt_pool() -> bool {
    std::env::var("NFSU2_RT_POOL").map_or(true, |v| v != "0")
}
/// `NFSU2_POP_ORACLE`: set at all draws everything whatever its size.
pub fn nfsu2_pop_oracle() -> bool {
    set("NFSU2_POP_ORACLE")
}
/// `NFSU2_POP_VERBOSE`: set at all prints per-event pop lines.
pub fn nfsu2_pop_verbose() -> bool {
    set("NFSU2_POP_VERBOSE")
}
/// `NFSU2_FRAME_CAP`: frames per second cap as `f64`; unset or unparsable: `None` (the caller drops values <= 0).
pub fn nfsu2_frame_cap() -> Option<f64> {
    parsed("NFSU2_FRAME_CAP")
}
/// `NFSU2_QUAD_DUMP`: set at all dumps the UI quads.
pub fn nfsu2_quad_dump() -> bool {
    set("NFSU2_QUAD_DUMP")
}
/// `NFSU2_SURFACE_DEBUG`: set at all logs surface-kind debug lines.
pub fn nfsu2_surface_debug() -> bool {
    set("NFSU2_SURFACE_DEBUG")
}
/// `NFSU2_PROBE_LOG`: set at all logs the light probes.
pub fn nfsu2_probe_log() -> bool {
    set("NFSU2_PROBE_LOG")
}
/// `NFSU2_PROBE_BURST_SERIAL`: value `1` makes the light probes' arrival
/// burst wait for each probe before the next (a check and measuring aid).
pub fn nfsu2_probe_burst_serial() -> bool {
    is_one("NFSU2_PROBE_BURST_SERIAL")
}
/// `NFSU2_AUDIO_ONLY`: which audio-only car paint to draw (raw text).
pub fn nfsu2_audio_only() -> Option<String> {
    text("NFSU2_AUDIO_ONLY")
}

// --- upscaling, temporal, HDR -------------------------------------------

/// `NFSU2_TAA_DEBUG`: temporal debug view as `f32`; unset or unparsable: `None`.
pub fn nfsu2_taa_debug() -> Option<f32> {
    parsed("NFSU2_TAA_DEBUG")
}
/// `NFSU2_TAA_COMPUTE`: `0` keeps TAA's resolve in the fragment pass,
/// another number asks for the compute pass on any device (unset: on an
/// integrated GPU); the self-check still decides.
pub fn nfsu2_taa_compute() -> Option<u32> {
    parsed("NFSU2_TAA_COMPUTE")
}
/// `NFSU2_TAA_VEL`: velocity blend tuning, `f32` (unset or unparsable: `None`).
pub fn nfsu2_taa_vel() -> Option<f32> {
    parsed("NFSU2_TAA_VEL")
}
/// `NFSU2_TAA_VEL_MAX`: velocity blend maximum tuning, `f32`.
pub fn nfsu2_taa_vel_max() -> Option<f32> {
    parsed("NFSU2_TAA_VEL_MAX")
}
/// `NFSU2_FSR2_DEBUG`: FSR 2 debug view, `f32`.
pub fn nfsu2_fsr2_debug() -> Option<f32> {
    parsed("NFSU2_FSR2_DEBUG")
}
/// `NFSU2_FSR2_LUMA_BLOCK`: luma pyramid block size, `f32` (read as a number, then cast).
pub fn nfsu2_fsr2_luma_block() -> Option<f32> {
    parsed("NFSU2_FSR2_LUMA_BLOCK")
}
/// `NFSU2_FSR2_TC_GAIN`: transparency/composition gain, `f32`.
pub fn nfsu2_fsr2_tc_gain() -> Option<f32> {
    parsed("NFSU2_FSR2_TC_GAIN")
}
/// `NFSU2_FSR2_WHEEL`: wheel reactive value, `f32`.
pub fn nfsu2_fsr2_wheel() -> Option<f32> {
    parsed("NFSU2_FSR2_WHEEL")
}
/// `NFSU2_FSR2_LOCKS`: locks on/off value, `f32` (1 on).
pub fn nfsu2_fsr2_locks() -> Option<f32> {
    parsed("NFSU2_FSR2_LOCKS")
}
/// `NFSU2_FSR2_BOX`: wide-box scale maximum, `f32`.
pub fn nfsu2_fsr2_box() -> Option<f32> {
    parsed("NFSU2_FSR2_BOX")
}
/// `NFSU2_FSR2_AUTO`: auto-reactive strength, `f32`.
pub fn nfsu2_fsr2_auto() -> Option<f32> {
    parsed("NFSU2_FSR2_AUTO")
}
/// `NFSU2_FSR2_SHARPEN`: sharpness in stops, `f32` (>= 9 turns RCAS off).
pub fn nfsu2_fsr2_sharpen() -> Option<f32> {
    parsed("NFSU2_FSR2_SHARPEN")
}
/// `NFSU2_FSR2_STEP_REACTIVE`: stepped animation reactive value, `f64`.
pub fn nfsu2_fsr2_step_reactive() -> Option<f64> {
    parsed("NFSU2_FSR2_STEP_REACTIVE")
}
/// `NFSU2_FSR2_ANIM_TC`: animation composition value, `f64`.
pub fn nfsu2_fsr2_anim_tc() -> Option<f64> {
    parsed("NFSU2_FSR2_ANIM_TC")
}
/// `NFSU2_FSR2_ANIM_REACTIVE`: continuous animation reactive value, `f64`.
pub fn nfsu2_fsr2_anim_reactive() -> Option<f64> {
    parsed("NFSU2_FSR2_ANIM_REACTIVE")
}
/// `NFSU2_FSR2_STILL_REACTIVE`: still animation reactive value, `f64`.
pub fn nfsu2_fsr2_still_reactive() -> Option<f64> {
    parsed("NFSU2_FSR2_STILL_REACTIVE")
}
/// `NFSU2_FSR2_RIPPLE_TC`: ripple composition value, `f64`.
pub fn nfsu2_fsr2_ripple_tc() -> Option<f64> {
    parsed("NFSU2_FSR2_RIPPLE_TC")
}
/// `NFSU2_HDR_SIGNAL`: `scrgb` prefers scRGB (raw text, parsed by the caller).
pub fn nfsu2_hdr_signal() -> Option<String> {
    text("NFSU2_HDR_SIGNAL")
}
/// `NFSU2_DOF_ALWAYS`: `=1` keeps the depth of field always on.
pub fn nfsu2_dof_always() -> bool {
    is_one("NFSU2_DOF_ALWAYS")
}
/// `NFSU2_BLUR_DEBUG`: blur debug view, `f32`.
pub fn nfsu2_blur_debug() -> Option<f32> {
    parsed("NFSU2_BLUR_DEBUG")
}

// --- post effects (postfx.rs) -------------------------------------------

/// `NFSU2_POST_CHECK`: `=all` runs every variant's self-check (unicode only).
pub fn nfsu2_post_check_all() -> bool {
    std::env::var("NFSU2_POST_CHECK").is_ok_and(|v| v == "all")
}
/// `NFSU2_POST_CHECK`: set at all (any value) runs the self-check again instead of using the kept verdict.
pub fn nfsu2_post_check_set() -> bool {
    set("NFSU2_POST_CHECK")
}
/// `NFSU2_CHECK_STAGES`: self-check stage count as `u32`; the caller keeps 1..=4.
pub fn nfsu2_check_stages() -> Option<u32> {
    parsed("NFSU2_CHECK_STAGES")
}
/// `NFSU2_FUSED_POST`: `measured` or `even` forces a fused variant (raw text).
pub fn nfsu2_fused_post() -> Option<String> {
    text("NFSU2_FUSED_POST")
}
/// `NFSU2_WEATHER_HELPERS`: the render-pool jobs the weather's element-wise steps use besides the calling thread, `usize` (0: every step serial on the calling thread; unset: the rule in `weather::par`).
pub fn nfsu2_weather_helpers() -> Option<usize> {
    parsed("NFSU2_WEATHER_HELPERS")
}
/// `NFSU2_NO_FUSED_POST`: set at all (even `=0`) keeps the separate passes.
pub fn nfsu2_no_fused_post() -> bool {
    set("NFSU2_NO_FUSED_POST")
}
/// `NFSU2_SSAA_SCENE_SIZE`: under SSAA, the passes named (comma-separated: `ssao`, `post`; `all`) run at
/// the scene's size as before they moved to the screen's (docs/ssaa-screen-passes.md), for comparisons. Read once.
pub fn ssaa_scene_size(pass: &str) -> bool {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| text_or_empty("NFSU2_SSAA_SCENE_SIZE").split(',').map(|s| s.trim().to_string()).collect())
        .iter()
        .any(|n| n == pass || n == "all")
}
/// `NFSU2_MODERN_STRENGTH`: Modern Lighting strength in percent, `f32` (the caller clamps).
pub fn nfsu2_modern_strength() -> Option<f32> {
    parsed("NFSU2_MODERN_STRENGTH")
}
/// `NFSU2_MODERN_LIGHTING`: the hidden district switch (raw text, parsed by the caller).
pub fn nfsu2_modern_lighting() -> Option<String> {
    text("NFSU2_MODERN_LIGHTING")
}
/// `NFSU2_MODERN_LIGHTING_BOX`: the hidden switch's box (raw text).
pub fn nfsu2_modern_lighting_box() -> Option<String> {
    text("NFSU2_MODERN_LIGHTING_BOX")
}
/// `NFSU2_MODERN_LIGHTING_DEBUG`: debug view `1`/`2` (raw `var` result, matched by the caller).
pub fn nfsu2_modern_lighting_debug() -> Result<String, VarError> {
    std::env::var("NFSU2_MODERN_LIGHTING_DEBUG")
}
/// `NFSU2_CARLIGHTS`: `k=v,...` overrides of the car-light look (raw `var` result).
pub fn nfsu2_carlights() -> Result<String, VarError> {
    std::env::var("NFSU2_CARLIGHTS")
}

// --- volumes, weather, water, particles ---------------------------------

/// `NFSU2_SPRAY_LOOK`: `k=v,...` spray look overrides (unset: empty).
pub fn nfsu2_spray_look() -> String {
    text_or_empty("NFSU2_SPRAY_LOOK")
}
/// `NFSU2_SPRAY_LOOK`: set at all (even empty).
pub fn nfsu2_spray_look_set() -> bool {
    set("NFSU2_SPRAY_LOOK")
}
/// `NFSU2_SPRAY_STEP`: spray step scale, `f32`.
pub fn nfsu2_spray_step() -> Option<f32> {
    parsed("NFSU2_SPRAY_STEP")
}
/// `NFSU2_SPRAY_JITTER`: spray jitter share, `f32`.
pub fn nfsu2_spray_jitter() -> Option<f32> {
    parsed("NFSU2_SPRAY_JITTER")
}
/// `NFSU2_NIGHT_SKY_RAIN`: night-sky rain override, `f32` after trimming.
pub fn nfsu2_night_sky_rain() -> Option<f32> {
    trimmed_f32("NFSU2_NIGHT_SKY_RAIN")
}
/// `NFSU2_NIGHT_SKY_TIME`: night-sky time offset, `f32` after trimming.
pub fn nfsu2_night_sky_time() -> Option<f32> {
    trimmed_f32("NFSU2_NIGHT_SKY_TIME")
}
/// `NFSU2_NIGHT_SKY_COVER`: night-sky cloud cover, `f32` after trimming.
pub fn nfsu2_night_sky_cover() -> Option<f32> {
    trimmed_f32("NFSU2_NIGHT_SKY_COVER")
}
/// `NFSU2_NIGHT_SKY_DEBUG`: night-sky debug, `f32` after trimming (> 0.5 is on).
pub fn nfsu2_night_sky_debug() -> Option<f32> {
    trimmed_f32("NFSU2_NIGHT_SKY_DEBUG")
}
/// `NFSU2_SKY_GLOW`: sky glow, `f32` after trimming.
pub fn nfsu2_sky_glow() -> Option<f32> {
    trimmed_f32("NFSU2_SKY_GLOW")
}
/// `var(name)` trimmed, parsed as `f32`.
fn trimmed_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok())
}
/// `NFSU2_AMBIENT_STATS`: set (unicode) at all prints the ambient-detail statistics.
pub fn nfsu2_ambient_stats() -> bool {
    std::env::var("NFSU2_AMBIENT_STATS").is_ok()
}
/// `NFSU2_AMBIENT_STATS`: `=2` prints the per-prop statistics.
pub fn nfsu2_ambient_stats_two() -> bool {
    std::env::var("NFSU2_AMBIENT_STATS").is_ok_and(|v| v == "2")
}
/// `NFSU2_AMBIENT_DEBUG`: `=1` turns the ambient-detail debug on.
pub fn nfsu2_ambient_debug() -> bool {
    is_one("NFSU2_AMBIENT_DEBUG")
}
/// `NFSU2_WATER_NG_LOOK`: `k=v,...` water look overrides (raw `var` result).
pub fn nfsu2_water_ng_look() -> Result<String, VarError> {
    std::env::var("NFSU2_WATER_NG_LOOK")
}
/// `NFSU2_WATER_NG_DEBUG`: `=1` turns the water debug on.
pub fn nfsu2_water_ng_debug() -> bool {
    is_one("NFSU2_WATER_NG_DEBUG")
}
/// `NFSU2_WATER_NG_VIEW`: water debug view, `f32`.
pub fn nfsu2_water_ng_view() -> Option<f32> {
    parsed("NFSU2_WATER_NG_VIEW")
}
/// `NFSU2_WATER_PLAIN`: `=1` draws the plain water.
pub fn nfsu2_water_plain() -> bool {
    is_one("NFSU2_WATER_PLAIN")
}
/// `NFSU2_WATER_SRC_DIR`: folder to read the water shader sources from (`var_os`; set at all counts).
pub fn nfsu2_water_src_dir() -> Option<OsString> {
    os("NFSU2_WATER_SRC_DIR")
}
/// `NFSU2_SMOKE3_LOOK`: `k=v,...` volume smoke look overrides (unset: empty).
pub fn nfsu2_smoke3_look() -> String {
    text_or_empty("NFSU2_SMOKE3_LOOK")
}
/// `NFSU2_SMOKE3_LOOK`: set at all (even empty).
pub fn nfsu2_smoke3_look_set() -> bool {
    set("NFSU2_SMOKE3_LOOK")
}
/// `NFSU2_ATMO_LOOK`: atmosphere look overrides (unset: empty).
pub fn nfsu2_atmo_look() -> String {
    text_or_empty("NFSU2_ATMO_LOOK")
}
/// `NFSU2_ATMO_DIMS`: `x,y,z` froxel grid size (unset: empty; the caller validates 8..=512).
pub fn nfsu2_atmo_dims() -> String {
    text_or_empty("NFSU2_ATMO_DIMS")
}
/// `NFSU2_SF_PASSES`: sparks/fire pass mask, `u32`.
pub fn nfsu2_sf_passes() -> Option<u32> {
    parsed("NFSU2_SF_PASSES")
}
/// `NFSU2_VFX_DIV`: volume march divisor, `u32` (the caller clamps 1..=4).
pub fn nfsu2_vfx_div() -> Option<u32> {
    parsed("NFSU2_VFX_DIV")
}
/// `NFSU2_VFX_DIV`: set at all (even unparsable).
pub fn nfsu2_vfx_div_set() -> bool {
    set("NFSU2_VFX_DIV")
}
/// `NFSU2_VFX_DEBUG`: volume debug view, `f32`.
pub fn nfsu2_vfx_debug() -> Option<f32> {
    parsed("NFSU2_VFX_DEBUG")
}
/// `NFSU2_RAIN_HAZE`: rain haze look overrides (unset: empty).
pub fn nfsu2_rain_haze() -> String {
    text_or_empty("NFSU2_RAIN_HAZE")
}
/// `NFSU2_RAIN_NG_SKIP`: comma list of rain draws to leave out (unset: empty).
pub fn nfsu2_rain_ng_skip() -> String {
    text_or_empty("NFSU2_RAIN_NG_SKIP")
}
/// `NFSU2_SMOKE2_DIV`: cinematic smoke divisor, `u32` (the caller clamps 1..=4).
pub fn nfsu2_smoke2_div() -> Option<u32> {
    parsed("NFSU2_SMOKE2_DIV")
}
/// `NFSU2_SMOKE2_DIV`: set at all (even unparsable).
pub fn nfsu2_smoke2_div_set() -> bool {
    set("NFSU2_SMOKE2_DIV")
}
/// `NFSU2_SMOKE2_DEBUG`: cinematic smoke debug view, `f32`.
pub fn nfsu2_smoke2_debug() -> Option<f32> {
    parsed("NFSU2_SMOKE2_DEBUG")
}
/// `NFSU2_SMOKE2_G10`: `=1` turns the billboard Cinematic smoke on.
pub fn nfsu2_smoke2_g10() -> bool {
    is_one("NFSU2_SMOKE2_G10")
}

// --- texture packs ------------------------------------------------------

/// `NFSU2_WORLD_TEXTURE_PACK`: PATH-style list of world pack folders; empty counts as unset.
pub fn nfsu2_world_texture_pack() -> Option<OsString> {
    os("NFSU2_WORLD_TEXTURE_PACK").filter(|p| !p.is_empty())
}
/// `NFSU2_CAR_TEXTURE_PACK`: PATH-style list of car pack folders; empty counts as unset.
pub fn nfsu2_car_texture_pack() -> Option<OsString> {
    os("NFSU2_CAR_TEXTURE_PACK").filter(|p| !p.is_empty())
}

// --- test harness variables (child processes, data fixtures) -------------

/// `NFSU2_TP1_TEST_PACKS`: the pack-list test's value; empty counts as unset.
#[cfg(test)]
pub fn nfsu2_tp1_test_packs() -> Option<OsString> {
    os("NFSU2_TP1_TEST_PACKS").filter(|p| !p.is_empty())
}
/// `NFSU2_CAPTURE_FIXTURE_DIR`: where the reflection tests write capture fixtures (`var_os`).
#[cfg(test)]
pub fn nfsu2_capture_fixture_dir() -> Option<OsString> {
    os("NFSU2_CAPTURE_FIXTURE_DIR")
}
/// `NFSU2_PACK_TEST_GPU`: set at all lets the pack tests use the GPU.
#[cfg(test)]
pub fn nfsu2_pack_test_gpu() -> bool {
    set("NFSU2_PACK_TEST_GPU")
}
/// `NFSU2_PACK_WATCH_TIME`: where the pack-watch test writes its timing (`var_os`).
#[cfg(test)]
pub fn nfsu2_pack_watch_time() -> Option<OsString> {
    os("NFSU2_PACK_WATCH_TIME")
}
/// A test's child-process marker, `var_os(name)` (the name is the test's own constant).
#[cfg(test)]
pub fn test_child(name: &str) -> Option<OsString> {
    os(name)
}
/// The scoped-pack test's child mode, `var(name)` (the name is the test's own constant).
#[cfg(test)]
pub fn test_child_mode(name: &str) -> Result<String, VarError> {
    std::env::var(name)
}
/// `NFSU2_WORLD_TEXTURE_PACK`: the pack-reload tests' raw `var_os` (no empty filter).
#[cfg(test)]
pub fn nfsu2_world_texture_pack_raw() -> Option<OsString> {
    os("NFSU2_WORLD_TEXTURE_PACK")
}
