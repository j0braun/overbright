//! Photo mode's grade (docs/playing/photo-mode.md; clone deviation, not in
//! SPEED2.EXE): after the game's own post chain (its tint, bleach bypass,
//! overbright glow — the original grade, unchanged) one full-screen pass
//! applies the photo's exposure (a gain, 2^EV), an optional bleach (a
//! luminance overlay mixed in by the Bleach row) and a filter (black and
//! white or sepia). Alpha (the UI's destination-alpha mask) is the
//! pixel's own. Only drawn while photo mode is up.

use crate::render::Gpu;
use std::collections::HashMap;

const SHADER: &str = "
struct U { gain: f32, mode: u32, bleach: f32, pad: f32 };
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> u: U;
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(frag.xy), 0);
    var rgb = c.rgb * u.gain;
    let l = dot(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.299, 0.587, 0.114));
    // bleach: the luminance overlaid on the colour (a silver retention look)
    let lo = 2.0 * rgb * l;
    let hi = 1.0 - 2.0 * (1.0 - rgb) * (1.0 - l);
    let over = select(hi, lo, vec3<f32>(l) < vec3<f32>(0.5));
    rgb = mix(rgb, mix(over, vec3<f32>(l), 0.35), u.bleach);
    let y = dot(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.299, 0.587, 0.114));
    if u.mode == 1u {
        rgb = vec3<f32>(y);
    } else if u.mode == 2u {
        rgb = y * vec3<f32>(1.07, 0.92, 0.70) + vec3<f32>(0.02, 0.01, 0.0);
    }
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), c.a);
}
";

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    gain: f32,
    filter: u32,
    bleach: f32,
    pad: f32,
}

/// The pass and its intermediate target (the post chain's output while
/// photo mode is up).
pub struct PhotoGrade {
    layout: wgpu::BindGroupLayout,
    module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    uniforms: wgpu::Buffer,
    target: Option<((u32, u32), wgpu::TextureFormat, wgpu::TextureView)>,
}

impl PhotoGrade {
    pub fn new(gpu: &Gpu) -> PhotoGrade {
        let device = &gpu.device;
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("photo grade"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("photo grade"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("photo grade"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let uniforms = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("photo grade"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        PhotoGrade { layout, module, pipeline_layout, pipelines: HashMap::new(), uniforms, target: None }
    }

    /// The intermediate the post chain draws into (`size`, `format`).
    pub fn target(&mut self, gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> wgpu::TextureView {
        if self.target.as_ref().is_none_or(|(s, f, _)| *s != size || *f != format) {
            let view = gpu
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("photo grade source"),
                    size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            self.target = Some((size, format, view));
        }
        self.target.as_ref().unwrap().2.clone()
    }

    /// Drops the intermediate (photo mode closed, or after a capture's
    /// large one).
    pub fn release(&mut self) {
        self.target = None;
    }

    /// The grade's pipeline into `format`, made where missing.
    fn pipeline(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let (module, pl) = (&self.module, &self.pipeline_layout);
        self.pipelines.entry(format).or_insert_with(|| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("photo grade"),
                layout: Some(pl),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        })
        .clone()
    }

    /// The pipeline a [`PhotoGrade::run`] into `format` uses, made now (a
    /// warm-up's copy, docs/developing/first-use-builds.md).
    pub fn warm(mut self, gpu: &Gpu, format: wgpu::TextureFormat) {
        self.pipeline(gpu, format);
    }

    /// The grade of the intermediate into `output` (`format`).
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        look: &crate::photo::Look,
    ) {
        if self.target.is_none() {
            return;
        }
        let u =
            Uniforms { gain: look.gain(), filter: look.filter.id(), bleach: f32::from(look.bleach) / 100.0, pad: 0.0 };
        gpu.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let pipeline = self.pipeline(gpu, format);
        let Some((_, _, src)) = self.target.as_ref() else { return };
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("photo grade"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                wgpu::BindGroupEntry { binding: 1, resource: self.uniforms.as_entire_binding() },
            ],
        });
        let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("photo grade"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&pipeline);
        p.set_bind_group(0, &group, &[]);
        p.draw(0..3, 0..1);
    }
}
