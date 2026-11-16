use super::*;
use crate::render::ssr::{TRACE_FORMAT, multisampled, skip_test};
/// The SSR trace shader with ray queries: `ssr.wgsl` with its misses and
/// early outs sent to `rt_fill` (`rt.wgsl`). Panics if `ssr.wgsl` lost an
/// anchor (a test checks them).
pub fn trace_source(ssr: &str) -> String {
    let mut s = format!(
        "enable wgpu_ray_query;\n{ssr}\n{}",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/rt.wgsl"))
    );
    for (from, to) in ANCHORS {
        assert!(s.contains(from), "ssr.wgsl anchor missing: {from}");
        s = s.replacen(from, to, 1);
    }
    s
}

/// (ssr.wgsl text, its RT form): the trace's early outs once the ray is
/// known, its miss and its result.
pub(in crate::render) const ANCHORS: [(&str, &str); 5] = [
    (
        "    if reach <= 0.05 {\n        return vec4<f32>(0.0);\n    }",
        "    if reach <= 0.05 {\n        return rt_fill(vec4<f32>(0.0), origin, r, n, car, term, false);\n    }",
    ),
    (
        "    if dist < 1.0 {\n        return vec4<f32>(0.0);\n    }",
        "    if dist < 1.0 {\n        return rt_fill(vec4<f32>(0.0), origin, r, n, car, term, false);\n    }",
    ),
    (
        "    if !hit {\n        return vec4<f32>(0.0);\n    }",
        "    if !hit {\n        return rt_fill(vec4<f32>(0.0), origin, r, n, car, term, false);\n    }",
    ),
    (
        "    return vec4<f32>(colour, edge * travel * facing);\n}",
        "    return rt_fill(vec4<f32>(colour, edge * travel * facing), origin, r, n, car, term, hit_car);\n}",
    ),
    // The rays not marched (no confidence whatever they meet): traced as
    // the march's 0 was.
    (
        "    if p.right.w < 1.5 && r.z <= -0.4 {\n        return vec4<f32>(0.0);\n    }",
        "    if p.right.w < 1.5 && r.z <= -0.4 {\n        return rt_fill(vec4<f32>(0.0), origin, r, n, car, term, false);\n    }",
    ),
];

// ---- The ray list (docs/rt-reflections.md "The ray list") ----
//
// A ray query outside a compute shader keeps its traversal stack in
// scratch memory on some drivers (RADV: 76 entries per lane), which every
// wave of the trace's fragment pass pays whether its pixels trace or not;
// and a wave of a fragment pass waits for its slowest lane, so the few
// pixels that need a ray cost about what all would. Here the trace's
// fragment pass keeps the screen march and writes, where `rt_fill` would
// trace (the same test), the pixel and the march's result into a list; a
// compute pass then gives each listed pixel a lane of its own and runs
// `rt_fill` on it with the same inputs (the trace's own head recomputed
// from the pixel, the march's result as it was) and writes its result over
// the pixel's: the values the fragment form writes.

/// fs_trace's head in ssr.wgsl, the ray's inputs from the pixel (its
/// mask texel, surface, origin, normal, reflected ray): what the list's
/// compute pass recomputes, as the same text.
const HEAD_START: &str = "fn fs_trace(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {\n";
const HEAD_END: &str = "    // The ray's reach: the distance, and not behind the near plane.\n";

/// (ssr.wgsl text, its list form): where the RT form calls `rt_fill`, the
/// result goes to `rt_keep` (`zero`: the early outs and the miss, whose
/// result is 0).
pub(in crate::render) const LIST_ANCHORS: [(&str, &str); 5] = [
    (ANCHORS[0].0, "    if reach <= 0.05 {\n        return rt_keep(vec4<f32>(0.0), frag.xy, true);\n    }"),
    (ANCHORS[1].0, "    if dist < 1.0 {\n        return rt_keep(vec4<f32>(0.0), frag.xy, true);\n    }"),
    (ANCHORS[2].0, "    if !hit {\n        return rt_keep(vec4<f32>(0.0), frag.xy, true);\n    }"),
    (ANCHORS[3].0, "    return rt_keep(vec4<f32>(colour, edge * travel * facing), frag.xy, false);\n}"),
    (
        ANCHORS[4].0,
        "    if p.right.w < 1.5 && r.z <= -0.4 {\n        return rt_keep(vec4<f32>(0.0), frag.xy, true);\n    }",
    ),
];

