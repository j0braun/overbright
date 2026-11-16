//! Quality-of-life options: **clone deviations**, every one opt-in, every
//! default native (docs/formats.md, "QoL (clone deviation)").
//!
//! * [`Enhancements`]: the Enhancements page's values (Display → Enhancements,
//!   kept in options.txt as `enh.*` lines, written only when not native).
//!   The values in force live in this module's globals ([`Enhancements::apply`]),
//!   read by the camera, the HUD, the frame loop and the profile writer.
//! * [`saves`]: durable profile writes and rolling backups (the file's bytes
//!   stay the native format).
//! * [`crash`]: local crash reports (a panic hook; nothing leaves the machine).
//! * [`setup`]: finding and checking the game folder on a first run.

#[cfg(test)]
use crate::frontend::Quad;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU16, AtomicU64, Ordering};

mod atomics;
mod enhancements;
mod frame;
mod hdr;
mod hud_scale;
mod levels;
mod presets;
mod render_scale;

pub use atomics::*;
pub use enhancements::*;
pub use hdr::*;
pub use hud_scale::*;
pub use levels::*;
pub use presets::*;
pub use render_scale::*;

/// Local crash reports: a panic hook that writes `crash-<time>.txt` with
/// the version, the OS, the GPU adapter, the deviations on, the panic and
/// its backtrace, and the last log lines. Nothing is ever sent anywhere.
pub mod crash;
/// The game's log file: `<logs>/game-<time>.log`, the last 5 kept.
pub mod log_file;
/// Durable profile writes and rolling backups.
pub mod saves;
/// First run: finding the game folder and checking it.
pub mod setup;
/// What a player waits for and the drives' frame times, in the game's log.
pub mod waits;

