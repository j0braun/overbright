//! The post passes' targets, MSAA scene and pipeline cache (moved from render/postfx.rs).

use super::*;

// The textures are kept beside their views (read through `views`).
#[allow(dead_code)]
pub(crate) struct Targets {
    pub(crate) size: (u32, u32),
    /// High Precision Post's glow size (None: off, the native 8-bit
    /// targets; 1 half the screen's; 2 the screen's), as made.
    pub(super) hdr: Option<u8>,
    /// The glow targets' (A–C) size.
    glow: (u32, u32),
    scene: wgpu::Texture,
    full: [wgpu::Texture; 2],
    half: [wgpu::Texture; 3],
    pub(super) streak: wgpu::Texture,
    /// The image resolved to the screen's size (SSAA with High Precision
    /// Post, [`resolves_first`]), made on first use: its size and view.
    pub(super) resolved: Option<((u32, u32), wgpu::TextureView)>,
    /// Their views, made once (scene, full 0 and 1, half A–C, streaks).
    pub(crate) views: TargetViews,
    pub(super) groups: Vec<wgpu::BindGroup>,
    /// The streak buffer has been cleared.
    pub(super) fresh: bool,
}

pub(crate) struct TargetViews {
    pub(crate) scene: wgpu::TextureView,
    pub(super) full: [wgpu::TextureView; 2],
    pub(super) half: [wgpu::TextureView; 3],
    pub(super) streak: wgpu::TextureView,
}

/// The multisampled scene, depth and streak buffer
/// ([`PostFx::multisampled`]).
pub(crate) struct Msaa {
    samples: u32,
    size: (u32, u32),
    pub(super) color: wgpu::TextureView,
    depth: wgpu::TextureView,
    pub(crate) streak: wgpu::TextureView,
}

impl PostFx {
    /// Every pipeline the passes use, built now rather than at first use:
    /// the motion blur's first appears mid-drive, when the car first goes
    /// fast enough, and a compile there is a hitch.
    pub(super) fn warm(&mut self, gpu: &Gpu) {
        let out = self.format;
        let used: [(&'static str, wgpu::TextureFormat, Blend); 14] = [
            ("fs_motion", INTERMEDIATE, Blend::Replace),
            ("fs_filter", out, Blend::LerpConstant),
            ("fs_zero", INTERMEDIATE, Blend::ScaleDest),
            ("fs_add", INTERMEDIATE, Blend::Replace),
            ("fs_bleach0", INTERMEDIATE, Blend::Replace),
            ("fs_bleach1", INTERMEDIATE, Blend::Replace),
            ("fs_bleach2", INTERMEDIATE, Blend::Replace),
            ("fs_filter", INTERMEDIATE, Blend::Replace),
            ("fs_overbright", INTERMEDIATE, Blend::Replace),
            ("fs_filter", INTERMEDIATE, Blend::SubtractConstant),
            ("fs_filter", INTERMEDIATE, Blend::AddConstant),
            ("fs_add", out, Blend::Replace),
            ("fs_add_a0", out, Blend::Replace),
            ("fs_filter_a0", out, Blend::Replace),
        ];
        for (entry, format, blend) in used {
            self.pipeline(gpu, entry, format, blend);
        }
        self.pipeline(gpu, "fs_filter", out, Blend::Replace);
        self.fog_pipeline(gpu);
    }

    /// The anti-aliased scene (Full Screen Anti-Aliasing, 0x870D08: the
    /// back buffer made with MultiSampleType 1 and the option as its
    /// quality, 0x5B9B3B): the scene is drawn into a multisampled colour
    /// and depth target of `samples` (1: none, the scene texture itself)
    /// and resolved into the scene before the screen passes, as the game's
    /// StretchRect copies of the back buffer resolve it. The glow sprites
    /// go through a multisampled copy of the streak buffer as the game's
    /// do through 0x86F2E0 (0x5BBA84, 0x5CBBB2). Returns the colour and
    /// depth views to draw the scene with when multisampled.
    pub fn multisampled(
        &mut self,
        gpu: &Gpu,
        size: (u32, u32),
        samples: u32,
        opts: &crate::render::RenderOptions,
    ) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
        // Drop obsolete attachments before allocating their replacements.
        // Extreme SSAA/MSAA can make one bundle several gigabytes. SSAO
        // bind groups also retain the previous multisampled depth view.
        if self.msaa.as_ref().is_some_and(|m| samples <= 1 || m.samples != samples || m.size != size) {
            self.msaa = None;
            self.ssao = None;
            self.ssao_frame = None;
            self.ssr = None;
            self.ssr_frame = None;
            self.volumetric_frame = None;
            let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        }
        self.resize(gpu, size, opts);
        self.samples = samples;
        if samples <= 1 {
            return None;
        }
        // Made from the sample count and the size alone: the scene is
        // readable for every Particles tier (the volumetric smoke's scene
        // ambient, Animated Water), so a tier change keeps the targets.
        if self.msaa.as_ref().is_none_or(|m| m.samples != samples || m.size != size) {
            let target = |label, format| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        // Readable for Animated Water's Realistic copy of
                        // the scene (render/water_hq.rs; its first
                        // sample) and the Cinematic smoke's scene ambient.
                        // Nothing drawn changes.
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let depth = gpu
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("depth (multisampled)"),
                    size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::render::DEPTH_FORMAT,
                    // Readable for the Graphics page's ambient occlusion.
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            // With High Precision Post the multisampled streak copy
            // has the scene's format, and it is the same texture: the scene
            // is resolved (and nothing draws into it again this frame)
            // before the streak copy is first written (`pre_passes`' SSR
            // glows, `apply`'s glows: each rewrites every sample first),
            // and each frame's scene starts with a clear. Without it (the
            // Original look) the formats differ: two textures, as before.
            let color = target("scene (multisampled)", self.scene_format());
            let streak = if self.inter() == self.scene_format() && crate::render::pool::enabled() {
                color.clone()
            } else {
                target("streaks (multisampled)", self.inter())
            };
            self.msaa = Some(Msaa { samples, size, color, depth, streak });
            // The motion blur's lerp over the multisampled scene built
            // now, with the targets (it was the one pipeline an Original
            // drive built mid-race, the first time the car went fast).
            self.pipeline_ms(gpu, "fs_filter", self.scene_format(), Blend::LerpConstant, samples);
        }
        let m = self.msaa.as_ref().unwrap();
        Some((m.color.clone(), m.depth.clone()))
    }

