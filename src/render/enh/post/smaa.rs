//! SMAA 1x's precomputed area table (the Graphics page's Anti-Aliasing
//! "SMAA 1x", a clone deviation): Jorge Jimenez et al.'s AreaTex for
//! orthogonal patterns, its first sub-texture (subsample offset 0, the
//! only one SMAA 1x reads), computed here rather than shipped. The
//! diagonal half is not built (the passes run without SMAA's diagonal
//! search, as its Medium/High presets without `SMAA_DIAG_DETECTION`).
//!
//! The table is 5 × 5 slots of 16 × 16 texels: slot (e1, e2) for the
//! crossing edges at the line's two ends (0, 1, 3, 4: none, the row above,
//! this row, both), texel (x, y) for the distances x² to the left end and
//! y² to the right one; RG the share of this pixel's colour taken from the
//! one across the edge on each side.

/// Texels a side of a slot (SMAA_AREATEX_MAX_DISTANCE).
pub const SLOT: usize = 16;
/// The table's side (5 slots).
pub const SIDE: usize = 5 * SLOT;
/// Beyond this distance the smoothed corners take their exact area.
const SMOOTH_MAX_DISTANCE: f64 = 32.0;

/// The slot each pattern (bit 0: left crossing in this row, 1: right in
/// this row, 2: left in the row above, 3: right in the row above) sits in.
const EDGES_ORTHO: [(usize, usize); 16] = [
    (0, 0),
    (3, 0),
    (0, 3),
    (3, 3),
    (1, 0),
    (4, 0),
    (1, 3),
    (4, 3),
    (0, 1),
    (3, 1),
    (0, 4),
    (3, 4),
    (1, 1),
    (4, 1),
    (1, 4),
    (4, 4),
];

/// The area under the line p1 → p2 within pixel `x` (from x to x + 1),
/// split by side: (this row's share, the other's).
fn area(p1: (f64, f64), p2: (f64, f64), x: f64) -> (f64, f64) {
    let d = (p2.0 - p1.0, p2.1 - p1.1);
    let (x1, x2) = (x, x + 1.0);
    let y1 = p1.1 + d.1 * (x1 - p1.0) / d.0;
    let y2 = p1.1 + d.1 * (x2 - p1.0) / d.0;
    let inside = (x1 >= p1.0 && x1 < p2.0) || (x2 > p1.0 && x2 <= p2.0);
    if !inside {
        return (0.0, 0.0);
    }
    let trapezoid = y1.signum() == y2.signum() || y1.abs() < 1e-4 || y2.abs() < 1e-4;
    if trapezoid {
        let a = (y1 + y2) / 2.0;
        return if a < 0.0 { (a.abs(), 0.0) } else { (0.0, a.abs()) };
    }
    // Two triangles either side of the crossing.
    let xc = -p1.1 * d.0 / d.1 + p1.0;
    let fract = xc - xc.trunc();
    let a1 = if xc > p1.0 { y1 * fract / 2.0 } else { 0.0 };
    let a2 = if xc < p2.0 { y2 * (1.0 - fract) / 2.0 } else { 0.0 };
    let a = if a1.abs() > a2.abs() { a1 } else { -a2 };
    if a < 0.0 { (a1.abs(), a2.abs()) } else { (a2.abs(), a1.abs()) }
}

/// The U shapes' corners rounded off toward short lines.
fn smooth(d: f64, a1: (f64, f64), a2: (f64, f64)) -> ((f64, f64), (f64, f64)) {
    let b = |a: (f64, f64)| ((a.0 * 2.0).sqrt() * 0.5, (a.1 * 2.0).sqrt() * 0.5);
    let p = (d / SMOOTH_MAX_DISTANCE).clamp(0.0, 1.0);
    let lerp = |b: (f64, f64), a: (f64, f64)| (b.0 + (a.0 - b.0) * p, b.1 + (a.1 - b.1) * p);
    (lerp(b(a1), a1), lerp(b(a2), a2))
}

