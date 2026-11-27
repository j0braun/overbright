//! FSR 2's quality modes and the render size they give: the settings store's Upscaling row reads them
//! (the pass itself is `render::fsr2`).

/// FSR 2's quality modes: the Upscaling row's values 7..=11 and their
/// upscale ratio per side (ffxFsr2GetUpscaleRatioFromQualityMode; Native
/// AA is FSR 2 at 1.0).
pub const MODES: [(u8, &str, f64); 5] = [
    (7, "Native AA", 1.0),
    (8, "Quality", 1.5),
    (9, "Balanced", 1.7),
    (10, "Performance", 2.0),
    (11, "Ultra Performance", 3.0),
];

/// The upscale ratio of Upscaling value `v` when it is an FSR 2 mode.
pub fn ratio(v: u8) -> Option<f64> {
    MODES.iter().find(|m| m.0 == v).map(|m| m.2)
}

/// The render size for `display` at `ratio` (ffxFsr2GetRenderResolutionFromQualityMode:
/// each side divided and truncated, at least 1).
pub fn render_size(display: (u32, u32), ratio: f64) -> (u32, u32) {
    (((f64::from(display.0) / ratio) as u32).max(1), ((f64::from(display.1) / ratio) as u32).max(1))
}
