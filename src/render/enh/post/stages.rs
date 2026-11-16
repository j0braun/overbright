//! The Graphics page's passes before the native post passes (moved from render/postfx.rs).

use crate::render::Gpu;
use crate::render::native::post::*;

impl PostFx {
    /// The Graphics page's ambient occlusion for the next [`PostFx::apply`]
    /// (none: off, native).
    pub fn set_ssao(&mut self, frame: Option<crate::render::ssao::Frame>) {
        self.ssao_frame = frame;
    }

    /// The Graphics page's ambient occlusion now, into the scene as drawn
    /// so far (`scene`: the multisampled scene with `samples` > 1, else the
    /// scene texture; `size` its size; `texel_scale` the scene's height
    /// over the screen's). docs/glow-darkening.md: the game runs it
    /// on the lit scene before the additive layers (shadow texture and
    /// underglow, particles, glow sprites, horizon fog), so occlusion never
    /// multiplies emitted light; [`PostFx::set_ssao`] then stays None.
    #[allow(clippy::too_many_arguments)]
    pub fn ssao_into(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        samples: u32,
        size: (u32, u32),
        texel_scale: f32,
        frame: &crate::render::ssao::Frame,
    ) {
        let format = self.scene_format();
        self.ssao.get_or_insert_with(|| crate::render::ssao::Ssao::new(gpu, format)).run_into(
            gpu,
            encoder,
            scene,
            samples,
            size,
            texel_scale,
            frame,
        );
    }

    /// The Graphics page's screen-space reflections for the next
    /// [`PostFx::apply`] (none: off, native).
    pub fn set_ssr(&mut self, frame: Option<crate::render::ssr::Frame>) {
        self.ssr_frame = frame;
    }

