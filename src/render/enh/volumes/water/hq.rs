//! Animated Water, Realistic (clone deviation, opt-in; docs/water.md
//! "Realistic"). Nothing here is native (SPEED2.EXE has no water system;
//! docs/water.md "What the original does").
//!
//! The Animated Water row's top tier. It draws the same triangles as On
//! (`render::water`, which picks them), with `water_hq.wgsl` appended to
//! `water.wgsl`, in four passes into the scene before the temporal pass:
//! wet stone and underwater caustics (one screen rectangle over the near
//! water bodies; each body has a top-down distance field to its water's
//! outline, [`wet_fields`]), a half-size copy of the scene behind the water
//! (colour and linear depth), the surfaces (screen-space reflections of the
//! copy, refraction with depth-based absorption, caustics on a virtual floor
//! where nothing is modelled under the water), the sheets and jets (glassy,
//! aerated, lit). The spray is On's
//! with more drops. On's module, pipelines and frames are untouched.

use crate::render::Gpu;
use crate::render::water::{Kind, Mesh};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// A wet field's cells per side.
pub const WET_CELLS: usize = 96;
/// Wet bodies per frame (nearest first).
pub const MAX_WET: usize = 16;
/// How far around a body's water the field (and the wet stone) reaches (m).
pub const WET_MARGIN: f32 = 1.6;
/// Bodies farther than this are not made wet (m).
pub const WET_REACH: f32 = 90.0;
/// Spray billboards per landing at this tier (On: 48).
pub const PER_EMITTER: u32 = 80;

/// One water body's top-down field: per cell the distance (m) to the
/// water's outline (0 inside) and the level of the nearest water (m above
/// `base_z`).
#[derive(Clone, Debug)]
pub struct WetField {
    pub placement: usize,
    pub origin: Vec2,
    pub cell: f32,
    pub base_z: f32,
    /// The box the wet stone is looked for in (world).
    pub lo: Vec3,
    pub hi: Vec3,
    pub cells: Vec<[f32; 2]>,
}

impl WetField {
    /// The field at a world point (nearest cell), None outside it.
    pub fn at(&self, p: Vec2) -> Option<(f32, f32)> {
        let c = (p - self.origin) / self.cell;
        if c.x < 0.0 || c.y < 0.0 || c.x >= WET_CELLS as f32 || c.y >= WET_CELLS as f32 {
            return None;
        }
        let v = self.cells[c.y as usize * WET_CELLS + c.x as usize];
        Some((v[0], v[1] + self.base_z))
    }
}

