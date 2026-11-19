//! The device's programs made once: shader modules by their source text,
//! bind group layouts by their entries, pipeline layouts by their groups,
//! render and compute pipelines by their whole descriptor.
//!
//! The passes ask for their programs through these functions instead of
//! the device: the same descriptor gives the same object, so a pass made
//! again (switched off and on, the upscaling stage's second post chain, a
//! warm-up's throwaway copy, docs/developing/first-use-builds.md) builds nothing, and
//! a warm-up made before a frame that needs them leaves that frame nothing
//! to build. The objects are the ones the device would make for the same
//! descriptor (same source, same states), so what is drawn is the same.
//!
//! Each [`crate::render::Gpu`] holds its device's cache (shared with the
//! handles made from it for worker threads) and drops it with the device.
//! Labels are not part of a key (an object keeps the first caller's).
//! Two threads asking for the same key build it once; the second waits.
//! (The cache is reached through the `Gpu`, not looked up by the device:
//! wgpu compares devices by their id within an instance, and a process may
//! hold several instances, e.g. tests side by side.)

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::render::Gpu;

/// Shader modules made so far (a measuring aid, as
/// [`crate::render::pipelines_built`]: the benches and the first-frame
/// test count the ones made inside a drawn frame).
static MODULES_BUILT: AtomicU64 = AtomicU64::new(0);
/// Wall time spent making modules and pipelines through this cache (ns).
static MODULE_NS: AtomicU64 = AtomicU64::new(0);
static PIPELINE_NS: AtomicU64 = AtomicU64::new(0);

/// Modules made so far (every module of the renderers is made here).
pub fn modules_built() -> u64 {
    MODULES_BUILT.load(Ordering::Relaxed)
}

/// Wall milliseconds spent making (modules, pipelines) through this
/// cache so far, summed over threads.
pub fn build_ms() -> (f64, f64) {
    (MODULE_NS.load(Ordering::Relaxed) as f64 / 1e6, PIPELINE_NS.load(Ordering::Relaxed) as f64 / 1e6)
}

type Slot<V> = Arc<OnceLock<V>>;

/// One map's entries; the map's lock is held only to find or add a slot.
struct Map<K, V>(Mutex<HashMap<K, Slot<V>>>);

impl<K: Hash + Eq, V: Clone> Map<K, V> {
    fn new() -> Self {
        Map(Mutex::new(HashMap::new()))
    }

    fn get_or(&self, key: K, make: impl FnOnce() -> V) -> V {
        let slot = self.0.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default().clone();
        slot.get_or_init(make).clone()
    }
}

#[derive(PartialEq, Eq, Hash)]
struct StageKey {
    module: wgpu::ShaderModule,
    entry: Option<String>,
    /// Overridable constants: name and the value's bits.
    constants: Vec<(String, u64)>,
    zero_init: bool,
}

