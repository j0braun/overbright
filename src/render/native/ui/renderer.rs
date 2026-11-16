use super::*;

pub struct UiRenderer {
    /// By the texture's blend (TPK entry byte 0x56, 0x5BCC40): 0 alpha,
    /// 1 additive, 2 destination alpha, 3 destination alpha additive; 4
    /// writes the vertex alpha alone (the alpha-only quads); 5 none
    /// (byte 0: blending off).
    pub(super) pipelines: Vec<wgpu::RenderPipeline>,
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
    pub(super) white: wgpu::BindGroup,
    pub(super) textures: crate::fxhash::FxHashMap<u32, wgpu::BindGroup>,
    pub(super) bc: bool,
    /// The vertex buffer, reused from frame to frame (grown as needed).
    pub(super) vertices: Option<wgpu::Buffer>,
    /// A clamping sampler, for [`UiRenderer::set_texture_clamped`].
    pub(super) clamp: Option<wgpu::Sampler>,
    /// The textures bound from the HD UI Art pack (a replacement PNG,
    /// `texture_pack::Domain::Ui`), which [`UiRenderer::record`] draws
    /// with ui.wgsl's `sharp` when magnified. Empty with HD UI Art off.
    pub(super) hd: crate::fxhash::FxHashSet<u32>,
    /// docs/pack-hot-reload.md: the pack changes seen, and the
    /// textures bound from the assets (rebound when their file changes).
    pub(super) pack_seen: u64,
    pub(super) from_assets: crate::fxhash::FxHashSet<u32>,
}

