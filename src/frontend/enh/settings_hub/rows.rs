use super::*;

// ---------------------------------------------------------------------
// Rows.

/// Customize Display rows an enhancement needs, shown on its group page
/// as the same setting ([`Origin::Native`] in the page's terms).
pub const NATIVE_ROWS: [(u8, UxGroup); 2] = [(10, UxGroup::LightingEffects), (19, UxGroup::WeatherWater)];

/// The model rows and native rows of `g` in order.
pub fn group_ids(g: UxGroup) -> Vec<RowId> {
    let mut v: Vec<RowId> = MODEL.iter().filter(|m| m.group == g).map(|m| m.id).collect();
    for (c, at) in NATIVE_ROWS {
        if at == g {
            // After the rows that need it.
            let after = match c {
                10 => RowId::SparksFire,
                _ => RowId::LensDrops,
            };
            let i = v.iter().position(|&r| r == after).map_or(v.len(), |i| i + 1);
            v.insert(i, RowId::Custom(c));
        }
    }
    v
}

/// The row of `id` as the page shows it.
fn page_row(id: RowId, s: &Settings, graphics: &[Row], qol_rows: &[Row]) -> Option<Row> {
    if let RowId::Custom(c) = id {
        return options::rows(Category::CustomDisplay, s, 0, false).into_iter().find(|r| r.id == RowId::Custom(c));
    }
    if id == RowId::HdUi {
        return options::hd_ui_rows(
            s.enh.hd_ui,
            crate::hd_ui_pack::installed(),
            crate::hd_ui_pack::status(s.enh.hd_ui),
        )
        .into_iter()
        .next();
    }
    if id == RowId::GenerateTexturePack {
        let value = match crate::upscale_pack::job_status() {
            crate::upscale_pack::JobStatus::Running(_) | crate::upscale_pack::JobStatus::Cancelling => {
                Some(Value::Text("Running (Select cancels)"))
            }
            _ => None,
        };
        return Some(Row { id, label: options::GENERATE_TEXTURE_PACK, kind: Kind::Button, value, enabled: true });
    }
    graphics.iter().chain(qol_rows).find(|r| r.id == id).cloned()
}

/// Row `id` back to the original's value (the original's own settings and
/// the shared rows' native values as the player set them).
pub fn reset_row(id: RowId, s: &mut Settings) {
    let native = graphics_table::row(id).map(|_| native_reference(s));
    reset_row_to(id, s, native.as_ref());
}

/// `s` with the original's graphics values ([`Preset::Original`] on the
/// enhancements, normalised): what [`reset_row`] puts a graphics row back
/// to. The same for every row of `s`, so a whole page builds it once.
fn native_reference(s: &Settings) -> Settings {
    #[cfg(test)]
    NATIVE_REFERENCES.with(|n| n.set(n.get() + 1));
    let mut d = s.clone();
    Preset::Original.apply(&mut d.enh);
    graphics_table::normalize(&mut d);
    d
}

