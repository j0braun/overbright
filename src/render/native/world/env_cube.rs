//! The cars' environment cube: planning and recording its faces (moved from render/mod.rs).

use crate::render::*;

/// The cube's faces as [`WorldRenderer::plan_environment`] set them up.
pub(in crate::render) struct EnvPlan {
    pub(in crate::render) visible: Vec<bool>,
    /// What the six faces see together (the cube about the eye).
    pub(in crate::render) cube: PassCull,
    /// The faces drawn this frame and their culling.
    pub(in crate::render) faces: Vec<(usize, PassCull)>,
    /// The faces only cleared (no world in the cube).
    pub(in crate::render) cleared: Vec<usize>,
    /// All six faces in one multiview pass ([`EnvMultiview`]).
    multiview: bool,
    time: f32,
    reversed: bool,
    /// Snapshot shared by cube-wide, per-face and multiview culling.
    pub(in crate::render) lod: u8,
}

impl WorldCore {
    pub(in crate::render) fn record_environment(&self, encoder: &mut wgpu::CommandEncoder, plan: &EnvPlan) {
        for &i in &plan.cleared {
            clear_target(encoder, &self.env.faces[i], &self.env.depth);
        }
        if !plan.faces.is_empty() {
            let groups = self.kept_groups(
                &plan.visible,
                &plan.cube,
                WorldView::Environment,
                plan.reversed,
                false,
                None,
                plan.lod,
            );
            if let (true, Some(m)) = (plan.multiview, &self.env_multiview) {
                // The faces' kept groups together.
                let mut kept: Vec<u32> = plan
                    .faces
                    .iter()
                    .flat_map(|(_, cull)| {
                        self.kept_groups(
                            &plan.visible,
                            cull,
                            WorldView::Environment,
                            plan.reversed,
                            false,
                            Some(&groups),
                            plan.lod,
                        )
                    })
                    .collect();
                kept.sort_unstable();
                kept.dedup();
                let order = self.group_order(&kept);
                self.draw_pass_into(
                    encoder,
                    &m.color,
                    &m.depth,
                    wgpu::Color::BLACK,
                    &m.frames.1,
                    &self.no_reflection,
                    &order,
                    plan.time,
                    None,
                    WorldView::Environment,
                    (0, 0),
                    Some(&m.pipelines),
                );
                crate::prof::gpu::mark(encoder, "world: environment cube");
                return;
            }
            for (i, cull) in &plan.faces {
                let order = self.group_order(&self.kept_groups(
                    &plan.visible,
                    cull,
                    WorldView::Environment,
                    plan.reversed,
                    false,
                    Some(&groups),
                    plan.lod,
                ));
                self.draw_pass(
                    encoder,
                    &self.env.faces[*i],
                    &self.env.depth,
                    wgpu::Color::BLACK,
                    &self.env_frames[*i].1,
                    &self.no_reflection,
                    &order,
                    plan.time,
                    None,
                    WorldView::Environment,
                    (0, 0),
                );
            }
        }
        crate::prof::gpu::mark(encoder, "world: environment cube");
    }
}

