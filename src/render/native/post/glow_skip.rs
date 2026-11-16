//! High Precision Post's blur only where its result can be other than 0
//! (docs/hdr-post.md "Blur where it can be lit"). Exact: the same shaders
//! draw every texel they draw today; a texel left out is one whose every
//! tap is 0, so its result is the 0 its target holds.
//!
//! The prefilter (`fs_overbright_*_flag`, glow_skip.wgsl) marks the 8 x 8
//! tiles of the glow where A can be other than 0 (its input above the
//! overbright offset); two small dispatches (glow_need.wgsl)
//! widen them by the blur's reach, across and then down; one quad per tile
//! writes that need into a depth mask (0 both passes, 0.5 the pass down
//! only, 1 neither), and the two blur passes are drawn at depths 0.25 and
//! 0.75 against it (compare Greater: the early depth test leaves the rest
//! untouched). B and A are cleared first (A after the pass across read
//! it): where the pass down does not run, A's value is 0 (every such tile
//! is outside the marked ones, where the prefilter's A is 0 too).
use super::*;

/// The tiles' size in glow texels (glow_skip.wgsl, glow_need.wgsl).
pub(super) const GLOW_TILE: u32 = 8;
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;

/// The blur's reach in texels (`gauss`, postfx.wgsl: pairs up to 3σ, the
/// last pair's second texel one further) at High Precision Post `hdr`.
pub(super) fn reach(hdr: u8) -> u32 {
    let sigma = if hdr == 2 { HDR_SIGMA_FULL } else { HDR_SIGMA_HALF };
    let radius = (sigma * 3.0).ceil() as u32;
    // gauss's last pair is (i, i + 1) for the largest odd i <= radius.
    let last = if radius % 2 == 1 { radius } else { radius - 1 };
    last + 1
}

/// The reach in tiles: every texel of a tile reads within this many tiles.
pub(super) fn reach_tiles(hdr: u8) -> u32 {
    reach(hdr).div_ceil(GLOW_TILE)
}

/// Whether the device can take this path: storage writes from a fragment
/// shader (the prefilter's flags) and storage reads in a vertex shader
/// (the mask's quads).
pub(super) fn supported(gpu: &Gpu) -> bool {
    let flags = gpu.adapter.get_downlevel_capabilities().flags;
    flags.contains(wgpu::DownlevelFlags::FRAGMENT_WRITABLE_STORAGE | wgpu::DownlevelFlags::VERTEX_STORAGE)
}

pub(super) struct GlowSkip {
    flag_layout: wgpu::BindGroupLayout,
    rows_layout: wgpu::BindGroupLayout,
    need_layout: wgpu::BindGroupLayout,
    mask_layout: wgpu::BindGroupLayout,
    prefilter_mrt: wgpu::RenderPipeline,
    prefilter_sat: wgpu::RenderPipeline,
    across: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    rows: wgpu::ComputePipeline,
    need: wgpu::ComputePipeline,
    mask: wgpu::RenderPipeline,
    info: wgpu::Buffer,
    /// For the glow's size: the flags, rows and need buffers and the mask.
    sized: Option<Sized>,
}

struct Sized {
    glow: (u32, u32),
    tiles: (u32, u32),
    flags: wgpu::Buffer,
    flag_group: wgpu::BindGroup,
    rows_group: wgpu::BindGroup,
    need_group: wgpu::BindGroup,
    mask_group: wgpu::BindGroup,
    mask: wgpu::TextureView,
}