impl UiRenderer {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> UiRenderer {
        let device = &gpu.device;
        let layout = make_texture_layout_plain(device, "ui texture");
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("ui.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::Ui).into(),
            ),
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // No effect or code enables SEPARATEALPHABLENDENABLE, so the alpha
        // channel blends with the colour factors (see the world pipelines).
        let same = |src: wgpu::BlendFactor, dst: wgpu::BlendFactor| {
            let c = wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add };
            Some(wgpu::BlendState { color: c, alpha: c })
        };
        use wgpu::BlendFactor as F;
        let make_with = |entry: &str, label: &str, blend: Option<wgpu::BlendState>, write_mask: wgpu::ColorWrites| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x3],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let make = |label: &str, blend: Option<wgpu::BlendState>, write_mask: wgpu::ColorWrites| {
            make_with("fs_main", label, blend, write_mask)
        };
        // The HUD palette's (`fs_palette`) for the first four blends,
        // after the native six (`PALETTE_PIPELINES`).
        let pipelines = vec![
            make("ui", same(F::SrcAlpha, F::OneMinusSrcAlpha), wgpu::ColorWrites::ALL),
            make("ui additive", same(F::SrcAlpha, F::One), wgpu::ColorWrites::ALL),
            make("ui dest alpha", same(F::DstAlpha, F::OneMinusDstAlpha), wgpu::ColorWrites::ALL),
            make("ui dest alpha additive", same(F::DstAlpha, F::One), wgpu::ColorWrites::ALL),
            make("ui alpha only", None, wgpu::ColorWrites::ALPHA),
            make("ui opaque", None, wgpu::ColorWrites::ALL),
            make_with("fs_palette", "ui palette", same(F::SrcAlpha, F::OneMinusSrcAlpha), wgpu::ColorWrites::ALL),
            make_with("fs_palette", "ui palette additive", same(F::SrcAlpha, F::One), wgpu::ColorWrites::ALL),
            make_with(
                "fs_palette",
                "ui palette dest alpha",
                same(F::DstAlpha, F::OneMinusDstAlpha),
                wgpu::ColorWrites::ALL,
            ),
            make_with(
                "fs_palette",
                "ui palette dest alpha additive",
                same(F::DstAlpha, F::One),
                wgpu::ColorWrites::ALL,
            ),
        ];
        // Sampler (**verified**): fe.efx's technique "world" (blob at file
        // 0x426E40) sets D3DX effect states 165 / 166 (AddressU / V) = 1
        // WRAP and 169 / 170 / 171 (Mag / Min / Mip filter) = 2 LINEAR on
        // its texture (164 = the DiffuseMap parameter); 0x5CC1E5..0x5CC200
        // also set stage 0's ADDRESSU / V to WRAP before 0x5378D0 draws the
        // front end.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let white_view = upload_rgba(gpu, 1, 1, &[255, 255, 255, 255]);
        let white = bind_texture(device, &layout, &white_view, &sampler);
        let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        UiRenderer {
            pipelines,
            layout,
            sampler,
            white,
            textures: Default::default(),
            bc,
            vertices: None,
            clamp: None,
            hd: Default::default(),
            pack_seen: crate::render::pack_reload::generation(),
            from_assets: Default::default(),
        }
    }

    pub(super) fn bind(&mut self, gpu: &Gpu, assets: &Assets, hash: u32) {
        if self.textures.contains_key(&hash) {
            return;
        }
        if let Some(t) = assets.textures.get(&hash) {
            let view = upload_texture(gpu, t, self.bc);
            self.textures.insert(hash, bind_texture(&gpu.device, &self.layout, &view, &self.sampler));
            self.mark_hd(gpu, t);
            self.from_assets.insert(hash);
        }
    }

    /// Forget the asset textures whose pack file changed, so this
    /// frame's [`UiRenderer::bind`] uploads them again.
    pub(super) fn sync_pack(&mut self, assets: &Assets) {
        use crate::render::pack_reload::{Pending, since};
        let (now, pending) = since(self.pack_seen, crate::render::texture_pack::Domain::Ui);
        self.pack_seen = now;
        if !pending.any() {
            return;
        }
        let stale: Vec<u32> = self
            .from_assets
            .iter()
            .copied()
            .filter(|h| match &pending {
                Pending::All => true,
                p => assets.textures.get(h).is_some_and(|t| p.hits(t.hash, &t.name)),
            })
            .collect();
        for h in stale {
            self.textures.remove(&h);
            self.hd.remove(&h);
            self.from_assets.remove(&h);
        }
    }

    /// Notes whether `t` was uploaded from the HD UI Art pack (the same
    /// cached lookup `upload_texture` made).
    pub(super) fn mark_hd(&mut self, gpu: &Gpu, t: &crate::formats::texture::Texture) {
        use crate::render::texture_pack::{Domain, enabled, replacement};
        if enabled(Domain::Ui) && replacement(Domain::Ui, t, gpu.device.limits().max_texture_dimension_2d).is_some() {
            self.hd.insert(t.hash);
        } else {
            self.hd.remove(&t.hash);
        }
    }

    /// Binds a rendered texture (e.g. the magazine cover's shot) under
    /// `hash`, replacing what was there.
    pub fn set_texture(&mut self, gpu: &Gpu, hash: u32, view: &wgpu::TextureView) {
        self.hd.remove(&hash);
        self.from_assets.remove(&hash);
        self.textures.insert(hash, bind_texture(&gpu.device, &self.layout, view, &self.sampler));
    }

    /// Binds a rendered texture under `hash`, sampled clamped to its
    /// edges (the rear-view mirror's image: 0x5CC186..0x5CC1AE set stage
    /// 0's ADDRESSU / V to CLAMP for its quad).
    pub fn set_texture_clamped(&mut self, gpu: &Gpu, hash: u32, view: &wgpu::TextureView) {
        self.hd.remove(&hash);
        self.from_assets.remove(&hash);
        let sampler = self.clamp.get_or_insert_with(|| {
            gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("ui clamped"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        });
        self.textures.insert(hash, bind_texture(&gpu.device, &self.layout, view, sampler));
    }

    /// Uploads an RGBA image (e.g. a movie frame) and binds it under
    /// `hash`, replacing what was there.
    pub fn set_rgba(&mut self, gpu: &Gpu, hash: u32, width: u32, height: u32, rgba: &[u8]) {
        self.hd.remove(&hash);
        self.from_assets.remove(&hash);
        let view = upload_rgba(gpu, width, height, rgba);
        self.textures.insert(hash, bind_texture(&gpu.device, &self.layout, &view, &self.sampler));
    }

    /// Uploads `texture` and binds it under its hash (e.g. the magazine
    /// cover's streamed layers), replacing what was there.
    pub fn add_texture(&mut self, gpu: &Gpu, texture: &crate::formats::texture::Texture) {
        let view = upload_texture(gpu, texture, self.bc);
        self.textures.insert(texture.hash, bind_texture(&gpu.device, &self.layout, &view, &self.sampler));
        self.mark_hd(gpu, texture);
        self.from_assets.remove(&texture.hash);
    }

    /// Draw `quads` into `target` (its size in pixels is not needed: the
    /// canvas stretches over it). With `clear`, the
    /// target is cleared to black first.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        assets: &Assets,
        target: &wgpu::TextureView,
        size: (u32, u32),
        quads: &[Quad],
        clear: bool,
    ) {
        let buffer = self.record(gpu, assets, target, size, quads, clear, Alpha::default());
        gpu.queue.submit([buffer]);
    }

    /// Whether `quads` draw a destination-alpha texture, so the frame's
    /// alpha is set to 0 before them (see [`UiRenderer::record`]).
    pub fn masked(assets: &Assets, quads: &[Quad]) -> bool {
        use crate::formats::texture::Blend;
        quads.iter().any(|q| {
            matches!(
                q.texture.and_then(|h| assets.textures.get(&h)).map(|t| t.blend),
                Some(Blend::DestAlpha | Blend::DestAlphaAdditive)
            )
        })
    }

    /// [`UiRenderer::render`] without the submission: the command buffer,
    /// to submit after what comes before it. `alpha` says what the target's
    /// alpha needs (see [`Alpha`]).
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        gpu: &Gpu,
        assets: &Assets,
        target: &wgpu::TextureView,
        size: (u32, u32),
        quads: &[Quad],
        clear: bool,
        alpha: Alpha,
    ) -> wgpu::CommandBuffer {
        self.record_encoder(gpu, assets, target, size, quads, clear, alpha).finish()
    }
}
