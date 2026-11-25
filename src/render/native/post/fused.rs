//! The fused post passes and their self-check (moved from render/postfx.rs).

use super::*;

/// The post passes' self-check under way ([`PostFx::start_check`]).
pub(super) struct Check {
    /// The command buffers still to submit, one a frame.
    chunks: std::collections::VecDeque<wgpu::CommandBuffer>,
    done: std::sync::Arc<crate::render::probes::MapFlag>,
    read: wgpu::Buffer,
}

/// Counts the texels where two images differ, and the largest difference.
const COMPARE_WGSL: &str = "
@group(0) @binding(0) var a: texture_2d<f32>;
@group(0) @binding(1) var b: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<atomic<u32>, 2>;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(a);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let x = textureLoad(a, vec2<i32>(id.xy), 0);
    let y = textureLoad(b, vec2<i32>(id.xy), 0);
    if any(x != y) {
        atomicAdd(&out[0], 1u);
        let d = abs(x - y);
        atomicMax(&out[1], u32(round(max(max(d.x, d.y), max(d.z, d.w)) * 255.0)));
    }
}
";

/// The self-check's verdict file for this adapter, driver and build, in
/// the asset cache's folder (`cache::dir`: `NFSU2_CACHE_DIR` first, none
/// with `NFSU2_NO_CACHE`; docs/developing/asset-cache.md). None: no cache.
pub(super) fn check_cache_file(gpu: &Gpu) -> Option<std::path::PathBuf> {
    let base = crate::cache::dir()?;
    let info = gpu.adapter.get_info();
    let adapter = format!(
        "{}|{}|{}|{}|{}|{:?}",
        info.name, info.vendor, info.device, info.driver, info.driver_info, info.backend
    );
    Some(check_cache_path(&base, &adapter, crate::BUILD_ID.0, check_stages(), check_variants()))
}

/// `<base>/post-check-<hash>.<build id>.txt`. The hash keys the verdict to
/// the adapter and driver (`adapter`), the shader texts the check runs and
/// what the check compares (`NFSU2_CHECK_STAGES`' passes, the `q`
/// variants tried); the build id (every library source, as the cache's
/// entries: docs/developing/asset-cache.md) to the code that judges it, so a change
/// of the check's logic with the same shader text reads no old verdict.
pub(super) fn check_cache_path(
    base: &std::path::Path,
    adapter: &str,
    build: u64,
    stages: u32,
    variants: u8,
) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    (
        adapter,
        env!("CARGO_PKG_VERSION"),
        crate::render::core::shader::registry::text(crate::render::core::shader::registry::ShaderId::PostFx),
        BOUNDS_WGSL,
        COMPARE_WGSL,
        stages,
        variants,
    )
        .hash(&mut h);
    base.join(format!("post-check-{:016x}.{build:016x}.txt", h.finish()))
}

/// The variants the self-check tries: the measured steps only, or both
/// (`NFSU2_POST_CHECK=all`, for the report).
fn check_variants() -> u8 {
    if crate::render::debug_env::nfsu2_post_check_all() || check_stages() != 3 { VARIANTS } else { 1 }
}

/// `q`'s variants (postfx.wgsl's MEASURED).
const VARIANTS: u8 = 2;

fn variant_name(v: u8) -> &'static str {
    ["", "measured steps", "to nearest, ties to even"][v as usize]
}

/// `NFSU2_CHECK_STAGES=1|2|4`: the self-check compares only the first
/// bleach-bypass passes (a diagnosis of where the fused pass differs; the
/// fused pass then stays off).
pub(super) fn check_stages() -> u32 {
    crate::render::debug_env::once::check_stages().filter(|n| (1..=4).contains(n)).unwrap_or(3)
}

/// `unorm_bounds`'s size: 2 × 256 u32.
pub(super) const BOUNDS_SIZE: u64 = 2048;