/// Test isolation for the library's unit tests (docs/test-determinism.md;
/// test-only): the Enhancements' globals above are process-wide and libtest
/// runs the tests side by side, so a test that puts values in force
/// (`Enhancements::apply`, photo mode) or asserts the defaults holds this
/// guard: one such test at a time, the native defaults back when it ends
/// (also on a panic).
#[cfg(test)]
pub(crate) fn test_globals() -> TestGlobals {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TestGlobals(LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

#[cfg(test)]
pub(crate) struct TestGlobals(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

#[cfg(test)]
impl Drop for TestGlobals {
    fn drop(&mut self) {
        Enhancements::default().apply();
        crate::render::blur_dof::set_photo_mode(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The GPU class from the adapter; the presets follow it (High = the
    /// presets as measured before the GPU classes), a class never touches Original or Clean,
    /// never turns a row on that the High preset has off, and every class's
    /// preset is recognised as that preset (`Enhancements::preset` re-applies it).
    #[test]
    fn presets_follow_the_gpu_class() {
        use wgpu::DeviceType as D;
        assert_eq!(classify_gpu(D::IntegratedGpu, 0), GpuClass::Integrated);
        assert_eq!(classify_gpu(D::Cpu, 64 << 30), GpuClass::Integrated);
        assert_eq!(classify_gpu(D::DiscreteGpu, 16 << 30), GpuClass::High);
        assert_eq!(classify_gpu(D::DiscreteGpu, 12 << 30), GpuClass::High);
        assert_eq!(classify_gpu(D::DiscreteGpu, 8 << 30), GpuClass::Mid);
        assert_eq!(classify_gpu(D::DiscreteGpu, 0), GpuClass::High, "unknown memory: as before");
        assert_eq!(gpu_class(), GpuClass::High, "the default until a GPU is made");
        let of = |p: Preset, c: GpuClass| {
            let mut e = Enhancements::default();
            p.apply_for(c, &mut e);
            e
        };
        for p in PRESETS {
            let high = of(p, GpuClass::High);
            let mut via_apply = Enhancements::default();
            p.apply(&mut via_apply);
            assert_eq!(high, via_apply, "{p:?}: apply is the High class here");
            for c in [GpuClass::Mid, GpuClass::Integrated] {
                let e = of(p, c);
                if matches!(p, Preset::Original | Preset::Clean) {
                    assert_eq!(e, high, "{p:?} {c:?}");
                }
                // Steps down only.
                assert!(
                    e.volumetric_fog <= high.volumetric_fog
                        && e.particles <= high.particles
                        && e.rain_detail <= high.rain_detail
                );
                assert!(
                    e.water <= high.water
                        && e.wet_interaction <= high.wet_interaction
                        && e.modern_lighting <= high.modern_lighting
                );
                assert!(e.headlight_beams == high.headlight_beams, "beams stay (Off is slower on the iGPU)");
                assert!(ssaa_scale(e.post_aa).is_none() || ssaa_scale(high.post_aa).is_some());
            }
        }
        // The steps themselves.
        assert_eq!(of(Preset::Ultra, GpuClass::Mid), of(Preset::Ultra, GpuClass::High), "Mid: no measured step yet");
        let i = of(Preset::Ultra, GpuClass::Integrated);
        assert!(
            ssaa_scale(i.post_aa).is_none()
                && i.volumetric_fog == 0
                && i.particles == 0
                && i.rain_detail == 1
                && i.hdr_post == 1
        );
        assert!(of(Preset::Ultra, GpuClass::High).enhanced_glass, "Enhanced Glass is in Ultra");
        assert!(!of(Preset::Ultra, GpuClass::Integrated).enhanced_glass, "not on an integrated GPU");
        assert_eq!(
            of(Preset::Modern, GpuClass::Integrated),
            of(Preset::Modern, GpuClass::High),
            "Modern's rows are already the integrated ladder"
        );
        // No Next-Gen tier in any preset (the look rules: manual), Ambient Detail in none.
        for p in PRESETS {
            for c in [GpuClass::High, GpuClass::Mid, GpuClass::Integrated] {
                let e = of(p, c);
                assert!(
                    e.volumetric_fog < 3
                        && e.sky == 0
                        && e.rain_detail < RAIN_NEXT_GEN
                        && e.headlight_beams < 2
                        && e.sparks_fire == 0
                        && e.ambient_detail == 0,
                    "{p:?} {c:?}"
                );
            }
        }
    }

    #[test]
    fn enhanced_reflections_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.enhanced_reflections);
        assert!(e.parse("enh.enhanced_reflections", "1"));
        assert!(e.enhanced_reflections);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.enhanced_reflections 1\n");
        assert_eq!(e.deviations(), ["enhanced reflections"]);
        assert!(e.parse("enh.enhanced_reflections", "0"));
        assert!(e.is_native());
    }

    #[test]
    fn enhanced_paint_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.enhanced_paint);
        assert!(e.parse("enh.enhanced_paint", "1"));
        assert!(e.enhanced_paint);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.enhanced_paint 1\n");
        assert_eq!(e.deviations(), ["enhanced paint"]);
        let mut back = Enhancements::default();
        let (k, v) = out.trim().split_once(' ').unwrap();
        assert!(back.parse(k, v));
        assert_eq!(back, e);
        assert!(e.parse("enh.enhanced_paint", "0"));
        assert!(e.is_native());
    }

    #[test]
    fn hud_full_width_round_trip_kept_by_presets() {
        let mut e = Enhancements::default();
        assert!(!e.hud_full_width);
        assert!(e.parse("enh.hud_full_width", "1"));
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.hud_full_width 1\n");
        assert_eq!(e.deviations(), ["HUD full width"]);
        let mut back = Enhancements::default();
        let (k, v) = out.trim().split_once(' ').unwrap();
        assert!(back.parse(k, v));
        assert_eq!(back, e);
        // A layout choice, not a graphics value: presets keep it.
        Preset::Ultra.apply(&mut e);
        assert!(e.hud_full_width);
        assert!(e.parse("enh.hud_full_width", "0"));
        assert!(!e.hud_full_width);
    }

    #[test]
    fn lists_open_installed_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.lists_open_installed);
        assert!(e.parse("enh.lists_open_installed", "1"));
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.lists_open_installed 1\n");
        assert_eq!(e.deviations(), ["lists open on installed"]);
        let mut back = Enhancements::default();
        let (k, v) = out.trim().split_once(' ').unwrap();
        assert!(back.parse(k, v));
        assert_eq!(back, e);
        assert!(e.parse("enh.lists_open_installed", "0"));
        assert!(e.is_native());
    }

    /// Outrun Variety: off by default (native), one line when on, kept
    /// by the graphics presets (a QoL value).
    #[test]
    fn outrun_variety_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.outrun_variety);
        assert!(e.parse("enh.outrun_variety", "1"));
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.outrun_variety 1\n");
        assert_eq!(e.deviations(), ["outrun variety"]);
        Preset::Ultra.apply(&mut e);
        Preset::Original.apply(&mut e);
        assert!(e.outrun_variety);
        assert!(e.parse("enh.outrun_variety", "0"));
        assert!(e.is_native());
    }

    /// Fix Original Model Errors: off by default, one line when on.
    #[test]
    fn fix_models_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.fix_models);
        assert!(e.parse("enh.fix_models", "1"));
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.fix_models 1\n");
        assert_eq!(e.deviations(), ["model fixes"]);
        assert!(e.parse("enh.fix_models", "0"));
        assert!(e.is_native());
    }

    /// Lists Open on Installed and Fix Original Model Errors are
    /// not graphics rows (settings_page::reset_row resets them with the QoL
    /// rows), so every preset keeps them, and a preset's values with them
    /// on still read as that preset.
    #[test]
    fn presets_keep_lists_open_installed_and_fix_models() {
        for p in PRESETS {
            let mut e = Enhancements { lists_open_installed: true, fix_models: true, ..Enhancements::default() };
            Preset::Ultra.apply(&mut e);
            p.apply(&mut e);
            assert!(e.lists_open_installed && e.fix_models, "{p:?}");
            assert!(e.preset().is_some(), "{p:?}: reads as Custom");
        }
        let mut e = Enhancements { lists_open_installed: true, fix_models: true, ..Enhancements::default() };
        Preset::Original.apply(&mut e);
        assert_eq!(e.preset(), Some(Preset::Original));
    }

    #[test]
    fn lamps_and_shadows_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.dynamic_lamps && e.lamp_shadows == 0 && !e.contact_shadows);
        assert!(
            e.parse("enh.dynamic_lamps", "1")
                && e.parse("enh.lamp_shadows", "1")
                && e.parse("enh.contact_shadows", "1")
        );
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.dynamic_lamps 1\nenh.lamp_shadows 1\nenh.contact_shadows 1\n");
        assert_eq!(e.deviations(), ["dynamic lamps", "lamp shadows low", "contact shadows"]);
        assert!(
            e.parse("enh.dynamic_lamps", "0")
                && e.parse("enh.lamp_shadows", "0")
                && e.parse("enh.contact_shadows", "0")
        );
        assert!(e.is_native());
    }

    #[test]
    fn weather_rows_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.wet_car && e.rain_detail == 0 && !e.spray && !e.lens_drops);
        for (k, v) in [("enh.wet_car", "1"), ("enh.rain_detail", "9"), ("enh.spray", "1"), ("enh.lens_drops", "1")] {
            assert!(e.parse(k, v));
        }
        // Next-Gen (3) is the highest value.
        assert_eq!(e.rain_detail, RAIN_NEXT_GEN);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.wet_car 1\nenh.rain_detail 3\nenh.spray 1\nenh.lens_drops 1\n");
        assert!(e.parse("enh.rain_detail", "2"));
        assert_eq!(e.rain_detail, 2);
    }

    #[test]
    fn wet_interaction_round_trips_and_native_default() {
        let mut e = Enhancements::default();
        assert_eq!(e.wet_interaction, 0);
        assert!(e.is_native());
        assert!(e.parse("enh.wet_interaction", "7"));
        assert_eq!(e.wet_interaction, 2);
        assert_eq!(e.deviations(), ["wet road interaction high"]);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.wet_interaction 2\n");
        assert!(e.parse("enh.wet_interaction", "0") && e.is_native());
        let mut u = Enhancements::default();
        Preset::Ultra.apply(&mut u);
        assert_eq!(u.wet_interaction, 1);
        Preset::Original.apply(&mut u);
        assert_eq!(u.wet_interaction, 0);
    }

    #[test]
    fn ray_traced_tier_falls_back_to_ultra_without_ray_queries() {
        let _g = test_globals();
        let mut e = Enhancements::default();
        assert!(e.parse("enh.ssr", "3"));
        e.apply();
        set_rt_support(false);
        assert_eq!((ssr(), rt_reflections()), (2, false));
        set_rt_support(true);
        assert_eq!((ssr(), rt_reflections()), (3, true));
        set_rt_support(false);
        Enhancements::default().apply();
        assert_eq!(ssr(), 0);
    }

    #[test]
    fn wet_roads_and_ssr_round_trip_and_native_default() {
        let mut e = Enhancements::default();
        assert!(!e.wet_roads && e.ssr == 0);
        assert!(e.parse("enh.wet_roads", "1") && e.parse("enh.ssr", "1"));
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.wet_roads 1\nenh.ssr 1\n");
        assert_eq!(e.deviations(), ["wet roads", "SSR half"]);
        assert!(e.parse("enh.wet_roads", "0") && e.parse("enh.ssr", "0"));
        assert!(e.is_native());
    }

    /// HDR Output's values parse to their tiers (0 = Auto) and stay
    /// out of the presets; it is in force only with a signal offered.
    #[test]
    fn hdr_output_values() {
        let _globals = test_globals();
        let mut e = Enhancements::default();
        assert!(
            e.parse("enh.hdr_peak", "900")
                && e.parse("enh.hdr_paper", "190")
                && e.parse("enh.hdr_gamma", "7")
                && e.parse("enh.hdr_output", "1")
        );
        assert_eq!((e.hdr_peak, e.hdr_paper, e.hdr_gamma, e.hdr_output), (1000, 203, 2, true));
        assert!(e.parse("enh.hdr_peak", "0"));
        assert_eq!(e.hdr_peak, 0);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "enh.hdr_output 1\nenh.hdr_paper 203\nenh.hdr_gamma 2\n");
        assert_eq!(e.deviations(), ["HDR output"]);
        // No surface offers a signal in the tests: never in force.
        assert!(!hdr_output_active());
        let c = hdr_calibration();
        assert!(c.paper > 0.0 && c.peak >= c.paper);
    }

    #[test]
    fn enhancements_round_trip() {
        let _globals = test_globals();
        let d = Enhancements::default();
        let mut out = String::new();
        d.write(&mut out);
        assert!(out.is_empty(), "native values write nothing");
        assert!(d.is_native() && d.deviations().is_empty());
        let e = Enhancements {
            fov_offset: -5,
            hud_scale: 120,
            frame_cap: 144,
            durable_saves: true,
            save_backups: 3,
            crash_reports: true,
            render_scale: 150,
            post_aa: 1,
            aa_msaa: 4,
            aa_filter: 4,
            sharpen: 75,
            road_reflection: 2,
            car_reflection: 256,
            detail_distance: 150,
            car_detail: 200,
            hd_ui: true,
            reload_packs: true,
            lod_fade: true,
            keep_loaded: true,
            anisotropy: 8,
            reversed_z: true,
            shadows: 2,
            reflection_rate: 1,
            enhanced_reflections: true,
            ssao: 50,
            hdr_post: 2,
            enhanced_paint: true,
            lists_open_installed: true,
            outrun_variety: true,
            wet_roads: true,
            ssr: 2,
            hud_full_width: true,
            dynamic_lamps: true,
            lamp_shadows: 2,
            contact_shadows: true,
            upscale: 2,
            volumetric_fog: 1,
            sky: 1,
            particles: 1,
            sparks_fire: 1,
            fix_models: true,
            hdr_output: true,
            hdr_peak: 1400,
            hdr_paper: 250,
            hdr_gamma: 1,
            motion_blur: 2,
            depth_of_field: 1,
            photo_mode: true,
            water: 2,
            wet_car: true,
            rain_detail: 2,
            spray: true,
            lens_drops: true,
            wet_interaction: 2,
            headlight_beams: 2,
            light_probes: true,
            enhanced_glass: true,
            surface_detail: 2,
            extras: crate::extras::Extras {
                low_latency: true,
                replay_cameras: true,
                palette: 1,
                hud_minimap: 130,
                hud_speedo: 90,
                hud_info: 115,
                reduced_flashing: true,
                power_saving: 3,
                mark_spot: 2,
            },
            pad: crate::pad::extras::PadOptions {
                layout: 1,
                deadzone: 10,
                trigger_deadzone: 5,
                saturation: 95,
                steer_curve: 150,
                rumble: 70,
                pause_on_disconnect: true,
                safe_bind: true,
            },
            smooth_cars: true,
            far_draw: true,
            traffic_density: 2,
            texture_pack: 2,
            modern_lighting: 3,
            modern_strength: 50,
            boot_loading_min_ms: 1500,
            ambient_detail: 2,
            launcher_skip: true,
            skip_title: 1,
            skip_movies: true,
        };
        let mut out = String::new();
        e.write(&mut out);
        let mut back = Enhancements::default();
        for line in out.lines() {
            let (k, v) = line.split_once(' ').unwrap();
            assert!(back.parse(k, v));
        }
        assert_eq!(back, e);
        assert_eq!(e.deviations().len(), 80);
        assert!(!back.parse("detail", "3"));
        // Volumetric Fog's Next-Gen tier (3) and the Sky row round-trip, clamped.
        let mut n = Enhancements::default();
        assert!(n.parse("enh.volumetric_fog", "9") && n.parse("enh.sky", "5"));
        assert_eq!((n.volumetric_fog, n.sky), (3, 1));
        assert!(
            n.deviations().iter().any(|d| d == "volumetric fog next-gen")
                && n.deviations().iter().any(|d| d == "sky next-gen")
        );
        // The old Vector Fonts key is known and ignored (folded into HD UI
        // Art); it is never written.
        let mut old = Enhancements::default();
        assert!(old.parse("enh.vector_fonts", "1"));
        assert!(old.is_native() && !out.contains("vector_fonts"));
        // Out of range values are held to the rows' ranges.
        let mut c = Enhancements::default();
        c.parse("enh.fov", "90");
        c.parse("enh.hud_scale", "7");
        c.parse("enh.save_backups", "200");
        assert_eq!((c.fov_offset, c.hud_scale, c.save_backups), (30, 50, 10));
        // The game start's minimum loading time, held to 30 s; the
        // presets keep it (no row yet; options.txt only).
        c.parse("enh.boot_loading_min_ms", "999999");
        assert_eq!(c.boot_loading_min_ms, BOOT_LOADING_MAX_MS);
        c.parse("enh.boot_loading_min_ms", "x");
        assert_eq!(c.boot_loading_min_ms, BOOT_LOADING_MAX_MS, "a bad value keeps the old one");
        let mut p = Enhancements { boot_loading_min_ms: 800, ..Enhancements::default() };
        Preset::Ultra.apply(&mut p);
        assert_eq!(p.boot_loading_min_ms, 800);
        // Particles held to Original / High / Ultra.
        c.parse("enh.particles", "9");
        assert_eq!(c.particles, 3);
        assert_eq!(particles(), 0, "Original by default");
        // Animated Water off by default, any non-zero value on.
        assert!(!animated_water(), "off by default");
        c.parse("enh.water", "1");
        assert_eq!(c.water, 1);
        // Realistic; anything above held to it.
        c.parse("enh.water", "2");
        assert_eq!(c.water, 2);
        c.parse("enh.water", "9");
        assert_eq!(c.water, 3);
        let mut out = String::new();
        Enhancements::default().write(&mut out);
        assert!(!out.contains("enh.water"), "off writes nothing");
    }

    /// Old saved values between two tiers load as the nearest tier
    /// (the higher on a tie; an opt-in above native never drops to native)
    /// and tiers write their own stable values.
    #[test]
    fn old_values_map_to_the_nearest_tier() {
        let load = |k: &str, v: &str| {
            let mut e = Enhancements::default();
            assert!(e.parse(k, v));
            e
        };
        for (v, want) in
            [("0", 0), ("10", 25), ("25", 25), ("50", 75), ("60", 75), ("75", 75), ("90", 100), ("100", 100)]
        {
            assert_eq!(load("enh.sharpen", v).sharpen, want, "sharpen {v}");
        }
        for (v, want) in [("25", 25), ("50", 50), ("75", 100), ("100", 100)] {
            assert_eq!(load("enh.ssao", v).ssao, want, "ssao {v}");
        }
        for (v, want) in [("128", 128), ("256", 256), ("512", 1024), ("1024", 1024)] {
            assert_eq!(load("enh.car_reflection", v).car_reflection, want, "car refl {v}");
        }
        for (v, want) in [("100", 100), ("150", 150), ("200", 200), ("300", 200)] {
            assert_eq!(load("enh.detail_distance", v).detail_distance, want, "detail {v}");
        }
        for (v, want) in [("150", 200), ("200", 200), ("300", 200)] {
            assert_eq!(load("enh.car_detail", v).car_detail, want, "car detail {v}");
        }
        assert_eq!((load("enh.shadows", "1").shadows, load("enh.shadows", "2").shadows), (2, 2));
        // Every tier value is stable: written and read back unchanged.
        for v in SHARPEN_STEPS {
            assert_eq!(load("enh.sharpen", &v.to_string()).sharpen, v);
        }
        for v in SSAO_STEPS {
            assert_eq!(load("enh.ssao", &v.to_string()).ssao, v);
        }
        for v in CAR_REFLECTION_STEPS {
            assert_eq!(load("enh.car_reflection", &v.to_string()).car_reflection, v);
        }
        for v in DETAIL_STEPS {
            assert_eq!(load("enh.detail_distance", &v.to_string()).detail_distance, v);
        }
    }

    #[test]
    fn graphics_values_held_to_their_rows() {
        let mut c = Enhancements::default();
        c.parse("enh.render_scale", "400");
        c.parse("enh.post_aa", "99");
        c.parse("enh.sharpen", "300");
        c.parse("enh.road_reflection", "9");
        c.parse("enh.car_reflection", "300");
        // 300 px: 512 by its power of two, which ties 256 / 1024 → 1024.
        assert_eq!(
            (c.render_scale, c.post_aa, c.sharpen, c.road_reflection, c.car_reflection),
            (200, 11, 100, 2, 1024)
        );
        c.parse("enh.car_reflection", "5000");
        assert_eq!(c.car_reflection, 1024);
        c.parse("enh.car_reflection", "1");
        assert_eq!(c.car_reflection, 128);
        c.parse("enh.detail_distance", "50");
        c.parse("enh.car_detail", "1000");
        assert_eq!((c.detail_distance, c.car_detail), (100, 200));
        c.parse("enh.anisotropy", "3");
        c.parse("enh.shadows", "9");
        c.parse("enh.reflection_rate", "4");
        c.parse("enh.ssao", "250");
        assert_eq!((c.anisotropy, c.shadows, c.reflection_rate, c.ssao), (4, 2, 1, 100));
        c.parse("enh.ssr", "9");
        assert_eq!(c.ssr, 3);
        c.parse("enh.lamp_shadows", "7");
        assert_eq!(c.lamp_shadows, 2);
        c.parse("enh.hdr_post", "7");
        assert_eq!(c.hdr_post, 2);
        c.parse("enh.anisotropy", "1");
        c.parse("enh.post_aa", "4");
        assert_eq!((c.anisotropy, c.post_aa), (0, 4));
    }

    #[test]
    fn graphics_defaults_are_native() {
        let d = Enhancements::default();
        assert_eq!(
            (d.post_aa, d.anisotropy, d.reversed_z, d.shadows, d.reflection_rate, d.ssao, d.hdr_post),
            (0, 0, false, 0, 0, 0, 0)
        );
        assert_eq!(aa_name(0), None);
        assert_eq!(
            AA_STEPS.map(msaa_for),
            [None, None, None, Some(2), Some(4), Some(8), Some(1), Some(1), Some(1), Some(1), Some(1), Some(1)]
        );
    }

    #[test]
    fn lod_fade_and_keep_loaded_default_native() {
        // Both off by default, written only when on, read back.
        let d = Enhancements::default();
        assert!(!d.lod_fade && !d.keep_loaded);
        let mut c = Enhancements::default();
        assert!(c.parse("enh.lod_fade", "1") && c.parse("enh.keep_loaded", "1"));
        assert!(c.lod_fade && c.keep_loaded && !c.is_native());
        let mut out = String::new();
        c.write(&mut out);
        assert_eq!(out, "enh.lod_fade 1\nenh.keep_loaded 1\n");
        assert_eq!(c.deviations(), ["LOD fade", "keep all loaded"]);
        c.parse("enh.lod_fade", "0");
        c.parse("enh.keep_loaded", "0");
        assert!(c.is_native());
    }

    #[test]
    fn render_scale_caps() {
        // Downscaling is never capped; supersampling stays within the
        // largest texture and the pixel budgets, never below 1.
        assert_eq!(scale_for(50, (1920, 1080), 8, 8192, false, false), 0.5);
        assert_eq!(scale_for(200, (1920, 1080), 1, 8192, false, false), 2.0);
        let s = scale_for(200, (3840, 2160), 1, 8192, false, false);
        assert!((s - 2f64.sqrt()).abs() < 1e-9, "{s}");
        assert_eq!(scale_for(200, (3840, 2160), 8, 8192, false, false), 1.0);
        assert_eq!(scale_for(200, (1920, 1080), 1, 16384, false, true), 1.0);
        assert_eq!(scale_for(200, (3840, 2160), 1, 3840, false, false), 1.0);
        // Road reflection sizes: native, half and whole, never below native.
        assert_eq!(road_reflection_size((320, 240), (0, 0)), (320, 240));
    }

    #[test]
    fn steps_wrap() {
        assert_eq!(step_in(&FOV_STEPS, 0, true), 5);
        assert_eq!(step_in(&FOV_STEPS, 30, true), -10);
        assert_eq!(step_in(&FOV_STEPS, -10, false), 30);
        assert_eq!(step_in(&CAP_STEPS, 100, true), 120);
        assert_eq!(step_in(&CAP_STEPS, 100, false), 90);
    }

    #[test]
    fn hud_scale_keeps_corners() {
        let q = |a: (f32, f32), b: (f32, f32)| Quad {
            texture: None,
            rect: [glam::Vec2::new(a.0, a.1), glam::Vec2::new(b.0, b.1)],
            uv: [glam::Vec2::ZERO, glam::Vec2::ONE],
            color: [1.0; 4],
            z: 0.0,
            angle: 0.0,
            place: Default::default(),
        };
        // A corner gauge, a top-centre label, a panel crossing the first
        // third's edge with its text, and a full-canvas flash.
        let mut v = vec![
            q((600.0, 440.0), (640.0, 480.0)),
            q((300.0, 0.0), (340.0, 20.0)),
            q((60.0, 80.0), (380.0, 280.0)),
            q((180.0, 100.0), (370.0, 120.0)),
            q((0.0, 0.0), (640.0, 480.0)),
        ];
        // Their anchors (`display::Place`, as the HUD's packages set them).
        v[0].place = crate::render::display::Place::new(1, 1);
        v[1].place = crate::render::display::Place::TOP_CENTRE;
        let before = v.clone();
        let rects = |v: &[Quad]| v.iter().map(|q| q.rect).collect::<Vec<_>>();
        scale_hud(&mut v, 1.0);
        assert_eq!(rects(&v), rects(&before));
        scale_hud(&mut v, 0.5);
        assert_eq!(v[0].rect, [glam::Vec2::new(620.0, 460.0), glam::Vec2::new(640.0, 480.0)]);
        assert_eq!(v[1].rect, [glam::Vec2::new(310.0, 0.0), glam::Vec2::new(330.0, 10.0)]);
        // The panel and its text (one centred part, scaled about its own
        // middle) move together.
        let (panel, text) = (v[2].rect, v[3].rect);
        assert_eq!(text[0] - panel[0], (glam::Vec2::new(180.0, 100.0) - glam::Vec2::new(60.0, 80.0)) * 0.5);
        assert_eq!(v[4].rect, before[4].rect, "a full-canvas quad stays");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nfsu2-qol-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn atomic_write_replaces_whole_file() {
        let d = scratch("atomic");
        let path = d.join("NAME").join("NAME");
        for durable in [false, true] {
            saves::write_atomic(&path, b"first version", durable).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), b"first version");
            saves::write_atomic(&path, b"second", durable).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), b"second");
            // No temporary file is left.
            let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
            assert_eq!(entries.len(), 1);
        }
        // A failed rename (the target is a folder) keeps what was there
        // and leaves no temporary file.
        let blocked = d.join("B").join("B");
        std::fs::create_dir_all(blocked.join("x")).unwrap();
        assert!(saves::write_atomic(&blocked, b"data", true).is_err());
        assert!(blocked.is_dir() && !blocked.with_extension("tmp").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn backups_roll() {
        let d = scratch("backups");
        let profile = d.join("profiles").join("NAME").join("NAME");
        let store = d.join("backups").join("NAME");
        // Nothing to back up yet, or backups off.
        assert!(saves::backup(&profile, &store, 3).unwrap().is_none());
        saves::write_atomic(&profile, b"v0", false).unwrap();
        assert!(saves::backup(&profile, &store, 0).unwrap().is_none());
        for i in 1..=5u8 {
            let copy = saves::backup(&profile, &store, 3).unwrap().unwrap();
            assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&profile).unwrap(), "a backup is the file's bytes");
            saves::write_atomic(&profile, format!("v{i}").as_bytes(), false).unwrap();
        }
        let kept = saves::list(&store, "NAME");
        assert_eq!(kept.len(), 3);
        let contents: Vec<Vec<u8>> = kept.iter().map(|p| std::fs::read(p).unwrap()).collect();
        assert_eq!(contents, vec![b"v2".to_vec(), b"v3".to_vec(), b"v4".to_vec()], "the newest three, oldest first");
        // The profile's folder holds the profile alone.
        assert_eq!(std::fs::read_dir(profile.parent().unwrap()).unwrap().count(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A profile written the durable way, and its backup, are the
    /// original's own files byte for byte (the fixtures SPEED2.EXE wrote),
    /// so the original still loads them.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn saves_keep_the_native_format() {
        let d = scratch("native");
        for f in ["career_start.sav", "garage.sav", "options.sav"] {
            let original = crate::testkit::private(&format!("tests/fixtures/native_saves/{f}"));
            let save = crate::formats::native_save::NativeSave::parse(&original).unwrap();
            let path = d.join("NAME").join("NAME");
            saves::write_atomic(&path, &save.to_bytes(), true).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), original, "{f}");
            let copy = saves::backup(&path, &d.join("backups"), 2).unwrap().unwrap();
            assert_eq!(std::fs::read(&copy).unwrap(), original, "{f} backup");
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn timestamps() {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_793_450_096);
        assert_eq!(saves::timestamp(t), "20261031-123456");
        assert_eq!(saves::timestamp(std::time::UNIX_EPOCH), "19700101-000000");
    }

    #[test]
    fn setup_check() {
        let d = scratch("setup");
        let c = setup::check(&d, true);
        assert_eq!(c.missing.len(), setup::KEY_FILES.len());
        assert!(!c.ok() && !setup::looks_like_game(&d));
        assert!(setup::help(Some(&d), Some(&c)).contains("GLOBAL/GLOBALB.BUN"));
        if let Some(root) = std::env::var_os("NFSU2_ROOT").map(std::path::PathBuf::from) {
            let c = setup::check(&root, true);
            assert!(c.ok() && c.different.is_empty(), "{c:?}");
            assert!(setup::looks_like_game(&root));
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// L1: the language list of a LANGUAGES folder (any case; the shared
    /// tables left out), SPEED2.EXE found in any case, the shared check
    /// wording, and the kept texture-pack folder (in a sandboxed data
    /// folder).
    #[test]
    fn launcher_prerequisites() {
        let d = scratch("launcher-l1");
        std::fs::create_dir_all(d.join("languages")).unwrap();
        for f in ["English.bin", "french.BIN", "Labels.bin", "Largest.bin", "LanguageTextures.bin", "agree.eng", "German.bin"] {
            std::fs::write(d.join("languages").join(f), b"x").unwrap();
        }
        assert_eq!(setup::installed_languages(&d), ["English", "French", "German"]);
        assert!(setup::installed_languages(&d.join("nope")).is_empty());
        std::fs::write(d.join("speed2.exe"), b"x").unwrap();
        assert_eq!(setup::key_file_path(&d, "SPEED2.EXE"), d.join("speed2.exe"));
        let c = setup::check(&d, false);
        assert!(!c.missing.iter().any(|m| m == "SPEED2.EXE") && c.different.iter().any(|m| m == "SPEED2.EXE"));
        assert_eq!(setup::KEY_FILES[0].0, "SPEED2.EXE");
        // The wording: one line per missing file, the differing ones named.
        let lines = setup::check_lines(&c, None);
        assert_eq!(lines.iter().filter(|(m, _)| *m == setup::Mark::Error).count(), 1, "many missing: one line");
        assert!(lines.iter().any(|(_, t)| t.contains(&format!("{} of the {} files", c.missing.len(), setup::KEY_FILES.len()))));
        let few = setup::Check { missing: vec!["TRACKS/L4RA.BUN".into()], different: Vec::new() };
        assert_eq!(setup::check_lines(&few, None), [(setup::Mark::Error, "Missing: TRACKS/L4RA.BUN — reinstall or copy the full game folder".to_string())]);
        // (The folder check finds every key file in any case, so the dummy `languages/English.bin` differs too.)
        assert!(lines.iter().any(|(m, t)| *m == setup::Mark::Warn && t.contains("2 files differ") && t.contains("SPEED2.EXE") && t.contains("English.bin")));
        let fine = setup::Check::default();
        let lines = setup::check_lines(&fine, None);
        assert_eq!(lines[0].0, setup::Mark::Ok);
        assert!(lines.iter().any(|(_, t)| t == &format!("All {} files Overbright needs are present", setup::KEY_FILES.len())));
        crate::frontend::profiles::sandbox::enter(&d);
        assert_eq!(setup::saved_texture_pack(), None);
        setup::save_texture_pack(Some(&d.join("pack"))).unwrap();
        assert_eq!(setup::saved_texture_pack(), Some(d.join("pack")));
        assert!(setup::saved_texture_pack_file().starts_with(&d));
        setup::save_texture_pack(None).unwrap();
        setup::save_texture_pack(None).unwrap();
        assert_eq!(setup::saved_texture_pack(), None);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// L1: `launcher.skip` is a declared row: read, written (off writes
    /// nothing), kept by the presets and by Restore Native, and kept by a
    /// rewrite of options.txt.
    #[test]
    fn launcher_skip_row() {
        let mut e = Enhancements::default();
        assert!(!e.launcher_skip);
        assert!(e.parse("launcher.skip", "1") && e.launcher_skip);
        let mut out = String::new();
        e.write(&mut out);
        assert_eq!(out, "launcher.skip 1\n");
        assert!(e.deviations().is_empty(), "not a deviation of the game");
        for p in PRESETS {
            let mut x = e;
            p.apply_for(GpuClass::High, &mut x);
            assert!(x.launcher_skip, "{p:?}");
        }
        let mut s = crate::frontend::options::Settings::from_text("resolution 1280x960\nlauncher.skip 1\n");
        assert!(s.enh.launcher_skip);
        let text = s.options_text();
        assert!(text.lines().any(|l| l == "launcher.skip 1"), "{text}");
        assert!(crate::frontend::options::Settings::from_text(&text).enh.launcher_skip);
        s.restore_native();
        assert!(s.enh.launcher_skip);
    }

    #[test]
    /// FSR 2's modes are Upscaling values 7–11: kept by the parser,
    /// on the temporal path with one sample, named, at FSR 2's render
    /// sizes; FSR 1 and TAAU keep theirs.
    fn fsr2_modes() {
        let mut e = Enhancements::default();
        e.parse("enh.upscale", "11");
        assert_eq!(e.upscale, 11);
        e.parse("enh.upscale", "99");
        assert_eq!(e.upscale, UPSCALE_MAX);
        e.upscale = 8;
        e.post_aa = 6;
        assert!(is_fsr2(8) && !is_fsr2(4) && !is_fsr2(1) && !is_fsr(8));
        assert!(temporal_for(&e) && is_temporal_upscale(7) && !is_temporal_upscale(5));
        assert_eq!(msaa_choice(&e), Some(1));
        assert_eq!(upscale_name(7), Some("FSR 2 Native AA"));
        assert_eq!(upscale_name(11), Some("FSR 2 Ultra Performance"));
        assert_eq!(upscale_share(10), Some(0.5));
        assert_eq!(upscale_share(1), Some(0.67));
        assert_eq!(UPSCALE_STEPS.len(), usize::from(UPSCALE_MAX) + 1);
    }

    /// TAA / Upscaling run the scene at one sample where the temporal
    /// pass runs; the front end and garage keep the native MSAA (the AA
    /// row's MSAA Override does not apply under TAA either).
    #[test]
    fn temporal_msaa_falls_back_to_native_outside_the_pass() {
        assert_eq!(msaa_in_frame(true, true, TAA, 8), Some(1));
        assert_eq!(msaa_in_frame(true, false, TAA, 8), None);
        assert_eq!(msaa_in_frame(true, false, 5, 0), None);
        assert_eq!(msaa_in_frame(false, false, 5, 0), Some(8));
        assert_eq!(msaa_in_frame(false, false, 0, 0), None);
    }
}
