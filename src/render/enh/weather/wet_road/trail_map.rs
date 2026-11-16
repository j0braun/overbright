use glam::{Mat4, Vec2, Vec3};

use super::lights::{CarExtra, ViewCull, car_boxes, gather_lights, match_extra};
use crate::render::Gpu;
use crate::render::enh::materials::world::puddle::smoothstep;
use crate::render::enh::weather::spray::CarSample;

/// Lights mirrored at most (On, High).
pub const MAX_LIGHTS: [usize; 2] = [16, 32];
/// Cars whose bodies block the puddles' mirror at most.
pub const MAX_BOXES: usize = 8;
pub(super) const LIGHT_SLOTS: usize = 32;
/// The trail map's texels a side and the world size it covers (m), On and
/// High: 0.125 m and 0.078 m a texel.
pub const MAP_TEXELS: [u32; 2] = [1024, 2048];
pub const MAP_SIZE: [f32; 2] = [128.0, 160.0];
const MAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The wake's half width (m) and its decay rate (1/s).
pub const WAKE_HALF_WIDTH: f32 = 1.4;
pub const WAKE_RATE: f32 = 1.6;
/// A tyre track's half width (m).
pub const TRACK_HALF_WIDTH: f32 = 0.15;

/// The track's refill rate (1/s): fast in heavy rain, slow on a drying
/// road. **Inferred**, by eye.
pub fn track_rate(rain: f32) -> f32 {
    // Twice to three times longer than at first (0.3 + 0.9·rain), so
    // the dry line of the cars ahead reads from the chase camera.
    0.12 + 0.36 * rain.clamp(0.0, 1.0)
}

