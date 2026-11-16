//! The temporal resolve as a compute pass (docs/temporal.md "The resolve
//! as a compute pass"): TAA only (the input at the output's size), where it
//! replaces the fragment resolve into the history. The same `resolve` of
//! temporal.wgsl per output pixel, written through a storage texture, and
//! the car-motion loop skipping, per 8×8 tile, the cars whose screen
//! rectangle ([`car_rect`]) the tile does not touch: the same values in the
//! same order, so the same history.
//!
//! Chosen once, when the pass is made, on integrated GPUs (where it was
//! measured faster; on the desktop GPU it was as fast within 0.01 ms at
//! 1920×1080 and 0.03 ms slower at a 3840×2160 race start, so discrete
//! GPUs keep the fragment pass; `NFSU2_TAA_COMPUTE=1` asks for it on any
//! device, `=0` on none). Where the device has no storage binding for
//! [`HISTORY_FORMAT`], or where the self-check ([`self_check`]: both
//! passes on a made-up frame, the two histories compared byte for byte)
//! does not give the same bytes, the fragment pass stays. The verdict is
//! kept per device.

use super::{GpuCar, HISTORY_FORMAT, MAX_CARS, Uniforms};
use crate::render::Gpu;
use glam::{Mat4, Vec2, Vec3};

/// The compute module: temporal.wgsl with the tile's car test in its
/// car loop, and temporal_cs.wgsl.
pub(super) fn source() -> String {
    let base = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/temporal.wgsl"));
    const LOOP: &str = "let sphere = cars[i].sphere;";
    assert_eq!(base.matches(LOOP).count(), 1, "temporal.wgsl's car loop");
    let mut s = base
        .replace(LOOP, "if tile_misses(i) {\n            continue;\n        }\n        let sphere = cars[i].sphere;");
    s.push_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/temporal_cs.wgsl")));
    s
}

/// The output-pixel rectangle (x0, y0, x1, y1, inclusive) outside which no
/// output pixel can take the car whose bounding sphere is `centre`
/// (camera-relative) and `radius`: the sphere's box projected by
/// `to_clip` (camera-relative → this frame's jittered clip) into the
/// input's pixels, mapped to output pixels as `resolve` maps them (the
/// cross's closest sample lies within a pixel and a half of the output
/// pixel's place), with a margin of three input pixels and four output
/// pixels besides. The whole plane where any corner of the box is not in
/// front of the eye.
pub fn car_rect(
    to_clip: Mat4,
    centre: Vec3,
    radius: f32,
    input: (u32, u32),
    output: (u32, u32),
    jitter: Vec2,
) -> [i32; 4] {
    const ALL: [i32; 4] = [i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2];
    let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for k in 0..8 {
        let side = |bit: i32| if k & bit == 0 { -radius } else { radius };
        let c = to_clip * (centre + Vec3::new(side(1), side(2), side(4))).extend(1.0);
        if !(c.w >= 1e-3) {
            return ALL;
        }
        let px = Vec2::new((c.x / c.w * 0.5 + 0.5) * input.0 as f32, (0.5 - c.y / c.w * 0.5) * input.1 as f32);
        lo = lo.min(px);
        hi = hi.max(px);
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return ALL;
    }
    let scale = Vec2::new(output.0 as f32 / input.0.max(1) as f32, output.1 as f32 / input.1.max(1) as f32);
    let pad = scale * 3.0 + 4.0;
    let lo = ((lo - jitter) * scale - pad).floor();
    let hi = ((hi - jitter) * scale + pad).ceil();
    let i = |v: f32| v.clamp(-1.0e8, 1.0e8) as i32;
    [i(lo.x), i(lo.y), i(hi.x), i(hi.y)]
}

/// The compute pass's objects.
pub(super) struct Compute {
    /// Group 0: the fragment pass's bindings, seen by the compute stage.
    pub(super) inputs: wgpu::BindGroupLayout,
    /// Group 1: the history written, the cars' rectangles.
    pub(super) outputs: wgpu::BindGroupLayout,
    pub(super) pipeline: wgpu::ComputePipeline,
    pub(super) rects: wgpu::Buffer,
}

