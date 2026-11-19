use super::*;

type PipelineKey = (wgpu::TextureFormat, u32, bool);

struct Volume {
    fx: VolumeFx,
    pool: ParticlePool,
    /// Built this frame (by the first draw).
    pending: bool,
    drawn: bool,
}

/// Ambient Detail's state for one world: made ahead of a drive (the menus'
/// preload, a load's warm-up: `AmbientWarm::run`) or at the first frame the
/// row is on ([`crate::render::WorldRenderer::ensure_ambient`]), and kept
/// while the world gives the same props ([`AmbientDetail::made_for`]).
pub struct AmbientDetail {
    /// What the props were made from ([`AmbientDetail::made_for`]).
    made_from: props::SurfaceInputs,
    props: Vec<Placed>,
    /// The props by 32 m cell (only looked up: the fast hasher).
    cells: crate::fxhash::FxHashMap<(i32, i32), Vec<u32>>,
    ground: props::Ground,
    vents: Vec<Vec3>,
    loose: std::collections::HashMap<u32, Loose>,
    grit: Vec<Grit>,
    last_time: Option<f32>,
    last_cars: (Vec<CarIn>, f32),
    /// Drawn frames (the stats' lines only).
    frame: u64,
    last_emit: f32,
    /// The ticks stepped (the generator's seed, the first step's
    /// warm-up), the cars the last tick took (the draw's lights), the glow
    /// pass's list for the next tick, the volume alive, and the volume's
    /// steps not yet handed to its pool (a drawn frame hands them over).
    steps: u64,
    cars: Vec<CarIn>,
    next_cars: Vec<CarIn>,
    live: bool,
    pending_steps: Vec<(f32, Vec<Emit>, Fields)>,
    /// A running digest of the steps queued (the rate tests).
    queued: u64,
    // GPU.
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    ranges: [(u32, u32, i32); props::MESHES],
    instances: wgpu::Buffer,
    pipelines: std::collections::HashMap<PipelineKey, (wgpu::RenderPipeline, wgpu::RenderPipeline)>,
    volume: std::sync::Mutex<Option<Volume>>,
    /// Instances drawn last frame (stats, tests).
    pub drawn: usize,
}

const CELL: f32 = 32.0;

/// The props placed so far in this process ([`AmbientDetail::new`]; the
/// tests: whether a load or a frame placed them).
static PLACED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many times props were placed in this process.
pub fn placements_made() -> usize {
    PLACED.load(std::sync::atomic::Ordering::Relaxed)
}