/// The per-channel decay of the trail map over `dt` s.
pub fn decay_factors(rain: f32, dt: f32) -> [f32; 4] {
    let r = (-track_rate(rain) * dt).exp();
    let g = (-WAKE_RATE * dt).exp();
    [r, g, g, g]
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WetFxUniform {
    /// Centre x, y, size (m), on.
    window: [f32; 4],
    /// Track rate, wake rate, wake half width, no boxes (SSR draws the cars).
    params: [f32; 4],
    /// Lights, boxes, tier, 0.
    counts: [f32; 4],
    /// The cars' body colour in the mirror (rgb).
    ambient: [f32; 4],
    lights: [[f32; 4]; 3 * LIGHT_SLOTS],
    boxes: [[f32; 4]; 3 * MAX_BOXES],
}

const UNIFORM_SIZE: u64 = std::mem::size_of::<WetFxUniform>() as u64;

/// Bindings 12–14 of the world views' group 0.
pub(crate) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 3] {
    [
        wgpu::BindGroupLayoutEntry {
            binding: 12,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 13,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 14,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

/// What bindings 12–14 point at.
#[derive(Clone)]
pub(crate) struct WetBindings {
    uniform: wgpu::Buffer,
    map: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("wet road trails"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

fn map_texture(device: &wgpu::Device, texels: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("wet road trails"),
        size: wgpu::Extent3d { width: texels, height: texels, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MAP_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

impl WetBindings {
    /// Zeros (off) and a 1 × 1 map.
    pub(crate) fn placeholder(device: &wgpu::Device) -> WetBindings {
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("wet road (none)"),
                size: UNIFORM_SIZE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let map = map_texture(device, 1).create_view(&Default::default());
        WetBindings { uniform, map, sampler: sampler(device) }
    }

    pub(crate) fn entries(&self) -> [wgpu::BindGroupEntry<'_>; 3] {
        [
            wgpu::BindGroupEntry { binding: 12, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 13, resource: wgpu::BindingResource::TextureView(&self.map) },
            wgpu::BindGroupEntry { binding: 14, resource: wgpu::BindingResource::Sampler(&self.sampler) },
        ]
    }
}

// ---------------------------------------------------------------------
// The GPU half.

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MapUniform {
    /// New centre xy, old centre xy.
    centres: [f32; 4],
    /// Size (m), texels, 0, 0.
    size: [f32; 4],
    /// Decay factors.
    decay: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Stamp {
    /// Segment start, end (world xy).
    seg: [f32; 4],
    /// Track half width, wake half width, track strength, wake height.
    shape: [f32; 4],
    /// The torus copy's offset (uv), 0, 0.
    offset: [f32; 4],
}

const MAX_STAMPS: usize = 64 * 4 * 9;

/// The row's GPU state (made when the row is first on in a wet scene).
pub(crate) struct WetRoad {
    tier: u8,
    texels: u32,
    size: f32,
    pub(crate) bindings: WetBindings,
    map_view: wgpu::TextureView,
    map_uniform: wgpu::Buffer,
    stamps: wgpu::Buffer,
    map_group: wgpu::BindGroup,
    decay: wgpu::RenderPipeline,
    stamp: wgpu::RenderPipeline,
    /// The window's centre (texel-snapped) of the last commit applied,
    /// none before the first.
    centre: Option<Vec2>,
    /// The main view's group 0 without the lamps (their module adds these
    /// bindings to its own group).
    pub(crate) main_group: Option<wgpu::BindGroup>,
    /// The map holds a commit (else the next one clears it first).
    cleared: bool,
}

/// One tick's change to the trail map ("history" class: the map
/// accumulates; one commit per tick, applied in order by the next drawn
/// frame, [`WetRoad::apply`]): the window's new and old centre, the
/// decay over the tick's dt and the stamps of the cars' wheel paths since
/// the last tick.
#[derive(Clone, Debug)]
pub struct TrailCommit {
    centre: Vec2,
    old: Vec2,
    dt: f32,
    decay: [f32; 4],
    stamps: Vec<Stamp>,
    /// The first commit after a restart (a jump of the clock, the row or
    /// the wet road back): the map is cleared before it.
    restart: bool,
}

/// Commits kept for the next drawn frame at most (4 s of ticks: a
/// headless shot steps many before it draws; older trails have faded).
pub const MAX_COMMITS: usize = 240;

/// The trail map's CPU half, stepped on the tick
/// ([`TrailSim::commit`]): the clock and the window and the cars' wheel
/// contacts of the last tick; the commits not yet applied.
#[derive(Default)]
pub struct TrailSim {
    last_clock: Option<f32>,
    centre: Option<Vec2>,
    /// Last tick's contacts per car (to stamp the path between).
    last_contacts: Vec<(Vec3, [Vec3; 4])>,
    /// A commit since the last restart.
    cleared: bool,
    pub(crate) pending: std::collections::VecDeque<TrailCommit>,
}

impl TrailSim {
    /// The row off, a dry road or no scene at this tick: the next commit
    /// starts over (the GPU half is idle meanwhile).
    pub fn reset(&mut self) {
        self.last_clock = None;
        self.pending.clear();
    }

    /// One tick (`tier` the row's): the decay by the world clock's advance,
    /// the window moved with the eye, the cars' wheel paths since the last
    /// tick (matched by the predicted position), queued for the draw.
    pub fn commit(
        &mut self,
        tier: u8,
        eye: Vec3,
        scene: &crate::render::enh::materials::car::wet::Scene,
        cars: &[CarSample],
        extras: &[CarExtra],
    ) {
        let t = usize::from(tier.clamp(1, 2) - 1);
        let (size, texels) = (MAP_SIZE[t], MAP_TEXELS[t]);
        let texel = size / texels as f32;
        let centre = (eye.truncate() / texel).round() * texel;
        let dt = match self.last_clock {
            Some(c) if scene.clock > c && scene.clock - c < 0.5 => scene.clock - c,
            Some(c) if scene.clock == c => 0.0,
            _ => {
                self.last_contacts.clear();
                self.cleared = false;
                0.0
            }
        };
        self.last_clock = Some(scene.clock);
        let restart = !self.cleared;
        let old = if self.cleared { self.centre.unwrap_or(centre) } else { centre + Vec2::splat(size * 4.0) };
        self.centre = Some(centre);
        // Stamps: each car's wheels from last tick's contacts to these
        // (matched by the predicted position), nine torus copies each (the
        // neighbouring cells: a path across the map's edge wraps).
        let mut stamps = Vec::new();
        let mut contacts = Vec::with_capacity(cars.len());
        for car in cars {
            let at = car.world.w_axis.truncate();
            let speed = car.velocity.length();
            let prev =
                self.last_contacts.iter().filter(|(p, _)| (*p + car.velocity * dt).distance_squared(at) < 6.0).min_by(
                    |a, b| {
                        (a.0 + car.velocity * dt)
                            .distance_squared(at)
                            .total_cmp(&(b.0 + car.velocity * dt).distance_squared(at))
                    },
                );
            let extra = match_extra(car, extras);
            for i in 0..4 {
                if extra.is_some_and(|e| !e.wheels[i].2) {
                    continue;
                }
                let to = car.contacts[i].truncate();
                let from = prev.map_or(to, |p| p.1[i].truncate());
                if from.distance(to) > 8.0 {
                    continue;
                }
                let slip = extra.map_or(0.0, |e| (e.wheels[i].1.abs() / 6.0).min(1.0));
                let amp = smoothstep(1.0, 20.0, speed);
                let shape = [
                    TRACK_HALF_WIDTH * (1.0 + 0.6 * slip),
                    WAKE_HALF_WIDTH,
                    if speed > 0.3 || prev.is_none() { 1.0 } else { 0.0 },
                    amp,
                ];
                if shape[2] == 0.0 && amp == 0.0 {
                    continue;
                }
                let base = (from / size).floor();
                for k in 0..9 {
                    let o = Vec2::new((k % 3) as f32 - 1.0, (k / 3) as f32 - 1.0);
                    let a = from - base * size;
                    let b = to - base * size;
                    stamps.push(Stamp { seg: [a.x, a.y, b.x, b.y], shape, offset: [o.x, o.y, 0.0, 0.0] });
                }
            }
            contacts.push((at, car.contacts));
        }
        self.last_contacts = contacts;
        stamps.truncate(MAX_STAMPS);
        if crate::env_set!("NFSU2_WET_ROAD_DEBUG") {
            eprintln!(
                "wet road: clock {:.3} dt {dt:.4} cars {} extras {} stamps {} centre {centre:?}",
                scene.clock,
                cars.len(),
                extras.len(),
                stamps.len()
            );
        }
        self.cleared = true;
        self.pending.push_back(TrailCommit {
            centre,
            old,
            dt,
            decay: decay_factors(scene.rain, dt),
            stamps,
            restart,
        });
        // A long run of ticks with no draw: the oldest go, the map starts
        // over from the oldest kept (its trails had faded).
        while self.pending.len() > MAX_COMMITS {
            self.pending.pop_front();
            if let Some(c) = self.pending.front_mut() {
                c.restart = true;
            }
        }
    }

    /// A digest of the CPU half (the rate tests compare it at equal ticks).
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut eat = |v: f32| h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
        eat(self.last_clock.unwrap_or(-1.0));
        if let Some(c) = self.centre {
            eat(c.x);
            eat(c.y);
        }
        for (p, w) in &self.last_contacts {
            eat(p.x);
            eat(p.y);
            eat(p.z);
            for c in w {
                eat(c.x);
                eat(c.y);
            }
        }
        h
    }
}

impl WetRoad {
    pub(crate) fn new(gpu: &Gpu, tier: u8) -> WetRoad {
        let device = &gpu.device;
        let t = usize::from(tier.clamp(1, 2) - 1);
        let texels = MAP_TEXELS[t];
        let map = map_texture(device, texels);
        let map_view = map.create_view(&Default::default());
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("wet road"),
                size: UNIFORM_SIZE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let map_uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("wet road map"),
                size: std::mem::size_of::<MapUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let stamps = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("wet road stamps"),
                size: (MAX_STAMPS * std::mem::size_of::<Stamp>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("wet road map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let map_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wet road map"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: map_uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: stamps.as_entire_binding() },
            ],
        });
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("wet road map"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/wet_road.wgsl")).into(),
            ),
        });
        let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("wet road map"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |vs: &str, fs: &str, blend: wgpu::BlendState| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: MAP_FORMAT,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        // dst × the fragment's factors (per channel).
        let multiply = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::SrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let max = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Max,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Max,
            },
        };
        let decay = pipeline("vs_full", "fs_decay", multiply);
        let stamp = pipeline("vs_stamp", "fs_stamp", max);
        WetRoad {
            tier,
            texels,
            size: MAP_SIZE[t],
            bindings: WetBindings { uniform, map: map_view.clone(), sampler: sampler(device) },
            map_view,
            map_uniform,
            stamps,
            map_group,
            decay,
            stamp,
            centre: None,
            main_group: None,
            cleared: false,
        }
    }

    pub(crate) fn tier(&self) -> u8 {
        self.tier
    }

    /// The draw: one tick's commit to the map, in tick order
    /// (the decay, the window's move, the stamps); its own command buffer,
    /// submitted before the frame's.
    pub(crate) fn apply(&mut self, gpu: &Gpu, c: &TrailCommit) {
        let fresh = !self.cleared || c.restart;
        let moved = c.old != c.centre;
        let u = MapUniform {
            centres: [c.centre.x, c.centre.y, c.old.x, c.old.y],
            size: [self.size, self.texels as f32, 0.0, 0.0],
            decay: c.decay,
        };
        gpu.queue.write_buffer(&self.map_uniform, 0, bytemuck::bytes_of(&u));
        if !c.stamps.is_empty() {
            gpu.queue.write_buffer(&self.stamps, 0, bytemuck::cast_slice(&c.stamps));
        }
        let mut encoder =
            gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("wet road map") });
        {
            let load = if fresh { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wet road map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.map_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.map_group, &[]);
            if !fresh && (c.dt > 0.0 || moved) {
                pass.set_pipeline(&self.decay);
                pass.draw(0..3, 0..1);
            }
            if !c.stamps.is_empty() {
                pass.set_pipeline(&self.stamp);
                pass.draw(0..4, 0..c.stamps.len() as u32);
            }
        }
        crate::prof::gpu::mark(&mut encoder, "graphics: wet road map");
        gpu.queue.submit([encoder.finish()]);
        self.cleared = true;
        self.centre = Some(c.centre);
    }

    /// This frame: the road's values (the mirrored lights near the eye, the
    /// cars' boxes) over the map's window as the last commit left it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn road_values(
        &mut self,
        gpu: &Gpu,
        eye: Vec3,
        view: Option<ViewCull>,
        scene: &crate::render::enh::materials::car::wet::Scene,
        cars: &[CarSample],
        extras: &[CarExtra],
        ssr: bool,
        ambient: [f32; 3],
    ) {
        let texel = self.size / self.texels as f32;
        let centre = self.centre.unwrap_or_else(|| (eye.truncate() / texel).round() * texel);
        let lights = gather_lights(cars, extras, eye, view, MAX_LIGHTS[usize::from(self.tier.clamp(1, 2) - 1)]);
        let mut w = WetFxUniform {
            window: [centre.x, centre.y, self.size, 1.0],
            params: [track_rate(scene.rain), WAKE_RATE, WAKE_HALF_WIDTH, if ssr { 1.0 } else { 0.0 }],
            counts: [lights.len() as f32, 0.0, f32::from(self.tier), scene.clock],
            ambient: [
                ambient[0] * 0.3 + 0.015,
                ambient[1] * 0.3 + 0.015,
                ambient[2] * 0.3 + 0.018,
                if crate::env_set!("NFSU2_WET_ROAD_DEBUG") { 1.0 } else { 0.0 },
            ],
            lights: [[0.0; 4]; 3 * LIGHT_SLOTS],
            boxes: [[0.0; 4]; 3 * MAX_BOXES],
        };
        for (i, l) in lights.iter().enumerate() {
            w.lights[i * 3] = l[0];
            w.lights[i * 3 + 1] = l[1];
            w.lights[i * 3 + 2] = l[2];
        }
        let boxes = car_boxes(cars, eye);
        for (i, b) in boxes.iter().enumerate() {
            w.boxes[i * 3] = b[0];
            w.boxes[i * 3 + 1] = b[1];
            w.boxes[i * 3 + 2] = b[2];
        }
        w.counts[1] = boxes.len() as f32;
        gpu.queue.write_buffer(&self.bindings.uniform, 0, bytemuck::bytes_of(&w));
    }

    /// The road's branch off (a dry road, a pause in another scene): the
    /// uniform's `on` cleared.
    pub(crate) fn idle(&mut self, gpu: &Gpu) {
        gpu.queue.write_buffer(&self.bindings.uniform, 0, bytemuck::bytes_of(&[0.0f32; 4]));
    }
}