/// Group 0's entries for `visibility` (the fragment pass's layout with
/// FRAGMENT).
pub(super) fn input_entries(visibility: wgpu::ShaderStages) -> [wgpu::BindGroupLayoutEntry; 6] {
    let tex = |binding, sample_type| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let buffer = |binding, size: usize| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size as u64),
        },
        count: None,
    };
    [
        tex(0, wgpu::TextureSampleType::Float { filterable: false }),
        tex(1, wgpu::TextureSampleType::Depth),
        tex(2, wgpu::TextureSampleType::Float { filterable: true }),
        wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        buffer(4, std::mem::size_of::<Uniforms>()),
        buffer(5, std::mem::size_of::<GpuCar>() * MAX_CARS),
    ]
}

/// Whether the device can run the compute pass at all.
fn supported(gpu: &Gpu) -> bool {
    let usages = gpu.adapter.get_texture_format_features(HISTORY_FORMAT).allowed_usages;
    let limits = gpu.device.limits();
    usages.contains(wgpu::TextureUsages::STORAGE_BINDING)
        && limits.max_storage_textures_per_shader_stage >= 1
        && limits.max_compute_invocations_per_workgroup >= 64
        && limits.max_compute_workgroup_size_x >= 8
        && limits.max_compute_workgroup_size_y >= 8
}

/// The self-check's verdict per device ([`Gpu::build_tag`]).
static VERDICTS: std::sync::Mutex<Vec<(u64, bool)>> = std::sync::Mutex::new(Vec::new());

impl Compute {
    /// The compute pass, or None where the fragment pass stays (see the
    /// module). `fragment`: the fragment resolve and its layout, for the
    /// self-check.
    pub(super) fn new(
        gpu: &Gpu,
        fragment: &wgpu::RenderPipeline,
        fragment_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Option<Compute> {
        let wanted = match crate::render::debug_env::nfsu2_taa_compute() {
            Some(0) => false,
            Some(_) => true,
            None => gpu.adapter.get_info().device_type == wgpu::DeviceType::IntegratedGpu,
        };
        if !wanted {
            return None;
        }
        Self::checked(gpu, fragment, fragment_layout, sampler)
    }

    /// The compute pass on any device where it runs and passes the
    /// self-check.
    fn checked(
        gpu: &Gpu,
        fragment: &wgpu::RenderPipeline,
        fragment_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Option<Compute> {
        if !supported(gpu) {
            return None;
        }
        let known = VERDICTS.lock().unwrap().iter().find(|(t, _)| *t == gpu.build_tag()).map(|(_, v)| *v);
        if known == Some(false) {
            return None;
        }
        let device_cache = crate::render::core::device_cache::bind_group_layout;
        let inputs = device_cache(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("temporal compute inputs"),
                entries: &input_entries(wgpu::ShaderStages::COMPUTE),
            },
        );
        let outputs = device_cache(
            gpu,
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("temporal compute outputs"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: HISTORY_FORMAT,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(16 * MAX_CARS as u64),
                        },
                        count: None,
                    },
                ],
            },
        );
        let module = crate::render::core::device_cache::module(
            gpu,
            wgpu::ShaderModuleDescriptor {
                label: Some("temporal compute"),
                source: wgpu::ShaderSource::Wgsl(source().into()),
            },
        );
        let layout = crate::render::core::device_cache::pipeline_layout(
            gpu,
            &wgpu::PipelineLayoutDescriptor {
                label: Some("temporal compute"),
                bind_group_layouts: &[Some(&inputs), Some(&outputs)],
                immediate_size: 0,
            },
        );
        let pipeline = crate::render::core::device_cache::compute_pipeline(
            gpu,
            &wgpu::ComputePipelineDescriptor {
                label: Some("temporal resolve (compute)"),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("cs_resolve"),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            },
        );
        let rects = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("temporal car rectangles"),
            size: 16 * MAX_CARS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let c = Compute { inputs, outputs, pipeline, rects };
        if known.is_none() {
            let same = self_check(gpu, &c, fragment, fragment_layout, sampler);
            match &same {
                Ok(true) => log::info!("temporal resolve: the compute pass (self-check: the same history)"),
                Ok(false) => log::info!("temporal resolve: the fragment pass (self-check: the compute pass differs)"),
                Err(e) => log::warn!("temporal resolve: the fragment pass (self-check failed: {e})"),
            }
            let same = matches!(same, Ok(true));
            VERDICTS.lock().unwrap().push((gpu.build_tag(), same));
            if !same {
                return None;
            }
        }
        Some(c)
    }

    /// Group 1 for a frame: `history` written, the rectangles.
    pub(super) fn outputs_group(&self, gpu: &Gpu, history: &wgpu::TextureView) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temporal compute outputs"),
            layout: &self.outputs,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(history) },
                wgpu::BindGroupEntry { binding: 1, resource: self.rects.as_entire_binding() },
            ],
        })
    }

    /// The resolve of an `output`-sized frame into `group1`'s history.
    pub(super) fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        inputs: &wgpu::BindGroup,
        outputs: &wgpu::BindGroup,
        output: (u32, u32),
    ) {
        let mut p = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("temporal resolve"),
            timestamp_writes: None,
        });
        p.set_pipeline(&self.pipeline);
        p.set_bind_group(0, inputs, &[]);
        p.set_bind_group(1, outputs, &[]);
        p.dispatch_workgroups(output.0.div_ceil(8), output.1.div_ceil(8), 1);
    }
}

