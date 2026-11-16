use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct VolUniform {
    pub(super) origin: [f32; 4],
    pub(super) dims: [u32; 4],
    pub(super) eye: [f32; 4],
    pub(super) ambient: [f32; 4],
    pub(super) look2: [f32; 4],
    pub(super) inv_vp: [[f32; 4]; 4],
    pub(super) forward: [f32; 4],
    pub(super) viewport: [f32; 4],
    pub(super) sizes: [f32; 4],
    pub(super) depth: [f32; 4],
    pub(super) look3: [f32; 4],
    pub(super) wind: [f32; 4],
    pub(super) vp: [[f32; 4]; 4],
    pub(super) look4: [f32; 4],
    pub(super) fog_value: [f32; 4],
    pub(super) fog_color: [f32; 4],
    // scene ambient gain, near fade (m), shadow strength, scene ambient read (0/1)
    pub(super) look5: [f32; 4],
    // The temporal resolve (volume_fx.wgsl `Vol`)
    pub(super) prev_vp: [[f32; 4]; 4],
    pub(super) prev_eye: [f32; 4],
    pub(super) prev_view: [f32; 4],
    pub(super) temporal: [f32; 4],
    // March step scale, jitter share (+1; 0 = defaults)
    pub(super) look6: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LightsUniform {
    pub(super) counts: [u32; 4],
    pub(super) lamps: [[f32; 4]; 2 * MAX_LAMPS],
    spots: [[f32; 4]; 3 * MAX_SPOTS],
    points: [[f32; 4]; 2 * MAX_POINTS],
}

/// The lights eye-relative, the nearest first and at most the maxima.
pub(super) fn lights_uniform(l: &SceneLights, eye: Vec3) -> LightsUniform {
    let mut u: LightsUniform = bytemuck::Zeroable::zeroed();
    let near = |v: &[Lamp], max: usize| {
        let mut v: Vec<Lamp> = v.to_vec();
        v.sort_by(|a, b| a.0.distance_squared(eye).total_cmp(&b.0.distance_squared(eye)));
        v.truncate(max);
        v
    };
    let lamps = near(&l.lamps, MAX_LAMPS);
    for (i, (p, r, c)) in lamps.iter().enumerate() {
        u.lamps[i * 2] = (*p - eye).extend(*r).to_array();
        u.lamps[i * 2 + 1] = c.extend(0.0).to_array();
    }
    let mut spots = l.spots.clone();
    spots.sort_by(|a, b| a.position.distance_squared(eye).total_cmp(&b.position.distance_squared(eye)));
    spots.truncate(MAX_SPOTS);
    for (i, s) in spots.iter().enumerate() {
        u.spots[i * 3] = (s.position - eye).extend(s.reach).to_array();
        u.spots[i * 3 + 1] = s.direction.extend(s.cos_outer).to_array();
        u.spots[i * 3 + 2] = s.colour.extend(s.cos_inner).to_array();
    }
    let points = near(&l.points, MAX_POINTS);
    for (i, (p, r, c)) in points.iter().enumerate() {
        u.points[i * 2] = (*p - eye).extend(*r).to_array();
        u.points[i * 2 + 1] = c.extend(0.0).to_array();
    }
    u.counts = [lamps.len() as u32, spots.len() as u32, points.len() as u32, u32::from(l.modern)];
    u
}

pub(super) const CH: u64 = 9;
pub(super) const BRICK: u32 = 8;
/// The bounds buffer's u32s (this frame's, the last frame's particle
/// voxel bounds; the occupied bricks' bounds).
pub(super) const BOUNDS: usize = 18;
pub(super) const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
// (view depth, the medium's shadow on the surface 0..1)
// (b the medium's mean distance along the ray, the temporal
// resolve's reprojection depth.)
pub(super) const LDEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
// The light volume's dominant light direction.
pub(super) const LDIR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Snorm;

pub(super) fn empty_bounds() -> [u32; BOUNDS] {
    let mut b = [0u32; BOUNDS];
    for k in [0, 1, 2, 6, 7, 8, 12, 13, 14] {
        b[k] = u32::MAX;
    }
    b
}

/// The shader's sections, each with the common head: pool, splat,
/// resolve, light, march, and the downsample + composite (which take the
/// depth type, DEPTH_TEXTURE).
pub(super) fn sections() -> [String; 7] {
    let source = crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::VolumeFx);
    let (common, rest) = source.split_once("// ---- pool ----").expect("pool");
    let (pool, rest) = rest.split_once("// ---- splat ----").expect("splat");
    let (splat, rest) = rest.split_once("// ---- resolve ----").expect("resolve");
    let (resolve, rest) = rest.split_once("// ---- light ----").expect("light");
    let (light, rest) = rest.split_once("// ---- march ----").expect("march");
    let (march, rest) = rest.split_once("// ---- temporal ----").expect("temporal");
    let (temporal, full) = rest.split_once("// The nearest view depth").expect("full");
    [
        format!("{common}{pool}"),
        format!("{common}{splat}"),
        format!("{common}{resolve}"),
        format!("{common}{light}"),
        format!("{common}{march}"),
        format!("{common}// The nearest view depth{full}"),
        format!("{common}{temporal}"),
    ]
}

pub(super) fn storage(binding: u32, read_only: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(super) fn uniform(binding: u32, visibility: wgpu::ShaderStages, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(super) fn tex(
    binding: u32,
    visibility: wgpu::ShaderStages,
    sample_type: wgpu::TextureSampleType,
    view_dimension: wgpu::TextureViewDimension,
    multisampled: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture { sample_type, view_dimension, multisampled },
        count: None,
    }
}

pub(super) fn samp(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

pub(super) fn store(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D3,
        },
        count: None,
    }
}

pub(super) fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

pub(super) fn module(gpu: &Gpu, label: &str, src: String) -> wgpu::ShaderModule {
    crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    })
}

pub(super) fn compute_pipelines(
    gpu: &Gpu,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    m: &wgpu::ShaderModule,
    entries: &[&str],
) -> Vec<wgpu::ComputePipeline> {
    let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    entries
        .iter()
        .map(|e| {
            crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
                label: Some(e),
                layout: Some(&pl),
                module: m,
                entry_point: Some(e),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            })
        })
        .collect()
}

/// The noise volume the splat, the march and the pool use: the tiling
/// Perlin–Worley billow noise with its gradient (`smoke_cinematic`'s).
pub(super) fn noise_view(gpu: &Gpu) -> wgpu::TextureView {
    let n = crate::render::smoke_cinematic::NOISE as u32;
    let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vfx noise"),
        size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &t,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        crate::render::smoke_cinematic::noise_volume(),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(n) },
        wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
    );
    t.create_view(&Default::default())
}

pub(super) fn repeat_sampler(gpu: &Gpu) -> wgpu::Sampler {
    let r = wgpu::AddressMode::Repeat;
    gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: r,
        address_mode_v: r,
        address_mode_w: r,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// `NFSU2_VFX_DEBUG=1`: the march shows coverage (red), the peak density
/// (green) and the steps (blue). Diagnostics only.
pub(super) fn debug_mode() -> f32 {
    static D: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *D.get_or_init(|| crate::render::debug_env::nfsu2_vfx_debug().unwrap_or(0.0))
}
