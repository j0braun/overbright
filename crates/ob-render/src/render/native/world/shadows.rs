//! Car shadow textures and their draws (moved from render/mod.rs).

use crate::render::*;

/// At most this many car shadows (and underglows) a frame, 9 quads each.
pub const MAX_SHADOWS: usize = 128;

/// The shadows' quads at most: nine each, sixteen times that with the
/// Graphics page's Shadow Quality Ultra ([`crate::shadow::Shadow::refine`]).
pub(in crate::render) const MAX_SHADOW_QUADS: usize = MAX_SHADOWS * 9 * 16;

impl WorldRenderer {
    /// Register a car's shadow texture (`%s_SHADOWIG` / `%s_SHADOWFE`, render
    /// info +0x33C) once. The render info constructor 0x6381A0 sets its +0x54
    /// to 0 and its rendering order +0x51 to 2; the strips draw with
    /// worldnodepth (effect 13, 0x86EB5C), whose vertex shader writes no
    /// fog (so [`Effect::NoFog`] here), with the texture's own blend.
    ///
    /// The Graphics page's Car Shadow / Neon **Ultra** (clone deviation,
    /// docs/notes/graphics/neon-quality.md): a `%s_NEON` underglow is uploaded rebuilt
    /// ([`crate::neon_quality::enhance`]: 4× size, deblocked, border faded
    /// to 0, half float) unless a texture pack replaces it; a tier change
    /// re-uploads it.
    pub fn add_shadow_texture(&mut self, gpu: &Gpu, t: &Texture, opts: &RenderOptions) {
        let max = gpu.device.limits().max_texture_dimension_2d;
        let enhanced = t.name.ends_with("_NEON")
            && opts.shadow_quality >= 2
            && texture_pack::replacement(texture_pack::Domain::Car, t, max).is_none();
        if self.shadow_textures.contains_key(&t.hash)
            && self.shadow_enhanced.get(&t.hash).copied().unwrap_or(false) == enhanced
        {
            return;
        }
        let rebuilt = enhanced
            .then(|| crate::neon_quality::enhance(&t.decode_rgba(), t.width, t.height))
            .flatten()
            .filter(|e| e.width <= max && e.height <= max);
        let bc = gpu.device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let slot = self.texture_scrolls.len() + self.extra_textures.len();
        let scroll = scroll_params(&t.scroll);
        let (key, bytes) = match &rebuilt {
            Some(e) => (
                (wgpu::TextureFormat::Rgba16Float, e.width, e.height, e.mips()),
                std::borrow::Cow::Owned(e.rgba16f_bytes()),
            ),
            None => texture_layer_with_pack(texture_pack::Domain::Car, Some(t), bc, max),
        };
        let group = match self.core.texture_arrays.as_ref() {
            // An array of its own.
            Some(a) => a.single_group(gpu, &self.texture_sampler, &prepared::single(key, &bytes)),
            None => {
                let (format, width, height, mips) = key;
                let view = gpu
                    .device
                    .create_texture_with_data(
                        &gpu.queue,
                        &wgpu::TextureDescriptor {
                            label: Some(&t.name),
                            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                            mip_level_count: mips,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        },
                        wgpu::util::TextureDataOrder::LayerMajor,
                        &bytes,
                    )
                    .create_view(&Default::default());
                bind_world_texture(&gpu.device, &self.texture_layout, &self.texture_sampler, &view, &t.scroll)
            }
        };
        // The tier asked for (a rebuild too large for the device is not
        // retried every frame).
        self.shadow_enhanced.insert(t.hash, enhanced);
        self.extra_textures.push((group, scroll, 0));
        let key = PipelineKey { effect: Effect::NoFog, blend: t.blend, alpha_test: t.alpha_test };
        self.shadow_textures.insert(t.hash, (slot, key));
    }

    /// A texture drawn through the world pipelines outside the world's own
    /// draws (the race start scene's characters): a slot of its own, in an
    /// array of its own with texture arrays, else its own bind group.
    pub fn add_texture_slot(&mut self, gpu: &Gpu, t: &Texture) -> usize {
        let bc = gpu.device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let slot = self.texture_scrolls.len() + self.extra_textures.len();
        let scroll = scroll_params(&t.scroll);
        let group = match self.core.texture_arrays.as_ref() {
            Some(a) => {
                let (key, bytes) = texture_layer_with_pack(
                    texture_pack::Domain::World,
                    Some(t),
                    bc,
                    gpu.device.limits().max_texture_dimension_2d,
                );
                a.single_group(gpu, &self.texture_sampler, &prepared::single(key, &bytes))
            }
            None => bind_world_texture(
                &gpu.device,
                &self.texture_layout,
                &self.texture_sampler,
                &upload_texture_for(gpu, t, bc, texture_pack::Domain::World),
                &t.scroll,
            ),
        };
        self.extra_textures.push((group, scroll, 0));
        slot
    }