/// The list's head: the count, then the compute pass's indirect
/// dispatch (x, 1, 1) at [`ARGS_OFFSET`].
const LIST_HEAD: &str = r#"
// ---- The ray list (rt/trace.rs, docs/rt-reflections.md) ----
// Per listed pixel: x | y << 15, bit 31 for a result of 0 (the early outs,
// the miss); else the march's result in rt_c.
struct RtHead {
    count: atomic<u32>,
    groups: atomic<u32>,
    y: u32,
    z: u32,
}
@group(1) @binding(0) var<storage, read_write> rt_head: RtHead;
@group(1) @binding(1) var<storage, read_write> rt_px: array<u32>;
@group(1) @binding(2) var<storage, read_write> rt_c: array<vec4<f32>>;

// The trace's result c at pixel `frag`, listed where rt_fill would trace
// it (its own test: not c.a >= 0.98).
fn rt_keep(c: vec4<f32>, frag: vec2<f32>, zero: bool) -> vec4<f32> {
    if !(c.a >= 0.98) {
        let i = atomicAdd(&rt_head.count, 1u);
        let q = vec2<u32>(frag);
        rt_px[i] = q.x | (q.y << 15u) | select(0u, 0x80000000u, zero);
        if !zero {
            rt_c[i] = c;
        }
        // The compute pass's groups of 64: one more from each 64th on.
        if (i & 63u) == 0u {
            atomicMax(&rt_head.groups, min(i / 64u + 1u, MAX_GROUPS));
        }
    }
    return c;
}
"#;

/// The compute pass: each lane a listed pixel (as many rounds as the
/// list holds more than the groups' lanes).
const LIST_RAYS: &str = r#"
// ---- The ray list's rays (rt/trace.rs, docs/rt-reflections.md) ----
struct RtHead {
    count: u32,
    groups: u32,
    y: u32,
    z: u32,
}
@group(2) @binding(0) var<storage, read> rt_head: RtHead;
@group(2) @binding(1) var<storage, read> rt_px: array<u32>;
@group(2) @binding(2) var<storage, read> rt_c: array<vec4<f32>>;
@group(2) @binding(3) var rt_out: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(64)
fn cs_rt_rays(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    for (var i = id.x; i < rt_head.count; i += groups.x * 64u) {
        let e = rt_px[i];
        let q = vec2<u32>(e & 0x7FFFu, (e >> 15u) & 0x7FFFu);
        var c = vec4<f32>(0.0);
        if (e & 0x80000000u) == 0u {
            c = rt_c[i];
        }
        textureStore(rt_out, q, rt_ray(vec4<f32>(vec2<f32>(q) + 0.5, 0.0, 1.0), c));
    }
}
"#;

/// The compute pass's groups at most (the dispatch's x limit).
pub const MAX_GROUPS: u32 = 65535;
/// The dispatch's (x, y, z) in the list's head.
const ARGS_OFFSET: u64 = 4;
/// A listed pixel's x and y fit 15 bits each.
pub const MAX_SIDE: u32 = 1 << 15;

/// The trace's fragment pass in its list form: ssr.wgsl with the RT form's
/// `rt_fill` calls sent to `rt_keep` (no ray query in it).
pub fn list_trace_source(ssr: &str) -> String {
    let mut s = format!("{ssr}\n{}", LIST_HEAD.replace("MAX_GROUPS", &format!("{MAX_GROUPS}u")));
    for (from, to) in LIST_ANCHORS {
        assert!(s.contains(from), "ssr.wgsl anchor missing: {from}");
        s = s.replacen(from, to, 1);
    }
    s
}

