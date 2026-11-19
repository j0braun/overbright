//! Debug environment knobs of the game binary: every `NFSU2_*`
//! (and the few older unprefixed aids) the game's own paths read, read in
//! one place. The developer harness's own inputs (src/bin/game/dev/**)
//! read theirs where they are.
//!
//! When: once at the start (the first `debug_env()`), and again by each
//! `Game::new` (`reload_debug_env`), so a game sees the environment as it was when
//! it was made — what the per-call reads gave before, since nothing in
//! the game changes these variables (tests set them before `Game::new`).
//! Each field keeps its knob's own read: `var_os(..).is_some()` → bool
//! (set, even empty or "0", is on), `var(..).ok()` → `Option<String>`
//! (unset or not UTF-8 is `None`), `var_os` → `Option<OsString>`; the
//! parse stays at the use.
//!
//! Behaviour changers (what the game does or shows) are read only with
//! `dev-harness`; without it they are unset.

use std::ffi::OsString;
use std::sync::{Arc, RwLock};

/// The knobs (see the module docs for the read of each kind).
/// The harness's command-line knobs are read only by `dev-harness` code.
#[derive(Default, Debug)]
#[cfg_attr(not(feature = "dev-harness"), allow(dead_code))]
pub(crate) struct DebugEnv {
    // Shell and platform switches (all builds).
    /// NFSU2_NO_OVERLAP: each frame submitted and presented before `draw`
    /// returns.
    pub(crate) no_overlap: bool,
    /// NFSU2_NO_PRELOAD (off when unset or "0").
    pub(crate) no_preload: Option<OsString>,
    /// NFSU2_NO_PROGRAMS_AHEAD: the drive's programs are not made ahead in
    /// the menus (a measuring aid: the loads then make them, as before).
    pub(crate) no_programs_ahead: bool,
    /// NFSU2_NO_MALLOC_THP.
    pub(crate) no_malloc_thp: bool,
    /// NFSU2_OVERLAY: the performance overlay's sampler on.
    pub(crate) overlay: bool,
    /// NFSU2_NO_BELT: no staging belt.
    pub(crate) no_belt: bool,
    /// NFSU2_HEADLESS_ONLY: never open a window.
    pub(crate) headless_only: bool,
    // Logs and probes (all builds; stderr or a file only).
    /// NFSU2_AUDIO_DEBUG.
    pub(crate) audio_debug: bool,
    /// NFSU2_PREVIEW_LOG.
    pub(crate) preview_log: bool,
    /// HYDRO_LOG.
    pub(crate) hydro_log: bool,
    /// NFSU2_POP_VERBOSE.
    pub(crate) pop_verbose: bool,
    /// NFSU2_CARLIGHTS_LOG.
    pub(crate) carlights_log: bool,
    /// NFSU2_FOG_DEBUG.
    pub(crate) fog_debug: bool,
    /// NFSU2_SIGHT_DEBUG.
    pub(crate) sight_debug: bool,
    /// NFSU2_NIS_PROBE.
    pub(crate) nis_probe: bool,
    /// NFSU2_FLASH_PROBE.
    pub(crate) flash_probe: bool,
    /// NFSU2_CAR_DRAW_PROBE: the player car's draw decision each frame.
    pub(crate) car_draw_probe: bool,
    /// NFSU2_DEBUG_ACTIONS.
    pub(crate) debug_actions: bool,
    /// NFSU2_DEBUG_MARKERS.
    pub(crate) debug_markers: bool,
    /// NFSU2_FX_CONTACTS.
    pub(crate) fx_contacts: bool,
    /// NFSU2_PROBE.
    pub(crate) probe: bool,
    /// NFSU2_MIX_TRACE.
    pub(crate) mix_trace: Option<String>,
    /// NFSU2_REPLAY_CAMLOG.
    pub(crate) replay_camlog: Option<String>,
    /// NFSU2_INPUT_PROBE.
    pub(crate) input_probe: Option<String>,
    /// NFSU2_TRAFFIC_STOPLOG.
    pub(crate) traffic_stoplog: Option<OsString>,
    /// NFSU2_DRAG_LANE_LOG.
    pub(crate) drag_lane_log: Option<OsString>,
    /// NFSU2_INPUT_LOG.
    pub(crate) input_log: Option<OsString>,
    /// NFSU2_BENCH_ROUTEDUMP.
    pub(crate) bench_routedump: Option<OsString>,
    /// NFSU2_DUMP_RIVAL_RECORDS.
    pub(crate) dump_rival_records: Option<OsString>,
    // Behaviour changers (dev-harness only).
    /// NFSU2_TRAFFIC_SEED.
    pub(crate) traffic_seed: Option<String>,
    /// TRAFFIC_MODEL.
    pub(crate) traffic_model: Option<String>,
    /// NFSU2_ENCOUNTER_CAR.
    pub(crate) encounter_car: Option<String>,
    /// NFSU2_BENCH_RIVAL_NOS.
    pub(crate) bench_rival_nos: Option<String>,
    /// NFSU2_BENCH_RIVAL_ROWS.
    pub(crate) bench_rival_rows: Option<String>,
    /// NFSU2_BENCH_RIVAL_CARS.
    pub(crate) bench_rival_cars: Option<String>,
    /// NFSU2_INTRO_NIS.
    pub(crate) intro_nis: Option<String>,
    /// NFSU2_ENDING_NIS.
    pub(crate) ending_nis: Option<String>,
    /// NFSU2_HYDRAULICS.
    pub(crate) hydraulics: Option<String>,
    /// NFSU2_RAIN_CLOCK.
    pub(crate) rain_clock: Option<String>,
    /// NFSU2_COVER_CAMERA.
    pub(crate) cover_camera: Option<String>,
    /// NFSU2_COVER_LAYERS.
    pub(crate) cover_layers: Option<String>,
    /// NFSU2_HDR_PATTERN.
    pub(crate) hdr_pattern: Option<String>,
    /// NFS_FE_HW.
    pub(crate) fe_hw: Option<String>,
    /// NFSU2_GARAGE_NO_AHEAD.
    pub(crate) garage_no_ahead: bool,
    // The harness's command line (dev-harness only).
    /// NFSU2_GARAGE_SYNC.
    pub(crate) garage_sync: bool,
    /// NFSU2_AUDIO_WAV.
    pub(crate) audio_wav: Option<String>,
    /// NFSU2_FLOW_DEVICE.
    pub(crate) flow_device: bool,
    /// NFSU2_DEVICE_CHECK_OK.
    pub(crate) device_check_ok: bool,
    /// NFSU2_AUDIO_REALTIME.
    pub(crate) audio_realtime: bool,
    /// NFSU2_SHOT_REAL_SAVE.
    pub(crate) shot_real_save: bool,
    /// NFSU2_SHOT_DATA.
    pub(crate) shot_data: Option<OsString>,
    /// NFSU2_FLOW_DATA.
    pub(crate) flow_data: Option<OsString>,
    /// NFSU2_STREAK_FORM: `exact`, `cheap` or `auto` (`render::streak`).
    pub(crate) streak_form: Option<String>,
}

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