/// The self-check's frame size (not a multiple of the tile: the edge
/// tiles' test is in it).
const CHECK: u32 = 44;

/// A made-up frame through both passes, the two histories compared byte
/// for byte: colours and history over the half-float range the scene
/// uses (to 16), a valid history reprojected inside the image, two cars
/// (one over the left half with a wheel, one fresh with a small sphere
/// whose rectangle leaves most tiles out), a span of 1.7 ticks. Ok(true):
/// the same bytes.
fn self_check(
    gpu: &Gpu,
    c: &Compute,
    fragment: &wgpu::RenderPipeline,
    fragment_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> Result<bool, String> {
    let device = &gpu.device;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let n = CHECK;
    let extent = wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 };
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("temporal self-check"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    let colour = texture(HISTORY_FORMAT, sampled);
    let last = texture(HISTORY_FORMAT, sampled);
    let depth = texture(
        crate::render::DEPTH_FORMAT,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let by_fragment = texture(HISTORY_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
    let by_compute = texture(HISTORY_FORMAT, wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC);
    // Half floats from a fixed xorshift: exponents 2^-14 .. 2^3 (alpha
    // below 1), any mantissa.
    let mut state = 0x9E37_79B9u32;
    let mut half = |top: u32| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let exponent = 1 + (state >> 10) % top;
        (((exponent << 10) | (state & 0x3FF)) as u16).to_le_bytes()
    };
    let mut texels =
        || -> Vec<u8> { (0..n * n).flat_map(|_| [half(17), half(17), half(17), half(14)].concat()).collect() };
    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 8), rows_per_image: None };
    gpu.queue.write_texture(colour.as_image_copy(), &texels(), layout, extent);
    gpu.queue.write_texture(last.as_image_copy(), &texels(), layout, extent);
    // The frame: clip = camera-relative (rel = ndc at the depth), the last
    // frame's clip a little to the side.
    let clip_to_rel = Mat4::IDENTITY;
    let prev = Mat4::from_translation(Vec3::new(0.03, -0.02, 0.0));
    let size = [n as f32, n as f32, 1.0 / n as f32, 1.0 / n as f32];
    let jitter = Vec2::new(0.21, -0.17);
    let u = Uniforms {
        clip_to_rel: clip_to_rel.to_cols_array_2d(),
        prev_rel_to_clip: prev.to_cols_array_2d(),
        input: size,
        output: size,
        jitter: [jitter.x, jitter.y, 1.0, 0.0],
        params: [super::BLEND_TAA, 2.0, 1.0, 1.0],
        debug: [0.0, super::VELOCITY_BLEND, super::VELOCITY_BLEND_MAX, 1.0],
        rate: [1.7, 0.0, 0.0, 0.0],
    };
    let d = 0.6;
    let car = |motion: Mat4, lo: [f32; 4], hi: [f32; 4], sphere: (Vec3, f32)| GpuCar {
        inv: Mat4::IDENTITY.to_cols_array_2d(),
        motion: motion.to_cols_array_2d(),
        lo,
        hi,
        wheel: [[-0.5, 0.0, d, 0.3], [2.0, 2.0, 2.0, 0.1], [2.0, 2.0, 2.0, 0.1], [2.0, 2.0, 2.0, 0.1]],
        wheel_hw: [0.5, 0.1, 0.1, 0.1],
        sphere: [sphere.0.x, sphere.0.y, sphere.0.z, sphere.1 * sphere.1],
    };
    let mut cars: Vec<GpuCar> = vec![bytemuck::Zeroable::zeroed(); MAX_CARS];
    let spheres = [(Vec3::new(0.0, 0.0, d), 2.0), (Vec3::new(0.65, -0.5, d), 0.3)];
    cars[0] = car(
        Mat4::from_translation(Vec3::new(0.05, 0.04, 0.0)),
        [-1.0, -1.0, -2.0, 0.0],
        [0.1, 1.0, 2.0, 0.0],
        spheres[0],
    );
    cars[1] = car(Mat4::IDENTITY, [0.3, -1.0, -2.0, 1.0], [1.0, 0.0, 2.0, 0.0], spheres[1]);
    let mut rects = [[0i32; 4]; MAX_CARS];
    for (r, (centre, radius)) in rects.iter_mut().zip(spheres) {
        *r = car_rect(clip_to_rel.inverse(), centre, radius, (n, n), (n, n), jitter);
    }
    let buffer = |bytes: &[u8]| {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("temporal self-check"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&b, 0, bytes);
        b
    };
    let uniforms = buffer(bytemuck::bytes_of(&u));
    let car_buffer = buffer(bytemuck::cast_slice(&cars));
    gpu.queue.write_buffer(&c.rects, 0, bytemuck::cast_slice(&rects));
    let view = |t: &wgpu::Texture| t.create_view(&Default::default());
    let (colour, last, depth_view) = (view(&colour), view(&last), view(&depth));
    let group = |layout: &wgpu::BindGroupLayout| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temporal self-check"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&colour) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&last) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: car_buffer.as_entire_binding() },
            ],
        })
    };
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("temporal self-check") });
    // The depth: a plane at `d`.
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("temporal self-check depth"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_view,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(d), store: wgpu::StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    super::pass(&mut encoder, "temporal self-check (fragment)", &view(&by_fragment), fragment, &group(fragment_layout));
    c.dispatch(&mut encoder, &group(&c.inputs), &c.outputs_group(gpu, &view(&by_compute)), (n, n));
    let row = (n * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let read = |t: &wgpu::Texture, encoder: &mut wgpu::CommandEncoder| {
        let b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("temporal self-check read-back"),
            size: u64::from(row * n),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            t.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &b,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
            },
            extent,
        );
        b
    };
    let a = read(&by_fragment, &mut encoder);
    let b = read(&by_compute, &mut encoder);
    let index = gpu.queue.submit([encoder.finish()]);
    if let Some(e) = pollster::block_on(scope.pop()) {
        return Err(format!("{e}"));
    }
    for buffer in [&a, &b] {
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    }
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(std::time::Duration::from_secs(10)) })
        .map_err(|e| format!("{e}"))?;
    let bytes = |b: &wgpu::Buffer| -> Result<Vec<u8>, String> {
        let m = b.slice(..).get_mapped_range().map_err(|e| format!("{e:?}"))?;
        Ok((0..n as usize).flat_map(|y| m[y * row as usize..][..n as usize * 8].to_vec()).collect())
    };
    let (a, b) = (bytes(&a)?, bytes(&b)?);
    Ok(a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compute module is temporal.wgsl with one change, the tile's
    /// test before the car loop's sphere test, and parses.
    #[test]
    fn the_compute_module_is_the_fragment_module_and_the_tile_test() {
        let base = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/temporal.wgsl"));
        let s = source();
        let (head, tail) = s.split_once("if tile_misses(i) {").expect("the tile test");
        assert!(base.starts_with(head));
        let rest = tail.split_once("let sphere = cars[i].sphere;").expect("the sphere test after it").1;
        assert!(base.contains(rest.split("\n// The temporal resolve as a compute pass").next().unwrap()));
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(&s).expect("the compute module parses");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .expect("the compute module validates");
    }

    /// Every point of a car's sphere, seen this frame (jittered) by a
    /// camera at several turns, projects into the car's rectangle with the
    /// cross's reach to spare, near and far; a sphere around or behind the
    /// eye covers the whole plane.
    #[test]
    fn a_car_sphere_lies_inside_its_rectangle() {
        let size = (1280u32, 720u32);
        let aspect = size.0 as f32 / size.1 as f32;
        let mut rng = 0x1234_5678u32;
        let mut unit = || {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (rng >> 8) as f32 / (1u32 << 24) as f32
        };
        for turn in 0..6 {
            let mut cam = crate::render::camera::Camera::new(Vec3::new(10.0, -40.0, 3.0), turn as f32 * 1.1, -0.05);
            cam.game_fov = Some(1.248_277);
            let jitter = Vec2::new(0.31, -0.42);
            let input = super::super::FrameInput {
                camera: cam,
                aspect,
                jitter,
                reversed: true,
                cars: Vec::new(),
                cut: false,
                key: 0,
            };
            let r = super::super::reprojection(&input, super::super::jitter_ndc(jitter, size), None);
            let to_clip = r.clip_to_rel.inverse();
            for distance in [4.0f32, 15.0, 60.0, 300.0] {
                let centre = cam.forward() * distance + cam.right() * (distance * 0.2) - Vec3::Z;
                let radius = 2.6;
                let rect = car_rect(to_clip, centre, radius, size, size, jitter);
                for _ in 0..2000 {
                    let p = centre
                        + Vec3::new(unit() * 2.0 - 1.0, unit() * 2.0 - 1.0, unit() * 2.0 - 1.0).normalize_or_zero()
                            * radius
                            * unit();
                    let c = to_clip * p.extend(1.0);
                    let px =
                        Vec2::new((c.x / c.w * 0.5 + 0.5) * size.0 as f32, (0.5 - c.y / c.w * 0.5) * size.1 as f32);
                    // The output pixels whose cross can hold this input pixel.
                    for dx in [-2.0f32, 2.0] {
                        let o = (px.x - jitter.x + dx).floor() as i32;
                        assert!(o >= rect[0] && o <= rect[2], "x {o} outside {rect:?} (turn {turn}, {distance} m)");
                        let o = (px.y - jitter.y + dx).floor() as i32;
                        assert!(o >= rect[1] && o <= rect[3], "y {o} outside {rect:?} (turn {turn}, {distance} m)");
                    }
                }
            }
            // Around the eye, and behind it.
            assert_eq!(car_rect(to_clip, Vec3::ZERO, 2.0, size, size, jitter)[0], i32::MIN / 2);
            assert_eq!(car_rect(to_clip, -cam.forward() * 10.0, 2.0, size, size, jitter)[2], i32::MAX / 2);
        }
    }

    /// On this machine's adapter (integrated or discrete) the compute pass
    /// gives the fragment pass's history byte for byte (the self-check).
    #[test]
    #[ignore = "requires a GPU; run explicitly for the temporal resolve"]
    fn the_compute_pass_gives_the_fragment_pass_history() {
        let gpu = pollster::block_on(Gpu::new(None, wgpu::Instance::default())).unwrap();
        let t = super::super::Temporal::new(&gpu);
        let c = Compute::checked(&gpu, &t.resolve, &t.layout, &t.sampler);
        assert!(c.is_some(), "the self-check found the two passes differ on this adapter");
    }
}