impl WorldRenderer {
    /// The cars' environment cube at the Graphics page's Car Reflection
    /// Res (native 128), remade when it changed; true when it did (the
    /// cars then bind the new [`WorldRenderer::environment`]). Left for a
    /// later frame while a render thread still holds the views.
    pub fn sync_environment_size(&mut self, gpu: &Gpu, opts: &RenderOptions) -> bool {
        let size = opts.car_reflection_size.max(ENV_SIZE);
        if self.core.env.size == size {
            return false;
        }
        let Some(core) = std::sync::Arc::get_mut(&mut self.core) else { return false };
        core.env = EnvCube::new(gpu, size);
        if let Some(m) = core.env_multiview.as_mut() {
            let array = wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            };
            m.color = core.env.texture.create_view(&array);
            m.depth = env_multiview_depth(gpu, size).create_view(&array);
        }
        core.env_black.store(false, std::sync::atomic::Ordering::Relaxed);
        self.environment_enhanced = None;
        self.env_tick = None;
        log::info!("environment cube remade at {size} (Car Reflection Res)");
        true
    }

    /// The cube the cars reflect (views 0x10–0x15, rendered by 0x5CAAE0
    /// each frame at the top update setting): six 90° views from the
    /// player car's position + 0.95 m up (0x61AB60), oriented by the main
    /// camera (`right`, `down`, `forward` as the game's look-at builds
    /// them), near 0.5 and far 64 (0x7FEE88 / 0x7FEE8C), cleared to black
    /// (0x5BA3D0), then the world.
    ///
    /// `faces`: the faces rendered this frame (the rest keep their last
    /// image; [`display::env_faces`]); `world`: g_CarEnvironmentMapUpdateData
    // prov: open render.plan_wet_mask
    /// (0x5CAD86), without which a face is only cleared (**open**: the
    /// cube's own sky, 0x60F890 with solid 0x8A1F10 at × 0.004525 in views
    /// 0x10–0x15, which the clone does not draw).
    #[allow(clippy::too_many_arguments)]
    pub fn render_environment(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        centre: Vec3,
        axes: (Vec3, Vec3, Vec3),
        params: &FrameParams,
        faces: [bool; 6],
        world: bool,
    ) {
        let plan = self.plan_environment(gpu, centre, axes, params, faces, world);
        self.core.record_environment(encoder, &plan);
    }

    /// [`WorldRenderer::render_environment`] into a command buffer of its
    /// own, recorded and encoded on a render thread.
    #[allow(clippy::too_many_arguments)]
    pub fn render_environment_async(
        &mut self,
        gpu: &Gpu,
        centre: Vec3,
        axes: (Vec3, Vec3, Vec3),
        params: &FrameParams,
        faces: [bool; 6],
        world: bool,
    ) -> Finishing {
        let plan = self.plan_environment(gpu, centre, axes, params, faces, world);
        let core = self.core.clone();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        crate::prof::gpu::mark(&mut encoder, crate::prof::gpu::GAP);
        Finishing::spawn(move || {
            core.record_environment(&mut encoder, &plan);
            encoder.finish()
        })
    }

    /// The cube's faces set up: their uniforms written.
    pub(in crate::render) fn plan_environment(
        &mut self,
        gpu: &Gpu,
        centre: Vec3,
        axes: (Vec3, Vec3, Vec3),
        params: &FrameParams,
        faces: [bool; 6],
        world: bool,
    ) -> EnvPlan {
        self.core.env_black.store(false, std::sync::atomic::Ordering::Relaxed);
        self.core.sky_on.store(sky_ng::enabled(params.opts), std::sync::atomic::Ordering::Relaxed);
        let enhanced = params.opts.enhanced_reflections;
        // Retained faces must have one coordinate frame. Enhanced captures use
        // world XYZ, independent of camera yaw/pitch/roll. Switching either way
        // replaces every face before a shader can sample the new basis.
        let changed = self.environment_enhanced.is_some_and(|old| old != enhanced);
        let faces = if changed || (enhanced && self.environment_enhanced.is_none()) { [true; 6] } else { faces };
        self.environment_enhanced = Some(enhanced);
        let axes = if enhanced { (Vec3::X, Vec3::Y, Vec3::Z) } else { axes };
        let visible = visible_sections(params);
        let main_lod = if params.display.world_lod_b { MAIN_LOD_B } else { MAIN_LOD };
        let (lod, min_size) = reflection_capture_quality(enhanced, main_lod, detail_min_size(params.opts));
        let fog = params.fog.unwrap_or(NO_FOG);
        let projection = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.5, ENV_FAR);
        // The six faces together see the cube of ±ENV_FAR about the eye in
        // the camera's axes: the groups reaching into it, tested again by
        // each face. (No size test: a size of 0 is always met.)
        let planes =
            [axes.0, -axes.0, axes.1, -axes.1, axes.2, -axes.2].map(|n| n.extend(ENV_FAR - n.dot(centre))).to_vec();
        let cube =
            PassCull { planes, mirrored: false, eye: centre, forward: axes.2, scale: 0.0, min_size: 0.0, fades: None };
        let mut plan = EnvPlan {
            visible,
            cube,
            faces: Vec::new(),
            cleared: Vec::new(),
            multiview: false,
            time: params.time,
            reversed: params.reversed,
            lod,
        };
        let mut uniforms = Vec::with_capacity(6);
        for (i, (r, d, f)) in env_face_axes(axes.0, axes.1, axes.2).into_iter().enumerate() {
            if !faces[i] {
                continue;
            }
            if !world {
                plan.cleared.push(i);
                continue;
            }
            // Rows right, up, back: a mirrored view, as cube faces are.
            let rotation =
                Mat4::from_cols(r.extend(0.0), (-d).extend(0.0), (-f).extend(0.0), glam::Vec4::W).transpose();
            let view = rotation * Mat4::from_translation(-centre);
            let view_proj = projection * view;
            let uniform = FrameUniform {
                view_proj: view_proj.to_cols_array_2d(),
                camera: centre.extend(1.0).to_array(),
                forward: f.extend(0.0).to_array(),
                fog_value: [fog.value[0], fog.value[1], fog.value[2], 0.0],
                fog_color: [fog.color[0], fog.color[1], fog.color[2], 1.0],
                clip: [f32::MIN, params.time, 0.0, 0.0],
                // 90° faces: 256 / tan(45°).
                cull: [cull_scale(std::f32::consts::FRAC_PI_2), min_size, 0.0, 0.0],
                headlight: [[0.0; 4]; 5],
                wet: [0.0; 4],
                lamps: [0.0; 4],
                cluster: [0.0; 4],
                cluster_near: [0.0; 4],
            };
            uniforms.push((i, uniform));
            plan.faces.push((
                i,
                PassCull {
                    planes: frustum_planes(view_proj),
                    mirrored: false,
                    eye: centre,
                    forward: f,
                    scale: cull_scale(std::f32::consts::FRAC_PI_2),
                    min_size,
                    fades: None,
                },
            ));
        }
        // Every face this frame, with the world: one pass (one write of the
        // six faces' values); else a pass and a write per face.
        // With Sky Next-Gen the faces go one by one (the sky is drawn per face; the multiview pass has
        // no sky pipeline), so no face ever lacks it.
        match self.core.env_multiview.as_ref().filter(|_| !sky_ng::enabled(params.opts)) {
            Some(m) if uniforms.len() == 6 => {
                let all: Vec<FrameUniform> = uniforms.iter().map(|u| u.1).collect();
                gpu.queue.write_buffer(&m.frames.0, 0, bytemuck::cast_slice(&all));
                plan.multiview = true;
            }
            _ => {
                for (i, uniform) in &uniforms {
                    gpu.queue.write_buffer(&self.env_frames[*i].0, 0, bytemuck::bytes_of(uniform));
                }
            }
        }
        plan
    }
}