fn cell_of(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

impl AmbientDetail {
    pub fn new(gpu: &Gpu, world: &crate::world::World) -> AmbientDetail {
        let t = std::time::Instant::now();
        PLACED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut uses = props::SlotUses::default();
        let s = props::surfaces_with(world, &mut uses);
        let made_from = props::SurfaceInputs::of(world, &mut uses);
        let placement = props::place(&s);
        let mut vents: Vec<Vec3> = placement.drains.iter().filter(|d| d.1).map(|d| d.0).collect();
        vents.extend(props::grill_vents(&s.grills));
        let ground = props::Ground::new(&s);
        let mut cells: crate::fxhash::FxHashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, p) in placement.props.iter().enumerate() {
            cells.entry(cell_of(Vec3::from_slice(&p.inst.pos_s[..3]))).or_default().push(i as u32);
        }
        log::info!(
            "ambient detail: {} props, {} vents, {} ground triangles in {:.0} ms",
            placement.props.len(),
            vents.len(),
            ground.len(),
            t.elapsed().as_secs_f32() * 1000.0
        );
        if crate::render::debug_env::nfsu2_ambient_stats() {
            eprintln!(
                "ambient detail: {} props, {} drains ({} steam), {} vents, {} ground tris, {} placements of use, {:.0} ms",
                placement.props.len(),
                placement.drains.len(),
                placement.drains.iter().filter(|d| d.1).count(),
                vents.len(),
                ground.len(),
                made_from.placements_of_use(),
                t.elapsed().as_secs_f32() * 1000.0
            );
        }
        let device = &gpu.device;
        use wgpu::util::DeviceExt;
        let (mv, mi, ranges) = props::meshes();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ambient meshes"),
            contents: bytemuck::cast_slice(&mv),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut mi = mi;
        if mi.len() % 2 == 1 {
            mi.push(0);
        }
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ambient mesh indices"),
            contents: bytemuck::cast_slice(&mi),
            usage: wgpu::BufferUsages::INDEX,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ambient instances"),
            size: (MAX_INSTANCES * std::mem::size_of::<Inst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("ambient detail"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/ambient_detail.wgsl")).into(),
            ),
        });
        let bgl = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("ambient detail"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("ambient detail"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ambient detail"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ambient detail"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        AmbientDetail {
            made_from,
            props: placement.props,
            cells,
            ground,
            vents,
            loose: Default::default(),
            grit: Vec::new(),
            last_time: None,
            last_cars: (Vec::new(), 0.0),
            frame: 0,
            last_emit: f32::MIN,
            steps: 0,
            cars: Vec::new(),
            next_cars: Vec::new(),
            live: false,
            pending_steps: Vec::new(),
            queued: 0,
            shader,
            layout,
            uniform,
            group,
            vertices,
            indices,
            ranges,
            instances,
            pipelines: Default::default(),
            volume: std::sync::Mutex::new(None),
            drawn: 0,
        }
    }

    /// `world` (in its state now) gives the props these were made from:
    /// the placements that hold their surfaces are shown, hidden, placed
    /// and sectioned as then ([`props::SurfaceInputs`]). Made again, they
    /// would be the same list.
    pub fn made_for(&self, world: &crate::world::World) -> bool {
        self.made_from.same(world)
    }

    /// The props at rest, in placement order (the tests).
    pub fn props(&self) -> &[Placed] {
        &self.props
    }

    /// What was placed, as one digest (the tests): the props at rest (their
    /// instances, kinds and sections, in order), the steam vents and the
    /// ground the debris lands on. Placed again for a world that gives
    /// other surfaces, at least one of them differs.
    pub fn placement_digest(&self) -> u64 {
        let fold = |h: u64, w: u32| (h ^ u64::from(w)).wrapping_mul(0x100_0000_01b3);
        let mut h = fold(0xcbf2_9ce4_8422_2325, self.props.len() as u32);
        for p in &self.props {
            for &b in bytemuck::bytes_of(&p.inst) {
                h = fold(h, u32::from(b));
            }
            h = fold(fold(h, p.kind as u32), p.section);
        }
        for v in &self.vents {
            h = v.to_array().into_iter().fold(h, |h, c| fold(h, c.to_bits()));
        }
        self.ground.digest(fold(h, self.vents.len() as u32))
    }

    /// The props' pipelines for a main view of `format` × `samples`
    /// (`reversed`), made now: a drive's warm-up (docs/developing/first-use-builds.md).
    pub fn warm(&mut self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, reversed: bool) {
        self.pipelines(gpu, (format, samples, reversed));
    }

    fn pipelines(&mut self, gpu: &Gpu, key: PipelineKey) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
        if let Some(p) = self.pipelines.get(&key) {
            return p.clone();
        }
        let (format, samples, reversed) = key;
        let vattrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let iattrs = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<props::Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vattrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Inst>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &iattrs,
            }),
        ];
        let compare = if reversed { wgpu::CompareFunction::GreaterEqual } else { wgpu::CompareFunction::LessEqual };
        // The colour target's alpha (the bleach-bypass mask) is kept.
        let keep_alpha = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let make = |label: &str, fs: &str, blend: wgpu::BlendComponent, write: bool, a2c: bool| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState { color: blend, alpha: keep_alpha }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::render::DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    alpha_to_coverage_enabled: a2c,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let replace = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        };
        let over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let a2c = samples > 1;
        let solid = make("ambient props", if a2c { "fs_solid_a2c" } else { "fs_solid" }, replace, true, a2c);
        let decal = make("ambient decals", "fs_decal", over, false, false);
        self.pipelines.insert(key, (solid.clone(), decal.clone()));
        (solid, decal)
    }

    /// The tick: the loose debris and grit stepped by the tick's
    /// advance of the clock, and (Next-Gen) the volume's emits and step
    /// queued for its pool; the generator seeded by the tick count (the
    /// draw's frame count before: rate-dependent). The cars are the glow
    /// pass's list one tick late, as the draw's push took them a frame
    /// late before.
    pub fn tick(&mut self, t: &AmbientTick) {
        let tier = t.tier;
        if tier == 0 {
            return;
        }
        let next_gen = tier >= 2;
        self.steps += 1;
        let dt = self.last_time.map_or(0.0, |l| (t.time - l).clamp(0.0, 0.1));
        self.last_time = Some(t.time);
        let time = t.time;
        let eye = t.eye;
        // The cars (the last list for a moment when a tick had none).
        let mut cars = std::mem::replace(&mut self.next_cars, t.cars.clone());
        if cars.is_empty() && time - self.last_cars.1 < 0.25 {
            cars = self.last_cars.0.clone();
        } else if !cars.is_empty() {
            self.last_cars = (cars.clone(), time);
        }
        let mut rng = props::Rng(props::hash(self.steps ^ 0x414D_4249));
        // Next-Gen: the loose debris and grit.
        if next_gen && dt > 0.0 {
            self.simulate(&cars, eye, time, dt, &mut rng);
        }
        if next_gen {
            let emits = self.emits(&cars, eye, t.wet, time, dt, &mut rng);
            if crate::render::debug_env::nfsu2_ambient_stats_two() {
                eprintln!("ambient detail: step {} t {time} dt {dt} emits {}", self.steps, emits.len());
            }
            if !emits.is_empty() {
                self.last_emit = time;
            }
            self.live = time - self.last_emit < 7.0 || self.steps == 1;
            if self.live {
                let mut near: Vec<&CarIn> =
                    cars.iter().filter(|c| c.world.w_axis.truncate().distance(eye) < 60.0).collect();
                near.sort_by(|a, b| {
                    a.world
                        .w_axis
                        .truncate()
                        .distance_squared(eye)
                        .total_cmp(&b.world.w_axis.truncate().distance_squared(eye))
                });
                let fields = Fields {
                    wind: wind(eye, time),
                    turbulence: 1.4,
                    drag: 0.9,
                    buoyancy: 1.6,
                    cooling: 0.35,
                    glow: 0.0,
                    cars: near.iter().take(8).map(|c| (c.world.w_axis.truncate(), c.velocity)).collect(),
                };
                // The first step: the vents have been steaming for a
                // while already (4.5 s in 15 steps, no cars).
                if self.steps == 1 {
                    for k in 0..15 {
                        let at = time - (15 - k) as f32 * 0.3;
                        let warm = self.emits(&[], eye, t.wet, at, 0.3, &mut rng);
                        if k == 0 && crate::render::debug_env::nfsu2_ambient_stats() {
                            eprintln!("ambient detail: warm-up step emits {}", warm.len());
                        }
                        self.pending_steps.push((
                            0.3,
                            warm,
                            Fields { cars: Vec::new(), wind: wind(eye, at), ..fields.clone() },
                        ));
                    }
                }
                self.queued = emits.iter().fold(
                    (self.queued ^ u64::from(dt.to_bits()) ^ emits.len() as u64).wrapping_mul(0x100_0000_01b3),
                    |h, e| (h ^ u64::from(e.pos_life[0].to_bits()) ^ u64::from(e.vel_r[2].to_bits()) << 32).wrapping_mul(0x100_0000_01b3),
                );
                self.pending_steps.push((dt.max(1e-4), emits, fields));
                // Many ticks before a draw (a headless shot): the oldest
                // steps merged pairwise (their emits spawned together, one
                // update of their summed dt), so the pool's queue keeps them.
                while self.pending_steps.len() > volume_fx::MAX_STEPS {
                    let (dt1, mut e1, _) = self.pending_steps.remove(0);
                    let (dt2, e2, f2) = self.pending_steps.remove(0);
                    e1.extend(e2);
                    e1.truncate(volume_fx::MAX_EMITS);
                    self.pending_steps.insert(0, (dt1 + dt2, e1, f2));
                }
            } else {
                self.pending_steps.clear();
            }
        }
        self.cars = cars;
    }

    /// A digest of the CPU half (the rate tests compare it at equal ticks).
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64 ^ self.steps;
        let mut eat = |v: f32| h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
        let mut loose: Vec<(&u32, &Loose)> = self.loose.iter().collect();
        loose.sort_by_key(|l| *l.0);
        for (i, l) in loose {
            eat(*i as f32);
            eat(l.pos.x);
            eat(l.pos.y);
            eat(l.pos.z);
        }
        for g in &self.grit {
            eat(g.pos.x);
            eat(g.pos.y);
            eat(g.pos.z);
        }
        h ^ self.queued.rotate_left(7)
    }

    /// The frame's instances, uniform and volume input from the state the
    /// last tick left (nothing stepped here); the world pass's
    /// draw (None: nothing to draw).
    pub fn prepare(&mut self, gpu: &Gpu, f: &Frame) -> Option<DrawPlan> {
        let tier = f.tier;
        if tier == 0 {
            return None;
        }
        let next_gen = tier >= 2;
        self.frame += 1;
        let time = f.time;
        let eye = f.camera.position;
        let fwd = f.camera.forward();
        let cars = self.cars.clone();
        let lamps = crate::render::volumetric::nearest_lamps(f.lamps, eye, fwd, 16);
        let lights = scene_lights(&lamps, &cars, eye, f.modern);

        // The instances near the eye.
        let planes = frustum_planes(f.camera.view_projection(f.aspect));
        let visible = |p: Vec3, r: f32| planes.iter().all(|pl| pl.truncate().dot(p) + pl.w > -r);
        let mut by_mesh: Vec<Vec<Inst>> = vec![Vec::new(); props::MESHES * 2];
        let (cx, cy) = cell_of(eye);
        let reach = (FADE.1 / CELL).ceil() as i32;
        for x in cx - reach..=cx + reach {
            for y in cy - reach..=cy + reach {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    let p = &self.props[i as usize];
                    if !f.visible.get(p.section as usize).copied().unwrap_or(false) {
                        continue;
                    }
                    let mut inst = p.inst;
                    if let Some(l) = next_gen.then(|| self.loose.get(&i)).flatten() {
                        inst.pos_s = [l.pos.x, l.pos.y, l.pos.z, inst.pos_s[3]];
                        inst.rot = l.rot.to_array();
                    }
                    let at = Vec3::from_slice(&inst.pos_s[..3]);
                    let d = at.distance(eye);
                    if d > FADE.1 || !visible(at, 0.6) {
                        continue;
                    }
                    by_mesh[p.kind.mesh() as usize * 2 + usize::from(p.kind.decal())].push(inst);
                }
            }
        }
        if next_gen {
            for g in &self.grit {
                if g.pos.distance(eye) < 30.0 && visible(g.pos, 0.1) {
                    let fade = 1.0 - smooth(1.0, 1.5, g.age);
                    by_mesh[Mesh::Crumple as usize * 2].push(Inst {
                        pos_s: [g.pos.x, g.pos.y, g.pos.z, 0.13 * fade],
                        rot: g.rot.to_array(),
                        light_kind: [g.light.x, g.light.y, g.light.z, Kind::Grit as u32 as f32],
                        extra: [g.seed, 1.0, 0.6, 1.0],
                    });
                }
            }
            moths(&lamps, eye, time, &mut by_mesh[Mesh::Quad as usize * 2 + 1]);
        }
        let mut all: Vec<Inst> = Vec::new();
        let mut runs = Vec::new();
        for (k, list) in by_mesh.iter().enumerate() {
            let room = MAX_INSTANCES - all.len();
            let n = list.len().min(room);
            if n == 0 {
                continue;
            }
            let first = all.len() as u32;
            all.extend_from_slice(&list[..n]);
            runs.push((self.ranges[k / 2], first..first + n as u32, k % 2 == 1));
        }
        self.drawn = all.len();
        if self.frame == 1 && crate::render::debug_env::nfsu2_ambient_stats() {
            let mut v: Vec<&Vec3> = self.vents.iter().collect();
            v.sort_by(|a, b| a.distance(eye).total_cmp(&b.distance(eye)));
            eprintln!("ambient detail: nearest vents {:?}", v.iter().take(4).map(|p| p.to_array()).collect::<Vec<_>>());
            eprintln!(
                "ambient detail: nearest lamps {:?}",
                lamps.iter().take(4).map(|l| l.0.to_array()).collect::<Vec<_>>()
            );
        }
        if self.frame % 60 == 1 && crate::render::debug_env::nfsu2_ambient_stats() {
            eprintln!(
                "ambient detail: frame {} drew {} instances ({} runs), {} moved ({} flying), {} grit, {} cars, eye {eye}",
                self.frame,
                all.len(),
                runs.len(),
                self.loose.len(),
                self.loose.values().filter(|l| l.flying).count(),
                self.grit.len(),
                cars.len()
            );
        }
        if !all.is_empty() {
            gpu.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&all));
        }
        // The uniform.
        let rot = Mat4::look_to_rh(Vec3::ZERO, fwd, f.camera.up);
        let view_proj = f.camera.projection_depth(f.aspect, f.reversed) * rot;
        let right = f.camera.right();
        let up = right.cross(fwd).normalize_or(Vec3::Z);
        let mut u = Uniform {
            view_proj: view_proj.to_cols_array_2d(),
            eye: eye.extend(time).to_array(),
            right: right.extend(FADE.0).to_array(),
            up: up.extend(FADE.1).to_array(),
            forward: fwd.extend(f.wet.clamp(0.0, 1.0)).to_array(),
            fog_value: f.fog.map_or([0.0; 4], |(v, _)| [v[0], v[1], v[2], 1.0]),
            fog_color: f.fog.map_or([0.0; 4], |(_, c)| [c[0], c[1], c[2], 0.0]),
            counts: [0, 0, 0, u32::from(f.modern)],
            lamps: [[0.0; 4]; 32],
            spots: [[0.0; 4]; 24],
            points: [[0.0; 4]; 16],
        };
        if crate::render::debug_env::nfsu2_ambient_debug() {
            u.fog_color[3] = 1.0;
        }
        for (i, (p, r, c)) in lights.lamps.iter().take(16).enumerate() {
            u.lamps[i * 2] = p.extend(*r).to_array();
            u.lamps[i * 2 + 1] = c.extend(0.0).to_array();
            u.counts[0] += 1;
        }
        for (i, s) in lights.spots.iter().take(8).enumerate() {
            u.spots[i * 3] = s.position.extend(s.reach).to_array();
            u.spots[i * 3 + 1] = s.direction.extend(s.cos_outer).to_array();
            u.spots[i * 3 + 2] = s.colour.extend(s.cos_inner).to_array();
            u.counts[1] += 1;
        }
        for (i, (p, r, c)) in lights.points.iter().filter(|l| l.1 > 0.0).take(8).enumerate() {
            u.points[i * 2] = p.extend(*r).to_array();
            u.points[i * 2 + 1] = c.extend(0.0).to_array();
            u.counts[2] += 1;
        }
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));

        // Next-Gen: the volume's frame, its pool fed the ticks' steps.
        if next_gen {
            let mut vol = self.volume.lock().unwrap();
            // The first frame makes and builds the volume whatever the
            // ticks left (its pipelines then, not at a later frame's first
            // steam: a hitch mid-drive).
            if self.live || self.frame == 1 {
                let v = vol.get_or_insert_with(|| {
                    let look = volume_fx::Look { scene: 2.2, ..volume_fx::LOOK };
                    let mut fx = VolumeFx::new(gpu, BUDGET, look);
                    // Thin dust and steam: one step coarser than the smoke.
                    fx.set_coarser(1);
                    Volume { fx, pool: ParticlePool::new(gpu, 4096), pending: false, drawn: false }
                });
                for (dt, emits, fields) in self.pending_steps.drain(..) {
                    v.pool.step(dt, emits, fields);
                }
                v.fx.frame(
                    gpu,
                    volume_fx::View {
                        eye,
                        forward: fwd,
                        view_proj,
                        near: f.camera.near,
                        far: f.camera.far,
                        reversed: f.reversed,
                        time,
                        fog: f.fog,
                    },
                    &lights,
                );
                v.pending = true;
                v.drawn = false;
            } else if let Some(v) = vol.as_mut() {
                v.pending = false;
                v.drawn = true;
            }
        }

        let (solid, decal) = self.pipelines(gpu, (f.format, f.samples, f.reversed));
        Some(DrawPlan {
            solid,
            decal,
            group: self.group.clone(),
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            instances: self.instances.clone(),
            runs,
        })
    }

    /// Next-Gen's volume: built at the first call of a frame, then marched
    /// and composited over `target` (as `volume_fx::VolumeFx::draw`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_volume(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
        tier: u8,
    ) {
        if tier < 2 {
            return;
        }
        let mut vol = self.volume.lock().unwrap();
        let Some(v) = vol.as_mut() else { return };
        if v.pending {
            let n = v.pool.export(gpu, encoder, &v.fx, 0);
            if crate::render::debug_env::nfsu2_ambient_stats() {
                eprintln!("ambient detail: volume built from {n} pool slots");
            }
            v.fx.build(gpu, encoder, n);
            v.pending = false;
        } else if v.drawn {
            return;
        }
        v.drawn = true;
        v.fx.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
        crate::prof::gpu::mark(encoder, "graphics: ambient volume");
    }

    /// The loose debris and grit (Next-Gen), by `dt` in steps ≤ 1/60 s.
    fn simulate(&mut self, cars: &[CarIn], eye: Vec3, time: f32, dt: f32, rng: &mut props::Rng) {
        let steps = (dt * 60.0).ceil().clamp(1.0, 6.0) as u32;
        let h = dt / steps as f32;
        let movers: Vec<(Vec3, Vec3)> = cars
            .iter()
            .filter(|c| c.velocity.length() > 3.0 && c.world.w_axis.truncate().distance(eye) < SIM_RADIUS + 15.0)
            .map(|c| (c.world.w_axis.truncate(), c.velocity))
            .collect();
        let air_at = |p: Vec3, t: f32| -> Vec3 {
            wind(p, t)
                + movers
                    .iter()
                    .filter(|(c, _)| c.distance_squared(p) < 15.0 * 15.0)
                    .map(|(c, v)| car_air(p, *c, *v))
                    .sum::<Vec3>()
        };
        let (cx, cy) = cell_of(eye);
        let reach = (SIM_RADIUS / CELL).ceil() as i32;
        let mut ids = Vec::new();
        for x in cx - reach..=cx + reach {
            for y in cy - reach..=cy + reach {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    let p = &self.props[i as usize];
                    if p.kind.loose() {
                        ids.push(i);
                    }
                }
            }
        }
        for i in ids {
            let p = self.props[i as usize];
            let seed = p.inst.extra[0];
            let origin = Vec3::from_slice(&p.inst.pos_s[..3]);
            let mut l = match self.loose.get(&i) {
                Some(l) => *l,
                None => {
                    if origin.distance(eye) > SIM_RADIUS {
                        continue;
                    }
                    let air = air_at(origin, time);
                    if Vec3::new(air.x, air.y, 0.0).length() <= threshold(p.kind, seed) {
                        continue;
                    }
                    Loose {
                        pos: origin,
                        vel: Vec3::ZERO,
                        rot: Quat::from_array(p.inst.rot),
                        spin: Vec3::ZERO,
                        flying: false,
                        calm: 0.0,
                    }
                }
            };
            if l.pos.distance(eye) > SIM_RADIUS + 10.0 && !l.flying {
                continue;
            }
            let n0 = (Quat::from_array(p.inst.rot) * Vec3::Z).normalize_or(Vec3::Z);
            let mut moved = false;
            for k in 0..steps {
                let t = time - dt + h * (k + 1) as f32;
                let air = air_at(l.pos, t);
                let ground = self.ground.at(l.pos, 0.3);
                // The rest plane through its place, if no surface is known.
                let rest = (origin.z - (n0.x * (l.pos.x - origin.x) + n0.y * (l.pos.y - origin.y)) / n0.z.max(0.5), n0);
                moved |= step_loose(&mut l, p.kind, seed, air, t, h, ground, rest);
            }
            if moved || self.loose.contains_key(&i) {
                self.loose.insert(i, l);
            }
        }
        // Too many moved: the farthest settled ones go back to their place
        // (out of sight).
        if self.loose.len() > MAX_LOOSE {
            let mut far: Vec<(f32, u32)> =
                self.loose.iter().filter(|(_, l)| !l.flying).map(|(i, l)| (l.pos.distance(eye), *i)).collect();
            far.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (d, i) in far.into_iter().take(self.loose.len() - MAX_LOOSE) {
                if d > 50.0 {
                    self.loose.remove(&i);
                }
            }
        }
        // Grit: flicked up by spinning or fast tyres, more on gravel.
        for c in cars {
            let at = c.world.w_axis.truncate();
            let speed = c.velocity.length();
            if at.distance(eye) > 30.0 || speed < 4.0 {
                continue;
            }
            for (w, slip, on) in c.wheels {
                if !on || self.grit.len() >= MAX_GRIT {
                    continue;
                }
                let loose = self.ground.at(w + Vec3::Z * 0.2, 0.5).is_some_and(|g| g.2);
                let rate = (slip.abs() - 0.3).max(0.0) * 25.0 + if loose { speed * 0.8 } else { 0.0 };
                let mut n = (rate * dt).floor() as u32;
                if rng.f() < (rate * dt).fract() {
                    n += 1;
                }
                for _ in 0..n {
                    let dir = c.velocity / speed;
                    let side = Vec3::new(-dir.y, dir.x, 0.0);
                    let vel = c.velocity * 0.35 - dir * speed * 0.12
                        + Vec3::Z * (1.5 + 2.5 * rng.f())
                        + side * (rng.f() - 0.5) * 2.0;
                    let light = Vec3::splat(0.35);
                    self.grit.push(Grit {
                        pos: w + Vec3::Z * 0.05,
                        vel,
                        rot: Quat::from_euler(glam::EulerRot::XYZ, rng.f() * 6.3, rng.f() * 6.3, rng.f() * 6.3),
                        age: 0.0,
                        seed: rng.f(),
                        light,
                    });
                }
            }
        }
        let ground = &self.ground;
        self.grit.retain_mut(|g| {
            g.age += dt;
            g.vel += Vec3::Z * -9.81 * dt;
            g.vel *= (-0.3 * dt).exp();
            g.pos += g.vel * dt;
            if let Some((z, _, _)) = ground.at(g.pos, 0.5) {
                if g.pos.z < z + 0.005 {
                    g.pos.z = z + 0.005;
                    g.vel.z = -g.vel.z * 0.3;
                    g.vel.x *= 0.6;
                    g.vel.y *= 0.6;
                }
            }
            g.age < 1.5
        });
    }

    /// The volume's emits this frame: tyre dust on dry ground, steam from
    /// the nearest vents.
    fn emits(&self, cars: &[CarIn], eye: Vec3, wet: f32, time: f32, dt: f32, rng: &mut props::Rng) -> Vec<Emit> {
        let mut out = Vec::new();
        if dt <= 0.0 {
            return out;
        }
        let count = |rate: f32, rng: &mut props::Rng| -> u32 {
            let x = rate * dt;
            x.floor() as u32 + u32::from(rng.f() < x.fract())
        };
        let dry = (1.0 - wet / 0.15).clamp(0.0, 1.0);
        if dry > 0.0 {
            for c in cars {
                let speed = c.velocity.length();
                if c.world.w_axis.truncate().distance(eye) > 35.0 || speed < 5.0 {
                    continue;
                }
                let dir = c.velocity / speed;
                for (w, slip, on) in c.wheels {
                    if !on {
                        continue;
                    }
                    let loose = self.ground.at(w + Vec3::Z * 0.2, 0.5).is_some_and(|g| g.2);
                    let base = ((speed - 4.0) / 25.0).clamp(0.0, 1.0) * if loose { 30.0 } else { 10.0 };
                    let rate = dry * (base + (slip.abs() - 0.15).max(0.0) * 12.0);
                    let opacity = if loose { 0.25 } else { 0.14 };
                    for _ in 0..count(rate, rng) {
                        let r = 0.28 + 0.1 * rng.f();
                        let jitter = Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() * 0.5);
                        out.push(Emit {
                            pos_life: (w + Vec3::Z * 0.15 - dir * 0.35).extend(1.6 + 0.8 * rng.f()).to_array(),
                            vel_r: (c.velocity * 0.25 + jitter * 0.8 + Vec3::Z * 0.4).extend(r).to_array(),
                            albedo_sigma: [0.62, 0.57, 0.50, volume_fx::sigma_for_opacity(opacity, r)],
                            heat: [0.03, 0.7, 0.7, 0.3],
                        });
                    }
                }
            }
        }
        let mut vents: Vec<(f32, Vec3)> =
            self.vents.iter().map(|v| (v.distance(eye), *v)).filter(|(d, _)| *d < 45.0).collect();
        vents.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (k, (_, v)) in vents.into_iter().take(MAX_VENTS).enumerate() {
            // Each vent breathes (its own slow pulse).
            let pulse = 0.6 + 0.4 * noise1(time * 0.3, props::hash(k as u64 ^ v.x.to_bits() as u64));
            for _ in 0..count(10.0 * pulse, rng) {
                let r = 0.3 + 0.06 * rng.f();
                let at = v + Vec3::new(rng.f() - 0.5, rng.f() - 0.5, 0.0) * 0.5 + Vec3::Z * 0.1;
                out.push(Emit {
                    pos_life: at.extend(4.0 + 2.0 * rng.f()).to_array(),
                    vel_r: Vec3::new((rng.f() - 0.5) * 0.3, (rng.f() - 0.5) * 0.3, 0.5 + 0.3 * rng.f())
                        .extend(r)
                        .to_array(),
                    albedo_sigma: [0.9, 0.9, 0.9, volume_fx::sigma_for_opacity(0.8 * pulse, r)],
                    heat: [0.35, 0.4, 0.5, 0.45],
                });
            }
        }
        out.truncate(volume_fx::MAX_EMITS);
        out
    }

    /// Loose props currently moved off their places (tests, stats).
    pub fn moved(&self) -> usize {
        self.loose.len()
    }
}