    /// The passes on the scene before the native ones: the resolve of the
    /// multisampled scene, SSAO and SSR (each once: their frames are
    /// taken). [`PostFx::apply`] runs them; the temporal pass runs them
    /// first on the scene it reads ([`PostFx::temporal_pass`]).
    /// `streaks`: the glow sprites go to the streak buffer (SSR then
    /// draws them alone through `glow` for its rays to find).
    pub fn pre_passes(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        streaks: bool,
        glow: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        self.release_off_tiers(opts);
        let output_size = self.output_size;
        // The multisampled scene into the scene texture the passes read.
        self.resolve(encoder);
        // The Graphics page's ambient occlusion (clone deviation) on the
        // scene, before the native passes.
        if let (Some(f), Some(t)) = (self.ssao_frame.take(), self.targets.as_ref()) {
            let (scene, size, format) = (t.views.scene.clone(), t.size, self.scene_format());
            let texel_scale = output_size.map_or(1.0, |o| size.1 as f32 / o.1.max(1) as f32);
            self.ssao.get_or_insert_with(|| crate::render::ssao::Ssao::new(gpu, format)).run(
                gpu,
                encoder,
                &scene,
                size,
                texel_scale,
                &f,
            );
        }
        // Then the screen-space reflections (clone deviation), on the
        // occluded scene, before the native passes.
        if let (Some(f), Some(t)) = (self.ssr_frame.take(), self.targets.as_ref()) {
            let (scene, size, format) = (t.views.scene.clone(), t.size, self.scene_format());
            // The glow sprites go to the streak buffer (added after these
            // passes): drawn once more, alone, for the rays to find
            // (through the multisampled streak copy, which the streak pass
            // refills from the buffer before it draws).
            let glow_view = if streaks {
                let inter = self.inter();
                if self.ssr_glow.as_ref().is_none_or(|g| g.0 != size || g.1 != inter) {
                    // The pool's SceneAux (cleared before each use);
                    // the old one dropped first.
                    self.ssr_glow = None;
                    self.ssr_glow = Some((
                        size,
                        inter,
                        gpu.pool.get(
                            &gpu.device,
                            crate::render::pool::Slot::SceneAux,
                            "ssr glows",
                            size,
                            inter,
                            1,
                            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        ),
                    ));
                }
                let target = self.ssr_glow.as_ref().unwrap().2.view.clone();
                match self.msaa.as_ref().filter(|_| self.samples > 1) {
                    Some(m) => {
                        clear(encoder, &m.streak, wgpu::Color::TRANSPARENT);
                        glow(encoder, &m.streak, Some(&target));
                    }
                    None => {
                        clear(encoder, &target, wgpu::Color::TRANSPARENT);
                        glow(encoder, &target, None);
                    }
                }
                Some(target)
            } else {
                None
            };
            let out = output_size.unwrap_or(size);
            self.ssr.get_or_insert_with(|| crate::render::ssr::Ssr::new(gpu, format)).run(
                gpu,
                encoder,
                &scene,
                size,
                out,
                format,
                &f,
                glow_view.as_ref(),
            );
        }
        // One volume: with the medium running (any Volumetric Fog tier), the Next-Gen car lights' tail / brake
        // lamps feed it (their cones are its spots already, `atmosphere::NX6_SPOT_SCALE`) and their own march
        // stays off.
        let car_frame = self.car_lights_frame.take();
        let mut volumetric_frame = self.volumetric_frame.take();
        let medium_air = volumetric_frame.is_some() && self.targets.is_some();
        if let (Some(f), Some(c)) = (volumetric_frame.as_mut(), car_frame.as_ref()) {
            let n = f.next.tier.car_lamps;
            f.next.lights.extend(crate::render::atmosphere::car_lamps(&crate::render::car_lights::medium_lamps(c), n));
        }
        // Then Volumetric Fog (clone deviation): the medium at the row's tier.
        if let (Some(f), Some(t)) = (volumetric_frame, self.targets.as_ref()) {
            let (scene, size, format) = (t.views.scene.clone(), t.size, self.scene_format());
            let tint = self.fog_tint.take();
            self.tint_folded = self.atmosphere.get_or_insert_with(|| crate::render::atmosphere::Atmosphere::new(gpu)).run(
                gpu, encoder, &scene, format, size, &f.depth, f.samples, &f.camera, f.aspect, f.reversed, &f.next, tint,
            );
        }
        // Then the Next-Gen car lights in the air and the lens streaks.
        if let (Some(f), Some(t)) = (car_frame, self.targets.as_ref()) {
            let (scene, size, format) = (t.views.scene.clone(), t.size, self.scene_format());
            // matrix: High Precision Post switched live changes the
            // scene's format; the pass's pipelines follow it.
            if self.car_lights.as_ref().is_some_and(|c| c.format != format) {
                self.car_lights = None;
            }
            self.car_lights
                .get_or_insert_with(|| crate::render::car_lights::CarLightsPass::new(gpu, format))
                .run(gpu, encoder, &scene, size, &f, !medium_air);
        }
    }

    /// This frame's Next-Gen car lights (None: off, native).
    pub fn set_car_lights(&mut self, frame: Option<crate::render::car_lights::Frame>) {
        self.car_lights_frame = frame;
    }

    /// The targets of the Graphics page's passes whose row is off
    /// dropped (made again when switched back on; what they draw is the
    /// same, they keep nothing a later frame reads but the motion
    /// history, which an off motion blur does not use).
    fn release_off_tiers(&mut self, opts: &crate::render::RenderOptions) {
        if opts.ssr == 0 {
            self.ssr = None;
            self.ssr_glow = None;
        }
        if opts.ssao <= 0.0 {
            self.ssao = None;
        }
        if opts.volumetric_fog == 0 {
            self.atmosphere = None;
        }
        if opts.motion_blur == 0
            && !crate::render::blur_dof::dof_on(opts)
            && crate::render::blur_dof::photo_dof().is_none()
        {
            self.blur_dof = None;
        }
    }

    /// This frame's Volumetric Fog (None: off, native).
    pub fn set_volumetric(&mut self, frame: Option<crate::render::volumetric::Frame>) {
        self.volumetric_frame = frame;
    }

    /// Where the scene is drawn this frame: with Upscaling
    /// ([`PostFx::set_upscale`]) the stage at the lower size, else this.
    /// Motion blur, horizon fog, SSAO and SSR go to it.
    pub fn stage(&mut self) -> &mut PostFx {
        if self.stage.is_some() {
            return self.stage.as_deref_mut().unwrap();
        }
        self
    }

