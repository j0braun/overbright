use super::*;

/// The half-size targets' format (colour + transmittance; alpha sum).
pub(super) const HALF_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// For speed (docs/notes/performance/gpu.md, the project's performance rule for the
/// enhancements): the detailed smoke drawn at half size per axis and put
/// over the scene in one pass (smoke_half.wgsl). On unless
/// `NFSU2_SMOKE_FULL=1` (the full-size draw, as before).
pub fn half_size_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| !crate::env_set!("NFSU2_SMOKE_FULL"))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HalfUniform {
    depth_map: [f32; 4],
    half: [f32; 4],
    flags: [f32; 4],
}

/// The half-size draw's targets and the composite (made with the GPU half
/// when the device has dual-source blending).
pub(super) struct HalfSize {
    layouts: [wgpu::BindGroupLayout; 2],
    shaders: [wgpu::ShaderModule; 2],
    uniform: wgpu::Buffer,
    pipelines: std::sync::Mutex<std::collections::HashMap<(wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>>,
    targets: std::sync::Mutex<Option<((u32, u32), wgpu::TextureView, wgpu::TextureView)>>,
}

impl HalfSize {
    pub(super) fn new(gpu: &Gpu) -> Option<HalfSize> {
        let device = &gpu.device;
        if !device.features().contains(wgpu::Features::DUAL_SOURCE_BLENDING) {
            return None;
        }
        let f = wgpu::ShaderStages::FRAGMENT;
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: f,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = |multisampled| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("smoke composite"),
                entries: &[
                    tex(0),
                    tex(1),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: f,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    uniform(3, false, f),
                ],
            })
        };
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/smoke_half.wgsl"));
        let module = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shaders = [
            module("smoke composite", source.to_string()),
            module("smoke composite ms", source.replace("texture_depth_2d", "texture_depth_multisampled_2d")),
        ];
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("smoke composite"),
            size: std::mem::size_of::<HalfUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(HalfSize {
            layouts: [layout(false), layout(true)],
            shaders,
            uniform,
            pipelines: Default::default(),
            targets: Default::default(),
        })
    }

    /// The two half-size targets of `size` (made again when it changes).
    pub(super) fn targets(&self, gpu: &Gpu, size: (u32, u32)) -> (wgpu::TextureView, wgpu::TextureView) {
        let mut t = self.targets.lock().unwrap();
        if t.as_ref().is_none_or(|(s, _, _)| *s != size) {
            let make = |label| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: HALF_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            *t = Some((size, make("smoke half colour"), make("smoke half alpha")));
        }
        let (_, c, a) = t.as_ref().unwrap();
        (c.clone(), a.clone())
    }

    /// The composite's pipeline into `format` (`samples`), against a depth
    /// with (`ms`) or without samples.
    pub(super) fn pipeline(
        &self,
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (format, samples, ms);
        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines
            .entry(key)
            .or_insert_with(|| {
                let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                    label: Some("smoke composite"),
                    bind_group_layouts: &[Some(&self.layouts[usize::from(ms)])],
                    immediate_size: 0,
                });
                let module = &self.shaders[usize::from(ms)];
                // scene · T + C, alpha · T + A (T in source 1).
                let blend = wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::Src1,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::Src1Alpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                };
                crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                    label: Some("smoke composite"),
                    layout: Some(&layout),
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
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                    multiview_mask: None,
                    cache: gpu.disk_pipeline_cache(),
                })
            })
            .clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn composite(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        ms: bool,
        (colour, alpha): (&wgpu::TextureView, &wgpu::TextureView),
        hsize: (u32, u32),
        map: [f32; 4],
        viewport: Option<[f32; 4]>,
        frame_depth: &[f32; 3],
    ) {
        let pipeline = self.pipeline(gpu, format, samples, ms);
        let u = HalfUniform {
            depth_map: map,
            half: [hsize.0 as f32, hsize.1 as f32, frame_depth[0], frame_depth[1]],
            flags: [frame_depth[2], 0.0, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke composite"),
            layout: &self.layouts[usize::from(ms)],
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(colour) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(alpha) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 3, resource: self.uniform.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("smoke composite"),
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
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