impl crate::render::WorldRenderer {
    /// Bindings 12–14 as they stand (the row's when it is on, else the
    /// placeholders).
    pub(crate) fn wet_bindings(&self) -> WetBindings {
        self.wet_road.as_ref().map_or_else(|| self.wet_placeholder.clone(), |w| w.bindings.clone())
    }

    /// Plan time: make (or remake for another tier) the row's state while
    /// it is on in a wet in-game scene and hand back the main view's group
    /// 0 when the lamps' module has none (else theirs, rebound once to the
    /// row's bindings).
    pub(crate) fn plan_wet_road(
        &mut self,
        gpu: &Gpu,
        lamp_frame: Option<wgpu::BindGroup>,
        opts: &crate::render::RenderOptions,
    ) -> Option<wgpu::BindGroup> {
        // The tier the trail map's commits were stepped with.
        let tier = self.fx_sim.stepped_or(opts).wet_interaction;
        let wet = self.weather.is_some_and(|w| crate::render::wet_uniform(w)[0] > 0.5);
        if tier == 0 {
            if self.wet_road.take().is_some() {
                let p = self.wet_placeholder.clone();
                self.rebind_lamps_wet(gpu, p);
            }
            return lamp_frame;
        }
        if !wet {
            return lamp_frame;
        }
        if self.wet_road.as_ref().is_none_or(|w| w.tier() != tier) {
            let w = WetRoad::new(gpu, tier);
            let b = w.bindings.clone();
            self.wet_road = Some(w);
            self.rebind_lamps_wet(gpu, b);
        }
        if lamp_frame.is_some() {
            // The lamps' main group (rebound above when it existed; made
            // since with `wet_bindings`).
            return self.lamp_main_group_with_wet(&gpu.device);
        }
        if self.wet_road.as_ref().unwrap().main_group.is_none() {
            let mut b = crate::render::lamps::LampBindings::placeholder(&gpu.device);
            b.wet = self.wet_road.as_ref().unwrap().bindings.clone();
            let g = self.frame_group(&gpu.device, &self.core.main_frame.0, &b);
            self.wet_road.as_mut().unwrap().main_group = Some(g);
        }
        self.wet_road.as_ref().unwrap().main_group.clone()
    }

