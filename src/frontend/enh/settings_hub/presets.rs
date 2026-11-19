use super::*;

// ---------------------------------------------------------------------
// Presets and the 60 fps pick.

/// The preset row's entries in order (then the 60 fps entry).
pub const PAGE_PRESETS: [Preset; 5] = [Preset::Original, Preset::Clean, Preset::Modern, Preset::Ultra, Preset::Extreme];

/// The entries offered on this machine: a preset whose values on this GPU class are an earlier one's is left out
/// (Extreme on an integrated GPU is Ultra's ladder).
pub(super) fn offered_presets() -> Vec<Preset> {
    let mut seen: Vec<qol::Enhancements> = Vec::new();
    let mut v = Vec::new();
    for p in PAGE_PRESETS {
        let mut e = qol::Enhancements::default();
        p.apply(&mut e);
        if !seen.contains(&e) {
            seen.push(e);
            v.push(p);
        }
    }
    v
}

/// Measured preset totals: the heaviest of the four bench scenes (free
/// roam with the burnout's smoke, racing, the race start, rain at clock
/// 162), GPU busy median ms per frame, 1920×1080 and 3840×2160 on the
/// reference desktop GPU (the High class), and 1920×1080 on the reference integrated GPU (the
/// Integrated class, with its class's preset tiers). docs/developing/settings-page.md
/// "60 fps" (the maintainer's private capture).
pub struct PresetCost {
    pub preset: Preset,
    pub high_1080: f32,
    pub high_4k: f32,
    pub integrated_1080: f32,
}

pub const PRESET_COSTS: [PresetCost; 5] = [
    PresetCost { preset: Preset::Original, high_1080: 0.85, high_4k: 2.92, integrated_1080: 14.8 },
    PresetCost { preset: Preset::Clean, high_1080: 1.23, high_4k: 5.62, integrated_1080: 18.1 },
    PresetCost { preset: Preset::Modern, high_1080: 2.41, high_4k: 7.20, integrated_1080: 50.9 },
    PresetCost { preset: Preset::Ultra, high_1080: 3.97, high_4k: 13.07, integrated_1080: 72.0 },
    PresetCost { preset: Preset::Extreme, high_1080: 5.55, high_4k: 16.56, integrated_1080: 72.0 },
];

/// The GPU time a 60 fps pick may use: three quarters of the 16.7 ms
/// frame (room for the CPU, the frame's spikes and the scenes not
/// benched).
pub const FPS60_BUDGET_MS: f32 = 12.5;

/// The estimated heaviest-scene GPU ms of `c` at `out` on a `class` GPU:
/// High measured at 1080p and 4K, linear in the pixels between and beyond
/// (**measured** at the two sizes, **inferred** elsewhere); Integrated
/// measured at 1080p, scaled by the pixels (**inferred** off 1080p); Mid
/// twice High (**inferred**: no mid-class card measured).
pub fn preset_ms(c: &PresetCost, class: GpuClass, out: (u32, u32)) -> f32 {
    let px = out.0 as f32 * out.1 as f32;
    let (p1, p4) = (1920.0 * 1080.0, 3840.0 * 2160.0);
    let high = c.high_1080 + (c.high_4k - c.high_1080) * (px - p1) / (p4 - p1);
    match class {
        GpuClass::High => high.max(0.0),
        GpuClass::Mid => 2.0 * high.max(0.0),
        GpuClass::Integrated => c.integrated_1080 * px / p1,
    }
}

/// The preset the 60 fps entry picks: the costliest whose estimate fits
/// [`FPS60_BUDGET_MS`]; Original when none does. A one-time choice of fixed
/// values (no dynamic resolution, nothing changes while playing).
pub fn fps60_pick(class: GpuClass, out: (u32, u32)) -> (Preset, f32) {
    let mut best = (Preset::Original, preset_ms(&PRESET_COSTS[0], class, out));
    for c in &PRESET_COSTS {
        let ms = preset_ms(c, class, out);
        if ms <= FPS60_BUDGET_MS {
            best = (c.preset, ms);
        }
    }
    best
}

fn preset_name(p: Preset) -> &'static str {
    p.name()
}

