//! HDR Output's grade LUT (moved from render/postfx.rs).

use crate::render::Gpu;
use crate::render::native::post::*;

/// The grade (screenfilter.efx's three passes) as a [`LUT_N`]³ LUT: a strip
/// of `LUT_N`² × `LUT_N` texels, texel (r + `LUT_N`·b, g) the native
/// grade of the 8-bit colour ([`lattice_code`] of r, g, b), made by the
/// ported native passes on the identity strip (8-bit targets between them,
/// as native), for the frame values in `key`.
pub(crate) struct GradeLut {
    identity: wgpu::TextureView,
    tmp: [wgpu::TextureView; 2],
    strip_texture: wgpu::Texture,
    strip: wgpu::TextureView,
    uniform: wgpu::Buffer,
    key: Option<[f32; 5]>,
}

/// The grade LUT's size per axis (postfx.wgsl `LUT_N`).
pub const LUT_N: u32 = 32;

/// The 8-bit code of lattice point `i` (postfx.wgsl `lattice_code`): the
/// lattice sits on codes, so the LUT is the native grade exactly there.
pub fn lattice_code(i: u32) -> u8 {
    ((i as f32 * 255.0 / (LUT_N - 1) as f32) + 0.5).floor() as u8
}

/// The identity strip's texels (RGBA8).
pub fn identity_strip() -> Vec<u8> {
    let mut v = Vec::with_capacity((LUT_N * LUT_N * LUT_N * 4) as usize);
    for y in 0..LUT_N {
        for x in 0..LUT_N * LUT_N {
            v.extend_from_slice(&[lattice_code(x % LUT_N), lattice_code(y), lattice_code(x / LUT_N), 255]);
        }
    }
    v
}

/// How [`PostFx::grade_image`] grades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradeMode {
    /// The ported native passes (three, 8-bit hand-offs).
    Native,
    /// The LUT, tetrahedral interpolation.
    LutTetra,
    /// The LUT, trilinear (what HDR Output uses).
    LutTrilinear,
}

impl PostFx {
    /// The bleach passes' uniform for `frame` (one tap, TINTCOLOR.w,
    /// TINTCOLOR.rgb and SATURATIONSCALE).
    fn grade_uniform(frame: &Frame) -> PassUniform {
        let [r, g, b] = frame.tint_color();
        let mut u = PassUniform {
            weights: [[0.0; 4]; 4],
            offsets: [[0.0; 4]; 4],
            params: [1.0, frame.desaturate, 0.0, 0.0],
            grey: OVERBRIGHT_GREY,
            tint: [r, g, b, SATURATION_SCALE],
        };
        u.weights[0] = [1.0; 4];
        u
    }