    /// The multisampled scene resolved into the scene texture.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(m), Some(t)) = (self.msaa.as_ref().filter(|_| self.samples > 1), self.targets.as_ref()) else {
            return;
        };
        let scene = &t.views.scene;
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("resolve the scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &m.color,
                depth_slice: None,
                resolve_target: Some(scene),
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    pub(crate) fn resize(&mut self, gpu: &Gpu, size: (u32, u32), opts: &crate::render::RenderOptions) {
        // High Precision Post (clone deviation): the targets in half float.
        let hdr = opts.hdr_active.then_some(opts.hdr_post);
        if self.targets.as_ref().is_some_and(|t| t.size == size && t.hdr == hdr) {
            return;
        }
        if self.targets.is_some() {
            // Clear views and bind groups as well as texture owners. Drain
            // previous submissions only on resolution changes, not per frame.
            self.targets = None;
            self.msaa = None;
            self.scaled_depth = None;
            self.enhanced = None;
            self.enhance = None;
            self.ssao = None;
            self.ssao_frame = None;
            self.ssr = None;
            self.ssr_frame = None;
            self.volumetric_frame = None;
            let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        }
        // The glow targets at the size the screen's last apply asked for
        // ([`glow_size`]; corrected by `ensure_glow` when it changes).
        let half = glow_size(size, self.last_output.unwrap_or(size), hdr);
        let (scene_format, inter) = if hdr.is_some() {
            (crate::render::HDR_FORMAT, crate::render::HDR_FORMAT)
        } else {
            (self.format, INTERMEDIATE)
        };
        let scene = texture(gpu, "scene", size, scene_format);
        let full = [texture(gpu, "full 0", size, inter), texture(gpu, "full 1", size, inter)];
        let halves = [
            texture(gpu, "half A", half, inter),
            texture(gpu, "half B", half, inter),
            texture(gpu, "half C", half, inter),
        ];
        // COPY_SRC: phase 4's scratch frames keep C_{n−1} by a copy
        // ([`StreakFrame`]); the usage changes nothing drawn.
        let streak = streak_texture(gpu, size, inter);
        let gain_view = self.gain.create_view(&Default::default());
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let streak_view = view(&streak);
        let group = |t: &wgpu::Texture| self.group(gpu, &view(t), &gain_view);
        let group_with = |t: &wgpu::Texture, second: &wgpu::TextureView| self.group(gpu, &view(t), second);
        // In `Src` order.
        let mut groups = vec![
            group(&scene),
            group(&full[0]),
            group(&full[1]),
            group(&halves[0]),
            group(&halves[1]),
            group(&halves[2]),
            group_with(&scene, &streak_view),
            group_with(&full[0], &streak_view),
            group_with(&full[1], &streak_view),
            group(&streak),
        ];
        if hdr.is_some() {
            groups.push(group_with(&halves[1], &view(&halves[2])));
        }
        let views = TargetViews {
            scene: view(&scene),
            full: [view(&full[0]), view(&full[1])],
            half: [view(&halves[0]), view(&halves[1]), view(&halves[2])],
            streak: streak_view.clone(),
        };
        self.targets = Some(Targets {
            size,
            hdr,
            glow: (half.0.max(1), half.1.max(1)),
            scene,
            full,
            half: halves,
            streak,
            resolved: None,
            views,
            groups,
            fresh: false,
        });
        if hdr.is_some() {
            self.warm_hdr(gpu);
        }
    }

    /// A bind group reading `view` (binding 0) with `second` beside it
    /// (binding 1: the gain map, the streak buffer or half C).
    pub(super) fn group(&self, gpu: &Gpu, view: &wgpu::TextureView, second: &wgpu::TextureView) -> wgpu::BindGroup {
        let ramp_view = self.ramp.create_view(&Default::default());
        let lut_view = self.lut.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postfx"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(second) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ramp_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&lut_view) },
                wgpu::BindGroupEntry { binding: 6, resource: self.bounds.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                    }),
                },
            ],
        })
    }

    /// The glow targets (A–C) remade at `glow` when the screen's size
    /// asks for another (the scene and its copies are kept).
    pub(super) fn ensure_glow(&mut self, gpu: &Gpu, glow: (u32, u32)) {
        let Some(t) = self.targets.as_ref() else { return };
        if t.glow == glow {
            return;
        }
        let inter = self.inter();
        let hdr = t.hdr;
        let halves = [
            texture(gpu, "half A", glow, inter),
            texture(gpu, "half B", glow, inter),
            texture(gpu, "half C", glow, inter),
        ];
        let gain_view = self.gain.create_view(&Default::default());
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let mut groups = [Src::A, Src::B, Src::C]
            .map(|s| (s as usize, self.group(gpu, &view(&halves[s as usize - Src::A as usize]), &gain_view)))
            .to_vec();
        if hdr.is_some() {
            groups.push((Src::BWithC as usize, self.group(gpu, &view(&halves[1]), &view(&halves[2]))));
        }
        let t = self.targets.as_mut().unwrap();
        for (i, g) in groups {
            t.groups[i] = g;
        }
        t.views.half = [view(&halves[0]), view(&halves[1]), view(&halves[2])];
        t.half = halves;
        t.glow = glow;
    }

    /// The screen-size image the scene is resolved into under SSAA with
    /// High Precision Post (`size` the screen's), made when missing or of
    /// another size, with its bind groups ([`Src::Resolved`] beside the gain
    /// map, [`Src::ResolvedStreak`] beside the streak buffer). -> its view.
    pub(super) fn ensure_resolved(&mut self, gpu: &Gpu, size: (u32, u32)) -> wgpu::TextureView {
        let t = self.targets.as_ref().unwrap();
        if let Some((s, v)) = t.resolved.as_ref()
            && *s == size
        {
            return v.clone();
        }
        let view = texture(gpu, "resolved", size, crate::render::HDR_FORMAT).create_view(&Default::default());
        let gain_view = self.gain.create_view(&Default::default());
        let plain = self.group(gpu, &view, &gain_view);
        let with_streak = self.group(gpu, &view, &t.views.streak);
        let t = self.targets.as_mut().unwrap();
        // The groups in `Src` order: BWithC (High Precision Post's) before them.
        while t.groups.len() < Src::ResolvedStreak as usize + 1 {
            t.groups.push(plain.clone());
        }
        t.groups[Src::Resolved as usize] = plain;
        t.groups[Src::ResolvedStreak as usize] = with_streak;
        t.resolved = Some((size, view.clone()));
        view
    }

    /// The scene texture the chain reads (tests write their input into it).
    #[cfg(test)]
    pub(super) fn scene_texture(&self) -> &wgpu::Texture {
        &self.targets.as_ref().unwrap().scene
    }

    /// The scene's format: the output's, or [`crate::render::HDR_FORMAT`] with High
    /// Precision Post (as the targets were last made).
    pub fn scene_format(&self) -> wgpu::TextureFormat {
        if self.hdr() { crate::render::HDR_FORMAT } else { self.format }
    }

    /// The screen copies', glow targets' and streak buffer's format.
    pub(crate) fn inter(&self) -> wgpu::TextureFormat {
        if self.hdr() { crate::render::HDR_FORMAT } else { INTERMEDIATE }
    }

    pub(super) fn hdr(&self) -> bool {
        self.targets.as_ref().is_some_and(|t| t.hdr.is_some())
    }

    /// The pipelines a drive's frames with `opts` at `samples` add to the
    /// ones made with the chain (High Precision Post's set, the
    /// multisampled scene's lerp and streak copy, the fused passes once the
    /// self-check chose them, the horizon fog in the scene's mode), made now:
    /// a drive's warm-up (docs/first-use-builds.md). They go to the
    /// chain's own list as the frame's first use would put them; nothing
    /// drawn changes.
    pub fn warm_frame(&mut self, gpu: &Gpu, opts: &crate::render::RenderOptions, samples: u32) {
        let hdr = opts.hdr_active;
        if hdr {
            self.warm_hdr(gpu);
            // The prefilter of a glow smaller than the scene (`overbright`'s
            // scaled branch).
            let f = crate::render::HDR_FORMAT;
            self.pipeline(gpu, "fs_box_hdr", f, Blend::Replace);
            self.pipeline(gpu, "fs_overbright_sat", f, Blend::Replace);
        } else {
            // The native glow's prefilter when the scene is drawn at another
            // size than the glow's (Render Scale, FSR 1: the scaled branch).
            self.pipeline(gpu, "fs_box", INTERMEDIATE, Blend::Replace);
        }
        let (scene, inter) =
            if hdr { (crate::render::HDR_FORMAT, crate::render::HDR_FORMAT) } else { (self.format, INTERMEDIATE) };
        if samples > 1 {
            self.pipeline_ms(gpu, "fs_filter", scene, Blend::LerpConstant, samples);
            self.pipeline_ms(gpu, "fs_filter", inter, Blend::Replace, samples);
        }
        if let Some(v) = self.fused {
            self.pipeline_v(gpu, "fs_bleach_fused", INTERMEDIATE, 1, Blend::Replace, v);
            self.pipeline_v(gpu, "fs_overbright_mrt", INTERMEDIATE, 2, Blend::Replace, v);
        }
        self.fog_pipeline(gpu);
        // The rain's tint (0x5C7870, made with the first rain before): dest ×
        // m, and the second pass above 1, into the scene's and the
        // intermediate format.
        for f in [scene, inter] {
            self.pipeline(gpu, "fs_zero", f, Blend::ScaleDest);
            self.pipeline(gpu, "fs_one", f, Blend::AddScaledDest);
        }
        // The lens drops' pass (0x5C7AC0; made with the first drops before,
        // `set_rain`): it draws nothing until RAINDROPPC is set.
        let formats = [INTERMEDIATE, self.format, crate::render::HDR_FORMAT];
        self.lens.get_or_insert_with(|| crate::render::rain::LensPass::new(gpu, &formats));
        let mode = crate::render::SceneMode {
            lamps: false,
            lamp_features: 0,
            modern: 0,
            bias_q: 0,
            ..crate::render::SceneMode::driving(opts, samples)
        };
        if mode != crate::render::SceneMode::NATIVE && self.fog_ms.as_ref().is_none_or(|f| f.0 != mode) {
            self.fog_ms = Some((mode, self.make_fog_pipeline(gpu, mode)));
        }
    }

    /// High Precision Post's pipelines, built when it is first switched on.
    fn warm_hdr(&mut self, gpu: &Gpu) {
        let (out, f) = (self.format, crate::render::HDR_FORMAT);
        let used: [(&'static str, wgpu::TextureFormat, Blend); 16] = [
            ("fs_motion", f, Blend::Replace),
            ("fs_filter", f, Blend::LerpConstant),
            ("fs_zero", f, Blend::ScaleDest),
            ("fs_add", f, Blend::Replace),
            ("fs_one", f, Blend::Min),
            ("fs_bleach_hdr", f, Blend::Replace),
            ("fs_filter", f, Blend::Replace),
            ("fs_blur_hdr", f, Blend::Replace),
            ("fs_blur_sub_hdr", f, Blend::Replace),
            ("fs_filter", f, Blend::AddConstant),
            ("fs_out_add", out, Blend::Replace),
            ("fs_out_add_a0", out, Blend::Replace),
            ("fs_out", out, Blend::Replace),
            ("fs_out_a0", out, Blend::Replace),
            ("fs_out_add", f, Blend::Replace),
            ("fs_out", f, Blend::Replace),
        ];
        for (entry, format, blend) in used {
            self.pipeline(gpu, entry, format, blend);
        }
        self.pipeline_v(gpu, "fs_overbright_mrt_hdr", f, 2, Blend::Replace, 0);
        // SSAA's resolve before the bleach bypass and its output
        // (docs/ssaa-screen-passes.md).
        for bleach in [true, false] {
            let variant = super::resolve_variant(bleach);
            self.pipeline_v(gpu, "fs_resolve", f, 1, Blend::Replace, variant);
        }
        for entry in ["fs_out_add_resolved", "fs_out_add_resolved_a0"] {
            self.pipeline(gpu, entry, out, Blend::Replace);
        }
    }

    /// The view to draw the scene into, `size` big (with a depth buffer of
    /// that size).
    pub fn scene_view(&mut self, gpu: &Gpu, size: (u32, u32), opts: &crate::render::RenderOptions) -> wgpu::TextureView {
        self.resize(gpu, size, opts);
        self.targets.as_ref().unwrap().views.scene.clone()
    }

    /// The size of the output the next [`PostFx::apply`] writes, when the
    /// scene is drawn at another (the Graphics page's Render Scale).
    pub fn set_output_size(&mut self, size: (u32, u32)) {
        self.output_size = Some(size);
    }

    /// A depth buffer `size` big for the scene drawn at the Render Scale
    /// (the window's own is the screen's size).
    pub fn scaled_depth(&mut self, gpu: &Gpu, size: (u32, u32)) -> wgpu::TextureView {
        if self.scaled_depth.as_ref().is_none_or(|(s, _)| *s != size) {
            self.scaled_depth = Some((size, crate::render::create_depth(gpu, size.0, size.1)));
        }
        self.scaled_depth.as_ref().unwrap().1.clone()
    }

    pub(crate) fn pipeline(
        &mut self,
        gpu: &Gpu,
        entry: &'static str,
        format: wgpu::TextureFormat,
        blend: Blend,
    ) -> usize {
        self.pipeline_v(gpu, entry, format, 1, blend, 0)
    }

    /// A one-target pipeline at `samples` (into the multisampled scene).
    pub(super) fn pipeline_ms(
        &mut self,
        gpu: &Gpu,
        entry: &'static str,
        format: wgpu::TextureFormat,
        blend: Blend,
        samples: u32,
    ) -> usize {
        self.pipeline_key(gpu, entry, format, 1, blend, 0, samples)
    }

    /// A pipeline into `targets` targets of `format`, with `q`'s variant
    /// `variant` (0 for the passes that do not use it).
    pub(crate) fn pipeline_v(
        &mut self,
        gpu: &Gpu,
        entry: &'static str,
        format: wgpu::TextureFormat,
        targets: usize,
        blend: Blend,
        variant: u8,
    ) -> usize {
        self.pipeline_key(gpu, entry, format, targets, blend, variant, 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn pipeline_key(
        &mut self,
        gpu: &Gpu,
        entry: &'static str,
        format: wgpu::TextureFormat,
        targets: usize,
        blend: Blend,
        variant: u8,
        samples: u32,
    ) -> usize {
        let key = (entry, format, blend as u8, variant, samples);
        if let Some(i) = self.pipelines.iter().position(|(k, _)| *k == key) {
            return i;
        }
        // Variants 1 (the measured steps), 2 (to nearest, ties to even).
        let all = [("MEASURED", if variant == 1 { 1.0 } else { 0.0 }), ("STAGES", check_stages() as f64)];
        let resolve = [("RESOLVE_BLEACH", f64::from(variant & 1))];
        let constants: &[(&str, f64)] = match variant {
            0 => &[],
            _ if entry == "fs_resolve" => &resolve,
            // The LUT's tetrahedral interpolation (the verification).
            3 if entry.starts_with("fs_grade_lut") => &[("TETRA", 1.0)],
            _ if entry == "fs_bleach_fused" => &all,
            _ => &all[..1],
        };
        let target = Some(wgpu::ColorTargetState { format, blend: blend.state(), write_mask: wgpu::ColorWrites::ALL });
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions { constants, ..Default::default() },
                targets: &vec![target; targets],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        self.pipelines.push((key, p));
        self.pipelines.len() - 1
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        entry: &'static str,
        blend: Blend,
        src: Src,
        slot: u32,
        constant: Option<wgpu::Color>,
        clear: Option<wgpu::Color>,
    ) {
        let pipeline = self.pipeline(gpu, entry, format, blend);
        let targets = self.targets.as_ref().unwrap();
        // A replacing pass need not read the target in (`replace_load`).
        let load = match clear {
            Some(c) => wgpu::LoadOp::Clear(c),
            None if blend == Blend::Replace => replace_load(),
            None => wgpu::LoadOp::Load,
        };
        full_pass_op(
            encoder,
            entry,
            &self.pipelines[pipeline].1,
            &[target],
            &targets.groups[src as usize],
            slot,
            constant,
            load,
        );
    }

    /// A fused pass (variant `variant`) of `entry` into `outputs`, all
    /// [`INTERMEDIATE`].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_fused(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        outputs: &[&wgpu::TextureView],
        entry: &'static str,
        variant: u8,
        src: Src,
        slot: u32,
    ) {
        let format = self.inter();
        let pipeline = self.pipeline_v(gpu, entry, format, outputs.len(), Blend::Replace, variant);
        let targets = self.targets.as_ref().unwrap();
        // Replacing, every pixel: nothing to read in (`replace_load`).
        full_pass_op(
            encoder,
            entry,
            &self.pipelines[pipeline].1,
            outputs,
            &targets.groups[src as usize],
            slot,
            None,
            replace_load(),
        );
    }

    /// What the post passes run as: fused or separate, and why
    /// ([`PostFx::new`]'s self-check); None while the check runs.
    /// A bind group of `v0`, `v1`, `v2` (bindings 0–2) and `uniforms`.
    pub(crate) fn group_of(
        &self,
        gpu: &Gpu,
        v0: &wgpu::TextureView,
        v1: &wgpu::TextureView,
        v2: &wgpu::TextureView,
        uniforms: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let lut_view = self.lut.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postfx (grade lut)"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(v0) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(v1) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(v2) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&lut_view) },
                wgpu::BindGroupEntry { binding: 6, resource: self.bounds.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                    }),
                },
            ],
        })
    }

    /// The view an image source reads (the scene or a screen copy).
    pub(super) fn src_view(&self, src: Src) -> wgpu::TextureView {
        let v = &self.targets.as_ref().unwrap().views;
        match src {
            Src::Full0 => v.full[0].clone(),
            Src::Full1 => v.full[1].clone(),
            _ => v.scene.clone(),
        }
    }
}
