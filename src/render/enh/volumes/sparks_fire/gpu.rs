use super::*;

/// The view a frame's draw needs.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// The projection (with the scene's jitter and depth convention).
    pub projection: Mat4,
    pub forward: Vec3,
    pub up: Vec3,
    pub eye: Vec3,
    pub near: f32,
    pub far: f32,
    pub reversed: bool,
    /// The target is a float (HDR) scene: emission may exceed 1 and bloom.
    pub hdr: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    /// xyz eye, w time.
    eye: [f32; 4],
    /// near, far, reversed, emission gain.
    depth: [f32; 4],
    /// viewport x, y, w, h (target pixels).
    viewport: [f32; 4],
    /// x lights, y haze on, zw unused.
    counts: [f32; 4],
}

/// The GPU half: instances, lights, the pipelines.
pub struct Renderer {
    frame: wgpu::Buffer,
    instances: wgpu::Buffer,
    lights: wgpu::Buffer,
    layouts: [wgpu::BindGroupLayout; 2],
    haze_layout: wgpu::BindGroupLayout,
    shaders: [wgpu::ShaderModule; 2],
    sampler: wgpu::Sampler,
    pipelines: std::sync::Mutex<std::collections::HashMap<(u8, wgpu::TextureFormat, u32, bool), wgpu::RenderPipeline>>,
    copy: std::sync::Mutex<Option<(wgpu::Texture, wgpu::TextureView, (u32, u32), wgpu::TextureFormat)>>,
    sparks: u32,
    flames: u32,
    flame_first: u32,
    glows: u32,
    glow_first: u32,
    light_count: u32,
    uniform: FrameUniform,
}

const SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/sparks_fire.wgsl"));

