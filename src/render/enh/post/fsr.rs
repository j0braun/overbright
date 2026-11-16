//! FSR 1 upscaling (docs/modern-extras.md; clone deviation, opt-in on
//! the Upscaling row as "FSR Quality / Balanced / Performance").
//!
//! AMD FidelityFX Super Resolution 1.0 (MIT): EASU takes the scene from its
//! render size to the screen's, RCAS sharpens the result (`fsr.wgsl`).
//! FSR 2 was ported later as its own tiers (`fsr2.rs`, docs/fsr2.md);
//! FSR 1 stays: two small passes, adds no ghosting, needs
//! no history, keeps the native MSAA as its anti-aliasing, and works on
//! every frame the world is drawn (no history to reset on cuts). Same flow
//! as TAAU: the scene drawn at the share into the upscaling stage
//! (`PostFx::set_upscale`), the stage's pre-passes (MSAA resolve, SSAO, SSR,
//! volumetric fog), then EASU into the screen-size scene, RCAS, the depth at
//! the screen's size for the late sprites (particles, glow sprites, rain),
//! then the native post chain (streaks, bleach, overbright glow) and the
//! HUD at the screen's size.

use crate::render::Gpu;

/// RCAS sharpness in stops (0 sharpest; AMD's default 0.2).
pub const SHARPNESS_STOPS: f32 = 0.2;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    input: [f32; 4],
    output: [f32; 4],
    params: [f32; 4],
}

pub struct Fsr {
    shader: wgpu::ShaderModule,
    easu: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    rcas: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    depth: [Option<wgpu::RenderPipeline>; 2],
    uniforms: [wgpu::Buffer; 2],
    /// EASU's output (RCAS reads it).
    tmp: Option<((u32, u32), wgpu::TextureFormat, wgpu::TextureView)>,
}

fn uniforms(input: (u32, u32), output: (u32, u32), sharp: f32) -> Uniforms {
    let v = |s: (u32, u32)| [s.0 as f32, s.1 as f32, 1.0 / s.0.max(1) as f32, 1.0 / s.1.max(1) as f32];
    Uniforms { input: v(input), output: v(output), params: [sharp, 0.0, 0.0, 0.0] }
}

/// RCAS's linear sharpness for `stops` (ffx_fsr1.h FsrRcasCon: exp2(−stops)).
pub fn sharpness(stops: f32) -> f32 {
    (-stops).exp2()
}