/// The fields of every placement with water surfaces (basins, pools,
/// streams) at the top LOD, built from the same triangles On draws.
/// Bodies too big for a field (cells over 0.6 m: the long hill streams)
/// get none.
pub fn wet_fields(mesh: &Mesh) -> Vec<WetField> {
    let mut out = Vec::new();
    for r in mesh.records.iter().filter(|r| !r.lod_b && !r.surfaces.is_empty()) {
        let tris: Vec<[Vec3; 3]> = mesh.indices[r.surfaces.start as usize..r.surfaces.end as usize]
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]].map(|i| Vec3::from(mesh.vertices[i as usize].position)))
            .collect();
        // Only the water lying flat (a surface, not a lip running down).
        let tris: Vec<[Vec3; 3]> =
            tris.into_iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero().z.abs() > 0.7).collect();
        if tris.is_empty() {
            continue;
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for t in &tris {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let ext = (hi.x - lo.x).max(hi.y - lo.y) + 2.0 * WET_MARGIN;
        let cell = (ext / WET_CELLS as f32).max(0.12);
        if cell > 0.6 {
            continue;
        }
        let centre = Vec2::new(lo.x + hi.x, lo.y + hi.y) * 0.5;
        let origin = centre - Vec2::splat(cell * WET_CELLS as f32 * 0.5);
        let n = WET_CELLS;
        // Rasterized: the top water's level in each cell inside.
        let mut level = vec![f32::NEG_INFINITY; n * n];
        for t in &tris {
            let a = t.map(|p| (Vec2::new(p.x, p.y) - origin) / cell - Vec2::splat(0.5));
            let (x0, x1) = (
                a.iter().map(|p| p.x).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize,
                a.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max).ceil().min(n as f32 - 1.0) as isize,
            );
            let (y0, y1) = (
                a.iter().map(|p| p.y).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize,
                a.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max).ceil().min(n as f32 - 1.0) as isize,
            );
            let det = (a[1] - a[0]).perp_dot(a[2] - a[0]);
            if det.abs() < 1e-9 || x1 < 0 || y1 < 0 {
                continue;
            }
            for y in y0..=y1 as usize {
                for x in x0..=x1 as usize {
                    let p = Vec2::new(x as f32, y as f32);
                    let w1 = (p - a[0]).perp_dot(a[2] - a[0]) / det;
                    let w2 = (a[1] - a[0]).perp_dot(p - a[0]) / det;
                    let w0 = 1.0 - w1 - w2;
                    let e = -1e-4;
                    if w0 >= e && w1 >= e && w2 >= e {
                        let z = w0 * t[0].z + w1 * t[1].z + w2 * t[2].z;
                        let c = &mut level[y * n + x];
                        *c = c.max(z);
                    }
                }
            }
        }
        // The nearest water cell of every cell (two chamfer sweeps, twice).
        let mut seed: Vec<Option<(i32, i32)>> =
            (0..n * n).map(|i| level[i].is_finite().then_some(((i % n) as i32, (i / n) as i32))).collect();
        if seed.iter().all(|s| s.is_none()) {
            continue;
        }
        let d2 = |x: i32, y: i32, s: (i32, i32)| ((x - s.0) * (x - s.0) + (y - s.1) * (y - s.1)) as i64;
        for _ in 0..2 {
            for pass in 0..2 {
                let offsets: [(i32, i32); 4] =
                    if pass == 0 { [(-1, -1), (0, -1), (1, -1), (-1, 0)] } else { [(1, 1), (0, 1), (-1, 1), (1, 0)] };
                for k in 0..n * n {
                    let k = if pass == 0 { k } else { n * n - 1 - k };
                    let (x, y) = ((k % n) as i32, (k / n) as i32);
                    for (dx, dy) in offsets {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                            continue;
                        }
                        if let Some(s) = seed[ny as usize * n + nx as usize]
                            && seed[k].is_none_or(|c| d2(x, y, s) < d2(x, y, c))
                        {
                            seed[k] = Some(s);
                        }
                    }
                }
            }
        }
        let base_z = lo.z;
        let cells = (0..n * n)
            .map(|k| {
                let (x, y) = ((k % n) as i32, (k / n) as i32);
                let s = seed[k].unwrap();
                let d = (d2(x, y, s) as f32).sqrt() * cell;
                [d, level[s.1 as usize * n + s.0 as usize] - base_z]
            })
            .collect();
        let reach = WET_MARGIN - 0.1;
        out.push(WetField {
            placement: r.placement,
            origin,
            cell,
            base_z,
            lo: Vec3::new(lo.x - reach, lo.y - reach, lo.z - 1.6),
            hi: Vec3::new(hi.x + reach, hi.y + reach, hi.z + 0.8),
            cells,
        });
    }
    out
}

/// An f32 as IEEE half bits (round to nearest; the fields' metres and
/// levels are far inside its range).
pub fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32 - 127 + 15;
    let man = b & 0x7F_FFFF;
    if v == 0.0 || exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7C00;
    }
    let h = ((exp as u32) << 10 | (man >> 13)) + ((man >> 12) & 1);
    sign | h as u16
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct HqUniform {
    size: [f32; 4],
    misc: [f32; 4],
    tanv: [f32; 4],
    wet: [[f32; 4]; 3 * MAX_WET],
}

/// The frame's part of the tier, set by `Water::prepare`.
#[derive(Clone, Default)]
pub struct Frame {
    /// Field indices, nearest first.
    pub bodies: Vec<usize>,
    pub eye: Vec3,
    /// tan of the half field of view across and up.
    pub tan: Vec2,
    pub daylight: f32,
    pub rain: f32,
}