// Refactor: from render/mod.rs
/// The cars' environment map in game (0x5BA4C0's render-target cube
/// 0x870858): 128 texels a side (0x7FEE6C), with a depth buffer per face.
pub struct EnvCube {
    /// Its edge: ENV_SIZE, or the Graphics page's Car Reflection Res.
    pub(in crate::render) size: u32,
    pub(in crate::render) texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub(in crate::render) faces: Vec<wgpu::TextureView>,
    pub(in crate::render) depth: wgpu::TextureView,
}

/// Edge of the in-game cube (0x7FEE6C).
pub(in crate::render) const ENV_SIZE: u32 = 128;
/// The cube faces' far plane (0x7FEE8C).
pub(in crate::render) const ENV_FAR: f32 = 64.0;

impl EnvCube {
    pub(in crate::render) fn new(gpu: &Gpu, size: u32) -> EnvCube {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("environment cube"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: REFLECTION_FORMAT,
            // (The developer harness reads the faces back: the tick-stepped cube's
            // digest.)
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | if cfg!(feature = "dev-harness") { wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() },
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::Cube), ..Default::default() });
        let faces = (0..6)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        EnvCube { size, texture, view, faces, depth: transient_depth(gpu, size, size) }
    }
}

/// The multiview pass's depth layers for a cube `size` big.
pub(in crate::render) fn env_multiview_depth(gpu: &Gpu, size: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("environment depth"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        // (Not TRANSIENT_ATTACHMENT: wgpu takes that for one layer
        // only; the pass still discards it.)
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

/// The cube faces' cameras as 0x5C3460 sets views 0x10–0x15, in the main
/// camera's right, down and forward axes (R, D, F): per face its right,
/// down and forward — the standard cube faces of that frame, so car.efx
/// samples the map with the reflection vector in camera space.
pub(in crate::render) fn env_face_axes(r: Vec3, d: Vec3, f: Vec3) -> [(Vec3, Vec3, Vec3); 6] {
    [(-f, -d, r), (f, -d, -r), (r, f, d), (r, -f, -d), (r, -d, f), (-r, -d, -f)]
}

impl WorldRenderer {
    /// Clear every face to black: the cube as the cars see it when
    /// g_CarEnvironmentMapEnable is 0 (no cube, a null EnvMapTexture).
    pub fn clear_environment(&self, encoder: &mut wgpu::CommandEncoder) {
        // Once black, it stays so until a face is drawn (environment maps
        // off: six passes a frame for nothing).
        if self.env_black.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        for face in &self.env.faces {
            clear_target(encoder, face, &self.env.depth);
        }
    }

    /// The environment cube to bind for the cars.
    pub fn environment(&self) -> &wgpu::TextureView {
        &self.env.view
    }

    /// Developer harness: a digest of the six faces' pixels as the
    /// GPU holds them after everything submitted so far (xxh3 of the
    /// faces in order), to compare the cube the frames of two display
    /// rates sample at the same tick.
    #[cfg(feature = "dev-harness")]
    pub fn environment_digest(&self, gpu: &Gpu) -> u64 {
        let size = self.env.size;
        let row = (size * 4).div_ceil(256) * 256;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("environment digest"),
            size: u64::from(row * size * 6),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.env.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size) },
            },
            wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
        );
        gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let mapped = slice.get_mapped_range().expect("mapped environment");
        let mut h = xxhash_rust::xxh3::Xxh3::new();
        for line in mapped.chunks(row as usize) {
            h.update(&line[..(size * 4) as usize]);
        }
        h.digest()
    }
}

/// The cube's faces in one multiview pass where the device has
/// `Features::MULTIVIEW` with six views (else, or with
/// `NFSU2_NO_MULTIVIEW`, a pass per face): drawn when the frame redraws all
/// six faces (the top cube rate). The draws are the faces' kept groups
/// together, in the draw order; a draw a face's culling left out puts no
/// pixel in that face, so each face gets the same image as its own pass.
pub(in crate::render) struct EnvMultiview {
    /// The six faces' frame values (a `Frame` each) and group 0.
    pub(in crate::render) frames: (wgpu::Buffer, wgpu::BindGroup),
    /// The cube's six layers, and a depth layer each.
    pub(in crate::render) color: wgpu::TextureView,
    pub(in crate::render) depth: wgpu::TextureView,
    pub(in crate::render) pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

/// A pass that only clears `color` to opaque black and `depth` to 1.
/// A cube face cleared (its depth, transient, is not kept).
pub(in crate::render) fn clear_target(encoder: &mut wgpu::CommandEncoder, color: &wgpu::TextureView, depth: &wgpu::TextureView) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}
