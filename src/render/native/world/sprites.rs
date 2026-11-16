//! Native particles and glow sprites: upload and draw (moved from render/mod.rs).

use crate::render::*;

impl WorldRenderer {
    /// Draw this frame's particles over what the main view and the cars
    /// left (0x48BDF0 runs in the view's effects after the world): one
    /// quad each, the vertex colour the particle's RGBA as it is (0x5C0E20
    /// packs the clamped colour cubic into a D3DCOLOR, 0x5BB440 copies it to
    /// the vertices; the pixel shader is world.efx's `2·tex·v0`, alpha
    /// `tex.a·v0.a`, worldnodepth's for the particles, world's for the
    /// rain), in texture runs in particle order, depth-tested, not written
    /// for blended textures.
    pub fn render_particles(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        quads: &[crate::particles::Quad],
        opts: &RenderOptions,
    ) {
        self.upload_particles(gpu, quads, opts);
        self.draw_particles(gpu, encoder, color, depth, None);
    }

    /// [`WorldRenderer::render_particles`]' upload alone (the temporal pass
    /// draws them later, [`WorldRenderer::draw_particles`]).
    pub fn upload_particles(&mut self, gpu: &Gpu, quads: &[crate::particles::Quad], opts: &RenderOptions) {
        self.prepare_weather_fx(gpu, opts);
        let mut vertices = Vec::with_capacity(quads.len() * 4);
        self.particle_runs.clear();
        for q in quads.iter().filter(|q| self.particle_textures.contains_key(&q.texture)).take(MAX_PARTICLE_QUADS) {
            let [r, g, b, a] = q.rgba;
            let n = (vertices.len() / 4) as u32;
            match self.particle_runs.last_mut() {
                Some(run) if run.0 == q.texture => run.2 += 1,
                _ => self.particle_runs.push((q.texture, n, 1)),
            }
            for i in 0..4 {
                vertices.push(Vertex { position: q.corners[i].to_array(), color: [b, g, r, a], uv: q.uv[i] });
            }
        }
        if vertices.is_empty() {
            self.particle_runs.clear();
            return;
        }
        gpu.queue.write_buffer(&self.particle_vertices, 0, bytemuck::cast_slice(&vertices));
    }

    /// Draw the uploaded particles into `color` against `depth`; with
    /// `viewport` (x, y, w, h) the temporal pass's jitter taken off
    /// (`temporal::late_viewport`).
    pub fn draw_particles(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
    ) {
        self.draw_native_particles(encoder, color, depth, viewport);
        // Wet Road Interaction's volumetric spray, under the
        // droplets.
        if let Some(v) = self.spray_volume.as_ref().filter(|_| self.fx_view.is_some()) {
            v.draw(gpu, encoder, color, depth, viewport);
        }
        // Weather effects (clone deviation) over them.
        if let Some(fx) = self.weather_fx.as_ref().filter(|_| self.fx_view.is_some()) {
            let mode = self.scene_pipelines().mode;
            fx.draw(encoder, color, mode.target(self.scene_format), mode.samples, depth, viewport);
        }
    }

    fn draw_native_particles(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        viewport: Option<[f32; 4]>,
    ) {
        if self.particle_runs.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particles"),
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
        pass.set_bind_group(0, &self.scene_group(), &[]);
        pass.set_bind_group(2, &self.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.particle_vertices.slice(..));
        pass.set_vertex_buffer(1, self.particle_instance.slice(..));
        pass.set_index_buffer(self.particle_indices.slice(..), wgpu::IndexFormat::Uint32);
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        let mut bound = usize::MAX;
        let scene = self.scene_pipelines();
        for &(texture, first, count) in &self.particle_runs {
            let (slot, key) = self.particle_textures[&texture];
            pass.set_pipeline(&scene.pipelines[&key]);
            self.set_texture(&mut pass, slot, &mut bound);
            pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
        }
        drop(pass);
        // (Their own part of the GPU profile: without it their time went to the next mark, the spray volume's
        // build or the weather effects.)
        crate::prof::gpu::mark(encoder, "graphics: native particles");
    }