/// The copy of the scene behind the water.
pub(crate) struct Behind {
    pub size: (u32, u32),
    format: wgpu::TextureFormat,
    pub color: wgpu::TextureView,
    pub depth: wgpu::TextureView,
}

pub(crate) struct Hq {
    pub fields: Vec<WetField>,
    pub field_of: HashMap<usize, usize>,
    pub layout2: wgpu::BindGroupLayout,
    /// The scene's colour for the copy: single, multisampled.
    pub layout3: [wgpu::BindGroupLayout; 2],
    shaders: [wgpu::ShaderModule; 2],
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    pub wet: wgpu::TextureView,
    pub behind: Option<Behind>,
    pipelines: std::sync::Mutex<HashMap<(u8, wgpu::TextureFormat, u32, bool, bool), wgpu::RenderPipeline>>,
}

/// What `Hq::draw` needs of On's GPU half.
pub(crate) struct Base<'a> {
    pub layout0: &'a wgpu::BindGroupLayout,
    pub depth_layouts: &'a [wgpu::BindGroupLayout; 2],
    pub group0: &'a wgpu::BindGroup,
    pub vertices: &'a wgpu::Buffer,
    pub indices: &'a wgpu::Buffer,
    pub surfaces: &'a [std::ops::Range<u32>],
    pub sheets: &'a [std::ops::Range<u32>],
    pub reversed: bool,
    /// The water textures (the jets' art).
    pub water_tex: &'a wgpu::TextureView,
}

const BEHIND_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