    /// Particle time: the ticks' commits to the map since the last frame
    /// (stepped on the tick, [`TrailSim::commit`]) applied in
    /// order, then the road's values from the last tick's cars.
    pub(crate) fn prepare_wet_road(&mut self, gpu: &Gpu, cars: &[CarSample], extras: &[CarExtra]) {
        let ssr = self.wet_mask_drawn;
        let ambient = self.main_fog.1;
        let Some(w) = self.wet_road.as_mut() else {
            self.fx_sim.trail.pending.clear();
            return;
        };
        match (self.fx_view, crate::render::enh::materials::car::wet::scene()) {
            (Some(view), Some(scene)) if scene.road_wet > 1.0e-3 => {
                for c in self.fx_sim.trail.pending.drain(..) {
                    w.apply(gpu, &c);
                }
                let cam = &view.camera;
                let proj = cam.projection_depth(view.aspect, view.reversed);
                let cull = ViewCull {
                    view_proj: proj * Mat4::look_to_rh(cam.position, cam.forward(), cam.up),
                    scale_x: proj.x_axis.x,
                };
                w.road_values(gpu, cam.position, Some(cull), &scene, cars, extras, ssr, ambient)
            }
            _ => {
                self.fx_sim.trail.pending.clear();
                w.idle(gpu)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The track refills faster in the rain than on a drying road; the
    /// wake goes in ~2 s; a paused clock holds everything.
    #[test]
    fn trails_fade_over_seconds() {
        let [r, g, ..] = decay_factors(1.0, 1.0);
        assert!(r < 0.7 && r > 0.55, "{r}");
        assert!(g < 0.25, "{g}");
        let [dry, ..] = decay_factors(0.0, 1.0);
        assert!(dry > 0.85, "{dry}");
        // Half-life in heavy rain ≈ 1.4 s, on a drying road ≈ 5.8 s.
        let half = |rain: f32| 0.5f32.ln() / -track_rate(rain);
        assert!((half(1.0) - 1.44).abs() < 0.05 && (half(0.0) - 5.78).abs() < 0.05, "{} {}", half(1.0), half(0.0));
        assert_eq!(decay_factors(1.0, 0.0), [1.0; 4]);
    }

    /// Group 0's bindings stay apart: the lamps 3–8, Surface Detail 9–10,
    /// Modern Lighting's traced scene 11 (Modern + RT's layout only), this row 12–14.
    #[test]
    fn frame_bindings_do_not_collide() {
        let mut all: Vec<u32> = crate::render::lamps::layout_entries().iter().map(|e| e.binding).collect();
        all.extend(crate::render::surface::layout_entries().iter().map(|e| e.binding));
        all.push(crate::render::modern_light_bold::rt_layout_entry().binding);
        all.extend(layout_entries().iter().map(|e| e.binding));
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len(), "{all:?}");
        assert_eq!(layout_entries().map(|e| e.binding), [12, 13, 14]);
        let wgsl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/wet_road_world.wgsl"));
        for b in 12..=14 {
            assert!(wgsl.contains(&format!("@group(0) @binding({b})")));
        }
    }

    /// The world module holds the row's code outside the lamps' blocks (both
    /// modules) and the uniform matches its WGSL layout.
    #[test]
    fn world_module_carries_the_branch() {
        for lamps in [false, true] {
            let s = crate::render::world_source(lamps);
            assert!(s.contains("@group(0) @binding(12) var<uniform> wet_fx: WetFx;"));
            assert!(s.contains("fn wet_interact("));
        }
        assert_eq!(UNIFORM_SIZE, 16 * (4 + 3 * LIGHT_SLOTS + 3 * MAX_BOXES) as u64);
        let wgsl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/wet_road_world.wgsl"));
        assert!(wgsl.contains(&format!("lights: array<vec4<f32>, {}>", 3 * LIGHT_SLOTS)));
        assert!(wgsl.contains(&format!("boxes: array<vec4<f32>, {}>", 3 * MAX_BOXES)));
    }
}
