//! The horizon fog grid's pass (moved from render/postfx.rs).

use super::*;

impl PostFx {
    /// The horizon fog grid's pipeline and buffers, made once.
    pub(super) fn fog_pipeline(&mut self, gpu: &Gpu) {
        if self.fog.is_some() {
            return;
        }
        let pipeline = self.make_fog_pipeline(gpu, crate::render::SceneMode::NATIVE);
        use wgpu::util::DeviceExt;
        let device = &gpu.device;
        let size = (crate::horizon_fog::SIDE
            * crate::horizon_fog::SIDE
            * std::mem::size_of::<crate::horizon_fog::Vertex>()) as u64;
        let vb = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("horizon fog"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("horizon fog"),
            contents: bytemuck::cast_slice(&crate::horizon_fog::indices()),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.fog = Some((pipeline, vb, ib));
    }

    /// The horizon fog's pipeline for the scene's `mode`.
    pub(super) fn make_fog_pipeline(&self, gpu: &Gpu, mode: crate::render::SceneMode) -> wgpu::RenderPipeline {
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("horizon fog"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("horizon fog"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_fog"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::horizon_fog::Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_fog"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: mode.target(self.format),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(mode.compare()),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: mode.samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        pipeline
    }

    /// The horizon fog grid (0x5C7480) over the scene, depth-tested against
    /// `depth` (ZFUNC LESSEQUAL as 0x5C2440 leaves it, no z write),
    /// SRCALPHA / ONE, colour without alpha (horizon_fog.efx).
    pub fn horizon_fog(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        vertices: &[crate::horizon_fog::Vertex],
        opts: &crate::render::RenderOptions,
    ) {
        if self.targets.is_none() || vertices.len() != crate::horizon_fog::SIDE * crate::horizon_fog::SIDE {
            return;
        }
        self.fog_pipeline(gpu);
        // The scene's mode (with the Graphics page's Reversed-Z the grid's
        // depths, 0.9995 − row · 0.0001, go to 1 − z, and the test to
        // GREATEREQUAL).
        // The lamps' shader choice is the world's alone.
        // (nor the lamps' feature set: the grid's pipeline does not
        // read it, and a new lamp-row combination built it again.)
        let mode = crate::render::SceneMode {
            lamps: false,
            lamp_features: 0,
            modern: 0,
            bias_q: 0,
            ..crate::render::SceneMode::current(opts, self.samples)
        };
        if mode != crate::render::SceneMode::NATIVE && self.fog_ms.as_ref().is_none_or(|f| f.0 != mode) {
            self.fog_ms = Some((mode, self.make_fog_pipeline(gpu, mode)));
        }
        let t = self.targets.as_ref().unwrap();
        let (pipeline, vb, ib) = self.fog.as_ref().unwrap();
        if mode.reversed {
            let flipped: Vec<crate::horizon_fog::Vertex> = vertices
                .iter()
                .map(|v| {
                    let mut v = *v;
                    v.position[2] = 1.0 - v.position[2];
                    v
                })
                .collect();
            gpu.queue.write_buffer(vb, 0, bytemuck::cast_slice(&flipped));
        } else {
            gpu.queue.write_buffer(vb, 0, bytemuck::cast_slice(vertices));
        }
        let scene = match self.msaa.as_ref().filter(|_| self.samples > 1) {
            Some(m) => m.color.clone(),
            None => t.views.scene.clone(),
        };
        let pipeline =
            if mode == crate::render::SceneMode::NATIVE { pipeline } else { &self.fog_ms.as_ref().unwrap().1 };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("horizon fog"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &scene,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..crate::horizon_fog::INDEX_COUNT as u32, 0, 0..1);
    }
}
