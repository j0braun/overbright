//! Left / right on an option row.

use super::*;

/// Whether World Texture Pack can be switched: a generated or an imported
/// pack exists and none is being made (`NFSU2_WORLD_TEXTURE_PACK`
/// set: always, its folders come first).
pub(crate) fn texture_pack_available() -> bool {
    std::env::var_os("NFSU2_WORLD_TEXTURE_PACK").is_some_and(|v| !v.is_empty())
        || ((crate::upscale_pack::generated() || crate::upscale_pack::imported())
            && !crate::upscale_pack::job_running())
}

pub(crate) fn map_label(mode: u8) -> u32 {
    [OFF, 0xF755_95F2, 0xF4B0_0E99][mode.min(2) as usize]
}

/// Step a value in 0..=top: right adds one and wraps to 0, left takes one
/// and wraps to `top` (the item handlers 0x49AF20, 0x49B0B0, 0x4B7750…).
fn step(v: u8, top: u8, right: bool) -> u8 {
    if right {
        if v >= top { 0 } else { v + 1 }
    } else if v == 0 {
        top
    } else {
        v - 1
    }
}

/// Apply left (`right` false) or right to a row (each item's message
/// handler, slot 1). Sliders move by their step within their range
/// (0x4FF480): the volumes by 0.05, the detail by 1. `has_profile` gates
/// Autosave (0x502FE0: profile +0x20).
/// The size after `at` in `list` (sorted) toward `right`, no wrap; a size
/// not listed moves to the nearest listed one that way.
pub fn step_mode(list: &[(u32, u32)], at: (u32, u32), right: bool) -> (u32, u32) {
    if right {
        list.iter().copied().find(|&m| m > at).unwrap_or(at)
    } else {
        list.iter().rev().copied().find(|&m| m < at).unwrap_or(at)
    }
}

