//! The drops on the lens (0x5C7AC0, rain_drop.efx = effect 19, 0x86EB74).
//!
//! Traced: per drop of the weather object (0x6142D0: x, y, timer / life,
//! size) the effect gets `raindropalpha = 3·timer / life` (0x787ADC) and
//! `raindropoffset = (x − w/4, y + w/4, w, −w)` with `w = 0.12·size`
//! (0x785368, 0x784D28), and a quad is drawn with DrawPrimitiveUP
//! (strip, FVF 0x14C) straight in clip space (the vs_1_1 copies v0 to
//! oPos): corners (X, Y), (X + w, Y), (X, Y + w), (X + w, Y + w), UVs (0,0),
//! (1,0), (0,1), (1,1), `X = 2x − 1`, `Y = 1 − 2y`. The ps_2_0:
//! `d = RAINDROPPC(uv)`, `screen((d.g, d.b)·(w, −w) + (x − w/4, y + w/4))
//! · 1.2`, alpha `d.a · raindropalpha`. The pass (blob states): alpha
//! blend SRCALPHA / INVSRCALPHA, no z, cull none, colour writes 7. The
//! screen is the copy of the frame so far (0x86EB88, the copy 0x5CC8C0
// prov: inferred render.rain.module
//! made). **Inferred**: the samplers' filtering and addressing (linear,
//! clamped here).

use crate::formats::texture::Texture;
use crate::weather::{LensDrop, SPLATS};

use crate::render::Gpu;

const WGSL: &str = r#"
struct Drops { d: array<vec4<f32>, 20> };
@group(0) @binding(0) var<uniform> drops: Drops;
@group(0) @binding(1) var screen: texture_2d<f32>;
@group(0) @binding(2) var displace: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

struct V {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) i: u32,
};

@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> V {
    let d = drops.d[i];
    let w = 0.12 * d.z;
    let c = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    var o: V;
    o.pos = vec4<f32>(2.0 * d.x - 1.0 + c.x * w, 1.0 - 2.0 * d.y + c.y * w, 0.0, 1.0);
    o.uv = c;
    o.i = i;
    return o;
}

@fragment fn fs(in: V) -> @location(0) vec4<f32> {
    let d = drops.d[in.i];
    let w = 0.12 * d.z;
    let t = textureSample(displace, samp, in.uv);
    let uv = vec2<f32>(t.g * w + (d.x - 0.25 * w), t.b * -w + (d.y + 0.25 * w));
    let s = textureSample(screen, samp, uv);
    return clamp(vec4<f32>(s.rgb * 1.2, t.a * d.w), vec4<f32>(0.0), vec4<f32>(1.0));
}
"#;

