//! Adapter, device and queue (moved from render/mod.rs).

use crate::render::*;
use crate::render::core::backend;

/// Pipelines built so far (a measuring aid: the benches count the
/// ones built after a load's first frame, each a first-use hitch). Counted
/// where a pipeline is made: [`Gpu::pipeline_cache`] (asked once per
/// pipeline), the world's and the cars' shared pipeline sets, the weather
/// passes'. A relaxed atomic add per pipeline, nothing per frame.
pub(crate) static PIPELINES_BUILT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `NFSU2_PIPELINE_LOG=1` (measuring aid): each one's call site
/// on stderr, read once a run.
#[track_caller]
pub(crate) fn note_pipeline(tag: u64) {
    note_pipeline_at(std::panic::Location::caller(), tag);
}

/// [`note_pipeline`] for a pipeline built on behalf of `at` (a shared
/// set's caller).
pub(crate) fn note_pipeline_at(at: &'static std::panic::Location<'static>, tag: u64) {
    PIPELINES_BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    note_build(at, tag);
    if *crate::render::debug_env::once::pipeline_log() {
        eprintln!("pipeline built: {}:{}", at.file(), at.line());
    }
}

/// Where pipelines and shader modules were made for the `Gpu` whose
/// [`Gpu::build_tag`] is given to [`record_builds`], while it records (a
/// measuring aid: the first-frame test names what a drawn frame made;
/// other devices', e.g. parallel tests', are not recorded). Off: one
/// relaxed load per build.
static RECORDING: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RECORDED: std::sync::Mutex<Vec<&'static std::panic::Location<'static>>> = std::sync::Mutex::new(Vec::new());
/// The next [`Gpu::build_tag`] (0: none).
static NEXT_TAG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A pipeline or module made at `at` for the `Gpu` tagged `tag`.
pub(crate) fn note_build(at: &'static std::panic::Location<'static>, tag: u64) {
    if RECORDING.load(std::sync::atomic::Ordering::Relaxed) == tag {
        RECORDED.lock().unwrap_or_else(|e| e.into_inner()).push(at);
    }
}