impl StageKey {
    fn of(module: &wgpu::ShaderModule, entry: Option<&str>, o: &wgpu::PipelineCompilationOptions<'_>) -> StageKey {
        StageKey {
            module: module.clone(),
            entry: entry.map(str::to_owned),
            constants: o.constants.iter().map(|(n, v)| ((*n).to_owned(), v.to_bits())).collect(),
            zero_init: o.zero_initialize_workgroup_memory,
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
struct RenderKey {
    layout: Option<wgpu::PipelineLayout>,
    vertex: StageKey,
    buffers: Vec<Option<(u64, wgpu::VertexStepMode, Vec<wgpu::VertexAttribute>)>>,
    primitive: wgpu::PrimitiveState,
    depth_stencil: Option<wgpu::DepthStencilState>,
    multisample: wgpu::MultisampleState,
    fragment: Option<(StageKey, Vec<Option<wgpu::ColorTargetState>>)>,
    multiview_mask: Option<std::num::NonZeroU32>,
}

#[derive(PartialEq, Eq, Hash)]
struct ComputeKey {
    layout: Option<wgpu::PipelineLayout>,
    stage: StageKey,
}

/// A device's caches.
pub struct DeviceCache {
    /// WGSL text → module.
    modules: Map<(u64, Arc<str>), wgpu::ShaderModule>,
    bind_group_layouts: Map<Vec<wgpu::BindGroupLayoutEntry>, wgpu::BindGroupLayout>,
    pipeline_layouts: Map<(Vec<Option<wgpu::BindGroupLayout>>, u32), wgpu::PipelineLayout>,
    render: Map<RenderKey, wgpu::RenderPipeline>,
    compute: Map<ComputeKey, wgpu::ComputePipeline>,
}

impl DeviceCache {
    pub(crate) fn new() -> DeviceCache {
        DeviceCache {
            modules: Map::new(),
            bind_group_layouts: Map::new(),
            pipeline_layouts: Map::new(),
            render: Map::new(),
            compute: Map::new(),
        }
    }
}

#[track_caller]
#[track_caller]
fn timed<T>(counter: &AtomicU64, f: impl FnOnce() -> T) -> T {
    timed_at(counter, std::panic::Location::caller(), f)
}

std::thread_local! {
    /// What this thread is making now (a shader module or a pipeline, and
    /// where it is asked for), for the line a native crash writes: a
    /// driver that crashes while it compiles names the program.
    static MAKING: std::cell::Cell<Option<(&'static str, &'static std::panic::Location<'static>)>> =
        const { std::cell::Cell::new(None) };
}

/// What the calling thread is making now ([`MAKING`]), in words: `making a
/// pipeline (src/render/fsr.rs:120)`. None outside a build.
pub fn making_here() -> Option<String> {
    let (what, at) = MAKING.try_with(std::cell::Cell::get).ok().flatten()?;
    Some(format!("making a {what} ({}:{})", at.file(), at.line()))
}

fn timed_at<T>(counter: &AtomicU64, at: &'static std::panic::Location<'static>, f: impl FnOnce() -> T) -> T {
    let what = if std::ptr::eq(counter, &MODULE_NS) { "shader module" } else { "pipeline" };
    let before = MAKING.try_with(|m| m.replace(Some((what, at)))).ok().flatten();
    let t = std::time::Instant::now();
    let v = f();
    let _ = MAKING.try_with(|m| m.set(before));
    let ns = t.elapsed().as_nanos() as u64;
    counter.fetch_add(ns, Ordering::Relaxed);
    static LOG: OnceLock<bool> = OnceLock::new();
    if *LOG.get_or_init(crate::render::debug_env::nfsu2_pipeline_log) {
        let what = if std::ptr::eq(counter, &MODULE_NS) { "module" } else { "pipeline" };
        eprintln!("{what} made in {:.2} ms: {}:{}", ns as f64 / 1e6, at.file(), at.line());
    }
    v
}

/// A render pipeline made as `device.create_render_pipeline` makes it (no
/// cache, not counted: for the world's and the cars' shared sets, which
/// keep and count their own), its time added to [`build_ms`].
#[track_caller]
pub fn timed_render_pipeline(device: &wgpu::Device, desc: &wgpu::RenderPipelineDescriptor<'_>) -> wgpu::RenderPipeline {
    timed(&PIPELINE_NS, || device.create_render_pipeline(desc))
}

/// A shader module; WGSL text is made once per device.
#[track_caller]
pub fn module(gpu: &Gpu, desc: wgpu::ShaderModuleDescriptor<'_>) -> wgpu::ShaderModule {
    let device = &gpu.device;
    let at = std::panic::Location::caller();
    let make = |desc: wgpu::ShaderModuleDescriptor<'_>| {
        MODULES_BUILT.fetch_add(1, Ordering::Relaxed);
        crate::render::core::gpu::note_build(at, gpu.tag);
        timed_at(&MODULE_NS, at, || device.create_shader_module(desc))
    };
    let wgpu::ShaderSource::Wgsl(text) = &desc.source else { return make(desc) };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let key = (h.finish(), Arc::<str>::from(text.as_ref()));
    gpu.device_cache.modules.get_or(key, || make(desc))
}

/// [`module`] from WGSL text.
#[track_caller]
pub fn wgsl(gpu: &Gpu, label: &str, text: impl Into<std::borrow::Cow<'static, str>>) -> wgpu::ShaderModule {
    module(gpu, wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(text.into()) })
}

/// A shader module made as `device.create_shader_module` makes it (no
/// cache: for the world's and the cars' shared sets, which keep their own
/// modules), counted and timed as [`module`]'s.
#[track_caller]
pub fn uncached_module(device: &wgpu::Device, tag: u64, desc: wgpu::ShaderModuleDescriptor<'_>) -> wgpu::ShaderModule {
    let at = std::panic::Location::caller();
    MODULES_BUILT.fetch_add(1, Ordering::Relaxed);
    crate::render::core::gpu::note_build(at, tag);
    timed_at(&MODULE_NS, at, || device.create_shader_module(desc))
}

/// A bind group layout made without the cache (where only the device is
/// at hand: the world's and the cars' shared sets keep their own).
pub fn uncached_bind_group_layout(device: &wgpu::Device, desc: &wgpu::BindGroupLayoutDescriptor<'_>) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(desc)
}

/// A pipeline layout made without the cache (as [`uncached_bind_group_layout`]).
pub fn uncached_pipeline_layout(device: &wgpu::Device, desc: &wgpu::PipelineLayoutDescriptor<'_>) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(desc)
}

/// A render pipeline made without the cache (its owner keeps it once per
/// device), counted and timed as [`render_pipeline`]'s.
#[track_caller]
pub fn uncached_render_pipeline(device: &wgpu::Device, tag: u64, desc: &wgpu::RenderPipelineDescriptor<'_>) -> wgpu::RenderPipeline {
    let at = std::panic::Location::caller();
    crate::render::core::gpu::note_pipeline_at(at, tag);
    timed_at(&PIPELINE_NS, at, || device.create_render_pipeline(desc))
}

/// A compute pipeline made without the cache (as [`uncached_render_pipeline`]).
#[track_caller]
pub fn uncached_compute_pipeline(device: &wgpu::Device, tag: u64, desc: &wgpu::ComputePipelineDescriptor<'_>) -> wgpu::ComputePipeline {
    let at = std::panic::Location::caller();
    crate::render::core::gpu::note_pipeline_at(at, tag);
    timed_at(&PIPELINE_NS, at, || device.create_compute_pipeline(desc))
}

/// A bind group layout, one per set of entries.
pub fn bind_group_layout(gpu: &Gpu, desc: &wgpu::BindGroupLayoutDescriptor<'_>) -> wgpu::BindGroupLayout {
    gpu.device_cache.bind_group_layouts.get_or(desc.entries.to_vec(), || gpu.device.create_bind_group_layout(desc))
}

/// A pipeline layout, one per list of groups and immediates size.
pub fn pipeline_layout(gpu: &Gpu, desc: &wgpu::PipelineLayoutDescriptor<'_>) -> wgpu::PipelineLayout {
    pipeline_layout_in(&gpu.device_cache, &gpu.device, desc)
}

fn pipeline_layout_in(cache: &DeviceCache, device: &wgpu::Device, desc: &wgpu::PipelineLayoutDescriptor<'_>) -> wgpu::PipelineLayout {
    let key = (desc.bind_group_layouts.iter().map(|g| g.cloned()).collect(), desc.immediate_size);
    cache.pipeline_layouts.get_or(key, || device.create_pipeline_layout(desc))
}

/// A render pipeline, one per descriptor (label and pipeline cache aside).
/// Counted in [`crate::render::pipelines_built`] when made.
#[track_caller]
pub fn render_pipeline(gpu: &Gpu, desc: &wgpu::RenderPipelineDescriptor<'_>) -> wgpu::RenderPipeline {
    render_pipeline_in(&gpu.device_cache, &gpu.device, gpu.tag, std::panic::Location::caller(), desc)
}

fn render_pipeline_in(
    cache: &DeviceCache,
    device: &wgpu::Device,
    tag: u64,
    at: &'static std::panic::Location<'static>,
    desc: &wgpu::RenderPipelineDescriptor<'_>,
) -> wgpu::RenderPipeline {
    let make = || {
        crate::render::core::gpu::note_pipeline_at(at, tag);
        timed_at(&PIPELINE_NS, at, || device.create_render_pipeline(desc))
    };
    let key = RenderKey {
        layout: desc.layout.cloned(),
        vertex: StageKey::of(desc.vertex.module, desc.vertex.entry_point, &desc.vertex.compilation_options),
        buffers: desc
            .vertex
            .buffers
            .iter()
            .map(|b| b.as_ref().map(|b| (b.array_stride, b.step_mode, b.attributes.to_vec())))
            .collect(),
        primitive: desc.primitive,
        depth_stencil: desc.depth_stencil.clone(),
        multisample: desc.multisample,
        fragment: desc
            .fragment
            .as_ref()
            .map(|f| (StageKey::of(f.module, f.entry_point, &f.compilation_options), f.targets.to_vec())),
        multiview_mask: desc.multiview_mask,
    };
    cache.render.get_or(key, make)
}

/// What [`render_pipeline`] and [`pipeline_layout`] take of a [`Gpu`], kept
/// by a pass that makes its pipelines at its first draw of a target, where
/// no `Gpu` is at hand: the same device cache, so a warm-up's copy of the
/// pass (another object, made earlier) leaves those draws nothing to build.
#[derive(Clone)]
pub struct Programs {
    device: wgpu::Device,
    tag: u64,
    cache: Arc<DeviceCache>,
}

impl Programs {
    pub fn of(gpu: &Gpu) -> Programs {
        Programs { device: gpu.device.clone(), tag: gpu.tag, cache: gpu.device_cache.clone() }
    }

    /// [`pipeline_layout`].
    pub fn pipeline_layout(&self, desc: &wgpu::PipelineLayoutDescriptor<'_>) -> wgpu::PipelineLayout {
        pipeline_layout_in(&self.cache, &self.device, desc)
    }

    /// [`render_pipeline`].
    #[track_caller]
    pub fn render_pipeline(&self, desc: &wgpu::RenderPipelineDescriptor<'_>) -> wgpu::RenderPipeline {
        render_pipeline_in(&self.cache, &self.device, self.tag, std::panic::Location::caller(), desc)
    }
}

/// A compute pipeline, one per descriptor (label and pipeline cache
/// aside). Counted in [`crate::render::pipelines_built`] when made.
#[track_caller]
pub fn compute_pipeline(gpu: &Gpu, desc: &wgpu::ComputePipelineDescriptor<'_>) -> wgpu::ComputePipeline {
    let device = &gpu.device;
    let at = std::panic::Location::caller();
    let make = || {
        crate::render::core::gpu::note_pipeline_at(at, gpu.tag);
        timed_at(&PIPELINE_NS, at, || device.create_compute_pipeline(desc))
    };
    let key = ComputeKey {
        layout: desc.layout.cloned(),
        stage: StageKey::of(desc.module, desc.entry_point, &desc.compilation_options),
    };
    gpu.device_cache.compute.get_or(key, make)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same descriptors give the same objects (nothing built twice),
    /// another descriptor another; a module by its text, not its label.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_same_descriptor_builds_once() {
        let gpu = pollster::block_on(crate::render::Gpu::new(None, crate::render::Gpu::new_instance())).unwrap();
        let d = &gpu;
        let src = "@vertex fn vs() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0); }\n@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n";
        let m0 = modules_built();
        let a = wgsl(d, "a", src);
        let b = wgsl(d, "b", src);
        assert_eq!(a, b);
        assert_eq!(modules_built() - m0, 1);
        let layout = pipeline_layout(d, &wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], immediate_size: 0 });
        let targets = |format: wgpu::TextureFormat| [Some(wgpu::ColorTargetState::from(format))];
        let (t8, tb) = (targets(wgpu::TextureFormat::Rgba8Unorm), targets(wgpu::TextureFormat::Bgra8Unorm));
        let desc = |targets: &'static [Option<wgpu::ColorTargetState>]| wgpu::RenderPipelineDescriptor {
            label: Some("t"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &a, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &b,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets,
            }),
            multiview_mask: None,
            cache: None,
        };
        let p0 = crate::render::pipelines_built();
        let (t8, tb): (&'static [_], &'static [_]) = (Box::leak(Box::new(t8)), Box::leak(Box::new(tb)));
        let x = render_pipeline(d, &desc(t8));
        let y = render_pipeline(d, &desc(t8));
        let z = render_pipeline(d, &desc(tb));
        assert_eq!(x, y);
        assert_ne!(x, z);
        assert_eq!(crate::render::pipelines_built() - p0, 2);
    }

    #[test]
    fn a_build_names_itself_for_a_crash_line() {
        assert_eq!(super::making_here(), None);
        let inside = super::timed(&super::PIPELINE_NS, super::making_here);
        let inside = inside.expect("named while it builds");
        assert!(inside.starts_with("making a pipeline (") && inside.contains("device_cache.rs:"), "{inside}");
        let module = super::timed(&super::MODULE_NS, super::making_here).unwrap();
        assert!(module.starts_with("making a shader module ("), "{module}");
        assert_eq!(super::making_here(), None);
    }
}
