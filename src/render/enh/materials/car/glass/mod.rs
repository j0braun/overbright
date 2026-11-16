//! Enhanced Glass (clone deviation, opt-in; docs/lighting2.md): the
//! car's glass draws — the window glass (WINDSHIELD, WINDOWMASK and the
//! WINDSHIELD_TINT_* materials the Window Tint part picks, 0x638DC5) and
//! the lamp covers (CLEARPLASTIC, HEADLIGHTGLASS), all alpha-blended in
//! car.efx — are left out of the car pass and drawn after it over a copy of
//! the scene (resolved when multisampled), by car_glass.wgsl: the native
//! colour and alpha as the base, what is behind read through a screen-space
//! refraction, and a Fresnel reflection of the car's cube per pixel. Wet Car
//! and the glass: a wet car's glass is drawn with the wet module's
//! `car_colour` (the film's drops and streaks on the outer pane are part of
//! the glass's own colour, at full strength whatever the pane's alpha, as
//! car_wet.wgsl has it), then the glass composite reads what is behind
//! through it and adds the pane's reflection: film first, glass second.
//! Screen-
//! Space Reflections' car mask still takes the glass as before (its own
//! pass). Off: nothing here runs and the car pass draws the glass natively.

use glam::{Mat4, Vec3};

use crate::formats::texture::Blend;
use crate::render::lighting::CarLighting;
use crate::render::native::car::*;
use crate::render::{DEPTH_FORMAT, Gpu};

// Refactor: from `#[path]` child of car.rs to a module.
mod geom;
pub use self::geom::*;

pub(crate) struct GlassShared {
    /// car.wgsl (or Enhanced Paint's / Wet Car's form of it) + car_glass.wgsl,
    /// by Enhanced Paint | Wet Car << 1, built on first use.
    /// × 2 by the temporal pass's LOD bias (`temporal::biased_source`:
    /// TAA / TAAU / FSR 2 set `MIP_BIAS`, which the plain module lacks).
    modules: [std::sync::OnceLock<wgpu::ShaderModule>; 8],
    copy_module: wgpu::ShaderModule,
    scene_layout: wgpu::BindGroupLayout,
    copy_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    copy_pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    /// The device's [`crate::render::Gpu::build_tag`].
    tag: u64,
    /// By (format, samples, reversed, paint, wet): window, lamp cover.
    pipelines: std::sync::Mutex<
        Vec<((wgpu::TextureFormat, u32, bool, bool, bool), std::sync::Arc<[wgpu::RenderPipeline; 2]>)>,
    >,
    copy_pipelines: std::sync::Mutex<Vec<(wgpu::TextureFormat, std::sync::Arc<wgpu::RenderPipeline>)>>,
    /// The scene's copy: (size, format, the target, group 3 over it). For memory:
    /// the pool's [`crate::render::pool::Slot::SceneCopy`] (written in full
    /// below before the glass reads it).
    copy: std::sync::Mutex<
        Option<((u32, u32), wgpu::TextureFormat, std::sync::Arc<crate::render::pool::Target>, wgpu::BindGroup)>,
    >,
    /// The modules in their immediate form, the draw's entry in the
    /// immediates (`CarShared::immediates`).
    immediates: bool,
}

const COPY_WGSL: &str = "
@group(0) @binding(0) var source: texture_2d<f32>;
@vertex
fn vs_copy(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs_copy(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(frag.xy), 0);
}
";

#[cfg(test)]
pub(crate) use crate::render::native::car::shader_variants::glass_source;

