use super::*;
use crate::qol::{LEVELS, Level};

// ---------------------------------------------------------------------
// The streamlined graphics page (docs/developing/settings-page.md "Graphics mode and
// quality levels"): the hub's first rows. Graphics Mode (Original /
// Enhanced), Enhanced Quality (a level, or Custom), the main choices
// (Upscaling, Anti-Aliasing, Ray Tracing, HDR Output, Motion Blur, Rain on
// the Lens), then Advanced (every individual row, by group).

/// The groups on the Advanced page (the graphics groups and the packs);
/// the others stay on the hub.
pub const ADVANCED_GROUPS: [UxGroup; 6] = [
    UxGroup::Display,
    UxGroup::ImageQuality,
    UxGroup::LightingEffects,
    UxGroup::WeatherWater,
    UxGroup::Performance,
    UxGroup::Packs,
];

/// The groups listed on the hub after Advanced.
pub const HUB_GROUPS: [UxGroup; 2] = [UxGroup::GameplayAccessibility, UxGroup::Controller];

/// The main Upscaling row's values (`enh.upscale`: Off and the temporal
/// upscaler's three modes; FSR 1 / FSR 2 on Advanced).
const MAIN_UPSCALE: [usize; 4] = [0, 1, 2, 3];
/// The main Anti-Aliasing row's values (`enh.post_aa`): the original's MSAA,
/// TAA, SSAA 2x to 16x (MSAA 2x–8x with alpha-to-coverage, FXAA and SMAA
/// on Advanced).
const MAIN_AA: [usize; 7] = [0, qol::TAA as usize, 6, 7, 8, 9, 10];

/// Measured level totals (docs/developing/settings-page.md "Level costs",
/// `docs/evidence/settings-levels/levels.txt`): the heaviest of the bench
/// scenes, GPU busy median ms per frame, as [`PRESET_COSTS`] (Anti-Aliasing
/// Native; Original 0.70 / 2.60 / 15.0).
pub struct LevelCost {
    pub level: Level,
    pub high_1080: f32,
    pub high_4k: f32,
    pub integrated_1080: f32,
}

pub const LEVEL_COSTS: [LevelCost; 3] = [
    LevelCost { level: Level::Low, high_1080: 0.74, high_4k: 2.66, integrated_1080: 17.4 },
    LevelCost { level: Level::Medium, high_1080: 1.77, high_4k: 7.08, integrated_1080: 49.5 },
    LevelCost { level: Level::High, high_1080: 3.32, high_4k: 11.42, integrated_1080: 100.5 },
];

/// The estimated heaviest-scene GPU ms of a level at `out` ([`preset_ms`]'s
/// rule).
pub fn level_ms(l: Level, class: GpuClass, out: (u32, u32)) -> Option<f32> {
    let c = LEVEL_COSTS.iter().find(|c| c.level == l)?;
    let p = PresetCost {
        preset: Preset::Original,
        high_1080: c.high_1080,
        high_4k: c.high_4k,
        integrated_1080: c.integrated_1080,
    };
    Some(preset_ms(&p, class, out))
}

fn tier_of(id: RowId, s: &Settings) -> usize {
    graphics_table::row(id).map_or(0, |r| (r.get)(s))
}

/// The next value of `list` from `at` toward `right`, wrapping; a value not
/// listed steps from the list's first.
fn next_in(list: &[usize], at: usize, right: bool, offered: impl Fn(usize) -> bool) -> Option<usize> {
    let ok: Vec<usize> = list.iter().copied().filter(|&v| offered(v)).collect();
    if ok.is_empty() {
        return None;
    }
    let i = ok.iter().position(|&v| v == at).unwrap_or(0);
    let n = ok.len();
    Some(ok[if right { (i + 1) % n } else { (i + n - 1) % n }])
}

/// Graphics Mode Enhanced: the values are not the original's.
pub fn enhanced_mode(s: &Settings) -> bool {
    !qol::graphics_original(&s.enh)
}

