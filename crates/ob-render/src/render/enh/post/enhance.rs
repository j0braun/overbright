//! The Graphics page's screen passes (**clone deviations**, each opt-in;
//! docs/formats.md "Graphics enhancements"): after the native post passes,
//! Resample to output resolution, SMAA 1x, FXAA and contrast-adaptive
//! sharpening, in that order, each
//! only when asked for. With the page at its native values none runs and the post
//! passes write the screen as before ([`crate::render::postfx::PostFx::apply`]).

use crate::render::Gpu;
use crate::render::smaa::Smaa;

/// What the passes do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub fxaa: bool,
    /// SMAA 1x ([`crate::render::smaa`], smaa.wgsl).
    pub smaa: bool,
    /// CAS sharpness, 0 (off) to 1.
    pub sharpen: f32,
    /// docs/playing/modern-extras.md: the scene's upscale to the screen is
    /// AMD FSR 1 (EASU + RCAS, [`crate::render::fsr`]) instead of the resample.
    pub fsr: bool,
}

static FSR_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether this frame's upscale is FSR 1 (the game's world with an FSR
/// Upscaling tier; the front end and the garage resample as before).
pub fn set_fsr_frame(on: bool) {
    FSR_FRAME.store(on, std::sync::atomic::Ordering::Relaxed);
}