impl Hq {
    pub fn new(gpu: &Gpu, mesh: &Mesh) -> Hq {
        let device = &gpu.device;
        let fields = wet_fields(mesh);
        let field_of = fields.iter().enumerate().map(|(i, f)| (f.placement, i)).collect();
        let layers = fields.len().clamp(1, 256) as u32;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water wet fields"),
            size: wgpu::Extent3d { width: WET_CELLS as u32, height: WET_CELLS as u32, depth_or_array_layers: layers },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, f) in fields.iter().take(256).enumerate() {
            let data: Vec<u16> = f.cells.iter().flat_map(|c| [f16_bits(c[0]), f16_bits(c[1]), 0, 0]).collect();
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&data),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WET_CELLS as u32 * 8),
                    rows_per_image: Some(WET_CELLS as u32),
                },
                wgpu::Extent3d { width: WET_CELLS as u32, height: WET_CELLS as u32, depth_or_array_layers: 1 },
            );
        }
        let wet = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let fs = wgpu::ShaderStages::FRAGMENT;
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let tex_entry = |binding, filterable, dim| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: fs,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: dim,
                multisampled: false,
            },
            count: None,
        };
        let layout2 = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("water hq"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: vf,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1, true, wgpu::TextureViewDimension::D2),
                tex_entry(2, false, wgpu::TextureViewDimension::D2),
                // (The sampler and the wet fields also in the vertex stage:
                // The Next-Gen basin grid is placed by the fields.)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: vf,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: vf,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout3 = [false, true].map(|multisampled| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("water hq scene"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    count: None,
                }],
            })
        });
        let source = format!(
            "{}\n{}",
            crate::render::water::shader_src(
                "water.wgsl",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl"))
            ),
            crate::render::water::shader_src(
                "water_hq.wgsl",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_hq.wgsl"))
            )
        );
        let module = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shaders = [
            module("water hq", source.clone()),
            module(
                "water hq ms",
                source
                    .replace("texture_depth_2d", "texture_depth_multisampled_2d")
                    .replace("var scene_tex: texture_2d<f32>", "var scene_tex: texture_multisampled_2d<f32>"),
            ),
        ];
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("water hq"),
                size: std::mem::size_of::<HqUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water hq"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Hq {
            fields,
            field_of,
            layout2,
            layout3,
            shaders,
            uniform,
            sampler,
            wet,
            behind: None,
            pipelines: Default::default(),
        }
    }

    /// The nearest bodies with a field among the picked placements.
    pub fn bodies(&self, placements: impl Iterator<Item = usize>, eye: Vec3) -> Vec<usize> {
        let mut v: Vec<(f32, usize)> = placements
            .filter_map(|p| self.field_of.get(&p).copied())
            .filter(|&i| i < 256)
            .map(|i| {
                let f = &self.fields[i];
                (eye.clamp(f.lo, f.hi).distance(eye), i)
            })
            .filter(|(d, _)| *d < WET_REACH)
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        v.dedup_by_key(|x| x.1);
        v.into_iter().take(MAX_WET).map(|(_, i)| i).collect()
    }

    /// The pipelines [`Hq::draw`] uses into `format` (`samples`), made now
    /// (`Water::warm_tier`): the wet stone, the half-size copy, the
    /// surfaces and the sheets.
    pub(crate) fn warm(&self, gpu: &Gpu, base: &Base, format: wgpu::TextureFormat, samples: u32) {
        let ms = samples > 1;
        for which in [0, 1, 2] {
            self.pipeline(gpu, base, which, format, samples, ms);
        }
        self.pipeline(gpu, base, 4, format, 1, ms);
    }

    /// `which`: 0 surfaces, 1 sheets, 2 wet stone and caustics, 4 the
    /// half-size copy (colour + depth).
    pub(crate) fn pipeline(
        &self,
        gpu: &Gpu,
        base: &Base,
        which: u8,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (which, format, samples, base.reversed && which < 2, ms);
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(p) = pipelines.get(&key) {
            return p.clone();
        }
        let depth_layout = &base.depth_layouts[usize::from(ms)];
        let groups: Vec<Option<&wgpu::BindGroupLayout>> = match which {
            0 | 1 => vec![Some(base.layout0), None, Some(&self.layout2)],
            2 => vec![Some(base.layout0), Some(depth_layout), Some(&self.layout2)],
            _ => vec![Some(base.layout0), Some(depth_layout), None, Some(&self.layout3[usize::from(ms)])],
        };
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("water hq"),
            bind_group_layouts: &groups,
            immediate_size: 0,
        });
        let module = &self.shaders[usize::from(ms)];
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::render::water::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let alpha = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            ..alpha
        };
        let keep_alpha = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        // rgb + dst · a: the wet stone darker and glossy, the caustics' light.
        let wet = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::SrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: keep_alpha,
        };
        let target = |format, blend| Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL });
        let (vs, fs, targets): (&str, &str, Vec<Option<wgpu::ColorTargetState>>) = match which {
            0 => ("vs_water", "fs_surface_hq", vec![target(format, None)]),
            1 => ("vs_water", "fs_sheet_hq", vec![target(format, Some(premultiplied))]),
            2 => ("vs_wet", "fs_wet", vec![target(format, Some(wet))]),
            _ => ("vs_blit", "fs_blit", vec![target(format, None), target(BEHIND_DEPTH, None)]),
        };
        let depth_stencil = (which < 2).then(|| wgpu::DepthStencilState {
            format: crate::render::DEPTH_FORMAT,
            depth_write_enabled: Some(which == 0),
            depth_compare: Some(if base.reversed {
                wgpu::CompareFunction::GreaterEqual
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: Default::default(),
            bias: Default::default(),
        });
        let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: if which < 2 { &buffers } else { &[] },
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil,
            multisample: wgpu::MultisampleState { count: if which < 4 { samples } else { 1 }, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        pipelines.insert(key, pipeline.clone());
        pipeline
    }

    fn behind(&mut self, gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) {
        // Half the scene's size (rippled water: its refraction and
        // reflections never need the scene's full detail).
        let size = (size.0.div_ceil(2), size.1.div_ceil(2));
        if self.behind.as_ref().is_some_and(|b| b.size == size && b.format == format) {
            return;
        }
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
        self.behind = Some(Behind {
            size,
            format,
            color: make("water behind", format),
            depth: make("water behind depth", BEHIND_DEPTH),
        });
    }

    /// The tier's passes into the scene (`target`, `depth`: the main
    /// view's, `samples` each, `size` px).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        base: &Base,
        frame: &Frame,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        ng: Option<&mut crate::render::water_ng::Ng>,
    ) {
        self.behind(gpu, size, format);
        let ms = samples > 1;
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| eprintln!("water: realistic tier, {}x{} scene, {samples} sample(s)", size.0, size.1));
        // The frame's bodies.
        let mut u = HqUniform {
            size: [size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32],
            misc: [0.0, 0.0, frame.daylight, frame.rain],
            tanv: [frame.tan.x, frame.tan.y, 0.0, 0.0],
            wet: [[0.0; 4]; 3 * MAX_WET],
        };
        for (k, &i) in frame.bodies.iter().take(MAX_WET).enumerate() {
            let f = &self.fields[i];
            let e = frame.eye;
            u.wet[k * 3] = [f.origin.x - e.x, f.origin.y - e.y, f.cell, i as f32];
            u.wet[k * 3 + 1] = (f.lo - e).extend(f.base_z - e.z).to_array();
            u.wet[k * 3 + 2] = (f.hi - e).extend(0.0).to_array();
        }
        u.misc[0] = frame.bodies.len().min(MAX_WET) as f32;
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let b = self.behind.as_ref().unwrap();
        let group2 = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water hq"),
            layout: &self.layout2,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&b.color) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&b.depth) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.wet) },
            ],
        });
        let depth_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water hq depth"),
            layout: &base.depth_layouts[usize::from(ms)],
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) }],
        });
        let scene_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water hq scene"),
            layout: &self.layout3[usize::from(ms)],
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(target) }],
        });
        // 1. Wet stone and underwater caustics.
        if !frame.bodies.is_empty() {
            let wet = self.pipeline(gpu, base, 2, format, samples, ms);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water wet"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, base.group0, &[]);
            pass.set_bind_group(1, &depth_group, &[]);
            pass.set_bind_group(2, &group2, &[]);
            pass.set_pipeline(&wet);
            pass.draw(0..6, 0..1);
            drop(pass);
            crate::prof::gpu::mark(encoder, "graphics: water wet");
        }
        // 2. The scene behind the water, half size.
        {
            let b = self.behind.as_ref().unwrap();
            let p = self.pipeline(gpu, base, 4, format, 1, ms);
            let clear = wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store };
            let attachments = [
                Some(wgpu::RenderPassColorAttachment {
                    view: &b.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: clear,
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &b.depth,
                    depth_slice: None,
                    resolve_target: None,
                    ops: clear,
                }),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water behind"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&p);
            pass.set_bind_group(0, base.group0, &[]);
            pass.set_bind_group(1, &depth_group, &[]);
            pass.set_bind_group(3, &scene_group, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            crate::prof::gpu::mark(encoder, "graphics: water copy");
        }
        // Next-Gen: its own surfaces, sheets and jets.
        if let Some(ng) = ng {
            let pass = crate::render::water_ng::Pass {
                group2: &group2,
                depth_group: &depth_group,
                target,
                format,
                samples,
                depth,
                size,
                ms,
            };
            ng.draw(self, gpu, encoder, base, &pass);
            return;
        }
        // 3. / 4. The surfaces, then the sheets and jets.
        for (which, list, mark) in [(0u8, base.surfaces, "graphics: water"), (1, base.sheets, "graphics: water sheets")]
        {
            if list.is_empty() {
                continue;
            }
            let p = self.pipeline(gpu, base, which, format, samples, ms);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if which == 0 { "water hq surfaces" } else { "water hq sheets" }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&p);
            pass.set_bind_group(0, base.group0, &[]);
            pass.set_bind_group(2, &group2, &[]);
            pass.set_vertex_buffer(0, base.vertices.slice(..));
            pass.set_index_buffer(base.indices.slice(..), wgpu::IndexFormat::Uint32);
            for r in list {
                pass.draw_indexed(r.clone(), 0, 0..1);
            }
            drop(pass);
            crate::prof::gpu::mark(encoder, mark);
        }
    }
}

