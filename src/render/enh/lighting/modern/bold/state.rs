use super::*;
// ---------------------------------------------------------------------
// The cars lit by the same lights.

/// The lights of the last frame (as uploaded, tagged), for the cars.
static FRAME_LIGHTS: RwLock<Vec<GpuLamp>> = RwLock::new(Vec::new());
/// The last frame's params (None: Modern Lighting off).
pub(in crate::render) static FRAME_PARAMS: RwLock<Option<Params>> = RwLock::new(None);

/// Keep this frame's lights and params for [`car_lamps`].
pub fn publish(lights: &[GpuLamp], params: Option<Params>) {
    let mut l = FRAME_LIGHTS.write().unwrap();
    l.clear();
    if params.is_some() {
        l.extend_from_slice(lights);
    }
    *FRAME_PARAMS.write().unwrap() = params;
}

/// The physical pool of [`super::modern_light::pool_soft`] for a light of
/// kind `kind` (colour.w as tagged): a pack lamp the fitted one at
/// softening `soft`, the others 1 at their source falling as the inverse
/// square softened by (outer / 4)² (world.wgsl `ml_light_pool`).
pub fn light_pool(rel: Vec3, n: Vec3, outer: f32, kind: f32, soft: f32) -> f32 {
    if kind >= 0.0 {
        return crate::render::modern_light::pool_soft(rel, n, outer, soft);
    }
    let d2 = rel.length_squared();
    if outer <= 0.0 || d2 >= outer * outer {
        return 0.0;
    }
    let d = d2.sqrt().max(1e-3);
    let cos = (n.dot(rel) / d).max(0.0);
    let s = (outer * 0.25).powi(2);
    let x = d2 / (outer * outer);
    let window = (1.0 - x * x).max(0.0);
    let t = ((d - NEAR_FADE[0]) / (NEAR_FADE[1] - NEAR_FADE[0])).clamp(0.0, 1.0);
    s / (d2 + s) * cos * window * window * t * t * (3.0 - 2.0 * t)
}

/// world.wgsl `ML_NEAR_FADE`: a light of its own fades in over these
/// distances (m), so a sign does not light its own facade.
pub const NEAR_FADE: [f32; 2] = [1.0, 3.5];

/// The four strongest of the last frame's lights at a car at `at`, as
/// car.wgsl's `Draw.lamps` in Modern Lighting's form: (position, outer)
/// and (colour × the boost, −the car's ambient factor) — a negative w
/// selects car.wgsl's Modern path. None: Modern Lighting off.
pub fn car_lamps(at: Vec3) -> Option<[[f32; 4]; 8]> {
    let p = (*FRAME_PARAMS.read().unwrap())?;
    let lights = FRAME_LIGHTS.read().unwrap();
    let mut best: Vec<(f32, &GpuLamp)> = lights
        .iter()
        .filter_map(|l| {
            let c = Vec3::new(l.shadow[1], l.shadow[2], l.shadow[3]);
            if c.max_element() <= 0.0 {
                return None;
            }
            let pos = Vec3::new(l.position[0], l.position[1], l.position[2]);
            // Its own lights do not light the car.
            if l.colour[3] < -1.5 && pos.distance(at) < 3.5 {
                return None;
            }
            let rel = pos - at;
            let w = light_pool(rel, rel.normalize_or_zero(), l.position[3], l.colour[3], p.soft)
                * c.dot(Vec3::splat(1.0 / 3.0));
            (w > 1e-3).then_some((w, l))
        })
        .collect();
    best.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = [[0.0; 4]; 8];
    // The flag even without a light: lamps[0] far away and dark.
    out[0] = [at.x, at.y, at.z + 1000.0, 1e-3];
    out[1] = [0.0, 0.0, 0.0, -p.car_ambient];
    for (i, (_, l)) in best.iter().take(4).enumerate() {
        let gain = if l.colour[3] >= 0.0 { p.boost } else { p.emit };
        out[i * 2] = [l.position[0], l.position[1], l.position[2], l.position[3]];
        // The kind in the colour's sign is lost: the soft goes in x's
        // place below (car.wgsl reads the pack softening from lamps[7]).
        out[i * 2 + 1] = [
            l.shadow[1] * gain,
            l.shadow[2] * gain,
            l.shadow[3] * gain,
            -p.car_ambient - if l.colour[3] >= 0.0 { 0.0 } else { 10.0 },
        ];
    }
    Some(out)
}