/// Graphics Mode to Original (the Original preset, Customize Display as the
/// original at the World Detail level) or Enhanced (the GPU class's level).
pub fn set_mode(s: &mut Settings, enhanced: bool) {
    if enhanced {
        Level::for_class(qol::gpu_class()).apply(&mut s.enh);
        graphics_table::normalize(s);
    } else {
        Preset::Original.apply(&mut s.enh);
        s.native_display();
    }
}

/// Enhanced Quality to level `l`: its rows written, the choices kept.
pub fn set_level(s: &mut Settings, l: Level) {
    l.apply(&mut s.enh);
    graphics_table::normalize(s);
}

/// Left / Right on a main row.
pub fn step_main(id: RowId, s: &mut Settings, right: bool) {
    match id {
        RowId::GraphicsMode => set_mode(s, !enhanced_mode(s)),
        RowId::EnhancedQuality => {
            let n = LEVELS.len();
            let next = match Level::of(&s.enh).and_then(|l| LEVELS.iter().position(|&x| x == l)) {
                Some(i) if right => (i + 1) % n,
                Some(i) => (i + n - 1) % n,
                None if right => 0,
                None => n - 1,
            };
            set_level(s, LEVELS[next]);
        }
        RowId::MainUpscale => {
            let Some(r) = graphics_table::row(RowId::Upscale) else { return };
            if let Some(v) = next_in(&MAIN_UPSCALE, (r.get)(s), right, |v| r.is_offered(s, v)) {
                (r.set)(s, v);
            }
        }
        RowId::MainAa => {
            let Some(r) = graphics_table::row(RowId::PostAa) else { return };
            if let Some(v) = next_in(&MAIN_AA, (r.get)(s), right, |v| r.is_offered(s, v)) {
                (r.set)(s, v);
            }
        }
        RowId::RayTracing => {
            let on = qol::ray_traced(&s.enh);
            if on || qol::rt_offered() {
                qol::set_ray_traced(&mut s.enh, !on);
                graphics_table::normalize(s);
            }
        }
        RowId::MainMotionBlur => {
            let Some(r) = graphics_table::row(RowId::MotionBlur) else { return };
            let v = if (r.get)(s) == 0 { 1 } else { 0 };
            if r.is_offered(s, v) {
                (r.set)(s, v);
            }
        }
        _ => {}
    }
}

fn cycle(id: RowId, label: u32, value: String, enabled: bool) -> Row {
    Row { id, label, kind: Kind::Cycle, value: Some(Value::Owned(value)), enabled }
}

fn button(id: RowId, label: u32) -> Row {
    Row { id, label, kind: Kind::Button, value: None, enabled: true }
}

/// The main page's rows (the hub's first rows): Original lists only the
/// rows that are not enhancements of the picture (HDR Output, the
/// original's Motion Blur) besides the mode and Advanced.
pub fn main_rows(s: &Settings) -> Vec<Row> {
    let graphics = options::graphics_rows(s);
    let find = |id: RowId| graphics.iter().find(|r| r.id == id).cloned();
    let enhanced = enhanced_mode(s);
    let mut v = vec![cycle(
        RowId::GraphicsMode,
        options::GRAPHICS_MODE,
        text(if enhanced { Text::ModeEnhanced } else { Text::ModeOriginal }).to_string(),
        true,
    )];
    if enhanced {
        let level = Level::of(&s.enh).map_or(text(Text::Custom), Level::name);
        v.push(cycle(RowId::EnhancedQuality, options::ENHANCED_QUALITY, level.to_string(), true));
        if let Some(r) = find(RowId::Upscale) {
            v.push(Row { id: RowId::MainUpscale, label: options::UPSCALE, ..r });
        }
        if let Some(mut r) = find(RowId::PostAa) {
            if r.enabled && s.enh.post_aa == 0 && !matches!(s.enh.upscale, 1..=3) {
                r.value = Some(Value::Owned(text(Text::NativeMsaa).to_string()));
            }
            v.push(Row { id: RowId::MainAa, label: options::POST_AA, ..r });
        }
        let on = qol::ray_traced(&s.enh);
        let offered = qol::rt_offered();
        let value = match (on, offered) {
            (true, _) => text(Text::On),
            (false, true) => text(Text::Off),
            (false, false) => text(Text::NotSupported),
        };
        v.push(cycle(RowId::RayTracing, options::RAY_TRACING, value.to_string(), on || offered));
    }
    if let Some(r) = find(RowId::HdrOutput) {
        v.push(r);
    }
    if s.enh.hdr_output {
        v.push(button(RowId::UxGroup(UxGroup::Display), options::HDR_CALIBRATION));
    }
    if let Some(mut r) = find(RowId::MotionBlur) {
        if tier_of(RowId::MotionBlur, s) == 1 && r.enabled {
            r.value = Some(Value::Owned(text(Text::On).to_string()));
        }
        v.push(Row { id: RowId::MainMotionBlur, ..r });
    }
    if enhanced && let Some(r) = find(RowId::LensDrops) {
        v.push(Row { label: options::RAIN_ON_LENS, ..r });
    }
    v.push(button(RowId::AdvancedPage, options::ADVANCED));
    v
}

