//! Native particles and glow sprites: upload and draw (moved from render/mod.rs).

use crate::render::*;
use crate::render::native::world::pipelines::{DROP_PROBE_FORMAT, DROP_PROBE_SIZE};

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
            self.drops_lit = false;
            return;
        }
        gpu.queue.write_buffer(&self.particle_vertices, 0, bytemuck::cast_slice(&vertices));
        self.upload_drop_probe(gpu, &vertices);
    }

    /// The falling rain's drops lit at their corners (the lamps' module,
    /// rain_lit.wgsl): the probe's vertices, each drop's placed twice as
    /// far from its centre (positions and UVs; the other quads' as they
    /// are, never drawn by the probe).
    fn upload_drop_probe(&mut self, gpu: &Gpu, vertices: &[Vertex]) {
        let rain = crate::formats::hash::bin_hash("RAINDROP");
        let scene = self.scene_pipelines();
        self.drops_lit = scene.drops.as_ref().is_some_and(|d| {
            self.particle_textures.get(&rain).is_some_and(|&(_, key)| key == d.key)
                && self.particle_runs.iter().any(|r| r.0 == rain)
        });
        if !self.drops_lit {
            return;
        }
        let probe = self.drop_probe.get_or_insert_with(|| DropProbe::new(gpu, &self.reflection_layout, &self.reflection_sampler));
        let mut placed = vertices.to_vec();
        for &(texture, first, count) in &self.particle_runs {
            if texture != rain {
                continue;
            }
            for q in first as usize..(first + count) as usize {
                place_for_probe(&mut placed[q * 4..q * 4 + 4]);
            }
        }
        gpu.queue.write_buffer(&probe.vertices, 0, bytemuck::cast_slice(&placed));
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
        if self.draw_drop_probe(encoder) {
            crate::prof::gpu::mark(encoder, "graphics: rain drop probe");
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
        let rain = crate::formats::hash::bin_hash("RAINDROP");
        let lit = scene.drops.as_ref().zip(self.drop_probe.as_ref()).filter(|_| self.drops_lit);
        let mut drop_group = false;
        for &(texture, first, count) in &self.particle_runs {
            let (slot, key) = self.particle_textures[&texture];
            match lit {
                Some((d, probe)) if texture == rain => {
                    pass.set_pipeline(&d.draw);
                    if !drop_group {
                        pass.set_bind_group(2, &probe.group, &[]);
                        drop_group = true;
                    }
                }
                _ => {
                    pass.set_pipeline(&scene.pipelines[&key]);
                    if drop_group {
                        pass.set_bind_group(2, &self.no_reflection, &[]);
                        drop_group = false;
                    }
                }
            }
            self.set_texture(&mut pass, slot, &mut bound);
            pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
        }
        drop(pass);
        // (Their own part of the GPU profile: without it their time went to the next mark, the spray volume's
        // build or the weather effects.)
        crate::prof::gpu::mark(encoder, "graphics: native particles");
    }

    /// The drops' probe (rain_lit.wgsl): K into the target's upper half,
    /// S into the lower, one 2 x 2 block per drop.
    fn draw_drop_probe(&self, encoder: &mut wgpu::CommandEncoder) -> bool {
        let scene = self.scene_pipelines();
        let (Some(d), Some(probe)) = (scene.drops.as_ref(), self.drop_probe.as_ref()) else {
            return false;
        };
        if !self.drops_lit {
            return false;
        }
        let rain = crate::formats::hash::bin_hash("RAINDROP");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rain drop probe"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &probe.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.scene_group(), &[]);
        pass.set_bind_group(2, &self.no_reflection, &[]);
        pass.set_vertex_buffer(0, probe.vertices.slice(..));
        pass.set_vertex_buffer(1, self.particle_instance.slice(..));
        pass.set_index_buffer(self.particle_indices.slice(..), wgpu::IndexFormat::Uint32);
        let (w, h) = (DROP_PROBE_SIZE.0 as f32, DROP_PROBE_SIZE.1 as f32 * 0.5);
        let mut bound = usize::MAX;
        for (pipeline, y) in [(&d.probe_k, 0.0), (&d.probe_s, h)] {
            pass.set_pipeline(pipeline);
            pass.set_viewport(0.0, y, w, h, 0.0, 1.0);
            for &(texture, first, count) in self.particle_runs.iter().filter(|r| r.0 == rain) {
                let (slot, _) = self.particle_textures[&texture];
                self.set_texture(&mut pass, slot, &mut bound);
                pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
            }
        }
        true
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

/// A drop's vertices for the probe (rain_lit.wgsl `vs_drop_probe`):
/// twice as far from the quad's centre, positions and UVs (UVs 0 / 1 about
/// 0.5), so the probe block's pixel centres, a quarter and three quarters
/// across, interpolate to the drop's own corners (the drop is a
/// parallelogram: every input is affine across it).
pub(in crate::render) fn place_for_probe(quad: &mut [Vertex]) {
    let c = quad.iter().fold(glam::Vec3::ZERO, |a, v| a + glam::Vec3::from(v.position)) / quad.len() as f32;
    for v in quad {
        v.position = (glam::Vec3::from(v.position) * 2.0 - c).to_array();
        v.uv = [v.uv[0] * 2.0 - 0.5, v.uv[1] * 2.0 - 0.5];
    }
}

/// The rain drops' probe (rain_lit.wgsl): its target ([`DROP_PROBE_SIZE`],
/// read by the drops through group 2 with the reflection's linear
/// sampler) and its vertices.
pub(in crate::render) struct DropProbe {
    view: wgpu::TextureView,
    group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
}

impl DropProbe {
    fn new(gpu: &Gpu, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler) -> DropProbe {
        let view = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("rain drop probe"),
                size: wgpu::Extent3d { width: DROP_PROBE_SIZE.0, height: DROP_PROBE_SIZE.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DROP_PROBE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let group = bind_texture(&gpu.device, layout, &view, sampler);
        let vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rain drop probe"),
            size: (MAX_PARTICLE_QUADS * 4 * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        DropProbe { view, group, vertices }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::native::world::pipelines::{clear_skip, DROP_BLOCKS, DROP_KEY};

    /// The probe block's pixel centres (a quarter and three quarters
    /// across the placed quad) are the drop's corners and UVs.
    #[test]
    fn probe_placement_puts_the_pixel_centres_on_the_corners() {
        let (c, a, b) = (glam::Vec3::new(3.0, -2.0, 7.5), glam::Vec3::new(0.03, 0.01, 0.0), glam::Vec3::new(0.1, -0.2, -0.7));
        let corners = [c + a + b, c - a + b, c - a - b, c + a - b];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut quad: Vec<Vertex> =
            (0..4).map(|i| Vertex { position: corners[i].to_array(), color: [1, 2, 3, 4], uv: uvs[i] }).collect();
        place_for_probe(&mut quad);
        let p = |i: usize| glam::Vec3::from(quad[i].position);
        // Bilinear over the placed quad (affine: a parallelogram) at the
        // block's pixel centres (u, v) in {0.25, 0.75}.
        let at = |u: f32, v: f32| p(0) + (p(1) - p(0)) * u + (p(3) - p(0)) * v;
        let uv_at = |u: f32, v: f32| {
            let (q0, q1, q3) = (quad[0].uv, quad[1].uv, quad[3].uv);
            [q0[0] + (q1[0] - q0[0]) * u + (q3[0] - q0[0]) * v, q0[1] + (q1[1] - q0[1]) * u + (q3[1] - q0[1]) * v]
        };
        for (i, (u, v)) in [(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)].into_iter().enumerate() {
            assert!(at(u, v).distance(corners[i]) < 1e-5, "corner {i}");
            assert_eq!(uv_at(u, v), uvs[i], "uv {i}");
        }
        assert!(quad.iter().all(|v| v.color == [1, 2, 3, 4]));
    }

    /// The shader's probe layout is the Rust side's.
    #[test]
    fn the_drop_probe_layout_matches_the_shader() {
        let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/weather/rain_lit.wgsl"));
        assert!(text.contains(&format!("const DROP_BLOCKS: u32 = {DROP_BLOCKS}u;")));
        assert!(text.contains(&format!("const DROP_WIDTH: f32 = {}.0;", DROP_PROBE_SIZE.0)));
        assert!(text.contains(&format!("const DROP_HALF: f32 = {}.0;", DROP_PROBE_SIZE.1 / 2)));
        assert_eq!(DROP_KEY.blend, crate::formats::texture::Blend::Alpha);
    }

    /// The alpha-0 entries: only alpha-blended keys without the alpha
    /// test, with the lamps' module, in the plain colour pipelines; never
    /// the native modules.
    #[test]
    fn clear_skip_takes_only_alpha_blended_keys_with_the_lamps() {
        use crate::formats::texture::Blend;
        use crate::render::native::world::draws::Effect;
        let lamps = SceneMode { lamps: true, ..SceneMode::NATIVE };
        let key = |blend, alpha_test| PipelineKey { effect: Effect::World, blend, alpha_test };
        assert!(clear_skip(key(Blend::Alpha, false), lamps, 0, false, false));
        assert!(!clear_skip(key(Blend::Alpha, false), SceneMode::NATIVE, 0, false, false));
        for (b, t) in [(Blend::None, false), (Blend::Additive, false), (Blend::DestAlpha, false), (Blend::Alpha, true)] {
            assert!(!clear_skip(key(b, t), lamps, 0, false, false), "{b:?} {t}");
        }
        assert!(!clear_skip(key(Blend::Alpha, false), lamps, 1, false, false));
        assert!(!clear_skip(key(Blend::Alpha, false), lamps, 0, true, false));
        assert!(!clear_skip(key(Blend::Alpha, false), lamps, 0, false, true));
    }
}