/// The lamps (nearest 16), each car's headlight cone, tail lights (brighter
/// braking) for the props and the volume.
fn scene_lights(lamps: &[(Vec3, f32, Vec3)], cars: &[CarIn], eye: Vec3, modern: bool) -> SceneLights {
    let mut l = SceneLights { lamps: lamps.to_vec(), modern, ..Default::default() };
    let mut near: Vec<&CarIn> = cars.iter().filter(|c| c.world.w_axis.truncate().distance(eye) < 80.0).collect();
    near.sort_by(|a, b| {
        a.world.w_axis.truncate().distance_squared(eye).total_cmp(&b.world.w_axis.truncate().distance_squared(eye))
    });
    for c in near.into_iter().take(8) {
        let m = c.world;
        let at = m.w_axis.truncate();
        let forward = m.x_axis.truncate().normalize_or(Vec3::X);
        let up = m.z_axis.truncate().normalize_or(Vec3::Z);
        let col = Vec3::from(c.headlight.map(|b| (f32::from(b) / 128.0).min(1.0)));
        l.spots.push(Spot {
            position: at + forward * 2.0 + up * 0.7,
            reach: 28.0,
            direction: (forward - Vec3::Z * 0.1).normalize_or(forward),
            cos_outer: 0.82,
            cos_inner: 0.94,
            colour: col,
        });
        let tail = crate::render::gpu_particles::TAIL_COLOUR * if c.braking { 2.5 } else { 1.0 };
        for t in [Vec3::new(-2.1, 0.6, 0.85), Vec3::new(-2.1, -0.6, 0.85)] {
            l.points.push((m.transform_point3(t), crate::render::gpu_particles::TAIL_RADIUS, tail));
        }
    }
    l
}