pub fn change(id: RowId, s: &mut Settings, player: usize, right: bool, has_profile: bool) {
    let slide = |v: &mut f32, step_by: f32, min: f32, max: f32| {
        *v = (*v + if right { step_by } else { -step_by }).clamp(min, max);
    };
    let p = &mut s.players[player.min(1)];
    match id {
        RowId::Master => slide(&mut s.master, 0.05, 0.0, 1.0),
        RowId::MenuMusic => slide(&mut s.menu_music, 0.05, 0.0, 1.0),
        RowId::GameMusic => slide(&mut s.game_music, 0.05, 0.0, 1.0),
        RowId::Speech => slide(&mut s.speech, 0.05, 0.0, 1.0),
        RowId::Effects => slide(&mut s.effects, 0.05, 0.0, 1.0),
        RowId::Tires => slide(&mut s.tires, 0.05, 0.0, 1.0),
        RowId::Engine => slide(&mut s.engine, 0.05, 0.0, 1.0),
        RowId::AudioMode => s.stereo = !s.stereo,
        // 0x49B600: the next available mode either way, no wrap (the game
        // skips the sizes its adapter lacks, 0x86E880; the clone's list
        // holds only the sizes the active path can output,
        // `display::modes_on`).
        RowId::Resolution => s.resolution = step_mode(&crate::render::display::modes_on(s.fullscreen), s.resolution, right),
        // The size moves to one the new path can output (full
        // screen on Wayland: the desktop's).
        RowId::ScreenMode => {
            s.fullscreen = !s.fullscreen;
            s.resolution = crate::render::display::fit_mode(&crate::render::display::modes_on(s.fullscreen), s.resolution);
        }
        RowId::AspectRatio => s.aspect = 1 - s.aspect.min(1),
        RowId::VsyncOff => s.vsync_off_mailbox = !s.vsync_off_mailbox,
        RowId::Detail => {
            // The slider shows a stored 0 as 1 (0x4FF550 clamps the shown
            // value to [1, 5], 0x87099C stays). 0x4FF480 steps the shown
            // value within [1, 5] and reports "not changed" at an end stop;
            // only a change writes the detail and runs 0x49B76F → 0x5BEA00
            // (the custom values follow). So Left at 1 (or at 0) and Right
            // at 5 do nothing, and Right from 0 gives 2.
            let shown = s.detail.max(1);
            let new = if right { (shown + 1).min(5) } else { (shown - 1).max(1) };
            if new != shown {
                s.detail = new;
                s.custom = custom_presets(s.detail);
                s.custom_raised = [None; 22];
                // An enhanced Graphics tier keeps its value at the top.
                super::graphics_table::normalize(s);
            }
        }
        RowId::JumpCamera => s.jump_camera = !s.jump_camera,
        RowId::Rearview => s.rearview = !s.rearview,
        RowId::Units => s.metric = !s.metric,
        RowId::Difficulty => s.difficulty = step(s.difficulty, 2, right),
        RowId::ExploreMap => s.explore_map = step(s.explore_map, 2, right),
        RowId::RaceMap => s.race_map = step(s.race_map, 2, right),
        // 0x49B170: right goes Autosave → Prompt → Off → Autosave (Prompt
        // without a profile); left Autosave → Off → Prompt → Autosave (Off
        // without a profile).
        RowId::Save => {
            let (prompts, auto) = match (s.save_prompts, s.autosave, right) {
                (_, true, true) => (true, false),
                (true, false, true) => (false, false),
                (false, false, true) => {
                    if has_profile {
                        (true, true)
                    } else {
                        (true, false)
                    }
                }
                (_, true, false) => (false, false),
                (false, false, false) => (true, false),
                (true, false, false) => {
                    if has_profile {
                        (true, true)
                    } else {
                        (false, false)
                    }
                }
            };
            s.save_prompts = prompts;
            s.autosave = auto;
        }
        RowId::Transmission => p.manual = !p.manual,
        RowId::Stability => p.stability_off = !p.stability_off,
        // 0x4B7750: 0–3 either way (Super Far only from the camera button).
        RowId::Camera => p.camera = step(p.camera.min(3), 3, right),
        RowId::Gauges => p.gauges = !p.gauges,
        RowId::Position => p.position = !p.position,
        RowId::LapInfo => p.lap_info = !p.lap_info,
        RowId::SplitTime => p.split = step(p.split, 4, right),
        RowId::Score => p.score = !p.score,
        RowId::Leaderboard => p.leaderboard = !p.leaderboard,
        RowId::Feedback => p.feedback = !p.feedback,
        RowId::AskTransmission => p.ask_transmission = !p.ask_transmission,
        RowId::CustomizeDisplay
        | RowId::DefaultDisplay
        | RowId::WheelOptions
        | RowId::Enhancements
        | RowId::Graphics
        | RowId::GraphicsGroup(_)
        | RowId::HdUiPage
        | RowId::HdUiPackStatus
        | RowId::HdUiActivationStatus
        | RowId::RestoreNative
        | RowId::GenerateTexturePack => {}
        // Handled by the screen (session state, not settings).
        RowId::UxCompare | RowId::UxFilter | RowId::UxGroup(_) | RowId::UxReset(_) | RowId::UxNoMatch => {}
        // The preset row: the next / previous preset (from Custom, the
        // first / last).
        // The presets of the settings page, then its 60 FPS entry.
        RowId::GraphicsPreset => super::settings_page::step_preset(s, right),
        // The streamlined graphics page's rows (settings_page::main_page).
        RowId::GraphicsMode
        | RowId::EnhancedQuality
        | RowId::MainUpscale
        | RowId::MainAa
        | RowId::RayTracing
        | RowId::MainMotionBlur => super::settings_page::step_main(id, s, right),
        RowId::AdvancedPage => {}
        RowId::FovOffset => s.enh.fov_offset = crate::qol::step_in(&crate::qol::FOV_STEPS, s.enh.fov_offset, right),
        RowId::HudScale => s.enh.hud_scale = crate::qol::step_in(&crate::qol::HUD_STEPS, s.enh.hud_scale, right),
        RowId::HudLayout => s.enh.hud_full_width = !s.enh.hud_full_width,
        RowId::FrameCap => s.enh.frame_cap = crate::qol::step_in(&crate::qol::CAP_STEPS, s.enh.frame_cap, right),
        RowId::DurableSaves => s.enh.durable_saves = !s.enh.durable_saves,
        RowId::SaveBackups => {
            s.enh.save_backups = crate::qol::step_in(&crate::qol::BACKUP_STEPS, s.enh.save_backups, right)
        }
        RowId::CrashReports => s.enh.crash_reports = !s.enh.crash_reports,
        RowId::ListsOpenInstalled => s.enh.lists_open_installed = !s.enh.lists_open_installed,
        RowId::OutrunVariety => s.enh.outrun_variety = !s.enh.outrun_variety,
        RowId::FixModels => s.enh.fix_models = !s.enh.fix_models,
        RowId::TrafficDensity => {
            s.enh.traffic_density =
                crate::qol::step_in(&crate::qol::TRAFFIC_DENSITY_STEPS, s.enh.traffic_density, right)
        }
        RowId::PhotoMode => s.enh.photo_mode = !s.enh.photo_mode,
        RowId::SkipTitle => {
            s.enh.skip_title = crate::qol::step_in(&crate::qol::SKIP_TITLE_STEPS, s.enh.skip_title, right)
        }
        RowId::LowLatency => s.enh.extras.low_latency = !s.enh.extras.low_latency,
        RowId::MarkSpot => {
            let n = crate::mark_spot::KEYS.len() as u8;
            s.enh.extras.mark_spot =
                if right { (s.enh.extras.mark_spot + 1) % n } else { (s.enh.extras.mark_spot + n - 1) % n }
        }
        RowId::PowerSaving => {
            s.enh.extras.power_saving =
                if right { (s.enh.extras.power_saving + 1) % 4 } else { (s.enh.extras.power_saving + 3) % 4 }
        }
        RowId::ReplayCameras => s.enh.extras.replay_cameras = !s.enh.extras.replay_cameras,
        RowId::HudPalette => {
            s.enh.extras.palette = if right { (s.enh.extras.palette + 1) % 4 } else { (s.enh.extras.palette + 3) % 4 }
        }
        RowId::HudMinimap => {
            s.enh.extras.hud_minimap =
                crate::extras::step(&crate::extras::ELEMENT_STEPS, s.enh.extras.hud_minimap, right)
        }
        RowId::HudSpeedo => {
            s.enh.extras.hud_speedo = crate::extras::step(&crate::extras::ELEMENT_STEPS, s.enh.extras.hud_speedo, right)
        }
        RowId::HudInfo => {
            s.enh.extras.hud_info = crate::extras::step(&crate::extras::ELEMENT_STEPS, s.enh.extras.hud_info, right)
        }
        RowId::ReducedFlashing => s.enh.extras.reduced_flashing = !s.enh.extras.reduced_flashing,
        RowId::PadLayout => {
            s.enh.pad.layout = crate::pad::extras::cycle(&crate::pad::extras::LAYOUT_STEPS, s.enh.pad.layout, right)
        }
        RowId::PadDeadzone => {
            s.enh.pad.deadzone =
                crate::pad::extras::cycle(&crate::pad::extras::DEADZONE_STEPS, s.enh.pad.deadzone, right)
        }
        RowId::PadTriggerDeadzone => {
            s.enh.pad.trigger_deadzone = crate::pad::extras::cycle(
                &crate::pad::extras::TRIGGER_DEADZONE_STEPS,
                s.enh.pad.trigger_deadzone,
                right,
            )
        }
        RowId::PadSaturation => {
            s.enh.pad.saturation =
                crate::pad::extras::cycle(&crate::pad::extras::SATURATION_STEPS, s.enh.pad.saturation, right)
        }
        RowId::PadSteerCurve => {
            s.enh.pad.steer_curve =
                crate::pad::extras::cycle(&crate::pad::extras::STEER_CURVE_STEPS, s.enh.pad.steer_curve, right)
        }
        RowId::PadRumble => {
            s.enh.pad.rumble = crate::pad::extras::cycle(&crate::pad::extras::RUMBLE_STEPS, s.enh.pad.rumble, right)
        }
        RowId::PadPauseOnDisconnect => s.enh.pad.pause_on_disconnect = !s.enh.pad.pause_on_disconnect,
        RowId::PadSafeBind => s.enh.pad.safe_bind = !s.enh.pad.safe_bind,
        RowId::PerfOverlay => {
            s.overlay.layout = if right { (s.overlay.layout + 1) % 4 } else { (s.overlay.layout + 3) % 4 }
        }
        RowId::OverlayPosition => {
            s.overlay.position = if right { (s.overlay.position + 1) % 4 } else { (s.overlay.position + 3) % 4 }
        }
        RowId::OverlayScale => s.overlay.step_scale(right),
        RowId::OverlayLog => s.overlay.csv = !s.overlay.csv,
        RowId::HdUi => s.enh.hd_ui = !s.enh.hd_ui,
        RowId::ReloadPacks => s.enh.reload_packs = !s.enh.reload_packs,
        // The Graphics group rows: one tier left or right among the
        // offered ones, wrapping (`graphics_table::step`).
        RowId::RenderScale
        | RowId::PostAa
        | RowId::AaMsaa
        | RowId::AaFilter
        | RowId::Sharpen
        | RowId::RoadReflection
        | RowId::CarReflection
        | RowId::DetailDistance
        | RowId::CarDetail
        | RowId::LodFade
        | RowId::KeepLoaded
        | RowId::TextureFiltering
        | RowId::ReversedZ
        | RowId::ShadowQuality
        | RowId::ReflectionRate
        | RowId::EnhancedReflections
        | RowId::EnhancedPaint
        | RowId::SmoothCars
        | RowId::Ssao
        | RowId::HdrPost
        | RowId::WetRoads
        | RowId::DynamicLamps
        | RowId::LampShadows
        | RowId::ContactShadows
        | RowId::Upscale
        | RowId::VolumetricFog
        | RowId::Sky
        | RowId::Particles
        | RowId::SparksFire
        | RowId::WetCar
        | RowId::RainDetail
        | RowId::Spray
        | RowId::LensDrops
        | RowId::WetInteraction
        | RowId::MotionBlur
        | RowId::DepthOfField
        | RowId::AnimatedWater
        | RowId::AmbientDetail
        | RowId::HeadlightBeams
        | RowId::LightProbes
        | RowId::EnhancedGlass
        | RowId::SurfaceDetail
        | RowId::TexturePack
        | RowId::ModernLighting
        | RowId::ModernStrength
        | RowId::HdrOutput
        | RowId::HdrPeak
        | RowId::HdrPaper
        | RowId::HdrGamma
        | RowId::Ssr => {
            // Render Scale is greyed (fixed by SSAA) while SSAA is on.
            if id == RowId::RenderScale && (crate::qol::ssaa_scale(s.enh.post_aa).is_some() || s.enh.upscale != 0) {
                return;
            }
            // HDR Output's rows are greyed without an HDR signal.
            if matches!(id, RowId::HdrOutput | RowId::HdrPeak | RowId::HdrPaper | RowId::HdrGamma)
                && crate::qol::hdr_output_signal().is_none()
            {
                return;
            }
            if let Some(r) = super::graphics_table::row(id) {
                super::graphics_table::step(r, s, right);
            }
        }
        RowId::Custom(i) => {
            if i == 14 && crate::qol::msaa_choice(&s.enh).is_some() {
                return;
            }
            // The same setting as a Graphics row: while that row is
            // on an enhanced tier, any change here first goes back to the
            // original's top value.
            if let Some(r) = super::graphics_table::for_custom(i as usize).filter(|r| r.enhanced(s)) {
                (r.set)(s, r.native_top());
                return;
            }
            let i = i as usize;
            s.custom_raised[i] = None;
            let v = &mut s.custom[i];
            *v = match CUSTOM_ROWS[i].1 {
                CustomKind::Slider => {
                    if right {
                        (*v + 1).min(CUSTOM_CAPS[i])
                    } else {
                        v.saturating_sub(1)
                    }
                }
                // 0x49BAB0…: toggled only when the cap allows it.
                _ => {
                    if CUSTOM_CAPS[i] > 0 {
                        u8::from(*v == 0)
                    } else {
                        *v
                    }
                }
            };
        }
        // 0x49B350: the slider moves by 0.1 within 0.1–1.
        RowId::WheelRange => {
            p.wheel_range = ((p.wheel_range + if right { 0.1 } else { -0.1 }) * 10.0).round().clamp(1.0, 10.0) / 10.0
        }
        // 0x49B400 with slot 18 (0x49B450): v = 20 − f × 18 for the slider's
        // fraction f.
        RowId::RoadFeedback => {
            let f = 1.0 - (p.road_feedback.clamp(2.0, 20.0) - 2.0) / 18.0;
            let f = ((f + if right { 0.1 } else { -0.1 }) * 10.0).round().clamp(0.0, 10.0) / 10.0;
            p.road_feedback = 20.0 - f * 18.0;
        }
        RowId::ForceStrength => {
            p.force = ((p.force + if right { 0.1 } else { -0.1 }) * 10.0).round().clamp(0.0, 10.0) / 10.0
        }
        RowId::QrDirection
        | RowId::QrLapKo
        | RowId::QrLaps
        | RowId::QrTraffic
        | RowId::QrOpponents
        | RowId::QrDifficulty
        | RowId::QrCatchUp => {}
    }
}