/// (rt.wgsl text, the list's form): `rt_fill`'s two casts on one ray
/// query (the same rays, in the same order). A compute shader keeps the
/// query's traversal stack in workgroup memory on some drivers (RADV),
/// split between the shader's queries: one query keeps all of it.
pub(in crate::render) const ONE_QUERY: [(&str, &str); 2] = [
    (
        "    var h = rt_cast(start, d, select(0.25, 0.12, car), rt.eye.w, 0xFFu);\n",
        "    var rq: ray_query;\n    rayQueryInitialize(&rq, rt_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE, 0xFFu, select(0.25, 0.12, car), rt.eye.w, start, d));\n    while rayQueryProceed(&rq) {}\n    var h = rayQueryGetCommittedIntersection(&rq);\n",
    ),
    (
        "        h = rt_cast(start + d * h.t, d, 0.01, rt.eye.w - h.t, 0x01u);\n",
        "        rayQueryInitialize(&rq, rt_tlas, RayDesc(RAY_FLAG_FORCE_OPAQUE, 0x01u, 0.01, rt.eye.w - h.t, start + d * h.t, d));\n        while rayQueryProceed(&rq) {}\n        h = rayQueryGetCommittedIntersection(&rq);\n",
    ),
];

/// The list's compute pass: ssr.wgsl and rt.wgsl (its casts on one query,
/// [`ONE_QUERY`]), `rt_ray` (fs_trace's head as written, then `rt_fill`
/// on the march's result) and the pass.
pub fn list_rays_source(ssr: &str) -> String {
    let start = ssr.find(HEAD_START).expect("ssr.wgsl: fs_trace") + HEAD_START.len();
    let end = start + ssr[start..].find(HEAD_END).expect("ssr.wgsl: fs_trace's reach");
    let head = &ssr[start..end];
    let mut rt = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/rt.wgsl")).to_string();
    for (from, to) in ONE_QUERY {
        assert!(rt.matches(from).count() == 1, "rt.wgsl anchor missing: {from}");
        rt = rt.replacen(from, to, 1);
    }
    format!(
        "enable wgpu_ray_query;\n{ssr}\n{rt}\n// fs_trace's head (ssr.wgsl), then rt_fill on the march's result c.\nfn rt_ray(frag: vec4<f32>, c: vec4<f32>) -> vec4<f32> {{\n{head}    return rt_fill(c, origin, r, n, car, term, false);\n}}\n{LIST_RAYS}"
    )
}

/// Whether the trace's rays go through the list on this device: ray
/// queries, writable storage in fragment shaders, the trace's format as a
/// write-only storage texture (`NFSU2_RT_TRACE=fragment`: never, a check
/// aid). Decided when the trace's targets are made ([`list_fits`] for
/// their size), never per frame; elsewhere the fragment form stays.
pub fn list_path(gpu: &Gpu) -> bool {
    let caps = gpu.adapter.get_downlevel_capabilities();
    let format = gpu.adapter.get_texture_format_features(TRACE_FORMAT);
    available(gpu)
        && !crate::render::debug_env::nfsu2_rt_trace_fragment()
        && caps.flags.contains(wgpu::DownlevelFlags::FRAGMENT_WRITABLE_STORAGE)
        && format.allowed_usages.contains(wgpu::TextureUsages::STORAGE_BINDING)
        && format.flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY)
        && gpu.device.limits().max_storage_buffers_per_shader_stage >= 3
        && gpu.device.limits().max_storage_textures_per_shader_stage >= 1
}

/// A trace of `size` can be listed: every pixel's entry fits the device's
/// buffer limits, and its x and y the entry's bits.
pub fn list_fits(gpu: &Gpu, size: (u32, u32)) -> bool {
    let bytes = u64::from(size.0) * u64::from(size.1) * 16;
    let l = gpu.device.limits();
    size.0 < MAX_SIDE && size.1 < MAX_SIDE && bytes <= u64::from(l.max_storage_buffer_binding_size) && bytes <= l.max_buffer_size
}

/// The list for a trace of `size`: room for every pixel (each may need a
/// ray), 20 bytes each.
pub struct ListBuffers {
    head: wgpu::Buffer,
    px: wgpu::Buffer,
    c: wgpu::Buffer,
}

