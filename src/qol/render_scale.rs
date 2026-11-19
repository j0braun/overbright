use super::*;

/// The most scene pixels Render Scale may make (twice 3840 × 2160; half
/// that on an integrated GPU), and the most pixel samples with MSAA (8 ×
/// 3840 × 2160; a quarter on an integrated GPU): above them the scale is
/// held down, never below 100 %.
const MAX_SCENE_PIXELS: f64 = 2.0 * 3840.0 * 2160.0;
const MAX_SCENE_SAMPLES: f64 = 8.0 * 3840.0 * 2160.0;
/// Explicit SSAA permits up to 16x at 4K on capable discrete GPUs.
/// Render Scale and integrated/software adapters retain their old budgets.
const MAX_SSAA_PIXELS: f64 = 16.0 * 3840.0 * 2160.0;

/// The scale Render Scale `percent` gets for a screen `out` big at
/// `samples` (the caps: the adapter's largest texture, [`MAX_SCENE_PIXELS`],
/// [`MAX_SCENE_SAMPLES`]), as a factor.
pub fn effective_scale(percent: u8, out: (u32, u32), samples: u32) -> f64 {
    let (max_dim, small, software) = gpu_caps();
    scale_for(percent, out, samples, max_dim, small, software)
}

/// The adapter's caps in force: its largest texture edge, small, software.
fn gpu_caps() -> (u32, bool, bool) {
    read(|s| (s.max_dim, s.small_gpu, s.software_gpu))
}

/// [`effective_scale`] for an adapter with these caps.
pub(super) fn scale_for(percent: u8, out: (u32, u32), samples: u32, max_dim: u32, small: bool, software: bool) -> f64 {
    bounded_scale(f64::from(percent) / 100.0, out, samples, max_dim, small, software, false)
}

fn bounded_scale(
    want: f64,
    out: (u32, u32),
    samples: u32,
    max_dim: u32,
    small: bool,
    software: bool,
    ssaa: bool,
) -> f64 {
    if want <= 1.0 || software {
        return want.min(1.0);
    }
    let max_dim = f64::from(max_dim);
    let (w, h) = (f64::from(out.0.max(1)), f64::from(out.1.max(1)));
    let pixels = w * h;
    let max_pixels = if small {
        MAX_SCENE_PIXELS / 2.0
    } else if ssaa {
        MAX_SSAA_PIXELS
    } else {
        MAX_SCENE_PIXELS
    };
    let max_samples = if small {
        MAX_SCENE_SAMPLES / 4.0
    } else if ssaa {
        64.0 * 3840.0 * 2160.0
    } else {
        MAX_SCENE_SAMPLES
    };
    let cap = (max_dim / w)
        .min(max_dim / h)
        .min((max_pixels / pixels).sqrt())
        .min((max_samples / (pixels * f64::from(samples.max(1)))).sqrt());
    want.min(cap).max(1.0)
}

/// Whether Render Scale `percent` is offered at the resolution `out`
/// (supersampling the adapter or the caps cannot give is not).
pub fn scale_offered(percent: u8, out: (u32, u32)) -> bool {
    percent <= 100 || effective_scale(percent, out, 1) >= f64::from(percent) / 100.0 - 1e-6
}

/// SSAA pixel-count presets, expressed as a scale on each dimension.
pub fn ssaa_scale(aa: u8) -> Option<f64> {
    match aa {
        6 => Some(std::f64::consts::SQRT_2),
        7 => Some(2.0),
        8 => Some(8.0_f64.sqrt()),
        9 => Some(3.0),
        10 => Some(4.0),
        _ => None,
    }
}

/// Effective scene scale. SSAA controls the resolution independently of
/// the saved Render Scale; switching AA back restores that scale.
pub fn scene_scale(percent: u8, aa: u8, out: (u32, u32), samples: u32) -> f64 {
    let want = ssaa_scale(aa).unwrap_or(f64::from(percent) / 100.0);
    let (max_dim, small, software) = gpu_caps();
    bounded_scale(want, out, samples, max_dim, small, software, ssaa_scale(aa).is_some())
}

