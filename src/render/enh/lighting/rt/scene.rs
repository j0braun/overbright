use super::*;
/// The device feature the trace needs.
pub const FEATURE: wgpu::Features = wgpu::Features::EXPERIMENTAL_RAY_QUERY;

/// The adapter can trace: ray queries offered on Vulkan, on a discrete
/// GPU (`NFSU2_RT=1`: any; `NFSU2_RT=0`: never). The tier is offered then.
pub fn adapter_can(adapter: &wgpu::Adapter) -> bool {
    let info = adapter.get_info();
    let allowed = match crate::render::debug_env::nfsu2_rt().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => info.device_type == wgpu::DeviceType::DiscreteGpu,
    };
    allowed && info.backend == wgpu::Backend::Vulkan && adapter.features().contains(FEATURE)
}

/// The features to ask the device for: ray query only when the tier is
/// chosen as the device is made (options.txt's `enh.ssr 3`, or the
/// bench's / shot's `NFSU2_BENCH_ENH` / `NFSU2_SHOT_ENH` asking for it,
/// or `NFSU2_RT=1`) on an adapter that [`adapter_can`]. Otherwise the
/// device is made exactly as before (no experimental API on the default
/// path), so the tier takes effect after a restart.
pub fn wanted(adapter: &wgpu::Adapter) -> wgpu::Features {
    // Modern Lighting's "Modern + RT" (enh.modern_lighting 3) asks too.
    let env_asks =
        [crate::render::debug_env::nfsu2_bench_enh as fn() -> Option<String>, crate::render::debug_env::nfsu2_shot_enh]
            .iter()
            .any(|k| {
                k().is_some_and(|v| {
                    v.split(';').any(|l| {
                        matches!(
                            l.split_whitespace().collect::<Vec<_>>()[..],
                            ["enh.ssr", "3"] | ["enh.modern_lighting", "3"]
                        )
                    })
                })
            });
    let asked = crate::qol::ssr_requested() == 3
        || crate::qol::modern_lighting_requested() == 3
        || env_asks
        || crate::render::debug_env::nfsu2_rt().is_some_and(|v| v == "1");
    if asked && adapter_can(adapter) { FEATURE } else { wgpu::Features::empty() }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::render) struct Uniform {
    /// The eye in the world; w the reach.
    eye: [f32; 4],
    misc: [f32; 4],
}

pub(in crate::render) struct CarEntry {
    blas: wgpu::Blas,
    /// Its first triangle in the colour buffer.
    base: u32,
    last_used: u64,
}

/// A region's traced scene ([`frame`]).
pub(in crate::render) struct Scene {
    key: (String, usize, usize),
    world_blas: Option<wgpu::Blas>,
    world_triangles: u32,
    colors: wgpu::Buffer,
    /// The next free car slot in `colors` (triangles).
    car_next: u32,
    cars: HashMap<u64, CarEntry>,
    means: HashMap<u32, [f32; 4]>,
    tlas: [wgpu::Tlas; 3],
    uniforms: [wgpu::Buffer; 3],
    frame: u64,
    /// The cache's clock: +1 per world tick (`render::enh::tick_views`;
    /// per draw without published ticks), which the models' last use and
    /// the build budget count in.
    steps: u64,
    gate: crate::render::enh::tick_views::TickGate,
}

/// The group-1 layout the RT trace pipelines are made with (`rt.wgsl`'s
/// bindings, seen by the trace's fragment pass or by the ray list's
/// compute pass, `visibility`); one per device and stage, so a warm-up's
/// pipelines are the frame's (docs/first-use-builds.md).
pub fn layout(gpu: &Gpu, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayout {
    crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
        label: Some("rt"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                },
                count: None,
            },
        ],
    })
}

/// What a frame's trace binds at group 1 ([`super::ssr::Frame::rt`]).
#[derive(Clone)]
pub struct FrameRt {
    pub tlas: wgpu::Tlas,
    pub colors: wgpu::Buffer,
    pub uniform: wgpu::Buffer,
}

pub(in crate::render) static SCENE: Mutex<Option<Scene>> = Mutex::new(None);

/// The device can trace (asked for and given, [`wanted`]).
pub fn available(gpu: &Gpu) -> bool {
    gpu.device.features().contains(FEATURE)
}

/// Drop the traced scene (a region left, the row turned off).
pub fn release() {
    *SCENE.lock().unwrap() = None;
}