impl ListBuffers {
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> ListBuffers {
        let n = u64::from(size.0.max(1)) * u64::from(size.1.max(1));
        // count 0, groups 0, y 1, z 1 (the frame clears the first two).
        let head = wgpu::util::DeviceExt::create_buffer_init(&gpu.device, &wgpu::util::BufferInitDescriptor {
            label: Some("rt list head"),
            contents: bytemuck::cast_slice(&[0u32, 0, 1, 1]),
            // (Copied out by the tests and `NFSU2_RT_LIST_STATS`.)
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        // (The tests read the list back.)
        let copy = if cfg!(test) { wgpu::BufferUsages::COPY_SRC } else { wgpu::BufferUsages::empty() };
        let buffer = |label, size| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | copy,
                mapped_at_creation: false,
            })
        };
        ListBuffers { head, px: buffer("rt list pixels", n * 4), c: buffer("rt list results", n * 16) }
    }

    /// The bytes held (the frame's memory report).
    pub fn bytes(&self) -> u64 {
        self.head.size() + self.px.size() + self.c.size()
    }

    /// The list as the last trace left it: its head (count, groups, y,
    /// z), the listed pixels' words and the march's results (tests).
    #[cfg(test)]
    pub(in crate::render) fn read(&self, gpu: &Gpu) -> ([u32; 4], Vec<u32>, Vec<[f32; 4]>) {
        let read = |b: &wgpu::Buffer| -> Vec<u8> {
            let r = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: b.size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            enc.copy_buffer_to_buffer(b, 0, &r, 0, b.size());
            gpu.queue.submit([enc.finish()]);
            r.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let v = r.slice(..).get_mapped_range().expect("mapped").to_vec();
            v
        };
        let head: [u32; 4] = bytemuck::pod_read_unaligned(&read(&self.head));
        let n = head[0] as usize;
        let px: Vec<u32> = bytemuck::cast_slice(&read(&self.px))[..n].to_vec();
        let c: Vec<[f32; 4]> = bytemuck::cast_slice(&read(&self.c))[..n].to_vec();
        (head, px, c)
    }
}

fn storage(binding: u32, visibility: wgpu::ShaderStages, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The list path's programs: the trace's fragment pass in its list form
/// (single, multisampled depth) and the rays' compute pass, made through
/// the device cache (a warm-up's copy makes the frame's).
pub(in crate::render) struct ListTrace {
    /// The fragment pass's group 1 (the list, written).
    list_layout: wgpu::BindGroupLayout,
    fragment: [Option<wgpu::RenderPipeline>; 2],
    /// The compute pass's groups: the mask and the trace's uniform, the
    /// traced scene, the list and the trace target.
    rays_layouts: [wgpu::BindGroupLayout; 3],
    rays: wgpu::ComputePipeline,
}

impl ListTrace {
    pub(in crate::render) fn new(gpu: &Gpu) -> ListTrace {
        use wgpu::ShaderStages as S;
        let cache = crate::render::core::device_cache::bind_group_layout;
        let list_layout = cache(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("rt list"),
            entries: &[storage(0, S::FRAGMENT, false), storage(1, S::FRAGMENT, false), storage(2, S::FRAGMENT, false)],
        });
        let tex = |binding, filterable| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: S::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        // ssr.wgsl's group 0 as the head reads it: the mask's term and
        // surface, the trace's uniform (`p`).
        let mask_layout = cache(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("rt list mask"),
            entries: &[
                tex(3, true),
                tex(4, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: S::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let out_layout = cache(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("rt list rays"),
            entries: &[
                storage(0, S::COMPUTE, true),
                storage(1, S::COMPUTE, true),
                storage(2, S::COMPUTE, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: S::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: TRACE_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let rays_layouts = [mask_layout, crate::render::rt::layout(gpu, S::COMPUTE), out_layout];
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("ssr rt rays"),
            source: wgpu::ShaderSource::Wgsl(list_rays_source(src).into()),
        });
        let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("ssr rt rays"),
            bind_group_layouts: &[Some(&rays_layouts[0]), Some(&rays_layouts[1]), Some(&rays_layouts[2])],
            immediate_size: 0,
        });
        let rays = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
            label: Some("cs_rt_rays"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("cs_rt_rays"),
            compilation_options: Default::default(),
            cache: gpu.disk_pipeline_cache(),
        });
        ListTrace { list_layout, fragment: [None, None], rays_layouts, rays }
    }

    /// The fragment pass for a depth multisampled (`ms` 1) or not.
    pub(in crate::render) fn fragment(&mut self, gpu: &Gpu, layouts: &[wgpu::BindGroupLayout; 2], ms: usize) -> &wgpu::RenderPipeline {
        let list_layout = &self.list_layout;
        self.fragment[ms].get_or_insert_with(|| {
            let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
            let src = if ms == 1 { multisampled(src) } else { src.to_string() };
            let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("ssr rt list"),
                source: wgpu::ShaderSource::Wgsl(list_trace_source(&src).into()),
            });
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("ssr rt list"),
                bind_group_layouts: &[Some(&layouts[ms]), Some(list_layout)],
                immediate_size: 0,
            });
            trace_pipeline(gpu, &pl, &shader, "fs_trace rt list")
        })
    }
}

