#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TickUniform {
    pub(super) dt: [f32; 4],
    pub(super) counts: [u32; 4],
    pub(super) tier: [f32; 4],
    pub(super) wind: [f32; 4],
    pub(super) haze: [f32; 4],
    pub(super) ncars: [u32; 4],
    pub(super) cars: [[f32; 4]; 16],
}

pub(super) const TICK_STRIDE: u64 = 512;
pub(super) const MAX_LAMPS: usize = 16;
pub(super) const MAX_SPOTS: usize = 4;
pub(super) const MAX_POINTS: usize = 16;
pub(super) const PUFF_SIZE: u64 = 80;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FrameUniform {
    pub(super) view_proj: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) right: [f32; 4],
    pub(super) up: [f32; 4],
    pub(super) forward: [f32; 4],
    pub(super) counts: [u32; 4],
    pub(super) params: [f32; 4],
    pub(super) sizes: [f32; 4],
    pub(super) params2: [f32; 4],
    pub(super) lamps: [[f32; 4]; 2 * MAX_LAMPS],
    pub(super) spots: [[f32; 4]; 3 * MAX_SPOTS],
    pub(super) points: [[f32; 4]; 2 * MAX_POINTS],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FullUniform {
    pub(super) depth: [f32; 4],
    pub(super) sizes: [f32; 4],
    pub(super) debug: [f32; 4],
}

/// `NFSU2_SMOKE2_DEBUG`: 1 the composite shows depths and coverage, 2 the
/// puffs drawn flat (diagnostics only).
pub(super) fn debug_mode() -> f32 {
    static D: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *D.get_or_init(|| crate::render::debug_env::nfsu2_smoke2_debug().unwrap_or(0.0))
}

/// The low-res targets for one scene size.
pub(super) struct Targets {
    pub(super) size: (u32, u32),
    pub(super) div: u32,
    pub(super) depth: wgpu::TextureView,
    pub(super) accum: wgpu::TextureView,
    pub(super) reveal: wgpu::TextureView,
}

pub(super) const ACCUM: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(super) const REVEAL: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
pub(super) const LDEPTH: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

pub(super) const DESC_SIZE: u64 = 112;
pub(super) const COLOR_SIZE: u64 = 64;
pub(super) const MAX_DESCS: usize = 512;
pub(super) const MAX_COLORS: usize = 1024;
pub(super) const TEX: u32 = 128;

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

pub(super) fn uniform(binding: u32, dynamic: bool, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
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

pub(super) fn tex_entry(
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

pub(super) fn samp_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Bilinear resample of an RGBA8 image to TEX × TEX.
pub(super) fn resample(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (TEX * TEX * 4) as usize];
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return out;
    }
    let px = |x: i32, y: i32, c: usize| {
        rgba[((y.clamp(0, h as i32 - 1) as u32 * w + x.clamp(0, w as i32 - 1) as u32) * 4) as usize + c] as f32
    };
    for y in 0..TEX {
        for x in 0..TEX {
            let fx = (x as f32 + 0.5) * w as f32 / TEX as f32 - 0.5;
            let fy = (y as f32 + 0.5) * h as f32 / TEX as f32 - 0.5;
            let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
            let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - ax) + px(x0 + 1, y0, c) * ax;
                let bottom = px(x0, y0 + 1, c) * (1.0 - ax) + px(x0 + 1, y0 + 1, c) * ax;
                out[((y * TEX + x) * 4) as usize + c] =
                    (top * (1.0 - ay) + bottom * ay).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// The shader's sections: common, compute, puff draw, full-screen.
pub(super) fn sections() -> (String, String, String) {
    let source =
        crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::SmokeCinematic);
    let (common, rest) = source.split_once("// ---- compute ----").expect("compute marker");
    let (compute, rest) = rest.split_once("// ---- draw ----").expect("draw marker");
    let (draw, full) = rest.split_once("// ---- full ----").expect("full marker");
    (format!("{common}{compute}"), format!("{common}{draw}"), full.to_string())
}
