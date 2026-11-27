//! HDR Output (clone deviation, opt-in; docs/playing/hdr.md). The native image is
//! SDR: SPEED2.EXE draws into an A8R8G8B8 back buffer and every pass
//! saturates at 1. With HDR Output the frame is still made as before (the
//! SDR frame, UI included, at paper white), and a last pass, the **display
//! transform**, writes the swap chain in an HDR signal:
//!
//! * BT.2100 PQ (HDR10) on [`wgpu::TextureFormat::Rgb10a2Unorm`] with
//!   [`wgpu::SurfaceColorSpace::Bt2100Pq`] (Vulkan where the compositor
//!   offers it, DX12, Metal), or
//! * scRGB on [`wgpu::TextureFormat::Rgba16Float`] with
//!   [`wgpu::SurfaceColorSpace::ExtendedSrgbLinear`] (Windows DX12/Vulkan,
//!   Metal's EDR).
//!
//! Midtones keep the original grade exactly: every pixel up to SDR white is
//! the SDR frame's value through the SDR transfer times paper white. Only
//! where the post passes' unclipped image (High Precision Post's float
//! chain, graded through the captured grade LUT, `postfx.rs`
//! `fs_grade_lut`) goes above 1 — neon, headlights, glow, streaks, sun and
//! sky highlights — the light goes above paper white, up to the peak, on a
//! shoulder ([`shoulder`]).

pub use crate::qol::hdr_output::{Calibration, DEFAULT_PAPER, DEFAULT_PEAK, Signal, Transfer};
use crate::render::Gpu;

/// The signal a surface can carry: PQ first (every HDR display takes it;
/// 10 bits are enough for its curve), else scRGB. `NFSU2_HDR_SIGNAL=scrgb`
/// prefers scRGB where both are offered.
pub fn choose(caps: &wgpu::SurfaceCapabilities) -> Option<Signal> {
    let offered = |s: Signal| caps.color_spaces(s.format()).contains(s.color_space().to_color_spaces().unwrap());
    let mut order = [Signal::Pq, Signal::ScRgb];
    if crate::render::debug_env::nfsu2_hdr_signal().as_deref().and_then(Signal::parse) == Some(Signal::ScRgb) {
        order.reverse();
    }
    order.into_iter().find(|&s| offered(s))
}

/// Where `NFSU2_HDR_PATTERN=1` draws the pattern off the calibration page:
/// a band across the lower middle of the output.
pub const PATTERN_FALLBACK: [f32; 4] = [0.08, 0.53, 0.92, 0.78];

/// The pattern's rectangle on a `size` output for an area of the 640×480
/// canvas (top left, bottom right; centre-placed like the option rows):
/// fractions of the output, as `Calibration::pattern` takes them.
pub fn pattern_rect((lo, hi): (glam::Vec2, glam::Vec2), size: (u32, u32)) -> [f32; 4] {
    use crate::render::display::{Place, UiFrame};
    let frame = UiFrame::new(size);
    let (a, b) = (frame.to_pixels(lo, Place::CENTRE), frame.to_pixels(hi, Place::CENTRE));
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    [a.x / w, a.y / h, b.x / w, b.y / h]
}

/// Highlights: light `r` ≥ 1 relative to paper white, to [1, `h`) (`h` =
/// peak / paper white): slope 1 at white (no kink where the SDR range
/// ends), approaching the peak. Below white: unchanged.
pub fn shoulder(r: f32, h: f32) -> f32 {
    if h <= 1.0 {
        return r.min(1.0);
    }
    if r <= 1.0 {
        return r;
    }
    1.0 + (h - 1.0) * (1.0 - (-(r - 1.0) / (h - 1.0)).exp())
}

/// SMPTE ST 2084 inverse EOTF: nits to the PQ signal.
pub fn pq_encode(nits: f32) -> f32 {
    let (m1, m2, c1, c2, c3) = (0.159_301_76, 78.843_75, 0.835_937_5, 18.851_563, 18.6875);
    let y = (nits / 10000.0).clamp(0.0, 1.0).powf(m1);
    ((c1 + c2 * y) / (1.0 + c3 * y)).powf(m2)
}