/// The trace's fragment pipeline (vs_trace / fs_trace into the trace
/// target, against the trace-sized mask depth).
fn trace_pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule, label: &str) -> wgpu::RenderPipeline {
    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_trace"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_trace"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: TRACE_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(skip_test()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: gpu.disk_pipeline_cache(),
    })
}

/// The fragment form's pipeline (`ms` 1: a multisampled depth).
fn rt_pipeline(gpu: &Gpu, layouts: &[wgpu::BindGroupLayout; 2], layout: &wgpu::BindGroupLayout, ms: usize) -> wgpu::RenderPipeline {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ssr.wgsl"));
    let src = if ms == 1 { multisampled(src) } else { src.to_string() };
    let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
        label: Some("ssr rt"),
        source: wgpu::ShaderSource::Wgsl(crate::render::rt::trace_source(&src).into()),
    });
    let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
        label: Some("ssr rt"),
        bind_group_layouts: &[Some(&layouts[ms]), Some(layout)],
        immediate_size: 0,
    });
    trace_pipeline(gpu, &pl, &shader, "fs_trace rt")
}

/// The ray-traced trace's programs for a depth multisampled when `ms`,
/// made now (the drive's warm-up, docs/first-use-builds.md): the list
/// path's where the device takes it ([`list_path`]), else the fragment
/// form's.
pub(in crate::render) fn warm_trace(
    gpu: &Gpu,
    layouts: &[wgpu::BindGroupLayout; 2],
    fragment: &mut Option<(wgpu::BindGroupLayout, [Option<wgpu::RenderPipeline>; 2])>,
    list: &mut Option<ListTrace>,
    ms: bool,
) {
    let ms = usize::from(ms);
    if list_path(gpu) {
        list.get_or_insert_with(|| ListTrace::new(gpu)).fragment(gpu, layouts, ms);
    } else {
        let (layout, pipelines) =
            fragment.get_or_insert_with(|| (crate::render::rt::layout(gpu, wgpu::ShaderStages::FRAGMENT), [None, None]));
        if pipelines[ms].is_none() {
            pipelines[ms] = Some(rt_pipeline(gpu, layouts, layout, ms));
        }
    }
}

/// The ray-query variant of the SSR trace pass (made by `Ssr::run` when the
/// frame carries a traced scene and the device keeps the rays in the
/// fragment pass): moved out of `ssr.rs` whole.
pub(in crate::render) fn ssr_trace(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    layouts: &[wgpu::BindGroupLayout; 2],
    slot: &mut Option<(wgpu::BindGroupLayout, [Option<wgpu::RenderPipeline>; 2])>,
    ms: usize,
    rt: &FrameRt,
    t: &crate::render::ssr::Targets,
    trace_group: &wgpu::BindGroup,
) {
    let mut rt_state = slot.take();
    let (layout, rt_pipelines) =
        rt_state.get_or_insert_with(|| (crate::render::rt::layout(gpu, wgpu::ShaderStages::FRAGMENT), [None, None]));
    let pipeline = rt_pipelines[ms].get_or_insert_with(|| rt_pipeline(gpu, layouts, layout, ms));
    let rt_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ssr rt"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: rt.tlas.as_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: rt.colors.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: rt.uniform.as_entire_binding() },
        ],
    });
    let mut rp = begin_trace(encoder, t, "ssr rt trace");
    rp.set_pipeline(pipeline);
    rp.set_bind_group(0, trace_group, &[]);
    rp.set_bind_group(1, &rt_group, &[]);
    rp.draw(0..3, 0..1);
    drop(rp);
    crate::prof::gpu::mark(encoder, "graphics: rt trace");
    *slot = rt_state;
}

/// The trace's render pass: the target cleared, the mask depth tested.
fn begin_trace<'a>(encoder: &'a mut wgpu::CommandEncoder, t: &crate::render::ssr::Targets, label: &str) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &t.trace,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &t.skip,
            depth_ops: None,
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// What the list's compute pass reads besides the list: the mask the
/// trace's head reads and the trace's uniform.
pub(in crate::render) struct ListInputs<'a> {
    pub term: &'a wgpu::TextureView,
    pub surface: &'a wgpu::TextureView,
    pub uniform: &'a wgpu::Buffer,
}