    /// Draw this frame's car shadows (texture hash, shadow) over the world
    /// and the cars: each kept strip of 0x6162F0 (0x617323..0x6173E9) as
    /// quads, vertex colour the shadow's `color` (D3DCOLOR, 0x5BE0B0:
    /// 0x808080 with its alpha, or the underglow's), depth-tested; not depth-written (the flush 0x5CF430 draws
    /// strips, entry +0x0C = 1, as DrawPrimitiveUP triangle strips between
    /// SetRenderState(ZWRITEENABLE, 0) and (…, 1) at 0x5CF822 / 0x5CF89C),
    /// with an identity world matrix (0x6162F0 builds +0x118 as
    /// T(−pos)·T(pos)), so the vertices are world positions.
    pub fn render_shadows(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        shadows: &[(u32, crate::shadow::Shadow)],
    ) {
        let scene = self.scene_pipelines();
        // With the lamps' module (Contact Shadows), the alpha-blended
        // shadow textures through `fs_car_shadow`.
        let contact = (!scene.car_shadow.is_empty()).then_some(&scene.car_shadow);
        let group = self.core.scene_group();
        self.render_shadows_with(gpu, encoder, color, depth, shadows, &scene.pipelines, contact, &group);
    }

    /// Reward-cover SHADOWFE: native effect 13 writes RGB only, preserving
    /// destination alpha. This one-shot path uses the native scene mode.
    #[allow(clippy::too_many_arguments)]
    pub fn render_cover_shadow(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        shadows: &[(u32, crate::shadow::Shadow)],
    ) {
        let frame = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            clip: [f32::MIN, 1.0, 0.0, 0.0],
            ..bytemuck::Zeroable::zeroed()
        };
        gpu.queue.write_buffer(&self.main_frame.0, 0, bytemuck::bytes_of(&frame));
        let keys: Vec<_> = shadows.iter().filter_map(|(t, _)| self.shadow_textures.get(t).map(|(_, k)| *k)).collect();
        let pipelines =
            gpu.world_shared().pipelines(&gpu.device, self.arrays, self.scene_format, wgpu::ColorWrites::COLOR, &keys);
        let group = self.core.main_group();
        self.render_shadows_with(gpu, encoder, color, depth, shadows, &pipelines, None, &group);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_shadows_with(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        shadows: &[(u32, crate::shadow::Shadow)],
        pipelines: &HashMap<PipelineKey, wgpu::RenderPipeline>,
        contact: Option<&HashMap<PipelineKey, wgpu::RenderPipeline>>,
        group: &wgpu::BindGroup,
    ) {
        let mut vertices = Vec::new();
        let mut runs: Vec<(u32, u32, u32)> = Vec::new();
        for (texture, shadow) in shadows.iter().filter(|(t, _)| self.shadow_textures.contains_key(t)).take(MAX_SHADOWS)
        {
            let first = (vertices.len() / 4) as u32;
            let quads = shadow.quads();
            if first as usize + quads.len() > MAX_SHADOW_QUADS {
                break;
            }
            for q in &quads {
                for v in q {
                    vertices.push(Vertex { position: v.position.to_array(), color: shadow.color, uv: v.uv });
                }
            }
            runs.push((*texture, first, quads.len() as u32));
        }
        if vertices.is_empty() {
            return;
        }
        gpu.queue.write_buffer(&self.shadow_vertices, 0, bytemuck::cast_slice(&vertices));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("car shadows"),
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
        });
        pass.set_bind_group(0, group, &[]);
        pass.set_bind_group(2, &self.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.shadow_vertices.slice(..));
        pass.set_vertex_buffer(1, self.particle_instance.slice(..));
        pass.set_index_buffer(self.particle_indices.slice(..), wgpu::IndexFormat::Uint32);
        let mut bound = usize::MAX;
        for (texture, first, count) in runs {
            let (slot, key) = self.shadow_textures[&texture];
            let contact_pipeline = contact
                .filter(|_| key.blend == Blend::Alpha)
                .and_then(|c| c.get(&PipelineKey { effect: Effect::CarShadow, ..key }));
            pass.set_pipeline(contact_pipeline.unwrap_or(&pipelines[&key]));
            self.set_texture(&mut pass, slot, &mut bound);
            pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
        }
    }
}