impl Plan {
    /// The Graphics page's values in force, from the frame's settings.
    pub fn of(opts: &crate::render::RenderOptions) -> Plan {
        Plan {
            fxaa: opts.fxaa,
            smaa: opts.smaa,
            sharpen: opts.sharpen,
            fsr: FSR_FRAME.load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Whether anything runs for a scene `src` big shown `out` big.
    pub fn any(self, src: (u32, u32), out: (u32, u32)) -> bool {
        self.fxaa || self.smaa || self.sharpen > 0.0 || src != out
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PassUniform {
    pub(crate) texel: [f32; 4],
    pub(crate) params: [f32; 4],
}

const FXAA: usize = 0;
const RESAMPLE: usize = 1;
const CAS: usize = 2;
const ENTRIES: [&str; 6] = ["fs_fxaa", "fs_resample", "fs_cas", "fs_resample", "fs_resample", "fs_resample"];

// Select from dimensions, not a rounded float ratio: resized/fractional
// targets must retain the general footprint integrator.
fn resample_pipeline(src: (u32, u32), out: (u32, u32)) -> usize {
    for n in 2..=4 {
        if out.0.checked_mul(n) == Some(src.0) && out.1.checked_mul(n) == Some(src.1) && out.0 > 0 && out.1 > 0 {
            return n as usize + 1;
        }
    }
    RESAMPLE
}

struct Target {
    size: (u32, u32),
    view: wgpu::TextureView,
}

pub struct Enhance {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<wgpu::RenderPipeline>,
    sampler: wgpu::Sampler,
    uniforms: [wgpu::Buffer; 3],
    /// Each pass's bind group and the view it reads.
    groups: [Option<(wgpu::TextureView, wgpu::BindGroup)>; 3],
    /// FXAA's output at the scene's size; the resample's at the screen's
    /// (when CAS follows it).
    scene_tmp: Option<Target>,
    screen_tmp: Option<Target>,
    /// SMAA's passes and targets, made on its first use.
    smaa: Option<Smaa>,
    /// FSR 1's passes, made on first use.
    fsr: Option<crate::render::fsr::Fsr>,
}

fn target(gpu: &Gpu, label: &str, size: (u32, u32), format: wgpu::TextureFormat) -> Target {
    let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    Target { size, view: t.create_view(&Default::default()) }
}

impl Enhance {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Enhance {
        let device = &gpu.device;
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("enhance"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("enhance"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("enhance.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/enhance.wgsl")).into(),
            ),
        });
        let pipelines = ENTRIES
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &[("INTEGER_SCALE", if index >= 3 { (index - 1) as f64 } else { 0.0 })],
                            ..Default::default()
                        },
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
            .collect();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("enhance"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = || {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor {
                    label: Some("enhance"),
                    size: std::mem::size_of::<PassUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        Enhance {
            format,
            layout,
            pipelines,
            sampler,
            uniforms: [uniform(), uniform(), uniform()],
            groups: [None, None, None],
            scene_tmp: None,
            screen_tmp: None,
            smaa: None,
            fsr: None,
        }
    }

    fn pass(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        which: usize,
        source: &wgpu::TextureView,
        src: (u32, u32),
        target: &wgpu::TextureView,
        out: (u32, u32),
        sharpen: f32,
    ) {
        let u = PassUniform {
            texel: [1.0 / src.0.max(1) as f32, 1.0 / src.1.max(1) as f32, src.0 as f32, src.1 as f32],
            params: [sharpen, src.0 as f32 / out.0.max(1) as f32, src.1 as f32 / out.1.max(1) as f32, 0.0],
        };
        gpu.queue.write_buffer(&self.uniforms[which], 0, bytemuck::bytes_of(&u));
        if self.groups[which].as_ref().is_none_or(|(v, _)| v != source) {
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("enhance"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.uniforms[which].as_entire_binding() },
                ],
            });
            self.groups[which] = Some((source.clone(), group));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(ENTRIES[which]),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let pipeline = if which == RESAMPLE { resample_pipeline(src, out) } else { which };
        pass.set_pipeline(&self.pipelines[pipeline]);
        pass.set_bind_group(0, &self.groups[which].as_ref().unwrap().1, &[]);
        pass.draw(0..3, 0..1);
    }

    /// The passes `plan` asks for, from `source` (`src` big, the post
    /// passes' last image) to `output` (`out` big, the screen).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        src: (u32, u32),
        output: &wgpu::TextureView,
        out: (u32, u32),
        plan: Plan,
    ) {
        let scaled = src != out;
        let sharpen = plan.sharpen > 0.0;
        let mut cur = source.clone();
        // All post filters use output pixels: supersampling resolves first.
        // Two distinct scratch views allow SMAA + FXAA + CAS without feedback.
        if scaled {
            let last = !plan.smaa && !plan.fxaa && !sharpen;
            if !last && self.scene_tmp.as_ref().is_none_or(|t| t.size != out) {
                self.scene_tmp = Some(target(gpu, "AA downsample", out, self.format));
            }
            let to = if last { output.clone() } else { self.scene_tmp.as_ref().unwrap().view.clone() };
            if plan.fsr && src.0 < out.0 && src.1 < out.1 {
                // EASU + RCAS from the scene's size to the screen's.
                let format = self.format;
                self.fsr
                    .get_or_insert_with(|| crate::render::fsr::Fsr::new(gpu))
                    .run(gpu, encoder, &cur, src, &to, format, out, None);
            } else {
                self.pass(gpu, encoder, RESAMPLE, &cur, src, &to, out, 0.0);
                crate::prof::gpu::mark(encoder, "graphics: render scale");
            }
            if last {
                return;
            }
            cur = to;
        }
        if plan.smaa {
            let last = !plan.fxaa && !sharpen;
            if !last && self.screen_tmp.as_ref().is_none_or(|t| t.size != out) {
                self.screen_tmp = Some(target(gpu, "AA SMAA", out, self.format));
            }
            let to = if last { output.clone() } else { self.screen_tmp.as_ref().unwrap().view.clone() };
            let format = self.format;
            self.smaa.get_or_insert_with(|| Smaa::new(gpu, format)).run(gpu, encoder, &self.sampler, &cur, out, &to);
            crate::prof::gpu::mark(encoder, "graphics: smaa");
            if last {
                return;
            }
            cur = to;
        }
        if plan.fxaa {
            let last = !sharpen;
            // When scaled but no SMAA, cur already reads scene_tmp.
            // Use screen_tmp in that case, scene_tmp otherwise.
            let scratch = if scaled && !plan.smaa { &mut self.screen_tmp } else { &mut self.scene_tmp };
            if !last && scratch.as_ref().is_none_or(|t| t.size != out) {
                *scratch = Some(target(gpu, "AA FXAA", out, self.format));
            }
            let to = if last { output.clone() } else { scratch.as_ref().unwrap().view.clone() };
            self.pass(gpu, encoder, FXAA, &cur, out, &to, out, 0.0);
            crate::prof::gpu::mark(encoder, "graphics: fxaa");
            if last {
                return;
            }
            cur = to;
        }
        self.pass(gpu, encoder, CAS, &cur, out, output, out, plan.sharpen);
        crate::prof::gpu::mark(encoder, "graphics: sharpen");
    }
}
