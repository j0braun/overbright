//! The enhancement rows (HD UI, QoL, Graphics), their costs and detail lines.

use crate::frontend::options::*;

/// Keep the saved choice editable even when the combined pack is absent.
/// Pack availability and activation are informational, not selectable settings.
pub(crate) fn hd_ui_rows(requested: bool, installed: bool, status: &'static str) -> Vec<Row> {
    vec![
        Row { id: RowId::HdUi, label: HD_UI, kind: Kind::Cycle, value: on_off(requested), enabled: true },
        Row {
            id: RowId::HdUiPackStatus,
            label: HD_UI_PACK_STATUS,
            kind: Kind::Cycle,
            value: Some(Value::Text(if installed { "Installed" } else { "Pack missing or incomplete" })),
            enabled: false,
        },
        Row {
            id: RowId::HdUiActivationStatus,
            label: HD_UI_ACTIVATION_STATUS,
            kind: Kind::Cycle,
            value: Some(Value::Text(status)),
            enabled: false,
        },
    ]
}

/// The convenience and gameplay rows (the Enhancements page's before the settings page;
/// now on the settings page's groups), each native value reading "Native"
/// or "Off".
pub(crate) fn qol_rows(s: &Settings) -> Vec<Row> {
    let row = |id, label, kind, value| Row { id, label, kind, value, enabled: true };
    let e = &s.enh;
    let or_native = |native: bool, text: String| Some(if native { Value::Label(NATIVE) } else { Value::Owned(text) });
    let or_off = |off: bool, text: String| Some(if off { Value::Label(OFF) } else { Value::Owned(text) });
    vec![
        row(
            RowId::FovOffset,
            FOV_OFFSET,
            Kind::Cycle,
            or_native(e.fov_offset == 0, format!("{:+}\u{B0}", e.fov_offset)),
        ),
        row(RowId::HudScale, HUD_SCALE, Kind::Cycle, or_native(e.hud_scale == 100, format!("{}%", e.hud_scale))),
        row(
            RowId::HudLayout,
            HUD_LAYOUT,
            Kind::Cycle,
            Some(Value::Label(if e.hud_full_width { HUD_FULL_WIDTH } else { HUD_CENTERED })),
        ),
        row(RowId::FrameCap, FRAME_CAP, Kind::Cycle, or_off(e.frame_cap == 0, format!("{} FPS", e.frame_cap))),
        row(RowId::DurableSaves, DURABLE_SAVES, Kind::Cycle, on_off(e.durable_saves)),
        row(RowId::SaveBackups, SAVE_BACKUPS, Kind::Cycle, or_off(e.save_backups == 0, format!("{}", e.save_backups))),
        row(RowId::CrashReports, CRASH_REPORTS, Kind::Cycle, on_off(e.crash_reports)),
        row(
            RowId::MarkSpot,
            MARK_SPOT,
            Kind::Cycle,
            or_off(
                e.extras.mark_spot == 0,
                crate::mark_spot::KEYS[usize::from(e.extras.mark_spot).min(crate::mark_spot::KEYS.len() - 1)]
                    .to_string(),
            ),
        ),
        row(RowId::ListsOpenInstalled, LISTS_OPEN_INSTALLED, Kind::Cycle, on_off(e.lists_open_installed)),
        row(RowId::OutrunVariety, OUTRUN_VARIETY, Kind::Cycle, on_off(e.outrun_variety)),
        row(RowId::FixModels, FIX_MODELS, Kind::Cycle, on_off(e.fix_models)),
        row(
            RowId::TrafficDensity,
            TRAFFIC_DENSITY,
            Kind::Cycle,
            or_native(e.traffic_density == 0, if e.traffic_density == 1 { "More".into() } else { "Dense".into() }),
        ),
        row(RowId::PhotoMode, PHOTO_MODE_ROW, Kind::Cycle, on_off(e.photo_mode)),
        row(
            RowId::SkipTitle,
            SKIP_TITLE,
            Kind::Cycle,
            or_off(e.skip_title == 0, crate::qol::SKIP_TITLE_NAMES[usize::from(e.skip_title.min(2))].to_string()),
        ),
        // See docs/modern-extras.md.
        row(RowId::LowLatency, LOW_LATENCY, Kind::Cycle, on_off(e.extras.low_latency)),
        row(
            RowId::PowerSaving,
            POWER_SAVING,
            Kind::Cycle,
            or_off(
                e.extras.power_saving == 0,
                crate::extras::POWER_NAMES[usize::from(e.extras.power_saving.min(3))].to_string(),
            ),
        ),
        row(
            RowId::ReplayCameras,
            REPLAY_CAMERAS,
            Kind::Cycle,
            Some(if e.extras.replay_cameras { Value::Owned("Extended".into()) } else { Value::Label(NATIVE) }),
        ),
        row(
            RowId::HudPalette,
            HUD_PALETTE,
            Kind::Cycle,
            or_native(
                e.extras.palette == 0,
                crate::extras::PALETTE_NAMES[usize::from(e.extras.palette.min(3))].to_string(),
            ),
        ),
        row(
            RowId::HudMinimap,
            HUD_MINIMAP,
            Kind::Cycle,
            or_native(e.extras.hud_minimap == 100, format!("{}%", e.extras.hud_minimap)),
        ),
        row(
            RowId::HudSpeedo,
            HUD_SPEEDO,
            Kind::Cycle,
            or_native(e.extras.hud_speedo == 100, format!("{}%", e.extras.hud_speedo)),
        ),
        row(
            RowId::HudInfo,
            HUD_INFO,
            Kind::Cycle,
            or_native(e.extras.hud_info == 100, format!("{}%", e.extras.hud_info)),
        ),
        row(RowId::ReducedFlashing, REDUCED_FLASHING, Kind::Cycle, on_off(e.extras.reduced_flashing)),
        // See docs/perf-overlay.md.
        row(
            RowId::PerfOverlay,
            PERF_OVERLAY,
            Kind::Cycle,
            or_off(
                s.overlay.layout == 0,
                crate::overlay::LAYOUT_NAMES[usize::from(s.overlay.layout.min(3))].to_string(),
            ),
        ),
        row(
            RowId::OverlayPosition,
            OVERLAY_POSITION,
            Kind::Cycle,
            Some(Value::Text(crate::overlay::POSITION_NAMES[usize::from(s.overlay.position.min(3))])),
        ),
        row(RowId::OverlayScale, OVERLAY_SCALE, Kind::Cycle, Some(Value::Owned(format!("{}%", s.overlay.scale)))),
        row(RowId::OverlayLog, OVERLAY_LOG, Kind::Cycle, on_off(s.overlay.csv)),
        row(RowId::ReloadPacks, RELOAD_PACKS, Kind::Cycle, on_off(e.reload_packs)),
        // See docs/controller-support.md.
        row(RowId::PadLayout, PAD_LAYOUT, Kind::Cycle, Some(pad_layout_value(e.pad.layout))),
        row(
            RowId::PadDeadzone,
            PAD_DEADZONE,
            Kind::Cycle,
            or_native(e.pad.deadzone == 0, format!("{}%", e.pad.deadzone)),
        ),
        row(
            RowId::PadTriggerDeadzone,
            PAD_TRIGGER_DEADZONE,
            Kind::Cycle,
            or_native(e.pad.trigger_deadzone == 0, format!("{}%", e.pad.trigger_deadzone)),
        ),
        row(
            RowId::PadSaturation,
            PAD_SATURATION,
            Kind::Cycle,
            or_native(e.pad.saturation == 100, format!("{}%", e.pad.saturation)),
        ),
        row(
            RowId::PadSteerCurve,
            PAD_STEER_CURVE,
            Kind::Cycle,
            or_native(e.pad.steer_curve == 100, format!("{:.2}", f32::from(e.pad.steer_curve) / 100.0)),
        ),
        row(
            RowId::PadRumble,
            PAD_RUMBLE,
            Kind::Cycle,
            or_off(
                e.pad.rumble == 0,
                crate::pad::extras::RUMBLE_NAMES
                    [crate::pad::extras::RUMBLE_STEPS.iter().position(|&r| r == e.pad.rumble).unwrap_or(0)]
                .to_string(),
            ),
        ),
        row(RowId::PadPauseOnDisconnect, PAD_PAUSE_ON_DISCONNECT, Kind::Cycle, on_off(e.pad.pause_on_disconnect)),
        row(RowId::PadSafeBind, PAD_SAFE_BIND, Kind::Cycle, on_off(e.pad.safe_bind)),
    ]
}

