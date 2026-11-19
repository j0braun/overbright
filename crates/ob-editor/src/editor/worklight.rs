//! The work light: an editor-only pass that makes a dark world image
//! readable (the region at its default night time seen from above) when
//! the image is copied into the window. **Game look** skips it and shows
//! the renderer's own frame unchanged.
//!
//! Two small GPU steps over the finished world image, nothing in the
//! renderer, its options or its frame parameters:
//! 1. **meter** (one compute workgroup): the log-average luminance of a
//!    128 × 72 grid of samples, black pixels (the void around the region)
//!    left out, and from it the exposure that brings that average to
//!    [`KEY`] on the curve below;
//! 2. **light** (a full-screen draw into a second texture, the one egui
//!    shows): each pixel's largest linear channel `m` goes through
//!    `(1 − e^(−E·m)) / (1 − e^(−E))` (black stays black, white stays
//!    white, darks are raised about E times, highlights are not clipped),
//!    the colour is scaled with it so hue and saturation hold, and the
//!    darks get a small lift ([`LIFT`]) in display space.
//!
//! The same curve and exposure search in Rust ([`curve`], [`exposure`],
//! [`light_pixel`]) are what the tests check the shader against.

/// The display-space mid level the metered average is brought to (linear).
pub const KEY: f32 = 0.14;
/// The exposure range: never darker than about the game's frame, and
/// not more than this much brighter in the darks.
pub const EXPOSURE_MIN: f32 = 0.5;
pub const EXPOSURE_MAX: f32 = 32.0;
/// Lift of the darks in display space (0..1 of full white).
pub const LIFT: f32 = 0.03;
/// Linear luminance at or below which a sample counts as black and is not
/// metered.
pub const BLACK: f32 = 2.0e-4;
/// The metering grid.
pub const GRID: (u32, u32) = (128, 72);
/// Steps of the exposure search (bisection).
const STEPS: u32 = 24;

/// sRGB decode (the image holds display-encoded values).
pub fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// sRGB encode.
pub fn to_display(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.max(0.0).powf(1.0 / 2.4) - 0.055 }
}

/// The work light's tone curve at exposure `e` (0 → 0, 1 → 1, rising).
pub fn curve(x: f32, e: f32) -> f32 {
    (1.0 - (-e * x).exp()) / (1.0 - (-e).exp())
}

