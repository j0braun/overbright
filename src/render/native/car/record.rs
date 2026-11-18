//! Recording the cars' draws: passes, SSR mask, LOD draws (moved from car.rs).

use super::*;

impl CarRenderer {
    /// Draw the car at `world` (car space → world) on top of an existing
    /// colour/depth target.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        opts: &crate::render::RenderOptions,
    ) {
        self.render_with(&self.pipelines, gpu, encoder, color, depth, view_proj, view, lighting, world, wheels, opts);
    }

    /// Build the pipelines [`CarRenderer::render_reflection`] needs (once).
    pub fn prepare_reflection(&mut self, gpu: &Gpu) {
        if self.reflection_pipelines.is_none() {
            self.reflection_pipelines = Some(self.shared.pipelines(&gpu.device, crate::render::REFLECTION_FORMAT));
        }
    }

    /// Draw the car into the world renderer's reflection image (the front
    /// end's floor reflection, view 4): as [`CarRenderer::render`], with
    /// `world` already mirrored. The uniforms are shared with `render`, so
    /// the two must go in separate submissions. Needs
    /// [`CarRenderer::prepare_reflection`] first.
    #[allow(clippy::too_many_arguments)]
    pub fn render_reflection(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        opts: &crate::render::RenderOptions,
    ) {
        if let Some(p) = &self.reflection_pipelines {
            self.render_with(p, gpu, encoder, color, depth, view_proj, view, lighting, world, wheels, opts);
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// [`CarRenderer::render`] into a target of `samples` samples (Full
    /// Screen Anti-Aliasing; the front end's scenes), with the scene's
    /// [`crate::render::SceneMode::current`] pipelines (`view_proj` then from
    /// `Camera::view_projection_depth`).
    #[allow(clippy::too_many_arguments)]
    pub fn render_sampled(
        &mut self,
        gpu: &Gpu,
        samples: u32,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        opts: &crate::render::RenderOptions,
    ) {
        self.sync_texture_pack(gpu);
        self.pump_surface(gpu, opts);
        self.frame_wet(lighting, opts);
        // Enhanced Glass: the glass after the car, over its copy.
        self.split_glass(lighting, world, view, view_proj, opts);
        let mode = CarMode::current(samples, self.wet_now.is_some(), opts);
        self.glass_with.set(u8::from(mode != CarMode::NATIVE));
        if mode == CarMode::NATIVE {
            self.render(gpu, encoder, color, depth, view_proj, view, lighting, world, wheels, opts);
        } else {
            self.sync_scene(gpu, mode);
            let pipelines = self.sampled.as_ref().unwrap().1.clone();
            self.render_with(&pipelines, gpu, encoder, color, depth, view_proj, view, lighting, world, wheels, opts);
        }
        if self.glass_split.get() {
            glass::record_glass(
                gpu,
                encoder,
                color,
                depth,
                &[(self, &self.frame_group, &self.draw_group, 0, lighting.is_frontend())],
                opts,
            );
            self.glass_split.set(false);
        }
        // Screen-Space Reflections (clone deviation): the car into the
        // mask too, in game.
        if !lighting.is_frontend()
            && let Some(mask) = self.shared.armed_ssr_mask()
        {
            let pipelines = self.shared.ssr_pipelines(&gpu.device, mask.reversed, opts.surface_detail >= 2);
            let mut pass = begin_ssr_mask_pass(encoder, &mask);
            self.record_ssr_mask(&pipelines, &mut pass, false, &self.frame_group, &self.draw_group, 0);
        }
    }

    /// The car's draws at its LOD into the SSR mask pass `pass` (frame
    /// group `frame`, draw values from entry `base` on).
    #[allow(clippy::too_many_arguments)]
    fn record_ssr_mask(
        &self,
        pipelines: &SsrPipelines,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
    ) {
        let Some(lod) = self.lod else { return };
        pass.set_bind_group(0, frame, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(1, draws, &[]);
        let (mut current, mut texture) = (None, usize::MAX);
        for (i, d) in self.draws.iter().enumerate() {
            let blended = match d.blend {
                Blend::None => false,
                Blend::Alpha => true,
                _ => continue,
            };
            if !self.draw_visible(d, lod, frontend) {
                continue;
            }
            if current != Some((blended, d.alpha_test)) {
                pass.set_pipeline(&pipelines[&(blended, d.alpha_test)]);
                current = Some((blended, d.alpha_test));
            }
            if texture != d.texture {
                pass.set_bind_group(2, self.texture_group(d.texture), &[]);
                texture = d.texture;
            }
            draw_entry(pass, self.shared.immediates, d, base + i as u32);
        }
    }

    /// The scene's pipelines for `mode`, built (from the shared cache)
    /// when it changed.
    pub(super) fn sync_scene(&mut self, gpu: &Gpu, mode: CarMode) {
        if self.sampled.as_ref().is_none_or(|s| s.0 != mode) {
            self.sampled = Some((mode, self.shared.pipelines_n(&gpu.device, self.color_format, mode)));
        }
    }

    /// The pipelines a drive's first frame takes for this car
    /// (and, from the device's shared cache, every other car of the
    /// drive), built now, at the drive's load: the scene's at `samples`
    /// as the drive will run it ([`crate::render::SceneMode::driving`]),
    /// dry and (Wet Car on) wet, and LOD Fade's (on). The same pipelines
    /// as the frame would build; it then finds them in place.
    pub fn warm(&mut self, gpu: &Gpu, samples: u32, opts: &crate::render::RenderOptions) {
        let scene = crate::render::SceneMode::driving(opts, samples);
        let mode = CarMode::of_scene(scene, self.wet_now.is_some(), opts);
        if mode != CarMode::NATIVE {
            self.sync_scene(gpu, mode);
        }
        let mut modes = vec![mode];
        if opts.wet_car {
            modes.push(CarMode::of_scene(scene, self.wet_now.is_none(), opts));
        }
        for &m in &modes {
            if m != CarMode::NATIVE {
                self.shared.pipelines_n(&gpu.device, self.color_format, m);
            }
            if opts.lod_fade {
                self.shared.fade_pipelines_n(&gpu.device, self.color_format, m);
            }
        }
        // The cars in Screen-Space Reflections' mask, and Enhanced Glass.
        if opts.ssr != 0 {
            self.shared.ssr_pipelines(&gpu.device, scene.reversed, opts.surface_detail >= 2);
        }
        if opts.enhanced_glass {
            crate::render::enh::materials::car::glass::warm(gpu, scene.target(self.color_format), samples, opts);
        }
    }

    fn render_with(
        &self,
        pipelines: &HashMap<(Blend, bool), wgpu::RenderPipeline>,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        opts: &crate::render::RenderOptions,
    ) {
        self.write_uniforms(gpu, view_proj, view, lighting, world, wheels, opts);
        let mut pass = begin_pass(encoder, color, depth);
        self.record_with(pipelines, &mut pass, lighting.is_frontend());
    }

    /// Record the draws [`CarRenderer::queue`] queued, if any, into `pass`.
    pub fn record_queued(&self, pass: &mut wgpu::RenderPass<'_>, batch: &CarBatch, opts: &crate::render::RenderOptions) {
        let _t = crate::prof::acc::span("cars: recording");
        let mode = CarMode::current(batch.samples, self.wet_now.is_some(), opts);
        let pipelines = match &self.sampled {
            Some((m, p)) if *m == mode => p,
            _ => &self.pipelines,
        };
        let fade = self.queued_fade.take();
        self.glass_with.set(match (&fade, &self.fade_pipelines, &self.sampled) {
            (Some(_), Some(_), _) => 2,
            (_, _, Some((m, _))) if *m == mode => 1,
            _ => 0,
        });
        self.mask_queued.set(self.queued.get());
        self.glass_queued.set(self.queued.get());
        // The texture editor's list (docs/texture-editor.md; off: not run).
        if self.queued.get().is_some() && crate::tools::texed::recording() {
            crate::tools::texed::note_textures(true, self.pack_textures.iter().flatten());
        }
        match self.queued.take() {
            Some((frontend, Some(base))) => {
                if let (Some(frame), Some(draws)) = (&self.batch_frame_group, &batch.draw_group) {
                    match (fade, &self.fade_pipelines) {
                        (Some((from, old)), Some((_, faded))) => {
                            if let Some(lod) = self.lod {
                                self.record_lod(faded, pass, frontend, frame, draws, base, lod);
                            }
                            if let Some(lod) = from {
                                self.record_lod(faded, pass, frontend, frame, draws, old, lod);
                            }
                        }
                        _ => self.record_groups(pipelines, pass, frontend, frame, draws, base),
                    }
                }
            }
            Some((frontend, None)) => self.record_with(pipelines, pass, frontend),
            None => {}
        }
    }

    /// The car's draws into `pass` with the uniforms last written;
    /// `frontend`: the lighting is the front end's.
    fn record_with(
        &self,
        pipelines: &HashMap<(Blend, bool), wgpu::RenderPipeline>,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
    ) {
        self.record_groups(pipelines, pass, frontend, &self.frame_group, &self.draw_group, 0);
    }

    /// The draws with frame group `frame` and draw values `draws` from
    /// entry `base` on (a draw's instance index is its entry).
    #[allow(clippy::too_many_arguments)]
    fn record_groups(
        &self,
        pipelines: &HashMap<(Blend, bool), wgpu::RenderPipeline>,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
    ) {
        let Some(lod) = self.lod else { return };
        self.record_lod(pipelines, pass, frontend, frame, draws, base, lod);
    }

    /// Whether this car's draws set their entry in the immediates
    /// ([`draw_entry`]).
    pub(crate) fn immediates(&self) -> bool {
        self.shared.immediates
    }

    pub(crate) fn draw_visible(&self, d: &Draw, lod: u8, frontend: bool) -> bool {
        // Parts drawn in one scene only (the trunk audio components,
        // 0x630C34): the front end's lighting marks the front end.
        let skip = if frontend { crate::car::Scene::InGame } else { crate::car::Scene::FrontEnd };
        // A check aid: NFSU2_AUDIO_ONLY draws only the trunk audio and its
        // components (`comp`: the components alone), so they show without
        // the trunk lid over them.
        static AUDIO_ONLY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        let audio_only = AUDIO_ONLY.get_or_init(|| crate::render::debug_env::nfsu2_audio_only());
        let hidden = |a: crate::car::AudioPaint| match audio_only.as_deref() {
            None => false,
            Some("comp") => a != crate::car::AudioPaint::Component,
            Some(_) => a == crate::car::AudioPaint::None,
        };
        let gated = self.part_hood_gate[d.part] && self.poses[0][0].is_none();
        !self.part_lods[d.part].is_some_and(|m| m >> lod & 1 == 0)
            && self.part_scene[d.part] != skip
            && !hidden(self.part_audio[d.part])
            && !gated
    }

    /// [`CarRenderer::record_groups`] at `lod`.
    #[allow(clippy::too_many_arguments)]
    fn record_lod(
        &self,
        pipelines: &HashMap<(Blend, bool), wgpu::RenderPipeline>,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
        lod: u8,
    ) {
        self.record_lod_draws(pipelines, pass, frontend, frame, draws, base, lod, self.draws.iter().enumerate());
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_lod_draws<'a>(
        &'a self,
        pipelines: &HashMap<(Blend, bool), wgpu::RenderPipeline>,
        pass: &mut wgpu::RenderPass<'_>,
        frontend: bool,
        frame: &wgpu::BindGroup,
        draws: &wgpu::BindGroup,
        base: u32,
        lod: u8,
        order: impl Iterator<Item = (usize, &'a Draw)>,
    ) {
        pass.set_bind_group(0, frame, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(1, draws, &[]);
        let (mut current, mut texture) = (None, usize::MAX);
        for (i, d) in order {
            if !self.draw_visible(d, lod, frontend) || ((d.glass != 0 || d.glass_after) && self.glass_split.get()) {
                continue;
            }
            if current != Some((d.blend, d.alpha_test)) {
                pass.set_pipeline(&pipelines[&(d.blend, d.alpha_test)]);
                current = Some((d.blend, d.alpha_test));
            }
            if texture != d.texture {
                pass.set_bind_group(2, self.texture_group(d.texture), &[]);
                texture = d.texture;
            }
            draw_entry(pass, self.shared.immediates, d, base + i as u32);
        }
    }
}

/// Screen-Space Reflections (clone deviation): the cars the last
/// [`CarRenderer::record_queued`] drew, once more into the SSR mask the
/// world view armed (none: nothing recorded). After the car pass.
pub fn record_ssr_masks(gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, batch: &CarBatch, cars: &[&CarRenderer], opts: &crate::render::RenderOptions) {
    let shared = gpu.car_shared();
    let Some(mask) = shared.armed_ssr_mask() else { return };
    let pipelines = shared.ssr_pipelines(&gpu.device, mask.reversed, opts.surface_detail >= 2);
    let mut pass = begin_ssr_mask_pass(encoder, &mask);
    for car in cars {
        match car.mask_queued.take() {
            Some((frontend, Some(base))) => {
                if let (Some(frame), Some(draws)) = (&car.batch_frame_group, &batch.draw_group) {
                    car.record_ssr_mask(&pipelines, &mut pass, frontend, frame, draws, base);
                }
            }
            Some((frontend, None)) => {
                car.record_ssr_mask(&pipelines, &mut pass, frontend, &car.frame_group, &car.draw_group, 0)
            }
            None => {}
        }
    }
}

/// A pass over the SSR mask's targets and depth as they are.
fn begin_ssr_mask_pass<'e>(encoder: &'e mut wgpu::CommandEncoder, mask: &SsrMaskTargets) -> wgpu::RenderPass<'e> {
    let keep = wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store };
    let attach =
        |view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: keep });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("car ssr mask"),
        color_attachments: &[attach(&mask.term), attach(&mask.surface), attach(&mask.tint)],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &mask.depth,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// A pass drawing cars over `color` / `depth` as they are