// ---------------------------------------------------------------------
// Modern + RT: the frame's ray-query scene.

static FRAME_RT: Mutex<Option<crate::render::rt::FrameRt>> = Mutex::new(None);

/// Give this frame's traced scene (`rt::frame`) to the world's main view.
pub fn set_frame_rt(rt: Option<crate::render::rt::FrameRt>) {
    *FRAME_RT.lock().unwrap() = rt;
}

/// The traced scene of the frame, if any.
pub fn frame_rt() -> Option<crate::render::rt::FrameRt> {
    FRAME_RT.lock().unwrap().clone()
}

/// Whether the frame should build a traced scene for Modern + RT.
pub fn wants_rt(opts: &crate::render::RenderOptions) -> bool {
    tier(opts) == Tier::Rt
}

/// The scene's group 0 layout with Modern + RT's scene at binding 11.
pub fn rt_layout_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 11,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
        count: None,
    }
}

/// An empty, built scene to bind while no traced frame exists yet.
pub fn placeholder_tlas(gpu: &crate::render::Gpu) -> wgpu::Tlas {
    static T: Mutex<Option<wgpu::Tlas>> = Mutex::new(None);
    let mut guard = T.lock().unwrap();
    guard
        .get_or_insert_with(|| {
            let device = &gpu.device;
            let tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
                label: Some("modern lighting empty tlas"),
                max_instances: 1,
                flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: wgpu::AccelerationStructureUpdateMode::Build,
            });
            let mut enc = device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("modern lighting empty tlas") });
            enc.build_acceleration_structures(std::iter::empty(), [&tlas]);
            gpu.queue.submit([enc.finish()]);
            tlas
        })
        .clone()
}

/// The cars' Modern Lighting module (moved, S4).
pub use crate::render::car_shader_variants::modern_car_source as car_source;

/// The scene's Modern Lighting module tier ([`super::SceneMode::modern`]):
/// 0 off, 1 Modern / Low, 2 Modern + RT.
pub fn scene_modern(opts: &crate::render::RenderOptions) -> u8 {
    match tier(opts) {
        Tier::Off => 0,
        Tier::Low | Tier::Modern => 1,
        Tier::Rt => 2,
    }
}

/// The world's group 0 layout for Modern + RT (the frame layout and the
/// traced scene at binding 11).
pub fn frame_layout_rt(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    crate::render::frame_layout_with(device, &[rt_layout_entry(), crate::render::rt_shadow::layout_entry()])
}

impl crate::render::WorldRenderer {
    /// Modern + RT: the main view's group 0 with the frame's traced scene
    /// (an empty one until the first traced frame).
    pub(in crate::render) fn frame_group_rt(
        &self,
        gpu: &crate::render::Gpu,
        buffer: &wgpu::Buffer,
        lamps: &crate::render::lamps::LampBindings,
    ) -> wgpu::BindGroup {
        let device = &gpu.device;
        static LAYOUT: Mutex<Option<wgpu::BindGroupLayout>> = Mutex::new(None);
        let layout = LAYOUT.lock().unwrap().get_or_insert_with(|| frame_layout_rt(device)).clone();
        let tlas = frame_rt().map_or_else(|| placeholder_tlas(gpu), |f| f.tlas);
        let vis =
            self.rt_shadow.as_ref().map_or_else(|| crate::render::rt_shadow::placeholder(gpu), |r| r.vis().clone());
        let [e3, e4, e5, e6, e7, e8, e9, e10, e12, e13, e14] = lamps.entries();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame (modern lighting rt)"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.headlight.0) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.headlight.1) },
                e3,
                e4,
                e5,
                e6,
                e7,
                e8,
                e9,
                e10,
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::AccelerationStructure(&tlas) },
                // 15: the half-resolution pass's traced blocks (an empty 1 × 1 target until the
                // first traced frame).
                wgpu::BindGroupEntry { binding: 15, resource: wgpu::BindingResource::TextureView(&vis) },
                // 12–14: Wet Road Interaction.
                e12,
                e13,
                e14,
            ],
        })
    }
}
