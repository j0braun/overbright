//! One frame's queued cars in one upload (moved from car.rs).

use super::*;

impl CarRenderer {
    /// Queue the car for [`CarRenderer::record_queued`]: as
    /// [`CarRenderer::render`], but the draws go into a pass shared with
    /// other cars ([`begin_pass`]), their values into `batch` (one upload
    /// for all the cars, [`CarBatch::upload`]). Drawing the cars one after
    /// another in one pass leaves the same image as a pass each; each
    /// draw reads the same values, from another buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn queue(
        &mut self,
        gpu: &Gpu,
        batch: &mut CarBatch,
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
        self.split_glass(lighting, world, view, view_proj, opts);
        let mode = CarMode::current(batch.samples, self.wet_now.is_some(), opts);
        if mode != CarMode::NATIVE {
            self.sync_scene(gpu, mode);
        }
        let frame = frame_uniform(view_proj, view);
        // The batch has one frame's values: a car seen another way (never
        // so in the game's frame) keeps its own buffers.
        if batch.frame.is_some_and(|f| bytemuck::bytes_of(&f) != bytemuck::bytes_of(&frame)) {
            self.write_uniforms(gpu, view_proj, view, lighting, world, wheels, opts);
            self.queued.set(Some((lighting.is_frontend(), None)));
            return;
        }
        let _t = crate::prof::acc::span("cars: uniforms");
        batch.frame = Some(frame);
        if self.batch_frame_group.is_none() {
            let cube = self
                .environment
                .as_ref()
                .unwrap_or_else(|| self.shared.placeholder.get().expect("the placeholder cube"));
            self.batch_frame_group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("car frame (batch)"),
                layout: &self.shared.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: batch.frame_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(cube) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.env_sampler) },
                ],
            }));
        }
        let base = batch.draws.len() as u32;
        // LOD Fade's pipelines are taken (from the shared cache, built by
        // the first car drawn with the option on) whenever the option is
        // on, not at the first fade: a compile there was a hitch mid-drive.
        if opts.lod_fade && self.fade_pipelines.as_ref().is_none_or(|p| p.0 != mode) {
            self.fade_pipelines = Some((mode, self.shared.fade_pipelines_n(&gpu.device, self.color_format, mode)));
        }
        // LOD Fade: the new LOD's draws at its share, then the old LOD's at
        // the complement (negative: the other half of the pattern).
        if let Some((from, t)) = self.drawing_fade {
            if self.fade_pipelines.as_ref().is_none_or(|p| p.0 != mode) {
                self.fade_pipelines = Some((mode, self.shared.fade_pipelines_n(&gpu.device, self.color_format, mode)));
            }
            let samples = batch.samples as f32;
            if let Some(lod) = self.lod {
                self.draw_uniforms_at(lod, [t, samples], lighting, world, wheels, &mut batch.draws, opts);
            }
            let old = batch.draws.len() as u32;
            if let Some(lod) = from {
                self.draw_uniforms_at(lod, [-(1.0 - t), samples], lighting, world, wheels, &mut batch.draws, opts);
            }
            self.queued.set(Some((lighting.is_frontend(), Some(base))));
            self.queued_fade.set(Some((from, old)));
            return;
        }
        self.draw_uniforms(lighting, world, wheels, &mut batch.draws, opts);
        self.queued.set(Some((lighting.is_frontend(), Some(base))));
    }
}

/// A frame's queued cars' values in one buffer ([`CarRenderer::queue`]):
/// the frame's (the same camera for every car) and every draw's, uploaded
/// once ([`CarBatch::upload`]) instead of two buffer writes per car.
pub struct CarBatch {
    frame: Option<FrameUniform>,
    pub(super) draws: Vec<DrawUniform>,
    frame_buffer: wgpu::Buffer,
    draw_buffer: Option<wgpu::Buffer>,
    pub(crate) draw_group: Option<wgpu::BindGroup>,
    /// The scene's sample count (the anti-aliasing).
    pub samples: u32,
}

impl CarBatch {
    pub fn new(gpu: &Gpu) -> CarBatch {
        // The cars' start cube (made with the first renderer
        // otherwise), for frame groups made before any cube is set.
        gpu.car_shared().placeholder.get_or_init(|| start_envmap(gpu));
        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("car frame (batch)"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        CarBatch { frame: None, draws: Vec::new(), frame_buffer, draw_buffer: None, draw_group: None, samples: 1 }
    }

    /// Writes the queued values (with `belt`, as copies recorded in
    /// `encoder` ahead of the cars' pass) and starts the next batch.
    pub fn upload(
        &mut self,
        gpu: &Gpu,
        mut belt: Option<&mut wgpu::util::StagingBelt>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let bytes: &[u8] = bytemuck::cast_slice(&self.draws);
        let size = (bytes.len() as u64).max(DRAW_STRIDE);
        if self.draw_buffer.as_ref().is_none_or(|b| b.size() < size) {
            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("car draws (batch)"),
                size: size.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.draw_group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("car draw (batch)"),
                layout: &gpu.car_shared().draw_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            }));
            self.draw_buffer = Some(buffer);
        }
        if let Some(frame) = self.frame.take() {
            crate::render::upload(gpu, belt.as_deref_mut(), encoder, &self.frame_buffer, 0, bytemuck::bytes_of(&frame));
        }
        crate::render::upload(gpu, belt, encoder, self.draw_buffer.as_ref().unwrap(), 0, bytes);
        self.draws.clear();
    }
}