/// ([`CarRenderer::record_queued`]).
pub fn begin_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("car"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
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
    })
}


/// The cars' programs a drive's frames take that depend on the settings
/// and the window's format alone, made with no car at hand (the menus,
/// the prepare mode): the native set, the scene's at the drive's mode dry
/// and (Wet Car) wet, LOD Fade's, the cars in Screen-Space Reflections'
/// mask, Enhanced Glass and the reflection image's set. They go into the device's shared sets ([`Gpu::car_shared`]), where
/// a car renderer's own warm-up ([`CarRenderer::warm`]) and frames then
/// find them: the same objects from the same descriptors.
pub fn warm_programs(
    gpu: &Gpu,
    color_format: wgpu::TextureFormat,
    samples: u32,
    opts: &crate::render::RenderOptions,
) {
    let shared = gpu.car_shared();
    let device = &gpu.device;
    let scene = crate::render::SceneMode::driving(opts, samples);
    drop(shared.pipelines(device, color_format));
    let mut modes = vec![CarMode::of_scene(scene, false, opts)];
    if opts.wet_car {
        modes.push(CarMode::of_scene(scene, true, opts));
    }
    for &m in &modes {
        if m != CarMode::NATIVE {
            shared.pipelines_n(device, color_format, m);
        }
        if opts.lod_fade {
            shared.fade_pipelines_n(device, color_format, m);
        }
    }
    if opts.ssr != 0 {
        shared.ssr_pipelines(device, scene.reversed, opts.surface_detail >= 2);
    }
    if opts.enhanced_glass {
        crate::render::enh::materials::car::glass::warm(gpu, scene.target(color_format), samples, opts);
    }
    warm_reflection_set(gpu);
}