impl Fsr {
    pub fn new(gpu: &Gpu) -> Fsr {
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("fsr"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/fsr.wgsl")).into(),
            ),
        });
        let buf = || {
            crate::render::zeroed_buffer(
                &gpu.device,
                &wgpu::BufferDescriptor {
                    label: Some("fsr"),
                    size: std::mem::size_of::<Uniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        Fsr { shader, easu: Vec::new(), rcas: Vec::new(), depth: [None, None], uniforms: [buf(), buf()], tmp: None }
    }

    /// The EASU and RCAS pipelines into `format` that [`Fsr::run`] makes
    /// on its first use, made now (docs/first-use-builds.md): the same
    /// descriptors, from the device's cache.
    pub fn warm(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) {
        if !self.easu.iter().any(|(f, _)| *f == format) {
            let e = self.color_pipeline(gpu, "fs_easu", format);
            let r = self.color_pipeline(gpu, "fs_rcas", format);
            self.easu.push((format, e));
            self.rcas.push((format, r));
        }
    }

    fn color_pipeline(&self, gpu: &Gpu, entry: &str, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: None,
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        })
    }

    fn depth_pipeline(&self, gpu: &Gpu, ms: bool) -> wgpu::RenderPipeline {
        crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("fsr depth"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(if ms { "fs_depth_ms" } else { "fs_depth" }),
                compilation_options: Default::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        })
    }

    /// `color` (`input` big, the resolved scene of the stage) upscaled into
    /// `out` (`output` big, `format`) by EASU then RCAS; with `depth` /
    /// `out_depth` the input's depth (`depth_samples`) written at the
    /// output's size.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        input: (u32, u32),
        out: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        output: (u32, u32),
        depth: Option<(&wgpu::TextureView, u32, &wgpu::TextureView)>,
    ) {
        let sharp = sharpness(SHARPNESS_STOPS);
        gpu.queue.write_buffer(&self.uniforms[0], 0, bytemuck::bytes_of(&uniforms(input, output, sharp)));
        gpu.queue.write_buffer(&self.uniforms[1], 0, bytemuck::bytes_of(&uniforms(output, output, sharp)));
        if self.tmp.as_ref().is_none_or(|(s, f, _)| *s != output || *f != format) {
            let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("fsr easu"),
                size: wgpu::Extent3d { width: output.0.max(1), height: output.1.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.tmp = Some((output, format, t.create_view(&Default::default())));
        }
        self.warm(gpu, format);
        let easu = &self.easu.iter().find(|(f, _)| *f == format).unwrap().1;
        let rcas = &self.rcas.iter().find(|(f, _)| *f == format).unwrap().1;
        let tmp = self.tmp.as_ref().unwrap().2.clone();
        let group = |p: &wgpu::RenderPipeline, src: &wgpu::TextureView, u: &wgpu::Buffer| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fsr"),
                layout: &p.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 1, resource: u.as_entire_binding() },
                ],
            })
        };
        pass(encoder, "fsr easu", &tmp, easu, &group(easu, color, &self.uniforms[0]));
        crate::prof::gpu::mark(encoder, "graphics: fsr easu");
        pass(encoder, "fsr rcas", out, rcas, &group(rcas, &tmp, &self.uniforms[1]));
        crate::prof::gpu::mark(encoder, "graphics: fsr rcas");
        if let Some((depth, samples, out_depth)) = depth {
            let ms = samples > 1;
            if self.depth[usize::from(ms)].is_none() {
                self.depth[usize::from(ms)] = Some(self.depth_pipeline(gpu, ms));
            }
            let p = self.depth[usize::from(ms)].as_ref().unwrap();
            let g = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fsr depth"),
                layout: &p.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 1, resource: self.uniforms[0].as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: if ms { 3 } else { 2 },
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                ],
            });
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fsr depth"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: out_depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(p);
            rp.set_bind_group(0, &g, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

fn pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    p.set_pipeline(pipeline);
    p.set_bind_group(0, group, &[]);
    p.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rcas_sharpness_and_sizes() {
        assert!((sharpness(0.0) - 1.0).abs() < 1e-6);
        assert!((sharpness(SHARPNESS_STOPS) - 0.8706).abs() < 1e-3);
        // The FSR tiers' render sizes match TAAU's at the same share.
        for (v, f) in [(1u8, 4u8), (2, 5), (3, 6)] {
            assert_eq!(crate::qol::upscale_share(v), crate::qol::upscale_share(f));
            assert!(crate::qol::is_fsr(f) && !crate::qol::is_fsr(v));
        }
        assert_eq!(crate::qol::upscale_size((3840, 2160), crate::qol::upscale_share(4).unwrap()), (2573, 1447));
    }

    /// EASU and RCAS on a GPU: a flat image stays flat (the kernel's
    /// weights sum to one, RCAS adds nothing without contrast), an edge
    /// overshoots only by RCAS's own small lobe (EASU deringing).
    #[test]
    #[ignore = "needs a GPU"]
    fn easu_rcas_keep_flat_and_edges() {
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).expect("gpu");
        let fmt = wgpu::TextureFormat::Rgba8Unorm;
        let (iw, ih, ow, oh) = (32u32, 32u32, 48u32, 48u32);
        let mut px = vec![0u8; (iw * ih * 4) as usize];
        for y in 0..ih {
            for x in 0..iw {
                let v = if x < iw / 2 { 40 } else { 200 };
                let i = ((y * iw + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let tex = |w, h, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: fmt,
                usage,
                view_formats: &[],
            })
        };
        let input = tex(iw, ih, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        gpu.queue.write_texture(
            input.as_image_copy(),
            &px,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(iw * 4), rows_per_image: None },
            input.size(),
        );
        let out = tex(ow, oh, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let mut fsr = Fsr::new(&gpu);
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        fsr.run(
            &gpu,
            &mut enc,
            &input.create_view(&Default::default()),
            (iw, ih),
            &out.create_view(&Default::default()),
            fmt,
            (ow, oh),
            None,
        );
        gpu.queue.submit([enc.finish()]);
        let back = crate::render::read_back(&gpu, &out, ow, oh);
        for y in 4..oh - 4 {
            for x in 0..ow {
                let v = back[((y * ow + x) * 4) as usize];
                // RCAS bounds its lobe to stay within 0..1 (not the 2x2), so a
                // little overshoot at the edge is its own.
                assert!((32..=208).contains(&v), "({x},{y}) {v}");
                if x < 18 {
                    assert!((v as i32 - 40).abs() <= 2, "flat left ({x},{y}) {v}");
                }
                if x > 30 {
                    assert!((v as i32 - 200).abs() <= 2, "flat right ({x},{y}) {v}");
                }
            }
        }
    }
}