/// ST 2084 EOTF: the PQ signal to nits.
pub fn pq_decode(e: f32) -> f32 {
    let (m1, m2, c1, c2, c3) = (0.159_301_76, 78.843_75, 0.835_937_5, 18.851_563, 18.6875);
    let ep = e.clamp(0.0, 1.0).powf(1.0 / m2);
    10000.0 * ((ep - c1).max(0.0) / (c2 - c3 * ep)).powf(1.0 / m1)
}

/// BT.709 to BT.2020 (linear light; ITU-R BT.2087).
pub const BT709_TO_BT2020: [[f32; 3]; 3] = [
    [0.627_403_9, 0.329_283, 0.043_313_1],
    [0.069_097_3, 0.919_540_4, 0.011_362_3],
    [0.016_391_4, 0.088_013_3, 0.895_595_3],
];

pub fn bt709_to_bt2020(c: [f32; 3]) -> [f32; 3] {
    let m = BT709_TO_BT2020;
    [0, 1, 2].map(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
}

/// CPU reference of one pixel's light (nits, BT.709): `sdr` the SDR frame's
/// value, `unclipped` the post passes' image (None: not this frame's).
pub fn light(sdr: [f32; 3], unclipped: Option<[f32; 3]>, cal: &Calibration) -> [f32; 3] {
    let mut lin = sdr.map(|v| cal.transfer.eotf(v) * cal.paper);
    if let Some(t) = unclipped {
        let m = t[0].max(t[1]).max(t[2]);
        if m > 1.0 {
            let d = (0..3).map(|i| (sdr[i] - t[i].clamp(0.0, 1.0)).abs()).fold(0.0, f32::max);
            let w = 1.0 - smoothstep(3.0 / 255.0, 8.0 / 255.0, d);
            let o = shoulder(m.powf(cal.transfer.exponent()), cal.peak / cal.paper);
            for i in 0..3 {
                let hi = cal.transfer.eotf(t[i] / m) * o * cal.paper;
                lin[i] += (hi - lin[i]) * w;
            }
        }
    }
    lin
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// CPU reference of the encoded output.
pub fn encode(nits: [f32; 3], signal: Signal) -> [f32; 3] {
    match signal {
        Signal::ScRgb => nits.map(|n| n / 80.0),
        Signal::Pq => bt709_to_bt2020(nits).map(|n| pq_encode(n.max(0.0))),
    }
}

pub use crate::render::debug::hdr_evidence::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    /// The calibration pattern's rectangle (fractions of the output).
    c: [f32; 4],
}

/// The display transform pass.
pub struct DisplayTransform {
    layout: wgpu::BindGroupLayout,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    black: wgpu::TextureView,
}

impl DisplayTransform {
    pub fn new(gpu: &Gpu) -> DisplayTransform {
        let device = &gpu.device;
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("hdr output"),
            entries: &[
                tex(0),
                tex(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Params>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("hdr output"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("hdr_output.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/hdr_output.wgsl")).into(),
            ),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("hdr output"),
                size: std::mem::size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let black = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("hdr output: no unclipped image"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        DisplayTransform { layout, pipelines: Vec::new(), pipeline_layout, shader, sampler, uniform, black }
    }

    fn pipeline(&mut self, gpu: &Gpu, format: wgpu::TextureFormat) -> usize {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return i;
        }
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("hdr output"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        self.pipelines.push((format, p));
        self.pipelines.len() - 1
    }

    /// The SDR frame `sdr` (and the post passes' unclipped image, when this
    /// frame has one) to `target` in `signal`, `size` pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        sdr: &wgpu::TextureView,
        unclipped: Option<&wgpu::TextureView>,
        target: &wgpu::TextureView,
        signal: Signal,
        cal: &Calibration,
        size: (u32, u32),
    ) {
        let p = self.pipeline(gpu, signal.format());
        let params = Params {
            a: [
                cal.paper,
                cal.peak.max(cal.paper),
                if signal == Signal::ScRgb { 1.0 } else { 0.0 },
                cal.transfer.index(),
            ],
            b: [
                if cal.pattern.is_some() { 1.0 } else { 0.0 },
                if unclipped.is_some() { 1.0 } else { 0.0 },
                size.0 as f32,
                size.1 as f32,
            ],
            c: cal.pattern.unwrap_or([0.0; 4]),
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hdr output"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(sdr) },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(unclipped.unwrap_or(&self.black)),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: self.uniform.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hdr output: display transform"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipelines[p].1);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pq_round_trips_and_hits_the_standard_points() {
        // ST 2084: 100 nits ≈ 0.508, 1000 nits ≈ 0.752, 10 000 nits = 1.
        assert!((pq_encode(100.0) - 0.508).abs() < 0.001, "{}", pq_encode(100.0));
        assert!((pq_encode(1000.0) - 0.7518).abs() < 0.001, "{}", pq_encode(1000.0));
        assert!((pq_encode(10000.0) - 1.0).abs() < 1e-6);
        assert_eq!(pq_encode(0.0), (0.835_937_5f32).powf(78.843_75));
        for n in [0.05f32, 1.0, 80.0, 203.0, 600.0, 4000.0] {
            assert!((pq_decode(pq_encode(n)) - n).abs() / n < 1e-3, "{n}");
        }
    }

    #[test]
    fn bt2020_matrix_keeps_white_and_luminance() {
        let w = bt709_to_bt2020([1.0, 1.0, 1.0]);
        assert!(w.iter().all(|c| (c - 1.0).abs() < 1e-4), "{w:?}");
        // Luminance is the same in both (of the input = Y2020 of the output).
        let c = [0.8, 0.3, 0.1];
        let o = bt709_to_bt2020(c);
        let y709 = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let y2020 = 0.2627 * o[0] + 0.678 * o[1] + 0.0593 * o[2];
        assert!((y709 - y2020).abs() < 2e-3, "{y709} {y2020}");
    }

    #[test]
    fn shoulder_is_continuous_monotonic_and_bounded() {
        let h = 1000.0 / 203.0;
        assert_eq!(shoulder(1.0, h), 1.0);
        assert!((shoulder(1.0 + 1e-3, h) - (1.0 + 1e-3)).abs() < 1e-5, "slope 1 at white");
        let mut last = 1.0;
        for i in 1..2000 {
            let v = shoulder(1.0 + i as f32 * 0.05, h);
            // Rising (strictly while f32 resolves it), never past the peak.
            assert!(v >= last && v <= h && (i > 300 || v > last), "{i} {v}");
            last = v;
        }
        // No headroom (peak at paper white): clipped as SDR.
        assert_eq!(shoulder(3.0, 1.0), 1.0);
        assert_eq!(shoulder(0.5, h), 0.5);
    }

    #[test]
    fn midtones_are_the_sdr_frame_at_paper_white() {
        let cal = Calibration::default();
        // Every SDR code, with or without an unclipped image ≤ 1: exactly
        // the SDR transfer times paper white.
        for k in 0..=255u32 {
            let v = k as f32 / 255.0;
            let a = light([v, v * 0.5, 0.2], None, &cal);
            let b = light([v, v * 0.5, 0.2], Some([v, v * 0.5, 0.2]), &cal);
            assert_eq!(a, b);
            assert!((a[0] - v.powf(2.2) * 203.0).abs() < 1e-3);
        }
        // White: paper white.
        let w = light([1.0; 3], Some([1.0; 3]), &cal);
        assert!(w.iter().all(|&c| (c - 203.0).abs() < 1e-3));
    }

    #[test]
    fn highlights_rise_to_the_peak_with_their_hue() {
        let cal = Calibration::default();
        // A headlight the post passes left at 1.6 × white.
        let l = light([1.0; 3], Some([1.6, 1.6, 1.6]), &cal);
        assert!(l[0] > 400.0 && l[0] < cal.peak, "{l:?}");
        // Far above: close to the peak, never over.
        let l = light([1.0; 3], Some([40.0; 3]), &cal);
        assert!(l[0] > 0.99 * cal.peak && l[0] <= cal.peak, "{l:?}");
        // A neon tube (1.5, 0.3, 1.2): the hue of t / m, brighter than white.
        let t = [1.5, 0.3, 1.2];
        let sdr = t.map(|c: f32| c.min(1.0));
        let l = light(sdr, Some(t), &cal);
        assert!(l[0] > 203.0 && l[1] < l[2] && l[2] < l[0], "{l:?}");
        // The UI drawn over it (the frame no longer shows the post image):
        // no expansion.
        let l = light([0.2, 0.2, 0.2], Some([3.0; 3]), &cal);
        assert!((l[0] - 0.2f32.powf(2.2) * 203.0).abs() < 1e-3, "{l:?}");
    }

    #[test]
    fn encoded_white_is_paper_white() {
        let cal = Calibration::default();
        let w = light([1.0; 3], None, &cal);
        let pq = encode(w, Signal::Pq);
        assert!(pq.iter().all(|&c| (pq_decode(c) - 203.0).abs() < 0.5), "{pq:?}");
        let sc = encode(w, Signal::ScRgb);
        assert!(sc.iter().all(|&c| (c - 203.0 / 80.0).abs() < 1e-4));
    }

    #[test]
    fn decode_inverts_encode() {
        for c in [[203.0f32, 203.0, 203.0], [900.0, 120.0, 40.0], [0.5, 2.0, 30.0]] {
            for signal in [Signal::Pq, Signal::ScRgb] {
                let e = encode(c, signal);
                let texel: Vec<u8> = match signal {
                    Signal::Pq => {
                        let q = e.map(|v| (v * 1023.0).round() as u32);
                        (q[0] | q[1] << 10 | q[2] << 20 | 3 << 30).to_le_bytes().to_vec()
                    }
                    Signal::ScRgb => e.iter().flat_map(|&v| f32_to_f16(v).to_le_bytes()).chain([0, 0x3C]).collect(),
                };
                let d = decode(&texel, signal);
                for i in 0..3 {
                    // 10-bit PQ: ~1 % steps near these levels.
                    assert!((d[i] - c[i]).abs() <= c[i] * 0.02 + 0.05, "{signal:?} {c:?} {d:?}");
                }
            }
        }
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0xC000), -2.0);
    }

    fn f32_to_f16(v: f32) -> u16 {
        // Enough for the test's positive normal values.
        let b = v.to_bits();
        let e = ((b >> 23) & 0xFF) as i32 - 127 + 15;
        let m = (b >> 13) & 0x3FF;
        ((e as u32) << 10 | m) as u16
    }

    #[test]
    fn transfers() {
        assert!((Transfer::Srgb.eotf(0.5) - 0.214).abs() < 1e-3);
        assert!((Transfer::Gamma22.eotf(0.5) - 0.2176).abs() < 1e-3);
        assert!((Transfer::Gamma24.eotf(0.5) - 0.1895).abs() < 1e-3);
        assert_eq!(Transfer::from_index(9), Transfer::Gamma22);
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    /// The display transform on the device against the CPU reference
    /// (`light`, `encode`), both signals, with and without the unclipped
    /// image, and the pattern's corners.
    #[test]
    #[ignore = "requires a GPU; run explicitly"]
    fn display_transform_matches_the_cpu_reference() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let (w, h) = (64u32, 4u32);
        // Row 0: SDR codes 0..255 in steps of 4 (grey); row 1: the same
        // with an unclipped image at 1 + x/16 (highlights); row 2: a UI
        // pixel over a highlight; row 3: a coloured highlight.
        let mut sdr = vec![0u8; (w * h * 4) as usize];
        let mut unc = vec![[0f32; 4]; (w * h) as usize];
        for x in 0..w {
            let k = (x * 4).min(255) as u8;
            let set = |sdr: &mut Vec<u8>, y: u32, c: [u8; 3]| {
                sdr[((y * w + x) * 4) as usize..][..4].copy_from_slice(&[c[0], c[1], c[2], 255])
            };
            set(&mut sdr, 0, [k; 3]);
            unc[x as usize] = [f32::from(k) / 255.0, f32::from(k) / 255.0, f32::from(k) / 255.0, 1.0];
            set(&mut sdr, 1, [255; 3]);
            unc[(w + x) as usize] = [1.0 + x as f32 / 16.0; 4];
            set(&mut sdr, 2, [40; 3]);
            unc[(2 * w + x) as usize] = [3.0, 3.0, 3.0, 1.0];
            let t = [1.0 + x as f32 / 20.0, 0.4, 0.9];
            set(&mut sdr, 3, t.map(|c: f32| (c.min(1.0) * 255.0).round() as u8));
            unc[(3 * w + x) as usize] = [t[0], t[1], t[2], 1.0];
        }
        let tex = |format, usage, label| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let s = tex(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            "sdr",
        );
        gpu.queue.write_texture(
            s.as_image_copy(),
            &sdr,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
            s.size(),
        );
        // Half float, as the post passes write it (values chosen exact in
        // f16 or close: compared through `half`).
        let half = |v: f32| -> u16 {
            if v == 0.0 {
                return 0;
            }
            let b = v.to_bits();
            let e = ((b >> 23) & 0xFF) as i32 - 127 + 15;
            (((e as u32) << 10) | ((b >> 13) & 0x3FF)) as u16
        };
        let unc: Vec<[f32; 4]> = unc.iter().map(|c| c.map(|v| f16_to_f32(half(v)))).collect();
        let bytes: Vec<u8> =
            unc.iter().flat_map(|c| c.iter().flat_map(|&v| half(v).to_le_bytes()).collect::<Vec<u8>>()).collect();
        let u = tex(
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            "unclipped",
        );
        gpu.queue.write_texture(
            u.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 8), rows_per_image: None },
            u.size(),
        );
        let mut t = DisplayTransform::new(&gpu);
        for signal in [Signal::Pq, Signal::ScRgb] {
            for with in [false, true] {
                let target =
                    tex(signal.format(), wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, "out");
                let cal = Calibration::default();
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                let uv = u.create_view(&Default::default());
                t.run(
                    &gpu,
                    &mut encoder,
                    &s.create_view(&Default::default()),
                    with.then_some(&uv),
                    &target.create_view(&Default::default()),
                    signal,
                    &cal,
                    (w, h),
                );
                gpu.queue.submit([encoder.finish()]);
                let got = read_nits(&gpu, &target, signal, (w, h));
                for y in 0..h {
                    for x in 0..w {
                        let i = (y * w + x) as usize;
                        let px = &sdr[i * 4..i * 4 + 3];
                        let want = light(
                            [px[0], px[1], px[2]].map(|c| f32::from(c) / 255.0),
                            with.then(|| [unc[i][0], unc[i][1], unc[i][2]]),
                            &cal,
                        );
                        // Through the signal's quantisation (10-bit PQ:
                        // ~1 % per code at these levels) and back.
                        let back = decode_ref(encode(want, signal), signal);
                        for c in 0..3 {
                            // One 10-bit PQ code is ~1 % here; back to BT.709 a saturated
                            // colour's dim channel mixes three codes (×1.66).
                            let tol =
                                back[c].abs() * 0.04 + back.iter().fold(0.0f32, |a, &b| a.max(b.abs())) * 0.006 + 0.1;
                            assert!(
                                (got[i][c] - back[c]).abs() <= tol,
                                "{signal:?} unclipped {with} ({x}, {y}) ch {c}: gpu {} cpu {}",
                                got[i][c],
                                back[c]
                            );
                        }
                    }
                }
                // Highlights above paper white only where the unclipped
                // image is above 1 and the frame still shows it.
                let peak_row1 = got[(w + w - 1) as usize][0];
                if with {
                    assert!(peak_row1 > 2.0 * cal.paper && peak_row1 <= cal.peak * 1.03, "{peak_row1}");
                    assert!(got[(2 * w) as usize][0] < cal.paper * 0.2, "UI over a highlight stays SDR");
                } else {
                    assert!((peak_row1 - cal.paper).abs() < cal.paper * 0.03, "{peak_row1}");
                }
            }
        }
    }

    /// The encoded value read back as the GPU's target holds it (10-bit
    /// PQ codes; half floats for scRGB, close enough at f32).
    fn decode_ref(e: [f32; 3], signal: Signal) -> [f32; 3] {
        match signal {
            Signal::Pq => {
                let c = e.map(|v| pq_decode((v * 1023.0).round() / 1023.0));
                let m = BT2020_TO_BT709;
                [0, 1, 2].map(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
            }
            Signal::ScRgb => e.map(|v| v * 80.0),
        }
    }
}