/// A Graphics value's GPU cost class, from the audit's timings
/// (docs/graphics-settings-audit.md: reference desktop GPU `--bench`, GPU busy time
/// added at 1080p; an integrated GPU pays roughly 8–12× as much).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cost {
    /// Under 0.05 ms.
    Free,
    /// Under 0.3 ms.
    Low,
    /// Under 1 ms.
    Medium,
    /// Under 3 ms.
    High,
    /// 3 ms or more.
    Extreme,
}

impl Cost {
    pub fn name(self) -> &'static str {
        match self {
            Cost::Free => "Free",
            Cost::Low => "Low",
            Cost::Medium => "Med",
            Cost::High => "High",
            Cost::Extreme => "Extreme",
        }
    }
}

/// The cost class of row `id`'s current value (None when native).
pub fn cost(id: RowId, s: &Settings, out: (u32, u32)) -> Option<Cost> {
    let ms = cost_ms(id, s, out)?;
    Some(match ms {
        m if m < 0.05 => Cost::Free,
        m if m < 0.3 => Cost::Low,
        m if m < 1.0 => Cost::Medium,
        m if m < 3.0 => Cost::High,
        _ => Cost::Extreme,
    })
}

/// The time row `id`'s current tier adds over Original (World Detail 5,
/// which plays with 8x MSAA) at the output `out`, ms: the tier's
/// [`super::graphics_table::Tier::cost`] (reference desktop GPU `--bench`, GPU busy
/// ms at 4K scaled by the output's pixels, plus CPU ms). None when the
/// tier is native.
pub fn cost_ms(id: RowId, s: &Settings, out: (u32, u32)) -> Option<f32> {
    let r = crate::frontend::graphics_table::row(id)?;
    let tier = r.tiers.get((r.get)(s))?;
    let (mut gpu_4k, fixed) = tier.cost?;
    // MSAA on top of SSAA multiplies the supersampled scene's samples:
    // The settings conflict audit measured 8x under SSAA 2x at +9.6 ms (racing 4K, 8.70 → 18.30
    // ms, the player's setup); other counts and SSAA factors scaled from it
    // by samples and the SSAA's pixels (**inferred**).
    if id == RowId::AaMsaa
        && let Some(k) = crate::qol::ssaa_scale(s.enh.post_aa)
    {
        gpu_4k = 1.2 * f32::from(s.enh.aa_msaa) * (k * k / 2.0) as f32;
    }
    Some(gpu_4k * (out.0 as f32 * out.1 as f32) / (3840.0 * 2160.0) + fixed)
}

