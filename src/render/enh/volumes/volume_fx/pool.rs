use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FieldsUniform {
    step: [f32; 4],
    counts: [u32; 4],
    wind: [f32; 4],
    phys: [f32; 4],
    cars: [[f32; 4]; 16],
}

pub(super) const FIELDS_STRIDE: u64 = 512;
/// Steps a [`ParticlePool`] takes between two exports at most.
pub const MAX_STEPS: usize = 16;

/// A GPU particle ring (module docs): spawn [`Emit`]s, advance them by
/// [`Fields`], export them into a [`VolumeFx`]'s particle buffer.
pub struct ParticlePool {
    capacity: u32,
    sims: wgpu::Buffer,
    emits: wgpu::Buffer,
    fields: wgpu::Buffer,
    export_at: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    spawn: wgpu::ComputePipeline,
    update: wgpu::ComputePipeline,
    export: wgpu::ComputePipeline,
    noise: wgpu::TextureView,
    sampler: wgpu::Sampler,
    head: u32,
    pending: Vec<(Vec<Emit>, f32, Fields)>,
    time: f32,
}

/// Emits a pool takes a step at most.
pub const MAX_EMITS: usize = 8192;

impl ParticlePool {
    pub fn new(gpu: &Gpu, capacity: u32) -> ParticlePool {
        let device = &gpu.device;
        use wgpu::BufferUsages as U;
        let buffer = |label, size: u64, usage| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false },
            )
        };
        let sims = buffer("vfx pool", capacity as u64 * 96, U::STORAGE | U::COPY_DST);
        gpu.queue.write_buffer(&sims, 0, &vec![0u8; capacity as usize * 96]);
        let emits = buffer("vfx emits", (MAX_EMITS * MAX_STEPS * 64) as u64, U::STORAGE | U::COPY_DST);
        let fields = buffer("vfx fields", MAX_STEPS as u64 * FIELDS_STRIDE, U::UNIFORM | U::COPY_DST);
        let export_at = buffer("vfx export", 16, U::UNIFORM | U::COPY_DST);
        let cs = wgpu::ShaderStages::COMPUTE;
        let f3 = wgpu::TextureSampleType::Float { filterable: true };
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("vfx pool"),
            entries: &[
                uniform(0, cs, true),
                storage(1, false, cs),
                storage(2, true, cs),
                storage(3, false, cs),
                tex(4, cs, f3, wgpu::TextureViewDimension::D3, false),
                samp(5, cs),
                uniform(6, cs, false),
            ],
        });
        let [pool_src, ..] = sections();
        let mut p = compute_pipelines(
            gpu,
            "vfx pool",
            &layout,
            &module(gpu, "vfx pool", pool_src),
            &["cs_pool_spawn", "cs_pool_update", "cs_pool_export"],
        );
        let export = p.pop().unwrap();
        let update = p.pop().unwrap();
        let spawn = p.pop().unwrap();
        ParticlePool {
            capacity,
            sims,
            emits,
            fields,
            export_at,
            layout,
            spawn,
            update,
            export,
            noise: noise_view(gpu),
            sampler: repeat_sampler(gpu),
            head: 0,
            pending: Vec::new(),
            time: 0.0,
        }
    }

    /// Queue one simulation step of `dt` seconds with this step's `emits`
    /// (run by the next [`ParticlePool::export`]; the oldest steps beyond
    /// [`MAX_STEPS`] are dropped).
    pub fn step(&mut self, dt: f32, emits: Vec<Emit>, fields: Fields) {
        self.pending.push((emits, dt, fields));
        if self.pending.len() > MAX_STEPS {
            self.pending.remove(0);
        }
    }

    /// Run the queued steps, then write all `capacity` particles into
    /// `fx`'s buffer from `offset` (dead ones with radius 0). Returns the
    /// particles written (give `offset + this` to [`VolumeFx::build`]).
    pub fn export(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, fx: &VolumeFx, offset: u32) -> u32 {
        let n = self.capacity.min(fx.budget.particles.saturating_sub(offset));
        let steps = std::mem::take(&mut self.pending);
        let mut uniforms = vec![0u8; steps.len().max(1) * FIELDS_STRIDE as usize];
        let mut plan = Vec::new();
        for (i, (emits, dt, f)) in steps.iter().enumerate() {
            self.time += dt;
            let count = emits.len().min(MAX_EMITS);
            if count > 0 {
                gpu.queue.write_buffer(&self.emits, (i * MAX_EMITS * 64) as u64, bytemuck::cast_slice(&emits[..count]));
            }
            let mut cars = [[0.0f32; 4]; 16];
            for (k, (p, v)) in f.cars.iter().take(8).enumerate() {
                cars[k * 2] = p.extend(1.0).to_array();
                cars[k * 2 + 1] = v.extend(0.0).to_array();
            }
            let u = FieldsUniform {
                step: [*dt, self.time, count as f32, self.capacity as f32],
                counts: [self.head, count as u32, f.cars.len().min(8) as u32, 0],
                wind: f.wind.extend(f.turbulence).to_array(),
                phys: [f.drag, f.buoyancy, f.cooling, f.glow],
                cars,
            };
            let at = i * FIELDS_STRIDE as usize;
            uniforms[at..at + std::mem::size_of::<FieldsUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
            self.head = (self.head + count as u32) % self.capacity;
            plan.push((i, count as u32));
        }
        gpu.queue.write_buffer(&self.fields, 0, &uniforms);
        gpu.queue.write_buffer(&self.export_at, 0, bytemuck::cast_slice(&[offset, 0, 0, 0]));
        let mut pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("vfx pool"), timestamp_writes: None });
        let group = |i: usize| {
            let e = |binding, resource| wgpu::BindGroupEntry { binding, resource };
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vfx pool"),
                layout: &self.layout,
                entries: &[
                    e(
                        0,
                        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.fields,
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<FieldsUniform>() as u64),
                        }),
                    ),
                    e(1, self.sims.as_entire_binding()),
                    e(
                        2,
                        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.emits,
                            offset: (i * MAX_EMITS * 64) as u64,
                            size: wgpu::BufferSize::new((MAX_EMITS * 64) as u64),
                        }),
                    ),
                    e(3, fx.particles.as_entire_binding()),
                    e(4, wgpu::BindingResource::TextureView(&self.noise)),
                    e(5, wgpu::BindingResource::Sampler(&self.sampler)),
                    e(6, self.export_at.as_entire_binding()),
                ],
            })
        };
        // Emit slices need their own groups (the emits' offset); the fields
        // ride the dynamic offset.
        let groups: Vec<wgpu::BindGroup> = (0..steps.len().max(1)).map(group).collect();
        for &(i, count) in &plan {
            pass.set_bind_group(0, &groups[i], &[(i as u64 * FIELDS_STRIDE) as u32]);
            if count > 0 {
                pass.set_pipeline(&self.spawn);
                pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
            }
            pass.set_pipeline(&self.update);
            pass.dispatch_workgroups(self.capacity.div_ceil(64), 1, 1);
        }
        if n > 0 {
            let last = plan.last().map_or(0, |p| p.0);
            pass.set_bind_group(0, &groups[last], &[(last as u64 * FIELDS_STRIDE) as u32]);
            pass.set_pipeline(&self.export);
            pass.dispatch_workgroups(n.div_ceil(64), 1, 1);
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: volume fx pool");
        n
    }
}