/// The Advanced page's rows: Compare, Show, the graphics groups with how
/// many of their rows are changed.
pub fn advanced_rows(s: &Settings) -> Vec<Row> {
    let mut v = vec![
        cycle(
            RowId::UxCompare,
            options::UX_COMPARE,
            text(if qol::comparing() { Text::CompareOn } else { Text::CompareOff }).to_string(),
            true,
        ),
        cycle(
            RowId::UxFilter,
            options::UX_FILTER,
            text([Text::ShowAll, Text::ShowChanged, Text::ShowCostly][usize::from(filter().min(2))]).to_string(),
            true,
        ),
    ];
    let native = std::cell::OnceCell::new();
    v.extend(ADVANCED_GROUPS.iter().map(|&g| group_row(g, s, &native)));
    v
}

/// The detail line of a main row (None: not one).
pub fn main_detail(category: Category, id: RowId, s: &Settings) -> Option<String> {
    let out = qol::output_size_or(s.resolution);
    let class = qol::gpu_class();
    Some(match id {
        RowId::GraphicsMode if enhanced_mode(s) => text(Text::HelpModeEnhanced).to_string(),
        RowId::GraphicsMode => text(Text::HelpModeOriginal).to_string(),
        RowId::EnhancedQuality => {
            let mut t = match Level::of(&s.enh) {
                Some(Level::Low) => text(Text::HelpLow).to_string(),
                Some(Level::Medium) => text(Text::HelpMedium).to_string(),
                Some(Level::High) => text(Text::HelpHigh).to_string(),
                None => text(Text::HelpCustom).to_string(),
            };
            if let Some(ms) = Level::of(&s.enh).and_then(|l| level_ms(l, class, out)) {
                t.push(' ');
                t.push_str(&fill(Text::LevelCost, &[&format!("{ms:.1}"), &format!("{}x{}", out.0, out.1)]));
            }
            format!("{t} {}", text(Text::HelpLevels))
        }
        RowId::MainUpscale => main_row_detail(RowId::Upscale, Text::HelpUpscale, s),
        RowId::MainAa => main_row_detail(RowId::PostAa, Text::HelpAa, s),
        RowId::RayTracing if !qol::rt_offered() && !qol::ray_traced(&s.enh) => {
            text(Text::HelpRayTracingNone).to_string()
        }
        RowId::RayTracing => {
            let ms = options::cost_ms(RowId::ModernLighting, s, out).unwrap_or(0.0)
                + options::cost_ms(RowId::Ssr, s, out).unwrap_or(0.0);
            let mut t = text(Text::HelpRayTracing).to_string();
            if qol::ray_traced(&s.enh) && ms > 0.05 {
                t.push_str(&format!(" ~+{ms:.1} ms here."));
            }
            t
        }
        RowId::MainMotionBlur => main_row_detail(RowId::MotionBlur, Text::HelpMotionBlur, s),
        RowId::UxGroup(UxGroup::Display) if category == Category::Enhancements => {
            text(Text::HelpCalibration).to_string()
        }
        RowId::AdvancedPage => text(Text::HelpAdvanced).to_string(),
        _ => return None,
    })
}

