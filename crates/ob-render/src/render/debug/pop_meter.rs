//! Pop-in statistics (NFSU2_POP_STATS), the LOD Fade measuring aid (moved from render/mod.rs).

use crate::render::*;

/// Pop-in statistics (`NFSU2_POP_STATS=1`, a measuring aid for the LOD
/// Fade and Keep All Loaded options): per main-view frame, the scenery
/// instances whose drawn opacity jumped by half or more in one frame
/// while in the view (appeared or vanished), the sections that came into
/// the drawn set with a group in the view, and the cars whose LOD switched
/// at once ([`count_car_pop`]).
#[derive(Clone, Debug, Default)]
pub struct PopStats {
    pub frames: u64,
    /// Frames with at least one pop (instance, section or car).
    pub pop_frames: u64,
    pub appeared: u64,
    pub vanished: u64,
    /// The popped instances' sizes (the cull's units, ~ pixels of radius
    /// on a 512-wide view), summed.
    pub popped_size: f64,
    /// The most instances popping in one frame.
    pub worst: u32,
    pub sections: u64,
    pub car_pops: u64,
    pub car_fades: u64,
    /// Per frame: (appeared, vanished, sections, size, car pops).
    pub per_frame: Vec<(u32, u32, u32, f32, u32)>,
    /// Each block's distance from the eye (its nearest group in the view,
    /// metres).
    pub block_distances: Vec<f32>,
}

pub(in crate::render) static POP_STATS: std::sync::Mutex<Option<PopStats>> = std::sync::Mutex::new(None);

pub(in crate::render) static CAR_POPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Whether the pop statistics are kept (`NFSU2_POP_STATS`).
pub fn pop_stats_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| debug_env::nfsu2_pop_stats())
}

/// A car's LOD switched at once (`fade` false) or began a fade.
pub fn count_car_pop(fade: bool) {
    if !pop_stats_enabled() {
        return;
    }
    if fade {
        if let Some(s) = POP_STATS.lock().unwrap().as_mut() {
            s.car_fades += 1;
        }
    } else {
        CAR_POPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The statistics so far, emptied.
pub fn take_pop_stats() -> PopStats {
    POP_STATS.lock().unwrap().take().unwrap_or_default()
}