/// Moths round the nearest lamps within 28 m (Next-Gen): an erratic orbit
/// near the light, seeded per lamp.
fn moths(lamps: &[(Vec3, f32, Vec3)], eye: Vec3, time: f32, out: &mut Vec<Inst>) {
    for (k, (p, _, c)) in lamps.iter().filter(|l| l.0.distance(eye) < 28.0).take(6).enumerate() {
        let key = props::hash((p.x.to_bits() as u64) << 32 ^ p.y.to_bits() as u64);
        let mut rng = props::Rng(key);
        let lum = c.max_element().max(1e-3);
        for j in 0..7 {
            let (s0, s1, s2) = (rng.f(), rng.f(), rng.f());
            let w = 1.6 + 2.4 * s0;
            let a = time * w + s1 * std::f32::consts::TAU;
            let r = 0.35 + 0.55 * noise1(time * 1.3 + s2 * 50.0, key ^ j);
            let z = -0.25 + 0.5 * noise1(time * 2.1 + s0 * 30.0, key ^ (j + 99));
            let jitter = Vec3::new((time * 23.0 + s2 * 9.0).sin(), (time * 19.0 + s1 * 7.0).cos(), 0.0) * 0.05;
            let at = *p + Vec3::new(a.cos() * r, a.sin() * r, z) + jitter;
            let col = *c / lum;
            out.push(Inst {
                pos_s: [at.x, at.y, at.z, 0.05],
                rot: [0.0, 0.0, 0.0, 1.0],
                light_kind: [col.x, col.y, col.z, Kind::Moth as u32 as f32],
                extra: [s0 + k as f32 * 0.13, 1.0, 1.0, 1.0],
            });
        }
    }
}

/// The view's side and near planes (xyz inward normal, w offset) of a
/// world view-projection (0..1 depth).
pub(super) fn frustum_planes(m: Mat4) -> [Vec4; 5] {
    let r = m.transpose();
    let (r0, r1, r2, r3) = (r.x_axis, r.y_axis, r.z_axis, r.w_axis);
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2].map(|p| p / p.truncate().length().max(1e-6))
}
