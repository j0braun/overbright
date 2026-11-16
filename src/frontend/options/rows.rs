//! Option rows: kinds, values, ids and the native pages' row builders.

use crate::frontend::fe_ids;
use super::*;

/// What a row is (0x51E850 sliders, 0x51E240 value rows, 0x51DFD0
/// buttons).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Value, min, max (the slider object +0x24 / +0x28 / +0x30).
    Slider {
        value: f32,
        min: f32,
        max: f32,
    },
    Cycle,
    Button,
}

/// A row as the screen shows it: label, value text and kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: RowId,
    pub label: u32,
    pub kind: Kind,
    /// The value: a language label, or literal text (resolutions).
    pub value: Option<Value>,
    /// Disabled rows run DISABLE and are skipped (0x5390F0).
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Label(u32),
    Text(&'static str),
    /// Text the screen formats (the Quick Race options' "%d").
    Owned(String),
}

/// Each row's option (the item classes 0x7918F8…).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    /// The settings page (docs/settings-page.md): Compare, Show, a
    /// group's row on the hub, a group's reset, the no-match line.
    UxCompare,
    UxFilter,
    UxGroup(super::settings_model::UxGroup),
    UxReset(super::settings_model::UxGroup),
    UxNoMatch,
    Master,
    MenuMusic,
    GameMusic,
    Speech,
    Effects,
    Tires,
    Engine,
    AudioMode,
    Resolution,
    Detail,
    /// The clone's Display rows (agreed deviations).
    ScreenMode,
    /// The Display page's last row: the Enhancements page.
    Enhancements,
    /// The Enhancements page's rows (QoL, clone deviations).
    FovOffset,
    HudScale,
    /// Enhancements "HUD Layout" (ultrawide): Centered / Full Width.
    HudLayout,
    FrameCap,
    DurableSaves,
    SaveBackups,
    CrashReports,
    ListsOpenInstalled,
    OutrunVariety,
    /// Fix Original Model Errors (docs/model-fixes.md).
    FixModels,
    TrafficDensity,
    /// Skip Title Screen (docs/start-and-menus.md).
    SkipTitle,
    SmoothCars,
    /// Photo Mode in Pause Menu (docs/photo-mode.md).
    PhotoMode,
    /// docs/modern-extras.md: Low Latency, Replay Cameras and the
    /// Accessibility rows.
    LowLatency,
    /// Power Saving.
    PowerSaving,
    MarkSpot,
    ReplayCameras,
    HudPalette,
    HudMinimap,
    HudSpeedo,
    HudInfo,
    ReducedFlashing,
    /// docs/controller-support.md: the Controller group.
    PadLayout,
    PadDeadzone,
    PadTriggerDeadzone,
    PadSaturation,
    PadSteerCurve,
    PadRumble,
    PadPauseOnDisconnect,
    PadSafeBind,
    /// docs/perf-overlay.md: the performance overlay's rows.
    PerfOverlay,
    OverlayPosition,
    OverlayScale,
    OverlayLog,
    HdUiPage,
    HdUi,
    HdUiPackStatus,
    HdUiActivationStatus,
    /// docs/pack-hot-reload.md: Reload Packs When Changed.
    ReloadPacks,
    /// The Enhancements page's Graphics row and the Graphics page's rows.
    Graphics,
    RenderScale,
    PostAa,
    AaMsaa,
    AaFilter,
    Sharpen,
    RoadReflection,
    CarReflection,
    DetailDistance,
    CarDetail,
    LodFade,
    KeepLoaded,
    TextureFiltering,
    ReversedZ,
    ShadowQuality,
    ReflectionRate,
    EnhancedReflections,
    EnhancedPaint,
    Ssao,
    HdrPost,
    /// The Graphics page's preset row and group rows.
    GraphicsPreset,
    /// The streamlined graphics page (the settings hub): Graphics Mode,
    /// Enhanced Quality, the main page's Upscaling / Anti-Aliasing / Ray
    /// Tracing / Motion Blur choices and the Advanced page's row.
    GraphicsMode,
    EnhancedQuality,
    MainUpscale,
    MainAa,
    RayTracing,
    MainMotionBlur,
    AdvancedPage,
    GraphicsGroup(GraphicsGroup),
    WetRoads,
    DynamicLamps,
    LampShadows,
    ContactShadows,
    Ssr,
    /// See docs/temporal.md.
    Upscale,
    VolumetricFog,
    /// See docs/atmosphere.md.
    Sky,
    /// See docs/particles-smoke.md.
    Particles,
    SparksFire,
    /// docs/weather.md: the Weather group.
    WetCar,
    RainDetail,
    Spray,
    LensDrops,
    /// See docs/wet-road-car.md.
    WetInteraction,
    /// docs/motionblur-dof.md: Customize Display's Motion Blur and
    /// Depth Of Field with the clone's tiers above them.
    MotionBlur,
    DepthOfField,
    /// See docs/water.md.
    AnimatedWater,
    /// See docs/ambient-detail.md.
    AmbientDetail,
    /// See docs/lighting2.md.
    HeadlightBeams,
    LightProbes,
    EnhancedGlass,
    /// See docs/surface-maps.md.
    SurfaceDetail,
    /// docs/texture-pack-generator.md: World Texture Pack and the
    /// action row that makes it (Image group).
    TexturePack,
    GenerateTexturePack,
    ModernLighting,
    ModernStrength,
    /// docs/hdr.md: HDR Output and its calibration.
    HdrOutput,
    HdrPeak,
    HdrPaper,
    HdrGamma,
    RestoreNative,
    AspectRatio,
    VsyncOff,
    CustomizeDisplay,
    DefaultDisplay,
    JumpCamera,
    Rearview,
    Units,
    Difficulty,
    ExploreMap,
    RaceMap,
    Save,
    Transmission,
    Stability,
    Camera,
    Gauges,
    Position,
    LapInfo,
    SplitTime,
    Score,
    Leaderboard,
    WheelOptions,
    Feedback,
    AskTransmission,
    /// A UI_PC_Customize_Options row by [`CUSTOM_ROWS`] index.
    Custom(u8),
    WheelRange,
    RoadFeedback,
    ForceStrength,
    /// UI_QRModeOptions' items (see `quick_race`).
    QrDirection,
    QrLapKo,
    QrLaps,
    QrTraffic,
    QrOpponents,
    QrDifficulty,
    QrCatchUp,
}