fn value(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

impl DebugEnv {
    /// The process's environment now.
    pub(crate) fn read() -> DebugEnv {
        let mut e = DebugEnv {
            no_overlap: set("NFSU2_NO_OVERLAP"),
            no_preload: std::env::var_os("NFSU2_NO_PRELOAD"),
            no_programs_ahead: set("NFSU2_NO_PROGRAMS_AHEAD"),
            no_malloc_thp: set("NFSU2_NO_MALLOC_THP"),
            overlay: set("NFSU2_OVERLAY"),
            no_belt: set("NFSU2_NO_BELT"),
            headless_only: set("NFSU2_HEADLESS_ONLY"),
            audio_debug: set("NFSU2_AUDIO_DEBUG"),
            preview_log: set("NFSU2_PREVIEW_LOG"),
            hydro_log: set("HYDRO_LOG"),
            pop_verbose: set("NFSU2_POP_VERBOSE"),
            carlights_log: set("NFSU2_CARLIGHTS_LOG"),
            fog_debug: set("NFSU2_FOG_DEBUG"),
            sight_debug: set("NFSU2_SIGHT_DEBUG"),
            nis_probe: set("NFSU2_NIS_PROBE"),
            flash_probe: set("NFSU2_FLASH_PROBE"),
            car_draw_probe: set("NFSU2_CAR_DRAW_PROBE"),
            debug_actions: set("NFSU2_DEBUG_ACTIONS"),
            debug_markers: set("NFSU2_DEBUG_MARKERS"),
            fx_contacts: set("NFSU2_FX_CONTACTS"),
            probe: set("NFSU2_PROBE"),
            mix_trace: value("NFSU2_MIX_TRACE"),
            replay_camlog: value("NFSU2_REPLAY_CAMLOG"),
            input_probe: value("NFSU2_INPUT_PROBE"),
            traffic_stoplog: std::env::var_os("NFSU2_TRAFFIC_STOPLOG"),
            drag_lane_log: std::env::var_os("NFSU2_DRAG_LANE_LOG"),
            input_log: std::env::var_os("NFSU2_INPUT_LOG"),
            bench_routedump: std::env::var_os("NFSU2_BENCH_ROUTEDUMP"),
            dump_rival_records: std::env::var_os("NFSU2_DUMP_RIVAL_RECORDS"),
            ..Default::default()
        };
        #[cfg(feature = "dev-harness")]
        {
            e.traffic_seed = value("NFSU2_TRAFFIC_SEED");
            e.traffic_model = value("TRAFFIC_MODEL");
            e.encounter_car = value("NFSU2_ENCOUNTER_CAR");
            e.bench_rival_nos = value("NFSU2_BENCH_RIVAL_NOS");
            e.bench_rival_rows = value("NFSU2_BENCH_RIVAL_ROWS");
            e.bench_rival_cars = value("NFSU2_BENCH_RIVAL_CARS");
            e.intro_nis = value("NFSU2_INTRO_NIS");
            e.ending_nis = value("NFSU2_ENDING_NIS");
            e.hydraulics = value("NFSU2_HYDRAULICS");
            e.rain_clock = value("NFSU2_RAIN_CLOCK");
            e.cover_camera = value("NFSU2_COVER_CAMERA");
            e.cover_layers = value("NFSU2_COVER_LAYERS");
            e.hdr_pattern = value("NFSU2_HDR_PATTERN");
            e.fe_hw = value("NFS_FE_HW");
            e.garage_no_ahead = set("NFSU2_GARAGE_NO_AHEAD");
            e.garage_sync = set("NFSU2_GARAGE_SYNC");
            e.audio_wav = value("NFSU2_AUDIO_WAV");
            e.flow_device = set("NFSU2_FLOW_DEVICE");
            e.device_check_ok = set("NFSU2_DEVICE_CHECK_OK");
            e.audio_realtime = set("NFSU2_AUDIO_REALTIME");
            e.shot_real_save = set("NFSU2_SHOT_REAL_SAVE");
            e.shot_data = std::env::var_os("NFSU2_SHOT_DATA");
            e.flow_data = std::env::var_os("NFSU2_FLOW_DATA");
            e.streak_form = value("NFSU2_STREAK_FORM");
        }
        #[cfg(not(feature = "dev-harness"))]
        let _ = &mut e;
        e
    }

    /// `overlap_frames`: each frame's end overlapped with the next frame's
    /// simulation unless NFSU2_NO_OVERLAP is set.
    pub(crate) fn overlap(&self) -> bool {
        !self.no_overlap
    }
}

static ENV: RwLock<Option<Arc<DebugEnv>>> = RwLock::new(None);

/// The knobs as read at the start or by the latest `Game::new`.
pub(crate) fn debug_env() -> Arc<DebugEnv> {
    if let Some(e) = ENV.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return e.clone();
    }
    let mut slot = ENV.write().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert_with(|| Arc::new(DebugEnv::read())).clone()
}

/// Read the knobs again (`Game::new`).
pub(crate) fn reload_debug_env() {
    *ENV.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(DebugEnv::read()));
}

/// `NFSU2_NO_OVERLAP=1`: each frame is submitted and presented before
/// `draw` returns, as before the next frame's simulation was overlapped
/// with its end (for comparisons).
pub(crate) fn overlap_frames() -> bool {
    debug_env().overlap()
}