impl GlassShared {
    fn new(device: &wgpu::Device, shared: &CarShared) -> GlassShared {
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let scene_layout = crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
            label: Some("car glass scene"),
            entries: &[
                texture(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let copy_layout = crate::render::core::device_cache::uncached_bind_group_layout(device, &wgpu::BindGroupLayoutDescriptor {
            label: Some("car glass copy"),
            entries: &[texture(0)],
        });
        let layout = crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
            label: Some("car glass"),
            bind_group_layouts: &[
                Some(&shared.frame_layout),
                Some(&shared.draw_layout),
                Some(&shared.texture_layout),
                Some(&scene_layout),
            ],
            immediate_size: crate::render::native::car::draw_immediate_size(shared.immediates),
        });
        let copy_pipeline_layout = crate::render::core::device_cache::uncached_pipeline_layout(device, &wgpu::PipelineLayoutDescriptor {
            label: Some("car glass copy"),
            bind_group_layouts: &[Some(&copy_layout)],
            immediate_size: 0,
        });
        GlassShared {
            modules: Default::default(),
            copy_module: crate::render::core::device_cache::uncached_module(device, shared.tag, wgpu::ShaderModuleDescriptor {
                label: Some("car glass copy"),
                source: wgpu::ShaderSource::Wgsl(COPY_WGSL.into()),
            }),
            scene_layout,
            copy_layout,
            layout,
            copy_pipeline_layout,
            tag: shared.tag,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("car glass"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            pipelines: Default::default(),
            copy_pipelines: Default::default(),
            copy: Default::default(),
            immediates: shared.immediates,
        }
    }

    fn pipelines(
        &self,
        device: &wgpu::Device,
        cache: Option<&wgpu::PipelineCache>,
        format: wgpu::TextureFormat,
        mode: crate::render::SceneMode,
        paint: bool,
        wet: bool,
    ) -> std::sync::Arc<[wgpu::RenderPipeline; 2]> {
        let key = (format, mode.samples, mode.reversed, paint, wet);
        let mut all = self.pipelines.lock().unwrap();
        if let Some((_, p)) = all.iter().find(|(k, _)| *k == key) {
            return p.clone();
        }
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let biased = mode.bias_q != 0;
        let module =
            self.modules[usize::from(paint) | usize::from(wet) << 1 | usize::from(biased) << 2].get_or_init(|| {
                let source = crate::render::native::car::shader_variants::built(
                    crate::render::native::car::shader_variants::glass_module_source_for(
                        paint,
                        wet,
                        biased,
                        self.immediates,
                    ),
                );
                crate::render::core::device_cache::uncached_module(device, self.tag, wgpu::ShaderModuleDescriptor {
                    label: Some("car (enhanced glass)"),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                })
            });
        let make = |kind: f64| {
            // matrix, docs/settings-model.md: the glass module is not
            // the mip-biased one (temporal::biased_source), so the scene's
            // `MIP_BIAS` (TAA Upscaling / FSR 2 below the screen's size) is
            // not its constant; passing it failed pipeline creation (a
            // validation panic with Enhanced Glass under those tiers).
            let mut constants = mode.fragment_constants();
            constants.retain(|c| c.0 != "MIP_BIAS");
            constants.push(("GLASS_KIND", kind));
            crate::render::core::device_cache::uncached_render_pipeline(device, self.tag, &wgpu::RenderPipelineDescriptor {
                label: Some("car glass"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CarVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                // As car.efx's blended draws: CULLMODE CW, ZWRITE off.
                primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(mode.compare()),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: mode.samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_glass"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    },
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::COLOR,
                    })],
                }),
                multiview_mask: None,
                cache,
            })
        };
        let p = std::sync::Arc::new([make(1.0), make(2.0)]);
        all.push((key, p.clone()));
        p
    }

    /// Copy (or resolve) `color` into the scene's copy; its group 3.
    /// The single-sampled scene copy's pipeline into `format`, made
    /// where missing.
    fn copy_pipeline(
        &self,
        device: &wgpu::Device,
        cache: Option<&wgpu::PipelineCache>,
        format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let mut all = self.copy_pipelines.lock().unwrap();
        match all.iter().find(|(f, _)| *f == format) {
            Some((_, p)) => p.clone(),
            None => {
                let p = std::sync::Arc::new(crate::render::core::device_cache::uncached_render_pipeline(device, self.tag, &wgpu::RenderPipelineDescriptor {
                    label: Some("car glass copy"),
                    layout: Some(&self.copy_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &self.copy_module,
                        entry_point: Some("vs_copy"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.copy_module,
                        entry_point: Some("fs_copy"),
                        compilation_options: Default::default(),
                        targets: &[Some(format.into())],
                    }),
                    multiview_mask: None,
                    cache,
                }));
                all.push((format, p.clone()));
                p
            }
        }
    }

    /// `rect` the pixels the glass can read ([`copy_rect`]; a single-sampled scene is copied
    /// there only, the rest of the copy is never read).
    fn copy_scene(
        &self,
        gpu: &Gpu,
        cache: Option<&wgpu::PipelineCache>,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        rect: [u32; 4],
    ) -> wgpu::BindGroup {
        let device = &gpu.device;
        let t = color.texture();
        let (size, format, samples) = ((t.width(), t.height()), t.format(), t.sample_count());
        let mut copy = self.copy.lock().unwrap();
        if copy.as_ref().is_none_or(|c| c.0 != size || c.1 != format) {
            // Drop the old one first (a pooled target lives while held).
            *copy = None;
            let view = gpu.pool.get(
                device,
                crate::render::pool::Slot::SceneCopy,
                "car glass scene copy",
                size,
                format,
                1,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("car glass scene"),
                layout: &self.scene_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            *copy = Some((size, format, view, group));
        }
        let (_, _, view, group) = copy.as_ref().unwrap();
        let view: &wgpu::TextureView = view;
        let keep = wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store };
        if samples > 1 {
            // A pass with no draw resolves the samples into the copy.
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("car glass scene resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: Some(view),
                    ops: keep,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        } else {
            let pipeline = self.copy_pipeline(device, cache, format);
            let source = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("car glass copy"),
                layout: &self.copy_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color) }],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("car glass scene copy"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &source, &[]);
            pass.set_scissor_rect(rect[0], rect[1], rect[2], rect[3]);
            pass.draw(0..3, 0..1);
        }
        group.clone()
    }
}

