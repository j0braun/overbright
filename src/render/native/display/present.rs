use super::*;

/// g_VSyncOn (0x870D24, custom 21): the present parameters' interval
/// (0x5B9B13: D3DPRESENT_INTERVAL_ONE when set, else IMMEDIATE,
/// 0x80000000).
///
/// On is `Fifo` itself, which never tears, as the game's interval one does
/// (`AutoVsync` would take `FifoRelaxed` where offered, which tears when a
/// frame is late); with a variable-refresh monitor Fifo presents each
/// frame as it is done, up to the monitor's top rate. Off is Mailbox by
/// default (agreed deviation: no tearing, no wait; the "Vsync Off Mode"
/// row, [`set_vsync_off_mailbox`]) or Immediate (native, tears), each
/// falling back to the other, then Fifo. `NFSU2_PRESENT=fifo|mailbox|immediate` asks for a mode outright
/// (not a game option; [`present_mode_for`] falls back to Fifo where the
/// surface lacks it).
pub fn present_mode(custom: &[u8; 22]) -> wgpu::PresentMode {
    match crate::env_value!("NFSU2_PRESENT") {
        Some("fifo") => return wgpu::PresentMode::Fifo,
        Some("mailbox") => return wgpu::PresentMode::Mailbox,
        Some("immediate") => return wgpu::PresentMode::Immediate,
        _ => {}
    }
    if custom[21] != 0 {
        wgpu::PresentMode::Fifo
    } else if VSYNC_OFF_MAILBOX.load(Ordering::Relaxed) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::Immediate
    }
}

/// [`present_mode`] as a surface offering `supported` can take: an explicit
/// mode it lacks becomes Fifo (every surface has it).
pub fn present_mode_for(custom: &[u8; 22], supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    match present_mode(custom) {
        m @ (wgpu::PresentMode::AutoVsync | wgpu::PresentMode::AutoNoVsync) => m,
        m if supported.contains(&m) => m,
        // Mailbox missing: Immediate (vsync off still means no wait).
        wgpu::PresentMode::Mailbox if supported.contains(&wgpu::PresentMode::Immediate) => wgpu::PresentMode::Immediate,
        _ => wgpu::PresentMode::Fifo,
    }
}

/// Full Screen Anti-Aliasing (custom 14, 0x870D08): the scene's sample
/// count. The game asks for MultiSampleType 1 (non-maskable) with the
/// option as the quality level (0x5B9B3B), whose sample count is the
/// driver's; **inferred**: level v as 2^v samples (2×, 4×, 8×). Clamped to
/// what the adapter supports for the scene's targets (the largest power of
/// two not above, else none); `NFSU2_MSAA=n` asks for n samples outright,
/// the Graphics page's Anti-Aliasing MSAA 2× / 4× / 8× (clone deviation,
/// [`crate::qol::msaa_override`]) instead of the option's.
pub fn msaa_samples(custom: &[u8; 22], gpu: &crate::render::Gpu, opts: &crate::render::RenderOptions) -> u32 {
    // Explicit enhancement controls own AA; diagnostic override applies only
    // when the enhancements leave MSAA to the native setting.
    let wanted = opts
        .msaa_override()
        .or_else(|| crate::env_parsed!("NFSU2_MSAA", u32))
        .unwrap_or(if custom[14] == 0 { 1 } else { 1 << custom[14].min(4) });
    let _ = gpu; // Support is installed by Gpu::new for this device.
    opts.supported_msaa(wanted)
}

/// [`msaa_samples`] for a drive's frames, whatever the frame drawing now:
/// a drive's frames run the temporal pass whenever it is chosen
/// (`qol::set_temporal_frame` on the game's world), with no samples; a
/// menu's, a garage's or a loading screen's keep the native MSAA. What a
/// drive's programs are made ahead for (docs/first-use-builds.md).
pub fn drive_msaa_samples(custom: &[u8; 22], gpu: &crate::render::Gpu, opts: &crate::render::RenderOptions) -> u32 {
    msaa_samples(custom, gpu, &crate::render::RenderOptions { temporal_frame: opts.temporal, ..*opts })
}

/// [`msaa_samples`] for a front-end scene's frames (the main menu's and the
/// shops' garage scenes): the native MSAA, never the temporal pass
/// (`qol::set_temporal_frame` leaves it off there), whatever a drive takes.
pub fn front_msaa_samples(custom: &[u8; 22], gpu: &crate::render::Gpu, opts: &crate::render::RenderOptions) -> u32 {
    msaa_samples(custom, gpu, &crate::render::RenderOptions { temporal_frame: false, ..*opts })
}

/// `NFSU2_FRAME_CAP=fps`, else the Enhancements page's Frame Rate Cap: an
/// optional frame cap (off by default, native uncapped; not a game option).
/// The game loop then starts a frame no sooner than 1 / fps after the last
/// one: it sleeps to within a millisecond of that and spins the rest,
/// before input and the simulation, so the cap adds no latency.
pub fn frame_cap() -> Option<std::time::Duration> {
    static CAP: std::sync::OnceLock<Option<std::time::Duration>> = std::sync::OnceLock::new();
    CAP.get_or_init(|| {
        crate::render::debug_env::nfsu2_frame_cap()
            .filter(|&f| f > 0.0)
            .map(|f| std::time::Duration::from_secs_f64(1.0 / f))
    })
    .or_else(crate::qol::frame_cap)
}