    /// Upscaling: the scene drawn `Some(size)` big into a stage of its
    /// own, this one's targets at `output` (the screen's); None drops the
    /// stage. Returns the stage's scene view.
    /// `samples`: the stage's MSAA (1 under the temporal pass; FSR keeps
    /// the native MSAA at the lower size).
    pub fn set_upscale(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        size: Option<(u32, u32)>,
        output: (u32, u32),
        samples: u32,
    ) -> Option<wgpu::TextureView> {
        let Some(size) = size else {
            self.stage = None;
            return None;
        };
        // the combination matrix, docs/settings-model.md: switched on
        // live, this chain still held the unstaged frames' multisampled
        // scene (and `samples` > 1); the stage draws the scene now, so drop
        // it as a start with Upscaling never had it (switching Off → TAA
        // Upscaling / FSR 2 failed validation: "attachments have differing
        // sample counts", the render workers panicked).
        if self.msaa.is_some() || self.samples > 1 {
            self.multisampled(gpu, output, 1, opts);
        }
        self.resize(gpu, output, opts);
        let format = self.format;
        let stage = self.stage.get_or_insert_with(|| Box::new(PostFx::new(gpu, format)));
        stage.multisampled(gpu, size, samples, opts);
        stage.set_output_size(output);
        Some(stage.scene_view(gpu, size, opts))
    }

    /// The temporal pass: the stage's passes before the native ones
    /// (SSAO, SSR), then the scene (`depth`, its depth) resolved with the
    /// history into this one's scene; with Upscaling `out_depth` gets the
    /// depth at the screen's size for the late sprites.
    /// `streaks` / `glow`: as [`PostFx::pre_passes`] (the glows for SSR,
    /// drawn into the jittered scene's frame).
    #[allow(clippy::too_many_arguments)]
    pub fn temporal_pass(
        &mut self,
        gpu: &Gpu,
        opts: &crate::render::RenderOptions,
        encoder: &mut wgpu::CommandEncoder,
        frame: &crate::render::temporal::FrameInput,
        depth: &wgpu::TextureView,
        out_depth: Option<&wgpu::TextureView>,
        streaks: bool,
        glow: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, Option<&wgpu::TextureView>),
    ) {
        // docs/fsr2.md: FSR 2's composition mask is what SSR adds
        // to the scene: the pre-passes run in three steps (resolve + SSAO,
        // SSR, Volumetric Fog; each takes its frame), the scene kept
        // before SSR and compared after it. Same passes, same order.
        if opts.fsr2
            && let Some(display) = self.targets.as_ref().map(|t| t.size)
        {
            let s = self.stage();
            let (ssr, volumetric) = (s.ssr_frame.take(), s.volumetric_frame.take());
            s.pre_passes(gpu, opts, encoder, streaks, &mut *glow);
            if let (Some(f), Some((scene, size))) = (ssr, s.targets.as_ref().map(|t| (t.views.scene.clone(), t.size))) {
                let temporal = self.temporal.get_or_insert_with(|| crate::render::temporal::Temporal::new(gpu));
                temporal.fsr2_snapshot(gpu, encoder, &scene, size, display);
                let s = self.stage();
                s.ssr_frame = Some(f);
                s.pre_passes(gpu, opts, encoder, streaks, &mut *glow);
                self.temporal.as_mut().unwrap().fsr2_composite(gpu, encoder, &scene, size);
            }
            self.stage().volumetric_frame = volumetric;
        }
        self.stage().pre_passes(gpu, opts, encoder, streaks, glow);
        let (input_view, input_size) = {
            let s = self.stage();
            let t = s.targets.as_ref().expect("scene targets");
            (t.views.scene.clone(), t.size)
        };
        let Some(t) = self.targets.as_ref() else { return };
        let (out, out_size, format) = (t.views.scene.clone(), t.size, self.scene_format());
        let upscaled = self.stage.is_some();
        let temporal = self.temporal.get_or_insert_with(|| crate::render::temporal::Temporal::new(gpu));
        if upscaled {
            temporal.run(gpu, opts, encoder, frame, &input_view, depth, input_size, &out, format, out_size, out_depth);
        } else {
            // The scene is both input and output: the pass writes the
            // history, then copies it over the scene.
            temporal.run(gpu, opts, encoder, frame, &input_view, depth, input_size, &out, format, out_size, None);
        }
    }
}
