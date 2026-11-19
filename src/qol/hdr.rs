use super::*;

/// HDR Output's Peak Brightness tiers (nits; 0 is Auto).
pub const HDR_PEAK_STEPS: [u16; 8] = [400, 600, 800, 1000, 1400, 2000, 4000, 10000];
/// HDR Output's Paper White tiers (nits; 0 is Auto).
pub const HDR_PAPER_STEPS: [u16; 8] = [80, 100, 120, 160, 203, 250, 300, 400];

/// The HDR signal the window's surface offers (set at window creation;
/// None headless and on SDR-only surfaces).
pub fn set_hdr_output_support(signal: Option<crate::render::hdr_output::Signal>) {
    use crate::render::hdr_output::Signal;
    let signal = match signal {
        None => 0,
        Some(Signal::Pq) => 1,
        Some(Signal::ScRgb) => 2,
    };
    change(|s| s.hdr_signal = signal);
}

pub fn hdr_output_signal() -> Option<crate::render::hdr_output::Signal> {
    use crate::render::hdr_output::Signal;
    match read(|s| s.hdr_signal) {
        1 => Some(Signal::Pq),
        2 => Some(Signal::ScRgb),
        _ => None,
    }
}

/// HDR Output in force: asked for, offered by the surface, and the float
/// chain supported by the device.
pub fn hdr_output_active() -> bool {
    read(|s| s.values.hdr_output && s.hdr_signal != 0 && s.hdr_supported)
}

/// The display's reported luminance (nits; None unknown).
pub fn set_hdr_display_info(peak: Option<f32>, paper: Option<f32>) {
    let peak = peak.map_or(0, |v| v.clamp(0.0, 10000.0) as u16);
    let paper = paper.map_or(0, |v| v.clamp(0.0, 1000.0) as u16);
    change(|s| {
        s.hdr_display_peak = peak;
        s.hdr_display_paper = paper;
    });
}

/// The calibration pattern's rectangle (fractions of the output: left,
/// top, right, bottom), None for no pattern.
pub fn set_hdr_pattern(rect: Option<[f32; 4]>) {
    change(|s| s.hdr_pattern = rect);
}

/// The display transform's calibration in force (Auto: the display's
/// values, else the defaults).
pub fn hdr_calibration() -> crate::render::hdr_output::Calibration {
    use crate::render::hdr_output::{Calibration, DEFAULT_PAPER, DEFAULT_PEAK, Transfer};
    let pick = |set: u16, reported: u16, default: f32| {
        if set != 0 {
            f32::from(set)
        } else if reported != 0 {
            f32::from(reported)
        } else {
            default
        }
    };
    let s = read(|s| *s);
    let paper = pick(s.values.hdr_paper, s.hdr_display_paper, DEFAULT_PAPER);
    let peak = pick(s.values.hdr_peak, s.hdr_display_peak, DEFAULT_PEAK).max(paper);
    Calibration { peak, paper, transfer: Transfer::from_index(s.values.hdr_gamma), pattern: s.hdr_pattern }
}

/// Whether the device can run High Precision Post (set by `render::Gpu::new`).
pub fn set_hdr_support(ok: bool) {
    change(|s| s.hdr_supported = ok);
}

/// High Precision Post in force: asked for and supported (else the native
/// 8-bit path runs).
pub fn hdr_active() -> bool {
    hdr_post() != 0 && read(|s| s.hdr_supported)
}