/// The detail line of row `id` on `category` : what the row does,
/// what its current value does and its cost at the current output. None
/// for rows without one (the original's pages).
pub fn detail(category: Category, id: RowId, s: &Settings) -> Option<String> {
    // The settings page writes its own (docs/settings-page.md).
    if matches!(category, Category::Enhancements | Category::Settings(_) | Category::Advanced) {
        return crate::frontend::settings_page::detail(category, id, s);
    }
    if let Some(r) = crate::frontend::graphics_table::row(id).filter(|_| matches!(category, Category::GraphicsGroup(_)))
    {
        let tier = r.tiers.get((r.get)(s))?;
        let out = crate::qol::output_size_or(s.resolution);
        let cost = match (cost_ms(id, s, out), cost(id, s, out)) {
            (Some(ms), Some(c)) if ms >= 0.05 => {
                format!(" Cost: {} (+{ms:.2} ms here; integrated GPU est. ~10x).", c.name())
            }
            (Some(ms), _) if ms < 0.0 => format!(" Saves ~{:.1} ms here.", -ms),
            (Some(_), _) => " Cost: Free.".to_string(),
            (None, _) => String::new(),
        };
        let help = if tier.help.is_empty() { "" } else { tier.help };
        // The reason a rule in force gives (overridden, needs, …).
        let rule = crate::frontend::settings_model::note(id, s).map_or(String::new(), |n| format!(" {n}"));
        return Some(format!("{} {help}{cost}{rule}", r.help).trim().to_string());
    }
    let text = match id {
        RowId::GraphicsPreset => match s.enh.preset() {
            Some(crate::qol::Preset::Original) => "Original: every graphics value as the original (native).",
            Some(crate::qol::Preset::Clean) => {
                "Clean: smoother glow, MSAA 8x with alpha-to-coverage, richer paint. +1.9 ms at 4K (measured)."
            }
            Some(crate::qol::Preset::Modern) => {
                "Modern: Clean + reflections, wet roads, AO, shadows, LOD fade, draw distance. +2.5 ms at 4K (measured)."
            }
            Some(crate::qol::Preset::Ultra) => {
                "Ultra: Modern + SSAA 2x, full glow, full-res reflections, SSR High. +5.4 ms at 4K (measured)."
            }
            Some(crate::qol::Preset::Extreme) => {
                "Extreme: Ultra + full-resolution SSR, volumetric smoke, realistic water, Wet Road Interaction High, surface materials, smooth cars, World Detail Extreme."
            }
            None => "Custom: the values match no preset. Left / Right picks one.",
        },
        RowId::GraphicsGroup(g) => g.help(),
        RowId::RestoreNative => RESTORE_NATIVE_HELP,
        RowId::Graphics => "Presets and every graphics enhancement, by group.",
        RowId::FovOffset => "Degrees added to the driving cameras' view. Native: 0.",
        RowId::HudScale => "The in-race HUD's size. Native: 100%.",
        RowId::FrameCap => "Caps the frame rate (the game still plays at 60 Hz). Off: uncapped as the original.",
        RowId::DurableSaves => "Flushes profile saves to the disk before and after writing.",
        RowId::SaveBackups => "Keeps this many previous copies of each profile.",
        RowId::CrashReports => "A crash writes a report file.",
        RowId::ListsOpenInstalled => "Part lists open on the fitted part instead of the first.",
        RowId::OutrunVariety => "Outrun racers change car between encounters (same class and performance).",
        RowId::FixModels => "Corrects errors in the original car models (applies to cars loaded next).",
        RowId::TrafficDensity => {
            "More traffic cars in free roam (More 1.5x, Dense 2x; applies when free roam loads). Races keep the original's traffic."
        }
        RowId::PhotoMode => {
            "Adds Photo Mode to the pause menu: a free camera, focus, filters and high-resolution pictures."
        }
        RowId::SkipTitle => {
            "Starts without the title screen (Title Screen) or without the opening movies and the title screen. Off: as the original."
        }
        RowId::LowLatency => {
            "Reads your input as late as the display allows (one frame in flight; on a variable-refresh display, frames held just under its top rate). Off: as the original."
        }
        RowId::MarkSpot => {
            "Bug reports: the key writes where you are (camera, car, section, nearest object, time, settings) and a command that rebuilds the view to a file in the data folder's overbright/marks. Off: no key."
        }
        RowId::PowerSaving => {
            "Saves power where nothing you watch changes. Background: 30 FPS while the game is not the focused window. + Menus: menus held to the display's refresh rate. + Battery: on battery, the whole game held to the display's refresh rate. Off: uncapped."
        }
        RowId::ReplayCameras => {
            "Extended: trackside, helicopter, wheel and orbit cameras after the original six (C), smooth half / quarter speed slow motion (,). Native: the original's replay."
        }
        RowId::HudPalette => {
            "Colour-vision palette for the HUD: reds, greens or blues it confuses moved to colours it tells apart. Native: the original's colours."
        }
        RowId::HudMinimap => "The minimap's size. Native: 100%.",
        RowId::HudSpeedo => "The tachometer and speedometer's size. Native: 100%.",
        RowId::HudInfo => "The race information's size (position, laps, times). Native: 100%.",
        RowId::ReducedFlashing => {
            "Holds full-screen bright flashes to a quarter of their strength. Off: as the original."
        }
        RowId::PadLayout => {
            "Auto (default): Modern for pads known as standard gamepads (Xbox, PlayStation, Switch Pro, Steam Deck…), Original for older DirectInput-style pads, wheels and pads with your own Original bindings in this profile. Modern: any pad as a standard one: right trigger throttle, left trigger brake (analog), left stick steers, A nitrous, X handbrake, B look back, Y camera, bumpers shift, Start pause, Back world map, d-pad up Enter Race / Shop and down Read SMS; in menus A select, B back, bumpers page, the d-pad edits the profile name; help-bar names of your pad. Original: the pad as Windows and the original game present it (on many pads the triggers do nothing). A device setting: Restore Native and the presets leave it."
        }
        RowId::PadDeadzone => "Stick travel around the centre that reads as centred, the rest rescaled. Native: 0%.",
        RowId::PadTriggerDeadzone => "Trigger travel that reads as released, the rest rescaled. Native: 0%.",
        RowId::PadSaturation => {
            "Stick and trigger travel that already reads full. Native: 100% (only the end stop is full)."
        }
        RowId::PadSteerCurve => {
            "Steering stick response: above 1 finer near the centre, below 1 quicker. Native: 1.00 (linear)."
        }
        RowId::PadRumble => {
            "Pad rumble for bumps and hits, rough ground, nitrous and gear changes, after the original's wheel force feedback. Off: as the original (no rumble on a pad)."
        }
        RowId::PadPauseOnDisconnect => {
            "Pauses the race or free roam when the pad that drives is unplugged. Off: as the original."
        }
        RowId::PadSafeBind => {
            "Binding an axis on the Controls page takes only the axis you moved most; the original also switches off any other axis that moved at the same time."
        }
        RowId::PerfOverlay => {
            "The performance overlay shown at start: Minimal (FPS and frame times), Standard (lows, frame-time graph, CPU, GPU, memory) or Full (cores, threads, I/O, pacing, GPU passes). F3 cycles the layouts at any time, Shift+F3 hides it. Off: hidden until F3."
        }
        RowId::OverlayPosition => "The screen corner the performance overlay sits in.",
        RowId::OverlayScale => "The performance overlay's size. 100%: the F3 overlay's.",
        RowId::OverlayLog => {
            "Writes every overlay figure four times a second to a CSV file in the data folder's overbright/perf, for benchmarks. Off: nothing written."
        }
        RowId::HdUiPage | RowId::HdUi => {
            "The HUD mod set up in the launcher's Mods, the same switch as its Use the HUD mod (restart to apply, or at once with Reload Packs When Changed)."
        }
        RowId::ReloadPacks => {
            "Watches the HD UI pack and the world and car texture packs: a changed, added or removed image shows within a second, without a restart; Ctrl+R reloads them all. The HD UI and World Texture Pack rows then apply at once. Off: packs are read at startup."
        }
        RowId::GenerateTexturePack => {
            use crate::upscale_pack::JobStatus;
            let lead = "Upscales your own world and car textures into your data folder (a few minutes, in the background; Select again cancels).";
            return Some(match crate::upscale_pack::job_status() {
                JobStatus::Idle if crate::upscale_pack::generated() => {
                    format!("{lead} A pack exists: Select updates it.")
                }
                JobStatus::Idle => lead.to_string(),
                JobStatus::Running(p) => format!("Generating: {p}. Select cancels."),
                JobStatus::Cancelling => "Cancelling...".to_string(),
                JobStatus::Done(d) => format!("{d} Turn World Texture Pack on above."),
                JobStatus::Failed(e) => e,
            });
        }
        _ => return None,
    };
    // A preset's effect tiers follow the GPU class (qol::Preset::apply_for); say so off the High class.
    let class_note = match (id, s.enh.preset(), crate::qol::gpu_class()) {
        (RowId::GraphicsPreset, Some(p), c)
            if c != crate::qol::GpuClass::High && {
                let (mut a, mut b) = (s.enh, s.enh);
                p.apply_for(c, &mut a);
                p.apply_for(crate::qol::GpuClass::High, &mut b);
                a != b
            } =>
        {
            format!(" Effect tiers for this {} GPU (lighter AA, fog, smoke and extras).", c.name())
        }
        _ => String::new(),
    };
    matches!(category, Category::Graphics | Category::Enhancements | Category::HdUi)
        .then(|| format!("{text}{class_note}"))
}