    /// Upload this frame's glow sprites (`crate::glow`): one quad each, the
    /// vertex colour the sprite's RGBA as it is (worldnodepth's pixel
    /// shader is world.efx's, `2·tex·v0`, alpha `tex.a·v0.a`), in texture
    /// runs.
    ///
    /// With the clone's depth of field drawn this frame (Enhanced, scenes and photo mode only),
    /// each sprite takes the blur of its depth (`blur_dof::defocus_quads`); without
    /// it the quads are as given.
    pub fn upload_glows(&mut self, gpu: &Gpu, quads: &[crate::particles::Quad]) {
        let defocused;
        let quads = match crate::render::blur_dof::take_sprite_dof() {
            Some(d) => {
                defocused = crate::render::blur_dof::defocus_quads(quads, &d, MAX_PARTICLE_QUADS);
                &defocused[..]
            }
            None => quads,
        };
        let mut vertices = Vec::with_capacity(quads.len() * 4);
        self.glow_runs.clear();
        for q in quads.iter().filter(|q| self.glow_textures.contains_key(&q.texture)).take(MAX_PARTICLE_QUADS) {
            let [r, g, b, a] = q.rgba;
            let n = (vertices.len() / 4) as u32;
            match self.glow_runs.last_mut() {
                Some(run) if run.0 == q.texture => run.2 += 1,
                _ => self.glow_runs.push((q.texture, n, 1)),
            }
            for i in 0..4 {
                vertices.push(Vertex { position: q.corners[i].to_array(), color: [b, g, r, a], uv: q.uv[i] });
            }
        }
        if !vertices.is_empty() {
            gpu.queue.write_buffer(&self.glow_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Draw the uploaded glow sprites into `color` (the back buffer, or the
    /// light-streak buffer when `streak`) against the main view's depth,
    /// depth-tested, not depth-written (their textures blend), colour
    /// only (0x5CBC00: COLORWRITEENABLE 7; 0x5CBC65: ZENABLE; 0x5CBC76:
    /// ZWRITEENABLE 0).
    pub fn draw_glows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        streak: bool,
    ) {
        self.draw_glows_resolved(encoder, color, None, depth, streak);
    }

    /// [`WorldRenderer::draw_glows`] into a multisampled `color`, resolved
    /// into `resolve` at the pass's end.
    pub fn draw_glows_resolved(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        resolve: Option<&wgpu::TextureView>,
        depth: &wgpu::TextureView,
        streak: bool,
    ) {
        self.draw_glows_at(encoder, color, resolve, depth, streak, None);
    }

    /// [`WorldRenderer::draw_glows_resolved`] with the temporal pass's
    /// jitter taken off by `viewport` (`temporal::late_viewport`).
    pub fn draw_glows_at(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        resolve: Option<&wgpu::TextureView>,
        depth: &wgpu::TextureView,
        streak: bool,
        viewport: Option<[f32; 4]>,
    ) {
        if self.glow_runs.is_empty() {
            return;
        }
        let scene = self.scene_pipelines();
        let pipelines = if streak { &scene.glow_streak } else { &scene.glow };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glows"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: resolve,
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
        pass.set_bind_group(0, &self.scene_group(), &[]);
        pass.set_bind_group(2, &self.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.glow_vertices.slice(..));
        pass.set_vertex_buffer(1, self.particle_instance.slice(..));
        pass.set_index_buffer(self.particle_indices.slice(..), wgpu::IndexFormat::Uint32);
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        let mut bound = usize::MAX;
        for &(texture, first, count) in &self.glow_runs {
            let (slot, key) = self.glow_textures[&texture];
            pass.set_pipeline(&pipelines[&key]);
            self.set_texture(&mut pass, slot, &mut bound);
            pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
        }
    }

    /// Bind slot `slot`'s texture for the next draws: the core's, or one
    /// added since (`extra_textures`). `bound` is the group 1 bound now
    /// (usize::MAX for none), kept up to date.
    pub(in crate::render) fn set_texture(&self, pass: &mut wgpu::RenderPass<'_>, slot: usize, bound: &mut usize) {
        let Some(extra) = slot.checked_sub(self.texture_scrolls.len()) else {
            return self.core.set_world_texture(pass, slot, bound);
        };
        let (group, scroll, layer) = &self.extra_textures[extra];
        // Past the core's arrays' numbers.
        let id = self.texture_arrays.as_ref().map_or(0, |a| a.groups.len()) + extra;
        if *bound != id {
            pass.set_bind_group(1, group, &[]);
            *bound = id;
        }
        if self.texture_arrays.is_some() {
            let imm = TextureImmediates { scroll: *scroll, layer: *layer, _pad: [0; 3] };
            pass.set_immediates(0, bytemuck::bytes_of(&imm));
        }
    }
}