pub(in crate::render) fn blas_for(
    device: &wgpu::Device,
    label: &str,
    triangles: u32,
) -> (wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor) {
    let size = wgpu::BlasTriangleGeometrySizeDescriptor {
        vertex_format: wgpu::VertexFormat::Float32x3,
        vertex_count: triangles * 3,
        index_format: None,
        index_count: None,
        flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
    };
    let blas = device.create_blas(
        &wgpu::CreateBlasDescriptor {
            label: Some(label),
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        },
        wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
    );
    (blas, size)
}

/// The traced scene's world half for `world` (`region`), made where the
/// scene in `guard` is not that world's: the world's geometry gathered,
/// its colours, the BLAS created and its build submitted, the TLAS and
/// uniforms made (empty of cars).
fn ensure_world(gpu: &Gpu, world: &World, region: &str, guard: &mut Option<Scene>) {
    let device = &gpu.device;
    let key = (region.to_string(), world.solids.len(), world.placements.len());
    if guard.as_ref().is_some_and(|s| s.key == key) {
        return;
    }
    *guard = None;
    let span = crate::prof::span("rt: world geometry");
    let mut means = HashMap::new();
    let g = world_geometry(world, &mut means);
    drop(span);
    let world_triangles = g.triangles() as u32;
    log::info!("rt: world {} triangles ({} textures)", world_triangles, means.len());
    // Initialised at creation (the rule: `zeroed_buffer`): the
    // cars' colours are written later, frame by frame, into its tail.
    let colors = crate::render::zeroed_buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some("rt colours"),
            size: ((world_triangles + CAR_COLOR_ROOM) as u64 * 4).max(16),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        },
    );
    gpu.queue.write_buffer(&colors, 0, bytemuck::cast_slice(&g.colors));
    let tlas = std::array::from_fn(|i| {
        device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some(["rt tlas 0", "rt tlas 1", "rt tlas 2"][i]),
            max_instances: 1 + MAX_CARS as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        })
    });
    let uniforms = std::array::from_fn(|_| {
        crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("rt"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        )
    });
    let world_blas = (world_triangles > 0).then(|| {
        let (blas, size) = blas_for(device, "rt world", world_triangles);
        let vb = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("rt world positions"),
                contents: bytemuck::cast_slice(&g.positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            },
        );
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt world blas") });
        enc.build_acceleration_structures(
            [&wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                    size: &size,
                    vertex_buffer: &vb,
                    first_vertex: 0,
                    vertex_stride: 12,
                    index_buffer: None,
                    first_index: None,
                    transform_buffer: None,
                    transform_buffer_offset: None,
                }]),
            }],
            std::iter::empty(),
        );
        gpu.queue.submit([enc.finish()]);
        blas
    });
    *guard = Some(Scene {
        key,
        world_blas,
        world_triangles,
        colors,
        car_next: 0,
        cars: HashMap::new(),
        means,
        tlas,
        uniforms,
        frame: 0,
        steps: 0,
        gate: Default::default(),
    });
}

/// The traced scene's world half made now, at a drive's load or beside a
/// world loaded ahead, not in the drive's first frame ([`frame`] makes
/// it on first use: about 0.1 s of gathering on the calling thread). The
/// same objects from the same world; the first [`frame`] finds them made.
/// Nothing when the device cannot trace.
pub fn prepare(gpu: &Gpu, world: &World, region: &str) {
    if available(gpu) {
        ensure_world(gpu, world, region, &mut SCENE.lock().unwrap());
    }
}