/// The original's Customize Display value of row `c` shown with the
/// Graphics row that shares it ([`super::graphics_table::for_custom`]):
/// an enhanced tier's name on a Low / High row.
pub(crate) fn custom_enhanced(c: usize, s: &Settings) -> Option<&'static str> {
    let r = crate::frontend::graphics_table::for_custom(c).filter(|r| r.enhanced(s))?;
    Some(r.tiers[(r.get)(s)].name)
}

/// Every Graphics row with its tier's name ([`super::graphics_table`]):
/// a row the adapter cannot do anything but native on is greyed. Numbers
/// stay only where modern games show one (Render Scale, with the
/// resolution it renders at).
pub(crate) fn graphics_rows(s: &Settings) -> Vec<Row> {
    let e = &s.enh;
    let soft = crate::qol::software_gpu();
    let samples = crate::qol::supported_msaa(crate::qol::msaa_choice(e).unwrap_or(1));
    let out = crate::qol::output_size_or(s.resolution);
    let limited = (e.aa_msaa == 0
        && crate::qol::msaa_for(e.post_aa).is_some_and(|n| crate::qol::supported_msaa(n) != n))
        || crate::qol::ssaa_scale(e.post_aa)
            .is_some_and(|want| crate::qol::scene_scale(e.render_scale, e.post_aa, out, samples) < want - 1e-6);
    crate::frontend::graphics_table::ROWS
        .iter()
        .map(|r| {
            let i = (r.get)(s);
            let name = r.tiers.get(i).map_or("", |t| t.name);
            let mut row = Row {
                id: r.id,
                label: r.label,
                kind: Kind::Cycle,
                value: Some(Value::Owned(name.to_string())),
                enabled: true,
            };
            match r.id {
                RowId::RenderScale => {
                    let pct = if crate::qol::ssaa_scale(e.post_aa).is_some() {
                        (crate::qol::scene_scale(e.render_scale, e.post_aa, out, samples) * 100.0).round() as u32
                    } else {
                        u32::from(e.render_scale)
                    };
                    let h = |p: u32| out.1 * p / 100;
                    let text = if crate::qol::ssaa_scale(e.post_aa).is_some() {
                        format!("{pct}% (SSAA)")
                    } else if pct == 100 {
                        "100% (Native)".to_string()
                    } else {
                        // The rendered height ("→" is not in the font).
                        format!("{pct}% ({}p)", h(pct))
                    };
                    row.value = Some(Value::Owned(text));
                    row.enabled = crate::qol::ssaa_scale(e.post_aa).is_none();
                    // Upscaling sets the scene's size.
                    if let Some(u) = crate::qol::upscale_name(e.upscale) {
                        row.value = Some(Value::Owned(format!("Upscaling {u}")));
                        row.enabled = false;
                    }
                }
                // The temporal pass is the one AA: the combination
                // rows do nothing under it.
                RowId::AaMsaa | RowId::AaFilter if crate::qol::temporal_for(e) => row.enabled = false,
                RowId::PostAa if matches!(e.upscale, 1..=3) => row.value = Some(Value::Owned("TAA (Upscaling)".into())),
                // FSR 2 is the anti-aliasing (SSAA / MSAA / filters
                // off); the row is greyed and says why.
                RowId::PostAa if crate::qol::is_fsr2(e.upscale) => {
                    row.value = Some(Value::Owned("FSR 2 (Upscaling)".into()));
                    row.enabled = false;
                }
                RowId::PostAa if limited && e.post_aa != 0 => {
                    row.value = Some(Value::Owned(format!("{name} (limited)")))
                }
                RowId::AaMsaa if e.aa_msaa > 1 && !crate::qol::msaa_offered(e.aa_msaa) => {
                    row.value = Some(Value::Owned(format!("{name} (limited)")))
                }
                // HDR Output: greyed where the surface offers no HDR
                // signal (SDR displays, headless).
                RowId::HdrOutput | RowId::HdrPeak | RowId::HdrPaper | RowId::HdrGamma
                    if crate::qol::hdr_output_signal().is_none() =>
                {
                    row.enabled = false;
                    if r.id == RowId::HdrOutput {
                        row.value = Some(Value::Owned("Not Supported".into()));
                    }
                }
                RowId::HdrOutput if e.hdr_output => {
                    row.value =
                        Some(Value::Owned(format!("On ({})", crate::qol::hdr_output_signal().map_or("", |x| x.name()))))
                }
                // Greyed until a pack is generated (or a scoped
                // variable names one) and while one is being made.
                RowId::TexturePack if !texture_pack_available() => {
                    row.enabled = false;
                    row.value = Some(Value::Owned(if crate::upscale_pack::job_running() {
                        "Generating".into()
                    } else {
                        "Not Generated".into()
                    }));
                }
                // Nothing but the original's values on a software adapter.
                RowId::RoadReflection | RowId::CarReflection if soft && !r.enhanced(s) => {}
                _ => {}
            }
            // docs/settings-model.md: a row another one overrides, or
            // that needs one that is off, says so and is greyed; its stored
            // value stays (switching back restores it).
            if let Some((text, enabled)) = crate::frontend::settings_model::shown(r.id, name, s) {
                row.value = Some(Value::Owned(text));
                row.enabled &= enabled;
            }
            row
        })
        .collect()
}