/// Starts recording where pipelines and modules are made for the `Gpu`
/// tagged `tag` (None: stops); returns the places recorded since the last
/// call ("file:line").
pub fn record_builds(tag: Option<u64>) -> Vec<String> {
    let mut r = RECORDED.lock().unwrap_or_else(|e| e.into_inner());
    RECORDING.store(tag.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
    std::mem::take(&mut *r).into_iter().map(|at| format!("{}:{}", at.file(), at.line())).collect()
}

/// The count of [`note_pipeline`]s so far.
pub fn pipelines_built() -> u64 {
    PIPELINES_BUILT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Adapter, device and queue, created with or without a window surface.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// What the device's car renderers share ([`car::CarShared`]), made
    /// with the first.
    pub(crate) car: std::sync::OnceLock<std::sync::Arc<car::CarShared>>,
    /// What the device's world renderers share ([`WorldShared`]).
    pub(crate) world: std::sync::OnceLock<WorldShared>,
    /// The driver's pipeline cache kept on disk (Vulkan only).
    pub(crate) pipeline_cache: Option<std::sync::Arc<DiskPipelineCache>>,
    /// The shared transient render targets ([`pool`]).
    pub pool: std::sync::Arc<pool::TargetPool>,
    /// The device's programs made once ([`crate::render::core::device_cache`]).
    pub(crate) device_cache: std::sync::Arc<crate::render::core::device_cache::DeviceCache>,
    /// This device's tag for [`record_builds`] (unique in the process).
    pub(crate) tag: u64,
}

/// Which adapter (the software-GPU check reads it).
fn log_adapter(adapter: &wgpu::Adapter) {
    let i = adapter.get_info();
    crate::diag::events::record(
        crate::diag::events::Kind::Gpu,
        format!("adapter chosen: {} ({:?}, {:?}, driver {} {})", i.name, i.device_type, i.backend, i.driver, i.driver_info),
    );
    log::info!("GPU adapter: {} ({:?}, {:?})", adapter.get_info().name, adapter.get_info().device_type, adapter.get_info().backend);
}

/// A device opened before the window ([`Gpu::ahead`]) and the instance it
/// was made on, which the window's surface is made from.
pub struct GpuAhead {
    pub instance: wgpu::Instance,
    opened: Result<Gpu>,
}

impl GpuAhead {
    /// The device opened ahead when it is the one [`Gpu::new`] picks for
    /// `surface` (made from [`GpuAhead::instance`]), its facts set as
    /// `Gpu::new` sets them; else None, and the caller opens one as before.
    /// The same device: on Vulkan's one adapter asked for, the adapters that
    /// can show `surface` are a subset of all of them in the same order
    /// (wgpu ranks them by kind, a stable sort), so the first of all is the
    /// first of those when it can show it; on Windows the candidates are
    /// tried in the same ranked order (`backend::adapter_order`), so the
    /// first one, opened, is the pick when it presents.
    pub fn take(self, surface: &wgpu::Surface<'_>) -> Option<Gpu> {
        let gpu = match self.opened {
            Ok(g) => g,
            Err(e) => {
                log::info!("gpu ahead: none ({e:#}); opened with the window");
                return None;
            }
        };
        if !gpu.adapter.is_surface_supported(surface) {
            log::info!("gpu ahead: {} cannot show the window; another opened with it", gpu.adapter.get_info().name);
            return None;
        }
        crate::qol::crash::set_adapter(&gpu.adapter.get_info());
        log_adapter(&gpu.adapter);
        gpu.publish();
        Some(gpu)
    }
}

impl Gpu {
    pub async fn new(compatible_surface: Option<&wgpu::Surface<'_>>, instance: wgpu::Instance) -> Result<Gpu> {
        // Windows (`backend`): every Vulkan and DX12 adapter
        // ranked, the device asked from each in turn until one opens.
        if cfg!(windows) {
            let adapters = instance.enumerate_adapters(wgpu::Backends::all()).await;
            let cands: Vec<backend::Candidate> = adapters
                .iter()
                .map(|a| backend::Candidate {
                    backend: a.get_info().backend,
                    device_type: a.get_info().device_type,
                    presents: compatible_surface.is_none_or(|s| a.is_surface_supported(s)),
                })
                .collect();
            for a in &adapters {
                let i = a.get_info();
                log::info!("GPU candidate: {} ({:?}, {:?}, driver {})", i.name, i.device_type, i.backend, i.driver);
                crate::diag::events::record(
                    crate::diag::events::Kind::Gpu,
                    format!("candidate: {} ({:?}, {:?}, driver {})", i.name, i.device_type, i.backend, i.driver),
                );
            }
            let mut last = None;
            for i in backend::adapter_order(&cands) {
                match Self::open(instance.clone(), adapters[i].clone()).await {
                    Ok(g) => return Ok(g),
                    Err(e) => {
                        log::warn!("GPU {} ({:?}) did not open: {e:#}; trying the next", adapters[i].get_info().name, adapters[i].get_info().backend);
                        crate::diag::events::record(
                            crate::diag::events::Kind::Gpu,
                            format!("{} ({:?}) did not open: {e:#}", adapters[i].get_info().name, adapters[i].get_info().backend),
                        );
                        last = Some(e);
                    }
                }
            }
            return Err(last.unwrap_or_else(|| anyhow::anyhow!("no GPU adapter (Vulkan or DX12) that can show the window")));
        }
        // Linux: the player's line (no Vulkan loader or driver, and the
        // packages to install; `platform::vulkan`).
        let adapter = instance.request_adapter(&Gpu::adapter_options(compatible_surface)).await.with_context(|| {
            if cfg!(target_os = "linux") { crate::platform::vulkan::no_adapter_line() } else { "no GPU adapter".into() }
        })?;
        Self::open(instance, adapter).await
    }

    /// The game's device opened before its window exists ([`GpuAhead`]): on
    /// a thread of its own at the start, beside the game's data and the
    /// window's own making (the Vulkan driver's loading, the instance, the
    /// adapters and the device took 20–170 ms of the main thread there).
    /// The adapter is picked as [`Gpu::new`] picks it with no surface;
    /// nothing the rest of the game reads (the adapter's facts for the
    /// settings, the crash report) is set until [`GpuAhead::take`] accepts
    /// it. Call it after the options are in force (the Ray Traced tier asks
    /// for ray queries as the device is made).
    pub fn ahead() -> GpuAhead {
        let _t = crate::prof::span("gpu ahead");
        let instance = Gpu::new_instance();
        let opened = pollster::block_on(Self::pick_quiet(&instance));
        GpuAhead { instance, opened }
    }

    /// [`Gpu::new`]'s pick with no surface, quiet ([`Gpu::open_quiet`]): on
    /// Windows only the first ranked candidate (one that does not open
    /// leaves the pick to `Gpu::new`, which tries the next ones).
    async fn pick_quiet(instance: &wgpu::Instance) -> Result<Gpu> {
        if cfg!(windows) {
            let adapters = instance.enumerate_adapters(wgpu::Backends::all()).await;
            let cands: Vec<backend::Candidate> = adapters
                .iter()
                .map(|a| backend::Candidate {
                    backend: a.get_info().backend,
                    device_type: a.get_info().device_type,
                    presents: true,
                })
                .collect();
            let order = backend::adapter_order(&cands);
            let first = *order.first().context("no GPU adapter (Vulkan or DX12)")?;
            return Self::open_quiet(instance.clone(), adapters[first].clone()).await;
        }
        let adapter = instance.request_adapter(&Gpu::adapter_options(None)).await.context("no GPU adapter")?;
        Self::open_quiet(instance.clone(), adapter).await
    }

    /// The device and queue on `adapter`, with every optional feature it
    /// offers (each has a fallback, `backend::FALLBACKS`).
    async fn open(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Gpu> {
        crate::qol::crash::set_adapter(&adapter.get_info());
        log_adapter(&adapter);
        let gpu = Self::open_quiet(instance, adapter).await?;
        gpu.publish();
        Ok(gpu)
    }

    /// [`Gpu::open`]'s device and queue, and the driver's pipeline cache
    /// read from disk, with nothing set for the rest of the game
    /// ([`Gpu::publish`]).
    async fn open_quiet(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Gpu> {
        // Block compression lets the DXT textures go to the GPU untouched.
        // Immediates carry the world draws' texture layer ([`TextureArrays`])
        // and the car draws' entry (`CarShared::immediates`).
        let mut wanted = wgpu::Features::TEXTURE_COMPRESSION_BC | wgpu::Features::IMMEDIATES;
        // The world's texture arrays in one group ([`bound_arrays`]).
        wanted |= wgpu::Features::TEXTURE_BINDING_ARRAY | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY;
        // The anti-aliasing's sample counts beyond WebGPU's 4
        // (`display::msaa_samples`).
        wanted |= wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        // The environment cube's faces in one pass ([`EnvMultiview`]).
        if adapter.limits().max_multiview_view_count >= 6 {
            wanted |= wgpu::Features::MULTIVIEW;
        }
        // Timestamp queries where the adapter has them: the bench's GPU
        // timing (`prof::gpu`) and the streak buffer's cost measurement in
        // the window (the game's `render::streak`; it falls back to the
        // frames' wall time without them).
        wanted |= crate::prof::gpu::FEATURES;
        // The pipeline cache on disk ([`DiskPipelineCache`]).
        wanted |= wgpu::Features::PIPELINE_CACHE;
        // The half-size smoke's composite (`gpu_particles::HalfSize`).
        wanted |= wgpu::Features::DUAL_SOURCE_BLENDING;
        // The lamps' light-cluster loop one cluster per subgroup at a time
        // (`registry::subgroup_clusters`; the same sums, docs/notes/graphics/lamps-shadows.md).
        wanted |= wgpu::Features::SUBGROUP;
        let asked = wanted;
        // a check aid, docs/developing/software-gpu.md: NFSU2_GPU_DROP=bc,
        // multiview, … asks for the device without those features, as a GPU
        // without them gets it (each has a fallback).
        if let Ok(drop) = crate::render::debug_env::nfsu2_gpu_drop() {
            for name in drop.split(',') {
                wanted &= !match name.trim() {
                    "bc" => wgpu::Features::TEXTURE_COMPRESSION_BC,
                    "immediates" => wgpu::Features::IMMEDIATES,
                    "binding_array" => {
                        wgpu::Features::TEXTURE_BINDING_ARRAY | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY
                    }
                    "format_features" => wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    "multiview" => wgpu::Features::MULTIVIEW,
                    "pipeline_cache" => wgpu::Features::PIPELINE_CACHE,
                    "dual_source" => wgpu::Features::DUAL_SOURCE_BLENDING,
                    "subgroup" => wgpu::Features::SUBGROUP,
                    _ => wgpu::Features::empty(),
                };
            }
        }
        let required_features = adapter.features() & wanted;
        // Windows: one line naming what the device goes without (the
        // adapter lacks it, or NFSU2_GPU_DROP) and the fallback each uses
        // (DX12: pipeline cache, six-view multiview).
        let views =
            if required_features.contains(wgpu::Features::MULTIVIEW) { adapter.limits().max_multiview_view_count } else { 0 };
        if let Some(line) =
            backend::missing_line(adapter.get_info().backend, &backend::missing(asked, required_features, views))
        {
            log::warn!("{line}");
        }
        // docs/notes/graphics/reflections.md "Ray-traced reflections (experimental)": ray queries when the Ray Traced tier
        // is chosen, on a discrete Vulkan GPU that offers them (experimental
        // in wgpu); without them if refused.
        let rt = rt::wanted(&adapter);
        let request = |features: wgpu::Features| {
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("nfsu2"),
                required_features: features,
                required_limits: adapter.limits(),
                // SAFETY: only `rt::FEATURE` is experimental; it is used by
                // the Ray Traced tier alone (`render::rt`).
                experimental_features: if features.contains(rt::FEATURE) {
                    unsafe { wgpu::ExperimentalFeatures::enabled() }
                } else {
                    wgpu::ExperimentalFeatures::disabled()
                },
                memory_hints: memory_hints(),
                ..Default::default()
            })
        };
        let (device, queue) = match request(required_features | rt).await {
            Ok(d) => d,
            Err(e) if !rt.is_empty() => {
                log::warn!("ray query refused ({e}); without it");
                request(required_features).await.context("requesting device")?
            }
            Err(e) => return Err(e).context("requesting device"),
        };
        let pipeline_cache = DiskPipelineCache::open(&adapter, &device).map(std::sync::Arc::new);
        Ok(Gpu {
            instance,
            adapter,
            device,
            queue,
            car: Default::default(),
            world: Default::default(),
            pipeline_cache,
            pool: Default::default(),
            device_cache: std::sync::Arc::new(crate::render::core::device_cache::DeviceCache::new()),
            tag: NEXT_TAG.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    /// What the rest of the game reads of the device just opened: ray
    /// queries made and offered, the texture size limit and the adapter's
    /// kind, its memory and the presets' GPU class, the sample counts and
    /// HDR the menus offer.
    fn publish(&self) {
        let (adapter, device) = (&self.adapter, &self.device);
        // A lost device (a driver reset, a removed GPU) in the log with the
        // events before it (docs/troubleshooting.md); a device dropped at
        // the game's end is no loss.
        device.set_device_lost_callback(|reason, message| {
            if reason == wgpu::DeviceLostReason::Destroyed {
                return;
            }
            log::error!("GPU device lost ({reason:?}): {message}");
            crate::diag::events::record(crate::diag::events::Kind::Gpu, format!("device lost ({reason:?}): {message}"));
            crate::diag::events::dump_to_log("GPU device lost");
            crate::qol::log_file::flush(std::time::Duration::from_millis(300));
        });
        crate::qol::set_rt_support(device.features().contains(rt::FEATURE));
        crate::qol::set_rt_offered(rt::adapter_can(adapter));
        crate::qol::set_gpu_caps(device.limits().max_texture_dimension_2d, adapter.get_info().device_type);
        crate::upscale_pack::set_adapter(&adapter.get_info());
        crate::qol::set_gpu_class(crate::qol::classify_gpu(
            adapter.get_info().device_type,
            crate::upscale_pack::vram_bytes(),
        ));
        log::info!("GPU class: {} (the presets' effect tiers)", crate::qol::gpu_class().name());
        let specific = device.features().contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        let mut mask = 1;
        for n in [2u32, 4, 8, 16] {
            if (specific || n == 4)
                && [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm, DEPTH_FORMAT].iter().all(|&f| {
                    let flags = adapter.get_texture_format_features(f).flags;
                    flags.sample_count_supported(n)
                        && (f == DEPTH_FORMAT || flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE))
                })
            {
                mask |= 1 << n.ilog2();
            }
        }
        crate::qol::set_msaa_support(mask);
        crate::qol::set_hdr_support(hdr_supported(adapter, specific, mask));
    }

    /// The pipeline cache to build pipelines with (None: none). Every
    /// pipeline built through it is counted ([`pipelines_built`]).
    #[track_caller]
    pub(crate) fn pipeline_cache(&self) -> Option<&wgpu::PipelineCache> {
        note_pipeline(self.tag);
        self.disk_pipeline_cache()
    }

    /// The pipeline cache, not counted: for the pipelines made through
    /// [`crate::render::core::device_cache`], which counts the ones it
    /// makes (a pipeline it already has is not made again).
    pub(crate) fn disk_pipeline_cache(&self) -> Option<&wgpu::PipelineCache> {
        self.pipeline_cache.as_ref().map(|c| &c.cache)
    }

    /// Whether the backend keeps its pipelines in a cache on disk between
    /// runs ([`DiskPipelineCache`]: Vulkan).
    pub fn has_pipeline_cache(&self) -> bool {
        self.pipeline_cache.is_some()
    }

    /// This device's tag for [`record_builds`].
    pub fn build_tag(&self) -> u64 {
        self.tag
    }

    /// Keep the pipelines built so far for the next run (when the cache
    /// has grown): after a load has built its pipelines.
    pub fn save_pipeline_cache(&self) {
        if crate::render::debug_env::nfsu2_pipeline_log() {
            eprintln!("pipeline cache saved ({} pipelines built so far)", pipelines_built());
        }
        if let Some(c) = &self.pipeline_cache {
            c.save();
        }
    }

    /// The device's [`WorldShared`].
    pub(crate) fn world_shared(&self) -> &WorldShared {
        self.world.get_or_init(|| WorldShared::new(self.disk_pipeline_cache().cloned(), self.tag))
    }

    /// The device's [`car::CarShared`].
    pub(crate) fn car_shared(&self) -> std::sync::Arc<car::CarShared> {
        self.car
            .get_or_init(|| std::sync::Arc::new(car::CarShared::new(&self.device, self.disk_pipeline_cache().cloned(), self.tag)))
            .clone()
    }

    /// A handle on the same device for a thread that builds car renderers
    /// ahead (the garage's FE car, `draw_garage`): the device, queue and
    /// [`car::CarShared`] are this one's (its pipelines too); the world
    /// share is not (a car renderer does not use it).
    pub fn car_worker(&self) -> Gpu {
        let car = std::sync::OnceLock::new();
        let _ = car.set(self.car_shared());
        Gpu {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
            car,
            world: Default::default(),
            pipeline_cache: self.pipeline_cache.clone(),
            pool: self.pool.clone(),
            device_cache: self.device_cache.clone(),
            tag: self.tag,
        }
    }

    /// How the game asks for its adapter (shared with `game --probe`, so
    /// the launcher learns the adapter the game picks).
    pub fn adapter_options<'a, 'b>(
        compatible_surface: Option<&'a wgpu::Surface<'b>>,
    ) -> wgpu::RequestAdapterOptions<'a, 'b> {
        wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }
    }

    pub fn new_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            // `WGPU_BACKEND=vulkan|dx12|metal` picks one (to try another
            // where a driver's acquire stalls, wgpu #9559); else Vulkan,
            // Metal and DX12, the first adapter found.
            // Windows: Vulkan and DX12 (`backend::instance_backends`).
            backends: backend::instance_backends(cfg!(windows), wgpu::Backends::from_env()),
            // wgpu's own switches (`WGPU_DEBUG`, `WGPU_VALIDATION`, ...)
            // on top of the build's default, only when set: `WGPU_DEBUG=1`
            // puts source lines into the shaders handed to the driver,
            // which avoids a pipeline-compile crash in AMD's Windows Vulkan
            // driver (wgpu #6329; docs/troubleshooting.md "A crash in the
            // graphics driver").
            flags: wgpu::InstanceFlags::default().with_env(),
            memory_budget_thresholds: Default::default(),
            backend_options: backend_options(),
            display: None,
        })
    }
}