/// A main row's line: its own lead, then the Advanced row's status (origin,
/// cost, the rule in force) and tier text.
fn main_row_detail(row: RowId, lead: Text, s: &Settings) -> String {
    let rest = detail(Category::Settings(UxGroup::ImageQuality), row, s).unwrap_or_default();
    format!("{} {rest}", text(lead))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native() -> Settings {
        let mut s = Settings::default();
        s.custom = options::custom_presets(5);
        s
    }

    fn ids(s: &Settings) -> Vec<RowId> {
        hub_rows(s).iter().map(|r| r.id).collect()
    }

    /// A fresh install is Graphics Mode Original with no Enhanced row
    /// listed; Enhanced lists the quality, the choices and Rain on the
    /// Lens; every main row has a label and a detail line.
    #[test]
    fn the_main_page_follows_the_mode() {
        let mut s = native();
        assert!(!enhanced_mode(&s));
        let o = ids(&s);
        assert_eq!(o[0], RowId::GraphicsMode);
        for id in [RowId::EnhancedQuality, RowId::MainUpscale, RowId::MainAa, RowId::RayTracing, RowId::LensDrops] {
            assert!(!o.contains(&id), "{id:?} listed in Original");
        }
        assert!(o.contains(&RowId::AdvancedPage) && o.contains(&RowId::MainMotionBlur));
        step_main(RowId::GraphicsMode, &mut s, true);
        assert!(enhanced_mode(&s));
        assert_eq!(Level::of(&s.enh), Some(Level::for_class(qol::gpu_class())));
        let e = ids(&s);
        assert_eq!(
            &e[..5],
            [RowId::GraphicsMode, RowId::EnhancedQuality, RowId::MainUpscale, RowId::MainAa, RowId::RayTracing]
        );
        assert!(e.contains(&RowId::LensDrops));
        for r in hub_rows(&s) {
            assert!(options::clone_label(r.label).is_some() || r.label == 0x822E_4E9B, "{:?} has no label", r.id);
            assert!(detail(Category::Enhancements, r.id, &s).is_some_and(|d| d.len() > 10), "{:?}: no detail", r.id);
        }
        // Back to Original: the defaults' options file again.
        step_main(RowId::GraphicsMode, &mut s, true);
        assert_eq!(s.options_text(), native().options_text());
    }

    /// Choosing a level writes its rows, a row changed on Advanced flips
    /// the level to Custom, and choosing a level again overwrites it; the
    /// main choices survive a level change.
    #[test]
    fn levels_write_rows_and_custom_is_shown() {
        let mut s = native();
        set_mode(&mut s, true);
        for _ in 0..LEVELS.len() {
            step_main(RowId::EnhancedQuality, &mut s, true);
            assert!(Level::of(&s.enh).is_some());
        }
        set_level(&mut s, Level::Medium);
        step_main(RowId::MainAa, &mut s, true);
        let aa = s.enh.post_aa;
        assert_eq!(aa, qol::TAA, "Native MSAA → TAA");
        options::change(RowId::Sky, &mut s, 0, true, false);
        let q = hub_rows(&s).into_iter().find(|r| r.id == RowId::EnhancedQuality).unwrap();
        assert_eq!(q.value, Some(Value::Owned("Custom".into())));
        step_main(RowId::EnhancedQuality, &mut s, true);
        assert_eq!(Level::of(&s.enh), Some(Level::Low), "Custom → Right: the first level");
        assert_eq!((s.enh.sky, s.enh.post_aa), (0, aa), "the level overwrote Sky; the AA choice stayed");
    }

    /// The Anti-Aliasing row keeps SSAA up to 16x and TAA; Upscaling steps
    /// Off / Quality / Balanced / Performance; Motion Blur Off / On.
    #[test]
    fn main_choices_step_their_lists() {
        let mut s = native();
        set_mode(&mut s, true);
        let mut seen = Vec::new();
        for _ in 0..MAIN_AA.len() {
            step_main(RowId::MainAa, &mut s, true);
            seen.push(usize::from(s.enh.post_aa));
        }
        // SSAA tiers the output cannot take are skipped (qol::aa_offered); 2x is always there.
        assert!(seen.contains(&6) && seen.contains(&0) && seen.contains(&usize::from(qol::TAA)), "{seen:?}");
        s.enh.post_aa = 0;
        for want in [1, 2, 3, 0] {
            step_main(RowId::MainUpscale, &mut s, true);
            assert_eq!(usize::from(s.enh.upscale), want);
        }
        let mb = tier_of(RowId::MotionBlur, &s);
        step_main(RowId::MainMotionBlur, &mut s, true);
        assert_ne!(tier_of(RowId::MotionBlur, &s), mb);
        step_main(RowId::MainMotionBlur, &mut s, true);
        assert_eq!(tier_of(RowId::MotionBlur, &s), mb);
    }

    /// An options file of every earlier preset loads with the same values
    /// and writes back unchanged; the page reads it as the right mode and
    /// level (or Custom) without changing anything.
    #[test]
    fn old_option_files_survive() {
        for p in qol::PRESETS {
            let mut s = native();
            p.apply_for(GpuClass::High, &mut s.enh);
            graphics_table::normalize(&mut s);
            let text0 = s.options_text();
            let t = Settings::from_text(&text0);
            assert_eq!(t.enh, s.enh, "{p:?}");
            assert_eq!(t.custom, s.custom, "{p:?}");
            assert_eq!(t.options_text(), text0, "{p:?}: load → save unchanged");
            let rows = hub_rows(&t);
            assert_eq!(t.options_text(), text0, "{p:?}: showing the page changes nothing");
            assert_eq!(enhanced_mode(&t), p != Preset::Original, "{p:?}");
            let q = rows.iter().find(|r| r.id == RowId::EnhancedQuality).map(|r| r.value.clone());
            let want = match p {
                Preset::Original => None,
                Preset::Ultra => Some(Some(Value::Owned("High".into()))),
                _ => Some(Some(Value::Owned("Custom".into()))),
            };
            assert_eq!(q, want, "{p:?}");
        }
    }

    /// The measured table is ordered: each level costs more than the one
    /// before it on both GPUs; Low stays within Original + 3 ms on the
    /// integrated GPU at 1080p (Original 15.0 ms there).
    #[test]
    fn level_costs_are_ordered() {
        for w in LEVEL_COSTS.windows(2) {
            assert!(w[0].high_1080 < w[1].high_1080 && w[0].high_4k < w[1].high_4k);
            assert!(w[0].integrated_1080 < w[1].integrated_1080);
        }
        assert!(LEVEL_COSTS[0].integrated_1080 <= 15.0 + 3.0);
        assert_eq!(LEVEL_COSTS.map(|c| c.level), LEVELS);
    }

    /// Ray tracing: offered only where the device can trace; on, both
    /// ray-traced tiers; off, the level's own reflections back.
    #[test]
    fn ray_tracing_is_its_own_choice() {
        let mut s = native();
        set_mode(&mut s, true);
        set_level(&mut s, Level::High);
        let before = s.enh;
        if qol::rt_offered() {
            step_main(RowId::RayTracing, &mut s, true);
            assert!(qol::ray_traced(&s.enh));
            assert_eq!(Level::of(&s.enh), Some(Level::High));
            step_main(RowId::RayTracing, &mut s, true);
        } else {
            step_main(RowId::RayTracing, &mut s, true);
            let r = hub_rows(&s).into_iter().find(|r| r.id == RowId::RayTracing).unwrap();
            assert!(!r.enabled);
        }
        assert_eq!(s.enh, before);
    }
}