fn add(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

/// AreaTex's `areaortho` at subsample offset 0.
fn area_ortho(pattern: usize, left: f64, right: f64) -> (f64, f64) {
    let d = left + right + 1.0;
    let (o1, o2) = (0.5, -0.5);
    match pattern {
        1 if left <= right => area((0.0, o2), (d / 2.0, 0.0), left),
        2 if left >= right => area((d / 2.0, 0.0), (d, o2), left),
        3 => {
            let (a1, a2) = smooth(d, area((0.0, o2), (d / 2.0, 0.0), left), area((d / 2.0, 0.0), (d, o2), left));
            add(a1, a2)
        }
        4 if left <= right => area((0.0, o1), (d / 2.0, 0.0), left),
        6 | 7 => area((0.0, o1), (d, o2), left),
        8 if left >= right => area((d / 2.0, 0.0), (d, o1), left),
        9 | 11 | 13 => area((0.0, o2), (d, o1), left),
        12 => {
            let (a1, a2) = smooth(d, area((0.0, o1), (d / 2.0, 0.0), left), area((d / 2.0, 0.0), (d, o1), left));
            add(a1, a2)
        }
        14 => area((0.0, o1), (d, o2), left),
        _ => (0.0, 0.0),
    }
}

/// The table as RG8 texels, row by row (`SIDE` × `SIDE`).
pub fn area_texture() -> Vec<u8> {
    let mut out = vec![0u8; SIDE * SIDE * 2];
    for (pattern, &(sx, sy)) in EDGES_ORTHO.iter().enumerate() {
        for y in 0..SLOT {
            for x in 0..SLOT {
                let (r, g) = area_ortho(pattern, (x * x) as f64, (y * y) as f64);
                let i = ((sy * SLOT + y) * SIDE + sx * SLOT + x) * 2;
                out[i] = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
                out[i + 1] = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    out
}

use crate::render::Gpu;
use crate::render::enhance::PassUniform;

/// SMAA 1x: its three pipelines (edges, weights, blend), the area table,
/// the edges' and weights' targets at the scene's size, and the bind
/// groups reading them (keyed by the source view).
pub(crate) struct Smaa {
    layout: wgpu::BindGroupLayout,
    pipelines: [wgpu::RenderPipeline; 3],
    area: wgpu::TextureView,
    uniform: wgpu::Buffer,
    targets: Option<((u32, u32), wgpu::TextureView, wgpu::TextureView)>,
    groups: Option<(wgpu::TextureView, [wgpu::BindGroup; 3])>,
}

impl Smaa {
    pub(crate) fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Smaa {
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
            label: Some("smaa"),
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
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("smaa"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("smaa.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/smaa.wgsl")).into(),
            ),
        });
        let make = |entry: &str, format: wgpu::TextureFormat| {
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let pipelines = [
            make("fs_edges", wgpu::TextureFormat::Rg8Unorm),
            make("fs_weights", wgpu::TextureFormat::Rgba8Unorm),
            make("fs_blend", format),
        ];
        let side = SIDE as u32;
        let area = crate::render::upload_rg8(gpu, side, side, &area_texture());
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("smaa"),
                size: std::mem::size_of::<PassUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        Smaa { layout, pipelines, area, uniform, targets: None, groups: None }
    }

    /// The three passes from `source` (`size` big) into `target`.
    pub(crate) fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        sampler: &wgpu::Sampler,
        source: &wgpu::TextureView,
        size: (u32, u32),
        target: &wgpu::TextureView,
    ) {
        if self.targets.as_ref().is_none_or(|t| t.0 != size) {
            let make = |label, format| {
                gpu.device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            self.targets = Some((
                size,
                make("smaa edges", wgpu::TextureFormat::Rg8Unorm),
                make("smaa weights", wgpu::TextureFormat::Rgba8Unorm),
            ));
            self.groups = None;
        }
        let (_, edges, weights) = self.targets.clone().unwrap();
        if self.groups.as_ref().is_none_or(|(v, _)| v != source) {
            let group = |a: &wgpu::TextureView, b: &wgpu::TextureView| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("smaa"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(a) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(b) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                        wgpu::BindGroupEntry { binding: 3, resource: self.uniform.as_entire_binding() },
                    ],
                })
            };
            self.groups =
                Some((source.clone(), [group(source, &self.area), group(&edges, &self.area), group(source, &weights)]));
        }
        let u = PassUniform {
            texel: [1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32, size.0 as f32, size.1 as f32],
            params: [0.0; 4],
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let groups = &self.groups.as_ref().unwrap().1;
        for (i, out) in [&edges, &weights, target].into_iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(["smaa edges", "smaa weights", "smaa blend"][i]),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: out,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines[i]);
            pass.set_bind_group(0, &groups[i], &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_table_shapes() {
        let t = area_texture();
        let at = |e1: usize, e2: usize, x: usize, y: usize| {
            let i = ((e2 * SLOT + y) * SIDE + e1 * SLOT + x) * 2;
            (t[i], t[i + 1])
        };
        // No crossing edges: no blend.
        assert_eq!(at(0, 0, 3, 5), (0, 0));
        // A one-pixel step down on the left (crossing in this row): the
        // pixel at the step takes an eighth from across the edge (the
        // triangle under the line from (0, −0.5) to (0.5, 0)), none the
        // other way.
        assert_eq!(at(3, 0, 0, 0), (32, 0));
        // The same step on the right mirrors it.
        assert_eq!(at(0, 3, 0, 0), (32, 0));
        // A Z (crossings on opposite sides): an equal share each way at
        // the middle pixel of a one-pixel line.
        assert_eq!(at(3, 1, 0, 0), (32, 32));
        // Longer lines blend less at their far end: pixel 0 of a line
        // stepping down at the left after 9 more pixels to the right.
        let near = at(3, 0, 0, 3).0;
        let far = at(3, 0, 2, 3).0;
        assert!(near > far && far > 0, "{near} {far}");
        // Pattern 1 holds only while the left end is the nearer one.
        assert_eq!(at(3, 0, 3, 1), (0, 0));
    }
}