#[cfg(test)]
thread_local! {
    /// [`native_reference`]s built on this thread (the once-per-page test).
    pub(super) static NATIVE_REFERENCES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// [`reset_row`] with `s`'s [`native_reference`] given (`None` only for a
/// row that is not a graphics row).
fn reset_row_to(id: RowId, s: &mut Settings, native_of_s: Option<&Settings>) {
    if let Some(r) = graphics_table::row(id) {
        let native = (r.get)(native_of_s.expect("a graphics row's native reference"));
        if (r.get)(s) != native {
            (r.set)(s, native);
        }
        return;
    }
    let d = qol::Enhancements::default();
    let o = crate::overlay::Settings::default();
    let e = &mut s.enh;
    match id {
        RowId::FovOffset => e.fov_offset = d.fov_offset,
        RowId::HudScale => e.hud_scale = d.hud_scale,
        RowId::HudLayout => e.hud_full_width = d.hud_full_width,
        RowId::FrameCap => e.frame_cap = d.frame_cap,
        RowId::DurableSaves => e.durable_saves = d.durable_saves,
        RowId::SaveBackups => e.save_backups = d.save_backups,
        RowId::CrashReports => e.crash_reports = d.crash_reports,
        RowId::MarkSpot => e.extras.mark_spot = d.extras.mark_spot,
        RowId::ListsOpenInstalled => e.lists_open_installed = d.lists_open_installed,
        RowId::OutrunVariety => e.outrun_variety = d.outrun_variety,
        RowId::FixModels => e.fix_models = d.fix_models,
        RowId::TrafficDensity => e.traffic_density = d.traffic_density,
        RowId::PhotoMode => e.photo_mode = d.photo_mode,
        RowId::SkipMovies => {
            let title = e.skips_title();
            e.set_skips(d.skip_movies, title)
        }
        RowId::SkipTitle => {
            let movies = e.skips_movies();
            e.set_skips(movies, d.skip_title != 0)
        }
        RowId::LowLatency => e.extras.low_latency = d.extras.low_latency,
        RowId::PowerSaving => e.extras.power_saving = d.extras.power_saving,
        RowId::ReplayCameras => e.extras.replay_cameras = d.extras.replay_cameras,
        RowId::HudPalette => e.extras.palette = d.extras.palette,
        RowId::HudMinimap => e.extras.hud_minimap = d.extras.hud_minimap,
        RowId::HudSpeedo => e.extras.hud_speedo = d.extras.hud_speedo,
        RowId::HudInfo => e.extras.hud_info = d.extras.hud_info,
        RowId::ReducedFlashing => e.extras.reduced_flashing = d.extras.reduced_flashing,
        RowId::PadLayout => e.pad.layout = d.pad.layout,
        RowId::PadDeadzone => e.pad.deadzone = d.pad.deadzone,
        RowId::PadTriggerDeadzone => e.pad.trigger_deadzone = d.pad.trigger_deadzone,
        RowId::PadSaturation => e.pad.saturation = d.pad.saturation,
        RowId::PadSteerCurve => e.pad.steer_curve = d.pad.steer_curve,
        RowId::PadRumble => e.pad.rumble = d.pad.rumble,
        RowId::PadPauseOnDisconnect => e.pad.pause_on_disconnect = d.pad.pause_on_disconnect,
        RowId::PadSafeBind => e.pad.safe_bind = d.pad.safe_bind,
        RowId::PadSticksQuit => e.pad.sticks_quit = d.pad.sticks_quit,
        RowId::HdUi => e.hd_ui = d.hd_ui,
        RowId::ReloadPacks => e.reload_packs = d.reload_packs,
        RowId::PerfOverlay => s.overlay.layout = o.layout,
        RowId::OverlayPosition => s.overlay.position = o.position,
        RowId::OverlayScale => s.overlay.scale = o.scale,
        RowId::OverlayLog => s.overlay.csv = o.csv,
        _ => {}
    }
}

/// Whether row `id` is off the original's value.
pub fn changed(id: RowId, s: &Settings) -> bool {
    changed_against(id, s, &std::cell::OnceCell::new())
}

/// [`changed`] with `s`'s [`native_reference`] built at most once for all
/// the rows a page asks about (the hub and the
/// group pages made two whole-`Settings` copies per row on every drawn
/// frame). A graphics row already at its native value is unchanged without
/// a copy; otherwise the answer is [`reset_row`]'s, as before.
fn changed_against(id: RowId, s: &Settings, native: &std::cell::OnceCell<Settings>) -> bool {
    if matches!(id, RowId::Custom(_)) {
        return false;
    }
    let native = graphics_table::row(id).map(|r| (r, native.get_or_init(|| native_reference(s))));
    if let Some((r, n)) = native
        && (r.get)(s) == (r.get)(n)
    {
        return false;
    }
    let mut n = s.clone();
    reset_row_to(id, &mut n, native.map(|(_, n)| n));
    n != *s
}

/// Every row of `g` back to the original's value.
pub fn reset_group(g: UxGroup, s: &mut Settings) {
    for id in group_ids(g) {
        reset_row(id, s);
    }
    graphics_table::normalize(s);
}

/// The time row `id`'s current tier adds here (ms), if it has a cost.
fn cost_here(id: RowId, s: &Settings) -> Option<f32> {
    options::cost_ms(id, s, qol::output_size_or(s.resolution))
}

fn shown_by_filter(id: RowId, s: &Settings, native: &std::cell::OnceCell<Settings>) -> bool {
    match filter() {
        1 => changed_against(id, s, native),
        2 => cost_here(id, s).is_some_and(|ms| ms >= 0.5),
        _ => true,
    }
}

/// Group page `g`'s rows (the filter applied), then Reset Group to Native.
pub fn group_rows(g: UxGroup, s: &Settings) -> Vec<Row> {
    let graphics = options::graphics_rows(s);
    let qol_rows = options::qol_rows(s);
    let native = std::cell::OnceCell::new();
    let mut v: Vec<Row> = group_ids(g)
        .into_iter()
        .filter(|&id| graphics_table::row(id).is_none_or(|r| !r.legacy || (r.get)(s) != 0))
        .filter(|&id| shown_by_filter(id, s, &native))
        .filter_map(|id| page_row(id, s, &graphics, &qol_rows))
        .collect();
    // The HD UI pack's state under its row (read-only).
    if let Some(i) = v.iter().position(|r| r.id == RowId::HdUi) {
        let info =
            options::hd_ui_rows(s.enh.hd_ui, crate::hd_ui_pack::installed(), crate::hd_ui_pack::status(s.enh.hd_ui));
        for (k, r) in info.into_iter().skip(1).enumerate() {
            v.insert(i + 1 + k, r);
        }
    }
    if v.is_empty() {
        v.push(Row {
            id: RowId::UxNoMatch,
            label: options::UX_NO_MATCH,
            kind: Kind::Button,
            value: None,
            enabled: false,
        });
    }
    v.push(Row {
        id: RowId::UxReset(g),
        label: options::UX_RESET_GROUP,
        kind: Kind::Button,
        value: None,
        enabled: true,
    });
    v
}

/// A group's row on the hub or the Advanced page: its page, with how many
/// of its rows are changed.
pub(super) fn group_row(g: UxGroup, s: &Settings, native: &std::cell::OnceCell<Settings>) -> Row {
    let n = group_ids(g).into_iter().filter(|&id| changed_against(id, s, native)).count();
    let value = if n == 0 { text(Text::NoneChanged).to_string() } else { fill(Text::Changed, &[&n.to_string()]) };
    Row { id: RowId::UxGroup(g), label: g.label(), kind: Kind::Button, value: Some(Value::Owned(value)), enabled: true }
}

/// The hub's rows: the streamlined graphics page ([`main_rows`], ending
/// in Advanced), the groups that are not graphics, Restore Native.
pub fn hub_rows(s: &Settings) -> Vec<Row> {
    let mut v = main_rows(s);
    let native = std::cell::OnceCell::new();
    v.extend(HUB_GROUPS.iter().map(|&g| group_row(g, s, &native)));
    v.push(Row {
        id: RowId::RestoreNative,
        label: options::RESTORE_NATIVE,
        kind: Kind::Button,
        value: None,
        enabled: true,
    });
    v
}