/// This frame's traced scene: made for `world` (`region`) on first use
/// (the world's BLAS, once; [`prepare`] makes it ahead), each car model's
/// BLAS on first sight, the TLAS rebuilt with `cars` (model, car → world)
/// where they are now. The builds are submitted here, ahead of the
/// frame's own work. None when the device cannot trace.
pub fn frame(gpu: &Gpu, world: &World, region: &str, cars: &[(&Car, Mat4)], eye: Vec3) -> Option<FrameRt> {
    if !available(gpu) {
        return None;
    }
    let device = &gpu.device;
    let mut guard = SCENE.lock().unwrap();
    ensure_world(gpu, world, region, &mut guard);
    let s = guard.as_mut().unwrap();
    s.frame += 1;
    // The cache steps once per world tick (a frame between two
    // ticks builds and evicts nothing), so which models are built and kept
    // at tick n is the same at every display rate.
    let ticks = match s.gate.due(crate::render::enh::tick_views::frame().as_deref()) {
        crate::render::enh::tick_views::Due::Draw => 1,
        crate::render::enh::tick_views::Due::Ticks(v) => v.len() as u64,
    };
    s.steps += ticks;
    let now = s.steps;
    // New car models: their BLAS (and colours), at most a few a tick.
    let mut builds: Vec<(wgpu::Blas, wgpu::BlasTriangleGeometrySizeDescriptor, wgpu::Buffer)> = Vec::new();
    let mut placed: Vec<(u64, Mat4)> = Vec::with_capacity(cars.len().min(MAX_CARS));
    for &(car, m) in cars.iter().take(MAX_CARS) {
        let k = car_key(car);
        if let Some(e) = s.cars.get_mut(&k) {
            e.last_used = now;
            placed.push((k, m));
            continue;
        }
        if builds.len() as u64 >= 4 * ticks {
            continue;
        }
        let g = car_geometry(car, &mut s.means);
        let n = g.triangles() as u32;
        if n == 0 {
            continue;
        }
        if s.car_next + n > CAR_COLOR_ROOM {
            // Full: start the car cache over (models are rebuilt as seen).
            s.cars.clear();
            s.car_next = 0;
            placed.clear();
            if n > CAR_COLOR_ROOM {
                continue;
            }
        }
        let base = s.world_triangles + s.car_next;
        gpu.queue.write_buffer(&s.colors, base as u64 * 4, bytemuck::cast_slice(&g.colors));
        let (blas, size) = blas_for(device, "rt car", n);
        let vb = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("rt car positions"),
                contents: bytemuck::cast_slice(&g.positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            },
        );
        builds.push((blas.clone(), size, vb));
        s.cars.insert(k, CarEntry { blas, base, last_used: now });
        s.car_next += n;
        placed.push((k, m));
    }
    // Models unseen for ten seconds' worth of ticks go.
    if ticks > 0 && s.cars.len() > MAX_CARS {
        s.cars.retain(|_, e| now - e.last_used < 600);
    }
    let ring = (s.frame % 3) as usize;
    let mut instances = 0usize;
    {
        let tlas = &mut s.tlas[ring];
        if let Some(b) = &s.world_blas {
            tlas[instances] = Some(wgpu::TlasInstance::new(b, instance_transform(Mat4::IDENTITY), 0, MASK_WORLD));
            instances += 1;
        }
        for (k, m) in &placed {
            let Some(e) = s.cars.get(k) else { continue };
            tlas[instances] = Some(wgpu::TlasInstance::new(&e.blas, instance_transform(*m), e.base, MASK_CAR));
            instances += 1;
        }
        for i in instances..=MAX_CARS {
            tlas[i] = None;
        }
    }
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rt tlas") });
    let entries: Vec<wgpu::BlasBuildEntry> = builds
        .iter()
        .map(|(blas, size, vb)| wgpu::BlasBuildEntry {
            blas,
            geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                size,
                vertex_buffer: vb,
                first_vertex: 0,
                vertex_stride: 12,
                index_buffer: None,
                first_index: None,
                transform_buffer: None,
                transform_buffer_offset: None,
            }]),
        })
        .collect();
    enc.build_acceleration_structures(entries.iter(), [&s.tlas[ring]]);
    crate::prof::gpu::mark(&mut enc, "graphics: rt tlas");
    gpu.queue.write_buffer(
        &s.uniforms[ring],
        0,
        bytemuck::bytes_of(&Uniform { eye: [eye.x, eye.y, eye.z, REACH], misc: [instances as f32, 0.0, 0.0, 0.0] }),
    );
    gpu.queue.submit([enc.finish()]);
    Some(FrameRt { tlas: s.tlas[ring].clone(), colors: s.colors.clone(), uniform: s.uniforms[ring].clone() })
}

/// The car cache's tick-stepped state (its clock and the models
/// kept with their last use) as a digest.
pub(in crate::render) fn digest(h: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    if let Some(s) = SCENE.lock().unwrap().as_ref() {
        s.steps.hash(h);
        let mut cars: Vec<(u64, u64)> = s.cars.iter().map(|(k, e)| (*k, e.last_used)).collect();
        cars.sort_unstable();
        cars.hash(h);
    }
}