/// The bisection of [`PostFx::measure_bounds`]: each step writes the
/// midpoint of k's range to texel k of an 8-bit target, then narrows the
/// range by what the target holds.
const BOUNDS_WGSL: &str = "
@group(0) @binding(0) var<storage, read> range: array<u32, 512>;
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let k = u32(p.x);
    let mid = range[k] + (range[256u + k] - range[k]) / 2u;
    return vec4<f32>(bitcast<f32>(mid));
}
@group(0) @binding(0) var<storage, read_write> narrow: array<u32, 512>;
@group(0) @binding(1) var written: texture_2d<f32>;
@compute @workgroup_size(64)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    let k = id.x;
    if k >= 255u {
        return;
    }
    let mid = narrow[k] + (narrow[256u + k] - narrow[k]) / 2u;
    let got = u32(round(textureLoad(written, vec2<i32>(i32(k), 0), 0).r * 255.0));
    if got > k {
        narrow[256u + k] = mid;
    } else {
        narrow[k] = mid;
    }
}
";

impl PostFx {
    /// Measures where the device's 8-bit render target writes step from k
    /// to k + 1 (`unorm_bounds`), by bisection over the floats from 0.05
    /// of a step below each exact tie to 0.05 above: 32 steps of a 256×1
    /// pass and a compute pass, on the GPU (nothing read back here).
    pub(super) fn measure_bounds(&mut self, gpu: &Gpu) {
        let device = &gpu.device;
        let mut range = vec![0u32; 512];
        for k in 0..256usize {
            range[k] = (((k as f64 + 0.45) / 255.0) as f32).to_bits();
            range[256 + k] = (((k as f64 + 0.55) / 255.0) as f32).to_bits();
        }
        gpu.queue.write_buffer(&self.bounds, 0, bytemuck::cast_slice(&range));
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("unorm bounds"),
            source: wgpu::ShaderSource::Wgsl(BOUNDS_WGSL.into()),
        });
        let write = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some("unorm bounds"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(INTERMEDIATE.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        let narrow = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
            label: Some("unorm bounds"),
            layout: None,
            module: &module,
            entry_point: Some("step"),
            compilation_options: Default::default(),
            cache: gpu.disk_pipeline_cache(),
        });
        let target = texture(gpu, "unorm bounds", (256, 1), INTERMEDIATE);
        let view = target.create_view(&Default::default());
        let write_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &write.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.bounds.as_entire_binding() }],
        });
        let narrow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &narrow.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.bounds.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        for _ in 0..32 {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("unorm bounds"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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
                pass.set_pipeline(&write);
                pass.set_bind_group(0, &write_group, &[]);
                pass.draw(0..3, 0..1);
            }
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&narrow);
            pass.set_bind_group(0, &narrow_group, &[]);
            pass.dispatch_workgroups(4, 1, 1);
        }
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn fused_status(&self) -> Option<&str> {
        self.report.as_deref()
    }

    /// The self-check (R3): the three bleach-bypass passes against
    /// `fs_bleach_fused`, and overbright's C and A against
    /// `fs_overbright_mrt`, for both of `q`'s tie rules, on every 24-bit
    /// colour (four 4096×1024 tiles) and two sets of constants (the front
    /// end's and a tinted game frame's), counted on the GPU (mismatching
    /// texels and the largest difference, in 8-bit steps).
    pub(super) fn start_check(&mut self, gpu: &Gpu) -> Check {
        self.measure_bounds(gpu);
        const W: u32 = 4096;
        const H: u32 = 1024;
        let device = &gpu.device;
        let pattern = self.pipeline_v(gpu, "fs_pattern", INTERMEDIATE, 1, Blend::Replace, 0);
        let b0 = self.pipeline(gpu, "fs_bleach0", INTERMEDIATE, Blend::Replace);
        let b1 = self.pipeline(gpu, "fs_bleach1", INTERMEDIATE, Blend::Replace);
        let b2 = self.pipeline(gpu, "fs_bleach2", INTERMEDIATE, Blend::Replace);
        let copy = self.pipeline(gpu, "fs_filter", INTERMEDIATE, Blend::Replace);
        let over = self.pipeline(gpu, "fs_overbright", INTERMEDIATE, Blend::Replace);
        let fused: Vec<(usize, usize)> = (1..=check_variants())
            .map(|v| {
                (
                    self.pipeline_v(gpu, "fs_bleach_fused", INTERMEDIATE, 1, Blend::Replace, v),
                    self.pipeline_v(gpu, "fs_overbright_mrt", INTERMEDIATE, 2, Blend::Replace, v),
                )
            })
            .collect();
        let tex = |label, w, h| texture(gpu, label, (w, h), INTERMEDIATE);
        let src = tex("check source", W, H);
        let (u0, u1, f) = (tex("check 0", W, H), tex("check 1", W, H), tex("check fused", W, H));
        let (hw, hh) = (W / 2, H / 2);
        let (hc, ha, hc2, ha2) = (
            tex("check C", hw, hh),
            tex("check A", hw, hh),
            tex("check C fused", hw, hh),
            tex("check A fused", hw, hh),
        );
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let sets = [
            (Frame::FRONT_END, OverbrightLevel(OverbrightLevel::target(true))),
            (
                Frame { front_end: false, bleach: bleach_amount(0.7), desaturate: 0.35, tint: [0.6, 0.45, 0.3] },
                OverbrightLevel(OverbrightLevel::target(false)),
            ),
        ];
        // Slots 0..4: the tiles; 4, 5: the sets.
        let uniforms = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("post check"),
                size: SLOT * 6,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let mut data = vec![0u8; (SLOT * 6) as usize];
        let mut put = |slot: usize, u: PassUniform| {
            data[slot * SLOT as usize..slot * SLOT as usize + std::mem::size_of::<PassUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&u))
        };
        for t in 0..4 {
            put(
                t,
                PassUniform {
                    weights: [[0.0; 4]; 4],
                    offsets: [[0.0; 4]; 4],
                    params: [t as f32, 0.0, 0.0, 0.0],
                    grey: [0.0; 4],
                    tint: [0.0; 4],
                },
            );
        }
        for (i, (frame, level)) in sets.iter().enumerate() {
            let [r, g, b] = frame.tint_color();
            let mut u = PassUniform {
                weights: [[0.0; 4]; 4],
                offsets: [[0.0; 4]; 4],
                params: [1.0, frame.desaturate, f32::from(level.offset()) / 255.0 * 0.5, 0.0],
                grey: OVERBRIGHT_GREY,
                tint: [r, g, b, SATURATION_SCALE],
            };
            u.weights[0] = [1.0; 4];
            put(4 + i, u);
        }
        gpu.queue.write_buffer(&uniforms, 0, &data);
        let gains: Vec<wgpu::Texture> = sets
            .iter()
            .map(|(frame, _)| {
                let t = tex("check gain map", GAIN_SIZE as u32, GAIN_SIZE as u32);
                write_texture(gpu, &t, &gain_map(frame.bleach), GAIN_SIZE as u32, GAIN_SIZE as u32);
                t
            })
            .collect();
        let (ramp_view, lut_view) = (view(&self.ramp), view(&self.lut));
        let group = |t: &wgpu::Texture, gain: &wgpu::Texture| {
            let (v, g) = (view(t), view(gain));
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post check"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&v) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&g) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ramp_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniforms,
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<PassUniform>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&lut_view) },
                    wgpu::BindGroupEntry { binding: 6, resource: self.bounds.as_entire_binding() },
                ],
            })
        };
        // The comparison: a compute pass per pair, into result slot r
        // (mismatches, largest difference).
        let compare_shader = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("post check compare"),
            source: wgpu::ShaderSource::Wgsl(COMPARE_WGSL.into()),
        });
        let compare = crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
            label: Some("post check compare"),
            layout: None,
            module: &compare_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: gpu.disk_pipeline_cache(),
        });
        let compare_layout = compare.get_bind_group_layout(0);
        let results = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post check results"),
            size: 256 * 3 * VARIANTS as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post check read"),
            size: 256 * 3 * VARIANTS as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        // A command buffer per tile, submitted one a frame
        // ([`PostFx::poll_check`]), so no frame waits long for the GPU.
        let mut chunks = std::collections::VecDeque::new();
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&results, 0, None);
        let compare_pair = |encoder: &mut wgpu::CommandEncoder, a: &wgpu::Texture, b: &wgpu::Texture, r: u64| {
            let (va, vb) = (view(a), view(b));
            let g = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post check compare"),
                layout: &compare_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&va) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&vb) },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &results,
                            offset: r * 256,
                            size: wgpu::BufferSize::new(8),
                        }),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&compare);
            pass.set_bind_group(0, &g, &[]);
            pass.dispatch_workgroups(a.width().div_ceil(8), a.height().div_ceil(8), 1);
        };
        let p = |i: usize| &self.pipelines[i].1;
        let (vsrc, vu0, vu1, vf, vhc, vha, vhc2, vha2) =
            (view(&src), view(&u0), view(&u1), view(&f), view(&hc), view(&ha), view(&hc2), view(&ha2));
        for (si, gain) in gains.iter().enumerate() {
            let slot = 4 + si as u32;
            let (g_src, g_u0, g_u1, g_hc) = (group(&src, gain), group(&u0, gain), group(&u1, gain), group(&hc, gain));
            for tile in 0..4u32 {
                full_pass(&mut encoder, "check pattern", p(pattern), &[&vsrc], &g_hc, tile, None, None);
                full_pass(&mut encoder, "check", p(b0), &[&vu0], &g_src, slot, None, None);
                // (`NFSU2_CHECK_STAGES=n`: the first n passes only.)
                match check_stages() {
                    1 | 4 => {}
                    2 => {
                        full_pass(&mut encoder, "check", p(b1), &[&vu1], &g_u0, slot, None, None);
                    }
                    _ => {
                        full_pass(&mut encoder, "check", p(b1), &[&vu1], &g_u0, slot, None, None);
                        full_pass(&mut encoder, "check", p(b2), &[&vu0], &g_u1, slot, None, None);
                    }
                }
                full_pass(&mut encoder, "check", p(copy), &[&vhc], &g_src, slot, None, None);
                full_pass(&mut encoder, "check", p(over), &[&vha], &g_hc, slot, None, None);
                for (v, &(fb, fo)) in fused.iter().enumerate() {
                    full_pass(&mut encoder, "check fused", p(fb), &[&vf], &g_src, slot, None, None);
                    compare_pair(&mut encoder, if check_stages() == 2 { &u1 } else { &u0 }, &f, v as u64 * 3);
                    full_pass(&mut encoder, "check fused", p(fo), &[&vhc2, &vha2], &g_src, slot, None, None);
                    compare_pair(&mut encoder, &hc, &hc2, v as u64 * 3 + 1);
                    compare_pair(&mut encoder, &ha, &ha2, v as u64 * 3 + 2);
                }
                chunks.push_back(
                    std::mem::replace(&mut encoder, device.create_command_encoder(&Default::default())).finish(),
                );
            }
        }
        encoder.copy_buffer_to_buffer(&results, 0, &read, 0, 256 * 3 * VARIANTS as u64);
        chunks.push_back(encoder.finish());
        Check { chunks, done: Default::default(), read }
    }

    /// Takes the self-check's result once in: the first `q` variant with
    /// no mismatch anywhere turns the fused passes on.
    pub(super) fn poll_check(&mut self, gpu: &Gpu) {
        let Some(check) = &mut self.check else { return };
        if let Some(chunk) = check.chunks.pop_front() {
            gpu.queue.submit([chunk]);
            if check.chunks.is_empty() {
                let flag = check.done.clone();
                check.read.slice(..).map_async(wgpu::MapMode::Read, move |r| flag.set(r.is_ok()));
            }
            return;
        }
        let state = check.done.state();
        if state == crate::render::probes::MapState::Pending {
            return;
        }
        // A failed read-back (device lost) leaves the check unverified: the
        // separate passes stay in use, nothing is cached.
        let words: Option<Vec<u32>> = (state == crate::render::probes::MapState::Ready)
            .then(|| {
                check
                    .read
                    .slice(..)
                    .get_mapped_range()
                    .ok()
                    .map(|d| d.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect())
            })
            .flatten();
        if words.is_some() {
            check.read.unmap();
        }
        let Some(words) = words.filter(|w| w.len() >= check_variants() as usize * 3 * 64) else {
            self.check = None;
            self.report = Some("separate passes (self-check not verified: the read-back failed)".to_string());
            log::warn!("post passes: {}", self.report.as_deref().unwrap_or(""));
            return;
        };
        self.check = None;
        let kinds = ["bleach bypass", "overbright C", "overbright A"];
        let mut lines = Vec::new();
        for v in 0..check_variants() as usize {
            let r: Vec<(u32, u32)> = (0..3).map(|k| (words[(v * 3 + k) * 64], words[(v * 3 + k) * 64 + 1])).collect();
            if r.iter().all(|&(n, _)| n == 0) && self.fused.is_none() && check_stages() == 3 {
                self.fused = Some(v as u8 + 1);
            }
            let detail: Vec<String> = r
                .iter()
                .zip(kinds)
                .map(
                    |(&(n, d), k)| {
                        if n == 0 { format!("{k} equal") } else { format!("{k} {n} texels differ (up to ±{d})") }
                    },
                )
                .collect();
            lines.push(format!("{}: {}", variant_name(v as u8 + 1), detail.join(", ")));
        }
        self.report = Some(format!(
            "{} (self-check on all 2^24 colours, two constant sets: {})",
            if let Some(v) = self.fused {
                format!("fused, {}", variant_name(v))
            } else {
                "separate passes".to_string()
            },
            lines.join("; ")
        ));
        log::info!("post passes: {}", self.report.as_deref().unwrap_or(""));
        if let Some(path) = &self.cache_file {
            let verdict = if self.fused.is_some() { "fused" } else { "separate" };
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
            let _ = std::fs::write(path, format!("{verdict}\n{}\n", self.report.as_deref().unwrap_or("")));
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    /// The verdict's file: in the folder given (the asset cache's), named
    /// with the build id, and another file for another build, adapter,
    /// set of compared passes or variants.
    #[test]
    fn verdict_file_is_keyed_to_build_adapter_and_check() {
        let base = std::path::Path::new("cache-folder");
        let one = check_cache_path(base, "gpu a", 0x1234, 3, 1);
        assert_eq!(one.parent(), Some(base));
        let name = one.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("post-check-") && name.ends_with(".0000000000001234.txt"), "{name}");
        assert_eq!(one, check_cache_path(base, "gpu a", 0x1234, 3, 1), "the same run, the same file");
        for other in [
            check_cache_path(base, "gpu a", 0x1235, 3, 1),
            check_cache_path(base, "gpu b", 0x1234, 3, 1),
            check_cache_path(base, "gpu a", 0x1234, 2, 2),
            check_cache_path(base, "gpu a", 0x1234, 3, 2),
        ] {
            assert_ne!(one, other);
        }
    }

    /// The folder is the asset cache's (`NFSU2_CACHE_DIR` first): a run
    /// with its own cache folder never reads or writes another's verdict.
    #[test]
    fn verdict_file_lives_in_the_asset_cache_folder() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/native/post/fused.rs"));
        let body = &src[src.find("fn check_cache_file").unwrap()..];
        let body = &body[..body.find("\n}\n").unwrap()];
        assert!(body.contains("crate::cache::dir()?"), "{body}");
        assert!(!body.contains("platform::dirs"), "{body}");
    }
}