    /// The grade LUT's textures, made once.
    fn ensure_grade(&mut self, gpu: &Gpu) {
        if self.grade.is_some() {
            return;
        }
        let size = (LUT_N * LUT_N, LUT_N);
        let identity = texture(gpu, "grade lut identity", size, INTERMEDIATE);
        write_texture(gpu, &identity, &identity_strip(), size.0, size.1);
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let tmp = [
            view(&texture(gpu, "grade lut 0", size, INTERMEDIATE)),
            view(&texture(gpu, "grade lut 1", size, INTERMEDIATE)),
        ];
        let strip_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grade lut"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: INTERMEDIATE,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let strip = view(&strip_texture);
        let uniform = crate::render::zeroed_buffer(
            &gpu.device,
            &wgpu::BufferDescriptor {
                label: Some("grade lut"),
                size: SLOT,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        self.grade = Some(GradeLut { identity: view(&identity), tmp, strip_texture, strip, uniform, key: None });
    }

    /// Records the grade LUT for `frame` (the native passes on the identity
    /// strip with `gain`, the gain map of `frame.bleach`), when its values
    /// changed. Returns the strip's view.
    pub(crate) fn record_lut(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        gain: &wgpu::TextureView,
    ) -> wgpu::TextureView {
        self.ensure_grade(gpu);
        let key = [frame.bleach, frame.desaturate, frame.tint[0], frame.tint[1], frame.tint[2]];
        let g = self.grade.as_ref().unwrap();
        if g.key == Some(key) {
            return g.strip.clone();
        }
        gpu.queue.write_buffer(&g.uniform, 0, bytemuck::bytes_of(&Self::grade_uniform(frame)));
        let (identity, t0, t1, strip, uniform) =
            (g.identity.clone(), g.tmp[0].clone(), g.tmp[1].clone(), g.strip.clone(), g.uniform.clone());
        let ramp = self.ramp.create_view(&Default::default());
        let b0 = self.pipeline(gpu, "fs_bleach0", INTERMEDIATE, Blend::Replace);
        let b1 = self.pipeline(gpu, "fs_bleach1", INTERMEDIATE, Blend::Replace);
        let b2 = self.pipeline(gpu, "fs_bleach2", INTERMEDIATE, Blend::Replace);
        let passes = [(b0, &identity, &t0), (b1, &t0, &t1), (b2, &t1, &strip)];
        for (p, src, dst) in passes {
            let group = self.group_of(gpu, src, gain, &ramp, &uniform);
            full_pass_op(encoder, "grade lut", &self.pipelines[p].1, &[dst], &group, 0, None, replace_load());
        }
        self.grade.as_mut().unwrap().key = Some(key);
        strip
    }

    /// The grade LUT for `frame` as RGBA8 texels (strip order; for the
    /// evidence's .cube file and the tests).
    pub fn grade_lut_texels(&mut self, gpu: &Gpu, frame: Frame) -> Vec<u8> {
        let gain = texture(gpu, "grade lut gain", (GAIN_SIZE as u32, GAIN_SIZE as u32), INTERMEDIATE);
        write_texture(gpu, &gain, &gain_map(frame.bleach), GAIN_SIZE as u32, GAIN_SIZE as u32);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.ensure_grade(gpu);
        self.grade.as_mut().unwrap().key = None;
        self.record_lut(gpu, &mut encoder, &frame, &gain.create_view(&Default::default()));
        self.grade.as_mut().unwrap().key = None;
        gpu.queue.submit([encoder.finish()]);
        let g = self.grade.as_ref().unwrap();
        crate::render::read_back(gpu, &g.strip_texture, LUT_N * LUT_N, LUT_N)
    }

    /// Grades an RGBA8 image (`size`) with `frame`'s values by `mode` (the
    /// gain map `gain` when given, as an original's trace holds it; else
    /// [`gain_map`] of `frame.bleach`) and reads it back. For the LUT's
    /// verification (examples/grade_lut.rs, tests).
    pub fn grade_image(
        &mut self,
        gpu: &Gpu,
        frame: Frame,
        gain: Option<&[u8]>,
        rgba: &[u8],
        size: (u32, u32),
        mode: GradeMode,
    ) -> Vec<u8> {
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let gain_tex = texture(gpu, "grade gain", (GAIN_SIZE as u32, GAIN_SIZE as u32), INTERMEDIATE);
        let owned;
        let gain_bytes = match gain {
            Some(g) => g,
            None => {
                owned = gain_map(frame.bleach);
                &owned
            }
        };
        write_texture(gpu, &gain_tex, gain_bytes, GAIN_SIZE as u32, GAIN_SIZE as u32);
        let gain_view = view(&gain_tex);
        let src = texture(gpu, "grade source", size, INTERMEDIATE);
        write_texture(gpu, &src, rgba, size.0, size.1);
        let out = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grade out"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: INTERMEDIATE,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let uniform = crate::render::zeroed_buffer(
            &gpu.device,
            &wgpu::BufferDescriptor {
                label: Some("grade image"),
                size: SLOT,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        gpu.queue.write_buffer(&uniform, 0, bytemuck::bytes_of(&Self::grade_uniform(&frame)));
        let ramp = self.ramp.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let (vsrc, vout) = (view(&src), view(&out));
        match mode {
            GradeMode::Native => {
                let t0 = view(&texture(gpu, "grade 0", size, INTERMEDIATE));
                let t1 = view(&texture(gpu, "grade 1", size, INTERMEDIATE));
                let b0 = self.pipeline(gpu, "fs_bleach0", INTERMEDIATE, Blend::Replace);
                let b1 = self.pipeline(gpu, "fs_bleach1", INTERMEDIATE, Blend::Replace);
                let b2 = self.pipeline(gpu, "fs_bleach2", INTERMEDIATE, Blend::Replace);
                for (p, a, b) in [(b0, &vsrc, &t0), (b1, &t0, &t1), (b2, &t1, &vout)] {
                    let group = self.group_of(gpu, a, &gain_view, &ramp, &uniform);
                    full_pass_op(
                        &mut encoder,
                        "grade (native)",
                        &self.pipelines[p].1,
                        &[b],
                        &group,
                        0,
                        None,
                        replace_load(),
                    );
                }
            }
            GradeMode::LutTetra | GradeMode::LutTrilinear => {
                self.ensure_grade(gpu);
                self.grade.as_mut().unwrap().key = None;
                let strip = self.record_lut(gpu, &mut encoder, &frame, &gain_view);
                self.grade.as_mut().unwrap().key = None;
                let p = self.pipeline_v(
                    gpu,
                    "fs_grade_lut_check",
                    INTERMEDIATE,
                    1,
                    Blend::Replace,
                    if mode == GradeMode::LutTetra { 3 } else { 0 },
                );
                let group = self.group_of(gpu, &vsrc, &gain_view, &strip, &uniform);
                full_pass_op(
                    &mut encoder,
                    "grade (lut)",
                    &self.pipelines[p].1,
                    &[&vout],
                    &group,
                    0,
                    None,
                    replace_load(),
                );
            }
        }
        gpu.queue.submit([encoder.finish()]);
        crate::render::read_back(gpu, &out, size.0, size.1)
    }

    /// The last pass's two-target pipeline with HDR Output (the SDR image
    /// in the output's format, the unclipped one in half float).
    pub(crate) fn pipeline_hdr_out(&mut self, gpu: &Gpu, entry: &'static str) -> usize {
        let key = (entry, self.format, Blend::Replace as u8, 9, 1);
        if let Some(i) = self.pipelines.iter().position(|(k, _)| *k == key) {
            return i;
        }
        let targets = [
            Some(wgpu::ColorTargetState { format: self.format, blend: None, write_mask: wgpu::ColorWrites::ALL }),
            Some(wgpu::ColorTargetState {
                format: crate::render::HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];
        let p = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        self.pipelines.push((key, p));
        self.pipelines.len() - 1
    }

    /// HDR Output: the post passes' unclipped image (half float, the
    /// scene's size), when this frame's [`PostFx::apply`] wrote it.
    pub fn take_unclipped(&mut self) -> Option<wgpu::TextureView> {
        if !std::mem::take(&mut self.unclipped_fresh) {
            return None;
        }
        self.unclipped.as_ref().map(|(_, v)| v.clone())
    }
}