/// The six sizes the game's Resolution row offers (0x5BF620, tables
/// 0x800538 / 0x800550), each listed when the adapter has the mode
/// (0x5BE5B0: EnumAdapterModes in X8R8G8B8 marks 0x86E880[i]).
pub const NATIVE_MODES: [(u32, u32); 6] =
    [(640, 480), (800, 600), (1024, 768), (1280, 960), (1280, 1024), (1600, 1200)];

/// Sizes the clone always offers beyond the native six (an agreed
/// deviation, 2026-10-31): 1080p, 1440p and 2160p. Ultrawide sizes
/// (2560 × 1080, 3440 × 1440, 5120 × 1440 …) come from the monitor's own
/// modes when it has them.
pub const EXTRA_MODES: [(u32, u32); 3] = [(1920, 1080), (2560, 1440), (3840, 2160)];

/// The clone's window size when options.txt names none (what it opened
/// at before the Resolution row was applied).
pub const DEFAULT_MODE: (u32, u32) = (1280, 960);

/// The Resolution row's list: the native six, [`EXTRA_MODES`] and the
/// monitor's modes (`monitor`, at least 640 × 480), deduplicated and in
/// the native table's order (by width, then height).
pub fn mode_list(monitor: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut all: Vec<(u32, u32)> = NATIVE_MODES
        .iter()
        .chain(EXTRA_MODES.iter())
        .copied()
        .chain(monitor.iter().copied().filter(|&(w, h)| w >= 640 && h >= 480))
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

static MODES: std::sync::RwLock<Vec<(u32, u32)>> = std::sync::RwLock::new(Vec::new());

/// The list the Resolution row walks, set when the window opens (the
/// monitor's modes); [`mode_list`] of none before.
pub fn modes() -> Vec<(u32, u32)> {
    let m = MODES.read().unwrap();
    if m.is_empty() { mode_list(&[]) } else { m.clone() }
}

pub fn set_modes(list: Vec<(u32, u32)>) {
    *MODES.write().unwrap() = list;
}

/// The present mode for g_VSyncOn off (clone option, agreed deviation):
/// Mailbox (no tearing, no added latency, the frame rate uncapped) where
/// the surface offers it, else Immediate; or Immediate (the native
/// D3DPRESENT_INTERVAL_IMMEDIATE, which tears).
static VSYNC_OFF_MAILBOX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_vsync_off_mailbox(on: bool) {
    VSYNC_OFF_MAILBOX.store(on, Ordering::Relaxed);
}

/// The frame limiter's period during an in-engine scene: 1/30 s.
///
/// SPEED2.EXE's loop (0x5811D0) hands each iteration's measured time to
/// 0x57EAE0, which, when the time is under the period T (0x865558) and
/// above 0, spins on QueryPerformanceCounter (0x43BDF0 / 0x43BE20) until
/// T − dt more has passed and takes T as the frame's time. T is 1/120 s
/// (0x7FB710, set at start by 0x77DCE0 and by 0x576CE0); a scene's start
/// (0x437910 → 0x435F40) makes it 1/30 (0x576CC0(30.0)) when the scene
/// takes the cars (property 1, which every scene gets, 0x43ACF0), and the
/// cars' release (0x437A00 → 0x576CE0) makes it 1/120 again. Only the
/// scene's 30 is ported (a design choice): the clone's own frames are
/// not capped outside scenes.
pub const SCENE_FRAME: std::time::Duration = std::time::Duration::from_nanos(1_000_000_000 / 30);

/// How long before a wait's end [`wait_until`] stops sleeping and spins:
/// 1 ms on Linux / macOS (as before), 2 ms on Windows, whose
/// sleeps wake up to about a millisecond late even with the 1 ms timer
/// resolution the game holds (`platform::timer`) or std's high-resolution
/// waitable timer (Windows 10 1803+) **[unverified on real Windows]**.
pub const fn sleep_margin(windows: bool) -> std::time::Duration {
    std::time::Duration::from_millis(if windows { 2 } else { 1 })
}

/// Waits until `due`: sleeps to within [`sleep_margin`] of it and spins the
/// rest (the game spins all of it, 0x57EB74), so the wait ends on time
/// without holding a core for the whole period.
pub fn wait_until(due: std::time::Instant) {
    let margin = sleep_margin(cfg!(windows));
    let now = std::time::Instant::now();
    if due > now + margin {
        std::thread::sleep(due - now - margin);
    }
    while std::time::Instant::now() < due {
        std::hint::spin_loop();
    }
}

/// The period a frame waits out after the last one's start: the longer of
/// `NFSU2_FRAME_CAP`'s and, during a scene that holds the cars, the
/// scene's 1/30 s. None: no wait.
pub fn frame_period(scene: bool) -> Option<std::time::Duration> {
    let scene = scene.then_some(SCENE_FRAME);
    match (frame_cap(), scene) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}