/// The kinds the fields are made of (surfaces).
pub fn has_field(kind: Kind) -> bool {
    !kind.is_sheet()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::water::{Record, Vertex};

    fn square_pool(z: f32, half: f32) -> Mesh {
        let p = [[-half, -half], [half, -half], [half, half], [-half, half]];
        let vertices = p
            .iter()
            .map(|q| Vertex { position: [q[0], q[1], z], kind: 0.0, uv: [0.0; 2], aux: [0.0; 2], color: [1.0; 4] })
            .collect();
        Mesh {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            records: vec![Record {
                placement: 7,
                lod_b: false,
                section: 0,
                flags: 0,
                surfaces: 0..6,
                sheets: 6..6,
                centre: Vec3::new(0.0, 0.0, z),
                radius: half * 1.5,
                cull_position: Vec3::ZERO,
                cull_radius: 0.0,
                landings: vec![],
                apexes: vec![],
            }],
        }
    }

    #[test]
    fn wet_field_measures_the_edge() {
        let f = &wet_fields(&square_pool(10.0, 2.0))[0];
        assert_eq!(f.placement, 7);
        // Inside: no distance, the water's level.
        let (d, z) = f.at(Vec2::new(0.3, -0.4)).unwrap();
        assert!(d < 1e-3 && (z - 10.0).abs() < 1e-3, "{d} {z}");
        // One metre out of the side: about a metre (one cell of slack).
        let (d, z) = f.at(Vec2::new(3.0, 0.0)).unwrap();
        assert!((d - 1.0).abs() <= f.cell * 1.5, "{d} cell {}", f.cell);
        assert!((z - 10.0).abs() < 1e-3);
        // Out of a corner: the diagonal distance.
        let (d, _) = f.at(Vec2::new(3.0, 3.0)).unwrap();
        assert!((d - 2f32.sqrt()).abs() <= f.cell * 2.0, "{d}");
        // The box reaches past the water and below it.
        assert!(f.lo.x < -3.0 && f.hi.x > 3.0 && f.lo.z < 9.0 && f.hi.z > 10.5);
        assert_eq!(f.cells.len(), WET_CELLS * WET_CELLS);
    }

    #[test]
    fn big_bodies_and_sheets_have_no_field() {
        // 100 m across: cells over 0.6 m.
        assert!(wet_fields(&square_pool(0.0, 50.0)).is_empty());
        // Only sheets (no surfaces): nothing.
        let mut m = square_pool(0.0, 2.0);
        m.records[0].sheets = 0..6;
        m.records[0].surfaces = 0..0;
        assert!(wet_fields(&m).is_empty());
        assert!(has_field(Kind::Basin) && !has_field(Kind::Sheet));
    }

    /// Asset test (ignored without NFSU2_ROOT): every fountain basin and
    /// pool of Bayview gets a field, deterministic, its inside at the
    /// water's level.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_wet_fields() {
        let root = crate::testkit::game_root();
        let world = crate::world::World::load(std::path::Path::new(&root), "L4RA").unwrap();
        let mesh = crate::render::water::build(&world);
        let fields = wet_fields(&mesh);
        let names: std::collections::BTreeSet<String> =
            fields.iter().map(|f| world.solids[world.placements[f.placement].solid].name.clone()).collect();
        for n in ["XO_FOUNTAINBASEC_1A_00", "XO_ROUNDFOUNTAINA_1A_LL_00", "XB_POOLA_1A_DM_00"] {
            assert!(names.contains(n), "{n} has a field: {names:?}");
        }
        assert!(fields.len() <= 256, "{} fields fit the array", fields.len());
        let again = wet_fields(&mesh);
        assert!(fields.iter().zip(&again).all(|(a, b)| a.cells == b.cells && a.placement == b.placement));
        for f in &fields {
            // Some water inside, and the margin dry around it.
            assert!(f.cells.iter().any(|c| c[0] == 0.0), "water in the field");
            assert!(f.cells.iter().any(|c| c[0] > 0.5), "a margin around it");
            assert!(f.lo.z < f.base_z && f.hi.z > f.base_z);
        }
        println!("{} wet fields", fields.len());
    }

    #[test]
    fn half_floats() {
        assert_eq!(f16_bits(1.0), 0x3C00);
        assert_eq!(f16_bits(-2.0), 0xC000);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(65504.0), 0x7BFF);
    }

    #[test]
    fn wet_fields_are_deterministic() {
        let m = square_pool(3.0, 1.3);
        let (a, b) = (wet_fields(&m), wet_fields(&m));
        assert_eq!(a[0].cells, b[0].cells);
    }
}