impl Renderer {
    pub fn new(gpu: &Gpu) -> Renderer {
        let device = &gpu.device;
        let buf = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let frame = buf("sparks fire frame", std::mem::size_of::<FrameUniform>() as u64, wgpu::BufferUsages::UNIFORM);
        let instances = buf(
            "sparks fire instances",
            ((MAX_SPARKS + MAX_FLAMES + MAX_GLOWS) * std::mem::size_of::<Instance>()) as u64,
            wgpu::BufferUsages::STORAGE,
        );
        let lights =
            buf("sparks fire lights", (MAX_LIGHTS * std::mem::size_of::<Light>()) as u64, wgpu::BufferUsages::STORAGE);
        let layout = |ms: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("sparks fire"),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: ms,
                        },
                        count: None,
                    },
                ],
            })
        };
        let haze_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("sparks fire haze"),
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
            ],
        });
        let module = |label: &str, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shaders = [
            module("sparks fire", SHADER.to_string()),
            module("sparks fire ms", SHADER.replace("texture_depth_2d", "texture_depth_multisampled_2d")),
        ];
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sparks fire haze"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Renderer {
            frame,
            instances,
            lights,
            layouts: [layout(false), layout(true)],
            haze_layout,
            shaders,
            sampler,
            pipelines: Default::default(),
            copy: Default::default(),
            sparks: 0,
            flames: 0,
            flame_first: 0,
            glows: 0,
            glow_first: 0,
            light_count: 0,
            uniform: bytemuck::Zeroable::zeroed(),
        }
    }

    /// Upload this frame's instances and lights.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        sparks: &[Instance],
        flames: &[Instance],
        glows: &[Instance],
        lights: &[Light],
        view: &View,
        time: f32,
    ) {
        // Everything relative to the eye (the world's coordinates run to
        // thousands of metres: an inverse with them in it loses the depth).
        let rel = |v: [f32; 4], e: Vec3| [v[0] - e.x, v[1] - e.y, v[2] - e.z, v[3]];
        let sparks: Vec<Instance> = sparks[..sparks.len().min(MAX_SPARKS)]
            .iter()
            .map(|i| Instance { a: rel(i.a, view.eye), b: rel(i.b, view.eye), c: i.c })
            .collect();
        let flames: Vec<Instance> = flames[..flames.len().min(MAX_FLAMES)]
            .iter()
            .map(|i| Instance { a: rel(i.a, view.eye), b: i.b, c: i.c })
            .collect();
        let glows: Vec<Instance> = glows[..glows.len().min(MAX_GLOWS)]
            .iter()
            .map(|i| Instance { a: rel(i.a, view.eye), b: i.b, c: i.c })
            .collect();
        let lights: Vec<Light> = lights[..lights.len().min(MAX_LIGHTS)]
            .iter()
            .map(|l| Light { position: rel(l.position, view.eye), colour: l.colour })
            .collect();
        let (sparks, flames, glows, lights) = (&sparks[..], &flames[..], &glows[..], &lights[..]);
        let view_proj = view.projection * Mat4::look_to_rh(Vec3::ZERO, view.forward, view.up);
        self.sparks = sparks.len() as u32;
        self.flames = flames.len() as u32;
        self.flame_first = MAX_SPARKS as u32;
        self.glows = glows.len() as u32;
        self.glow_first = (MAX_SPARKS + MAX_FLAMES) as u32;
        self.light_count = lights.len() as u32;
        if !glows.is_empty() {
            gpu.queue.write_buffer(
                &self.instances,
                (self.glow_first as usize * std::mem::size_of::<Instance>()) as u64,
                bytemuck::cast_slice(glows),
            );
        }
        if !sparks.is_empty() {
            gpu.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(sparks));
        }
        if !flames.is_empty() {
            gpu.queue.write_buffer(
                &self.instances,
                (MAX_SPARKS * std::mem::size_of::<Instance>()) as u64,
                bytemuck::cast_slice(flames),
            );
        }
        if !lights.is_empty() {
            gpu.queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(lights));
        }
        self.uniform = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            eye: [0.0, 0.0, 0.0, time],
            depth: [view.near, view.far, if view.reversed { 1.0 } else { 0.0 }, if view.hdr { 1.0 } else { 0.0 }],
            viewport: [0.0; 4],
            counts: [self.light_count as f32, 0.0, 0.0, 0.0],
        };
    }

    /// Anything to draw this frame.
    pub fn busy(&self) -> bool {
        self.sparks + self.flames + self.glows + self.light_count > 0
    }

    /// pass 0 lights, 1 haze, 2 sparks, 3 flames, 4 contact glows.
    fn pipeline(
        &self,
        gpu: &Gpu,
        pass: u8,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (pass, format, samples, ms);
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(p) = pipelines.get(&key) {
            return p.clone();
        }
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = if pass == 1 {
            vec![Some(&self.layouts[usize::from(ms)]), Some(&self.haze_layout)]
        } else {
            vec![Some(&self.layouts[usize::from(ms)])]
        };
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("sparks fire"),
            bind_group_layouts: &layouts,
            immediate_size: 0,
        });
        let keep_alpha = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let blend = match pass {
            // dst + dst · light: the surface's own colour lit.
            0 => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: keep_alpha,
            },
            // The refracted copy over the scene by its mask.
            1 => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: keep_alpha,
            },
            // Emission added.
            _ => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: keep_alpha,
            },
        };
        let (vs, fs) = match pass {
            0 => ("vs_light", "fs_light"),
            1 => ("vs_fx", "fs_haze"),
            2 => ("vs_fx", "fs_spark"),
            4 => ("vs_fx", "fs_glow"),
            _ => ("vs_fx", "fs_flame"),
        };
        let module = &self.shaders[usize::from(ms)];
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("sparks fire"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: samples.max(1), ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        pipelines.insert(key, p.clone());
        p
    }

    /// Build the pipelines ahead (no first-use hitch).
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        for pass in 0..5 {
            self.pipeline(gpu, pass, format, samples, depth_samples > 1);
        }
    }

    /// Lights, haze, sparks and flames into `target` (`samples`), against
    /// `depth`; `viewport` the temporal pass's (late draw) when given.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
    ) {
        if !self.busy() {
            return;
        }
        let size = target.texture().size();
        let vp = viewport.unwrap_or([0.0, 0.0, size.width as f32, size.height as f32]);
        // Heat haze needs a copy of the scene: resolved from a multisampled
        // target, or copied from a single-sampled one that allows it.
        let tex = target.texture();
        let haze = self.flames > 0
            && (tex.sample_count() > 1 || tex.usage().contains(wgpu::TextureUsages::COPY_SRC))
            && tex.dimension() == wgpu::TextureDimension::D2;
        let mut u = self.uniform;
        u.viewport = vp;
        u.counts[1] = if haze { 1.0 } else { 0.0 };
        gpu.queue.write_buffer(&self.frame, 0, bytemuck::bytes_of(&u));
        let ms = depth_samples > 1;
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sparks fire"),
            layout: &self.layouts[usize::from(ms)],
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.instances.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth) },
            ],
        });
        let haze_group = if haze { Some(self.scene_copy(gpu, encoder, target, format)) } else { None };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sparks & fire"),
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
        pass.set_bind_group(0, &group, &[]);
        // NFSU2_SF_PASSES=<mask> (tuning aid): bit 0 lights, 1 haze, 2 sparks, 3 flames, 4 glows.
        let mask = crate::render::debug_env::nfsu2_sf_passes().unwrap_or(31);
        let haze_group = haze_group.filter(|_| mask & 2 != 0);
        if self.light_count > 0 && mask & 1 != 0 {
            pass.set_pipeline(&self.pipeline(gpu, 0, format, samples, ms));
            pass.draw(0..6, 0..self.light_count);
        }
        if let Some(h) = haze_group.as_ref() {
            pass.set_pipeline(&self.pipeline(gpu, 1, format, samples, ms));
            pass.set_bind_group(1, h, &[]);
            pass.draw(0..6, self.flame_first..self.flame_first + self.flames);
        }
        if self.glows > 0 && mask & 16 != 0 {
            pass.set_pipeline(&self.pipeline(gpu, 4, format, samples, ms));
            pass.draw(0..6, self.glow_first..self.glow_first + self.glows);
        }
        if self.sparks > 0 && mask & 4 != 0 {
            pass.set_pipeline(&self.pipeline(gpu, 2, format, samples, ms));
            pass.draw(0..6, 0..self.sparks);
        }
        if self.flames > 0 && mask & 8 != 0 {
            pass.set_pipeline(&self.pipeline(gpu, 3, format, samples, ms));
            pass.draw(0..6, self.flame_first..self.flame_first + self.flames);
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: sparks & fire");
    }

    /// The scene as it stands, in a single-sampled texture the haze reads.
    fn scene_copy(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> wgpu::BindGroup {
        let tex = target.texture();
        let size = (tex.width(), tex.height());
        let mut copy = self.copy.lock().unwrap();
        if copy.as_ref().is_none_or(|c| c.2 != size || c.3 != format) {
            let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sparks fire scene copy"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let v = t.create_view(&Default::default());
            *copy = Some((t, v, size, format));
        }
        let (t, v, ..) = copy.as_ref().unwrap();
        if tex.sample_count() > 1 {
            // A pass with nothing drawn: the target resolved into the copy.
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sparks fire scene resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: Some(v),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(pass);
        } else {
            encoder.copy_texture_to_texture(
                tex.as_image_copy(),
                t.as_image_copy(),
                wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            );
        }
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sparks fire haze"),
            layout: &self.haze_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(v) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }
}