/// The exposure that puts a log-average linear luminance `avg` at [`KEY`]
/// (`None`: nothing but black was metered, exposure 1).
pub fn exposure(avg: Option<f32>) -> f32 {
    let Some(avg) = avg else { return 1.0 };
    let (mut lo, mut hi) = (EXPOSURE_MIN, EXPOSURE_MAX);
    for _ in 0..STEPS {
        let mid = 0.5 * (lo + hi);
        if curve(avg, mid) < KEY {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Linear luminance (Rec. 709) of display-encoded RGB.
pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * to_linear(rgb[0]) + 0.7152 * to_linear(rgb[1]) + 0.0722 * to_linear(rgb[2])
}

/// One display-encoded pixel through the work light at exposure `e`.
pub fn light_pixel(rgb: [f32; 3], e: f32) -> [f32; 3] {
    let lin = rgb.map(to_linear);
    let m = lin[0].max(lin[1]).max(lin[2]);
    let scale = if m > 1.0e-6 { curve(m, e) / m } else { e / (1.0 - (-e).exp()) };
    lin.map(|c| LIFT + (1.0 - LIFT) * to_display((c * scale).min(1.0)))
}

/// The meter and the light pass in WGSL, with the constants above.
fn shader() -> String {
    format!(
        r#"
const KEY: f32 = {KEY:?};
const EXPOSURE_MIN: f32 = {EXPOSURE_MIN:?};
const EXPOSURE_MAX: f32 = {EXPOSURE_MAX:?};
const LIFT: f32 = {LIFT:?};
const BLACK: f32 = {BLACK:?};
const GX: u32 = {gx}u;
const GY: u32 = {gy}u;
const STEPS: u32 = {STEPS}u;

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> meter: array<f32, 4>;

fn to_linear(c: vec3<f32>) -> vec3<f32> {{
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}}
fn to_display(c: vec3<f32>) -> vec3<f32> {{
    return select(1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}}
fn curve(x: f32, e: f32) -> f32 {{
    return (1.0 - exp(-e * x)) / (1.0 - exp(-e));
}}

var<workgroup> sums: array<f32, 256>;
var<workgroup> counts: array<f32, 256>;

@compute @workgroup_size(256)
fn measure(@builtin(local_invocation_index) t: u32) {{
    let size = vec2<f32>(textureDimensions(src));
    var sum = 0.0;
    var count = 0.0;
    for (var i = t; i < GX * GY; i += 256u) {{
        let cell = vec2<f32>(f32(i % GX) + 0.5, f32(i / GX) + 0.5);
        let p = vec2<i32>(cell * size / vec2<f32>(f32(GX), f32(GY)));
        let lin = to_linear(textureLoad(src, p, 0).rgb);
        let l = dot(lin, vec3<f32>(0.2126, 0.7152, 0.0722));
        if l > BLACK {{
            sum += log(l);
            count += 1.0;
        }}
    }}
    sums[t] = sum;
    counts[t] = count;
    workgroupBarrier();
    for (var half = 128u; half > 0u; half >>= 1u) {{
        if t < half {{
            sums[t] += sums[t + half];
            counts[t] += counts[t + half];
        }}
        workgroupBarrier();
    }}
    if t == 0u {{
        var e = 1.0;
        if counts[0] > 0.0 {{
            let avg = exp(sums[0] / counts[0]);
            var lo = EXPOSURE_MIN;
            var hi = EXPOSURE_MAX;
            for (var s = 0u; s < STEPS; s++) {{
                let mid = 0.5 * (lo + hi);
                if curve(avg, mid) < KEY {{ lo = mid; }} else {{ hi = mid; }}
            }}
            e = 0.5 * (lo + hi);
            meter[1] = avg;
        }}
        meter[0] = e;
        meter[2] = counts[0];
    }}
}}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
}}

@group(0) @binding(1) var<storage, read> exposure: array<f32, 4>;

@fragment
fn light(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {{
    let c = textureLoad(src, vec2<i32>(p.xy), 0);
    let e = exposure[0];
    let lin = to_linear(c.rgb);
    let m = max(lin.r, max(lin.g, lin.b));
    var scale = e / (1.0 - exp(-e));
    if m > 1.0e-6 {{ scale = curve(m, e) / m; }}
    let out = to_display(min(lin * scale, vec3<f32>(1.0)));
    return vec4<f32>(LIFT + (1.0 - LIFT) * out, c.a);
}}
"#,
        gx = GRID.0,
        gy = GRID.1,
    )
}

/// The work light's GPU side for one source image.
pub struct WorkLight {
    measure: wgpu::ComputePipeline,
    light: wgpu::RenderPipeline,
    meter: wgpu::Buffer,
    /// Per source: the lit image, its egui texture, the bind groups.
    out: Option<Output>,
}

struct Output {
    size: (u32, u32),
    view: wgpu::TextureView,
    id: egui::TextureId,
    measure_group: wgpu::BindGroup,
    light_group: wgpu::BindGroup,
}

impl WorkLight {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> WorkLight {
        // Two modules from one source: the meter writes the buffer, the
        // light pass reads it, and each entry point's layout is derived.
        let src = shader();
        let module = |label| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.clone().into()),
            })
        };
        let m = module("editor work light meter");
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("editor work light meter"),
            layout: None,
            module: &m,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let l = module("editor work light");
        let light = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("editor work light"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &l,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &l,
                entry_point: Some("light"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let meter = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("editor work light exposure"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        WorkLight { measure, light, meter, out: None }
    }

    /// Point the pass at a (new) source image of `size` pixels in `format`:
    /// the lit image is made (again) at that size, under the same egui
    /// texture id once it has one. Returns that id.
    pub fn set_source(
        &mut self,
        device: &wgpu::Device,
        egui: &mut egui_wgpu::Renderer,
        source: &wgpu::TextureView,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> egui::TextureId {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("editor work light image"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let group = |pipeline_layout: wgpu::BindGroupLayout| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source) },
                    wgpu::BindGroupEntry { binding: 1, resource: self.meter.as_entire_binding() },
                ],
            })
        };
        let measure_group = group(self.measure.get_bind_group_layout(0));
        let light_group = group(self.light.get_bind_group_layout(0));
        let id = match self.out.take() {
            Some(old) => {
                egui.update_egui_texture_from_wgpu_texture(device, &view, wgpu::FilterMode::Linear, old.id);
                old.id
            }
            None => egui.register_native_texture(device, &view, wgpu::FilterMode::Linear),
        };
        self.out = Some(Output { size, view, id, measure_group, light_group });
        id
    }

    /// The egui texture of the lit image (None before [`WorkLight::set_source`]).
    pub fn id(&self) -> Option<egui::TextureId> {
        self.out.as_ref().map(|o| o.id)
    }

    /// The lit image's view (tests read it back).
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        self.out.as_ref().map(|o| &o.view)
    }

    /// The buffer the meter writes: exposure, log-average luminance,
    /// samples metered (tests read it back).
    pub fn meter(&self) -> &wgpu::Buffer {
        &self.meter
    }

    /// Record the meter and the light pass over the current source.
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(o) = &self.out else { return };
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("editor work light meter"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.measure);
            pass.set_bind_group(0, &o.measure_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("editor work light"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &o.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0.0, 0.0, o.size.0 as f32, o.size.1 as f32, 0.0, 1.0);
        pass.set_pipeline(&self.light);
        pass.set_bind_group(0, &o.light_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Free the egui texture.
    pub fn release(&mut self, egui: &mut egui_wgpu::Renderer) {
        if let Some(o) = self.out.take() {
            egui.free_texture(&o.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_keeps_black_and_white_and_never_darkens() {
        for e in [EXPOSURE_MIN.max(1.0), 2.0, 8.0, EXPOSURE_MAX] {
            assert!(curve(0.0, e).abs() < 1e-6);
            assert!((curve(1.0, e) - 1.0).abs() < 1e-6);
            let mut last = 0.0;
            for i in 1..=100 {
                let x = i as f32 / 100.0;
                let y = curve(x, e);
                assert!(y >= last, "rising");
                assert!(y >= x - 1e-6, "never below the input at e = {e}");
                last = y;
            }
        }
    }

    #[test]
    fn a_dark_image_gets_a_strong_exposure_a_bright_one_almost_none() {
        let night = exposure(Some(0.014));
        assert!(night > 8.0 && night <= EXPOSURE_MAX, "{night}");
        assert!((curve(0.014, night) - KEY).abs() < 1e-3);
        let day = exposure(Some(0.25));
        assert!(day <= EXPOSURE_MIN + 0.01, "{day}");
        assert_eq!(exposure(None), 1.0);
        assert!((exposure(Some(1e-6)) - EXPOSURE_MAX).abs() < 1e-3, "the darkest image stops at the maximum");
    }

    #[test]
    fn a_pixel_keeps_its_hue_and_black_is_lifted_only_a_little() {
        let e = 10.0;
        let [r, g, b] = light_pixel([0.2, 0.1, 0.05], e);
        assert!(r > 0.2 && g > 0.1 && b > 0.05);
        // The linear ratios hold.
        let (lr, lg) = (to_linear((r - LIFT) / (1.0 - LIFT)), to_linear((g - LIFT) / (1.0 - LIFT)));
        let ratio = to_linear(0.2) / to_linear(0.1);
        assert!((lr / lg - ratio).abs() / ratio < 0.02, "{} vs {ratio}", lr / lg);
        assert_eq!(light_pixel([0.0; 3], e), [LIFT; 3]);
        let white = light_pixel([1.0; 3], e);
        assert!(white.iter().all(|&c| (c - 1.0).abs() < 1e-5));
    }

    #[test]
    fn the_shader_parses() {
        let src = shader();
        assert!(src.contains("fn measure") && src.contains("fn light"));
    }
}
