//! Weak-GPU mode for Studio's world view: a cheaper picture on slow adapters. The view draws at a lower
//! resolution scale, at half the detail distance, without shadows, reflections or screen-space effects.
//! Only Studio's view reads it; the game's own render paths are not touched.
//!
//! The setting (Settings ▸ World view) is Auto (weak on integrated and software adapters), Full or Weak.
//! It is kept in `studio-settings.toml` beside the game's settings and shown in the status bar while weak.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::render::RenderOptions;

/// The resolution scale of the weak view (the picture is drawn this fraction of the window's pixels, then
/// stretched).
pub const SCALE: f32 = 0.6;
/// The weak view's detail distance, as a fraction of the player's.
pub const DETAIL: f32 = 0.5;
/// The settings file's name.
pub const FILE: &str = "studio-settings.toml";

/// What the player chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pref {
    #[default]
    Auto,
    Full,
    Weak,
}

impl Pref {
    pub const ALL: [Pref; 3] = [Pref::Auto, Pref::Full, Pref::Weak];

    pub fn label(self) -> &'static str {
        match self {
            Pref::Auto => "Auto",
            Pref::Full => "Full",
            Pref::Weak => "Weak GPU",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Pref::Auto => "auto",
            Pref::Full => "full",
            Pref::Weak => "weak",
        }
    }

    pub fn parse(v: &str) -> Option<Pref> {
        Pref::ALL.into_iter().find(|p| p.key() == v.trim().to_ascii_lowercase())
    }
}

/// Whether an adapter counts as low-end: a software rasteriser or an integrated GPU.
pub fn weak_adapter(info: &wgpu::AdapterInfo) -> bool {
    let name = info.name.to_ascii_lowercase();
    matches!(info.device_type, wgpu::DeviceType::Cpu | wgpu::DeviceType::IntegratedGpu)
        || ["llvmpipe", "swiftshader", "softpipe", "software"].iter().any(|n| name.contains(n))
}

/// The mode in force for a choice on an adapter.
pub fn resolve(pref: Pref, info: &wgpu::AdapterInfo) -> bool {
    match pref {
        Pref::Auto => weak_adapter(info),
        Pref::Full => false,
        Pref::Weak => true,
    }
}

/// The render options of the weak view: `opts` with the costly passes off and a shorter reach.
pub fn lighten(opts: &RenderOptions) -> RenderOptions {
    RenderOptions {
        post_aa: 0,
        fxaa: false,
        smaa: false,
        render_scale: 100,
        anisotropy: 0,
        depth_of_field: 0,
        detail_distance: (opts.detail_distance * DETAIL).max(0.25),
        car_detail: (opts.car_detail * DETAIL).max(0.25),
        road_reflection: 0,
        car_reflection_size: 64,
        enhanced_reflections: false,
        shadow_quality: 0,
        ssr: 0,
        ssao: 0.0,
        lamp_shadows: 0,
        contact_shadows: false,
        volumetric_fog: 0,
        motion_blur: 0,
        ..*opts
    }
}

/// The size the view draws at for a window of `want` pixels.
pub fn drawn_size(want: (u32, u32), weak: bool) -> (u32, u32) {
    if !weak || want.0 == 0 || want.1 == 0 {
        return want;
    }
    (((want.0 as f32 * SCALE).round() as u32).max(1), ((want.1 as f32 * SCALE).round() as u32).max(1))
}

static PREF: AtomicU8 = AtomicU8::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The choice in force.
pub fn pref() -> Pref {
    Pref::ALL[PREF.load(Ordering::Relaxed) as usize % 3]
}

pub fn set_pref(p: Pref) {
    PREF.store(Pref::ALL.iter().position(|q| *q == p).unwrap_or(0) as u8, Ordering::Relaxed);
}

/// Whether the view is drawing in weak mode now (set by the view; the status bar reads it).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
}

/// The status bar's words while weak, or None.
pub fn status_words() -> Option<String> {
    active().then(|| {
        let why = if pref() == Pref::Auto { "this adapter is low-end" } else { "set in Settings" };
        format!("Weak GPU mode: {why}")
    })
}

/// The settings file's text.
pub fn to_text(p: Pref) -> String {
    format!("world_view = \"{}\"\n", p.key())
}

/// The choice in a settings file's text (Auto when it says nothing usable).
pub fn from_text(text: &str) -> Pref {
    text.parse::<toml::Table>()
        .ok()
        .and_then(|t| t.get("world_view").and_then(toml::Value::as_str).and_then(Pref::parse))
        .unwrap_or_default()
}

fn path() -> std::path::PathBuf {
    ob_core::platform::dirs::config().join(FILE)
}

/// Read the saved choice (Studio's start).
pub fn load() {
    if let Ok(t) = std::fs::read_to_string(path()) {
        set_pref(from_text(&t));
    }
}

/// Keep the choice.
pub fn save(p: Pref) {
    let f = path();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&f, to_text(p)) {
        log::warn!("studio: could not save {}: {e}", f.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(device_type: wgpu::DeviceType, name: &str) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: name.into(),
            device_type,
            ..wgpu::AdapterInfo::new(device_type, wgpu::Backend::Vulkan)
        }
    }

    #[test]
    fn auto_picks_weak_on_low_end_adapters_only() {
        assert!(weak_adapter(&info(wgpu::DeviceType::IntegratedGpu, "Intel UHD")));
        assert!(weak_adapter(&info(wgpu::DeviceType::Cpu, "x")));
        assert!(weak_adapter(&info(wgpu::DeviceType::Other, "llvmpipe (LLVM 17)")));
        assert!(!weak_adapter(&info(wgpu::DeviceType::DiscreteGpu, "AMD Radeon RX 7800 XT")));
    }

    #[test]
    fn the_choice_overrides_the_adapter() {
        let low = info(wgpu::DeviceType::IntegratedGpu, "i");
        let high = info(wgpu::DeviceType::DiscreteGpu, "d");
        assert!(resolve(Pref::Auto, &low) && !resolve(Pref::Auto, &high));
        assert!(!resolve(Pref::Full, &low));
        assert!(resolve(Pref::Weak, &high));
    }

    #[test]
    fn weak_draws_smaller_with_shorter_reach_and_no_extras() {
        assert_eq!(drawn_size((1000, 500), false), (1000, 500));
        assert_eq!(drawn_size((1000, 500), true), (600, 300));
        assert_eq!(drawn_size((0, 500), true), (0, 500));
        let o = lighten(&RenderOptions::default());
        assert_eq!((o.shadow_quality, o.ssr, o.lamp_shadows, o.road_reflection), (0, 0, 0, 0));
        assert!(!o.contact_shadows);
        let full = RenderOptions { detail_distance: 2.0, ..RenderOptions::default() };
        assert_eq!(lighten(&full).detail_distance, 1.0);
    }

    #[test]
    fn the_choice_round_trips_through_the_file() {
        for p in Pref::ALL {
            assert_eq!(from_text(&to_text(p)), p);
        }
        assert_eq!(from_text("garbage ="), Pref::Auto);
        assert_eq!(from_text("world_view = \"x\""), Pref::Auto);
    }
}