/// The tier's state in the game: the simulation and, once drawn, the GPU half.
#[derive(Default)]
pub struct SparksFire {
    pub sim: Sim,
    pub renderer: Option<Renderer>,
    /// Whether the last prepare handed the GPU anything (the quads then
    /// leave the sparks and flames out).
    pub active: bool,
}

impl SparksFire {
    /// The native particle `p` is drawn by this tier (its quad left out).
    pub fn replaces(&self, p: &Particle) -> bool {
        self.active && self.sim.kind_cached(p.description as usize).is_some()
    }

    /// Build the frame: instances from the fragments and the native flames,
    /// the lights; upload. `on`: the row's tier is not Original.
    /// The fragments drawn `alpha` of the way between the last
    /// two ticks ([`Sim::drawn`]); the sim is stepped (and cleared with
    /// the row off) on the tick only.
    pub fn prepare(&mut self, gpu: &Gpu, on: bool, library: &Library, particles: &[Particle], view: &View, alpha: f32) {
        self.active = on;
        if !on {
            if let Some(r) = self.renderer.as_mut() {
                r.prepare(gpu, &[], &[], &[], &[], view, 0.0);
            }
            return;
        }
        // The kinds cache (draw side) follows the library.
        let _ = self.sim.kind(library, 0);
        let mut scratch = Vec::new();
        let drawn = self.sim.drawn(alpha, &mut scratch);
        let mut sparks = Vec::with_capacity(drawn.len());
        spark_instances(drawn, &mut sparks);
        let mut flames = Vec::new();
        flame_instances(library, &self.sim, particles, &mut flames);
        let mut glows = Vec::new();
        glow_instances(library, &self.sim, particles, &mut glows);
        let lights = lights(library, &self.sim, drawn, particles);
        let r = self.renderer.get_or_insert_with(|| Renderer::new(gpu));
        r.prepare(gpu, &sparks, &flames, &glows, &lights, view, self.sim.time);
    }

    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32) {
        if let Some(r) = self.renderer.as_ref().filter(|_| self.active) {
            r.warm(gpu, format, samples, depth_samples);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
    ) {
        if let Some(r) = self.renderer.as_ref().filter(|_| self.active) {
            r.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
        }
    }
}