/// The backends' options: on Windows DX12's shader compiler (DXC next to
/// the exe, else FXC, one log line; `backend::dx12_compiler`); elsewhere
/// wgpu's defaults, as before.
fn backend_options() -> wgpu::BackendOptions {
    #[cfg(windows)]
    {
        let choice = backend::dx12_for_this_exe();
        if matches!(choice, backend::Dx12Choice::Fxc(_)) {
            log::warn!("{}", choice.log_line());
        } else {
            log::info!("{}", choice.log_line());
        }
        let mut o = wgpu::BackendOptions::default();
        o.dx12.shader_compiler = choice.compiler();
        o
    }
    #[cfg(not(windows))]
    {
        Default::default()
    }
}

/// Whether [`HDR_FORMAT`] can stand in for the 8-bit scene on this
/// adapter: renderable, blendable and filterable, and multisampled (with
/// resolve) at every sample count in `msaa_mask` (the counts the
/// Graphics page offers). Else High Precision Post stays off (native path).
pub(crate) fn hdr_supported(adapter: &wgpu::Adapter, specific: bool, msaa_mask: u8) -> bool {
    use wgpu::TextureFormatFeatureFlags as F;
    let flags = adapter.get_texture_format_features(HDR_FORMAT).flags;
    let usages = adapter.get_texture_format_features(HDR_FORMAT).allowed_usages;
    if !usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        || !flags.contains(F::BLENDABLE | F::FILTERABLE)
    {
        return false;
    }
    [2u32, 4, 8, 16]
        .into_iter()
        .filter(|n| msaa_mask & (1 << n.ilog2()) != 0)
        .all(|n| (!specific && n == 4) || (flags.sample_count_supported(n) && flags.contains(F::MULTISAMPLE_RESOLVE)))
}