/// The SSR trace pass with the rays from a list ([`list_path`]): the
/// fragment pass marches and lists the pixels that need a ray, the compute
/// pass traces them and writes their results into the trace target.
#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn ssr_trace_list(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    layouts: &[wgpu::BindGroupLayout; 2],
    slot: &mut Option<ListTrace>,
    ms: usize,
    rt: &FrameRt,
    t: &crate::render::ssr::Targets,
    list: &ListBuffers,
    trace_group: &wgpu::BindGroup,
    inputs: ListInputs<'_>,
) {
    let lt = slot.get_or_insert_with(|| ListTrace::new(gpu));
    let fragment = lt.fragment(gpu, layouts, ms).clone();
    let entry = |binding, resource| wgpu::BindGroupEntry { binding, resource };
    let group = |layout, entries: &[wgpu::BindGroupEntry<'_>]| {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("ssr rt list"), layout, entries })
    };
    let list_group = group(&lt.list_layout, &[
        entry(0, list.head.as_entire_binding()),
        entry(1, list.px.as_entire_binding()),
        entry(2, list.c.as_entire_binding()),
    ]);
    // The count and the groups from 0 (y and z stay 1).
    encoder.clear_buffer(&list.head, 0, Some(8));
    let mut rp = begin_trace(encoder, t, "ssr rt list trace");
    rp.set_pipeline(&fragment);
    rp.set_bind_group(0, trace_group, &[]);
    rp.set_bind_group(1, &list_group, &[]);
    rp.draw(0..3, 0..1);
    drop(rp);
    crate::prof::gpu::mark(encoder, "graphics: rt trace");
    use wgpu::BindingResource as R;
    let mask_group = group(&lt.rays_layouts[0], &[
        entry(3, R::TextureView(inputs.term)),
        entry(4, R::TextureView(inputs.surface)),
        entry(7, inputs.uniform.as_entire_binding()),
    ]);
    let scene_group = group(&lt.rays_layouts[1], &[
        entry(0, rt.tlas.as_binding()),
        entry(1, rt.colors.as_entire_binding()),
        entry(2, rt.uniform.as_entire_binding()),
    ]);
    let out_group = group(&lt.rays_layouts[2], &[
        entry(0, list.head.as_entire_binding()),
        entry(1, list.px.as_entire_binding()),
        entry(2, list.c.as_entire_binding()),
        entry(3, R::TextureView(&t.trace)),
    ]);
    {
        let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("ssr rt rays"), timestamp_writes: None });
        cp.set_pipeline(&lt.rays);
        cp.set_bind_group(0, &mask_group, &[]);
        cp.set_bind_group(1, &scene_group, &[]);
        cp.set_bind_group(2, &out_group, &[]);
        cp.dispatch_workgroups_indirect(&list.head, ARGS_OFFSET);
    }
    crate::prof::gpu::mark(encoder, "graphics: rt trace rays");
    if let Some(every) = crate::render::debug_env::nfsu2_rt_list_stats() {
        list_stats(gpu, encoder, list, t.trace_size(), every);
    }
}

/// `NFSU2_RT_LIST_STATS` (a measuring aid): every `every`-th trace, the
/// list's count copied out and printed once its submission is done (no
/// wait in the frame).
fn list_stats(gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, list: &ListBuffers, size: (u32, u32), every: u64) {
    static TRACES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = TRACES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n % every != 0 {
        return;
    }
    let out = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rt list stats"),
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&list.head, 0, &out, 0, 16);
    let read = out.clone();
    encoder.map_buffer_on_submit(&out, wgpu::MapMode::Read, .., move |r| {
        if r.is_ok()
            && let Ok(v) = read.slice(..).get_mapped_range()
        {
            let count = u32::from_le_bytes([v[0], v[1], v[2], v[3]]);
            let pixels = u64::from(size.0) * u64::from(size.1);
            eprintln!(
                "rt list: trace {n}: {count} of {pixels} trace pixels need a ray ({:.2} %)",
                100.0 * f64::from(count) / pixels.max(1) as f64
            );
        }
    });
}