/// Skip SSAA presets which cannot fit the current output/adapter. A saved
/// preset can still become limited after resizing; scene_scale caps it.
pub fn aa_offered(aa: u8, out: (u32, u32)) -> bool {
    msaa_for(aa).is_none_or(|n| supported_msaa(n) == n)
        && ssaa_scale(aa).is_none_or(|want| scene_scale(100, aa, out, 1) >= want - 1e-6)
}

/// The 3D scene's size; UI and HUD retain the output dimensions.
pub fn scene_size(out: (u32, u32), samples: u32) -> (u32, u32) {
    let (v, percent, aa) = read(|s| (s.values.upscale, s.values.render_scale, s.values.post_aa));
    scene_size_for(v, percent, aa, out, samples)
}

/// [`scene_size`] for Upscaling `v`, Render Scale `percent` and the
/// Anti-Aliasing row `aa`.
pub(super) fn scene_size_for(v: u8, percent: u8, aa: u8, out: (u32, u32), samples: u32) -> (u32, u32) {
    // Upscaling: the scene at its share of the screen, whatever the
    // Render Scale and the SSAA (the temporal pass takes it to the screen).
    // FSR 2's render sizes (each side divided by the mode's ratio).
    if let Some(ratio) = crate::render::fsr2::ratio(v) {
        return crate::render::fsr2::render_size(out, ratio);
    }
    if let Some(share) = upscale_share(v) {
        return upscale_size(out, share);
    }
    if percent == 100 && ssaa_scale(aa).is_none() {
        return out;
    }
    let s = scene_scale(percent, aa, out, samples);
    (((f64::from(out.0) * s).round() as u32).max(1), ((f64::from(out.1) * s).round() as u32).max(1))
}

/// The Anti-Aliasing row in force (0 off, 1 FXAA, 2 SMAA, 3–5 MSAA, 6–7 SSAA).
pub fn post_aa() -> u8 {
    read(|s| s.values.post_aa)
}

/// The Anti-Aliasing row's name when not off.
pub fn aa_name(v: u8) -> Option<&'static str> {
    Some(match v {
        0 => return None,
        1 => "FXAA",
        2 => "SMAA 1x",
        3 => "MSAA 2x",
        4 => "MSAA 4x",
        5 => "MSAA 8x",
        6 => "SSAA 2x",
        7 => "SSAA 4x",
        8 => "SSAA 8x",
        9 => "SSAA 9x",
        10 => "SSAA 16x",
        TAA => "TAA",
        _ => return None,
    })
}

/// Upscaling's render share of each side (None: off).
pub fn upscale_share(v: u8) -> Option<f64> {
    match v {
        1 | 4 => Some(0.67),
        2 | 5 => Some(0.58),
        3 | 6 => Some(0.5),
        v => crate::render::fsr2::ratio(v).map(|r| 1.0 / r),
    }
}

pub fn upscale_name(v: u8) -> Option<&'static str> {
    Some(match v {
        1 => "Quality",
        2 => "Balanced",
        3 => "Performance",
        4 => "FSR Quality",
        5 => "FSR Balanced",
        6 => "FSR Performance",
        7 => "FSR 2 Native AA",
        8 => "FSR 2 Quality",
        9 => "FSR 2 Balanced",
        10 => "FSR 2 Performance",
        11 => "FSR 2 Ultra Performance",
        _ => return None,
    })
}

/// The scene's size at `share` of the screen `out` (each side rounded,
/// at least 1).
pub fn upscale_size(out: (u32, u32), share: f64) -> (u32, u32) {
    (((f64::from(out.0) * share).round() as u32).max(1), ((f64::from(out.1) * share).round() as u32).max(1))
}