impl CarRenderer {
    /// Whether this car's glass leaves the car pass this frame (Enhanced
    /// Glass on, in game, the car has glass), and its eye in car space.
    pub(crate) fn split_glass(
        &self,
        lighting: &CarLighting,
        world: Mat4,
        view: Mat4,
        view_proj: Mat4,
        opts: &crate::render::RenderOptions,
    ) {
        let on = opts.enhanced_glass && !lighting.is_frontend() && self.draws.iter().any(|d| d.glass != 0);
        self.glass_split.set(on);
        if on {
            self.glass_eye.set(world.inverse().transform_point3(view.inverse().w_axis.truncate()));
            // (0.3 m about the panes: the parts' poses move them a little.)
            self.glass_rect.set(glass_rect(self.glass_box, 0.3, view_proj * world));
        }
    }

    /// The glass draws at this LOD, far to near from the eye.
    fn record_glass_draws(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipelines: &[wgpu::RenderPipeline; 2],
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
    ) {
        let Some(lod) = self.lod else { return };
        let eye = self.glass_eye.get();
        let mut order: Vec<(f32, usize)> = self
            .draws
            .iter()
            .enumerate()
            .filter(|(_, d)| d.glass != 0 && !d.glass_after && self.draw_visible(d, lod, frontend))
            .map(|(i, d)| (d.centre.distance_squared(eye), i))
            .collect();
        if order.is_empty() {
            return;
        }
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        pass.set_bind_group(0, frame, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(1, draws, &[]);
        let (mut current, mut texture) = (0u8, usize::MAX);
        for (_, i) in order {
            let d = &self.draws[i];
            if current != d.glass {
                pass.set_pipeline(&pipelines[usize::from(d.glass - 1)]);
                current = d.glass;
            }
            if texture != d.texture {
                pass.set_bind_group(2, self.texture_group(d.texture), &[]);
                texture = d.texture;
            }
            crate::render::native::car::draw_entry(pass, self.immediates(), d, base + i as u32);
        }
    }

    /// The draws after the glass ([`mark_after_glass`]) in their native
    /// order, with the pipelines the car pass drew with.
    fn record_after_glass(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
    ) {
        let Some(lod) = self.lod else { return };
        let pipelines = match (self.glass_with.get(), &self.sampled, &self.fade_pipelines) {
            (1, Some((_, p)), _) | (2, _, Some((_, p))) => &**p,
            _ => &self.pipelines,
        };
        let after = self.draws.iter().enumerate().filter(|(_, d)| d.glass_after);
        if after.clone().next().is_none() {
            return;
        }
        // Not split while recording them (record_lod_draws skips split draws).
        let split = self.glass_split.replace(false);
        self.record_lod_draws(pipelines, pass, frontend, frame, draws, base, lod, after);
        self.glass_split.set(split);
    }
}

/// The pipelines [`record_glass`] uses for a scene of `format` × `samples`
/// with `opts` (both wet states with Wet Car on), made now: a drive's
/// warm-up (docs/first-use-builds.md). Nothing is drawn.
pub(crate) fn warm(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, opts: &crate::render::RenderOptions) {
    let shared = gpu.car_shared();
    let device = &gpu.device;
    let glass = shared.glass.get_or_init(|| GlassShared::new(device, &shared));
    let mode = crate::render::SceneMode::current(opts, samples);
    for wet in [false, true] {
        if wet && !opts.wet_car {
            continue;
        }
        glass.pipelines(device, shared.cache.as_ref(), format, mode, opts.enhanced_paint, wet);
    }
    if samples == 1 {
        glass.copy_pipeline(device, shared.cache.as_ref(), format);
    }
}

/// The glass pass over `color` for `cars` (renderer, frame group, draw
/// group, first draw entry, front end): the scene copied, then each car's
/// glass. Nothing when none of them split its glass off.
pub(crate) fn record_glass(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    cars: &[(&CarRenderer, &wgpu::BindGroup, &wgpu::BindGroup, u32, bool)],
    opts: &crate::render::RenderOptions,
) {
    if cars.is_empty() {
        return;
    }
    let shared = gpu.car_shared();
    let device = &gpu.device;
    let glass = shared.glass.get_or_init(|| GlassShared::new(device, &shared));
    let samples = color.texture().sample_count();
    let mode = crate::render::SceneMode::current(opts, samples);
    let format = color.texture().format();
    let pipelines: Vec<_> = cars
        .iter()
        .map(|(car, ..)| {
            glass.pipelines(
                device,
                shared.cache.as_ref(),
                format,
                mode,
                opts.enhanced_paint,
                car.wet_now.is_some(),
            )
        })
        .collect();
    let t = color.texture();
    // Every car's glass off screen: no copy and no glass (the draws
    // after it still go: a decal or lamp may be in view).
    let rect =
        copy_rect(&cars.iter().map(|(car, ..)| car.glass_rect.get()).collect::<Vec<_>>(), (t.width(), t.height()));
    let scene = rect.map(|rect| glass.copy_scene(gpu, shared.cache.as_ref(), encoder, color, rect));
    let mut pass = begin_pass(encoder, color, depth);
    for ((car, frame, draws, base, frontend), pipelines) in cars.iter().zip(&pipelines) {
        if let Some(scene) = &scene {
            // (Again per car: the draws after the glass switch to the car's layout.)
            pass.set_bind_group(3, scene, &[]);
            car.record_glass_draws(&mut pass, pipelines, *frontend, frame, draws, *base);
        }
        car.record_after_glass(&mut pass, *frontend, frame, draws, *base);
    }
    drop(pass);
    crate::prof::gpu::mark(encoder, "cars: enhanced glass");
}

/// The batched cars' glass ([`CarRenderer::record_queued`]'s), after their pass.
pub fn record_glass_batch(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    batch: &CarBatch,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    cars: &[&CarRenderer],
    opts: &crate::render::RenderOptions,
) {
    let list: Vec<_> = cars
        .iter()
        .filter(|c| c.glass_split.replace(false))
        .filter_map(|c| match c.glass_queued.take() {
            Some((frontend, Some(base))) => {
                Some((*c, c.batch_frame_group.as_ref()?, batch.draw_group.as_ref()?, base, frontend))
            }
            Some((frontend, None)) => Some((*c, &c.frame_group, &c.draw_group, 0, frontend)),
            None => None,
        })
        .collect();
    // Enhanced Glass off, its scene copy goes (a pooled target).
    if !opts.enhanced_glass
        && let Some(glass) = gpu.car_shared().glass.get()
    {
        *glass.copy.lock().unwrap() = None;
    }
    record_glass(gpu, encoder, color, depth, &list, opts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_kinds_by_material() {
        assert_eq!(kind("WINDSHIELD"), 1);
        assert_eq!(kind("WINDSHIELD_TINT_L3_AQUA"), 1);
        assert_eq!(kind("HEADLIGHTGLASS"), 2);
        assert_eq!(kind("CLEARPLASTIC"), 2);
        assert_eq!(kind("METPAINT1"), 0);
        assert_eq!(kind("MOLDINGS"), 0);
    }

    /// The blended draws car.efx draws over the glass go after the
    /// glass composite (the Lancer's order: the body's own decal before the
    /// glass stays, the rear-window decal part after it moves, a second
    /// layer of a pane moves, another pane and the opaque draws stay).
    #[test]
    fn draws_over_the_glass_follow_it() {
        let d = |blend, part, glass, x: f32| Draw {
            blend,
            alpha_test: false,
            texture: 0,
            cover_bucket: (false, 0),
            submission: 0,
            first_index: 0,
            index_count: 0,
            base_vertex: 0,
            part,
            constants: Default::default(),
            paint: [0.0; 4],
            wet_class: crate::render::wet_car::WetClass::None,
            glass,
            centre: Vec3::new(x, 0.0, 0.95),
            glass_after: false,
        };
        let mut draws = vec![
            d(Blend::None, 0, 0, 0.0),
            d(Blend::Alpha, 0, 0, -1.17),   // BASE_A / DECAL (drawn before the glass)
            d(Blend::Alpha, 0, 1, -1.315),  // rear WINDSHIELD_TINT
            d(Blend::Alpha, 0, 1, -1.304),  // rear WINDOWMASK: the same pane
            d(Blend::Alpha, 0, 1, 0.715),   // front WINDSHIELD_TINT: another pane
            d(Blend::Alpha, 26, 0, -1.14),  // DECAL_REAR_WINDOW
            d(Blend::Additive, 10, 0, 2.0), // HEADLIGHTGLASS (additive)
        ];
        mark_after_glass(&mut draws);
        assert_eq!(
            draws.iter().map(|d| d.glass_after).collect::<Vec<_>>(),
            [false, false, false, true, false, true, true]
        );
        // A car without glass: nothing moves.
        let mut plain = vec![d(Blend::None, 0, 0, 0.0), d(Blend::Alpha, 1, 0, 0.0)];
        mark_after_glass(&mut plain);
        assert!(plain.iter().all(|d| !d.glass_after));
    }

    /// The copy's rect covers the glass's screen box and the
    /// refraction's reach, the whole target when a car needs it, nothing
    /// when all the glass is off screen.
    #[test]
    fn the_copy_covers_what_the_glass_reads() {
        let size = (1000, 500);
        assert_eq!(copy_rect(&[Some([-0.1, -0.2, 0.1, 0.2])], size), Some([399, 174, 202, 152]));
        assert_eq!(copy_rect(&[Some([-0.1, -0.2, 0.1, 0.2]), None], size), Some([0, 0, 1000, 500]));
        assert_eq!(copy_rect(&[Some([1.5, 1.5, 2.0, 2.0])], size), None);
        assert_eq!(copy_rect(&[], size), None);
        // The box in front of the eye projects to a rect; behind it: the whole view.
        let clip = Mat4::perspective_rh(1.0, 2.0, 0.1, 100.0) * Mat4::look_to_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let r = glass_rect((Vec3::new(-0.5, -0.5, -10.5), Vec3::new(0.5, 0.5, -9.5)), 0.0, clip).unwrap();
        assert!(r[0] < 0.0 && r[2] > 0.0 && r[2] - r[0] < 0.2, "{r:?}");
        assert_eq!(glass_rect((Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)), 0.0, clip), None);
        assert_eq!(glass_rect((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), 0.0, clip), None);
    }

    /// The glass module only appends to car.wgsl, and keeps the native
    /// colour and alpha as its base.
    #[test]
    fn glass_module_appends_to_the_native_car_shader() {
        let src = glass_source(false, false).unwrap();
        assert!(src.starts_with(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl"))));
        // Wet Car's module under the glass: film first, then the glass.
        let wet = glass_source(false, true).unwrap();
        assert!(
            wet.starts_with(&wet_source(false).unwrap())
                && wet.contains("fn wet_layer(")
                && wet.ends_with(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_glass.wgsl")))
        );
        assert!(!include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car.wgsl")).contains("fs_glass"));
        assert!(src.contains("let c = car_colour(in);") && src.contains("c.rgb * a + behind * (1.0 - a)"));
    }
}