impl GlowSkip {
    pub(super) fn new(gpu: &Gpu, post: &PostFx) -> GlowSkip {
        use crate::render::core::device_cache as dc;
        let storage = |binding, read_only, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(32),
            },
            count: None,
        };
        let (fragment, compute, vertex) =
            (wgpu::ShaderStages::FRAGMENT, wgpu::ShaderStages::COMPUTE, wgpu::ShaderStages::VERTEX);
        let flag_layout = dc::bind_group_layout(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("glow flags"),
                entries: &[storage(0, false, fragment), uniform(1, fragment)],
            },
        );
        let rows_layout = dc::bind_group_layout(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("glow rows"),
                entries: &[storage(0, true, compute), storage(1, false, compute), uniform(6, compute)],
            },
        );
        let need_layout = dc::bind_group_layout(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("glow need"),
                entries: &[storage(2, true, compute), storage(3, false, compute), uniform(6, compute)],
            },
        );
        let mask_layout = dc::bind_group_layout(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("glow need mask"),
                entries: &[storage(4, true, vertex), uniform(5, vertex)],
            },
        );
        let module = dc::wgsl(
            gpu,
            "postfx glow skip",
            format!(
                "{}\n{}",
                crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::PostFx),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/glow_skip.wgsl"))
            ),
        );
        let need_module = dc::wgsl(
            gpu,
            "glow need",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/glow_need.wgsl")),
        );
        let flag_pl = dc::pipeline_layout(
            gpu,
            &wgpu::PipelineLayoutDescriptor {
                label: Some("glow flags"),
                bind_group_layouts: &[Some(&post.layout), Some(&flag_layout)],
                immediate_size: 0,
            },
        );
        let rows_pl = dc::pipeline_layout(
            gpu,
            &wgpu::PipelineLayoutDescriptor {
                label: Some("glow rows"),
                bind_group_layouts: &[Some(&rows_layout)],
                immediate_size: 0,
            },
        );
        let need_pl = dc::pipeline_layout(
            gpu,
            &wgpu::PipelineLayoutDescriptor {
                label: Some("glow need"),
                bind_group_layouts: &[Some(&need_layout)],
                immediate_size: 0,
            },
        );
        let mask_pl = dc::pipeline_layout(
            gpu,
            &wgpu::PipelineLayoutDescriptor {
                label: Some("glow need mask"),
                bind_group_layouts: &[Some(&mask_layout)],
                immediate_size: 0,
            },
        );
        let f = crate::render::HDR_FORMAT;
        let target = Some(wgpu::ColorTargetState { format: f, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let tested = wgpu::DepthStencilState {
            format: MASK_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let render = |label: &str,
                      layout: &wgpu::PipelineLayout,
                      vs: &str,
                      fs: Option<&str>,
                      targets: &[Option<wgpu::ColorTargetState>],
                      depth: Option<wgpu::DepthStencilState>,
                      m: &wgpu::ShaderModule| {
            dc::render_pipeline(
                gpu,
                &wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: m,
                        entry_point: Some(vs),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: fs.map(|fs| wgpu::FragmentState {
                        module: m,
                        entry_point: Some(fs),
                        compilation_options: Default::default(),
                        targets,
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: depth,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: gpu.disk_pipeline_cache(),
                },
            )
        };
        let prefilter_mrt = render(
            "fs_overbright_mrt_hdr_flag",
            &flag_pl,
            "vs_main",
            Some("fs_overbright_mrt_hdr_flag"),
            &[target.clone(), target.clone()],
            None,
            &module,
        );
        let prefilter_sat = render(
            "fs_overbright_sat_flag",
            &flag_pl,
            "vs_main",
            Some("fs_overbright_sat_flag"),
            &[target.clone()],
            None,
            &module,
        );
        let across = render(
            "fs_blur_hdr (where lit)",
            &post.pipeline_layout,
            "vs_glow_across",
            Some("fs_blur_hdr"),
            &[target.clone()],
            Some(tested.clone()),
            &module,
        );
        let down = render(
            "fs_blur_sub_hdr (where lit)",
            &post.pipeline_layout,
            "vs_glow_down",
            Some("fs_blur_sub_hdr"),
            &[target],
            Some(tested),
            &module,
        );
        let compute = |entry: &str, layout: &wgpu::PipelineLayout| {
            dc::compute_pipeline(
                gpu,
                &wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(layout),
                    module: &need_module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: gpu.disk_pipeline_cache(),
                },
            )
        };
        let rows = compute("cs_rows", &rows_pl);
        let need = compute("cs_need", &need_pl);
        let mask = render(
            "glow need mask",
            &mask_pl,
            "vs_need",
            None,
            &[],
            Some(wgpu::DepthStencilState {
                format: MASK_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            &need_module,
        );
        let info = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glow tiles"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        GlowSkip {
            flag_layout,
            rows_layout,
            need_layout,
            mask_layout,
            prefilter_mrt,
            prefilter_sat,
            across,
            down,
            rows,
            need,
            mask,
            info,
            sized: None,
        }
    }

    /// The buffers and the mask for a glow `glow` big (made when it changes).
    fn ensure(&mut self, gpu: &Gpu, glow: (u32, u32)) {
        if self.sized.as_ref().is_some_and(|s| s.glow == glow) {
            return;
        }
        let tiles = (glow.0.div_ceil(GLOW_TILE), glow.1.div_ceil(GLOW_TILE));
        let bytes = u64::from(tiles.0 * tiles.1).max(1) * 4;
        let buffer = |label| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let (flags, rows, need) = (buffer("glow flags"), buffer("glow rows"), buffer("glow need"));
        let group = |layout: &wgpu::BindGroupLayout, entries: &[(u32, &wgpu::Buffer)]| {
            let entries: Vec<_> = entries
                .iter()
                .map(|&(binding, b)| wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() })
                .collect();
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("glow need"),
                layout,
                entries: &entries,
            })
        };
        let flag_group = group(&self.flag_layout, &[(0, &flags), (1, &self.info)]);
        let rows_group = group(&self.rows_layout, &[(0, &flags), (1, &rows), (6, &self.info)]);
        let need_group = group(&self.need_layout, &[(2, &rows), (3, &need), (6, &self.info)]);
        let mask_group = group(&self.mask_layout, &[(4, &need), (5, &self.info)]);
        let mask = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("glow need mask"),
                size: wgpu::Extent3d { width: glow.0.max(1), height: glow.1.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        self.sized = Some(Sized { glow, tiles, flags, flag_group, rows_group, need_group, mask_group, mask });
    }

    /// The prefilter (C, then A; `scaled`: C drawn by the caller), the
    /// need, and the blur across and down into A, for the chain's targets
    /// `cv` (High Precision Post `hdr`, the image so far `cur`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        post: &PostFx,
        cv: &ChainViews,
        cur: Src,
        scaled: bool,
        hdr: u8,
    ) {
        let t = post.targets.as_ref().unwrap();
        self.ensure(gpu, t.glow);
        let s = self.sized.as_ref().unwrap();
        let info = [s.tiles.0, s.tiles.1, reach_tiles(hdr), 0, s.glow.0, s.glow.1, 0, 0];
        gpu.queue.write_buffer(&self.info, 0, bytemuck::cast_slice(&info));
        encoder.clear_buffer(&s.flags, 0, None);
        let draw = |encoder: &mut wgpu::CommandEncoder,
                    label: &str,
                    pipeline: &wgpu::RenderPipeline,
                    targets: &[&wgpu::TextureView],
                    load: wgpu::LoadOp<wgpu::Color>,
                    mask: bool,
                    group: &wgpu::BindGroup,
                    slot: u32,
                    flags: bool| {
            let attachments: Vec<_> = targets
                .iter()
                .map(|&view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                    })
                })
                .collect();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &attachments,
                // Read only: the need's mask.
                depth_stencil_attachment: mask.then(|| wgpu::RenderPassDepthStencilAttachment {
                    view: &s.mask,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[slot * SLOT as u32]);
            if flags {
                pass.set_bind_group(1, &s.flag_group, &[]);
            }
            pass.draw(0..3, 0..1);
        };
        if scaled {
            draw(
                encoder,
                "fs_overbright_sat_flag",
                &self.prefilter_sat,
                &[&cv.a],
                replace_load(),
                false,
                &t.groups[Src::C as usize],
                OVERBRIGHT,
                true,
            );
        } else {
            draw(
                encoder,
                "fs_overbright_mrt_hdr_flag",
                &self.prefilter_mrt,
                &[&cv.c, &cv.a],
                replace_load(),
                false,
                &t.groups[cur as usize],
                PREFILTER,
                true,
            );
        }
        let groups = (s.tiles.0.div_ceil(64), s.tiles.1);
        for (pipeline, group) in [(&self.rows, &s.rows_group), (&self.need, &s.need_group)] {
            let mut pass = encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("glow need"), timestamp_writes: None });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glow need mask"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &s.mask,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.mask);
            pass.set_bind_group(0, &s.mask_group, &[]);
            pass.draw(0..6, 0..s.tiles.0 * s.tiles.1);
        }
        draw(
            encoder,
            "fs_blur_hdr",
            &self.across,
            &[&cv.b],
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            true,
            &t.groups[Src::A as usize],
            ACROSS,
            false,
        );
        // A is cleared too: where the pass down does not run its value is 0
        // (outside the marked tiles), and cleared blocks are cheap to read.
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        draw(
            encoder,
            "fs_blur_sub_hdr",
            &self.down,
            &[&cv.a],
            clear,
            true,
            &t.groups[Src::BWithC as usize],
            DOWN,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `reach` is the farthest texel `gauss` reads (postfx.wgsl's loop),
    /// and a tile's texels read within `reach_tiles` tiles.
    #[test]
    fn reach_is_gauss_s_farthest_texel() {
        for (hdr, sigma) in [(1u8, HDR_SIGMA_HALF), (2, HDR_SIGMA_FULL)] {
            let radius = (sigma * 3.0).ceil() as i32;
            let mut far = 0.0f32;
            let k = -0.5 / (sigma * sigma);
            let mut i = 1;
            while i <= radius {
                let a = i as f32;
                let (w0, w1) = ((k * a * a).exp(), (k * (a + 1.0) * (a + 1.0)).exp());
                let o = (a * w0 + (a + 1.0) * w1) / (w0 + w1);
                // A bilinear tap at o reads texels floor(o) and floor(o) + 1.
                far = far.max(o.floor() + 1.0);
                i += 2;
            }
            assert_eq!(reach(hdr), far as u32, "hdr {hdr}");
            let r = reach(hdr) as i32;
            let t = GLOW_TILE as i32;
            // The first and last texel of tile 0 read tiles in [-k, k].
            let k = reach_tiles(hdr) as i32;
            assert!((-r).div_euclid(t) >= -k && (t - 1 + r).div_euclid(t) <= k, "hdr {hdr}");
        }
        assert_eq!((reach(1), reach(2)), (10, 18));
    }

    /// The need (glow_need.wgsl, modelled here) covers every texel whose
    /// blur can be other than 0: random sparse A on a small glow, the
    /// exact supports of B (A within the reach along the row) and of the
    /// result (B within the reach along the column) against the tiles;
    /// and with one tile less across or down it would not (the reach in
    /// tiles is the least that covers them).
    #[test]
    fn need_covers_every_texel_the_blur_can_light() {
        let (w, h) = (93u32, 61u32);
        let (tw, th) = (w.div_ceil(GLOW_TILE) as i32, h.div_ceil(GLOW_TILE) as i32);
        let mut x = 0x9e37_79b9u32;
        for hdr in [1u8, 2] {
            let r = reach(hdr) as i32;
            let k = reach_tiles(hdr) as i32;
            let mut tighter_misses = false;
            for round in 0..40 {
                let mut a = vec![false; (w * h) as usize];
                for _ in 0..(1 + round % 7) {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    a[(x % (w * h)) as usize] = true;
                }
                let at = |v: &Vec<bool>, px: i32, py: i32| {
                    v[(py.clamp(0, h as i32 - 1) as u32 * w + px.clamp(0, w as i32 - 1) as u32) as usize]
                };
                // Edge texels are clamped as the sampler does.
                let b: Vec<bool> =
                    (0..(w * h) as i32).map(|i| (-r..=r).any(|d| at(&a, i % w as i32 + d, i / w as i32))).collect();
                let out: Vec<bool> =
                    (0..(w * h) as i32).map(|i| (-r..=r).any(|d| at(&b, i % w as i32, i / w as i32 + d))).collect();
                let flag = |tx: i32, ty: i32| {
                    (0..GLOW_TILE as i32).any(|j| {
                        (0..GLOW_TILE as i32).any(|i| {
                            let (px, py) = (tx * GLOW_TILE as i32 + i, ty * GLOW_TILE as i32 + j);
                            px < w as i32 && py < h as i32 && a[(py as u32 * w + px as u32) as usize]
                        })
                    })
                };
                let any = |x0: i32, x1: i32, y0: i32, y1: i32| {
                    (y0.max(0)..=y1.min(th - 1)).any(|ty| (x0.max(0)..=x1.min(tw - 1)).any(|tx| flag(tx, ty)))
                };
                for py in 0..h as i32 {
                    for px in 0..w as i32 {
                        let (tx, ty) = (px / GLOW_TILE as i32, py / GLOW_TILE as i32);
                        let across = any(tx - k, tx + k, ty, ty);
                        let down = any(tx - k, tx + k, ty - k, ty + k);
                        let i = (py as u32 * w + px as u32) as usize;
                        assert!(!b[i] || across, "hdr {hdr} round {round}: B at {px},{py}");
                        tighter_misses |= b[i] && !any(tx - k + 1, tx + k - 1, ty, ty);
                        assert!(!out[i] || down, "hdr {hdr} round {round}: result at {px},{py}");
                        // Where the pass down does not run, A itself is 0.
                        assert!(down || !a[i]);
                    }
                }
            }
            assert!(tighter_misses, "hdr {hdr}: k - 1 tiles would do");
        }
    }

    /// The flagging entries draw what the plain ones draw: the same
    /// functions, then the flag; and glow_skip.wgsl's tile and the vertex
    /// shaders' uv are those of glow_skip.rs and vs_main.
    #[test]
    fn flagging_prefilter_and_tested_blur_are_the_plain_passes() {
        let skip = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/glow_skip.wgsl"));
        let need = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/glow_need.wgsl"));
        let hp = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/high_precision.wgsl"));
        let scale = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/post/post_scale.wgsl"));
        let post = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/native/post/post.wgsl"));
        assert!(hp.contains(
            "fn fs_overbright_mrt_hdr(in: VsOut) -> OverbrightOut {\n    return overbright_mrt_hdr(in.uv);\n}"
        ));
        assert!(scale.contains(
            "fn fs_overbright_sat(in: VsOut) -> @location(0) vec4<f32> {\n    return overbright_sat(in.uv);\n}"
        ));
        assert!(
            skip.contains(
                "    let out = overbright_mrt_hdr(in.uv);\n    glow_flag(in.position, out.c);\n    return out;"
            )
        );
        assert!(skip.contains("    glow_flag(in.position, tap(in.uv));\n    return overbright_sat(in.uv);"));
        // The flag's test is overbright's: x = sat(4 (t0 - OVERBRIGHTOFFSET)) is 0
        // where no channel is above the offset, and A = (x g)^2 with it.
        assert!(
            post.contains("let x = clamp((t0.rgb - vec3<f32>(pass_.params.z)) * 4.0, vec3<f32>(0.0), vec3<f32>(1.0));")
        );
        assert!(skip.contains("if any(t0.rgb > vec3<f32>(pass_.params.z)) {"));
        for text in [skip, need] {
            assert!(text.contains(&format!("const GLOW_TILE: u32 = {GLOW_TILE}u;")));
        }
        // vs_at is vs_main with another z.
        let vs_main = post.split("fn vs_main(").nth(1).unwrap();
        let vs_at = skip.split("fn vs_at(").nth(1).unwrap();
        for line in ["let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));", "out.uv = vec2<f32>(p.x, 1.0 - p.y);"] {
            assert!(vs_main.contains(line) && vs_at.contains(line), "{line}");
        }
        assert!(vs_at.contains("out.position = vec4<f32>(p * 2.0 - 1.0, z, 1.0);"));
        assert!(vs_main.contains("out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);"));
        // Both modules parse and validate (naga, no GPU).
        let full = format!(
            "{}\n{skip}",
            crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::PostFx)
        );
        for s in [full.as_str(), need] {
            let module = wgpu::naga::front::wgsl::parse_str(s).expect("parse");
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("valid");
        }
    }
}