/// A scene's depth: also readable (the Graphics page's ambient occlusion
/// reads it; nothing native does).
/// The device memory allocator's block sizes (docs/notes/performance/gpu.md
/// "Render-target memory"): device blocks of 4 MiB growing to 32 MiB,
/// so a target over 32 MiB (every screen-sized one at 1440p and up) gets
/// a block of exactly its size. wgpu's default (128–256 MiB blocks) left a
/// 4K target of 130 MiB alone in a 256 MiB block: 0.5–1 GB reserved and
/// never used at 4K Ultra/Extreme, and 130–450 MiB at 1080p Original, run
/// to run different. Where memory sits changes no pixel.
/// `NFSU2_GPU_BLOCK_MIB=min,max` (or `perf`: wgpu's default) for A/B.
pub(crate) fn memory_hints() -> wgpu::MemoryHints {
    memory_hints_from(debug_env::nfsu2_gpu_block_mib().as_deref())
}

pub(crate) fn memory_hints_from(set: Option<&str>) -> wgpu::MemoryHints {
    const MIB: u64 = 1024 * 1024;
    if set == Some("perf") {
        return wgpu::MemoryHints::Performance;
    }
    let (min, max) = set
        .and_then(|v| {
            v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u64>().ok()?, b.trim().parse::<u64>().ok()?)))
        })
        .unwrap_or((4, 32));
    wgpu::MemoryHints::Manual { suballocated_device_memory_block_size: min * MIB..max * MIB }
}

#[cfg(test)]
mod memory_hints_tests {
    use crate::render::*;

    #[test]
    fn blocks_default_to_4_to_32_mib_and_parse_the_override() {
        const MIB: u64 = 1024 * 1024;
        let manual = |h: wgpu::MemoryHints| match h {
            wgpu::MemoryHints::Manual { suballocated_device_memory_block_size: r } => Some((r.start / MIB, r.end / MIB)),
            _ => None,
        };
        assert_eq!(manual(memory_hints_from(None)), Some((4, 32)));
        assert_eq!(manual(memory_hints_from(Some("8, 64"))), Some((8, 64)));
        assert_eq!(manual(memory_hints_from(Some("junk"))), Some((4, 32)));
        assert!(matches!(memory_hints_from(Some("perf")), wgpu::MemoryHints::Performance));
    }
}