/// The preset row's value: the preset, "60 FPS (High)" after the 60 fps
/// entry picked it, or Custom.
pub fn preset_value(s: &Settings) -> String {
    let fps = FPS60.load(Ordering::Relaxed);
    match s.enh.preset() {
        Some(p) if fps != 0 && PAGE_PRESETS.get(usize::from(fps - 1)) == Some(&p) => {
            format!("{} ({})", text(Text::Fps60), preset_name(p))
        }
        Some(p) => preset_name(p).to_string(),
        None => text(Text::Custom).to_string(),
    }
}

/// Left / Right on the preset row: the next / previous entry (the 60 fps
/// entry after the presets), wrapping; from Custom the first / last.
pub fn step_preset(s: &mut Settings, right: bool) {
    let list = offered_presets();
    let n = list.len() + 1;
    let fps = FPS60.load(Ordering::Relaxed);
    let at = match s.enh.preset() {
        Some(p) if fps != 0 && PAGE_PRESETS.get(usize::from(fps - 1)) == Some(&p) => Some(list.len()),
        Some(p) => list.iter().position(|&q| q == p),
        None => None,
    };
    let next = match at {
        Some(i) => {
            if right {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            }
        }
        None => {
            if right {
                0
            } else {
                n - 1
            }
        }
    };
    if next == list.len() {
        let (p, _) = fps60_pick(qol::gpu_class(), qol::output_size_or(s.resolution));
        p.apply(&mut s.enh);
        FPS60.store(PAGE_PRESETS.iter().position(|&q| q == p).map_or(0, |i| i as u8 + 1), Ordering::Relaxed);
    } else {
        list[next].apply(&mut s.enh);
        FPS60.store(0, Ordering::Relaxed);
    }
    if s.enh.preset() == Some(Preset::Original) {
        // Original: Customize Display as the original at the World Detail level.
        s.native_display();
    } else {
        graphics_table::normalize(s);
    }
}

pub(super) fn preset_help(s: &Settings) -> String {
    let out = qol::output_size_or(s.resolution);
    let class = qol::gpu_class();
    let size = format!("{}x{}", out.0, out.1);
    let mut t = match s.enh.preset() {
        Some(Preset::Original) => "Original: every value as the original (native).".to_string(),
        Some(Preset::Clean) => "Balanced: smoother edges (MSAA 8x with alpha-to-coverage), high-precision glow, richer car paint.".to_string(),
        Some(Preset::Modern) => "High: Balanced + sharper reflections, wet roads, ambient occlusion, softer car shadow, lamp light and shadows, headlight beams, light probes, LOD fade, 1.5x draw distance.".to_string(),
        Some(Preset::Ultra) => "Ultra: High + SSAA 2x, full-size glow, full-resolution reflections, SSR, volumetric fog, finer smoke, animated water, rain detail, wet road interaction, glass, 2x draw distance.".to_string(),
        Some(Preset::Extreme) => "Extreme: Ultra + full-resolution SSR, volumetric (Cinematic) smoke, realistic water, Wet Road Interaction High, surface materials, smooth car shapes, World Detail Extreme.".to_string(),
        None => "Custom: the values match no preset. Left / Right picks one.".to_string(),
    };
    if let Some(c) = s
        .enh
        .preset()
        .and_then(|p| PRESET_COSTS.iter().find(|c| c.preset == p))
        .filter(|_| FPS60.load(Ordering::Relaxed) == 0)
    {
        let ms = preset_ms(c, class, out);
        t.push_str(&format!(" ~{ms:.1} ms of GPU time at {size} in the heaviest scene."));
    }
    if FPS60.load(Ordering::Relaxed) != 0 && s.enh.preset().is_some() {
        let (p, ms) = fps60_pick(class, out);
        t = if ms <= FPS60_BUDGET_MS {
            format!(
                "60 FPS on this machine ({} GPU, {size}): {} fits, ~{ms:.1} of the {FPS60_BUDGET_MS} ms it may use. {t}",
                class.name(),
                p.name()
            )
        } else {
            format!(
                "60 FPS on this machine ({} GPU, {size}): even Original needs ~{ms:.1} ms (of {FPS60_BUDGET_MS}); a lower resolution or Upscaling (Performance) gets closer. {t}",
                class.name()
            )
        };
    }
    if class != GpuClass::High {
        t.push_str(&format!(" Effect tiers for this {} GPU (lighter AA, fog, smoke and extras).", class.name()));
    }
    format!("{t} {}", text(Text::HubPreset))
}