static PLAIN_COPY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The drops drawn over a whole copy of the frame (true), as natively, also where [`LensPass::copy_sources`]
/// could take only their sources: for the test that compares the two.
pub fn set_plain_copy(on: bool) {
    PLAIN_COPY.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the drops may be drawn onto the frame itself ([`set_plain_copy`] off).
pub fn in_place_allowed() -> bool {
    !PLAIN_COPY.load(std::sync::atomic::Ordering::Relaxed)
}

static IN_PLACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The frames whose drops were drawn onto the frame itself so far (the comparison test's check that the
/// path ran).
pub fn in_place_frames() -> u64 {
    IN_PLACE.load(std::sync::atomic::Ordering::Relaxed)
}

/// One more frame drawn in place ([`in_place_frames`]).
pub fn count_in_place() {
    IN_PLACE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The drops' sources copied texel for texel (`LensPass::copy_sources`): one rectangle (NDC x0, y0, x1, y1)
/// an instance.
const COPY_WGSL: &str = r#"
struct Rects { r: array<vec4<f32>, 80> };
@group(0) @binding(0) var<uniform> rects: Rects;
@group(0) @binding(1) var frame: texture_2d<f32>;

@vertex fn vs_rect(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> @builtin(position) vec4<f32> {
    let r = rects.r[i];
    let c = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    return vec4<f32>(mix(r.xy, r.zw, c), 0.0, 1.0);
}

@fragment fn fs_rect(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(frame, vec2<i32>(p.xy), 0);
}
"#;

/// The texels a drop's source reads, (x0, y0, x1, y1) with x1, y1 exclusive, in a `w` × `h` image:
/// `fs` samples the frame at `(t.g · s + x − s/4, −t.b · s + y + s/4)`, `s` = 0.12 × size, t in 0..1
/// (the displacement texture's unorm channels), bilinear and clamped to the edge: the box
/// x − s/4 ..= x + 3s/4 by y − 3s/4 ..= y + s/4 and the filter's neighbour, two texels more each way
/// for the rounding. None when it lies off the image.
pub fn drop_source_texels(d: &LensDrop, w: u32, h: u32) -> Option<[u32; 4]> {
    let s = 0.12 * d.size;
    let (u0, u1) = (d.x - 0.25 * s, d.x + 0.75 * s);
    let (v0, v1) = (d.y - 0.75 * s, d.y + 0.25 * s);
    if !(u0.is_finite() && u1.is_finite() && v0.is_finite() && v1.is_finite()) {
        return Some([0, 0, w, h]);
    }
    let lo = |u: f32, n: u32| ((u * n as f32 - 0.5).floor() - 2.0).clamp(0.0, n as f32) as u32;
    let hi = |u: f32, n: u32| ((u * n as f32 - 0.5).floor() + 4.0).clamp(0.0, n as f32) as u32;
    let r = [lo(u0.min(u1), w), lo(v0.min(v1), h), hi(u0.max(u1), w), hi(v0.max(v1), h)];
    (r[2] > r[0] && r[3] > r[1]).then_some(r)
}

/// Lens Drops: at most this many chunks of [`SPLATS`] drops.
const CHUNKS: usize = 4;
const CHUNK_STRIDE: u64 = 512;

pub struct LensPass {
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    raindrop: Option<(u32, wgpu::TextureView)>,
    /// The sources' copy (`copy_sources`): its pipelines (per format), layout and rectangles.
    copy_pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    copy_layout: wgpu::BindGroupLayout,
    rects: wgpu::Buffer,
}

impl LensPass {
    /// Pipelines for each of `formats` (the targets it may draw into).
    pub fn new(gpu: &Gpu, formats: &[wgpu::TextureFormat]) -> LensPass {
        let device = &gpu.device;
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("lens drops"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex(1),
                tex(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("lens drops"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("lens drops"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipelines = formats
            .iter()
            .map(|&format| {
                (
                    format,
                    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                        label: Some("lens drops"),
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &module,
                            entry_point: Some("vs"),
                            buffers: &[],
                            compilation_options: Default::default(),
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &module,
                            entry_point: Some("fs"),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                                write_mask: wgpu::ColorWrites::COLOR,
                            })],
                            compilation_options: Default::default(),
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleStrip,
                            ..Default::default()
                        },
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview_mask: None,
                        cache: gpu.disk_pipeline_cache(),
                    }),
                )
            })
            .collect();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lens drops"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Room for Lens Drops' extra drops (docs/notes/graphics/weather.md): chunks of
        // 20 at 512-byte offsets; the native 20 are the first chunk.
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("lens drops"),
                size: CHUNK_STRIDE * CHUNKS as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let copy_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("lens drops sources"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let copy_pipeline_layout =
            crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("lens drops sources"),
                bind_group_layouts: &[Some(&copy_layout)],
                immediate_size: 0,
            });
        let copy_module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("lens drops sources"),
            source: wgpu::ShaderSource::Wgsl(COPY_WGSL.into()),
        });
        let copy_pipelines = formats
            .iter()
            .map(|&format| {
                (
                    format,
                    crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                        label: Some("lens drops sources"),
                        layout: Some(&copy_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &copy_module,
                            entry_point: Some("vs_rect"),
                            buffers: &[],
                            compilation_options: Default::default(),
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &copy_module,
                            entry_point: Some("fs_rect"),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                            compilation_options: Default::default(),
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleStrip,
                            ..Default::default()
                        },
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview_mask: None,
                        cache: gpu.disk_pipeline_cache(),
                    }),
                )
            })
            .collect();
        let rects = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("lens drops sources"),
                size: (SPLATS * CHUNKS * 16) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        LensPass { pipelines, layout, sampler, uniform, raindrop: None, copy_pipelines, copy_layout, rects }
    }

    /// The texels `drops` read from the frame (`drop_source_texels`) copied from `frame` into `into` (the same
    /// size and format), each at its own place: [`LensPass::draw`] can then draw them onto `frame` itself,
    /// sampling `into`, as it draws onto a whole copy of the frame. False (nothing recorded) without a
    /// pipeline for `format`.
    pub fn copy_sources(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        into: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        frame: &wgpu::TextureView,
        drops: &[LensDrop],
    ) -> bool {
        let Some((_, pipeline)) = self.copy_pipelines.iter().find(|(f, _)| *f == format) else { return false };
        let size = frame.texture().size();
        let (w, h) = (size.width, size.height);
        let rects: Vec<[f32; 4]> = drops
            .iter()
            .take(SPLATS * CHUNKS)
            .filter_map(|d| drop_source_texels(d, w, h))
            .map(|[x0, y0, x1, y1]| {
                let nx = |x: u32| x as f32 / w as f32 * 2.0 - 1.0;
                let ny = |y: u32| 1.0 - y as f32 / h as f32 * 2.0;
                [nx(x0), ny(y0), nx(x1), ny(y1)]
            })
            .collect();
        if rects.is_empty() {
            return true;
        }
        gpu.queue.write_buffer(&self.rects, 0, bytemuck::cast_slice(&rects));
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lens drops sources"),
            layout: &self.copy_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.rects.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(frame) },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lens drops sources"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: into,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..4, 0..rects.len() as u32);
        true
    }

    /// RAINDROPPC (0x7A3C6C), uploaded once per texture.
    pub fn set_texture(&mut self, gpu: &Gpu, texture: &Texture) {
        if self.raindrop.as_ref().is_some_and(|(h, _)| *h == texture.hash) {
            return;
        }
        let size = wgpu::Extent3d { width: texture.width, height: texture.height, depth_or_array_layers: 1 };
        let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RAINDROPPC"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let rgba = texture.decode_rgba();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..(texture.width * texture.height * 4) as usize],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(texture.width * 4), rows_per_image: None },
            size,
        );
        self.raindrop = Some((texture.hash, t.create_view(&Default::default())));
    }

    /// Draw `drops` onto `target`, sampling `source` (the frame so far).
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        source: &wgpu::TextureView,
        drops: &[LensDrop],
    ) {
        let Some((_, raindrop)) = &self.raindrop else { return };
        let Some((_, pipeline)) = self.pipelines.iter().find(|(f, _)| *f == format) else { return };
        let n = drops.len().min(SPLATS * CHUNKS);
        if n == 0 {
            return;
        }
        // The native 20 (0x5C7AC0) in the first chunk, Lens Drops' extra
        // ones (clone deviation) in the next, each a draw of its own.
        let mut groups = Vec::new();
        for (c, chunk) in drops[..n].chunks(SPLATS).enumerate() {
            let mut data = [[0.0f32; 4]; SPLATS];
            for (d, out) in chunk.iter().zip(data.iter_mut()) {
                *out = [d.x, d.y, d.size, d.alpha];
            }
            let offset = c as u64 * CHUNK_STRIDE;
            gpu.queue.write_buffer(&self.uniform, offset, bytemuck::cast_slice(&data));
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lens drops"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.uniform,
                            offset,
                            size: std::num::NonZeroU64::new((SPLATS * 16) as u64),
                        }),
                    },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(raindrop) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            groups.push((group, chunk.len() as u32));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lens drops"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        for (group, count) in &groups {
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..4, 0..*count);
        }
    }
}