/// The reflection image's set (`prepare_reflection`): the rear-view
/// mirror's, and the floor reflection of the garage scenes (the menus' and
/// the shops') the FE car draws, made ahead into the device's shared sets.
pub fn warm_reflection_set(gpu: &Gpu) {
    drop(gpu.car_shared().pipelines(&gpu.device, crate::render::REFLECTION_FORMAT));
}

#[cfg(test)]
mod warm_tests {
    use super::*;

    /// After a drive's warm-up ([`WorldRenderer::warm_drive`],
    /// [`CarRenderer::warm`]) the first frame's own calls (the world's
    /// `set_samples`, the car's scene and LOD Fade pipelines, at the frame's
    /// samples with the temporal pass as driving sets it) build no pipeline,
    /// for the Original look and the enhanced presets, at 1 and 8 samples.
    #[test]
    #[ignore = "requires original assets and a GPU; run serially"]
    fn warm_drive_leaves_the_first_frame_no_pipeline_to_build() {
        let _globals = crate::qol::test_globals();
        let root = crate::game_root();
        let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
        let world = crate::world::World::load(&root, "L4RA").unwrap();
        let car = crate::car::Car::load(&root, "RX8").unwrap();
        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        use crate::qol::Preset;
        for preset in [Preset::Original, Preset::Clean, Preset::Modern, Preset::Ultra, Preset::Extreme] {
            let mut e = crate::qol::Enhancements::default();
            preset.apply(&mut e);
            e.apply();
            for samples in [1, 8] {
                let opts = crate::render::RenderOptions::in_force();
                let mut w = crate::render::WorldRenderer::new(&gpu, &world, format, &opts);
                let mut c = CarRenderer::new(&gpu, &car, format, &opts);
                w.warm_drive(&gpu, &world, samples, &opts);
                c.warm(&gpu, samples, &opts);
                let before = crate::render::pipelines_built();
                crate::qol::set_temporal_frame(crate::qol::temporal());
                let opts = crate::render::RenderOptions::in_force();
                w.set_samples(&gpu, samples, &opts);
                let mode = CarMode::current(samples, false, &opts);
                if mode != CarMode::NATIVE {
                    c.sync_scene(&gpu, mode);
                }
                if opts.lod_fade {
                    c.shared.fade_pipelines_n(&gpu.device, c.color_format, mode);
                }
                crate::qol::set_temporal_frame(false);
                assert_eq!(crate::render::pipelines_built(), before, "{preset:?} at {samples} samples");
            }
        }
        crate::qol::Enhancements::default().apply();
    }
}
