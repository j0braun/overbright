//! The road reflection's (view 4) headlight pools and flares (moved from render/mod.rs).

use crate::render::*;

impl WorldRenderer {
    /// Upload this frame's headlight pools. 0x60D5E0 draws a pool only
    /// while the car is upright (its matrix's z-axis z, +0x28, at least
    /// 0.707); the colour goes out with alpha 0x80 (`+0x380 | 0x80000000`)
    /// as the vertex colour of world.efx, so the pool adds
    /// `2·tex·colour · tex.a·0.5`.
    pub(in crate::render) fn upload_pools(&mut self, gpu: &Gpu, pools: &[HeadlightPool]) {
        let mut vertices = Vec::new();
        let mut instances = Vec::new();
        for pool in pools.iter().filter(|p| p.transform.z_axis.z >= 0.707).take(MAX_POOLS) {
            let [r, g, b] = pool.color;
            for (position, uv) in POOL_CORNERS {
                vertices.push(Vertex { position, color: [b, g, r, 0x80], uv });
            }
            instances.push(instance_matrix(pool.transform, false, 0.0, false));
        }
        self.pool_count = instances.len() as u32;
        if self.pool_count > 0 {
            gpu.queue.write_buffer(&self.pool_vertices, 0, bytemuck::cast_slice(&vertices));
            gpu.queue.write_buffer(&self.pool_instances, 0, bytemuck::cast_slice(&instances));
        }
    }

    /// Upload this frame's road reflection flares (vertex colour as the
    /// glow sprites', [`WorldRenderer::upload_glows`]).
    pub(in crate::render) fn upload_reflection_glows(&mut self, gpu: &Gpu, quads: &[crate::particles::Quad]) {
        let mut vertices = Vec::new();
        for q in quads.iter().take(MAX_REFLECTION_GLOWS) {
            let [r, g, b, a] = q.rgba;
            for i in 0..4 {
                vertices.push(Vertex { position: q.corners[i].to_array(), color: [b, g, r, a], uv: q.uv[i] });
            }
        }
        self.reflection_glow_count = (vertices.len() / 4) as u32;
        if !vertices.is_empty() {
            gpu.queue.write_buffer(&self.reflection_glow_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }
}

// Refactor: from render/mod.rs
/// Reflect the complete world about a horizontal receiving road plane.
pub(in crate::render) fn road_plane_matrix(height: f32) -> Mat4 {
    Mat4::from_cols(Vec3::X.extend(0.0), Vec3::Y.extend(0.0), (-Vec3::Z).extend(0.0), Vec3::new(0.0, 0.0, 2.0 * height).extend(1.0))
}

/// Size of view 4's target (0x5C21F0 sets 0x140 × 0xF0; 0x5BA220 creates
/// render-target record 1 with it).
pub(in crate::render) const REFLECTION_SIZE: (u32, u32) = (320, 240);

/// The off-screen reflection image (view 4).
pub(in crate::render) struct ReflectionTarget {
    pub(in crate::render) size: (u32, u32),
    pub(in crate::render) texture: wgpu::Texture,
    pub(in crate::render) color: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
    /// Group 2 for the main pass: samples `color`.
    pub(in crate::render) bind_group: wgpu::BindGroup,
}

impl WorldRenderer {
    /// The reflection image (view 4) as last rendered, for inspection.
    pub fn reflection_image(&self) -> Option<(&wgpu::Texture, (u32, u32))> {
        self.reflection.as_ref().map(|r| (&r.texture, r.size))
    }

    pub(in crate::render) fn ensure_reflection_target(&mut self, gpu: &Gpu, size: (u32, u32)) {
        if self.reflection.as_ref().is_some_and(|r| r.size == size) {
            return;
        }
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reflection"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: REFLECTION_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color = texture.create_view(&Default::default());
        let bind_group = bind_texture(&gpu.device, &self.reflection_layout, &color, &self.reflection_sampler);
        let depth = create_depth(gpu, size.0, size.1);
        self.reflection = Some(ReflectionTarget { size, texture, color, depth, bind_group });
    }
}