/// Upscaling in force (0 off, 1–3).
pub fn upscale() -> u8 {
    read(|s| s.values.upscale)
}

/// The temporal pass in force: TAA on the Anti-Aliasing row or
/// Upscaling on.
pub fn temporal() -> bool {
    post_aa() == TAA || is_temporal_upscale(upscale())
}

/// Whether Upscaling value `v` runs the temporal pass: the TAAU (1–3) or
/// FSR 2 (7–11).
pub fn is_temporal_upscale(v: u8) -> bool {
    matches!(v, 1..=3) || is_fsr2(v)
}

/// Whether Upscaling value `v` is an FSR 2 mode (docs/fsr2.md): the
/// temporal pass with FSR 2's passes in place of the TAA resolve.
pub fn is_fsr2(v: u8) -> bool {
    matches!(v, 7..=UPSCALE_MAX)
}

/// FSR 2 in force (its temporal frames still need [`temporal_frame`]).
pub fn fsr2() -> bool {
    is_fsr2(upscale())
}

/// Whether Upscaling value `v` is one of FSR 1's tiers: a spatial
/// upscale (EASU + RCAS, `render::fsr`) of the scene with its native MSAA,
/// no temporal pass.
pub fn is_fsr(v: u8) -> bool {
    matches!(v, 4..=6)
}

/// FSR 1 upscaling in force.
pub fn fsr() -> bool {
    is_fsr(upscale())
}

/// The scene's sample count the Anti-Aliasing row asks for instead of
/// Full Screen Anti-Aliasing's (MSAA 2× / 4× / 8×), if any.
pub fn msaa_override() -> Option<u32> {
    msaa_in_frame(temporal(), temporal_frame(), post_aa(), read(|s| s.values.aa_msaa))
}

/// [`msaa_override`] for these values: the temporal pass is the one AA
/// where it runs (no samples); elsewhere (front end, garage) TAA /
/// Upscaling fall back to the native MSAA.
pub fn msaa_in_frame(temporal: bool, temporal_frame: bool, aa: u8, override_: u8) -> Option<u32> {
    if temporal {
        return temporal_frame.then_some(1);
    }
    msaa_combined(aa, override_)
}

/// [`msaa_override`] for the row's value `aa`.
pub fn msaa_for(aa: u8) -> Option<u32> {
    match aa {
        v @ 3..=5 => Some(1 << (v - 2)),
        6..=10 | TAA => Some(1),
        _ => None,
    }
}

/// Requested MSAA after explicit combination controls, before hardware fallback.
pub fn msaa_combined(preset: u8, override_: u8) -> Option<u32> {
    // TAA is the one AA setting: no samples under it.
    if preset == TAA {
        return Some(1);
    }
    if override_ == 0 { msaa_for(preset) } else { Some(override_ as u32) }
}
/// [`msaa_combined`] for stored values `e`, with Upscaling's (no
/// samples under the temporal pass).
pub fn msaa_choice(e: &Enhancements) -> Option<u32> {
    if is_temporal_upscale(e.upscale) { Some(1) } else { msaa_combined(e.post_aa, e.aa_msaa) }
}
/// Whether stored values `e` run the temporal pass (TAA or Upscaling).
pub fn temporal_for(e: &Enhancements) -> bool {
    e.post_aa == TAA || is_temporal_upscale(e.upscale)
}
pub fn filter_name(v: u8) -> &'static str {
    match v {
        0 => "Preset",
        1 => "Off",
        2 => "FXAA",
        3 => "SMAA",
        _ => "SMAA + FXAA",
    }
}
pub fn filters(preset: u8, override_: u8) -> (bool, bool) {
    match override_ {
        0 => (preset == 1, preset == 2),
        1 => (false, false),
        2 => (true, false),
        3 => (false, true),
        _ => (true, true),
    }
}
pub fn fxaa() -> bool {
    !temporal() && filters(post_aa(), read(|s| s.values.aa_filter)).0
}