pub(crate) fn on_off(v: bool) -> Option<Value> {
    Some(Value::Label(if v { ON } else { OFF }))
}

/// A category's rows in the builders' order (0x4B6E60, 0x4B7180,
/// 0x4B7310, 0x4B7CD0) with their current values (each item's text
/// method).
pub fn rows(category: Category, s: &Settings, player: usize, force_feedback: bool) -> Vec<Row> {
    rows_in(category, s, player, force_feedback, None)
}

/// [`rows`] on UI_PauseOptions when `pause` is set: its builders (0x4C3370
/// Audio, 0x4C36C0 Display, 0x4C38F0 Gameplay, 0x4C4740 Player) list the
/// same items with these differences. Audio: the seven volumes, no Audio
/// Mode. Display: as UI_Options'. Gameplay: Career Difficulty first and
/// only in a career's free roam (0x500B60), then Jump Camera, Rearview
/// Mirror, Speedometer Units, Explore Map Mode, Race Map Mode, Save.
/// Player (the paused player's, 0x8900AC / 0x8900B0): Transmission (not in
/// a drag race), Stability Control, Camera, Feedback, Gauges, then Race
/// Position, Lap Info, Split Time, Leaderboard, Score (not in a test run,
/// 0x89E7E6); no Wheel Options and no Ask for Transmission. Rearview
/// Mirror and Explore Map Mode are left out (0x4C3A7E, 0x4C3B3F) when
/// 0x5026F0 on the race record 0x83AAB4 returns 2: byte +0x0C of the
/// current type's mode record (+0x100 for type 12), the players. Its only
/// writers, 0x502930 from UI_QRModeSelect's 0x4B2C00 and 0x4DF2B7, store
/// 2 only in game mode 4 (split screen) and 1 otherwise, and the profile
/// reset 0x515800 sets 1; the clone has no split screen, so both rows
/// are always listed.
pub fn rows_in(
    category: Category,
    s: &Settings,
    player: usize,
    force_feedback: bool,
    pause: Option<Pause>,
) -> Vec<Row> {
    if let Some(pause) = pause {
        let all = rows_in(category, s, player, force_feedback, None);
        let pick =
            |ids: &[RowId]| ids.iter().filter_map(|id| all.iter().find(|r| r.id == *id).cloned()).collect::<Vec<_>>();
        return match category {
            Category::Audio => all.into_iter().filter(|r| r.id != RowId::AudioMode).collect(),
            Category::Gameplay => {
                let mut ids = Vec::new();
                if pause.career_roam {
                    ids.push(RowId::Difficulty);
                }
                ids.extend([
                    RowId::JumpCamera,
                    RowId::Rearview,
                    RowId::Units,
                    RowId::ExploreMap,
                    RowId::RaceMap,
                    RowId::Save,
                ]);
                pick(&ids)
            }
            Category::Player => {
                let mut ids = Vec::new();
                if !pause.drag {
                    ids.push(RowId::Transmission);
                }
                ids.extend([RowId::Stability, RowId::Camera, RowId::Feedback, RowId::Gauges]);
                if !pause.test_run {
                    ids.extend([RowId::Position, RowId::LapInfo, RowId::SplitTime, RowId::Leaderboard, RowId::Score]);
                }
                pick(&ids)
            }
            _ => all,
        };
    }
    let row = |id, label, kind, value| Row { id, label, kind, value, enabled: true };
    let slider = |id, label, value: f32| row(id, label, Kind::Slider { value, min: 0.0, max: 1.0 }, None);
    let p = &s.players[player.min(1)];
    match category {
        Category::Audio => vec![
            slider(RowId::Master, 0x2678_2C3E, s.master),
            slider(RowId::MenuMusic, 0x418E_681D, s.menu_music),
            slider(RowId::GameMusic, 0xDF21_EAC2, s.game_music),
            slider(RowId::Speech, 0x9E5F_B82A, s.speech),
            slider(RowId::Effects, 0xFD48_7543, s.effects),
            slider(RowId::Tires, 0x69B7_45C6, s.tires),
            slider(RowId::Engine, 0xA2B1_F888, s.engine),
            row(
                RowId::AudioMode,
                0x2881_AB87,
                Kind::Cycle,
                Some(Value::Label(if s.stereo { 0x55DA_8BF8 } else { 0xC50F_A35F })),
            ),
        ],
        Category::Display => vec![
            row(
                RowId::Resolution,
                0xA9A7_6BBA,
                Kind::Cycle,
                Some(Value::Owned(format!("{}x{}", s.resolution.0, s.resolution.1))),
            ),
            row(RowId::Detail, 0xB55E_7665, Kind::Slider { value: s.detail.max(1) as f32, min: 1.0, max: 5.0 }, None),
            row(RowId::CustomizeDisplay, fe_ids::OBJ_381FE1E9, Kind::Button, None),
            row(RowId::DefaultDisplay, 0x13F0_EDEB, Kind::Button, None),
            // The clone's rows, after the game's four.
            row(
                RowId::ScreenMode,
                SCREEN_MODE,
                Kind::Cycle,
                Some(Value::Label(if s.fullscreen { FULL_SCREEN } else { WINDOWED })),
            ),
            row(
                RowId::AspectRatio,
                ASPECT_RATIO,
                Kind::Cycle,
                Some(Value::Label(if s.aspect == 1 { ASPECT_STRETCHED } else { ASPECT_WIDE })),
            ),
            row(
                RowId::VsyncOff,
                VSYNC_OFF_MODE,
                Kind::Cycle,
                Some(Value::Label(if s.vsync_off_mailbox { MAILBOX } else { IMMEDIATE })),
            ),
            row(RowId::Enhancements, ENHANCEMENTS, Kind::Button, None),
        ],
        // The clone's Enhancements page: each row's native value reads
        // "Native" or "Off".
        // docs/settings-page.md: the settings page's hub and its
        // group pages.
        Category::Enhancements => super::settings_page::hub_rows(s),
        Category::Settings(g) => super::settings_page::group_rows(g, s),
        Category::Advanced => super::settings_page::advanced_rows(s),
        // The clone's Graphics page: the preset, the groups, Restore
        // Native.
        Category::Graphics => {
            let preset = s.enh.preset();
            let mut v = vec![row(
                RowId::GraphicsPreset,
                GRAPHICS_PRESET,
                Kind::Cycle,
                Some(match preset {
                    Some(p) => Value::Text(p.name()),
                    None => Value::Label(CUSTOM_PRESET),
                }),
            )];
            v.extend(GRAPHICS_GROUPS.iter().map(|g| row(RowId::GraphicsGroup(*g), g.label(), Kind::Button, None)));
            v.push(row(RowId::RestoreNative, RESTORE_NATIVE, Kind::Button, None));
            v
        }
        // A group's rows, each value with its GPU cost class.
        Category::GraphicsGroup(g) => {
            let all = graphics_rows(s);
            let mut v: Vec<Row> = super::graphics_table::group_rows(g, s)
                .iter()
                .filter_map(|r| all.iter().find(|x| x.id == r.id).cloned())
                .collect();
            // The action row after World Texture Pack.
            if let Some(i) = v.iter().position(|r| r.id == RowId::TexturePack) {
                let value = match crate::upscale_pack::job_status() {
                    crate::upscale_pack::JobStatus::Running(_) | crate::upscale_pack::JobStatus::Cancelling => {
                        Some(Value::Text("Running (Select cancels)"))
                    }
                    _ => None,
                };
                v.insert(
                    i + 1,
                    Row {
                        id: RowId::GenerateTexturePack,
                        label: GENERATE_TEXTURE_PACK,
                        kind: Kind::Button,
                        value,
                        enabled: true,
                    },
                );
            }
            v
        }
        Category::HdUi => {
            hd_ui_rows(s.enh.hd_ui, crate::hd_ui_pack::installed(), crate::hd_ui_pack::status(s.enh.hd_ui))
        }
        Category::Gameplay => vec![
            row(RowId::JumpCamera, 0xF26A_5CBF, Kind::Cycle, on_off(s.jump_camera)),
            row(RowId::Rearview, 0x85A6_CE05, Kind::Cycle, on_off(s.rearview)),
            row(
                RowId::Units,
                0xF86A_6272,
                Kind::Cycle,
                Some(Value::Label(if s.metric { fe_ids::KMH } else { fe_ids::MPH })),
            ),
            row(
                RowId::Difficulty,
                0xBC0E_6C55,
                Kind::Cycle,
                Some(Value::Label([0xDBBB_4E8F, 0xCAB2_62DE, 0xDBBC_F37C][s.difficulty.min(2) as usize])),
            ),
            row(RowId::ExploreMap, 0xC626_9082, Kind::Cycle, Some(Value::Label(map_label(s.explore_map)))),
            row(RowId::RaceMap, 0x9FA5_EC9E, Kind::Cycle, Some(Value::Label(map_label(s.race_map)))),
            row(
                RowId::Save,
                0x9964_63E6,
                Kind::Cycle,
                Some(Value::Label(if s.autosave {
                    0x1B7A_BF8E
                } else if s.save_prompts {
                    0x1B7A_BF8F
                } else {
                    0x1B7A_BF90
                })),
            ),
        ],
        Category::Player => vec![
            row(
                RowId::Transmission,
                0xD314_07E7,
                Kind::Cycle,
                Some(Value::Label(if p.manual { 0x1B3A_2485 } else { 0x61CB_0720 })),
            ),
            // 0x4B76F0: value 0 reads "On", 1 "Off".
            row(RowId::Stability, 0xDF06_ED22, Kind::Cycle, on_off(!p.stability_off)),
            row(
                RowId::Camera,
                0xF6CC_DC5F,
                Kind::Cycle,
                Some(Value::Label(
                    [0xC3E9_AE58, 0x414F_19D7, 0x5AE3_441F, 0x1EA4_CEC2, 0x9160_39B4][p.camera.min(4) as usize],
                )),
            ),
            row(RowId::Gauges, 0xAC14_8579, Kind::Cycle, on_off(p.gauges)),
            row(RowId::Position, 0x82CD_8F92, Kind::Cycle, on_off(p.position)),
            row(RowId::LapInfo, 0x969F_96A5, Kind::Cycle, on_off(p.lap_info)),
            row(
                RowId::SplitTime,
                0x084B_C378,
                Kind::Cycle,
                Some(Value::Label([0xD93F_8F55, 0xC44D_3943, 0x17FA_FC32, 0x1EA4_59F8, OFF][p.split.min(4) as usize])),
            ),
            row(RowId::Score, fe_ids::OBJ_C03F9F19, Kind::Cycle, on_off(p.score)),
            row(RowId::Leaderboard, 0xC93F_BFB5, Kind::Cycle, on_off(p.leaderboard)),
            row(RowId::WheelOptions, fe_ids::OBJ_4DEAB931, Kind::Button, None),
            // Enabled when a force-feedback device is present (0x5B8250).
            Row {
                id: RowId::Feedback,
                label: 0x358C_DCFD,
                kind: Kind::Cycle,
                value: on_off(p.feedback),
                enabled: force_feedback,
            },
            row(RowId::AskTransmission, 0xBFB3_4120, Kind::Cycle, on_off(p.ask_transmission)),
        ],
        // A row shows when its cap is nonzero (0x4B6526…); Vsync always.
        Category::CustomDisplay => CUSTOM_ROWS
            .iter()
            .enumerate()
            .filter(|&(i, r)| r.3 == 0 || CUSTOM_CAPS[i] > 0)
            .map(|(i, &(label, kind, _, _))| {
                let v = s.custom[i];
                let id = RowId::Custom(i as u8);
                if i == 14 && crate::qol::msaa_choice(&s.enh).is_some() {
                    return Row {
                        enabled: false,
                        ..row(id, label, Kind::Cycle, Some(Value::Owned("Disabled - Enhancements".into())))
                    };
                }
                match kind {
                    CustomKind::Slider => {
                        row(id, label, Kind::Slider { value: v as f32, min: 0.0, max: CUSTOM_CAPS[i] as f32 }, None)
                    }
                    // An enhanced Graphics tier above the original's High
                    // reads its name.
                    CustomKind::LowHigh if custom_enhanced(i, s).is_some() => {
                        row(id, label, Kind::Cycle, custom_enhanced(i, s).map(|n| Value::Owned(n.to_string())))
                    }
                    // 0x49BAF0: 0xB55E7678 + (v ≠ 0) × 0xABCC822E.
                    CustomKind::LowHigh => {
                        row(id, label, Kind::Cycle, Some(Value::Label(if v != 0 { 0x612A_F8A6 } else { 0xB55E_7678 })))
                    }
                    CustomKind::OnOff => row(id, label, Kind::Cycle, on_off(v != 0)),
                }
            })
            .collect(),
        // 0x49B3C0 / 0x49B4D0 / 0x49B5C0 set each slider's range and step
        // 0.1; Road Feedback shows 1 − (clamp(v, 2, 20) − 2) / 18.
        Category::Wheel => vec![
            row(RowId::WheelRange, 0x56FE_B0D2, Kind::Slider { value: p.wheel_range, min: 0.1, max: 1.0 }, None),
            row(
                RowId::RoadFeedback,
                0x1CC4_B396,
                Kind::Slider { value: 1.0 - (p.road_feedback.clamp(2.0, 20.0) - 2.0) / 18.0, min: 0.0, max: 1.0 },
                None,
            ),
            row(RowId::ForceStrength, 0x28F4_2AE8, Kind::Slider { value: p.force, min: 0.0, max: 1.0 }, None),
        ],
    }
}

/// The Controller Layout row's value: Original, Modern, or Auto with what
/// it resolved the attached pads to ("Auto (Modern)", "Auto (Original)",
/// "Auto (Mixed)"; "Auto" with no pad).
pub(crate) fn pad_layout_value(layout: u8) -> Value {
    use crate::pad::extras::{LAYOUT_AUTO, LAYOUT_NAMES, resolved};
    if layout != LAYOUT_AUTO {
        return Value::Text(LAYOUT_NAMES[usize::from(layout.min(1))]);
    }
    Value::Text(match resolved() {
        (false, false) => "Auto",
        (true, false) => "Auto (Original)",
        (false, true) => "Auto (Modern)",
        (true, true) => "Auto (Mixed)",
    })
}
